//! 常駐文書エンジンの差分編集（`apply`）と履歴（`undo`・`redo`）（`docs/live.md`、#878）。
//!
//! - モデル空間の図形に対する対話編集（移動・スタイル変更・置き換え・追加・削除）と点の表し方（`SetPointStyle`）は、
//!   [`zumen_live::run_transaction`] で [`VersionedTable`](zumen_live::VersionedTable) と [`SpatialGrid`](zumen_live::SpatialGrid) を
//!   `O(1)` 差分で更新し、表示バッファー [`Display`](zumen_live::Display) と形式別バックエンド（[`LiveBacking`]）にも
//!   変更したレコードの差分だけを積む。
//! - 画層・線種・文字スタイル・寸法スタイルの表の編集や、ペーパー空間・ブロック定義・拡張辞書木・寸法や表のブロック再生成など
//!   構造全体の整合を伴う変更は、現在のバイト列の上に既存の [`DxfFile::apply`](crate::DxfFile) /
//!   [`DwgFile::apply`](crate::DwgFile) を通して常駐状態を更新し、同じ履歴列（[`LiveJournal`]）に積むことで、
//!   高速経路と構造経路を自由に行き来しても `undo`・`redo`・`save`・診断が 100% 一致する。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Key, Kind, Ref, codes};
use zumen_drawing::{
    Applied, Changes, Edit, Element, Hash, Inherit, LayerId, Length, LineTypeId, ModelLength,
    ModelPoint, Placed, PointMark, PointSize, PointStyle, Seen, Shape, SpaceId,
};
use zumen_live::{
    CommittedTxn, ExistingRowDiff, History, PatchPayload, Revision, RowId, SideIndexes, TxnError,
    run_transaction,
};
use zumen_sdk::{Apply, Codec, Error};

use super::super::single::{self, RestyleLineType};
use super::super::{BLOCK, FIT_DATA, Lifter, Out, SHEET, TABLE_CONTENT, work};
use super::backing::{
    DwgBornRecord, DwgRecordPatch, DwgStepPatch, DxfStepPatch, LiveBacking, LiveFormat,
};
use super::open::{
    LiveState, collect_row_dep_sources, open_dxf, open_from_dwg_file, open_from_dxf_file,
};
use super::shape::{
    GeneralRow, LiveKind, LiveRow, PlainLifted, ROW_BORN, ROW_DWG_ORIGIN, ROW_HAS_102_OR_360,
    ROW_HAS_HANDLE, ROW_UNIQUE_HANDLE, general_from_element, try_lift_plain_attrib,
    try_lift_plain_entity,
};
use crate::db::{AcadDb, Data, Object, Origin, Section, Value};
use crate::dwg::Dwg;
use crate::dxf::live_io::{OverlayRecord, apply_tag_edits};
use crate::dxf::{Dxf, diff_values};

/// 既定の履歴上限（64 MiB）
pub const DEFAULT_HISTORY_MAX_BYTES: usize = 64 * 1024 * 1024;

/// 1 段階の高速トランザクションが保持する形式別の差分
#[derive(Clone, Debug)]
pub enum FastBackendPatch {
    /// ASCII DXF のタグ差分とヘッダー変数差分
    Dxf(DxfStepPatch),
    /// DWG の単一レコード差分とステップ記録
    Dwg(DwgStepPatch),
}

impl PatchPayload for FastBackendPatch {
    const INLINE_BYTES: usize = {
        assert!(core::mem::size_of::<Self>() <= 1024);
        1024
    };

    fn heap_bytes(&self) -> usize {
        match self {
            Self::Dxf(p) => p.approx_bytes(),
            Self::Dwg(p) => p.approx_bytes(),
        }
    }
}

/// 高速編集 1 段階の履歴項目（[`zumen_live::History`] の 1 段分を内包して表・空間索引の巻き戻しを再利用する）
#[derive(Clone)]
pub struct FastEntry {
    step: History<LiveRow, FastBackendPatch>,
    symbol_edits: Vec<(u32, u32, Element<ModelPoint>, Element<ModelPoint>)>,
    dirty_symbols: Vec<u32>,
    before_point_style: Option<PointStyle>,
    after_point_style: Option<PointStyle>,
    approx_bytes: usize,
}

/// 構造編集 1 段階の履歴項目
#[derive(Clone)]
pub struct StructuralEntry {
    before_state: Box<LiveState>,
    after_state: Box<LiveState>,
    approx_bytes: usize,
}

#[derive(Clone)]
enum JournalEntry {
    Fast(FastEntry),
    Structural(Box<StructuralEntry>),
}

impl JournalEntry {
    const fn bytes(&self) -> usize {
        match self {
            Self::Fast(e) => e.approx_bytes,
            Self::Structural(e) => e.approx_bytes,
        }
    }
}

/// 高速編集と構造編集を統一して扱う履歴
#[derive(Clone)]
pub struct LiveJournal {
    undo_stack: Vec<JournalEntry>,
    redo_stack: Vec<JournalEntry>,
    max_bytes: usize,
}

impl Default for LiveJournal {
    fn default() -> Self {
        Self::new(DEFAULT_HISTORY_MAX_BYTES)
    }
}

impl LiveJournal {
    /// バイト上限を指定して空の履歴を作る
    pub const fn new(max_bytes: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_bytes,
        }
    }

    /// 取り消せる段数があるか
    #[inline]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// やり直せる段数があるか
    #[inline]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// 取り消せる段数
    #[inline]
    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    /// やり直せる段数
    #[inline]
    pub fn redo_len(&self) -> usize {
        self.redo_stack.len()
    }

    /// 履歴が保持している概算バイト数
    pub fn bytes(&self) -> usize {
        self.undo_stack
            .iter()
            .map(JournalEntry::bytes)
            .sum::<usize>()
            + self
                .redo_stack
                .iter()
                .map(JournalEntry::bytes)
                .sum::<usize>()
    }

    /// 履歴を空にする
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    fn push_entry(&mut self, entry: JournalEntry) {
        self.redo_stack.clear();
        self.undo_stack.push(entry);
        let mut total: usize = self.undo_stack.iter().map(JournalEntry::bytes).sum();
        while total > self.max_bytes && self.undo_stack.len() > 1 {
            let removed = self.undo_stack.remove(0);
            total = total.saturating_sub(removed.bytes());
        }
    }

    /// 直前の変更を取り消す。取り消したら `Ok(true)`、履歴が空なら `Ok(false)` を返す
    pub fn undo(&mut self, state: &mut LiveState, cx: &mut Cx<'_>) -> Result<bool, Error> {
        let Some(mut entry) = self.undo_stack.pop() else {
            return Ok(false);
        };
        match &mut entry {
            JournalEntry::Fast(fast) => {
                for (def_idx, elem_idx, before_el, _) in fast.symbol_edits.iter().rev() {
                    if let Some(sym) = state.symbols.get_mut(*def_idx as usize)
                        && let Some(slot) = sym.space.elements.get_mut(*elem_idx as usize)
                    {
                        *slot = before_el.clone();
                    }
                }
                let side = SideIndexes {
                    grid: Some(&mut state.spatial),
                    dirty_rows: None,
                    dep: Some(&mut state.dep),
                };
                let committed = match fast.step.undo(&mut state.table, side, cx) {
                    Ok(Some(c)) => c,
                    Ok(None) => return Ok(false),
                    Err(stop) => {
                        for (def_idx, elem_idx, _, after_el) in &fast.symbol_edits {
                            if let Some(sym) = state.symbols.get_mut(*def_idx as usize)
                                && let Some(slot) = sym.space.elements.get_mut(*elem_idx as usize)
                            {
                                *slot = after_el.clone();
                            }
                        }
                        self.undo_stack.push(entry);
                        cx.report(stop.to_diagnostic());
                        return Err(Error::Stopped(stop));
                    }
                };
                revert_display_for_txn(state, committed);
                for patch in committed.patches().iter().rev() {
                    match (&mut state.backing, patch) {
                        (LiveBacking::AsciiDxf(dxf), FastBackendPatch::Dxf(p)) => {
                            dxf.undo_patch(p);
                        }
                        (LiveBacking::Dwg(dwg), FastBackendPatch::Dwg(p)) => {
                            dwg.undo_patch(p);
                        }
                        _ => {}
                    }
                }
                state.point_style = fast.before_point_style;
                state
                    .dirty_symbols
                    .extend(fast.dirty_symbols.iter().copied());
            }
            JournalEntry::Structural(st) => {
                cx.check(Kind::Work, 1).map_err(|stop| {
                    cx.report(stop.to_diagnostic());
                    Error::Stopped(stop)
                })?;
                let next_rev = state.table.revision().next();
                let next_sid = state
                    .table
                    .next_state_id()
                    .max(st.before_state.table.next_state_id());
                let target_sid = st.before_state.table.state_id();
                let mut restored = (*st.before_state).clone();
                restored.table.set_version(next_rev, target_sid, next_sid);
                restored.display.mark_all_dirty();
                let sym_len = restored.symbols.len() as u32;
                restored.dirty_symbols.extend(0..sym_len);
                *state = restored;
            }
        }
        self.redo_stack.push(entry);
        Ok(true)
    }

    /// 直前に取り消した変更をやり直す。やり直したら `Ok(true)`、履歴が空なら `Ok(false)` を返す
    pub fn redo(&mut self, state: &mut LiveState, cx: &mut Cx<'_>) -> Result<bool, Error> {
        let Some(mut entry) = self.redo_stack.pop() else {
            return Ok(false);
        };
        match &mut entry {
            JournalEntry::Fast(fast) => {
                for (def_idx, elem_idx, _, after_el) in &fast.symbol_edits {
                    if let Some(sym) = state.symbols.get_mut(*def_idx as usize)
                        && let Some(slot) = sym.space.elements.get_mut(*elem_idx as usize)
                    {
                        *slot = after_el.clone();
                    }
                }
                let side = SideIndexes {
                    grid: Some(&mut state.spatial),
                    dirty_rows: None,
                    dep: Some(&mut state.dep),
                };
                let committed = match fast.step.redo(&mut state.table, side, cx) {
                    Ok(Some(c)) => c,
                    Ok(None) => return Ok(false),
                    Err(stop) => {
                        for (def_idx, elem_idx, before_el, _) in fast.symbol_edits.iter().rev() {
                            if let Some(sym) = state.symbols.get_mut(*def_idx as usize)
                                && let Some(slot) = sym.space.elements.get_mut(*elem_idx as usize)
                            {
                                *slot = before_el.clone();
                            }
                        }
                        self.redo_stack.push(entry);
                        cx.report(stop.to_diagnostic());
                        return Err(Error::Stopped(stop));
                    }
                };
                reapply_display_for_txn(state, committed);
                for patch in committed.patches() {
                    match (&mut state.backing, patch) {
                        (LiveBacking::AsciiDxf(dxf), FastBackendPatch::Dxf(p)) => {
                            dxf.commit_patch(p);
                        }
                        (LiveBacking::Dwg(dwg), FastBackendPatch::Dwg(p)) => {
                            dwg.commit_patch(p, true);
                        }
                        _ => {}
                    }
                }
                state.point_style = fast.after_point_style;
                state
                    .dirty_symbols
                    .extend(fast.dirty_symbols.iter().copied());
            }
            JournalEntry::Structural(st) => {
                cx.check(Kind::Work, 1).map_err(|stop| {
                    cx.report(stop.to_diagnostic());
                    Error::Stopped(stop)
                })?;
                let next_rev = state.table.revision().next();
                let next_sid = state
                    .table
                    .next_state_id()
                    .max(st.after_state.table.next_state_id());
                let target_sid = st.after_state.table.state_id();
                let mut restored = (*st.after_state).clone();
                restored.table.set_version(next_rev, target_sid, next_sid);
                restored.display.mark_all_dirty();
                let sym_len = restored.symbols.len() as u32;
                restored.dirty_symbols.extend(0..sym_len);
                *state = restored;
            }
        }
        self.undo_stack.push(entry);
        Ok(true)
    }
}

fn revert_display_for_txn(
    state: &mut LiveState,
    committed: &CommittedTxn<LiveRow, FastBackendPatch>,
) {
    for _ in 0..committed.born().len() {
        state.display.pop();
    }
    for diff in committed.existing() {
        match diff {
            ExistingRowDiff::Removed { row, .. } => {
                state.display.show(row.0);
            }
            ExistingRowDiff::Updated {
                row, before_row, ..
            } => {
                let counts = state.shapes.display_counts(before_row, &state.symbols);
                let style = [before_row.layer_slot, before_row.color];
                let shapes = &state.shapes;
                let symbols = &state.symbols;
                state.display.update_with(row.0, counts, style, |w| {
                    shapes.write_display(before_row, symbols, w)
                });
            }
        }
    }
}

