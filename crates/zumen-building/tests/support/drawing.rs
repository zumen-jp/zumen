//! 合成した切り出しの入力。検査とネイティブ・wasm の照合で共用する。
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "試験用の既知の値"
)]
use super::fixtures::*;
use zumen_building::{
    drawing::{Options, Paper, View},
    *,
};
use zumen_crs::ProjectPoint;
use zumen_drawing::PaperLength;
pub fn paper() -> Paper {
    Paper {
        denominator: 100.0,
        origin: [PaperLength(20.0), PaperLength(30.0)],
        size: Some([PaperLength(297.0), PaperLength(210.0)]),
    }
}
pub fn options(view: View) -> Options<'static> {
    Options {
        view,
        tolerance: 0.01,
        chord: 1.0,
        paper: paper(),
        symbols: &[],
    }
}
pub fn point(x: f64, y: f64, z: f64) -> ProjectPoint {
    ProjectPoint { x, y, z }
}
pub fn cube() -> Building {
    let mut b = Building::new(Document {
        units: vec![mm()],
        ..Document::default()
    });
    let mut shape = cuboid([0.0; 3], [1000.0; 3]);
    shape.source.instance = Some(42);
    let e = add(&mut b, data(vec![shape], &[0], "Body"), "IfcWall");
    b.source_mut(e).unwrap().global_id = Some("0000000000000000000001".into());
    b
}

pub fn line_graph(length: f64) -> CurveGraph {
    CurveGraph {
        root: CurveRef(0),
        items: vec![CurveItem {
            kind: CurveKind::Polyline(vec![
                Coordinates::Two([0.0, 0.0]),
                Coordinates::Two([length, 0.0]),
            ]),
            source: CurveSource::default(),
        }],
        length_unit: Some(mm()),
        angle_unit: None,
    }
}

pub fn door() -> Building {
    let mut b = cube();
    b.components.class.insert(
        ElementRef(0),
        Class {
            name: "IfcDoor".into(),
            original: "IfcDoor".into(),
            predefined: None,
            user_defined: None,
        },
    );
    b.components.properties.insert(
        ElementRef(0),
        Properties {
            sets: vec![PropertySet {
                name: "panel".into(),
                properties: vec![Property {
                    name: "PanelOperation".into(),
                    unit: None,
                    value: Some(TypedValue {
                        type_name: Some("IfcDoorPanelOperationEnum".into()),
                        value: Literal::Enumeration("SWINGING".into()),
                    }),
                }],
            }],
        },
    );
    b
}

pub fn add_plan(b: &mut Building) {
    let mut geometry = b.geometry(ElementRef(0)).unwrap().data().clone();
    let root = geometry.root.unwrap();
    let curve = GeometryRef(geometry.nodes.len() as u32);
    let mut item = node(
        GeometryLinks::Leaf,
        GeometryDefinition::Curve(line_graph(123.0)),
    );
    item.source.instance = Some(99);
    geometry.nodes.push(item);
    let representation = GeometryRef(geometry.nodes.len() as u32);
    let GeometryLinks::Definition(ref mut reps) = geometry.nodes[root.0 as usize].links else {
        panic!()
    };
    reps.push(representation);
    geometry.nodes.push(node(
        GeometryLinks::Representation {
            identifier: Some("Annotation".into()),
            kind: Some("Curve2D".into()),
            items: vec![curve],
        },
        GeometryDefinition::Context {
            definition: RepresentationContext {
                geometric: GeometricContext {
                    identifier: None,
                    kind: Some("Plan".into()),
                    dimension: 2,
                    precision: Some(0.001),
                    world_coordinate_system: ContextPlacement::Plane(AxisPlacement2 {
                        location: [0.0; 2],
                        reference_direction: None,
                    }),
                    true_north: None,
                },
                view: None,
            },
            length_unit: Some(mm()),
        },
    ));
    with(|cx| b.insert_geometry(ElementRef(0), Geometry::new(geometry, cx).unwrap(), cx)).unwrap();
}

pub fn add_grid(b: &mut Building) -> (ElementRef, ElementRef) {
    let owner = with(|cx| b.push(None, 201, cx)).unwrap();
    let at = with(|cx| b.push(None, 202, cx)).unwrap();
    let mut curve = line_graph(2000.0);
    curve.items[0].source.instance = Some(203);
    b.components.grid.insert(
        owner,
        Grid::Grid {
            axes: vec![(GridSet::U, at)],
        },
    );
    b.components.grid.insert(
        at,
        Grid::Axis {
            tag: Some("A".into()),
            same_sense: true,
            curve: None,
        },
    );
    let grid = GeometryGrid {
        axes: vec![GeometryGridAxis {
            definition: GridAxisDefinition {
                set: GridSet::U,
                tag: Some("A".into()),
                same_sense: true,
            },
            curve,
            source: GeometrySource {
                instance: Some(202),
                ..GeometrySource::default()
            },
        }],
        layout: Some(GridLayout::Rectangular),
        source: GeometrySource::default(),
    };
    let geometry = GeometryData {
        root: None,
        nodes: vec![],
        styles: vec![],
        grid: Some(Acquired::Value(grid)),
        source: GeometrySource::default(),
    };
    with(|cx| b.insert_geometry(owner, Geometry::new(geometry, cx).unwrap(), cx)).unwrap();
    (owner, at)
}
