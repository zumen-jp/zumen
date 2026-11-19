//! SXF（SFC・P21）の常駐文書エンジン [`SxfLive`]。
//!
//! 開いた SXF バイト列を [`SourceBytes`] として共有保持し、要素を [`VersionedTable`]・[`HandleIndex`]・
//! [`SpatialGrid`]・[`Display`] に索引付けしたまま `O(変更数)` で差分適用・`undo`・`redo` を行う。
//! 全体を書く [`SxfLive::save`] は、`SxfFile::apply` を 1 手ずつ当てて `Sxf::write` した結果と
//! バイト列単位で一致する。

mod bounds;
mod diagnostics;
mod display;
mod ipc;
mod p21;
mod sfc;
mod source;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use zumen_base::{Cx, Diagnostic, Key, Kind, Ref, codes};
use zumen_drawing::{
    Affine, Applied, Axes, Changes, Draft, Drawing2D, Edit, Element, Hash, Inherit, Layer,
    ModelPoint, ModelSpace, PaperLength, PaperPoint, Placed, Position, Restyle, Seen, Shape, Sheet,
    SheetId, SheetItem, SpaceId, Style, SymbolDefinition, SymbolKey, TitleBlock, Unit, View, id,
    transform,
};
use zumen_live::display::Display;
use zumen_live::{
    DepGraph, EMPTY_BOUNDS, ExistingRowDiff, History, PatchPayload, Revision, RowId, SideIndexes,
    SourceBytes, SpatialGrid, TableRow, TxnError, VersionedTable, run_transaction,
};
use zumen_sdk::{Codec, Error, quietly};

use self::diagnostics::LiveDiagnostics;
use self::display::{
    DisplayContext, apply_display_commit, compute_sheet_bounds, display_counts_and_style,
    model_shape_bounds, revert_display_commit, write_display_row,
};
use self::p21::{P21Patch, P21State, P21StepTx};
use self::sfc::{SfcPatch, SfcSlot, SfcState, SfcStepTx};
use self::source::SourceMap;
use super::apply::{
    Arguments, Fonts, MARKER, OnSheet, REMOVABLE, SHAPED, STYLED, Tables, Writer, arguments,
    count_element_compare, count_element_copy, count_shape, grouped, kind_of, layers, placed_near,
    placed_same, shape_arguments, text, unset_field, unsupported,
};
use super::{
    Arg, LifterTables, LiveRasterGroup, SHEET, USER_LINE_TYPE, bind, ensure_element_tables,
    layer_feature, lift_for_live, lift_single_model, lift_single_paper, sfc_records, work,
};
use crate::p21::records_and_codes as p21_records_and_codes;
use crate::saf::AttributeFile;
use crate::{Mode, Sxf, SxfFile};

/// 要素が入っている器
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum SxfContainer {
    Space(SpaceId),
    Symbol(usize),
    Sheet(SheetId),
}

/// [`VersionedTable`] の 1 行に持つ SXF 要素
#[derive(Clone, Debug)]
pub(super) struct SxfRow {
    pub(super) instance: Option<u64>,
    pub(super) unique_handle: Option<u64>,
    pub(super) record_idx: usize,
    pub(super) local_space_idx: u32,
    sfc_slot: Option<SfcSlot>,
    pub(super) p21_kind: Box<str>,
    pub(super) p21_name: Box<str>,
    pub(super) grouped: bool,
    pub(super) container: SxfContainer,
    pub(super) canonical_id: Box<str>,
    pub(super) placed: Placed,
    pub(super) drafts: Box<[Draft]>,
    diagnostics: Vec<Diagnostic>,
}

impl TableRow for SxfRow {
    const INLINE_BYTES: usize = {
        assert!(core::mem::size_of::<Self>() <= 1024);
        1024
    };

    #[inline]
    fn row_handle(&self) -> Option<u64> {
        self.unique_handle
    }

    fn extra_heap_bytes(&self) -> usize {
        // 予算にも使うため、可変幅の値は 64 bit の論理バイト数で見積もる。
        self.p21_kind.len()
            + self.p21_name.len()
            + self.canonical_id.len()
            + self.drafts.len() * 256
            + diagnostics::heap_bytes(&self.diagnostics)
    }
}

#[derive(Clone)]
enum SxfBacking {
    Sfc(SfcState),
    P21(Box<P21State>),
}

impl SxfBacking {
    fn mode(&self) -> Mode {
        match self {
            Self::Sfc(_) => Mode::Sfc,
            Self::P21(_) => Mode::P21,
        }
    }

    fn format_name(&self) -> &'static str {
        match self {
            Self::Sfc(_) => "SFC",
            Self::P21(_) => "P21",
        }
    }

    fn resident_bytes(&self) -> usize {
        match self {
            Self::Sfc(s) => s.resident_bytes(),
            Self::P21(p) => p.resident_bytes(),
        }
    }

    fn save(&self, cx: &mut Cx) -> Result<Vec<u8>, Error> {
        match self {
            Self::Sfc(s) => s.save(cx),
            Self::P21(p) => p.save(cx),
        }
    }

    fn source_map(&self, cx: &mut Cx<'_>) -> Result<SourceMap, Error> {
        match self {
            Self::Sfc(state) => state.source_map(cx),
            Self::P21(state) => state.source_map(cx),
        }
    }
}

#[derive(Clone, Debug)]
enum BackingPatch {
    Sfc(SfcPatch),
    P21(P21Patch),
}

type RasterOutlineDiff = (usize, Option<usize>, Option<usize>);

/// 読み直した要素と拡張、読み直しの途中で出た診断
type LiftedPlaced = (Option<(Placed, Vec<Draft>)>, Vec<Diagnostic>);

#[derive(Clone, Debug)]
struct SxfUnifiedPatch {
    backing: BackingPatch,
    raster_outline_diffs: Vec<RasterOutlineDiff>,
    symbol_bounds: Vec<bounds::SymbolBoundsDiff>,
    /// レイヤと字体の表の変更
    tables: Option<Box<TablesDiff>>,
}

/// 要素の外の、名前で引く表（レイヤと字体）
#[derive(Clone, Debug)]
struct NamedTables {
    layers: Vec<Layer>,
    defined_layers: usize,
    fonts: Vec<String>,
}

impl NamedTables {
    fn heap_bytes(&self) -> usize {
        self.layers.capacity() * core::mem::size_of::<Layer>()
            + self.layers.iter().map(|l| l.name.capacity()).sum::<usize>()
            + self.fonts.capacity() * core::mem::size_of::<String>()
            + self.fonts.iter().map(String::capacity).sum::<usize>()
    }
}

/// 1 手の前後の表
#[derive(Clone, Debug)]
struct TablesDiff {
    before: NamedTables,
    after: NamedTables,
}

#[derive(Clone, Debug)]
struct SharedPatch(Arc<SxfUnifiedPatch>);

impl PatchPayload for SharedPatch {
    const INLINE_BYTES: usize = {
        assert!(core::mem::size_of::<Self>() <= 8);
        8
    };

    fn heap_bytes(&self) -> usize {
        let base = match &self.0.backing {
            BackingPatch::Sfc(s) => s.heap_bytes(),
            BackingPatch::P21(p) => p.heap_bytes(),
        };
        let tables = self
            .0
            .tables
            .as_ref()
            .map_or(0, |t| t.before.heap_bytes() + t.after.heap_bytes());
        base + self.0.raster_outline_diffs.capacity() * 32
            + self.0.symbol_bounds.capacity() * 96
            + tables
    }
}

#[derive(Clone)]
struct SpaceSpatialIndex {
    grid: SpatialGrid,
    bounds: Vec<[f64; 4]>,
    rows: Vec<RowId>,
}

#[derive(Clone, Copy, Debug)]
enum BaseSheetSlot {
    View(usize),
    Row(RowId),
}

#[derive(Clone)]
struct SpaceTemplate {
    id: SpaceId,
    unit: Unit,
    axes: Axes,
}

#[derive(Clone)]
struct SymbolTemplate {
    key: SymbolKey,
    space_id: SpaceId,
    unit: Unit,
    axes: Axes,
    base: Option<ModelPoint>,
    source: Ref,
}

#[derive(Clone)]
struct SheetTemplate {
    id: SheetId,
    name: Option<String>,
    size: Option<[PaperLength; 2]>,
    views: Vec<View>,
    source: Ref,
}

/// SXF（SFC・P21）の常駐編集文書エンジン
#[derive(Clone)]
pub struct SxfLive {
    backing: SxfBacking,
    attributes: Option<AttributeFile>,
    diagnostics: LiveDiagnostics,
    lifter_tables: LifterTables,
    /// 読めない字体の定義があり、字体のコードを決められない
    fonts_broken: bool,
    static_layer_ids: BTreeSet<u32>,
    static_user_lt_ids: BTreeSet<u32>,
    raster_groups: Vec<LiveRasterGroup>,
    record_to_raster_group: BTreeMap<usize, usize>,
    space_templates: Vec<SpaceTemplate>,
    space_rows: Vec<Vec<RowId>>,
    symbol_templates: Vec<SymbolTemplate>,
    symbol_rows: Vec<Vec<RowId>>,
    sheet_template: SheetTemplate,
    base_sheet_slots: Vec<BaseSheetSlot>,
    base_row_count: u32,
    title_blocks: Vec<TitleBlock>,
    doc_drafts: Vec<Draft>,
    sheet_name_map: BTreeMap<String, SheetId>,
    space_views: BTreeMap<SpaceId, Vec<Affine>>,
    symbol_bounds: BTreeMap<SymbolKey, [f64; 4]>,
    symbol_indices: BTreeMap<SymbolKey, u32>,
    dirty_symbols: BTreeSet<u32>,
    table: VersionedTable<SxfRow>,
    by_canonical_id: BTreeMap<String, RowId>,
    spatial: SpatialGrid,
    space_spatial: BTreeMap<SpaceId, SpaceSpatialIndex>,
    dep: DepGraph,
    display: Display,
    history: History<SxfRow, SharedPatch>,
    history_max_bytes: usize,
}

const DEFAULT_HISTORY_LIMIT: usize = 64 * 1024 * 1024;

impl SxfLive {
    /// バイト列から常駐 SXF 文書エンジンを開く
    pub fn open(bytes: &[u8], cx: &mut Cx<'_>) -> Result<Self, Error> {
        Self::open_with_threads(bytes, 1, cx)
    }

    /// 属性ファイル（SAF）を伴ってバイト列から常駐 SXF 文書エンジンを開く
    pub fn open_with_attributes(
        bytes: &[u8],
        attributes: Option<AttributeFile>,
        cx: &mut Cx<'_>,
    ) -> Result<Self, Error> {
        Self::open_with_threads_and_attributes(bytes, 1, attributes, cx)
    }

    /// スレッド数を指定して開く（`threads` が異なっても結果と診断は決定的に一致する）
    pub fn open_with_threads(bytes: &[u8], threads: usize, cx: &mut Cx<'_>) -> Result<Self, Error> {
        Self::open_with_threads_and_attributes(bytes, threads, None, cx)
    }

