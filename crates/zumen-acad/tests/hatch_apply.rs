//! 非連動のハッチングを追加し、既存の図形を保ったまま再読する。
#![cfg(feature = "drawing")]

use std::path::Path;
use zumen_acad::{Dwg, Dxf};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Ref};
use zumen_drawing::{
    Applied, Boundary, Changes, Drawing2D, Edge, Edit, Element, Fill, Hash, Hatch, HatchLoop,
    Islands, Length, ModelLength, ModelPoint, Pattern, PatternLine, PatternOrigin, Placed, Shape,
    SpaceId, Vertex, id,
};
use zumen_sdk::{Apply, Codec, FileModel, Lift, testkit};

fn fixture(space: SpaceId, pattern: bool) -> Hatch<ModelPoint> {
    let point = |x, y| ModelPoint { space, x, y };
    let path = |boundary, external| HatchLoop {
        boundary,
        external,
        outermost: external,
        derived: false,
        text_box: false,
        sources: Vec::new(),
    };
    Hatch {
        loops: vec![
            path(
                Boundary::Polyline {
                    vertices: [
                        (10.0, 20.0, 0.0),
                        (50.0, 20.0, 0.5),
                        (50.0, 60.0, 0.0),
                        (10.0, 60.0, 0.0),
                    ]
                    .into_iter()
                    .map(|(x, y, bulge)| Vertex {
                        point: point(x, y),
                        bulge,
                    })
                    .collect(),
                    closed: true,
                },
                true,
            ),
            path(
                Boundary::Edges(vec![Edge::Arc {
                    center: point(30.0, 40.0),
                    radius: Length::Model(ModelLength { space, value: 3.0 }),
                    start: 0.0,
                    end: 360.0,
                    counterclockwise: true,
                }]),
                false,
            ),
        ],
        islands: Islands::Nested,
        fill: if pattern {
            Fill::Pattern(Pattern {
                name: "ZUMEN_EXPLICIT".into(),
                origin: PatternOrigin::Custom,
                angle: 90.0,
                scale: 2.0,
                double: true,
                lines: vec![PatternLine {
                    angle: 45.0,
                    base: [1.0, 2.0],
                    offset: [-2.0, 2.0],
                    dashes: vec![2.0, -1.0, 0.0],
                }],
            })
        } else {
            Fill::Solid
        },
        associative: false,
        seeds: vec![point(12.0, 22.0)],
    }
}

