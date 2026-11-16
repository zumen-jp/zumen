//! 体積・開口・重なり・単位・丸める段と、記録された数量からの独立を確かめる。
#![cfg(feature = "measure")]

#[path = "support/evaluation.rs"]
#[allow(
    dead_code,
    reason = "形状評価と共有する自作の入力のうち必要な部品を使う"
)]
mod fixtures;

use fixtures::{add, cuboid, data, mm, with};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::measure::{
    self, LengthUnit, Options, Profile, Rounding, RoundingMode, RoundingStage, Unit,
};
use zumen_building::{Building, Document, ElementRef, Local, Opening, Placement};
use zumen_geom3d::{Failure, Tolerance};

fn options(stage: RoundingStage) -> Options<'static> {
    Options {
        unit: Unit::Volume(LengthUnit::Meter),
        rounding: Rounding {
            mode: RoundingMode::HalfAway,
            digits: 2,
            stage,
        },
        tolerance: Tolerance {
            length: 1e-6,
            chord: 0.001,
        },
        profile: Some(Profile {
            id: "test.volume",
            version: "1",
            hash: [3; 32],
        }),
    }
}

fn building() -> Building {
    Building::new(Document {
        units: vec![mm()],
        ..Document::default()
    })
}

fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
}

#[test]
fn overlapping_parts_are_unioned_and_openings_are_explicit() {
    let mut b = building();
    let at = add(
        &mut b,
        data(
            vec![
                cuboid([0.0; 3], [2000.0, 1000.0, 1000.0]),
                cuboid([1000.0, 0.0, 0.0], [3000.0, 1000.0, 1000.0]),
            ],
            &[0, 1],
            "Body",
        ),
        "IfcWall",
    );
    let hole = add(
        &mut b,
        data(
            vec![cuboid([0.0; 3], [500.0, 1000.0, 1000.0])],
            &[0],
            "Reference",
        ),
        "IfcOpeningElement",
    );
    b.components.opening.insert(hole, Opening::Voids(at));
    let before = b.clone();
    let gross =
        with(|cx| measure::volumes(&b, &[at], false, options(RoundingStage::Total), cx)).unwrap();
    let net =
        with(|cx| measure::volumes(&b, &[at], true, options(RoundingStage::Total), cx)).unwrap();
    near(gross.raw_total.unwrap(), 3.0);
    near(net.raw_total.unwrap(), 2.5);
    assert_eq!(net.rounded_total.unwrap().to_string(), "2.50");
    assert_eq!(net.measurements[0].sources.len(), 2);
    assert_eq!(net.profile.unwrap().hash, [3; 32]);
    assert_eq!(b, before);
}

#[test]
fn rounding_each_element_and_rounding_the_total_are_different() {
    let mut b = building();
    let a = add(
        &mut b,
        data(vec![cuboid([0.0; 3], [500.0; 3])], &[0], "Body"),
        "IfcSpace",
    );
    let c = add(
        &mut b,
        data(vec![cuboid([0.0; 3], [500.0; 3])], &[0], "Body"),
        "IfcSpace",
    );
    let each = with(|cx| measure::volumes(&b, &[a, c], false, options(RoundingStage::Element), cx))
        .unwrap();
    let total =
        with(|cx| measure::volumes(&b, &[a, c], false, options(RoundingStage::Total), cx)).unwrap();
    near(each.raw_total.unwrap(), 0.25);
    assert_eq!(each.rounded_total.unwrap().to_string(), "0.26");
    assert_eq!(total.rounded_total.unwrap().to_string(), "0.25");
    assert!(total.measurements.iter().all(|m| m.rounded.is_none()));
    let cm = Options {
        unit: Unit::Volume(LengthUnit::Centimeter),
        ..options(RoundingStage::Total)
    };
    near(
        with(|cx| measure::volumes(&b, &[a], false, cm, cx))
            .unwrap()
            .raw_total
            .unwrap(),
        125000.0,
    );
}

#[test]
fn selected_parts_keep_parent_openings_without_evaluating_unrelated_geometry() {
    use zumen_building::{Membership, Parent, ParentKind, Spatial, SpatialKind};
    let mut b = fixtures::building();
    let parent = with(|cx| b.push(None, 100, cx)).unwrap();
    b.components.parent.insert(
        ElementRef(0),
        Parent {
            of: Some(Membership {
                target: parent,
                kind: ParentKind::Aggregates,
                order: None,
            }),
            referenced_in: vec![],
        },
    );
    b.components
        .opening
        .insert(ElementRef(4), Opening::Voids(parent));
    // 同じ空間に集約された未対応の表現へは広げない。
    let unrelated = add(
        &mut b,
        data(vec![cuboid([0.0; 3], [1000.0; 3])], &[0], "Unknown"),
        "IfcProxy",
    );
    let storey = with(|cx| b.push(None, 101, cx)).unwrap();
    b.components.spatial.insert(
        storey,
        Spatial {
            kind: SpatialKind::Storey,
            elevation: None,
            ref_elevation: None,
        },
    );
    for child in [parent, unrelated] {
        b.components.parent.insert(
            child,
            Parent {
                of: Some(Membership {
                    target: storey,
                    kind: ParentKind::Aggregates,
                    order: None,
                }),
                referenced_in: vec![],
            },
        );
    }
    let mut diagnostics = Diagnostics::new();
    let out = measure::volumes(
        &b,
        &[ElementRef(0)],
        true,
        options(RoundingStage::Total),
        &mut Cx::new(
            &mut Budget::default(),
            &mut diagnostics,
            &CancelToken::new(),
        ),
    )
    .unwrap();
    near(out.raw_total.unwrap(), 2.58);
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code() == codes::BUILDING_SHAPE_DROPPED)
    );
    assert!(matches!(
        out.method,
        measure::Method::Volume { openings: true }
    ));
}