fn reapply_display_for_txn(
    state: &mut LiveState,
    committed: &CommittedTxn<LiveRow, FastBackendPatch>,
) {
    for diff in committed.existing() {
        match diff {
            ExistingRowDiff::Updated { row, after_row, .. } => {
                let counts = state.shapes.display_counts(after_row, &state.symbols);
                let style = [after_row.layer_slot, after_row.color];
                let shapes = &state.shapes;
                let symbols = &state.symbols;
                state.display.update_with(row.0, counts, style, |w| {
                    shapes.write_display(after_row, symbols, w)
                });
            }
            ExistingRowDiff::Removed { row, .. } => {
                state.display.hide(row.0);
            }
        }
    }
    for born in committed.born() {
        let counts = state.shapes.display_counts(&born.after_row, &state.symbols);
        let style = [born.after_row.layer_slot, born.after_row.color];
        let shapes = &state.shapes;
        let symbols = &state.symbols;
        state.display.push_with(counts, style, |w| {
            shapes.write_display(&born.after_row, symbols, w);
        });
    }
}

enum FastAttempt {
    Applied(Vec<Applied>),
    Fallback,
    Err(Error),
}

/// 変更 `changes` を常駐状態へ反映し、履歴に積んで各項目の結果を返す
pub fn apply_changes(
    state: &mut LiveState,
    journal: &mut LiveJournal,
    changes: &Changes,
    threads: usize,
    cx: &mut Cx<'_>,
) -> Result<Vec<Applied>, Error> {
    state.backing.check_editable(cx)?;
    if changes.document.is_none() && changes.edits.is_empty() {
        cx.check(Kind::Work, 1).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        return Ok(Vec::new());
    }
    match try_apply_fast(state, journal, changes, cx) {
        FastAttempt::Applied(applied) => Ok(applied),
        FastAttempt::Err(err) => Err(err),
        FastAttempt::Fallback => apply_structural(state, journal, changes, threads, cx),
    }
}

#[allow(clippy::large_enum_variant)]
enum StagedLifted {
    Plain {
        item: PlainLifted,
        unique_handle: bool,
        born: bool,
    },
    General {
        record: u32,
        origin: Origin,
        handle: Option<u64>,
        unique_handle: bool,
        has_102_or_360: bool,
        born: bool,
        element: Element<ModelPoint>,
        dxf_kind: Vec<u8>,
        follower_records: Vec<u32>,
        raw_attrib_count: u32,
        drafts: Vec<zumen_drawing::Draft>,
        raw_layers: Vec<zumen_drawing::Layer>,
        raw_line_types: Vec<zumen_drawing::LineType>,
    },
}

enum StagedRowAction {
    Update { row_id: RowId, lifted: StagedLifted },
    Remove { row_id: RowId },
    Born { lifted: StagedLifted },
}

fn parse_canonical_handle_id(id: &str) -> Option<u64> {
    let rest = id.strip_prefix("space/0/h")?;
    if rest.is_empty() || rest.contains('/') {
        return None;
    }
    let h = u64::from_str_radix(rest, 16).ok()?;
    (format!("{h:X}") == rest).then_some(h)
}

fn resolve_model_target(state: &LiveState, id: &str) -> Option<RowId> {
    if let Some(h) = parse_canonical_handle_id(id)
        && let Some(rid) = state.table.find_handle(h)
    {
        return Some(rid);
    }
    state
        .non_unique_model_ids
        .get(id)
        .copied()
        .filter(|&rid| state.table.is_alive(rid))
}

fn row_origin(row: &LiveRow) -> Origin {
    if row.flags & ROW_BORN != 0 {
        Origin::New
    } else if row.flags & ROW_DWG_ORIGIN != 0 {
        Origin::Dwg {
            offset: row.origin_start as usize,
            end: row.origin_end as usize,
        }
    } else {
        Origin::Dxf {
            record: row.record as usize,
        }
    }
}

fn row_follower_records(state: &LiveState, row: &LiveRow) -> Option<(Vec<u32>, u32)> {
    match row.kind {
        LiveKind::Insert => {
            let g = state.shapes.inserts.get(row.shape as usize)?;
            let mut recs = Vec::with_capacity(g.attrib_count as usize);
            for i in 0..g.attrib_count {
                let attr = state
                    .shapes
                    .pools
                    .attribs
                    .get((g.attrib_start + i) as usize)?;
                recs.push(attr.record);
            }
            Some((recs, g.raw_attrib_count))
        }
        LiveKind::General => {
            let g = state.shapes.generals.get(row.shape as usize)?;
            Some((g.follower_records.to_vec(), g.raw_attrib_count))
        }
        _ => Some((Vec::new(), 0)),
    }
}

fn row_fallback_kind<'a>(state: &'a LiveState, row: &LiveRow) -> &'a [u8] {
    match row.kind {
        LiveKind::Line => b"LINE",
        LiveKind::Circle => b"CIRCLE",
        LiveKind::Arc => b"ARC",
        LiveKind::Polyline => b"LWPOLYLINE",
        LiveKind::Text => b"TEXT",
        LiveKind::Insert => b"INSERT",
        LiveKind::General => state
            .shapes
            .generals
            .get(row.shape as usize)
            .map_or(b"LINE", |g| g.dxf_kind.as_ref()),
    }
}

fn resolve_layer_name_for_edit(catalog: &super::shape::SlotCatalog, id: LayerId) -> Option<&str> {
    if let Some(slot) = catalog.slot_of_layer_id(id) {
        return Some(catalog.layer_name(slot));
    }
    if !catalog.by_layer_lower.contains_key("0")
        && id.0 as usize == catalog.active_layer_slots.len()
    {
        return Some("0");
    }
    None
}

fn has_ext_or_paper(values: &[Value]) -> bool {
    values.iter().any(|v| {
        v.code >= 1000
            || matches!(v.code, 102 | 360 | 101)
            || (v.code == 39 && !matches!(v.data, Data::Real(r) if r == 0.0))
            || (v.code == 67 && v.data == Data::Int(1))
    })
}

/// 1 つのモデル空間図形（と従属レコード）を `StagedLifted` に持ち上げる（`state` は変更しない）
type FollowerRecordData = (u32, Origin, Option<u64>, Vec<u8>, Vec<Value>);

#[allow(clippy::too_many_arguments)]
fn stage_lift_model_entity(
    state: &LiveState,
    record: u32,
    origin: Origin,
    handle: Option<u64>,
    unique_handle: bool,
    born: bool,
    kind: &[u8],
    values: Vec<Value>,
    followers: &[FollowerRecordData],
) -> Option<(StagedLifted, Element<ModelPoint>)> {
    let is_plain_kind = matches!(
        kind,
        b"LINE" | b"CIRCLE" | b"ARC" | b"LWPOLYLINE" | b"TEXT" | b"INSERT"
    );
    if is_plain_kind && !has_ext_or_paper(&values) {
        let mut plain_attribs = Vec::new();
        let mut raw_attrib_count = 0u32;
        let mut followers_ok = true;
        for (f_rec, f_origin, f_handle, f_kind, f_vals) in followers {
            if f_kind == b"SEQEND" {
                continue;
            }
            if f_kind != b"ATTRIB" || has_ext_or_paper(f_vals) {
                followers_ok = false;
                break;
            }
            raw_attrib_count = raw_attrib_count.saturating_add(1);
            let f_obj = Object {
                kind: f_kind.clone(),
                section: Section::Entities,
                values: f_vals.clone(),
                origin: *f_origin,
            };
            if let Some(pa) =
                try_lift_plain_attrib(&&f_obj, state.charset, *f_rec, *f_origin, *f_handle)
            {
                plain_attribs.push(pa);
            } else {
                followers_ok = false;
                break;
            }
        }
        if followers_ok {
            let obj = Object {
                kind: kind.to_vec(),
                section: Section::Entities,
                values: values.clone(),
                origin,
            };
            let poly_stream = (kind == b"LWPOLYLINE").then_some(obj.values.as_slice());
            if let Some(plain) = try_lift_plain_entity(
                kind,
                &&obj,
                poly_stream,
                state.charset,
                record,
                origin,
                handle,
                plain_attribs,
                raw_attrib_count,
            ) {
                let mut temp_shapes = super::shape::LiveShapes::default();
                let mut temp_cat = state.catalog.clone();
                let temp_row = temp_shapes.push_plain(
                    plain.clone(),
                    &mut temp_cat,
                    unique_handle,
                    born,
                    &state.block_defs_by_lower,
                );
                let el = temp_shapes.to_element(&temp_row, &temp_cat)?;
                return Some((
                    StagedLifted::Plain {
                        item: plain,
                        unique_handle,
                        born,
                    },
                    el,
                ));
            }
        }
    }

    // `OBJECTS` セクションの辞書やスタイルレコードを参照する種類は高速経路で単独リフトしない
    if values.iter().any(|v| matches!(v.code, 102 | 360))
        || matches!(
            kind,
            b"DIMENSION"
                | b"ARC_DIMENSION"
                | b"MULTILEADER"
                | b"ACAD_TABLE"
                | b"IMAGE"
                | b"PDFUNDERLAY"
                | b"DWFUNDERLAY"
                | b"DGNUNDERLAY"
        )
    {
        return None;
    }

    let mut lookup_db = AcadDb::default();
    if matches!(kind, b"HATCH" | b"LEADER") {
        let ref_code = if kind == b"HATCH" { 330 } else { 340 };
        let mut added_stubs = BTreeSet::new();
        for v in &values {
            if v.code == ref_code
                && let Data::Handle(h) = v.data
                && h != 0
                && added_stubs.insert(h)
                && let Some(ref_rid) = state.table.find_handle(h)
                && let Some(ref_row) = state.table.row(ref_rid)
            {
                lookup_db.push(Object {
                    kind: Vec::new(),
                    section: Section::Entities,
                    values: vec![Value {
                        code: 5,
                        data: Data::Handle(h),
                    }],
                    origin: row_origin(ref_row),
                });
            }
        }
    }
    let empty_layouts = BTreeMap::new();
    let ver = state.single_tables.version.as_slice();
    let lifter = Lifter {
        layouts: &empty_layouts,
        sheet: SHEET,
        charset: state.charset,
        fit_data: ver >= FIT_DATA,
        table_content: ver >= TABLE_CONTENT,
        version: (!ver.is_empty()).then(|| String::from_utf8_lossy(ver).into_owned()),
        mm: state.unit.millimeters(),
        unit: state.unit,
        origin: [0.0, 0.0],
    };
    let mut out = Out {
        layers: state.catalog.build_layers(),
        line_types: state.catalog.build_line_types(),
        ..Out::default()
    };
    for l in &out.layers {
        out.by_name
            .entry(l.name.to_ascii_lowercase())
            .or_insert(l.id);
    }
    for lt in &out.line_types {
        out.line_type_names
            .entry(lt.name.to_ascii_lowercase())
            .or_insert(lt.id);
    }
    let init_layer_len = out.layers.len();
    let init_lt_len = out.line_types.len();

    let main_obj = Object {
        kind: kind.to_vec(),
        section: Section::Entities,
        values,
        origin,
    };
    let src_ref = {
        let base = match origin {
            Origin::New => Ref::default(),
            Origin::Dwg { offset, end } => Ref::at_bytes(offset as u64, end as u64),
            Origin::Dxf { record: r } => Ref::at_record(r as u64),
        };
        match handle {
            Some(h) => base.with_key(Key::Handle(h)),
            None => base,
        }
    };
    let child_objs: Vec<(u32, Object)> = followers
        .iter()
        .filter(|(_, _, _, fk, _)| fk != b"SEQEND")
        .map(|(fr, fo, _, fk, fv)| {
            (
                *fr,
                Object {
                    kind: fk.clone(),
                    section: Section::Entities,
                    values: fv.clone(),
                    origin: *fo,
                },
            )
        })
        .collect();
    let child_refs: Vec<&Object> = child_objs.iter().map(|(_, o)| o).collect();

    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut local_cx = Cx::new(&mut budget, &mut diag, &cancel);
    if lifter
        .entity(
            &lookup_db,
            &main_obj,
            src_ref,
            &child_refs,
            &mut out,
            &mut local_cx,
        )
        .is_err()
        || !diag.is_empty()
    {
        return None;
    }
    // 新しい暗黙画層・線種を増やす変更は構造経路に任せる
    if out.layers.len() != init_layer_len || out.line_types.len() != init_lt_len {
        return None;
    }
    let el = out.model.pop()?;
    let drafts = out.extensions;
    let follower_records: Vec<u32> = followers.iter().map(|(r, ..)| *r).collect();
    let raw_attrib_count = if kind == b"INSERT" {
        child_objs.len() as u32
    } else {
        0
    };
    Some((
        StagedLifted::General {
            record,
            origin,
            handle,
            unique_handle,
            has_102_or_360: false,
            born,
            element: el.clone(),
            dxf_kind: kind.to_vec(),
            follower_records,
            raw_attrib_count,
            drafts,
            raw_layers: out.layers,
            raw_line_types: out.line_types,
        },
        el,
    ))
}

