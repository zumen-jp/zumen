//! 名指した曲線の長さを、巨大配置・円の元定義・不連続な合成曲線で確かめる。
#![cfg(feature = "measure")]

#[path = "support/evaluation.rs"]
#[allow(dead_code, reason = "形状評価と共有する自作の入力の一部を使う")]
mod fixtures;

use fixtures::{add, data, mm, node, with};
use zumen_building::measure::{self, LengthUnit, Options, Rounding, RoundingMode, RoundingStage};
use zumen_building::*;
use zumen_geom3d::Tolerance;

fn options() -> Options<'static> {
    Options {
        unit: measure::Unit::Length(LengthUnit::Meter),
        rounding: Rounding {
            mode: RoundingMode::HalfEven,
            digits: 6,
            stage: RoundingStage::Total,
        },
        tolerance: Tolerance {
            chord: 0.001,
            length: 1e-6,
        },
        profile: None,
    }
}

fn graph(kinds: Vec<CurveKind>) -> GeometryNode {
    node(
        GeometryLinks::Leaf,
        GeometryDefinition::Curve(CurveGraph {
            root: CurveRef(0),
            items: kinds
                .into_iter()
                .map(|kind| CurveItem {
                    kind,
                    source: CurveSource::default(),
                })
                .collect(),
            length_unit: Some(mm()),
            angle_unit: None,
        }),
    )
}

fn building() -> Building {
    Building::new(Document {
        units: vec![mm()],
        ..Document::default()
    })
}

#[test]
fn named_polyline_uses_vector_differences_before_large_translation() {
    let mut b = building();
    let at = add(
        &mut b,
        data(
            vec![graph(vec![CurveKind::Polyline(vec![
                Coordinates::Three([0.0; 3]),
                Coordinates::Three([3000.0, 4000.0, 0.0]),
            ])])],
            &[0],
            "Axis",
        ),
        "IfcBeam",
    );
    b.components.placement.insert(
        at,
        Placement {
            relative_to: None,
            local: Local::Transform(Transform::translation(1e18, -1e18, 0.0)),
        },
    );
    let out = with(|cx| measure::lengths(&b, &[at], "Axis", options(), cx)).unwrap();
    assert_eq!(out.raw_total, Some(5.0));
    assert_eq!(out.rounded_total.unwrap().to_string(), "5.000000");
    assert_eq!(out.measurements[0].sources[0].item, GeometryRef(0));
    assert!(
        with(|cx| measure::lengths(&b, &[at], "Body", options(), cx))
            .unwrap()
            .raw_total
            .is_none()
    );
}

#[test]
fn circle_keeps_its_radius_and_composite_does_not_invent_a_bridge() {
    let mut b = building();
    let circle = add(
        &mut b,
        data(
            vec![graph(vec![CurveKind::Circle {
                position: CurvePosition {
                    location: Coordinates::Two([0.0, 0.0]),
                    axis: None,
                    reference_direction: None,
                },
                radius: 1000.0,
            }])],
            &[0],
            "Axis",
        ),
        "IfcMember",
    );
    let out = with(|cx| measure::lengths(&b, &[circle], "Axis", options(), cx)).unwrap();
    assert_eq!(out.raw_total, Some(core::f64::consts::TAU));
    let segment = |curve| CurveSegment {
        curve: CurveRef(curve),
        same_sense: true,
        transition: CurveTransition::Discontinuous,
    };
    let composite = add(
        &mut b,
        data(
            vec![graph(vec![
                CurveKind::Composite {
                    segments: vec![segment(1), segment(2)],
                    self_intersect: None,
                },
                CurveKind::Polyline(vec![
                    Coordinates::Two([0.0, 0.0]),
                    Coordinates::Two([1000.0, 0.0]),
                ]),
                CurveKind::Polyline(vec![
                    Coordinates::Two([2000.0, 0.0]),
                    Coordinates::Two([3000.0, 0.0]),
                ]),
            ])],
            &[0],
            "Axis",
        ),
        "IfcMember",
    );
    let out = with(|cx| measure::lengths(&b, &[composite], "Axis", options(), cx)).unwrap();
    assert_eq!(out.raw_total, Some(2.0));
}

#[test]
fn unbounded_line_is_not_measured_from_its_direction_magnitude() {
    let mut b = building();
    let at = add(
        &mut b,
        data(
            vec![graph(vec![CurveKind::Line {
                point: Coordinates::Two([0.0; 2]),
                direction: Coordinates::Two([1.0, 0.0]),
                magnitude: 1000.0,
            }])],
            &[0],
            "Axis",
        ),
        "IfcMember",
    );
    let out = with(|cx| measure::lengths(&b, &[at], "Axis", options(), cx)).unwrap();
    assert!(out.raw_total.is_none());
}
