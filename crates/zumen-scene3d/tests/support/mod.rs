//! シーンと環境間の照合に使う、自作の建物。
#![allow(clippy::unwrap_used, reason = "合成した試験入力を組み立てる")]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_building::*;

pub fn with<T>(operation: impl FnOnce(&mut Cx) -> T) -> T {
    operation(&mut Cx::new(
        &mut Budget::default(),
        &mut Diagnostics::new(),
        &CancelToken::new(),
    ))
}

pub fn unit() -> Unit {
    Unit {
        kind: "LENGTHUNIT".into(),
        prefix: Some("MILLI".into()),
        name: "METRE".into(),
        scale: None,
    }
}

fn node(links: GeometryLinks, definition: GeometryDefinition) -> GeometryNode {
    GeometryNode {
        links,
        definition,
        source: GeometrySource::default(),
    }
}

pub fn geometry(kind: AreaProfileKind, copies: u32) -> GeometryData {
    let mut nodes = vec![node(
        GeometryLinks::Leaf,
        GeometryDefinition::Extrusion {
            definition: ProfileExtrusion {
                profile: AreaProfile {
                    name: None,
                    position: None,
                    kind,
                },
                position: None,
                direction: [0.0, 0.0, 1.0],
                depth: 1000.0,
            },
            boundary: None,
            length_unit: Some(unit()),
        },
    )];
    nodes.push(node(
        GeometryLinks::Representation {
            identifier: Some("Body".into()),
            kind: None,
            items: vec![GeometryRef(0)],
        },
        GeometryDefinition::Links,
    ));
    nodes.push(node(
        GeometryLinks::Map(GeometryRef(1)),
        GeometryDefinition::Links,
    ));
    let mut items = Vec::new();
    for i in 0..copies {
        items.push(GeometryRef(nodes.len() as u32));
        nodes.push(node(
            GeometryLinks::Mapped(GeometryRef(2)),
            GeometryDefinition::Mapping {
                definition: ShapeMapping3 {
                    origin: AxisPlacement3 {
                        location: [0.0; 3],
                        axis: None,
                        reference_direction: None,
                    },
                    target: CartesianMapping3 {
                        local_origin: [f64::from(i) * 2000.0, 0.0, 0.0],
                        axis1: None,
                        axis2: None,
                        axis3: None,
                        scale: MappingScale3::Uniform(None),
                    },
                },
                length_unit: Some(unit()),
            },
        ));
    }
    let representation = GeometryRef(nodes.len() as u32);
    nodes.push(node(
        GeometryLinks::Representation {
            identifier: Some("Body".into()),
            kind: None,
            items,
        },
        GeometryDefinition::Links,
    ));
    let root = GeometryRef(nodes.len() as u32);
    nodes.push(node(
        GeometryLinks::Definition(vec![representation]),
        GeometryDefinition::Links,
    ));
    GeometryData {
        root: Some(root),
        nodes,
        styles: Vec::new(),
        grid: None,
        source: GeometrySource::default(),
    }
}

pub fn add(building: &mut Building, class: &str, shape: Option<GeometryData>) -> ElementRef {
    with(|cx| {
        let at = building
            .push(None, building.elements().len() as u64 + 1, cx)
            .unwrap();
        building.components.class.insert(
            at,
            Class {
                name: class.into(),
                predefined: None,
                user_defined: None,
                original: class.into(),
            },
        );
        if let Some(shape) = shape {
            building
                .insert_geometry(at, Geometry::new(shape, cx).unwrap(), cx)
                .unwrap();
        }
        at
    })
}

pub fn building() -> Building {
    let mut building = Building::new(Document {
        units: vec![unit()],
        ..Document::default()
    });
    let floor = add(&mut building, "IfcBuildingStorey", None);
    building.components.spatial.insert(
        floor,
        Spatial {
            kind: SpatialKind::Storey,
            elevation: None,
            ref_elevation: None,
        },
    );
    let first = add(
        &mut building,
        "IfcWall",
        Some(geometry(
            AreaProfileKind::Rectangle {
                x_dim: 1000.0,
                y_dim: 200.0,
            },
            2,
        )),
    );
    let second = add(
        &mut building,
        "IfcWall",
        Some(geometry(
            AreaProfileKind::Rectangle {
                x_dim: 1000.0,
                y_dim: 200.0,
            },
            1,
        )),
    );
    for (at, x) in [(first, 1e12), (second, 1e12 + 6000.0)] {
        building.components.parent.insert(
            at,
            Parent {
                of: Some(Membership {
                    target: floor,
                    kind: ParentKind::Contained,
                    order: None,
                }),
                referenced_in: Vec::new(),
            },
        );
        building.components.placement.insert(
            at,
            Placement {
                relative_to: None,
                local: Local::Transform(Transform::translation(x, 2000.0, 0.0)),
            },
        );
    }
    building.components.identity.insert(
        first,
        Identity {
            name: Some("壁".into()),
            ..Identity::default()
        },
    );
    building.components.style.insert(
        first,
        Style {
            surface_color: Some([0.25, 0.5, 1.0]),
            transparency: Some(0.25),
            ..Style::default()
        },
    );
    building
}
