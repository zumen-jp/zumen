//! 形状評価の試験と環境間の照合で使う、自作の建物。

#![allow(clippy::unwrap_used, reason = "試験と照合用の実行ファイルの固定入力")]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_building::*;

pub fn with<T>(f: impl FnOnce(&mut Cx) -> T) -> T {
    f(&mut Cx::new(
        &mut Budget::default(),
        &mut Diagnostics::new(),
        &CancelToken::new(),
    ))
}

pub fn mm() -> Unit {
    Unit {
        kind: "LENGTHUNIT".into(),
        prefix: Some("MILLI".into()),
        name: "METRE".into(),
        scale: None,
    }
}

pub fn axis(location: [f64; 3]) -> AxisPlacement3 {
    AxisPlacement3 {
        location,
        axis: None,
        reference_direction: None,
    }
}

pub fn node(links: GeometryLinks, definition: GeometryDefinition) -> GeometryNode {
    GeometryNode {
        links,
        definition,
        source: GeometrySource::default(),
    }
}

pub fn cuboid(lo: [f64; 3], hi: [f64; 3]) -> GeometryNode {
    node(
        GeometryLinks::Leaf,
        GeometryDefinition::Extrusion {
            definition: ProfileExtrusion {
                profile: AreaProfile {
                    name: None,
                    position: None,
                    kind: AreaProfileKind::Rectangle {
                        x_dim: hi[0] - lo[0],
                        y_dim: hi[1] - lo[1],
                    },
                },
                position: Some(axis([(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, lo[2]])),
                direction: [0.0, 0.0, 1.0],
                depth: hi[2] - lo[2],
            },
            boundary: None,
            length_unit: Some(mm()),
        },
    )
}

pub fn data(mut nodes: Vec<GeometryNode>, roots: &[u32], identifier: &str) -> GeometryData {
    let representation = nodes.len() as u32;
    nodes.push(node(
        GeometryLinks::Representation {
            identifier: Some(identifier.into()),
            kind: Some("SolidModel".into()),
            items: roots.iter().copied().map(GeometryRef).collect(),
        },
        GeometryDefinition::Context {
            definition: RepresentationContext {
                geometric: GeometricContext {
                    identifier: None,
                    kind: Some("Model".into()),
                    dimension: 3,
                    precision: Some(0.001),
                    world_coordinate_system: ContextPlacement::Space(axis([0.0; 3])),
                    true_north: None,
                },
                view: Some(SubContext {
                    identifier: Some(identifier.into()),
                    kind: None,
                    target_scale: None,
                    target_view: TargetView::Model,
                    user_defined_target_view: None,
                }),
            },
            length_unit: Some(mm()),
        },
    ));
    let root = nodes.len() as u32;
    nodes.push(node(
        GeometryLinks::Definition(vec![GeometryRef(representation)]),
        GeometryDefinition::Links,
    ));
    GeometryData {
        root: Some(GeometryRef(root)),
        nodes,
        styles: vec![],
        grid: None,
        source: GeometrySource::default(),
    }
}

pub fn add(building: &mut Building, data: GeometryData, class: &str) -> ElementRef {
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
        building
            .insert_geometry(at, Geometry::new(data, cx).unwrap(), cx)
            .unwrap();
        at
    })
}

pub fn building() -> Building {
    let mut b = Building::new(Document {
        units: vec![mm()],
        ..Document::default()
    });
    let wall = add(
        &mut b,
        data(
            vec![cuboid([0.0; 3], [5000.0, 200.0, 3000.0])],
            &[0],
            "Body",
        ),
        "IfcWall",
    );
    let slab = add(
        &mut b,
        data(
            vec![cuboid([0.0, 0.0, -250.0], [6000.0, 4000.0, 0.0])],
            &[0],
            "Body",
        ),
        "IfcSlab",
    );
    let door = add(
        &mut b,
        data(
            vec![
                cuboid([0.0; 3], [1000.0, 60.0, 2100.0]),
                cuboid([100.0, -10.0, 0.0], [900.0, 70.0, 1900.0]),
                node(
                    GeometryLinks::Boolean {
                        operation: BooleanOperation::Difference,
                        first: GeometryRef(0),
                        second: GeometryRef(1),
                    },
                    GeometryDefinition::Links,
                ),
            ],
            &[2],
            "Body",
        ),
        "IfcDoor",
    );
    let window = add(
        &mut b,
        data(
            vec![
                cuboid([0.0; 3], [1000.0, 60.0, 1000.0]),
                cuboid([100.0, -10.0, 100.0], [900.0, 70.0, 900.0]),
                node(
                    GeometryLinks::Boolean {
                        operation: BooleanOperation::Difference,
                        first: GeometryRef(0),
                        second: GeometryRef(1),
                    },
                    GeometryDefinition::Links,
                ),
            ],
            &[2],
            "Body",
        ),
        "IfcWindow",
    );
    let wall_opening = add(
        &mut b,
        data(
            vec![cuboid([500.0, -100.0, 0.0], [1500.0, 300.0, 2100.0])],
            &[0],
            "Reference",
        ),
        "IfcOpeningElement",
    );
    let slab_opening = add(
        &mut b,
        data(
            vec![cuboid([1000.0, 1000.0, -500.0], [3000.0, 2000.0, 250.0])],
            &[0],
            "Body",
        ),
        "IfcOpeningElement",
    );
    for at in [wall, slab, door, window] {
        b.components.placement.insert(
            at,
            Placement {
                relative_to: None,
                local: Local::Transform(Transform::translation(1e9, 2e9, 3000.0)),
            },
        );
    }
    for (at, host) in [(wall_opening, wall), (slab_opening, slab)] {
        b.components.opening.insert(at, Opening::Voids(host));
        b.components.placement.insert(
            at,
            Placement {
                relative_to: Some(host),
                local: Local::Transform(Transform::IDENTITY),
            },
        );
    }
    b.components
        .opening
        .insert(door, Opening::Fills(wall_opening));
    b
}
