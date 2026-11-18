//! 要素の形（芯の線・押し出し・面集合）を書く変更（#395）。
//!
//! 属性の並びは、IFC2X3 TC1 と IFC4 ADD2 TC1・IFC4X3 ADD2 の、形の資源・断面の資源・表現の資源の実体の定義による。
//!
//! - `IfcProduct`: 6 ObjectPlacement・7 Representation。表現を持つなら配置が要る
//! - `IfcProductDefinitionShape`: 1 Name・2 Description・3 Representations
//! - `IfcShapeRepresentation`: 1 ContextOfItems・2 RepresentationIdentifier・3 RepresentationType・4 Items
//! - `IfcExtrudedAreaSolid`: 1 SweptArea・2 Position・3 ExtrudedDirection・4 Depth。Position は IFC4X3 から
//!   省略できるが、どのバージョンでも書く。押し出す向きは断面の面に平行にできない
//! - `IfcRectangleProfileDef`: 1 ProfileType・2 ProfileName・3 Position・4 XDim・5 YDim
//! - `IfcCircleProfileDef`: 1 ProfileType・2 ProfileName・3 Position・4 Radius。Position は IFC4X3 から省略できる
//! - `IfcArbitraryClosedProfileDef`: 1 ProfileType・2 ProfileName・3 OuterCurve
//! - `IfcArbitraryProfileDefWithVoids`: 4 InnerCurves
//! - `IfcAxis2Placement3D`: 1 Location・2 Axis・3 RefDirection。`IfcAxis2Placement2D`: 1 Location・2 RefDirection
//! - `IfcFacetedBrep`: 1 Outer（`IfcClosedShell`）。`IfcShellBasedSurfaceModel`: 1 SbsmBoundary（殻の集合）。
//!   `IfcFaceBasedSurfaceModel`: 1 FbsmFaces（`IfcConnectedFaceSet` の集合）。面の模型は IFC4 から非推奨
//!   （ifc-gherkin-rules の IFC102）なので、IFC2X3 だけに書く
//! - `IfcConnectedFaceSet`・`IfcOpenShell`・`IfcClosedShell`: 1 CfsFaces。`IfcFace`: 1 Bounds。外輪
//!   （`IfcFaceOuterBound`）は面に 1 つまで
//! - `IfcFaceBound`・`IfcFaceOuterBound`: 1 Bound・2 Orientation。`IfcPolyLoop`: 1 Polygon（3 点以上で、点は重ならない）
//! - IFC4 から: `IfcTriangulatedFaceSet`: 1 Coordinates・2 Normals・3 Closed・4 CoordIndex・5 PnIndex。
//!   `IfcPolygonalFaceSet`: 1 Coordinates・2 Closed・3 Faces・4 PnIndex。`IfcIndexedPolygonalFace`: 1 CoordIndex。
//!   `WithVoids`: 2 InnerCoordIndices。`IfcCartesianPointList3D`: 1 CoordList・2 TagList（IFC4X3 から）
//! - 表現の種類（RepresentationType）は項目から決める。押し出しは `SweptSolid`、`IfcFacetedBrep` は `Brep`、
//!   両方なら `SolidModel`、面の模型は `SurfaceModel`、添字の面集合は `Tessellation`。ほかの組み合わせは書かない

use std::collections::BTreeSet;

use zumen_base::Kind;
use zumen_building::{FaceGraph, FaceModelKind, IndexedFaceSet, IndexedFaces};
use zumen_sdk::{Cx, Error};

use super::{Category, Draft, Item, Stmt, Target, null, raw, real, text, unsupported};
use crate::building::Version;
use crate::edit::refs_of;
use crate::stopped;

/// 要素の形。芯の線は `Axis`、押し出しは `Body` の表現に書き、元の表現は置き換える
#[derive(Clone, Debug, PartialEq)]
pub struct NewShape {
    /// 芯の線（`Axis` の表現の `IfcPolyline`）。2 点以上
    pub axis: Option<Vec<[f64; 2]>>,
    /// 押し出し（`Body` の表現の `IfcExtrudedAreaSolid`）
    pub solids: Vec<NewSolid>,
    /// 面集合（`Body` の表現）。押し出しとともに空なら `Body` を書かない
    pub faces: Vec<NewFaces>,
}