    /// スレッド数と任意の属性ファイル（SAF）を指定して開く
    pub fn open_with_threads_and_attributes(
        bytes: &[u8],
        threads: usize,
        attributes: Option<AttributeFile>,
        cx: &mut Cx<'_>,
    ) -> Result<Self, Error> {
        let file = Sxf::read(bytes, cx)?;
        let src = SourceBytes::compress_slice_with_cx(file.as_bytes(), threads.max(1), cx)
            .map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Error::Stopped(stop)
            })?;
        Self::build(src, &file, attributes, cx)
    }

    /// 既読の [`SxfFile`] から常駐エンジンを構築する
    pub fn from_file(file: &SxfFile, cx: &mut Cx<'_>) -> Result<Self, Error> {
        Self::from_file_with(file, None, cx)
    }

    /// 既読の [`SxfFile`] と任意の属性ファイル（SAF）から常駐エンジンを構築する
    pub fn from_file_with(
        file: &SxfFile,
        attributes: Option<AttributeFile>,
        cx: &mut Cx<'_>,
    ) -> Result<Self, Error> {
        let src = SourceBytes::compress_slice_with_cx(file.as_bytes(), 1, cx).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        Self::build(src, file, attributes, cx)
    }

    fn build(
        src: SourceBytes,
        file: &SxfFile,
        attributes: Option<AttributeFile>,
        cx: &mut Cx<'_>,
    ) -> Result<Self, Error> {
        let ((records, codes, mut lifted), events) = cx.with_diagnostic_events(u64::MAX, |cx| {
            cx.with_diagnostics_in_record_order(|cx| {
                let (records, codes) = match file.mode() {
                    Mode::Sfc => (sfc_records(file), None),
                    Mode::P21 => {
                        let (records, codes) = p21_records_and_codes(file, cx)?;
                        (records, Some(codes))
                    }
                };
                let lifted = lift_for_live(file, &records, attributes.as_ref(), cx)?;
                Ok::<_, Error>((records, codes, lifted))
            })
        })?;

        let grouped_set = grouped(&records, cx)?;
        let fonts_broken = records.iter().any(|r| {
            r.name.eq_ignore_ascii_case("text_font")
                && !matches!(r.args.first(), Some(Arg::Text(_)))
        });
        let record_meta: BTreeMap<usize, (&str, &str)> = records
            .iter()
            .map(|r| (r.record, (r.name.as_str(), r.kind.as_str())))
            .collect();

        let static_layer_ids: BTreeSet<u32> = lifted
            .tables
            .base_layers
            .iter()
            .filter(|l| l.state.is_some())
            .map(|l| l.id.0)
            .collect();
        let static_user_lt_ids: BTreeSet<u32> = lifted
            .tables
            .base_line_types
            .iter()
            .filter(|lt| i64::from(lt.id.0) > USER_LINE_TYPE && lt.source != Ref::default())
            .map(|lt| lt.id.0)
            .collect();

        let backing = match (file.mode(), codes) {
            (Mode::Sfc, _) => SxfBacking::Sfc(SfcState::new(src, file, &records, &grouped_set)),
            (Mode::P21, Some(codes)) => {
                let p21_state = P21State::new(src, file, codes, lifted.tables.widths.clone(), cx)?;
                SxfBacking::P21(Box::new(p21_state))
            }
            (Mode::P21, None) => return Err(Error::Rejected),
        };

        let mut model_drafts: BTreeMap<(SpaceId, usize), Vec<Draft>> = BTreeMap::new();
        let mut paper_drafts: BTreeMap<(SheetId, usize), Vec<Draft>> = BTreeMap::new();
        let mut doc_drafts: Vec<Draft> = Vec::new();
        for draft in lifted.raw_drafts {
            match draft.at {
                Position::Model { space, index } => {
                    model_drafts.entry((space, index)).or_default().push(draft);
                }
                Position::Paper { sheet, index } => {
                    paper_drafts.entry((sheet, index)).or_default().push(draft);
                }
                _ => doc_drafts.push(draft),
            }
        }

        let mut record_to_raster_group = BTreeMap::new();
        for (g_idx, rg) in lifted.raster_groups.iter().enumerate() {
            for &m_rec in &rg.members {
                record_to_raster_group.insert(m_rec, g_idx);
            }
        }

        let drawing = lifted.drawing;
        let sheet = drawing.sheets.into_iter().next().unwrap_or(Sheet {
            id: SHEET,
            name: None,
            size: None,
            elements: Vec::new(),
            views: Vec::new(),
            order: Vec::new(),
            source: Ref::default(),
        });

        let mut space_views: BTreeMap<SpaceId, Vec<Affine>> = BTreeMap::new();
        for v in &sheet.views {
            space_views.entry(v.space).or_default().push(v.transform);
        }

        let mut symbol_indices: BTreeMap<SymbolKey, u32> = BTreeMap::new();
        for (idx, sym_def) in drawing.symbols.iter().enumerate() {
            if let Ok(idx_u32) = u32::try_from(idx) {
                symbol_indices.insert(sym_def.key.clone(), idx_u32);
            }
        }
        let mut dep = DepGraph::new();
        for (idx, sym_def) in drawing.symbols.iter().enumerate() {
            let Some(vnode) = u32::try_from(idx).ok().and_then(RowId::virtual_node) else {
                continue;
            };
            let mut srcs = BTreeSet::new();
            for elem in &sym_def.space.elements {
                if let Shape::Symbol(sym) = &elem.shape
                    && let Some(&inner_idx) = symbol_indices.get(&sym.key)
                    && let Some(inner_vnode) = RowId::virtual_node(inner_idx)
                {
                    srcs.insert(inner_vnode);
                }
            }
            if !srcs.is_empty() {
                dep.set_sources(vnode, srcs);
            }
        }
        let mut symbol_bounds: BTreeMap<SymbolKey, [f64; 4]> = BTreeMap::new();
        for sym_idx in ordered_symbol_indices(drawing.symbols.len(), &dep, cx)? {
            let Some(sym_def) = drawing.symbols.get(sym_idx) else {
                continue;
            };
            let mut acc: Option<[f64; 4]> = None;
            for elem in &sym_def.space.elements {
                let b = model_shape_bounds(&elem.shape, &symbol_bounds);
                let [bx0, by0, bx1, by1] = b;
                if bx0 <= bx1 && by0 <= by1 {
                    match &mut acc {
                        Some([x0, y0, x1, y1]) => {
                            *x0 = x0.min(bx0);
                            *y0 = y0.min(by0);
                            *x1 = x1.max(bx1);
                            *y1 = y1.max(by1);
                        }
                        None => acc = Some(b),
                    }
                }
            }
            if let Some(b) = acc {
                symbol_bounds.insert(sym_def.key.clone(), b);
            }
        }

        let total_rows = drawing
            .spaces
            .iter()
            .map(|s| s.elements.len())
            .sum::<usize>()
            .saturating_add(drawing.symbols.iter().map(|s| s.space.elements.len()).sum())
            .saturating_add(sheet.elements.len());

        let mut rows: Vec<SxfRow> = Vec::with_capacity(total_rows);
        let mut space_templates = Vec::with_capacity(drawing.spaces.len());
        let mut space_rows = Vec::with_capacity(drawing.spaces.len());
        let mut space_spatial: BTreeMap<SpaceId, SpaceSpatialIndex> = BTreeMap::new();

        let make_row_meta = |source: &Ref,
                             backing: &SxfBacking,
                             record_meta: &BTreeMap<usize, (&str, &str)>,
                             grouped_set: &BTreeSet<usize>| {
            let instance = match source.key() {
                Some(Key::Instance(n)) => Some(n),
                _ => None,
            };
            let record_idx = source
                .record()
                .and_then(|r| usize::try_from(r).ok())
                .unwrap_or(usize::MAX);
            let sfc_slot = match backing {
                SxfBacking::Sfc(s) => s.slot_for_record(record_idx),
                SxfBacking::P21(_) => None,
            };
            let (fname, fkind) = record_meta.get(&record_idx).copied().unwrap_or(("", ""));
            let is_grouped = grouped_set.contains(&record_idx);
            (
                instance,
                record_idx,
                sfc_slot,
                fkind.to_owned().into_boxed_str(),
                fname.to_ascii_lowercase().into_boxed_str(),
                is_grouped,
            )
        };

        for sp in drawing.spaces {
            let place = id::space(sp.id);
            let sp_id = sp.id;
            space_templates.push(SpaceTemplate {
                id: sp_id,
                unit: sp.unit,
                axes: sp.axes,
            });
            let mut sp_bounds = Vec::with_capacity(sp.elements.len());
            let mut r_ids = Vec::with_capacity(sp.elements.len());
            let elem_refs: Vec<String> = {
                let mut namer = id::Namer::new(&place);
                sp.elements
                    .iter()
                    .enumerate()
                    .map(|(at, e)| namer.next(at, e))
                    .collect()
            };
            for ((at, elem), cid) in sp.elements.into_iter().enumerate().zip(elem_refs) {
                let row_id = RowId(rows.len() as u32);
                let local_space_idx = u32::try_from(sp_bounds.len()).unwrap_or(0);
                sp_bounds.push(model_shape_bounds(&elem.shape, &symbol_bounds));
                let (instance, record_idx, sfc_slot, p21_kind, p21_name, is_grouped) =
                    make_row_meta(&elem.source, &backing, &record_meta, &grouped_set);
                let drafts = model_drafts
                    .remove(&(sp_id, at))
                    .unwrap_or_default()
                    .into_boxed_slice();
                rows.push(SxfRow {
                    instance,
                    unique_handle: None,
                    record_idx,
                    local_space_idx,
                    sfc_slot,
                    p21_kind,
                    p21_name,
                    grouped: is_grouped,
                    container: SxfContainer::Space(sp_id),
                    canonical_id: cid.into_boxed_str(),
                    placed: Placed::Model(elem),
                    drafts,
                    diagnostics: Vec::new(),
                });
                r_ids.push(row_id);
            }
            let sp_grid = SpatialGrid::build_with_cx(sp_bounds.as_slice(), cx).map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Error::Stopped(stop)
            })?;
            space_spatial.insert(
                sp_id,
                SpaceSpatialIndex {
                    grid: sp_grid,
                    bounds: sp_bounds,
                    rows: r_ids.clone(),
                },
            );
            space_rows.push(r_ids);
        }

        let mut symbol_templates = Vec::with_capacity(drawing.symbols.len());
        let mut symbol_rows = Vec::with_capacity(drawing.symbols.len());
        let mut def_namer = id::DefinitionNamer::default();
        for (sym_idx, sym_def) in drawing.symbols.into_iter().enumerate() {
            let place = def_namer.next(&sym_def);
            let sp_id = sym_def.space.id;
            symbol_templates.push(SymbolTemplate {
                key: sym_def.key,
                space_id: sp_id,
                unit: sym_def.space.unit,
                axes: sym_def.space.axes,
                base: sym_def.base,
                source: sym_def.source,
            });
            let elem_refs: Vec<String> = {
                let mut namer = id::Namer::new(&place);
                sym_def
                    .space
                    .elements
                    .iter()
                    .enumerate()
                    .map(|(at, e)| namer.next(at, e))
                    .collect()
            };
            let mut r_ids = Vec::with_capacity(sym_def.space.elements.len());
            for ((at, elem), cid) in sym_def
                .space
                .elements
                .into_iter()
                .enumerate()
                .zip(elem_refs)
            {
                let row_id = RowId(rows.len() as u32);
                let (instance, record_idx, sfc_slot, p21_kind, p21_name, is_grouped) =
                    make_row_meta(&elem.source, &backing, &record_meta, &grouped_set);
                let drafts = model_drafts
                    .remove(&(sp_id, at))
                    .unwrap_or_default()
                    .into_boxed_slice();
                rows.push(SxfRow {
                    instance,
                    unique_handle: None,
                    record_idx,
                    local_space_idx: 0,
                    sfc_slot,
                    p21_kind,
                    p21_name,
                    grouped: is_grouped,
                    container: SxfContainer::Symbol(sym_idx),
                    canonical_id: cid.into_boxed_str(),
                    placed: Placed::Model(elem),
                    drafts,
                    diagnostics: Vec::new(),
                });
                r_ids.push(row_id);
            }
            symbol_rows.push(r_ids);
        }

        let mut sheet_namer = id::SheetNamer::default();
        let sheet_place = sheet_namer.next(&sheet);
        let mut sheet_name_map = BTreeMap::new();
        sheet_name_map.insert(sheet_place.clone(), sheet.id);

        let sheet_cids: Vec<String> = {
            let mut namer = id::Namer::new(&sheet_place);
            sheet
                .elements
                .iter()
                .enumerate()
                .map(|(at, e)| namer.next(at, e))
                .collect()
        };
        let mut paper_idx_to_row: Vec<RowId> = Vec::with_capacity(sheet.elements.len());
        for ((at, elem), cid) in sheet.elements.into_iter().enumerate().zip(sheet_cids) {
            let row_id = RowId(rows.len() as u32);
            let (instance, record_idx, sfc_slot, p21_kind, p21_name, is_grouped) =
                make_row_meta(&elem.source, &backing, &record_meta, &grouped_set);
            let drafts = paper_drafts
                .remove(&(sheet.id, at))
                .unwrap_or_default()
                .into_boxed_slice();
            rows.push(SxfRow {
                instance,
                unique_handle: None,
                record_idx,
                local_space_idx: 0,
                sfc_slot,
                p21_kind,
                p21_name,
                grouped: is_grouped,
                container: SxfContainer::Sheet(sheet.id),
                canonical_id: cid.into_boxed_str(),
                placed: Placed::Paper(elem),
                drafts,
                diagnostics: Vec::new(),
            });
            paper_idx_to_row.push(row_id);
        }

        let mut base_sheet_slots = Vec::with_capacity(sheet.order.len());
        for item in sheet.order {
            match item {
                SheetItem::View(v) => base_sheet_slots.push(BaseSheetSlot::View(v)),
                SheetItem::Element(idx) => {
                    if let Some(&r_id) = paper_idx_to_row.get(idx) {
                        base_sheet_slots.push(BaseSheetSlot::Row(r_id));
                    }
                }
            }
        }

        let sheet_template = SheetTemplate {
            id: sheet.id,
            name: sheet.name,
            size: sheet.size,
            views: sheet.views,
            source: sheet.source,
        };

        let diagnostics = LiveDiagnostics::build(
            events.details,
            &records,
            file,
            &mut rows,
            &lifted.tables,
            (&base_sheet_slots, &sheet_template),
            cx,
        )?;
        // 空の複合曲線の線種も使用順から求めるので、初期変換の補助表は持ち続けない。
        lifted.tables.extra_line_types.clear();
        lifted.tables.extra_line_types.shrink_to_fit();

        let mut inst_counts: BTreeMap<u64, u32> = BTreeMap::new();
        for row in &rows {
            if let Some(inst) = row.instance {
                *inst_counts.entry(inst).or_default() += 1;
            }
        }
        let shared_instances: BTreeSet<u64> = inst_counts
            .into_iter()
            .filter_map(|(inst, cnt)| (cnt > 1).then_some(inst))
            .collect();

        let mut by_canonical_id = BTreeMap::new();
        let mut display = Display::with_rows_with_cx(cx, rows.len()).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        let disp_ctx = DisplayContext {
            space_views: &space_views,
            symbol_indices: &symbol_indices,
        };

        let mut initial_rows = Vec::with_capacity(rows.len());
        for (i, mut row) in rows.into_iter().enumerate() {
            let row_id = RowId(i as u32);
            by_canonical_id.insert(row.canonical_id.to_string(), row_id);
            if matches!(
                row.container,
                SxfContainer::Space(_) | SxfContainer::Sheet(_)
            ) && let Some(vnode) = placed_symbol_dep_source(&row.placed, &symbol_indices)
            {
                dep.set_sources(row_id, [vnode]);
            }
            row.unique_handle = match (row.instance, placed_source(&row.placed).record()) {
                (Some(inst), Some(_)) if !shared_instances.contains(&inst) => Some(inst),
                _ => None,
            };
            let bounds =
                compute_sheet_bounds(row.container, &row.placed, &space_views, &symbol_bounds);
            let (counts, style) = display_counts_and_style(&row, &disp_ctx);
            display
                .push_with_cx(cx, counts, style, |w| {
                    write_display_row(&row, &disp_ctx, w);
                })
                .map_err(|stop| {
                    cx.report(stop.to_diagnostic());
                    Error::Stopped(stop)
                })?;
            initial_rows.push((row, bounds, true));
        }
        display.clear_dirty();

        let base_row_count = initial_rows.len() as u32;
        let table = VersionedTable::from_rows_with_cx(initial_rows, cx).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        let spatial = SpatialGrid::build_with_cx(table.bounds_column(), cx).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;

        Ok(Self {
            backing,
            attributes,
            diagnostics,
            lifter_tables: lifted.tables,
            fonts_broken,
            static_layer_ids,
            static_user_lt_ids,
            raster_groups: lifted.raster_groups,
            record_to_raster_group,
            space_templates,
            space_rows,
            symbol_templates,
            symbol_rows,
            sheet_template,
            base_sheet_slots,
            base_row_count,
            title_blocks: drawing.title_blocks,
            doc_drafts,
            sheet_name_map,
            space_views,
            symbol_bounds,
            symbol_indices,
            dirty_symbols: BTreeSet::new(),
            table,
            by_canonical_id,
            spatial,
            space_spatial,
            dep,
            display,
            history: History::new(),
            history_max_bytes: DEFAULT_HISTORY_LIMIT,
        })
    }

    /// 部分図と配置要素の依存グラフへの参照
    #[inline]
    pub const fn dep_graph(&self) -> &DepGraph {
        &self.dep
    }

    /// 部分図定義の添字（`Drawing2D::symbols` 上の位置）に対応する仮想依存ノード ID を返す
    #[inline]
    pub const fn symbol_dep_node(sym_index: u32) -> Option<RowId> {
        RowId::virtual_node(sym_index)
    }

    /// 変更された行または仮想ノード群 `changed` から推移的に影響を受けるノードを返す
    pub fn affected_rows(&self, changed: &[RowId], cx: &mut Cx<'_>) -> Result<Vec<RowId>, Error> {
        self.dep.affected_rows_with_cx(changed, cx).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })
    }

    /// 内容または入れ子先の変更により再送が必要な作図部品（`symbols`）の添字集合への参照
    #[inline]
    pub const fn dirty_symbols(&self) -> &BTreeSet<u32> {
        &self.dirty_symbols
    }

    /// 作図部品の変更フラグを消去する
    #[inline]
    pub fn clear_dirty_symbols(&mut self) {
        self.dirty_symbols.clear();
    }

    /// `undo` / `redo` 履歴の上限バイト数を設定する
    pub fn with_history_limit(mut self, max_bytes: usize) -> Self {
        self.set_history_max_bytes(max_bytes);
        self
    }

    /// `undo` / `redo` 履歴の上限バイト数を動的に更新する
    pub fn set_history_max_bytes(&mut self, max_bytes: usize) {
        self.history_max_bytes = max_bytes;
        self.history.trim_to_bytes(max_bytes);
    }

    /// 履歴を空にする
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// 表示バッファーの GPU 変更追跡ビットを消去する
    pub fn clear_display_dirty(&mut self) {
        self.display.clear_dirty();
    }

    /// SFC か P21 か
    #[inline]
    pub fn mode(&self) -> Mode {
        self.backing.mode()
    }

    /// 現在のリビジョン番号（開いた直後は `0`、編集や `undo` / `redo` のたびに単調増加）
    #[inline]
    pub const fn revision(&self) -> Revision {
        self.table.revision()
    }

    /// 現在の論理状態識別子（`undo` で元の状態に戻ると同じ値に戻る）
    #[inline]
    pub const fn state_id(&self) -> u64 {
        self.table.state_id()
    }

    /// 生存している要素の数
    #[inline]
    pub const fn live_count(&self) -> u32 {
        self.table.live_count()
    }

    /// 削除済みスロットを含む総行数
    #[inline]
    pub fn row_count(&self) -> usize {
        self.table.len()
    }

    /// 取り消せる編集があるか
    #[inline]
    pub fn can_undo(&self) -> bool {
        self.history.undo_len() > 0
    }

    /// やり直せる編集があるか
    #[inline]
    pub fn can_redo(&self) -> bool {
        self.history.redo_len() > 0
    }

    /// 取り消せる段数
    #[inline]
    pub fn undo_len(&self) -> usize {
        self.history.undo_len()
    }

    /// やり直せる段数
    #[inline]
    pub fn redo_len(&self) -> usize {
        self.history.redo_len()
    }

    /// 常駐データ構造の概算メモリ使用量（バイト）
    pub fn resident_bytes(&self) -> usize {
        self.backing.resident_bytes()
            + self.diagnostics.bytes()
            + self.table.len() * core::mem::size_of::<SxfRow>()
            + self.spatial.bytes()
            + self.display.bytes()
            + self.history.bytes()
    }

    /// 履歴が保持している概算バイト数
    #[inline]
    pub fn history_bytes(&self) -> usize {
        self.history.bytes()
    }

    /// シート座標系の空間索引
    #[inline]
    pub const fn spatial(&self) -> &SpatialGrid {
        &self.spatial
    }

    /// 差分更新される表示バッファーへの参照
    #[inline]
    pub const fn display(&self) -> &Display {
        &self.display
    }

    /// 差分更新される表示バッファーへの可変参照（描画側の `clear_dirty` 用）
    #[inline]
    pub fn display_mut(&mut self) -> &mut Display {
        &mut self.display
    }

    /// シート座標の外接矩形と交差する生存要素の [`RowId`] を昇順で返す
    pub fn select_sheet(&self, rect: [f64; 4]) -> Vec<RowId> {
        let mut raw = Vec::new();
        self.spatial
            .query(self.table.bounds_column(), rect, &mut raw);
        raw.into_iter()
            .map(RowId)
            .filter(|&r| self.table.is_alive(r))
            .collect()
    }

    /// 部分図空間 `space` の局所座標の外接矩形と交差する生存要素の [`RowId`] を昇順で返す
    pub fn select_in_space(&self, space: SpaceId, rect: [f64; 4]) -> Vec<RowId> {
        let Some(sp_idx) = self.space_spatial.get(&space) else {
            return Vec::new();
        };
        let mut raw = Vec::new();
        sp_idx.grid.query(sp_idx.bounds.as_slice(), rect, &mut raw);
        raw.into_iter()
            .filter_map(|local| sp_idx.rows.get(local as usize).copied())
            .filter(|&r| self.table.is_alive(r))
            .collect()
    }

    /// 一意なインスタンス番号 `#n` から生存 [`RowId`] を `O(1)` で引く
    pub fn row_id_for_instance(&self, instance: u64) -> Option<RowId> {
        self.table.find_handle(instance)
    }

    /// 正規 ID（`sheet/0/i70` など）から生存 [`RowId`] を引く
    pub fn row_id_for_canonical_id(&self, id: &str) -> Option<RowId> {
        let &row = self.by_canonical_id.get(id)?;
        self.table.is_alive(row).then_some(row)
    }

    /// [`RowId`] の正規 ID を返す
    pub fn canonical_id(&self, row: RowId) -> Option<&str> {
        Some(&self.table.row(row)?.canonical_id)
    }

    /// [`RowId`] の保持値を借りる。出所の番号は保存順に補正していない。
    /// 保存した図面と照らすときは [`Self::placed_with_source`] を使う。
    pub fn placed(&self, row: RowId) -> Option<&Placed> {
        Some(&self.table.row(row)?.placed)
    }

    /// 保存順の出所と参照を伴う要素を返す。コピーと補正は呼び出し元の予算で数える。
    pub fn placed_with_source(&self, row: RowId, cx: &mut Cx<'_>) -> Result<Option<Placed>, Error> {
        work(cx, 1, &Ref::default())?;
        let Some(placed) = self.placed(row) else {
            return Ok(None);
        };
        let before = cx.budget.used(Kind::Work);
        count_placed_copy(placed, cx, 1)?;
        // ハッシュの 1 回の書き込みは高々 1024 バイト。コピーする列と文字列の容量を予約する。
        let bytes = cx
            .budget
            .used(Kind::Work)
            .saturating_sub(before)
            .saturating_mul(1024)
            .saturating_add(placed_source(placed).id().map_or(0, str::len) as u64);
        cx.check(Kind::DecodedBytes, bytes).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        let sources = self.backing.source_map(cx)?;
        let mut placed = placed.clone();
        sources.placed(&mut placed, cx)?;
        Ok(Some(placed))
    }

    fn named_tables(&self) -> NamedTables {
        NamedTables {
            layers: self.lifter_tables.base_layers.clone(),
            defined_layers: self.lifter_tables.defined_layers,
            fonts: self.lifter_tables.fonts.clone(),
        }
    }

    fn set_tables(&mut self, tables: &NamedTables) {
        self.lifter_tables.base_layers.clone_from(&tables.layers);
        self.lifter_tables.defined_layers = tables.defined_layers;
        self.lifter_tables.fonts.clone_from(&tables.fonts);
        self.static_layer_ids = tables
            .layers
            .iter()
            .filter(|l| l.state.is_some())
            .map(|l| l.id.0)
            .collect();
    }

    /// 文書のレイヤの表（要素が参照する未定義のレイヤも含む）
    fn current_layers(&self, cx: &mut Cx) -> Result<Vec<Layer>, Error> {
        let mut layers = self.lifter_tables.base_layers.clone();
        let mut line_types = Vec::new();
        for (_, row) in self.table.iter_live() {
            work(cx, 1 + layers.len() as u64 / 64, &Ref::default())?;
            let (layer, style) = match &row.placed {
                Placed::Model(e) => (e.layer, &e.style),
                Placed::Paper(e) => (e.layer, &e.style),
            };
            ensure_element_tables(
                self.lifter_tables.defined_layers,
                &mut layers,
                &mut line_types,
                layer,
                style,
            );
        }
        Ok(layers)
    }

    /// 段取りの終わった表を確かめに使い、確定できなければ元の表に戻す
    fn commit_with_tables(
        &mut self,
        stage: TableStage,
        cx: &mut Cx,
        commit: impl FnOnce(&mut Self, Option<Box<TablesDiff>>, &mut Cx) -> Result<Vec<Applied>, Error>,
    ) -> Result<Vec<Applied>, Error> {
        let TableStage {
            fonts,
            layers: changed,
            added_layers,
            estimated_texts,
            ..
        } = stage;
        let before = self.named_tables();
        let mut after = before.clone();
        let mut dirty = !changed.is_empty();
        if let Some(names) = fonts.into_names()
            && names.len() != after.fonts.len()
        {
            after.fonts = names;
            dirty = true;
        }
        for layer in changed {
            if let Some(slot) = after.layers.iter_mut().find(|l| l.id == layer.id) {
                *slot = layer;
            } else {
                after.layers.push(layer);
            }
        }
        after.defined_layers = after.defined_layers.saturating_add(added_layers);
        let tables = dirty.then(|| {
            Box::new(TablesDiff {
                before: before.clone(),
                after,
            })
        });
        if let Some(diff) = &tables {
            self.set_tables(&diff.after);
        }
        let applied = match commit(self, tables, cx) {
            Ok(applied) => applied,
            Err(error) => {
                if dirty {
                    self.set_tables(&before);
                }
                return Err(error);
            }
        };
        for instance in estimated_texts {
            self.report_at(
                Diagnostic::new(codes::SXF_TEXT_WIDTH_APPROXIMATED),
                &Ref::default().with_key(Key::Instance(instance)),
                cx,
            )?;
        }
        Ok(applied)
    }

    /// SFC の要素の形の引数。文字は字体を引き、元のフィーチャのコードも引数に入れる（バッチと同じ）
    #[allow(clippy::too_many_arguments)]
    fn sfc_shape_args(
        sfc: &SfcState,
        tx: &mut SfcStepTx,
        slot: SfcSlot,
        name_lower: &str,
        is_grouped: bool,
        expected: &mut Placed,
        fonts: &mut Fonts<'static>,
        cx: &mut Cx,
    ) -> Result<Option<Arguments>, Error> {
        let annotation = name_lower == text::NAME;
        if !(name_lower == MARKER || annotation || SHAPED.contains(&name_lower)) || is_grouped {
            return Ok(None);
        }
        let original = if annotation {
            match sfc.parse_staged_feature(tx, slot, cx)? {
                Some((_, record)) => Some(record),
                None => return Ok(None),
            }
        } else {
            None
        };
        let mut writer = LiveSfcWriter { sfc, tx };
        let Some(args) = shape_arguments(expected, original.as_ref(), fonts, &mut writer, cx)?
        else {
            return Ok(None);
        };
        Ok((args.name == name_lower).then_some(args))
    }

    fn current_tables(&self) -> Tables {
        let widths = match &self.backing {
            SxfBacking::Sfc(_) => self.lifter_tables.widths.clone(),
            SxfBacking::P21(p21) => p21.active_widths(),
        };
        Tables::from_live(
            self.static_layer_ids.clone(),
            self.static_user_lt_ids.clone(),
            self.lifter_tables.colors.clone(),
            widths,
        )
    }

    fn lookup_target(
        &self,
        seen: &Seen,
        targets: &mut BTreeSet<u64>,
        (format, edit): (&str, &str),
        cx: &mut Cx,
    ) -> Result<Option<(RowId, u64)>, Error> {
        let Some(&row_id) = self.by_canonical_id.get(seen.id.as_str()) else {
            cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", seen.id.as_str()));
            return Ok(None);
        };
        let Some(row) = self.table.row(row_id) else {
            cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", seen.id.as_str()));
            return Ok(None);
        };
        let elem_hash = hash_placed(&row.placed, cx)?;
        if elem_hash != seen.hash {
            cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_STALE).with("id", seen.id.as_str()));
            return Ok(None);
        }
        let source = placed_source(&row.placed);
        let valid_inst = row
            .unique_handle
            .filter(|&inst| self.table.find_handle(inst) == Some(row_id));
        let Some(inst) = valid_inst.filter(|&inst| targets.insert(inst)) else {
            self.report_at(unsupported(format, edit, "shared"), source, cx)?;
            return Ok(None);
        };
        Ok(Some((row_id, inst)))
    }

    fn report_at(
        &self,
        diagnostic: Diagnostic,
        source: &Ref,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        let sources = self.backing.source_map(cx)?;
        let mut source = source.clone();
        sources.remap(&mut source, cx)?;
        cx.report(diagnostic.at(source));
        Ok(())
    }

    /// 変更群を `O(変更数)` で差分適用する
    pub fn apply(&mut self, changes: &Changes, cx: &mut Cx<'_>) -> Result<Vec<Applied>, Error> {
        let current = match changes.document {
            None => true,
            Some(expected_hash) => {
                let saved = self.backing.save(cx)?;
                let same = Hash::document_checked(&saved, cx).map_err(|stop| {
                    cx.report(stop.to_diagnostic());
                    Error::Stopped(stop)
                })? == expected_hash;
                if !same {
                    cx.report(Diagnostic::new(codes::CHANGE_DOCUMENT_STALE));
                }
                same
            }
        };

        let tables = self.current_tables();
        let mut targets: BTreeSet<u64> = BTreeSet::new();
        let mut rejected = false;

        match &self.backing {
            SxfBacking::Sfc(sfc) => {
                let mut tx = sfc.begin_step();
                let mut staged_ops = Vec::new();
                let mut stage = TableStage::new(&self.lifter_tables.fonts, self.fonts_broken);
                for change in &changes.edits {
                    work(cx, 1, &Ref::default())?;
                    let staged = self.stage_one_sfc(
                        sfc,
                        &mut tx,
                        &tables,
                        change,
                        &mut targets,
                        &mut stage,
                        cx,
                    )?;
                    match staged {
                        Some(op) => staged_ops.push((change.name(), op)),
                        None => rejected = true,
                    }
                }
                if rejected || !current {
                    return Err(Error::Rejected);
                }
                self.commit_with_tables(stage, cx, |this, diff, cx| {
                    this.verify_and_commit_sfc(tx, staged_ops, diff, cx)
                })
            }
            SxfBacking::P21(p21) => {
                let mut tx = p21.begin_step();
                let mut staged_ops = Vec::new();
                let mut stage = TableStage::new(&self.lifter_tables.fonts, self.fonts_broken);
                for change in &changes.edits {
                    work(cx, 1, &Ref::default())?;
                    let staged = self.stage_one_p21(
                        p21,
                        &mut tx,
                        &tables,
                        change,
                        &mut targets,
                        &mut stage,
                        cx,
                    )?;
                    match staged {
                        Some(op) => staged_ops.push((change.name(), op)),
                        None => rejected = true,
                    }
                }
                if rejected || !current {
                    return Err(Error::Rejected);
                }
                p21.collect_garbage(&mut tx, cx)?;
                self.commit_with_tables(stage, cx, |this, diff, cx| {
                    this.verify_and_commit_p21(tx, &tables.widths, staged_ops, diff, cx)
                })
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn stage_one_sfc(
        &self,
        sfc: &SfcState,
        tx: &mut SfcStepTx,
        tables: &Tables,
        change: &Edit,
        targets: &mut BTreeSet<u64>,
        stage: &mut TableStage,
        cx: &mut Cx,
    ) -> Result<Option<StagedSfcOp>, Error> {
        let format = "SFC";
        match change {
            Edit::Remove { target } => {
                let Some((row_id, inst)) =
                    self.lookup_target(target, targets, (format, "remove"), cx)?
                else {
                    return Ok(None);
                };
                let Some(row) = self.table.row(row_id) else {
                    return Ok(None);
                };
                let meta = row.sfc_slot.and_then(|slot| sfc.slot_meta(slot));
                let kind_str = meta.map(|(_, k, _, _)| k).unwrap_or(row.p21_kind.as_ref());
                let ok = row.sfc_slot.and_then(|slot| {
                    let (name_lower, _, is_grouped, _) = meta?;
                    if !REMOVABLE.contains(&name_lower) || is_grouped {
                        return None;
                    }
                    sfc.stage_remove(tx, slot)?;
                    Some(StagedSfcOp::Removed { row_id, inst })
                });
                if ok.is_none() {
                    self.report_at(
                        unsupported(format, "remove", kind_str),
                        placed_source(&row.placed),
                        cx,
                    )?;
                }
                Ok(ok)
            }
            Edit::Restyle { target, style } => {
                let Some((row_id, _)) =
                    self.lookup_target(target, targets, (format, "restyle"), cx)?
                else {
                    return Ok(None);
                };
                let Some(row) = self.table.row(row_id) else {
                    return Ok(None);
                };
                let (new_layer, new_style) = over_placed(&row.placed, style);
                work(
                    cx,
                    tables.style_work(&new_style, Some(placed_style(&row.placed))),
                    &Ref::default(),
                )?;
                count_placed_copy(&row.placed, cx, 1)?;
                let mut expected = with_layer_style(&row.placed, new_layer, new_style, None);
                let meta = row.sfc_slot.and_then(|slot| sfc.slot_meta(slot));
                let kind_str = meta.map(|(_, k, _, _)| k).unwrap_or(row.p21_kind.as_ref());
                let ok = row.sfc_slot.and_then(|slot| {
                    let (name_lower, _, is_grouped, _) = meta?;
                    let codes = compute_style_codes(
                        name_lower,
                        is_grouped,
                        &row.placed,
                        &mut expected,
                        tables,
                        Annotation::Restyle,
                    )?;
                    if !codes.is_empty() {
                        sfc.stage_codes(tx, slot, &codes)?;
                    }
                    Some(StagedSfcOp::Updated {
                        row_id,
                        slot,
                        expected: Box::new(expected),
                    })
                });
                if ok.is_none() {
                    self.report_at(
                        unsupported(format, "restyle", kind_str),
                        placed_source(&row.placed),
                        cx,
                    )?;
                }
                Ok(ok)
            }
            Edit::Replace { target, element } => {
                let Some((row_id, inst)) =
                    self.lookup_target(target, targets, (format, "replace"), cx)?
                else {
                    return Ok(None);
                };
                let _ = inst;
                let Some(row) = self.table.row(row_id) else {
                    return Ok(None);
                };
                count_placed_copy(element, cx, 1)?;
                let replaced = replaced_placed(&row.placed, element);
                if let Some(rep) = &replaced {
                    work(
                        cx,
                        tables.style_work(placed_style(rep), Some(placed_style(&row.placed))),
                        &Ref::default(),
                    )?;
                }
                let meta = row.sfc_slot.and_then(|slot| sfc.slot_meta(slot));
                let kind_str = meta.map(|(_, k, _, _)| k).unwrap_or(row.p21_kind.as_ref());
                let mut shape_counted = false;
                if let Some(rep) = &replaced {
                    count_placed_shape(rep, cx)?;
                    shape_counted = true;
                }
                let _ = shape_counted;
                let mut ok = None;
                if let Some(mut expected) = replaced
                    && let Some(slot) = row.sfc_slot
                    && let Some((name_lower, _, is_grouped, _)) = meta
                    && let Some(codes) = compute_style_codes(
                        name_lower,
                        is_grouped,
                        &row.placed,
                        &mut expected,
                        tables,
                        Annotation::Rewrite,
                    )
                    && (codes.is_empty() || sfc.stage_codes(tx, slot, &codes).is_some())
                    && let Some(shape_args) = Self::sfc_shape_args(
                        sfc,
                        tx,
                        slot,
                        name_lower,
                        is_grouped,
                        &mut expected,
                        &mut stage.fonts,
                        cx,
                    )?
                    && sfc.stage_shape(tx, slot, &shape_args).is_some()
                {
                    ok = Some(StagedSfcOp::Updated {
                        row_id,
                        slot,
                        expected: Box::new(expected),
                    });
                }
                if ok.is_none() {
                    self.report_at(
                        unsupported(format, "replace", kind_str),
                        placed_source(&row.placed),
                        cx,
                    )?;
                }
                Ok(ok)
            }
            Edit::Transform { target, transform } => {
                let Some((row_id, _)) =
                    self.lookup_target(target, targets, (format, "transform"), cx)?
                else {
                    return Ok(None);
                };
                let Some(row) = self.table.row(row_id) else {
                    return Ok(None);
                };
                count_placed_copy(&row.placed, cx, 2)?;
                let transformed = transformed_placed(&row.placed, transform);
                if let Some(tr) = &transformed {
                    count_placed_shape(tr, cx)?;
                }
                let meta = row.sfc_slot.and_then(|slot| sfc.slot_meta(slot));
                let kind_str = meta.map(|(_, k, _, _)| k).unwrap_or(row.p21_kind.as_ref());
                let mut ok = None;
                if let Some(mut expected) = transformed
                    && let Some(slot) = row.sfc_slot
                    && let Some((name_lower, _, is_grouped, _)) = meta
                    && let Some(shape_args) = Self::sfc_shape_args(
                        sfc,
                        tx,
                        slot,
                        name_lower,
                        is_grouped,
                        &mut expected,
                        &mut stage.fonts,
                        cx,
                    )?
                    && sfc.stage_shape(tx, slot, &shape_args).is_some()
                {
                    ok = Some(StagedSfcOp::Updated {
                        row_id,
                        slot,
                        expected: Box::new(expected),
                    });
                }
                if ok.is_none() {
                    self.report_at(
                        unsupported(format, "transform", kind_str),
                        placed_source(&row.placed),
                        cx,
                    )?;
                }
                Ok(ok)
            }
            Edit::Add { place, element } => {
                let mut unset = None;
                let mut added = None;
                if let Placed::Paper(paper_elem) = element.as_ref()
                    && self.sheet_name_map.get(place) == Some(&paper_elem.shape_sheet())
                {
                    work(
                        cx,
                        tables.style_work(&paper_elem.style, None),
                        &Ref::default(),
                    )?;
                    count_element_copy(paper_elem, cx, 1)?;
                    count_shape(&paper_elem.shape, cx)?;
                    text::count(&paper_elem.shape, cx)?;
                    let mut placed = Placed::Paper(paper_elem.clone());
                    let args = {
                        let mut writer = LiveSfcWriter { sfc, tx };
                        shape_arguments(&mut placed, None, &mut stage.fonts, &mut writer, cx)?
                    };
                    if let Some(args) = args
                        && let Placed::Paper(elem) = placed
                        && let Some((add_args, expected_elem)) =
                            prepare_add_args(elem, args, tables, &mut unset)
                        && let Some((seq, inst)) = sfc.stage_add(tx, &add_args)
                    {
                        if add_args.name == text::NAME {
                            stage.estimated_texts.push(inst);
                        }
                        added = Some(StagedSfcOp::Added {
                            seq,
                            inst,
                            place_name: place.clone(),
                            sheet_id: self.sheet_template.id,
                            expected: Box::new(Placed::Paper(expected_elem)),
                        });
                    }
                }
                if let Some(field) = unset {
                    cx.report(
                        Diagnostic::new(codes::CHANGE_STYLE_UNSUPPORTED)
                            .with("field", field)
                            .with("format", format),
                    );
                    return Ok(None);
                }
                if added.is_none() {
                    cx.report(unsupported(format, "add", kind_of(element)));
                }
                Ok(added)
            }
            Edit::ReplaceLayer { target, layer } => {
                if stage.before_layers.is_none() {
                    stage.before_layers = Some(self.current_layers(cx)?);
                }
                let before = stage.before_layers.as_deref().unwrap_or_default();
                let mut writer = LiveSfcWriter { sfc, tx };
                let Some(layer) =
                    stage
                        .layer_edits
                        .replace(before, target, layer, &mut writer, format, cx)?
                else {
                    return Ok(None);
                };
                Ok(stage.staged_layer(sfc, tx, layer, false, format, cx))
            }
            Edit::AddLayer { layer } => {
                if stage.before_layers.is_none() {
                    stage.before_layers = Some(self.current_layers(cx)?);
                }
                let before = stage.before_layers.as_deref().unwrap_or_default();
                let mut writer = LiveSfcWriter { sfc, tx };
                let Some(layer) = stage
                    .layer_edits
                    .add(before, layer, &mut writer, format, cx)?
                else {
                    return Ok(None);
                };
                Ok(stage.staged_layer(sfc, tx, layer, true, format, cx))
            }
            other => {
                report_table_edit_unsupported(format, other, cx);
                Ok(None)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn stage_one_p21(
        &self,
        p21: &P21State,
        tx: &mut P21StepTx,
        tables: &Tables,
        change: &Edit,
        targets: &mut BTreeSet<u64>,
        stage: &mut TableStage,
        cx: &mut Cx,
    ) -> Result<Option<StagedP21Op>, Error> {
        let format = "P21";
        match change {
            Edit::Remove { target } => {
                let Some((row_id, inst)) =
                    self.lookup_target(target, targets, (format, "remove"), cx)?
                else {
                    return Ok(None);
                };
                let Some(row) = self.table.row(row_id) else {
                    return Ok(None);
                };
                let stmt_opt = p21.stmt_for_instance(inst);
                let ok = if REMOVABLE.contains(&row.p21_name.as_ref()) && !row.grouped {
                    if let Some(stmt) = stmt_opt {
                        p21.stage_remove(tx, stmt, cx)?
                            .map(|()| StagedP21Op::Removed { row_id, inst })
                    } else {
                        None
                    }
                } else {
                    None
                };
                if ok.is_none() {
                    self.report_at(
                        unsupported(format, "remove", &row.p21_kind),
                        placed_source(&row.placed),
                        cx,
                    )?;
                }
                Ok(ok)
            }
            Edit::Restyle { target, style } => {
                let Some((row_id, inst)) =
                    self.lookup_target(target, targets, (format, "restyle"), cx)?
                else {
                    return Ok(None);
                };
                let Some(row) = self.table.row(row_id) else {
                    return Ok(None);
                };
                let (new_layer, new_style) = over_placed(&row.placed, style);
                work(
                    cx,
                    tables.style_work(&new_style, Some(placed_style(&row.placed))),
                    &Ref::default(),
                )?;
                count_placed_copy(&row.placed, cx, 1)?;
                let mut expected = with_layer_style(&row.placed, new_layer, new_style, None);
                let stmt_opt = p21.stmt_for_instance(inst);
                let codes_opt = compute_style_codes(
                    &row.p21_name,
                    row.grouped,
                    &row.placed,
                    &mut expected,
                    tables,
                    Annotation::Restyle,
                );
                let ok = match (stmt_opt, codes_opt) {
                    (Some(stmt), Some(codes)) => {
                        let applied = if codes.is_empty() {
                            Some(())
                        } else {
                            p21.stage_codes(tx, &tables.widths, stmt, &codes, cx)?
                        };
                        applied.map(|()| StagedP21Op::Updated {
                            row_id,
                            inst,
                            expected: Box::new(expected),
                        })
                    }
                    _ => None,
                };
                if ok.is_none() {
                    self.report_at(
                        unsupported(format, "restyle", &row.p21_kind),
                        placed_source(&row.placed),
                        cx,
                    )?;
                }
                Ok(ok)
            }
            Edit::Replace { target, element } => {
                let Some((row_id, inst)) =
                    self.lookup_target(target, targets, (format, "replace"), cx)?
                else {
                    return Ok(None);
                };
                let Some(row) = self.table.row(row_id) else {
                    return Ok(None);
                };
                count_placed_copy(element, cx, 1)?;
                let replaced = replaced_placed(&row.placed, element);
                if let Some(rep) = &replaced {
                    work(
                        cx,
                        tables.style_work(placed_style(rep), Some(placed_style(&row.placed))),
                        &Ref::default(),
                    )?;
                }
                let stmt_opt = p21.stmt_for_instance(inst);
                let ok = if let (Some(stmt), Some(mut expected)) = (stmt_opt, replaced) {
                    let style_ok = if let Some(codes) = compute_style_codes(
                        &row.p21_name,
                        row.grouped,
                        &row.placed,
                        &mut expected,
                        tables,
                        Annotation::Rewrite,
                    ) {
                        if codes.is_empty() {
                            Some(())
                        } else {
                            p21.stage_codes(tx, &tables.widths, stmt, &codes, cx)?
                        }
                    } else {
                        None
                    };
                    if style_ok.is_some() {
                        count_placed_shape(&expected, cx)?;
                        if let Some(shape_args) = Self::p21_shape_args(
                            p21,
                            tx,
                            &tables.widths,
                            (inst, row.record_idx),
                            (&row.p21_name, row.grouped),
                            &mut expected,
                            &mut stage.fonts,
                            cx,
                        )? {
                            p21.stage_shape(tx, stmt, &shape_args, cx)?.map(|()| {
                                StagedP21Op::Updated {
                                    row_id,
                                    inst,
                                    expected: Box::new(expected),
                                }
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };
                if ok.is_none() {
                    self.report_at(
                        unsupported(format, "replace", &row.p21_kind),
                        placed_source(&row.placed),
                        cx,
                    )?;
                }
                Ok(ok)
            }
            Edit::Transform { target, transform } => {
                let Some((row_id, inst)) =
                    self.lookup_target(target, targets, (format, "transform"), cx)?
                else {
                    return Ok(None);
                };
                let Some(row) = self.table.row(row_id) else {
                    return Ok(None);
                };
                count_placed_copy(&row.placed, cx, 2)?;
                let transformed = transformed_placed(&row.placed, transform);
                let stmt_opt = p21.stmt_for_instance(inst);
                let ok = if let (Some(stmt), Some(mut expected)) = (stmt_opt, transformed) {
                    count_placed_shape(&expected, cx)?;
                    if let Some(shape_args) = Self::p21_shape_args(
                        p21,
                        tx,
                        &tables.widths,
                        (inst, row.record_idx),
                        (&row.p21_name, row.grouped),
                        &mut expected,
                        &mut stage.fonts,
                        cx,
                    )? {
                        p21.stage_shape(tx, stmt, &shape_args, cx)?
                            .map(|()| StagedP21Op::Updated {
                                row_id,
                                inst,
                                expected: Box::new(expected),
                            })
                    } else {
                        None
                    }
                } else {
                    None
                };
                if ok.is_none() {
                    self.report_at(
                        unsupported(format, "transform", &row.p21_kind),
                        placed_source(&row.placed),
                        cx,
                    )?;
                }
                Ok(ok)
            }
            Edit::Add { place, element } => {
                let mut unset = None;
                let mut add_err = None;
                let added = match element.as_ref() {
                    Placed::Paper(paper_elem)
                        if self.sheet_name_map.get(place) == Some(&paper_elem.shape_sheet()) =>
                    {
                        work(
                            cx,
                            tables.style_work(&paper_elem.style, None),
                            &Ref::default(),
                        )?;
                        count_element_copy(paper_elem, cx, 1)?;
                        count_shape(&paper_elem.shape, cx)?;
                        text::count(&paper_elem.shape, cx)?;
                        let mut placed = Placed::Paper(paper_elem.clone());
                        let args = {
                            let mut writer = LiveP21Writer {
                                p21,
                                tx,
                                widths: &tables.widths,
                            };
                            shape_arguments(&mut placed, None, &mut stage.fonts, &mut writer, cx)?
                        };
                        if let Some(args) = args
                            && let Placed::Paper(elem) = placed
                            && let Some((add_args, expected_elem)) =
                                prepare_add_args(elem, args, tables, &mut unset)
                        {
                            match p21.stage_add(tx, &tables.widths, &add_args, cx) {
                                Ok(Some(inst)) => Some(StagedP21Op::Added {
                                    inst,
                                    place_name: place.clone(),
                                    sheet_id: self.sheet_template.id,
                                    add_name: add_args.name,
                                    expected: Box::new(Placed::Paper(expected_elem)),
                                }),
                                Ok(None) => None,
                                Err(e) => {
                                    add_err = Some(e);
                                    None
                                }
                            }
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(e) = add_err {
                    return Err(e);
                }
                if let Some(field) = unset {
                    cx.report(
                        Diagnostic::new(codes::CHANGE_STYLE_UNSUPPORTED)
                            .with("field", field)
                            .with("format", format),
                    );
                    return Ok(None);
                }
                if added.is_none() {
                    cx.report(unsupported(format, "add", kind_of(element)));
                }
                if let Some(StagedP21Op::Added { inst, add_name, .. }) = &added
                    && *add_name == text::NAME
                {
                    stage.estimated_texts.push(*inst);
                }
                Ok(added)
            }
            Edit::ReplaceLayer { target, layer } => {
                if stage.before_layers.is_none() {
                    stage.before_layers = Some(self.current_layers(cx)?);
                }
                let before = stage.before_layers.as_deref().unwrap_or_default();
                let mut writer = LiveP21Writer {
                    p21,
                    tx,
                    widths: &tables.widths,
                };
                let Some(layer) =
                    stage
                        .layer_edits
                        .replace(before, target, layer, &mut writer, format, cx)?
                else {
                    return Ok(None);
                };
                Ok(Some(stage.p21_layer(layer, false)))
            }
            Edit::AddLayer { layer } => {
                if stage.before_layers.is_none() {
                    stage.before_layers = Some(self.current_layers(cx)?);
                }
                let before = stage.before_layers.as_deref().unwrap_or_default();
                let mut writer = LiveP21Writer {
                    p21,
                    tx,
                    widths: &tables.widths,
                };
                let Some(layer) = stage
                    .layer_edits
                    .add(before, layer, &mut writer, format, cx)?
                else {
                    return Ok(None);
                };
                Ok(Some(stage.p21_layer(layer, true)))
            }
            other => {
                report_table_edit_unsupported(format, other, cx);
                Ok(None)
            }
        }
    }

    /// P21 の要素の形の引数。文字は字体を引き、元の表記の実体から読んだフィーチャも引数に入れる（バッチと同じ）
    #[allow(clippy::too_many_arguments)]
    fn p21_shape_args(
        p21: &P21State,
        tx: &mut P21StepTx,
        widths: &[Option<f64>],
        (inst, record_idx): (u64, usize),
        (name_lower, is_grouped): (&str, bool),
        expected: &mut Placed,
        fonts: &mut Fonts<'static>,
        cx: &mut Cx,
    ) -> Result<Option<Arguments>, Error> {
        if name_lower != text::NAME {
            return Ok(compute_shape_args(name_lower, is_grouped, expected));
        }
        if is_grouped {
            return Ok(None);
        }
        let Some(original) = p21.parse_staged_occurrence(tx, inst, widths, record_idx, cx)? else {
            return Ok(None);
        };
        let mut writer = LiveP21Writer { p21, tx, widths };
        let Some(args) = shape_arguments(expected, Some(&original), fonts, &mut writer, cx)? else {
            return Ok(None);
        };
        Ok((args.name == name_lower).then_some(args))
    }

    /// `$$RASTER$$` 作図グループの枠折線が入れ替わって他要素を巻き込まないか検査し、更新後の枠レコードを返す
    fn check_raster_groups(
        &self,
        removed_rows: &BTreeSet<RowId>,
        updated_shapes: &BTreeMap<RowId, &Placed>,
        format: &str,
        cx: &mut Cx,
    ) -> Result<Vec<RasterOutlineDiff>, Error> {
        if self.raster_groups.is_empty() {
            return Ok(Vec::new());
        }
        let mut touched_groups = BTreeSet::new();
        for &r in removed_rows
            .iter()
            .chain(updated_shapes.keys().copied().collect::<Vec<_>>().iter())
        {
            if let Some(row) = self.table.row(r)
                && let Some(&g_idx) = self.record_to_raster_group.get(&row.record_idx)
            {
                touched_groups.insert(g_idx);
            }
        }
        if touched_groups.is_empty() {
            return Ok(Vec::new());
        }

        let mut diffs = Vec::new();
        for g_idx in touched_groups {
            let Some(group) = self.raster_groups.get(g_idx) else {
                continue;
            };
            let mut new_outline: Option<usize> = None;
            for &m_rec in &group.members {
                // このレコードに対応する生存行を探す
                let member_row = (0..self.base_row_count).map(RowId).find(|&rid| {
                    !removed_rows.contains(&rid)
                        && self.table.row(rid).is_some_and(|r| r.record_idx == m_rec)
                });
                let Some(rid) = member_row else {
                    continue;
                };
                let Some(row) = self.table.row(rid) else {
                    continue;
                };
                let effective_placed = updated_shapes.get(&rid).copied().unwrap_or(&row.placed);
                if is_four_or_more_polyline_or_underlay(effective_placed) {
                    new_outline = Some(m_rec);
                    break;
                }
            }

            if new_outline != group.outline_record {
                // 編集対象ではない生存メンバーが枠になったり枠でなくなったりしていれば、他要素不変条件に反する
                for rid_raw in 0..self.base_row_count {
                    let rid = RowId(rid_raw);
                    if removed_rows.contains(&rid) || updated_shapes.contains_key(&rid) {
                        continue;
                    }
                    let Some(row) = self.table.row(rid) else {
                        continue;
                    };
                    if self.record_to_raster_group.get(&row.record_idx) == Some(&g_idx) {
                        let was_outline = group.outline_record == Some(row.record_idx);
                        let now_outline = new_outline == Some(row.record_idx);
                        if was_outline != now_outline {
                            self.report_at(
                                unsupported(format, "apply", "round_trip"),
                                placed_source(&row.placed),
                                cx,
                            )?;
                            return Err(Error::Rejected);
                        }
                    }
                }
                diffs.push((g_idx, group.outline_record, new_outline));
            }
        }
        Ok(diffs)
    }

    fn raster_info_for_record(
        &self,
        record_idx: usize,
        raster_diffs: &[(usize, Option<usize>, Option<usize>)],
    ) -> Option<(&str, &Ref)> {
        let &g_idx = self.record_to_raster_group.get(&record_idx)?;
        let group = self.raster_groups.get(g_idx)?;
        let effective_outline = raster_diffs
            .iter()
            .find(|(idx, _, _)| *idx == g_idx)
            .map_or(group.outline_record, |(_, _, after)| *after);
        (effective_outline == Some(record_idx)).then_some((group.path.as_str(), &group.definition))
    }

    fn verify_and_commit_sfc(
        &mut self,
        tx: SfcStepTx,
        staged_ops: Vec<(&'static str, StagedSfcOp)>,
        tables: Option<Box<TablesDiff>>,
        cx: &mut Cx,
    ) -> Result<Vec<Applied>, Error> {
        let format = "SFC";
        let mut removed_rows = BTreeSet::new();
        let mut updated_shapes: BTreeMap<RowId, &Placed> = BTreeMap::new();
        for (_, op) in &staged_ops {
            match op {
                StagedSfcOp::Removed { row_id, .. } => {
                    removed_rows.insert(*row_id);
                }
                StagedSfcOp::Updated {
                    row_id, expected, ..
                } => {
                    updated_shapes.insert(*row_id, expected.as_ref());
                }
                StagedSfcOp::Added { .. } | StagedSfcOp::Layer { .. } => {}
            }
        }

        let raster_diffs = self.check_raster_groups(&removed_rows, &updated_shapes, format, cx)?;

        let SxfBacking::Sfc(sfc) = &self.backing else {
            return Err(Error::Rejected);
        };

        let mut verified = Vec::with_capacity(staged_ops.len());
        let mut applied_out = Vec::with_capacity(staged_ops.len());

        for (edit_name, op) in staged_ops {
            match op {
                StagedSfcOp::Layer { slot, expected } => {
                    let found = sfc
                        .parse_staged_feature(&tx, slot, cx)?
                        .and_then(|(_, rec)| {
                            if !rec.name.eq_ignore_ascii_case("layer") {
                                return None;
                            }
                            layer_feature(&rec.args, expected.id, expected.source.clone())
                        });
                    let hash = layers::hash(&expected, cx)?;
                    let same = match &found {
                        Some(found) => layers::hash(found, cx)? == hash,
                        None => false,
                    };
                    if !same {
                        cx.report(unsupported(format, edit_name, "round_trip"));
                        return Err(Error::Rejected);
                    }
                    applied_out.push(Applied {
                        id: Some(id::layer(&expected)),
                        hash: Some(hash),
                    });
                }
                StagedSfcOp::Removed { row_id, inst } => {
                    if sfc.has_surviving_feature_id(&tx, inst) {
                        cx.report(unsupported(format, edit_name, "round_trip"));
                        return Err(Error::Rejected);
                    }
                    applied_out.push(Applied {
                        id: None,
                        hash: None,
                    });
                    verified.push(VerifiedOp::Remove(row_id));
                }
                StagedSfcOp::Updated {
                    row_id,
                    slot,
                    expected,
                } => {
                    let Some(row) = self.table.row(row_id) else {
                        return Err(Error::Rejected);
                    };
                    let Some((inst, feat_rec)) = sfc.parse_staged_feature(&tx, slot, cx)? else {
                        cx.report(unsupported(format, edit_name, "round_trip"));
                        return Err(Error::Rejected);
                    };
                    let raster = self.raster_info_for_record(row.record_idx, &raster_diffs);
                    let (lifted, diagnostics) = self.lift_single_for_container(
                        row.container,
                        Some(inst),
                        &feat_rec,
                        raster,
                        cx,
                    )?;
                    let Some((lifted_placed, drafts)) =
                        lifted.filter(|(lp, _)| placed_same(lp, &expected))
                    else {
                        cx.report(unsupported(format, edit_name, "round_trip"));
                        return Err(Error::Rejected);
                    };
                    let elem_hash = hash_placed(&lifted_placed, cx)?;
                    applied_out.push(Applied {
                        id: Some(row.canonical_id.to_string()),
                        hash: Some(elem_hash),
                    });
                    verified.push(VerifiedOp::Update {
                        row_id,
                        placed: lifted_placed,
                        drafts,
                        diagnostics,
                    });
                }
                StagedSfcOp::Added {
                    seq,
                    inst: expected_inst,
                    place_name,
                    sheet_id,
                    expected,
                } => {
                    let slot = SfcSlot::Born(seq);
                    let Some((inst, feat_rec)) = sfc.parse_staged_feature(&tx, slot, cx)? else {
                        cx.report(unsupported(format, edit_name, "round_trip"));
                        return Err(Error::Rejected);
                    };
                    if inst != expected_inst {
                        cx.report(unsupported(format, edit_name, "round_trip"));
                        return Err(Error::Rejected);
                    }
                    let (lifted, diagnostics) = self.lift_single_for_container(
                        SxfContainer::Sheet(sheet_id),
                        Some(inst),
                        &feat_rec,
                        None,
                        cx,
                    )?;
                    let Some((lifted_placed, drafts)) =
                        lifted.filter(|(lp, _)| placed_same(lp, &expected))
                    else {
                        cx.report(unsupported(format, edit_name, "round_trip"));
                        return Err(Error::Rejected);
                    };
                    let cid = format!("{place_name}/i{inst}");
                    let elem_hash = hash_placed(&lifted_placed, cx)?;
                    applied_out.push(Applied {
                        id: Some(cid.clone()),
                        hash: Some(elem_hash),
                    });
                    verified.push(VerifiedOp::AddSfc {
                        seq,
                        inst,
                        sheet_id,
                        canonical_id: cid,
                        feat_name: feat_rec.name,
                        feat_kind: feat_rec.kind,
                        placed: lifted_placed,
                        drafts,
                        diagnostics,
                    });
                }
            }
        }

        let SxfBacking::Sfc(sfc_mut) = &mut self.backing else {
            return Err(Error::Rejected);
        };
        let sfc_patch = sfc_mut.commit_step(tx);
        self.commit_table_and_indexes(
            verified,
            SxfUnifiedPatch {
                backing: BackingPatch::Sfc(sfc_patch),
                raster_outline_diffs: raster_diffs,
                symbol_bounds: Vec::new(),
                tables,
            },
            cx,
        )?;
        Ok(applied_out)
    }

    fn verify_and_commit_p21(
        &mut self,
        tx: P21StepTx,
        active_widths: &[Option<f64>],
        staged_ops: Vec<(&'static str, StagedP21Op)>,
        tables: Option<Box<TablesDiff>>,
        cx: &mut Cx,
    ) -> Result<Vec<Applied>, Error> {
        let format = "P21";
        let mut removed_rows = BTreeSet::new();
        let mut updated_shapes: BTreeMap<RowId, &Placed> = BTreeMap::new();
        for (_, op) in &staged_ops {
            match op {
                StagedP21Op::Removed { row_id, .. } => {
                    removed_rows.insert(*row_id);
                }
                StagedP21Op::Updated {
                    row_id, expected, ..
                } => {
                    updated_shapes.insert(*row_id, expected.as_ref());
                }
                StagedP21Op::Added { .. } | StagedP21Op::Layer { .. } => {}
            }
        }

        let raster_diffs = self.check_raster_groups(&removed_rows, &updated_shapes, format, cx)?;

        let SxfBacking::P21(p21) = &self.backing else {
            return Err(Error::Rejected);
        };

        let mut verified = Vec::with_capacity(staged_ops.len());
        let mut applied_out = Vec::with_capacity(staged_ops.len());

        for (edit_name, op) in staged_ops {
            match op {
                StagedP21Op::Layer { expected } => {
                    applied_out.push(Applied {
                        id: Some(id::layer(&expected)),
                        hash: Some(layers::hash(&expected, cx)?),
                    });
                }
                StagedP21Op::Removed { row_id, inst } => {
                    if p21.has_surviving_id(&tx, inst) {
                        cx.report(unsupported(format, edit_name, "round_trip"));
                        return Err(Error::Rejected);
                    }
                    applied_out.push(Applied {
                        id: None,
                        hash: None,
                    });
                    verified.push(VerifiedOp::Remove(row_id));
                }
                StagedP21Op::Updated {
                    row_id,
                    inst,
                    expected,
                } => {
                    let Some(row) = self.table.row(row_id) else {
                        return Err(Error::Rejected);
                    };
                    let Some(feat_rec) =
                        p21.parse_staged_occurrence(&tx, inst, active_widths, row.record_idx, cx)?
                    else {
                        cx.report(unsupported(format, edit_name, "round_trip"));
                        return Err(Error::Rejected);
                    };
                    let raster = self.raster_info_for_record(row.record_idx, &raster_diffs);
                    let (lifted, diagnostics) = self.lift_single_for_container(
                        row.container,
                        Some(inst),
                        &feat_rec,
                        raster,
                        cx,
                    )?;
                    let Some((lifted_placed, drafts)) = lifted
                        .filter(|(lp, _)| placed_same(lp, &expected) || placed_near(lp, &expected))
                    else {
                        cx.report(unsupported(format, edit_name, "round_trip"));
                        return Err(Error::Rejected);
                    };
                    let elem_hash = hash_placed(&lifted_placed, cx)?;
                    applied_out.push(Applied {
                        id: Some(row.canonical_id.to_string()),
                        hash: Some(elem_hash),
                    });
                    verified.push(VerifiedOp::Update {
                        row_id,
                        placed: lifted_placed,
                        drafts,
                        diagnostics,
                    });
                }
                StagedP21Op::Added {
                    inst,
                    place_name,
                    sheet_id,
                    add_name,
                    expected,
                } => {
                    let Some(feat_rec) =
                        p21.parse_staged_occurrence(&tx, inst, active_widths, 0, cx)?
                    else {
                        cx.report(unsupported(format, edit_name, "round_trip"));
                        return Err(Error::Rejected);
                    };
                    let _ = add_name;
                    let (lifted, diagnostics) = self.lift_single_for_container(
                        SxfContainer::Sheet(sheet_id),
                        Some(inst),
                        &feat_rec,
                        None,
                        cx,
                    )?;
                    let Some((lifted_placed, drafts)) = lifted
                        .filter(|(lp, _)| placed_same(lp, &expected) || placed_near(lp, &expected))
                    else {
                        cx.report(unsupported(format, edit_name, "round_trip"));
                        return Err(Error::Rejected);
                    };
                    let cid = format!("{place_name}/i{inst}");
                    let elem_hash = hash_placed(&lifted_placed, cx)?;
                    applied_out.push(Applied {
                        id: Some(cid.clone()),
                        hash: Some(elem_hash),
                    });
                    verified.push(VerifiedOp::AddP21 {
                        inst,
                        sheet_id,
                        canonical_id: cid,
                        feat_name: feat_rec.name,
                        feat_kind: feat_rec.kind,
                        placed: lifted_placed,
                        drafts,
                        diagnostics,
                    });
                }
            }
        }

        let SxfBacking::P21(p21_mut) = &mut self.backing else {
            return Err(Error::Rejected);
        };
        let p21_patch = p21_mut.commit_step(tx, cx)?;
        self.commit_table_and_indexes(
            verified,
            SxfUnifiedPatch {
                backing: BackingPatch::P21(p21_patch),
                raster_outline_diffs: raster_diffs,
                symbol_bounds: Vec::new(),
                tables,
            },
            cx,
        )?;
        Ok(applied_out)
    }

    fn lift_single_for_container(
        &self,
        container: SxfContainer,
        instance: Option<u64>,
        feat_rec: &super::FeatureRecord,
        raster: Option<(&str, &Ref)>,
        cx: &mut Cx,
    ) -> Result<LiftedPlaced, Error> {
        let mut tables = self.lifter_tables.clone();
        if let SxfBacking::P21(p21) = &self.backing {
            tables.widths = p21.active_widths();
        }
        let (placed, events) = quietly(cx, |cx| {
            cx.with_diagnostic_events(u64::MAX, |cx| match container {
                SxfContainer::Sheet(_) => {
                    Ok(lift_single_paper(&tables, instance, feat_rec, raster, cx)?
                        .map(|(e, drafts)| (Placed::Paper(e), drafts)))
                }
                SxfContainer::Space(sp) => Ok(lift_single_model(
                    &tables, sp, instance, feat_rec, raster, cx,
                )?
                .map(|(e, drafts)| (Placed::Model(e), drafts))),
                SxfContainer::Symbol(sym_idx) => {
                    let Some(sym_t) = self.symbol_templates.get(sym_idx) else {
                        return Ok(None);
                    };
                    Ok(
                        lift_single_model(&tables, sym_t.space_id, instance, feat_rec, raster, cx)?
                            .map(|(e, drafts)| (Placed::Model(e), drafts)),
                    )
                }
            })
        })?;
        Ok((placed, events.details))
    }

    fn commit_table_and_indexes(
        &mut self,
        verified: Vec<VerifiedOp>,
        mut patch: SxfUnifiedPatch,
        cx: &mut Cx,
    ) -> Result<(), Error> {
        let had_ops = !verified.is_empty() || patch.tables.is_some();
        let prepared = match self.prepare_symbols(&verified, cx) {
            Ok(prepared) => prepared,
            Err(error) => {
                match (&mut self.backing, &patch.backing) {
                    (SxfBacking::Sfc(sfc), BackingPatch::Sfc(p)) => sfc.revert_patch(p),
                    (SxfBacking::P21(p21), BackingPatch::P21(p)) => p21.revert_patch(p),
                    _ => {}
                }
                return Err(error);
            }
        };
        let next_symbol_bounds = prepared.next;
        let affected = prepared.affected;
        patch.symbol_bounds = prepared.diffs;
        let patch_arc = Arc::new(patch);
        let symbol_bounds_ref = next_symbol_bounds.as_ref().unwrap_or(&self.symbol_bounds);

        let mut explicit_rows = BTreeSet::new();
        for op in &verified {
            match op {
                VerifiedOp::Remove(rid) => {
                    explicit_rows.insert(*rid);
                }
                VerifiedOp::Update { row_id, .. } => {
                    explicit_rows.insert(*row_id);
                }
                VerifiedOp::AddSfc { .. } | VerifiedOp::AddP21 { .. } => {}
            }
        }
        let mut dependent_updates = Vec::new();
        for dep_rid in affected {
            if dep_rid.is_virtual() || explicit_rows.contains(&dep_rid) {
                continue;
            }
            if let Some(dep_row) = self.table.row(dep_rid).cloned() {
                let new_bounds = compute_sheet_bounds(
                    dep_row.container,
                    &dep_row.placed,
                    &self.space_views,
                    symbol_bounds_ref,
                );
                dependent_updates.push((dep_rid, dep_row, new_bounds));
            }
        }

        let patch_for_txn = Arc::clone(&patch_arc);
        let space_views = &self.space_views;
        let symbol_indices = &self.symbol_indices;
        let side = SideIndexes {
            grid: Some(&mut self.spatial),
            dirty_rows: None,
            dep: Some(&mut self.dep),
        };

        let txn_res = run_transaction(
            &mut self.table,
            side,
            &mut self.history,
            cx,
            |txn, _cx| -> Result<(), TxnError<()>> {
                for op in verified {
                    match op {
                        VerifiedOp::Remove(row_id) => {
                            txn.remove_row(row_id)?;
                        }
                        VerifiedOp::Update {
                            row_id,
                            placed,
                            drafts,
                            diagnostics,
                        } => {
                            let Some(prev_row) = txn.row(row_id).cloned() else {
                                return Err(TxnError::Rejected(()));
                            };
                            let new_bounds = compute_sheet_bounds(
                                prev_row.container,
                                &placed,
                                space_views,
                                symbol_bounds_ref,
                            );
                            let dep_src = matches!(
                                prev_row.container,
                                SxfContainer::Space(_) | SxfContainer::Sheet(_)
                            )
                            .then(|| placed_symbol_dep_source(&placed, symbol_indices))
                            .flatten();
                            let mut next_row = prev_row;
                            let track_dep = matches!(
                                next_row.container,
                                SxfContainer::Space(_) | SxfContainer::Sheet(_)
                            );
                            // 要素自身の拡張は読み直した値にし、複合図形の属性など外から付いた拡張は残す
                            let own = placed_source(&next_row.placed).clone();
                            let kept = next_row.drafts.iter().filter(|d| d.source != own).cloned();
                            next_row.drafts = drafts.into_iter().chain(kept).collect();
                            next_row.placed = placed;
                            next_row.diagnostics = diagnostics;
                            txn.update_row(row_id, next_row, new_bounds)?;
                            if track_dep {
                                txn.set_dep_sources(row_id, dep_src)?;
                            }
                        }
                        VerifiedOp::AddSfc {
                            seq,
                            inst,
                            sheet_id,
                            canonical_id,
                            feat_name,
                            feat_kind,
                            placed,
                            drafts,
                            diagnostics,
                        } => {
                            let container = SxfContainer::Sheet(sheet_id);
                            let new_bounds = compute_sheet_bounds(
                                container,
                                &placed,
                                space_views,
                                symbol_bounds_ref,
                            );
                            let dep_src = placed_symbol_dep_source(&placed, symbol_indices);
                            let new_row = SxfRow {
                                instance: Some(inst),
                                unique_handle: Some(inst),
                                record_idx: 0,
                                local_space_idx: 0,
                                sfc_slot: Some(SfcSlot::Born(seq)),
                                p21_kind: feat_kind.into_boxed_str(),
                                p21_name: feat_name.to_ascii_lowercase().into_boxed_str(),
                                grouped: false,
                                container,
                                canonical_id: canonical_id.into_boxed_str(),
                                placed,
                                drafts: drafts.into_boxed_slice(),
                                diagnostics,
                            };
                            let rid = txn.insert_row(new_row, new_bounds)?;
                            if let Some(vnode) = dep_src {
                                txn.set_dep_sources(rid, [vnode])?;
                            }
                        }
                        VerifiedOp::AddP21 {
                            inst,
                            sheet_id,
                            canonical_id,
                            feat_name,
                            feat_kind,
                            placed,
                            drafts,
                            diagnostics,
                        } => {
                            let container = SxfContainer::Sheet(sheet_id);
                            let new_bounds = compute_sheet_bounds(
                                container,
                                &placed,
                                space_views,
                                symbol_bounds_ref,
                            );
                            let dep_src = placed_symbol_dep_source(&placed, symbol_indices);
                            let new_row = SxfRow {
                                instance: Some(inst),
                                unique_handle: Some(inst),
                                record_idx: 0,
                                local_space_idx: 0,
                                sfc_slot: None,
                                p21_kind: feat_kind.into_boxed_str(),
                                p21_name: feat_name.to_ascii_lowercase().into_boxed_str(),
                                grouped: false,
                                container,
                                canonical_id: canonical_id.into_boxed_str(),
                                placed,
                                drafts: drafts.into_boxed_slice(),
                                diagnostics,
                            };
                            let rid = txn.insert_row(new_row, new_bounds)?;
                            if let Some(vnode) = dep_src {
                                txn.set_dep_sources(rid, [vnode])?;
                            }
                        }
                    }
                }
                for (dep_rid, dep_row, new_bounds) in dependent_updates {
                    txn.update_row(dep_rid, dep_row, new_bounds)?;
                }
                if had_ops {
                    txn.push_patch(SharedPatch(patch_for_txn));
                }
                Ok(())
            },
        );

        match txn_res {
            Ok(_) => {}
            Err(err) => {
                match (&mut self.backing, &patch_arc.backing) {
                    (SxfBacking::Sfc(sfc), BackingPatch::Sfc(p)) => sfc.revert_patch(p),
                    (SxfBacking::P21(p21), BackingPatch::P21(p)) => p21.revert_patch(p),
                    _ => {}
                }
                return match err {
                    TxnError::Stop(stop) => {
                        cx.report(stop.to_diagnostic());
                        Err(Error::Stopped(stop))
                    }
                    TxnError::Stage(_) | TxnError::Rejected(()) => {
                        cx.report(unsupported(
                            self.backing.format_name(),
                            "apply",
                            "round_trip",
                        ));
                        Err(Error::Rejected)
                    }
                };
            }
        }

        if had_ops && let Some(committed) = self.history.last_undo().cloned() {
            if let Some(next) = next_symbol_bounds {
                self.symbol_bounds = next;
            }
            self.dirty_symbols
                .extend(patch_arc.symbol_bounds.iter().map(|diff| diff.index));
            for &(g_idx, _, after) in &patch_arc.raster_outline_diffs {
                if let Some(group) = self.raster_groups.get_mut(g_idx) {
                    group.outline_record = after;
                }
            }
            for diff in committed.existing() {
                match diff {
                    ExistingRowDiff::Removed { before_row, .. } => {
                        self.by_canonical_id
                            .remove(before_row.canonical_id.as_ref());
                        if let SxfContainer::Space(sp) = before_row.container
                            && let Some(sp_idx) = self.space_spatial.get_mut(&sp)
                        {
                            let local = before_row.local_space_idx as usize;
                            if let Some(b) = sp_idx.bounds.get_mut(local) {
                                *b = EMPTY_BOUNDS;
                            }
                            sp_idx.grid.update(before_row.local_space_idx, EMPTY_BOUNDS);
                        }
                    }
                    ExistingRowDiff::Updated { after_row, .. } => {
                        if let SxfContainer::Space(sp) = after_row.container
                            && let Placed::Model(m_elem) = &after_row.placed
                            && let Some(sp_idx) = self.space_spatial.get_mut(&sp)
                        {
                            let new_b = model_shape_bounds(&m_elem.shape, &self.symbol_bounds);
                            let local = after_row.local_space_idx as usize;
                            if let Some(b) = sp_idx.bounds.get_mut(local) {
                                *b = new_b;
                            }
                            sp_idx.grid.update(after_row.local_space_idx, new_b);
                        }
                    }
                }
            }
            for born in committed.born() {
                self.by_canonical_id
                    .insert(born.after_row.canonical_id.to_string(), born.row);
            }
            let disp_ctx = DisplayContext {
                space_views: &self.space_views,
                symbol_indices: &self.symbol_indices,
            };
            apply_display_commit(&mut self.display, &disp_ctx, &committed);
            self.history.trim_to_bytes(self.history_max_bytes);
        }

        Ok(())
    }

    /// 直前の編集トランザクションを `O(変更数)` で取り消す
    pub fn undo(&mut self, cx: &mut Cx<'_>) -> Result<bool, Error> {
        if let Some(committed) = self.history.last_undo() {
            self.check_symbol_history(committed, true, cx)?;
        }
        let side = SideIndexes {
            grid: Some(&mut self.spatial),
            dirty_rows: None,
            dep: Some(&mut self.dep),
        };
        let committed = match self.history.undo(&mut self.table, side, cx) {
            Ok(Some(c)) => c.clone(),
            Ok(None) => return Ok(false),
            Err(stop) => {
                cx.report(stop.to_diagnostic());
                return Err(Error::Stopped(stop));
            }
        };

        let disp_ctx = DisplayContext {
            space_views: &self.space_views,
            symbol_indices: &self.symbol_indices,
        };
        revert_display_commit(&mut self.display, &disp_ctx, &committed);
        for born in committed.born().iter().rev() {
            self.by_canonical_id
                .remove(born.after_row.canonical_id.as_ref());
        }
        for patch in committed.patches().iter().rev() {
            self.restore_symbol_bounds(&patch.0.symbol_bounds, true);
        }
        for diff in committed.existing().iter().rev() {
            match diff {
                ExistingRowDiff::Removed {
                    row, before_row, ..
                } => {
                    self.by_canonical_id
                        .insert(before_row.canonical_id.to_string(), *row);
                    if let SxfContainer::Space(sp) = before_row.container
                        && let Placed::Model(m_elem) = &before_row.placed
                        && let Some(sp_idx) = self.space_spatial.get_mut(&sp)
                    {
                        let old_b = model_shape_bounds(&m_elem.shape, &self.symbol_bounds);
                        let local = before_row.local_space_idx as usize;
                        if let Some(b) = sp_idx.bounds.get_mut(local) {
                            *b = old_b;
                        }
                        sp_idx.grid.update(before_row.local_space_idx, old_b);
                    }
                }
                ExistingRowDiff::Updated { before_row, .. } => {
                    if let SxfContainer::Space(sp) = before_row.container
                        && let Placed::Model(m_elem) = &before_row.placed
                        && let Some(sp_idx) = self.space_spatial.get_mut(&sp)
                    {
                        let old_b = model_shape_bounds(&m_elem.shape, &self.symbol_bounds);
                        let local = before_row.local_space_idx as usize;
                        if let Some(b) = sp_idx.bounds.get_mut(local) {
                            *b = old_b;
                        }
                        sp_idx.grid.update(before_row.local_space_idx, old_b);
                    }
                }
            }
        }
        for patch in committed.patches().iter().rev() {
            for &(g_idx, before_outline, _) in patch.0.raster_outline_diffs.iter().rev() {
                if let Some(group) = self.raster_groups.get_mut(g_idx) {
                    group.outline_record = before_outline;
                }
            }
            match (&mut self.backing, &patch.0.backing) {
                (SxfBacking::Sfc(sfc), BackingPatch::Sfc(p)) => sfc.revert_patch(p),
                (SxfBacking::P21(p21), BackingPatch::P21(p)) => p21.revert_patch(p),
                _ => {}
            }
            if let Some(tables) = &patch.0.tables {
                self.set_tables(&tables.before);
            }
        }
        Ok(true)
    }

    /// 直前に取り消した編集トランザクションを `O(変更数)` でやり直す
    pub fn redo(&mut self, cx: &mut Cx<'_>) -> Result<bool, Error> {
        if let Some(committed) = self.history.last_redo() {
            self.check_symbol_history(committed, false, cx)?;
        }
        let side = SideIndexes {
            grid: Some(&mut self.spatial),
            dirty_rows: None,
            dep: Some(&mut self.dep),
        };
        let committed = match self.history.redo(&mut self.table, side, cx) {
            Ok(Some(c)) => c.clone(),
            Ok(None) => return Ok(false),
            Err(stop) => {
                cx.report(stop.to_diagnostic());
                return Err(Error::Stopped(stop));
            }
        };

        for patch in committed.patches() {
            match (&mut self.backing, &patch.0.backing) {
                (SxfBacking::Sfc(sfc), BackingPatch::Sfc(p)) => sfc.reapply_patch(p),
                (SxfBacking::P21(p21), BackingPatch::P21(p)) => p21.reapply_patch(p),
                _ => {}
            }
            if let Some(tables) = &patch.0.tables {
                self.set_tables(&tables.after);
            }
            for &(g_idx, _, after_outline) in &patch.0.raster_outline_diffs {
                if let Some(group) = self.raster_groups.get_mut(g_idx) {
                    group.outline_record = after_outline;
                }
            }
        }
        for patch in committed.patches() {
            self.restore_symbol_bounds(&patch.0.symbol_bounds, false);
        }
        for diff in committed.existing() {
            match diff {
                ExistingRowDiff::Removed { before_row, .. } => {
                    self.by_canonical_id
                        .remove(before_row.canonical_id.as_ref());
                    if let SxfContainer::Space(sp) = before_row.container
                        && let Some(sp_idx) = self.space_spatial.get_mut(&sp)
                    {
                        let local = before_row.local_space_idx as usize;
                        if let Some(b) = sp_idx.bounds.get_mut(local) {
                            *b = EMPTY_BOUNDS;
                        }
                        sp_idx.grid.update(before_row.local_space_idx, EMPTY_BOUNDS);
                    }
                }
                ExistingRowDiff::Updated { after_row, .. } => {
                    if let SxfContainer::Space(sp) = after_row.container
                        && let Placed::Model(m_elem) = &after_row.placed
                        && let Some(sp_idx) = self.space_spatial.get_mut(&sp)
                    {
                        let new_b = model_shape_bounds(&m_elem.shape, &self.symbol_bounds);
                        let local = after_row.local_space_idx as usize;
                        if let Some(b) = sp_idx.bounds.get_mut(local) {
                            *b = new_b;
                        }
                        sp_idx.grid.update(after_row.local_space_idx, new_b);
                    }
                }
            }
        }
        for born in committed.born() {
            self.by_canonical_id
                .insert(born.after_row.canonical_id.to_string(), born.row);
        }
        let disp_ctx = DisplayContext {
            space_views: &self.space_views,
            symbol_indices: &self.symbol_indices,
        };
        apply_display_commit(&mut self.display, &disp_ctx, &committed);
        Ok(true)
    }

    /// 常駐状態から [`Drawing2D`] を組み立てる
    pub fn drawing(&self, cx: &mut Cx<'_>) -> Result<Drawing2D, Error> {
        self.drawing_with_attributes(self.attributes.as_ref(), cx)
    }

    /// 属性ファイル（SAF）を指定して、常駐状態から [`Drawing2D`] を組み立てる
    pub fn drawing_with_attributes(
        &self,
        attributes: Option<&AttributeFile>,
        cx: &mut Cx<'_>,
    ) -> Result<Drawing2D, Error> {
        cx.with_diagnostics_in_record_order(|cx| self.drawing_inner(attributes, cx))
    }

    fn drawing_inner(
        &self,
        attributes: Option<&AttributeFile>,
        cx: &mut Cx<'_>,
    ) -> Result<Drawing2D, Error> {
        let mut layers = self.lifter_tables.base_layers.clone();
        let (mut line_types, mut line_types_seen) = self.diagnostics.line_types(
            &self.table,
            self.base_row_count,
            &self.lifter_tables,
            cx,
        )?;
        let mut drafts = self.doc_drafts.clone();

        let mut spaces = Vec::with_capacity(self.space_templates.len());
        for (sp_t, r_ids) in self.space_templates.iter().zip(&self.space_rows) {
            let mut elems = Vec::new();
            for &rid in r_ids {
                if let Some(row) = self.table.row(rid)
                    && let Placed::Model(m_elem) = &row.placed
                {
                    work(cx, 1, &Ref::default())?;
                    let idx = elems.len();
                    ensure_element_tables(
                        self.lifter_tables.defined_layers,
                        &mut layers,
                        &mut line_types,
                        m_elem.layer,
                        &m_elem.style,
                    );
                    for d in &row.drafts {
                        let mut dc = d.clone();
                        dc.at = Position::Model {
                            space: sp_t.id,
                            index: idx,
                        };
                        drafts.push(dc);
                    }
                    elems.push(m_elem.clone());
                }
            }
            let order = (0..elems.len()).collect();
            spaces.push(ModelSpace {
                id: sp_t.id,
                unit: sp_t.unit,
                axes: sp_t.axes,
                elements: elems,
                order,
            });
        }

        let mut symbols = Vec::with_capacity(self.symbol_templates.len());
        for (sym_t, r_ids) in self.symbol_templates.iter().zip(&self.symbol_rows) {
            let mut elems = Vec::new();
            for &rid in r_ids {
                if let Some(row) = self.table.row(rid)
                    && let Placed::Model(m_elem) = &row.placed
                {
                    work(cx, 1, &Ref::default())?;
                    let idx = elems.len();
                    ensure_element_tables(
                        self.lifter_tables.defined_layers,
                        &mut layers,
                        &mut line_types,
                        m_elem.layer,
                        &m_elem.style,
                    );
                    for d in &row.drafts {
                        let mut dc = d.clone();
                        dc.at = Position::Model {
                            space: sym_t.space_id,
                            index: idx,
                        };
                        drafts.push(dc);
                    }
                    elems.push(m_elem.clone());
                }
            }
            let order = (0..elems.len()).collect();
            symbols.push(SymbolDefinition {
                key: sym_t.key.clone(),
                space: ModelSpace {
                    id: sym_t.space_id,
                    unit: sym_t.unit,
                    axes: sym_t.axes,
                    elements: elems,
                    order,
                },
                base: sym_t.base,
                attributes: Vec::new(),
                anonymous: false,
                external: None,
                source: sym_t.source.clone(),
            });
        }

        let mut sheet_elems = Vec::new();
        let mut sheet_order = Vec::new();
        let insertion = self
            .diagnostics
            .sheet_insertion()
            .min(self.base_sheet_slots.len());
        let (before, after) = self.base_sheet_slots.split_at(insertion);
        let born = (self.base_row_count..self.table.len() as u32)
            .map(|index| BaseSheetSlot::Row(RowId(index)));
        for slot in before
            .iter()
            .copied()
            .chain(born)
            .chain(after.iter().copied())
        {
            match slot {
                BaseSheetSlot::View(v) => sheet_order.push(SheetItem::View(v)),
                BaseSheetSlot::Row(rid) => {
                    if let Some(row) = self.table.row(rid)
                        && let Placed::Paper(p_elem) = &row.placed
                    {
                        work(cx, 1, &Ref::default())?;
                        let idx = sheet_elems.len();
                        ensure_element_tables(
                            self.lifter_tables.defined_layers,
                            &mut layers,
                            &mut line_types,
                            p_elem.layer,
                            &p_elem.style,
                        );
                        for d in &row.drafts {
                            let mut dc = d.clone();
                            dc.at = Position::Paper {
                                sheet: self.sheet_template.id,
                                index: idx,
                            };
                            drafts.push(dc);
                        }
                        sheet_elems.push(p_elem.clone());
                        sheet_order.push(SheetItem::Element(idx));
                    }
                }
            }
        }

        {
            let sources = self.backing.source_map(cx)?;
            self.diagnostics
                .report(&self.backing, &sources, &mut line_types_seen, cx)?;
            for index in 0..self.table.len() {
                work(cx, 1, &Ref::default())?;
                if let Some(row) = self.table.row(RowId(index as u32)) {
                    for diagnostic in &row.diagnostics {
                        diagnostics::report(diagnostic, &sources, &mut line_types_seen, cx)?;
                    }
                }
            }
            for draft in &mut drafts {
                sources.remap(&mut draft.source, cx)?;
            }
            if let Some(attr_file) = attributes {
                bind(attr_file, &mut drafts, cx);
            }
            let mut drawing = Drawing2D {
                spaces,
                sheets: vec![Sheet {
                    id: self.sheet_template.id,
                    name: self.sheet_template.name.clone(),
                    size: self.sheet_template.size,
                    elements: sheet_elems,
                    views: self.sheet_template.views.clone(),
                    order: sheet_order,
                    source: self.sheet_template.source.clone(),
                }],
                layers,
                line_types,
                title_blocks: self.title_blocks.clone(),
                symbols,
                ..Drawing2D::default()
            };
            sources.drawing(&mut drawing, cx)?;
            drawing.settle_extensions(drafts, cx).map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Error::Stopped(stop)
            })?;
            Ok(drawing)
        }
    }

    /// 現在の常駐状態を [`SxfFile`] に書き出す
    pub fn snapshot_file(&self, cx: &mut Cx<'_>) -> Result<SxfFile, Error> {
        let bytes = self.save(cx)?;
        quietly(cx, |cx| Sxf::read(&bytes, cx))
    }

    /// 全体を書く保存（`SxfFile::apply` を 1 手ずつ当てて `Sxf::write` した結果とバイト列一致）
    pub fn save(&self, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
        self.backing.save(cx)
    }
}

/// 1 回の `apply` で、要素の外の表（字体とレイヤ）に当てる変更
struct TableStage {
    fonts: Fonts<'static>,
    /// 当てる前のレイヤの表（レイヤを変えるときだけ読む）
    before_layers: Option<Vec<Layer>>,
    layer_edits: layers::Changes,
    /// 変えたか足したレイヤ
    layers: Vec<Layer>,
    added_layers: usize,
    /// 幅を見積もって足した文字のインスタンス番号
    estimated_texts: Vec<u64>,
}

impl TableStage {
    fn new(fonts: &[String], broken: bool) -> Self {
        Self {
            fonts: Fonts::known((!broken).then(|| fonts.to_vec())),
            before_layers: None,
            layer_edits: layers::Changes::default(),
            layers: Vec::new(),
            added_layers: 0,
            estimated_texts: Vec::new(),
        }
    }

    /// 書いたレイヤフィーチャの場所を引き、確かめる値として残す
    fn staged_layer(
        &mut self,
        sfc: &SfcState,
        tx: &SfcStepTx,
        layer: Layer,
        added: bool,
        format: &str,
        cx: &mut Cx,
    ) -> Option<StagedSfcOp> {
        let Some(slot) = sfc.slot_for_source(tx, &layer.source) else {
            let edit = if added { "add_layer" } else { "replace_layer" };
            cx.report(unsupported(format, edit, "layer_feature"));
            return None;
        };
        if added {
            self.added_layers += 1;
        }
        self.layers.push(layer.clone());
        Some(StagedSfcOp::Layer {
            slot,
            expected: Box::new(layer),
        })
    }
}

impl TableStage {
    /// 書いた P21 のレイヤを、確かめる値として残す
    fn p21_layer(&mut self, layer: Layer, added: bool) -> StagedP21Op {
        if added {
            self.added_layers += 1;
        }
        self.layers.push(layer.clone());
        StagedP21Op::Layer {
            expected: Box::new(layer),
        }
    }
}

/// 文書エンジンの P21 の仮置きに書く、バッチと共通の書き手。レイヤ・字体の定義と、足す要素だけを書く
struct LiveP21Writer<'a> {
    p21: &'a P21State,
    tx: &'a mut P21StepTx,
    widths: &'a [Option<f64>],
}

impl Writer for LiveP21Writer<'_> {
    fn layer_change(
        &mut self,
        source: Option<&Ref>,
        layer: &Layer,
        cx: &mut Cx,
    ) -> Result<Option<u64>, Error> {
        self.p21.stage_layer(self.tx, source, layer, cx)
    }

    fn codes(
        &mut self,
        _record: usize,
        _codes: &[(usize, i64)],
        _cx: &mut Cx,
    ) -> Result<Option<()>, Error> {
        Ok(None)
    }

    fn shape(
        &mut self,
        _record: usize,
        _args: &Arguments,
        _cx: &mut Cx,
    ) -> Result<Option<()>, Error> {
        Ok(None)
    }

    fn remove(&mut self, _record: usize, _cx: &mut Cx) -> Result<Option<()>, Error> {
        Ok(None)
    }

    fn add(&mut self, args: &Arguments, cx: &mut Cx) -> Result<Option<u64>, Error> {
        self.p21.stage_add(self.tx, self.widths, args, cx)
    }

    fn bytes(self: Box<Self>, _src: &[u8], _cx: &mut Cx) -> Result<Vec<u8>, Error> {
        Err(Error::Rejected)
    }

    fn markers(&self) -> bool {
        true
    }
}

/// 文書エンジンの仮置きに書く、バッチと共通の書き手。レイヤ・字体の定義と、足す要素だけを書く
struct LiveSfcWriter<'a> {
    sfc: &'a SfcState,
    tx: &'a mut SfcStepTx,
}

impl Writer for LiveSfcWriter<'_> {
    fn layer_change(
        &mut self,
        source: Option<&Ref>,
        layer: &Layer,
        _cx: &mut Cx,
    ) -> Result<Option<u64>, Error> {
        let Some(state) = layer.state else {
            return Ok(None);
        };
        let Some(source) = source else {
            let visible = if state.visible { "1" } else { "0" };
            let args = Arguments {
                name: "layer",
                values: vec![layer.name.clone(), visible.into()],
            };
            return Ok(self.sfc.stage_add(self.tx, &args).map(|(_, id)| id));
        };
        Ok(self.sfc.slot_for_source(self.tx, source).and_then(|slot| {
            self.sfc
                .stage_layer(self.tx, slot, &layer.name, state.visible)
        }))
    }

    fn codes(
        &mut self,
        _record: usize,
        _codes: &[(usize, i64)],
        _cx: &mut Cx,
    ) -> Result<Option<()>, Error> {
        Ok(None)
    }

    fn shape(
        &mut self,
        _record: usize,
        _args: &Arguments,
        _cx: &mut Cx,
    ) -> Result<Option<()>, Error> {
        Ok(None)
    }

    fn remove(&mut self, _record: usize, _cx: &mut Cx) -> Result<Option<()>, Error> {
        Ok(None)
    }

    fn add(&mut self, args: &Arguments, _cx: &mut Cx) -> Result<Option<u64>, Error> {
        Ok(self.sfc.stage_add(self.tx, args).map(|(_, id)| id))
    }

    fn bytes(self: Box<Self>, _src: &[u8], _cx: &mut Cx) -> Result<Vec<u8>, Error> {
        Err(Error::Rejected)
    }

    fn markers(&self) -> bool {
        true
    }
}

enum StagedSfcOp {
    /// レイヤフィーチャをこの値にする
    Layer {
        slot: SfcSlot,
        expected: Box<Layer>,
    },
    Removed {
        row_id: RowId,
        inst: u64,
    },
    Updated {
        row_id: RowId,
        slot: SfcSlot,
        expected: Box<Placed>,
    },
    Added {
        seq: u32,
        inst: u64,
        place_name: String,
        sheet_id: SheetId,
        expected: Box<Placed>,
    },
}

enum StagedP21Op {
    /// レイヤをこの値にする
    Layer {
        expected: Box<Layer>,
    },
    Removed {
        row_id: RowId,
        inst: u64,
    },
    Updated {
        row_id: RowId,
        inst: u64,
        expected: Box<Placed>,
    },
    Added {
        inst: u64,
        place_name: String,
        sheet_id: SheetId,
        add_name: &'static str,
        expected: Box<Placed>,
    },
}

enum VerifiedOp {
    Remove(RowId),
    Update {
        row_id: RowId,
        placed: Placed,
        /// 要素自身の拡張（文字の幅など）
        drafts: Vec<Draft>,
        diagnostics: Vec<Diagnostic>,
    },
    AddSfc {
        seq: u32,
        inst: u64,
        sheet_id: SheetId,
        canonical_id: String,
        feat_name: String,
        feat_kind: String,
        placed: Placed,
        drafts: Vec<Draft>,
        diagnostics: Vec<Diagnostic>,
    },
    AddP21 {
        inst: u64,
        sheet_id: SheetId,
        canonical_id: String,
        feat_name: String,
        feat_kind: String,
        placed: Placed,
        drafts: Vec<Draft>,
        diagnostics: Vec<Diagnostic>,
    },
}

fn placed_source(placed: &Placed) -> &Ref {
    match placed {
        Placed::Model(e) => &e.source,
        Placed::Paper(e) => &e.source,
    }
}

fn placed_style(placed: &Placed) -> &Style {
    match placed {
        Placed::Model(e) => &e.style,
        Placed::Paper(e) => &e.style,
    }
}

fn hash_placed(placed: &Placed, cx: &mut Cx) -> Result<Hash, Error> {
    let res = match placed {
        Placed::Model(e) => Hash::element_checked(e, cx),
        Placed::Paper(e) => Hash::element_checked(e, cx),
    };
    res.map_err(|stop| {
        cx.report(stop.to_diagnostic().at(placed_source(placed).clone()));
        Error::Stopped(stop)
    })
}

fn count_placed_copy(placed: &Placed, cx: &mut Cx, copies: u64) -> Result<(), Error> {
    match placed {
        Placed::Model(e) => count_element_copy(e, cx, copies),
        Placed::Paper(e) => count_element_copy(e, cx, copies),
    }
}

#[allow(dead_code)]
fn count_placed_compare(placed: &Placed, cx: &mut Cx) -> Result<(), Error> {
    match placed {
        Placed::Model(e) => count_element_compare(e, cx),
        Placed::Paper(e) => count_element_compare(e, cx),
    }
}

fn count_placed_shape(placed: &Placed, cx: &mut Cx) -> Result<(), Error> {
    match placed {
        Placed::Model(e) => count_shape(&e.shape, cx),
        Placed::Paper(e) => count_shape(&e.shape, cx),
    }
}

fn over_placed(placed: &Placed, restyle: &Restyle) -> (zumen_drawing::LayerId, Style) {
    match placed {
        Placed::Model(e) => restyle.over(e.layer, &e.style),
        Placed::Paper(e) => restyle.over(e.layer, &e.style),
    }
}

fn with_layer_style(
    was: &Placed,
    layer: zumen_drawing::LayerId,
    style: Style,
    shape: Option<Placed>,
) -> Placed {
    match (was, shape) {
        (Placed::Model(e), Some(Placed::Model(n))) => Placed::Model(Element {
            layer,
            style,
            shape: n.shape,
            source: e.source.clone(),
        }),
        (Placed::Paper(e), Some(Placed::Paper(n))) => Placed::Paper(Element {
            layer,
            style,
            shape: n.shape,
            source: e.source.clone(),
        }),
        _ => {
            let mut cloned = was.clone();
            match &mut cloned {
                Placed::Model(e) => {
                    e.layer = layer;
                    e.style = style;
                }
                Placed::Paper(e) => {
                    e.layer = layer;
                    e.style = style;
                }
            }
            cloned
        }
    }
}

fn replaced_placed(was: &Placed, new: &Placed) -> Option<Placed> {
    match (was, new) {
        (Placed::Model(_), Placed::Model(e)) => {
            Some(with_layer_style(was, e.layer, e.style, Some(new.clone())))
        }
        (Placed::Paper(_), Placed::Paper(e)) => {
            Some(with_layer_style(was, e.layer, e.style, Some(new.clone())))
        }
        _ => None,
    }
}

fn transformed_placed(was: &Placed, affine: &Affine) -> Option<Placed> {
    Some(match was {
        Placed::Model(e) => Placed::Model(Element {
            shape: transform::transform(&e.shape, affine)?,
            ..e.clone()
        }),
        Placed::Paper(e) => Placed::Paper(Element {
            shape: transform::transform(&e.shape, affine)?,
            ..e.clone()
        }),
    })
}

/// 文字（注記）のレイヤと色の変え方
#[derive(Clone, Copy, PartialEq, Eq)]
enum Annotation {
    /// 文字は変えない
    Refuse,
    /// コードを書く
    Restyle,
    /// 形と一緒に書き直すので、コードは書かない
    Rewrite,
}

fn compute_style_codes(
    name_lower: &str,
    is_grouped: bool,
    was_placed: &Placed,
    expected: &mut Placed,
    tables: &Tables,
    text_mode: Annotation,
) -> Option<Vec<(usize, i64)>> {
    let annotation = name_lower == text::NAME;
    if annotation && text_mode == Annotation::Refuse {
        return None;
    }
    // 点マーカーと文字は線種と線幅を持たない
    let marker = name_lower == MARKER || annotation;
    if !(marker || STYLED.contains(&name_lower)) || is_grouped {
        return None;
    }
    let (layer, style) = match expected {
        Placed::Model(e) => (e.layer, &mut e.style),
        Placed::Paper(e) => (e.layer, &mut e.style),
    };
    let (was_layer, was_style) = match was_placed {
        Placed::Model(e) => (e.layer, &e.style),
        Placed::Paper(e) => (e.layer, &e.style),
    };
    if style.line_type_scale != 1.0 || style.transparency != Inherit::Layer {
        return None;
    }
    let mut codes = Vec::new();
    if layer != was_layer {
        codes.push((0, tables.layer(layer)?));
    }
    if style.color != was_style.color {
        let (code, normalized) = tables.color(style.color)?;
        style.color = Inherit::Own(normalized);
        codes.push((1, code));
    }
    if style.line_type != was_style.line_type {
        if marker {
            return None;
        }
        codes.push((2, tables.line_type(style.line_type)?));
    }
    if style.line_weight != was_style.line_weight {
        if marker {
            return None;
        }
        codes.push((3, tables.width(style.line_weight?)?));
    }
    if annotation && text_mode == Annotation::Rewrite {
        codes.clear();
    }
    Some(codes)
}

fn compute_shape_args(name_lower: &str, is_grouped: bool, expected: &Placed) -> Option<Arguments> {
    let marker = name_lower == MARKER;
    if !(marker || SHAPED.contains(&name_lower)) || is_grouped {
        return None;
    }
    let args = match expected {
        Placed::Model(e) => arguments(&e.shape)?,
        Placed::Paper(e) => arguments(&e.shape)?,
    };
    (args.name == name_lower).then_some(args)
}

/// 足す要素の引数にレイヤ・色・線種・線幅のコードを入れる（バッチの `Plan::add` と同じ順で断る）
fn prepare_add_args(
    mut element: Element<PaperPoint>,
    mut args: Arguments,
    tables: &Tables,
    unset: &mut Option<&'static str>,
) -> Option<(Arguments, Element<PaperPoint>)> {
    let style = element.style;
    let marker = args.name == MARKER || args.name == text::NAME;
    if let Some(field) = unset_field(&style, marker) {
        *unset = Some(field);
        return None;
    }
    if style.line_type_scale != 1.0 || style.transparency != Inherit::Layer {
        return None;
    }
    let (color_code, read_color) = tables.color(style.color)?;
    element.style.color = Inherit::Own(read_color);
    let codes = if marker {
        if style.line_type != Inherit::Layer
            || !matches!(style.line_weight, None | Some(Inherit::Layer))
        {
            return None;
        }
        element.style.line_weight = None;
        vec![tables.layer(element.layer)?, color_code]
    } else {
        vec![
            tables.layer(element.layer)?,
            color_code,
            tables.line_type(style.line_type)?,
            tables.width(style.line_weight?)?,
        ]
    };
    for (at, code) in codes.iter().enumerate() {
        *args.values.get_mut(at)? = code.to_string();
    }
    let _ = Kind::Work;
    Some((args, element))
}

fn is_four_or_more_polyline_or_underlay(placed: &Placed) -> bool {
    match placed {
        Placed::Model(e) => match &e.shape {
            Shape::Underlay(_) => true,
            Shape::Polyline(p) => p.vertices.len() >= 4,
            _ => false,
        },
        Placed::Paper(e) => match &e.shape {
            Shape::Underlay(_) => true,
            Shape::Polyline(p) => p.vertices.len() >= 4,
            _ => false,
        },
    }
}

fn report_table_edit_unsupported(format: &str, change: &Edit, cx: &mut Cx) {
    let kind = match change {
        Edit::ReplaceLayer { .. } | Edit::AddLayer { .. } | Edit::RemoveLayer { .. } => {
            "layer_feature"
        }
        Edit::ReplaceTextStyle { .. }
        | Edit::AddTextStyle { .. }
        | Edit::RemoveTextStyle { .. } => "text_font_feature",
        Edit::ReplaceLineType { .. } | Edit::AddLineType { .. } | Edit::RemoveLineType { .. } => {
            "user_defined_font_feature"
        }
        Edit::ReplaceDimensionStyle { .. }
        | Edit::AddDimensionStyle { .. }
        | Edit::RemoveDimensionStyle { .. } => "dimension_style",
        Edit::SetPointStyle { .. } => "point_style",
        _ => "element",
    };
    cx.report(unsupported(format, change.name(), kind));
}

fn placed_symbol_dep_source(
    placed: &Placed,
    symbol_indices: &BTreeMap<SymbolKey, u32>,
) -> Option<RowId> {
    let key = match placed {
        Placed::Model(e) => match &e.shape {
            Shape::Symbol(sym) => &sym.key,
            _ => return None,
        },
        Placed::Paper(e) => match &e.shape {
            Shape::Symbol(sym) => &sym.key,
            _ => return None,
        },
    };
    symbol_indices
        .get(key)
        .copied()
        .and_then(RowId::virtual_node)
}

fn ordered_symbol_indices(
    symbol_count: usize,
    dep: &DepGraph,
    cx: &mut Cx<'_>,
) -> Result<Vec<usize>, Error> {
    let check = |cx: &mut Cx<'_>, kind, amount| {
        cx.check(kind, amount).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })
    };
    check(cx, Kind::Work, (symbol_count as u64).max(1))?;
    check(
        cx,
        Kind::DecodedBytes,
        (symbol_count as u64).saturating_mul(128),
    )?;
    let vnodes: BTreeSet<RowId> = (0..symbol_count)
        .filter_map(|i| u32::try_from(i).ok().and_then(RowId::virtual_node))
        .collect();
    let ordered = dep.topological_order_with_cx(&vnodes, cx).map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })?;
    check(cx, Kind::Work, ordered.len() as u64)?;
    check(
        cx,
        Kind::DecodedBytes,
        (ordered.len() as u64).saturating_mul(8),
    )?;
    Ok(ordered
        .into_iter()
        .filter_map(|r| r.as_virtual().map(|v| v as usize))
        .collect())
}

#[cfg(test)]
mod equivalence_tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Diagnostics};
    use zumen_sdk::{Apply, Lift};

    fn run<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Vec<Diagnostic>) {
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let result = f(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
        (result, diagnostics.iter().cloned().collect())
    }

    #[test]
    fn public_sample_preserves_full_model_and_diagnostics() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples/sxf/ezsxf-writer-all-features.sfc");
        let bytes = std::fs::read(path).unwrap();
        let file = run(|cx| Sxf::read(&bytes, cx)).0.unwrap();
        let expected = run(|cx| file.lift(cx));
        for threads in [1, 4] {
            let live = run(|cx| SxfLive::open_with_threads(&bytes, threads, cx))
                .0
                .unwrap();
            assert_eq!(run(|cx| live.drawing(cx)), expected);
        }
    }

    #[test]
    fn public_sample_edits_preserve_source_records_before_save() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples/sxf/ezsxf-writer-all-features.sfc");
        let bytes = std::fs::read(path).unwrap();
        for threads in [1, 4] {
            let mut file = run(|cx| Sxf::read(&bytes, cx)).0.unwrap();
            let mut live = run(|cx| SxfLive::open_with_threads(&bytes, threads, cx))
                .0
                .unwrap();
            let before = run(|cx| live.drawing(cx)).0.unwrap();
            let sheet = &before.sheets[0];
            let place = id::sheet(sheet);
            let mut namer = id::Namer::new(&place);
            let target = sheet
                .elements
                .iter()
                .enumerate()
                .find_map(|(index, element)| {
                    let id = namer.next(index, element);
                    matches!(element.shape, Shape::Line { .. }).then(|| Seen {
                        id,
                        hash: Hash::element(element),
                    })
                })
                .unwrap();
            let changes = Changes {
                document: None,
                edits: vec![Edit::Remove { target }],
            };
            let actual = run(|cx| live.apply(&changes, cx));
            assert_eq!(actual, run(|cx| file.apply(&changes, cx)));
            actual.0.unwrap();
            let expected: Drawing2D = run(|cx| file.lift(cx)).0.unwrap();
            let model = run(|cx| live.drawing(cx)).0.unwrap();
            assert_eq!(model, expected);
            let saved = run(|cx| live.save(cx)).0.unwrap();
            let reference = run(|cx| {
                let mut out = Vec::new();
                Sxf::write(&file, &mut out, cx).map(|()| out)
            })
            .0
            .unwrap();
            assert_eq!(saved, reference);
            assert!(run(|cx| live.undo(cx)).0.unwrap());
            assert_eq!(run(|cx| live.drawing(cx)).0.unwrap(), before);
            assert_eq!(run(|cx| live.save(cx)).0.unwrap(), bytes);
        }
    }

    fn sfc_features(features: &[&str]) -> Vec<u8> {
        let mut bytes = concat!(
            "ISO-10303-21;\nHEADER;\n",
            "FILE_DESCRIPTION(('SCADEC level2 feature_mode'),'2;1');\n",
            "FILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n",
        )
        .as_bytes()
        .to_vec();
        for (index, feature) in features.iter().enumerate() {
            bytes.extend_from_slice(
                format!("/*SXF3\n#{} = {feature}\nSXF3*/\n", 10 * (index + 1)).as_bytes(),
            );
        }
        bytes.extend_from_slice(b"ENDSEC;\nEND-ISO-10303-21;\n");
        bytes
    }

    fn seen(live: &SxfLive, instance: u64) -> Seen {
        let row = live.row_id_for_instance(instance).unwrap();
        Seen {
            id: live.canonical_id(row).unwrap().to_owned(),
            hash: run(|cx| hash_placed(live.placed(row).unwrap(), cx))
                .0
                .unwrap(),
        }
    }

    #[test]
    fn cyclic_part_bounds_match_reopening_and_restore_exactly() {
        let bytes = sfc_features(&[
            "layer_feature(\\'A\\','1')",
            "line_feature('1','1','1','1','0','0','1','1')",
            "sfig_locate_feature('1',\\'B\\','10','0','0','1','1')",
            "sfig_org_feature(\\'A\\','4')",
            "sfig_locate_feature('1',\\'A\\','10','0','0','1','1')",
            "sfig_org_feature(\\'B\\','4')",
            "sfig_locate_feature('1',\\'A\\','100','0','0','1','1')",
            "sfig_locate_feature('1',\\'B\\','200','0','0','1','1')",
            "drawing_sheet_feature(\\'S\\','3','1','0','0')",
        ]);
        for threads in [1, 4] {
            let mut file = run(|cx| Sxf::read(&bytes, cx)).0.unwrap();
            let mut live = run(|cx| SxfLive::open_with_threads(&bytes, threads, cx))
                .0
                .unwrap();
            let before_bounds = live.symbol_bounds.clone();
            let before_hits = live.select_sheet([105.0, 0.0, 106.0, 1.0]);
            let change = Changes {
                document: None,
                edits: vec![Edit::Transform {
                    target: seen(&live, 20),
                    transform: Affine::translation(100.0, 0.0),
                }],
            };
            let expected = run(|cx| file.apply(&change, cx));
            let actual = run(|cx| live.apply(&change, cx));
            assert_eq!(actual, expected);
            actual.0.unwrap();
            assert_source_parity(&live, &file);
            let saved = run(|cx| live.save(cx)).0.unwrap();
            let reopened = run(|cx| SxfLive::open_with_threads(&saved, threads, cx))
                .0
                .unwrap();
            assert_eq!(live.symbol_bounds, reopened.symbol_bounds);
            assert_eq!(
                live.select_sheet([105.0, 0.0, 106.0, 1.0]),
                reopened.select_sheet([105.0, 0.0, 106.0, 1.0])
            );
            let after_bounds = live.symbol_bounds.clone();
            assert!(run(|cx| live.undo(cx)).0.unwrap());
            assert_eq!(run(|cx| live.save(cx)).0.unwrap(), bytes);
            assert_eq!(live.symbol_bounds, before_bounds);
            assert_eq!(live.select_sheet([105.0, 0.0, 106.0, 1.0]), before_hits);
            assert!(run(|cx| live.redo(cx)).0.unwrap());
            assert_eq!(run(|cx| live.save(cx)).0.unwrap(), saved);
            assert_eq!(live.symbol_bounds, after_bounds);
        }
    }

    fn part_bounds_input(cyclic: bool) -> Vec<u8> {
        let mut features = vec![
            r"layer_feature(\'L\','1')",
            "line_feature('1','1','1','1','0','0','1','1')",
        ];
        if cyclic {
            features.push(r"sfig_locate_feature('1',\'B\','10','0','0','1','1')");
        }
        features.extend([
            r"sfig_org_feature(\'A\','4')",
            r"sfig_locate_feature('1',\'A\','10','0','0','1','1')",
            r"sfig_org_feature(\'B\','4')",
            r"sfig_locate_feature('1',\'B\','30','40','0','2','1')",
            r"sfig_org_feature(\'view\','1')",
            r"sfig_locate_feature('1',\'A\','100','0','0','1','1')",
            r"sfig_locate_feature('1',\'B\','200','0','0','1','1')",
            r"sfig_locate_feature('1',\'view\','0','0','0','1','1')",
            r"drawing_sheet_feature(\'S\','3','1','0','0')",
        ]);
        sfc_features(&features)
    }

    fn part_bounds_snapshot(live: &SxfLive) -> String {
        let mut result = format!(
            "model={:?}\nsave={:?}\nbounds={:?}\nrows={:?}\ndep={:?}\nflags={:?}/{:?}\nhistory={:?}\n",
            run(|cx| live.drawing(cx)),
            run(|cx| live.save(cx)),
            live.symbol_bounds,
            live.table.bounds_column(),
            live.dep,
            live.dirty_symbols,
            live.display.dirty,
            (
                live.state_id(),
                live.revision(),
                live.undo_len(),
                live.redo_len(),
                live.history_bytes()
            ),
        );
        use std::fmt::Write;
        for row in 0..live.table.len() {
            writeln!(
                result,
                "display={row}:{:?}",
                live.display.row(u32::try_from(row).unwrap())
            )
            .unwrap();
        }
        for query in [
            [105.0, 0.0, 106.0, 1.0],
            [115.0, 40.0, 116.0, 41.0],
            [0.0, 0.0, 1000.0, 1000.0],
        ] {
            writeln!(result, "sheet={query:?}:{:?}", live.select_sheet(query)).unwrap();
            for space in &live.space_templates {
                writeln!(
                    result,
                    "space={:?}:{query:?}:{:?}",
                    space.id,
                    live.select_in_space(space.id, query)
                )
                .unwrap();
            }
        }
        result
    }

    #[derive(Clone, Copy, Debug)]
    enum PartAction {
        Move,
        Remove,
        Undo,
        Redo,
    }

    fn act_on_part(
        live: &mut SxfLive,
        action: PartAction,
        changes: &Changes,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        match action {
            PartAction::Move | PartAction::Remove => live.apply(changes, cx).map(|_| ()),
            PartAction::Undo => live.undo(cx).map(|changed| assert!(changed)),
            PartAction::Redo => live.redo(cx).map(|changed| assert!(changed)),
        }
    }

    #[test]
    fn part_bounds_budget_stops_keep_all_state_and_history_atomic() {
        use std::fmt::Write;
        use zumen_base::{Limit, Limits};
        let mut transcript = String::new();
        for cyclic in [false, true] {
            let bytes = part_bounds_input(cyclic);
            for threads in [1, 4] {
                let mut current = run(|cx| SxfLive::open_with_threads(&bytes, threads, cx))
                    .0
                    .unwrap();
                let mut file = run(|cx| Sxf::read(&bytes, cx)).0.unwrap();
                let initial_model = run(|cx| current.drawing(cx));
                let initial_bounds = current.symbol_bounds.clone();
                for action in [
                    PartAction::Move,
                    PartAction::Remove,
                    PartAction::Undo,
                    PartAction::Undo,
                    PartAction::Redo,
                    PartAction::Redo,
                ] {
                    current.clear_dirty_symbols();
                    current.clear_display_dirty();
                    let changes = Changes {
                        document: None,
                        edits: match action {
                            PartAction::Move => vec![Edit::Transform {
                                target: seen(&current, 20),
                                transform: Affine::translation(100.0, 0.0),
                            }],
                            PartAction::Remove => vec![Edit::Remove {
                                target: seen(&current, 20),
                            }],
                            PartAction::Undo | PartAction::Redo => Vec::new(),
                        },
                    };
                    let mut measured = current.clone();
                    let mut complete = Budget::default();
                    let mut events = Diagnostics::new();
                    let cancel = CancelToken::new();
                    act_on_part(
                        &mut measured,
                        action,
                        &changes,
                        &mut Cx::new(&mut complete, &mut events, &cancel),
                    )
                    .unwrap();
                    let expected_state = part_bounds_snapshot(&measured);
                    writeln!(transcript, "cyclic={cyclic} threads={threads} action={action:?} used={:?} diag={:?}\n{expected_state}", Kind::ALL.map(|kind| complete.used(kind)), events.iter().collect::<Vec<_>>()).unwrap();
                    for kind in [Kind::Work, Kind::DecodedBytes] {
                        let used = complete.used(kind);
                        assert!(used > 0);
                        for (limit, spent) in [
                            (0, 0),
                            (used / 2, 0),
                            (used - 1, 0),
                            (used, 0),
                            (used, 1),
                            (used + 1, 1),
                        ] {
                            let mut candidate = current.clone();
                            let before = part_bounds_snapshot(&candidate);
                            let mut budget =
                                Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
                            budget.spend(kind, spent).unwrap();
                            let mut diagnostics = Diagnostics::new();
                            let result = act_on_part(
                                &mut candidate,
                                action,
                                &changes,
                                &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
                            );
                            writeln!(transcript, "{kind:?} limit={limit} spent={spent} result={result:?} used={:?} diag={:?}", Kind::ALL.map(|kind| budget.used(kind)), diagnostics.iter().collect::<Vec<_>>()).unwrap();
                            if limit - spent == used {
                                result.unwrap();
                                assert_eq!(part_bounds_snapshot(&candidate), expected_state);
                                assert_eq!(
                                    diagnostics.iter().collect::<Vec<_>>(),
                                    events.iter().collect::<Vec<_>>()
                                );
                            } else {
                                assert!(
                                    matches!(result, Err(Error::Stopped(_))),
                                    "{action:?} {kind:?} limit={limit} spent={spent}"
                                );
                                assert_eq!(part_bounds_snapshot(&candidate), before);
                                assert_eq!(
                                    diagnostics
                                        .iter()
                                        .filter(|event| event.code() == kind.exceeded_code())
                                        .count(),
                                    1
                                );
                            }
                            assert_eq!(budget.used(Kind::Depth), 0);
                        }
                    }
                    let mut candidate = current.clone();
                    let before = part_bounds_snapshot(&candidate);
                    cancel.cancel();
                    assert!(matches!(
                        act_on_part(
                            &mut candidate,
                            action,
                            &changes,
                            &mut Cx::new(&mut Budget::default(), &mut Diagnostics::new(), &cancel)
                        ),
                        Err(Error::Stopped(_))
                    ));
                    assert_eq!(part_bounds_snapshot(&candidate), before);
                    if matches!(action, PartAction::Move | PartAction::Remove) {
                        run(|cx| file.apply(&changes, cx)).0.unwrap();
                        assert_source_parity(&measured, &file);
                    }
                    let saved = run(|cx| measured.save(cx)).0.unwrap();
                    let reopened = run(|cx| SxfLive::open_with_threads(&saved, threads, cx))
                        .0
                        .unwrap();
                    assert_eq!(measured.symbol_bounds, reopened.symbol_bounds);
                    assert_eq!(
                        run(|cx| measured.drawing(cx)),
                        run(|cx| reopened.drawing(cx))
                    );
                    for query in [[105.0, 0.0, 106.0, 1.0], [115.0, 40.0, 116.0, 41.0]] {
                        // 再読後は行が詰まるため、選択した要素の ID で比べる。
                        let selected = |live: &SxfLive| {
                            live.select_sheet(query)
                                .iter()
                                .map(|row| live.canonical_id(*row).unwrap().to_owned())
                                .collect::<Vec<_>>()
                        };
                        assert_eq!(selected(&measured), selected(&reopened));
                    }
                    if measured.undo_len() == 0 {
                        assert_eq!(saved, bytes);
                        assert_eq!(run(|cx| measured.drawing(cx)), initial_model);
                        assert_eq!(measured.symbol_bounds, initial_bounds);
                    }
                    current = measured;
                }
            }
        }
        write_part_bounds_transcript(&transcript);
    }

    #[allow(
        clippy::disallowed_methods,
        reason = "ネイティブと wasm の比較記録を、指定された試験用ファイルへ書く"
    )]
    fn write_part_bounds_transcript(transcript: &str) {
        if let Some(path) = std::env::var_os("ZUMEN_SXF_LIVE_BOUNDS_OUT") {
            std::fs::write(path, transcript).unwrap();
        }
    }

    #[test]
    fn diagnostics_follow_changed_styles_deleted_first_use_tail_and_history() {
        let bytes = sfc_features(&[
            "layer_feature(\\'A\\','1')",
            "line_feature('1','99','90','1','0','0','10','0')",
            "line_feature('1','98','90','1','0','1','10','1')",
            "drawing_sheet_feature(\\'A3\\','3','1','0','0')",
            "line_feature('1','97','91','1','0','2','10','2')",
            "line_feature('1','96','91','1','0','3','10','3')",
        ]);
        for threads in [1, 4] {
            let mut file = run(|cx| Sxf::read(&bytes, cx)).0.unwrap();
            let mut live = run(|cx| SxfLive::open_with_threads(&bytes, threads, cx))
                .0
                .unwrap();
            assert_source_parity(&live, &file);
            let mut history = vec![(run(|cx| live.drawing(cx)), bytes.clone())];
            for (instance, restyle) in [
                (20, true),
                (20, false),
                (50, false),
                (60, false),
                (30, false),
            ] {
                let changes = Changes {
                    document: None,
                    edits: vec![if restyle {
                        Edit::Restyle {
                            target: seen(&live, instance),
                            style: Restyle {
                                line_type: Some(Inherit::Own(zumen_drawing::LineTypeId(1))),
                                ..Restyle::default()
                            },
                        }
                    } else {
                        Edit::Remove {
                            target: seen(&live, instance),
                        }
                    }],
                };
                let actual = run(|cx| live.apply(&changes, cx));
                assert_eq!(actual, run(|cx| file.apply(&changes, cx)));
                actual.0.unwrap();
                assert_source_parity(&live, &file);
                history.push((
                    run(|cx| live.drawing(cx)),
                    run(|cx| live.save(cx)).0.unwrap(),
                ));
            }
            for expected in history.iter().rev().skip(1) {
                assert!(run(|cx| live.undo(cx)).0.unwrap());
                assert_eq!(run(|cx| live.drawing(cx)), expected.0);
                assert_eq!(run(|cx| live.save(cx)).0.unwrap(), expected.1);
            }
            for expected in history.iter().skip(1) {
                assert!(run(|cx| live.redo(cx)).0.unwrap());
                assert_eq!(run(|cx| live.drawing(cx)), expected.0);
                assert_eq!(run(|cx| live.save(cx)).0.unwrap(), expected.1);
            }
        }
    }

    fn p21_lines() -> Vec<u8> {
        concat!(
            "ISO-10303-21;\nHEADER;\n",
            "FILE_DESCRIPTION(('SCADEC level2 AP202_mode'),'2;1');\n",
            "FILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n",
            "#10=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));\n",
            "#20=DRAUGHTING_PRE_DEFINED_COLOUR('red');\n",
            "#30=DRAUGHTING_PRE_DEFINED_CURVE_FONT('continuous');\n",
            "#40=LENGTH_MEASURE_WITH_UNIT(POSITIVE_LENGTH_MEASURE(0.25),#10);\n",
            "#60=CURVE_STYLE(' ',#30,#40,#20);\n",
            "#70=PRESENTATION_STYLE_ASSIGNMENT((#60));\n",
            "#100=CARTESIAN_POINT(' ',(0.,0.));\n",
            "#110=CARTESIAN_POINT(' ',(10.,0.));\n",
            "#120=DIRECTION(' ',(1.,0.));\n",
            "#130=VECTOR(' ',#120,10.);\n",
            "#140=LINE(' ',#100,#130);\n",
            "#150=TRIMMED_CURVE(' ',#140,(#100),(#110),.T.,.CARTESIAN.);\n",
            "#160=ANNOTATION_CURVE_OCCURRENCE(' ',(#70),#150);\n",
            "#170=ANNOTATION_CURVE_OCCURRENCE(' ',(#70),#150);\n",
            "#900=PRESENTATION_LAYER_ASSIGNMENT('A',' ',(#160,#170));\n",
            "#950=DRAWING_SHEET_REVISION('A3_horizontal',(#160,#170),' ',' ');\n",
            "ENDSEC;\nEND-ISO-10303-21;\n",
        )
        .as_bytes()
        .to_vec()
    }

    #[test]
    fn first_line_type_use_moves_between_subfigures_and_composites() {
        for (first, second) in [(90, 90), (-5, -6)] {
            for (member, visible) in [
                ("line_feature('1','1','1','1','0','1','10','1')", 1),
                ("point_marker_feature('1','1','1','2','2','0','1')", 1),
                ("line_feature('1','1','1','1','0','1','10','1')", 0),
            ] {
                for flag in [1, 2, 3, 4] {
                    let definition = format!(r"sfig_org_feature(\'A\','{flag}')");
                    let line = format!("line_feature('1','99','{first}','99','0','0','10','0')");
                    let composite =
                        format!("composite_curve_org_feature('98','{second}','98','{visible}')");
                    let bytes = sfc_features(&[
                        r"layer_feature(\'L\','1')",
                        &line,
                        &definition,
                        member,
                        &composite,
                        r"sfig_locate_feature('1',\'A\','0','0','0','1','1')",
                        r"drawing_sheet_feature(\'S\','3','1','0','0')",
                    ]);
                    for threads in [1, 4] {
                        let mut file = run(|cx| Sxf::read(&bytes, cx)).0.unwrap();
                        let mut live = run(|cx| SxfLive::open_with_threads(&bytes, threads, cx))
                            .0
                            .unwrap();
                        assert_type_parity(&live, &file);
                        let before = run(|cx| live.drawing(cx));
                        let changes = Changes {
                            document: None,
                            edits: vec![Edit::Remove {
                                target: seen(&live, 20),
                            }],
                        };
                        let actual = run(|cx| live.apply(&changes, cx));
                        assert_eq!(actual, run(|cx| file.apply(&changes, cx)));
                        actual.0.unwrap();
                        assert_type_parity(&live, &file);
                        let after = run(|cx| live.drawing(cx));
                        let saved = run(|cx| live.save(cx)).0.unwrap();
                        assert!(run(|cx| live.undo(cx)).0.unwrap());
                        assert_eq!(run(|cx| live.drawing(cx)), before);
                        assert_eq!(run(|cx| live.save(cx)).0.unwrap(), bytes);
                        assert!(run(|cx| live.redo(cx)).0.unwrap());
                        assert_eq!(run(|cx| live.drawing(cx)), after);
                        assert_eq!(run(|cx| live.save(cx)).0.unwrap(), saved);
                    }
                }
            }
        }
    }

    fn assert_type_parity(live: &SxfLive, file: &SxfFile) {
        for places in [0, 1, 2, 32] {
            let actual = run_places(places, |cx| live.drawing(cx));
            let expected = run_places(places, |cx| file.lift(cx));
            assert_eq!(actual.1, expected.1);
            assert_eq!(
                actual.0.as_ref().unwrap().line_types,
                expected.0.as_ref().unwrap().line_types
            );
            assert_eq!(actual, expected);
        }
        assert_source_parity(live, file);
    }

    #[test]
    fn line_type_order_follows_interleaved_definitions_groups_and_insertions() {
        let bytes = sfc_features(&[
            r"layer_feature(\'L\','1')",
            "line_feature('1','1','90','1','0','0','10','0')",
            r"sfig_org_feature(\'part\','4')",
            "line_feature('1','1','91','1','0','1','10','1')",
            r"sfig_org_feature(\'group\','3')",
            r"sfig_locate_feature('1',\'group\','0','0','0','1','1')",
            "line_feature('1','1','92','1','0','2','10','2')",
            r"sfig_org_feature(\'model\','1')",
            r"sfig_locate_feature('1',\'part\','0','0','0','1','1')",
            "line_feature('1','1','1','1','0','3','10','3')",
            "composite_curve_org_feature('1','90','1','1')",
            "line_feature('1','1','90','1','0','4','10','4')",
            r"drawing_sheet_feature(\'S\','3','1','0','0')",
            "line_feature('1','1','91','1','0','5','10','5')",
        ]);
        for threads in [1, 4] {
            let mut file = run(|cx| Sxf::read(&bytes, cx)).0.unwrap();
            let mut live = run(|cx| SxfLive::open_with_threads(&bytes, threads, cx))
                .0
                .unwrap();
            assert_type_parity(&live, &file);
            let mut history = vec![(run(|cx| live.drawing(cx)), bytes.clone())];
            for target in [None, Some(20), Some(40), Some(70), Some(120)] {
                let edit = if let Some(instance) = target {
                    Edit::Remove {
                        target: seen(&live, instance),
                    }
                } else {
                    let model = run(|cx| live.drawing(cx)).0.unwrap();
                    let row = live.row_id_for_instance(120).unwrap();
                    let mut element = live.placed(row).unwrap().clone();
                    assert!(matches!(element, Placed::Paper(_)));
                    if let Placed::Paper(line) = &mut element {
                        line.style.line_type = Inherit::Own(zumen_drawing::LineTypeId(1));
                    }
                    Edit::Add {
                        place: id::sheet(&model.sheets[0]),
                        element: Box::new(element),
                    }
                };
                let changes = Changes {
                    document: None,
                    edits: vec![edit],
                };
                let actual = run(|cx| live.apply(&changes, cx));
                assert_eq!(actual, run(|cx| file.apply(&changes, cx)));
                assert!(actual.0.is_ok(), "{target:?}: {actual:?}");
                assert_type_parity(&live, &file);
                history.push((
                    run(|cx| live.drawing(cx)),
                    run(|cx| live.save(cx)).0.unwrap(),
                ));
            }
            for expected in history.iter().rev().skip(1) {
                assert!(run(|cx| live.undo(cx)).0.unwrap());
                assert_eq!(run(|cx| live.drawing(cx)), expected.0);
                assert_eq!(run(|cx| live.save(cx)).0.unwrap(), expected.1);
            }
            for expected in history.iter().skip(1) {
                assert!(run(|cx| live.redo(cx)).0.unwrap());
                assert_eq!(run(|cx| live.drawing(cx)), expected.0);
                assert_eq!(run(|cx| live.save(cx)).0.unwrap(), expected.1);
            }
        }
    }

    #[test]
    fn inserted_rows_precede_trailing_groups_and_keep_nested_view_order() {
        let bytes = sfc_features(&[
            r"layer_feature(\'L\','1')",
            "line_feature('1','1','90','1','0','0','10','0')",
            r"sfig_org_feature(\'inner\','1')",
            r"sfig_locate_feature('1',\'inner\','0','0','0','1','1')",
            r"sfig_org_feature(\'outer\','2')",
            "line_feature('1','1','91','1','0','1','10','1')",
            r"sfig_org_feature(\'group\','3')",
            r"sfig_locate_feature('1',\'group\','0','0','0','1','1')",
            r"sfig_locate_feature('1',\'outer\','0','0','0','1','1')",
            "line_feature('1','1','1','1','0','2','10','2')",
            r"drawing_sheet_feature(\'S\','3','1','0','0')",
            r"sfig_locate_feature('1',\'group\','0','0','0','1','1')",
        ]);
        for threads in [1, 4] {
            let mut file = run(|cx| Sxf::read(&bytes, cx)).0.unwrap();
            let mut live = run(|cx| SxfLive::open_with_threads(&bytes, threads, cx))
                .0
                .unwrap();
            assert_type_parity(&live, &file);
            let initial = run(|cx| live.drawing(cx));
            let row = live.row_id_for_instance(100).unwrap();
            let element = live.placed(row).unwrap().clone();
            let changes = Changes {
                document: None,
                edits: vec![Edit::Add {
                    place: id::sheet(&initial.0.as_ref().unwrap().sheets[0]),
                    element: Box::new(element),
                }],
            };
            let actual = run(|cx| live.apply(&changes, cx));
            assert_eq!(actual, run(|cx| file.apply(&changes, cx)));
            actual.0.unwrap();
            assert_type_parity(&live, &file);
            assert!(run(|cx| live.undo(cx)).0.unwrap());
            assert_eq!(run(|cx| live.drawing(cx)), initial);
            assert_eq!(run(|cx| live.save(cx)).0.unwrap(), bytes);
            assert!(run(|cx| live.redo(cx)).0.unwrap());
            assert_type_parity(&live, &file);
        }
    }

    #[test]
    fn grouped_order_checks_shared_budget_and_cancellation() {
        use zumen_base::{Limit, Limits};
        let bytes = sfc_features(&[
            r"layer_feature(\'L\','1')",
            "line_feature('1','99','90','99','0','0','10','0')",
            r"sfig_locate_feature('1',\'cycle\','0','0','0','1','1')",
            r"sfig_org_feature(\'cycle\','3')",
            r"sfig_locate_feature('1',\'cycle\','0','0','0','1','1')",
            r"drawing_sheet_feature(\'first\','3','1','0','0')",
            "line_feature('1','1','1','1','0','1','10','1')",
            "composite_curve_org_feature('1','90','1','1')",
            r"drawing_sheet_feature(\'S\','3','1','0','0')",
        ]);
        for threads in [1, 4] {
            let file = run(|cx| Sxf::read(&bytes, cx)).0.unwrap();
            let live = run(|cx| SxfLive::open_with_threads(&bytes, threads, cx))
                .0
                .unwrap();
            assert_type_parity(&live, &file);
            let before = run(|cx| live.drawing(cx));
            for opening in [true, false] {
                let operation = |cx: &mut Cx<'_>| {
                    if opening {
                        SxfLive::open_with_threads(&bytes, threads, cx).map(|_| ())
                    } else {
                        live.drawing(cx).map(|_| ())
                    }
                };
                let mut complete = Budget::default();
                let cancel = CancelToken::new();
                let mut diagnostics = Diagnostics::new();
                operation(&mut Cx::new(&mut complete, &mut diagnostics, &cancel)).unwrap();
                print_budget(threads, opening, &complete);
                for kind in [Kind::Work, Kind::DecodedBytes] {
                    let used = complete.used(kind);
                    assert!(used > 0);
                    for (limit, spent) in
                        [(0, 0), (used - 1, 0), (used, 0), (used, 1), (used + 1, 1)]
                    {
                        let mut budget =
                            Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
                        budget.spend(kind, spent).unwrap();
                        let mut diag = Diagnostics::new();
                        let actual = operation(&mut Cx::new(&mut budget, &mut diag, &cancel));
                        assert_eq!(
                            actual.is_ok(),
                            limit >= used + spent,
                            "opening={opening} {kind:?} {limit} {spent}"
                        );
                        if actual.is_err() {
                            assert!(matches!(actual, Err(Error::Stopped(_))));
                        }
                        assert_eq!(budget.used(Kind::Depth), 0);
                    }
                }
                cancel.cancel();
                let mut budget = Budget::default();
                let mut diag = Diagnostics::new();
                assert_eq!(
                    operation(&mut Cx::new(&mut budget, &mut diag, &cancel)),
                    Err(Error::Stopped(zumen_base::Stop::Cancelled))
                );
                assert_eq!(budget.used(Kind::Depth), 0);
            }
            assert_eq!(run(|cx| live.drawing(cx)), before);
            assert_eq!(run(|cx| live.save(cx)).0.unwrap(), bytes);
            assert_eq!((live.undo_len(), live.redo_len()), (0, 0));
        }
    }

    #[allow(
        clippy::print_stdout,
        reason = "ネイティブと wasm の予算計上を検査の出力で照合する"
    )]
    fn print_budget(threads: usize, opening: bool, budget: &Budget) {
        println!(
            "live_sxf_budget threads={threads} opening={opening} used={:?}",
            Kind::ALL.map(|kind| budget.used(kind))
        );
    }

    #[test]
    fn p21_diagnostics_follow_reference_and_layer_membership_removal() {
        let source = String::from_utf8(p21_lines()).unwrap()
            .replace("(#60));", "(#60,#999));")
            .replace("#950=DRAWING_SHEET_REVISION", "#910=PRESENTATION_LAYER_ASSIGNMENT('B',' ',(#160,#170));\n#950=DRAWING_SHEET_REVISION");
        for threads in [1, 4] {
            let bytes = source.as_bytes();
            let mut file = run(|cx| Sxf::read(bytes, cx)).0.unwrap();
            let mut live = run(|cx| SxfLive::open_with_threads(bytes, threads, cx))
                .0
                .unwrap();
            assert_source_parity(&live, &file);
            let initial = run(|cx| live.drawing(cx));
            assert!(
                initial
                    .1
                    .iter()
                    .any(|event| event.code() == codes::SXF_P21_REFERENCE_MISSING)
            );
            assert!(
                initial
                    .1
                    .iter()
                    .any(|event| event.code() == codes::SXF_P21_LAYER_MEMBER_DROPPED)
            );
            for instance in [160, 170] {
                let changes = Changes {
                    document: None,
                    edits: vec![Edit::Remove {
                        target: seen(&live, instance),
                    }],
                };
                let actual = run(|cx| live.apply(&changes, cx));
                assert_eq!(actual, run(|cx| file.apply(&changes, cx)));
                if actual.0.is_err() {
                    assert_eq!(actual.0, Err(Error::Rejected));
                }
                assert_source_parity(&live, &file);
            }
            assert_eq!(live.undo_len(), 1);
            assert!(run(|cx| live.undo(cx)).0.unwrap());
            assert_eq!(run(|cx| live.drawing(cx)), initial);
            assert_eq!(run(|cx| live.save(cx)).0.unwrap(), bytes);
        }
    }

    #[test]
    fn saf_diagnostics_use_current_bindings_and_keep_their_own_source_positions() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<SxfAttributeXML version="3.1" date="2026-11-15" sxfFile="test.SFC" application="test">
