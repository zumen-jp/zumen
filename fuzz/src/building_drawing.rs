//! 合成建物と切断の指定に、有限でない値・低い予算・中断も渡す。
#[path = "../../crates/zumen-building/tests/support/drawing.rs"]
mod drawing_fixtures;
#[allow(dead_code, reason = "建物モデルの共通の合成入力から必要なものを使う")]
#[path = "../../crates/zumen-building/tests/support/evaluation.rs"]
mod fixtures;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_building::{
    ElementRef, Geometry, GeometryDefinition,
    drawing::{self, Side, View},
};

fn byte(data: &[u8], index: usize) -> u8 {
    data.get(index).copied().unwrap_or(0)
}
fn number(data: &[u8], index: usize) -> f64 {
    if byte(data, 0) & 1 == 0 {
        f64::from(byte(data, index)) * 8.0 - 16.0
    } else {
        f64::from_bits(u64::from_le_bytes(std::array::from_fn(|i| {
            byte(data, index + i)
        })))
    }
}

pub fn probe(data: &[u8], cx: &mut Cx<'_>) -> Result<drawing::Output, zumen_geom3d::Failure> {
    let building = drawing_fixtures::cube();
    drawing::cut(
        &building,
        drawing_fixtures::options(View::Plan {
            floor: drawing_fixtures::point(0.0, 0.0, 0.0),
            height: number(data, 1),
            depth: 3000.0,
        }),
        cx,
    )
}
pub fn exercise(data: &[u8]) {
    let mut building = if byte(data, 0) & 8 == 0 {
        drawing_fixtures::cube()
    } else {
        drawing_fixtures::door()
    };
    if byte(data, 0) & 16 != 0 {
        drawing_fixtures::add_plan(&mut building);
    }
    if byte(data, 0) & 32 != 0 {
        drawing_fixtures::add_grid(&mut building);
    }
    if byte(data, 0) & 64 != 0 {
        let mut geometry = building.geometry(ElementRef(0)).unwrap().data().clone();
        if let Some(node) = geometry.nodes.first_mut()
            && let GeometryDefinition::Extrusion { definition, .. } = &mut node.definition
        {
            definition.depth = number(data, 8);
        }
        if let Ok(geometry) = fixtures::with(|cx| Geometry::new(geometry, cx)) {
            fixtures::with(|cx| building.insert_geometry(ElementRef(0), geometry, cx)).unwrap();
        }
    }
    let point = drawing_fixtures::point;
    let view = if byte(data, 0) & 2 == 0 {
        View::Plan {
            floor: point(number(data, 24), number(data, 32), 0.0),
            height: number(data, 1),
            depth: 3000.0,
        }
    } else {
        View::Vertical {
            start: point(0.0, number(data, 1), 0.0),
            end: point(number(data, 9), number(data, 17), 0.0),
            side: if byte(data, 40) & 1 == 0 {
                Side::Right
            } else {
                Side::Left
            },
            depth: 3000.0,
            section: byte(data, 0) & 4 != 0,
        }
    };
    let mut options = drawing_fixtures::options(view);
    if byte(data, 0) & 128 != 0 {
        options.tolerance = number(data, 41);
    }
    let curves = [drawing_fixtures::line_graph(321.0)];
    let rules = [drawing::JoinerySymbol {
        element: None,
        set: "panel",
        property: "PanelOperation",
        type_name: "IfcDoorPanelOperationEnum",
        operation: "SWINGING",
        curves: &curves,
    }];
    let options = drawing::Options {
        symbols: &rules,
        ..options
    };
    let limits = Limits::default()
        .with(Kind::DecodedBytes, Limit::Finite(8 * 1024 * 1024))
        .with(Kind::Work, Limit::Finite(1 << 20))
        .with(Kind::Segments, Limit::Finite(8192))
        .with(Kind::Depth, Limit::Finite(32));
    let run = |limits, cancelled| {
        let (mut budget, mut diagnostics, cancel) =
            (Budget::new(limits), Diagnostics::new(), CancelToken::new());
        if cancelled {
            cancel.cancel();
        }
        let result = drawing::cut(
            &building,
            options,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        );
        assert_eq!(budget.used(Kind::Depth), 0);
        (
            result,
            format!("{diagnostics:?}"),
            Kind::ALL.map(|kind| budget.used(kind)),
        )
    };
    assert_eq!(run(limits, false), run(limits, false));
    for kind in [Kind::Work, Kind::DecodedBytes, Kind::Segments, Kind::Depth] {
        let limits = limits.with(kind, Limit::Finite(u64::from(byte(data, 49))));
        assert_eq!(run(limits, false), run(limits, false));
    }
    assert_eq!(run(limits, true), run(limits, true));
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_view_and_non_finite_inputs_keep_their_result_and_budget() {
        for flags in [0, 2, 6, 8, 24, 32, 64, 128, 255] {
            for fill in [0, 1, 48, 127, 255] {
                let mut data = vec![fill; 64];
                data[0] = flags;
                super::exercise(&data);
            }
        }
    }
}
