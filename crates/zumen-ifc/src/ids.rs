//! IDS の照合に、建物モデルにない IFC の属性を、渡した EXPRESS の名前で L1 から引いて渡す（決定 0217）。

use zumen_building::{Building, ElementRef, Literal, Measured};
use zumen_ids::{AttributeValue, Lookup, Missing};

use crate::building::{MEASURES, decode};
use crate::express::{Base, Explicit};
use crate::{Express, IfcFile, Value};

/// IFC から作った建物モデルを IDS で照合するときの、[`Lookup`]。
///
/// IFC からの変換が建物モデルに写さない値（分類、材料の分類と組の名前、単一の値でないプロパティ。決定 0215）を
/// 持たないと示す。EXPRESS を渡せば、要素のインスタンスのすべての明示の属性を、EXPRESS の属性の名前で返す。
#[derive(Clone, Copy, Debug)]
pub struct IdsLookup<'a> {
    file: &'a IfcFile,
    schema: Option<&'a Express>,
}

impl<'a> IdsLookup<'a> {
    /// 建物モデルを作った IFC と、属性の名前を引く EXPRESS
    pub fn new(file: &'a IfcFile, schema: Option<&'a Express>) -> Self {
        Self { file, schema }
    }

    fn value(&self, building: &Building, slot: &Explicit, value: &Value) -> AttributeValue {
        self.typed(building, slot.type_name.as_deref(), slot.base, value)
    }

    fn typed(
        &self,
        building: &Building,
        type_name: Option<&str>,
        base: Base,
        value: &Value,
    ) -> AttributeValue {
        let bytes = |range: &core::ops::Range<usize>| self.file.bytes(range.clone());
        match value {
            Value::Null | Value::Derived => AttributeValue::Null,
            Value::Ref(_) => AttributeValue::Reference,
            Value::Enum(range) => {
                let name = String::from_utf8_lossy(bytes(range)).to_ascii_uppercase();
                let boolean = base == Base::Boolean || type_name == Some("IFCBOOLEAN");
                let logical = base == Base::Logical || type_name == Some("IFCLOGICAL");
                AttributeValue::Literal(match name.as_str() {
                    "T" if boolean => Literal::Boolean(true),
                    "F" if boolean => Literal::Boolean(false),
                    "T" if logical => Literal::Logical(Some(true)),
                    "F" if logical => Literal::Logical(Some(false)),
                    "U" if logical => Literal::Logical(None),
                    _ => Literal::Enumeration(name),
                })
            }
            Value::Text { raw, .. } => AttributeValue::Literal(Literal::Text(decode(bytes(raw)).0)),
            Value::Number(range) => {
                let Ok(text) = core::str::from_utf8(bytes(range)) else {
                    return AttributeValue::Reference;
                };
                let integer = match base {
                    Base::Integer => true,
                    Base::Real => false,
                    _ => !text.contains(['.', 'E', 'e']),
                };
                if integer && let Ok(i) = text.parse::<i64>() {
                    return AttributeValue::Literal(Literal::Integer(i));
                }
                let Ok(x) = text.parse::<f64>() else {
                    return AttributeValue::Reference;
                };
                let unit_kind = type_name.and_then(|name| {
                    MEASURES
                        .iter()
                        .find(|(measure, _)| measure.eq_ignore_ascii_case(name))
                        .and_then(|(_, unit)| *unit)
                });
                match unit_kind {
                    Some(kind) => AttributeValue::Measured(Measured {
                        value: x,
                        unit: building
                            .document
                            .units
                            .iter()
                            .find(|unit| unit.kind == kind)
                            .cloned(),
                    }),
                    None => AttributeValue::Literal(Literal::Real(x)),
                }
            }
            Value::Typed(part) => {
                let name =
                    String::from_utf8_lossy(self.file.bytes(part.name())).to_ascii_uppercase();
                match part.args() {
                    [inner] => self.typed(building, Some(&name), Base::Other, inner),
                    _ => AttributeValue::Reference,
                }
            }
            Value::List(items) => {
                let mut literals = Vec::with_capacity(items.len());
                for item in items {
                    match self.typed(building, type_name, base, item) {
                        AttributeValue::Literal(literal) => literals.push(literal),
                        AttributeValue::Measured(measured) => {
                            literals.push(Literal::Real(measured.value))
                        }
                        _ => return AttributeValue::Reference,
                    }
                }
                AttributeValue::Literal(Literal::List(literals))
            }
            _ => AttributeValue::Reference,
        }
    }
}

impl Lookup for IdsLookup<'_> {
    fn missing(&self, what: Missing) -> bool {
        matches!(
            what,
            Missing::Classification | Missing::MaterialNames | Missing::PropertyKinds
        )
    }

    fn attributes(
        &self,
        building: &Building,
        element: ElementRef,
    ) -> Option<Vec<(String, AttributeValue)>> {
        let schema = self.schema?;
        let instance = building.element(element)?.source().instance?;
        let record = self.file.instance(instance)?;
        // 複合のインスタンスは、属性の名前が部分ごとに分かれる。まだ引かない
        let [part] = record.parts() else {
            return None;
        };
        let kind = String::from_utf8_lossy(self.file.bytes(part.name())).to_ascii_uppercase();
        let slots = schema.explicit(&kind)?;
        if slots.len() != part.args().len() {
            return None;
        }
        Some(
            slots
                .iter()
                .zip(part.args())
                .filter(|(slot, _)| !slot.derived)
                .map(|(slot, value)| (slot.name.clone(), self.value(building, slot, value)))
                .collect(),
        )
    }
}
