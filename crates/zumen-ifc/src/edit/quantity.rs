//! 数量（`IfcElementQuantity`）と SI の単位（`IfcSIUnit`）を書く変更（#395）。
//!
//! 数量の実体は、IFC2X3 では 1 Name・2 Description・3 Unit・4 値の 4 属性、IFC4 と IFC4X3 では 5 Formula を足した
//! 5 属性（IFC2X3 TC1 と IFC4 ADD2 TC1・IFC4X3 ADD2 の数量の資源の各実体）。値は負にできず、IFC4X3 の
//! `IfcCountMeasure` は整数なので、個数は整数で書く。単位は書かず（`$`）、文書の単位の割り当てに従う。
//! `IfcElementQuantity` は 1 GlobalId・2 OwnerHistory・3 Name・4 Description・5 MethodOfMeasurement・6 Quantities。

use zumen_base::Kind;
use zumen_building::QuantityKind;
use zumen_sdk::{Cx, Error};

use super::author::{null, raw};
use super::{Draft, Item, Stmt, refs_of, swap, text, unsupported};
use crate::building::{Category, Version};
use crate::stopped;

/// 書く数量
#[derive(Clone, Debug, PartialEq)]
pub struct NewQuantity {
    /// 名前
    pub name: String,
    /// 種類
    pub kind: QuantityKind,
    /// 値（文書の単位）。有限で負でないもの。IFC4X3 の個数は整数
    pub value: f64,
    /// 式。IFC2X3 は持たないので、渡せば断る
    pub formula: Option<String>,
}

/// 足す SI の単位
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewUnit {
    /// 単位の種類（`AREAUNIT` など）。長さと平面角はひな形が持つので受けない
    pub kind: String,
    /// SI の接頭辞（`MILLI` など）
    pub prefix: Option<String>,
    /// SI の単位の名前（`SQUARE_METRE` など）。種類に合うもの
    pub name: String,
}

/// 数量のセットの実体
const QUANTITY_SET: &[u8] = b"IFCELEMENTQUANTITY";

/// 足せる単位の種類と、その次元に合う SI の単位の名前（`IfcUnitEnum` と `IfcSIUnitName`）。`IfcNamedUnit` の規則
/// は、単位の次元が種類に合うことを求める
const SI_UNITS: &[(&str, &[&str])] = &[
    ("AREAUNIT", &["SQUARE_METRE"]),
    ("VOLUMEUNIT", &["CUBIC_METRE"]),
    ("MASSUNIT", &["GRAM"]),
    ("TIMEUNIT", &["SECOND"]),
    (
        "THERMODYNAMICTEMPERATUREUNIT",
        &["KELVIN", "DEGREE_CELSIUS"],
    ),
    ("ELECTRICCURRENTUNIT", &["AMPERE"]),
    ("AMOUNTOFSUBSTANCEUNIT", &["MOLE"]),
    ("LUMINOUSINTENSITYUNIT", &["CANDELA"]),
    ("SOLIDANGLEUNIT", &["STERADIAN"]),
    ("FREQUENCYUNIT", &["HERTZ"]),
    ("FORCEUNIT", &["NEWTON"]),
    ("PRESSUREUNIT", &["PASCAL"]),
    ("ENERGYUNIT", &["JOULE"]),
    ("POWERUNIT", &["WATT"]),
    ("ELECTRICCHARGEUNIT", &["COULOMB"]),
    ("ELECTRICVOLTAGEUNIT", &["VOLT"]),
    ("ELECTRICCAPACITANCEUNIT", &["FARAD"]),
    ("ELECTRICRESISTANCEUNIT", &["OHM"]),
    ("ELECTRICCONDUCTANCEUNIT", &["SIEMENS"]),
    ("MAGNETICFLUXUNIT", &["WEBER"]),
    ("MAGNETICFLUXDENSITYUNIT", &["TESLA"]),
    ("INDUCTANCEUNIT", &["HENRY"]),
    ("LUMINOUSFLUXUNIT", &["LUMEN"]),
    ("ILLUMINANCEUNIT", &["LUX"]),
    ("RADIOACTIVITYUNIT", &["BECQUEREL"]),
    ("ABSORBEDDOSEUNIT", &["GRAY"]),
    ("DOSEEQUIVALENTUNIT", &["SIEVERT"]),
];

