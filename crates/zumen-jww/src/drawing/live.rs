//! JWW の常駐文書エンジン（`docs/live.md`、#878）。
//!
//! 元のバイト列を [`SourceBytes`](zumen_live::SourceBytes) のページ配列に保ち、図面要素を
//! [`VersionedTable`](zumen_live::VersionedTable)・[`SpatialGrid`](zumen_live::SpatialGrid)・
//! [`Display`](zumen_live::Display) に常駐させる。JWW の要素はハンドルを持たずレコード番号で ID を作るため、
//! 基底レコード列と追加レコード列の生存ビット列 [`RankBitset`](zumen_live::RankBitset) により、
//! 実効レコード番号と内部スロット番号の相互変換および `shifted` 判定を要素総数に依らない定数時間で行う。

use core::f64::consts::TAU;
use core::ops::Range;
use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Cx, Diagnostic, Kind, Ref, codes};
use zumen_drawing::{
    Affine, Applied, Axes, Boundary, Changes, Color, Draft, Drawing2D, Edge, Edit, Element, Field,
    FieldValue, Hash, HatchLoop, Height, Inherit, LayerId, Length, LineTypeId, ModelPoint,
    ModelSpace, PaperLength, Placed, Placement, Position, Seen, Shape, Sheet, SheetItem, SpaceId,
    Style, SymbolDefinition, Unit, View, transform,
};
use zumen_live::display::{Display, DisplayWriter, radians_from_deg};
use zumen_live::{
    CommittedTxn, EMPTY_BOUNDS, ExistingRowDiff, History, PatchPayload, RankBitset, Revision,
    RowId, SideIndexes, SourceBytes, SpatialGrid, TableRow, TxnError, VersionedTable,
    run_transaction,
};
use zumen_sdk::{Codec, Error, quietly};

use super::apply::{
    ARC, Base as RawBase, FORMAT, LINE, POINT, base_for_style, class_of, count,
    encode_added_entity, kind_of, near, scale_of_group, shape_bytes, space_of, style_of_base,
    unset, unsupported, unturned, write_count, write_object_tag,
};
use super::{
    DEFINITION_SPACES, Header, PAPERS, POINT_SYMBOL, SCHEMA, SHEET, SingleEntity, block_key,
    build_edit_state_drafts, build_layers, dropped, images, invalid, lift_single_entity,
    make_line_type, read_block_definition,
};
use crate::parse::{self, Reader};
use crate::{Jww, JwwFile, RecordKind};

/// 既定の履歴上限（64 MiB）
pub const DEFAULT_HISTORY_MAX_BYTES: usize = 64 * 1024 * 1024;

const DISPLAY_TOLERANCE: f64 = 0.5;

/// ファイル内のレコードスロット（開いた時点の基底レコードか、追加した先頭図形一覧のレコード）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum SlotRef {
    Base(usize),
    Born(usize),
}

/// [`VersionedTable`] に置く JWW の 1 要素
#[derive(Clone, Debug)]
struct JwwRow {
    slot: SlotRef,
    space: SpaceId,
    element: Element<ModelPoint>,
    own_extensions: Vec<(&'static str, Vec<Field>)>,
    /// `Shape::Underlay` の `resource.definition` が指す基底レコード番号（`None` なら自分自身のレコード番号）
    underlay_image_record: Option<usize>,
}

impl TableRow for JwwRow {
    #[inline]
    fn row_handle(&self) -> Option<u64> {
        None
    }

    fn extra_heap_bytes(&self) -> usize {
        let ext_bytes: usize = self
            .own_extensions
            .iter()
            .map(|(_, fields)| fields.capacity() * core::mem::size_of::<Field>())
            .sum();
        let shape_bytes = match &self.element.shape {
            Shape::Text(t) => t.content.capacity() + t.style.capacity(),
            Shape::Symbol(s) => s.key.name.capacity(),
            Shape::Underlay(u) => u.resource.path.capacity(),
            Shape::Hatch(h) => h.loops.capacity() * core::mem::size_of::<HatchLoop<ModelPoint>>(),
            Shape::Dimension(_) => 128,
            _ => 0,
        };
        ext_bytes + shape_bytes
    }
}

#[derive(Clone, Debug)]
enum BaseOutcome {
    None,
    Element(RowId),
    BlockDef {
        symbol_index: usize,
        number: u32,
        referenced: u32,
        name: String,
    },
    DocumentDraft {
        namespace: &'static str,
        name: String,
        fields: Vec<Field>,
    },
    Dropped,
    Invalid,
}

#[derive(Clone, Debug)]
struct BaseRecord {
    span: Range<usize>,
    body: Range<usize>,
    kind: RecordKind,
    kind_name: Vec<u8>,
    class_bucket: Option<u16>,
    block: Option<usize>,
    outcome: BaseOutcome,
}

#[derive(Clone, Debug)]
struct BornRecord {
    class_name: &'static [u8],
    schema: u16,
    class_bucket: u16,
    body: Vec<u8>,
    row_id: RowId,
    group: usize,
}

#[derive(Clone, Debug)]
struct ClassBucket {
    name: Vec<u8>,
    schema: u16,
    alive_count: u32,
}

#[derive(Clone, Debug)]
struct BlockSpan {
    def_record: usize,
    head_end: usize,
    inner_start: usize,
    inner_end: usize,
}

/// 一覧の再構築（`Add`・`Remove`）に必要な構造境界
#[derive(Clone, Debug)]
struct ListLayout {
    header_end: usize,
    top_end: usize,
    blocks: Vec<BlockSpan>,
    after_blocks_record: usize,
    tail_byte_start: usize,
    standard_header_end: bool,
}

#[derive(Clone, Debug)]
enum BodyDiff {
    Base {
        record: usize,
        before: Option<Vec<u8>>,
        after: Vec<u8>,
    },
    Born {
        index: usize,
        before: Vec<u8>,
        after: Vec<u8>,
    },
}

impl BodyDiff {
    fn heap_bytes(&self) -> usize {
        match self {
            Self::Base { before, after, .. } => {
                before.as_ref().map_or(0, Vec::capacity) + after.capacity()
            }
            Self::Born { before, after, .. } => before.capacity() + after.capacity(),
        }
    }
}

#[derive(Clone, Debug)]
struct RemovedSlot {
    slot: SlotRef,
    class_bucket: Option<u16>,
    top_group: Option<usize>,
}

/// 1 トランザクション分の JWW 固有の差分記録
#[derive(Clone, Debug, Default)]
struct JwwStepPatch {
    bodies: Vec<BodyDiff>,
    removed: Vec<RemovedSlot>,
    added: Vec<BornRecord>,
}

impl PatchPayload for JwwStepPatch {
    fn heap_bytes(&self) -> usize {
        let b: usize = self.bodies.iter().map(BodyDiff::heap_bytes).sum();
        let r = self.removed.capacity() * core::mem::size_of::<RemovedSlot>();
        let a: usize = self
            .added
            .iter()
            .map(|item| item.body.capacity() + core::mem::size_of::<BornRecord>())
            .sum();
        b + r + a
    }
}

/// JWW の常駐文書エンジン
#[derive(Clone)]
pub struct JwwLive {
    source: SourceBytes,
    version: u32,
    header_valid: bool,
    header: Header,
    paper_origin: [f64; 2],
    list_layout: Option<ListLayout>,
    top_end: usize,
    base_records: Vec<BaseRecord>,
    patched_bodies: BTreeMap<usize, Vec<u8>>,
    born_records: Vec<BornRecord>,
    base_alive: RankBitset,
    born_alive: RankBitset,
    element_base_alive: RankBitset,
    element_born_alive: RankBitset,
    class_buckets: Vec<ClassBucket>,
    same_name_multi_schema: bool,
    images: BTreeMap<String, usize>,
    symbol_index_by_key: BTreeMap<String, u32>,
    symbol_bounds: Vec<Option<[f64; 4]>>,
    definition_rows: Vec<Vec<RowId>>,
    underlay_rows: Vec<RowId>,
    group_counts: [u32; 16],
    lists_rebuilt_steps: u32,
    table: VersionedTable<JwwRow>,
    spatial: SpatialGrid,
    display: Display,
    history: History<JwwRow, JwwStepPatch>,
    history_max_bytes: usize,
}

struct WorkerItem {
    outcome: WorkerOutcome,
    bounds: [f64; 4],
}

#[expect(clippy::large_enum_variant)]
enum WorkerOutcome {
    Skip,
    Element {
        element: Element<ModelPoint>,
        own: Vec<(&'static str, Vec<Field>)>,
        embedded_image: Option<usize>,
    },
    DocumentDraft {
        namespace: &'static str,
        name: String,
        fields: Vec<Field>,
    },
    Dropped,
    Invalid,
}

struct StagedEdit {
    kind_label: &'static str,
    action: StagedAction,
}

enum StagedAction {
    Removed {
        row_id: RowId,
        slot: SlotRef,
        class_bucket: Option<u16>,
        top_group: Option<usize>,
    },
    Updated {
        row_id: RowId,
        slot: SlotRef,
        after_row: JwwRow,
        after_bounds: [f64; 4],
        body_diff: BodyDiff,
    },
    Added {
        born_offset: usize,
        class_name: &'static [u8],
        schema: u16,
        class_bucket: u16,
        body: Vec<u8>,
        after_row: JwwRow,
        after_bounds: [f64; 4],
        group: usize,
    },
}

impl JwwLive {
    /// JWW のバイト列を開いて常駐文書エンジンを構築する
    pub fn open(src: &[u8], cx: &mut Cx<'_>) -> Result<Self, Error> {
        Self::open_with_threads(src, 1, cx)
    }

    /// 指定したスレッド数で JWW のバイト列を開いて常駐文書エンジンを構築する
    pub fn open_with_threads(src: &[u8], threads: usize, cx: &mut Cx<'_>) -> Result<Self, Error> {
        let file = Jww::read(src, cx)?;
        Self::build_from_file(&file, threads.max(1), cx)
    }

    /// 既存の [`JwwFile`] から常駐文書エンジンを構築する
    pub fn from_file(file: &JwwFile, cx: &mut Cx<'_>) -> Result<Self, Error> {
        Self::build_from_file(file, 1, cx)
    }

    fn build_from_file(file: &JwwFile, threads: usize, cx: &mut Cx<'_>) -> Result<Self, Error> {
        let full_bytes = file.as_bytes();
        let source =
            SourceBytes::compress_slice_with_cx(full_bytes, threads, cx).map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Error::Stopped(stop)
            })?;
        let version = file.version();
        let parsed_header = file
            .records()
            .first()
            .filter(|record| record.kind() == RecordKind::Header)
            .and_then(|record| Header::read_slice(file.bytes(0..record.span().end), version).ok());
        let header_valid = parsed_header.is_some();
        if !header_valid {
            cx.report(Diagnostic::new(codes::JWW_HEADER_INVALID).at(Ref::at_record(0)));
        }
        let header = parsed_header.unwrap_or_default();
        let paper_origin = paper_origin_of(&header);
        let images_map = images(file);
        let list_layout = inspect_list_layout(file, full_bytes, version);
        let top_end = list_layout
            .as_ref()
            .map_or_else(|| file.records().len(), |l| l.top_end);

        let mut class_buckets: Vec<ClassBucket> = Vec::new();
        let mut bucket_lookup: BTreeMap<(Vec<u8>, u16), u16> = BTreeMap::new();
        let mut seen_names: BTreeMap<Vec<u8>, u16> = BTreeMap::new();
        let mut same_name_multi_schema = false;

        // ブロック定義を先に走査して空間番号と名前表を確定する（決定 0135）
        let records = file.records();
        let mut definitions: BTreeMap<usize, (usize, SpaceId)> = BTreeMap::new();
        let mut block_parsed: BTreeMap<usize, BaseOutcome> = BTreeMap::new();
        let mut symbol_index_by_key: BTreeMap<String, u32> = BTreeMap::new();
        let mut symbol_count = 0usize;
        for (index, record) in records.iter().enumerate() {
            if record.kind() == RecordKind::Block
                && let Ok((number, referenced, name)) =
                    read_block_definition(file.bytes(record.body()), version)
            {
                let sym_idx = symbol_count;
                let space = SpaceId(DEFINITION_SPACES + sym_idx as u32);
                symbol_count += 1;
                definitions.insert(index, (sym_idx, space));
                symbol_index_by_key
                    .entry(number.to_string())
                    .or_insert(sym_idx as u32);
                block_parsed.insert(
                    index,
                    BaseOutcome::BlockDef {
                        symbol_index: sym_idx,
                        number,
                        referenced,
                        name,
                    },
                );
            }
        }

        // 図形レコードの単体変換は純粋関数なので、スレッド数に応じて分割して計算し、元の順序で統合する
        let record_len = records.len();
        let worker_results = lift_records_deterministic(
            file,
            &header,
            paper_origin,
            &definitions,
            &images_map,
            threads,
        );

        let mut base_records = Vec::with_capacity(record_len);
        let base_alive = RankBitset::all_ones(record_len);
        let mut element_base_alive = RankBitset::all_zeros(record_len);
        let mut initial_rows: Vec<(JwwRow, [f64; 4], bool)> = Vec::new();
        let mut definition_rows: Vec<Vec<RowId>> = vec![Vec::new(); symbol_count];
        let mut symbol_bounds: Vec<Option<[f64; 4]>> = vec![None; symbol_count];
        let mut top_symbol_rows: Vec<usize> = Vec::new();
        let mut underlay_rows: Vec<RowId> = Vec::new();
        let mut group_counts = [0u32; 16];