/// 面集合。点は文書の長さの単位で、参照と共有は値のまま書く
#[derive(Clone, Debug, PartialEq)]
pub enum NewFaces {
    /// 多角形の面（`IfcFacetedBrep`・`IfcShellBasedSurfaceModel`・IFC2X3 の `IfcFaceBasedSurfaceModel`）。
    /// 根からたどれる表の行だけを、たどった順で書く
    Polygons(FaceGraph),
    /// 添字の面（`IfcTriangulatedFaceSet`・`IfcPolygonalFaceSet`）。IFC4 から。点のタグは IFC4X3 から
    Indexed(IndexedFaceSet),
}

/// 断面の押し出し
#[derive(Clone, Debug, PartialEq)]
pub struct NewSolid {
    /// 断面
    pub profile: NewProfile,
    /// 立体の局所の配置。なければ要素の配置のまま
    pub position: Option<NewAxis3>,
    /// 局所の押し出す向き。z は 0 にできない
    pub direction: [f64; 3],
    /// 深さ（文書の長さの単位）。正
    pub depth: f64,
}

/// 3 次元の局所の配置。向きは元の比のまま書く
#[derive(Clone, Debug, PartialEq)]
pub struct NewAxis3 {
    /// 原点
    pub location: [f64; 3],
    /// 局所の z 軸
    pub axis: Option<[f64; 3]>,
    /// 局所の x 軸の参照方向
    pub reference: Option<[f64; 3]>,
}

/// 2 次元の局所の配置
#[derive(Clone, Debug, PartialEq)]
pub struct NewAxis2 {
    /// 原点
    pub location: [f64; 2],
    /// 局所の x 軸の参照方向
    pub reference: Option<[f64; 2]>,
}

/// 面の断面
#[derive(Clone, Debug, PartialEq)]
pub struct NewProfile {
    /// 断面の名前
    pub name: Option<String>,
    /// 形
    pub kind: NewProfileKind,
}

/// 断面の形。長さは文書の長さの単位
#[derive(Clone, Debug, PartialEq)]
pub enum NewProfileKind {
    /// 中心を原点とする矩形（`IfcRectangleProfileDef`）
    Rectangle {
        /// 局所の x の全幅。正
        x: f64,
        /// 局所の y の全高。正
        y: f64,
        /// 断面の局所の配置
        position: Option<NewAxis2>,
    },
    /// 中心を原点とする円（`IfcCircleProfileDef`）
    Circle {
        /// 半径。正
        radius: f64,
        /// 断面の局所の配置
        position: Option<NewAxis2>,
    },
    /// 折れ線の外輪と内輪（`IfcArbitraryClosedProfileDef`・`IfcArbitraryProfileDefWithVoids`）。
    /// 輪は 3 点以上で、始点に戻る点は書かない（始点と同じインスタンスで閉じる）
    Polygon {
        /// 外輪
        outer: Vec<[f64; 2]>,
        /// 内輪
        holes: Vec<Vec<[f64; 2]>>,
    },
}

