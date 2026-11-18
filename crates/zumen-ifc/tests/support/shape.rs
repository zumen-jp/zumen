//! 形を書く試験の補助。読んだ形を、書くときの値（`NewShape`）に戻して比べる。

#![allow(dead_code)]

use std::collections::BTreeMap;

use zumen_building::{
    AreaProfileKind, AxisPlacement3, BooleanOperation, Building, CartesianMapping3, ConnectedFaces,
    Coordinates, CurveGraph, CurveKind, CurveRef, ElementRef, FaceBoundRef, FaceBoundary,
    FaceGraph, FaceLoopRef, FaceModelKind, FacePointRef, FacePolygon, FaceRef, FaceSetRef,
    GeometryData, GeometryDefinition, GeometryLinks, GeometryNode, GeometryRef, HalfSpaceLimit,
    IndexedFaceSet, IndexedFaces, IndexedPolygon, IndexedPolygonRef, MappingScale3, PolygonFace,
};
use zumen_ifc::{
    NewAxis2, NewAxis3, NewBoolean, NewFaces, NewHalfSpace, NewHalfSpaceLimit, NewItem, NewMapped,
    NewProfile, NewProfileKind, NewShape, NewSolid,
};

/// 要素の `Axis` の折れ線と `Body` の項目。ほかの表現や項目があれば `None`
pub fn shape_of(building: &Building, element: ElementRef) -> Option<NewShape> {
    let data = building.geometry(element)?.data();
    let node = |at: GeometryRef| data.nodes.get(at.0 as usize);
    let GeometryLinks::Definition(representations) = &node(data.root?)?.links else {
        return None;
    };
    let mut shape = NewShape {
        axis: None,
        body: Vec::new(),
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
                    shape.body.push(item_of(data, item)?);
                }
            }
            _ => return None,
        }
    }
    Some(shape)
}

fn axis3(p: &AxisPlacement3) -> NewAxis3 {
    NewAxis3 {
        location: p.location,
        axis: p.axis,
        reference: p.reference_direction,
    }
}

