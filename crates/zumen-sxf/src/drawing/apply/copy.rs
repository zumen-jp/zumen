//! SXF（SFC・P21）の要素を複写する（`Edit::Copy`、#889）。
//!
//! 用紙の線・折れ線・円・円弧・点マーカー・文字・複合図形配置を、スタイルと形式固有の拡張（文字範囲の幅・字間など）を
//! 保ったまま新しいフィーチャ（または P21 の実体）として追加する。複合曲線（`composite_curve_org`）の要素や、
//! 属性付加機構（`$$ATR*`・SAF の `$$ATRF$$`）が付いた要素、1 フィーチャから複数要素に展開されたものは、関連が壊れるため
//! `change.copy.unsupported` で変更の一覧全体を拒む。

use std::collections::BTreeSet;

use zumen_base::{Diagnostic, Key, Ref, codes};
use zumen_drawing::{
    Affine, Drawing2D, Element, Field, FieldValue, Hash, Inherit, PaperPoint, Placed, Seen, Shape,
    SymbolDefinition, Target,
};
use zumen_sdk::{Cx, Error};

use super::{
    Any, Expected, FeatureRecord, Index, MARKER, Mode, OnSheet, Plan, SxfFile, Tables,
    count_element_copy, count_shape, shape_arguments, text, unset_field, work,
};

/// `Edit::Copy` ごとの配置先と複写元の指定（#889）。`Changes::edits` と同じ添字で対応させる
#[derive(Clone, Copy, Debug, Default)]
pub struct CopyFrom<'a> {
    /// 配置先の用紙の ID。`None` なら元の要素と同じ場所
    pub place: Option<&'a str>,
    /// 複写元の文書。`None` なら当てる文書と同じ文書
    pub from: Option<&'a SxfFile>,
}

pub(super) struct SourceSxf {
    pub(super) mode: Mode,
    pub(super) records: Vec<FeatureRecord>,
    pub(super) grouped: BTreeSet<usize>,
    pub(super) drawing: Drawing2D,
}

#[derive(Clone, Copy, Default)]
pub(super) struct CopySpec<'a> {
    pub(super) place: Option<&'a str>,
    pub(super) from: Option<&'a SourceSxf>,
}

pub(in crate::drawing) type Refusal = (&'static str, &'static str);
pub(in crate::drawing) type ExpectedExt = (String, Vec<Field>);

