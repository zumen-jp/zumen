//! 単純な用紙要素からなる SFC の並列直接変換経路。
//!
//! 部分図・作図部品・複合曲線・属性ファイル・変換診断を含まない SFC 入力では、
//! 文字列引数配列の中間確保を省いてチャンク並列に [`SxfRow`] と [`SfcState`] を構築する。
//! 条件から外れる入力は `None` を返し、通常の [`Sxf::read`] 経路へそのまま委ねる。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Budget, Cx, Key, Kind, Limit, Ref};
use zumen_drawing::{
    Color, ColorCode, ColorSystem, Draft, Edge, Element, Field, FieldValue, Inherit, Layer,
    LayerId, LayerState, Length, LineType, LineTypeDefinition, LineTypeId, LineWeight, MarkerKey,
    PaperLength, PaperPoint, PatternBasis, Placed, Point, PointLook, Polyline, Position, Shape,
    Style, Vertex,
};
use zumen_live::{DepGraph, RowId, SourceBytes};
use zumen_sdk::Error;

use super::diagnostics::LiveDiagnostics;
use super::sfc::{BaseFeature, SfcSlot, SfcState};
use super::{
    BaseSheetSlot, PreparedBuild, SHEET, SheetTemplate, SmallId, SxfBacking, SxfContainer, SxfRow,
    USER_LINE_TYPE, intern_feature_meta_vec,
};
use crate::Marker;
use crate::drawing::apply::{SfcColors, line_end, newline};
use crate::drawing::{A_SIZES, FREE_SIZE, LINE_TYPES, LifterTables, WIDTHS};

const SXF_SCHEMA_VER: u32 = 2;
const PREDEFINED_COLORS: i64 = 16;
const USER_WIDTH: i64 = 11;

#[derive(Clone, Copy)]
enum GeomKind {
    Line,
    Polyline,
    Circle,
    Arc,
    PointMarker,
}

impl GeomKind {
    #[inline]
    const fn sxf_meta_slot(self) -> usize {
        match self {
            Self::Line => 1,
            Self::Polyline => 2,
            Self::Circle => 3,
            Self::Arc => 4,
            Self::PointMarker => 5,
        }
    }

    #[inline]
    const fn meta_bytes(self) -> u32 {
        match self {
            Self::Line => (b"line".len() + b"line_feature".len()) as u32,
            Self::Polyline => (b"polyline".len() + b"polyline_feature".len()) as u32,
            Self::Circle => (b"circle".len() + b"circle_feature".len()) as u32,
            Self::Arc => (b"arc".len() + b"arc_feature".len()) as u32,
            Self::PointMarker => (b"point_marker".len() + b"point_marker_feature".len()) as u32,
        }
    }
}

#[derive(Clone, Copy)]
struct GeomSpan {
    base_idx: u32,
    record_idx: u32,
    args_start: u32,
    args_end: u32,
    instance: u64,
    kind: GeomKind,
}

#[inline]
fn budget_has_room(budget: &Budget, kind: Kind, need: u64) -> bool {
    budget.exceeded().is_none()
        && match budget.limits().get(kind) {
            Limit::Unlimited => true,
            Limit::Finite(lim) => budget.used(kind).saturating_add(need) <= lim,
        }
}

#[inline]
pub(super) fn has_generous_budget(cx: &Cx<'_>, byte_len: usize) -> bool {
    if cx.cancel.stop().is_some() {
        return false;
    }
    let b = byte_len as u64;
    budget_has_room(
        cx.budget,
        Kind::InputBytes,
        b.saturating_mul(4).saturating_add(4096),
    ) && budget_has_room(
        cx.budget,
        Kind::DecodedBytes,
        b.saturating_mul(64).saturating_add(64 * 1024 * 1024),
    ) && budget_has_room(cx.budget, Kind::Records, b.saturating_add(4096))
        && budget_has_room(cx.budget, Kind::Elements, b.saturating_add(4096))
        && budget_has_room(
            cx.budget,
            Kind::Work,
            b.saturating_mul(64).saturating_add(1_000_000),
        )
        && budget_has_room(cx.budget, Kind::Depth, 16)
}

#[inline]
pub(super) fn small_id_sheet0_instance(inst: u64) -> SmallId {
    let mut v = inst;
    let mut digits = [0u8; 20];
    let mut dlen = 0usize;
    if v == 0 {
        if let Some(d) = digits.first_mut() {
            *d = b'0';
        }
        dlen = 1;
    } else {
        while v > 0 {
            if let Some(d) = digits.get_mut(dlen) {
                *d = b'0' + (v % 10) as u8;
            }
            v /= 10;
            dlen += 1;
        }
    }
    if 9 + dlen <= 22 {
        let mut buf = [0u8; 22];
        if let Some(prefix) = buf.get_mut(..9) {
            prefix.copy_from_slice(b"sheet/0/i");
        }
        if let (Some(dst), Some(src)) = (buf.get_mut(9..9 + dlen), digits.get(..dlen)) {
            for (out, &ch) in dst.iter_mut().zip(src.iter().rev()) {
                *out = ch;
            }
            return SmallId::Inline(buf, (9 + dlen) as u8);
        }
    }
    SmallId::from_string(format!("sheet/0/i{inst}"))
}

