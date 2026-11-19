//! DIMSTYLE の値と、寸法の ACAD/DSTYLE の個別指定を表示用のモデルに移す。

use std::collections::BTreeMap;

use super::{Charset, XDATA, work};
use crate::db::{AcadDb, Data, Object};
use zumen_base::{Cx, Key, Kind, Ref};
use zumen_drawing::{
    Basis, DimensionDisplayRecord, DimensionDisplayTarget, DimensionDisplayValues, Drawing2D,
    FieldValue, Shape, Target, id,
};
use zumen_sdk::Error;

pub(super) fn style_values(
    db: &AcadDb,
    object: &Object,
    charset: Charset,
) -> DimensionDisplayValues {
    let real = |code| match object.first(code) {
        Some(Data::Real(value)) if value.is_finite() => Some(*value),
        _ => None,
    };
    let int = |code| match object.first(code) {
        Some(Data::Int(value)) => Some(*value),
        Some(Data::Bool(value)) => Some(i64::from(*value)),
        _ => None,
    };
    let text_style = match object.first(340) {
        Some(Data::Handle(key)) => db
            .get(*key)
            .filter(|object| object.kind == b"STYLE")
            .and_then(|object| object.first(2))
            .and_then(|data| match data {
                Data::Text(name) => charset.decode(name),
                _ => None,
            }),
        _ => None,
    };
    // DIMSCALE、DIMASZ、DIMTSZ は従来の DimensionStyle が保持する。
    DimensionDisplayValues {
        extension_offset: real(42),
        extension_beyond: real(44),
        suppress_first_extension: int(75).map(|v| v != 0),
        suppress_second_extension: int(76).map(|v| v != 0),
        text_vertical_placement: int(77).and_then(|v| u8::try_from(v).ok()),
        text_height: real(140),
        linear_factor: real(144),
        text_gap: real(147),
        linear_precision: int(271).and_then(|v| u8::try_from(v).ok()),
        text_style,
        ..Default::default()
    }
}