#[allow(clippy::too_many_arguments)]
pub(super) fn copy_element(
    plan: &mut Plan<'_>,
    format: &str,
    self_mode: Mode,
    records: &[FeatureRecord],
    drawing: &Drawing2D,
    before: &Index<'_>,
    tables: &Tables,
    grouped: &BTreeSet<usize>,
    targets: &BTreeSet<u64>,
    target: &Seen,
    transform: &Affine,
    spec: CopySpec<'_>,
    edit_index: usize,
    cx: &mut Cx,
) -> Result<Option<Expected>, Error> {
    if let Some(source) = spec.from {
        return copy_from_other(
            plan, format, self_mode, drawing, before, tables, source, target, transform,
            spec.place, edit_index, cx,
        );
    }

    let Some(element) = before.element(&target.id) else {
        cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", target.id.as_str()));
        return Ok(None);
    };
    if element.hash(cx)? != target.hash {
        cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_STALE).with("id", target.id.as_str()));
        return Ok(None);
    }

    let source_ref = element.source();
    let rec_num = source_ref.record().and_then(|r| usize::try_from(r).ok());
    let _ = records;
    let kind_name = rec_num
        .and_then(|r| plan.record(r))
        .map(|rec| rec.kind.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "element".to_owned());
    let refuse = |cx: &mut Cx, (field, value): Refusal| {
        cx.report(
            Diagnostic::new(codes::CHANGE_COPY_UNSUPPORTED)
                .with("edit", edit_index as u64)
                .with("kind", kind_name.as_str())
                .with("field", field)
                .with("value", value)
                .with("format", format)
                .at(source_ref.clone()),
        );
    };

    let found = match (source_ref.key(), rec_num) {
        (Some(Key::Instance(instance)), Some(record)) => before
            .found(instance)
            .filter(|found| found.id == target.id)
            .map(|found| (found, record)),
        _ => None,
    };
    let Some((found, record)) = found else {
        refuse(cx, ("source", "shared"));
        return Ok(None);
    };
    if targets.contains(&found.instance) {
        refuse(cx, ("source", "changed"));
        return Ok(None);
    }
    if grouped.contains(&record) {
        refuse(cx, ("reference", "composite_curve"));
        return Ok(None);
    }
    if let Some(attr_kind) = attribute_extension(drawing, &target.id) {
        refuse(cx, ("extensions", attr_kind));
        return Ok(None);
    }

    if let Some(place_id) = spec.place {
        if !before.all_places.contains(place_id) {
            cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", place_id));
            return Ok(None);
        }
        if !before.sheets.contains_key(place_id) {
            refuse(cx, ("place", "unsupported"));
        }
        if !before.sheets.contains_key(place_id) {
            return Ok(None);
        }
    }
    let Any::Paper(orig_paper) = found.element else {
        refuse(cx, ("place", "unsupported"));
        return Ok(None);
    };
    if let Some(place_id) = spec.place
        && before.sheets.get(place_id) != Some(&orig_paper.shape_sheet())
    {
        refuse(cx, ("place", "unsupported"));
        return Ok(None);
    }

    found.element.count_copy(cx, 2)?;
    let Some(Placed::Paper(moved_paper)) = found.element.transformed(transform) else {
        refuse(cx, ("transform", "unsupported"));
        return Ok(None);
    };

    let orig_record = plan.record(record);
    match add_copied_paper(plan, drawing, tables, &moved_paper, orig_record, false, cx)? {
        Ok((instance, added_elem, exts)) => Ok(Some(Expected::Copied(
            instance,
            Box::new(Placed::Paper(added_elem)),
            exts,
        ))),
        Err(reason) => {
            refuse(cx, reason);
            Ok(None)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn copy_from_other(
    plan: &mut Plan<'_>,
    format: &str,
    self_mode: Mode,
    dst_drawing: &Drawing2D,
    before: &Index<'_>,
    tables: &Tables,
    source: &SourceSxf,
    target: &Seen,
    transform: &Affine,
    place_id: Option<&str>,
    edit_index: usize,
    cx: &mut Cx,
) -> Result<Option<Expected>, Error> {
    let src_index = Index::new(&source.drawing, true, cx)?;
    let Some(element) = src_index.element(&target.id) else {
        cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", target.id.as_str()));
        return Ok(None);
    };
    if element.hash(cx)? != target.hash {
        cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_STALE).with("id", target.id.as_str()));
        return Ok(None);
    }

    let source_ref = element.source();
    let rec_num = source_ref.record().and_then(|r| usize::try_from(r).ok());
    let kind_name = rec_num
        .and_then(|r| source.records.iter().find(|rec| rec.record == r))
        .map(|rec| rec.kind.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "element".to_owned());
    let refuse = |cx: &mut Cx, (field, value): Refusal| {
        cx.report(
            Diagnostic::new(codes::CHANGE_COPY_UNSUPPORTED)
                .with("edit", edit_index as u64)
                .with("kind", kind_name.as_str())
                .with("field", field)
                .with("value", value)
                .with("format", format)
                .at(source_ref.clone()),
        );
    };

    if source.mode != self_mode {
        refuse(cx, ("format", "mismatch"));
        return Ok(None);
    }
    let default_place = target.id.rsplit_once('/').map(|(p, _)| p);
    let Some(place_id) = place_id.or(default_place) else {
        refuse(cx, ("place", "missing"));
        return Ok(None);
    };
    if !before.all_places.contains(place_id) {
        cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", place_id));
        return Ok(None);
    }
    let Some(&dst_sheet) = before.sheets.get(place_id) else {
        refuse(cx, ("place", "unsupported"));
        return Ok(None);
    };

    let found = match (source_ref.key(), rec_num) {
        (Some(Key::Instance(instance)), Some(record)) => src_index
            .found(instance)
            .filter(|f| f.id == target.id)
            .map(|f| (f, record)),
        _ => None,
    };
    let Some((found, record)) = found else {
        refuse(cx, ("source", "shared"));
        return Ok(None);
    };
    if source.grouped.contains(&record) {
        refuse(cx, ("reference", "composite_curve"));
        return Ok(None);
    }
    if let Some(attr_kind) = attribute_extension(&source.drawing, &target.id) {
        refuse(cx, ("extensions", attr_kind));
        return Ok(None);
    }
    let Any::Paper(_) = found.element else {
        refuse(cx, ("place", "unsupported"));
        return Ok(None);
    };

    found.element.count_copy(cx, 2)?;
    let Some(Placed::Paper(mut moved_paper)) = found.element.transformed(transform) else {
        refuse(cx, ("transform", "unsupported"));
        return Ok(None);
    };
    retarget_sheet(&mut moved_paper.shape, dst_sheet);

    // 1. レイヤ名で対応を確かめる
    let Some(src_layer) = source
        .drawing
        .layers
        .iter()
        .find(|l| l.id == moved_paper.layer)
    else {
        refuse(cx, ("layer", "missing"));
        return Ok(None);
    };
    let Some(dst_layer) = dst_drawing.layers.iter().find(|l| l.name == src_layer.name) else {
        refuse(cx, ("layer", "missing"));
        return Ok(None);
    };
    moved_paper.layer = dst_layer.id;

    // 2. 線種の対応と衝突を確かめる
    if let Inherit::Own(lt_id) = moved_paper.style.line_type
        && i64::from(lt_id.0) >= super::USER_LINE_TYPE
    {
        let src_lt = source.drawing.line_types.iter().find(|l| l.id == lt_id);
        let dst_lt = src_lt.and_then(|s| {
            dst_drawing
                .line_types
                .iter()
                .find(|d| i64::from(d.id.0) >= super::USER_LINE_TYPE && d.name == s.name)
        });
        match (src_lt, dst_lt) {
            (Some(s), Some(d)) if s.definition == d.definition => {
                moved_paper.style.line_type = Inherit::Own(d.id);
            }
            (Some(_), Some(_)) => {
                refuse(cx, ("line_type", "conflict"));
                return Ok(None);
            }
            _ => {
                refuse(cx, ("line_type", "missing"));
                return Ok(None);
            }
        }
    }

    // 3. 色の衝突（利用者定義色が異なる RGB にならないか）を確かめる
    if let Inherit::Own(src_color) = moved_paper.style.color {
        match tables.color(Inherit::Own(src_color)) {
            Some((_, dst_read)) if dst_read.rgb == src_color.rgb => {}
            Some(_) => {
                refuse(cx, ("style", "color_conflict"));
                return Ok(None);
            }
            None => {
                refuse(cx, ("style", "color_missing"));
                return Ok(None);
            }
        }
    }

    // 4. 文字フォントの存在を確かめる
    let font_name = match &moved_paper.shape {
        Shape::Text(t) if !t.style.is_empty() => Some(t.style.as_str()),
        Shape::Formatted(t) => t
            .spans
            .first()
            .and_then(|s| s.format.font.as_ref())
            .map(|f| f.name.as_str()),
        _ => None,
    };
    if let Some(font_name) = font_name
        && !dst_drawing.text_styles.iter().any(|s| s.name == font_name)
    {
        refuse(cx, ("text_style", "missing"));
        return Ok(None);
    }

    // 5. 複合図形配置（sfig_locate）なら定義の存在と一致を確かめる
    if let Shape::Symbol(sym) = &moved_paper.shape {
        let src_sym = source.drawing.symbols.iter().find(|s| s.key == sym.key);
        let dst_sym = dst_drawing.symbols.iter().find(|s| s.key == sym.key);
        match (src_sym, dst_sym) {
            (Some(s), Some(d)) => {
                if !same_symbol(s, d, cx, source_ref)? {
                    refuse(cx, ("symbol", "conflict"));
                    return Ok(None);
                }
            }
            _ => {
                refuse(cx, ("symbol", "missing"));
                return Ok(None);
            }
        }
    }

    let orig_record = source.records.iter().find(|r| r.record == record);
    match add_copied_paper(
        plan,
        dst_drawing,
        tables,
        &moved_paper,
        orig_record,
        true,
        cx,
    )? {
        Ok((instance, added_elem, exts)) => Ok(Some(Expected::Copied(
            instance,
            Box::new(Placed::Paper(added_elem)),
            exts,
        ))),
        Err(reason) => {
            refuse(cx, reason);
            Ok(None)
        }
    }
}

