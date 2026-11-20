//! 単純な用紙曲線要素からなる P21（STEP AP202）の並列直接変換経路。
//!
//! 部分図・複合曲線・文字・寸法・ハッチング・変換診断を含まない P21 入力では、
//! 全実体の `Record` 木保持と文字列往復変換を省き、チャンク並列に [`SxfRow`] と [`P21State`] を構築する。
//! 条件から外れる入力は `None` を返し、通常の [`Sxf::read`] 経路へそのまま委ねる。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Cx, Key, Kind, Ref};
use zumen_drawing::{
    Draft, Edge, Element, Field, FieldValue, Inherit, Layer, LayerId, LayerState, Length,
    LineTypeId, LineWeight, PaperLength, PaperPoint, Placed, Polyline, Position, Shape, Style,
    Vertex,
};
use zumen_live::{RowId, SourceBytes};
use zumen_sdk::Error;

use super::diagnostics::LiveDiagnostics;
use super::fast_sfc::{
    checked_color, checked_layer, contains_ascii_ci, next_simple_stmt, parse_trimmed_f64,
    small_id_sheet0_instance, verify_trailing_end,
};
use super::p21::{
    BaseSpan, ContainerList, ContainerMembership, FastP21StateInit, InstanceIndex, P21State, Slot,
};
use super::{
    BaseSheetSlot, PreparedBuild, SHEET, SheetTemplate, SxfBacking, SxfContainer, SxfRow,
    intern_feature_meta_vec,
};
use crate::drawing::apply::p21::RefCounts;
use crate::drawing::apply::{line_end, newline};
use crate::drawing::{A_SIZES, LINE_TYPES, LifterTables, WIDTHS};
use crate::p21::{COLORS, Codes};

const SXF_SCHEMA_VER: u32 = 2;

#[derive(Clone, Copy)]
enum FastTrim {
    Ref(u64),
    Param(f64),
}

#[derive(Clone, Copy)]
enum FastInstKind {
    Empty,
    Aux0,
    Point([f64; 2]),
    Direction([f64; 2]),
    Vector {
        dir: u64,
        mag: f64,
    },
    Line {
        pt: u64,
        vec: u64,
    },
    Placement {
        pt: u64,
        dir: u64,
    },
    Circle {
        placement: u64,
        r: f64,
    },
    Trimmed {
        basis: u64,
        t1: FastTrim,
        t2: FastTrim,
        sense: bool,
    },
    Polyline {
        chunk_idx: u16,
        pts_start: u32,
        pts_len: u32,
    },
    PreColour(i64),
    RgbColour {
        sxf_predef: i64,
        rgb: [u8; 3],
    },
    PreFont(i64),
    LengthMeasure {
        mm: f64,
        unit_ref: u64,
        part_idx: u16,
    },
    CurveStyle {
        font: u64,
        width_mm: f64,
        width_ref: u64,
        colour: u64,
    },
    StyleAssign(u64),
    CurveOcc {
        style_assign: u64,
        geom: u64,
    },
    LayerUsage {
        layer: u64,
        sheet: u64,
    },
    Container,
}

#[derive(Clone, Copy)]
struct ParsedInst {
    id: u64,
    record_idx: u32,
    span_start: u32,
    span_end: u32,
    kind: FastInstKind,
}

enum ContainerKind {
    Layer {
        name: String,
        items: Box<[u64]>,
    },
    Sheet {
        size_name: String,
        items: Box<[u64]>,
    },
    Invisibility {
        items: Box<[u64]>,
    },
}

struct ParsedContainer {
    id: u64,
    record_idx: usize,
    slot: Slot,
    kind: ContainerKind,
}

struct WorkerChunk {
    insts: Vec<ParsedInst>,
    poly_pts: Vec<u64>,
    containers: Vec<ParsedContainer>,
    max_id: u64,
}

#[derive(Clone, Copy)]
struct IndexedEntity {
    record_idx: u32,
    kind: FastInstKind,
}