pub(super) fn try_open_fast_sfc(
    bytes: &[u8],
    threads: usize,
    cx: &mut Cx<'_>,
) -> Result<Option<PreparedBuild>, Error> {
    let Some((data_pos, mut record_idx)) = scan_sfc_header(bytes) else {
        return Ok(None);
    };

    let nl = newline(bytes);
    let mut sfc_meta: Vec<(Box<str>, Box<str>)> = Vec::with_capacity(16);
    let mut intern_sfc = |name_lower: &str| -> u16 {
        if let Some(p) = sfc_meta.iter().position(|(n, _)| n.as_ref() == name_lower) {
            p as u16
        } else {
            let idx = sfc_meta.len() as u16;
            let mut kind = String::with_capacity(name_lower.len() + 8);
            kind.push_str(name_lower);
            kind.push_str("_feature");
            sfc_meta.push((
                name_lower.to_owned().into_boxed_str(),
                kind.into_boxed_str(),
            ));
            idx
        }
    };
    let sfc_line_meta = intern_sfc("line");
    let sfc_poly_meta = intern_sfc("polyline");
    let sfc_circle_meta = intern_sfc("circle");
    let sfc_arc_meta = intern_sfc("arc");
    let sfc_marker_meta = intern_sfc("point_marker");

    let mut base_features: Vec<BaseFeature> = Vec::new();
    let mut geom_spans: Vec<GeomSpan> = Vec::new();
    let mut defined_colors = SfcColors::default();
    let mut layers: Vec<Layer> = Vec::new();
    let mut user_line_types: Vec<LineType> = Vec::new();
    let mut colors: Vec<Option<[u8; 3]>> = Vec::new();
    let mut widths: Vec<Option<f64>> = Vec::new();
    let mut fonts: Vec<String> = Vec::new();
    let mut sheet_info: Option<(usize, [PaperLength; 2], Draft)> = None;

    let mut pos = data_pos;
    let end = bytes.len();
    loop {
        pos = skip_ws(bytes, pos);
        let Some(rest) = bytes.get(pos..) else {
            return Ok(None);
        };
        if rest.is_empty() {
            return Ok(None);
        }
        if rest.starts_with(b"ENDSEC") {
            let after = skip_ws(bytes, pos + b"ENDSEC".len());
            if bytes.get(after) != Some(&b';') {
                return Ok(None);
            }
            if !verify_trailing_end(bytes, after + 1) {
                return Ok(None);
            }
            break;
        }

        let Some((marker, open, close)) = Marker::ALL
            .iter()
            .copied()
            .find(|(_, open, _)| rest.starts_with(open))
        else {
            return Ok(None);
        };
        let body_start = pos + open.len();
        let Some(rel_close) = bytes
            .get(body_start..end)
            .and_then(|s| find_close(s, close))
        else {
            return Ok(None);
        };
        let closing = body_start + rel_close;
        let span_start = pos;
        let span_end = closing + close.len();
        pos = span_end;

        let Some((inst, feat_name, args_start, args_end)) =
            parse_feature_head(bytes, body_start, closing)
        else {
            return Ok(None);
        };
        if inst == 0 || u32::try_from(record_idx).is_err() || u32::try_from(span_end).is_err() {
            return Ok(None);
        }

        let cur_rec = record_idx;
        record_idx += 1;
        let cur_base = base_features.len();
        if u32::try_from(cur_base).is_err() {
            return Ok(None);
        }
        let le = line_end(bytes, span_end);

        let (sfc_meta_idx, arg_count) = if feat_name.eq_ignore_ascii_case(b"line") {
            if sheet_info.is_some() {
                return Ok(None);
            }
            geom_spans.push(GeomSpan {
                base_idx: cur_base as u32,
                record_idx: cur_rec as u32,
                args_start: args_start as u32,
                args_end: args_end as u32,
                instance: inst,
                kind: GeomKind::Line,
            });
            (sfc_line_meta, 8)
        } else if feat_name.eq_ignore_ascii_case(b"polyline") {
            if sheet_info.is_some() {
                return Ok(None);
            }
            geom_spans.push(GeomSpan {
                base_idx: cur_base as u32,
                record_idx: cur_rec as u32,
                args_start: args_start as u32,
                args_end: args_end as u32,
                instance: inst,
                kind: GeomKind::Polyline,
            });
            (sfc_poly_meta, 7)
        } else if feat_name.eq_ignore_ascii_case(b"circle") {
            if sheet_info.is_some() {
                return Ok(None);
            }
            geom_spans.push(GeomSpan {
                base_idx: cur_base as u32,
                record_idx: cur_rec as u32,
                args_start: args_start as u32,
                args_end: args_end as u32,
                instance: inst,
                kind: GeomKind::Circle,
            });
            (sfc_circle_meta, 7)
        } else if feat_name.eq_ignore_ascii_case(b"arc") {
            if sheet_info.is_some() {
                return Ok(None);
            }
            geom_spans.push(GeomSpan {
                base_idx: cur_base as u32,
                record_idx: cur_rec as u32,
                args_start: args_start as u32,
                args_end: args_end as u32,
                instance: inst,
                kind: GeomKind::Arc,
            });
            (sfc_arc_meta, 10)
        } else if feat_name.eq_ignore_ascii_case(b"point_marker") {
            if sheet_info.is_some() {
                return Ok(None);
            }
            geom_spans.push(GeomSpan {
                base_idx: cur_base as u32,
                record_idx: cur_rec as u32,
                args_start: args_start as u32,
                args_end: args_end as u32,
                instance: inst,
                kind: GeomKind::PointMarker,
            });
            (sfc_marker_meta, 7)
        } else {
            let Some(raw_args) = bytes.get(args_start..args_end).and_then(split_sfc_args) else {
                return Ok(None);
            };
            let ac = u16::try_from(raw_args.len()).unwrap_or(u16::MAX);
            let source = Ref::at_record(cur_rec as u64).with_key(Key::Instance(inst));
            if feat_name.eq_ignore_ascii_case(b"layer") {
                let m = intern_sfc("layer");
                let Some(layer) = parse_layer_table(&raw_args, layers.len() + 1, source) else {
                    return Ok(None);
                };
                layers.push(layer);
                (m, ac)
            } else if feat_name.eq_ignore_ascii_case(b"pre_defined_colour") {
                if let Some(name) = arg_decoded_string(raw_args.first().copied()) {
                    defined_colors.record(&name);
                }
                (intern_sfc("pre_defined_colour"), ac)
            } else if feat_name.eq_ignore_ascii_case(b"pre_defined_font") {
                (intern_sfc("pre_defined_font"), ac)
            } else if feat_name.eq_ignore_ascii_case(b"user_defined_colour") {
                let m = intern_sfc("user_defined_colour");
                let Some(rgb) = parse_user_colour(&raw_args) else {
                    return Ok(None);
                };
                colors.push(Some(rgb));
                (m, ac)
            } else if feat_name.eq_ignore_ascii_case(b"user_defined_font") {
                let m = intern_sfc("user_defined_font");
                let code = USER_LINE_TYPE + 1 + user_line_types.len() as i64;
                let Some(lt) = parse_user_font(&raw_args, code, source) else {
                    return Ok(None);
                };
                user_line_types.push(lt);
                (m, ac)
            } else if feat_name.eq_ignore_ascii_case(b"width") {
                let m = intern_sfc("width");
                let Some(w) = arg_f64(raw_args.first().copied()).filter(|v| *v > 0.0) else {
                    return Ok(None);
                };
                widths.push(Some(w));
                (m, ac)
            } else if feat_name.eq_ignore_ascii_case(b"text_font") {
                let m = intern_sfc("text_font");
                let Some(font_name) = arg_decoded_string(raw_args.first().copied()) else {
                    return Ok(None);
                };
                fonts.push(font_name);
                (m, ac)
            } else if feat_name.eq_ignore_ascii_case(b"drawing_sheet") {
                if sheet_info.is_some() {
                    return Ok(None);
                }
                let m = intern_sfc("drawing_sheet");
                let Some((sz, draft)) = parse_sheet_table(&raw_args, source) else {
                    return Ok(None);
                };
                sheet_info = Some((cur_base, sz, draft));
                (m, ac)
            } else {
                return Ok(None);
            }
        };

        base_features.push(BaseFeature {
            record_idx: cur_rec,
            span_start,
            span_end,
            line_end: le,
            instance: inst,
            meta_idx: sfc_meta_idx,
            arg_count,
            marker,
            grouped: false,
            has_instance: true,
        });
    }

    let Some((first_sheet_base, sheet_size, sheet_draft)) = sheet_info else {
        return Ok(None);
    };
    if layers.is_empty() {
        return Ok(None);
    }

    // インスタンス番号の重複がある場合は通常経路へ委ねる
    {
        let mut seen_ids: Vec<u64> = base_features.iter().map(|bf| bf.instance).collect();
        if !seen_ids.is_sorted_by(|a, b| a < b) {
            seen_ids.sort_unstable();
            if seen_ids.windows(2).any(|w| matches!(w, [a, b] if a == b)) {
                return Ok(None);
            }
        }
    }

    // 予算消費（通常経路と整合する入力・レコード・作業量）
    let total_records = (record_idx + 2) as u64;
    cx.check(Kind::InputBytes, bytes.len() as u64)
        .map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
    cx.check(Kind::Records, total_records).map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })?;
    cx.check(
        Kind::Work,
        total_records.saturating_add(bytes.len() as u64 / 1024),
    )
    .map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })?;

    let src = SourceBytes::compress_slice_with_cx(bytes, threads.max(1), cx).map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })?;

    let defined_layers = layers.len();
    let user_lt_count = user_line_types.len();
    let worker_threads = threads.max(1).min(geom_spans.len().max(1));

    let parsed_chunks: Option<Vec<(Vec<SxfRow>, [u32; 15])>> =
        if worker_threads <= 1 || geom_spans.len() < 512 {
            parse_geom_chunk(
                bytes,
                &geom_spans,
                0,
                defined_layers,
                &colors,
                user_lt_count,
                &widths,
            )
            .map(|one| vec![one])
        } else {
            let chunk_size = geom_spans.len().div_ceil(worker_threads);
            std::thread::scope(|scope| {
                let mut handles = Vec::with_capacity(worker_threads);
                for (chunk_idx, slice) in geom_spans.chunks(chunk_size).enumerate() {
                    let global_offset = chunk_idx * chunk_size;
                    let colors_ref = &colors;
                    let widths_ref = &widths;
                    handles.push(scope.spawn(move || {
                        parse_geom_chunk(
                            bytes,
                            slice,
                            global_offset,
                            defined_layers,
                            colors_ref,
                            user_lt_count,
                            widths_ref,
                        )
                    }));
                }
                let mut out = Vec::with_capacity(handles.len());
                for h in handles {
                    match h.join() {
                        Ok(Some(res)) => out.push(res),
                        _ => return None,
                    }
                }
                Some(out)
            })
        };

    let Some(parsed_chunks) = parsed_chunks else {
        return Ok(None);
    };

    let mut first_predef_lt = [u32::MAX; 15];
    let mut rows = Vec::with_capacity(geom_spans.len());
    for (mut chunk_rows, chunk_lt) in parsed_chunks {
        for (dst, &ord) in first_predef_lt.iter_mut().zip(&chunk_lt) {
            if ord < *dst {
                *dst = ord;
            }
        }
        rows.append(&mut chunk_rows);
    }

    let base_layers = layers.clone();
    let base_line_types = user_line_types.clone();
    let mut predef_used: Vec<(u32, usize)> = first_predef_lt
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(i, ord)| (ord != u32::MAX).then_some((ord, i)))
        .collect();
    predef_used.sort_unstable_by_key(|&(ord, _)| ord);
    let mut all_line_types = user_line_types;
    for (_, i) in predef_used {
        let Some(&lt_name) = LINE_TYPES.get(i) else {
            return Ok(None);
        };
        all_line_types.push(LineType {
            id: LineTypeId((i + 1) as u32),
            name: lt_name.to_owned(),
            definition: None,
            source: Ref::default(),
        });
    }

    let mut feature_meta: Vec<(Box<str>, Box<str>)> = Vec::with_capacity(8);
    let _ = intern_feature_meta_vec(&mut feature_meta, "", "");
    let _ = intern_feature_meta_vec(&mut feature_meta, "line", "line_feature");
    let _ = intern_feature_meta_vec(&mut feature_meta, "polyline", "polyline_feature");
    let _ = intern_feature_meta_vec(&mut feature_meta, "circle", "circle_feature");
    let _ = intern_feature_meta_vec(&mut feature_meta, "arc", "arc_feature");
    let _ = intern_feature_meta_vec(&mut feature_meta, "point_marker", "point_marker_feature");

    let static_layer_ids: BTreeSet<u32> = base_layers
        .iter()
        .filter(|l| l.state.is_some())
        .map(|l| l.id.0)
        .collect();
    let static_user_lt_ids: BTreeSet<u32> = base_line_types
        .iter()
        .filter(|lt| i64::from(lt.id.0) > USER_LINE_TYPE && lt.source != Ref::default())
        .map(|lt| lt.id.0)
        .collect();

    let sfc_state = SfcState::from_fast(
        src,
        nl,
        sfc_meta,
        base_features,
        Some(first_sheet_base),
        defined_colors,
    );
    let backing = SxfBacking::Sfc(sfc_state);

    let row_count = rows.len();
    let mut base_sheet_slots = Vec::with_capacity(row_count);
    for i in 0..row_count {
        base_sheet_slots.push(BaseSheetSlot::Row(RowId(i as u32)));
    }
    let diagnostics = LiveDiagnostics::from_clean_sheet(row_count);

    let sheet_template = SheetTemplate {
        id: SHEET,
        name: None,
        size: Some(sheet_size),
        views: Vec::new(),
        source: Ref::default(),
    };
    let mut sheet_name_map = BTreeMap::new();
    sheet_name_map.insert("sheet/0".to_owned(), SHEET);

    let lifter_tables = LifterTables {
        defined_layers,
        base_layers,
        base_line_types,
        extra_line_types: Vec::new(),
        colors,
        widths,
        fonts,
    };
    let _ = all_line_types;

    Ok(Some(PreparedBuild {
        backing,
        attributes: None,
        diagnostics,
        lifter_tables,
        feature_meta,
        fonts_broken: false,
        static_layer_ids,
        static_user_lt_ids,
        raster_groups: Vec::new(),
        record_to_raster_group: BTreeMap::new(),
        space_templates: Vec::new(),
        space_rows: Vec::new(),
        symbol_templates: Vec::new(),
        symbol_rows: Vec::new(),
        sheet_template,
        base_sheet_slots,
        title_blocks: Vec::new(),
        doc_drafts: vec![sheet_draft],
        sheet_name_map,
        space_views: BTreeMap::new(),
        symbol_bounds: BTreeMap::new(),
        symbol_indices: BTreeMap::new(),
        space_spatial: BTreeMap::new(),
        symbol_dep_edges: Vec::<(RowId, BTreeSet<RowId>)>::new(),
        rows,
    }))
}

