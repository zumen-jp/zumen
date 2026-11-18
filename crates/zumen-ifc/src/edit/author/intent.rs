//! 意図の値で足す変更（#898）。壁・床・柱・梁を、芯や外形・高さ・断面・材料の値で受け、配置・表現・材料・
//! 所属の関係を、バージョンの表のとおりにコアが書く。
//!
//! 属性の並びは、buildingSMART が公開する各バージョンの仕様の実体の定義による。IFC2X3 TC1 では、
//! `IfcMaterial` は 1 Name、`IfcMaterialLayer` は 1 Material・2 LayerThickness・3 IsVentilated、
//! `IfcMaterialLayerSet` は 1 MaterialLayers・2 LayerSetName、`IfcMaterialLayerSetUsage` は 1 ForLayerSet・
//! 2 LayerSetDirection・3 DirectionSense・4 OffsetFromReferenceLine。IFC4 ADD2 TC1 と IFC4X3 ADD2 では、
//! `IfcMaterial` に 2 Description・3 Category、`IfcMaterialLayer` に 4 Name・5 Description・6 Category・7 Priority、
//! `IfcMaterialLayerSet` に 3 Description、`IfcMaterialLayerSetUsage` に 5 ReferenceExtent が続く。
//! 層のある壁は、IFC2X3 では `IfcWallStandardCase`、IFC4 と IFC4X3 では `IfcWall` に書く。`IfcSlab` は
//! 3 版とも 9 PredefinedType まで、`IfcColumn` と `IfcBeam` は IFC2X3 では 8 Tag まで、IFC4 と IFC4X3 では
//! 9 PredefinedType まで。

use std::collections::BTreeMap;

use zumen_base::Kind;
use zumen_building::{GlobalId, Transform};
use zumen_sdk::{Cx, Error};

use super::super::{Draft, Item, Stmt, text, unsupported};
use super::{CONTAINED, FILLS, Target, VOIDS, null, raw, real, set_at};
use crate::building::{Category, Version};
use crate::stopped;

/// 材料の層
#[derive(Clone, Debug, PartialEq)]
pub struct NewLayer {
    /// 材料の名前（`IfcMaterial` の Name）。同じ要素の中で同じ名前の層は、同じ材料を指す
    pub material: String,
    /// 層の厚さ（文書の長さの単位）。正の値
    pub thickness: f64,
}

/// 芯の 2 点・高さ・材料の層で足す壁
#[derive(Clone, Debug, PartialEq)]
pub struct NewWall {
    /// 所属する空間（階など）。壁は、この空間の配置からの相対に置く
    pub storey: Target,
    /// 芯の始点。空間の配置の XY で、文書の長さの単位
    pub start: [f64; 2],
    /// 芯の終点。始点と違う点
    pub end: [f64; 2],
    /// 高さ（文書の長さの単位）。正の値
    pub height: f64,
    /// 材料の層。始点から終点を見て、芯の右から左へ並べる。1 つ以上
    pub layers: Vec<NewLayer>,
    /// 芯から最初の層の面までのずれ（`OffsetFromReferenceLine`、左が正）。なければ厚さの合計の半分を負にして、
    /// 芯を壁の厚さの中央にする
    pub offset: Option<f64>,
    /// 名前
    pub name: Option<String>,
    /// GlobalId。なければ文書のハッシュと足した数から作る
    pub global_id: Option<GlobalId>,
}

/// 外形・材料の層で足す床。上の面を基準にし、層は上から下へ並べる
#[derive(Clone, Debug, PartialEq)]
pub struct NewSlab {
    /// 所属する空間（階など）。床は、この空間の配置からの相対に置く
    pub storey: Target,
    /// 外形の頂点。空間の配置の XY で、3 つ以上。最後の点が最初の点と同じなら、その点は除く
    pub outline: Vec<[f64; 2]>,
    /// 材料の層。上から下へ並べる。1 つ以上
    pub layers: Vec<NewLayer>,
    /// 上の面の高さ（空間の配置の Z）。なければ 0
    pub elevation: Option<f64>,
    /// 名前
    pub name: Option<String>,
    /// GlobalId。なければ文書のハッシュと足した数から作る
    pub global_id: Option<GlobalId>,
}

/// 矩形の断面
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NewSection {
    /// 幅（文書の長さの単位）。正の値
    pub width: f64,
    /// せい（文書の長さの単位）。正の値
    pub depth: f64,
}

/// 位置・高さ・矩形の断面で足す柱
#[derive(Clone, Debug, PartialEq)]
pub struct NewColumn {
    /// 所属する空間（階など）。柱は、この空間の配置からの相対に置く
    pub storey: Target,
    /// 断面の中心。空間の配置の XY
    pub position: [f64; 2],
    /// 柱の脚の高さ（空間の配置の Z）。なければ 0
    pub base: Option<f64>,
    /// 高さ（文書の長さの単位）。正の値
    pub height: f64,
    /// 断面。幅は柱の x 軸、せいは y 軸の向き
    pub section: NewSection,
    /// 柱の x 軸の向き（空間の配置の XY）。なければ空間の x 軸
    pub direction: Option<[f64; 2]>,
    /// 材料の名前（`IfcMaterial` の Name）
    pub material: Option<String>,
    /// 名前
    pub name: Option<String>,
    /// GlobalId。なければ文書のハッシュと足した数から作る
    pub global_id: Option<GlobalId>,
}

