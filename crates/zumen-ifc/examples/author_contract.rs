//! 変更だけで作った 2 階建ての建物と、それを直した文書から、IFC のバイト列・建物モデルのハッシュ・平断面・体積・
//! 予算停止を書き、ネイティブと WASI で照合する（#898）。
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "照合用の固定入力。失敗すれば記録が変わる"
)]

use std::io::Write;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_building::{
    Building, ElementRef, GlobalId, Hash, Id, Transform,
    drawing::{self, Paper, Side, View},
    measure::{self, LengthUnit, Rounding, RoundingMode, RoundingStage, Unit},
};
use zumen_crs::ProjectPoint;
use zumen_drawing::{PaperLength, Shape};
use zumen_geom3d::Tolerance;
use zumen_ifc::{
    Edit, FillingKind, Ifc, IfcLive, NewBeam, NewColumn, NewFilling, NewGrid, NewGridAxis,
    NewLayer, NewOpening, NewPlacement, NewSection, NewSlab, NewSpace, NewWall, OpeningChange,
    SpaceKind, Target, WallChange, apply,
};
use zumen_sdk::{Codec, Lift};

const SCHEMAS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];

fn global(n: u32) -> GlobalId {
    GlobalId::from_uuid(u128::from(n).to_be_bytes())
}

fn source(schema: &str) -> String {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\nFILE_NAME('a.ifc','',(''),(''),'','','');\nFILE_SCHEMA(('{schema}'));\nENDSEC;\nDATA;\n#1=IFCPROJECT('{}',#2,'Project',$,$,$,$,$,#3);\n#2=IFCOWNERHISTORY($,$,$,.ADDED.,$,$,$,0);\n#3=IFCUNITASSIGNMENT((#4));\n#4=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\nENDSEC;\nEND-ISO-10303-21;\n",
        global(1)
    )
}

fn layer(material: &str, thickness: f64) -> NewLayer {
    NewLayer {
        material: material.to_owned(),
        thickness,
    }
}

fn axis(tag: &str, start: [f64; 2], end: [f64; 2]) -> NewGridAxis {
    NewGridAxis {
        tag: tag.to_owned(),
        start,
        end,
    }
}