pub(super) fn try_open_fast_p21(
    bytes: &[u8],
    threads: usize,
    cx: &mut Cx<'_>,
) -> Result<Option<PreparedBuild>, Error> {
    let Some((header_spans, data_spans, tail_spans)) = scan_p21_spans(bytes) else {
        return Ok(None);
    };
    if data_spans.is_empty() {
        return Ok(None);
    }

    let header_len = header_spans.len();
    let total_records = header_len + data_spans.len() + tail_spans.len();
    if u32::try_from(total_records).is_err() {
        return Ok(None);
    }

    let worker_threads = threads.max(1).min(data_spans.len().max(1)).min(64);
    let chunks_opt: Option<Vec<WorkerChunk>> = if worker_threads <= 1 || data_spans.len() < 512 {
        parse_stmt_chunk(bytes, &data_spans, header_len, 0).map(|c| vec![c])
    } else {
        let chunk_size = data_spans.len().div_ceil(worker_threads);
        std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(worker_threads);
            for (c_idx, slice) in data_spans.chunks(chunk_size).enumerate() {
                let rec_base = header_len + c_idx * chunk_size;
                handles.push(
                    scope.spawn(move || parse_stmt_chunk(bytes, slice, rec_base, c_idx as u16)),
                );
            }
            let mut out = Vec::with_capacity(handles.len());
            for h in handles {
                match h.join() {
                    Ok(Some(c)) => out.push(c),
                    _ => return None,
                }
            }
            Some(out)
        })
    };

    let Some(chunks) = chunks_opt else {
        return Ok(None);
    };

    let mut max_id = 0u64;
    for c in &chunks {
        if c.max_id > max_id {
            max_id = c.max_id;
        }
    }
    let max_dense = (data_spans.len() as u64)
        .saturating_mul(8)
        .saturating_add(4096);
    if max_id == 0 || max_id > max_dense {
        return Ok(None);
    }
    let id_table_len = (max_id as usize) + 1;

    // 予算消費
    cx.check(Kind::InputBytes, bytes.len() as u64)
        .map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
    cx.check(Kind::Records, total_records as u64)
        .map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
    cx.check(
        Kind::Work,
        (total_records as u64).saturating_add(bytes.len() as u64 / 1024),
    )
    .map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })?;

    let src = SourceBytes::compress_slice_with_cx(bytes, threads.max(1), cx).map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })?;

    let mut by_id = vec![
        IndexedEntity {
            record_idx: u32::MAX,
            kind: FastInstKind::Empty,
        };
        id_table_len
    ];
    let mut base_spans: Vec<BaseSpan> = Vec::with_capacity(total_records);
    for &(s, e) in &header_spans {
        base_spans.push(BaseSpan::new(s as usize..e as usize, None, false));
    }

    let dense_cap = total_records
        .saturating_mul(4)
        .saturating_add(64)
        .max(id_table_len);
    let mut instances = InstanceIndex::with_capacity(dense_cap);
    let mut alive_id_counts = RefCounts::with_capacity(dense_cap);
    let mut sorted_base_ids: Vec<(u64, u32)> = Vec::with_capacity(data_spans.len());
    let mut last_base_instance = None;
    let mut length_measures: Vec<(u64, usize, usize)> = Vec::new();
    let mut all_containers: Vec<ParsedContainer> = Vec::new();
    let mut chunk_polys: Vec<Vec<u64>> = Vec::with_capacity(chunks.len());

    for chunk in chunks {
        for inst in chunk.insts {
            let Some(slot) = by_id.get_mut(inst.id as usize) else {
                return Ok(None);
            };
            if slot.record_idx != u32::MAX {
                // 重複 ID は通常経路へ委ねる
                return Ok(None);
            }
            *slot = IndexedEntity {
                record_idx: inst.record_idx,
                kind: inst.kind,
            };
            let span = inst.span_start as usize..inst.span_end as usize;
            let le = line_end(bytes, span.end);
            last_base_instance = Some((inst.record_idx as usize, span.clone(), le));
            base_spans.push(BaseSpan::new(span, Some(inst.id), true));
            instances.insert_first(inst.id, inst.record_idx as usize);
            alive_id_counts.add(inst.id);
            sorted_base_ids.push((inst.id, inst.record_idx));
            if let FastInstKind::LengthMeasure { part_idx, .. } = inst.kind {
                length_measures.push((inst.id, inst.record_idx as usize, part_idx as usize));
            }
        }
        all_containers.extend(chunk.containers);
        chunk_polys.push(chunk.poly_pts);
    }
    for &(s, e) in &tail_spans {
        base_spans.push(BaseSpan::new(s as usize..e as usize, None, false));
    }
    if !sorted_base_ids.is_sorted_by(|a, b| a <= b) {
        sorted_base_ids.sort_unstable();
    }
    length_measures.sort_unstable_by_key(|(id, _, _)| *id);

    // 参照検査と参照カウント構築
    let mut ref_counts = RefCounts::with_capacity(dense_cap);
    let mut non_container_refs = RefCounts::with_capacity(dense_cap);
    let mut check_and_add_non_container = |rid: u64| -> bool {
        if rid == 0
            || by_id
                .get(rid as usize)
                .is_none_or(|e| e.record_idx == u32::MAX)
        {
            return false;
        }
        ref_counts.add(rid);
        non_container_refs.add(rid);
        true
    };

    // `#id` 昇順で表（色・線種）も同時に確定する
    let mut color_codes: BTreeMap<u64, i64> = BTreeMap::new();
    let mut line_type_codes: BTreeMap<u64, i64> = BTreeMap::new();
    let mut user_colors: Vec<Option<[u8; 3]>> = Vec::new();
    let mut user_color_seq = 0i64;

    for (id_usize, ent) in by_id.iter().enumerate() {
        if ent.record_idx == u32::MAX {
            continue;
        }
        let id = id_usize as u64;
        match ent.kind {
            FastInstKind::Empty
            | FastInstKind::Aux0
            | FastInstKind::Point(_)
            | FastInstKind::Direction(_)
            | FastInstKind::Container => {}
            FastInstKind::PreColour(code) => {
                if code <= 0 {
                    return Ok(None);
                }
                color_codes.insert(id, code);
            }
            FastInstKind::RgbColour { sxf_predef, rgb } => {
                if sxf_predef > 0 {
                    color_codes.insert(id, sxf_predef);
                } else {
                    user_color_seq += 1;
                    user_colors.push(Some(rgb));
                    color_codes.insert(id, 16 + user_color_seq);
                }
            }
            FastInstKind::PreFont(code) => {
                line_type_codes.insert(id, code);
            }
            FastInstKind::Vector { dir, .. } => {
                if !check_and_add_non_container(dir) {
                    return Ok(None);
                }
            }
            FastInstKind::Line { pt, vec } => {
                if !check_and_add_non_container(pt) || !check_and_add_non_container(vec) {
                    return Ok(None);
                }
            }
            FastInstKind::Placement { pt, dir } => {
                if !check_and_add_non_container(pt)
                    || (dir != 0 && !check_and_add_non_container(dir))
                {
                    return Ok(None);
                }
            }
            FastInstKind::Circle { placement, .. } => {
                if !check_and_add_non_container(placement) {
                    return Ok(None);
                }
            }
            FastInstKind::Trimmed { basis, t1, t2, .. } => {
                if !check_and_add_non_container(basis) {
                    return Ok(None);
                }
                if let FastTrim::Ref(r1) = t1
                    && !check_and_add_non_container(r1)
                {
                    return Ok(None);
                }
                if let FastTrim::Ref(r2) = t2
                    && !check_and_add_non_container(r2)
                {
                    return Ok(None);
                }
            }
            FastInstKind::Polyline {
                chunk_idx,
                pts_start,
                pts_len,
            } => {
                let Some(pts) = chunk_polys
                    .get(chunk_idx as usize)
                    .and_then(|v| v.get(pts_start as usize..(pts_start + pts_len) as usize))
                else {
                    return Ok(None);
                };
                for &pid in pts {
                    if !check_and_add_non_container(pid) {
                        return Ok(None);
                    }
                }
            }
            FastInstKind::LengthMeasure { unit_ref, .. } => {
                if unit_ref != 0 && !check_and_add_non_container(unit_ref) {
                    return Ok(None);
                }
            }
            FastInstKind::CurveStyle {
                font,
                width_ref,
                colour,
                ..
            } => {
                if (font != 0 && !check_and_add_non_container(font))
                    || (width_ref != 0 && !check_and_add_non_container(width_ref))
                    || (colour != 0 && !check_and_add_non_container(colour))
                {
                    return Ok(None);
                }
            }
            FastInstKind::StyleAssign(st) => {
                if !check_and_add_non_container(st) {
                    return Ok(None);
                }
            }
            FastInstKind::CurveOcc { style_assign, geom } => {
                if !check_and_add_non_container(style_assign) || !check_and_add_non_container(geom)
                {
                    return Ok(None);
                }
            }
            FastInstKind::LayerUsage { layer, sheet } => {
                if !check_and_add_non_container(layer) || !check_and_add_non_container(sheet) {
                    return Ok(None);
                }
            }
        }
    }

    // コンテナーを `#id` 昇順に処理（`p21.rs` の `Builder::tables` / `Builder::sheet` と同じ順）
    all_containers.sort_unstable_by_key(|c| c.id);

    let mut invisible_ids: BTreeSet<u64> = BTreeSet::new();
    let mut container_lists: BTreeMap<Slot, ContainerList> = BTreeMap::new();
    let mut member_slots: Vec<Slot> = Vec::new();
    let mut member_pairs: Vec<(u64, u32)> = Vec::new();

    for c in &all_containers {
        if let ContainerKind::Invisibility { items } = &c.kind {
            for &rid in items.iter() {
                if rid == 0
                    || by_id
                        .get(rid as usize)
                        .is_none_or(|e| e.record_idx == u32::MAX)
                {
                    return Ok(None);
                }
                ref_counts.add(rid);
                invisible_ids.insert(rid);
            }
        }
    }

    type FoundSheet = (u64, usize, Slot, String, Box<[u64]>);
    let mut layer_of_occ = vec![0i64; id_table_len];
    let mut layers: Vec<Layer> = Vec::new();
    let mut layer_entity_ids: Vec<u64> = Vec::new();
    let mut sheet_found: Option<FoundSheet> = None;

    for c in all_containers {
        let slot = c.slot;
        match c.kind {
            ContainerKind::Invisibility { items } => {
                let (clist, unique_ids) = ContainerList::from_base_ids(items);
                let slot_idx = member_slots.len() as u32;
                member_slots.push(slot);
                member_pairs.reserve(unique_ids.len());
                for uid in unique_ids {
                    member_pairs.push((uid, slot_idx));
                }
                container_lists.insert(slot, clist);
            }
            ContainerKind::Layer { name, items } => {
                for &rid in items.iter() {
                    if rid == 0
                        || by_id
                            .get(rid as usize)
                            .is_none_or(|e| e.record_idx == u32::MAX)
                    {
                        return Ok(None);
                    }
                    ref_counts.add(rid);
                }
                if !name.starts_with("$$SXF_dummy") {
                    let code = (layers.len() + 1) as i64;
                    let visible = !invisible_ids.contains(&c.id);
                    let source = Ref::at_record(c.record_idx as u64).with_key(Key::Instance(c.id));
                    layers.push(Layer {
                        id: LayerId(code as u32),
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
                    });
                    layer_entity_ids.push(c.id);
                    for &rid in items.iter() {
                        let Some(cur) = layer_of_occ.get_mut(rid as usize) else {
                            return Ok(None);
                        };
                        if *cur == 0 {
                            *cur = code;
                        } else if *cur != code {
                            // 複数レイヤ所属（SXF_P21_LAYER_MEMBER_DROPPED）は通常経路へ委ねる
                            return Ok(None);
                        }
                    }
                }
                let (clist, unique_ids) = ContainerList::from_base_ids(items);
                let slot_idx = member_slots.len() as u32;
                member_slots.push(slot);
                member_pairs.reserve(unique_ids.len());
                for uid in unique_ids {
                    member_pairs.push((uid, slot_idx));
                }
                container_lists.insert(slot, clist);
            }
            ContainerKind::Sheet { size_name, items } => {
                if sheet_found.is_some() {
                    return Ok(None);
                }
                for &rid in items.iter() {
                    if rid == 0
                        || by_id
                            .get(rid as usize)
                            .is_none_or(|e| e.record_idx == u32::MAX)
                    {
                        return Ok(None);
                    }
                    ref_counts.add(rid);
                }
                let items_clone = items.clone();
                let (clist, unique_ids) = ContainerList::from_base_ids(items);
                let slot_idx = member_slots.len() as u32;
                member_slots.push(slot);
                member_pairs.reserve(unique_ids.len());
                for uid in unique_ids {
                    member_pairs.push((uid, slot_idx));
                }
                container_lists.insert(slot, clist);
                sheet_found = Some((c.id, c.record_idx, slot, size_name, items_clone));
            }
        }
    }

    let Some((sheet_id, sheet_rec_idx, sheet_slot, sheet_size_name, sheet_items)) = sheet_found
    else {
        return Ok(None);
    };
    if layers.is_empty() {
        return Ok(None);
    }

    // 用紙サイズ（A0〜A4 の縦横。自由サイズは通常経路へ委ねる）
    let Some((sheet_kind, sheet_orient, sheet_size)) = parse_p21_sheet_kind(sheet_size_name.trim())
    else {
        return Ok(None);
    };
    let sheet_draft = Draft {
        namespace: "sxf.sheet".into(),
        name: String::new(),
        version: None,
        schema: SXF_SCHEMA_VER,
        at: Position::Document,
        fields: vec![
            Field {
                key: "size".into(),
                code: None,
                value: FieldValue::Integer(sheet_kind),
            },
            Field {
                key: "orientation".into(),
                code: None,
                value: FieldValue::Integer(sheet_orient),
            },
        ],
        source: Ref::at_record(sheet_rec_idx as u64).with_key(Key::Instance(sheet_id)),
    };

    // 用紙上の重複要素があれば通常経路へ委ねる
    {
        let mut sorted_items = sheet_items.to_vec();
        if !sorted_items.is_sorted_by(|a, b| a < b) {
            sorted_items.sort_unstable();
            if sorted_items
                .windows(2)
                .any(|w| matches!(w, [a, b] if a == b))
            {
                return Ok(None);
            }
        }
    }

    let defined_layers = layers.len();
    let lift_threads = threads.max(1).min(sheet_items.len().max(1));
    let lifted_chunks: Option<Vec<(Vec<SxfRow>, [u32; 15])>> =
        if lift_threads <= 1 || sheet_items.len() < 512 {
            lift_occ_chunk(
                &sheet_items,
                0,
                &by_id,
                &chunk_polys,
                &layer_of_occ,
                &invisible_ids,
                defined_layers,
                &color_codes,
                &user_colors,
                &line_type_codes,
            )
            .map(|c| vec![c])
        } else {
            let chunk_size = sheet_items.len().div_ceil(lift_threads);
            std::thread::scope(|scope| {
                let mut handles = Vec::with_capacity(lift_threads);
                for (c_idx, slice) in sheet_items.chunks(chunk_size).enumerate() {
                    let global_offset = c_idx * chunk_size;
                    let by_id_ref = &by_id;
                    let polys_ref = &chunk_polys;
                    let layer_ref = &layer_of_occ;
                    let invis_ref = &invisible_ids;
                    let cc_ref = &color_codes;
                    let uc_ref = &user_colors;
                    let ltc_ref = &line_type_codes;
                    handles.push(scope.spawn(move || {
                        lift_occ_chunk(
                            slice,
                            global_offset,
                            by_id_ref,
                            polys_ref,
                            layer_ref,
                            invis_ref,
                            defined_layers,
                            cc_ref,
                            uc_ref,
                            ltc_ref,
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

    let Some(lifted_chunks) = lifted_chunks else {
        return Ok(None);
    };

    drop(by_id);
    drop(chunk_polys);
    drop(layer_of_occ);

    let mut first_predef_lt = [u32::MAX; 15];
    let mut rows = Vec::with_capacity(sheet_items.len());
    for (mut chunk_rows, chunk_lt) in lifted_chunks {
        for (dst, &ord) in first_predef_lt.iter_mut().zip(&chunk_lt) {
            if ord < *dst {
                *dst = ord;
            }
        }
        rows.append(&mut chunk_rows);
    }

    let membership = ContainerMembership::from_raw(member_slots, member_pairs);
    let codes = Codes {
        layers: layer_entity_ids,
        colors: color_codes,
        line_types: line_type_codes,
        fonts: Vec::new(),
    };
    let p21_state = P21State::from_fast(FastP21StateInit {
        src,
        newline: newline(bytes),
        base_spans,
        last_base_instance,
        instances,
        sorted_base_ids,
        alive_id_counts,
        ref_counts,
        container_lists,
        membership,
        non_container_refs,
        first_sheet: Some((sheet_id, sheet_slot)),
        length_measures,
        codes,
    });
    let backing = SxfBacking::P21(Box::new(p21_state));

    let mut feature_meta: Vec<(Box<str>, Box<str>)> = Vec::with_capacity(8);
    let _ = intern_feature_meta_vec(&mut feature_meta, "", "");
    let _ = intern_feature_meta_vec(&mut feature_meta, "line", "trimmed_curve");
    let _ = intern_feature_meta_vec(&mut feature_meta, "polyline", "polyline");
    let _ = intern_feature_meta_vec(&mut feature_meta, "circle", "circle");
    let _ = intern_feature_meta_vec(&mut feature_meta, "arc", "trimmed_curve");

    let static_layer_ids: BTreeSet<u32> = layers
        .iter()
        .filter(|l| l.state.is_some())
        .map(|l| l.id.0)
        .collect();

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
        base_layers: layers,
        base_line_types: Vec::new(),
        extra_line_types: Vec::new(),
        colors: user_colors,
        widths: Vec::new(),
        fonts: Vec::new(),
    };
    let _ = (first_predef_lt, LINE_TYPES);

    Ok(Some(PreparedBuild {
        backing,
        attributes: None,
        diagnostics,
        lifter_tables,
        feature_meta,
        fonts_broken: false,
        static_layer_ids,
        static_user_lt_ids: BTreeSet::new(),
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
        symbol_dep_edges: Vec::new(),
        rows,
    }))
}

#[allow(clippy::too_many_arguments)]
fn lift_occ_chunk(
    occ_ids: &[u64],
    global_offset: usize,
    by_id: &[IndexedEntity],
    chunk_polys: &[Vec<u64>],
    layer_of_occ: &[i64],
    invisible_ids: &BTreeSet<u64>,
    defined_layers: usize,
    color_codes: &BTreeMap<u64, i64>,
    user_colors: &[Option<[u8; 3]>],
    line_type_codes: &BTreeMap<u64, i64>,
) -> Option<(Vec<SxfRow>, [u32; 15])> {
    let mut rows = Vec::with_capacity(occ_ids.len());
    let mut first_predef_lt = [u32::MAX; 15];

    for (local_i, &occ_id) in occ_ids.iter().enumerate() {
        if invisible_ids.contains(&occ_id) {
            return None;
        }
        let global_i = (global_offset + local_i) as u32;
        let occ_ent = *by_id.get(occ_id as usize)?;
        let FastInstKind::CurveOcc { style_assign, geom } = occ_ent.kind else {
            return None;
        };

        let layer_code = *layer_of_occ.get(occ_id as usize)?;
        let layer = checked_layer(layer_code, defined_layers)?;

        let sa_ent = *by_id.get(style_assign as usize)?;
        let FastInstKind::StyleAssign(cs_id) = sa_ent.kind else {
            return None;
        };
        let cs_ent = *by_id.get(cs_id as usize)?;
        let FastInstKind::CurveStyle {
            font,
            width_mm,
            width_ref,
            colour,
        } = cs_ent.kind
        else {
            return None;
        };

        let color_code = if colour == 0 {
            8
        } else {
            *color_codes.get(&colour)?
        };
        let color = checked_color(color_code, user_colors)?;

        let lt_code = if font == 0 {
            1
        } else {
            *line_type_codes.get(&font)?
        };
        if !(1..=15).contains(&lt_code) {
            return None;
        }
        let lt_slot = (lt_code - 1) as usize;
        let lt_dst = first_predef_lt.get_mut(lt_slot)?;
        if global_i < *lt_dst {
            *lt_dst = global_i;
        }
        let lt_id = LineTypeId(lt_code as u32);

        let resolved_mm = if width_ref != 0 {
            match by_id.get(width_ref as usize)?.kind {
                FastInstKind::LengthMeasure { mm, .. } => mm,
                _ => return None,
            }
        } else {
            width_mm
        };
        let w_pos = WIDTHS
            .iter()
            .position(|known| (known - resolved_mm).abs() < 1e-6)?;
        let width_val = *WIDTHS.get(w_pos)?;

        let style = Style {
            color: Inherit::Own(color),
            line_type: Inherit::Own(lt_id),
            line_type_scale: 1.0,
            line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(width_val)))),
            transparency: Inherit::Layer,
        };

        let geom_ent = *by_id.get(geom as usize)?;
        let (meta_idx, meta_bytes, shape) = match geom_ent.kind {
            FastInstKind::Polyline {
                chunk_idx,
                pts_start,
                pts_len,
            } => {
                if pts_len < 2 {
                    return None;
                }
                let pts = chunk_polys
                    .get(chunk_idx as usize)?
                    .get(pts_start as usize..(pts_start + pts_len) as usize)?;
                let mut vertices = Vec::with_capacity(pts.len());
                for &pid in pts {
                    let [x, y] = get_point(by_id, pid)?;
                    vertices.push(Vertex {
                        point: PaperPoint { sheet: SHEET, x, y },
                        bulge: 0.0,
                    });
                }
                (
                    2u32,
                    (b"polyline".len() + b"polyline".len()) as u32,
                    Shape::Polyline(Polyline {
                        vertices,
                        closed: false,
                        width: None,
                        vertex_widths: Vec::new(),
                        line_type_generation: false,
                    }),
                )
            }
            FastInstKind::Circle { placement, r } => {
                if r.partial_cmp(&0.0) != Some(core::cmp::Ordering::Greater) {
                    return None;
                }
                let (cx_v, cy_v, _) = get_placement(by_id, placement)?;
                (
                    3u32,
                    (b"circle".len() + b"circle".len()) as u32,
                    Shape::Circle {
                        center: PaperPoint {
                            sheet: SHEET,
                            x: cx_v,
                            y: cy_v,
                        },
                        radius: Length::Paper(PaperLength(r)),
                    },
                )
            }
            FastInstKind::Trimmed {
                basis,
                t1,
                t2,
                sense,
            } => match by_id.get(basis as usize)?.kind {
                FastInstKind::Line { pt, vec } => {
                    let origin = get_point(by_id, pt)?;
                    let (dir, mag) = match by_id.get(vec as usize)?.kind {
                        FastInstKind::Vector { dir, mag } => (get_direction(by_id, dir)?, mag),
                        _ => return None,
                    };
                    let length = libm::hypot(dir[0], dir[1]);
                    let eval_end = |tr: FastTrim| -> Option<[f64; 2]> {
                        match tr {
                            FastTrim::Ref(pid) => get_point(by_id, pid),
                            FastTrim::Param(t) if length > 0.0 => {
                                let x = origin[0] + dir[0] / length * mag * t;
                                let y = origin[1] + dir[1] / length * mag * t;
                                (x.is_finite() && y.is_finite()).then_some([x, y])
                            }
                            _ => None,
                        }
                    };
                    let [x1, y1] = eval_end(t1)?;
                    let [x2, y2] = eval_end(t2)?;
                    (
                        1u32,
                        (b"line".len() + b"trimmed_curve".len()) as u32,
                        Shape::Line {
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
                    )
                }
                FastInstKind::Circle { placement, r } => {
                    if r.partial_cmp(&0.0) != Some(core::cmp::Ordering::Greater) {
                        return None;
                    }
                    let (cx_v, cy_v, rot_deg) = get_placement(by_id, placement)?;
                    let eval_angle = |tr: FastTrim| -> Option<f64> {
                        let a = match tr {
                            FastTrim::Param(t) => p21_degrees(t),
                            FastTrim::Ref(pid) => {
                                let [px, py] = get_point(by_id, pid)?;
                                p21_degrees(libm::atan2(py - cy_v, px - cx_v)) - rot_deg
                            }
                        };
                        let turned = a + rot_deg;
                        turned.is_finite().then_some(turned)
                    };
                    let start = eval_angle(t1)?;
                    let end = eval_angle(t2)?;
                    (
                        4u32,
                        (b"arc".len() + b"trimmed_curve".len()) as u32,
                        Shape::Curve(Edge::Arc {
                            center: PaperPoint {
                                sheet: SHEET,
                                x: cx_v,
                                y: cy_v,
                            },
                            radius: Length::Paper(PaperLength(r)),
                            start,
                            end,
                            counterclockwise: sense,
                        }),
                    )
                }
                _ => return None,
            },
            _ => return None,
        };

        let source = Ref::at_record(u64::from(occ_ent.record_idx)).with_key(Key::Instance(occ_id));
        rows.push(SxfRow {
            instance: Some(occ_id),
            unique_handle: Some(occ_id),
            record_idx: occ_ent.record_idx as usize,
            local_space_idx: 0,
            sfc_slot: None,
            meta_idx,
            meta_bytes,
            grouped: false,
            container: SxfContainer::Sheet(SHEET),
            canonical_id: small_id_sheet0_instance(occ_id),
            placed: Placed::Paper(Element {
                layer,
                style,
                source,
                shape,
            }),
            drafts: Box::new([]),
            diagnostics: Vec::new(),
        });
    }

    Some((rows, first_predef_lt))
}

#[inline]
fn p21_degrees(radians: f64) -> f64 {
    radians * 180.0 / core::f64::consts::PI
}

#[inline]
fn get_point(by_id: &[IndexedEntity], id: u64) -> Option<[f64; 2]> {
    match by_id.get(id as usize)?.kind {
        FastInstKind::Point(p) => Some(p),
        _ => None,
    }
}

#[inline]
fn get_direction(by_id: &[IndexedEntity], id: u64) -> Option<[f64; 2]> {
    match by_id.get(id as usize)?.kind {
        FastInstKind::Direction(d) => Some(d),
        _ => None,
    }
}

#[inline]
fn get_placement(by_id: &[IndexedEntity], id: u64) -> Option<(f64, f64, f64)> {
    let FastInstKind::Placement { pt, dir } = by_id.get(id as usize)?.kind else {
        return None;
    };
    let [x, y] = get_point(by_id, pt)?;
    let [dx, dy] = if dir == 0 {
        [1.0, 0.0]
    } else {
        get_direction(by_id, dir)?
    };
    let len = libm::hypot(dx, dy);
    if len == 0.0 {
        return None;
    }
    Some((x, y, p21_degrees(libm::atan2(dy, dx))))
}

fn parse_stmt_chunk(
    bytes: &[u8],
    spans: &[(u32, u32)],
    rec_base: usize,
    chunk_idx: u16,
) -> Option<WorkerChunk> {
    let mut insts = Vec::with_capacity(spans.len());
    let mut poly_pts: Vec<u64> = Vec::new();
    let mut containers: Vec<ParsedContainer> = Vec::new();
    let mut max_id = 0u64;

    for (local_i, &(s_u32, e_u32)) in spans.iter().enumerate() {
        let record_idx = (rec_base + local_i) as u32;
        let stmt = bytes.get(s_u32 as usize..e_u32 as usize)?;
        let (id, name, args_raw) = parse_p21_simple_head(stmt)?;
        if id == 0 {
            return None;
        }
        if id > max_id {
            max_id = id;
        }

        let kind = if name.eq_ignore_ascii_case(b"CARTESIAN_POINT") {
            FastInstKind::Point(parse_point_or_dir_args(args_raw)?)
        } else if name.eq_ignore_ascii_case(b"ANNOTATION_CURVE_OCCURRENCE") {
            let (sa, geom) = parse_curve_occ_args(args_raw)?;
            FastInstKind::CurveOcc {
                style_assign: sa,
                geom,
            }
        } else if name.eq_ignore_ascii_case(b"PRESENTATION_STYLE_ASSIGNMENT") {
            FastInstKind::StyleAssign(parse_style_assign_args(args_raw)?)
        } else if name.eq_ignore_ascii_case(b"CURVE_STYLE") {
            parse_curve_style_args(args_raw)?
        } else if name.eq_ignore_ascii_case(b"TRIMMED_CURVE") {
            parse_trimmed_curve_args(args_raw)?
        } else if name.eq_ignore_ascii_case(b"CIRCLE") {
            let (pl, r) = parse_circle_args(args_raw)?;
            FastInstKind::Circle { placement: pl, r }
        } else if name.eq_ignore_ascii_case(b"AXIS2_PLACEMENT_2D") {
            let (pt, dir) = parse_placement_args(args_raw)?;
            FastInstKind::Placement { pt, dir }
        } else if name.eq_ignore_ascii_case(b"POLYLINE") {
            let pts_start = u32::try_from(poly_pts.len()).ok()?;
            parse_polyline_args(args_raw, &mut poly_pts)?;
            let pts_len = u32::try_from(poly_pts.len()).ok()? - pts_start;
            FastInstKind::Polyline {
                chunk_idx,
                pts_start,
                pts_len,
            }
        } else if name.eq_ignore_ascii_case(b"LINE") {
            let (pt, vec) = parse_line_args(args_raw)?;
            FastInstKind::Line { pt, vec }
        } else if name.eq_ignore_ascii_case(b"VECTOR") {
            let (dir, mag) = parse_vector_args(args_raw)?;
            FastInstKind::Vector { dir, mag }
        } else if name.eq_ignore_ascii_case(b"DIRECTION") {
            FastInstKind::Direction(parse_point_or_dir_args(args_raw)?)
        } else if name.eq_ignore_ascii_case(b"DRAUGHTING_PRE_DEFINED_COLOUR") {
            let cname = parse_single_ascii_string(args_raw)?;
            let code = COLORS
                .iter()
                .position(|known| known.eq_ignore_ascii_case(cname.trim()))
                .map_or(0, |at| at as i64 + 1);
            FastInstKind::PreColour(code)
        } else if name.eq_ignore_ascii_case(b"COLOUR_RGB") {
            parse_rgb_colour_args(args_raw)?
        } else if name.eq_ignore_ascii_case(b"DRAUGHTING_PRE_DEFINED_CURVE_FONT") {
            let fname = parse_single_ascii_string(args_raw)?;
            let code = LINE_TYPES
                .iter()
                .position(|known| known.eq_ignore_ascii_case(fname.trim()))
                .map_or(1, |at| at as i64 + 1);
            FastInstKind::PreFont(code)
        } else if name.eq_ignore_ascii_case(b"LENGTH_MEASURE_WITH_UNIT") {
            let (mm, unit_ref) = parse_length_measure_args(args_raw)?;
            FastInstKind::LengthMeasure {
                mm,
                unit_ref,
                part_idx: 0,
            }
        } else if name.eq_ignore_ascii_case(b"PRESENTATION_LAYER_ASSIGNMENT") {
            let (lname, items) = parse_layer_assignment_args(args_raw)?;
            containers.push(ParsedContainer {
                id,
                record_idx: record_idx as usize,
                slot: (record_idx as usize, 0, 2),
                kind: ContainerKind::Layer { name: lname, items },
            });
            FastInstKind::Container
        } else if name.eq_ignore_ascii_case(b"PRESENTATION_LAYER_USAGE") {
            let (layer, sheet) = parse_two_refs(args_raw)?;
            FastInstKind::LayerUsage { layer, sheet }
        } else if name.eq_ignore_ascii_case(b"INVISIBILITY") {
            let items = parse_single_ref_list(args_raw)?;
            containers.push(ParsedContainer {
                id,
                record_idx: record_idx as usize,
                slot: (record_idx as usize, 0, 0),
                kind: ContainerKind::Invisibility { items },
            });
            FastInstKind::Container
        } else if name.eq_ignore_ascii_case(b"DRAWING_SHEET_REVISION") {
            let (sname, items) = parse_sheet_revision_args(args_raw)?;
            containers.push(ParsedContainer {
                id,
                record_idx: record_idx as usize,
                slot: (record_idx as usize, 0, 1),
                kind: ContainerKind::Sheet {
                    size_name: sname,
                    items,
                },
            });
            FastInstKind::Container
        } else if name.eq_ignore_ascii_case(b"DIMENSIONAL_EXPONENTS") {
            if args_raw.contains(&b'#') {
                return None;
            }
            FastInstKind::Aux0
        } else {
            return None;
        };

        insts.push(ParsedInst {
            id,
            record_idx,
            span_start: s_u32,
            span_end: e_u32,
            kind,
        });
    }

    Some(WorkerChunk {
        insts,
        poly_pts,
        containers,
        max_id,
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
fn parse_p21_simple_head(stmt: &[u8]) -> Option<(u64, &[u8], &[u8])> {
    let end = stmt.len();
    let mut p = skip_ws(stmt, 0);
    if stmt.get(p) != Some(&b'#') {
        return None;
    }
    p += 1;
    let id_start = p;
    while stmt.get(p).is_some_and(u8::is_ascii_digit) {
        p += 1;
    }
    if p == id_start {
        return None;
    }
    let mut id = 0u64;
    for &b in stmt.get(id_start..p)? {
        id = id.checked_mul(10)?.checked_add(u64::from(b - b'0'))?;
    }
    p = skip_ws(stmt, p);
    if stmt.get(p) != Some(&b'=') {
        return None;
    }
    p = skip_ws(stmt, p + 1);
    let name_start = p;
    while stmt
        .get(p)
        .is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_')
    {
        p += 1;
    }
    if p == name_start {
        return None;
    }
    let name = stmt.get(name_start..p)?;
    p = skip_ws(stmt, p);
    if stmt.get(p) != Some(&b'(') {
        return None;
    }
    let args_start = p + 1;
    let mut q = end;
    while q > args_start && stmt.get(q - 1).is_some_and(u8::is_ascii_whitespace) {
        q -= 1;
    }
    if stmt.get(q.checked_sub(1)?) != Some(&b';') {
        return None;
    }
    q -= 1;
    while q > args_start && stmt.get(q - 1).is_some_and(u8::is_ascii_whitespace) {
        q -= 1;
    }
    if q <= args_start || stmt.get(q - 1) != Some(&b')') {
        return None;
    }
    Some((id, name, stmt.get(args_start..q - 1)?))
}

#[inline]
fn skip_p21_string(raw: &[u8], cursor: &mut usize) -> Option<RangeSlice> {
    let mut p = skip_ws(raw, *cursor);
    if raw.get(p) != Some(&b'\'') {
        return None;
    }
    p += 1;
    let start = p;
    let mut has_escape = false;
    while let Some(&b) = raw.get(p) {
        if b == b'\\' || b >= 0x80 {
            has_escape = true;
        }
        if b == b'\'' {
            if raw.get(p + 1) == Some(&b'\'') {
                has_escape = true;
                p += 2;
                continue;
            }
            let inner_end = p;
            *cursor = p + 1;
            return Some(RangeSlice {
                start,
                end: inner_end,
                has_escape,
            });
        }
        p += 1;
    }
    None
}

struct RangeSlice {
    start: usize,
    end: usize,
    has_escape: bool,
}

fn decode_p21_string(raw: &[u8], cursor: &mut usize) -> Option<String> {
    let rs = skip_p21_string(raw, cursor)?;
    let slice = raw.get(rs.start..rs.end)?;
    if !rs.has_escape {
        return Some(String::from_utf8_lossy(slice).into_owned());
    }
    Some(zumen_step::decode(slice, |rest| {
        let mut len = 0;
        while let Some(&b) = rest.get(len) {
            if b < 0x80 {
                break;
            }
            len += if matches!(b, 0x81..=0x9f | 0xe0..=0xfc) {
                2
            } else {
                1
            };
        }
        let run = rest.get(..len.min(rest.len())).unwrap_or_default();
        let (text, _) = encoding_rs::SHIFT_JIS.decode_without_bom_handling(run);
        (text.into_owned(), run.len())
    }))
}

#[inline]
fn consume_comma(raw: &[u8], cursor: &mut usize) -> Option<()> {
    let p = skip_ws(raw, *cursor);
    if raw.get(p) != Some(&b',') {
        return None;
    }
    *cursor = p + 1;
    Some(())
}

#[inline]
fn ensure_end(raw: &[u8], cursor: usize) -> Option<()> {
    (skip_ws(raw, cursor) == raw.len()).then_some(())
}

#[inline]
fn next_ref(raw: &[u8], cursor: &mut usize) -> Option<u64> {
    let mut p = skip_ws(raw, *cursor);
    if raw.get(p) != Some(&b'#') {
        return None;
    }
    p += 1;
    let start = p;
    while raw.get(p).is_some_and(u8::is_ascii_digit) {
        p += 1;
    }
    if p == start {
        return None;
    }
    let mut id = 0u64;
    for &b in raw.get(start..p)? {
        id = id.checked_mul(10)?.checked_add(u64::from(b - b'0'))?;
    }
    *cursor = p;
    (id > 0).then_some(id)
}

#[inline]
fn next_number(raw: &[u8], cursor: &mut usize) -> Option<f64> {
    let mut p = skip_ws(raw, *cursor);
    let start = p;
    while raw
        .get(p)
        .is_some_and(|&b| !matches!(b, b',' | b')') && !b.is_ascii_whitespace())
    {
        p += 1;
    }
    if p == start {
        return None;
    }
    *cursor = p;
    parse_trimmed_f64(raw.get(start..p)?)
}

#[inline]
fn parse_point_or_dir_args(raw: &[u8]) -> Option<[f64; 2]> {
    let mut c = 0usize;
    let _ = skip_p21_string(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    c = skip_ws(raw, c);
    if raw.get(c) != Some(&b'(') {
        return None;
    }
    c += 1;
    let x = next_number(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    let y = next_number(raw, &mut c)?;
    c = skip_ws(raw, c);
    if raw.get(c) == Some(&b',') {
        // 3 次元座標 (x, y, z) が来た場合も最初の 2 成分を取り、3 番目が数値であることを確認
        c += 1;
        let _z = next_number(raw, &mut c)?;
        c = skip_ws(raw, c);
    }
    if raw.get(c) != Some(&b')') {
        return None;
    }
    ensure_end(raw, c + 1)?;
    Some([x, y])
}

#[inline]
fn parse_vector_args(raw: &[u8]) -> Option<(u64, f64)> {
    let mut c = 0usize;
    let _ = skip_p21_string(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    let dir = next_ref(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    let mag = next_number(raw, &mut c)?;
    ensure_end(raw, c)?;
    Some((dir, mag))
}

#[inline]
fn parse_line_args(raw: &[u8]) -> Option<(u64, u64)> {
    let mut c = 0usize;
    let _ = skip_p21_string(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    let pt = next_ref(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    let vec = next_ref(raw, &mut c)?;
    ensure_end(raw, c)?;
    Some((pt, vec))
}

#[inline]
fn parse_placement_args(raw: &[u8]) -> Option<(u64, u64)> {
    let mut c = 0usize;
    let _ = skip_p21_string(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    let pt = next_ref(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    c = skip_ws(raw, c);
    let dir = if raw.get(c) == Some(&b'$') {
        c += 1;
        0
    } else {
        next_ref(raw, &mut c)?
    };
    ensure_end(raw, c)?;
    Some((pt, dir))
}

#[inline]
fn parse_circle_args(raw: &[u8]) -> Option<(u64, f64)> {
    let mut c = 0usize;
    let _ = skip_p21_string(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    let pl = next_ref(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    let r = next_number(raw, &mut c)?;
    ensure_end(raw, c)?;
    Some((pl, r))
}

#[inline]
fn parse_polyline_args(raw: &[u8], out: &mut Vec<u64>) -> Option<()> {
    let mut c = 0usize;
    let _ = skip_p21_string(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    parse_ref_list_into(raw, &mut c, out)?;
    ensure_end(raw, c)
}

#[inline]
fn parse_ref_list_into(raw: &[u8], cursor: &mut usize, out: &mut Vec<u64>) -> Option<()> {
    let mut p = skip_ws(raw, *cursor);
    if raw.get(p) != Some(&b'(') {
        return None;
    }
    p = skip_ws(raw, p + 1);
    if raw.get(p) == Some(&b')') {
        *cursor = p + 1;
        return Some(());
    }
    loop {
        let r = next_ref(raw, &mut p)?;
        out.push(r);
        p = skip_ws(raw, p);
        match raw.get(p)? {
            b',' => p += 1,
            b')' => {
                *cursor = p + 1;
                return Some(());
            }
            _ => return None,
        }
    }
}

#[inline]
fn parse_trimmed_curve_args(raw: &[u8]) -> Option<FastInstKind> {
    let mut c = 0usize;
    let _ = skip_p21_string(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    let basis = next_ref(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    let t1 = parse_trim_select(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    let t2 = parse_trim_select(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    let sense_tok = parse_enum(raw, &mut c)?;
    let sense = !sense_tok.eq_ignore_ascii_case(b"F");
    consume_comma(raw, &mut c)?;
    let _rep = parse_enum(raw, &mut c)?;
    ensure_end(raw, c)?;
    Some(FastInstKind::Trimmed {
        basis,
        t1,
        t2,
        sense,
    })
}

#[inline]
fn parse_trim_select(raw: &[u8], cursor: &mut usize) -> Option<FastTrim> {
    let mut p = skip_ws(raw, *cursor);
    if raw.get(p) != Some(&b'(') {
        return None;
    }
    p = skip_ws(raw, p + 1);
    let tr = if raw.get(p) == Some(&b'#') {
        FastTrim::Ref(next_ref(raw, &mut p)?)
    } else {
        let ns = p;
        while raw
            .get(p)
            .is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_')
        {
            p += 1;
        }
        if !raw.get(ns..p)?.eq_ignore_ascii_case(b"PARAMETER_VALUE") {
            return None;
        }
        p = skip_ws(raw, p);
        if raw.get(p) != Some(&b'(') {
            return None;
        }
        p += 1;
        let v = next_number(raw, &mut p)?;
        p = skip_ws(raw, p);
        if raw.get(p) != Some(&b')') {
            return None;
        }
        p += 1;
        FastTrim::Param(v)
    };
    p = skip_ws(raw, p);
    if raw.get(p) != Some(&b')') {
        return None;
    }
    *cursor = p + 1;
    Some(tr)
}

#[inline]
fn parse_enum<'a>(raw: &'a [u8], cursor: &mut usize) -> Option<&'a [u8]> {
    let mut p = skip_ws(raw, *cursor);
    if raw.get(p) != Some(&b'.') {
        return None;
    }
    p += 1;
    let start = p;
    while let Some(&b) = raw.get(p) {
        if b == b'.' {
            break;
        }
        if !(b.is_ascii_alphanumeric() || b == b'_') {
            return None;
        }
        p += 1;
    }
    if p == start || raw.get(p) != Some(&b'.') {
        return None;
    }
    *cursor = p + 1;
    raw.get(start..p)
}

#[inline]
fn parse_curve_style_args(raw: &[u8]) -> Option<FastInstKind> {
    let mut c = 0usize;
    let _ = skip_p21_string(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    c = skip_ws(raw, c);
    let font = if raw.get(c) == Some(&b'$') {
        c += 1;
        0
    } else {
        next_ref(raw, &mut c)?
    };
    consume_comma(raw, &mut c)?;
    c = skip_ws(raw, c);
    let (width_mm, width_ref) = if raw.get(c) == Some(&b'#') {
        (0.0, next_ref(raw, &mut c)?)
    } else if raw.get(c).is_some_and(u8::is_ascii_alphabetic) {
        let ns = c;
        while raw
            .get(c)
            .is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_')
        {
            c += 1;
        }
        if ns == c {
            return None;
        }
        c = skip_ws(raw, c);
        if raw.get(c) != Some(&b'(') {
            return None;
        }
        c += 1;
        let v = next_number(raw, &mut c)?;
        c = skip_ws(raw, c);
        if raw.get(c) != Some(&b')') {
            return None;
        }
        c += 1;
        (v, 0)
    } else {
        (next_number(raw, &mut c)?, 0)
    };
    consume_comma(raw, &mut c)?;
    c = skip_ws(raw, c);
    let colour = if raw.get(c) == Some(&b'$') {
        c += 1;
        0
    } else {
        next_ref(raw, &mut c)?
    };
    ensure_end(raw, c)?;
    Some(FastInstKind::CurveStyle {
        font,
        width_mm,
        width_ref,
        colour,
    })
}

#[inline]
fn parse_style_assign_args(raw: &[u8]) -> Option<u64> {
    let mut c = 0usize;
    let mut list = Vec::with_capacity(1);
    parse_ref_list_into(raw, &mut c, &mut list)?;
    ensure_end(raw, c)?;
    match list.as_slice() {
        &[only] => Some(only),
        _ => None,
    }
}

#[inline]
fn parse_curve_occ_args(raw: &[u8]) -> Option<(u64, u64)> {
    let mut c = 0usize;
    let _ = skip_p21_string(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    let mut list = Vec::with_capacity(1);
    parse_ref_list_into(raw, &mut c, &mut list)?;
    let &[sa] = list.as_slice() else {
        return None;
    };
    consume_comma(raw, &mut c)?;
    let geom = next_ref(raw, &mut c)?;
    ensure_end(raw, c)?;
    Some((sa, geom))
}

fn parse_single_ascii_string(raw: &[u8]) -> Option<String> {
    let mut c = 0usize;
    let s = decode_p21_string(raw, &mut c)?;
    ensure_end(raw, c)?;
    Some(s)
}

fn parse_rgb_colour_args(raw: &[u8]) -> Option<FastInstKind> {
    let mut c = 0usize;
    let name = decode_p21_string(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    let r = next_number(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    let g = next_number(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    let b = next_number(raw, &mut c)?;
    ensure_end(raw, c)?;
    let sxf_predef = name
        .trim()
        .strip_prefix("$$SXF_")
        .and_then(|rest| {
            COLORS
                .iter()
                .position(|k| k.eq_ignore_ascii_case(rest.trim()))
                .map(|at| at as i64 + 1)
        })
        .unwrap_or(0);
    let to_u8 = |v: f64| -> Option<u8> {
        let rounded = libm::round(v * 255.0);
        (rounded.fract() == 0.0 && (0.0..=255.0).contains(&rounded)).then_some(rounded as u8)
    };
    let rgb = if sxf_predef > 0 {
        [0, 0, 0]
    } else {
        [to_u8(r)?, to_u8(g)?, to_u8(b)?]
    };
    Some(FastInstKind::RgbColour { sxf_predef, rgb })
}

fn parse_length_measure_args(raw: &[u8]) -> Option<(f64, u64)> {
    let mut c = skip_ws(raw, 0);
    let mm = if raw.get(c)?.is_ascii_alphabetic() {
        while raw
            .get(c)
            .is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_')
        {
            c += 1;
        }
        c = skip_ws(raw, c);
        if raw.get(c) != Some(&b'(') {
            return None;
        }
        c += 1;
        let v = next_number(raw, &mut c)?;
        c = skip_ws(raw, c);
        if raw.get(c) != Some(&b')') {
            return None;
        }
        c += 1;
        v
    } else {
        next_number(raw, &mut c)?
    };
    consume_comma(raw, &mut c)?;
    let unit_ref = next_ref(raw, &mut c)?;
    ensure_end(raw, c)?;
    Some((mm, unit_ref))
}

fn parse_layer_assignment_args(raw: &[u8]) -> Option<(String, Box<[u64]>)> {
    let mut c = 0usize;
    let lname = decode_p21_string(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    let _desc = skip_p21_string(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    let mut items = Vec::new();
    parse_ref_list_into(raw, &mut c, &mut items)?;
    ensure_end(raw, c)?;
    Some((lname, items.into_boxed_slice()))
}

fn parse_two_refs(raw: &[u8]) -> Option<(u64, u64)> {
    let mut c = 0usize;
    let a = next_ref(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    let b = next_ref(raw, &mut c)?;
    ensure_end(raw, c)?;
    Some((a, b))
}

fn parse_single_ref_list(raw: &[u8]) -> Option<Box<[u64]>> {
    let mut c = 0usize;
    let mut items = Vec::new();
    parse_ref_list_into(raw, &mut c, &mut items)?;
    ensure_end(raw, c)?;
    Some(items.into_boxed_slice())
}

fn parse_sheet_revision_args(raw: &[u8]) -> Option<(String, Box<[u64]>)> {
    let mut c = 0usize;
    let sname = decode_p21_string(raw, &mut c)?;
    consume_comma(raw, &mut c)?;
    let mut items = Vec::new();
    parse_ref_list_into(raw, &mut c, &mut items)?;
    // 残りの引数は文字列または `$` のみであることを確かめる
    while c < raw.len() {
        c = skip_ws(raw, c);
        if c >= raw.len() {
            break;
        }
        consume_comma(raw, &mut c)?;
        c = skip_ws(raw, c);
        if raw.get(c) == Some(&b'$') {
            c += 1;
        } else {
            let _ = skip_p21_string(raw, &mut c)?;
        }
    }
    Some((sname, items.into_boxed_slice()))
}

fn parse_p21_sheet_kind(name: &str) -> Option<(i64, i64, [PaperLength; 2])> {
    let (size_part, orient_part) = name.split_once('_')?;
    let kind = size_part
        .strip_prefix(['A', 'a'])?
        .parse::<i64>()
        .ok()
        .filter(|k| (0..=4).contains(k))?;
    let orient = if orient_part.eq_ignore_ascii_case("horizontal") {
        1
    } else if orient_part.eq_ignore_ascii_case("vertical") {
        0
    } else {
        return None;
    };
    let &[w, h] = A_SIZES.get(kind as usize)?;
    let sz = if orient == 1 { [h, w] } else { [w, h] };
    Some((kind, orient, sz.map(PaperLength)))
}

type P21SpanLists = (Vec<(u32, u32)>, Vec<(u32, u32)>, Vec<(u32, u32)>);

fn scan_p21_spans(bytes: &[u8]) -> Option<P21SpanLists> {
    if u32::try_from(bytes.len()).is_err() {
        return None;
    }
    let mut header_spans = Vec::with_capacity(8);
    let mut pos = 0usize;

    let (next_p, s0, e0, kw0) = next_stmt_with_span(bytes, pos)?;
    if kw0 != b"ISO-10303-21" {
        return None;
    }
    header_spans.push((s0, e0));
    pos = next_p;

    let (next_p, s1, e1, kw1) = next_stmt_with_span(bytes, pos)?;
    if kw1 != b"HEADER" {
        return None;
    }
    header_spans.push((s1, e1));
    pos = next_p;

    let mut has_assoc_schema = false;
    loop {
        let (next_p, s, e, body) = next_stmt_with_span(bytes, pos)?;
        header_spans.push((s, e));
        pos = next_p;
        if body == b"ENDSEC" {
            break;
        }
        if body.starts_with(b"FILE_SCHEMA") {
            if contains_ascii_ci(body, b"ASSOCIATIVE_DRAUGHTING") {
                has_assoc_schema = true;
            } else {
                return None;
            }
        } else if body.starts_with(b"FILE_DESCRIPTION") && contains_ascii_ci(body, b"feature_mode")
        {
            return None;
        }
    }
    if !has_assoc_schema {
        return None;
    }

    let (mut pos, sd, ed, kwd) = next_stmt_with_span(bytes, pos)?;
    if kwd != b"DATA" {
        return None;
    }
    header_spans.push((sd, ed));

    // DATA セクションを高速走査
    let mut data_spans: Vec<(u32, u32)> = Vec::new();
    let end = bytes.len();
    loop {
        pos = skip_ws(bytes, pos);
        let &first = bytes.get(pos)?;
        let stmt_start = pos;
        if first != b'#' {
            let (after_endsec, se, ee, kw_end) = next_stmt_with_span(bytes, pos)?;
            if kw_end != b"ENDSEC" {
                return None;
            }
            let (after_iso, sf, ef, kw_fin) = next_stmt_with_span(bytes, after_endsec)?;
            if kw_fin != b"END-ISO-10303-21" {
                return None;
            }
            let tail = skip_ws(bytes, after_iso);
            if tail != end {
                return None;
            }
            let _ = verify_trailing_end;
            return Some((header_spans, data_spans, vec![(se, ee), (sf, ef)]));
        }

        let mut in_quote = false;
        let mut found_semi = false;
        while let Some(&b) = bytes.get(pos) {
            if !in_quote && b == b'/' && bytes.get(pos + 1) == Some(&b'*') {
                return None;
            }
            if b == b'\'' {
                in_quote = !in_quote;
            } else if b == b';' && !in_quote {
                pos += 1;
                data_spans.push((stmt_start as u32, pos as u32));
                found_semi = true;
                break;
            }
            pos += 1;
        }
        if !found_semi {
            return None;
        }
    }
}

fn next_stmt_with_span(bytes: &[u8], pos: usize) -> Option<(usize, u32, u32, &[u8])> {
    let stmt_start = skip_ws(bytes, pos);
    let (next_pos, body) = next_simple_stmt(bytes, stmt_start)?;
    Some((next_pos, stmt_start as u32, next_pos as u32, body))
}