/// 天端の芯の 2 点・矩形の断面で足す梁
#[derive(Clone, Debug, PartialEq)]
pub struct NewBeam {
    /// 所属する空間（階など）。梁は、この空間の配置からの相対に置く
    pub storey: Target,
    /// 天端の芯の始点。空間の配置の XYZ
    pub start: [f64; 3],
    /// 天端の芯の終点。始点と XY が違う点
    pub end: [f64; 3],
    /// 断面。幅は芯の左右、せいは天端から下へ
    pub section: NewSection,
    /// 材料の名前（`IfcMaterial` の Name）
    pub material: Option<String>,
    /// 名前
    pub name: Option<String>,
    /// GlobalId。なければ文書のハッシュと足した数から作る
    pub global_id: Option<GlobalId>,
}

/// 壁の芯に沿う位置・下端・矩形の大きさで足す開口。奥行きは壁の材料の層の厚さ
#[derive(Clone, Debug, PartialEq)]
pub struct NewOpening {
    /// 穴をあける要素。材料の層の使い方（`AXIS2`）を持つ壁
    pub host: Target,
    /// 壁の配置の x 軸に沿う、壁の原点から開口の端までの距離（文書の長さの単位）
    pub distance: f64,
    /// 壁の下端から開口の下端までの高さ（文書の長さの単位）
    pub sill: f64,
    /// 幅（文書の長さの単位）。正の値
    pub width: f64,
    /// 高さ（文書の長さの単位）。正の値
    pub height: f64,
    /// 名前
    pub name: Option<String>,
    /// GlobalId。なければ文書のハッシュと足した数から作る
    pub global_id: Option<GlobalId>,
}

/// 建具の種類
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillingKind {
    /// 扉（`IfcDoor`）
    Door,
    /// 窓（`IfcWindow`）
    Window,
}

/// 開口を埋める建具
#[derive(Clone, Debug, PartialEq)]
pub struct NewFilling {
    /// 埋める開口（`IfcOpeningElement`）
    pub opening: Target,
    /// 種類
    pub kind: FillingKind,
    /// 全体の幅（`OverallWidth`）。正の値
    pub width: Option<f64>,
    /// 全体の高さ（`OverallHeight`）。正の値
    pub height: Option<f64>,
    /// 名前
    pub name: Option<String>,
    /// GlobalId。なければ文書のハッシュと足した数から作る
    pub global_id: Option<GlobalId>,
}

/// 正の有限の値
fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

/// 層の厚さの合計。書けない層があれば `None`
fn total(layers: &[NewLayer]) -> Option<f64> {
    let mut total = 0.0;
    for layer in layers {
        if !positive(layer.thickness) {
            return None;
        }
        total += layer.thickness;
    }
    (!layers.is_empty() && total.is_finite()).then_some(total)
}

/// 確かめた壁の値
struct WallShape {
    start: [f64; 2],
    /// 芯の向きの単位ベクトル
    direction: [f64; 2],
    length: f64,
    height: f64,
    total: f64,
    offset: f64,
}

/// 壁の値を確かめる。書けなければ、書けない値の名前
fn wall_shape(wall: &NewWall) -> Result<WallShape, &'static str> {
    let finite = |v: &[f64]| v.iter().all(|v| v.is_finite());
    if !finite(&wall.start) {
        return Err("start");
    }
    if !finite(&wall.end) {
        return Err("end");
    }
    let dx = wall.end[0] - wall.start[0];
    let dy = wall.end[1] - wall.start[1];
    let length = libm::hypot(dx, dy);
    if !positive(length) {
        return Err("end");
    }
    if !positive(wall.height) {
        return Err("height");
    }
    let total = total(&wall.layers).ok_or("layers")?;
    let offset = wall.offset.unwrap_or(-total / 2.0);
    if !offset.is_finite() || !(offset + total).is_finite() {
        return Err("offset");
    }
    Ok(WallShape {
        start: wall.start,
        direction: [dx / length, dy / length],
        length,
        height: wall.height,
        total,
        offset,
    })
}

/// 床の外形を確かめる。最後の点が最初の点と同じなら除く。並ぶ 2 点が重なるか、面積がなければ `None`
fn outline(points: &[[f64; 2]]) -> Option<Vec<[f64; 2]>> {
    let mut points = points.to_vec();
    if points.len() > 1 && points.first() == points.last() {
        points.pop();
    }
    if points.len() < 3 || points.iter().flatten().any(|v| !v.is_finite()) {
        return None;
    }
    let mut twice = 0.0;
    for (i, a) in points.iter().enumerate() {
        let b = points.get(i + 1).or(points.first())?;
        if a == b {
            return None;
        }
        twice += a[0] * b[1] - b[0] * a[1];
    }
    (twice.is_finite() && twice != 0.0).then_some(points)
}

/// 単位の向き。長さがなければ `None`
fn unit(v: [f64; 2]) -> Option<[f64; 2]> {
    let length = libm::hypot(v[0], v[1]);
    positive(length).then(|| [v[0] / length, v[1] / length])
}

/// 足す要素に共通の値
struct Begun {
    /// 所属する空間か、基準にする要素
    anchor: u64,
    history: Item,
    global_id: Item,
}