        for (index, (record, worker)) in records.iter().zip(worker_results).enumerate() {
            let source = Ref::at_record(index as u64);
            cx.check(Kind::Work, 1).map_err(|stop| {
                cx.report(stop.to_diagnostic().at(source.clone()));
                Error::Stopped(stop)
            })?;
            let kind_name = file.kind_name(record).to_vec();
            let class_bucket = match record.kind() {
                RecordKind::Entity | RecordKind::Block => record
                    .tag()
                    .and_then(|(_, cls_idx)| file.classes().get(cls_idx))
                    .and_then(|cls| {
                        let name = file.bytes(cls.name()).to_vec();
                        let schema = cls.schema();
                        if let Some(&prev_schema) = seen_names.get(&name) {
                            if prev_schema != schema {
                                same_name_multi_schema = true;
                            }
                        } else {
                            seen_names.insert(name.clone(), schema);
                        }
                        let key = (name.clone(), schema);
                        if let Some(&b_idx) = bucket_lookup.get(&key) {
                            if let Some(bucket) = class_buckets.get_mut(usize::from(b_idx)) {
                                bucket.alive_count = bucket.alive_count.saturating_add(1);
                            }
                            Some(b_idx)
                        } else {
                            let b_idx = u16::try_from(class_buckets.len()).ok()?;
                            class_buckets.push(ClassBucket {
                                name,
                                schema,
                                alive_count: 1,
                            });
                            bucket_lookup.insert(key, b_idx);
                            Some(b_idx)
                        }
                    }),
                _ => None,
            };

            let outcome = match record.kind() {
                RecordKind::Block => match block_parsed.remove(&index) {
                    Some(def) => def,
                    None => {
                        invalid(cx, &kind_name, source);
                        BaseOutcome::Invalid
                    }
                },
                RecordKind::Entity => match worker.outcome {
                    WorkerOutcome::Skip => BaseOutcome::None,
                    WorkerOutcome::Dropped => {
                        dropped(cx, &kind_name, source);
                        BaseOutcome::Dropped
                    }
                    WorkerOutcome::Invalid => {
                        invalid(cx, &kind_name, source);
                        BaseOutcome::Invalid
                    }
                    WorkerOutcome::DocumentDraft {
                        namespace,
                        name,
                        fields,
                    } => BaseOutcome::DocumentDraft {
                        namespace,
                        name,
                        fields,
                    },
                    WorkerOutcome::Element {
                        element,
                        own,
                        embedded_image,
                    } => {
                        let row_idx = initial_rows.len();
                        let Some(row_id) = RowId::from_index(row_idx) else {
                            let stop = zumen_sdk::Stop::Exceeded(zumen_base::Exceeded {
                                kind: Kind::Elements,
                                limit: u64::from(u32::MAX - 1),
                                requested: (row_idx as u64).saturating_add(1),
                            });
                            cx.report(stop.to_diagnostic().at(source));
                            return Err(Error::Stopped(stop));
                        };
                        let space = element.space_hint(record.block(), &definitions);
                        let is_underlay = matches!(element.shape, Shape::Underlay(_));
                        let is_symbol = matches!(element.shape, Shape::Symbol(_));
                        if space.0 < DEFINITION_SPACES
                            && let Some(cnt) = group_counts.get_mut(space.0 as usize)
                        {
                            *cnt = cnt.saturating_add(1);
                        }
                        if let Some(def_idx) = space.0.checked_sub(DEFINITION_SPACES) {
                            let model_box = model_element_bounds(&element.shape);
                            if is_finite_box(model_box)
                                && let Some(slot_box) = symbol_bounds.get_mut(def_idx as usize)
                            {
                                *slot_box = Some(match *slot_box {
                                    Some(prev) => union_box(prev, model_box),
                                    None => model_box,
                                });
                            }
                        }
                        let row = JwwRow {
                            slot: SlotRef::Base(index),
                            space,
                            element,
                            own_extensions: own,
                            underlay_image_record: embedded_image,
                        };
                        initial_rows.push((row, worker.bounds, true));
                        element_base_alive.set(index, true);
                        if let Some(def_idx) = space.0.checked_sub(DEFINITION_SPACES)
                            && let Some(rows_in_def) = definition_rows.get_mut(def_idx as usize)
                        {
                            rows_in_def.push(row_id);
                        }
                        if is_underlay {
                            underlay_rows.push(row_id);
                        }
                        if is_symbol && space.0 < DEFINITION_SPACES {
                            top_symbol_rows.push(row_idx);
                        }
                        BaseOutcome::Element(row_id)
                    }
                },
                _ => BaseOutcome::None,
            };

            base_records.push(BaseRecord {
                span: record.span(),
                body: record.body(),
                kind: record.kind(),
                kind_name,
                class_bucket,
                block: record.block(),
                outcome,
            });
        }

        // ブロック定義の囲む箱が決まった後に、最上位の記号参照の囲む箱を反映する
        for row_idx in top_symbol_rows {
            if let Some((row, bbox, _)) = initial_rows.get_mut(row_idx) {
                *bbox = compute_sheet_bounds(
                    &row.element,
                    row.space,
                    &header,
                    paper_origin,
                    &symbol_index_by_key,
                    &symbol_bounds,
                );
            }
        }

        let table = VersionedTable::from_rows_with_cx(initial_rows, cx).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        let spatial = SpatialGrid::build_with_cx(table.bounds_column(), cx).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;

        let mut display = Display::with_rows_with_cx(cx, table.len()).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        for (_, row) in table.iter_live() {
            let (counts, style) =
                display_counts_and_style(&row.element, row.space, &symbol_index_by_key);
            display
                .push_with_cx(cx, counts, style, |w| {
                    write_display_element(
                        &row.element,
                        row.space,
                        &header,
                        paper_origin,
                        &symbol_index_by_key,
                        w,
                    );
                })
                .map_err(|stop| {
                    cx.report(stop.to_diagnostic());
                    Error::Stopped(stop)
                })?;
        }
        display.clear_dirty();

        Ok(Self {
            source,
            version,
            header_valid,
            header,
            paper_origin,
            list_layout,
            top_end,
            base_records,
            patched_bodies: BTreeMap::new(),
            born_records: Vec::new(),
            base_alive,
            born_alive: RankBitset::new(),
            element_base_alive,
            element_born_alive: RankBitset::new(),
            class_buckets,
            same_name_multi_schema,
            images: images_map,
            symbol_index_by_key,
            symbol_bounds,
            definition_rows,
            underlay_rows,
            group_counts,
            lists_rebuilt_steps: 0,
            table,
            spatial,
            display,
            history: History::new(),
            history_max_bytes: DEFAULT_HISTORY_MAX_BYTES,
        })
    }

    /// JWW のファイルバージョン
    #[inline]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// 単調増加のリビジョン番号
    #[inline]
    pub const fn revision(&self) -> Revision {
        self.table.revision()
    }

    /// 論理状態識別子（`undo` で元の値に戻る）
    #[inline]
    pub const fn state_id(&self) -> u64 {
        self.table.state_id()
    }

    /// 現在生きている要素の数（モデル空間とブロック定義空間の合計）
    #[inline]
    pub const fn live_count(&self) -> u32 {
        self.table.live_count()
    }

    /// 差分で更新される表示バッファーへの参照
    #[inline]
    pub const fn display(&self) -> &Display {
        &self.display
    }

    /// 表示バッファーの GPU 変更追跡ビットを消去する
    pub fn clear_display_dirty(&mut self) {
        self.display.clear_dirty();
    }

    /// 履歴が保持する最大バイト数を設定する
    pub fn set_history_max_bytes(&mut self, max_bytes: usize) {
        self.history_max_bytes = max_bytes;
        self.history.trim_to_bytes(max_bytes);
    }

    /// 履歴を空にする
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// 履歴が保持している概算バイト数
    pub fn history_bytes(&self) -> usize {
        self.history.bytes()
    }

    /// 常駐構造全体が保持している概算バイト数
    pub fn resident_bytes(&self) -> usize {
        let patched: usize = self.patched_bodies.values().map(Vec::capacity).sum();
        let born: usize = self
            .born_records
            .iter()
            .map(|b| b.body.capacity() + core::mem::size_of::<BornRecord>())
            .sum();
        let row_est = self.table.len()
            * (core::mem::size_of::<JwwRow>()
                + core::mem::size_of::<bool>()
                + core::mem::size_of::<[f64; 4]>());
        self.source.stored_bytes()
            + self.base_records.capacity() * core::mem::size_of::<BaseRecord>()
            + patched
            + born
            + self.base_alive.resident_bytes()
            + self.born_alive.resident_bytes()
            + self.element_base_alive.resident_bytes()
            + self.element_born_alive.resident_bytes()
            + row_est
            + self.spatial.bytes()
            + self.display.bytes()
            + self.history.bytes()
    }

    fn effective_record(&self, slot: SlotRef) -> usize {
        match slot {
            SlotRef::Base(r) if r < self.top_end => self.base_alive.rank(r) as usize,
            SlotRef::Born(b) => {
                (self.base_alive.rank(self.top_end) as usize) + (self.born_alive.rank(b) as usize)
            }
            SlotRef::Base(r) => {
                (self.base_alive.rank(r) as usize) + (self.born_alive.count_ones() as usize)
            }
        }
    }

    fn slot_of_effective(&self, eff: usize) -> Option<SlotRef> {
        let alive_top = self.base_alive.rank(self.top_end) as usize;
        let alive_born = self.born_alive.count_ones() as usize;
        let total = (self.base_alive.count_ones() as usize) + alive_born;
        if eff >= total {
            return None;
        }
        if eff < alive_top {
            let k = u32::try_from(eff).ok()?;
            self.base_alive.select(k).map(SlotRef::Base)
        } else if eff < alive_top + alive_born {
            let k = u32::try_from(eff - alive_top).ok()?;
            self.born_alive.select(k).map(SlotRef::Born)
        } else {
            let k = u32::try_from(eff - alive_born).ok()?;
            self.base_alive.select(k).map(SlotRef::Base)
        }
    }

    fn canonical_id(&self, space: SpaceId, slot: SlotRef) -> String {
        format!("space/{}/r{}", space.0, self.effective_record(slot))
    }

    fn current_body_bytes(&self, slot: SlotRef) -> Option<Vec<u8>> {
        match slot {
            SlotRef::Base(r) => {
                if let Some(patched) = self.patched_bodies.get(&r) {
                    return Some(patched.clone());
                }
                let rec = self.base_records.get(r)?;
                let want = rec.body.len();
                let mut out = Vec::with_capacity(want);
                self.source.append_range(&mut out, rec.body.clone());
                (out.len() == want).then_some(out)
            }
            SlotRef::Born(b) => self.born_records.get(b).map(|rec| rec.body.clone()),
        }
    }

    fn slot_class_and_def(&self, slot: SlotRef) -> Option<(&[u8], Option<SpaceId>, bool)> {
        match slot {
            SlotRef::Base(r) => {
                let rec = self.base_records.get(r)?;
                if rec.kind != RecordKind::Entity {
                    return None;
                }
                let in_def = rec.block.is_some();
                let def_space = match &rec.outcome {
                    BaseOutcome::Element(rid) if in_def => {
                        self.table.row(*rid).map(|row| row.space)
                    }
                    _ => None,
                };
                Some((rec.kind_name.as_slice(), def_space, in_def))
            }
            SlotRef::Born(b) => {
                let rec = self.born_records.get(b)?;
                Some((rec.class_name, None, false))
            }
        }
    }

    /// 変更前のクラス一覧（出現順）を組み立てる
    fn snapshot_active_classes(&self) -> Vec<(Vec<u8>, u16)> {
        if !self.same_name_multi_schema {
            return self
                .class_buckets
                .iter()
                .filter(|b| b.alive_count > 0)
                .map(|b| (b.name.clone(), b.schema))
                .collect();
        }
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for (idx, rec) in self.base_records.iter().enumerate() {
            if idx >= self.top_end {
                break;
            }
            if self.base_alive.get(idx)
                && let Some(b_idx) = rec.class_bucket
                && seen.insert(b_idx)
                && let Some(bucket) = self.class_buckets.get(usize::from(b_idx))
            {
                out.push((bucket.name.clone(), bucket.schema));
            }
        }
        for (idx, born) in self.born_records.iter().enumerate() {
            if self.born_alive.get(idx)
                && seen.insert(born.class_bucket)
                && let Some(bucket) = self.class_buckets.get(usize::from(born.class_bucket))
            {
                out.push((bucket.name.clone(), bucket.schema));
            }
        }
        for (idx, rec) in self.base_records.iter().enumerate().skip(self.top_end) {
            if self.base_alive.get(idx)
                && let Some(b_idx) = rec.class_bucket
                && seen.insert(b_idx)
                && let Some(bucket) = self.class_buckets.get(usize::from(b_idx))
            {
                out.push((bucket.name.clone(), bucket.schema));
            }
        }
        out
    }

    fn resolve_or_create_bucket(&mut self, name: &[u8], schema: u16) -> Option<u16> {
        if let Some((idx, _)) = self
            .class_buckets
            .iter()
            .enumerate()
            .find(|(_, b)| b.name == name && b.schema == schema)
        {
            return u16::try_from(idx).ok();
        }
        let idx = u16::try_from(self.class_buckets.len()).ok()?;
        if self
            .class_buckets
            .iter()
            .any(|b| b.name == name && b.schema != schema)
        {
            self.same_name_multi_schema = true;
        }
        self.class_buckets.push(ClassBucket {
            name: name.to_vec(),
            schema,
            alive_count: 0,
        });
        Some(idx)
    }