fn parse_geom_chunk(
    bytes: &[u8],
    slice: &[GeomSpan],
    global_offset: usize,
    defined_layers: usize,
    colors: &[Option<[u8; 3]>],
    user_lt_count: usize,
    widths: &[Option<f64>],
) -> Option<(Vec<SxfRow>, [u32; 15])> {
    let mut rows = Vec::with_capacity(slice.len());
    let mut first_predef_lt = [u32::MAX; 15];
    let mut poly_xs: Vec<f64> = Vec::new();
    let mut poly_ys: Vec<f64> = Vec::new();

    for (local_i, item) in slice.iter().enumerate() {
        let global_i = (global_offset + local_i) as u32;
        let raw = bytes.get(item.args_start as usize..item.args_end as usize)?;
        let mut cursor = 0usize;

        let source =
            Ref::at_record(u64::from(item.record_idx)).with_key(Key::Instance(item.instance));
        let elem = match item.kind {
            GeomKind::PointMarker => {
                let layer_code = next_sfc_i64(raw, &mut cursor, false)?;
                let color_code = next_sfc_i64(raw, &mut cursor, false)?;
                let px = next_sfc_f64(raw, &mut cursor, false)?;
                let py = next_sfc_f64(raw, &mut cursor, false)?;
                let mcode = next_sfc_i64(raw, &mut cursor, false)?;
                let rot = next_sfc_f64(raw, &mut cursor, false)?;
                let scale = next_sfc_f64(raw, &mut cursor, true)?;
                let layer = checked_layer(layer_code, defined_layers)?;
                let color = checked_color(color_code, colors)?;
                Element {
                    layer,
                    style: Style {
                        color: Inherit::Own(color),
                        line_type: Inherit::Layer,
                        line_type_scale: 1.0,
                        line_weight: None,
                        transparency: Inherit::Layer,
                    },
                    source,
                    shape: Shape::Point(Point {
                        position: PaperPoint {
                            sheet: SHEET,
                            x: px,
                            y: py,
                        },
                        rotation: rot,
                        look: PointLook::Marker {
                            key: MarkerKey {
                                namespace: "sxf.marker".into(),
                                code: mcode.to_string(),
                            },
                            scale,
                        },
                        construction: false,
                    }),
                }
            }
            GeomKind::Line => {
                let (layer, style) = next_curve_style(
                    raw,
                    &mut cursor,
                    defined_layers,
                    colors,
                    user_lt_count,
                    widths,
                    global_i,
                    &mut first_predef_lt,
                )?;
                let x1 = next_sfc_f64(raw, &mut cursor, false)?;
                let y1 = next_sfc_f64(raw, &mut cursor, false)?;
                let x2 = next_sfc_f64(raw, &mut cursor, false)?;
                let y2 = next_sfc_f64(raw, &mut cursor, true)?;
                Element {
                    layer,
                    style,
                    source,
                    shape: Shape::Line {
                        start: PaperPoint {
                            sheet: SHEET,
                            x: x1,
                            y: y1,
                        },
                        end: PaperPoint {
                            sheet: SHEET,
                            x: x2,
                            y: y2,
                        },
                    },
                }
            }
            GeomKind::Circle => {
                let (layer, style) = next_curve_style(
                    raw,
                    &mut cursor,
                    defined_layers,
                    colors,
                    user_lt_count,
                    widths,
                    global_i,
                    &mut first_predef_lt,
                )?;
                let cx_v = next_sfc_f64(raw, &mut cursor, false)?;
                let cy_v = next_sfc_f64(raw, &mut cursor, false)?;
                let r = next_sfc_f64(raw, &mut cursor, true)?;
                if r.partial_cmp(&0.0) != Some(core::cmp::Ordering::Greater) {
                    return None;
                }
                Element {
                    layer,
                    style,
                    source,
                    shape: Shape::Circle {
                        center: PaperPoint {
                            sheet: SHEET,
                            x: cx_v,
                            y: cy_v,
                        },
                        radius: Length::Paper(PaperLength(r)),
                    },
                }
            }
            GeomKind::Arc => {
                let (layer, style) = next_curve_style(
                    raw,
                    &mut cursor,
                    defined_layers,
                    colors,
                    user_lt_count,
                    widths,
                    global_i,
                    &mut first_predef_lt,
                )?;
                let cx_v = next_sfc_f64(raw, &mut cursor, false)?;
                let cy_v = next_sfc_f64(raw, &mut cursor, false)?;
                let r = next_sfc_f64(raw, &mut cursor, false)?;
                let dir = next_sfc_i64(raw, &mut cursor, false)?;
                let start = next_sfc_f64(raw, &mut cursor, false)?;
                let end = next_sfc_f64(raw, &mut cursor, true)?;
                if r.partial_cmp(&0.0) != Some(core::cmp::Ordering::Greater) {
                    return None;
                }
                Element {
                    layer,
                    style,
                    source,
                    shape: Shape::Curve(Edge::Arc {
                        center: PaperPoint {
                            sheet: SHEET,
                            x: cx_v,
                            y: cy_v,
                        },
                        radius: Length::Paper(PaperLength(r)),
                        start,
                        end,
                        counterclockwise: dir == 0,
                    }),
                }
            }
            GeomKind::Polyline => {
                let (layer, style) = next_curve_style(
                    raw,
                    &mut cursor,
                    defined_layers,
                    colors,
                    user_lt_count,
                    widths,
                    global_i,
                    &mut first_predef_lt,
                )?;
                let _n = next_sfc_i64(raw, &mut cursor, false)?;
                next_sfc_f64_list(raw, &mut cursor, false, &mut poly_xs)?;
                next_sfc_f64_list(raw, &mut cursor, true, &mut poly_ys)?;
                if poly_xs.len() != poly_ys.len() || poly_xs.len() < 2 {
                    return None;
                }
                let mut vertices = Vec::with_capacity(poly_xs.len());
                for (&x, &y) in poly_xs.iter().zip(&poly_ys) {
                    vertices.push(Vertex {
                        point: PaperPoint { sheet: SHEET, x, y },
                        bulge: 0.0,
                    });
                }
                Element {
                    layer,
                    style,
                    source,
                    shape: Shape::Polyline(Polyline {
                        vertices,
                        closed: false,
                        width: None,
                        vertex_widths: Vec::new(),
                        line_type_generation: false,
                    }),
                }
            }
        };

        rows.push(SxfRow {
            instance: Some(item.instance),
            unique_handle: Some(item.instance),
            record_idx: item.record_idx as usize,
            local_space_idx: 0,
            sfc_slot: Some(SfcSlot::Base(item.base_idx as usize)),
            meta_idx: item.kind.sxf_meta_slot() as u32,
            meta_bytes: item.kind.meta_bytes(),
            grouped: false,
            container: SxfContainer::Sheet(SHEET),
            canonical_id: small_id_sheet0_instance(item.instance),
            placed: Placed::Paper(elem),
            drafts: Box::new([]),
            diagnostics: Vec::new(),
        });
    }
    let _ = DepGraph::new;
    Some((rows, first_predef_lt))
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn next_curve_style(
    raw: &[u8],
    cursor: &mut usize,
    defined_layers: usize,
    colors: &[Option<[u8; 3]>],
    user_lt_count: usize,
    widths: &[Option<f64>],
    global_i: u32,
    first_predef_lt: &mut [u32; 15],
) -> Option<(LayerId, Style)> {
    let layer_code = next_sfc_i64(raw, cursor, false)?;
    let color_code = next_sfc_i64(raw, cursor, false)?;
    let lt_code = next_sfc_i64(raw, cursor, false)?;
    let w_code = next_sfc_i64(raw, cursor, false)?;

    let layer = checked_layer(layer_code, defined_layers)?;
    let color = checked_color(color_code, colors)?;
    let lt_id = if (1..=15).contains(&lt_code) {
        let slot = (lt_code - 1) as usize;
        let dst = first_predef_lt.get_mut(slot)?;
        if global_i < *dst {
            *dst = global_i;
        }
        LineTypeId(lt_code as u32)
    } else if lt_code > USER_LINE_TYPE && (lt_code - USER_LINE_TYPE) as usize <= user_lt_count {
        LineTypeId(lt_code as u32)
    } else {
        return None;
    };
    let width = if (1..=9).contains(&w_code) {
        *WIDTHS.get((w_code - 1) as usize)?
    } else if w_code >= USER_WIDTH {
        (*widths.get((w_code - USER_WIDTH) as usize)?)?
    } else {
        return None;
    };

    Some((
        layer,
        Style {
            color: Inherit::Own(color),
            line_type: Inherit::Own(lt_id),
            line_type_scale: 1.0,
            line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(width)))),
            transparency: Inherit::Layer,
        },
    ))
}