#[test]
fn an_opening_that_removes_the_whole_solid_leaves_zero_volume() {
    let mut b = building();
    let at = add(
        &mut b,
        data(vec![cuboid([0.0; 3], [500.0; 3])], &[0], "Body"),
        "IfcWall",
    );
    let hole = add(
        &mut b,
        data(vec![cuboid([-100.0; 3], [600.0; 3])], &[0], "Reference"),
        "IfcOpeningElement",
    );
    b.components.opening.insert(hole, Opening::Voids(at));
    let out =
        with(|cx| measure::volumes(&b, &[at], true, options(RoundingStage::Total), cx)).unwrap();
    assert_eq!(out.raw_total, Some(0.0));
    assert_eq!(out.rounded_total.unwrap().to_string(), "0.00");
}

#[test]
fn large_translation_and_mirrored_nonuniform_scale_keep_volume() {
    let mut b = building();
    let at = add(
        &mut b,
        data(vec![cuboid([0.0; 3], [500.0; 3])], &[0], "Body"),
        "IfcSlab",
    );
    b.components.placement.insert(
        at,
        Placement {
            relative_to: None,
            local: Local::Transform(zumen_building::Transform([
                [-2.0, 0.0, 0.0, 1e15],
                [0.0, 3.0, 0.0, -1e15],
                [0.0, 0.0, 4.0, 1e15],
            ])),
        },
    );
    let out =
        with(|cx| measure::volumes(&b, &[at], false, options(RoundingStage::Total), cx)).unwrap();
    near(out.raw_total.unwrap(), 3.0);
}

#[test]
fn missing_shape_never_uses_a_recorded_quantity_and_prevents_a_partial_total() {
    use zumen_building::{Measured, Quantities, Quantity, QuantityKind, QuantitySet};
    let mut b = building();
    let at = with(|cx| b.push(None, 1, cx)).unwrap();
    b.components.quantities.insert(
        at,
        Quantities {
            sets: vec![QuantitySet {
                name: "written".into(),
                method: None,
                quantities: vec![Quantity {
                    name: "volume".into(),
                    kind: QuantityKind::Volume,
                    value: Measured {
                        value: 12345.0,
                        unit: None,
                    },
                    formula: None,
                }],
            }],
        },
    );
    let (mut budget, mut diagnostics, cancel) =
        (Budget::default(), Diagnostics::new(), CancelToken::new());
    let out = measure::volumes(
        &b,
        &[at],
        true,
        options(RoundingStage::Total),
        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
    )
    .unwrap();
    assert!(out.measurements[0].raw.is_none());
    assert!(out.raw_total.is_none());
    assert!(out.rounded_total.is_none());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code() == codes::BUILDING_MEASURE_FAILED)
    );
}

#[test]
fn invalid_selection_units_budgets_and_cancel_return_no_report() {
    let mut b = building();
    let at = add(
        &mut b,
        data(vec![cuboid([0.0; 3], [500.0; 3])], &[0], "Body"),
        "IfcSpace",
    );
    for selection in [vec![], vec![at, at], vec![ElementRef(50)]] {
        assert!(
            with(|cx| measure::volumes(&b, &selection, false, options(RoundingStage::Total), cx))
                .is_err()
        );
    }
    assert!(
        with(|cx| measure::volumes(
            &b,
            &[at],
            false,
            Options {
                unit: Unit::Area(LengthUnit::Meter),
                ..options(RoundingStage::Total)
            },
            cx
        ))
        .is_err()
    );
    for kind in [Kind::Work, Kind::DecodedBytes, Kind::Segments, Kind::Depth] {
        let (mut budget, mut diagnostics, cancel) = (
            Budget::new(Limits::default().with(kind, Limit::Finite(0))),
            Diagnostics::new(),
            CancelToken::new(),
        );
        assert!(matches!(
            measure::volumes(
                &b,
                &[at],
                false,
                options(RoundingStage::Total),
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel)
            ),
            Err(Failure::Stopped(_))
        ));
    }
    let (mut budget, mut diagnostics, cancel) =
        (Budget::default(), Diagnostics::new(), CancelToken::new());
    cancel.cancel();
    assert!(matches!(
        measure::volumes(
            &b,
            &[at],
            false,
            options(RoundingStage::Total),
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel)
        ),
        Err(Failure::Stopped(_))
    ));
}
