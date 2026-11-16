//! 図面・出所・浮動小数点のビット・診断・予算停止を、環境に依存しない順で記録する。
#![allow(clippy::unwrap_used, reason = "試験と照合用の固定入力")]

use super::{drawing_fixtures as f, fixtures};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_building::{
    Building,
    drawing::{self, Options, Output, Side, View},
};

fn number(out: &mut Vec<u8>, n: u64) {
    out.extend_from_slice(&n.to_le_bytes());
}
fn text(out: &mut Vec<u8>, value: &str) {
    number(out, value.len() as u64);
    out.extend_from_slice(value.as_bytes());
}
fn output(out: &mut Vec<u8>, value: &Output) {
    // 表と出所を含む全体に加え、計算した数値を丸めずビットでも記録する。
    text(out, &format!("{value:?}"));
    let origin = value.frame.origin();
    for v in [
        origin.x,
        origin.y,
        origin.z,
        value.frame.depth(),
        value.grid.spacing(),
    ] {
        number(out, v.to_bits());
    }
    for axis in value.frame.axes() {
        for v in axis {
            number(out, v.to_bits());
        }
    }
    for space in &value.drawing.spaces {
        for element in &space.elements {
            if let zumen_drawing::Shape::Line { start, end } = element.shape {
                for v in [start.x, start.y, end.x, end.y] {
                    number(out, v.to_bits());
                }
            }
        }
    }
    for sheet in &value.drawing.sheets {
        for view in &sheet.views {
            for v in view.transform.m {
                number(out, v.to_bits());
            }
        }
    }
}

fn case(out: &mut Vec<u8>, building: &Building, options: Options<'_>) {
    let mut used = Budget::default();
    drawing::cut(
        building,
        options,
        &mut Cx::new(&mut used, &mut Diagnostics::new(), &CancelToken::new()),
    )
    .unwrap();
    let mut runs = vec![(Limits::default(), false), (Limits::default(), true)];
    for kind in [Kind::Work, Kind::DecodedBytes, Kind::Segments, Kind::Depth] {
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
        match drawing::cut(
            building,
            options,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        ) {
            Ok(value) => {
                text(out, "ok");
                output(out, &value);
            }
            Err(error) => text(out, &format!("{error:?}")),
        }
        for kind in Kind::ALL {
            number(out, budget.used(kind));
        }
        text(out, &format!("{diagnostics:?}"));
    }
}

pub fn record() -> Vec<u8> {
    let mut out = b"zumen-building drawing 1\n".to_vec();
    let plan = View::Plan {
        floor: f::point(0.0, 0.0, 0.0),
        height: 500.0,
        depth: 1000.0,
    };
    let mut cube = f::cube();
    let (grid, axis) = f::add_grid(&mut cube);
    case(&mut out, &cube, f::options(plan));
    for section in [false, true] {
        for side in [Side::Right, Side::Left] {
            case(
                &mut out,
                &cube,
                f::options(View::Vertical {
                    start: f::point(0.0, 500.0, 0.0),
                    end: f::point(1000.0, 500.0, 0.0),
                    side,
                    depth: 1000.0,
                    section,
                }),
            );
        }
    }
    let view = fixtures::with(|cx| {
        drawing::GridView {
            grid,
            axis,
            elevation: 0.0,
            side: Side::Left,
            depth: 1000.0,
            section: true,
        }
        .view(&cube, cx)
    })
    .unwrap();
    case(&mut out, &cube, f::options(view));
    case(
        &mut out,
        &fixtures::building(),
        f::options(View::Plan {
            floor: f::point(1e9, 2e9, 3000.0),
            height: 1000.0,
            depth: 1500.0,
        }),
    );
    let mut door = f::door();
    case(&mut out, &door, f::options(plan));
    let curves = [f::line_graph(321.0)];
    let rules = [drawing::JoinerySymbol {
        element: None,
        set: "panel",
        property: "PanelOperation",
        type_name: "IfcDoorPanelOperationEnum",
        operation: "SWINGING",
        curves: &curves,
    }];
    let options = Options {
        symbols: &rules,
        ..f::options(plan)
    };
    case(&mut out, &door, options);
    f::add_plan(&mut door);
    case(&mut out, &door, options);
    let mut space = f::cube();
    space.components.spatial.insert(
        zumen_building::ElementRef(0),
        zumen_building::Spatial {
            kind: zumen_building::SpatialKind::Space,
            elevation: None,
            ref_elevation: None,
        },
    );
    fixtures::add(
        &mut space,
        fixtures::data(
            vec![fixtures::cuboid([200.0, 200.0, 0.0], [800.0, 800.0, 100.0])],
            &[0],
            "Body",
        ),
        "IfcSlab",
    );
    case(&mut out, &space, f::options(plan));
    let mut geometry = cube
        .geometry(zumen_building::ElementRef(0))
        .unwrap()
        .data()
        .clone();
    if let Some(node) = geometry.nodes.first_mut() {
        node.definition =
            zumen_building::GeometryDefinition::Unsupported("contract_surface".into());
    }
    fixtures::with(|cx| {
        cube.insert_geometry(
            zumen_building::ElementRef(0),
            zumen_building::Geometry::new(geometry, cx).unwrap(),
            cx,
        )
    })
    .unwrap();
    case(&mut out, &cube, f::options(plan));
    out
}
