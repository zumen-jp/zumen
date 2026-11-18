//! 形を書く試験の補助。読んだ形を、書くときの値（`NewShape`）に戻して比べる。

#![allow(dead_code)]

use std::collections::BTreeMap;

use zumen_building::{
    AreaProfileKind, Building, ConnectedFaces, Coordinates, CurveGraph, CurveKind, CurveRef,
    ElementRef, FaceBoundRef, FaceBoundary, FaceGraph, FaceLoopRef, FaceModelKind, FacePointRef,
    FacePolygon, FaceRef, FaceSetRef, GeometryDefinition, GeometryLinks, GeometryNode, GeometryRef,
    IndexedFaceSet, IndexedFaces, IndexedPolygon, IndexedPolygonRef, PolygonFace,
};
use zumen_ifc::{NewAxis2, NewAxis3, NewFaces, NewProfile, NewProfileKind, NewShape, NewSolid};

/// 要素の `Axis` の折れ線と `Body` の押し出し・面集合。ほかの表現や項目があれば `None`
pub fn shape_of(building: &Building, element: ElementRef) -> Option<NewShape> {
    let data = building.geometry(element)?.data();
    let node = |at: GeometryRef| data.nodes.get(at.0 as usize);
    let GeometryLinks::Definition(representations) = &node(data.root?)?.links else {
        return None;
    };
    let mut shape = NewShape {
        axis: None,
        solids: Vec::new(),
        faces: Vec::new(),
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
                    let node = node(item)?;
                    match &node.definition {
                        GeometryDefinition::Faces { definition, .. } => {
                            shape.faces.push(NewFaces::Polygons(definition.clone()));
                        }
                        GeometryDefinition::Tessellation { definition, .. } => {
                            shape.faces.push(NewFaces::Indexed(definition.clone()));
                        }
                        _ => shape.solids.push(solid(node)?),
                    }
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
        faces: Vec::new(),
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
        faces: Vec::new(),
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
        faces: Vec::new(),
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

/// 面の輪。点・外輪か・ループと同じ向きか
pub type Ring<'a> = (&'a [[f64; 3]], bool, bool);

/// 多角形の面の模型。面は輪の並び、面集合は面の並びで、点は最初に使った順に共有する。読み直した表と同じ順になる
pub fn face_graph(kind: FaceModelKind, sets: &[(Option<bool>, &[&[Ring<'_>]])]) -> FaceGraph {
    let mut graph = FaceGraph {
        kind,
        roots: Vec::new(),
        sets: Vec::new(),
        faces: Vec::new(),
        bounds: Vec::new(),
        loops: Vec::new(),
        points: Vec::new(),
    };
    let mut known = BTreeMap::new();
    let number = |len: usize| u32::try_from(len).unwrap_or(u32::MAX);
    for &(closed, faces) in sets {
        let mut set = ConnectedFaces {
            faces: Vec::new(),
            closed,
        };
        for rings in faces {
            let mut bounds = Vec::new();
            for &(points, outer, orientation) in *rings {
                let mut refs = Vec::new();
                for point in points {
                    let key = point.map(f64::to_bits);
                    let at = *known.entry(key).or_insert_with(|| {
                        graph.points.push(*point);
                        number(graph.points.len() - 1)
                    });
                    refs.push(FacePointRef(at));
                }
                graph.loops.push(FacePolygon { points: refs });
                graph.bounds.push(FaceBoundary {
                    polygon: FaceLoopRef(number(graph.loops.len() - 1)),
                    orientation,
                    outer,
                });
                bounds.push(FaceBoundRef(number(graph.bounds.len() - 1)));
            }
            graph.faces.push(PolygonFace { bounds });
            set.faces.push(FaceRef(number(graph.faces.len() - 1)));
        }
        graph.sets.push(set);
        graph.roots.push(FaceSetRef(number(graph.sets.len() - 1)));
    }
    graph
}

/// 原点を角とする x・y・z の箱の 8 点。下の 4 点、上の 4 点の順
fn corners([x, y, z]: [f64; 3]) -> [[f64; 3]; 8] {
    [
        [0.0, 0.0, 0.0],
        [x, 0.0, 0.0],
        [x, y, 0.0],
        [0.0, y, 0.0],
        [0.0, 0.0, z],
        [x, 0.0, z],
        [x, y, z],
        [0.0, y, z],
    ]
}

/// 箱の 6 面の、外から見て左回りの点の番号
const BOX_FACES: [[usize; 4]; 6] = [
    [0, 3, 2, 1],
    [4, 5, 6, 7],
    [0, 1, 5, 4],
    [1, 2, 6, 5],
    [2, 3, 7, 6],
    [3, 0, 4, 7],
];

/// 箱の閉殻の `IfcFacetedBrep`
pub fn brep_box(size: [f64; 3]) -> FaceGraph {
    let p = corners(size);
    let quads: Vec<[[f64; 3]; 4]> = BOX_FACES
        .iter()
        .map(|f| f.map(|i| p.get(i).copied().unwrap_or_default()))
        .collect();
    let rings: Vec<[Ring<'_>; 1]> = quads.iter().map(|q| [(q.as_slice(), true, true)]).collect();
    let faces: Vec<&[Ring<'_>]> = rings.iter().map(|r| r.as_slice()).collect();
    face_graph(FaceModelKind::FacetedSolid, &[(Some(true), &faces)])
}

/// 箱の閉じた三角形の面集合。法線は点ごと、点のタグは IFC4X3 だけ
pub fn triangulated_box(size: [f64; 3], tags: bool) -> IndexedFaceSet {
    let points = corners(size).to_vec();
    let mut indices = Vec::new();
    for [a, b, c, d] in BOX_FACES {
        let n = |i: usize| u32::try_from(i + 1).unwrap_or(u32::MAX);
        indices.push([n(a), n(b), n(c)]);
        indices.push([n(a), n(c), n(d)]);
    }
    let normals = points
        .iter()
        .map(|p| p.map(|v| if v == 0.0 { -1.0 } else { 1.0 }))
        .collect();
    IndexedFaceSet {
        tags: tags.then(|| (1..=points.len()).map(|n| format!("P{n}")).collect()),
        points,
        point_index: None,
        closed: Some(true),
        faces: IndexedFaces::Triangles {
            indices,
            normals: Some(normals),
        },
    }
}

/// 穴のある 1 枚の多角形。点の対応を通し、閉殻の申告はない
pub fn polygonal_sheet() -> IndexedFaceSet {
    IndexedFaceSet {
        points: vec![
            [1000.0, 1000.0, 0.0],
            [2000.0, 1000.0, 0.0],
            [2000.0, 2000.0, 0.0],
            [1000.0, 2000.0, 0.0],
            [0.0, 0.0, 0.0],
            [3000.0, 0.0, 0.0],
            [3000.0, 3000.0, 0.0],
            [0.0, 3000.0, 0.0],
        ],
        tags: None,
        point_index: Some(vec![5, 6, 7, 8, 1, 2, 3, 4]),
        closed: None,
        faces: IndexedFaces::Polygons {
            indices: vec![IndexedPolygonRef(0)],
            faces: vec![IndexedPolygon {
                boundary: vec![1, 2, 3, 4],
                holes: vec![vec![8, 7, 6, 5]],
            }],
        },
    }
}

/// 開殻の殻の面の模型（2 枚の並んだ面）と、穴のある面。穴のある面は、IFC2X3 では面の模型、IFC4 からは開殻の
/// 殻の面の模型
pub fn surfaces(later: bool) -> [FaceGraph; 2] {
    let left = [
        [0.0, 0.0, 0.0],
        [1000.0, 0.0, 0.0],
        [1000.0, 1000.0, 0.0],
        [0.0, 1000.0, 0.0],
    ];
    let right = [
        [1000.0, 0.0, 0.0],
        [2000.0, 0.0, 0.0],
        [2000.0, 1000.0, 0.0],
        [1000.0, 1000.0, 0.0],
    ];
    let shell = face_graph(
        FaceModelKind::ShellSurface,
        &[(
            Some(false),
            &[&[(&left, true, true)], &[(&right, true, true)]],
        )],
    );
    let outer = [
        [0.0, 0.0, 500.0],
        [3000.0, 0.0, 500.0],
        [3000.0, 3000.0, 500.0],
        [0.0, 3000.0, 500.0],
    ];
    // 穴は外輪と同じ回りで、境界の向きを逆にする
    let hole = [
        [1000.0, 1000.0, 500.0],
        [2000.0, 1000.0, 500.0],
        [2000.0, 2000.0, 500.0],
        [1000.0, 2000.0, 500.0],
    ];
    let (kind, closed) = if later {
        (FaceModelKind::ShellSurface, Some(false))
    } else {
        (FaceModelKind::FaceSurface, None)
    };
    let sheet = face_graph(
        kind,
        &[(closed, &[&[(&outer, true, true), (&hole, false, false)]])],
    );
    [shell, sheet]
}

/// 面集合の例。押し出しと箱の Brep・2 つの面の模型・2 つの添字の面集合（IFC2X3 では箱の Brep）
pub fn face_examples(later: bool, tags: bool) -> [NewShape; 3] {
    let [wall, ..] = examples();
    let solid = NewShape {
        axis: None,
        solids: wall.solids,
        faces: vec![NewFaces::Polygons(brep_box([500.0, 500.0, 500.0]))],
    };
    let surface = NewShape {
        axis: None,
        solids: Vec::new(),
        faces: surfaces(later).map(NewFaces::Polygons).to_vec(),
    };
    let tessellation = NewShape {
        axis: None,
        solids: Vec::new(),
        faces: if later {
            vec![
                NewFaces::Indexed(triangulated_box([900.0, 100.0, 2000.0], tags)),
                NewFaces::Indexed(polygonal_sheet()),
            ]
        } else {
            vec![NewFaces::Polygons(brep_box([900.0, 100.0, 2000.0]))]
        },
    };
    [solid, surface, tessellation]
}

/// 扉の形。IFC4 からは三角形の箱、IFC2X3 は箱の Brep
pub fn door(later: bool) -> NewShape {
    let size = [900.0, 100.0, 2000.0];
    NewShape {
        axis: None,
        solids: Vec::new(),
        faces: vec![if later {
            NewFaces::Indexed(triangulated_box(size, false))
        } else {
            NewFaces::Polygons(brep_box(size))
        }],
    }
}