/// 建物・1F・2F（高さ 3000）・1F の通り芯と、各階の 6000×4000 を囲む 4 つの壁・床・柱・梁、1F の扉と 2F の窓。
/// `change` なら、同じ要求の中で 2F を高さ 3200 に、1F の最初の壁を 3 層に、2F の窓の開口を動かして大きさを変える
fn edits(change: bool) -> Vec<Edit> {
    let storey = |n: u32, elevation: f64| {
        Edit::Space(NewSpace {
            kind: SpaceKind::Storey,
            name: Some(format!("{}F", n - 1000)),
            parent: Target::Added(0),
            placement: Some(NewPlacement {
                relative_to: Some(Target::Added(0)),
                local: Transform::translation(0.0, 0.0, elevation),
            }),
            elevation: Some(elevation),
            global_id: Some(global(n)),
        })
    };
    let mut edits = vec![
        Edit::Space(NewSpace {
            kind: SpaceKind::Building,
            name: Some("B".to_owned()),
            parent: Target::Instance(1),
            placement: Some(NewPlacement {
                relative_to: None,
                local: Transform::translation(0.0, 0.0, 0.0),
            }),
            elevation: None,
            global_id: Some(global(1000)),
        }),
        storey(1001, 0.0),
        storey(1002, 3000.0),
        Edit::Grid(NewGrid {
            storey: Target::Added(1),
            u: vec![
                axis("A", [0.0, -1000.0], [0.0, 5000.0]),
                axis("B", [6000.0, -1000.0], [6000.0, 5000.0]),
            ],
            v: vec![
                axis("1", [-1000.0, 0.0], [7000.0, 0.0]),
                axis("2", [-1000.0, 4000.0], [7000.0, 4000.0]),
            ],
            elevation: None,
            name: Some("G".to_owned()),
            global_id: Some(global(1003)),
        }),
    ];
    let corners = [[0.0, 0.0], [6000.0, 0.0], [6000.0, 4000.0], [0.0, 4000.0]];
    let section = |width, depth| NewSection { width, depth };
    let mut first_wall = 0;
    let mut window = 0;
    for level in [1u32, 2] {
        let base = 1000 * (level + 1);
        let on = Target::Added(level as usize);
        let first = edits.len();
        if level == 1 {
            first_wall = first;
        }
        for (i, start) in corners.iter().enumerate() {
            edits.push(Edit::Wall(NewWall {
                storey: on,
                start: *start,
                end: corners[(i + 1) % corners.len()],
                height: 2800.0,
                layers: vec![layer("Concrete", 150.0), layer("Insulation", 50.0)],
                offset: None,
                type_of: None,
                name: Some(format!("W{}", base + i as u32)),
                global_id: Some(global(base + i as u32)),
            }));
        }
        edits.extend([
            Edit::Slab(NewSlab {
                storey: on,
                outline: corners.to_vec(),
                layers: vec![layer("Concrete", 200.0)],
                elevation: None,
                type_of: None,
                name: None,
                global_id: Some(global(base + 10)),
            }),
            Edit::Column(NewColumn {
                storey: on,
                position: [0.0, 0.0],
                base: None,
                height: 2800.0,
                section: section(400.0, 400.0),
                direction: Some([0.0, 1.0]),
                material: Some("Concrete".to_owned()),
                type_of: None,
                name: None,
                global_id: Some(global(base + 11)),
            }),
            Edit::Beam(NewBeam {
                storey: on,
                start: [0.0, 4000.0, 2800.0],
                end: [6000.0, 4000.0, 2800.0],
                section: section(300.0, 600.0),
                material: Some("Steel".to_owned()),
                type_of: None,
                name: None,
                global_id: Some(global(base + 12)),
            }),
        ]);
        let (kind, sill, size) = if level == 1 {
            (FillingKind::Door, 0.0, [900.0, 2100.0])
        } else {
            (FillingKind::Window, 900.0, [1200.0, 1200.0])
        };
        if level == 2 {
            window = edits.len();
        }
        edits.push(Edit::Opening(NewOpening {
            host: Target::Added(first),
            distance: 1000.0,
            sill,
            width: size[0],
            height: size[1],
            name: None,
            global_id: Some(global(base + 20)),
        }));
        edits.push(Edit::Filling(NewFilling {
            opening: Target::Added(edits.len() - 1),
            kind,
            width: Some(size[0]),
            height: Some(size[1]),
            type_of: None,
            name: None,
            global_id: Some(global(base + 21)),
        }));
    }
    if change {
        edits.extend([
            Edit::Elevation {
                storey: Target::Added(2),
                elevation: 3200.0,
            },
            Edit::WallShape(WallChange {
                wall: Target::Added(first_wall),
                start: [0.0, 0.0],
                end: [6500.0, 0.0],
                height: 3000.0,
                layers: vec![
                    layer("Concrete", 200.0),
                    layer("Insulation", 50.0),
                    layer("Board", 20.0),
                ],
                offset: None,
                type_of: None,
            }),
            Edit::OpeningShape(OpeningChange {
                opening: Target::Added(window),
                distance: 2000.0,
                sill: 1000.0,
                width: 1600.0,
                height: 1000.0,
            }),
        ]);
    }
    edits
}

fn number(out: &mut Vec<u8>, n: u64) {
    out.extend_from_slice(&n.to_le_bytes());
}

fn text(out: &mut Vec<u8>, value: &str) {
    number(out, value.len() as u64);
    out.extend_from_slice(value.as_bytes());
}

fn bits(out: &mut Vec<u8>, values: impl IntoIterator<Item = f64>) {
    for v in values {
        number(out, v.to_bits());
    }
}

/// 変更を、予算を変えて当てる。結果のバイト列と、診断・使った量・停止の位置を書く
fn author(out: &mut Vec<u8>, src: &[u8], edits: &[Edit]) -> Vec<u8> {
    let mut used = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut used, &mut diagnostics, &cancel);
    let file = Ifc::read(src, &mut cx).unwrap();
    let bytes = apply(&file, edits, b"seed", &mut cx).unwrap().bytes;
    let mut runs = vec![(Limits::default(), false), (Limits::default(), true)];
    for kind in [Kind::Work, Kind::DecodedBytes, Kind::Depth] {
        for bound in [0, used.used(kind).saturating_sub(1), used.used(kind)] {
            runs.push((Limits::default().with(kind, Limit::Finite(bound)), false));
        }
    }
    for (limits, cancelled) in runs {
        let cancel = CancelToken::new();
        if cancelled {
            cancel.cancel();
        }
        let mut budget = Budget::new(limits);
        let mut diagnostics = Diagnostics::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        match Ifc::read(src, &mut cx).and_then(|file| apply(&file, edits, b"seed", &mut cx)) {
            Ok(applied) => {
                text(out, "ok");
                text(out, &Hash::document(&applied.bytes).to_string());
            }
            Err(error) => text(out, &format!("{error:?}")),
        }
        for kind in Kind::ALL {
            number(out, budget.used(kind));
        }
        text(out, &format!("{diagnostics:?}"));
    }
    bytes
}

fn with<T>(f: impl FnOnce(&mut Cx) -> T) -> T {
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    f(&mut Cx::new(&mut budget, &mut diagnostics, &cancel))
}