#[inline]
pub(super) fn checked_layer(code: i64, defined_layers: usize) -> Option<LayerId> {
    if code >= 1 && (code as usize) <= defined_layers {
        Some(LayerId(code as u32))
    } else {
        None
    }
}

#[inline]
pub(super) fn checked_color(code: i64, colors: &[Option<[u8; 3]>]) -> Option<Color> {
    let rgb = if (1..=PREDEFINED_COLORS).contains(&code) {
        None
    } else if code > PREDEFINED_COLORS {
        Some((*colors.get((code - PREDEFINED_COLORS - 1) as usize)?)?)
    } else {
        return None;
    };
    Some(Color {
        code: Some(ColorCode {
            system: ColorSystem::Sxf,
            value: code as u32,
        }),
        rgb,
    })
}

#[inline]
fn skip_ws(s: &[u8], mut p: usize) -> usize {
    while s.get(p).is_some_and(u8::is_ascii_whitespace) {
        p += 1;
    }
    p
}

#[inline]
fn next_sfc_raw_token<'a>(
    raw: &'a [u8],
    cursor: &mut usize,
    is_last: bool,
) -> Option<(&'a [u8], bool)> {
    let end = raw.len();
    let mut p = skip_ws(raw, *cursor);
    let &first = raw.get(p)?;
    let (tok, backslash_quoted) = if first == b'\'' {
        p += 1;
        let start = p;
        while raw.get(p).is_some_and(|&c| c != b'\'') {
            p += 1;
        }
        if p >= end {
            return None;
        }
        let s = raw.get(start..p)?;
        p += 1;
        (s, false)
    } else if raw.get(p..).is_some_and(|s| s.starts_with(b"\\'")) {
        p += 2;
        let start = p;
        let mut close_pos = None;
        while let Some(&b) = raw.get(p) {
            if matches!(b, 0x81..=0x9f | 0xe0..=0xfc) {
                p = (p + 2).min(end);
                continue;
            }
            if raw.get(p..).is_some_and(|s| s.starts_with(b"\\'")) {
                let after = skip_ws(raw, p + 2);
                if after == end || raw.get(after) == Some(&b',') {
                    close_pos = Some(p);
                    p += 2;
                    break;
                }
            }
            p += 1;
        }
        (raw.get(start..close_pos?)?, true)
    } else {
        return None;
    };

    p = skip_ws(raw, p);
    if is_last {
        if p != end {
            return None;
        }
    } else {
        if raw.get(p) != Some(&b',') {
            return None;
        }
        p += 1;
    }
    *cursor = p;
    Some((tok, backslash_quoted))
}

