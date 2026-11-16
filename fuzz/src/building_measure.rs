//! 壁芯・内法・形のままの境界と、長さ・体積・丸めへ変異した数を渡す。
#[path = "../../crates/zumen-building/tests/support/evaluation.rs"]
#[allow(dead_code, reason = "数量の試験と共有する固定入力")]
mod fixtures;
#[path = "../../crates/zumen-building/tests/support/measure.rs"]
#[allow(dead_code, reason = "数量の試験と共有する固定入力")]
mod input;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_building::{
    Geometry, GeometryDefinition,
    measure::{self, Boundary, InputBoundary, LengthUnit, RoundingMode, RoundingStage, Unit},
};
use zumen_geom3d::Failure;

fn byte(data: &[u8], i: usize) -> u8 {
    data.get(i).copied().unwrap_or(0)
}
fn number(data: &[u8], i: usize) -> f64 {
    if byte(data, 0) & 1 == 0 {
        f64::from(byte(data, i)) * 8.0 - 16.0
    } else {
        f64::from_bits(u64::from_le_bytes(std::array::from_fn(|j| {
            byte(data, i + j)
        })))
    }
}
pub fn probe(data: &[u8], cx: &mut Cx<'_>) -> Result<measure::Report, Failure> {
    let (building, space, walls) = input::room(true, Some(0.0), false);
    let mut request = input::request(&walls, Boundary::WallCenter, InputBoundary::Auto);
    request.origin.z = number(data, 1);
    measure::areas(&building, &[space], request, input::options(), cx)
}
pub fn exercise(data: &[u8]) {
    let (mut building, space, walls) = input::room(
        byte(data, 0) & 2 != 0,
        if byte(data, 0) & 4 != 0 {
            None
        } else {
            Some(number(data, 9))
        },
        byte(data, 0) & 8 != 0,
    );
    if byte(data, 0) & 16 != 0 {
        let mut geometry = building.geometry(space).unwrap().data().clone();
        if let Some(node) = geometry.nodes.first_mut()
            && let GeometryDefinition::Extrusion { definition, .. } = &mut node.definition
        {
            definition.depth = number(data, 17);
        }
        if let Ok(geometry) = fixtures::with(|cx| Geometry::new(geometry, cx)) {
            fixtures::with(|cx| building.insert_geometry(space, geometry, cx)).unwrap();
        }
    }
    let mut options = input::options();
    options.rounding.digits = byte(data, 25) as i8;
    options.rounding.mode = match byte(data, 26) % 5 {
        0 => RoundingMode::Truncate,
        1 => RoundingMode::Floor,
        2 => RoundingMode::Ceiling,
        3 => RoundingMode::HalfEven,
        _ => RoundingMode::HalfAway,
    };
    options.rounding.stage = if byte(data, 27) & 1 == 0 {
        RoundingStage::Element
    } else {
        RoundingStage::Total
    };
    if byte(data, 0) & 32 != 0 {
        options.tolerance.length = number(data, 28);
    }
    if byte(data, 0) & 64 != 0 {
        options.tolerance.chord = number(data, 36);
    }
    let boundary = match byte(data, 44) % 3 {
        0 => Boundary::Shape,
        1 => Boundary::WallCenter,
        _ => Boundary::Inside,
    };
    let interpretation = match byte(data, 45) % 3 {
        0 => InputBoundary::Auto,
        1 => InputBoundary::WallCenter,
        _ => InputBoundary::Inside,
    };
    let mut request = input::request(&walls, boundary, interpretation);
    request.origin.z = number(data, 1);
    let operation = byte(data, 46) % 3;
    let unit = if byte(data, 47) & 1 == 0 {
        LengthUnit::Meter
    } else {
        LengthUnit::Millimeter
    };
    options.unit = match operation {
        0 => Unit::Area(unit),
        1 => Unit::Volume(unit),
        _ => Unit::Length(unit),
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
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let result = match operation {
            0 => measure::areas(&building, &[space], request, options, &mut cx),
            1 => measure::volumes(
                &building,
                &[space],
                byte(data, 47) & 2 != 0,
                options,
                &mut cx,
            ),
            _ => measure::lengths(&building, &walls, "Axis", options, &mut cx),
        };
        assert_eq!(budget.used(Kind::Depth), 0);
        (
            result,
            format!("{diagnostics:?}"),
            Kind::ALL.map(|kind| budget.used(kind)),
        )
    };
    assert_eq!(run(limits, false), run(limits, false));
    for kind in [Kind::Work, Kind::DecodedBytes, Kind::Segments, Kind::Depth] {
        let limits = limits.with(kind, Limit::Finite(u64::from(byte(data, 48))));
        assert_eq!(run(limits, false), run(limits, false));
    }
    assert_eq!(run(limits, true), run(limits, true));
}

#[cfg(test)]
mod tests {
    #[test]
    fn synthetic_mutations_keep_measurement_results_and_stops_deterministic() {
        for byte in 0..=255u8 {
            super::exercise(&[byte; 64]);
        }
        for number in [0.0f64, -0.0, 1e-12, 1.0, 1000.0, f64::INFINITY, f64::NAN] {
            let mut input = [1u8; 64];
            for range in input[1..].chunks_mut(8) {
                let value = number.to_le_bytes();
                range.copy_from_slice(&value[..range.len()]);
            }
            super::exercise(&input);
        }
    }
}