fn element(building: &Building, n: u32) -> ElementRef {
    building.find(Id::Global(global(n))).unwrap()
}

/// 建物モデル・各階の平断面・要素の体積を書く
fn outputs(out: &mut Vec<u8>, bytes: &[u8], floors: [f64; 2]) {
    text(out, &format!("{}", bytes.len()));
    text(out, &Hash::document(bytes).to_string());
    let (building, diagnostics) = {
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let file = Ifc::read(bytes, &mut cx).unwrap();
        let building = file.lift_geometry(&mut cx).unwrap();
        (building, diagnostics)
    };
    text(out, &format!("{diagnostics:?}"));
    text(out, &Hash::document_values(&building).to_string());
    let lifted = with(|cx| Ifc::read(bytes, cx).and_then(|file| file.lift(cx))).unwrap();
    text(out, &Hash::document_values(&lifted).to_string());
    let mut views = vec![
        View::Plan {
            floor: ProjectPoint {
                x: 0.0,
                y: 0.0,
                z: floors[0],
            },
            height: 1200.0,
            depth: 2000.0,
        },
        View::Plan {
            floor: ProjectPoint {
                x: 0.0,
                y: 0.0,
                z: floors[1],
            },
            height: 1200.0,
            depth: 2000.0,
        },
        View::Vertical {
            start: ProjectPoint {
                x: 1500.0,
                y: -1000.0,
                z: 0.0,
            },
            end: ProjectPoint {
                x: 1500.0,
                y: 5000.0,
                z: 0.0,
            },
            side: Side::Right,
            depth: 1500.0,
            section: true,
        },
    ];
    for view in views.drain(..) {
        let options = drawing::Options {
            view,
            tolerance: 0.01,
            chord: 1.0,
            paper: Paper {
                denominator: 100.0,
                origin: [PaperLength(0.0); 2],
                size: None,
            },
            symbols: &[],
        };
        let mut diagnostics = Diagnostics::new();
        let cut = {
            let mut budget = Budget::default();
            let cancel = CancelToken::new();
            drawing::cut(
                &building,
                options,
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
            )
            .unwrap()
        };
        text(out, &format!("{cut:?}"));
        text(out, &format!("{diagnostics:?}"));
        for space in &cut.drawing.spaces {
            for element in &space.elements {
                if let Shape::Line { start, end } = element.shape {
                    bits(out, [start.x, start.y, end.x, end.y]);
                }
            }
        }
    }
    let options = measure::Options {
        unit: Unit::Volume(LengthUnit::Meter),
        rounding: Rounding {
            mode: RoundingMode::HalfAway,
            digits: 3,
            stage: RoundingStage::Element,
        },
        tolerance: Tolerance {
            length: 1e-6,
            chord: 0.001,
        },
        profile: None,
    };
    let selected: Vec<ElementRef> = [
        2000, 2001, 2002, 2003, 2010, 2011, 2012, 2021, 3000, 3010, 3021,
    ]
    .into_iter()
    .map(|n| element(&building, n))
    .collect();
    for openings in [false, true] {
        let report =
            with(|cx| measure::volumes(&building, &selected, openings, options, cx)).unwrap();
        text(out, &format!("{report:?}"));
        bits(out, report.raw_total);
        for value in &report.measurements {
            bits(out, value.raw);
        }
    }
}

fn record() -> Vec<u8> {
    let mut out = b"zumen-ifc author 1\n".to_vec();
    for schema in SCHEMAS {
        let src = source(schema);
        text(&mut out, schema);
        let built = author(&mut out, src.as_bytes(), &edits(false));
        outputs(&mut out, &built, [0.0, 3000.0]);
        let changed = author(&mut out, src.as_bytes(), &edits(true));
        outputs(&mut out, &changed, [0.0, 3200.0]);
        // 文書エンジンでの一括適用・取り消し・やり直しが元のバイト列と一致することも記録する
        let mut live = with(|cx| IfcLive::open(src.as_bytes(), cx)).unwrap();
        with(|cx| live.apply(&edits(false), b"seed", cx)).unwrap();
        assert_eq!(with(|cx| live.save_spf(cx)).unwrap(), built);
        assert!(with(|cx| live.undo(cx)).unwrap());
        assert_eq!(with(|cx| live.save_spf(cx)).unwrap(), src.as_bytes());
        assert!(with(|cx| live.redo(cx)).unwrap());
        assert_eq!(with(|cx| live.save_spf(cx)).unwrap(), built);
        out.extend_from_slice(&built);
        out.extend_from_slice(&changed);
    }
    out
}

fn main() -> std::io::Result<()> {
    std::io::stdout().write_all(&record())
}