/// SI の接頭辞（`IfcSIPrefix`）
const PREFIXES: &[&str] = &[
    "EXA", "PETA", "TERA", "GIGA", "MEGA", "KILO", "HECTO", "DECA", "DECI", "CENTI", "MILLI",
    "MICRO", "NANO", "PICO", "FEMTO", "ATTO",
];

/// 足せる SI の単位か。種類・名前・接頭辞を大文字で返す
pub(super) fn si_unit(unit: &NewUnit) -> Option<(String, Option<String>, String)> {
    let kind = unit.kind.to_ascii_uppercase();
    let name = unit.name.to_ascii_uppercase();
    let (_, names) = SI_UNITS.iter().find(|(k, _)| *k == kind)?;
    if !names.contains(&name.as_str()) {
        return None;
    }
    let prefix = match &unit.prefix {
        Some(prefix) => {
            let prefix = prefix.to_ascii_uppercase();
            PREFIXES
                .contains(&prefix.as_str())
                .then_some(Some(prefix))?
        }
        None => None,
    };
    Some((kind, prefix, name))
}

/// 数量の実体の名前
fn entity(kind: QuantityKind) -> &'static [u8] {
    match kind {
        QuantityKind::Length => b"IFCQUANTITYLENGTH",
        QuantityKind::Area => b"IFCQUANTITYAREA",
        QuantityKind::Volume => b"IFCQUANTITYVOLUME",
        QuantityKind::Count => b"IFCQUANTITYCOUNT",
        QuantityKind::Weight => b"IFCQUANTITYWEIGHT",
        QuantityKind::Time => b"IFCQUANTITYTIME",
    }
}

/// 書く値。有限で負でないもの。個数は整数なら整数で書き、IFC4X3 では整数でなければ書かない
pub(super) fn quantity_value(kind: QuantityKind, value: f64, version: Version) -> Option<Vec<u8>> {
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    // -0 も 0 として書く
    let value = if value == 0.0 { 0.0 } else { value };
    const EXACT: f64 = 9_007_199_254_740_992.0;
    if kind == QuantityKind::Count && value.fract() == 0.0 && value <= EXACT {
        return Some(format!("{value:.0}").into_bytes());
    }
    if kind == QuantityKind::Count && version == Version::Ifc4x3 {
        return None;
    }
    let mut written = format!("{value}");
    if !written.contains('.') {
        written.push('.');
    }
    Some(written.into_bytes())
}

