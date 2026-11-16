//! 数量の試験・環境間の照合に使う、4 面の壁と 1 室の自作入力。

#![allow(clippy::unwrap_used, reason = "試験と照合の固定入力")]

use super::fixtures::{add, cuboid, data, mm, node, with};
use zumen_building::measure::{
    self, Area, Boundary, InputBoundary, LengthUnit, Options, Rounding, RoundingMode, RoundingStage,
};
use zumen_building::*;
use zumen_crs::ProjectPoint;
use zumen_geom3d::Tolerance;

pub fn options() -> Options<'static> {
    Options {
        unit: measure::Unit::Area(LengthUnit::Meter),
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

pub fn request(walls: &[ElementRef], boundary: Boundary, input: InputBoundary) -> Area<'_> {
    Area {
        origin: ProjectPoint {
            x: 0.0,
            y: 0.0,
            z: 1000.0,
        },
        boundary,
        input,
        walls,
    }
}

pub fn wall_data(length: f64, axis_offset: Option<f64>, thickness: f64) -> GeometryData {
    let mut g = data(
        vec![cuboid(
            [0.0, -thickness / 2.0, 0.0],
            [length, thickness / 2.0, 3000.0],
        )],
        &[0],
        "Body",
    );
    if let Some(offset) = axis_offset {
        let curve = g.nodes.len() as u32;
        g.nodes.push(node(
            GeometryLinks::Leaf,
            GeometryDefinition::Curve(CurveGraph {
                root: CurveRef(0),
                items: vec![CurveItem {
                    source: CurveSource::default(),
                    kind: CurveKind::Polyline(vec![
                        Coordinates::Two([0.0, offset]),
                        Coordinates::Two([length, offset]),
                    ]),
                }],
                length_unit: Some(mm()),
                angle_unit: None,
            }),
        ));
        let mut rep = g.nodes.get(1).unwrap().clone();
        rep.links = GeometryLinks::Representation {
            identifier: Some("Axis".into()),
            kind: Some("Curve2D".into()),
            items: vec![GeometryRef(curve)],
        };
        let rep_ref = GeometryRef(g.nodes.len() as u32);
        g.nodes.push(rep);
        g.nodes.get_mut(2).unwrap().links =
            GeometryLinks::Definition(vec![GeometryRef(1), rep_ref]);
    }
    g
}

pub fn material(thickness: f64) -> Material {
    Material {
        kind: MaterialKind::Layers,
        parts: vec![MaterialPart {
            material: None,
            name: None,
            thickness: Some(Measured {
                value: thickness,
                unit: Some(mm()),
            }),
        }],
        direction: Some("AXIS2".into()),
        offset: Some(Measured {
            value: -thickness / 2.0,
            unit: Some(mm()),
        }),
    }
}

pub fn room(
    inside: bool,
    axis_offset: Option<f64>,
    layers: bool,
) -> (Building, ElementRef, Vec<ElementRef>) {
    let mut b = Building::new(Document {
        units: vec![mm()],
        ..Document::default()
    });
    let shrink = if inside { 100.0 } else { 0.0 };
    let room = add(
        &mut b,
        data(
            vec![cuboid(
                [shrink, shrink, 0.0],
                [4000.0 - shrink, 3000.0 - shrink, 3000.0],
            )],
            &[0],
            "Body",
        ),
        "IfcSpace",
    );
    let mut walls = Vec::new();
    for (length, transform) in [
        (4000.0, Transform::IDENTITY),
        (4000.0, Transform::translation(0.0, 3000.0, 0.0)),
        (
            3000.0,
            Transform([
                [0.0, -1.0, 0.0, 0.0],
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
            ]),
        ),
        (
            3000.0,
            Transform([
                [0.0, -1.0, 0.0, 4000.0],
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
            ]),
        ),
    ] {
        let at = add(&mut b, wall_data(length, axis_offset, 200.0), "IfcWall");
        b.components.placement.insert(
            at,
            Placement {
                relative_to: None,
                local: Local::Transform(transform),
            },
        );
        if layers {
            b.components.material.insert(at, material(200.0));
        }
        walls.push(at);
    }
    (b, room, walls)
}

pub fn replace_wall(b: &mut Building, wall: ElementRef, data: GeometryData) {
    with(|cx| b.insert_geometry(wall, Geometry::new(data, cx).unwrap(), cx)).unwrap();
}
