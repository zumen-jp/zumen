//! `building_apply` の `shape` の項目の表を、L1 の形（[`NewShape`]）の木にする（#395）。
//!
//! 集合演算と写像は、表の前にある項目を位置で指す。前だけを指すので輪にならず、同じ項目を何度も指す表は
//! 指すたびに写す。写すと項目が指数で増えうるので、組み立てる前に、項目ごとの深さと写した数を表の順に
//! 数え、深さが [`DEPTH`] を超える項目と、写して [`ITEMS`] を超える `Body` を断る。組み立ての再帰は
//! `Body` からたどれる項目だけで、深さは [`DEPTH`] までになる。

use zumen_base::{Cx, Kind};
use zumen_building::{
    BooleanOperation, CartesianMapping3, ConnectedFaces, FaceBoundRef, FaceBoundary, FaceGraph,
    FaceLoopRef, FaceModelKind, FacePointRef, FacePolygon, FaceRef, FaceSetRef, IndexedFaceSet,
    IndexedFaces, IndexedPolygon, IndexedPolygonRef, MappingScale3, PolygonFace,
};
use zumen_ifc::{
    NewAxis2, NewAxis3, NewBoolean, NewFaces, NewHalfSpace, NewHalfSpaceLimit, NewItem, NewMapped,
    NewProfile, NewProfileKind, NewShape, NewSolid,
};

use super::{pair, triple};
use crate::messages::{
    BuildingAxis3, BuildingBooleanOperation, BuildingShapeEdit, BuildingShapeFace,
    BuildingShapeItem, BuildingShapeItemKind, BuildingShapeMapping, BuildingShapeProfile,
    BuildingShapeProfileKind,
};
use crate::session::{Failed, fail, invalid_value, stopped};

/// 組み立てる項目の深さの上限。L1 は 16 段を超える入れ子を診断して断るので、それより深い表だけをここで断る
const DEPTH: u32 = 64;

/// `Body` に写して置ける項目の数の上限。L1 の 1 つの `Body` の上限と同じ
const ITEMS: u64 = 10_000;

/// 変更の項目の表を形にする。値の形が違えば `at` からの位置を診断して失敗する。`ifc2x3` は文書が IFC2X3 か
pub(super) fn new_shape(
    edit: &BuildingShapeEdit,
    ifc2x3: bool,
    at: &str,
    cx: &mut Cx<'_>,
) -> Result<NewShape, Failed> {
    let axis = match &edit.axis {
        Some(points) => Some(
            points
                .iter()
                .map(|p| pair(p))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| fail(cx, invalid_value(format!("{at}/axis"))))?,
        ),
        None => None,
    };
    // 項目ごとの深さと、写した数。指す項目は前にあるので、表の順に決まる
    let mut depth: Vec<u32> = Vec::with_capacity(edit.items.len());
    let mut size: Vec<u64> = Vec::with_capacity(edit.items.len());
    for (index, item) in edit.items.iter().enumerate() {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        let here = format!("{at}/items/{index}");
        let mut deepest = 0;
        let mut total: u64 = 1;
        for (field, child) in children(item) {
            let found = usize::try_from(child)
                .ok()
                .filter(|&n| n < index)
                .and_then(|n| Some((*depth.get(n)?, *size.get(n)?)));
            let Some((d, s)) = found else {
                return Err(fail(cx, invalid_value(format!("{here}/{field}"))));
            };
            deepest = deepest.max(d);
            total = total.saturating_add(s);
        }
        if deepest >= DEPTH {
            return Err(fail(cx, invalid_value(here)));
        }
        depth.push(deepest + 1);
        size.push(total);
    }
    let mut total: u64 = 0;
    for (n, &index) in edit.body.iter().enumerate() {
        let found = usize::try_from(index).ok().and_then(|i| size.get(i));
        let Some(s) = found else {
            return Err(fail(cx, invalid_value(format!("{at}/body/{n}"))));
        };
        total = total.saturating_add(*s);
    }
    if total > ITEMS {
        return Err(fail(cx, invalid_value(format!("{at}/body"))));
    }
    let builder = Builder {
        items: &edit.items,
        ifc2x3,
        at,
    };
    let mut body = Vec::with_capacity(edit.body.len());
    for &index in &edit.body {
        body.push(builder.item(index, cx)?);
    }
    Ok(NewShape { axis, body })
}