impl Draft<'_> {
    /// 数量のセット `set` の数量を書く。セットがなければ足し、共有されていれば写しを作る
    pub(super) fn quantity(
        &mut self,
        element: u64,
        set: &str,
        method: Option<&str>,
        quantity: &NewQuantity,
        cx: &mut Cx,
    ) -> Result<bool, Error> {
        let Some((owner, category)) = self.root(element, "quantity", cx) else {
            return Ok(false);
        };
        let kind = entity(quantity.kind);
        let Some(version) = self.version() else {
            cx.report(unsupported("quantity", kind));
            return Ok(false);
        };
        let Some(value) = quantity_value(quantity.kind, quantity.value, version) else {
            cx.report(unsupported("quantity", kind).with("value", "value"));
            return Ok(false);
        };
        if quantity.formula.is_some() && version == Version::Ifc2x3 {
            cx.report(unsupported("quantity", kind).with("value", "formula"));
            return Ok(false);
        }
        let typed_object = matches!(category, Category::Type | Category::Style);
        let found = if typed_object {
            self.type_set(element, &owner, QUANTITY_SET, set)
        } else {
            self.occurrence_set(element, QUANTITY_SET, set, cx)?
        };
        let target = match found {
            Some((holder, qset)) => self.own_set(element, &owner, holder, qset, typed_object),
            None => {
                let history = owner.arg(2).cloned().unwrap_or_else(null);
                let stmt = Stmt {
                    name: QUANTITY_SET.to_vec(),
                    args: vec![
                        self.keyed_global_id(QUANTITY_SET, &[element], set),
                        history,
                        text(set),
                        null(),
                        method.map_or_else(null, text),
                        Item::List(Vec::new()),
                    ],
                };
                self.new_set(element, &owner, stmt, typed_object)
            }
        };
        let mut args = vec![text(&quantity.name), null(), null(), raw(&value)];
        if version != Version::Ifc2x3 {
            args.push(quantity.formula.as_deref().map_or_else(null, text));
        }
        self.set_quantity(target, kind, args, cx)
    }

    /// セット `qset` の、名前が同じ数量を書き直す。なければ足す。ほかのセットと共有する数量は写しを作る
    fn set_quantity(
        &mut self,
        qset: u64,
        kind: &[u8],
        args: Vec<Item>,
        cx: &mut Cx,
    ) -> Result<bool, Error> {
        if !self.changed.contains_key(&qset) && self.stmt(qset).is_none() {
            cx.report(unsupported("quantity", kind));
            return Ok(false);
        }
        let name = match args.first() {
            Some(item) => self.text_of(item),
            None => None,
        };
        let mut found = None;
        for existing in self.arg_refs(qset, 6) {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, None))?;
            let same = self
                .arg_text(existing, 1)
                .is_some_and(|text| Some(&text) == name.as_ref());
            if same {
                found = Some(existing);
                break;
            }
        }
        let stmt = Stmt {
            name: kind.to_vec(),
            args,
        };
        match found {
            Some(existing) => {
                if self.kind(existing) != kind {
                    let other = self.kind(existing);
                    cx.report(unsupported("quantity", &other));
                    return Ok(false);
                }
                if self.referrers(existing) == [qset] {
                    self.put(existing, stmt);
                } else {
                    let Some(mut set) = self.stmt(qset) else {
                        cx.report(unsupported("quantity", kind));
                        return Ok(false);
                    };
                    let copy = self.add(stmt);
                    let quantities = swap(set.arg(6), existing, copy);
                    set.set(6, quantities);
                    self.put(qset, set);
                }
            }
            None => {
                let added = self.add(stmt);
                self.append_ref(qset, 6, added);
            }
        }
        Ok(true)
    }

    /// プロジェクトの単位の割り当てに SI の単位を足す。同じ種類の単位があれば断る
    pub(super) fn add_unit(&mut self, unit: &NewUnit, cx: &mut Cx) -> Result<bool, Error> {
        let Some((kind, prefix, name)) = si_unit(unit) else {
            cx.report(unsupported("unit", b"IFCSIUNIT").with("value", "unit"));
            return Ok(false);
        };
        let assignment = self
            .project
            .and_then(|project| self.stmt(project))
            .and_then(|project| match project.arg(9) {
                Some(Item::Ref(id)) => Some(*id),
                _ => None,
            });
        let Some((assignment, mut stmt)) = assignment.and_then(|id| {
            Some((
                id,
                self.stmt(id).filter(|s| s.name == b"IFCUNITASSIGNMENT")?,
            ))
        }) else {
            cx.report(unsupported("unit", b"IFCSIUNIT").with("missing", "IfcUnitAssignment"));
            return Ok(false);
        };
        let wanted = format!(".{kind}.").into_bytes();
        let mut units = refs_of(stmt.arg(1));
        for &existing in &units {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, None))?;
            // 名前つきの単位（SI・換算・文脈による単位）は 2 UnitType に種類を持つ
            let same = self
                .stmt(existing)
                .is_some_and(|s| matches!(s.arg(2), Some(Item::Raw(bytes)) if bytes.eq_ignore_ascii_case(&wanted)));
            if same {
                cx.report(unsupported("unit", b"IFCSIUNIT").with("value", "kind"));
                return Ok(false);
            }
        }
        let prefix = prefix.map_or_else(null, |p| raw(format!(".{p}.").as_bytes()));
        let added = self.add(Stmt {
            name: b"IFCSIUNIT".to_vec(),
            args: vec![
                raw(b"*"),
                raw(&wanted),
                prefix,
                raw(format!(".{name}.").as_bytes()),
            ],
        });
        units.push(added);
        stmt.set(1, Item::List(units.into_iter().map(Item::Ref).collect()));
        self.put(assignment, stmt);
        Ok(true)
    }
}