impl Draft<'_> {
    /// 要素の形を置き換える。どこからも使われなくなった元の表現は除く
    pub(in crate::edit) fn set_shape(
        &mut self,
        target: Target,
        shape: &NewShape,
        cx: &mut Cx,
    ) -> Result<bool, Error> {
        const EDIT: &str = "shape";
        let Some((id, mut stmt, _)) = self.object(target, &[Category::Element], EDIT, cx) else {
            return Ok(false);
        };
        let Some(version) = self.version() else {
            cx.report(unsupported(EDIT, &stmt.name));
            return Ok(false);
        };
        let reason = rejected(shape, version)
            .or_else(|| matches!(stmt.arg(6), None | Some(Item::Raw(_))).then_some("placement"));
        if let Some(value) = reason {
            cx.report(unsupported(EDIT, &stmt.name).with("value", value));
            return Ok(false);
        }
        let Some(model) = self.model_context() else {
            cx.report(unsupported(EDIT, &stmt.name).with("value", "context"));
            return Ok(false);
        };
        let mut representations = Vec::new();
        if let Some(points) = &shape.axis {
            let context = self.subcontext(model, "Axis", b".GRAPH_VIEW.");
            let line = self.polyline(points, false);
            representations.push(self.add(Stmt {
                name: b"IFCSHAPEREPRESENTATION".to_vec(),
                args: vec![
                    Item::Ref(context),
                    text("Axis"),
                    text("Curve2D"),
                    Item::List(vec![Item::Ref(line)]),
                ],
            }));
        }
        if let Some(kind) = body_type(shape) {
            let work = shape.faces.iter().map(faces_size).sum::<u64>();
            cx.check(Kind::Work, work)
                .map_err(|stop| stopped(cx, stop, None))?;
            let context = self.subcontext(model, "Body", b".MODEL_VIEW.");
            let mut items = Vec::with_capacity(shape.solids.len() + shape.faces.len());
            for solid in &shape.solids {
                items.push(Item::Ref(self.solid(solid)));
            }
            for faces in &shape.faces {
                items.push(Item::Ref(match faces {
                    NewFaces::Polygons(graph) => self.face_graph(graph),
                    NewFaces::Indexed(set) => self.indexed_faces(set),
                }));
            }
            representations.push(self.add(Stmt {
                name: b"IFCSHAPEREPRESENTATION".to_vec(),
                args: vec![
                    Item::Ref(context),
                    text("Body"),
                    text(kind),
                    Item::List(items),
                ],
            }));
        }
        let product = self.product_shape(&representations);
        let old = refs_of(stmt.arg(7));
        stmt.set(7, Item::Ref(product));
        self.put(id, stmt);
        self.collect(old, cx)?;
        Ok(true)
    }

    /// 押し出し。値は [`rejected`] で確かめてある
    fn solid(&mut self, solid: &NewSolid) -> u64 {
        let profile = self.profile(&solid.profile);
        let position = solid.position.clone().unwrap_or(NewAxis3 {
            location: [0.0; 3],
            axis: None,
            reference: None,
        });
        let position = self.axis3(&position);
        let direction = self.point(b"IFCDIRECTION", &solid.direction);
        self.add(Stmt {
            name: b"IFCEXTRUDEDAREASOLID".to_vec(),
            args: vec![
                Item::Ref(profile),
                Item::Ref(position),
                Item::Ref(direction),
                number(solid.depth),
            ],
        })
    }

    fn profile(&mut self, profile: &NewProfile) -> u64 {
        let name = profile.name.as_deref().map_or_else(null, text);
        let origin = NewAxis2 {
            location: [0.0; 2],
            reference: None,
        };
        let (kind, args): (&[u8], Vec<Item>) = match &profile.kind {
            NewProfileKind::Rectangle { x, y, position } => {
                let position = self.axis2(position.as_ref().unwrap_or(&origin));
                (
                    b"IFCRECTANGLEPROFILEDEF",
                    vec![Item::Ref(position), number(*x), number(*y)],
                )
            }
            NewProfileKind::Circle { radius, position } => {
                let position = self.axis2(position.as_ref().unwrap_or(&origin));
                (
                    b"IFCCIRCLEPROFILEDEF",
                    vec![Item::Ref(position), number(*radius)],
                )
            }
            NewProfileKind::Polygon { outer, holes } if holes.is_empty() => {
                let outer = self.polyline(outer, true);
                (b"IFCARBITRARYCLOSEDPROFILEDEF", vec![Item::Ref(outer)])
            }
            NewProfileKind::Polygon { outer, holes } => {
                let outer = self.polyline(outer, true);
                let holes = holes
                    .iter()
                    .map(|hole| Item::Ref(self.polyline(hole, true)))
                    .collect();
                (
                    b"IFCARBITRARYPROFILEDEFWITHVOIDS",
                    vec![Item::Ref(outer), Item::List(holes)],
                )
            }
        };
        let mut all = vec![raw(b".AREA."), name];
        all.extend(args);
        self.add(Stmt {
            name: kind.to_vec(),
            args: all,
        })
    }

    /// 2 次元の点の `IfcPolyline`。`closed` なら始点と同じインスタンスで閉じる
    fn polyline(&mut self, points: &[[f64; 2]], closed: bool) -> u64 {
        let mut items: Vec<Item> = points
            .iter()
            .map(|p| Item::Ref(self.point(b"IFCCARTESIANPOINT", p)))
            .collect();
        if closed && let Some(first) = items.first().cloned() {
            items.push(first);
        }
        self.add(Stmt {
            name: b"IFCPOLYLINE".to_vec(),
            args: vec![Item::List(items)],
        })
    }

    fn axis3(&mut self, axis: &NewAxis3) -> u64 {
        let location = self.point(b"IFCCARTESIANPOINT", &axis.location);
        let z = axis
            .axis
            .map_or_else(null, |v| Item::Ref(self.point(b"IFCDIRECTION", &v)));
        let x = axis
            .reference
            .map_or_else(null, |v| Item::Ref(self.point(b"IFCDIRECTION", &v)));
        self.add(Stmt {
            name: b"IFCAXIS2PLACEMENT3D".to_vec(),
            args: vec![Item::Ref(location), z, x],
        })
    }

    fn axis2(&mut self, axis: &NewAxis2) -> u64 {
        let location = self.point(b"IFCCARTESIANPOINT", &axis.location);
        let x = axis
            .reference
            .map_or_else(null, |v| Item::Ref(self.point(b"IFCDIRECTION", &v)));
        self.add(Stmt {
            name: b"IFCAXIS2PLACEMENT2D".to_vec(),
            args: vec![Item::Ref(location), x],
        })
    }

    /// 多角形の面の模型。根からたどり、共有する行は 1 つのインスタンスにする。値は [`rejected`] で確かめてある
    fn face_graph(&mut self, graph: &FaceGraph) -> u64 {
        let mut written = FaceIds {
            sets: vec![None; graph.sets.len()],
            faces: vec![None; graph.faces.len()],
            bounds: vec![None; graph.bounds.len()],
            loops: vec![None; graph.loops.len()],
            points: vec![None; graph.points.len()],
        };
        let roots: Vec<Item> = graph
            .roots
            .iter()
            .map(|root| Item::Ref(self.face_set(graph, root.0 as usize, &mut written)))
            .collect();
        let (name, outer) = match graph.kind {
            FaceModelKind::FacetedSolid => (
                b"IFCFACETEDBREP".as_slice(),
                roots.into_iter().next().unwrap_or_else(null),
            ),
            FaceModelKind::ShellSurface => {
                (b"IFCSHELLBASEDSURFACEMODEL".as_slice(), Item::List(roots))
            }
            FaceModelKind::FaceSurface => {
                (b"IFCFACEBASEDSURFACEMODEL".as_slice(), Item::List(roots))
            }
        };
        self.add(Stmt {
            name: name.to_vec(),
            args: vec![outer],
        })
    }

    fn face_set(&mut self, graph: &FaceGraph, at: usize, written: &mut FaceIds) -> u64 {
        if let Some(Some(id)) = written.sets.get(at) {
            return *id;
        }
        let Some(set) = graph.sets.get(at) else {
            return 0;
        };
        let faces = set
            .faces
            .iter()
            .map(|face| Item::Ref(self.face(graph, face.0 as usize, written)))
            .collect();
        let name: &[u8] = match set.closed {
            Some(true) => b"IFCCLOSEDSHELL",
            Some(false) => b"IFCOPENSHELL",
            None => b"IFCCONNECTEDFACESET",
        };
        let id = self.add(Stmt {
            name: name.to_vec(),
            args: vec![Item::List(faces)],
        });
        if let Some(slot) = written.sets.get_mut(at) {
            *slot = Some(id);
        }
        id
    }

    fn face(&mut self, graph: &FaceGraph, at: usize, written: &mut FaceIds) -> u64 {
        if let Some(Some(id)) = written.faces.get(at) {
            return *id;
        }
        let Some(face) = graph.faces.get(at) else {
            return 0;
        };
        let mut bounds = Vec::with_capacity(face.bounds.len());
        for bound in &face.bounds {
            let at = bound.0 as usize;
            if let Some(Some(id)) = written.bounds.get(at) {
                bounds.push(Item::Ref(*id));
                continue;
            }
            let Some(bound) = graph.bounds.get(at) else {
                continue;
            };
            let polygon = self.face_loop(graph, bound.polygon.0 as usize, written);
            let id = self.add(Stmt {
                name: if bound.outer {
                    b"IFCFACEOUTERBOUND".to_vec()
                } else {
                    b"IFCFACEBOUND".to_vec()
                },
                args: vec![Item::Ref(polygon), boolean(bound.orientation)],
            });
            if let Some(slot) = written.bounds.get_mut(at) {
                *slot = Some(id);
            }
            bounds.push(Item::Ref(id));
        }
        let id = self.add(Stmt {
            name: b"IFCFACE".to_vec(),
            args: vec![Item::List(bounds)],
        });
        if let Some(slot) = written.faces.get_mut(at) {
            *slot = Some(id);
        }
        id
    }

    fn face_loop(&mut self, graph: &FaceGraph, at: usize, written: &mut FaceIds) -> u64 {
        if let Some(Some(id)) = written.loops.get(at) {
            return *id;
        }
        let Some(polygon) = graph.loops.get(at) else {
            return 0;
        };
        let mut points = Vec::with_capacity(polygon.points.len());
        for point in &polygon.points {
            let at = point.0 as usize;
            let id = match (written.points.get(at), graph.points.get(at)) {
                (Some(Some(id)), _) => *id,
                (_, Some(values)) => {
                    let id = self.point(b"IFCCARTESIANPOINT", values);
                    if let Some(slot) = written.points.get_mut(at) {
                        *slot = Some(id);
                    }
                    id
                }
                _ => continue,
            };
            points.push(Item::Ref(id));
        }
        let id = self.add(Stmt {
            name: b"IFCPOLYLOOP".to_vec(),
            args: vec![Item::List(points)],
        });
        if let Some(slot) = written.loops.get_mut(at) {
            *slot = Some(id);
        }
        id
    }

    /// 添字の面集合。値は [`rejected`] で確かめてある
    fn indexed_faces(&mut self, set: &IndexedFaceSet) -> u64 {
        let mut coordinates = vec![Item::List(
            set.points
                .iter()
                .map(|p| Item::List(p.iter().map(|&v| number(v)).collect()))
                .collect(),
        )];
        if self.version() == Some(Version::Ifc4x3) {
            coordinates.push(set.tags.as_ref().map_or_else(null, |tags| {
                Item::List(tags.iter().map(|tag| text(tag)).collect())
            }));
        }
        let coordinates = self.add(Stmt {
            name: b"IFCCARTESIANPOINTLIST3D".to_vec(),
            args: coordinates,
        });
        let closed = set.closed.map_or_else(null, boolean);
        let point_index = set.point_index.as_deref().map_or_else(null, integers);
        match &set.faces {
            IndexedFaces::Triangles { indices, normals } => {
                let normals = normals.as_ref().map_or_else(null, |normals| {
                    Item::List(
                        normals
                            .iter()
                            .map(|n| Item::List(n.iter().map(|&v| number(v)).collect()))
                            .collect(),
                    )
                });
                let indices = Item::List(indices.iter().map(|row| integers(row)).collect());
                self.add(Stmt {
                    name: b"IFCTRIANGULATEDFACESET".to_vec(),
                    args: vec![
                        Item::Ref(coordinates),
                        normals,
                        closed,
                        indices,
                        point_index,
                    ],
                })
            }
            IndexedFaces::Polygons { indices, faces } => {
                let mut written = vec![None; faces.len()];
                let mut refs = Vec::with_capacity(indices.len());
                for index in indices {
                    let at = index.0 as usize;
                    let id = match (written.get(at), faces.get(at)) {
                        (Some(Some(id)), _) => *id,
                        (_, Some(face)) => {
                            let mut args = vec![integers(&face.boundary)];
                            let name: &[u8] = if face.holes.is_empty() {
                                b"IFCINDEXEDPOLYGONALFACE"
                            } else {
                                args.push(Item::List(
                                    face.holes.iter().map(|hole| integers(hole)).collect(),
                                ));
                                b"IFCINDEXEDPOLYGONALFACEWITHVOIDS"
                            };
                            let id = self.add(Stmt {
                                name: name.to_vec(),
                                args,
                            });
                            if let Some(slot) = written.get_mut(at) {
                                *slot = Some(id);
                            }
                            id
                        }
                        _ => continue,
                    };
                    refs.push(Item::Ref(id));
                }
                self.add(Stmt {
                    name: b"IFCPOLYGONALFACESET".to_vec(),
                    args: vec![
                        Item::Ref(coordinates),
                        closed,
                        Item::List(refs),
                        point_index,
                    ],
                })
            }
        }
    }

    /// 座標の並びの点か向き。値は有限と確かめてある
    fn point(&mut self, name: &[u8], values: &[f64]) -> u64 {
        self.add(Stmt {
            name: name.to_vec(),
            args: vec![Item::List(values.iter().map(|&v| number(v)).collect())],
        })
    }
}

