//! 要素自身のスタイルと拡張を、継承を解かずに公開する。指定があるときだけ索引と値を作る。

use std::collections::BTreeMap;
use std::ops::Range;
use zumen_base::{Cx, Diagnostic, Key, Kind, Ref, Stop, codes};
use zumen_drawing::{
    Basis, ColorSystem, Drawing2D, FieldValue, Hash, Inherit, LineTypeId, LineWeight, Style,
    Target, id,
};

use crate::messages as m;
use crate::session::{Failed, fail};
use crate::tables::table_id;

pub(super) enum Error {
    Invalid(&'static str),
    Stopped(Stop),
}
impl From<Stop> for Error {
    fn from(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

pub(super) fn failed(error: Error, id: &str, source: &Ref, cx: &mut Cx<'_>) -> Failed {
    let diagnostic = match error {
        Error::Stopped(stop) => stop.to_diagnostic(),
        Error::Invalid(reason) => {
            let bytes = (id.len() as u64).saturating_add(128);
            if let Err(stop) = cx
                .check(Kind::Work, bytes)
                .and_then(|()| cx.check(Kind::DecodedBytes, bytes))
            {
                return fail(cx, stop.to_diagnostic().at(source.clone()));
            }
            Diagnostic::new(codes::DRAWING_ENTITY_DETAILS_FAILED)
                .with("id", id)
                .with("reason", reason)
        }
    };
    fail(cx, diagnostic.at(source.clone()))
}

fn reserve<T>(values: &mut Vec<T>, count: usize, slot: u64, cx: &mut Cx<'_>) -> Result<(), Error> {
    let bytes = (count as u64)
        .checked_mul(slot)
        .ok_or(Error::Invalid("allocation"))?;
    cx.check(Kind::DecodedBytes, bytes)?;
    if bytes > i32::MAX as u64 {
        return Err(Error::Invalid("allocation"));
    }
    values
        .try_reserve_exact(count)
        .map_err(|_| Error::Invalid("allocation"))
}

fn copied(value: &str, cx: &mut Cx<'_>) -> Result<String, Error> {
    let bytes = value.len() as u64;
    cx.check(Kind::Work, bytes)?;
    cx.check(Kind::DecodedBytes, bytes)?;
    if bytes > i32::MAX as u64 {
        return Err(Error::Invalid("allocation"));
    }
    let mut result = String::new();
    result
        .try_reserve_exact(value.len())
        .map_err(|_| Error::Invalid("allocation"))?;
    result.push_str(value);
    Ok(result)
}

#[derive(Default)]
pub(in crate::session) struct Index {
    line_types: Vec<(LineTypeId, usize, String)>,
    extensions: Vec<usize>,
    max_target_bytes: u64,
}

impl Index {
    pub(super) fn new(
        drawing: &Drawing2D,
        styles: bool,
        extensions: bool,
        cx: &mut Cx<'_>,
    ) -> Result<Self, Error> {
        let mut result = Self::default();
        if styles {
            let count = drawing.line_types.len();
            let depth = u64::from(count.max(1).ilog2()) + 1;
            // 公開 ID はハンドルか数値と重複の番号だけ。索引・一時の木と文字列を固定の量で数える。
            cx.check(
                Kind::Work,
                (count as u64).saturating_mul(depth).saturating_mul(4096),
            )?;
            reserve(&mut result.line_types, count, 512, cx)?;
            let mut seen = BTreeMap::new();
            for (index, entry) in drawing.line_types.iter().enumerate() {
                cx.check(Kind::Work, 0)?;
                result.line_types.push((
                    entry.id,
                    index,
                    table_id(id::line_type(entry), &mut seen),
                ));
            }
            result
                .line_types
                .sort_unstable_by_key(|(id, index, _)| (*id, *index));
        }
        if extensions {
            let mut count = 0;
            for entry in &drawing.extensions {
                cx.check(Kind::Work, 1)?;
                if let Target::Element(id) = &entry.target {
                    count += 1;
                    result.max_target_bytes = result.max_target_bytes.max(id.len() as u64 + 1);
                }
            }
            reserve(&mut result.extensions, count, 8, cx)?;
            for (index, entry) in drawing.extensions.iter().enumerate() {
                cx.check(Kind::Work, 1)?;
                if matches!(entry.target, Target::Element(_)) {
                    result.extensions.push(index);
                }
            }
            // 並べ替え中の最大比較量を先に数える。添字も比べ、同じ対象の値を元の順に保つ。
            let depth = u64::from(count.max(1).ilog2()) + 1;
            let work = (count as u64)
                .saturating_mul(depth)
                .saturating_mul(result.max_target_bytes)
                .saturating_mul(64);
            cx.check(Kind::Work, work)?;
            result.extensions.sort_unstable_by(|&a, &b| {
                target(drawing, a).cmp(target(drawing, b)).then(a.cmp(&b))
            });
        }
        Ok(result)
    }

    pub(super) fn extension_range(
        &self,
        drawing: &Drawing2D,
        id: &str,
        cx: &mut Cx<'_>,
    ) -> Result<Range<usize>, Error> {
        let depth = u64::from(self.extensions.len().max(1).ilog2()) + 1;
        cx.check(
            Kind::Work,
            self.max_target_bytes
                .saturating_mul(depth)
                .saturating_mul(2),
        )?;
        let start = self
            .extensions
            .partition_point(|&index| target(drawing, index) < id);
        let end = self
            .extensions
            .partition_point(|&index| target(drawing, index) <= id);
        Ok(start..end)
    }

    pub(super) fn style(&self, style: &Style, cx: &mut Cx<'_>) -> Result<m::EntityStyle, Error> {
        cx.check(Kind::Work, 32)?;
        cx.check(Kind::DecodedBytes, 512)?;
        let mut color = m::ColorChange {
            inherit: None,
            aci: None,
            sxf: None,
            jww: None,
            rgb: None,
        };
        match style.color {
            Inherit::Layer => color.inherit = Some(m::Inherit::Layer),
            Inherit::Block => color.inherit = Some(m::Inherit::Block),
            Inherit::Own(value) => {
                if let Some(code) = value.code {
                    let number = u64::from(code.value);
                    match code.system {
                        ColorSystem::Aci if (1..=255).contains(&number) => color.aci = Some(number),
                        ColorSystem::Sxf if number > 0 => color.sxf = Some(number),
                        ColorSystem::Jww if number <= 356 => color.jww = Some(number),
                        _ => return Err(Error::Invalid("color-code")),
                    }
                }
                color.rgb = value.rgb.map(|rgb| rgb.map(u64::from).to_vec());
            }
        }
        let mut line_type = m::LineTypeChange {
            inherit: None,
            name: None,
            id: None,
        };
        match style.line_type {
            Inherit::Layer => line_type.inherit = Some(m::Inherit::Layer),
            Inherit::Block => line_type.inherit = Some(m::Inherit::Block),
            Inherit::Own(wanted) => {
                cx.check(
                    Kind::Work,
                    u64::from(self.line_types.len().max(1).ilog2()) + 1,
                )?;
                let at = self.line_types.partition_point(|(id, _, _)| *id < wanted);
                let Some((_, _, public)) =
                    self.line_types.get(at).filter(|(id, _, _)| *id == wanted)
                else {
                    return Err(Error::Invalid("line-type-reference"));
                };
                line_type.id = Some(copied(public, cx)?);
            }
        }
        let line_weight = style
            .line_weight
            .map(|value| {
                let mut output = m::LineWeightChange {
                    inherit: None,
                    mm: None,
                    default: None,
                };
                match value {
                    Inherit::Layer => output.inherit = Some(m::Inherit::Layer),
                    Inherit::Block => output.inherit = Some(m::Inherit::Block),
                    Inherit::Own(LineWeight::Default) => output.default = Some(true),
                    Inherit::Own(LineWeight::Width(width))
                        if width.0.is_finite() && width.0 >= 0.0 =>
                    {
                        output.mm = Some(width.0)
                    }
                    _ => return Err(Error::Invalid("line-weight-number")),
                }
                Ok(output)
            })
            .transpose()?;
        if !style.line_type_scale.is_finite() {
            return Err(Error::Invalid("line-type-scale-number"));
        }
        let transparency = match style.transparency {
            Inherit::Layer => m::EntityTransparency {
                inherit: Some(m::Inherit::Layer),
                alpha: None,
            },
            Inherit::Block => m::EntityTransparency {
                inherit: Some(m::Inherit::Block),
                alpha: None,
            },
            Inherit::Own(value) => m::EntityTransparency {
                inherit: None,
                alpha: Some(u64::from(value.alpha)),
            },
        };
        Ok(m::EntityStyle {
            color,
            line_type,
            line_type_scale: style.line_type_scale,
            line_weight,
            transparency,
        })
    }

    pub(super) fn extensions(
        &self,
        drawing: &Drawing2D,
        range: Range<usize>,
        hash: Option<Hash>,
        cx: &mut Cx<'_>,
    ) -> Result<Vec<m::EntityExtension>, Error> {
        let mut output = Vec::new();
        reserve(&mut output, range.len(), 512, cx)?;
        let indices = self
            .extensions
            .get(range)
            .ok_or(Error::Invalid("extension-reference"))?;
        for &index in indices {
            cx.check(Kind::Work, 1)?;
            let extension = drawing
                .extensions
                .get(index)
                .ok_or(Error::Invalid("extension-reference"))?;
            let mut fields = Vec::new();
            reserve(&mut fields, extension.fields.len(), 256, cx)?;
            for field in &extension.fields {
                cx.check(Kind::Work, 1)?;
                fields.push(m::EntityExtensionField {
                    key: copied(&field.key, cx)?,
                    code: field.code.map(i64::from),
                    value: field_value(&field.value, cx)?,
                });
            }
            let basis = match extension.basis {
                Basis::Document => None,
                Basis::Hash(hash) => Some(hash.to_string()),
            };
            output.push(m::EntityExtension {
                namespace: copied(&extension.namespace, cx)?,
                name: copied(&extension.name, cx)?,
                version: extension
                    .version
                    .as_deref()
                    .map(|value| copied(value, cx))
                    .transpose()?,
                schema: u64::from(extension.schema),
                basis,
                current: hash.is_some_and(|hash| extension.basis == Basis::Hash(hash)),
                fields,
                source: source(&extension.source),
            });
        }
        Ok(output)
    }
}

fn target(drawing: &Drawing2D, index: usize) -> &str {
    match drawing.extensions.get(index).map(|entry| &entry.target) {
        Some(Target::Element(id)) => id,
        _ => "",
    }
}

fn field_value(value: &FieldValue, cx: &mut Cx<'_>) -> Result<m::EntityExtensionValue, Error> {
    Ok(match value {
        FieldValue::Text(value) => m::EntityExtensionValue::Text(m::EntityExtensionText {
            value: copied(value, cx)?,
        }),
        FieldValue::Integer(value) => m::EntityExtensionValue::Integer(m::EntityExtensionInteger {
            value: value.to_string(),
        }),
        FieldValue::Handle(value) => m::EntityExtensionValue::Handle(m::EntityExtensionHandle {
            value: format!("{value:X}"),
        }),
        FieldValue::Real(value) if value.is_finite() => {
            m::EntityExtensionValue::Real(m::EntityExtensionReal { value: *value })
        }
        FieldValue::Point(value) if value.iter().all(|v| v.is_finite()) => {
            m::EntityExtensionValue::Point(m::EntityExtensionPoint {
                value: value.to_vec(),
            })
        }
        FieldValue::Bytes(value) => {
            let count = value
                .len()
                .checked_mul(2)
                .ok_or(Error::Invalid("allocation"))?;
            cx.check(Kind::Work, count as u64)?;
            cx.check(Kind::DecodedBytes, count as u64)?;
            if count > i32::MAX as usize {
                return Err(Error::Invalid("allocation"));
            }
            let mut hex = String::new();
            hex.try_reserve_exact(count)
                .map_err(|_| Error::Invalid("allocation"))?;
            let digit = |n: u8| char::from(if n < 10 { b'0' + n } else { b'a' + n - 10 });
            for &byte in value {
                cx.check(Kind::Work, 0)?;
                hex.push(digit(byte >> 4));
                hex.push(digit(byte & 15));
            }
            m::EntityExtensionValue::Bytes(m::EntityExtensionBytes { value: hex })
        }
        _ => return Err(Error::Invalid("extension-number")),
    })
}

fn source(value: &Ref) -> m::Source {
    m::Source {
        record: value.record(),
        bytes: value.bytes().map(|bytes| m::ByteRange {
            start: bytes.start,
            end: bytes.end,
        }),
        handle: match value.key() {
            Some(Key::Handle(handle)) => Some(format!("{handle:X}")),
            _ => None,
        },
        instance: match value.key() {
            Some(Key::Instance(number)) => Some(number.to_string()),
            _ => None,
        },
    }
}