/// 項目が指す、前の項目のフィールドの名前と位置
fn children(item: &BuildingShapeItem) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    if let Some(first) = item.first {
        out.push(("first".to_owned(), first));
    }
    if let Some(second) = item.second {
        out.push(("second".to_owned(), second));
    }
    for (n, &member) in item.members.iter().flatten().enumerate() {
        out.push((format!("members/{n}"), member));
    }
    out
}

/// 項目の表から木を組み立てるもの
struct Builder<'a> {
    items: &'a [BuildingShapeItem],
    ifc2x3: bool,
    at: &'a str,
}

impl Builder<'_> {
    /// 位置 `index` の項目を組み立てる。指す項目は前にあり、深さは確かめてある
    fn item(&self, index: u64, cx: &mut Cx<'_>) -> Result<NewItem, Failed> {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        let at = format!("{}/items/{index}", self.at);
        let item = usize::try_from(index)
            .ok()
            .and_then(|i| self.items.get(i))
            .ok_or_else(|| fail(cx, invalid_value(at.clone())))?;
        if !fields_match(item) {
            return Err(fail(cx, invalid_value(at)));
        }
        let invalid =
            |cx: &mut Cx<'_>, field: &str| fail(cx, invalid_value(format!("{at}/{field}")));
        Ok(match item.kind {
            BuildingShapeItemKind::Extrusion => {
                let Some(profile) = &item.profile else {
                    return Err(invalid(cx, "profile"));
                };
                let profile = new_profile(profile).ok_or_else(|| invalid(cx, "profile"))?;
                let position = match &item.position {
                    Some(p) => Some(axis3(p).ok_or_else(|| invalid(cx, "position"))?),
                    None => None,
                };
                let direction = match &item.direction {
                    Some(d) => triple(d).ok_or_else(|| invalid(cx, "direction"))?,
                    None => [0.0, 0.0, 1.0],
                };
                let depth = item.depth.ok_or_else(|| invalid(cx, "depth"))?;
                NewItem::Solid(NewSolid {
                    profile,
                    position,
                    direction,
                    depth,
                })
            }
            BuildingShapeItemKind::Faces => {
                let (Some(points), Some(faces)) = (&item.points, &item.faces) else {
                    return Err(invalid(cx, "faces"));
                };
                let points = points
                    .iter()
                    .map(|p| triple(p))
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| invalid(cx, "points"))?;
                let closed = item.closed == Some(true);
                let faces = if closed || self.ifc2x3 {
                    graph(points, faces, closed)
                } else {
                    indexed(points, faces, item.closed)
                };
                NewItem::Faces(faces.ok_or_else(|| invalid(cx, "faces"))?)
            }
            BuildingShapeItemKind::Boolean => {
                let (Some(operation), Some(first), Some(second)) =
                    (item.operation, item.first, item.second)
                else {
                    return Err(invalid(cx, "operation"));
                };
                NewItem::Boolean(Box::new(NewBoolean {
                    operation: match operation {
                        BuildingBooleanOperation::Union => BooleanOperation::Union,
                        BuildingBooleanOperation::Intersection => BooleanOperation::Intersection,
                        BuildingBooleanOperation::Difference => BooleanOperation::Difference,
                    },
                    first: self.item(first, cx)?,
                    second: self.item(second, cx)?,
                }))
            }
            BuildingShapeItemKind::HalfSpace => {
                let Some(plane) = &item.plane else {
                    return Err(invalid(cx, "plane"));
                };
                let plane = axis3(plane).ok_or_else(|| invalid(cx, "plane"))?;
                let limit = match (&item.r#box, &item.polygon) {
                    (None, None) => NewHalfSpaceLimit::Unbounded,
                    (Some(b), None) => match (triple(&b.corner), triple(&b.dimensions)) {
                        (Some(corner), Some(dimensions)) => {
                            NewHalfSpaceLimit::Box { corner, dimensions }
                        }
                        _ => return Err(invalid(cx, "box")),
                    },
                    (None, Some(p)) => {
                        let position = axis3(&p.position);
                        let boundary = p.boundary.iter().map(|q| pair(q)).collect();
                        let (Some(position), Some(boundary)) = (position, boundary) else {
                            return Err(invalid(cx, "polygon"));
                        };
                        NewHalfSpaceLimit::Polygonal { position, boundary }
                    }
                    (Some(_), Some(_)) => return Err(invalid(cx, "polygon")),
                };
                NewItem::HalfSpace(NewHalfSpace {
                    plane,
                    agreement: item.agreement.unwrap_or(false),
                    limit,
                })
            }
            BuildingShapeItemKind::Mapped => {
                let (Some(target), Some(members)) = (&item.target, &item.members) else {
                    return Err(invalid(cx, "target"));
                };
                let origin = match &item.origin {
                    Some(o) => axis3(o).ok_or_else(|| invalid(cx, "origin"))?,
                    None => NewAxis3 {
                        location: [0.0; 3],
                        axis: None,
                        reference: None,
                    },
                };
                let target = mapping(target).ok_or_else(|| invalid(cx, "target"))?;
                let mut items = Vec::with_capacity(members.len());
                for &member in members {
                    items.push(self.item(member, cx)?);
                }
                NewItem::Mapped(Box::new(NewMapped {
                    origin,
                    target,
                    items,
                }))
            }
        })
    }
}

