//! 公開 IFC の階ごとの平面図に作図の慣習を当て、`convert` と同じ書き手で新しい DXF に書く（#397）。
//!
//! 用紙の mm の値は、図面のビューの縮尺で実寸に直してから書く。書き手が表せない形は、元の建物の要素を指す
//! 出所付きの診断で確かめる。
#![cfg(feature = "building-drawing")]
#![allow(clippy::unwrap_used, reason = "固定した試験入力")]
#[path = "support/drawing.rs"]
mod support;
use support::{building, cut, grid, profile};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{Building, SpatialKind};
use zumen_crs::ProjectPoint;
use zumen_drawing::{Boundary, Drawing2D, Length, PatternBasis, Shape, Target};
use zumen_ifc::Ifc;
use zumen_profile::building_drawing::{Drawing, DrawingProfile};
use zumen_sdk::{Codec, Lift};

fn limits() -> Limits {
    Limits::default()
        .with(Kind::DecodedBytes, Limit::Finite(16 * 1024 * 1024 * 1024))
        .with(Kind::Work, Limit::Finite(100_000_000_000))
}

fn run<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> T {
    let mut budget = Budget::new(limits());
    let (mut diagnostics, cancel) = (Diagnostics::new(), CancelToken::new());
    f(&mut Cx::new(&mut budget, &mut diagnostics, &cancel))
}

fn read(bytes: &[u8]) -> Building {
    run(|cx| Ifc::read(bytes, cx).unwrap().lift_geometry(cx).unwrap())
}

/// IFC の階の高さ（文書の単位）。高さを持たない階は切らない。
fn storeys(building: &Building) -> Vec<(Option<u64>, f64)> {
    building
        .components
        .spatial
        .iter()
        .filter(|(_, spatial)| spatial.kind == SpatialKind::Storey)
        .filter_map(|(at, spatial)| {
            let z = spatial.elevation.as_ref()?.value;
            Some((building.element(at)?.source().instance, z))
        })
        .collect()
}

fn plan(profile: &DrawingProfile, building: &Building, z: f64) -> Drawing {
    run(|cx| {
        let view = profile
            .default_plan(building, ProjectPoint { x: 0.0, y: 0.0, z }, cx)
            .unwrap();
        profile
            .cut(building, view, 0.01, 1.0, None, cx)
            .unwrap()
            .at_view_scale(cx)
            .unwrap()
    })
}

fn scaled(drawing: &Drawing) -> Drawing {
    run(|cx| drawing.at_view_scale(cx)).unwrap()
}

fn lower(drawing: &Drawing) -> (zumen_acad::Lowered, Diagnostics) {
    let mut budget = Budget::new(limits());
    let (mut diagnostics, cancel) = (Diagnostics::with_max_places(1 << 20), CancelToken::new());
    let lowered = zumen_acad::lower_to_dxf(
        &drawing.output.drawing,
        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
    )
    .unwrap();
    (lowered, diagnostics)
}

