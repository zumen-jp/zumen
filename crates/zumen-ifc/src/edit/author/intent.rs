//! 意図の値で足す変更（#898）。壁は芯の 2 点・高さ・材料の層で受け、配置・`Axis` と `Body` の表現・
//! `IfcMaterialLayerSetUsage`・所属の関係を、バージョンの表のとおりにコアが書く。
//!
//! 属性の並びは、buildingSMART が公開する各バージョンの仕様の実体の定義による。IFC2X3 TC1 では、
//! `IfcMaterial` は 1 Name、`IfcMaterialLayer` は 1 Material・2 LayerThickness・3 IsVentilated、
//! `IfcMaterialLayerSet` は 1 MaterialLayers・2 LayerSetName、`IfcMaterialLayerSetUsage` は 1 ForLayerSet・
//! 2 LayerSetDirection・3 DirectionSense・4 OffsetFromReferenceLine。IFC4 ADD2 TC1 と IFC4X3 ADD2 では、
//! `IfcMaterial` に 2 Description・3 Category、`IfcMaterialLayer` に 4 Name・5 Description・6 Category・7 Priority、
//! `IfcMaterialLayerSet` に 3 Description、`IfcMaterialLayerSetUsage` に 5 ReferenceExtent が続く。
//! 層のある壁は、IFC2X3 では `IfcWallStandardCase`、IFC4 と IFC4X3 では `IfcWall` に書く。

use std::collections::BTreeMap;

use zumen_base::Kind;
use zumen_building::{GlobalId, Transform};
use zumen_sdk::{Cx, Error};

use super::super::{Draft, Item, Stmt, text, unsupported};
use super::{CONTAINED, Target, null, raw, real};
use crate::building::{Category, Version};
use crate::stopped;

/// 壁の材料の層
#[derive(Clone, Debug, PartialEq)]
pub struct NewLayer {
    /// 材料の名前（`IfcMaterial` の Name）。同じ壁の中で同じ名前の層は、同じ材料を指す
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

const EDIT: &str = "wall";

/// 確かめた壁の値
struct Shape {
    start: [f64; 2],
    /// 芯の向きの単位ベクトル
    direction: [f64; 2],
    length: f64,
    height: f64,
    total: f64,
    offset: f64,
}

/// 壁の値を確かめる。書けなければ、書けない値の名前
fn shape(wall: &NewWall) -> Result<Shape, &'static str> {
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
    if !(length.is_finite() && length > 0.0) {
        return Err("end");
    }
    if !(wall.height.is_finite() && wall.height > 0.0) {
        return Err("height");
    }
    let mut total = 0.0;
    for layer in &wall.layers {
        if !(layer.thickness.is_finite() && layer.thickness > 0.0) {
            return Err("layers");
        }
        total += layer.thickness;
    }
    if wall.layers.is_empty() || !total.is_finite() {
        return Err("layers");
    }
    let offset = wall.offset.unwrap_or(-total / 2.0);
    if !offset.is_finite() || !(offset + total).is_finite() {
        return Err("offset");
    }
    Ok(Shape {
        start: wall.start,
        direction: [dx / length, dy / length],
        length,
        height: wall.height,
        total,
        offset,
    })
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

    /// 壁の `Axis`（芯の線）と `Body`（層の厚さの矩形を高さだけ押し出す）の表現
    fn wall_shape(&mut self, shape: &Shape) -> Option<u64> {
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
        // 断面は、層の厚さの合計の矩形を閉じた折れ線で書く。始点に戻る点は、始点と同じインスタンスにする
        let (near, far) = (shape.offset, shape.offset + shape.total);
        let mut corners = Vec::with_capacity(5);
        for corner in [
            [0.0, near],
            [shape.length, near],
            [shape.length, far],
            [0.0, far],
        ] {
            corners.push(Item::Ref(self.coordinates(b"IFCCARTESIANPOINT", &corner)?));
        }
        if let Some(first) = corners.first().cloned() {
            corners.push(first);
        }
        let outline = self.add(Stmt {
            name: b"IFCPOLYLINE".to_vec(),
            args: vec![Item::List(corners)],
        });
        // 1 ProfileType・2 ProfileName・3 OuterCurve
        let profile = self.add(Stmt {
            name: b"IFCARBITRARYCLOSEDPROFILEDEF".to_vec(),
            args: vec![raw(b".AREA."), null(), Item::Ref(outline)],
        });
        let origin = self.coordinates(b"IFCCARTESIANPOINT", &[0.0, 0.0, 0.0])?;
        let placement = self.add(Stmt {
            name: b"IFCAXIS2PLACEMENT3D".to_vec(),
            args: vec![Item::Ref(origin), null(), null()],
        });
        let up = self.coordinates(b"IFCDIRECTION", &[0.0, 0.0, 1.0])?;
        // 1 SweptArea・2 Position・3 ExtrudedDirection・4 Depth
        let solid = self.add(Stmt {
            name: b"IFCEXTRUDEDAREASOLID".to_vec(),
            args: vec![
                Item::Ref(profile),
                Item::Ref(placement),
                Item::Ref(up),
                real(shape.height)?,
            ],
        });
        let body = self.add(Stmt {
            name: b"IFCSHAPEREPRESENTATION".to_vec(),
            args: vec![
                Item::Ref(body_context),
                text("Body"),
                text("SweptSolid"),
                Item::List(vec![Item::Ref(solid)]),
            ],
        });
        Some(self.add(Stmt {
            name: b"IFCPRODUCTDEFINITIONSHAPE".to_vec(),
            args: vec![
                null(),
                null(),
                Item::List(vec![Item::Ref(axis), Item::Ref(body)]),
            ],
        }))
    }