fn same_symbol(
    a: &SymbolDefinition,
    b: &SymbolDefinition,
    cx: &mut Cx,
    at: &Ref,
) -> Result<bool, Error> {
    if a.space.elements.len() != b.space.elements.len() || a.base != b.base {
        return Ok(false);
    }
    for (ea, eb) in a.space.elements.iter().zip(&b.space.elements) {
        let ha = Hash::element_checked(ea, cx).map_err(|stop| {
            cx.report(stop.to_diagnostic().at(at.clone()));
            Error::Stopped(stop)
        })?;
        let hb = Hash::element_checked(eb, cx).map_err(|stop| {
            cx.report(stop.to_diagnostic().at(at.clone()));
            Error::Stopped(stop)
        })?;
        if ha != hb {
            return Ok(false);
        }
    }
    Ok(true)
}

fn retarget_sheet(shape: &mut Shape<PaperPoint>, sheet: zumen_drawing::SheetId) {
    let pt = |p: &mut PaperPoint| p.sheet = sheet;
    match shape {
        Shape::Line { start, end } => {
            pt(start);
            pt(end);
        }
        Shape::Circle { center, .. } | Shape::Curve(zumen_drawing::Edge::Arc { center, .. }) => {
            pt(center)
        }
        Shape::Polyline(line) => {
            for v in &mut line.vertices {
                pt(&mut v.point);
            }
        }
        Shape::Point(dot) => pt(&mut dot.position),
        Shape::Text(t) => match &mut t.placement {
            zumen_drawing::Placement::Anchor { point, .. } => pt(point),
            zumen_drawing::Placement::Between { first, second, .. } => {
                pt(first);
                pt(second);
            }
        },
        Shape::Formatted(t) => pt(&mut t.point),
        Shape::Symbol(s) => pt(&mut s.position),
        _ => {}
    }
}