<Figure id="10" name="bound"/><Figure id="20" name="unbound"/>
</SxfAttributeXML>"#;
        let attributes = run(|cx| crate::saf::read(xml.as_bytes(), cx)).0.unwrap();
        let bytes = sfc_features(&[
            "layer_feature(\\'A\\','1')",
            "circle_feature('1','1','1','1','10','10','5')",
            "drawing_sheet_feature(\\'A3\\','3','1','0','0')",
            "line_feature('1','1','1','1','0','0','1','0')",
            "sfig_org_feature(\\'$$ATRF$$10$$test.SAF\\','3')",
            "sfig_locate_feature('1',\\'$$ATRF$$10$$test.SAF\\','0','0','0','1','1')",
            "drawing_sheet_feature(\\'A3\\','3','1','0','0')",
        ]);
        let mut file = run(|cx| Sxf::read(&bytes, cx)).0.unwrap();
        let mut live = run_places(0, |cx| {
            SxfLive::open_with_attributes(&bytes, Some(attributes.clone()), cx)
        })
        .0
        .unwrap();
        let initial = run(|cx| file.lift_with(Some(&attributes), cx));
        assert!(
            initial
                .1
                .iter()
                .any(|event| event.code() == codes::SXF_SAF_FIGURE_DROPPED)
        );
        assert_eq!(run(|cx| live.drawing(cx)), initial);
        let changes = Changes {
            document: None,
            edits: vec![Edit::Remove {
                target: seen(&live, 20),
            }],
        };
        let actual = run(|cx| live.apply(&changes, cx));
        assert_eq!(actual, run(|cx| file.apply(&changes, cx)));
        actual.0.unwrap();
        assert_eq!(
            run(|cx| live.drawing(cx)),
            run(|cx| file.lift_with(Some(&attributes), cx))
        );
        assert!(run(|cx| live.undo(cx)).0.unwrap());
        assert_eq!(run(|cx| live.drawing(cx)), initial);
        assert_eq!(run(|cx| live.save(cx)).0.unwrap(), bytes);
    }

    fn run_places<T>(places: usize, f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Vec<Diagnostic>) {
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::with_max_places(places);
        let cancel = CancelToken::new();
        let result = f(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
        (result, diagnostics.iter().cloned().collect())
    }

    #[test]
    fn diagnostic_places_are_selected_at_query_time_and_stops_preserve_history() {
        use zumen_base::{Limit, Limits};
        let mut features = vec!["layer_feature(\\'A\\','1')".to_owned()];
        for index in 0..10 {
            features.push(format!(
                "line_feature('1','99','1','1','0','{index}','10','{index}')"
            ));
        }
        features.push("drawing_sheet_feature(\\'A3\\','3','1','0','0')".to_owned());
        let bytes = sfc_features(&features.iter().map(String::as_str).collect::<Vec<_>>());
        let file = run(|cx| Sxf::read(&bytes, cx)).0.unwrap();
        let live = run_places(0, |cx| SxfLive::open(&bytes, cx)).0.unwrap();
        for places in [0, 1, 2, 32] {
            assert_eq!(
                run_places(places, |cx| live.drawing(cx)),
                run_places(places, |cx| file.lift(cx))
            );
        }
        let changes = Changes {
            document: None,
            edits: vec![Edit::Transform {
                target: seen(&live, 20),
                transform: Affine::translation(2.0, 3.0),
            }],
        };
        let mut probe = live.clone();
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        probe
            .apply(&changes, &mut Cx::new(&mut budget, &mut diag, &cancel))
            .unwrap();
        for kind in [Kind::Work, Kind::DecodedBytes] {
            let used = budget.used(kind);
            assert!(used > 0);
            for amount in [0, 1, used / 2, used - 1] {
                let mut candidate = live.clone();
                let mut limited = Budget::new(Limits::default().with(kind, Limit::Finite(amount)));
                let mut diagnostics = Diagnostics::new();
                let result = candidate.apply(
                    &changes,
                    &mut Cx::new(&mut limited, &mut diagnostics, &cancel),
                );
                assert!(
                    matches!(result, Err(Error::Stopped(_))),
                    "{kind:?}: {amount}"
                );
                assert_eq!(run(|cx| candidate.drawing(cx)), run(|cx| live.drawing(cx)));
                assert_eq!(run(|cx| candidate.save(cx)).0.unwrap(), bytes);
                assert_eq!(candidate.state_id(), live.state_id());
                assert_eq!(candidate.revision(), live.revision());
                assert_eq!((candidate.undo_len(), candidate.redo_len()), (0, 0));
            }
        }
        cancel.cancel();
        let mut candidate = live.clone();
        assert!(matches!(
            candidate.apply(
                &changes,
                &mut Cx::new(&mut Budget::default(), &mut Diagnostics::new(), &cancel)
            ),
            Err(Error::Stopped(_))
        ));
        assert_eq!(run(|cx| candidate.drawing(cx)), run(|cx| live.drawing(cx)));
    }

    fn assert_single_sources(live: &SxfLive, drawing: &Drawing2D) {
        let check = |id: String, expected: Placed| {
            let row = live.row_id_for_canonical_id(&id).unwrap();
            let (actual, diagnostics) = run(|cx| live.placed_with_source(row, cx));
            assert!(diagnostics.is_empty());
            assert_eq!(actual.unwrap(), Some(expected));
        };
        for space in &drawing.spaces {
            let place = id::space(space.id);
            let mut namer = id::Namer::new(&place);
            for (index, element) in space.elements.iter().enumerate() {
                check(namer.next(index, element), Placed::Model(element.clone()));
            }
        }
        let mut definitions = id::DefinitionNamer::default();
        for symbol in &drawing.symbols {
            let place = definitions.next(symbol);
            let mut namer = id::Namer::new(&place);
            for (index, element) in symbol.space.elements.iter().enumerate() {
                check(namer.next(index, element), Placed::Model(element.clone()));
            }
        }
        let mut sheets = id::SheetNamer::default();
        for sheet in &drawing.sheets {
            let place = sheets.next(sheet);
            let mut namer = id::Namer::new(&place);
            for (index, element) in sheet.elements.iter().enumerate() {
                check(namer.next(index, element), Placed::Paper(element.clone()));
            }
        }
    }

    #[test]
    fn single_lookup_checks_shared_budget_and_avoids_scanning_unchanged_rows() {
        use zumen_base::{Limit, Limits};
        let mut costs = Vec::new();
        for count in [2, 10_000] {
            let mut features = vec!["layer_feature(\\'A\\','1')".to_owned()];
            features.extend((0..count).map(|index| {
                format!("line_feature('1','1','1','1','0','{index}','10','{index}')")
            }));
            // 用紙のない SFC でも、追加がなければ挿入位置の走査は要らない。
            let bytes = sfc_features(&features.iter().map(String::as_str).collect::<Vec<_>>());
            let mut live = run(|cx| SxfLive::open(&bytes, cx)).0.unwrap();
            let row = live.row_id_for_instance(20).unwrap();
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let value = live
                .placed_with_source(row, &mut Cx::new(&mut budget, &mut diagnostics, &cancel))
                .unwrap()
                .unwrap();
            assert!(diagnostics.is_empty());
            costs.push((budget.used(Kind::Work), budget.used(Kind::DecodedBytes)));
            for kind in [Kind::Work, Kind::DecodedBytes] {
                let used = budget.used(kind);
                for limit in [0, used - 1, used] {
                    let mut budget =
                        Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
                    let result = live.placed_with_source(
                        row,
                        &mut Cx::new(&mut budget, &mut Diagnostics::new(), &cancel),
                    );
                    if limit == used {
                        assert_eq!(result.unwrap(), Some(value.clone()));
                    } else {
                        assert!(matches!(result, Err(Error::Stopped(_))));
                    }
                }
            }
            let cancelled = CancelToken::new();
            cancelled.cancel();
            assert!(matches!(
                live.placed_with_source(
                    row,
                    &mut Cx::new(&mut Budget::default(), &mut Diagnostics::new(), &cancelled),
                ),
                Err(Error::Stopped(_))
            ));
            let missing = RowId(u32::MAX);
            assert_eq!(
                run(|cx| live.placed_with_source(missing, cx)).0.unwrap(),
                None
            );
            let changes = Changes {
                document: None,
                edits: vec![Edit::Remove {
                    target: seen(&live, 20),
                }],
            };
            run(|cx| live.apply(&changes, cx)).0.unwrap();
            assert_eq!(run(|cx| live.placed_with_source(row, cx)).0.unwrap(), None);
            assert!(run(|cx| live.undo(cx)).0.unwrap());
            assert_eq!(
                run(|cx| live.placed_with_source(row, cx)).0.unwrap(),
                Some(value)
            );
            assert_eq!(run(|cx| live.save(cx)).0.unwrap(), bytes);
        }
        assert_eq!(costs[0], costs[1]);
    }

    fn assert_source_parity(live: &SxfLive, file: &SxfFile) {
        let expected = run(|cx| file.lift(cx));
        let held = run(|cx| live.drawing(cx));
        assert_eq!(held, expected);
        assert_single_sources(live, held.0.as_ref().unwrap());
        let saved = run(|cx| live.save(cx)).0.unwrap();
        let reference = run(|cx| {
            let mut bytes = Vec::new();
            Sxf::write(file, &mut bytes, cx).map(|()| bytes)
        })
        .0
        .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&saved),
            String::from_utf8_lossy(&reference)
        );
        assert_eq!(saved, reference);
        let reread = run(|cx| Sxf::read(&saved, cx).and_then(|file| file.lift(cx)));
        assert_eq!(held, reread);
    }

    fn sxf_color(value: u32) -> Inherit<zumen_drawing::Color> {
        Inherit::Own(zumen_drawing::Color {
            code: Some(zumen_drawing::ColorCode {
                system: zumen_drawing::ColorSystem::Sxf,
                value,
            }),
            rgb: None,
        })
    }

    fn paper(live: &SxfLive, instance: u64) -> Element<PaperPoint> {
        let row = live.row_id_for_instance(instance).unwrap();
        let Some(Placed::Paper(element)) = live.placed(row) else {
            panic!()
        };
        element.clone()
    }

    fn seen_layer(live: &SxfLive, at: usize) -> (Seen, Layer) {
        let layer = run(|cx| live.drawing(cx)).0.unwrap().layers[at].clone();
        let seen = Seen {
            id: id::layer(&layer),
            hash: Hash::layer(&layer),
        };
        (seen, layer)
    }

    #[test]
    fn sfc_text_and_layer_edits_match_batch_and_undo_redo() {
        use zumen_drawing::{
            Height, HeightMeaning, Horizontal, LayerId, Length, PaperLength, Piece, Placement,
            Text, Vertical,
        };
        let bytes = sfc_features(&[
            "layer_feature(\\'A\\','1')",
            "layer_feature(\\'B\\')",
            "text_font_feature(\\'MS\\')",
            "text_string_feature('1','1','1',\\'old\\','1','1','4','8','0.25','0','0','1','1')",
            "text_string_feature('1','1','1',\\'old\\','1','1','4','8','0.25','0','0','1')",
            "line_feature('1','1','1','1','0','0','10','10')",
            "drawing_sheet_feature(\\'S\\','3','1','0','0')",
        ]);
        let mut file = run(|cx| Sxf::read(&bytes, cx)).0.unwrap();
        let mut live = run(|cx| SxfLive::open(&bytes, cx)).0.unwrap();
        let place = id::sheet(&run(|cx| live.drawing(cx)).0.unwrap().sheets[0]);
        let mut snapshots = vec![(run(|cx| live.drawing(cx)).0.unwrap(), bytes.clone())];
        let mut step = |live: &mut SxfLive, file: &mut SxfFile, edits: Vec<Edit>| {
            let changes = Changes {
                document: None,
                edits,
            };
            let expected = run(|cx| file.apply(&changes, cx));
            let actual = run(|cx| live.apply(&changes, cx));
            assert_eq!(actual, expected);
            let (applied, diagnostics) = actual;
            let applied = applied.unwrap();
            assert_source_parity(live, file);
            snapshots.push((
                run(|cx| live.drawing(cx)).0.unwrap(),
                run(|cx| live.save(cx)).0.unwrap(),
            ));
            (applied, diagnostics)
        };
        macro_rules! step {
            ($edits:expr $(,)?) => {{
                let edits = $edits;
                step(&mut live, &mut file, edits)
            }};
        }

        step!(vec![Edit::Restyle {
            target: seen(&live, 40),
            style: Restyle {
                color: Some(sxf_color(4)),
                layer: Some(LayerId(2)),
                ..Restyle::default()
            },
        }],);
        let mut element = paper(&live, 40);
        let Shape::Formatted(text) = &mut element.shape else {
            panic!()
        };
        text.content = "変更'\\表記".into();
        text.spans[0].piece = Piece::Text(text.content.clone());
        text.height.value = Length::Paper(PaperLength(8.0));
        element.layer = LayerId(1);
        element.style.color = sxf_color(3);
        let mut other = paper(&live, 50);
        let Shape::Formatted(text) = &mut other.shape else {
            panic!()
        };
        text.content = "横".into();
        text.spans[0].piece = Piece::Text(text.content.clone());
        step!(vec![
            Edit::Replace {
                target: seen(&live, 40),
                element: Box::new(Placed::Paper(element)),
            },
            Edit::Replace {
                target: seen(&live, 50),
                element: Box::new(Placed::Paper(other)),
            },
            Edit::Transform {
                target: seen(&live, 60),
                transform: Affine::translation(5.0, 0.0),
            },
        ],);
        step!(vec![Edit::Transform {
            target: seen(&live, 40),
            transform: Affine {
                m: [0.0, -1.0, 20.0, 1.0, 0.0, 30.0],
            },
        }],);
        let mut added = paper(&live, 40);
        added.shape = Shape::Text(Text {
            content: "日本'\\語".into(),
            height: Height {
                value: Length::Paper(PaperLength(3.5)),
                meaning: HeightMeaning::Frame,
            },
            placement: Placement::Anchor {
                point: PaperPoint {
                    sheet: self_sheet(&live),
                    x: 10.0,
                    y: 20.0,
                },
                horizontal: Horizontal::Center,
                vertical: Vertical::Top,
            },
            rotation: 30.0,
            width_factor: 1.0,
            oblique: 10.0,
            backward: false,
            upside_down: false,
            style: "ＭＳ ゴシック".into(),
        });
        let (result, diagnostics) = step!(vec![Edit::Add {
            place: place.clone(),
            element: Box::new(Placed::Paper(added)),
        }],);
        assert_eq!(
            diagnostics.iter().map(Diagnostic::code).collect::<Vec<_>>(),
            [codes::SXF_TEXT_WIDTH_APPROXIMATED]
        );
        let added_id = result[0].id.clone().unwrap();
        let added_instance: u64 = added_id.rsplit_once("/i").unwrap().1.parse().unwrap();

        let (target, was) = seen_layer(&live, 0);
        let mut renamed = was.clone();
        renamed.name = "改名'\\図面".into();
        renamed.state.as_mut().unwrap().visible = false;
        renamed.state.as_mut().unwrap().printable = false;
        let (hidden_target, hidden_was) = seen_layer(&live, 1);
        let mut hidden = hidden_was.clone();
        hidden.state.as_mut().unwrap().visible = false;
        hidden.state.as_mut().unwrap().printable = false;
        let mut new_layer = renamed.clone();
        new_layer.name = "追加レイヤ".into();
        step!(vec![
            Edit::ReplaceLayer {
                target,
                layer: Box::new(renamed),
            },
            Edit::ReplaceLayer {
                target: hidden_target,
                layer: Box::new(hidden),
            },
            Edit::AddLayer {
                layer: Box::new(new_layer),
            },
        ],);
        let (target, was) = seen_layer(&live, 2);
        let mut shown = was.clone();
        shown.name = "再改名".into();
        shown.state.as_mut().unwrap().visible = true;
        shown.state.as_mut().unwrap().printable = true;
        step!(vec![Edit::ReplaceLayer {
            target,
            layer: Box::new(shown),
        }],);
        let mut element = paper(&live, added_instance);
        let Shape::Formatted(text) = &mut element.shape else {
            panic!()
        };
        text.spans[0].format.font = Some(text::font("Second"));
        element.layer = LayerId(3);
        step!(vec![
            Edit::Replace {
                target: seen(&live, added_instance),
                element: Box::new(Placed::Paper(element)),
            },
            Edit::Restyle {
                target: seen(&live, 60),
                style: Restyle {
                    layer: Some(LayerId(3)),
                    ..Restyle::default()
                },
            },
        ],);

        let last = snapshots.len() - 1;
        for at in (0..last).rev() {
            assert!(run(|cx| live.undo(cx)).0.unwrap());
            assert_eq!(run(|cx| live.drawing(cx)).0.unwrap(), snapshots[at].0);
            assert_eq!(run(|cx| live.save(cx)).0.unwrap(), snapshots[at].1);
        }
        assert_eq!(run(|cx| live.save(cx)).0.unwrap(), bytes);
        for snapshot in &snapshots[1..] {
            assert!(run(|cx| live.redo(cx)).0.unwrap());
            assert_eq!(run(|cx| live.drawing(cx)).0.unwrap(), snapshot.0);
            assert_eq!(run(|cx| live.save(cx)).0.unwrap(), snapshot.1);
        }
        assert_source_parity(&live, &file);
    }

    #[test]
    fn p21_text_and_layer_edits_match_batch_and_undo_redo() {
        use zumen_drawing::{
            Height, HeightMeaning, Horizontal, LayerId, Length, PaperLength, Piece, Placement,
            Text, Vertical,
        };
        let instance_of = |applied: &Applied| -> u64 {
            let id = applied.id.clone().unwrap();
            id.rsplit_once("/i").unwrap().1.parse().unwrap()
        };
        let new_text = |sheet: SheetId, content: &str, style: &str| {
            Shape::Text(Text {
                content: content.into(),
                height: Height {
                    value: Length::Paper(PaperLength(3.5)),
                    meaning: HeightMeaning::Frame,
                },
                placement: Placement::Anchor {
                    point: PaperPoint {
                        sheet,
                        x: 10.0,
                        y: 20.0,
                    },
                    horizontal: Horizontal::Center,
                    vertical: Vertical::Top,
                },
                rotation: 30.0,
                width_factor: 1.0,
                oblique: 10.0,
                backward: false,
                upside_down: false,
                style: style.into(),
            })
        };

        // 文字と 2 つ目のレイヤを持つ P21 を、バッチで作って始めの図面にする
        let lines = p21_lines();
        let mut seed = run(|cx| Sxf::read(&lines, cx)).0.unwrap();
        let seed_live = run(|cx| SxfLive::open(&lines, cx)).0.unwrap();
        let place = id::sheet(&run(|cx| seed_live.drawing(cx)).0.unwrap().sheets[0]);
        let (_, first_layer) = seen_layer(&seed_live, 0);
        let mut second_layer = first_layer.clone();
        second_layer.name = "B".into();
        let mut seeded = paper(&seed_live, 160);
        seeded.style.line_type = Inherit::Layer;
        seeded.style.line_weight = None;
        seeded.shape = new_text(seeded.shape_sheet(), "old", "MS");
        run(|cx| {
            seed.apply(
                &Changes {
                    document: None,
                    edits: vec![Edit::AddLayer {
                        layer: Box::new(second_layer),
                    }],
                },
                cx,
            )
        })
        .0
        .unwrap();
        let seeded_result = run(|cx| {
            seed.apply(
                &Changes {
                    document: None,
                    edits: vec![Edit::Add {
                        place: place.clone(),
                        element: Box::new(Placed::Paper(seeded)),
                    }],
                },
                cx,
            )
        });
        let seeded_result = seeded_result
            .0
            .unwrap_or_else(|_| panic!("{:?}", seeded_result.1));
        let text_instance = instance_of(&seeded_result[0]);
        let bytes = run(|cx| {
            let mut bytes = Vec::new();
            Sxf::write(&seed, &mut bytes, cx).map(|()| bytes)
        })
        .0
        .unwrap();

        let mut file = run(|cx| Sxf::read(&bytes, cx)).0.unwrap();
        let mut live = run(|cx| SxfLive::open(&bytes, cx)).0.unwrap();
        let mut snapshots = vec![(run(|cx| live.drawing(cx)).0.unwrap(), bytes.clone())];
        let mut step = |live: &mut SxfLive, file: &mut SxfFile, edits: Vec<Edit>| {
            let changes = Changes {
                document: None,
                edits,
            };
            let expected = run(|cx| file.apply(&changes, cx));
            let actual = run(|cx| live.apply(&changes, cx));
            assert_eq!(actual, expected);
            let (applied, diagnostics) = actual;
            let applied = applied.unwrap_or_else(|_| panic!("{diagnostics:?}"));
            assert_source_parity(live, file);
            snapshots.push((
                run(|cx| live.drawing(cx)).0.unwrap(),
                run(|cx| live.save(cx)).0.unwrap(),
            ));
            (applied, diagnostics)
        };
        macro_rules! step {
            ($edits:expr $(,)?) => {{
                let edits = $edits;
                step(&mut live, &mut file, edits)
            }};
        }

        step!(vec![Edit::Restyle {
            target: seen(&live, text_instance),
            style: Restyle {
                color: Some(sxf_color(4)),
                layer: Some(LayerId(2)),
                ..Restyle::default()
            },
        }],);
        let mut element = paper(&live, text_instance);
        let Shape::Formatted(text) = &mut element.shape else {
            panic!()
        };
        text.content = "変更'\\表記".into();
        text.spans[0].piece = Piece::Text(text.content.clone());
        text.height.value = Length::Paper(PaperLength(8.0));
        element.layer = LayerId(1);
        element.style.color = sxf_color(3);
        step!(vec![
            Edit::Replace {
                target: seen(&live, text_instance),
                element: Box::new(Placed::Paper(element)),
            },
            Edit::Transform {
                target: seen(&live, 160),
                transform: Affine::translation(5.0, 0.0),
            },
        ],);
        step!(vec![Edit::Transform {
            target: seen(&live, text_instance),
            transform: Affine {
                m: [0.0, -1.0, 20.0, 1.0, 0.0, 30.0],
            },
        }],);
        let mut added = paper(&live, 170);
        added.style.line_type = Inherit::Layer;
        added.style.line_weight = None;
        added.shape = new_text(added.shape_sheet(), "日本'\\語", "ＭＳ ゴシック");
        let (result, _) = step!(vec![Edit::Add {
            place: place.clone(),
            element: Box::new(Placed::Paper(added)),
        }],);
        let added_instance = instance_of(&result[0]);

        let (target, was) = seen_layer(&live, 0);
        let mut renamed = was.clone();
        renamed.name = "改名'\\図面".into();
        renamed.state.as_mut().unwrap().visible = false;
        renamed.state.as_mut().unwrap().printable = false;
        let (hidden_target, hidden_was) = seen_layer(&live, 1);
        let mut hidden = hidden_was.clone();
        hidden.state.as_mut().unwrap().visible = false;
        hidden.state.as_mut().unwrap().printable = false;
        let mut new_layer = was.clone();
        new_layer.name = "追加レイヤ".into();
        step!(vec![
            Edit::ReplaceLayer {
                target,
                layer: Box::new(renamed),
            },
            Edit::ReplaceLayer {
                target: hidden_target,
                layer: Box::new(hidden),
            },
            Edit::AddLayer {
                layer: Box::new(new_layer),
            },
        ],);
        let (target, was) = seen_layer(&live, 1);
        let mut shown = was.clone();
        shown.name = "再改名".into();
        shown.state.as_mut().unwrap().visible = true;
        shown.state.as_mut().unwrap().printable = true;
        step!(vec![Edit::ReplaceLayer {
            target,
            layer: Box::new(shown),
        }],);
        let mut element = paper(&live, added_instance);
        let Shape::Formatted(text) = &mut element.shape else {
            panic!()
        };
        text.spans[0].format.font = Some(text::font("Second"));
        element.layer = LayerId(3);
        step!(vec![
            Edit::Replace {
                target: seen(&live, added_instance),
                element: Box::new(Placed::Paper(element)),
            },
            Edit::Restyle {
                target: seen(&live, 160),
                style: Restyle {
                    layer: Some(LayerId(3)),
                    ..Restyle::default()
                },
            },
        ],);

        let last = snapshots.len() - 1;
        for at in (0..last).rev() {
            assert!(run(|cx| live.undo(cx)).0.unwrap());
            assert_eq!(run(|cx| live.drawing(cx)).0.unwrap(), snapshots[at].0);
            assert_eq!(run(|cx| live.save(cx)).0.unwrap(), snapshots[at].1);
        }
        assert_eq!(run(|cx| live.save(cx)).0.unwrap(), bytes);
        for snapshot in &snapshots[1..] {
            assert!(run(|cx| live.redo(cx)).0.unwrap());
            assert_eq!(run(|cx| live.drawing(cx)).0.unwrap(), snapshot.0);
            assert_eq!(run(|cx| live.save(cx)).0.unwrap(), snapshot.1);
        }
        assert_source_parity(&live, &file);
    }

    fn self_sheet(live: &SxfLive) -> SheetId {
        live.sheet_template.id
    }

    #[test]
    fn sfc_and_p21_add_remove_sequences_preserve_full_sources_and_undo() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples/sxf/ezsxf-writer-all-features.sfc");
        let no_sheet = concat!(
            "ISO-10303-21;\nHEADER;\n",
            "FILE_DESCRIPTION(('SCADEC level2 feature_mode'),'2;1');\n",
            "FILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n",
            "/*SXF\n#10 = layer_feature(\\'A\\','1')\nSXF*/\n",
            "/*SXF\n#20 = line_feature('1','1','1','1','0','0','20','10')\nSXF*/\n",
            "/*SXF\n#30 = line_feature('1','1','1','1','30','0','40','10')\nSXF*/\n",
            "ENDSEC;\nEND-ISO-10303-21;\n",
        )
        .as_bytes()
        .to_vec();
        for bytes in [std::fs::read(path).unwrap(), p21_lines(), no_sheet] {
            for threads in [1, 4] {
                let mut file = run(|cx| Sxf::read(&bytes, cx)).0.unwrap();
                let mut live = run(|cx| SxfLive::open_with_threads(&bytes, threads, cx))
                    .0
                    .unwrap();
                let initial = run(|cx| live.drawing(cx)).0.unwrap();
                let sheet = &initial.sheets[0];
                let place = id::sheet(sheet);
                let line = sheet
                    .elements
                    .iter()
                    .find(|element| matches!(element.shape, Shape::Line { .. }))
                    .unwrap()
                    .clone();
                let mut history = vec![(initial, bytes.clone())];
                for step in 0..5 {
                    let edit = if step < 2 || step == 4 {
                        Edit::Add {
                            place: place.clone(),
                            element: Box::new(Placed::Paper(line.clone())),
                        }
                    } else {
                        let model = run(|cx| live.drawing(cx)).0.unwrap();
                        let sheet = &model.sheets[0];
                        let mut namer = id::Namer::new(&place);
                        let mut lines = Vec::new();
                        for (index, element) in sheet.elements.iter().enumerate() {
                            let id = namer.next(index, element);
                            if matches!(element.shape, Shape::Line { .. }) {
                                lines.push(Seen {
                                    id,
                                    hash: Hash::element(element),
                                });
                            }
                        }
                        Edit::Remove {
                            target: if step == 2 {
                                lines.remove(0)
                            } else {
                                lines.pop().unwrap()
                            },
                        }
                    };
                    let changes = Changes {
                        document: None,
                        edits: vec![edit],
                    };
                    let actual = run(|cx| live.apply(&changes, cx));
                    assert_eq!(actual, run(|cx| file.apply(&changes, cx)));
                    actual.0.unwrap();
                    assert_source_parity(&live, &file);
                    history.push((
                        run(|cx| live.drawing(cx)).0.unwrap(),
                        run(|cx| live.save(cx)).0.unwrap(),
                    ));
                }
                for expected in history.iter().rev().skip(1) {
                    assert!(run(|cx| live.undo(cx)).0.unwrap());
                    assert_eq!(&run(|cx| live.drawing(cx)).0.unwrap(), &expected.0);
                    assert_single_sources(&live, &expected.0);
                    assert_eq!(&run(|cx| live.save(cx)).0.unwrap(), &expected.1);
                }
                for expected in history.iter().skip(1) {
                    assert!(run(|cx| live.redo(cx)).0.unwrap());
                    assert_eq!(&run(|cx| live.drawing(cx)).0.unwrap(), &expected.0);
                    assert_single_sources(&live, &expected.0);
                    assert_eq!(&run(|cx| live.save(cx)).0.unwrap(), &expected.1);
                }
            }
        }
    }
}
