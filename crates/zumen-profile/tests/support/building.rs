//! プロファイルを照らすためだけの、メートル単位の自作の建物。
#![allow(clippy::unwrap_used, reason = "試験・照合の固定入力")]
#![allow(dead_code, reason = "試験・例で必要な入力を共有する")]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_building::*;
use zumen_profile::building::BuildingProfile;

pub const BASE: &[u8] =
    include_bytes!("../../../../profiles/mlit-bim-drawing/2026-03/profile.json");
pub const CHILD: &[u8] = br#"{
  "format":"0.1", "id":"test.permit", "edition":"1", "revision":1,
  "extends":{"id":"mlit-bim-drawing","edition":"2026-03"},
  "rules":[
    {"id":"room-use","check":"property-present","level":"warning","classes":["spaces"],"property-set":"Test","property":"use"},
    {"id":"area-region-code","check":"property-present","level":"warning","classes":["spaces"],"property-set":"Test","property":"code"},
    {"id":"floor-area-ratio","check":"property-present","level":"warning","classes":["spaces"],"property-set":"Test","property":"ratio"},
    {"id":"fire-compartment","check":"property-present","level":"warning","classes":["walls"],"property-set":"Test","property":"fire"}
  ]
}"#;

pub fn with<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> T {
    f(&mut Cx::new(
        &mut Budget::default(),
        &mut Diagnostics::new(),
        &CancelToken::new(),
    ))
}
pub fn profile(child: bool) -> BuildingProfile {
    with(|cx| {
        if child {
            BuildingProfile::load(&[("base", BASE), ("child", CHILD)], "test.permit", "1", cx)
        } else {
            BuildingProfile::load(&[("base", BASE)], "mlit-bim-drawing", "2026-03", cx)
        }
    })
    .unwrap()
}
pub fn unit() -> Unit {
    Unit {
        kind: "LENGTHUNIT".into(),
        name: "METRE".into(),
        prefix: None,
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
pub fn geometry(width: f64) -> GeometryData {
    let node = |links, definition| GeometryNode {
        links,
        definition,
        source: GeometrySource::default(),
    };
    GeometryData {
        root: Some(GeometryRef(2)),
        grid: None,
        styles: vec![],
        source: GeometrySource::default(),
        nodes: vec![
            node(
                GeometryLinks::Leaf,
                GeometryDefinition::Extrusion {
                    definition: ProfileExtrusion {
                        profile: AreaProfile {
                            name: None,
                            position: None,
                            kind: AreaProfileKind::Rectangle {
                                x_dim: width,
                                y_dim: 1.0,
                            },
                        },
                        position: None,
                        direction: [0.0, 0.0, 1.0],
                        depth: 3.0,
                    },
                    boundary: None,
                    length_unit: Some(unit()),
                },
            ),
            node(
                GeometryLinks::Representation {
                    identifier: Some("Body".into()),
                    kind: Some("SweptSolid".into()),
                    items: vec![GeometryRef(0)],
                },
                GeometryDefinition::Context {
                    definition: RepresentationContext {
                        geometric: GeometricContext {
                            identifier: None,
                            kind: Some("Model".into()),
                            dimension: 3,
                            precision: None,
                            world_coordinate_system: ContextPlacement::Space(axis([0.0; 3])),
                            true_north: None,
                        },
                        view: None,
                    },
                    length_unit: Some(unit()),
                },
            ),
            node(
                GeometryLinks::Definition(vec![GeometryRef(1)]),
                GeometryDefinition::Links,
            ),
        ],
    }
}
pub fn add(b: &mut Building, class: &str, width: f64) -> ElementRef {
    with(|cx| {
        let at = b.push(None, b.elements().len() as u64 + 1, cx).unwrap();
        b.components.class.insert(
            at,
            Class {
                name: class.into(),
                original: class.into(),
                predefined: None,
                user_defined: None,
            },
        );
        b.components.placement.insert(
            at,
            Placement {
                relative_to: None,
                local: Local::Transform(Transform::IDENTITY),
            },
        );
        b.components.representation.insert(
            at,
            Representation {
                representations: vec![ShapeRepresentation {
                    context: Some("Body".into()),
                    kind: Some("SweptSolid".into()),
                    items: vec![ShapeItem {
                        kind: "IfcExtrudedAreaSolid".into(),
                        children: vec![],
                    }],
                }],
            },
        );
        b.insert_geometry(at, Geometry::new(geometry(width), cx).unwrap(), cx)
            .unwrap();
        at
    })
}
pub fn properties(b: &mut Building, at: ElementRef, values: &[(&str, Option<Literal>)]) {
    b.components.properties.insert(
        at,
        Properties {
            sets: vec![PropertySet {
                name: "Test".into(),
                properties: values
                    .iter()
                    .map(|(name, value)| Property {
                        name: (*name).into(),
                        value: value.clone().map(|value| TypedValue {
                            type_name: None,
                            value,
                        }),
                        unit: None,
                    })
                    .collect(),
            }],
        },
    );
}
pub fn building() -> Building {
    let mut b = Building::new(Document {
        units: vec![unit()],
        header: Header {
            schemas: vec!["IFC2X3".into()],
            ..Header::default()
        },
        ..Document::default()
    });
    let anchor = add(&mut b, "IfcBuilding", 1.0);
    b.components.spatial.insert(
        anchor,
        Spatial {
            kind: SpatialKind::Building,
            elevation: None,
            ref_elevation: None,
        },
    );
    let a = add(&mut b, "IfcSpace", 1.006);
    let c = add(&mut b, "IfcSpace", 1.006);
    let wall = add(&mut b, "IfcWallStandardCase", 0.2);
    for at in [a, c] {
        properties(
            &mut b,
            at,
            &[
                ("use", Some(Literal::Text("test".into()))),
                ("code", Some(Literal::Text("A".into()))),
                ("ratio", Some(Literal::Boolean(false))),
            ],
        );
    }
    properties(
        &mut b,
        wall,
        &[("fire", Some(Literal::Text("test".into())))],
    );
    b
}
pub fn origin() -> zumen_crs::ProjectPoint {
    zumen_crs::ProjectPoint {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    }
}
pub fn tolerance() -> zumen_geom3d::Tolerance {
    zumen_geom3d::Tolerance {
        length: 1e-6,
        chord: 0.001,
    }
}