fn reread(bytes: &[u8]) -> Drawing2D {
    run(|cx| zumen_acad::Dxf::read(bytes, cx).unwrap().lift(cx).unwrap())
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn storey_plans_of_public_samples_write_to_dxf() {
    let profile = profile();
    for (bytes, expected, grids) in [
        (
            &include_bytes!("../../../samples/ifc/text2bim/sample_model.ifc")[..],
            2,
            false,
        ),
        (
            &include_bytes!("../../../samples/ifc/gni/model_9.ifc")[..],
            5,
            true,
        ),
    ] {
        let building = read(bytes);
        let storeys = storeys(&building);
        assert_eq!(storeys.len(), expected);
        for (storey, z) in storeys {
            let drawing = plan(&profile, &building, z);
            let elements = drawing.output.drawing.spaces[0].elements.len();
            let (lowered, diagnostics) = lower(&drawing);
            // 線・通り芯の円と文字を、どれも落とさずに書く。
            let dropped: Vec<_> = diagnostics
                .iter()
                .filter(|d| d.code() == codes::CONVERT_ELEMENT_UNSUPPORTED)
                .map(|d| d.params().to_vec())
                .collect();
            assert!(dropped.is_empty(), "{storey:?}: {dropped:?}");
            assert_eq!(lowered.elements, elements as u64, "{storey:?}");
            assert_eq!(lower(&drawing).0.bytes, lowered.bytes, "{storey:?}");

            let written = reread(&lowered.bytes);
            let model = written.spaces.first().unwrap();
            assert_eq!(model.elements.len(), elements, "{storey:?}");
            for name in ["A-CUT", "A-VISIBLE", "A-HIDDEN"] {
                assert!(written.layers.iter().any(|l| l.name == name), "{name}");
            }
            // 紙上 3 mm・間 1.5 mm の破線は、1/100 の図面で実寸 300 mm・-150 mm になる。
            let unit = model.unit.millimeters().unwrap();
            let dashed = written
                .line_types
                .iter()
                .find(|t| t.name.eq_ignore_ascii_case("dashed"))
                .and_then(|t| t.definition.as_ref())
                .unwrap();
            assert_eq!(dashed.basis, PatternBasis::Element);
            let pattern: Vec<f64> = dashed.pattern.iter().map(|v| v * unit).collect();
            assert!(
                matches!(pattern.as_slice(), [a, b] if near(*a, 300.0) && near(*b, -150.0)),
                "{pattern:?}"
            );
            // 通り芯の円は紙上の半径 4 mm、文字は高さ 4 mm を、実寸 400 mm で書く。
            let circles: Vec<f64> = model
                .elements
                .iter()
                .filter_map(|e| match e.shape {
                    Shape::Circle {
                        radius: Length::Model(r),
                        ..
                    } => Some(r.value * unit),
                    _ => None,
                })
                .collect();
            assert_eq!(!circles.is_empty(), grids, "{storey:?}");
            assert!(circles.iter().all(|r| near(*r, 400.0)), "{circles:?}");
            let labels = model
                .elements
                .iter()
                .filter(|e| match &e.shape {
                    Shape::Text(t) => {
                        matches!(t.height.value, Length::Model(h) if near(h.value * unit, 400.0))
                    }
                    _ => false,
                })
                .count();
            assert_eq!(labels, circles.len(), "{storey:?}");
        }
    }
}

#[test]
fn view_scale_keeps_lines_ids_and_decoration_records() {
    let mut b = building();
    grid(&mut b);
    let base = cut(&profile(), &b);
    let scaled = scaled(&base);
    assert_eq!(base.output.lines, scaled.output.lines);
    assert_eq!(base.decorations, scaled.decorations);
    assert!(scaled.decorations.iter().any(|d| d.kind == "grid-circle"));
    let drawing = &scaled.output.drawing;
    for decoration in &scaled.decorations {
        let target = Target::Element(decoration.drawing_id.clone());
        let extension = drawing
            .extensions
            .iter()
            .find(|e| e.name == "decoration" && e.target == target)
            .unwrap();
        assert!(extension.is_current(drawing), "{}", decoration.id);
    }
    // 元の線の形と記録は変えない。
    let (lines, before) = (
        &drawing.spaces.first().unwrap().elements,
        &base.output.drawing.spaces.first().unwrap().elements,
    );
    for line in &scaled.output.lines {
        assert_eq!(lines.get(line.index), before.get(line.index));
        assert!(
            drawing
                .extensions
                .get(line.index)
                .unwrap()
                .is_current(drawing)
        );
    }
    // 用紙の mm の線種は、実寸の列にする。
    assert!(drawing.line_types.iter().all(|t| {
        t.definition
            .as_ref()
            .is_none_or(|d| d.basis == PatternBasis::Element)
    }));
    // 予算で止めたら、部分結果を返さない。
    let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(1)));
    let (mut diagnostics, cancel) = (Diagnostics::new(), CancelToken::new());
    assert!(
        base.at_view_scale(&mut Cx::new(&mut budget, &mut diagnostics, &cancel))
            .is_err()
    );
}

#[test]
fn hatches_of_building_elements_write_to_dxf() {
    let b = building();
    let drawing = scaled(&cut(&profile(), &b));
    let hatches: Vec<_> = drawing
        .decorations
        .iter()
        .filter(|d| d.kind == "hatch")
        .collect();
    assert!(!hatches.is_empty());
    for hatch in &hatches {
        let element = b.id(hatch.element).unwrap().to_string();
        assert!(
            hatch
                .id
                .starts_with(&format!("building/{element}/profile/hatch/"))
        );
    }
    let (lowered, diagnostics) = lower(&drawing);
    // 書き手はハッチングを境界と模様の値のまま足す（#684）。落とす要素はない。
    assert!(
        diagnostics
            .iter()
            .all(|d| d.code() != codes::CONVERT_ELEMENT_UNSUPPORTED)
    );
    let elements = &drawing.output.drawing.spaces.first().unwrap().elements;
    assert_eq!(lowered.elements, elements.len() as u64);
    let written: Vec<_> = elements
        .iter()
        .filter_map(|element| match &element.shape {
            Shape::Hatch(hatch) => Some(hatch),
            _ => None,
        })
        .collect();
    assert_eq!(written.len(), hatches.len());
    let back = reread(&lowered.bytes);
    let read: Vec<_> = back.spaces[0]
        .elements
        .iter()
        .filter_map(|element| match &element.shape {
            Shape::Hatch(hatch) => Some(hatch),
            _ => None,
        })
        .collect();
    assert_eq!(read.len(), written.len());
    for (read, written) in read.iter().zip(&written) {
        assert_eq!(read.fill, written.fill);
        assert_eq!(read.islands, written.islands);
        assert_eq!(read.loops.len(), written.loops.len());
        for (read, written) in read.loops.iter().zip(&written.loops) {
            // 境界を探した印は書かない（明細の reasons の hatch_derived_loop）。形は同じ。
            assert!(!read.derived);
            let (
                Boundary::Polyline {
                    vertices: read,
                    closed: true,
                },
                Boundary::Polyline {
                    vertices: written,
                    closed: true,
                },
            ) = (&read.boundary, &written.boundary)
            else {
                panic!("{:?}", read.boundary)
            };
            assert_eq!(read.len(), written.len());
            for (read, written) in read.iter().zip(written) {
                assert!(near(read.point.x, written.point.x));
                assert!(near(read.point.y, written.point.y));
                assert!(near(read.bulge, written.bulge));
            }
        }
    }
}