#[inline]
fn next_sfc_f64(raw: &[u8], cursor: &mut usize, is_last: bool) -> Option<f64> {
    let (tok, bsq) = next_sfc_raw_token(raw, cursor, is_last)?;
    if bsq || tok.iter().any(|b| matches!(b, b'\r' | b'\n')) {
        return None;
    }
    parse_trimmed_f64(tok)
}

#[inline]
fn next_sfc_i64(raw: &[u8], cursor: &mut usize, is_last: bool) -> Option<i64> {
    let v = next_sfc_f64(raw, cursor, is_last)?;
    (v.fract() == 0.0 && v.abs() < 9.0e15).then_some(v as i64)
}

#[inline]
fn next_sfc_f64_list(
    raw: &[u8],
    cursor: &mut usize,
    is_last: bool,
    out: &mut Vec<f64>,
) -> Option<()> {
    out.clear();
    let end = raw.len();
    let mut p = skip_ws(raw, *cursor);
    let &first = raw.get(p)?;
    if first == b'(' {
        p += 1;
        loop {
            p = skip_ws(raw, p);
            if raw.get(p) == Some(&b')') {
                p += 1;
                break;
            }
            let mut inner_cur = p;
            let (tok, bsq) = next_sfc_list_elem(raw, &mut inner_cur)?;
            p = inner_cur;
            if bsq || tok.iter().any(|b| matches!(b, b'\r' | b'\n')) {
                return None;
            }
            out.push(parse_trimmed_f64(tok)?);
            p = skip_ws(raw, p);
            match raw.get(p)? {
                b',' => p += 1,
                b')' => {
                    p += 1;
                    break;
                }
                _ => return None,
            }
        }
        p = skip_ws(raw, p);
        if is_last {
            if p != end {
                return None;
            }
        } else {
            if raw.get(p) != Some(&b',') {
                return None;
            }
            p += 1;
        }
        *cursor = p;
        Some(())
    } else {
        let (tok, bsq) = next_sfc_raw_token(raw, cursor, is_last)?;
        if bsq || tok.iter().any(|b| matches!(b, b'\r' | b'\n')) {
            return None;
        }
        let trimmed = trim_ascii(tok);
        let inner = trimmed.strip_prefix(b"(")?.strip_suffix(b")")?;
        let inner = trim_ascii(inner);
        if inner.is_empty() {
            return Some(());
        }
        for part in inner.split(|&b| b == b',') {
            out.push(parse_trimmed_f64(part)?);
        }
        Some(())
    }
}