/// 項目が、種類に合うフィールドだけを持つか
fn fields_match(item: &BuildingShapeItem) -> bool {
    use BuildingShapeItemKind as K;
    let fields = [
        (item.profile.is_some(), K::Extrusion),
        (item.position.is_some(), K::Extrusion),
        (item.direction.is_some(), K::Extrusion),
        (item.depth.is_some(), K::Extrusion),
        (item.points.is_some(), K::Faces),
        (item.faces.is_some(), K::Faces),
        (item.closed.is_some(), K::Faces),
        (item.operation.is_some(), K::Boolean),
        (item.first.is_some(), K::Boolean),
        (item.second.is_some(), K::Boolean),
        (item.plane.is_some(), K::HalfSpace),
        (item.agreement.is_some(), K::HalfSpace),
        (item.r#box.is_some(), K::HalfSpace),
        (item.polygon.is_some(), K::HalfSpace),
        (item.origin.is_some(), K::Mapped),
        (item.target.is_some(), K::Mapped),
        (item.members.is_some(), K::Mapped),
    ];
    fields
        .iter()
        .all(|&(given, kind)| !given || kind == item.kind)
}

/// 3 次元の配置。数の数が違えば `None`
fn axis3(placement: &BuildingAxis3) -> Option<NewAxis3> {
    Some(NewAxis3 {
        location: triple(&placement.location)?,
        axis: match &placement.axis {
            Some(v) => Some(triple(v)?),
            None => None,
        },
        reference: match &placement.reference {
            Some(v) => Some(triple(v)?),
            None => None,
        },
    })
}

/// 断面。種類に合うフィールドがないか、ほかの種類のフィールドがあれば `None`
fn new_profile(profile: &BuildingShapeProfile) -> Option<NewProfile> {
    let BuildingShapeProfile {
        kind,
        name,
        x,
        y,
        radius,
        location,
        reference,
        outer,
        holes,
    } = profile;
    let position = match (location, reference) {
        (Some(location), reference) => Some(NewAxis2 {
            location: pair(location)?,
            reference: match reference {
                Some(r) => Some(pair(r)?),
                None => None,
            },
        }),
        (None, None) => None,
        (None, Some(_)) => return None,
    };
    let ring = |points: &[Vec<f64>]| points.iter().map(|p| pair(p)).collect::<Option<Vec<_>>>();
    let kind = match kind {
        BuildingShapeProfileKind::Rectangle => {
            if radius.is_some() || outer.is_some() || holes.is_some() {
                return None;
            }
            NewProfileKind::Rectangle {
                x: (*x)?,
                y: (*y)?,
                position,
            }
        }
        BuildingShapeProfileKind::Circle => {
            if x.is_some() || y.is_some() || outer.is_some() || holes.is_some() {
                return None;
            }
            NewProfileKind::Circle {
                radius: (*radius)?,
                position,
            }
        }
        BuildingShapeProfileKind::Polygon => {
            if x.is_some() || y.is_some() || radius.is_some() || position.is_some() {
                return None;
            }
            NewProfileKind::Polygon {
                outer: ring(outer.as_deref()?)?,
                holes: holes
                    .iter()
                    .flatten()
                    .map(|hole| ring(hole))
                    .collect::<Option<Vec<_>>>()?,
            }
        }
    };
    Some(NewProfile {
        name: name.clone(),
        kind,
    })
}

/// 写像先の変換。数の数が違うか、尺度を 2 通りに書けば `None`
fn mapping(target: &BuildingShapeMapping) -> Option<CartesianMapping3> {
    let direction = |v: &Option<Vec<f64>>| match v {
        Some(v) => triple(v).map(Some),
        None => Some(None),
    };
    let scale = match (target.scale, &target.scales) {
        (scale, None) => MappingScale3::Uniform(scale),
        (None, Some(scales)) => {
            let [x, y, z] = triple(scales)?;
            MappingScale3::NonUniform {
                x: Some(x),
                y: Some(y),
                z: Some(z),
            }
        }
        (Some(_), Some(_)) => return None,
    };
    Some(CartesianMapping3 {
        local_origin: triple(&target.origin)?,
        axis1: direction(&target.axis1)?,
        axis2: direction(&target.axis2)?,
        axis3: direction(&target.axis3)?,
        scale,
    })
}

/// 点の位置を 32 ビットの番号にする。範囲は L1 が確かめる
fn index(n: u64) -> Option<u32> {
    u32::try_from(n).ok()
}

/// 面を、多角形の面のグラフにする。閉殻なら立体（`IfcFacetedBrep`）、ほかは殻の面の模型
fn graph(points: Vec<[f64; 3]>, faces: &[BuildingShapeFace], closed: bool) -> Option<NewFaces> {
    let mut loops = Vec::new();
    let mut bounds = Vec::new();
    let mut polygons = Vec::with_capacity(faces.len());
    for face in faces {
        let mut refs = Vec::new();
        let rings = std::iter::once((&face.boundary, true))
            .chain(face.holes.iter().flatten().map(|hole| (hole, false)));
        for (ring, outer) in rings {
            let points = ring
                .iter()
                .map(|&n| index(n).map(FacePointRef))
                .collect::<Option<Vec<_>>>()?;
            let polygon = FaceLoopRef(u32::try_from(loops.len()).ok()?);
            loops.push(FacePolygon { points });
            refs.push(FaceBoundRef(u32::try_from(bounds.len()).ok()?));
            bounds.push(FaceBoundary {
                polygon,
                orientation: true,
                outer,
            });
        }
        polygons.push(PolygonFace { bounds: refs });
    }
    let all = (0..polygons.len())
        .map(|n| u32::try_from(n).ok().map(FaceRef))
        .collect::<Option<Vec<_>>>()?;
    Some(NewFaces::Polygons(FaceGraph {
        kind: if closed {
            FaceModelKind::FacetedSolid
        } else {
            FaceModelKind::ShellSurface
        },
        roots: vec![FaceSetRef(0)],
        sets: vec![ConnectedFaces {
            faces: all,
            closed: Some(closed),
        }],
        faces: polygons,
        bounds,
        loops,
        points,
    }))
}

/// 面を、添字の多角形の面（`IfcPolygonalFaceSet`）にする。添字は 1 から
fn indexed(
    points: Vec<[f64; 3]>,
    faces: &[BuildingShapeFace],
    closed: Option<bool>,
) -> Option<NewFaces> {
    let ring = |ring: &[u64]| {
        ring.iter()
            .map(|&n| index(n.checked_add(1)?))
            .collect::<Option<Vec<_>>>()
    };
    let polygons = faces
        .iter()
        .map(|face| {
            Some(IndexedPolygon {
                boundary: ring(&face.boundary)?,
                holes: face
                    .holes
                    .iter()
                    .flatten()
                    .map(|hole| ring(hole))
                    .collect::<Option<Vec<_>>>()?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let indices = (0..polygons.len())
        .map(|n| u32::try_from(n).ok().map(IndexedPolygonRef))
        .collect::<Option<Vec<_>>>()?;
    Some(NewFaces::Indexed(IndexedFaceSet {
        points,
        tags: None,
        point_index: None,
        closed,
        faces: IndexedFaces::Polygons {
            indices,
            faces: polygons,
        },
    }))
}
