//! 元の領域の水平断面で、開口・部品の重なり・大きな座標を確かめる。
#![cfg(feature = "measure")]

#[path = "support/evaluation.rs"]
#[allow(dead_code, reason = "形状評価と共有する自作の入力の一部を使う")]
mod fixtures;

use fixtures::{add, cuboid, data, mm, with};
use zumen_building::measure::{
    self, LengthUnit, Options, Rounding, RoundingMode, RoundingStage, Unit,
};
use zumen_building::{Building, Document, Local, Opening, Placement, Transform};
use zumen_crs::ProjectPoint;
use zumen_geom3d::Tolerance;

fn options() -> Options<'static> {
    Options {
        unit: Unit::Area(LengthUnit::Meter),
        rounding: Rounding {
            mode: RoundingMode::HalfAway,
            digits: 3,
            stage: RoundingStage::Total,
        },
        tolerance: Tolerance {
            length: 1e-6,
            chord: 0.001,
        },
        profile: None,
    }
}

#[test]
fn section_counts_overlap_once_and_removes_an_interior_hole() {
    let mut b = Building::new(Document {
        units: vec![mm()],
        ..Document::default()
    });
    let at = add(
        &mut b,
        data(
            vec![
                cuboid([0.0; 3], [3000.0, 2000.0, 3000.0]),
                cuboid([2000.0, 0.0, 0.0], [4000.0, 2000.0, 3000.0]),
            ],
            &[0, 1],
            "Body",
        ),
        "IfcSpace",
    );
    let hole = add(
        &mut b,
        data(
            vec![cuboid([1000.0, 500.0, -100.0], [2000.0, 1500.0, 4000.0])],
            &[0],
            "Reference",
        ),
        "IfcOpeningElement",
    );
    b.components.opening.insert(hole, Opening::Voids(at));
    let origin = ProjectPoint {
        x: 0.0,
        y: 0.0,
        z: 1000.0,
    };
    let out = with(|cx| measure::section_areas(&b, &[at], origin, options(), cx)).unwrap();
    assert!((out.raw_total.unwrap() - 7.0).abs() < 1e-10);
    assert_eq!(out.rounded_total.unwrap().to_string(), "7.000");
    let outside = with(|cx| {
        measure::section_areas(
            &b,
            &[at],
            ProjectPoint {
                z: 4000.0,
                ..origin
            },
            options(),
            cx,
        )
    })
    .unwrap();
    assert_eq!(outside.raw_total, Some(0.0));
    let translation = Transform::translation(1e15, -1e15, 0.0);
    for element in [at, hole] {
        b.components.placement.insert(
            element,
            Placement {
                relative_to: None,
                local: Local::Transform(translation),
            },
        );
    }
    // 開口の演算に大きな世界座標を渡さない場合にも、断面の原点との差は先に取る。
    b.components.opening.remove(hole);
    let translated = with(|cx| {
        measure::section_areas(
            &b,
            &[at],
            ProjectPoint {
                x: 1e15,
                y: -1e15,
                ..origin
            },
            options(),
            cx,
        )
    })
    .unwrap();
    assert!((translated.raw_total.unwrap() - 8.0).abs() < 1e-10);
}

#[test]
fn circular_profile_area_uses_the_radius_independently_of_chord_error() -> Result<(), &'static str>
{
    use zumen_building::{AreaProfileKind, GeometryDefinition};
    let mut b = Building::new(Document {
        units: vec![mm()],
        ..Document::default()
    });
    let mut cylinder = cuboid([-1000.0, -1000.0, 0.0], [1000.0, 1000.0, 3000.0]);
    let GeometryDefinition::Extrusion { definition, .. } = &mut cylinder.definition else {
        return Err("固定入力は押し出しのはず");
    };
    definition.profile.kind = AreaProfileKind::Circle { radius: 1000.0 };
    // 3-4-5 の斜めの押し出し。水平面積は変わらず、鉛直の高さは 2.4 m。
    definition.direction = [3.0, 0.0, 4.0];
    let at = add(&mut b, data(vec![cylinder], &[0], "Body"), "IfcSpace");
    b.components.placement.insert(
        at,
        Placement {
            relative_to: None,
            local: Local::Transform(Transform([
                [-2.0, 0.0, 0.0, 1e15],
                [0.0, 3.0, 0.0, -1e15],
                [0.0, 0.0, 1.0, 0.0],
            ])),
        },
    );
    for chord in [0.01, 0.0001] {
        let opt = Options {
            tolerance: Tolerance {
                chord,
                ..options().tolerance
            },
            ..options()
        };
        let out = with(|cx| {
            measure::section_areas(
                &b,
                &[at],
                ProjectPoint {
                    x: 1e15,
                    y: -1e15,
                    z: 1000.0,
                },
                opt,
                cx,
            )
        })
        .unwrap();
        assert!((out.raw_total.unwrap() - 6.0 * core::f64::consts::PI).abs() < 1e-12);
        assert_eq!(
            out.measurements[0].sources[0].basis,
            measure::SourceBasis::Definition
        );
        let volume = with(|cx| {
            measure::volumes(
                &b,
                &[at],
                false,
                Options {
                    unit: Unit::Volume(LengthUnit::Meter),
                    ..opt
                },
                cx,
            )
        })
        .unwrap();
        assert!((volume.raw_total.unwrap() - 14.4 * core::f64::consts::PI).abs() < 1e-12);
        assert_eq!(
            volume.measurements[0].sources[0].basis,
            measure::SourceBasis::Definition
        );
    }
    Ok(())
}