/// 書いた面の模型の行のインスタンス
struct FaceIds {
    sets: Vec<Option<u64>>,
    faces: Vec<Option<u64>>,
    bounds: Vec<Option<u64>>,
    loops: Vec<Option<u64>>,
    points: Vec<Option<u64>>,
}

/// 有限と確かめた値
fn number(value: f64) -> Item {
    real(value).unwrap_or_else(null)
}

fn boolean(value: bool) -> Item {
    raw(if value { b".T." } else { b".F." })
}

fn integers(values: &[u32]) -> Item {
    Item::List(
        values
            .iter()
            .map(|v| Item::Raw(v.to_string().into_bytes()))
            .collect(),
    )
}

/// 面集合を書く作業量。行の数
fn faces_size(faces: &NewFaces) -> u64 {
    let rows = match faces {
        NewFaces::Polygons(g) => {
            g.sets.len() + g.faces.len() + g.bounds.len() + g.loops.len() + g.points.len()
        }
        NewFaces::Indexed(set) => {
            set.points.len()
                + match &set.faces {
                    IndexedFaces::Triangles { indices, .. } => indices.len(),
                    IndexedFaces::Polygons { indices, .. } => indices.len(),
                }
        }
    };
    rows as u64
}

/// `Body` の表現の種類。項目がなければ `None`
fn body_type(shape: &NewShape) -> Option<&'static str> {
    let solids = !shape.solids.is_empty();
    let (mut brep, mut surface, mut tessellation) = (false, false, false);
    for faces in &shape.faces {
        match faces {
            NewFaces::Polygons(graph) if graph.kind == FaceModelKind::FacetedSolid => brep = true,
            NewFaces::Polygons(_) => surface = true,
            NewFaces::Indexed(_) => tessellation = true,
        }
    }
    match (solids, brep, surface, tessellation) {
        (false, false, false, false) => None,
        (true, false, false, false) => Some("SweptSolid"),
        (false, true, false, false) => Some("Brep"),
        (true, true, false, false) => Some("SolidModel"),
        (false, false, true, false) => Some("SurfaceModel"),
        (false, false, false, true) => Some("Tessellation"),
        _ => Some(""),
    }
}

