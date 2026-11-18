//! 要素と型に材料の構成を結ぶ変更（#395）。
//!
//! 属性の並びは、IFC2X3 TC1 の材料の資源（8.10）と、IFC4 ADD2 TC1・IFC4X3 ADD2 の材料の資源（8.10）の実体の
//! 定義による。
//!
//! - `IfcMaterial`: 1 Name。IFC4 から 2 Description・3 Category
//! - `IfcMaterialList`: 1 Materials
//! - `IfcMaterialLayer`: 1 Material・2 LayerThickness・3 IsVentilated。IFC4 から 4 Name・5 Description・
//!   6 Category・7 Priority。厚さは IFC2X3 では正、IFC4 からは負でない値
//! - `IfcMaterialLayerSet`: 1 MaterialLayers・2 LayerSetName。IFC4 から 3 Description
//! - `IfcMaterialLayerSetUsage`: 1 ForLayerSet・2 LayerSetDirection・3 DirectionSense・
//!   4 OffsetFromReferenceLine。IFC4 から 5 ReferenceExtent
//! - `IfcMaterialConstituent`（IFC4 から）: 1 Name・2 Description・3 Material・4 Fraction・5 Category
//! - `IfcMaterialConstituentSet`（IFC4 から）: 1 Name・2 Description・3 MaterialConstituents
//! - `IfcRelAssociatesMaterial`: 5 RelatedObjects・6 RelatingMaterial

use std::collections::BTreeMap;

use zumen_base::Kind;
use zumen_sdk::{Cx, Error};

use super::{Category, Draft, Item, Relation, Stmt, Target, null, raw, real, text, unsupported};
use crate::building::Version;
use crate::stopped;

/// `IfcRelAssociatesMaterial`: 5 RelatedObjects・6 RelatingMaterial
const ASSOCIATES_MATERIAL: Relation = Relation {
    name: b"IFCRELASSOCIATESMATERIAL",
    relating: 6,
    related: 5,
};

/// 要素に結ぶ材料の構成
#[derive(Clone, Debug, PartialEq)]
pub enum NewMaterial {
    /// 単一の材料（`IfcMaterial`）。値は材料の名前
    Single(String),
    /// 材料の並び（`IfcMaterialList`）。IFC2X3 だけ。IFC4 からは非推奨（ifc-gherkin-rules の IFC102）なので、
    /// 構成要素を使う。空にはできない
    List(Vec<String>),
    /// 材料の層（`IfcMaterialLayerSet`）。`usage` があれば、層の使い方（`IfcMaterialLayerSetUsage`）で結ぶ
    Layers {
        /// 層。空にはできない
        layers: Vec<NewMaterialLayer>,
        /// 層の使い方
        usage: Option<NewLayerUsage>,
    },
    /// 構成要素（`IfcMaterialConstituentSet`）。IFC4 から。空にはできない
    Constituents(Vec<NewConstituent>),
}

/// 材料の層
#[derive(Clone, Debug, PartialEq)]
pub struct NewMaterialLayer {
    /// 材料の名前。`None` は材料のない層（空気層など）
    pub material: Option<String>,
    /// 厚さ（文書の長さの単位）。IFC2X3 は正、IFC4 からは負でない値
    pub thickness: f64,
    /// 層の名前。IFC2X3 は持たないので、渡せば断る
    pub name: Option<String>,
}

/// 材料の層の使い方
#[derive(Clone, Debug, PartialEq)]
pub struct NewLayerUsage {
    /// 層を重ねる向き（`AXIS1`・`AXIS2`・`AXIS3`）
    pub direction: String,
    /// 層を負の向きに重ねるなら `true`（`NEGATIVE`）
    pub negative: bool,
    /// 基準線からのずれ（文書の長さの単位）
    pub offset: f64,
}

/// 材料の構成要素
#[derive(Clone, Debug, PartialEq)]
pub struct NewConstituent {
    /// 材料の名前
    pub material: String,
    /// 構成要素の名前
    pub name: Option<String>,
}

/// 層を重ねる向き（`IfcLayerSetDirectionEnum`）
const DIRECTIONS: &[&str] = &["AXIS1", "AXIS2", "AXIS3"];