fn summary(drawing: &Drawing2D) -> Vec<(String, Hash)> {
    drawing
        .spaces
        .iter()
        .flat_map(|space| {
            let place = id::space(space.id);
            let mut names = id::Namer::new(&place);
            space
                .elements
                .iter()
                .enumerate()
                .map(|(index, element)| (names.next(index, element), Hash::element(element)))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
fn roundtrip<C: Codec>(src: &[u8]) -> Vec<u8>
where
    C::File: Lift<Drawing2D> + Apply<Changes, Output = Vec<Applied>>,
{
    let mut diagnostics = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let mut file = C::read(src, &mut cx).unwrap();
    let before = file.lift(&mut cx).unwrap();
    let space = &before.spaces[0];
    let source = &space.elements[0];
    let added: Vec<_> = [false, true]
        .into_iter()
        .map(|pattern| Element {
            source: Ref::default(),
            layer: source.layer,
            style: source.style,
            shape: Shape::Hatch(fixture(space.id, pattern)),
        })
        .collect();
    let edits = added
        .iter()
        .cloned()
        .map(|element| Edit::Add {
            place: id::space(space.id),
            element: Box::new(Placed::Model(element)),
        })
        .collect();
    let results = file.apply(
        &Changes {
            document: None,
            edits,
        },
        &mut cx,
    );
    assert!(results.is_ok(), "{results:?}: {diagnostics:?}");
    let out = testkit::write::<C>(&file).unwrap();
    let after = testkit::read::<C>(&out)
        .0
        .unwrap()
        .lift(&mut Cx::new(
            &mut Budget::default(),
            &mut Diagnostics::new(),
            &cancel,
        ))
        .unwrap();
    let known = summary(&before);
    let found = summary(&after);
    for (id, hash) in known {
        assert!(found.contains(&(id, hash)));
    }
    let actual = &after.spaces[0].elements;
    for expected in added {
        assert!(
            actual
                .iter()
                .any(|e| e.layer == expected.layer && e.shape == expected.shape),
            "{:?}",
            actual
                .iter()
                .filter(|e| matches!(e.shape, Shape::Hatch(_)))
                .collect::<Vec<_>>()
        );
    }
    out
}

#[test]
fn hatches_are_added_to_public_dxf_and_dwg_versions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/dwg");
    for version in ["r2000", "r2004", "r2018"] {
        let src = std::fs::read(root.join(format!("zumen-layout-{version}_ascii.dxf"))).unwrap();
        let out = roundtrip::<Dxf>(&src);
        let before = testkit::read::<Dxf>(&src).0.unwrap();
        let after = testkit::read::<Dxf>(&out).0.unwrap();
        for (index, record) in before.records().iter().enumerate() {
            if before.kind(record) == b"SECTION" {
                continue;
            }
            let Some(handle) = record
                .tags()
                .iter()
                .find(|tag| matches!(tag.code(), 5 | 105))
            else {
                continue;
            };
            let found = after
                .records()
                .iter()
                .position(|candidate| {
                    candidate.tags().iter().any(|tag| {
                        tag.code() == handle.code() && after.value(tag) == before.value(handle)
                    })
                })
                .unwrap();
            let a = before.record(index).unwrap().bytes.unwrap();
            let b = after.record(found).unwrap().bytes.unwrap();
            assert_eq!(
                &src[a.start as usize..a.end as usize],
                &out[b.start as usize..b.end as usize],
                "{version}: {index}"
            );
        }
        let src = std::fs::read(root.join(format!("zumen-layout-{version}.dwg"))).unwrap();
        let out = roundtrip::<Dwg>(&src);
        let before = testkit::read::<Dwg>(&src).0.unwrap();
        let after = testkit::read::<Dwg>(&out).0.unwrap();
        let mut compared = 0;
        for record in 0..before.entry_count() {
            let (handle, kind, _, _) = before.entry_summary(record).unwrap();
            // LAYER・STYLE・LTYPE・APPID・DIMSTYLE は、追加で所有や連結を変えない。
            if !matches!(kind, 0x33 | 0x35 | 0x39 | 0x43 | 0x45) {
                continue;
            }
            let found = (0..after.entry_count())
                .find(|&index| after.entry_summary(index).is_some_and(|e| e.0 == handle))
                .unwrap();
            assert_eq!(
                before.object_data(record),
                after.object_data(found),
                "{version}: {handle:X}"
            );
            compared += 1;
        }
        assert!(compared >= 10, "{version}: {compared}");
    }
}

#[test]
fn paper_solid_uses_millimeters_and_pattern_rejects_a_different_frame() {
    use zumen_drawing::{PaperLength, PaperPoint};
    let src = b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$INSUNITS\n70\n1\n9\n$HANDSEED\n5\n20\n9\n$PLIMMIN\n10\n10\n20\n20\n9\n$PLIMMAX\n10\n30\n20\n40\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n5\n1A\n67\n1\n8\n0\n10\n10\n20\n20\n11\n11\n21\n21\n0\nENDSEC\n0\nEOF\n";
    for pattern in [false, true] {
        let mut file = testkit::read::<Dxf>(src).0.unwrap();
        let mut diagnostics = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let drawing = file.lift(&mut cx).unwrap();
        let sheet = &drawing.sheets[0];
        let point = |x, y| PaperPoint {
            sheet: sheet.id,
            x,
            y,
        };
        let hatch = Hatch {
            loops: vec![HatchLoop {
                boundary: Boundary::Edges(vec![Edge::Arc {
                    center: point(25.4, 50.8),
                    radius: Length::Paper(PaperLength(12.7)),
                    start: 0.0,
                    end: 360.0,
                    counterclockwise: true,
                }]),
                external: true,
                outermost: true,
                derived: false,
                text_box: false,
                sources: Vec::new(),
            }],
            fill: fixture(SpaceId(0), pattern).fill,
            islands: Islands::Nested,
            associative: false,
            seeds: vec![point(25.4, 50.8)],
        };
        let mut element = sheet.elements[0].clone();
        element.source = Ref::default();
        element.shape = Shape::Hatch(hatch);
        let result = file.apply(
            &Changes {
                document: None,
                edits: vec![Edit::Add {
                    place: id::sheet(sheet),
                    element: Box::new(Placed::Paper(element.clone())),
                }],
            },
            &mut cx,
        );
        if pattern {
            assert!(result.is_err());
            let rejected = diagnostics
                .iter()
                .find(|d| d.code() == zumen_base::codes::CHANGE_HATCH_UNSUPPORTED)
                .unwrap();
            assert_eq!(rejected.param("field"), Some(&"geometry/pattern".into()));
            assert_eq!(
                rejected.param("value"),
                Some(&"paper_coordinate_frame".into())
            );
            assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
        } else {
            assert!(result.is_ok(), "{result:?}");
            let out = testkit::write::<Dxf>(&file).unwrap();
            let read = testkit::read::<Dxf>(&out).0.unwrap().lift(&mut cx).unwrap();
            assert_eq!(read.sheets[0].elements.last().unwrap().shape, element.shape);
        }
    }
}

#[test]
fn unsupported_hatches_are_atomic() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples/dwg/zumen-layout-r2000_ascii.dxf");
    let src = std::fs::read(root).unwrap();
    for case in 0..7 {
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let mut diagnostics = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let before = file.lift(&mut cx).unwrap();
        let space = &before.spaces[0];
        let mut hatch = fixture(space.id, case >= 5);
        match case {
            0 => hatch.associative = true,
            1 => hatch.loops[0].derived = true,
            2 => hatch.loops[0].sources.push(Ref::at_record(1)),
            3 => {
                if let Boundary::Polyline { closed, .. } = &mut hatch.loops[0].boundary {
                    *closed = false;
                }
            }
            4 => {
                if let Boundary::Edges(edges) = &mut hatch.loops[1].boundary
                    && let Edge::Arc { end, .. } = &mut edges[0]
                {
                    *end = 90.0;
                }
            }
            5 => {
                if let Fill::Pattern(pattern) = &mut hatch.fill {
                    pattern.scale = 0.0;
                }
            }
            _ => {
                if let Fill::Pattern(pattern) = &mut hatch.fill {
                    pattern.lines[0].offset = [0.0, 0.0];
                }
            }
        }
        let element = Element {
            source: Ref::default(),
            layer: space.elements[0].layer,
            style: space.elements[0].style,
            shape: Shape::Hatch(hatch),
        };
        let changes = Changes {
            document: None,
            edits: vec![Edit::Add {
                place: id::space(space.id),
                element: Box::new(Placed::Model(element)),
            }],
        };
        assert!(file.apply(&changes, &mut cx).is_err());
        let rejected = diagnostics
            .iter()
            .find(|d| d.code() == zumen_base::codes::CHANGE_HATCH_UNSUPPORTED)
            .unwrap();
        let (field, value) = match case {
            0 => ("geometry/associative", "true"),
            1 => ("geometry/loops/0/derived", "true"),
            2 => ("geometry/loops/0/sources", "1"),
            3 => ("geometry/loops/0/boundary/closed", "false"),
            4 => ("geometry/loops/1/boundary/sweep", "90"),
            5 => ("geometry/pattern/scale", "0"),
            _ => ("geometry/pattern/lines/0/offset", "[0,0]"),
        };
        assert_eq!(rejected.param("edit"), Some(&0u64.into()));
        assert_eq!(rejected.param("field"), Some(&field.into()));
        assert_eq!(rejected.param("value"), Some(&value.into()));
        assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
    }
}