/// 参照が表の中にあり、重ならない
fn unique_in(refs: impl Iterator<Item = u32>, len: usize) -> bool {
    let mut seen = BTreeSet::new();
    refs.into_iter()
        .all(|at| (at as usize) < len && seen.insert(at))
}

/// 書ける多角形の面の模型か
fn graph_ok(graph: &FaceGraph) -> bool {
    let roots = !graph.roots.is_empty()
        && unique_in(graph.roots.iter().map(|r| r.0), graph.sets.len())
        && graph.roots.iter().all(|root| {
            let closed = graph.sets.get(root.0 as usize).and_then(|set| set.closed);
            match graph.kind {
                FaceModelKind::FacetedSolid => closed == Some(true),
                FaceModelKind::ShellSurface => closed.is_some(),
                FaceModelKind::FaceSurface => true,
            }
        })
        && (graph.kind != FaceModelKind::FacetedSolid || graph.roots.len() == 1);
    roots
        && graph.sets.iter().all(|set| {
            !set.faces.is_empty() && unique_in(set.faces.iter().map(|f| f.0), graph.faces.len())
        })
        && graph.faces.iter().all(|face| {
            !face.bounds.is_empty()
                && unique_in(face.bounds.iter().map(|b| b.0), graph.bounds.len())
                && face
                    .bounds
                    .iter()
                    .filter(|b| graph.bounds.get(b.0 as usize).is_some_and(|b| b.outer))
                    .count()
                    <= 1
        })
        && graph
            .bounds
            .iter()
            .all(|bound| (bound.polygon.0 as usize) < graph.loops.len())
        && graph.points.iter().all(|p| finite(p))
        && graph.loops.iter().all(|polygon| {
            // 点のインスタンスも座標も重ならない
            let mut seen = BTreeSet::new();
            polygon.points.len() >= 3
                && polygon.points.iter().all(|p| {
                    graph.points.get(p.0 as usize).is_some_and(|v| {
                        seen.insert(v.map(|c| if c == 0.0 { 0 } else { c.to_bits() }))
                    })
                })
        })
}

