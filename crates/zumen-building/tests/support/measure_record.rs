//! 数量・丸め・出所・数値のビット・診断・停止条件の環境間照合。
#![allow(clippy::unwrap_used, reason = "試験と照合の固定入力")]

use super::{fixtures, input};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_building::measure::{
    self, Boundary, InputBoundary, LengthUnit, Method, Profile, Report, Rounding, RoundingMode,
    RoundingStage, Unit,
};
use zumen_geom3d::Failure;

fn number(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn text(out: &mut Vec<u8>, value: &str) {
    number(out, value.len() as u64);
    out.extend_from_slice(value.as_bytes());
}
fn bits(out: &mut Vec<u8>, values: impl IntoIterator<Item = f64>) {
    for value in values {
        number(out, value.to_bits());
    }
}
fn report(out: &mut Vec<u8>, report: &Report) {
    text(out, &format!("{report:?}"));
    bits(out, [report.tolerance.length, report.tolerance.chord]);
    bits(out, report.raw_total);
    if let Method::Area { origin, .. } = report.method {
        bits(out, [origin.x, origin.y, origin.z]);
    }
    for value in &report.measurements {
        bits(out, value.raw);
        for source in &value.sources {
            bits(out, [source.tolerance.length, source.tolerance.chord]);
        }
        if let Some(decision) = &value.boundary {
            for wall in &decision.walls {
                bits(out, wall.ends.into_iter().flatten().chain([wall.thickness]));
            }
        }
    }
}
fn case(out: &mut Vec<u8>, run: impl Fn(&mut Cx<'_>) -> Result<Report, Failure>) {
    let mut used = Budget::default();
    run(&mut Cx::new(
        &mut used,
        &mut Diagnostics::new(),
        &CancelToken::new(),
    ))
    .unwrap();
    let mut runs = vec![(Limits::default(), false), (Limits::default(), true)];
    for kind in [Kind::Work, Kind::DecodedBytes, Kind::Segments, Kind::Depth] {
        for limit in [0, used.used(kind).saturating_sub(1), used.used(kind)] {
            runs.push((Limits::default().with(kind, Limit::Finite(limit)), false));
        }
    }
    for (limits, cancelled) in runs {
        let (mut budget, mut diagnostics, cancel) =
            (Budget::new(limits), Diagnostics::new(), CancelToken::new());
        if cancelled {
            cancel.cancel();
        }
        match run(&mut Cx::new(&mut budget, &mut diagnostics, &cancel)) {
            Ok(value) => {
                text(out, "ok");
                report(out, &value);
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
    let mut out = b"zumen-building measure 1\n".to_vec();
    for inside in [false, true] {
        for (axis, layers) in [(Some(65.0), false), (None, true), (None, false)] {
            let (b, space, walls) = input::room(inside, axis, layers);
            let mut options = input::options();
            options.profile = Some(Profile {
                id: "test.building",
                version: "1",
                hash: [7; 32],
            });
            for boundary in [Boundary::Shape, Boundary::Inside, Boundary::WallCenter] {
                for input in [
                    InputBoundary::Auto,
                    InputBoundary::Inside,
                    InputBoundary::WallCenter,
                ] {
                    case(&mut out, |cx| {
                        measure::areas(
                            &b,
                            &[space],
                            input::request(&walls, boundary, input),
                            options,
                            cx,
                        )
                    });
                }
            }
            case(&mut out, |cx| {
                measure::lengths(
                    &b,
                    &walls,
                    "Axis",
                    measure::Options {
                        unit: Unit::Length(LengthUnit::Millimeter),
                        ..options
                    },
                    cx,
                )
            });
        }
    }
    let b = fixtures::building();
    for openings in [false, true] {
        for stage in [RoundingStage::Element, RoundingStage::Total] {
            let options = measure::Options {
                unit: Unit::Volume(LengthUnit::Meter),
                rounding: Rounding {
                    stage,
                    ..input::options().rounding
                },
                ..input::options()
            };
            case(&mut out, |cx| {
                measure::volumes(
                    &b,
                    &[zumen_building::ElementRef(0), zumen_building::ElementRef(1)],
                    openings,
                    options,
                    cx,
                )
            });
        }
    }
    let (mut b, space, walls) = input::room(true, Some(0.0), false);
    let rotation = zumen_building::Transform([
        [0.6, -0.8, 0.0, 100000.0],
        [0.8, 0.6, 0.0, -200000.0],
        [0.0, 0.0, 1.0, 0.0],
    ]);
    for at in (0..b.elements().len()).map(|n| zumen_building::ElementRef(n as u32)) {
        let local = match b.components.placement.get(at) {
            Some(zumen_building::Placement {
                local: zumen_building::Local::Transform(t),
                ..
            }) => *t,
            _ => zumen_building::Transform::IDENTITY,
        };
        b.components.placement.insert(
            at,
            zumen_building::Placement {
                relative_to: None,
                local: zumen_building::Local::Transform(rotation.then(&local)),
            },
        );
    }
    let mut request = input::request(&walls, Boundary::WallCenter, InputBoundary::Auto);
    request.origin.x = 100000.0;
    request.origin.y = -200000.0;
    case(&mut out, |cx| {
        measure::areas(&b, &[space], request, input::options(), cx)
    });
    let mut b = zumen_building::Building::new(zumen_building::Document {
        units: vec![fixtures::mm()],
        ..Default::default()
    });
    let mut cylinder = fixtures::cuboid([-1000.0, -1000.0, 0.0], [1000.0, 1000.0, 3000.0]);
    if let zumen_building::GeometryDefinition::Extrusion { definition, .. } =
        &mut cylinder.definition
    {
        definition.profile.kind = zumen_building::AreaProfileKind::Circle { radius: 1000.0 };
        definition.direction = [3.0, 0.0, 4.0];
    }
    let at = fixtures::add(
        &mut b,
        fixtures::data(vec![cylinder], &[0], "Body"),
        "IfcSpace",
    );
    case(&mut out, |cx| {
        measure::areas(
            &b,
            &[at],
            input::request(&[], Boundary::Shape, InputBoundary::Auto),
            input::options(),
            cx,
        )
    });
    case(&mut out, |cx| {
        measure::volumes(
            &b,
            &[at],
            false,
            measure::Options {
                unit: Unit::Volume(LengthUnit::Meter),
                ..input::options()
            },
            cx,
        )
    });
    for mode in [
        RoundingMode::Truncate,
        RoundingMode::Floor,
        RoundingMode::Ceiling,
        RoundingMode::HalfAway,
        RoundingMode::HalfEven,
    ] {
        for digits in [-18, -2, 0, 2, 18] {
            for value in [
                0.0,
                -0.0,
                1.005,
                -1.005,
                2.675,
                -2.685,
                145.0,
                155.0,
                f64::from_bits(1),
                -f64::from_bits(1),
                f64::MAX,
                f64::INFINITY,
            ] {
                let mut budget = Budget::default();
                let result = Rounding {
                    mode,
                    digits,
                    stage: RoundingStage::Total,
                }
                .apply(
                    value,
                    &mut Cx::new(&mut budget, &mut Diagnostics::new(), &CancelToken::new()),
                );
                text(&mut out, &format!("{result:?}"));
                for kind in Kind::ALL {
                    number(&mut out, budget.used(kind));
                }
            }
        }
    }
    out
}