/// ACAD の XDATA にある DSTYLE の組を走査する。別の { } の中の値を上書きとして読まない。
pub(super) fn dstyle_pairs<'a, V: 'a>(
    fields: impl Iterator<Item = (i16, &'a V)>,
    text: impl Fn(&V) -> Option<&[u8]>,
    int: impl Fn(&V) -> Option<i64>,
    mut visit: impl FnMut(i16, i16, &V),
) {
    let mut tagged = false;
    let mut dstyle = false;
    let mut depth = 0usize;
    let mut key = None;
    for (code, value) in fields {
        if code == 1002 {
            match text(value) {
                Some(b"{") => {
                    if depth == 0 {
                        dstyle = tagged;
                    }
                    depth = depth.saturating_add(1);
                }
                Some(b"}") => depth = depth.saturating_sub(1),
                _ => {}
            }
            tagged = false;
            key = None;
        } else if depth == 0 {
            tagged = code == 1000 && text(value) == Some(b"DSTYLE");
        } else if dstyle && depth == 1 {
            if let Some(field) = key.take() {
                visit(field, code, value);
            } else if code == 1070 {
                key = int(value).and_then(|i| i16::try_from(i).ok());
            }
        }
    }
}

/// XDATA にすでに結び付いた ID とハッシュを使う。ライブの並べ替えやキーのない要素も同じ経路で解決する。
pub(super) fn attach_overrides(drawing: &mut Drawing2D, cx: &mut Cx<'_>) -> Result<(), Error> {
    let mut pending: BTreeMap<_, (_, DimensionDisplayValues)> = BTreeMap::new();
    let mut fonts = None;
    for ext in &drawing.extensions {
        work(cx, 1, &ext.source)?;
        let (Target::Element(id), Basis::Hash(basis)) = (&ext.target, ext.basis) else {
            continue;
        };
        if ext.namespace != XDATA || !ext.name.eq_ignore_ascii_case("ACAD") {
            continue;
        }
        work(cx, ext.fields.len() as u64, &ext.source)?;
        // 同じ要素に ACAD の組が複数あれば、後の組にある項目だけを重ねる。
        let previous = pending.remove(id.as_str());
        let mut values = previous.map_or_else(DimensionDisplayValues::default, |(_, v)| v);
        let mut font = None;
        dstyle_pairs(
            ext.fields.iter().filter_map(|f| Some((f.code?, &f.value))),
            |v| match v {
                FieldValue::Text(s) => Some(s.as_bytes()),
                _ => None,
            },
            |v| match v {
                FieldValue::Integer(i) => Some(*i),
                _ => None,
            },
            |field, code, v| match (field, code, v) {
                (40, 1040, FieldValue::Real(x)) => values.scale = Some(*x),
                (41, 1040, FieldValue::Real(x)) => values.arrow_size = Some(*x),
                (42, 1040, FieldValue::Real(x)) => values.extension_offset = Some(*x),
                (44, 1040, FieldValue::Real(x)) => values.extension_beyond = Some(*x),
                (140, 1040, FieldValue::Real(x)) => values.text_height = Some(*x),
                (142, 1040, FieldValue::Real(x)) => values.tick_size = Some(*x),
                (144, 1040, FieldValue::Real(x)) => values.linear_factor = Some(*x),
                (147, 1040, FieldValue::Real(x)) => values.text_gap = Some(*x),
                (75, 1070 | 1071, FieldValue::Integer(x)) => {
                    values.suppress_first_extension = Some(*x != 0)
                }
                (76, 1070 | 1071, FieldValue::Integer(x)) => {
                    values.suppress_second_extension = Some(*x != 0)
                }
                (77, 1070 | 1071, FieldValue::Integer(x)) => {
                    values.text_vertical_placement = u8::try_from(*x).ok()
                }
                (271, 1070 | 1071, FieldValue::Integer(x)) => {
                    values.linear_precision = u8::try_from(*x).ok()
                }
                (340, 1005, FieldValue::Handle(h)) => font = Some(*h),
                (340, 1000, FieldValue::Text(s)) => font = u64::from_str_radix(s.trim(), 16).ok(),
                _ => {}
            },
        );
        if let Some(key) = font {
            if fonts.is_none() {
                work(cx, drawing.text_styles.len() as u64, &ext.source)?;
                scratch(
                    cx,
                    (drawing.text_styles.len() as u64).saturating_mul(32),
                    &ext.source,
                )?;
                fonts = Some(
                    drawing
                        .text_styles
                        .iter()
                        .filter_map(|style| {
                            let Key::Handle(h) = style.source.key()? else {
                                return None;
                            };
                            Some((h, style.name.as_str()))
                        })
                        .collect::<BTreeMap<_, _>>(),
                );
            }
            if let Some(name) = fonts.as_ref().and_then(|table| table.get(&key)) {
                scratch(cx, name.len() as u64, &ext.source)?;
                values.text_style = Some((*name).to_owned());
            }
        }
        if values != DimensionDisplayValues::default() {
            scratch(
                cx,
                <DimensionDisplayRecord as zumen_drawing::ModelSize>::SIZE,
                &ext.source,
            )?;
            pending.insert(id.as_str(), (basis, values));
        }
    }
    if pending.is_empty() {
        return Ok(());
    }
    let mut records = Vec::new();
    id::each_element(drawing, |_, id, element| {
        let (source, is_dimension) = match element {
            id::Found::Model(element) => (
                &element.source,
                matches!(element.shape, Shape::Dimension(_)),
            ),
            id::Found::Paper(element) => (
                &element.source,
                matches!(element.shape, Shape::Dimension(_)),
            ),
        };
        work(cx, 1, source)?;
        if is_dimension && let Some((basis, values)) = pending.remove(id.as_str()) {
            records.push(DimensionDisplayRecord {
                target: DimensionDisplayTarget::Element { id, basis },
                values,
            });
        }
        Ok::<_, Error>(())
    })?;
    drawing.dimension_display.extend(records);
    Ok(())
}

fn scratch(cx: &mut Cx<'_>, bytes: u64, source: &Ref) -> Result<(), Error> {
    cx.check(Kind::DecodedBytes, bytes).map_err(|stop| {
        cx.report(stop.to_diagnostic().at(source.clone()));
        Error::Stopped(stop)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_fields_in_each_dstyle_group_but_skips_nested_lists() {
        let txt = |s: &[u8]| Data::Text(s.to_vec());
        let tags = [
            (1070, Data::Int(42)),
            (1040, Data::Real(12.0)),
            (1002, txt(b"{")),
            (1000, txt(b"DSTYLE")),
            (1002, txt(b"{")),
            (1070, Data::Int(140)),
            (1040, Data::Real(80.0)),
            (1002, txt(b"}")),
            (1002, txt(b"}")),
            (1000, txt(b"DSTYLE")),
            (1002, txt(b"{")),
            (1070, Data::Int(140)),
            (1040, Data::Real(0.25)),
            (1002, txt(b"{")),
            (1070, Data::Int(140)),
            (1040, Data::Real(99.0)),
            (1002, txt(b"}")),
            (1002, txt(b"}")),
            (1000, txt(b"DSTYLE")),
            (1002, txt(b"{")),
            (1070, Data::Int(44)),
            (1040, Data::Real(0.5)),
            (1002, txt(b"}")),
        ];
        let mut seen = Vec::new();
        dstyle_pairs(
            tags.iter().map(|(code, data)| (*code, data)),
            |data| {
                if let Data::Text(s) = data {
                    Some(s)
                } else {
                    None
                }
            },
            |data| {
                if let Data::Int(i) = data {
                    Some(*i)
                } else {
                    None
                }
            },
            |field, code, value| {
                if let Data::Real(x) = value {
                    seen.push((field, code, *x))
                }
            },
        );
        assert_eq!(seen, [(140, 1040, 0.25), (44, 1040, 0.5)]);
    }
}
