//! 形を書く試験の補助。読んだ形を、書くときの値（`NewShape`）に戻して比べる。

use zumen_building::{
    AreaProfileKind, Building, Coordinates, CurveGraph, CurveKind, CurveRef, ElementRef,
    GeometryDefinition, GeometryLinks, GeometryNode, GeometryRef,
};
use zumen_ifc::{NewAxis2, NewAxis3, NewProfile, NewProfileKind, NewShape, NewSolid};

/// 要素の `Axis` の折れ線と `Body` の押し出し。ほかの表現や項目があれば `None`
pub fn shape_of(building: &Building, element: ElementRef) -> Option<NewShape> {
    let data = building.geometry(element)?.data();
    let node = |at: GeometryRef| data.nodes.get(at.0 as usize);
    let GeometryLinks::Definition(representations) = &node(data.root?)?.links else {
        return None;
    };
    let mut shape = NewShape {
        axis: None,
        solids: Vec::new(),
    };
    for &representation in representations {
        let GeometryLinks::Representation {
            identifier, items, ..
        } = &node(representation)?.links
        else {
            return None;
        };
        match identifier.as_deref() {
            Some("Axis") => {
                let [item] = items.as_slice() else {
                    return None;
                };
                let GeometryDefinition::Curve(graph) = &node(*item)?.definition else {
                    return None;
                };
                let CurveKind::Polyline(points) = &graph.item(graph.root)?.kind else {
                    return None;
                };
                shape.axis = Some(points.iter().map(planar).collect::<Option<_>>()?);
            }
            Some("Body") => {
                for &item in items {
                    shape.solids.push(solid(node(item)?)?);
                }
            }
            _ => return None,
        }
    }
    Some(shape)
}

fn solid(node: &GeometryNode) -> Option<NewSolid> {
    let GeometryDefinition::Extrusion {
        definition,
        boundary,
        ..
    } = &node.definition
    else {
        return None;
    };
    let profile = &definition.profile;
    let position = profile.position.as_ref().map(|p| NewAxis2 {
        location: p.location,
        reference: p.reference_direction,
    });
    let kind = match &profile.kind {
        AreaProfileKind::Rectangle { x_dim, y_dim } => NewProfileKind::Rectangle {
            x: *x_dim,
            y: *y_dim,
            position,
        },
        AreaProfileKind::Circle { radius } => NewProfileKind::Circle {
            radius: *radius,
            position,
        },
        AreaProfileKind::ArbitraryClosed { boundary: outer } => NewProfileKind::Polygon {
            outer: ring(boundary.as_ref()?, *outer)?,
            holes: Vec::new(),
        },
        AreaProfileKind::ArbitraryWithVoids {
            boundary: outer,
            holes,
        } => {
            let graph = boundary.as_ref()?;
            NewProfileKind::Polygon {
                outer: ring(graph, *outer)?,
                holes: holes
                    .iter()
                    .map(|&hole| ring(graph, hole))
                    .collect::<Option<_>>()?,
            }
        }
        _ => return None,
    };
    Some(NewSolid {
        profile: NewProfile {
            name: profile.name.clone(),
            kind,
        },
        position: definition.position.as_ref().map(|p| NewAxis3 {
            location: p.location,
            axis: p.axis,
            reference: p.reference_direction,
        }),
        direction: definition.direction,
        depth: definition.depth,
    })
}

fn planar(point: &Coordinates) -> Option<[f64; 2]> {
    match point {
        Coordinates::Two(point) => Some(*point),
        Coordinates::Three(_) => None,
    }
}

fn ring(graph: &CurveGraph, at: CurveRef) -> Option<Vec<[f64; 2]>> {
    let CurveKind::Polyline(points) = &graph.item(at)?.kind else {
        return None;
    };
    let mut ring = points.iter().map(planar).collect::<Option<Vec<_>>>()?;
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    Some(ring)
}

/// 書いた後に読める形。書き手は立体と、矩形と円の断面の配置を省かずに書く
pub fn as_written(shape: &NewShape) -> NewShape {
    let origin2 = NewAxis2 {
        location: [0.0; 2],
        reference: None,
    };
    let mut shape = shape.clone();
    for solid in &mut shape.solids {
        solid.position.get_or_insert(NewAxis3 {
            location: [0.0; 3],
            axis: None,
            reference: None,
        });
        if let NewProfileKind::Rectangle { position, .. }
        | NewProfileKind::Circle { position, .. } = &mut solid.profile.kind
        {
            position.get_or_insert(origin2.clone());
        }
    }
    shape
}

/// 押し出しの例。壁の矩形・床の穴のある多角形・柱の円
pub fn examples() -> [NewShape; 3] {
    let at = |location: [f64; 3]| {
        Some(NewAxis3 {
            location,
            axis: None,
            reference: None,
        })
    };
    let wall = NewShape {
        axis: Some(vec![[0.0, 0.0], [3000.0, 0.0]]),
        solids: vec![NewSolid {
            profile: NewProfile {
                name: Some("壁".to_owned()),
                kind: NewProfileKind::Rectangle {
                    x: 3000.0,
                    y: 250.0,
                    position: Some(NewAxis2 {
                        location: [1500.0, 0.0],
                        reference: None,
                    }),
                },
            },
            position: None,
            direction: [0.0, 0.0, 1.0],
            depth: 2700.0,
        }],
    };
    let slab = NewShape {
        axis: None,
        solids: vec![NewSolid {
            profile: NewProfile {
                name: None,
                kind: NewProfileKind::Polygon {
                    outer: vec![[0.0, 0.0], [6000.0, 0.0], [6000.0, 4000.0], [0.0, 4000.0]],
                    holes: vec![vec![
                        [1000.0, 1000.0],
                        [1000.0, 2000.0],
                        [2000.0, 2000.0],
                        [2000.0, 1000.0],
                    ]],
                },
            },
            position: at([0.0, 0.0, -200.0]),
            direction: [0.0, 0.0, 1.0],
            depth: 200.0,
        }],
    };
    let column = NewShape {
        axis: None,
        solids: vec![NewSolid {
            profile: NewProfile {
                name: None,
                kind: NewProfileKind::Circle {
                    radius: 300.0,
                    position: None,
                },
            },
            position: Some(NewAxis3 {
                location: [0.0, 0.0, 0.0],
                axis: Some([0.0, 0.0, 1.0]),
                reference: Some([0.0, 1.0, 0.0]),
            }),
            direction: [0.0, 0.0, 1.0],
            depth: 3000.0,
        }],
    };
    [wall, slab, column]
}