    /// 図面モデルの変更 [`Changes`] を常駐状態へ反映し、各項目の結果と他要素の ID ずれの有無を返す
    pub fn apply(
        &mut self,
        changes: &Changes,
        cx: &mut Cx<'_>,
    ) -> Result<(Vec<Applied>, bool), Error> {
        let current = match changes.document {
            None => true,
            Some(expected_hash) => {
                let bytes = quietly(cx, |cx| self.save(cx))?;
                let same = Hash::document_checked(&bytes, cx).map_err(|stop| {
                    cx.report(stop.to_diagnostic());
                    Error::Stopped(stop)
                })? == expected_hash;
                if !same {
                    cx.report(Diagnostic::new(codes::CHANGE_DOCUMENT_STALE));
                }
                same
            }
        };
        if !self.header_valid {
            cx.report(unsupported("apply", "header"));
            return Err(Error::Rejected);
        }

        let active_classes = self.snapshot_active_classes();
        let mut seen_slots = BTreeSet::new();
        let mut staged: Vec<StagedEdit> = Vec::with_capacity(changes.edits.len());
        let mut removed_base = BTreeSet::new();
        let mut removed_born = BTreeSet::new();
        let mut changed_base = BTreeSet::new();
        let mut changed_born = BTreeSet::new();
        let mut added_count = 0usize;
        let mut rejected = false;

        for change in &changes.edits {
            work_live(cx, 1)?;
            let step = match change {
                Edit::Remove { target } => {
                    let Some((row_id, slot, eff_rec)) =
                        self.resolve_target(target, &mut seen_slots, "remove", cx)?
                    else {
                        rejected = true;
                        continue;
                    };
                    match self.stage_remove(row_id, slot) {
                        Some(action) => {
                            match slot {
                                SlotRef::Base(r) => {
                                    removed_base.insert(r);
                                }
                                SlotRef::Born(b) => {
                                    removed_born.insert(b);
                                }
                            }
                            Some(StagedEdit {
                                kind_label: "remove",
                                action,
                            })
                        }
                        None => {
                            cx.report(
                                unsupported("remove", &self.slot_kind_label(slot))
                                    .at(Ref::at_record(eff_rec as u64)),
                            );
                            None
                        }
                    }
                }
                Edit::Restyle { target, style } => {
                    let Some((row_id, slot, eff_rec)) =
                        self.resolve_target(target, &mut seen_slots, "restyle", cx)?
                    else {
                        rejected = true;
                        continue;
                    };
                    match self.stage_restyle(row_id, slot, style) {
                        Ok(Some(action)) => {
                            note_changed_slot(slot, &mut changed_base, &mut changed_born);
                            Some(StagedEdit {
                                kind_label: "restyle",
                                action,
                            })
                        }
                        Ok(None) => {
                            cx.report(
                                unsupported("restyle", &self.slot_kind_label(slot))
                                    .at(Ref::at_record(eff_rec as u64)),
                            );
                            None
                        }
                        Err(()) => {
                            cx.report(unsupported("restyle", "round_trip"));
                            return Err(Error::Rejected);
                        }
                    }
                }
                Edit::Replace { target, element } => {
                    let Some((row_id, slot, eff_rec)) =
                        self.resolve_target(target, &mut seen_slots, "replace", cx)?
                    else {
                        rejected = true;
                        continue;
                    };
                    match self.stage_replace(row_id, slot, element) {
                        Ok(Some(action)) => {
                            note_changed_slot(slot, &mut changed_base, &mut changed_born);
                            Some(StagedEdit {
                                kind_label: "replace",
                                action,
                            })
                        }
                        Ok(None) => {
                            cx.report(
                                unsupported("replace", &self.slot_kind_label(slot))
                                    .at(Ref::at_record(eff_rec as u64)),
                            );
                            None
                        }
                        Err(()) => {
                            cx.report(unsupported("replace", "round_trip"));
                            return Err(Error::Rejected);
                        }
                    }
                }
                Edit::Transform { target, transform } => {
                    let Some((row_id, slot, eff_rec)) =
                        self.resolve_target(target, &mut seen_slots, "transform", cx)?
                    else {
                        rejected = true;
                        continue;
                    };
                    match self.stage_transform(row_id, slot, transform) {
                        Ok(Some(action)) => {
                            note_changed_slot(slot, &mut changed_base, &mut changed_born);
                            Some(StagedEdit {
                                kind_label: "transform",
                                action,
                            })
                        }
                        Ok(None) => {
                            cx.report(
                                unsupported("transform", &self.slot_kind_label(slot))
                                    .at(Ref::at_record(eff_rec as u64)),
                            );
                            None
                        }
                        Err(()) => {
                            cx.report(unsupported("transform", "round_trip"));
                            return Err(Error::Rejected);
                        }
                    }
                }
                Edit::Add { place, element } => {
                    let missing_field = match element.as_ref() {
                        Placed::Model(el) => unset(el),
                        Placed::Paper(_) => None,
                    };
                    if let Some(field) = missing_field {
                        cx.report(
                            Diagnostic::new(codes::CHANGE_STYLE_UNSUPPORTED)
                                .with("field", field)
                                .with("format", FORMAT),
                        );
                        None
                    } else {
                        let encoded = match element.as_ref() {
                            Placed::Model(el) => self
                                .resolve_existing_group_space(place)
                                .filter(|_| !active_classes.is_empty())
                                .and_then(|space| {
                                    let (cls, body, layer, style, shape) =
                                        encode_added_entity(&self.header, space, el)?;
                                    Some((cls, body, layer, style, shape, space.0 as usize))
                                }),
                            Placed::Paper(_) => None,
                        };
                        match encoded {
                            Some((cls, body, layer, style, shape, group)) => {
                                let Some(schema) = resolve_schema_for_class(&active_classes, cls)
                                else {
                                    cx.report(unsupported("apply", "round_trip"));
                                    return Err(Error::Rejected);
                                };
                                let born_offset = added_count;
                                added_count += 1;
                                match self.verify_added_round_trip(
                                    cls,
                                    schema,
                                    born_offset,
                                    body,
                                    layer,
                                    style,
                                    &shape,
                                    group,
                                ) {
                                    Some(action) => Some(StagedEdit {
                                        kind_label: "add",
                                        action,
                                    }),
                                    None => {
                                        cx.report(unsupported("add", "round_trip"));
                                        return Err(Error::Rejected);
                                    }
                                }
                            }
                            None => {
                                cx.report(unsupported("add", kind_of(element)));
                                None
                            }
                        }
                    }
                }
                Edit::ReplaceLayer { .. } | Edit::AddLayer { .. } | Edit::RemoveLayer { .. } => {
                    cx.report(unsupported(change.name(), "layer"));
                    None
                }
                Edit::ReplaceTextStyle { .. }
                | Edit::AddTextStyle { .. }
                | Edit::RemoveTextStyle { .. } => {
                    cx.report(unsupported(change.name(), "text_style"));
                    None
                }
                Edit::ReplaceLineType { .. }
                | Edit::AddLineType { .. }
                | Edit::RemoveLineType { .. } => {
                    cx.report(unsupported(change.name(), "line_type"));
                    None
                }
                Edit::ReplaceDimensionStyle { .. }
                | Edit::AddDimensionStyle { .. }
                | Edit::RemoveDimensionStyle { .. } => {
                    cx.report(unsupported(change.name(), "dimension_style"));
                    None
                }
                Edit::SetPointStyle { .. } => {
                    cx.report(unsupported(change.name(), "point_style"));
                    None
                }
            };
            match step {
                Some(s) => staged.push(s),
                None => rejected = true,
            }
        }

        if rejected || !current {
            return Err(Error::Rejected);
        }

        let rebuild_needed =
            !removed_base.is_empty() || !removed_born.is_empty() || added_count > 0;
        if rebuild_needed {
            let Some(layout) = &self.list_layout else {
                cx.report(unsupported("apply", "round_trip"));
                return Err(Error::Rejected);
            };
            if !layout.standard_header_end
                && !self.has_surviving_cdata_anchor(
                    layout,
                    &removed_base,
                    &removed_born,
                    added_count,
                )
            {
                cx.report(unsupported("apply", "round_trip"));
                return Err(Error::Rejected);
            }
            if let Some(bad_source) =
                self.check_underlay_shift(&removed_base, &removed_born, added_count)
            {
                cx.report(unsupported("apply", "round_trip").at(bad_source));
                return Err(Error::Rejected);
            }
        }

        let shifted = if rebuild_needed {
            self.compute_shifted(
                &removed_base,
                &removed_born,
                &changed_base,
                &changed_born,
                added_count,
            )
        } else {
            false
        };

        // バケット番号を先に確定する
        for item in &mut staged {
            let _ = item.kind_label;
            if let StagedAction::Added {
                class_name,
                schema,
                class_bucket,
                ..
            } = &mut item.action
            {
                let Some(bucket) = self.resolve_or_create_bucket(class_name, *schema) else {
                    cx.report(unsupported("apply", "round_trip"));
                    return Err(Error::Rejected);
                };
                *class_bucket = bucket;
            }
        }

        let old_born_len = self.born_records.len();
        let mut post_results: Vec<Option<(SpaceId, SlotRef, Hash)>> =
            Vec::with_capacity(staged.len());

        for item in &staged {
            match &item.action {
                StagedAction::Removed { .. } => post_results.push(None),
                StagedAction::Updated {
                    after_row, slot, ..
                } => {
                    let h = Hash::element_checked(&after_row.element, cx).map_err(|stop| {
                        cx.report(stop.to_diagnostic());
                        Error::Stopped(stop)
                    })?;
                    post_results.push(Some((after_row.space, *slot, h)));
                }
                StagedAction::Added {
                    born_offset,
                    after_row,
                    ..
                } => {
                    let h = Hash::element_checked(&after_row.element, cx).map_err(|stop| {
                        cx.report(stop.to_diagnostic());
                        Error::Stopped(stop)
                    })?;
                    post_results.push(Some((
                        after_row.space,
                        SlotRef::Born(old_born_len + *born_offset),
                        h,
                    )));
                }
            }
        }

        let had_changes = !staged.is_empty();
        let side = SideIndexes {
            grid: Some(&mut self.spatial),
            dirty_rows: None,
            dep: None,
        };
        let txn_res = run_transaction(
            &mut self.table,
            side,
            &mut self.history,
            cx,
            |txn, _cx| -> Result<(), TxnError<()>> {
                let mut step_patch = JwwStepPatch::default();
                for item in staged {
                    match item.action {
                        StagedAction::Removed {
                            row_id,
                            slot,
                            class_bucket,
                            top_group,
                        } => {
                            txn.remove_row(row_id)?;
                            step_patch.removed.push(RemovedSlot {
                                slot,
                                class_bucket,
                                top_group,
                            });
                        }
                        StagedAction::Updated {
                            row_id,
                            after_row,
                            after_bounds,
                            body_diff,
                            ..
                        } => {
                            txn.update_row(row_id, after_row, after_bounds)?;
                            step_patch.bodies.push(body_diff);
                        }
                        StagedAction::Added {
                            born_offset,
                            class_name,
                            schema,
                            class_bucket,
                            body,
                            mut after_row,
                            after_bounds,
                            group,
                        } => {
                            let born_idx = old_born_len + born_offset;
                            after_row.slot = SlotRef::Born(born_idx);
                            let row_id = txn.insert_row(after_row, after_bounds)?;
                            step_patch.added.push(BornRecord {
                                class_name,
                                schema,
                                class_bucket,
                                body,
                                row_id,
                                group,
                            });
                        }
                    }
                }
                if !step_patch.bodies.is_empty()
                    || !step_patch.removed.is_empty()
                    || !step_patch.added.is_empty()
                {
                    txn.push_patch(step_patch);
                }
                Ok(())
            },
        );

        match txn_res {
            Ok(_) => {}
            Err(TxnError::Stop(stop)) => {
                cx.report(stop.to_diagnostic());
                return Err(Error::Stopped(stop));
            }
            Err(TxnError::Stage(_) | TxnError::Rejected(())) => {
                cx.report(unsupported("apply", "round_trip"));
                return Err(Error::Rejected);
            }
        }

        if had_changes && let Some(committed) = self.history.last_undo().cloned() {
            reapply_display_for_jww(
                &mut self.display,
                &self.header,
                self.paper_origin,
                &self.symbol_index_by_key,
                &committed,
            );
            for patch in committed.patches() {
                self.apply_backend_patch(patch);
            }
            self.refresh_modified_symbol_bounds(&committed);
            self.history.trim_to_bytes(self.history_max_bytes);
        }

        let mut applied = Vec::with_capacity(post_results.len());
        for entry in post_results {
            match entry {
                None => applied.push(Applied {
                    id: None,
                    hash: None,
                }),
                Some((space, slot, hash)) => applied.push(Applied {
                    id: Some(self.canonical_id(space, slot)),
                    hash: Some(hash),
                }),
            }
        }

        Ok((applied, shifted))
    }

    /// 直前の変更を取り消す。取り消したら `Ok(true)`、履歴が空なら `Ok(false)` を返す
    pub fn undo(&mut self, cx: &mut Cx<'_>) -> Result<bool, Error> {
        let side = SideIndexes {
            grid: Some(&mut self.spatial),
            dirty_rows: None,
            dep: None,
        };
        let committed = match self.history.undo(&mut self.table, side, cx) {
            Ok(Some(c)) => c.clone(),
            Ok(None) => return Ok(false),
            Err(stop) => {
                cx.report(stop.to_diagnostic());
                return Err(Error::Stopped(stop));
            }
        };
        revert_display_for_jww(
            &mut self.display,
            &self.header,
            self.paper_origin,
            &self.symbol_index_by_key,
            &committed,
        );
        let touched_defs = collect_touched_defs_on_undo(&committed);
        for patch in committed.patches().iter().rev() {
            self.undo_backend_patch(patch);
        }
        for def_idx in touched_defs {
            self.recompute_symbol_bound(def_idx);
        }
        Ok(true)
    }

    /// 直前に取り消した変更をやり直す。やり直したら `Ok(true)`、履歴が空なら `Ok(false)` を返す
    pub fn redo(&mut self, cx: &mut Cx<'_>) -> Result<bool, Error> {
        let side = SideIndexes {
            grid: Some(&mut self.spatial),
            dirty_rows: None,
            dep: None,
        };
        let committed = match self.history.redo(&mut self.table, side, cx) {
            Ok(Some(c)) => c.clone(),
            Ok(None) => return Ok(false),
            Err(stop) => {
                cx.report(stop.to_diagnostic());
                return Err(Error::Stopped(stop));
            }
        };
        reapply_display_for_jww(
            &mut self.display,
            &self.header,
            self.paper_origin,
            &self.symbol_index_by_key,
            &committed,
        );
        let touched_defs = collect_touched_defs_on_redo(&committed);
        for patch in committed.patches() {
            self.apply_backend_patch(patch);
        }
        for def_idx in touched_defs {
            self.recompute_symbol_bound(def_idx);
        }
        Ok(true)
    }

