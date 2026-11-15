//! 上下限・複合・選択肢を、親が先の平らなプロパティの列に写す。
//! IfcPropertyBoundedValue / IfcComplexProperty / IfcPropertyEnumeration の自然文の属性の節。

use super::*;
use zumen_building::{DetailedProperty, DetailedPropertySet, PropertyChoices, PropertyDetailValue};

const NODE_BYTES: u64 = 1024;
const FRAME_BYTES: u64 = 32;

fn typed_size(value: &TypedValue) -> u64 {
    optional_text_size(value.type_name.as_deref()).saturating_add(literal_size(&value.value))
}
fn values_size(values: Option<&[TypedValue]>) -> u64 {
    values
        .unwrap_or_default()
        .iter()
        .map(typed_size)
        .fold(0, u64::saturating_add)
}
pub(super) fn details_size(set: &DetailedPropertySet) -> u64 {
    set.properties.iter().fold(
        text_size(&set.name) + optional_text_size(set.description.as_deref()),
        |size, p| {
            let value = match &p.value {
                PropertyDetailValue::Single { value, unit } => {
                    value.as_ref().map_or(0, typed_size) + unit.as_ref().map_or(0, unit_size)
                }
                PropertyDetailValue::List { values, unit } => {
                    values_size(values.as_deref()) + unit.as_ref().map_or(0, unit_size)
                }
                PropertyDetailValue::Enumerated { values, choices } => {
                    values_size(values.as_deref())
                        + choices.as_ref().map_or(0, |c| {
                            text_size(&c.name)
                                + values_size(Some(&c.values))
                                + c.unit.as_ref().map_or(0, unit_size)
                        })
                }
                PropertyDetailValue::Bounded {
                    lower,
                    upper,
                    set_point,
                    unit,
                } => {
                    [lower, upper, set_point]
                        .iter()
                        .map(|v| v.as_ref().map_or(0, typed_size))
                        .sum::<u64>()
                        + unit.as_ref().map_or(0, unit_size)
                }
                PropertyDetailValue::Complex { usage_name } => text_size(usage_name),
                PropertyDetailValue::Unsupported { kind } => text_size(kind),
            };
            size.saturating_add(NODE_BYTES)
                .saturating_add(text_size(&p.name))
                .saturating_add(optional_text_size(p.description.as_deref()))
                .saturating_add(value)
        },
    )
}

