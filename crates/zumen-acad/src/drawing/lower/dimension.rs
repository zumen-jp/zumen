//! 新しい DXF の寸法スタイルと個別の DSTYLE。表示部品を合成する前に書き先の参照を決める。

use super::{Report, dimension_style_index, failed, quiet, report_dropped, stopped};
use crate::DxfFile;
use crate::db::{Data, Section, Value};
use crate::drawing::charset_of;
use std::collections::{BTreeMap, BTreeSet};
use zumen_base::{Cx, Kind, Ref};
use zumen_drawing::{
    DimensionDisplayTarget, DimensionDisplayValues, Drawing2D, Position, ResolvedDimensionDisplay,
    Shape, id,
};
use zumen_sdk::Error;

// DXF のグループコードと値の保持量。ポインター幅に依存しない論理バイト数で数える。
const FIELD_BYTES: u64 = 40;

/// スタイルを先に反映し、個別設定は追加する元の要素の位置に結ぶ。
pub(super) fn prepare(
    file: &mut DxfFile,
    drawing: &Drawing2D,
    report: &mut Report,
    cx: &mut Cx<'_>,
) -> Result<BTreeMap<Position, Vec<Value>>, Error> {
    let mut result = BTreeMap::new();
    if drawing.dimension_display.is_empty() {
        return Ok(result);
    }
    let mut db = quiet(cx, |cx| file.objects(cx)).map_err(|error| failed(cx, error))?;
    let charset = charset_of(&db);
    let mut text_styles = BTreeMap::new();
    let mut dim_styles = BTreeMap::new();
    for (index, row) in db.objects().iter().enumerate() {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        if row.section != Section::Tables || !matches!(row.kind.as_slice(), b"STYLE" | b"DIMSTYLE")
        {
            continue;
        }
        let Some(Data::Text(bytes)) = row.first(2) else {
            continue;
        };
        cx.check(Kind::DecodedBytes, 64 + bytes.len() as u64 * 4)
            .map_err(|stop| stopped(cx, stop))?;
        let Some(name) = charset.decode(bytes) else {
            continue;
        };
        if row.kind == b"DIMSTYLE" {
            dim_styles.entry(name.to_ascii_lowercase()).or_insert(index);
        } else if let Some(handle) = row.handle() {
            text_styles
                .entry(name.to_ascii_lowercase())
                .or_insert(handle);
        }
    }
    let mut locals = BTreeMap::new();
    let mut styles_seen = BTreeSet::new();
    let style_sources = dimension_style_index(drawing, cx)?;
    let mut changed = false;
    for record in &drawing.dimension_display {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        cx.check(Kind::DecodedBytes, 64)
            .map_err(|stop| stopped(cx, stop))?;
        match &record.target {
            DimensionDisplayTarget::Element { id, basis } => {
                locals.entry(id.as_str()).or_insert((basis, &record.values));
            }
            DimensionDisplayTarget::Style(name) => {
                cx.check(Kind::DecodedBytes, name.len() as u64)
                    .map_err(|stop| stopped(cx, stop))?;
                let name = name.to_ascii_lowercase();
                if !styles_seen.insert(name.clone()) {
                    continue;
                }
                let empty = Ref::default();
                let source = style_sources
                    .get(&name)
                    .map_or(&empty, |(_, style)| &style.source);
                let Some(row) = dim_styles
                    .get(&name)
                    .and_then(|index| db.object_mut(*index))
                else {
                    report_dropped(cx, report, "dimension_display_style", source)?;
                    continue;
                };
                for field in fields(&record.values, false, &text_styles, source, report, cx)? {
                    cx.check(Kind::Work, row.values.len() as u64)
                        .map_err(|stop| stopped(cx, stop))?;
                    if let Some(old) = row.values.iter_mut().find(|v| v.code == field.code) {
                        *old = field;
                    } else {
                        row.values.push(field);
                    }
                    changed = true;
                }
            }
        }
    }
    if changed {
        quiet(cx, |cx| file.write_back(&db, cx)).map_err(|error| failed(cx, error))?;
    }
    if !locals.is_empty() {
        id::each_element(drawing, |position, id, element| -> Result<(), Error> {
            cx.check(Kind::Work, 1 + id.len() as u64)
                .map_err(|stop| stopped(cx, stop))?;
            let Some((basis, values)) = locals.remove(id.as_str()) else {
                return Ok(());
            };
            if element.hash_checked(cx).map_err(|stop| stopped(cx, stop))? != *basis {
                return Ok(());
            }
            let (dimension, source) = match element {
                id::Found::Model(e) => (matches!(e.shape, Shape::Dimension(_)), &e.source),
                id::Found::Paper(e) => (matches!(e.shape, Shape::Dimension(_)), &e.source),
            };
            if !dimension {
                report_dropped(cx, report, "dimension_display_target", source)?;
                return Ok(());
            }
            let fields = fields(values, true, &text_styles, source, report, cx)?;
            if !fields.is_empty() {
                // DXF Reference「Extended Data」。DSTYLE のキーは 1070、値には対応する型の拡張コードを使う。
                cx.check(
                    Kind::DecodedBytes,
                    (4 + 2 * fields.len() as u64) * FIELD_BYTES,
                )
                .map_err(|stop| stopped(cx, stop))?;
                let text = |code, bytes: &[u8]| Value {
                    code,
                    data: Data::Text(bytes.to_vec()),
                };
                let mut xdata = vec![text(1001, b"ACAD"), text(1000, b"DSTYLE"), text(1002, b"{")];
                for field in fields {
                    xdata.push(Value {
                        code: 1070,
                        data: Data::Int(i64::from(field.code)),
                    });
                    let code = match field.data {
                        Data::Real(_) => 1040,
                        Data::Handle(_) => 1005,
                        _ => 1070,
                    };
                    xdata.push(Value {
                        code,
                        data: field.data,
                    });
                }
                xdata.push(text(1002, b"}"));
                result.insert(position, xdata);
            }
            Ok(())
        })?;
    }
    Ok(result)
}