impl Draft<'_> {
    /// 座標の並びの `IfcCartesianPoint` か `IfcDirection` を足す。有限でない値があれば `None`
    fn coordinates(&mut self, name: &[u8], values: &[f64]) -> Option<u64> {
        let items = values
            .iter()
            .map(|&v| real(v))
            .collect::<Option<Vec<_>>>()?;
        Some(self.add(Stmt {
            name: name.to_vec(),
            args: vec![Item::List(items)],
        }))
    }

    /// プロジェクトの 3 次元の `Model` の表現の文脈。なければ足し、プロジェクトの 8 RepresentationContexts に加える
    fn model_context(&mut self) -> Option<u64> {
        let project = self.project?;
        let mut stmt = self.stmt(project)?;
        let listed = match stmt.arg(8) {
            Some(Item::List(items)) => items.clone(),
            _ => Vec::new(),
        };
        let found = listed.iter().find_map(|item| {
            let Item::Ref(id) = item else {
                return None;
            };
            let context = self.stmt(*id)?;
            let model = context.name == b"IFCGEOMETRICREPRESENTATIONCONTEXT"
                && context
                    .arg(2)
                    .and_then(|kind| self.text_of(kind))
                    .is_some_and(|kind| kind.eq_ignore_ascii_case("Model"));
            model.then_some(*id)
        });
        if found.is_some() {
            return found;
        }
        let origin = self.coordinates(b"IFCCARTESIANPOINT", &[0.0, 0.0, 0.0])?;
        let system = self.add(Stmt {
            name: b"IFCAXIS2PLACEMENT3D".to_vec(),
            args: vec![Item::Ref(origin), null(), null()],
        });
        let context = self.add(Stmt {
            name: b"IFCGEOMETRICREPRESENTATIONCONTEXT".to_vec(),
            args: vec![
                null(),
                text("Model"),
                raw(b"3"),
                real(1e-5)?,
                Item::Ref(system),
                null(),
            ],
        });
        let mut contexts = listed;
        contexts.push(Item::Ref(context));
        stmt.set(8, Item::List(contexts));
        self.put(project, stmt);
        Some(context)
    }

    /// `parent` の下の、識別子が `identifier` の `IfcGeometricRepresentationSubContext`。なければ足す
    fn subcontext(&mut self, parent: u64, identifier: &str, view: &[u8]) -> u64 {
        let found = self.referrers(parent).into_iter().find(|&id| {
            self.stmt(id).is_some_and(|context| {
                context.name == b"IFCGEOMETRICREPRESENTATIONSUBCONTEXT"
                    && context.arg(7) == Some(&Item::Ref(parent))
                    && context
                        .arg(1)
                        .and_then(|name| self.text_of(name))
                        .is_some_and(|name| name.eq_ignore_ascii_case(identifier))
            })
        });
        if let Some(found) = found {
            return found;
        }
        // 1 ContextIdentifier・2 ContextType・3〜6 は導く値・7 ParentContext・8 TargetScale・9 TargetView・
        // 10 UserDefinedTargetView
        self.add(Stmt {
            name: b"IFCGEOMETRICREPRESENTATIONSUBCONTEXT".to_vec(),
            args: vec![
                text(identifier),
                text("Model"),
                raw(b"*"),
                raw(b"*"),
                raw(b"*"),
                raw(b"*"),
                Item::Ref(parent),
                null(),
                raw(view),
                null(),
            ],
        })
    }

    /// `Body` の表現。閉じた折れ線の断面を `position` の XY に置き、`direction` へ `depth` だけ押し出す。
    /// 始点に戻る点は、始点と同じインスタンスにする
    fn extruded_body(
        &mut self,
        context: u64,
        corners: &[[f64; 2]],
        position: &Transform,
        direction: [f64; 3],
        depth: f64,
    ) -> Option<u64> {
        let mut points = Vec::with_capacity(corners.len() + 1);
        for corner in corners {
            points.push(Item::Ref(self.coordinates(b"IFCCARTESIANPOINT", corner)?));
        }
        points.push(points.first().cloned()?);
        let outline = self.add(Stmt {
            name: b"IFCPOLYLINE".to_vec(),
            args: vec![Item::List(points)],
        });
        // 1 ProfileType・2 ProfileName・3 OuterCurve
        let profile = self.add(Stmt {
            name: b"IFCARBITRARYCLOSEDPROFILEDEF".to_vec(),
            args: vec![raw(b".AREA."), null(), Item::Ref(outline)],
        });
        let placement = self.axis_placement(position)?;
        let direction = self.coordinates(b"IFCDIRECTION", &direction)?;
        // 1 SweptArea・2 Position・3 ExtrudedDirection・4 Depth
        let solid = self.add(Stmt {
            name: b"IFCEXTRUDEDAREASOLID".to_vec(),
            args: vec![
                Item::Ref(profile),
                Item::Ref(placement),
                Item::Ref(direction),
                real(depth)?,
            ],
        });
        Some(self.add(Stmt {
            name: b"IFCSHAPEREPRESENTATION".to_vec(),
            args: vec![
                Item::Ref(context),
                text("Body"),
                text("SweptSolid"),
                Item::List(vec![Item::Ref(solid)]),
            ],
        }))
    }

    /// 表現の並びの `IfcProductDefinitionShape`
    fn product_shape(&mut self, representations: &[u64]) -> u64 {
        self.add(Stmt {
            name: b"IFCPRODUCTDEFINITIONSHAPE".to_vec(),
            args: vec![
                null(),
                null(),
                Item::List(representations.iter().map(|&id| Item::Ref(id)).collect()),
            ],
        })
    }

    /// `Body` の表現だけの形。断面を `position` の XY に置き、`direction` へ `depth` だけ押し出す
    fn body_shape(
        &mut self,
        corners: &[[f64; 2]],
        position: &Transform,
        direction: [f64; 3],
        depth: f64,
    ) -> Option<u64> {
        let model = self.model_context()?;
        let context = self.subcontext(model, "Body", b".MODEL_VIEW.");
        let body = self.extruded_body(context, corners, position, direction, depth)?;
        Some(self.product_shape(&[body]))
    }

    /// 壁の `Axis`（芯の線）と `Body`（層の厚さの矩形を高さだけ押し出す）の表現
    fn wall_representation(&mut self, shape: &WallShape) -> Option<u64> {
        let model = self.model_context()?;
        let axis_context = self.subcontext(model, "Axis", b".GRAPH_VIEW.");
        let body_context = self.subcontext(model, "Body", b".MODEL_VIEW.");
        let p0 = self.coordinates(b"IFCCARTESIANPOINT", &[0.0, 0.0])?;
        let p1 = self.coordinates(b"IFCCARTESIANPOINT", &[shape.length, 0.0])?;
        let line = self.add(Stmt {
            name: b"IFCPOLYLINE".to_vec(),
            args: vec![Item::List(vec![Item::Ref(p0), Item::Ref(p1)])],
        });
        let axis = self.add(Stmt {
            name: b"IFCSHAPEREPRESENTATION".to_vec(),
            args: vec![
                Item::Ref(axis_context),
                text("Axis"),
                text("Curve2D"),
                Item::List(vec![Item::Ref(line)]),
            ],
        });
        let (near, far) = (shape.offset, shape.offset + shape.total);
        let body = self.extruded_body(
            body_context,
            &[
                [0.0, near],
                [shape.length, near],
                [shape.length, far],
                [0.0, far],
            ],
            &IDENTITY,
            [0.0, 0.0, 1.0],
            shape.height,
        )?;
        Some(self.product_shape(&[axis, body]))
    }

    /// 材料の層の使い方（`IfcMaterialLayerSetUsage`）。同じ名前の層は、同じ `IfcMaterial` を指す
    fn layer_usage(
        &mut self,
        version: Version,
        layers: &[NewLayer],
        (direction, sense): (&[u8], &[u8]),
        offset: f64,
        cx: &mut Cx,
    ) -> Result<Option<u64>, Error> {
        let later = version != Version::Ifc2x3;
        let mut materials: BTreeMap<&str, u64> = BTreeMap::new();
        let mut written = Vec::with_capacity(layers.len());
        for layer in layers {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, None))?;
            let material = match materials.get(layer.material.as_str()) {
                Some(&id) => id,
                None => {
                    let id = self.material(version, &layer.material);
                    materials.insert(&layer.material, id);
                    id
                }
            };
            let Some(thickness) = real(layer.thickness) else {
                return Ok(None);
            };
            let mut args = vec![Item::Ref(material), thickness, null()];
            if later {
                args.extend([null(), null(), null(), null()]);
            }
            written.push(Item::Ref(self.add(Stmt {
                name: b"IFCMATERIALLAYER".to_vec(),
                args,
            })));
        }
        let mut args = vec![Item::List(written), null()];
        if later {
            args.push(null());
        }
        let set = self.add(Stmt {
            name: b"IFCMATERIALLAYERSET".to_vec(),
            args,
        });
        let Some(offset) = real(offset) else {
            return Ok(None);
        };
        let mut args = vec![Item::Ref(set), raw(direction), raw(sense), offset];
        if later {
            args.push(null());
        }
        Ok(Some(self.add(Stmt {
            name: b"IFCMATERIALLAYERSETUSAGE".to_vec(),
            args,
        })))
    }

    /// `IfcMaterial` を足す
    fn material(&mut self, version: Version, name: &str) -> u64 {
        let mut args = vec![text(name)];
        if version != Version::Ifc2x3 {
            args.extend([null(), null()]);
        }
        self.add(Stmt {
            name: b"IFCMATERIAL".to_vec(),
            args,
        })
    }

    /// 足す要素に共通の値。所属する空間か基準にする要素・所有履歴・GlobalId。書けなければ診断して `None`
    fn begin(
        &mut self,
        (anchor, allowed): (Target, &[Category]),
        given: Option<GlobalId>,
        edit: &str,
        kind: &[u8],
        cx: &mut Cx,
    ) -> Result<Option<Begun>, Error> {
        let Some((anchor, ..)) = self.object(anchor, allowed, edit, cx) else {
            return Ok(None);
        };
        if self.project.is_none() {
            cx.report(unsupported(edit, kind).with("missing", "IfcProject"));
            return Ok(None);
        }
        let Some(history) = self.history(edit, kind, cx) else {
            return Ok(None);
        };
        let Some(global_id) = self.new_global_id(given, edit, kind, cx)? else {
            return Ok(None);
        };
        Ok(Some(Begun {
            anchor,
            history,
            global_id,
        }))
    }

    /// 空間か要素の配置からの相対の `IfcLocalPlacement`
    fn local_placement(&mut self, relative_to: u64, local: &Transform) -> Option<u64> {
        let parent = self.placement_of(relative_to);
        let axis = self.axis_placement(local)?;
        Some(self.add(Stmt {
            name: b"IFCLOCALPLACEMENT".to_vec(),
            args: vec![parent.map_or_else(null, Item::Ref), Item::Ref(axis)],
        }))
    }

    /// 要素の文を足す。`extra` は、ほかの属性（1 から数えた位置）に書く値
    fn element_of(
        &mut self,
        (kind, arity): (&[u8], usize),
        begun: &Begun,
        name: Option<&str>,
        (placement, shape): (u64, Option<u64>),
        extra: Vec<(usize, Item)>,
    ) -> u64 {
        let mut args = vec![null(); arity];
        set_at(&mut args, 1, begun.global_id.clone());
        set_at(&mut args, 2, begun.history.clone());
        set_at(&mut args, 3, name.map_or_else(null, text));
        set_at(&mut args, 6, Item::Ref(placement));
        set_at(&mut args, 7, shape.map_or_else(null, Item::Ref));
        for (at, item) in extra {
            set_at(&mut args, at, item);
        }
        self.add(Stmt {
            name: kind.to_vec(),
            args,
        })
    }

    /// 要素を足し、空間に包含で結ぶ
    fn contained_element(
        &mut self,
        entity: (&[u8], usize),
        begun: &Begun,
        name: Option<&str>,
        (placement, shape): (u64, u64),
        extra: Vec<(usize, Item)>,
    ) -> u64 {
        let id = self.element_of(entity, begun, name, (placement, Some(shape)), extra);
        self.join(CONTAINED, begun.anchor, id, begun.history.clone());
        id
    }

    /// 要素に材料を `IfcRelAssociatesMaterial` で結ぶ
    fn associate(&mut self, element: u64, material: u64, history: Item) {
        // 5 RelatedObjects・6 RelatingMaterial
        let relation = self.global_id();
        self.add(Stmt {
            name: b"IFCRELASSOCIATESMATERIAL".to_vec(),
            args: vec![
                relation,
                history,
                null(),
                null(),
                Item::List(vec![Item::Ref(element)]),
                Item::Ref(material),
            ],
        });
    }

    /// 芯の 2 点・高さ・材料の層で壁を足す
    pub(in crate::edit) fn add_wall(&mut self, wall: &NewWall, cx: &mut Cx) -> Result<bool, Error> {
        const EDIT: &str = "wall";
        let Some(version) = self.version() else {
            cx.report(unsupported(EDIT, b"IFCWALL"));
            return Ok(false);
        };
        let entity: (&[u8], usize) = match version {
            Version::Ifc2x3 => (b"IFCWALLSTANDARDCASE", 8),
            Version::Ifc4 | Version::Ifc4x3 => (b"IFCWALL", 9),
        };
        let kind = entity.0;
        let shape = match wall_shape(wall) {
            Ok(shape) => shape,
            Err(value) => {
                cx.report(unsupported(EDIT, kind).with("value", value));
                return Ok(false);
            }
        };
        let Some(begun) = self.begin((wall.storey, SPATIAL), wall.global_id, EDIT, kind, cx)?
        else {
            return Ok(false);
        };
        let [ux, uy] = shape.direction;
        let [sx, sy] = shape.start;
        let local = Transform([[ux, -uy, 0.0, sx], [uy, ux, 0.0, sy], [0.0, 0.0, 1.0, 0.0]]);
        let placement = self.local_placement(begun.anchor, &local);
        let representation = self.wall_representation(&shape);
        let (Some(placement), Some(representation)) = (placement, representation) else {
            cx.report(unsupported(EDIT, kind).with("value", "geometry"));
            return Ok(false);
        };
        let id = self.contained_element(
            entity,
            &begun,
            wall.name.as_deref(),
            (placement, representation),
            Vec::new(),
        );
        let axis2 = (b".AXIS2.".as_slice(), b".POSITIVE.".as_slice());
        let Some(usage) = self.layer_usage(version, &wall.layers, axis2, shape.offset, cx)? else {
            cx.report(unsupported(EDIT, kind).with("value", "layers"));
            return Ok(false);
        };
        self.associate(id, usage, begun.history);
        self.handles.push(id);
        Ok(true)
    }

    /// 外形・材料の層で床を足す。上の面から層の厚さの合計だけ下へ押し出す
    pub(in crate::edit) fn add_slab(&mut self, slab: &NewSlab, cx: &mut Cx) -> Result<bool, Error> {
        const EDIT: &str = "slab";
        const KIND: &[u8] = b"IFCSLAB";
        let Some(version) = self.version() else {
            cx.report(unsupported(EDIT, KIND));
            return Ok(false);
        };
        cx.check(
            Kind::Work,
            u64::try_from(slab.outline.len()).unwrap_or(u64::MAX),
        )
        .map_err(|stop| stopped(cx, stop, None))?;
        let checked = (|| {
            let corners = outline(&slab.outline).ok_or("outline")?;
            let total = total(&slab.layers).ok_or("layers")?;
            let elevation = slab.elevation.unwrap_or(0.0);
            if !elevation.is_finite() {
                return Err("elevation");
            }
            Ok((corners, total, elevation))
        })();
        let (corners, total, elevation) = match checked {
            Ok(checked) => checked,
            Err(value) => {
                cx.report(unsupported(EDIT, KIND).with("value", value));
                return Ok(false);
            }
        };
        let Some(begun) = self.begin((slab.storey, SPATIAL), slab.global_id, EDIT, KIND, cx)?
        else {
            return Ok(false);
        };
        let local = Transform([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, elevation],
        ]);
        let placement = self.local_placement(begun.anchor, &local);
        let shape = self.body_shape(&corners, &IDENTITY, [0.0, 0.0, -1.0], total);
        let (Some(placement), Some(shape)) = (placement, shape) else {
            cx.report(unsupported(EDIT, KIND).with("value", "geometry"));
            return Ok(false);
        };
        let id = self.contained_element(
            (KIND, 9),
            &begun,
            slab.name.as_deref(),
            (placement, shape),
            vec![(9, raw(b".FLOOR."))],
        );
        let axis3 = (b".AXIS3.".as_slice(), b".NEGATIVE.".as_slice());
        let Some(usage) = self.layer_usage(version, &slab.layers, axis3, 0.0, cx)? else {
            cx.report(unsupported(EDIT, KIND).with("value", "layers"));
            return Ok(false);
        };
        self.associate(id, usage, begun.history);
        self.handles.push(id);
        Ok(true)
    }

    /// 位置・高さ・矩形の断面で柱を足す。断面の中心を配置の原点にし、脚から上へ押し出す
    pub(in crate::edit) fn add_column(
        &mut self,
        column: &NewColumn,
        cx: &mut Cx,
    ) -> Result<bool, Error> {
        const EDIT: &str = "column";
        const KIND: &[u8] = b"IFCCOLUMN";
        let Some(version) = self.version() else {
            cx.report(unsupported(EDIT, KIND));
            return Ok(false);
        };
        let checked = (|| {
            if !column.position.iter().all(|v| v.is_finite()) {
                return Err("position");
            }
            let base = column.base.unwrap_or(0.0);
            if !base.is_finite() {
                return Err("base");
            }
            if !positive(column.height) {
                return Err("height");
            }
            if !(positive(column.section.width) && positive(column.section.depth)) {
                return Err("section");
            }
            let direction = unit(column.direction.unwrap_or([1.0, 0.0])).ok_or("direction")?;
            Ok((base, direction))
        })();
        let (base, [ux, uy]) = match checked {
            Ok(checked) => checked,
            Err(value) => {
                cx.report(unsupported(EDIT, KIND).with("value", value));
                return Ok(false);
            }
        };
        let Some(begun) = self.begin((column.storey, SPATIAL), column.global_id, EDIT, KIND, cx)?
        else {
            return Ok(false);
        };
        let [px, py] = column.position;
        let local = Transform([[ux, -uy, 0.0, px], [uy, ux, 0.0, py], [0.0, 0.0, 1.0, base]]);
        let (w, d) = (column.section.width / 2.0, column.section.depth / 2.0);
        let placement = self.local_placement(begun.anchor, &local);
        let shape = self.body_shape(
            &[[-w, -d], [w, -d], [w, d], [-w, d]],
            &IDENTITY,
            [0.0, 0.0, 1.0],
            column.height,
        );
        let (Some(placement), Some(shape)) = (placement, shape) else {
            cx.report(unsupported(EDIT, KIND).with("value", "geometry"));
            return Ok(false);
        };
        let id = self.contained_element(
            (KIND, if version == Version::Ifc2x3 { 8 } else { 9 }),
            &begun,
            column.name.as_deref(),
            (placement, shape),
            Vec::new(),
        );
        if let Some(name) = &column.material {
            let material = self.material(version, name);
            self.associate(id, material, begun.history);
        }
        self.handles.push(id);
        Ok(true)
    }

    /// 天端の芯の 2 点・矩形の断面で梁を足す。始点を配置の原点、芯の向きを x 軸にし、断面を芯に沿って押し出す
    pub(in crate::edit) fn add_beam(&mut self, beam: &NewBeam, cx: &mut Cx) -> Result<bool, Error> {
        const EDIT: &str = "beam";
        const KIND: &[u8] = b"IFCBEAM";
        let Some(version) = self.version() else {
            cx.report(unsupported(EDIT, KIND));
            return Ok(false);
        };
        let checked = (|| {
            if !beam.start.iter().all(|v| v.is_finite()) {
                return Err("start");
            }
            if !beam.end.iter().all(|v| v.is_finite()) {
                return Err("end");
            }
            let [dx, dy, dz] = [
                beam.end[0] - beam.start[0],
                beam.end[1] - beam.start[1],
                beam.end[2] - beam.start[2],
            ];
            // 芯の XY の向き。鉛直の芯は柱にするので断る
            let plan = libm::hypot(dx, dy);
            let length = libm::hypot(plan, dz);
            if !(positive(plan) && positive(length)) {
                return Err("end");
            }
            if !(positive(beam.section.width) && positive(beam.section.depth)) {
                return Err("section");
            }
            Ok((
                [dx / length, dy / length, dz / length],
                [-dy / plan, dx / plan],
                length,
            ))
        })();
        let ([ux, uy, uz], [yx, yy], length) = match checked {
            Ok(checked) => checked,
            Err(value) => {
                cx.report(unsupported(EDIT, KIND).with("value", value));
                return Ok(false);
            }
        };
        let Some(begun) = self.begin((beam.storey, SPATIAL), beam.global_id, EDIT, KIND, cx)?
        else {
            return Ok(false);
        };
        // x は芯の向き、y は水平で芯の左、z は x と y の外積（上を向く）
        let z = [-uz * yy, uz * yx, ux * yy - uy * yx];
        let [sx, sy, sz] = beam.start;
        let local = Transform([[ux, yx, z[0], sx], [uy, yy, z[1], sy], [uz, 0.0, z[2], sz]]);
        // 断面は配置の YZ に置き（断面の x を配置の y、断面の y を配置の z）、配置の x へ押し出す
        let position = Transform([
            [0.0, 0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
        ]);
        let (w, d) = (beam.section.width / 2.0, beam.section.depth);
        let placement = self.local_placement(begun.anchor, &local);
        let shape = self.body_shape(
            &[[-w, -d], [w, -d], [w, 0.0], [-w, 0.0]],
            &position,
            [0.0, 0.0, 1.0],
            length,
        );
        let (Some(placement), Some(shape)) = (placement, shape) else {
            cx.report(unsupported(EDIT, KIND).with("value", "geometry"));
            return Ok(false);
        };
        let id = self.contained_element(
            (KIND, if version == Version::Ifc2x3 { 8 } else { 9 }),
            &begun,
            beam.name.as_deref(),
            (placement, shape),
            Vec::new(),
        );
        if let Some(name) = &beam.material {
            let material = self.material(version, name);
            self.associate(id, material, begun.history);
        }
        self.handles.push(id);
        Ok(true)
    }

    /// 要素の材料の層の使い方（`AXIS2`）の、配置の y で見た層の範囲（小さいほう、大きいほう）
    fn layer_span(&self, element: u64) -> Option<(f64, f64)> {
        let number = |item: Option<&Item>| match item {
            Some(Item::Raw(bytes)) => std::str::from_utf8(bytes).ok()?.parse::<f64>().ok(),
            _ => None,
        };
        let usage = self.referrers(element).into_iter().find_map(|rel| {
            let stmt = self.stmt(rel)?;
            if stmt.name != b"IFCRELASSOCIATESMATERIAL"
                || !matches!(stmt.arg(5), Some(Item::List(items)) if items.contains(&Item::Ref(element)))
            {
                return None;
            }
            let Some(Item::Ref(usage)) = stmt.arg(6) else {
                return None;
            };
            let usage = self.stmt(*usage)?;
            (usage.name == b"IFCMATERIALLAYERSETUSAGE"
                && usage.arg(2) == Some(&raw(b".AXIS2.")))
            .then_some(usage)
        })?;
        let Some(Item::Ref(set)) = usage.arg(1) else {
            return None;
        };
        let Some(Item::List(layers)) = self.stmt(*set)?.arg(1).cloned() else {
            return None;
        };
        let mut total = 0.0;
        for layer in layers {
            let Item::Ref(layer) = layer else {
                return None;
            };
            total += number(self.stmt(layer)?.arg(2))?;
        }
        let offset = number(usage.arg(4))?;
        let span = if usage.arg(3) == Some(&raw(b".NEGATIVE.")) {
            (offset - total, offset)
        } else {
            (offset, offset + total)
        };
        (positive(total) && span.0.is_finite() && span.1.is_finite()).then_some(span)
    }

    /// 壁の芯に沿う位置・下端・矩形の大きさで開口を足し、`IfcRelVoidsElement` で壁に穴をあける。奥行きは
    /// 壁の材料の層の厚さ
    pub(in crate::edit) fn add_opening(
        &mut self,
        opening: &NewOpening,
        cx: &mut Cx,
    ) -> Result<bool, Error> {
        const EDIT: &str = "opening";
        const KIND: &[u8] = b"IFCOPENINGELEMENT";
        let Some(version) = self.version() else {
            cx.report(unsupported(EDIT, KIND));
            return Ok(false);
        };
        let checked = if !(opening.distance.is_finite() && opening.sill.is_finite()) {
            Err("position")
        } else if !(positive(opening.width) && positive(opening.height)) {
            Err("size")
        } else {
            Ok(())
        };
        if let Err(value) = checked {
            cx.report(unsupported(EDIT, KIND).with("value", value));
            return Ok(false);
        }
        let host = (opening.host, ELEMENT);
        let Some(begun) = self.begin(host, opening.global_id, EDIT, KIND, cx)? else {
            return Ok(false);
        };
        let Some((near, far)) = self.layer_span(begun.anchor) else {
            cx.report(unsupported(EDIT, KIND).with("missing", "IfcMaterialLayerSetUsage"));
            return Ok(false);
        };
        let local = Transform([
            [1.0, 0.0, 0.0, opening.distance],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, opening.sill],
        ]);
        // 断面は壁の XZ に置き（断面の y を壁の z）、層の遠い面から近い面へ押し出す
        let position = Transform([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, -1.0, far],
            [0.0, 1.0, 0.0, 0.0],
        ]);
        let (w, h) = (opening.width, opening.height);
        let placement = self.local_placement(begun.anchor, &local);
        let shape = self.body_shape(
            &[[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]],
            &position,
            [0.0, 0.0, 1.0],
            far - near,
        );
        let (Some(placement), Some(shape)) = (placement, shape) else {
            cx.report(unsupported(EDIT, KIND).with("value", "geometry"));
            return Ok(false);
        };
        let (arity, extra) = match version {
            Version::Ifc2x3 => (8, Vec::new()),
            Version::Ifc4 | Version::Ifc4x3 => (9, vec![(9, raw(b".OPENING."))]),
        };
        let id = self.element_of(
            (KIND, arity),
            &begun,
            opening.name.as_deref(),
            (placement, Some(shape)),
            extra,
        );
        self.relation(VOIDS, begun.anchor, id, begun.history);
        self.handles.push(id);
        Ok(true)
    }

    /// 開口を建具で埋める（`IfcRelFillsElement`）。建具は開口の配置に置き、壁と同じ空間に包含で結ぶ
    pub(in crate::edit) fn add_filling(
        &mut self,
        filling: &NewFilling,
        cx: &mut Cx,
    ) -> Result<bool, Error> {
        const EDIT: &str = "filling";
        let kind: &[u8] = match filling.kind {
            FillingKind::Door => b"IFCDOOR",
            FillingKind::Window => b"IFCWINDOW",
        };
        let Some(version) = self.version() else {
            cx.report(unsupported(EDIT, kind));
            return Ok(false);
        };
        let size = |value: Option<f64>| match value {
            None => Some(null()),
            Some(value) if positive(value) => real(value),
            Some(_) => None,
        };
        let (Some(width), Some(height)) = (size(filling.width), size(filling.height)) else {
            cx.report(unsupported(EDIT, kind).with("value", "size"));
            return Ok(false);
        };
        let target = (filling.opening, ELEMENT);
        let Some(begun) = self.begin(target, filling.global_id, EDIT, kind, cx)? else {
            return Ok(false);
        };
        let opening = begun.anchor;
        if self.kind(opening) != b"IFCOPENINGELEMENT" {
            cx.report(unsupported(EDIT, &self.kind(opening)));
            return Ok(false);
        }
        // 開口を埋める建具は 1 つにする
        let taken = self.referrers(opening).into_iter().any(|rel| {
            self.kind(rel) == FILLS
                && self
                    .stmt(rel)
                    .is_some_and(|stmt| stmt.arg(5) == Some(&Item::Ref(opening)))
        });
        if taken {
            cx.report(unsupported(EDIT, kind).with("value", "taken"));
            return Ok(false);
        }
        // 壁の空間。開口が穴をあける壁の、包含の関係の空間
        let host = self.referrers(opening).into_iter().find_map(|rel| {
            let stmt = self.stmt(rel)?;
            match (stmt.name == VOIDS, stmt.arg(5), stmt.arg(6)) {
                (true, Some(Item::Ref(host)), Some(Item::Ref(of))) if *of == opening => Some(*host),
                _ => None,
            }
        });
        let space = host.and_then(|host| {
            self.referrers(host).into_iter().find_map(|rel| {
                let stmt = self.stmt(rel)?;
                if stmt.name != CONTAINED.name {
                    return None;
                }
                match (stmt.arg(CONTAINED.related), stmt.arg(CONTAINED.relating)) {
                    (Some(Item::List(items)), Some(Item::Ref(space)))
                        if items.contains(&Item::Ref(host)) =>
                    {
                        Some(*space)
                    }
                    _ => None,
                }
            })
        });
        let Some(placement) = self.local_placement(opening, &IDENTITY) else {
            cx.report(unsupported(EDIT, kind).with("value", "geometry"));
            return Ok(false);
        };
        // IFC2X3 は 9 OverallHeight・10 OverallWidth まで。IFC4 と IFC4X3 は 11 PredefinedType が続く
        let mut extra = vec![(9, height), (10, width)];
        let arity = match version {
            Version::Ifc2x3 => 10,
            Version::Ifc4 | Version::Ifc4x3 => {
                let predefined: &[u8] = match filling.kind {
                    FillingKind::Door => b".DOOR.",
                    FillingKind::Window => b".WINDOW.",
                };
                extra.push((11, raw(predefined)));
                13
            }
        };
        let id = self.element_of(
            (kind, arity),
            &begun,
            filling.name.as_deref(),
            (placement, None),
            extra,
        );
        if let Some(space) = space {
            self.join(CONTAINED, space, id, begun.history.clone());
        }
        self.relation(FILLS, opening, id, begun.history);
        self.handles.push(id);
        Ok(true)
    }

    /// 5 Relating・6 Related の 1 対 1 の関係を足す
    fn relation(&mut self, name: &[u8], relating: u64, related: u64, history: Item) {
        let global_id = self.global_id();
        self.add(Stmt {
            name: name.to_vec(),
            args: vec![
                global_id,
                history,
                null(),
                null(),
                Item::Ref(relating),
                Item::Ref(related),
            ],
        });
    }
}

/// 所属できる空間
const SPATIAL: &[Category] = &[Category::Spatial];

/// 穴をあける要素と、埋める開口
const ELEMENT: &[Category] = &[Category::Element];

/// 向きを変えない配置
const IDENTITY: Transform = Transform([
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
]);