impl Lifter<'_> {
    pub(super) fn detailed_properties(
        &mut self,
        set: &Record,
        at: usize,
        name: &str,
        properties: &[Property],
        cx: &mut Cx,
    ) -> Option<DetailedPropertySet> {
        let Value::List(roots) = arg(set, at)? else {
            return None;
        };
        let mut needed = false;
        for root in roots {
            if !self.charge(cx, Kind::Work, 1) {
                return None;
            }
            if let Some(id) = reference(Some(root)) {
                needed |= self.is(id, b"IFCCOMPLEXPROPERTY")
                    || self.is(id, b"IFCPROPERTYBOUNDEDVALUE")
                    || (self.is(id, b"IFCPROPERTYENUMERATEDVALUE")
                        && self
                            .file
                            .instance(id)
                            .is_some_and(|r| !matches!(arg(r, 4), None | Some(Value::Null))));
            }
        }
        if !needed {
            return None;
        }
        // 平らな成分で読み終えた根の値を借りる。同じ不正値や参照をもう一度診断しない。
        let mut flat = BTreeMap::new();
        let mut values = properties.iter();
        for root in roots {
            if !self.charge(cx, Kind::Work, 1) || !self.copied(cx, 64) {
                return None;
            }
            if let Some(id) = reference(Some(root))
                && (self.is(id, b"IFCPROPERTYSINGLEVALUE")
                    || self.is(id, b"IFCPROPERTYLISTVALUE")
                    || self.is(id, b"IFCPROPERTYENUMERATEDVALUE"))
                && let Some(value) = values.next()
            {
                flat.insert(id, value);
            }
        }
        if !self.copied(
            cx,
            text_size(name) + (roots.len() as u64).saturating_mul(FRAME_BYTES),
        ) {
            return None;
        }
        let description_at = if self
            .file
            .kind(set)
            .eq_ignore_ascii_case(b"IFCMATERIALPROPERTIES")
        {
            2
        } else if self
            .file
            .kind(set)
            .eq_ignore_ascii_case(b"IFCEXTENDEDMATERIALPROPERTIES")
        {
            3
        } else {
            4
        };
        let mut result = DetailedPropertySet {
            name: name.into(),
            description: self.text(arg(set, description_at), set, cx),
            properties: Vec::new(),
            source_instance: set.id(),
        };
        // exit のフレームで深さを戻す。共有は各親へ写し、経路内だけで循環を検出する。
        let mut stack = Vec::with_capacity(roots.len());
        for value in roots.iter().rev() {
            if let Some(id) = reference(Some(value)) {
                stack.push((id, None, false));
            } else {
                self.invalid(set, at as u64, cx);
            }
        }
        let mut ancestors = Vec::new();
        while let Some((id, parent, exit)) = stack.pop() {
            if exit {
                ancestors.pop();
                cx.leave();
                continue;
            }
            if !self.charge(cx, Kind::Work, 1 + ancestors.len() as u64) {
                break;
            }
            let parent_record = parent
                .and_then(|p: u32| result.properties.get(p as usize))
                .and_then(|p| p.source_instance)
                .and_then(|id| self.file.instance(id))
                .unwrap_or(set);
            let parent_at = if parent.is_some() { 4 } else { at as u64 };
            let Some(record) = self.file.instance(id) else {
                self.invalid(parent_record, parent_at, cx);
                continue;
            };
            if ancestors.contains(&id) {
                self.invalid(parent_record, parent_at, cx);
                continue;
            }
            if !self.charge(cx, Kind::Depth, 1) {
                break;
            }
            if !self.copied(cx, NODE_BYTES + FRAME_BYTES) {
                cx.leave();
                break;
            }
            ancestors.push(id);
            let node_name = self.text(arg(record, 1), record, cx).unwrap_or_default();
            let description = self.text(arg(record, 2), record, cx);
            let value = self.detailed_value(id, record, flat.get(&id).copied(), cx);
            if self.is_halted() {
                break;
            }
            let Some(value) = value else {
                ancestors.pop();
                cx.leave();
                continue;
            };
            let complex = matches!(value, PropertyDetailValue::Complex { .. });
            if !matches!(value, PropertyDetailValue::Unsupported { .. }) {
                self.used.insert(id);
            }
            let Ok(index) = u32::try_from(result.properties.len()) else {
                self.invalid(record, 4, cx);
                ancestors.pop();
                cx.leave();
                break;
            };
            result.properties.push(DetailedProperty {
                parent,
                name: node_name,
                description,
                value,
                source_instance: Some(id),
            });
            if complex {
                let Some(Value::List(children)) = arg(record, 4) else {
                    self.invalid(record, 4, cx);
                    ancestors.pop();
                    cx.leave();
                    continue;
                };
                if children.is_empty() {
                    self.invalid(record, 4, cx);
                }
                if !self.charge(cx, Kind::Work, children.len() as u64)
                    || !self.copied(cx, (children.len() as u64 + 1).saturating_mul(FRAME_BYTES))
                {
                    break;
                }
                stack.reserve_exact(children.len() + 1);
                stack.push((id, parent, true));
                for child in children.iter().rev() {
                    if let Some(id) = reference(Some(child)) {
                        stack.push((id, Some(index), false));
                    } else {
                        self.invalid(record, 4, cx);
                    }
                }
            } else {
                ancestors.pop();
                cx.leave();
            }
        }
        for _ in ancestors {
            cx.leave();
        }
        (!self.is_halted()).then_some(result)
    }

    fn detailed_value(
        &mut self,
        id: u64,
        record: &Record,
        cached: Option<&Property>,
        cx: &mut Cx,
    ) -> Option<PropertyDetailValue> {
        if self.is(id, b"IFCCOMPLEXPROPERTY") {
            let usage_name = self.text(arg(record, 3), record, cx);
            if usage_name.is_none() {
                self.invalid(record, 3, cx);
            }
            return usage_name.map(|usage_name| PropertyDetailValue::Complex { usage_name });
        }
        if self.is(id, b"IFCPROPERTYBOUNDEDVALUE") {
            let upper = self.detailed_optional_value(record, 3, cx);
            let lower = self.detailed_optional_value(record, 4, cx);
            let set_point = if self.version == Version::Ifc2x3 {
                None
            } else {
                self.detailed_optional_value(record, 6, cx)
            };
            let first = upper.as_ref().or(lower.as_ref()).or(set_point.as_ref());
            let kind = first.and_then(|v| v.type_name.as_deref());
            for (at, value) in [(4, &lower), (6, &set_point)] {
                if value
                    .as_ref()
                    .is_some_and(|v| v.type_name.as_deref() != kind)
                {
                    self.invalid(record, at, cx);
                }
            }
            let unit = self.detailed_unit(record, 5, kind, true, cx);
            return Some(PropertyDetailValue::Bounded {
                lower,
                upper,
                set_point,
                unit,
            });
        }
        if self.is(id, b"IFCPROPERTYENUMERATEDVALUE") {
            let values = if let Some(p) = cached {
                if !self.copied(cx, property_size(p, true)) {
                    return None;
                }
                self.expand_values(p.value.clone(), cx)
            } else {
                self.detailed_list(record, 3, cx)
            };
            let choices = match arg(record, 4) {
                None | Some(Value::Null) => None,
                value => {
                    if let Some(id) =
                        reference(value).filter(|&id| self.is(id, b"IFCPROPERTYENUMERATION"))
                    {
                        self.detailed_choices(id, cached, cx)
                    } else {
                        if cached.is_none() {
                            self.invalid(record, 4, cx);
                        }
                        None
                    }
                }
            };
            return Some(PropertyDetailValue::Enumerated { values, choices });
        }
        if self.is(id, b"IFCPROPERTYLISTVALUE") {
            let value = self.detailed_flat_value(id, cached, cx)?;
            let values = self.expand_values(value.value, cx);
            return Some(PropertyDetailValue::List {
                values,
                unit: value.unit,
            });
        }
        if self.is(id, b"IFCPROPERTYSINGLEVALUE") {
            let value = self.detailed_flat_value(id, cached, cx)?;
            return Some(PropertyDetailValue::Single {
                value: value.value,
                unit: value.unit,
            });
        }
        let kind = self.file.kind(record);
        if !self.copied(cx, text_size(core::str::from_utf8(kind).ok()?)) {
            return None;
        }
        Some(PropertyDetailValue::Unsupported {
            kind: String::from_utf8_lossy(kind).into_owned(),
        })
    }

    fn detailed_flat_value(
        &mut self,
        id: u64,
        cached: Option<&Property>,
        cx: &mut Cx,
    ) -> Option<Property> {
        if let Some(value) = cached {
            self.copied(cx, property_size(value, true))
                .then(|| value.clone())
        } else {
            self.property(id, cx)
        }
    }

    fn detailed_optional_value(
        &mut self,
        record: &Record,
        at: usize,
        cx: &mut Cx,
    ) -> Option<TypedValue> {
        let raw = arg(record, at)?;
        if matches!(raw, Value::Null) {
            return None;
        }
        if !self.charge(cx, Kind::Work, 1) || !self.copied(cx, NODE_BYTES) {
            return None;
        }
        let value = self.typed_value(raw, record, cx);
        let value = value.filter(|v| !matches!(v.value,Literal::Real(f) if !f.is_finite()));
        if value.is_none() && !self.is_halted() {
            self.invalid(record, at as u64, cx);
        }
        value
    }
    fn detailed_list(
        &mut self,
        record: &Record,
        at: usize,
        cx: &mut Cx,
    ) -> Option<Vec<TypedValue>> {
        let raw = arg(record, at)?;
        if matches!(raw, Value::Null) {
            return None;
        }
        let value = self.property_values(raw, record, cx);
        if value.is_none() && !self.is_halted() {
            self.invalid(record, at as u64, cx);
        }
        self.expand_values(value, cx)
    }
    fn expand_values(&self, value: Option<TypedValue>, cx: &mut Cx) -> Option<Vec<TypedValue>> {
        let TypedValue {
            type_name,
            value: Literal::List(values),
        } = value?
        else {
            return None;
        };
        let bytes = (values.len() as u64)
            .saturating_mul(ITEM * 4 + optional_text_size(type_name.as_deref()));
        if !self.copied(cx, bytes) {
            return None;
        }
        Some(
            values
                .into_iter()
                .map(|value| TypedValue {
                    type_name: type_name.clone(),
                    value,
                })
                .collect(),
        )
    }
    fn detailed_choices(
        &mut self,
        id: u64,
        cached: Option<&Property>,
        cx: &mut Cx,
    ) -> Option<PropertyChoices> {
        let record = self.file.instance(id)?;
        if !self.charge(cx, Kind::Work, 1) || !self.copied(cx, NODE_BYTES) {
            return None;
        }
        let name = self.text(arg(record, 1), record, cx)?;
        let values = self.detailed_list(record, 2, cx)?;
        let kind = values.first().and_then(|v| v.type_name.as_deref());
        let unit = if let Some(value) = cached {
            if matches!(arg(record, 3), Some(Value::Null)) {
                None
            } else {
                if !self.copied(cx, value.unit.as_ref().map_or(0, unit_size)) {
                    return None;
                }
                value.unit.clone()
            }
        } else {
            self.detailed_unit(record, 3, kind, false, cx)
        };
        self.used.insert(id);
        Some(PropertyChoices {
            name,
            values,
            unit,
            source_instance: Some(id),
        })
    }

    fn detailed_unit(
        &mut self,
        record: &Record,
        at: usize,
        kind: Option<&str>,
        default: bool,
        cx: &mut Cx,
    ) -> Option<Unit> {
        match arg(record, at) {
            Some(Value::Null) => {
                if default {
                    kind.and_then(|k| self.measure_unit(k, record, cx))
                } else {
                    None
                }
            }
            Some(Value::Ref(id)) => {
                let unit = self.unit(*id, cx);
                if unit.is_none() && !self.is_halted() {
                    cx.report(
                        Diagnostic::new(codes::IFC_UNIT_MISSING)
                            .with("type", kind.unwrap_or("").to_owned())
                            .at(at_bytes(record.span())),
                    );
                }
                unit
            }
            _ => {
                self.invalid(record, at as u64, cx);
                None
            }
        }
    }
}