/// ブロック定義（`Section::Blocks`）内の 1 要素を空間 `space` の `Element<ModelPoint>` に持ち上げる
fn stage_lift_block_element(
    state: &LiveState,
    space: SpaceId,
    source_ref: Ref,
    origin: Origin,
    was_shape: &Shape<ModelPoint>,
    kind: &[u8],
    values: Vec<Value>,
) -> Option<Element<ModelPoint>> {
    if state.catalog.has_implicit
        || values
            .iter()
            .any(|v| v.code >= 1000 || matches!(v.code, 102 | 360 | 101))
        || matches!(
            kind,
            b"DIMENSION"
                | b"ARC_DIMENSION"
                | b"MULTILEADER"
                | b"ACAD_TABLE"
                | b"IMAGE"
                | b"PDFUNDERLAY"
                | b"DWFUNDERLAY"
                | b"DGNUNDERLAY"
                | b"POLYLINE"
                | b"VIEWPORT"
        )
    {
        return None;
    }
    let empty_db = AcadDb::default();
    let empty_layouts = BTreeMap::new();
    let ver = state.single_tables.version.as_slice();
    let lifter = Lifter {
        layouts: &empty_layouts,
        sheet: SHEET,
        charset: state.charset,
        fit_data: ver >= FIT_DATA,
        table_content: ver >= TABLE_CONTENT,
        version: (!ver.is_empty()).then(|| String::from_utf8_lossy(ver).into_owned()),
        mm: state.unit.millimeters(),
        unit: state.unit,
        origin: [0.0, 0.0],
    };
    let mut out = Out {
        layers: state.catalog.build_layers(),
        line_types: state.catalog.build_line_types(),
        ..Out::default()
    };
    for l in &out.layers {
        out.by_name
            .entry(l.name.to_ascii_lowercase())
            .or_insert(l.id);
    }
    for lt in &out.line_types {
        out.line_type_names
            .entry(lt.name.to_ascii_lowercase())
            .or_insert(lt.id);
    }
    let init_layer_len = out.layers.len();
    let init_lt_len = out.line_types.len();

    let obj = Object {
        kind: kind.to_vec(),
        section: Section::Blocks,
        values,
        origin,
    };
    let point = |x: f64, y: f64| ModelPoint { space, x, y };
    let length = |value: f64| Length::Model(ModelLength { space, value });
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut local_cx = Cx::new(&mut budget, &mut diag, &cancel);
    let el = lifter
        .element(
            &empty_db,
            &obj,
            source_ref,
            &[],
            point,
            length,
            &mut out,
            &mut local_cx,
        )
        .ok()?;
    if !diag.is_empty() || out.layers.len() != init_layer_len || out.line_types.len() != init_lt_len
    {
        return None;
    }
    // 入れ子ブロック参照の名前変更は循環検査を通すため構造経路へ回す
    if let Shape::Symbol(now_sym) = &el.shape {
        match was_shape {
            Shape::Symbol(prev_sym) if prev_sym.key == now_sym.key => {}
            _ => {
                if now_sym.key.namespace == BLOCK {
                    return None;
                }
            }
        }
    }
    Some(el)
}

fn commit_staged_lifted(state: &mut LiveState, staged: StagedLifted) -> LiveRow {
    match staged {
        StagedLifted::Plain {
            item,
            unique_handle,
            born,
        } => state.shapes.push_plain(
            item,
            &mut state.catalog,
            unique_handle,
            born,
            &state.block_defs_by_lower,
        ),
        StagedLifted::General {
            record,
            origin,
            handle,
            unique_handle,
            has_102_or_360,
            born,
            element,
            dxf_kind,
            follower_records,
            raw_attrib_count,
            drafts,
            raw_layers,
            raw_line_types,
        } => {
            let general: GeneralRow = general_from_element(
                element,
                &raw_layers,
                &raw_line_types,
                &mut state.catalog,
                &dxf_kind,
                follower_records,
                raw_attrib_count,
                drafts,
            );
            state.shapes.push_general(
                record,
                origin,
                handle,
                unique_handle,
                has_102_or_360,
                born,
                general,
            )
        }
    }
}

fn dxf_seed_from_values(vals: Option<&[Value]>) -> Option<u64> {
    vals?.iter().find_map(|v| match v.data {
        Data::Handle(seed) if v.code == 5 => Some(seed),
        _ => None,
    })
}

type PointStyleDelta = (
    Option<Vec<Value>>,
    Option<Vec<Value>>,
    Option<i16>,
    Option<f64>,
);

fn check_point_style_change(
    style: &PointStyle,
    cur_pdmode: Option<&[Value]>,
    cur_pdsize: Option<&[Value]>,
) -> Option<PointStyleDelta> {
    let size = match style.size {
        PointSize::Length(length) if length > 0.0 && length.is_finite() => length,
        PointSize::Screen(percent) if percent > 0.0 && percent.is_finite() => -percent,
        _ => return None,
    };
    let mark = match style.mark {
        PointMark::Dot => 0i64,
        PointMark::None => 1,
        PointMark::Plus => 2,
        PointMark::Cross => 3,
        PointMark::Tick => 4,
    };
    let mode_int = mark | if style.circle { 32 } else { 0 } | if style.square { 64 } else { 0 };
    let mode_vals = vec![Value {
        code: 70,
        data: Data::Int(mode_int),
    }];
    let same_mode = match cur_pdmode {
        None => mark == 0 && !style.circle && !style.square,
        Some(values) => *values == mode_vals,
    };
    let meaning = |sz: f64| match sz {
        sz if sz > 0.0 => PointSize::Length(sz),
        sz if sz < 0.0 => PointSize::Screen(-sz),
        _ => PointSize::Screen(5.0),
    };
    let same_size = match cur_pdsize {
        None => meaning(0.0) == style.size,
        Some(
            [
                Value {
                    code: 40,
                    data: Data::Real(was),
                },
            ],
        ) if was.is_finite() => meaning(*was) == style.size,
        Some(_) => false,
    };
    let dxf_mode = (!same_mode).then_some(mode_vals);
    let dxf_size = (!same_size).then(|| {
        vec![Value {
            code: 40,
            data: Data::Real(size),
        }]
    });
    let dwg_mode = (!same_mode).then(|| i16::try_from(mode_int).unwrap_or(0));
    let dwg_size = (!same_size).then_some(size);
    Some((dxf_mode, dxf_size, dwg_mode, dwg_size))
}

type SeenTargetRow = (RowId, LiveRow, Element<ModelPoint>);

fn check_target_seen(
    state: &LiveState,
    target: &Seen,
    single_edit: bool,
    cx: &mut Cx<'_>,
) -> Result<Result<SeenTargetRow, FastAttempt>, Error> {
    if !target.id.starts_with("space/0/") || state.non_model_element_ids.contains(&target.id) {
        return Ok(Err(FastAttempt::Fallback));
    }
    let Some(row_id) = resolve_model_target(state, &target.id) else {
        if single_edit {
            cx.report(
                Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", target.id.as_str()),
            );
            return Err(Error::Rejected);
        }
        return Ok(Err(FastAttempt::Fallback));
    };
    let Some(row) = state.table.row(row_id).copied() else {
        return Ok(Err(FastAttempt::Fallback));
    };
    let Some(was) = state.shapes.to_element(&row, &state.catalog) else {
        return Ok(Err(FastAttempt::Fallback));
    };
    let cur_hash = Hash::element_checked(&was, cx).map_err(|stop| {
        cx.report(stop.to_diagnostic().at(was.source.clone()));
        Error::Stopped(stop)
    })?;
    if cur_hash != target.hash {
        if single_edit {
            cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_STALE).with("id", target.id.as_str()));
            return Err(Error::Rejected);
        }
        return Ok(Err(FastAttempt::Fallback));
    }
    if state.shared_sources.contains(&was.source) {
        return Ok(Err(FastAttempt::Fallback));
    }
    Ok(Ok((row_id, row, was)))
}

type SeenBlockElement = (u32, u32, SpaceId, u64, Element<ModelPoint>);
type StagedSymbolEdit = (u32, u32, Element<ModelPoint>, Element<ModelPoint>);

fn check_block_element_seen(
    state: &LiveState,
    target: &Seen,
    single_edit: bool,
    cx: &mut Cx<'_>,
) -> Result<Result<SeenBlockElement, FastAttempt>, Error> {
    if state.catalog.has_implicit {
        return Ok(Err(FastAttempt::Fallback));
    }
    let Some(&(def_idx, elem_idx)) = state.block_element_targets.get(&target.id) else {
        return Ok(Err(FastAttempt::Fallback));
    };
    let Some(sym) = state.symbols.get(def_idx as usize) else {
        return Ok(Err(FastAttempt::Fallback));
    };
    let Some(was) = sym.space.elements.get(elem_idx as usize).cloned() else {
        return Ok(Err(FastAttempt::Fallback));
    };
    let cur_hash = Hash::element_checked(&was, cx).map_err(|stop| {
        cx.report(stop.to_diagnostic().at(was.source.clone()));
        Error::Stopped(stop)
    })?;
    if cur_hash != target.hash {
        if single_edit {
            cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_STALE).with("id", target.id.as_str()));
            return Err(Error::Rejected);
        }
        return Ok(Err(FastAttempt::Fallback));
    }
    if state.shared_sources.contains(&was.source)
        || matches!(was.shape, Shape::Dimension(_) | Shape::Table(_))
    {
        return Ok(Err(FastAttempt::Fallback));
    }
    if let Shape::Symbol(s) = &was.shape
        && !s.attributes.is_empty()
    {
        return Ok(Err(FastAttempt::Fallback));
    }
    let Some(Key::Handle(handle)) = was.source.key() else {
        return Ok(Err(FastAttempt::Fallback));
    };
    if handle == 0 {
        return Ok(Err(FastAttempt::Fallback));
    }
    Ok(Ok((def_idx, elem_idx, sym.space.id, handle, was)))
}