#[allow(clippy::type_complexity)]
fn add_copied_paper(
    plan: &mut Plan<'_>,
    drawing: &Drawing2D,
    tables: &Tables,
    element: &Element<PaperPoint>,
    orig_record: Option<&FeatureRecord>,
    cross_doc: bool,
    cx: &mut Cx,
) -> Result<Result<(u64, Element<PaperPoint>, Vec<ExpectedExt>), Refusal>, Error> {
    work(cx, tables.style_work(&element.style, None), &Ref::default())?;
    if let Shape::Symbol(symbol) = &element.shape {
        work(
            cx,
            drawing.symbols.len() as u64 + symbol.key.name.len() as u64,
            &Ref::default(),
        )?;
        let Some(definition) = drawing
            .symbols
            .iter()
            .find(|definition| definition.key == symbol.key)
        else {
            return Ok(Err(("symbol", "missing")));
        };
        let Some((instance, added)) = plan.add_symbol(element, tables, &definition.source, cx)?
        else {
            return Ok(Err(("kind", "unsupported")));
        };
        return Ok(Ok((instance, added, Vec::new())));
    }

    let (args, out_elem, expected_exts) = match prepare_paper(
        tables,
        element,
        orig_record,
        cross_doc,
        &mut plan.fonts,
        plan.writer.as_mut(),
        cx,
    )? {
        Ok(prepared) => prepared,
        Err(reason) => return Ok(Err(reason)),
    };
    let Some(instance) = plan.writer.add(&args, cx)? else {
        return Ok(Err(("kind", "unsupported")));
    };
    if args.name == text::NAME && orig_record.is_none() {
        plan.estimated_texts.push(instance);
    }
    Ok(Ok((instance, out_elem, expected_exts)))
}

pub(in crate::drawing) type Prepared = (super::Arguments, Element<PaperPoint>, Vec<ExpectedExt>);

pub(in crate::drawing) fn prepare_paper(
    tables: &Tables,
    element: &Element<PaperPoint>,
    orig_record: Option<&FeatureRecord>,
    cross_doc: bool,
    fonts: &mut super::Fonts<'_>,
    writer: &mut dyn super::Writer,
    cx: &mut Cx,
) -> Result<Result<Prepared, Refusal>, Error> {
    count_element_copy(element, cx, 1)?;
    count_shape(&element.shape, cx)?;
    text::count(&element.shape, cx)?;
    let mut placed = Placed::Paper(element.clone());
    // 別文書では字体番号の継承に元のレコードを使わず、文字幅と字間の計算だけに使う
    let Some(mut args) = (if cross_doc {
        shape_arguments_cross_doc(&mut placed, orig_record, fonts, writer, cx)?
    } else {
        shape_arguments(&mut placed, orig_record, fonts, writer, cx)?
    }) else {
        return Ok(Err(("kind", "unsupported")));
    };
    let Placed::Paper(mut out_elem) = placed else {
        return Ok(Err(("kind", "unsupported")));
    };
    let style = out_elem.style;
    if unset_field(&style, args.name == MARKER || args.name == text::NAME).is_some()
        || style.line_type_scale != 1.0
        || style.transparency != Inherit::Layer
    {
        return Ok(Err(("style", "unsupported")));
    }
    let Some((color, read)) = tables.color(style.color) else {
        return Ok(Err(("style", "color")));
    };
    out_elem.style.color = Inherit::Own(read);
    let codes = if args.name == MARKER || args.name == text::NAME {
        if !writer.markers()
            || style.line_type != Inherit::Layer
            || !matches!(style.line_weight, None | Some(Inherit::Layer))
        {
            return Ok(Err(("style", "unsupported")));
        }
        out_elem.style.line_weight = None;
        let Some(layer_code) = tables.layer(out_elem.layer) else {
            return Ok(Err(("layer", "missing")));
        };
        vec![layer_code, color]
    } else {
        let Some(layer_code) = tables.layer(out_elem.layer) else {
            return Ok(Err(("layer", "missing")));
        };
        let Some(lt_code) = tables.line_type(style.line_type) else {
            return Ok(Err(("line_type", "missing")));
        };
        let Some(lw) = style.line_weight.and_then(|w| tables.width(w)) else {
            return Ok(Err(("style", "line_weight")));
        };
        vec![layer_code, color, lt_code, lw]
    };
    for (at, code) in codes.iter().enumerate() {
        let Some(slot) = args.values.get_mut(at) else {
            return Ok(Err(("kind", "unsupported")));
        };
        *slot = code.to_string();
    }
    let mut expected_exts = Vec::new();
    if args.name == text::NAME {
        let (Some(w), Some(s)) = (
            args.values.get(7).and_then(|v| v.parse::<f64>().ok()),
            args.values.get(8).and_then(|v| v.parse::<f64>().ok()),
        ) else {
            return Ok(Err(("kind", "unsupported")));
        };
        expected_exts.push((
            "sxf.text".to_owned(),
            vec![
                Field {
                    key: "width".into(),
                    code: None,
                    value: FieldValue::Real(w),
                },
                Field {
                    key: "spacing".into(),
                    code: None,
                    value: FieldValue::Real(s),
                },
            ],
        ));
    }
    Ok(Ok((args, out_elem, expected_exts)))
}

