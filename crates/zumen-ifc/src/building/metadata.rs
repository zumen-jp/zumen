//! 建具の既定のプロパティ。各版の自然文の属性の節から読む。
//! IFC4 ADD2 TC1 6.1.3 の Door/Window Lining/PanelProperties の Attributes と Change log。
//! IFC2X3 の属性の順は同名の自然文の説明と合成入力の外部照合で確認する。

use super::*;

/// 順序は属性の位置。空の項目は形の参照で、この属性の成分には含めない。
const DOOR_LINING: &[(&str, &str)] = &[
    ("LiningDepth", "IfcPositiveLengthMeasure"),
    ("LiningThickness", "IfcNonNegativeLengthMeasure"),
    ("ThresholdDepth", "IfcPositiveLengthMeasure"),
    ("ThresholdThickness", "IfcNonNegativeLengthMeasure"),
    ("TransomThickness", "IfcNonNegativeLengthMeasure"),
    ("TransomOffset", "IfcLengthMeasure"),
    ("LiningOffset", "IfcLengthMeasure"),
    ("ThresholdOffset", "IfcLengthMeasure"),
    ("CasingThickness", "IfcPositiveLengthMeasure"),
    ("CasingDepth", "IfcPositiveLengthMeasure"),
    ("", ""),
    ("LiningToPanelOffsetX", "IfcLengthMeasure"),
    ("LiningToPanelOffsetY", "IfcLengthMeasure"),
];
const DOOR_PANEL: &[(&str, &str)] = &[
    ("PanelDepth", "IfcPositiveLengthMeasure"),
    ("PanelOperation", "IfcDoorPanelOperationEnum"),
    ("PanelWidth", "IfcNormalisedRatioMeasure"),
    ("PanelPosition", "IfcDoorPanelPositionEnum"),
];
const WINDOW_LINING: &[(&str, &str)] = &[
    ("LiningDepth", "IfcPositiveLengthMeasure"),
    ("LiningThickness", "IfcNonNegativeLengthMeasure"),
    ("TransomThickness", "IfcNonNegativeLengthMeasure"),
    ("MullionThickness", "IfcNonNegativeLengthMeasure"),
    ("FirstTransomOffset", "IfcNormalisedRatioMeasure"),
    ("SecondTransomOffset", "IfcNormalisedRatioMeasure"),
    ("FirstMullionOffset", "IfcNormalisedRatioMeasure"),
    ("SecondMullionOffset", "IfcNormalisedRatioMeasure"),
    ("", ""),
    ("LiningOffset", "IfcLengthMeasure"),
    ("LiningToPanelOffsetX", "IfcLengthMeasure"),
    ("LiningToPanelOffsetY", "IfcLengthMeasure"),
];
const WINDOW_PANEL: &[(&str, &str)] = &[
    ("OperationType", "IfcWindowPanelOperationEnum"),
    ("PanelPosition", "IfcWindowPanelPositionEnum"),
    ("FrameDepth", "IfcPositiveLengthMeasure"),
    ("FrameThickness", "IfcPositiveLengthMeasure"),
];

fn fields(kind: &[u8]) -> Option<&'static [(&'static str, &'static str)]> {
    [
        (b"IFCDOORLININGPROPERTIES".as_slice(), DOOR_LINING),
        (b"IFCDOORPANELPROPERTIES".as_slice(), DOOR_PANEL),
        (b"IFCWINDOWLININGPROPERTIES".as_slice(), WINDOW_LINING),
        (b"IFCWINDOWPANELPROPERTIES".as_slice(), WINDOW_PANEL),
    ]
    .into_iter()
    .find_map(|(name, fields)| name.eq_ignore_ascii_case(kind).then_some(fields))
}