    /// 用紙座標系（mm）の矩形と交差する最上位要素の現在の正規 ID 一覧を返す
    pub fn select_sheet(&self, rect: [f64; 4], cx: &mut Cx<'_>) -> Result<Vec<String>, Error> {
        let mut raw = Vec::new();
        self.spatial
            .query_with_cx(self.table.bounds_column(), rect, &mut raw, cx)
            .map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Error::Stopped(stop)
            })?;
        let mut hits: Vec<((u8, usize), String)> = Vec::with_capacity(raw.len());
        for rid in raw {
            work_live(cx, 1)?;
            if let Some(row) = self.table.row(rid)
                && row.space.0 < DEFINITION_SPACES
            {
                hits.push((
                    file_order_key(row.slot, self.top_end),
                    self.canonical_id(row.space, row.slot),
                ));
            }
        }
        hits.sort_by_key(|(ord, _)| *ord);
        Ok(hits.into_iter().map(|(_, id)| id).collect())
    }

    /// 指定した空間の座標系の矩形と交差する要素の現在の正規 ID 一覧を返す
    pub fn select_in_space(
        &self,
        space: SpaceId,
        rect: [f64; 4],
        cx: &mut Cx<'_>,
    ) -> Result<Vec<String>, Error> {
        let [rx0, ry0, rx1, ry1] = rect;
        if !(rx0 <= rx1 && ry0 <= ry1) {
            return Ok(Vec::new());
        }
        if space.0 < DEFINITION_SPACES {
            let k = 1.0 / self.header.scale(space.0 as usize);
            let [ox, oy] = self.paper_origin;
            let paper_rect = [rx0 * k + ox, ry0 * k + oy, rx1 * k + ox, ry1 * k + oy];
            let mut raw = Vec::new();
            self.spatial
                .query_with_cx(self.table.bounds_column(), paper_rect, &mut raw, cx)
                .map_err(|stop| {
                    cx.report(stop.to_diagnostic());
                    Error::Stopped(stop)
                })?;
            let mut hits: Vec<((u8, usize), String)> = Vec::new();
            for rid in raw {
                work_live(cx, 1)?;
                if let Some(row) = self.table.row(rid)
                    && row.space == space
                {
                    hits.push((
                        file_order_key(row.slot, self.top_end),
                        self.canonical_id(row.space, row.slot),
                    ));
                }
            }
            hits.sort_by_key(|(ord, _)| *ord);
            return Ok(hits.into_iter().map(|(_, id)| id).collect());
        }

        let def_idx = (space.0 - DEFINITION_SPACES) as usize;
        let Some(rows) = self.definition_rows.get(def_idx) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for &rid in rows {
            work_live(cx, 1)?;
            if let Some(row) = self.table.row(rid) {
                let b = model_element_bounds(&row.element.shape);
                if is_finite_box(b) && boxes_intersect(b, rect) {
                    out.push(self.canonical_id(row.space, row.slot));
                }
            }
        }
        Ok(out)
    }

    /// 常駐状態から現在の図面モデル [`Drawing2D`] を組み立てる（保存して読み直した結果と完全一致する）
    pub fn drawing(&self, cx: &mut Cx<'_>) -> Result<Drawing2D, Error> {
        if !self.header_valid {
            cx.report(Diagnostic::new(codes::JWW_HEADER_INVALID).at(Ref::at_record(0)));
        }
        let mut out = Drawing2D {
            layers: build_layers(&self.header),
            ..Drawing2D::default()
        };
        let mut drafts = build_edit_state_drafts(&self.header);
        let mut seen_line_types: BTreeSet<u8> = BTreeSet::new();
        let mut groups: BTreeMap<usize, Vec<Element<ModelPoint>>> = BTreeMap::new();
        let version_text = self.header.version.to_string();
        let mut eff_rec = 0usize;

        for (idx, rec) in self.base_records.iter().enumerate() {
            if idx >= self.top_end {
                break;
            }
            if !self.base_alive.get(idx) {
                continue;
            }
            self.emit_slot_to_drawing(
                SlotRef::Base(idx),
                Some(rec),
                eff_rec,
                &version_text,
                &mut seen_line_types,
                &mut groups,
                &mut drafts,
                &mut out,
                cx,
            )?;
            eff_rec += 1;
        }

        for (idx, born) in self.born_records.iter().enumerate() {
            if !self.born_alive.get(idx) {
                continue;
            }
            let source = Ref::at_record(eff_rec as u64);
            cx.check(Kind::Work, 1).map_err(|stop| {
                cx.report(stop.to_diagnostic().at(source.clone()));
                Error::Stopped(stop)
            })?;
            if let Some(row) = self.table.row(born.row_id) {
                push_row_to_drawing(
                    row,
                    source,
                    &version_text,
                    &self.header,
                    self,
                    &mut seen_line_types,
                    &mut groups,
                    &mut drafts,
                    &mut out,
                );
            }
            eff_rec += 1;
        }

        for (idx, rec) in self.base_records.iter().enumerate().skip(self.top_end) {
            if !self.base_alive.get(idx) {
                continue;
            }
            self.emit_slot_to_drawing(
                SlotRef::Base(idx),
                Some(rec),
                eff_rec,
                &version_text,
                &mut seen_line_types,
                &mut groups,
                &mut drafts,
                &mut out,
                cx,
            )?;
            eff_rec += 1;
        }

        let size = PAPERS
            .iter()
            .find(|(code, _)| *code == self.header.paper)
            .map(|(_, size)| *size);
        let [ox, oy] = self.paper_origin;
        let mut sheet = Sheet {
            id: SHEET,
            name: None,
            size: size.map(|[w, h]| [PaperLength(w), PaperLength(h)]),
            elements: Vec::new(),
            views: Vec::new(),
            order: Vec::new(),
            source: Ref::default(),
        };
        for (group, elements) in groups {
            let id = SpaceId(group as u32);
            let k = 1.0 / self.header.scale(group);
            sheet.order.push(SheetItem::View(sheet.views.len()));
            sheet.views.push(View {
                space: id,
                sheet: SHEET,
                transform: Affine {
                    m: [k, 0.0, ox, 0.0, k, oy],
                },
                clip: Vec::new(),
                hidden_layers: Vec::new(),
                source: Ref::at_record(0),
            });
            out.spaces.push(ModelSpace {
                id,
                unit: Unit::Millimeter,
                axes: Axes::EastNorth,
                order: (0..elements.len()).collect(),
                elements,
            });
        }
        out.sheets.push(sheet);
        out.settle_extensions(drafts, cx).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        Ok(out)
    }

    /// 現在の常駐状態を [`JwwFile`] として取り出す
    pub fn snapshot_file(&self, cx: &mut Cx<'_>) -> Result<JwwFile, Error> {
        let bytes = self.save(cx)?;
        quietly(cx, |cx| Jww::read(&bytes, cx))
    }

    /// 全体を書く保存（現行実装で変更を順に `apply` して書き出したバイト列と完全一致する）
    pub fn save(&self, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
        let src_len = self.source.len();
        work_live(cx, (src_len as u64).div_ceil(1024))?;

        if self.lists_rebuilt_steps == 0 {
            let mut out = Vec::with_capacity(src_len);
            self.source.append_range(&mut out, 0..src_len);
            if out.len() != src_len {
                cx.report(unsupported("save", "round_trip"));
                return Err(Error::Rejected);
            }
            for (&rec_idx, body) in &self.patched_bodies {
                work_live(cx, 1)?;
                let Some(rec) = self.base_records.get(rec_idx) else {
                    cx.report(unsupported("save", "round_trip"));
                    return Err(Error::Rejected);
                };
                let Some(dst) = out.get_mut(rec.body.start..rec.body.start + body.len()) else {
                    cx.report(unsupported("save", "round_trip"));
                    return Err(Error::Rejected);
                };
                dst.copy_from_slice(body);
            }
            return Ok(out);
        }

        let Some(layout) = &self.list_layout else {
            cx.report(unsupported("save", "round_trip"));
            return Err(Error::Rejected);
        };
        let added_bytes = self
            .born_records
            .iter()
            .enumerate()
            .filter(|(idx, _)| self.born_alive.get(*idx))
            .fold(0u64, |acc, (_, b)| acc.saturating_add(b.body.len() as u64));
        work_live(
            cx,
            (src_len as u64).saturating_add(added_bytes).div_ceil(1024),
        )?;

        let mut out = Vec::with_capacity(src_len + added_bytes as usize);
        if !append_exact_range(&self.source, 0..layout.header_end, &mut out) {
            cx.report(unsupported("save", "round_trip"));
            return Err(Error::Rejected);
        }

        let mut numbers: BTreeMap<(Vec<u8>, u16), u32> = BTreeMap::new();
        let mut next = 1u32;

        let kept_top = (self.base_alive.rank(layout.top_end) - self.base_alive.rank(1)) as usize;
        let alive_born = self.born_alive.count_ones() as usize;
        write_count(&mut out, kept_top + alive_born);

        for rec_idx in 1..layout.top_end {
            if !self.base_alive.get(rec_idx) {
                continue;
            }
            work_live(cx, 1)?;
            if !self.write_base_object(rec_idx, None, &mut out, &mut numbers, &mut next) {
                cx.report(unsupported("save", "round_trip"));
                return Err(Error::Rejected);
            }
        }

        for (born_idx, born) in self.born_records.iter().enumerate() {
            if !self.born_alive.get(born_idx) {
                continue;
            }
            work_live(cx, 1)?;
            write_object_tag(
                &mut out,
                &mut numbers,
                &mut next,
                born.class_name,
                born.schema,
            );
            out.extend_from_slice(&born.body);
        }

        write_count(&mut out, layout.blocks.len());
        for block in &layout.blocks {
            if !self.write_base_object(
                block.def_record,
                Some(block.head_end),
                &mut out,
                &mut numbers,
                &mut next,
            ) {
                cx.report(unsupported("save", "round_trip"));
                return Err(Error::Rejected);
            }
            let kept_inner = (self.base_alive.rank(block.inner_end)
                - self.base_alive.rank(block.inner_start)) as usize;
            write_count(&mut out, kept_inner);
            for rec_idx in block.inner_start..block.inner_end {
                if !self.base_alive.get(rec_idx) {
                    continue;
                }
                work_live(cx, 1)?;
                if !self.write_base_object(rec_idx, None, &mut out, &mut numbers, &mut next) {
                    cx.report(unsupported("save", "round_trip"));
                    return Err(Error::Rejected);
                }
            }
        }

        let _ = layout.after_blocks_record;
        if layout.tail_byte_start <= src_len
            && !append_exact_range(&self.source, layout.tail_byte_start..src_len, &mut out)
        {
            cx.report(unsupported("save", "round_trip"));
            return Err(Error::Rejected);
        }

        Ok(out)
    }

    fn write_base_object(
        &self,
        rec_idx: usize,
        until_override: Option<usize>,
        out: &mut Vec<u8>,
        numbers: &mut BTreeMap<(Vec<u8>, u16), u32>,
        next: &mut u32,
    ) -> bool {
        let Some(rec) = self.base_records.get(rec_idx) else {
            return false;
        };
        match rec.kind {
            RecordKind::Null => {
                out.extend([0, 0]);
                true
            }
            RecordKind::Entity | RecordKind::Block => {
                let Some(b_idx) = rec.class_bucket else {
                    return false;
                };
                let Some(bucket) = self.class_buckets.get(usize::from(b_idx)) else {
                    return false;
                };
                write_object_tag(out, numbers, next, &bucket.name, bucket.schema);
                let end = until_override.unwrap_or(rec.span.end);
                if end < rec.body.start {
                    return false;
                }
                let want_len = end - rec.body.start;
                if let Some(patched) = self.patched_bodies.get(&rec_idx) {
                    let Some(slice) = patched.get(..want_len) else {
                        return false;
                    };
                    out.extend_from_slice(slice);
                    true
                } else {
                    append_exact_range(&self.source, rec.body.start..end, out)
                }
            }
            _ => false,
        }
    }

    #[expect(clippy::too_many_arguments)]
    fn emit_slot_to_drawing(
        &self,
        slot: SlotRef,
        rec: Option<&BaseRecord>,
        eff_rec: usize,
        version_text: &str,
        seen_line_types: &mut BTreeSet<u8>,
        groups: &mut BTreeMap<usize, Vec<Element<ModelPoint>>>,
        drafts: &mut Vec<Draft>,
        out: &mut Drawing2D,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        let source = Ref::at_record(eff_rec as u64);
        cx.check(Kind::Work, 1).map_err(|stop| {
            cx.report(stop.to_diagnostic().at(source.clone()));
            Error::Stopped(stop)
        })?;
        let Some(rec) = rec else {
            return Ok(());
        };
        match &rec.outcome {
            BaseOutcome::None => {}
            BaseOutcome::Dropped => dropped(cx, &rec.kind_name, source),
            BaseOutcome::Invalid => invalid(cx, &rec.kind_name, source),
            BaseOutcome::BlockDef {
                symbol_index,
                number,
                referenced,
                name,
            } => {
                let space = SpaceId(DEFINITION_SPACES + *symbol_index as u32);
                out.symbols.push(SymbolDefinition {
                    key: block_key(*number),
                    space: ModelSpace {
                        id: space,
                        unit: Unit::Millimeter,
                        axes: Axes::EastNorth,
                        elements: Vec::new(),
                        order: Vec::new(),
                    },
                    base: Some(ModelPoint {
                        space,
                        x: 0.0,
                        y: 0.0,
                    }),
                    attributes: Vec::new(),
                    anonymous: false,
                    external: None,
                    source: source.clone(),
                });
                drafts.push(Draft {
                    namespace: "jww.block".into(),
                    name: number.to_string(),
                    version: Some(version_text.to_owned()),
                    schema: SCHEMA,
                    at: Position::Document,
                    fields: vec![
                        Field {
                            key: "name".into(),
                            code: None,
                            value: FieldValue::Text(name.clone()),
                        },
                        Field {
                            key: "referenced".into(),
                            code: None,
                            value: FieldValue::Integer(i64::from(*referenced)),
                        },
                    ],
                    source,
                });
            }
            BaseOutcome::DocumentDraft {
                namespace,
                name,
                fields,
            } => {
                drafts.push(Draft {
                    namespace: (*namespace).into(),
                    name: name.clone(),
                    version: Some(version_text.to_owned()),
                    schema: SCHEMA,
                    at: Position::Document,
                    fields: fields.clone(),
                    source,
                });
            }
            BaseOutcome::Element(rid) => {
                let _ = slot;
                if let Some(row) = self.table.row(*rid) {
                    push_row_to_drawing(
                        row,
                        source,
                        version_text,
                        &self.header,
                        self,
                        seen_line_types,
                        groups,
                        drafts,
                        out,
                    );
                }
            }
        }
        Ok(())
    }

    fn resolve_target(
        &self,
        seen: &Seen,
        seen_slots: &mut BTreeSet<SlotRef>,
        edit: &str,
        cx: &mut Cx<'_>,
    ) -> Result<Option<(RowId, SlotRef, usize)>, Error> {
        let found = parse_jww_element_id(&seen.id).and_then(|(expected_space, eff_rec)| {
            let slot = self.slot_of_effective(eff_rec)?;
            let row_id = match slot {
                SlotRef::Base(r) => match &self.base_records.get(r)?.outcome {
                    BaseOutcome::Element(rid) => *rid,
                    _ => return None,
                },
                SlotRef::Born(b) => self.born_records.get(b)?.row_id,
            };
            let row = self.table.row(row_id)?;
            (row.space == expected_space).then_some((row_id, row, slot, eff_rec))
        });

        let Some((row_id, row, slot, eff_rec)) = found else {
            cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", seen.id.as_str()));
            return Ok(None);
        };
        let actual_hash = Hash::element_checked(&row.element, cx).map_err(|stop| {
            cx.report(stop.to_diagnostic().at(Ref::at_record(eff_rec as u64)));
            Error::Stopped(stop)
        })?;
        if actual_hash != seen.hash {
            cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_STALE).with("id", seen.id.as_str()));
            return Ok(None);
        }
        if !seen_slots.insert(slot) {
            cx.report(unsupported(edit, "shared").at(Ref::at_record(eff_rec as u64)));
            return Ok(None);
        }
        Ok(Some((row_id, slot, eff_rec)))
    }

    fn slot_kind_label(&self, slot: SlotRef) -> String {
        match slot {
            SlotRef::Base(r) => self
                .base_records
                .get(r)
                .map(|rec| String::from_utf8_lossy(&rec.kind_name).into_owned())
                .unwrap_or_default(),
            SlotRef::Born(b) => self
                .born_records
                .get(b)
                .map(|rec| String::from_utf8_lossy(rec.class_name).into_owned())
                .unwrap_or_default(),
        }
    }

    fn resolve_existing_group_space(&self, place: &str) -> Option<SpaceId> {
        let num_str = place.strip_prefix("space/")?;
        let g: usize = num_str.parse().ok()?;
        if g >= 16 || g.to_string() != num_str {
            return None;
        }
        (*self.group_counts.get(g)? > 0).then_some(SpaceId(g as u32))
    }

    fn stage_remove(&self, row_id: RowId, slot: SlotRef) -> Option<StagedAction> {
        let row = self.table.row(row_id)?;
        let (class_bucket, is_entity) = match slot {
            SlotRef::Base(r) => {
                let rec = self.base_records.get(r)?;
                (rec.class_bucket, rec.kind == RecordKind::Entity)
            }
            SlotRef::Born(b) => {
                let rec = self.born_records.get(b)?;
                (Some(rec.class_bucket), true)
            }
        };
        if !is_entity {
            return None;
        }
        let top_group = (row.space.0 < DEFINITION_SPACES).then_some(row.space.0 as usize);
        Some(StagedAction::Removed {
            row_id,
            slot,
            class_bucket,
            top_group,
        })
    }

    fn stage_restyle(
        &self,
        row_id: RowId,
        slot: SlotRef,
        restyle: &zumen_drawing::Restyle,
    ) -> Result<Option<StagedAction>, ()> {
        let Some(row) = self.table.row(row_id) else {
            return Ok(None);
        };
        let Some((class_name_raw, def_space, in_def)) = self.slot_class_and_def(slot) else {
            return Ok(None);
        };
        let Some(class) = [LINE, ARC, POINT]
            .into_iter()
            .find(|c| *c == class_name_raw)
        else {
            return Ok(None);
        };
        let Some(mut body) = self.current_body_bytes(slot) else {
            return Ok(None);
        };
        let Some(was) = RawBase::read(&body, 0, self.version) else {
            return Ok(None);
        };
        let (want_layer, want_style) = restyle.over(row.element.layer, &row.element.style);
        let Some(new_base) = base_for_style(
            &self.header,
            was,
            &row.element,
            want_layer,
            &want_style,
            in_def,
        ) else {
            return Ok(None);
        };
        if class == POINT
            && (new_base.pen_style, new_base.pen_width) != (was.pen_style, was.pen_width)
        {
            return Ok(None);
        }
        let patch_bytes = new_base.bytes(self.version);
        let Some(dst) = body.get_mut(..patch_bytes.len()) else {
            return Err(());
        };
        dst.copy_from_slice(&patch_bytes);
        let expected_layer = new_base.layer();
        let expected_style = style_of_base(&self.header, &new_base, class);
        self.finish_updated_stage(
            row_id,
            slot,
            row,
            class,
            def_space,
            body,
            expected_layer,
            expected_style,
            &row.element.shape,
        )
        .map(Some)
    }

    fn stage_replace(
        &self,
        row_id: RowId,
        slot: SlotRef,
        placed: &Placed,
    ) -> Result<Option<StagedAction>, ()> {
        let Placed::Model(target_el) = placed else {
            return Ok(None);
        };
        let Some(row) = self.table.row(row_id) else {
            return Ok(None);
        };
        let Some((class_name_raw, def_space, in_def)) = self.slot_class_and_def(slot) else {
            return Ok(None);
        };
        let Some(class) = [LINE, ARC, POINT]
            .into_iter()
            .find(|c| *c == class_name_raw)
        else {
            return Ok(None);
        };
        if class_of(&target_el.shape) != Some(class) {
            return Ok(None);
        }
        let Some(mut body) = self.current_body_bytes(slot) else {
            return Ok(None);
        };
        let Some(was) = RawBase::read(&body, 0, self.version) else {
            return Ok(None);
        };
        let Some(new_base) = base_for_style(
            &self.header,
            was,
            &row.element,
            target_el.layer,
            &target_el.style,
            in_def,
        ) else {
            return Ok(None);
        };
        if class == POINT
            && (new_base.pen_style, new_base.pen_width) != (was.pen_style, was.pen_width)
        {
            return Ok(None);
        }
        let Some(space) = space_of(&row.element.shape) else {
            return Ok(None);
        };
        let Some(geom_bytes) = shape_bytes(
            &target_el.shape,
            space,
            scale_of_group(&self.header, was.group, in_def),
            was.pen_style == POINT_SYMBOL,
        ) else {
            return Ok(None);
        };
        let base_bytes = new_base.bytes(self.version);
        let base_len = base_bytes.len();
        let Some(dst_base) = body.get_mut(..base_len) else {
            return Err(());
        };
        dst_base.copy_from_slice(&base_bytes);
        let Some(dst_geom) = body.get_mut(base_len..base_len + geom_bytes.len()) else {
            return Err(());
        };
        dst_geom.copy_from_slice(&geom_bytes);
        let expected_layer = new_base.layer();
        let expected_style = style_of_base(&self.header, &new_base, class);
        self.finish_updated_stage(
            row_id,
            slot,
            row,
            class,
            def_space,
            body,
            expected_layer,
            expected_style,
            &target_el.shape,
        )
        .map(Some)
    }

    fn stage_transform(
        &self,
        row_id: RowId,
        slot: SlotRef,
        xform: &Affine,
    ) -> Result<Option<StagedAction>, ()> {
        let Some(row) = self.table.row(row_id) else {
            return Ok(None);
        };
        let Some((class_name_raw, def_space, in_def)) = self.slot_class_and_def(slot) else {
            return Ok(None);
        };
        let Some(transformed) = transform::transform(&row.element.shape, xform).map(unturned)
        else {
            return Ok(None);
        };
        if class_of(&transformed) != Some(class_name_raw) {
            return Ok(None);
        }
        let Some(mut body) = self.current_body_bytes(slot) else {
            return Ok(None);
        };
        let Some(was) = RawBase::read(&body, 0, self.version) else {
            return Ok(None);
        };
        let Some(space) = space_of(&row.element.shape) else {
            return Ok(None);
        };
        let Some(geom_bytes) = shape_bytes(
            &transformed,
            space,
            scale_of_group(&self.header, was.group, in_def),
            was.pen_style == POINT_SYMBOL,
        ) else {
            return Ok(None);
        };
        let base_len = was.bytes(self.version).len();
        let Some(dst_geom) = body.get_mut(base_len..base_len + geom_bytes.len()) else {
            return Err(());
        };
        dst_geom.copy_from_slice(&geom_bytes);
        self.finish_updated_stage(
            row_id,
            slot,
            row,
            class_name_raw,
            def_space,
            body,
            row.element.layer,
            row.element.style,
            &transformed,
        )
        .map(Some)
    }

    #[expect(clippy::too_many_arguments)]
    fn finish_updated_stage(
        &self,
        row_id: RowId,
        slot: SlotRef,
        before_row: &JwwRow,
        class_name: &[u8],
        def_space: Option<SpaceId>,
        after_body: Vec<u8>,
        expected_layer: LayerId,
        expected_style: Style,
        expected_shape: &Shape<ModelPoint>,
    ) -> Result<StagedAction, ()> {
        let dummy_source = Ref::at_record(0);
        let SingleEntity::Element {
            element: lifted_el,
            own,
            embedded_image,
        } = lift_single_entity(
            &after_body,
            class_name,
            &self.header,
            def_space,
            &self.images,
            &dummy_source,
        )
        else {
            return Err(());
        };
        if lifted_el.layer != expected_layer
            || lifted_el.style != expected_style
            || !near(&lifted_el.shape, expected_shape)
        {
            return Err(());
        }
        let after_space =
            def_space.unwrap_or_else(|| space_of(&lifted_el.shape).unwrap_or(before_row.space));
        let after_bounds = compute_sheet_bounds(
            &lifted_el,
            after_space,
            &self.header,
            self.paper_origin,
            &self.symbol_index_by_key,
            &self.symbol_bounds,
        );
        let after_row = JwwRow {
            slot,
            space: after_space,
            element: lifted_el,
            own_extensions: own,
            underlay_image_record: embedded_image,
        };
        let body_diff = match slot {
            SlotRef::Base(r) => BodyDiff::Base {
                record: r,
                before: self.patched_bodies.get(&r).cloned(),
                after: after_body,
            },
            SlotRef::Born(b) => BodyDiff::Born {
                index: b,
                before: self
                    .born_records
                    .get(b)
                    .map(|rec| rec.body.clone())
                    .unwrap_or_default(),
                after: after_body,
            },
        };
        Ok(StagedAction::Updated {
            row_id,
            slot,
            after_row,
            after_bounds,
            body_diff,
        })
    }

    #[expect(clippy::too_many_arguments)]
    fn verify_added_round_trip(
        &self,
        class_name: &'static [u8],
        schema: u16,
        born_offset: usize,
        body: Vec<u8>,
        expected_layer: LayerId,
        expected_style: Style,
        expected_shape: &Shape<ModelPoint>,
        group: usize,
    ) -> Option<StagedAction> {
        let dummy_source = Ref::at_record(0);
        let SingleEntity::Element {
            element: lifted_el,
            own,
            embedded_image,
        } = lift_single_entity(
            &body,
            class_name,
            &self.header,
            None,
            &self.images,
            &dummy_source,
        )
        else {
            return None;
        };
        if lifted_el.layer != expected_layer
            || lifted_el.style != expected_style
            || !near(&lifted_el.shape, expected_shape)
        {
            return None;
        }
        let space = SpaceId(group as u32);
        let after_bounds = compute_sheet_bounds(
            &lifted_el,
            space,
            &self.header,
            self.paper_origin,
            &self.symbol_index_by_key,
            &self.symbol_bounds,
        );
        let after_row = JwwRow {
            slot: SlotRef::Born(0),
            space,
            element: lifted_el,
            own_extensions: own,
            underlay_image_record: embedded_image,
        };
        Some(StagedAction::Added {
            born_offset,
            class_name,
            schema,
            class_bucket: 0,
            body,
            after_row,
            after_bounds,
            group,
        })
    }

    fn has_surviving_cdata_anchor(
        &self,
        layout: &ListLayout,
        removed_base: &BTreeSet<usize>,
        removed_born: &BTreeSet<usize>,
        added_len: usize,
    ) -> bool {
        let removed_top = removed_base.range(1..layout.top_end).count();
        let kept_top =
            (self.base_alive.rank(layout.top_end) - self.base_alive.rank(1)) as usize - removed_top;
        let kept_born = self.born_alive.count_ones() as usize - removed_born.len();
        if kept_top + kept_born + added_len > 0 {
            let has_non_null_top = (1..layout.top_end).any(|r| {
                self.base_alive.get(r)
                    && !removed_base.contains(&r)
                    && self
                        .base_records
                        .get(r)
                        .is_some_and(|rec| rec.kind == RecordKind::Entity)
            });
            if has_non_null_top || kept_born + added_len > 0 {
                return true;
            }
        }
        layout.blocks.first().is_some_and(|b| {
            self.base_records
                .get(b.def_record)
                .is_some_and(|rec| rec.kind == RecordKind::Block)
        })
    }

    fn check_underlay_shift(
        &self,
        removed_base: &BTreeSet<usize>,
        removed_born: &BTreeSet<usize>,
        added_len: usize,
    ) -> Option<Ref> {
        if self.underlay_rows.is_empty() {
            return None;
        }
        for &rid in &self.underlay_rows {
            let Some(row) = self.table.row(rid) else {
                continue;
            };
            if let SlotRef::Base(r) = row.slot
                && removed_base.contains(&r)
            {
                continue;
            }
            if let SlotRef::Born(b) = row.slot
                && removed_born.contains(&b)
            {
                continue;
            }
            let target_slot = match row.underlay_image_record {
                Some(img_rec) => SlotRef::Base(img_rec),
                None => row.slot,
            };
            let delta = self.slot_shift_delta(target_slot, removed_base, removed_born, added_len);
            if delta != 0 {
                let was_eff = self.effective_record(row.slot);
                return Some(Ref::at_record(was_eff as u64));
            }
        }
        None
    }

    fn slot_shift_delta(
        &self,
        slot: SlotRef,
        removed_base: &BTreeSet<usize>,
        removed_born: &BTreeSet<usize>,
        added_len: usize,
    ) -> isize {
        match slot {
            SlotRef::Base(r) if r < self.top_end => -(removed_base.range(..r).count() as isize),
            SlotRef::Born(b) => {
                let rem_top = removed_base.range(..self.top_end).count() as isize;
                let rem_born = removed_born.range(..b).count() as isize;
                -(rem_top + rem_born)
            }
            SlotRef::Base(r) => {
                let rem_base = removed_base.range(..r).count() as isize;
                let rem_born = removed_born.len() as isize;
                (added_len as isize) - rem_base - rem_born
            }
        }
    }

    /// 変更しなかった生存要素のうち、実効レコード番号（ID）がずれたものが 1 件でもあるかを `O(edits)` で判定する
    fn compute_shifted(
        &self,
        removed_base: &BTreeSet<usize>,
        removed_born: &BTreeSet<usize>,
        changed_base: &BTreeSet<usize>,
        changed_born: &BTreeSet<usize>,
        added_len: usize,
    ) -> bool {
        let base_len = self.base_records.len();
        let mut splits = BTreeSet::new();
        splits.insert(0);
        splits.insert(self.top_end.min(base_len));
        splits.insert(base_len);
        for &r in removed_base {
            splits.insert(r.min(base_len));
            splits.insert((r + 1).min(base_len));
        }
        let pts: Vec<usize> = splits.into_iter().collect();
        for pair in pts.windows(2) {
            let Some(&[a, b]) = pair.first_chunk::<2>() else {
                continue;
            };
            if a >= b || removed_base.contains(&a) {
                continue;
            }
            let delta =
                self.slot_shift_delta(SlotRef::Base(a), removed_base, removed_born, added_len);
            if delta != 0 {
                let alive =
                    (self.element_base_alive.rank(b) - self.element_base_alive.rank(a)) as usize;
                let changed = changed_base.range(a..b).count();
                if alive > changed {
                    return true;
                }
            }
        }

        let born_len = self.born_records.len();
        if born_len > 0 {
            let mut b_splits = BTreeSet::new();
            b_splits.insert(0);
            b_splits.insert(born_len);
            for &b in removed_born {
                b_splits.insert(b.min(born_len));
                b_splits.insert((b + 1).min(born_len));
            }
            let b_pts: Vec<usize> = b_splits.into_iter().collect();
            for pair in b_pts.windows(2) {
                let Some(&[a, b]) = pair.first_chunk::<2>() else {
                    continue;
                };
                if a >= b || removed_born.contains(&a) {
                    continue;
                }
                let delta =
                    self.slot_shift_delta(SlotRef::Born(a), removed_base, removed_born, added_len);
                if delta != 0 {
                    let alive = (self.element_born_alive.rank(b) - self.element_born_alive.rank(a))
                        as usize;
                    let changed = changed_born.range(a..b).count();
                    if alive > changed {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn apply_backend_patch(&mut self, patch: &JwwStepPatch) {
        for diff in &patch.bodies {
            match diff {
                BodyDiff::Base { record, after, .. } => {
                    self.patched_bodies.insert(*record, after.clone());
                }
                BodyDiff::Born { index, after, .. } => {
                    if let Some(born) = self.born_records.get_mut(*index) {
                        born.body.clone_from(after);
                    }
                }
            }
        }
        for rem in &patch.removed {
            match rem.slot {
                SlotRef::Base(r) => {
                    self.base_alive.set(r, false);
                    self.element_base_alive.set(r, false);
                }
                SlotRef::Born(b) => {
                    self.born_alive.set(b, false);
                    self.element_born_alive.set(b, false);
                }
            }
            if let Some(b_idx) = rem.class_bucket
                && let Some(bucket) = self.class_buckets.get_mut(usize::from(b_idx))
            {
                bucket.alive_count = bucket.alive_count.saturating_sub(1);
            }
            if let Some(g) = rem.top_group
                && let Some(cnt) = self.group_counts.get_mut(g)
            {
                *cnt = cnt.saturating_sub(1);
            }
        }
        for born in &patch.added {
            self.born_records.push(born.clone());
            self.born_alive.push(true);
            self.element_born_alive.push(true);
            if let Some(bucket) = self.class_buckets.get_mut(usize::from(born.class_bucket)) {
                bucket.alive_count = bucket.alive_count.saturating_add(1);
            }
            if let Some(cnt) = self.group_counts.get_mut(born.group) {
                *cnt = cnt.saturating_add(1);
            }
        }
        if !patch.removed.is_empty() || !patch.added.is_empty() {
            self.lists_rebuilt_steps = self.lists_rebuilt_steps.saturating_add(1);
        }
    }

    fn undo_backend_patch(&mut self, patch: &JwwStepPatch) {
        if !patch.removed.is_empty() || !patch.added.is_empty() {
            self.lists_rebuilt_steps = self.lists_rebuilt_steps.saturating_sub(1);
        }
        for born in patch.added.iter().rev() {
            if let Some(cnt) = self.group_counts.get_mut(born.group) {
                *cnt = cnt.saturating_sub(1);
            }
            if let Some(bucket) = self.class_buckets.get_mut(usize::from(born.class_bucket)) {
                bucket.alive_count = bucket.alive_count.saturating_sub(1);
            }
            self.element_born_alive.pop();
            self.born_alive.pop();
            self.born_records.pop();
        }
        for rem in patch.removed.iter().rev() {
            match rem.slot {
                SlotRef::Base(r) => {
                    self.base_alive.set(r, true);
                    self.element_base_alive.set(r, true);
                }
                SlotRef::Born(b) => {
                    self.born_alive.set(b, true);
                    self.element_born_alive.set(b, true);
                }
            }
            if let Some(b_idx) = rem.class_bucket
                && let Some(bucket) = self.class_buckets.get_mut(usize::from(b_idx))
            {
                bucket.alive_count = bucket.alive_count.saturating_add(1);
            }
            if let Some(g) = rem.top_group
                && let Some(cnt) = self.group_counts.get_mut(g)
            {
                *cnt = cnt.saturating_add(1);
            }
        }
        for diff in patch.bodies.iter().rev() {
            match diff {
                BodyDiff::Base { record, before, .. } => match before {
                    Some(prev) => {
                        self.patched_bodies.insert(*record, prev.clone());
                    }
                    None => {
                        self.patched_bodies.remove(record);
                    }
                },
                BodyDiff::Born { index, before, .. } => {
                    if let Some(born) = self.born_records.get_mut(*index) {
                        born.body.clone_from(before);
                    }
                }
            }
        }
    }

    fn refresh_modified_symbol_bounds(&mut self, committed: &CommittedTxn<JwwRow, JwwStepPatch>) {
        let mut touched = BTreeSet::new();
        for diff in committed.existing() {
            let space = match diff {
                ExistingRowDiff::Updated { after_row, .. } => after_row.space,
                ExistingRowDiff::Removed { before_row, .. } => before_row.space,
            };
            if let Some(def_idx) = space.0.checked_sub(DEFINITION_SPACES) {
                touched.insert(def_idx as usize);
            }
        }
        for def_idx in touched {
            self.recompute_symbol_bound(def_idx);
        }
    }

    fn recompute_symbol_bound(&mut self, def_idx: usize) {
        let Some(rows) = self.definition_rows.get(def_idx) else {
            return;
        };
        let mut acc: Option<[f64; 4]> = None;
        for &rid in rows {
            if let Some(row) = self.table.row(rid) {
                let b = model_element_bounds(&row.element.shape);
                if is_finite_box(b) {
                    acc = Some(match acc {
                        Some(prev) => union_box(prev, b),
                        None => b,
                    });
                }
            }
        }
        if let Some(slot) = self.symbol_bounds.get_mut(def_idx) {
            *slot = acc;
        }
    }
}

fn collect_touched_defs_on_undo(committed: &CommittedTxn<JwwRow, JwwStepPatch>) -> BTreeSet<usize> {
    let mut touched = BTreeSet::new();
    for diff in committed.existing() {
        let space = match diff {
            ExistingRowDiff::Updated { before_row, .. }
            | ExistingRowDiff::Removed { before_row, .. } => before_row.space,
        };
        if let Some(def_idx) = space.0.checked_sub(DEFINITION_SPACES) {
            touched.insert(def_idx as usize);
        }
    }
    touched
}

fn collect_touched_defs_on_redo(committed: &CommittedTxn<JwwRow, JwwStepPatch>) -> BTreeSet<usize> {
    let mut touched = BTreeSet::new();
    for diff in committed.existing() {
        let space = match diff {
            ExistingRowDiff::Updated { after_row, .. } => after_row.space,
            ExistingRowDiff::Removed { before_row, .. } => before_row.space,
        };
        if let Some(def_idx) = space.0.checked_sub(DEFINITION_SPACES) {
            touched.insert(def_idx as usize);
        }
    }
    touched
}

fn file_order_key(slot: SlotRef, top_end: usize) -> (u8, usize) {
    match slot {
        SlotRef::Base(r) if r < top_end => (0, r),
        SlotRef::Born(b) => (1, b),
        SlotRef::Base(r) => (2, r),
    }
}

fn note_changed_slot(
    slot: SlotRef,
    changed_base: &mut BTreeSet<usize>,
    changed_born: &mut BTreeSet<usize>,
) {
    match slot {
        SlotRef::Base(r) => {
            changed_base.insert(r);
        }
        SlotRef::Born(b) => {
            changed_born.insert(b);
        }
    }
}

fn resolve_schema_for_class(classes: &[(Vec<u8>, u16)], class_name: &[u8]) -> Option<u16> {
    if let Some((_, schema)) = classes
        .iter()
        .find(|(name, _)| name.as_slice() == class_name)
    {
        return Some(*schema);
    }
    let (_, first_schema) = classes.first()?;
    classes
        .iter()
        .all(|(_, s)| s == first_schema)
        .then_some(*first_schema)
}

fn parse_jww_element_id(id: &str) -> Option<(SpaceId, usize)> {
    let rest = id.strip_prefix("space/")?;
    let (space_str, rec_part) = rest.split_once('/')?;
    let rec_str = rec_part.strip_prefix('r')?;
    if rec_str.contains('/') {
        return None;
    }
    let space_num: u32 = space_str.parse().ok()?;
    let rec: usize = rec_str.parse().ok()?;
    if space_num.to_string() != space_str || rec.to_string() != rec_str {
        return None;
    }
    Some((SpaceId(space_num), rec))
}

fn paper_origin_of(header: &Header) -> [f64; 2] {
    PAPERS
        .iter()
        .find(|(code, _)| *code == header.paper)
        .map_or([0.0, 0.0], |(_, [w, h])| [*w / 2.0, *h / 2.0])
}

fn inspect_list_layout(file: &JwwFile, src: &[u8], version: u32) -> Option<ListLayout> {
    let records = file.records();
    let header_rec = records.first()?;
    let header_end = header_rec.span().end;
    let standard_header_end = parse::header_end(src, version).ok() == Some(header_end);

    let mut r = Reader::new(src, header_end);
    let entities = count(&mut r)?;
    let mut at = 1usize;
    for _ in 0..entities {
        let rec = records.get(at)?;
        if rec.block().is_some()
            || !matches!(
                rec.kind(),
                RecordKind::Null | RecordKind::Entity | RecordKind::Block
            )
        {
            return None;
        }
        r.pos = rec.span().end;
        at += 1;
    }
    let top_end = at;
    let blocks_count = count(&mut r)?;
    let mut blocks = Vec::with_capacity(blocks_count as usize);
    for _ in 0..blocks_count {
        let rec = records.get(at)?;
        let def_record = at;
        let head_end = match rec.kind() {
            RecordKind::Null => rec.span().start.checked_add(2)?,
            RecordKind::Block => {
                let mut head = Reader::new(src, rec.body().start);
                let skip =
                    RawBase::read(src, head.pos, version).map(|base| base.bytes(version).len())?;
                head.skip(skip + 12).ok()?;
                head.string().ok()?;
                head.pos
            }
            _ => return None,
        };
        let mut inner = Reader::new(src, head_end);
        let n = count(&mut inner)?;
        if inner.pos != rec.span().end {
            return None;
        }
        at += 1;
        let inner_start = at;
        for _ in 0..n {
            let inner_rec = records.get(at)?;
            if inner_rec.block() != Some(def_record)
                || !matches!(
                    inner_rec.kind(),
                    RecordKind::Null | RecordKind::Entity | RecordKind::Block
                )
            {
                return None;
            }
            at += 1;
        }
        let inner_end = at;
        blocks.push(BlockSpan {
            def_record,
            head_end,
            inner_start,
            inner_end,
        });
    }
    let next = records.get(at);
    let rest = next.map_or(src.len(), |rec| rec.span().start);
    let tail_byte_start = if blocks_count == 0 {
        r.pos
    } else {
        records
            .get(at.wrapping_sub(1))
            .map_or(header_end, |rec| rec.span().end)
    };
    let trailer = next.is_some_and(|rec| rec.kind() == RecordKind::Trailer);
    if tail_byte_start > rest && !trailer {
        return None;
    }
    Some(ListLayout {
        header_end,
        top_end,
        blocks,
        after_blocks_record: at,
        tail_byte_start,
        standard_header_end,
    })
}

fn append_exact_range(source: &SourceBytes, range: Range<usize>, out: &mut Vec<u8>) -> bool {
    let Some(want) = range.end.checked_sub(range.start) else {
        return false;
    };
    let before = out.len();
    source.append_range(out, range);
    out.len() - before == want
}

fn work_live(cx: &mut Cx<'_>, amount: u64) -> Result<(), Error> {
    cx.check(Kind::Work, amount).map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })
}

#[expect(clippy::too_many_arguments)]
fn push_row_to_drawing(
    row: &JwwRow,
    source: Ref,
    version_text: &str,
    header: &Header,
    live: &JwwLive,
    seen_line_types: &mut BTreeSet<u8>,
    groups: &mut BTreeMap<usize, Vec<Element<ModelPoint>>>,
    drafts: &mut Vec<Draft>,
    out: &mut Drawing2D,
) {
    let mut element = row.element.clone();
    element.source = source.clone();
    if let Shape::Underlay(u) = &mut element.shape {
        u.resource.definition = match row.underlay_image_record {
            Some(img_rec) => Ref::at_record(live.effective_record(SlotRef::Base(img_rec)) as u64),
            None => source.clone(),
        };
    }
    if let Inherit::Own(LineTypeId(code)) = element.style.line_type
        && let Ok(code_u8) = u8::try_from(code)
    {
        let norm = code_u8.max(1);
        if seen_line_types.insert(norm) {
            out.line_types.push(make_line_type(header, norm));
        }
    }
    let elements = if let Some(def_idx) = row.space.0.checked_sub(DEFINITION_SPACES) {
        let Some(sym) = out.symbols.get_mut(def_idx as usize) else {
            return;
        };
        sym.space.order.push(sym.space.elements.len());
        &mut sym.space.elements
    } else {
        groups.entry(row.space.0 as usize).or_default()
    };
    let elem_index = elements.len();
    for (namespace, fields) in &row.own_extensions {
        drafts.push(Draft {
            namespace: (*namespace).into(),
            name: String::new(),
            version: Some(version_text.to_owned()),
            schema: SCHEMA,
            at: Position::Model {
                space: row.space,
                index: elem_index,
            },
            fields: fields.clone(),
            source: source.clone(),
        });
    }
    elements.push(element);
}

trait ElementSpaceExt {
    fn space_hint(
        &self,
        block_parent: Option<usize>,
        definitions: &BTreeMap<usize, (usize, SpaceId)>,
    ) -> SpaceId;
}

impl ElementSpaceExt for Element<ModelPoint> {
    fn space_hint(
        &self,
        block_parent: Option<usize>,
        definitions: &BTreeMap<usize, (usize, SpaceId)>,
    ) -> SpaceId {
        if let Some(parent) = block_parent
            && let Some(&(_, space)) = definitions.get(&parent)
        {
            return space;
        }
        let n = self.layer.0.saturating_sub(1);
        SpaceId(n / 16)
    }
}

fn lift_records_deterministic(
    file: &JwwFile,
    header: &Header,
    paper_origin: [f64; 2],
    definitions: &BTreeMap<usize, (usize, SpaceId)>,
    images_map: &BTreeMap<String, usize>,
    threads: usize,
) -> Vec<WorkerItem> {
    let records = file.records();
    let total = records.len();
    let lift_one = |index: usize, record: &crate::Record| -> WorkerItem {
        if record.kind() != RecordKind::Entity {
            return WorkerItem {
                outcome: WorkerOutcome::Skip,
                bounds: EMPTY_BOUNDS,
            };
        }
        let def_space = match record.block() {
            Some(parent) => match definitions.get(&parent) {
                Some(&(_, space)) => Some(space),
                None => {
                    return WorkerItem {
                        outcome: WorkerOutcome::Skip,
                        bounds: EMPTY_BOUNDS,
                    };
                }
            },
            None => None,
        };
        let source = Ref::at_record(index as u64);
        let kind_name = file.kind_name(record);
        match lift_single_entity(
            file.bytes(record.body()),
            kind_name,
            header,
            def_space,
            images_map,
            &source,
        ) {
            SingleEntity::Dropped => WorkerItem {
                outcome: WorkerOutcome::Dropped,
                bounds: EMPTY_BOUNDS,
            },
            SingleEntity::Invalid => WorkerItem {
                outcome: WorkerOutcome::Invalid,
                bounds: EMPTY_BOUNDS,
            },
            SingleEntity::DocumentDraft {
                namespace,
                name,
                fields,
            } => WorkerItem {
                outcome: WorkerOutcome::DocumentDraft {
                    namespace,
                    name,
                    fields,
                },
                bounds: EMPTY_BOUNDS,
            },
            SingleEntity::Element {
                element,
                own,
                embedded_image,
            } => {
                let space = element.space_hint(record.block(), definitions);
                let bounds =
                    if space.0 < DEFINITION_SPACES && !matches!(element.shape, Shape::Symbol(_)) {
                        compute_sheet_bounds(
                            &element,
                            space,
                            header,
                            paper_origin,
                            &BTreeMap::new(),
                            &[],
                        )
                    } else {
                        EMPTY_BOUNDS
                    };
                WorkerItem {
                    outcome: WorkerOutcome::Element {
                        element,
                        own,
                        embedded_image,
                    },
                    bounds,
                }
            }
        }
    };

    if threads <= 1 || total < 256 {
        return records
            .iter()
            .enumerate()
            .map(|(idx, rec)| lift_one(idx, rec))
            .collect();
    }

    let chunk_size = total.div_ceil(threads);
    let mut chunks_out: Vec<Vec<WorkerItem>> = Vec::with_capacity(threads);
    std::thread::scope(|s| {
        let mut handles = Vec::new();
        let mut start = 0usize;
        while start < total {
            let end = (start + chunk_size).min(total);
            let slice = records.get(start..end).unwrap_or_default();
            let base_idx = start;
            handles.push(s.spawn(move || {
                slice
                    .iter()
                    .enumerate()
                    .map(|(off, rec)| lift_one(base_idx + off, rec))
                    .collect::<Vec<_>>()
            }));
            start = end;
        }
        for h in handles {
            if let Ok(part) = h.join() {
                chunks_out.push(part);
            }
        }
    });
    let mut flat = Vec::with_capacity(total);
    for part in chunks_out {
        flat.extend(part);
    }
    flat
}

fn revert_display_for_jww(
    display: &mut Display,
    header: &Header,
    paper_origin: [f64; 2],
    symbol_index_by_key: &BTreeMap<String, u32>,
    committed: &CommittedTxn<JwwRow, JwwStepPatch>,
) {
    for _ in 0..committed.born().len() {
        display.pop();
    }
    for diff in committed.existing() {
        match diff {
            ExistingRowDiff::Removed { row, .. } => {
                display.show(row.0);
            }
            ExistingRowDiff::Updated {
                row, before_row, ..
            } => {
                let (counts, style) = display_counts_and_style(
                    &before_row.element,
                    before_row.space,
                    symbol_index_by_key,
                );
                display.update_with(row.0, counts, style, |w| {
                    write_display_element(
                        &before_row.element,
                        before_row.space,
                        header,
                        paper_origin,
                        symbol_index_by_key,
                        w,
                    );
                });
            }
        }
    }
}

fn reapply_display_for_jww(
    display: &mut Display,
    header: &Header,
    paper_origin: [f64; 2],
    symbol_index_by_key: &BTreeMap<String, u32>,
    committed: &CommittedTxn<JwwRow, JwwStepPatch>,
) {
    for diff in committed.existing() {
        match diff {
            ExistingRowDiff::Updated { row, after_row, .. } => {
                let (counts, style) = display_counts_and_style(
                    &after_row.element,
                    after_row.space,
                    symbol_index_by_key,
                );
                display.update_with(row.0, counts, style, |w| {
                    write_display_element(
                        &after_row.element,
                        after_row.space,
                        header,
                        paper_origin,
                        symbol_index_by_key,
                        w,
                    );
                });
            }
            ExistingRowDiff::Removed { row, .. } => {
                display.hide(row.0);
            }
        }
    }
    for born in committed.born() {
        let (counts, style) = display_counts_and_style(
            &born.after_row.element,
            born.after_row.space,
            symbol_index_by_key,
        );
        display.push_with(counts, style, |w| {
            write_display_element(
                &born.after_row.element,
                born.after_row.space,
                header,
                paper_origin,
                symbol_index_by_key,
                w,
            );
        });
    }
}

fn pack_jww_color(color: &Inherit<Color>) -> u32 {
    match color {
        Inherit::Own(c) => {
            if let Some([r, g, b]) = c.rgb {
                0x0100_0000 | (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
            } else if let Some(code) = c.code {
                0x0200_0000 | (code.value & 0x00FF_FFFF)
            } else {
                0
            }
        }
        Inherit::Layer | Inherit::Block => 0,
    }
}

fn display_counts_and_style(
    element: &Element<ModelPoint>,
    space: SpaceId,
    symbol_index_by_key: &BTreeMap<String, u32>,
) -> ([u32; 4], [u32; 2]) {
    let style = [element.layer.0, pack_jww_color(&element.style.color)];
    if space.0 >= DEFINITION_SPACES {
        return ([0, 0, 0, 0], style);
    }
    let counts = match &element.shape {
        Shape::Line { .. } | Shape::Point(_) => [1, 0, 0, 0],
        Shape::Circle { .. } | Shape::Curve(Edge::Arc { .. }) => [0, 1, 0, 0],
        Shape::Curve(Edge::Elliptic {
            center,
            major,
            ratio,
            start,
            end,
            counterclockwise,
        }) => {
            let mut pts = Vec::new();
            let sweep = zumen_geom2d::sweep(*start, *end, *counterclockwise);
            zumen_geom2d::ellipse(
                [center.x, center.y],
                [major.x, major.y],
                *ratio,
                *start,
                sweep,
                DISPLAY_TOLERANCE,
                &mut pts,
            );
            [pts.len().saturating_sub(1) as u32, 0, 0, 0]
        }
        Shape::Text(_) => [0, 0, 1, 0],
        Shape::Symbol(sym) => [
            0,
            0,
            0,
            u32::from(symbol_index_by_key.contains_key(&sym.key.name)),
        ],
        Shape::Underlay(u) => match u.frame {
            zumen_drawing::UnderlayFrame::Corners { .. }
            | zumen_drawing::UnderlayFrame::Pixels { .. } => [4, 0, 0, 0],
            zumen_drawing::UnderlayFrame::Scaled { .. } => [1, 0, 0, 0],
        },
        Shape::Hatch(hatch) => count_hatch_primitives(hatch),
        Shape::Dimension(dim) => [1, 0, u32::from(dim.text.position.is_some()), 0],
        _ => [0, 0, 0, 0],
    };
    (counts, style)
}

fn count_hatch_primitives(hatch: &zumen_drawing::Hatch<ModelPoint>) -> [u32; 4] {
    let mut segs = 0u32;
    let mut arcs = 0u32;
    for lp in &hatch.loops {
        match &lp.boundary {
            Boundary::Polyline { vertices, closed } => {
                if vertices.len() >= 2 {
                    segs = segs.saturating_add((vertices.len() - 1 + usize::from(*closed)) as u32);
                }
            }
            Boundary::Edges(edges) => {
                for edge in edges {
                    match edge {
                        Edge::Line { .. } => segs = segs.saturating_add(1),
                        Edge::Arc { .. } => arcs = arcs.saturating_add(1),
                        Edge::Elliptic {
                            center,
                            major,
                            ratio,
                            start,
                            end,
                            counterclockwise,
                        } => {
                            let mut pts = Vec::new();
                            let sweep = zumen_geom2d::sweep(*start, *end, *counterclockwise);
                            zumen_geom2d::ellipse(
                                [center.x, center.y],
                                [major.x, major.y],
                                *ratio,
                                *start,
                                sweep,
                                DISPLAY_TOLERANCE,
                                &mut pts,
                            );
                            segs = segs.saturating_add(pts.len().saturating_sub(1) as u32);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    [segs, arcs, 0, 0]
}

fn write_display_element(
    element: &Element<ModelPoint>,
    space: SpaceId,
    header: &Header,
    paper_origin: [f64; 2],
    symbol_index_by_key: &BTreeMap<String, u32>,
    w: &mut DisplayWriter<'_>,
) {
    if space.0 >= DEFINITION_SPACES {
        return;
    }
    let k = 1.0 / header.scale(space.0 as usize);
    let [ox, oy] = paper_origin;
    let to_paper = |p: ModelPoint| [p.x * k + ox, p.y * k + oy];
    let len_paper = |len: &Length| match len {
        Length::Model(m) => m.value * k,
        Length::Paper(p) => p.0,
    };

    match &element.shape {
        Shape::Line { start, end } => {
            w.push_segment(to_paper(*start), to_paper(*end));
        }
        Shape::Point(dot) => {
            let p = to_paper(dot.position);
            w.push_segment(p, p);
        }
        Shape::Circle { center, radius } => {
            w.push_arc_rad(to_paper(*center), len_paper(radius), 0.0, TAU);
        }
        Shape::Curve(Edge::Arc {
            center,
            radius,
            start,
            end,
            counterclockwise,
        }) => {
            let sweep = zumen_geom2d::sweep(*start, *end, *counterclockwise);
            w.push_arc_rad(
                to_paper(*center),
                len_paper(radius),
                radians_from_deg(*start),
                radians_from_deg(sweep),
            );
        }
        Shape::Curve(Edge::Elliptic {
            center,
            major,
            ratio,
            start,
            end,
            counterclockwise,
        }) => {
            let mut pts = Vec::new();
            let sweep = zumen_geom2d::sweep(*start, *end, *counterclockwise);
            zumen_geom2d::ellipse(
                [center.x, center.y],
                [major.x, major.y],
                *ratio,
                *start,
                sweep,
                DISPLAY_TOLERANCE,
                &mut pts,
            );
            for pair in pts.windows(2) {
                if let Some(&[[ax, ay], [bx, by]]) = pair.first_chunk::<2>() {
                    w.push_segment([ax * k + ox, ay * k + oy], [bx * k + ox, by * k + oy]);
                }
            }
        }
        Shape::Text(text) => {
            let pos = match text.placement {
                Placement::Anchor { point, .. } => to_paper(point),
                Placement::Between { first, .. } => to_paper(first),
            };
            let h = len_paper(&text.height.value);
            w.push_text(
                pos,
                h,
                radians_from_deg(text.rotation),
                text.width_factor,
                radians_from_deg(text.oblique),
            );
        }
        Shape::Symbol(sym) => {
            let [sx, sy] = sym.scale;
            if let Some(&def_idx) = symbol_index_by_key.get(&sym.key.name) {
                w.push_instance(
                    to_paper(sym.position),
                    [sx * k, sy * k],
                    radians_from_deg(sym.rotation),
                    [0.0, 0.0],
                    def_idx,
                    [1, 1],
                );
            }
        }
        Shape::Underlay(u) => match u.frame {
            zumen_drawing::UnderlayFrame::Corners { origin, right, top }
            | zumen_drawing::UnderlayFrame::Pixels {
                origin, right, top, ..
            } => {
                let [o0, o1] = to_paper(origin);
                let [r0, r1] = to_paper(right);
                let [t0, t1] = to_paper(top);
                let opp = [r0 + t0 - o0, r1 + t1 - o1];
                w.push_segment([o0, o1], [r0, r1]);
                w.push_segment([r0, r1], opp);
                w.push_segment(opp, [t0, t1]);
                w.push_segment([t0, t1], [o0, o1]);
            }
            zumen_drawing::UnderlayFrame::Scaled { origin, .. } => {
                let p = to_paper(origin);
                w.push_segment(p, p);
            }
        },
        Shape::Hatch(hatch) => {
            for lp in &hatch.loops {
                match &lp.boundary {
                    Boundary::Polyline { vertices, closed } => {
                        if vertices.len() >= 2 {
                            for pair in vertices.windows(2) {
                                if let Some([a, b]) = pair.first_chunk::<2>() {
                                    w.push_segment(to_paper(a.point), to_paper(b.point));
                                }
                            }
                            if *closed
                                && let (Some(last), Some(first)) =
                                    (vertices.last(), vertices.first())
                            {
                                w.push_segment(to_paper(last.point), to_paper(first.point));
                            }
                        }
                    }
                    Boundary::Edges(edges) => {
                        for edge in edges {
                            match edge {
                                Edge::Line { start, end } => {
                                    w.push_segment(to_paper(*start), to_paper(*end));
                                }
                                Edge::Arc {
                                    center,
                                    radius,
                                    start,
                                    end,
                                    counterclockwise,
                                } => {
                                    let sweep =
                                        zumen_geom2d::sweep(*start, *end, *counterclockwise);
                                    w.push_arc_rad(
                                        to_paper(*center),
                                        len_paper(radius),
                                        radians_from_deg(*start),
                                        radians_from_deg(sweep),
                                    );
                                }
                                Edge::Elliptic {
                                    center,
                                    major,
                                    ratio,
                                    start,
                                    end,
                                    counterclockwise,
                                } => {
                                    let mut pts = Vec::new();
                                    let sweep =
                                        zumen_geom2d::sweep(*start, *end, *counterclockwise);
                                    zumen_geom2d::ellipse(
                                        [center.x, center.y],
                                        [major.x, major.y],
                                        *ratio,
                                        *start,
                                        sweep,
                                        DISPLAY_TOLERANCE,
                                        &mut pts,
                                    );
                                    for pair in pts.windows(2) {
                                        if let Some(&[[ax, ay], [bx, by]]) = pair.first_chunk::<2>()
                                        {
                                            w.push_segment(
                                                [ax * k + ox, ay * k + oy],
                                                [bx * k + ox, by * k + oy],
                                            );
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        Shape::Dimension(dim) => {
            match dim.kind {
                zumen_drawing::DimensionKind::Linear { first, second, .. }
                | zumen_drawing::DimensionKind::Aligned { first, second, .. } => {
                    w.push_segment(to_paper(first), to_paper(second));
                }
                _ => {
                    w.push_segment([ox, oy], [ox, oy]);
                }
            }
            if let Some(pos) = dim.text.position {
                w.push_text(
                    to_paper(pos),
                    3.0,
                    radians_from_deg(dim.text.rotation.unwrap_or(0.0)),
                    1.0,
                    0.0,
                );
            }
        }
        _ => {}
    }
}

fn is_finite_box([x0, y0, x1, y1]: [f64; 4]) -> bool {
    x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite() && x0 <= x1 && y0 <= y1
}

fn union_box([ax0, ay0, ax1, ay1]: [f64; 4], [bx0, by0, bx1, by1]: [f64; 4]) -> [f64; 4] {
    [ax0.min(bx0), ay0.min(by0), ax1.max(bx1), ay1.max(by1)]
}

fn boxes_intersect([ax0, ay0, ax1, ay1]: [f64; 4], [bx0, by0, bx1, by1]: [f64; 4]) -> bool {
    ax0 <= bx1 && ax1 >= bx0 && ay0 <= by1 && ay1 >= by0
}

fn arc_bounds_2d(
    [cx, cy]: [f64; 2],
    radius: f64,
    start_deg: f64,
    end_deg: f64,
    ccw: bool,
) -> [f64; 4] {
    let sweep = zumen_geom2d::sweep(start_deg, end_deg, ccw);
    if sweep.abs() >= 360.0 {
        return [cx - radius, cy - radius, cx + radius, cy + radius];
    }
    let a0 = start_deg.to_radians();
    let a1 = (start_deg + sweep).to_radians();
    let [p0x, p0y] = [cx + radius * libm::cos(a0), cy + radius * libm::sin(a0)];
    let [p1x, p1y] = [cx + radius * libm::cos(a1), cy + radius * libm::sin(a1)];
    let mut bx0 = p0x.min(p1x);
    let mut by0 = p0y.min(p1y);
    let mut bx1 = p0x.max(p1x);
    let mut by1 = p0y.max(p1y);
    for cardinal in [0.0, 90.0, 180.0, 270.0] {
        let rel = if sweep >= 0.0 {
            (cardinal - start_deg).rem_euclid(360.0)
        } else {
            (start_deg - cardinal).rem_euclid(360.0)
        };
        if rel <= sweep.abs() {
            let rad = cardinal.to_radians();
            let px = cx + radius * libm::cos(rad);
            let py = cy + radius * libm::sin(rad);
            bx0 = bx0.min(px);
            by0 = by0.min(py);
            bx1 = bx1.max(px);
            by1 = by1.max(py);
        }
    }
    [bx0, by0, bx1, by1]
}

/// 要素のモデル空間での囲む箱を求める（ブロック定義空間ではそのまま mm）
fn model_element_bounds(shape: &Shape<ModelPoint>) -> [f64; 4] {
    let len_val = |l: &Length| match l {
        Length::Model(m) => m.value.abs(),
        Length::Paper(p) => p.0.abs(),
    };
    match shape {
        Shape::Line { start, end } => [
            start.x.min(end.x),
            start.y.min(end.y),
            start.x.max(end.x),
            start.y.max(end.y),
        ],
        Shape::Point(dot) => [
            dot.position.x,
            dot.position.y,
            dot.position.x,
            dot.position.y,
        ],
        Shape::Circle { center, radius } => {
            let r = len_val(radius);
            [center.x - r, center.y - r, center.x + r, center.y + r]
        }
        Shape::Curve(Edge::Arc {
            center,
            radius,
            start,
            end,
            counterclockwise,
        }) => arc_bounds_2d(
            [center.x, center.y],
            len_val(radius),
            *start,
            *end,
            *counterclockwise,
        ),
        Shape::Curve(Edge::Elliptic {
            center,
            major,
            ratio,
            start,
            end,
            counterclockwise,
        }) => {
            let mut pts = Vec::new();
            let sweep = zumen_geom2d::sweep(*start, *end, *counterclockwise);
            zumen_geom2d::ellipse(
                [center.x, center.y],
                [major.x, major.y],
                *ratio,
                *start,
                sweep,
                DISPLAY_TOLERANCE,
                &mut pts,
            );
            points_bounds(&pts)
        }
        Shape::Text(text) => text_bounds_in_coords(text, 1.0),
        Shape::Underlay(u) => match u.frame {
            zumen_drawing::UnderlayFrame::Corners { origin, right, top }
            | zumen_drawing::UnderlayFrame::Pixels {
                origin, right, top, ..
            } => {
                let opp = [right.x + top.x - origin.x, right.y + top.y - origin.y];
                points_bounds(&[
                    [origin.x, origin.y],
                    [right.x, right.y],
                    [top.x, top.y],
                    opp,
                ])
            }
            zumen_drawing::UnderlayFrame::Scaled { origin, .. } => {
                [origin.x, origin.y, origin.x, origin.y]
            }
        },
        Shape::Hatch(hatch) => {
            let mut acc: Option<[f64; 4]> = None;
            let mut fold = |b: [f64; 4]| {
                if is_finite_box(b) {
                    acc = Some(match acc {
                        Some(prev) => union_box(prev, b),
                        None => b,
                    });
                }
            };
            for lp in &hatch.loops {
                match &lp.boundary {
                    Boundary::Polyline { vertices, .. } => {
                        let pts: Vec<[f64; 2]> =
                            vertices.iter().map(|v| [v.point.x, v.point.y]).collect();
                        fold(points_bounds(&pts));
                    }
                    Boundary::Edges(edges) => {
                        for edge in edges {
                            match edge {
                                Edge::Line { start, end } => fold([
                                    start.x.min(end.x),
                                    start.y.min(end.y),
                                    start.x.max(end.x),
                                    start.y.max(end.y),
                                ]),
                                Edge::Arc {
                                    center,
                                    radius,
                                    start,
                                    end,
                                    counterclockwise,
                                } => fold(arc_bounds_2d(
                                    [center.x, center.y],
                                    len_val(radius),
                                    *start,
                                    *end,
                                    *counterclockwise,
                                )),
                                Edge::Elliptic {
                                    center,
                                    major,
                                    ratio,
                                    start,
                                    end,
                                    counterclockwise,
                                } => {
                                    let mut pts = Vec::new();
                                    let sweep =
                                        zumen_geom2d::sweep(*start, *end, *counterclockwise);
                                    zumen_geom2d::ellipse(
                                        [center.x, center.y],
                                        [major.x, major.y],
                                        *ratio,
                                        *start,
                                        sweep,
                                        DISPLAY_TOLERANCE,
                                        &mut pts,
                                    );
                                    fold(points_bounds(&pts));
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            acc.unwrap_or(EMPTY_BOUNDS)
        }
        Shape::Dimension(dim) => {
            let mut pts = Vec::new();
            if let zumen_drawing::DimensionKind::Linear {
                first,
                second,
                line,
                ..
            } = dim.kind
            {
                pts.push([first.x, first.y]);
                pts.push([second.x, second.y]);
                pts.push([line.x, line.y]);
            }
            if let Some(tp) = dim.text.position {
                pts.push([tp.x, tp.y]);
            }
            points_bounds(&pts)
        }
        Shape::Symbol(sym) => [
            sym.position.x,
            sym.position.y,
            sym.position.x,
            sym.position.y,
        ],
        _ => EMPTY_BOUNDS,
    }
}

fn text_bounds_in_coords(text: &zumen_drawing::Text<ModelPoint>, inv_k: f64) -> [f64; 4] {
    let anchor = match text.placement {
        Placement::Anchor { point, .. } => point,
        Placement::Between { first, .. } => first,
    };
    let h = match &text.height {
        Height {
            value: Length::Paper(p),
            ..
        } => p.0 * inv_k,
        Height {
            value: Length::Model(m),
            ..
        } => m.value,
    }
    .abs();
    let chars = text.content.chars().count().max(1) as f64;
    let w = h * text.width_factor.abs().max(0.5) * chars;
    let rad = text.rotation.to_radians();
    let (s, c) = (libm::sin(rad), libm::cos(rad));
    let corners = [
        [anchor.x, anchor.y],
        [anchor.x + w * c, anchor.y + w * s],
        [anchor.x - h * s, anchor.y + h * c],
        [anchor.x + w * c - h * s, anchor.y + w * s + h * c],
    ];
    points_bounds(&corners)
}

fn points_bounds(pts: &[[f64; 2]]) -> [f64; 4] {
    let mut bx0 = f64::INFINITY;
    let mut by0 = f64::INFINITY;
    let mut bx1 = f64::NEG_INFINITY;
    let mut by1 = f64::NEG_INFINITY;
    for &[x, y] in pts {
        if x.is_finite() && y.is_finite() {
            bx0 = bx0.min(x);
            by0 = by0.min(y);
            bx1 = bx1.max(x);
            by1 = by1.max(y);
        }
    }
    [bx0, by0, bx1, by1]
}

fn compute_sheet_bounds(
    element: &Element<ModelPoint>,
    space: SpaceId,
    header: &Header,
    [ox, oy]: [f64; 2],
    symbol_index_by_key: &BTreeMap<String, u32>,
    symbol_bounds: &[Option<[f64; 4]>],
) -> [f64; 4] {
    if space.0 >= DEFINITION_SPACES {
        return EMPTY_BOUNDS;
    }
    let k = 1.0 / header.scale(space.0 as usize);

    if let Shape::Symbol(sym) = &element.shape {
        let [sx_raw, sy_raw] = sym.scale;
        if let Some(&def_idx) = symbol_index_by_key.get(&sym.key.name)
            && let Some(&Some([dx0, dy0, dx1, dy1])) = symbol_bounds.get(def_idx as usize)
        {
            let sx = sx_raw * k;
            let sy = sy_raw * k;
            let rad = sym.rotation.to_radians();
            let (s, c) = (libm::sin(rad), libm::cos(rad));
            let px = sym.position.x * k + ox;
            let py = sym.position.y * k + oy;
            let raw_corners = [[dx0, dy0], [dx1, dy0], [dx0, dy1], [dx1, dy1]];
            let mut xformed = [[0.0; 2]; 4];
            for (dst, &[dx, dy]) in xformed.iter_mut().zip(&raw_corners) {
                let rx = dx * sx;
                let ry = dy * sy;
                *dst = [px + rx * c - ry * s, py + rx * s + ry * c];
            }
            return points_bounds(&xformed);
        }
        let px = sym.position.x * k + ox;
        let py = sym.position.y * k + oy;
        return [px, py, px, py];
    }

    if let Shape::Text(text) = &element.shape {
        let model_box = text_bounds_in_coords(text, header.scale(space.0 as usize));
        if !is_finite_box(model_box) {
            return EMPTY_BOUNDS;
        }
        let [mx0, my0, mx1, my1] = model_box;
        return [mx0 * k + ox, my0 * k + oy, mx1 * k + ox, my1 * k + oy];
    }

    let model_box = model_element_bounds(&element.shape);
    if !is_finite_box(model_box) {
        return EMPTY_BOUNDS;
    }
    let [mx0, my0, mx1, my1] = model_box;
    [mx0 * k + ox, my0 * k + oy, mx1 * k + ox, my1 * k + oy]
}