fn fields(
    values: &DimensionDisplayValues,
    local: bool,
    text_styles: &BTreeMap<String, u64>,
    source: &Ref,
    report: &mut Report,
    cx: &mut Cx<'_>,
) -> Result<Vec<Value>, Error> {
    // 未指定・範囲外は書かず、出力を再読したときも上位の値を継承する。
    let valid = if local {
        ResolvedDimensionDisplay::from_records(None, Some(values))
    } else {
        ResolvedDimensionDisplay::from_records(Some(values), None)
    };
    let mut fields = Vec::new();
    cx.check(Kind::DecodedBytes, 13 * FIELD_BYTES)
        .map_err(|stop| stopped(cx, stop))?;
    for (code, value, specified) in [
        (40, valid.scale, local && values.scale.is_some()),
        (41, valid.arrow_size, local && values.arrow_size.is_some()),
        (142, valid.tick_size, local && values.tick_size.is_some()),
        (
            42,
            valid.extension_offset,
            values.extension_offset.is_some(),
        ),
        (
            44,
            valid.extension_beyond,
            values.extension_beyond.is_some(),
        ),
        (140, valid.text_height, values.text_height.is_some()),
        (144, valid.linear_factor, values.linear_factor.is_some()),
        (147, valid.text_gap, values.text_gap.is_some()),
    ] {
        if let Some(number) = value {
            fields.push(Value {
                code,
                data: Data::Real(number),
            });
        } else if specified {
            report_dropped(cx, report, "dimension_display_value", source)?;
        }
    }
    for (code, value) in [
        (75, valid.suppress_first_extension.map(i64::from)),
        (76, valid.suppress_second_extension.map(i64::from)),
        (77, valid.text_vertical_placement.map(i64::from)),
        (271, valid.linear_precision.map(i64::from)),
    ] {
        if let Some(number) = value {
            fields.push(Value {
                code,
                data: Data::Int(number),
            });
        }
    }
    if values.linear_precision.is_some() && valid.linear_precision.is_none() {
        report_dropped(cx, report, "dimension_linear_precision", source)?;
    }
    if values.text_vertical_placement.is_some() && valid.text_vertical_placement.is_none() {
        report_dropped(cx, report, "dimension_display_value", source)?;
    }
    if let Some(style) = valid.text_style {
        cx.check(Kind::Work, 1 + style.len() as u64)
            .map_err(|stop| stopped(cx, stop))?;
        cx.check(Kind::DecodedBytes, style.len() as u64)
            .map_err(|stop| stopped(cx, stop))?;
        if let Some(&handle) = text_styles.get(&style.to_ascii_lowercase()) {
            fields.push(Value {
                code: 340,
                data: Data::Handle(handle),
            });
        } else {
            report_dropped(cx, report, "dimension_text_style", source)?;
        }
    }
    Ok(fields)
}