/// 従来変換と Live が、同じ版の属性位置・名前・型を使う。
pub(crate) fn predefined_property_fields(
    kind: &[u8],
    version: Version,
) -> Option<impl Iterator<Item = (usize, &'static str, &'static str)>> {
    let fields = fields(kind)?;
    let old = version == Version::Ifc2x3;
    let last = if old {
        if kind.eq_ignore_ascii_case(b"IFCDOORLININGPROPERTIES") {
            15
        } else if kind.eq_ignore_ascii_case(b"IFCWINDOWLININGPROPERTIES") {
            13
        } else {
            9
        }
    } else {
        usize::MAX
    };
    Some(
        fields
            .iter()
            .enumerate()
            .filter_map(move |(index, &(name, kind))| {
                let at = index + 5;
                if at > last || name.is_empty() {
                    return None;
                }
                let kind = if old && kind == "IfcNonNegativeLengthMeasure" {
                    "IfcPositiveLengthMeasure"
                } else {
                    kind
                };
                Some((at, name, kind))
            }),
    )
}

impl Lifter<'_> {
    /// IfcPropertyListValue.ListValues と IfcPropertyEnumeratedValue.EnumerationValues。
    /// 値の型を共通の TypedValue に、順序と各値を Literal::List に保つ。
    pub(super) fn property_values(
        &mut self,
        value: &Value,
        record: &Record,
        cx: &mut Cx,
    ) -> Option<TypedValue> {
        let Value::List(values) = value else {
            return None;
        };
        if values.is_empty() {
            return None;
        }
        if !self.copied(cx, (values.len() as u64).saturating_mul(ITEM * 4)) {
            return None;
        }
        let mut result = Vec::with_capacity(values.len());
        let mut type_name = None;
        for (index, value) in values.iter().enumerate() {
            if !self.charge(cx, Kind::Work, 1) {
                return None;
            }
            let value = self.typed_value(value, record, cx)?;
            if matches!(value.value, Literal::Real(v) if !v.is_finite()) {
                return None;
            }
            if index == 0 {
                type_name = value.type_name
            } else if type_name != value.type_name {
                return None;
            }
            result.push(value.value);
        }
        Some(TypedValue {
            type_name,
            value: Literal::List(result),
        })
    }

    /// IfcPropertyEnumeratedValue の 4 EnumerationReference → IfcPropertyEnumeration の 3 Unit。
    /// ここでは平らな値の単位だけを読む。選択肢の定義は property-details に保持して取得済みと数える。
    pub(super) fn enumeration_unit(
        &mut self,
        record: &Record,
        cx: &mut Cx,
    ) -> Result<Option<u64>, ()> {
        match arg(record, 4) {
            Some(Value::Null) | None => Ok(None),
            value => {
                if !self.charge(cx, Kind::Work, 1) {
                    return Err(());
                }
                let Some(id) =
                    reference(value).filter(|&id| self.is(id, b"IFCPROPERTYENUMERATION"))
                else {
                    self.invalid(record, 4, cx);
                    return Err(());
                };
                let enumeration = self.file.instance(id).ok_or(())?;
                match arg(enumeration, 3) {
                    Some(Value::Null) => Ok(None),
                    value => {
                        let unit = reference(value);
                        if unit.is_none() {
                            self.invalid(enumeration, 3, cx);
                            return Err(());
                        }
                        Ok(unit)
                    }
                }
            }
        }
    }

    /// IFC4/4.3 IfcMaterialProperties: 1 Name・3 Properties・4 Material。
    /// IFC2X3 IfcExtendedMaterialProperties: 1 Material・2 ExtendedProperties・4 Name。
    pub(super) fn material_properties(&mut self, id: u64, record: &Record, cx: &mut Cx) -> bool {
        let position = match self.version {
            Version::Ifc2x3 if self.is(id, b"IFCEXTENDEDMATERIALPROPERTIES") => 1,
            Version::Ifc4 | Version::Ifc4x3 if self.is(id, b"IFCMATERIALPROPERTIES") => 4,
            _ => return false,
        };
        let Some(material) =
            reference(arg(record, position)).filter(|&m| self.is(m, b"IFCMATERIAL"))
        else {
            self.invalid(record, position as u64, cx);
            return true;
        };
        let Some(element) = self.elements.get(&material).copied() else {
            return false;
        };
        self.definitions_of(element, &[id], cx);
        true
    }

    /// IFC4/4.3 IfcMaterialConstituentSet / IfcMaterialProfileSet の 3 の構成順。
    /// 個々の構成は 1 Name・3 Material。断面そのものは形を取得する側で扱う。
    pub(super) fn material_members(
        &mut self,
        set: &Record,
        profiles: bool,
        cx: &mut Cx,
    ) -> Vec<MaterialPart> {
        let mut parts = Vec::new();
        if !profiles && matches!(arg(set, 3), Some(Value::Null)) {
            return parts;
        }
        let Some(Value::List(members)) = arg(set, 3) else {
            self.invalid(set, 3, cx);
            return parts;
        };
        if members.is_empty() {
            self.invalid(set, 3, cx);
            return parts;
        }
        let expected = if profiles {
            b"IFCMATERIALPROFILE".as_slice()
        } else {
            b"IFCMATERIALCONSTITUENT".as_slice()
        };
        let mut seen = BTreeSet::new();
        for member in members {
            if !self.charge(cx, Kind::Work, 1) {
                break;
            }
            let Some(member) = reference(Some(member)).filter(|&id| self.is(id, expected)) else {
                self.invalid(set, 3, cx);
                continue;
            };
            if !profiles {
                if !self.copied(cx, 64) {
                    break;
                }
                if !seen.insert(member) {
                    self.invalid(set, 3, cx);
                    continue;
                }
            }
            let Some(record) = self.take(member) else {
                continue;
            };
            let material = match arg(record, 3) {
                Some(Value::Null) if profiles => None,
                value => {
                    let target = reference(value)
                        .filter(|&id| self.is(id, b"IFCMATERIAL"))
                        .and_then(|id| self.elements.get(&id).copied());
                    if target.is_none() {
                        self.invalid(record, 3, cx);
                    }
                    target
                }
            };
            if !self.copied(cx, ITEM * 4) {
                break;
            }
            let name = self.text(arg(record, 1), record, cx);
            if self.is_halted() {
                break;
            }
            parts.push(MaterialPart {
                material,
                name,
                thickness: None,
            });
        }
        parts
    }

    pub(super) fn predefined_property_set(&self, id: u64) -> bool {
        self.file
            .instance(id)
            .is_some_and(|r| fields(self.file.kind(r)).is_some())
    }

    /// 形の参照は使ったと数えず、形の種類と場所を既存の欠落の診断に残す。
    pub(super) fn predefined_properties(&mut self, record: &Record, cx: &mut Cx) -> Vec<Property> {
        let Some(fields) = predefined_property_fields(self.file.kind(record), self.version) else {
            return Vec::new();
        };
        let mut properties = Vec::new();
        for (at, name, kind) in fields {
            if !self.charge(cx, Kind::Work, 1) {
                break;
            }
            let Some(value) = arg(record, at).filter(|v| !matches!(v, Value::Null)) else {
                continue;
            };
            let bytes = match value {
                Value::Enum(raw) => raw.len() as u64,
                _ => 0,
            };
            if !self.copied(cx, text_size(name) + text_size(kind) + ITEM * 4 + bytes) {
                break;
            }
            let value = if kind.ends_with("Enum") {
                self.enumeration(Some(value)).map(Literal::Enumeration)
            } else {
                self.real(Some(value))
                    .filter(|v| v.is_finite())
                    .map(Literal::Real)
            };
            let Some(value) = value else {
                self.invalid(record, at as u64, cx);
                continue;
            };
            let unit = if kind.ends_with("LengthMeasure") {
                self.document_unit("LENGTHUNIT", kind, record, cx)
            } else {
                None
            };
            if self.is_halted() {
                break;
            }
            properties.push(Property {
                name: name.into(),
                value: Some(TypedValue {
                    type_name: Some(kind.into()),
                    value,
                }),
                unit,
            });
        }
        properties
    }
}