#[inline]
fn next_sfc_list_elem<'a>(raw: &'a [u8], cursor: &mut usize) -> Option<(&'a [u8], bool)> {
    let end = raw.len();
    let mut p = skip_ws(raw, *cursor);
    if raw.get(p) != Some(&b'\'') {
        return None;
    }
    p += 1;
    let start = p;
    while raw.get(p).is_some_and(|&c| c != b'\'') {
        p += 1;
    }
    if p >= end {
        return None;
    }
    let s = raw.get(start..p)?;
    *cursor = p + 1;
    Some((s, false))
}

#[inline]
pub(super) fn trim_ascii(mut s: &[u8]) -> &[u8] {
    while let Some((first, rest)) = s.split_first() {
        if first.is_ascii_whitespace() {
            s = rest;
        } else {
            break;
        }
    }
    while let Some((last, rest)) = s.split_last() {
        if last.is_ascii_whitespace() {
            s = rest;
        } else {
            break;
        }
    }
    s
}

/// 15 桁以内の単純な 10 進表記は整数演算で厳密に `f64` 化し、それ以外は標準パーサーへ渡す。
#[inline]
pub(super) fn parse_trimmed_f64(bytes: &[u8]) -> Option<f64> {
    let s = trim_ascii(bytes);
    let (&first, after_sign) = s.split_first()?;
    let (neg, body) = match first {
        b'-' => (true, after_sign),
        b'+' => (false, after_sign),
        _ => (false, s),
    };
    if body.is_empty() {
        return None;
    }
    let mut mantissa: u64 = 0;
    let mut digits: u32 = 0;
    let mut frac_digits: u32 = 0;
    let mut seen_dot = false;
    let mut fast_ok = true;
    for &b in body {
        if b.is_ascii_digit() {
            let d = u64::from(b - b'0');
            if mantissa != 0 || d != 0 {
                digits += 1;
                if digits > 15 {
                    fast_ok = false;
                    break;
                }
                mantissa = mantissa * 10 + d;
            }
            if seen_dot {
                frac_digits += 1;
            }
        } else if b == b'.' && !seen_dot {
            seen_dot = true;
        } else {
            fast_ok = false;
            break;
        }
    }
    const POW10: [f64; 16] = [
        1.0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15,
    ];
    if fast_ok
        && body.len() > usize::from(seen_dot)
        && let Some(&scale) = POW10.get(frac_digits as usize)
    {
        let val = (mantissa as f64) / scale;
        return Some(if neg { -val } else { val });
    }
    core::str::from_utf8(s)
        .ok()?
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

#[derive(Clone, Copy)]
enum RawSfcArg<'a> {
    Plain(&'a [u8]),
    Backslash(&'a [u8]),
}

fn split_sfc_args(raw: &[u8]) -> Option<Vec<RawSfcArg<'_>>> {
    let mut out = Vec::new();
    let mut cur = 0usize;
    let trim = trim_ascii(raw);
    if trim.is_empty() {
        return Some(out);
    }
    loop {
        let p = skip_ws(raw, cur);
        let &first = raw.get(p)?;
        if first == b'(' {
            return None;
        }
        cur = p;
        let is_last_probe = {
            let mut tmp = cur;
            let _ = next_sfc_raw_token(raw, &mut tmp, true);
            tmp == raw.len()
        };
        let (tok, bsq) = next_sfc_raw_token(raw, &mut cur, is_last_probe)?;
        out.push(if bsq {
            RawSfcArg::Backslash(tok)
        } else {
            RawSfcArg::Plain(tok)
        });
        if is_last_probe {
            break;
        }
    }
    Some(out)
}

fn arg_decoded_string(arg: Option<RawSfcArg<'_>>) -> Option<String> {
    let (raw, bsq) = match arg? {
        RawSfcArg::Plain(r) => (r, false),
        RawSfcArg::Backslash(r) => (r, true),
    };
    if !bsq && raw.iter().all(|&b| b < 0x80 && b != b'\r' && b != b'\n') {
        return Some(String::from_utf8_lossy(raw).into_owned());
    }
    let mut buf = Vec::with_capacity(raw.len());
    let mut i = 0;
    while let Some(&b) = raw.get(i) {
        if matches!(b, b'\r' | b'\n') {
            i += 1;
            continue;
        }
        if matches!(b, 0x81..=0x9f | 0xe0..=0xfc) {
            buf.push(b);
            if let Some(&b2) = raw.get(i + 1) {
                buf.push(b2);
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if bsq && b == b'\\' && raw.get(i + 1) == Some(&b'\\') {
            buf.push(b'\\');
            i += 2;
            continue;
        }
        buf.push(b);
        i += 1;
    }
    let (text, _) = encoding_rs::SHIFT_JIS.decode_without_bom_handling(&buf);
    Some(text.into_owned())
}

fn arg_f64(arg: Option<RawSfcArg<'_>>) -> Option<f64> {
    let s = arg_decoded_string(arg)?;
    parse_trimmed_f64(s.as_bytes())
}

fn arg_i64(arg: Option<RawSfcArg<'_>>) -> Option<i64> {
    let v = arg_f64(arg)?;
    (v.fract() == 0.0 && v.abs() < 9.0e15).then_some(v as i64)
}

fn parse_layer_table(args: &[RawSfcArg<'_>], code: usize, source: Ref) -> Option<Layer> {
    let name = arg_decoded_string(args.first().copied())?;
    let visible = match arg_i64(args.get(1).copied()) {
        Some(flag) => flag != 0,
        None if args.len() < 2 => true,
        None => return None,
    };
    Some(Layer {
        id: LayerId(u32::try_from(code).ok()?),
        name,
        state: Some(LayerState {
            visible,
            printable: visible,
            editable: true,
            frozen: false,
        }),
        style: None,
        follows_placement: false,
        source,
        description: None,
    })
}

fn parse_user_colour(args: &[RawSfcArg<'_>]) -> Option<[u8; 3]> {
    let r = u8::try_from(arg_i64(args.first().copied())?).ok()?;
    let g = u8::try_from(arg_i64(args.get(1).copied())?).ok()?;
    let b = u8::try_from(arg_i64(args.get(2).copied())?).ok()?;
    Some([r, g, b])
}

fn parse_user_font(args: &[RawSfcArg<'_>], code: i64, source: Ref) -> Option<LineType> {
    let name = arg_decoded_string(args.first().copied())?;
    let pitch_str = arg_decoded_string(args.get(2).copied())?;
    let inner = pitch_str
        .trim()
        .strip_prefix('(')?
        .strip_suffix(')')?
        .trim();
    if inner.is_empty() {
        return None;
    }
    let mut pitch = Vec::new();
    for item in inner.split(',') {
        let v = parse_trimmed_f64(item.as_bytes()).filter(|v| *v > 0.0)?;
        pitch.push(v);
    }
    if pitch.is_empty() || pitch.len() % 2 != 0 {
        return None;
    }
    let definition = Some(LineTypeDefinition {
        description: String::new(),
        pattern: pitch
            .iter()
            .enumerate()
            .map(|(at, v)| if at % 2 == 0 { *v } else { -*v })
            .collect(),
        basis: PatternBasis::Paper,
        marks: Vec::new(),
    });
    Some(LineType {
        id: LineTypeId(u32::try_from(code).ok()?),
        name,
        definition,
        source,
    })
}

fn parse_sheet_table(args: &[RawSfcArg<'_>], source: Ref) -> Option<([PaperLength; 2], Draft)> {
    let name = arg_decoded_string(args.first().copied()).unwrap_or_default();
    let kind = arg_i64(args.get(1).copied())?;
    let orientation = arg_i64(args.get(2).copied()).unwrap_or(1);
    let size = match kind {
        FREE_SIZE => {
            let w = arg_f64(args.get(3).copied()).filter(|v| *v > 0.0)?;
            let h = arg_f64(args.get(4).copied()).filter(|v| *v > 0.0)?;
            [w, h]
        }
        _ => {
            let &[w, h] = A_SIZES.get(usize::try_from(kind).ok()?)?;
            if orientation == 1 { [h, w] } else { [w, h] }
        }
    };
    let mut fields = vec![
        Field {
            key: "size".into(),
            code: None,
            value: FieldValue::Integer(kind),
        },
        Field {
            key: "orientation".into(),
            code: None,
            value: FieldValue::Integer(orientation),
        },
    ];
    if kind == FREE_SIZE {
        fields.extend([
            Field {
                key: "width".into(),
                code: None,
                value: FieldValue::Real(size[0]),
            },
            Field {
                key: "height".into(),
                code: None,
                value: FieldValue::Real(size[1]),
            },
        ]);
    }
    let draft = Draft {
        namespace: "sxf.sheet".into(),
        name,
        version: None,
        schema: SXF_SCHEMA_VER,
        at: Position::Document,
        fields,
        source,
    };
    Some((size.map(PaperLength), draft))
}

fn parse_feature_head(
    bytes: &[u8],
    body_start: usize,
    closing: usize,
) -> Option<(u64, &[u8], usize, usize)> {
    let body = bytes.get(..closing)?;
    let mut p = skip_ws(body, body_start);
    if body.get(p) != Some(&b'#') {
        return None;
    }
    p += 1;
    let id_start = p;
    while body.get(p).is_some_and(u8::is_ascii_digit) {
        p += 1;
    }
    if p == id_start {
        return None;
    }
    let mut id = 0u64;
    for &b in body.get(id_start..p)? {
        id = id.checked_mul(10)?.checked_add(u64::from(b - b'0'))?;
    }
    p = skip_ws(body, p);
    if body.get(p) != Some(&b'=') {
        return None;
    }
    p = skip_ws(body, p + 1);
    let name_start = p;
    while body
        .get(p)
        .is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_')
    {
        p += 1;
    }
    let full_name = body.get(name_start..p)?;
    let stem_len = full_name.len().checked_sub(b"_feature".len())?;
    let (stem, suffix) = full_name.split_at(stem_len);
    if stem.is_empty() || !suffix.eq_ignore_ascii_case(b"_feature") {
        return None;
    }
    p = skip_ws(body, p);
    if body.get(p) != Some(&b'(') {
        return None;
    }
    let args_start = p + 1;
    let mut q = closing;
    while q > args_start && body.get(q - 1).is_some_and(u8::is_ascii_whitespace) {
        q -= 1;
    }
    if q <= args_start || body.get(q - 1) != Some(&b')') {
        return None;
    }
    let args_end = q - 1;
    Some((id, stem, args_start, args_end))
}

fn find_close(haystack: &[u8], close: &[u8]) -> Option<usize> {
    if close.is_empty() {
        return None;
    }
    haystack.windows(close.len()).position(|w| w == close)
}

/// `ISO-10303-21;` から `DATA;` 直後までを検証し、`DATA;` 直後の位置と直前までのレコード数を返す。
fn scan_sfc_header(bytes: &[u8]) -> Option<(usize, usize)> {
    let (pos, first_kw) = next_simple_stmt(bytes, 0)?;
    if first_kw != b"ISO-10303-21" {
        return None;
    }
    let (mut pos, second_kw) = next_simple_stmt(bytes, pos)?;
    if second_kw != b"HEADER" {
        return None;
    }
    let mut records = 2usize;
    let mut has_assoc_schema = false;
    let mut has_feature_mode = false;
    loop {
        let (next_pos, stmt_raw) = next_simple_stmt(bytes, pos)?;
        pos = next_pos;
        records += 1;
        if stmt_raw == b"ENDSEC" {
            break;
        }
        if stmt_raw.starts_with(b"FILE_SCHEMA") {
            if contains_ascii_ci(stmt_raw, b"ASSOCIATIVE_DRAUGHTING") {
                has_assoc_schema = true;
            } else {
                return None;
            }
        } else if stmt_raw.starts_with(b"FILE_DESCRIPTION") {
            if contains_ascii_ci(stmt_raw, b"AP202") {
                return None;
            }
            if contains_ascii_ci(stmt_raw, b"feature_mode") {
                has_feature_mode = true;
            }
        }
    }
    if !has_assoc_schema {
        return None;
    }
    let (data_pos, data_kw) = next_simple_stmt(bytes, pos)?;
    if data_kw != b"DATA" {
        return None;
    }
    records += 1;
    let _ = has_feature_mode;
    Some((data_pos, records))
}

pub(super) fn next_simple_stmt(bytes: &[u8], pos: usize) -> Option<(usize, &[u8])> {
    let mut pos = skip_ws(bytes, pos);
    let rest0 = bytes.get(pos..)?;
    if rest0.is_empty() || rest0.starts_with(b"/*") {
        return None;
    }
    let start = pos;
    let mut in_quote = false;
    while let Some(&b) = bytes.get(pos) {
        if !in_quote && bytes.get(pos..).is_some_and(|s| s.starts_with(b"/*")) {
            return None;
        }
        if b == b'\'' {
            in_quote = !in_quote;
        } else if b == b';' && !in_quote {
            let body = trim_ascii(bytes.get(start..pos)?);
            return Some((pos + 1, body));
        }
        pos += 1;
    }
    None
}

pub(super) fn verify_trailing_end(bytes: &[u8], pos: usize) -> bool {
    let Some((after, kw)) = next_simple_stmt(bytes, pos) else {
        return false;
    };
    if kw != b"END-ISO-10303-21" {
        return false;
    }
    skip_ws(bytes, after) == bytes.len()
}

pub(super) fn contains_ascii_ci(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|w| w.eq_ignore_ascii_case(needle))
}