impl Draft<'_> {
    /// 要素か型に材料の構成を結ぶ。結んであった材料は外す
    pub(in crate::edit) fn set_material(
        &mut self,
        target: Target,
        material: &NewMaterial,
        cx: &mut Cx,
    ) -> Result<bool, Error> {
        const EDIT: &str = "material";
        let Some((id, stmt, _)) = self.object(
            target,
            &[Category::Element, Category::Type, Category::Style],
            EDIT,
            cx,
        ) else {
            return Ok(false);
        };
        let Some(version) = self.version() else {
            cx.report(unsupported(EDIT, &stmt.name));
            return Ok(false);
        };
        if let Some(value) = rejected(material, version) {
            cx.report(unsupported(EDIT, &stmt.name).with("value", value));
            return Ok(false);
        }
        let history = stmt.arg(2).cloned().unwrap_or_else(null);
        let relating = match material {
            NewMaterial::Single(name) => self.named_material(version, name, cx)?,
            NewMaterial::List(names) => {
                let mut items = Vec::with_capacity(names.len());
                for name in names {
                    items.push(Item::Ref(self.named_material(version, name, cx)?));
                }
                self.add(Stmt {
                    name: b"IFCMATERIALLIST".to_vec(),
                    args: vec![Item::List(items)],
                })
            }
            NewMaterial::Layers { layers, usage } => {
                self.layer_set(version, layers, usage.as_ref(), cx)?
            }
            NewMaterial::Constituents(parts) => {
                let mut items = Vec::with_capacity(parts.len());
                for part in parts {
                    let material = self.named_material(version, &part.material, cx)?;
                    items.push(Item::Ref(self.add(Stmt {
                        name: b"IFCMATERIALCONSTITUENT".to_vec(),
                        args: vec![
                            part.name.as_deref().map_or_else(null, text),
                            null(),
                            Item::Ref(material),
                            null(),
                            null(),
                        ],
                    })));
                }
                self.add(Stmt {
                    name: b"IFCMATERIALCONSTITUENTSET".to_vec(),
                    args: vec![null(), null(), Item::List(items)],
                })
            }
        };
        self.detach(ASSOCIATES_MATERIAL, id);
        self.join(ASSOCIATES_MATERIAL, relating, id, history);
        Ok(true)
    }

    /// 層の集まりと、渡されれば層の使い方。返すのは結ぶ側
    fn layer_set(
        &mut self,
        version: Version,
        layers: &[NewMaterialLayer],
        usage: Option<&NewLayerUsage>,
        cx: &mut Cx,
    ) -> Result<u64, Error> {
        let later = version != Version::Ifc2x3;
        let mut written = Vec::with_capacity(layers.len());
        for layer in layers {
            let material = match &layer.material {
                Some(name) => Item::Ref(self.named_material(version, name, cx)?),
                None => null(),
            };
            let mut args = vec![material, real(layer.thickness).unwrap_or_else(null), null()];
            if later {
                args.extend([
                    layer.name.as_deref().map_or_else(null, text),
                    null(),
                    null(),
                    null(),
                ]);
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
        let Some(usage) = usage else {
            return Ok(set);
        };
        let direction = format!(".{}.", usage.direction.to_ascii_uppercase());
        let sense: &[u8] = if usage.negative {
            b".NEGATIVE."
        } else {
            b".POSITIVE."
        };
        let mut args = vec![
            Item::Ref(set),
            raw(direction.as_bytes()),
            raw(sense),
            real(usage.offset).unwrap_or_else(null),
        ];
        if later {
            args.push(null());
        }
        Ok(self.add(Stmt {
            name: b"IFCMATERIALLAYERSETUSAGE".to_vec(),
            args,
        }))
    }

    /// 名前が `name` の `IfcMaterial`。文書か、この要求で足したものにあれば使い、なければ足す
    fn named_material(&mut self, version: Version, name: &str, cx: &mut Cx) -> Result<u64, Error> {
        if self.materials.is_none() {
            let mut found = BTreeMap::new();
            for id in self.file.ids() {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| stopped(cx, stop, None))?;
                if self.removed.contains(&id) || self.kind(id) != b"IFCMATERIAL" {
                    continue;
                }
                if let Some(name) = self
                    .stmt(id)
                    .and_then(|stmt| stmt.arg(1).and_then(|item| self.text_of(item)))
                {
                    found.entry(name).or_insert(id);
                }
            }
            self.materials = Some(found);
        }
        if let Some(&id) = self.materials.as_ref().and_then(|m| m.get(name))
            && !self.removed.contains(&id)
        {
            return Ok(id);
        }
        let mut args = vec![text(name)];
        if version != Version::Ifc2x3 {
            args.extend([null(), null()]);
        }
        let id = self.add(Stmt {
            name: b"IFCMATERIAL".to_vec(),
            args,
        });
        if let Some(materials) = self.materials.as_mut() {
            materials.insert(name.to_owned(), id);
        }
        Ok(id)
    }
}

/// そのバージョンに書けない構成なら、診断に書く理由
pub(in crate::edit) fn rejected(material: &NewMaterial, version: Version) -> Option<&'static str> {
    let later = version != Version::Ifc2x3;
    match material {
        NewMaterial::Single(_) => None,
        NewMaterial::List(_) if later => Some("deprecated"),
        NewMaterial::List(names) => names.is_empty().then_some("empty"),
        NewMaterial::Layers { layers, usage } => {
            if layers.is_empty() {
                return Some("empty");
            }
            for layer in layers {
                let thickness = layer.thickness;
                if !thickness.is_finite() || thickness < 0.0 || (!later && thickness <= 0.0) {
                    return Some("thickness");
                }
                if !later && layer.name.is_some() {
                    return Some("name");
                }
            }
            match usage {
                Some(usage)
                    if !DIRECTIONS
                        .iter()
                        .any(|d| d.eq_ignore_ascii_case(&usage.direction)) =>
                {
                    Some("direction")
                }
                Some(usage) if !usage.offset.is_finite() => Some("offset"),
                _ => None,
            }
        }
        NewMaterial::Constituents(_) if !later => Some("constituents"),
        NewMaterial::Constituents(parts) => parts.is_empty().then_some("empty"),
    }
}