fn compute_block_element_values(
    state: &LiveState,
    edit: &Edit,
    was: &Element<ModelPoint>,
    kind_vec: &[u8],
    old_vals: &[Value],
) -> Option<Vec<Value>> {
    if single::is_followed(kind_vec, old_vals)
        || old_vals
            .iter()
            .any(|v| v.code >= 1000 || matches!(v.code, 102 | 360 | 101))
    {
        return None;
    }
    match edit {
        Edit::Transform { transform, .. } => {
            let (new_vals, moved_atts) =
                single::move_model(was, kind_vec, old_vals, &[], transform, state.charset)?;
            moved_atts.is_empty().then_some(new_vals)
        }
        Edit::Restyle { style, .. } => {
            let cur_layer = resolve_layer_name_for_edit(&state.catalog, was.layer)?;
            let cur_lt = match was.style.line_type {
                Inherit::Own(lt_id) => state
                    .catalog
                    .slot_of_line_type_id(lt_id)
                    .and_then(|s| state.catalog.line_type_name(s)),
                _ => None,
            };
            let new_layer = match style.layer {
                None => None,
                Some(lid) => Some(resolve_layer_name_for_edit(&state.catalog, lid)?),
            };
            let new_lt = match style.line_type {
                None => None,
                Some(Inherit::Layer) => Some(RestyleLineType::Layer),
                Some(Inherit::Block) => Some(RestyleLineType::Block),
                Some(Inherit::Own(lt_id)) => {
                    let slot = state.catalog.slot_of_line_type_id(lt_id)?;
                    let name = state.catalog.line_type_name(slot)?;
                    Some(RestyleLineType::Named(name))
                }
            };
            single::restyle_model(
                &state.single_tables,
                kind_vec,
                old_vals,
                cur_layer,
                cur_lt,
                new_layer,
                new_lt,
                style.color,
                style.line_type_scale,
                style.line_weight,
            )
        }
        Edit::Replace { element, .. } => {
            let Placed::Model(now_el) = element.as_ref() else {
                return None;
            };
            if now_el.shape != was.shape && single::moving_xdata(old_vals).is_some() {
                return None;
            }
            let (new_vals, changed_atts) = single::replace_model_with_attribs(
                was,
                now_el,
                kind_vec,
                old_vals,
                &[],
                0,
                state.charset,
            )?;
            changed_atts.is_empty().then_some(new_vals)
        }
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn stage_block_element_dxf(
    state: &LiveState,
    dxf: &super::backing::DxfLiveBacking,
    edit: &Edit,
    target: &Seen,
    single_edit: bool,
    record_ops: &mut Vec<super::backing::DxfRecordOp>,
    symbol_edits: &mut Vec<StagedSymbolEdit>,
    cx: &mut Cx<'_>,
) -> Result<Result<Applied, FastAttempt>, Error> {
    let (def_idx, elem_idx, space, _, was) =
        match check_block_element_seen(state, target, single_edit, cx)? {
            Ok(found) => found,
            Err(fb) => return Ok(Err(fb)),
        };
    let Some(rec_u64) = was.source.record() else {
        return Ok(Err(FastAttempt::Fallback));
    };
    let Ok(rec_u32) = u32::try_from(rec_u64) else {
        return Ok(Err(FastAttempt::Fallback));
    };
    let Some((kind_vec, old_vals)) = dxf.decode_current(rec_u32, b"LINE") else {
        return Ok(Err(FastAttempt::Fallback));
    };
    let Some(new_vals) = compute_block_element_values(state, edit, &was, &kind_vec, &old_vals)
    else {
        return Ok(Err(FastAttempt::Fallback));
    };
    let Some(op) = stage_dxf_record_op(dxf, rec_u32, &old_vals, new_vals.clone()) else {
        return Ok(Err(FastAttempt::Fallback));
    };
    let origin = Origin::Dxf {
        record: rec_u32 as usize,
    };
    let Some(after_el) = stage_lift_block_element(
        state,
        space,
        was.source.clone(),
        origin,
        &was.shape,
        &kind_vec,
        new_vals,
    ) else {
        return Ok(Err(FastAttempt::Fallback));
    };
    let hash = Hash::element_checked(&after_el, cx).map_err(|stop| {
        cx.report(stop.to_diagnostic().at(after_el.source.clone()));
        Error::Stopped(stop)
    })?;
    record_ops.push(op);
    symbol_edits.push((def_idx, elem_idx, was, after_el));
    Ok(Ok(Applied {
        id: Some(target.id.clone()),
        hash: Some(hash),
    }))
}

#[allow(clippy::too_many_arguments)]
fn stage_block_element_dwg(
    state: &LiveState,
    dwg: &super::backing::DwgLiveBacking,
    edit: &Edit,
    target: &Seen,
    single_edit: bool,
    step_updated: &mut Vec<crate::dwg::single::UpdatedDwgRecord>,
    patched_diffs: &mut Vec<(u32, Option<DwgRecordPatch>, DwgRecordPatch)>,
    born_diffs: &mut Vec<(u32, Option<DwgBornRecord>, DwgBornRecord)>,
    symbol_edits: &mut Vec<StagedSymbolEdit>,
    cx: &mut Cx<'_>,
) -> Result<Result<Applied, FastAttempt>, Error> {
    let (def_idx, elem_idx, space, handle, was) =
        match check_block_element_seen(state, target, single_edit, cx)? {
            Ok(found) => found,
            Err(fb) => return Ok(Err(fb)),
        };
    let Some(rec_usize) = dwg.single.record_of_handle(handle) else {
        return Ok(Err(FastAttempt::Fallback));
    };
    let Ok(rec_u32) = u32::try_from(rec_usize) else {
        return Ok(Err(FastAttempt::Fallback));
    };
    let mut quiet = Diagnostics::new();
    let Some((kind_vec, sec, cur_enc, old_vals)) =
        dwg.current_record_data(rec_u32, &mut Cx::new(cx.budget, &mut quiet, cx.cancel))
    else {
        return Ok(Err(FastAttempt::Fallback));
    };
    if sec != Section::Blocks {
        return Ok(Err(FastAttempt::Fallback));
    }
    let Some(new_vals_raw) = compute_block_element_values(state, edit, &was, &kind_vec, &old_vals)
    else {
        return Ok(Err(FastAttempt::Fallback));
    };
    let Some(new_enc) = dwg.single.encode_record_edit(
        &cur_enc,
        Section::Blocks,
        &kind_vec,
        &old_vals,
        &new_vals_raw,
    ) else {
        return Ok(Err(FastAttempt::Fallback));
    };
    let Some((_, _, settled_vals)) = dwg.single.decode_encoded(handle, &new_enc) else {
        return Ok(Err(FastAttempt::Fallback));
    };
    let origin = match was.source.bytes() {
        Some(r) => Origin::Dwg {
            offset: r.start as usize,
            end: r.end as usize,
        },
        None => Origin::New,
    };
    let Some(after_el) = stage_lift_block_element(
        state,
        space,
        was.source.clone(),
        origin,
        &was.shape,
        &kind_vec,
        settled_vals.clone(),
    ) else {
        return Ok(Err(FastAttempt::Fallback));
    };
    let hash = Hash::element_checked(&after_el, cx).map_err(|stop| {
        cx.report(stop.to_diagnostic().at(after_el.source.clone()));
        Error::Stopped(stop)
    })?;
    step_updated.push((
        handle,
        Section::Blocks,
        kind_vec.clone(),
        old_vals,
        new_vals_raw,
    ));
    record_dwg_diff(
        dwg,
        rec_u32,
        handle,
        &kind_vec,
        Section::Blocks,
        settled_vals,
        new_enc,
        patched_diffs,
        born_diffs,
    );
    symbol_edits.push((def_idx, elem_idx, was, after_el));
    Ok(Ok(Applied {
        id: Some(target.id.clone()),
        hash: Some(hash),
    }))
}

fn try_apply_fast(
    state: &mut LiveState,
    journal: &mut LiveJournal,
    changes: &Changes,
    cx: &mut Cx<'_>,
) -> FastAttempt {
    if changes.document.is_some()
        || state.duplicate_handles_global
        || matches!(state.backing.format(), LiveFormat::BinaryDxf)
    {
        return FastAttempt::Fallback;
    }
    for edit in &changes.edits {
        let eligible = match edit {
            Edit::Transform { target, .. }
            | Edit::Restyle { target, .. }
            | Edit::Replace { target, .. } => {
                target.id.starts_with("space/0/")
                    || state.block_element_targets.contains_key(&target.id)
            }
            Edit::Remove { target } => target.id.starts_with("space/0/"),
            Edit::Add { place, element } => {
                place == "space/0" && matches!(element.as_ref(), Placed::Model(_))
            }
            Edit::SetPointStyle { .. } => true,
            _ => false,
        };
        if !eligible {
            return FastAttempt::Fallback;
        }
    }

    let single_edit = changes.edits.len() == 1;
    let mut seen_targets: BTreeSet<&str> = BTreeSet::new();
    let mut row_actions: Vec<StagedRowAction> = Vec::with_capacity(changes.edits.len());
    let mut symbol_edits: Vec<StagedSymbolEdit> = Vec::new();
    let mut applied_out: Vec<Applied> = Vec::with_capacity(changes.edits.len());
    let mut new_point_style = state.point_style;

    let backend_patch = match &state.backing {
        LiveBacking::BinaryDxf(_) => return FastAttempt::Fallback,
        LiveBacking::AsciiDxf(dxf) => {
            if dxf.entities_end_record.is_none()
                && changes.edits.iter().any(|e| matches!(e, Edit::Add { .. }))
            {
                return FastAttempt::Fallback;
            }
            let mut record_ops = Vec::new();
            let mut born_added = Vec::new();
            let mut removed_records = Vec::new();
            let mut touched_model: Vec<(Vec<u8>, Vec<Value>)> = Vec::new();
            let mut pdmode_override: Option<Vec<Value>> = None;
            let mut pdsize_override: Option<Vec<Value>> = None;
            let before_max_h = dxf
                .current_max_handle
                .or_else(|| state.current_max_handle());
            let mut cur_max_h = before_max_h;
            let mut next_born_rec = dxf.next_born_record;

            for edit in &changes.edits {
                if let Edit::Transform { target, .. }
                | Edit::Restyle { target, .. }
                | Edit::Replace { target, .. } = edit
                    && state.block_element_targets.contains_key(&target.id)
                {
                    if !seen_targets.insert(target.id.as_str()) {
                        return FastAttempt::Fallback;
                    }
                    match stage_block_element_dxf(
                        state,
                        dxf,
                        edit,
                        target,
                        single_edit,
                        &mut record_ops,
                        &mut symbol_edits,
                        cx,
                    ) {
                        Ok(Ok(ap)) => {
                            applied_out.push(ap);
                            continue;
                        }
                        Ok(Err(fb)) => return fb,
                        Err(e) => return FastAttempt::Err(e),
                    }
                }
                match edit {
                    Edit::SetPointStyle { style } => {
                        if !seen_targets.insert("point_style") {
                            return FastAttempt::Fallback;
                        }
                        let Some((dm, ds, _, _)) = check_point_style_change(
                            style,
                            dxf.header.pdmode.as_deref(),
                            dxf.header.pdsize.as_deref(),
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        if let Some(m) = dm {
                            pdmode_override = Some(m);
                        }
                        if let Some(s) = ds {
                            pdsize_override = Some(s);
                        }
                        new_point_style = Some(*style);
                        applied_out.push(Applied {
                            id: None,
                            hash: None,
                        });
                    }
                    Edit::Remove { target } => {
                        if !seen_targets.insert(target.id.as_str()) {
                            return FastAttempt::Fallback;
                        }
                        let (row_id, row, _) =
                            match check_target_seen(state, target, single_edit, cx) {
                                Ok(Ok(found)) => found,
                                Ok(Err(fb)) => return fb,
                                Err(e) => return FastAttempt::Err(e),
                            };
                        if state.catalog.has_implicit
                            || !state.non_unique_model_ids.is_empty()
                            || (row.flags & ROW_HAS_102_OR_360 != 0)
                            || (row.flags & (ROW_HAS_HANDLE | ROW_UNIQUE_HANDLE)
                                != (ROW_HAS_HANDLE | ROW_UNIQUE_HANDLE))
                            || state.forget_referenced_handles.contains(&row.handle)
                            || !state.dep.dependents_of(row_id).is_empty()
                        {
                            return FastAttempt::Fallback;
                        }
                        let Some((f_recs, raw_att)) = row_follower_records(state, &row) else {
                            return FastAttempt::Fallback;
                        };
                        if !f_recs.is_empty() || raw_att > 0 {
                            return FastAttempt::Fallback;
                        }
                        let Some((kind_vec, old_vals)) =
                            dxf.decode_current(row.record, row_fallback_kind(state, &row))
                        else {
                            return FastAttempt::Fallback;
                        };
                        if single::is_followed(&kind_vec, &old_vals)
                            || old_vals.iter().any(|v| matches!(v.code, 102 | 360))
                        {
                            return FastAttempt::Fallback;
                        }
                        removed_records.push(row.record);
                        touched_model.push((kind_vec, old_vals));
                        row_actions.push(StagedRowAction::Remove { row_id });
                        applied_out.push(Applied {
                            id: None,
                            hash: None,
                        });
                    }
                    Edit::Transform { target, transform } => {
                        if !seen_targets.insert(target.id.as_str()) {
                            return FastAttempt::Fallback;
                        }
                        let (row_id, row, was) =
                            match check_target_seen(state, target, single_edit, cx) {
                                Ok(Ok(found)) => found,
                                Ok(Err(fb)) => return fb,
                                Err(e) => return FastAttempt::Err(e),
                            };
                        if matches!(was.shape, Shape::Dimension(_) | Shape::Table(_)) {
                            return FastAttempt::Fallback;
                        }
                        let Some((f_recs, raw_att)) = row_follower_records(state, &row) else {
                            return FastAttempt::Fallback;
                        };
                        if let Shape::Symbol(sym) = &was.shape
                            && raw_att as usize != sym.attributes.len()
                        {
                            return FastAttempt::Fallback;
                        }
                        let Some((kind_vec, old_vals)) =
                            dxf.decode_current(row.record, row_fallback_kind(state, &row))
                        else {
                            return FastAttempt::Fallback;
                        };
                        let mut att_decoded: Vec<(u32, Vec<Value>)> =
                            Vec::with_capacity(f_recs.len());
                        for &f_rec in &f_recs {
                            let Some((fk, fv)) = dxf.decode_current(f_rec, b"ATTRIB") else {
                                return FastAttempt::Fallback;
                            };
                            if fk != b"ATTRIB" {
                                return FastAttempt::Fallback;
                            }
                            att_decoded.push((f_rec, fv));
                        }
                        let att_slices: Vec<&[Value]> =
                            att_decoded.iter().map(|(_, v)| v.as_slice()).collect();
                        let Some((new_vals, moved_atts)) = single::move_model(
                            &was,
                            &kind_vec,
                            &old_vals,
                            &att_slices,
                            transform,
                            state.charset,
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        if moved_atts.len() != att_decoded.len() {
                            return FastAttempt::Fallback;
                        }
                        let Some(main_op) =
                            stage_dxf_record_op(dxf, row.record, &old_vals, new_vals.clone())
                        else {
                            return FastAttempt::Fallback;
                        };
                        record_ops.push(main_op);
                        touched_model.push((kind_vec.clone(), new_vals.clone()));

                        let mut follower_tuples = Vec::with_capacity(moved_atts.len());
                        for ((f_rec, old_fv), new_fv) in att_decoded.into_iter().zip(moved_atts) {
                            let Some(f_op) =
                                stage_dxf_record_op(dxf, f_rec, &old_fv, new_fv.clone())
                            else {
                                return FastAttempt::Fallback;
                            };
                            let fh = crate::db::handle_of_values(b"ATTRIB", &new_fv);
                            let fo = if f_rec < dxf.source_records() {
                                Origin::Dxf {
                                    record: f_rec as usize,
                                }
                            } else {
                                Origin::New
                            };
                            record_ops.push(f_op);
                            touched_model.push((b"ATTRIB".to_vec(), new_fv.clone()));
                            follower_tuples.push((f_rec, fo, fh, b"ATTRIB".to_vec(), new_fv));
                        }

                        let Some((lifted, after_el)) = stage_lift_model_entity(
                            state,
                            row.record,
                            row_origin(&row),
                            row.raw_handle(),
                            row.flags & ROW_UNIQUE_HANDLE != 0,
                            row.flags & ROW_BORN != 0,
                            &kind_vec,
                            new_vals,
                            &follower_tuples,
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        let ap = match build_applied(&row, &after_el, cx) {
                            Ok(ap) => ap,
                            Err(e) => return FastAttempt::Err(e),
                        };
                        row_actions.push(StagedRowAction::Update { row_id, lifted });
                        applied_out.push(ap);
                    }
                    Edit::Restyle { target, style } => {
                        if !seen_targets.insert(target.id.as_str()) || state.catalog.has_implicit {
                            return FastAttempt::Fallback;
                        }
                        let (row_id, row, was) =
                            match check_target_seen(state, target, single_edit, cx) {
                                Ok(Ok(found)) => found,
                                Ok(Err(fb)) => return fb,
                                Err(e) => return FastAttempt::Err(e),
                            };
                        let Some((f_recs, _)) = row_follower_records(state, &row) else {
                            return FastAttempt::Fallback;
                        };
                        let cur_layer = state.catalog.layer_name(row.layer_slot);
                        let cur_lt = match was.style.line_type {
                            Inherit::Own(lt_id) => state
                                .catalog
                                .slot_of_line_type_id(lt_id)
                                .and_then(|s| state.catalog.line_type_name(s)),
                            _ => None,
                        };
                        let new_layer = match style.layer {
                            None => None,
                            Some(lid) => {
                                Some(match resolve_layer_name_for_edit(&state.catalog, lid) {
                                    Some(n) => n,
                                    None => return FastAttempt::Fallback,
                                })
                            }
                        };
                        let new_lt = match style.line_type {
                            None => None,
                            Some(Inherit::Layer) => Some(RestyleLineType::Layer),
                            Some(Inherit::Block) => Some(RestyleLineType::Block),
                            Some(Inherit::Own(lt_id)) => {
                                let Some(slot) = state.catalog.slot_of_line_type_id(lt_id) else {
                                    return FastAttempt::Fallback;
                                };
                                let Some(name) = state.catalog.line_type_name(slot) else {
                                    return FastAttempt::Fallback;
                                };
                                Some(RestyleLineType::Named(name))
                            }
                        };
                        let Some((kind_vec, old_vals)) =
                            dxf.decode_current(row.record, row_fallback_kind(state, &row))
                        else {
                            return FastAttempt::Fallback;
                        };
                        let Some(new_vals) = single::restyle_model(
                            &state.single_tables,
                            &kind_vec,
                            &old_vals,
                            cur_layer,
                            cur_lt,
                            new_layer,
                            new_lt,
                            style.color,
                            style.line_type_scale,
                            style.line_weight,
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        let Some(main_op) =
                            stage_dxf_record_op(dxf, row.record, &old_vals, new_vals.clone())
                        else {
                            return FastAttempt::Fallback;
                        };
                        record_ops.push(main_op);
                        touched_model.push((kind_vec.clone(), new_vals.clone()));

                        let mut follower_tuples = Vec::with_capacity(f_recs.len());
                        for &f_rec in &f_recs {
                            let Some((fk, fv)) = dxf.decode_current(f_rec, b"ATTRIB") else {
                                return FastAttempt::Fallback;
                            };
                            let fh = crate::db::handle_of_values(&fk, &fv);
                            let fo = if f_rec < dxf.source_records() {
                                Origin::Dxf {
                                    record: f_rec as usize,
                                }
                            } else {
                                Origin::New
                            };
                            follower_tuples.push((f_rec, fo, fh, fk, fv));
                        }
                        let Some((lifted, after_el)) = stage_lift_model_entity(
                            state,
                            row.record,
                            row_origin(&row),
                            row.raw_handle(),
                            row.flags & ROW_UNIQUE_HANDLE != 0,
                            row.flags & ROW_BORN != 0,
                            &kind_vec,
                            new_vals,
                            &follower_tuples,
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        let ap = match build_applied(&row, &after_el, cx) {
                            Ok(ap) => ap,
                            Err(e) => return FastAttempt::Err(e),
                        };
                        row_actions.push(StagedRowAction::Update { row_id, lifted });
                        applied_out.push(ap);
                    }
                    Edit::Replace { target, element } => {
                        if !seen_targets.insert(target.id.as_str()) || state.catalog.has_implicit {
                            return FastAttempt::Fallback;
                        }
                        let Placed::Model(now_el) = element.as_ref() else {
                            return FastAttempt::Fallback;
                        };
                        let (row_id, row, was) =
                            match check_target_seen(state, target, single_edit, cx) {
                                Ok(Ok(found)) => found,
                                Ok(Err(fb)) => return fb,
                                Err(e) => return FastAttempt::Err(e),
                            };
                        if matches!(was.shape, Shape::Dimension(_) | Shape::Table(_)) {
                            return FastAttempt::Fallback;
                        }
                        let Some((f_recs, raw_att)) = row_follower_records(state, &row) else {
                            return FastAttempt::Fallback;
                        };
                        let Some((kind_vec, old_vals)) =
                            dxf.decode_current(row.record, row_fallback_kind(state, &row))
                        else {
                            return FastAttempt::Fallback;
                        };
                        if now_el.shape != was.shape && single::moving_xdata(&old_vals).is_some() {
                            return FastAttempt::Fallback;
                        }
                        let mut att_decoded: Vec<(u32, Vec<Value>)> =
                            Vec::with_capacity(f_recs.len());
                        for &f_rec in &f_recs {
                            let Some((fk, fv)) = dxf.decode_current(f_rec, b"ATTRIB") else {
                                return FastAttempt::Fallback;
                            };
                            if fk != b"ATTRIB" {
                                return FastAttempt::Fallback;
                            }
                            att_decoded.push((f_rec, fv));
                        }
                        let att_slices: Vec<&[Value]> =
                            att_decoded.iter().map(|(_, v)| v.as_slice()).collect();
                        let Some((new_vals, changed_atts)) = single::replace_model_with_attribs(
                            &was,
                            now_el,
                            &kind_vec,
                            &old_vals,
                            &att_slices,
                            raw_att as usize,
                            state.charset,
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        let Some(main_op) =
                            stage_dxf_record_op(dxf, row.record, &old_vals, new_vals.clone())
                        else {
                            return FastAttempt::Fallback;
                        };
                        record_ops.push(main_op);
                        touched_model.push((kind_vec.clone(), new_vals.clone()));

                        let mut changed_map: BTreeMap<usize, Vec<Value>> =
                            changed_atts.into_iter().collect();
                        let mut follower_tuples = Vec::with_capacity(att_decoded.len());
                        for (idx, (f_rec, old_fv)) in att_decoded.into_iter().enumerate() {
                            let final_fv = if let Some(new_fv) = changed_map.remove(&idx) {
                                let Some(f_op) =
                                    stage_dxf_record_op(dxf, f_rec, &old_fv, new_fv.clone())
                                else {
                                    return FastAttempt::Fallback;
                                };
                                record_ops.push(f_op);
                                touched_model.push((b"ATTRIB".to_vec(), new_fv.clone()));
                                new_fv
                            } else {
                                old_fv
                            };
                            let fh = crate::db::handle_of_values(b"ATTRIB", &final_fv);
                            let fo = if f_rec < dxf.source_records() {
                                Origin::Dxf {
                                    record: f_rec as usize,
                                }
                            } else {
                                Origin::New
                            };
                            follower_tuples.push((f_rec, fo, fh, b"ATTRIB".to_vec(), final_fv));
                        }

                        let Some((lifted, after_el)) = stage_lift_model_entity(
                            state,
                            row.record,
                            row_origin(&row),
                            row.raw_handle(),
                            row.flags & ROW_UNIQUE_HANDLE != 0,
                            row.flags & ROW_BORN != 0,
                            &kind_vec,
                            new_vals,
                            &follower_tuples,
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        let ap = match build_applied(&row, &after_el, cx) {
                            Ok(ap) => ap,
                            Err(e) => return FastAttempt::Err(e),
                        };
                        row_actions.push(StagedRowAction::Update { row_id, lifted });
                        applied_out.push(ap);
                    }
                    Edit::Add { element, .. } => {
                        let Placed::Model(new_el) = element.as_ref() else {
                            return FastAttempt::Fallback;
                        };
                        let seed = dxf_seed_from_values(dxf.header.handseed.as_deref());
                        let Some(first_h) = single::next_handle(cur_max_h, seed) else {
                            return FastAttempt::Fallback;
                        };
                        let Some((layer_name, lt_name, att_layers, att_lts)) =
                            collect_born_table_names(&state.catalog, new_el)
                        else {
                            return FastAttempt::Fallback;
                        };
                        let Some(born_recs) = single::born_model(
                            &state.single_tables,
                            new_el,
                            layer_name,
                            lt_name,
                            &att_layers,
                            &att_lts,
                            first_h,
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        let Some((main_kind, main_vals)) = born_recs.first().cloned() else {
                            return FastAttempt::Fallback;
                        };
                        let main_rec_id = next_born_rec;
                        next_born_rec = next_born_rec.saturating_add(1);
                        let Ok(main_ov) = build_dxf_born_overlay(main_kind, &main_vals) else {
                            return FastAttempt::Fallback;
                        };
                        born_added.push((main_rec_id, main_ov, main_vals.clone()));
                        touched_model.push((main_kind.to_vec(), main_vals.clone()));

                        let mut last_h = first_h;
                        let mut follower_tuples = Vec::new();
                        for (fk, fv) in born_recs.into_iter().skip(1) {
                            let f_rec_id = next_born_rec;
                            next_born_rec = next_born_rec.saturating_add(1);
                            let Ok(f_ov) = build_dxf_born_overlay(fk, &fv) else {
                                return FastAttempt::Fallback;
                            };
                            if let Some(fh) = crate::db::handle_of_values(fk, &fv) {
                                last_h = last_h.max(fh);
                                follower_tuples.push((
                                    f_rec_id,
                                    Origin::New,
                                    Some(fh),
                                    fk.to_vec(),
                                    fv.clone(),
                                ));
                            }
                            born_added.push((f_rec_id, f_ov, fv.clone()));
                            touched_model.push((fk.to_vec(), fv));
                        }
                        cur_max_h = Some(cur_max_h.map_or(last_h, |m| m.max(last_h)));

                        let Some((lifted, after_el)) = stage_lift_model_entity(
                            state,
                            main_rec_id,
                            Origin::New,
                            Some(first_h),
                            true,
                            true,
                            main_kind,
                            main_vals,
                            &follower_tuples,
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        let hash = match Hash::element_checked(&after_el, cx) {
                            Ok(h) => h,
                            Err(stop) => {
                                cx.report(stop.to_diagnostic());
                                return FastAttempt::Err(Error::Stopped(stop));
                            }
                        };
                        row_actions.push(StagedRowAction::Born { lifted });
                        applied_out.push(Applied {
                            id: Some(format!("space/0/h{first_h:X}")),
                            hash: Some(hash),
                        });
                    }
                    _ => return FastAttempt::Fallback,
                }
            }

            let touched_refs: Vec<(&[u8], &[Value])> = touched_model
                .iter()
                .map(|(k, v)| (k.as_slice(), v.as_slice()))
                .collect();
            let Ok((after_header, _)) = dxf.compute_updated_header(
                &touched_refs,
                pdmode_override,
                pdsize_override,
                cur_max_h,
            ) else {
                return FastAttempt::Fallback;
            };
            FastBackendPatch::Dxf(DxfStepPatch {
                record_ops,
                born_added,
                removed_records,
                before_header: dxf.header.clone(),
                after_header,
                before_max_handle: before_max_h,
                after_max_handle: cur_max_h,
            })
        }
        LiveBacking::Dwg(dwg) => {
            let mut step_updated = Vec::new();
            let mut step_removed_handles = BTreeSet::new();
            let mut step_removed_model_handles = BTreeSet::new();
            let mut step_added = Vec::new();
            let mut step_new_seed: Option<u64> = None;
            let mut step_point_mode: Option<i16> = None;
            let mut step_point_size: Option<f64> = None;

            let mut patched_diffs = Vec::new();
            let mut born_diffs = Vec::new();
            let mut removed_records = Vec::new();
            let mut removed_model_handles = Vec::new();

            let before_max_h = dwg
                .current_max_handle
                .or_else(|| state.current_max_handle());
            let before_meta = (
                dwg.handseed,
                dwg.point_mode_override,
                dwg.point_size_override,
                before_max_h,
            );
            let mut cur_seed = dwg.handseed;
            let mut cur_pmode = dwg.point_mode_override;
            let mut cur_psize = dwg.point_size_override;
            let mut cur_max_h = before_max_h;
            let mut next_born_rec = dwg.next_born_record;

            let cur_pdmode_val = cur_pmode.or_else(|| dwg.single.point_mode()).map(|m| {
                vec![Value {
                    code: 70,
                    data: Data::Int(i64::from(m)),
                }]
            });
            let cur_pdsize_val = cur_psize.or_else(|| dwg.single.point_size()).map(|s| {
                vec![Value {
                    code: 40,
                    data: Data::Real(s),
                }]
            });

            for edit in &changes.edits {
                if let Edit::Transform { target, .. }
                | Edit::Restyle { target, .. }
                | Edit::Replace { target, .. } = edit
                    && state.block_element_targets.contains_key(&target.id)
                {
                    if !seen_targets.insert(target.id.as_str()) {
                        return FastAttempt::Fallback;
                    }
                    match stage_block_element_dwg(
                        state,
                        dwg,
                        edit,
                        target,
                        single_edit,
                        &mut step_updated,
                        &mut patched_diffs,
                        &mut born_diffs,
                        &mut symbol_edits,
                        cx,
                    ) {
                        Ok(Ok(ap)) => {
                            applied_out.push(ap);
                            continue;
                        }
                        Ok(Err(fb)) => return fb,
                        Err(e) => return FastAttempt::Err(e),
                    }
                }
                match edit {
                    Edit::SetPointStyle { style } => {
                        if !seen_targets.insert("point_style") {
                            return FastAttempt::Fallback;
                        }
                        if dwg.single.point_mode().is_none() || dwg.single.point_size().is_none() {
                            return FastAttempt::Fallback;
                        }
                        let Some((_, _, dm, ds)) = check_point_style_change(
                            style,
                            cur_pdmode_val.as_deref(),
                            cur_pdsize_val.as_deref(),
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        if let Some(m) = dm {
                            step_point_mode = Some(m);
                            cur_pmode = Some(m);
                        }
                        if let Some(s) = ds {
                            step_point_size = Some(s);
                            cur_psize = Some(s);
                        }
                        new_point_style = Some(*style);
                        applied_out.push(Applied {
                            id: None,
                            hash: None,
                        });
                    }
                    Edit::Remove { target } => {
                        if !seen_targets.insert(target.id.as_str()) {
                            return FastAttempt::Fallback;
                        }
                        let (row_id, row, _) =
                            match check_target_seen(state, target, single_edit, cx) {
                                Ok(Ok(found)) => found,
                                Ok(Err(fb)) => return fb,
                                Err(e) => return FastAttempt::Err(e),
                            };
                        if state.catalog.has_implicit
                            || !state.non_unique_model_ids.is_empty()
                            || (row.flags & ROW_HAS_102_OR_360 != 0)
                            || (row.flags & (ROW_HAS_HANDLE | ROW_UNIQUE_HANDLE)
                                != (ROW_HAS_HANDLE | ROW_UNIQUE_HANDLE))
                            || state.forget_referenced_handles.contains(&row.handle)
                            || !state.dep.dependents_of(row_id).is_empty()
                        {
                            return FastAttempt::Fallback;
                        }
                        let Some((f_recs, raw_att)) = row_follower_records(state, &row) else {
                            return FastAttempt::Fallback;
                        };
                        if !f_recs.is_empty() || raw_att > 0 {
                            return FastAttempt::Fallback;
                        }
                        let mut quiet = Diagnostics::new();
                        let Some((kind_vec, _, cur_enc, old_vals)) = dwg.current_record_data(
                            row.record,
                            &mut Cx::new(cx.budget, &mut quiet, cx.cancel),
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        if single::is_followed(&kind_vec, &old_vals)
                            || old_vals.iter().any(|v| matches!(v.code, 102 | 360))
                        {
                            return FastAttempt::Fallback;
                        }
                        if row.record < dwg.source_records() {
                            let group =
                                [(row.record as usize, row.handle, dwg.single.model_owner())];
                            if !dwg.single.can_unlink_model_entity(
                                &dwg.base_file,
                                row.record as usize,
                                &cur_enc,
                                &kind_vec,
                                &old_vals,
                                &group,
                            ) {
                                return FastAttempt::Fallback;
                            }
                        }
                        step_removed_handles.insert(row.handle);
                        step_removed_model_handles.insert(row.handle);
                        removed_records.push(row.record);
                        removed_model_handles.push(row.handle);
                        row_actions.push(StagedRowAction::Remove { row_id });
                        applied_out.push(Applied {
                            id: None,
                            hash: None,
                        });
                    }
                    Edit::Transform { target, transform } => {
                        if !seen_targets.insert(target.id.as_str()) {
                            return FastAttempt::Fallback;
                        }
                        let (row_id, row, was) =
                            match check_target_seen(state, target, single_edit, cx) {
                                Ok(Ok(found)) => found,
                                Ok(Err(fb)) => return fb,
                                Err(e) => return FastAttempt::Err(e),
                            };
                        if matches!(was.shape, Shape::Dimension(_) | Shape::Table(_))
                            || (row.flags & ROW_HAS_HANDLE == 0)
                        {
                            return FastAttempt::Fallback;
                        }
                        let Some((f_recs, raw_att)) = row_follower_records(state, &row) else {
                            return FastAttempt::Fallback;
                        };
                        if let Shape::Symbol(sym) = &was.shape
                            && raw_att as usize != sym.attributes.len()
                        {
                            return FastAttempt::Fallback;
                        }
                        let mut quiet = Diagnostics::new();
                        let Some((kind_vec, sec, cur_enc, old_vals)) = dwg.current_record_data(
                            row.record,
                            &mut Cx::new(cx.budget, &mut quiet, cx.cancel),
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        let mut att_decoded = Vec::with_capacity(f_recs.len());
                        for &f_rec in &f_recs {
                            let Some((fk, fs, fe, fv)) = dwg.current_record_data(
                                f_rec,
                                &mut Cx::new(cx.budget, &mut quiet, cx.cancel),
                            ) else {
                                return FastAttempt::Fallback;
                            };
                            if fk != b"ATTRIB" {
                                return FastAttempt::Fallback;
                            }
                            let Some(fh) = crate::db::handle_of_values(b"ATTRIB", &fv) else {
                                return FastAttempt::Fallback;
                            };
                            att_decoded.push((f_rec, fh, fs, fe, fv));
                        }
                        let att_slices: Vec<&[Value]> = att_decoded
                            .iter()
                            .map(|(_, _, _, _, v)| v.as_slice())
                            .collect();
                        let Some((new_vals_raw, moved_atts_raw)) = single::move_model(
                            &was,
                            &kind_vec,
                            &old_vals,
                            &att_slices,
                            transform,
                            state.charset,
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        if moved_atts_raw.len() != att_decoded.len() {
                            return FastAttempt::Fallback;
                        }
                        let Some(new_enc) = dwg.single.encode_entity_edit(
                            &cur_enc,
                            &kind_vec,
                            &old_vals,
                            &new_vals_raw,
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        let Some((_, _, settled_vals)) =
                            dwg.single.decode_encoded(row.handle, &new_enc)
                        else {
                            return FastAttempt::Fallback;
                        };
                        step_updated.push((
                            row.handle,
                            sec.clone(),
                            kind_vec.clone(),
                            old_vals,
                            new_vals_raw,
                        ));
                        record_dwg_diff(
                            dwg,
                            row.record,
                            row.handle,
                            &kind_vec,
                            sec,
                            settled_vals.clone(),
                            new_enc,
                            &mut patched_diffs,
                            &mut born_diffs,
                        );

                        let mut follower_tuples = Vec::with_capacity(moved_atts_raw.len());
                        for ((f_rec, fh, fs, fe, old_fv), new_fv_raw) in
                            att_decoded.into_iter().zip(moved_atts_raw)
                        {
                            let Some(f_new_enc) =
                                dwg.single
                                    .encode_entity_edit(&fe, b"ATTRIB", &old_fv, &new_fv_raw)
                            else {
                                return FastAttempt::Fallback;
                            };
                            let Some((_, _, f_settled)) = dwg.single.decode_encoded(fh, &f_new_enc)
                            else {
                                return FastAttempt::Fallback;
                            };
                            step_updated.push((
                                fh,
                                fs.clone(),
                                b"ATTRIB".to_vec(),
                                old_fv,
                                new_fv_raw,
                            ));
                            record_dwg_diff(
                                dwg,
                                f_rec,
                                fh,
                                b"ATTRIB",
                                fs,
                                f_settled.clone(),
                                f_new_enc,
                                &mut patched_diffs,
                                &mut born_diffs,
                            );
                            let fo = dwg_follower_origin(dwg, f_rec);
                            follower_tuples.push((
                                f_rec,
                                fo,
                                Some(fh),
                                b"ATTRIB".to_vec(),
                                f_settled,
                            ));
                        }

                        let Some((lifted, after_el)) = stage_lift_model_entity(
                            state,
                            row.record,
                            row_origin(&row),
                            row.raw_handle(),
                            row.flags & ROW_UNIQUE_HANDLE != 0,
                            row.flags & ROW_BORN != 0,
                            &kind_vec,
                            settled_vals,
                            &follower_tuples,
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        let ap = match build_applied(&row, &after_el, cx) {
                            Ok(ap) => ap,
                            Err(e) => return FastAttempt::Err(e),
                        };
                        row_actions.push(StagedRowAction::Update { row_id, lifted });
                        applied_out.push(ap);
                    }
                    Edit::Restyle { target, style } => {
                        if !seen_targets.insert(target.id.as_str()) || state.catalog.has_implicit {
                            return FastAttempt::Fallback;
                        }
                        let (row_id, row, was) =
                            match check_target_seen(state, target, single_edit, cx) {
                                Ok(Ok(found)) => found,
                                Ok(Err(fb)) => return fb,
                                Err(e) => return FastAttempt::Err(e),
                            };
                        if row.flags & ROW_HAS_HANDLE == 0 {
                            return FastAttempt::Fallback;
                        }
                        let Some((f_recs, _)) = row_follower_records(state, &row) else {
                            return FastAttempt::Fallback;
                        };
                        let cur_layer = state.catalog.layer_name(row.layer_slot);
                        let cur_lt = match was.style.line_type {
                            Inherit::Own(lt_id) => state
                                .catalog
                                .slot_of_line_type_id(lt_id)
                                .and_then(|s| state.catalog.line_type_name(s)),
                            _ => None,
                        };
                        let new_layer = match style.layer {
                            None => None,
                            Some(lid) => {
                                Some(match resolve_layer_name_for_edit(&state.catalog, lid) {
                                    Some(n) => n,
                                    None => return FastAttempt::Fallback,
                                })
                            }
                        };
                        let new_lt = match style.line_type {
                            None => None,
                            Some(Inherit::Layer) => Some(RestyleLineType::Layer),
                            Some(Inherit::Block) => Some(RestyleLineType::Block),
                            Some(Inherit::Own(lt_id)) => {
                                let Some(slot) = state.catalog.slot_of_line_type_id(lt_id) else {
                                    return FastAttempt::Fallback;
                                };
                                let Some(name) = state.catalog.line_type_name(slot) else {
                                    return FastAttempt::Fallback;
                                };
                                Some(RestyleLineType::Named(name))
                            }
                        };
                        let mut quiet = Diagnostics::new();
                        let Some((kind_vec, sec, cur_enc, old_vals)) = dwg.current_record_data(
                            row.record,
                            &mut Cx::new(cx.budget, &mut quiet, cx.cancel),
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        let Some(new_vals_raw) = single::restyle_model(
                            &state.single_tables,
                            &kind_vec,
                            &old_vals,
                            cur_layer,
                            cur_lt,
                            new_layer,
                            new_lt,
                            style.color,
                            style.line_type_scale,
                            style.line_weight,
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        let Some(new_enc) = dwg.single.encode_entity_edit(
                            &cur_enc,
                            &kind_vec,
                            &old_vals,
                            &new_vals_raw,
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        let Some((_, _, settled_vals)) =
                            dwg.single.decode_encoded(row.handle, &new_enc)
                        else {
                            return FastAttempt::Fallback;
                        };
                        step_updated.push((
                            row.handle,
                            sec.clone(),
                            kind_vec.clone(),
                            old_vals,
                            new_vals_raw,
                        ));
                        record_dwg_diff(
                            dwg,
                            row.record,
                            row.handle,
                            &kind_vec,
                            sec,
                            settled_vals.clone(),
                            new_enc,
                            &mut patched_diffs,
                            &mut born_diffs,
                        );

                        let mut follower_tuples = Vec::with_capacity(f_recs.len());
                        for &f_rec in &f_recs {
                            let Some((fk, _, _, fv)) = dwg.current_record_data(
                                f_rec,
                                &mut Cx::new(cx.budget, &mut quiet, cx.cancel),
                            ) else {
                                return FastAttempt::Fallback;
                            };
                            let fh = crate::db::handle_of_values(&fk, &fv);
                            let fo = dwg_follower_origin(dwg, f_rec);
                            follower_tuples.push((f_rec, fo, fh, fk, fv));
                        }
                        let Some((lifted, after_el)) = stage_lift_model_entity(
                            state,
                            row.record,
                            row_origin(&row),
                            row.raw_handle(),
                            row.flags & ROW_UNIQUE_HANDLE != 0,
                            row.flags & ROW_BORN != 0,
                            &kind_vec,
                            settled_vals,
                            &follower_tuples,
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        let ap = match build_applied(&row, &after_el, cx) {
                            Ok(ap) => ap,
                            Err(e) => return FastAttempt::Err(e),
                        };
                        row_actions.push(StagedRowAction::Update { row_id, lifted });
                        applied_out.push(ap);
                    }
                    Edit::Replace { target, element } => {
                        if !seen_targets.insert(target.id.as_str()) || state.catalog.has_implicit {
                            return FastAttempt::Fallback;
                        }
                        let Placed::Model(now_el) = element.as_ref() else {
                            return FastAttempt::Fallback;
                        };
                        let (row_id, row, was) =
                            match check_target_seen(state, target, single_edit, cx) {
                                Ok(Ok(found)) => found,
                                Ok(Err(fb)) => return fb,
                                Err(e) => return FastAttempt::Err(e),
                            };
                        if matches!(was.shape, Shape::Dimension(_) | Shape::Table(_))
                            || (row.flags & ROW_HAS_HANDLE == 0)
                        {
                            return FastAttempt::Fallback;
                        }
                        let Some((f_recs, raw_att)) = row_follower_records(state, &row) else {
                            return FastAttempt::Fallback;
                        };
                        let mut quiet = Diagnostics::new();
                        let Some((kind_vec, sec, cur_enc, old_vals)) = dwg.current_record_data(
                            row.record,
                            &mut Cx::new(cx.budget, &mut quiet, cx.cancel),
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        if now_el.shape != was.shape && single::moving_xdata(&old_vals).is_some() {
                            return FastAttempt::Fallback;
                        }
                        let mut att_decoded = Vec::with_capacity(f_recs.len());
                        for &f_rec in &f_recs {
                            let Some((fk, fs, fe, fv)) = dwg.current_record_data(
                                f_rec,
                                &mut Cx::new(cx.budget, &mut quiet, cx.cancel),
                            ) else {
                                return FastAttempt::Fallback;
                            };
                            if fk != b"ATTRIB" {
                                return FastAttempt::Fallback;
                            }
                            let Some(fh) = crate::db::handle_of_values(b"ATTRIB", &fv) else {
                                return FastAttempt::Fallback;
                            };
                            att_decoded.push((f_rec, fh, fs, fe, fv));
                        }
                        let att_slices: Vec<&[Value]> = att_decoded
                            .iter()
                            .map(|(_, _, _, _, v)| v.as_slice())
                            .collect();
                        let Some((new_vals_raw, changed_atts_raw)) =
                            single::replace_model_with_attribs(
                                &was,
                                now_el,
                                &kind_vec,
                                &old_vals,
                                &att_slices,
                                raw_att as usize,
                                state.charset,
                            )
                        else {
                            return FastAttempt::Fallback;
                        };
                        let Some(new_enc) = dwg.single.encode_entity_edit(
                            &cur_enc,
                            &kind_vec,
                            &old_vals,
                            &new_vals_raw,
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        let Some((_, _, settled_vals)) =
                            dwg.single.decode_encoded(row.handle, &new_enc)
                        else {
                            return FastAttempt::Fallback;
                        };
                        step_updated.push((
                            row.handle,
                            sec.clone(),
                            kind_vec.clone(),
                            old_vals,
                            new_vals_raw,
                        ));
                        record_dwg_diff(
                            dwg,
                            row.record,
                            row.handle,
                            &kind_vec,
                            sec,
                            settled_vals.clone(),
                            new_enc,
                            &mut patched_diffs,
                            &mut born_diffs,
                        );

                        let mut changed_map: BTreeMap<usize, Vec<Value>> =
                            changed_atts_raw.into_iter().collect();
                        let mut follower_tuples = Vec::with_capacity(att_decoded.len());
                        for (idx, (f_rec, fh, fs, fe, old_fv)) in
                            att_decoded.into_iter().enumerate()
                        {
                            let final_fv = if let Some(new_fv_raw) = changed_map.remove(&idx) {
                                let Some(f_new_enc) = dwg.single.encode_entity_edit(
                                    &fe,
                                    b"ATTRIB",
                                    &old_fv,
                                    &new_fv_raw,
                                ) else {
                                    return FastAttempt::Fallback;
                                };
                                let Some((_, _, f_settled)) =
                                    dwg.single.decode_encoded(fh, &f_new_enc)
                                else {
                                    return FastAttempt::Fallback;
                                };
                                step_updated.push((
                                    fh,
                                    fs.clone(),
                                    b"ATTRIB".to_vec(),
                                    old_fv,
                                    new_fv_raw,
                                ));
                                record_dwg_diff(
                                    dwg,
                                    f_rec,
                                    fh,
                                    b"ATTRIB",
                                    fs,
                                    f_settled.clone(),
                                    f_new_enc,
                                    &mut patched_diffs,
                                    &mut born_diffs,
                                );
                                f_settled
                            } else {
                                old_fv
                            };
                            let fo = dwg_follower_origin(dwg, f_rec);
                            follower_tuples.push((
                                f_rec,
                                fo,
                                Some(fh),
                                b"ATTRIB".to_vec(),
                                final_fv,
                            ));
                        }

                        let Some((lifted, after_el)) = stage_lift_model_entity(
                            state,
                            row.record,
                            row_origin(&row),
                            row.raw_handle(),
                            row.flags & ROW_UNIQUE_HANDLE != 0,
                            row.flags & ROW_BORN != 0,
                            &kind_vec,
                            settled_vals,
                            &follower_tuples,
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        let ap = match build_applied(&row, &after_el, cx) {
                            Ok(ap) => ap,
                            Err(e) => return FastAttempt::Err(e),
                        };
                        row_actions.push(StagedRowAction::Update { row_id, lifted });
                        applied_out.push(ap);
                    }
                    Edit::Add { element, .. } => {
                        let Placed::Model(new_el) = element.as_ref() else {
                            return FastAttempt::Fallback;
                        };
                        let Some(seed_val) = cur_seed else {
                            return FastAttempt::Fallback;
                        };
                        let Some(first_h) = single::next_handle(cur_max_h, Some(seed_val)) else {
                            return FastAttempt::Fallback;
                        };
                        let Some((layer_name, lt_name, att_layers, att_lts)) =
                            collect_born_table_names(&state.catalog, new_el)
                        else {
                            return FastAttempt::Fallback;
                        };
                        let mut tables_with_seed = state.single_tables.clone();
                        tables_with_seed.dwg_seed = Some(Some(seed_val));
                        let Some(born_recs) = single::born_model(
                            &tables_with_seed,
                            new_el,
                            layer_name,
                            lt_name,
                            &att_layers,
                            &att_lts,
                            first_h,
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        let Some((encoded_list, next_seed)) =
                            dwg.single.encode_born_model(&born_recs, seed_val, |r| {
                                dwg.gone.contains(&(r as u32))
                                    || removed_records.contains(&(r as u32))
                            })
                        else {
                            return FastAttempt::Fallback;
                        };
                        if encoded_list.len() != born_recs.len() {
                            return FastAttempt::Fallback;
                        }
                        cur_seed = Some(next_seed);
                        step_new_seed = Some(next_seed);

                        let mut main_settled: Option<(u32, u64, &'static [u8], Vec<Value>)> = None;
                        let mut follower_tuples = Vec::new();
                        for (idx, ((b_kind, _), (bh, benc))) in
                            born_recs.into_iter().zip(encoded_list).enumerate()
                        {
                            let Some((_, _, b_vals)) = dwg.single.decode_encoded(bh, &benc) else {
                                return FastAttempt::Fallback;
                            };
                            let in_model = idx == 0;
                            let rec_id = next_born_rec;
                            next_born_rec = next_born_rec.saturating_add(1);
                            cur_max_h = Some(cur_max_h.map_or(bh, |m| m.max(bh)));

                            step_added.push((bh, benc.clone(), in_model));
                            born_diffs.push((
                                rec_id,
                                None,
                                DwgBornRecord {
                                    handle: bh,
                                    kind: b_kind.to_vec(),
                                    values: b_vals.clone(),
                                    encoded: benc,
                                    in_model,
                                },
                            ));
                            if idx == 0 {
                                main_settled = Some((rec_id, bh, b_kind, b_vals));
                            } else {
                                follower_tuples.push((
                                    rec_id,
                                    Origin::New,
                                    Some(bh),
                                    b_kind.to_vec(),
                                    b_vals,
                                ));
                            }
                        }
                        let Some((main_rec_id, main_h, main_kind, main_vals)) = main_settled else {
                            return FastAttempt::Fallback;
                        };
                        let Some((lifted, after_el)) = stage_lift_model_entity(
                            state,
                            main_rec_id,
                            Origin::New,
                            Some(main_h),
                            true,
                            true,
                            main_kind,
                            main_vals,
                            &follower_tuples,
                        ) else {
                            return FastAttempt::Fallback;
                        };
                        let hash = match Hash::element_checked(&after_el, cx) {
                            Ok(h) => h,
                            Err(stop) => {
                                cx.report(stop.to_diagnostic());
                                return FastAttempt::Err(Error::Stopped(stop));
                            }
                        };
                        row_actions.push(StagedRowAction::Born { lifted });
                        applied_out.push(Applied {
                            id: Some(format!("space/0/h{main_h:X}")),
                            hash: Some(hash),
                        });
                    }
                    _ => return FastAttempt::Fallback,
                }
            }

            let after_meta = (cur_seed, cur_pmode, cur_psize, cur_max_h);
            FastBackendPatch::Dwg(DwgStepPatch {
                step_updated,
                step_removed_handles,
                step_removed_model_handles,
                step_added,
                step_new_seed,
                step_point_mode,
                step_point_size,
                patched_diffs,
                born_diffs,
                removed_records,
                removed_model_handles,
                before_meta,
                after_meta,
            })
        }
    };

    // ブロック定義要素の編集から影響を受けるモデル空間の行を先に辿る（ここで止まれば `state` は不変）
    let mut changed_vnodes: Vec<RowId> = symbol_edits
        .iter()
        .filter_map(|&(def_idx, ..)| RowId::virtual_node(def_idx))
        .collect();
    changed_vnodes.sort_unstable();
    changed_vnodes.dedup();
    let affected_from_symbols = if changed_vnodes.is_empty() {
        Vec::new()
    } else {
        match state.dep.affected_rows_with_cx(&changed_vnodes, cx) {
            Ok(list) => list,
            Err(stop) => {
                cx.report(stop.to_diagnostic());
                return FastAttempt::Err(Error::Stopped(stop));
            }
        }
    };

    // 反映前の予算検査（ここで止まれば `state` も `journal` も 1 ビットも変わらない）
    let affected_count = row_actions
        .len()
        .saturating_add(affected_from_symbols.len())
        .saturating_add(symbol_edits.len());
    let external_source_count = row_actions.iter().fold(0u64, |total, action| {
        let StagedRowAction::Update { row_id, .. } = action else {
            return total;
        };
        total.saturating_add(
            state
                .external_dep_sources
                .get(row_id)
                .map_or(0, |sources| sources.len() as u64),
        )
    });
    let est_work = (affected_count.max(1) as u64)
        .saturating_add(external_source_count)
        .saturating_mul(4);
    if let Err(stop) = cx.check(Kind::Work, est_work) {
        cx.report(stop.to_diagnostic());
        return FastAttempt::Err(Error::Stopped(stop));
    }
    let est_display_bytes = (row_actions
        .len()
        .saturating_add(affected_from_symbols.len()) as u64)
        .saturating_mul(128)
        .saturating_add(external_source_count.saturating_mul(size_of::<RowId>() as u64));
    if let Err(stop) = cx.check(Kind::DecodedBytes, est_display_bytes) {
        cx.report(stop.to_diagnostic());
        return FastAttempt::Err(Error::Stopped(stop));
    }

    // ブロック定義要素の更新を先に反映して、以降の囲む箱の計算が更新後の定義を見るようにする
    for (def_idx, elem_idx, _, after_el) in &symbol_edits {
        if let Some(sym) = state.symbols.get_mut(*def_idx as usize)
            && let Some(slot) = sym.space.elements.get_mut(*elem_idx as usize)
        {
            *slot = after_el.clone();
        }
    }

    let mut removed_set = BTreeSet::new();
    let mut explicit_rows = BTreeSet::new();
    for action in &row_actions {
        match action {
            StagedRowAction::Update { row_id, .. } => {
                explicit_rows.insert(*row_id);
            }
            StagedRowAction::Remove { row_id } => {
                removed_set.insert(*row_id);
                explicit_rows.insert(*row_id);
            }
            StagedRowAction::Born { .. } => {}
        }
    }

    // `LiveShapes` に新しい図形データを確定登録し、`Transaction` 用の操作列へ変換する
    enum PreparedRowOp {
        Update(RowId, LiveRow, [f64; 4], Option<Vec<RowId>>),
        Remove(RowId),
        Born(LiveRow, [f64; 4], Vec<RowId>),
    }
    let mut prepared_ops = Vec::with_capacity(
        row_actions
            .len()
            .saturating_add(affected_from_symbols.len()),
    );
    for action in row_actions {
        match action {
            StagedRowAction::Update { row_id, lifted } => {
                let row = commit_staged_lifted(state, lifted);
                let bbox = state.shapes.row_bounds(&row, &state.symbols);
                let mut dep_srcs = collect_row_dep_sources(
                    &state.shapes,
                    &state.block_defs_by_lower,
                    |h| {
                        state
                            .table
                            .find_handle(h)
                            .filter(|r| !removed_set.contains(r))
                    },
                    &row,
                );
                if let Some(external) = state.external_dep_sources.get(&row_id) {
                    dep_srcs.extend(external.iter().copied().filter(|source| {
                        state.table.is_alive(*source) && !removed_set.contains(source)
                    }));
                    dep_srcs.sort_unstable();
                    dep_srcs.dedup();
                }
                prepared_ops.push(PreparedRowOp::Update(row_id, row, bbox, Some(dep_srcs)));
            }
            StagedRowAction::Remove { row_id } => {
                prepared_ops.push(PreparedRowOp::Remove(row_id));
            }
            StagedRowAction::Born { lifted } => {
                let row = commit_staged_lifted(state, lifted);
                let bbox = state.shapes.row_bounds(&row, &state.symbols);
                let dep_srcs = collect_row_dep_sources(
                    &state.shapes,
                    &state.block_defs_by_lower,
                    |h| {
                        state
                            .table
                            .find_handle(h)
                            .filter(|r| !removed_set.contains(r))
                    },
                    &row,
                );
                prepared_ops.push(PreparedRowOp::Born(row, bbox, dep_srcs));
            }
        }
    }
    for &dep_rid in &affected_from_symbols {
        if dep_rid.is_virtual() || explicit_rows.contains(&dep_rid) {
            continue;
        }
        if let Some(&dep_row) = state.table.row(dep_rid) {
            let new_bbox = state.shapes.row_bounds(&dep_row, &state.symbols);
            prepared_ops.push(PreparedRowOp::Update(dep_rid, dep_row, new_bbox, None));
        }
    }

    let before_point_style = state.point_style;
    let mut step_history: History<LiveRow, FastBackendPatch> = History::new();
    let side = SideIndexes {
        grid: Some(&mut state.spatial),
        dirty_rows: None,
        dep: Some(&mut state.dep),
    };
    let txn_res = run_transaction::<LiveRow, FastBackendPatch, (), ()>(
        &mut state.table,
        side,
        &mut step_history,
        cx,
        |txn, _cx| {
            for op in prepared_ops {
                match op {
                    PreparedRowOp::Update(rid, row, bbox, dep_srcs) => {
                        txn.update_row(rid, row, bbox)?;
                        if let Some(srcs) = dep_srcs {
                            txn.set_dep_sources(rid, srcs)?;
                        }
                    }
                    PreparedRowOp::Remove(rid) => {
                        let _ = txn.remove_row(rid)?;
                    }
                    PreparedRowOp::Born(row, bbox, dep_srcs) => {
                        let rid = txn.insert_row(row, bbox)?;
                        if !dep_srcs.is_empty() {
                            txn.set_dep_sources(rid, dep_srcs)?;
                        }
                    }
                }
            }
            txn.push_patch(backend_patch.clone());
            Ok(())
        },
    );
    match txn_res {
        Ok(_) => {}
        Err(TxnError::Stop(stop)) => {
            for (def_idx, elem_idx, before_el, _) in symbol_edits.iter().rev() {
                if let Some(sym) = state.symbols.get_mut(*def_idx as usize)
                    && let Some(slot) = sym.space.elements.get_mut(*elem_idx as usize)
                {
                    *slot = before_el.clone();
                }
            }
            cx.report(stop.to_diagnostic());
            return FastAttempt::Err(Error::Stopped(stop));
        }
        Err(TxnError::Stage(_) | TxnError::Rejected(())) => {
            for (def_idx, elem_idx, before_el, _) in symbol_edits.iter().rev() {
                if let Some(sym) = state.symbols.get_mut(*def_idx as usize)
                    && let Some(slot) = sym.space.elements.get_mut(*elem_idx as usize)
                {
                    *slot = before_el.clone();
                }
            }
            return FastAttempt::Fallback;
        }
    }

    // 表示バッファー・形式別バックエンド・点スタイルを同期し、履歴へ積む
    match &mut state.backing {
        LiveBacking::AsciiDxf(dxf) => {
            if let FastBackendPatch::Dxf(p) = &backend_patch {
                dxf.commit_patch(p);
            }
        }
        LiveBacking::Dwg(dwg) => {
            if let FastBackendPatch::Dwg(p) = &backend_patch {
                dwg.commit_patch(p, false);
            }
        }
        LiveBacking::BinaryDxf(_) => {}
    }
    state.point_style = new_point_style;

    let mut dirty_symbols: Vec<u32> = changed_vnodes
        .iter()
        .chain(&affected_from_symbols)
        .filter_map(|r| r.as_virtual())
        .collect();
    dirty_symbols.sort_unstable();
    dirty_symbols.dedup();
    state.dirty_symbols.extend(dirty_symbols.iter().copied());

    sync_display_from_step_history(state, &step_history);

    let approx_bytes =
        step_history.bytes() + symbol_edits.len() * 256 + dirty_symbols.len() * 4 + 64;
    journal.push_entry(JournalEntry::Fast(FastEntry {
        step: step_history,
        symbol_edits,
        dirty_symbols,
        before_point_style,
        after_point_style: new_point_style,
        approx_bytes,
    }));

    FastAttempt::Applied(applied_out)
}

fn sync_display_from_step_history(
    state: &mut LiveState,
    step_history: &History<LiveRow, FastBackendPatch>,
) {
    if let Some(committed) = step_history.last_undo() {
        reapply_display_for_txn(state, committed);
    }
}

fn build_dxf_born_overlay(kind: &[u8], values: &[Value]) -> Result<OverlayRecord, i16> {
    super::backing::DxfLiveBacking::build_born_overlay(kind, values)
}

fn stage_dxf_record_op(
    dxf: &super::backing::DxfLiveBacking,
    record: u32,
    old_vals: &[Value],
    new_vals: Vec<Value>,
) -> Option<super::backing::DxfRecordOp> {
    let edits = diff_values(1, old_vals, &new_vals).ok()?;
    let created_overlay = !dxf.overlay.contains_key(&record);
    let mut temp_ov = match dxf.overlay.get(&record) {
        Some(existing) => existing.clone(),
        None => {
            if record >= dxf.source_records() {
                return None;
            }
            let range = dxf.record_range(record);
            dxf.src.with_range(range, OverlayRecord::from_slice)
        }
    };
    let ops = apply_tag_edits(&mut temp_ov.tags, edits)?;
    let before_edited = dxf.edited_values.get(&record).cloned();
    Some((record, ops, before_edited, new_vals, created_overlay))
}

#[allow(clippy::too_many_arguments)]
fn record_dwg_diff(
    dwg: &super::backing::DwgLiveBacking,
    record: u32,
    handle: u64,
    kind: &[u8],
    section: Section,
    settled_vals: Vec<Value>,
    new_enc: Vec<u8>,
    patched_diffs: &mut Vec<(u32, Option<DwgRecordPatch>, DwgRecordPatch)>,
    born_diffs: &mut Vec<(u32, Option<DwgBornRecord>, DwgBornRecord)>,
) {
    if record < dwg.source_records() {
        let before = dwg.patched.get(&record).cloned();
        patched_diffs.push((
            record,
            before,
            DwgRecordPatch {
                kind: kind.to_vec(),
                section,
                values: settled_vals,
                encoded: new_enc,
            },
        ));
    } else if let Some(prev_born) = dwg.born.get(&record).cloned() {
        let in_model = prev_born.in_model;
        born_diffs.push((
            record,
            Some(prev_born),
            DwgBornRecord {
                handle,
                kind: kind.to_vec(),
                values: settled_vals,
                encoded: new_enc,
                in_model,
            },
        ));
    }
}

fn dwg_follower_origin(dwg: &super::backing::DwgLiveBacking, record: u32) -> Origin {
    if record < dwg.source_records()
        && let Some((_, _, _, span)) = dwg.base_file.entry_summary(record as usize)
    {
        Origin::Dwg {
            offset: span.start,
            end: span.end,
        }
    } else {
        Origin::New
    }
}

fn build_applied(
    row: &LiveRow,
    after_el: &Element<ModelPoint>,
    cx: &mut Cx<'_>,
) -> Result<Applied, Error> {
    if row.flags & (ROW_HAS_HANDLE | ROW_UNIQUE_HANDLE) != (ROW_HAS_HANDLE | ROW_UNIQUE_HANDLE) {
        let _ = work(cx, 1, &after_el.source);
        return Ok(Applied {
            id: None,
            hash: None,
        });
    }
    let hash = Hash::element_checked(after_el, cx).map_err(|stop| {
        cx.report(stop.to_diagnostic().at(after_el.source.clone()));
        Error::Stopped(stop)
    })?;
    Ok(Applied {
        id: Some(format!("space/0/h{:X}", row.handle)),
        hash: Some(hash),
    })
}

type BornTableNames<'a> = (
    &'a str,
    Option<&'a str>,
    Vec<(LayerId, String)>,
    Vec<(LineTypeId, String)>,
);

fn collect_born_table_names<'a>(
    catalog: &'a super::shape::SlotCatalog,
    el: &Element<ModelPoint>,
) -> Option<BornTableNames<'a>> {
    let layer_name = resolve_layer_name_for_edit(catalog, el.layer)?;
    let lt_name = match el.style.line_type {
        Inherit::Layer | Inherit::Block => None,
        Inherit::Own(lt_id) => {
            let slot = catalog.slot_of_line_type_id(lt_id)?;
            Some(catalog.line_type_name(slot)?)
        }
    };
    let mut att_layers = Vec::new();
    let mut att_lts = Vec::new();
    if let Shape::Symbol(sym) = &el.shape {
        for attr in &sym.attributes {
            let a_lname = resolve_layer_name_for_edit(catalog, attr.layer)?;
            att_layers.push((attr.layer, a_lname.to_owned()));
            if let Inherit::Own(lt_id) = attr.style.line_type {
                let slot = catalog.slot_of_line_type_id(lt_id)?;
                let name = catalog.line_type_name(slot)?;
                att_lts.push((lt_id, name.to_owned()));
            }
        }
    }
    Some((layer_name, lt_name, att_layers, att_lts))
}

fn apply_structural(
    state: &mut LiveState,
    journal: &mut LiveJournal,
    changes: &Changes,
    threads: usize,
    cx: &mut Cx<'_>,
) -> Result<Vec<Applied>, Error> {
    let format = state.backing.format();
    let before_bytes = state.backing.save(cx)?;
    let (applied, mut after_state, approx_bytes) = match format {
        LiveFormat::AsciiDxf => {
            let mut quiet = Diagnostics::new();
            let mut file = Dxf::read(
                &before_bytes,
                &mut Cx::new(cx.budget, &mut quiet, cx.cancel),
            )?;
            let applied = Apply::apply(&mut file, changes, cx)?;
            let mut out = Vec::with_capacity(before_bytes.len() + 256);
            Dxf::write(
                &file,
                &mut out,
                &mut Cx::new(cx.budget, &mut quiet, cx.cancel),
            )?;
            let reopened = open_dxf(
                &out,
                threads,
                &mut Cx::new(cx.budget, &mut quiet, cx.cancel),
            )?;
            let approx = before_bytes.len().saturating_add(out.len()).max(4096);
            (applied, reopened, approx)
        }
        LiveFormat::BinaryDxf => {
            let mut quiet = Diagnostics::new();
            let mut file = Dxf::read(
                &before_bytes,
                &mut Cx::new(cx.budget, &mut quiet, cx.cancel),
            )?;
            let applied = Apply::apply(&mut file, changes, cx)?;
            let reopened =
                open_from_dxf_file(file, &mut Cx::new(cx.budget, &mut quiet, cx.cancel))?;
            let approx = before_bytes.len().saturating_mul(2).max(4096);
            (applied, reopened, approx)
        }
        LiveFormat::Dwg => {
            let mut quiet = Diagnostics::new();
            let mut file = Dwg::read(
                &before_bytes,
                &mut Cx::new(cx.budget, &mut quiet, cx.cancel),
            )?;
            let applied = Apply::apply(&mut file, changes, cx)?;
            let reopened = open_from_dwg_file(
                file,
                threads,
                &mut Cx::new(cx.budget, &mut quiet, cx.cancel),
            )?;
            let approx = before_bytes.len().saturating_mul(2).max(4096);
            (applied, reopened, approx)
        }
    };

    let next_rev: Revision = state.table.revision().next();
    let new_state_id = state.table.next_state_id();
    let next_state_id = new_state_id.saturating_add(1);
    after_state
        .table
        .set_version(next_rev, new_state_id, next_state_id);
    after_state.display.mark_all_dirty();
    let sym_len = after_state.symbols.len() as u32;
    after_state.dirty_symbols.extend(0..sym_len);

    let before_box = Box::new(state.clone());
    let after_box = Box::new(after_state.clone());
    *state = after_state;
    journal.push_entry(JournalEntry::Structural(Box::new(StructuralEntry {
        before_state: before_box,
        after_state: after_box,
        approx_bytes,
    })));
    Ok(applied)
}
