//! IDS の照合に、建物モデルにない IFC の属性を、渡した EXPRESS の名前で L1 から引いて渡す（決定 0217）。

use core::cell::RefCell;

use zumen_building::{Building, ElementRef, Literal, Measured};
use zumen_ids::{AttributeValue, Lookup, Missing};
use zumen_sdk::Stop;

use crate::building::{MEASURES, decode};
use crate::express::{Base, Layouts, Slot};
use crate::{Express, IfcFile, Value};

/// IFC から作った建物モデルを IDS で照合するときの、[`Lookup`]。
///
/// IFC からの変換が建物モデルに写さない値（分類、材料の分類と組の名前、単一の値でないプロパティ。決定 0215）を
/// 持たないと示す。EXPRESS を渡せば、要素のインスタンスのすべての明示の属性を、EXPRESS の属性の名前で返す。
///
/// 実体の属性の並びは、初めて使うときに作って覚え、要素ごとに作り直さない。覚えるのは、属性の数の合う要素があった
/// 実体の並びだけにする（#672）。
#[derive(Clone, Debug)]
pub struct IdsLookup<'a> {
    file: &'a IfcFile,
    /// EXPRESS の実体ごとの属性の数と並び。照合は `&self` で呼ばれるので、内側で覚える
    layouts: Option<RefCell<Layouts<'a>>>,
}

impl<'a> IdsLookup<'a> {
    /// 建物モデルを作った IFC と、属性の名前を引く EXPRESS
    pub fn new(file: &'a IfcFile, schema: Option<&'a Express>) -> Self {
        Self {
            file,
            layouts: schema.map(|schema| RefCell::new(Layouts::new(schema))),
        }
    }

    fn value(
        &self,
        building: &Building,
        schema: &Express,
        slot: &Slot,
        value: &Value,
    ) -> AttributeValue {
        self.typed(building, schema.type_name(slot), schema.base(slot), value)
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
        let layouts = self.layouts.as_ref()?;
        let instance = building.element(element)?.source().instance?;
        let record = self.file.instance(instance)?;
        // 複合のインスタンスは、属性の名前が部分ごとに分かれる。まだ引かない
        let [part] = record.parts() else {
            return None;
        };
        let kind = String::from_utf8_lossy(self.file.bytes(part.name())).to_ascii_uppercase();
        // 照合は文脈を受けないので、並びを作る手順は work で数えない。作るのは実体ごとに 1 度で、実体は、建物モデルが
        // 要素にする IFC の実体（決まった数）に限られる
        let mut free = |_: u64| -> Result<(), Stop> { Ok(()) };
        let (schema, slots) = {
            let mut layouts = layouts.try_borrow_mut().ok()?;
            let schema = layouts.schema();
            let entity = schema.entity_named(&kind)?;
            if layouts.count(entity, &mut free).ok()? != part.args().len() {
                return None;
            }
            (schema, layouts.slots(entity, &mut free).ok()?)
        };
        Some(
            slots
                .iter()
                .zip(part.args())
                .filter(|(slot, _)| !slot.is_derived())
                .map(|(slot, value)| {
                    (
                        slot.written().to_owned(),
                        self.value(building, schema, slot, value),
                    )
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use zumen_base::{Budget, CancelToken, Diagnostics, Limits};
    use zumen_sdk::{Codec, Cx, Lift};

    use super::*;
    use crate::Ifc;

    /// 要素の属性の並びは、実体ごとに 1 度だけ作って覚え、要素ごとに作り直さない。属性の数の合わない実体は、数だけを
    /// 覚えて並びを覚えない（#672）
    #[test]
    fn remembers_the_slots_once_per_entity() {
        let express = "SCHEMA IFC4;\nTYPE IfcLabel = STRING; END_TYPE;\nTYPE IfcReal = REAL; END_TYPE;\n\
            ENTITY IfcRoot ABSTRACT;\n  GlobalId : IfcLabel;\n  OwnerHistory : OPTIONAL IfcRoot;\n  \
            Name : OPTIONAL IfcLabel;\n  Description : OPTIONAL IfcLabel;\nEND_ENTITY;\n\
            ENTITY IfcProject SUBTYPE OF (IfcRoot);\n  A, B, C, D, E : OPTIONAL IfcLabel;\nEND_ENTITY;\n\
            ENTITY IfcDoor SUBTYPE OF (IfcRoot);\n  F, G, H, Tag : OPTIONAL IfcLabel;\n  \
            OverallHeight : OPTIONAL IfcReal;\n  I, J, K, L : OPTIONAL IfcLabel;\nEND_ENTITY;\n\
            ENTITY IfcWall SUBTYPE OF (IfcRoot);\n  ObjectType : OPTIONAL IfcLabel;\nEND_ENTITY;\n\
            END_SCHEMA;\n";
        let ifc = "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\n\
            FILE_NAME('','',(''),(''),'','','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n\
            #1=IFCPROJECT('0000000000000000000001',$,'Project',$,$,$,$,$,#10);\n\
            #10=IFCUNITASSIGNMENT((#11));\n#11=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n\
            #80=IFCDOOR('0000000000000000000080',$,'Door',$,$,$,$,'D-1',2100.,$,.DOOR.,$,$);\n\
            #81=IFCDOOR('0000000000000000000081',$,'Door',$,$,$,$,'D-2',2000.,$,.DOOR.,$,$);\n\
            #90=IFCWALL('0000000000000000000090',$,'Wall',$,$,$,$,$,$);\nENDSEC;\nEND-ISO-10303-21;\n";
        let mut budget = Budget::new(Limits::default());
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let schema = Express::read(express.as_bytes(), &mut cx).unwrap();
        let file = Ifc::read(ifc.as_bytes(), &mut cx).unwrap();
        let building: Building = file.lift(&mut cx).unwrap();
        let lookup = IdsLookup::new(&file, Some(&schema));
        let mut tags = Vec::new();
        for _ in 0..2 {
            tags.clear();
            for index in 0..building.elements().len() {
                let attributes = lookup.attributes(&building, ElementRef(index as u32));
                let tag = attributes.and_then(|list| {
                    list.into_iter()
                        .find(|(name, _)| name == "Tag")
                        .map(|(_, value)| value)
                });
                tags.push(tag);
            }
            // 属性の数は IfcRoot・IfcProject・IfcDoor・IfcWall、並びは数の合う IfcProject と IfcDoor だけ
            let remembered = lookup.layouts.as_ref().unwrap().borrow().remembered();
            assert_eq!(remembered, (4, 2));
        }
        let text = |tag: &str| AttributeValue::Literal(Literal::Text(tag.to_owned()));
        assert!(tags.contains(&Some(text("D-1"))));
        assert!(tags.contains(&Some(text("D-2"))));
    }
}