/// 書ける添字の面集合か。書けなければ理由
fn indexed_rejected(set: &IndexedFaceSet, version: Version) -> Option<&'static str> {
    if version == Version::Ifc2x3 {
        return Some("tessellation");
    }
    if set.tags.is_some() && version != Version::Ifc4x3 {
        return Some("tags");
    }
    let in_range = |values: &[u32], len: usize| values.iter().all(|&v| v >= 1 && v as usize <= len);
    let count = set.point_index.as_ref().map_or(set.points.len(), Vec::len);
    let ok = !set.points.is_empty()
        && set.points.iter().all(|p| finite(p))
        && set
            .tags
            .as_ref()
            .is_none_or(|tags| tags.len() == set.points.len())
        && set
            .point_index
            .as_ref()
            .is_none_or(|index| !index.is_empty() && in_range(index, set.points.len()))
        && match &set.faces {
            IndexedFaces::Triangles { indices, normals } => {
                let limit = normals.as_ref().map_or(count, |n| count.min(n.len()));
                !indices.is_empty()
                    && indices.iter().all(|row| in_range(row, limit))
                    && normals
                        .as_ref()
                        .is_none_or(|n| !n.is_empty() && n.iter().all(|v| finite(v)))
            }
            IndexedFaces::Polygons { indices, faces } => {
                !indices.is_empty()
                    && indices.iter().all(|at| (at.0 as usize) < faces.len())
                    && faces.iter().all(|face| {
                        face.boundary.len() >= 3
                            && in_range(&face.boundary, count)
                            && face
                                .holes
                                .iter()
                                .all(|hole| hole.len() >= 3 && in_range(hole, count))
                    })
            }
        };
    (!ok).then_some("faces")
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|v| v.is_finite())
}