    /// 壁の材料の層の使い方（`IfcMaterialLayerSetUsage`）。層の向きは芯の左（`AXIS2` の正の向き）
    fn layer_usage(
        &mut self,
        version: Version,
        layers: &[NewLayer],
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
                    let mut args = vec![text(&layer.material)];
                    if later {
                        args.extend([null(), null()]);
                    }
                    let id = self.add(Stmt {
                        name: b"IFCMATERIAL".to_vec(),
                        args,
                    });
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
        let mut args = vec![Item::Ref(set), raw(b".AXIS2."), raw(b".POSITIVE."), offset];
        if later {
            args.push(null());
        }
        Ok(Some(self.add(Stmt {
            name: b"IFCMATERIALLAYERSETUSAGE".to_vec(),
            args,
        })))
    }

    /// 芯の 2 点・高さ・材料の層で壁を足す
    pub(in crate::edit) fn add_wall(&mut self, wall: &NewWall, cx: &mut Cx) -> Result<bool, Error> {
        let Some(version) = self.version() else {
            cx.report(unsupported(EDIT, b"IFCWALL"));
            return Ok(false);
        };
        let (kind, arity): (&[u8], usize) = match version {
            Version::Ifc2x3 => (b"IFCWALLSTANDARDCASE", 8),
            Version::Ifc4 | Version::Ifc4x3 => (b"IFCWALL", 9),
        };
        let shape = match shape(wall) {
            Ok(shape) => shape,
            Err(value) => {
                cx.report(unsupported(EDIT, kind).with("value", value));
                return Ok(false);
            }
        };
        let Some((storey, ..)) = self.object(wall.storey, &[Category::Spatial], EDIT, cx) else {
            return Ok(false);
        };
        if self.project.is_none() {
            cx.report(unsupported(EDIT, kind).with("missing", "IfcProject"));
            return Ok(false);
        }
        let Some(history) = self.history(EDIT, kind, cx) else {
            return Ok(false);
        };
        let Some(global_id) = self.new_global_id(wall.global_id, EDIT, kind, cx)? else {
            return Ok(false);
        };
        let [ux, uy] = shape.direction;
        let [sx, sy] = shape.start;
        let local = Transform([[ux, -uy, 0.0, sx], [uy, ux, 0.0, sy], [0.0, 0.0, 1.0, 0.0]]);
        let parent = self.placement_of(storey);
        let placement = self.axis_placement(&local).map(|axis| {
            self.add(Stmt {
                name: b"IFCLOCALPLACEMENT".to_vec(),
                args: vec![parent.map_or_else(null, Item::Ref), Item::Ref(axis)],
            })
        });
        let representation = self.wall_shape(&shape);
        let (Some(placement), Some(representation)) = (placement, representation) else {
            cx.report(unsupported(EDIT, kind).with("value", "geometry"));
            return Ok(false);
        };
        let mut args = vec![null(); arity];
        if let [gid, owner, name, _, _, place, shape, ..] = args.as_mut_slice() {
            *gid = global_id;
            *owner = history.clone();
            *name = wall.name.as_deref().map_or_else(null, text);
            *place = Item::Ref(placement);
            *shape = Item::Ref(representation);
        }
        let id = self.add(Stmt {
            name: kind.to_vec(),
            args,
        });
        self.join(CONTAINED, storey, id, history.clone());
        let Some(usage) = self.layer_usage(version, &wall.layers, shape.offset, cx)? else {
            cx.report(unsupported(EDIT, kind).with("value", "layers"));
            return Ok(false);
        };
        // `IfcRelAssociatesMaterial`: 5 RelatedObjects・6 RelatingMaterial
        let relation = self.global_id();
        self.add(Stmt {
            name: b"IFCRELASSOCIATESMATERIAL".to_vec(),
            args: vec![
                relation,
                history,
                null(),
                null(),
                Item::List(vec![Item::Ref(id)]),
                Item::Ref(usage),
            ],
        });
        self.handles.push(id);
        Ok(true)
    }
}