/// 読んだ形の項目。箱の囲みの座標系は書くバージョンで決まるので比べない
fn item_of(data: &GeometryData, at: GeometryRef) -> Option<NewItem> {
    let node = data.nodes.get(at.0 as usize)?;
    match &node.links {
        GeometryLinks::Boolean {
            operation,
            first,
            second,
        } => {
            return Some(NewItem::Boolean(Box::new(NewBoolean {
                operation: *operation,
                first: item_of(data, *first)?,
                second: item_of(data, *second)?,
            })));
        }
        GeometryLinks::Mapped(map) => {
            let GeometryDefinition::Mapping { definition, .. } = &node.definition else {
                return None;
            };
            let GeometryLinks::Map(representation) = &data.nodes.get(map.0 as usize)?.links else {
                return None;
            };
            let GeometryLinks::Representation { items, .. } =
                &data.nodes.get(representation.0 as usize)?.links
            else {
                return None;
            };
            return Some(NewItem::Mapped(Box::new(NewMapped {
                origin: axis3(&definition.origin),
                target: definition.target.clone(),
                items: items
                    .iter()
                    .map(|&item| item_of(data, item))
                    .collect::<Option<_>>()?,
            })));
        }
        _ => {}
    }
    Some(match &node.definition {
        GeometryDefinition::Faces { definition, .. } => {
            NewItem::Faces(NewFaces::Polygons(definition.clone()))
        }
        GeometryDefinition::Tessellation { definition, .. } => {
            NewItem::Faces(NewFaces::Indexed(definition.clone()))
        }
        GeometryDefinition::HalfSpace {
            definition,
            boundary,
            ..
        } => NewItem::HalfSpace(NewHalfSpace {
            plane: axis3(&definition.plane),
            agreement: definition.agreement,
            limit: match &definition.limit {
                HalfSpaceLimit::Unbounded => NewHalfSpaceLimit::Unbounded,
                HalfSpaceLimit::Box {
                    corner, dimensions, ..
                } => NewHalfSpaceLimit::Box {
                    corner: *corner,
                    dimensions: *dimensions,
                },
                HalfSpaceLimit::Polygonal {
                    position,
                    boundary: at,
                } => NewHalfSpaceLimit::Polygonal {
                    position: axis3(position),
                    boundary: ring(boundary.as_ref()?, *at)?,
                },
            },
        }),
        _ => NewItem::Solid(solid(node)?),
    })
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
        position: definition.position.as_ref().map(axis3),
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
    let mut stack: Vec<&mut NewItem> = shape.body.iter_mut().collect();
    while let Some(item) = stack.pop() {
        match item {
            NewItem::Solid(solid) => {
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
            NewItem::Boolean(boolean) => {
                let NewBoolean { first, second, .. } = &mut **boolean;
                stack.push(first);
                stack.push(second);
            }
            NewItem::Mapped(mapped) => stack.extend(mapped.items.iter_mut()),
            NewItem::Faces(_) | NewItem::HalfSpace(_) => {}
        }
    }
    shape
}

/// 項目が押し出しだけの形
fn solids(axis: Option<Vec<[f64; 2]>>, solids: Vec<NewSolid>) -> NewShape {
    NewShape {
        axis,
        body: solids.into_iter().map(NewItem::Solid).collect(),
    }
}

/// 項目が面集合だけの形
fn faces(faces: Vec<NewFaces>) -> NewShape {
    NewShape {
        axis: None,
        body: faces.into_iter().map(NewItem::Faces).collect(),
    }
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
    let wall = solids(
        Some(vec![[0.0, 0.0], [3000.0, 0.0]]),
        vec![NewSolid {
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
    );
    let slab = solids(
        None,
        vec![NewSolid {
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
    );
    let column = solids(
        None,
        vec![NewSolid {
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
    );
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
    let mut solid = NewShape {
        axis: None,
        body: wall.body,
    };
    solid.body.push(NewItem::Faces(NewFaces::Polygons(brep_box([
        500.0, 500.0, 500.0,
    ]))));
    let surface = faces(surfaces(later).map(NewFaces::Polygons).to_vec());
    let tessellation = faces(if later {
        vec![
            NewFaces::Indexed(triangulated_box([900.0, 100.0, 2000.0], tags)),
            NewFaces::Indexed(polygonal_sheet()),
        ]
    } else {
        vec![NewFaces::Polygons(brep_box([900.0, 100.0, 2000.0]))]
    });
    [solid, surface, tessellation]
}

/// 扉の形。IFC4 からは三角形の箱、IFC2X3 は箱の Brep
pub fn door(later: bool) -> NewShape {
    let size = [900.0, 100.0, 2000.0];
    faces(vec![if later {
        NewFaces::Indexed(triangulated_box(size, false))
    } else {
        NewFaces::Polygons(brep_box(size))
    }])
}

/// 原点と、z 軸と x 軸の向き
pub fn placed(location: [f64; 3], axes: Option<([f64; 3], [f64; 3])>) -> NewAxis3 {
    NewAxis3 {
        location,
        axis: axes.map(|(z, _)| z),
        reference: axes.map(|(_, x)| x),
    }
}

/// 平面の半空間。真なら法線と逆の側が内部
pub fn half_space(plane: NewAxis3, agreement: bool, limit: NewHalfSpaceLimit) -> NewItem {
    NewItem::HalfSpace(NewHalfSpace {
        plane,
        agreement,
        limit,
    })
}

/// 集合演算
pub fn boolean(operation: BooleanOperation, first: NewItem, second: NewItem) -> NewItem {
    NewItem::Boolean(Box::new(NewBoolean {
        operation,
        first,
        second,
    }))
}

/// 角を原点に置いた矩形の押し出し
pub fn block(x: f64, y: f64, depth: f64) -> NewItem {
    NewItem::Solid(NewSolid {
        profile: NewProfile {
            name: None,
            kind: NewProfileKind::Rectangle {
                x,
                y,
                position: Some(NewAxis2 {
                    location: [x / 2.0, y / 2.0],
                    reference: None,
                }),
            },
        },
        position: None,
        direction: [0.0, 0.0, 1.0],
        depth,
    })
}

/// 集合演算と写像の例。
///
/// - 切り取り: 3000×250×2700 の壁を z = 2000 の面と、箱で囲んだ x = 2000 の面で切り、2000×250×2000 を残す
/// - 集合演算: 1000 の立方体（IFC4 から三角形の面集合、IFC2X3 は Brep）から、z = 500 より上で 500 の正方形の
///   柱で制限した半空間を除き、0.875 の立方メートルを残す
/// - 写像: 200×200×1000 の押し出しを、そのままと y を 2 倍にして x = 1000 に置く。写像元は 1 つ
pub fn csg_examples(later: bool) -> [NewShape; 3] {
    let wall = block(3000.0, 250.0, 2700.0);
    let top = half_space(
        placed([0.0, 0.0, 2000.0], None),
        false,
        NewHalfSpaceLimit::Unbounded,
    );
    let side = half_space(
        placed([2000.0, 0.0, 0.0], Some(([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]))),
        false,
        NewHalfSpaceLimit::Box {
            corner: [-10000.0, -10000.0, -10000.0],
            dimensions: [20000.0, 20000.0, 20000.0],
        },
    );
    let clipped = boolean(
        BooleanOperation::Difference,
        boolean(BooleanOperation::Difference, wall, top),
        side,
    );
    let cube = if later {
        NewItem::Faces(NewFaces::Indexed(triangulated_box(
            [1000.0, 1000.0, 1000.0],
            false,
        )))
    } else {
        NewItem::Faces(NewFaces::Polygons(brep_box([1000.0, 1000.0, 1000.0])))
    };
    let corner = half_space(
        placed([0.0, 0.0, 500.0], None),
        false,
        NewHalfSpaceLimit::Polygonal {
            position: placed([0.0, 0.0, 0.0], None),
            boundary: vec![[0.0, 0.0], [500.0, 0.0], [500.0, 500.0], [0.0, 500.0]],
        },
    );
    let csg = boolean(BooleanOperation::Difference, cube, corner);
    let mapped = |local_origin: [f64; 3], scale: MappingScale3| {
        NewItem::Mapped(Box::new(NewMapped {
            origin: placed([0.0; 3], None),
            target: CartesianMapping3 {
                local_origin,
                axis1: None,
                axis2: None,
                axis3: None,
                scale,
            },
            items: vec![block(200.0, 200.0, 1000.0)],
        }))
    };
    let shape = |body: Vec<NewItem>| NewShape { axis: None, body };
    [
        shape(vec![clipped]),
        shape(vec![csg]),
        shape(vec![
            mapped([0.0; 3], MappingScale3::Uniform(None)),
            mapped(
                [1000.0, 0.0, 0.0],
                MappingScale3::NonUniform {
                    x: Some(1.0),
                    y: Some(2.0),
                    z: Some(1.0),
                },
            ),
        ]),
    ]
}

/// 形の押し出しを、z = 2000 の面と、箱で囲んだ x = 2000 の面で切る
pub fn clipped(mut shape: NewShape) -> NewShape {
    let top = half_space(
        placed([0.0, 0.0, 2000.0], None),
        false,
        NewHalfSpaceLimit::Unbounded,
    );
    let side = half_space(
        placed([2000.0, 0.0, 0.0], Some(([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]))),
        false,
        NewHalfSpaceLimit::Box {
            corner: [-10000.0, -10000.0, -10000.0],
            dimensions: [20000.0, 20000.0, 20000.0],
        },
    );
    for item in &mut shape.body {
        let solid = std::mem::replace(
            item,
            NewItem::HalfSpace(NewHalfSpace {
                plane: placed([0.0; 3], None),
                agreement: false,
                limit: NewHalfSpaceLimit::Unbounded,
            }),
        );
        *item = boolean(
            BooleanOperation::Difference,
            boolean(BooleanOperation::Difference, solid, top.clone()),
            side.clone(),
        );
    }
    shape
}

/// 形の `Body` を 1 つの写像にし、x に 100 ずらして置く
pub fn mapped(shape: NewShape) -> NewShape {
    NewShape {
        axis: shape.axis,
        body: vec![NewItem::Mapped(Box::new(NewMapped {
            origin: placed([0.0; 3], None),
            target: CartesianMapping3 {
                local_origin: [100.0, 0.0, 0.0],
                axis1: Some([1.0, 0.0, 0.0]),
                axis2: Some([0.0, 1.0, 0.0]),
                axis3: Some([0.0, 0.0, 1.0]),
                scale: MappingScale3::Uniform(Some(1.0)),
            },
            items: shape.body,
        }))],
    }
}

/// 半空間の囲みの箱を外した形。バージョンで箱の座標系が違うと、変換は箱を落とす
pub fn unboxed(mut shape: NewShape) -> NewShape {
    let mut stack: Vec<&mut NewItem> = shape.body.iter_mut().collect();
    while let Some(item) = stack.pop() {
        match item {
            NewItem::HalfSpace(half) => {
                if matches!(half.limit, NewHalfSpaceLimit::Box { .. }) {
                    half.limit = NewHalfSpaceLimit::Unbounded;
                }
            }
            NewItem::Boolean(boolean) => {
                let NewBoolean { first, second, .. } = &mut **boolean;
                stack.push(first);
                stack.push(second);
            }
            NewItem::Mapped(mapped) => stack.extend(mapped.items.iter_mut()),
            NewItem::Solid(_) | NewItem::Faces(_) => {}
        }
    }
    shape
}