/// 有限で 0 でない向き
fn direction(values: &[f64]) -> bool {
    finite(values) && values.iter().any(|&v| v != 0.0)
}

fn ring(points: &[[f64; 2]]) -> bool {
    points.len() >= 3 && points.iter().all(|p| finite(p))
}

fn axis2(position: Option<&NewAxis2>) -> bool {
    position.is_none_or(|p| finite(&p.location) && p.reference.is_none_or(|r| direction(&r)))
}

/// 書けない形なら、診断に書く理由
pub(in crate::edit) fn rejected(shape: &NewShape, version: Version) -> Option<&'static str> {
    if shape.axis.is_none() && shape.solids.is_empty() && shape.faces.is_empty() {
        return Some("empty");
    }
    if body_type(shape) == Some("") {
        return Some("mixed");
    }
    for faces in &shape.faces {
        let reason = match faces {
            NewFaces::Polygons(graph)
                if graph.kind == FaceModelKind::FaceSurface && version != Version::Ifc2x3 =>
            {
                Some("deprecated")
            }
            NewFaces::Polygons(graph) => (!graph_ok(graph)).then_some("faces"),
            NewFaces::Indexed(set) => indexed_rejected(set, version),
        };
        if reason.is_some() {
            return reason;
        }
    }
    if let Some(points) = &shape.axis
        && (points.len() < 2 || !points.iter().all(|p| finite(p)))
    {
        return Some("axis");
    }
    for solid in &shape.solids {
        if !(solid.depth.is_finite() && solid.depth > 0.0) {
            return Some("depth");
        }
        if !direction(&solid.direction) || solid.direction[2] == 0.0 {
            return Some("direction");
        }
        if let Some(p) = &solid.position
            && !(finite(&p.location)
                && p.axis.is_none_or(|v| direction(&v))
                && p.reference.is_none_or(|v| direction(&v)))
        {
            return Some("position");
        }
        let positive = |v: f64| v.is_finite() && v > 0.0;
        let profile = match &solid.profile.kind {
            NewProfileKind::Rectangle { x, y, position } => {
                positive(*x) && positive(*y) && axis2(position.as_ref())
            }
            NewProfileKind::Circle { radius, position } => {
                positive(*radius) && axis2(position.as_ref())
            }
            NewProfileKind::Polygon { outer, holes } => {
                ring(outer) && holes.iter().all(|hole| ring(hole))
            }
        };
        if !profile {
            return Some("profile");
        }
    }
    None
}