fn shape_arguments_cross_doc(
    element: &mut Placed,
    orig_record: Option<&FeatureRecord>,
    fonts: &mut super::Fonts<'_>,
    writer: &mut dyn super::Writer,
    cx: &mut Cx,
) -> Result<Option<super::Arguments>, Error> {
    let Placed::Paper(elem) = element else {
        return Ok(None);
    };
    if matches!(elem.shape, Shape::Text(_) | Shape::Formatted(_)) {
        text::count(&elem.shape, cx)?;
        let Some(mut prepared) = text::prepare(&elem.shape, orig_record) else {
            return Ok(None);
        };
        let Some(font_name) = prepared.font.as_deref() else {
            return Ok(None);
        };
        let Some(code) = fonts.code(Some(font_name), None, writer, cx)? else {
            return Ok(None);
        };
        let Some(slot) = prepared.args.values.get_mut(2) else {
            return Ok(None);
        };
        *slot = code.to_string();
        elem.shape = prepared.shape;
        Ok(Some(prepared.args))
    } else {
        shape_arguments(element, None, fonts, writer, cx)
    }
}

/// 要素に属性付加機構（`sxf.attribute`、SAF の `ATRF` を含む）が付いていれば、その種類を返す
fn attribute_extension(drawing: &Drawing2D, id: &str) -> Option<&'static str> {
    for ext in &drawing.extensions {
        if ext.namespace == "sxf.attribute"
            && matches!(&ext.target, Target::Element(eid) if eid == id)
        {
            return Some(attribute_kind(&ext.fields));
        }
    }
    None
}

pub(in crate::drawing) fn attribute_kind(fields: &[Field]) -> &'static str {
    let is_saf = fields
        .iter()
        .filter(|f| f.key == "part")
        .find_map(|f| match &f.value {
            FieldValue::Text(s) => Some(s.as_str()),
            _ => None,
        })
        == Some("ATRF");
    if is_saf { "saf" } else { "attribute" }
}

/// 読み直した要素の拡張が、期待した拡張を含むかを確かめる
pub(super) fn extensions_match(drawing: &Drawing2D, id: &str, expected: &[ExpectedExt]) -> bool {
    for (ns, fields) in expected {
        let Some(actual) = drawing.extensions.iter().find(|ext| {
            ext.namespace.as_ref() == ns.as_str()
                && matches!(&ext.target, Target::Element(eid) if eid == id)
        }) else {
            return false;
        };
        if !fields_match(&actual.fields, fields) {
            return false;
        }
    }
    true
}

pub(in crate::drawing) fn fields_match(actual: &[Field], fields: &[Field]) -> bool {
    if actual.len() != fields.len() {
        return false;
    }
    for (fa, fb) in actual.iter().zip(fields) {
        if fa.key != fb.key || fa.code != fb.code || !field_near(&fa.value, &fb.value) {
            return false;
        }
    }
    true
}

fn field_near(a: &FieldValue, b: &FieldValue) -> bool {
    match (a, b) {
        (FieldValue::Real(x), FieldValue::Real(y)) => {
            (x - y).abs() <= 1e-9 * x.abs().max(y.abs()).max(1.0)
        }
        _ => a == b,
    }
}
