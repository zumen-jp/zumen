//! 作図の表現だけを検査する自作のモデル。
#![allow(clippy::unwrap_used, reason = "固定した試験入力")]
#![allow(dead_code, reason = "試験と照合用の例で共有する")]
#[path = "building.rs"]
mod base;
pub use base::{add, unit, with};
use zumen_building::*;
use zumen_profile::building_drawing::DrawingProfile;
pub const BUILDING_DRAWING: &[u8] =
    include_bytes!("../../../../profiles/zumen-building-drawing/2026-11/profile.json");

pub fn profile() -> DrawingProfile {
    with(|cx| {
        DrawingProfile::load(
            &[("base", BUILDING_DRAWING)],
            "zumen-building-drawing",
            "2026-11",
            cx,
        )
    })
    .unwrap()
}
pub fn origin() -> zumen_crs::ProjectPoint {
    zumen_crs::ProjectPoint {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    }
}
pub fn building() -> Building {
    let mut b = Building::new(Document {
        units: vec![unit()],
        ..Document::default()
    });
    let wall = add(&mut b, "IfcWall", 2.0);
    b.components.material.insert(
        wall,
        Material {
            kind: MaterialKind::Single,
            parts: vec![MaterialPart {
                material: None,
                name: Some("Concrete".into()),
                thickness: None,
            }],
            direction: None,
            offset: None,
        },
    );
    b
}
pub fn attribute(name: &str, type_name: &str, value: Literal) -> ExtensionValue {
    ExtensionValue {
        namespace: "ifc.attribute".into(),
        name: name.into(),
        value: TypedValue {
            type_name: Some(type_name.into()),
            value,
        },
    }
}
pub fn door(b: &mut Building, operation: &str) -> ElementRef {
    let at = add(b, "IfcDoor", 0.9);
    b.components.extension.insert(
        at,
        Extension {
            values: vec![
                attribute(
                    "OverallWidth",
                    "IfcPositiveLengthMeasure",
                    Literal::Real(0.9),
                ),
                attribute(
                    "OperationType",
                    "IfcDoorTypeOperationEnum",
                    Literal::Enumeration(operation.into()),
                ),
            ],
        },
    );
    b.components.placement.insert(
        at,
        Placement {
            relative_to: None,
            local: Local::Transform(Transform::translation(4.0, 0.0, 0.0)),
        },
    );
    at
}
pub fn grid(b: &mut Building) -> ElementRef {
    let owner = with(|cx| b.push(None, 899, cx)).unwrap();
    let axis = with(|cx| b.push(None, 900, cx)).unwrap();
    b.components.grid.insert(
        axis,
        Grid::Axis {
            tag: Some("X1".into()),
            same_sense: true,
            curve: None,
        },
    );
    b.components.grid.insert(
        owner,
        Grid::Grid {
            axes: vec![(GridSet::U, axis)],
        },
    );
    let curve = CurveGraph {
        root: CurveRef(0),
        items: vec![CurveItem {
            kind: CurveKind::Polyline(vec![
                Coordinates::Two([-2.0, 2.0]),
                Coordinates::Two([0.0, 2.0]),
                Coordinates::Two([2.0, 2.0]),
            ]),
            source: CurveSource::default(),
        }],
        length_unit: Some(unit()),
        angle_unit: None,
    };
    let grid = GeometryGrid {
        axes: vec![GeometryGridAxis {
            definition: GridAxisDefinition {
                set: GridSet::U,
                tag: Some("X1".into()),
                same_sense: true,
            },
            curve,
            source: GeometrySource::default(),
        }],
        layout: Some(GridLayout::Rectangular),
        source: GeometrySource::default(),
    };
    with(|cx| {
        b.insert_geometry(
            owner,
            Geometry::new(
                GeometryData {
                    root: None,
                    nodes: vec![],
                    styles: vec![],
                    grid: Some(Acquired::Value(grid)),
                    source: GeometrySource::default(),
                },
                cx,
            )
            .unwrap(),
            cx,
        )
    })
    .unwrap();
    axis
}

pub fn hole(b: &mut Building) {
    let opening = add(b, "IfcOpeningElement", 0.5);
    let mut shape = base::geometry(0.5);
    if let Some(GeometryNode {
        definition: GeometryDefinition::Extrusion { definition, .. },
        ..
    }) = shape.nodes.first_mut()
    {
        definition.profile.kind = AreaProfileKind::Rectangle {
            x_dim: 0.5,
            y_dim: 0.4,
        };
    }
    with(|cx| b.insert_geometry(opening, Geometry::new(shape, cx).unwrap(), cx)).unwrap();
    b.components
        .opening
        .insert(opening, Opening::Voids(ElementRef(0)));
}
pub fn mapped(b: &mut Building, scale: f64, translation: [f64; 3]) {
    let at = ElementRef(0);
    let mut data = b.geometry(at).unwrap().data().clone();
    let mapped = GeometryRef(data.nodes.len() as u32);
    data.nodes.push(GeometryNode {
        links: GeometryLinks::Mapped(GeometryRef(0)),
        definition: GeometryDefinition::Mapping {
            definition: ShapeMapping3 {
                origin: AxisPlacement3 {
                    location: [0.0; 3],
                    axis: None,
                    reference_direction: None,
                },
                target: CartesianMapping3 {
                    local_origin: translation,
                    axis1: None,
                    axis2: None,
                    axis3: None,
                    scale: MappingScale3::Uniform(Some(scale)),
                },
            },
            length_unit: Some(unit()),
        },
        source: GeometrySource::default(),
    });
    if let GeometryLinks::Representation { items, .. } = &mut data.nodes.get_mut(1).unwrap().links {
        items.push(mapped);
    }
    with(|cx| b.insert_geometry(at, Geometry::new(data, cx).unwrap(), cx)).unwrap();
}
pub fn cut(profile: &DrawingProfile, b: &Building) -> zumen_profile::building_drawing::Drawing {
    with(|cx| {
        let view = profile.default_plan(b, origin(), cx)?;
        profile.cut(b, view, 0.00001, 0.001, None, cx)
    })
    .unwrap()
}
