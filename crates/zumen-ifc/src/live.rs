//! IFC の常駐文書エンジン（決定 0207・0215・0217・0244、#878）。
//!
//! 初回の読み込みで元バイト列を 16 KiB ページ単位の [`SourceBytes`] に圧縮し、レコード位置の表・逆向きの参照・
//! 建物モデル（[`Building`]）・版の表（[`VersionedTable`]）を常駐させる。変更（[`Edit`]）は全体の逆参照の
//! 作り直しや STEP 全文の書き出し・再解析を行わず、触れたインスタンスと関係する要素だけを `O(変更数)` で更新し、
//! 変更前の値を [`History`] に記録して完全な取り消し・やり直しを提供する。

#[cfg(feature = "ids")]
use core::cell::RefCell;
use core::ops::Range;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use zumen_base::{Budget, CancelToken, Diagnostic, Diagnostics, Kind, Limits, codes};
use zumen_building::{
    Building, ElementRef, GlobalId, Grid, Hash, Identity, Literal, Measured, Opening, Properties,
    Property, PropertySet, Quantities, Quantity, QuantitySet, RelationSource, TypedValue, Unit,
};
#[cfg(feature = "ids")]
use zumen_ids::{AttributeValue, Lookup, Missing};
use zumen_live::{
    DepGraph, DirtyColumn, EMPTY_BOUNDS, History, PatchPayload, Revision, RowId, SideIndexes,
    SourceBytes, TableRow, TxnError, VersionedTable, run_transaction,
};
#[cfg(feature = "ids")]
use zumen_sdk::Stop;
use zumen_sdk::{Codec, Cx, Error};

use crate::building::{
    Category, MEASURES, QUANTITIES, Version, arg, category, decode, lift_with_units, reference,
    refs,
};
use crate::edit::{
    Edit, Item, NewValue, RootAttribute, Stmt, refs_in, refs_of, stmt_from_slice, swap, text,
    text_of, typed, unsupported, write_stmt,
};
#[cfg(feature = "ids")]
use crate::express::{Express, Layouts};
#[cfg(feature = "ids")]
use crate::ids::attributes_from_record_slice;
use crate::{DIALECT, Ifc, IfcFile, IfcZip, IfcZipFile, Record, Value, at_bytes, stopped};

/// [`VersionedTable`] に並べる建物要素 1 行。要素番号 [`ElementRef`] と [`RowId`] は 1 対 1 に対応する
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IfcElementRow {
    /// 建物モデル内の要素番号
    pub element: ElementRef,
    /// 出所の STEP インスタンス番号
    pub instance: Option<u64>,
    /// この要素が受けた変更の世代番号
    pub generation: u32,
}

impl TableRow for IfcElementRow {
    #[inline]
    fn row_handle(&self) -> Option<u64> {
        self.instance
    }
}

#[derive(Clone, Debug)]
struct StmtDiff {
    id: u64,
    before: Option<Stmt>,
    after: Stmt,
    old_refs: Vec<u64>,
    new_refs: Vec<u64>,
}

impl StmtDiff {
    fn heap_bytes(&self) -> usize {
        stmt_heap_bytes(&self.after)
            + self.before.as_ref().map_or(0, stmt_heap_bytes)
            + (self.old_refs.capacity() + self.new_refs.capacity()) * core::mem::size_of::<u64>()
    }
}

fn stmt_heap_bytes(stmt: &Stmt) -> usize {
    stmt.name.capacity() + stmt.args.iter().map(item_heap_bytes).sum::<usize>()
}

fn item_heap_bytes(item: &Item) -> usize {
    match item {
        Item::Ref(_) => 0,
        Item::Raw(b) => b.capacity(),
        Item::List(items) => {
            items.capacity() * core::mem::size_of::<Item>()
                + items.iter().map(item_heap_bytes).sum::<usize>()
        }
    }
}

#[derive(Clone, Debug)]
struct ElementSnap {
    identity: Option<Identity>,
    has_level: bool,
    level_name: Option<String>,
    properties: Option<Properties>,
    quantities: Option<Quantities>,
    relations: Vec<RelationSource>,
}

impl ElementSnap {
    fn heap_bytes(&self) -> usize {
        let id_bytes = self.identity.as_ref().map_or(0, |id| {
            id.name.as_ref().map_or(0, String::capacity)
                + id.description.as_ref().map_or(0, String::capacity)
                + id.tag.as_ref().map_or(0, String::capacity)
                + id.long_name.as_ref().map_or(0, String::capacity)
        });
        let prop_bytes = self.properties.as_ref().map_or(0, |p| {
            p.sets.capacity() * core::mem::size_of::<PropertySet>()
        });
        let qty_bytes = self.quantities.as_ref().map_or(0, |q| {
            q.sets.capacity() * core::mem::size_of::<QuantitySet>()
        });
        let rel_bytes = self.relations.capacity() * core::mem::size_of::<RelationSource>();
        id_bytes
            + self.level_name.as_ref().map_or(0, String::capacity)
            + prop_bytes
            + qty_bytes
            + rel_bytes
    }
}

#[derive(Clone, Debug)]
struct ElementSnapDiff {
    element: ElementRef,
    before: ElementSnap,
    after: ElementSnap,
}

#[derive(Clone, Debug)]
struct IfcPatch {
    stmt_diffs: Vec<StmtDiff>,
    added_ids: Vec<u64>,
    before_next: u64,
    after_next: u64,
    project_name: Option<(Option<String>, Option<String>)>,
    element_diffs: Vec<ElementSnapDiff>,
}

#[derive(Clone, Debug)]
struct SharedIfcPatch(Arc<IfcPatch>);

impl PatchPayload for SharedIfcPatch {
    fn heap_bytes(&self) -> usize {
        let p = &self.0;
        p.stmt_diffs.iter().map(StmtDiff::heap_bytes).sum::<usize>()
            + p.added_ids.capacity() * core::mem::size_of::<u64>()
            + p.element_diffs
                .iter()
                .map(|d| d.before.heap_bytes() + d.after.heap_bytes())
                .sum::<usize>()
    }
}

/// IFC-SPF および ifcZIP の常駐文書エンジン
#[derive(Clone, Debug)]
pub struct IfcLive {
    /// 16 KiB ページ単位で LZ4 圧縮した元の IFC-SPF バイト列
    src: SourceBytes,
    /// 元のレコード配列
    records: Vec<Record>,
    /// `FILE_SCHEMA` の最初のスキーマ名
    schema: Vec<u8>,
    /// 建物モデルの IFC バージョン
    version: Version,
    /// インスタンス番号から最初のレコード番号への表
    index: BTreeMap<u64, usize>,
    /// データのセクションを閉じる `ENDSEC` レコードの番号
    data_endsec_idx: Option<usize>,
    /// 元の IFC-SPF が `\r\n` を含むか
    has_crlf: bool,
    /// ifcZIP で開いた場合のアーカイブ内項目名
    zip_entry: Option<String>,
    /// ifcZIP で開いた場合の未編集アーカイブの圧縮バイト列
    clean_zip_bytes: Option<SourceBytes>,

    /// 書き直したインスタンスおよび追加したインスタンス
    changed: BTreeMap<u64, Stmt>,
    /// 追加したインスタンス番号（追加順）
    added: Vec<u64>,
    /// 次に採番するインスタンス番号
    next: u64,
    /// 常駐する逆向きの参照表（参照先 ID -> 参照元 ID の集合）
    referrers: BTreeMap<u64, BTreeSet<u64>>,

    /// 常駐する建物モデル
    building: Building,
    /// インスタンス番号から建物要素への逆引き
    elements_by_instance: BTreeMap<u64, ElementRef>,
    /// 最初の `IFCPROJECT` のインスタンス番号
    first_project_id: Option<u64>,
    /// 初回変換で解決済みの明示単位インスタンス表
    units: BTreeMap<u64, Option<Unit>>,
    /// 各要素の `IFCRELDEFINESBYPROPERTIES` 以外の不変な関係出所（文の位置順キー付き）
    base_non_prop_relations: Vec<Vec<(usize, RelationSource)>>,
    /// 各要素の不変な構造依存元の集合（空間階層・相対配置・型・材料・開口・グループ・通り芯・区画）
    base_dep_sources: Vec<BTreeSet<RowId>>,

    /// 要素ごとの版管理表とハンドル索引
    table: VersionedTable<IfcElementRow>,
    /// 64 行塊の変更追跡
    dirty_rows: DirtyColumn<12>,
    /// 要素間の依存グラフ（プロパティの要素参照も含む）
    dep: DepGraph,
    /// 取り消し・やり直し履歴
    history: History<IfcElementRow, SharedIfcPatch>,
}

impl IfcLive {
    /// IFC-SPF または ifcZIP のバイト列から常駐文書エンジンを開く
    pub fn open(bytes: &[u8], cx: &mut Cx) -> Result<Self, Error> {
        Self::open_with_threads(bytes, 1, cx)
    }

    /// ページ圧縮に使うスレッド数を指定して IFC-SPF または ifcZIP から開く
    pub fn open_with_threads(bytes: &[u8], threads: usize, cx: &mut Cx) -> Result<Self, Error> {
        if IfcZip::SIGNATURES.iter().any(|s| s.matches(bytes)) {
            let zipped = IfcZip::read(bytes, cx)?;
            Self::from_zip_file(zipped, threads, cx)
        } else {
            let file = Ifc::read(bytes, cx)?;
            Self::from_file(file, threads, cx)
        }
    }

    /// 読み込み済みの [`IfcFile`] から常駐文書エンジンを構築する
    pub fn from_file(file: IfcFile, threads: usize, cx: &mut Cx) -> Result<Self, Error> {
        Self::build(file, None, None, threads, cx)
    }

    /// 読み込み済みの [`IfcZipFile`] から常駐文書エンジンを構築する
    pub fn from_zip_file(zipped: IfcZipFile, threads: usize, cx: &mut Cx) -> Result<Self, Error> {
        let (archive_vec, entry_name, file) = zipped.into_parts();
        let clean_zip = SourceBytes::compress_slice_with_cx(&archive_vec, threads, cx)
            .map_err(|stop| stopped(cx, stop, None))?;
        drop(archive_vec);
        Self::build(file, Some(entry_name), Some(clean_zip), threads, cx)
    }

    fn build(
        file: IfcFile,
        zip_entry: Option<String>,
        clean_zip_bytes: Option<SourceBytes>,
        threads: usize,
        cx: &mut Cx,
    ) -> Result<Self, Error> {
        let (building, version, units) = lift_with_units(&file, cx)?;
        let inverse = file.inverse(cx)?;

        let mut referrers: BTreeMap<u64, BTreeSet<u64>> = BTreeMap::new();
        let mut max_id = 0u64;
        for id in file.ids() {
            max_id = max_id.max(id);
            for &by in inverse.referrers(id) {
                referrers.entry(id).or_default().insert(by);
            }
        }
        drop(inverse);

        let first_project_id = file.ids().find(|&id| {
            file.instance(id)
                .is_some_and(|rec| file.kind(rec).eq_ignore_ascii_case(b"IFCPROJECT"))
        });

        let mut data_endsec_idx = None;
        let mut in_data = false;
        for (rec_idx, record) in file.records().iter().enumerate() {
            if record.id().is_some() {
                continue;
            }
            let kind = file.kind(record);
            if kind.eq_ignore_ascii_case(b"DATA") {
                in_data = true;
            } else if kind.eq_ignore_ascii_case(b"ENDSEC") && in_data && data_endsec_idx.is_none() {
                data_endsec_idx = Some(rec_idx);
                in_data = false;
            }
        }

        let elem_count = building.elements().len();
        let mut elements_by_instance = BTreeMap::new();
        let mut base_non_prop_relations = Vec::with_capacity(elem_count);
        let mut initial_rows = Vec::with_capacity(elem_count);

        for (idx, elem) in building.elements().iter().enumerate() {
            let elem_ref = ElementRef(u32::try_from(idx).unwrap_or(u32::MAX));
            let instance = elem.source().instance;
            if let Some(inst) = instance {
                elements_by_instance.insert(inst, elem_ref);
            }
            let mut non_prop = Vec::new();
            for rel_src in &elem.source().relations {
                let is_prop_rel = file.instance(rel_src.instance).is_some_and(|rec| {
                    file.kind(rec)
                        .eq_ignore_ascii_case(b"IFCRELDEFINESBYPROPERTIES")
                });
                if !is_prop_rel {
                    let order_key = file.position(rel_src.instance).unwrap_or(usize::MAX);
                    non_prop.push((order_key, rel_src.clone()));
                }
            }
            base_non_prop_relations.push(non_prop);
            initial_rows.push((
                IfcElementRow {
                    element: elem_ref,
                    instance,
                    generation: 0,
                },
                EMPTY_BOUNDS,
                true,
            ));
        }

        let table = VersionedTable::from_rows_with_cx(initial_rows, cx)
            .map_err(|stop| stopped(cx, stop, None))?;
        let (base_dep_sources, dep) = build_ifc_dep_graph(&building, cx)?;

        let schema = file.schema().to_vec();
        let (src_vec, records, _schema_range, index) = file.into_parts();
        let has_crlf = src_vec.windows(2).any(|w| w == b"\r\n");
        let src = SourceBytes::compress_slice_with_cx(&src_vec, threads, cx)
            .map_err(|stop| stopped(cx, stop, None))?;
        drop(src_vec);

        let mut dirty_rows = DirtyColumn::new();
        dirty_rows.clear(table.len());

        Ok(Self {
            src,
            records,
            schema,
            version,
            index,
            data_endsec_idx,
            has_crlf,
            zip_entry,
            clean_zip_bytes,
            changed: BTreeMap::new(),
            added: Vec::new(),
            next: max_id.saturating_add(1),
            referrers,
            building,
            elements_by_instance,
            first_project_id,
            units,
            base_non_prop_relations,
            base_dep_sources,
            table,
            dirty_rows,
            dep,
            history: History::new(),
        })
    }

    /// 要素間の依存グラフへの参照
    #[inline]
    pub const fn dep_graph(&self) -> &DepGraph {
        &self.dep
    }

    /// `changed` に含まれる要素が変更されたとき、依存辺（空間階層・相対配置・型・材料・開口・グループ・通り芯・区画・プロパティ参照）をたどって影響を受ける要素列をトポロジカル順で返す
    pub fn affected_elements(
        &self,
        changed: &[ElementRef],
        cx: &mut Cx,
    ) -> Result<Vec<ElementRef>, Error> {
        let roots: Vec<RowId> = changed.iter().map(|e| RowId(e.0)).collect();
        let rows = self
            .dep
            .affected_rows_with_cx(&roots, cx)
            .map_err(|stop| stopped(cx, stop, None))?;
        Ok(rows.into_iter().map(|r| ElementRef(r.0)).collect())
    }

    /// `FILE_SCHEMA` の最初のスキーマ名（`IFC4` など）
    #[inline]
    pub fn schema(&self) -> &[u8] {
        &self.schema
    }

    /// ifcZIP で開いた場合のアーカイブ内項目名
    #[inline]
    pub fn zip_entry(&self) -> Option<&str> {
        self.zip_entry.as_deref()
    }

    /// ifcZIP アーカイブ由来か
    #[inline]
    pub fn is_zip(&self) -> bool {
        self.zip_entry.is_some()
    }

    /// 常駐する最新の建物モデルへの参照
    #[inline]
    pub fn building(&self) -> &Building {
        &self.building
    }

    /// 表の現在の版番号
    #[inline]
    pub fn revision(&self) -> Revision {
        self.table.revision()
    }

    /// 内容の状態番号（開いた直後および初期状態まで取り消したときは `0`）
    #[inline]
    pub fn state_id(&self) -> u64 {
        self.table.state_id()
    }

    /// 未変更（または初期状態まで取り消し済み）か
    #[inline]
    pub fn is_clean(&self) -> bool {
        self.table.state_id() == 0 && self.changed.is_empty() && self.added.is_empty()
    }

    /// 取り消せる操作があるか
    #[inline]
    pub fn can_undo(&self) -> bool {
        self.history.undo_len() > 0
    }

    /// やり直せる操作があるか
    #[inline]
    pub fn can_redo(&self) -> bool {
        self.history.redo_len() > 0
    }

    /// 取り消し履歴の段数
    #[inline]
    pub fn undo_len(&self) -> usize {
        self.history.undo_len()
    }

    /// やり直し履歴の段数
    #[inline]
    pub fn redo_len(&self) -> usize {
        self.history.redo_len()
    }

    /// STEP インスタンス番号から建物要素 [`ElementRef`] を `O(1)` で引く
    #[inline]
    pub fn element_by_instance(&self, instance: u64) -> Option<ElementRef> {
        self.table
            .find_handle(instance)
            .and_then(|row_id| self.table.row(row_id))
            .map(|row| row.element)
    }

    /// 圧縮済み元バイト列が保持しているメモリ上のバイト数
    pub fn stored_source_bytes(&self) -> usize {
        self.src.stored_bytes()
            + self
                .clean_zip_bytes
                .as_ref()
                .map_or(0, SourceBytes::stored_bytes)
    }

    /// 前回の同期以降に変更された要素行の範囲を取り出し、変更フラグをクリアする
    pub fn take_dirty_element_spans(&mut self) -> Vec<Range<usize>> {
        let len = self.table.len();
        let spans = self.dirty_rows.spans(len);
        self.dirty_rows.clear(len);
        spans
    }

    /// IDS 照合用の [`LiveIdsLookup`] を作る（全体書き出しや再解析を行わない）
    #[cfg(feature = "ids")]
    pub fn ids_lookup<'a>(&'a self, schema: Option<&'a Express>) -> LiveIdsLookup<'a> {
        LiveIdsLookup::new(self, schema)
    }

    /// IFC4.3 の線形・地表面を土木の領域モデルに変換する
    #[cfg(feature = "civil")]
    pub fn civil(&self, cx: &mut Cx) -> Result<zumen_civil::CivilModel, Error> {
        use zumen_sdk::Lift;
        let file = self.to_file(cx)?;
        file.lift(cx)
    }

    /// 現在の状態から [`IfcFile`] を復元する
    pub fn to_file(&self, cx: &mut Cx) -> Result<IfcFile, Error> {
        let bytes = self.save_spf(cx)?;
        let mut quiet_diag = Diagnostics::new();
        let mut inner = Cx::new(cx.budget, &mut quiet_diag, cx.cancel);
        match Ifc::read(&bytes, &mut inner) {
            Ok(file) => Ok(file),
            Err(Error::Stopped(stop)) => Err(stopped(cx, stop, None)),
            Err(other) => Err(other),
        }
    }

    /// 変更の一覧を原子的に当てる。書き直したインスタンスと追加したインスタンスの合計数を返す。
    /// 当てられない変更や予算超過・中断が起きた場合は、常駐状態を一切変えない。
    pub fn apply(&mut self, edits: &[Edit], seed: &[u8], cx: &mut Cx) -> Result<usize, Error> {
        let staged = {
            let mut draft = OverlayDraft::new(self, seed);
            let mut rejected = false;
            for edit in edits {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| stopped(cx, stop, None))?;
                let ok = match edit {
                    Edit::Property {
                        element,
                        set,
                        name,
                        value,
                    } => draft.property(*element, set, name, value, cx)?,
                    Edit::Attribute {
                        element,
                        attribute,
                        value,
                    } => draft.attribute(*element, *attribute, value.as_deref(), cx)?,
                };
                rejected |= !ok;
            }
            if rejected {
                return Err(Error::Invalid);
            }
            if !draft.staged_added.is_empty() && self.data_endsec_idx.is_none() {
                cx.report(Diagnostic::new(codes::IFC_FILE_INVALID).at(at_bytes(0..0)));
                return Err(Error::Invalid);
            }
            draft.finish(cx)?
        };

        let changed_count = staged.changed_count;
        if staged.patch.stmt_diffs.is_empty() && staged.patch.element_diffs.is_empty() {
            return Ok(0);
        }

        let shared_patch = SharedIfcPatch(Arc::new(staged.patch));
        let patch_for_txn = shared_patch.clone();
        let mut updated_rows: Vec<(ElementRef, Option<Vec<RowId>>)> =
            Vec::with_capacity(shared_patch.0.element_diffs.len());
        for diff in &shared_patch.0.element_diffs {
            let new_srcs = if diff.before.properties != diff.after.properties {
                let mut combined = self
                    .base_dep_sources
                    .get(diff.element.0 as usize)
                    .cloned()
                    .unwrap_or_default();
                collect_property_element_refs(diff.after.properties.as_ref(), &mut combined);
                Some(combined.into_iter().collect())
            } else {
                None
            };
            updated_rows.push((diff.element, new_srcs));
        }

        let side = SideIndexes {
            grid: None,
            dirty_rows: Some(&mut self.dirty_rows),
            dep: Some(&mut self.dep),
        };
        let txn_res = run_transaction(
            &mut self.table,
            side,
            &mut self.history,
            cx,
            |txn, _inner_cx| -> Result<(), TxnError<Error>> {
                for (elem_ref, maybe_srcs) in updated_rows {
                    let row_id = RowId(elem_ref.0);
                    if let Some(cur) = txn.row(row_id).copied() {
                        let next_row = IfcElementRow {
                            generation: cur.generation.wrapping_add(1),
                            ..cur
                        };
                        txn.update_row(row_id, next_row, EMPTY_BOUNDS)?;
                        if let Some(srcs) = maybe_srcs {
                            txn.set_dep_sources(row_id, srcs)?;
                        }
                    }
                }
                txn.push_patch(patch_for_txn);
                Ok(())
            },
        );

        match txn_res {
            Ok(_) => {
                self.apply_patch_forward(&shared_patch.0);
                Ok(changed_count)
            }
            Err(TxnError::Stop(stop)) => Err(stopped(cx, stop, None)),
            Err(TxnError::Rejected(err)) => Err(err),
            Err(TxnError::Stage(_)) => Err(Error::Invalid),
        }
    }

    /// 直前の変更を取り消す。取り消したら `Ok(true)`、履歴が空なら `Ok(false)` を返す
    pub fn undo(&mut self, cx: &mut Cx) -> Result<bool, Error> {
        let side = SideIndexes {
            grid: None,
            dirty_rows: Some(&mut self.dirty_rows),
            dep: Some(&mut self.dep),
        };
        let committed_patch = match self.history.undo(&mut self.table, side, cx) {
            Ok(Some(committed)) => committed.patches().first().cloned(),
            Ok(None) => return Ok(false),
            Err(stop) => return Err(stopped(cx, stop, None)),
        };
        if let Some(shared) = committed_patch {
            self.apply_patch_backward(&shared.0);
        }
        Ok(true)
    }

    /// 取り消した変更をやり直す。やり直したら `Ok(true)`、履歴が空なら `Ok(false)` を返す
    pub fn redo(&mut self, cx: &mut Cx) -> Result<bool, Error> {
        let side = SideIndexes {
            grid: None,
            dirty_rows: Some(&mut self.dirty_rows),
            dep: Some(&mut self.dep),
        };
        let committed_patch = match self.history.redo(&mut self.table, side, cx) {
            Ok(Some(committed)) => committed.patches().first().cloned(),
            Ok(None) => return Ok(false),
            Err(stop) => return Err(stopped(cx, stop, None)),
        };
        if let Some(shared) = committed_patch {
            self.apply_patch_forward(&shared.0);
        }
        Ok(true)
    }

    /// 全体を書く保存として IFC-SPF バイト列を書き出す（現行の [`crate::apply`] を順に当てた結果とバイト列一致）
    pub fn save_spf(&self, cx: &mut Cx) -> Result<Vec<u8>, Error> {
        if self.is_clean() {
            for record in &self.records {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| stopped(cx, stop, Some(record.span())))?;
            }
            let mut out = Vec::with_capacity(self.src.len());
            for page in 0..self.src.page_count() {
                self.src.append_page(&mut out, page);
            }
            return Ok(out);
        }

        let newline: &[u8] = if self.has_crlf { b"\r\n" } else { b"\n" };
        let mut out = Vec::with_capacity(self.src.len() + self.added.len() * 96);
        let mut reader = self.src.page_reader();
        let mut flush_start = 0usize;
        let mut placed = self.added.is_empty();

        for (rec_idx, record) in self.records.iter().enumerate() {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, Some(record.span())))?;
            if !placed && self.data_endsec_idx == Some(rec_idx) {
                if flush_start < record.span().start {
                    reader.append_range(&mut out, flush_start..record.span().start);
                }
                flush_start = record.span().start;
                for &id in &self.added {
                    if let Some(stmt) = self.changed.get(&id) {
                        write_stmt(id, stmt, &mut out);
                        out.extend_from_slice(newline);
                    }
                }
                placed = true;
            }

            let modified = record.id().and_then(|id| {
                let first = self.index.get(&id) == Some(&rec_idx);
                first.then(|| self.changed.get(&id).map(|stmt| (id, stmt)))?
            });
            if let Some((id, stmt)) = modified {
                if flush_start < record.span().start {
                    reader.append_range(&mut out, flush_start..record.span().start);
                }
                write_stmt(id, stmt, &mut out);
                flush_start = record.span().end;
            }
        }

        if flush_start < self.src.len() {
            reader.append_range(&mut out, flush_start..self.src.len());
        }
        if !placed {
            cx.report(Diagnostic::new(codes::IFC_FILE_INVALID).at(at_bytes(0..0)));
            return Err(Error::Invalid);
        }
        Ok(out)
    }

    /// 開いた形式（IFC-SPF または ifcZIP）に合わせて全体を書く保存を行う。
    /// ifcZIP で未変更（または初期状態まで取り消し済み）のときは元のアーカイブのバイト列をそのまま返す。
    pub fn save(&self, cx: &mut Cx) -> Result<Vec<u8>, Error> {
        let Some(entry) = self.zip_entry.as_deref() else {
            return self.save_spf(cx);
        };
        if self.is_clean()
            && let Some(clean_zip) = &self.clean_zip_bytes
        {
            cx.check(Kind::Work, clean_zip.page_count().max(1) as u64)
                .map_err(|stop| stopped(cx, stop, None))?;
            let mut out = Vec::with_capacity(clean_zip.len());
            for page in 0..clean_zip.page_count() {
                clean_zip.append_page(&mut out, page);
            }
            return Ok(out);
        }
        let spf = self.save_spf(cx)?;
        zumen_zip::write(&[(entry, &spf)]).ok_or_else(|| {
            cx.report(Diagnostic::new(codes::IFC_FILE_INVALID).at(at_bytes(0..0)));
            Error::Invalid
        })
    }

    fn base_instance(&self, id: u64) -> Option<&Record> {
        self.records.get(*self.index.get(&id)?)
    }

    fn base_stmt(&self, id: u64) -> Option<Stmt> {
        let record = self.base_instance(id)?;
        self.src.with_range(record.span(), |rs| {
            stmt_from_slice(record, rs.base, rs.bytes)
        })
    }

    fn stmt(&self, id: u64) -> Option<Stmt> {
        if let Some(stmt) = self.changed.get(&id) {
            return Some(stmt.clone());
        }
        self.base_stmt(id)
    }

    fn base_kind_upper(&self, id: u64) -> Vec<u8> {
        let Some(record) = self.base_instance(id) else {
            return Vec::new();
        };
        let Some(part) = record.parts().first() else {
            return Vec::new();
        };
        let name_range = part.name();
        self.src.with_range(name_range.clone(), |rs| {
            slice_at(rs.base, rs.bytes, name_range).to_ascii_uppercase()
        })
    }

    fn snapshot_element(&self, element: ElementRef) -> ElementSnap {
        let level_entry = self.building.components.level.get(element);
        ElementSnap {
            identity: self.building.components.identity.get(element).cloned(),
            has_level: level_entry.is_some(),
            level_name: level_entry.and_then(|l| l.name.clone()),
            properties: self.building.components.properties.get(element).cloned(),
            quantities: self.building.components.quantities.get(element).cloned(),
            relations: self
                .building
                .element(element)
                .map(|e| e.source().relations.clone())
                .unwrap_or_default(),
        }
    }

    fn restore_element_snap(&mut self, element: ElementRef, snap: &ElementSnap) {
        match &snap.identity {
            Some(id) if *id != Identity::default() => {
                self.building
                    .components
                    .identity
                    .insert(element, id.clone());
            }
            _ => {
                self.building.components.identity.remove(element);
            }
        }
        if snap.has_level
            && let Some(mut level) = self.building.components.level.remove(element)
        {
            level.name = snap.level_name.clone();
            self.building.components.level.insert(element, level);
        }
        match &snap.properties {
            Some(props) if !props.sets.is_empty() => {
                self.building
                    .components
                    .properties
                    .insert(element, props.clone());
            }
            _ => {
                self.building.components.properties.remove(element);
            }
        }
        match &snap.quantities {
            Some(qtys) if !qtys.sets.is_empty() => {
                self.building
                    .components
                    .quantities
                    .insert(element, qtys.clone());
            }
            _ => {
                self.building.components.quantities.remove(element);
            }
        }
        if let Some(src) = self.building.source_mut(element) {
            src.relations = snap.relations.clone();
        }
    }

    fn apply_patch_forward(&mut self, patch: &IfcPatch) {
        for diff in &patch.stmt_diffs {
            for &old_t in &diff.old_refs {
                if let Some(set) = self.referrers.get_mut(&old_t) {
                    set.remove(&diff.id);
                    if set.is_empty() {
                        self.referrers.remove(&old_t);
                    }
                }
            }
            for &new_t in &diff.new_refs {
                self.referrers.entry(new_t).or_default().insert(diff.id);
            }
            self.changed.insert(diff.id, diff.after.clone());
        }
        self.added.extend_from_slice(&patch.added_ids);
        self.next = patch.after_next;
        if let Some((_, after_name)) = &patch.project_name {
            self.building.document.project_name = after_name.clone();
        }
        for elem_diff in &patch.element_diffs {
            self.restore_element_snap(elem_diff.element, &elem_diff.after);
        }
    }

    fn apply_patch_backward(&mut self, patch: &IfcPatch) {
        let remove_added = self.added.len().saturating_sub(patch.added_ids.len());
        self.added.truncate(remove_added);
        self.next = patch.before_next;
        for diff in patch.stmt_diffs.iter().rev() {
            for &new_t in &diff.new_refs {
                if let Some(set) = self.referrers.get_mut(&new_t) {
                    set.remove(&diff.id);
                    if set.is_empty() {
                        self.referrers.remove(&new_t);
                    }
                }
            }
            for &old_t in &diff.old_refs {
                self.referrers.entry(old_t).or_default().insert(diff.id);
            }
            match &diff.before {
                Some(prev) => {
                    self.changed.insert(diff.id, prev.clone());
                }
                None => {
                    self.changed.remove(&diff.id);
                }
            }
        }
        if let Some((before_name, _)) = &patch.project_name {
            self.building.document.project_name = before_name.clone();
        }
        for elem_diff in patch.element_diffs.iter().rev() {
            self.restore_element_snap(elem_diff.element, &elem_diff.before);
        }
    }
}

struct StagedOutcome {
    changed_count: usize,
    patch: IfcPatch,
}

struct OverlayDraft<'a> {
    live: &'a IfcLive,
    staged_changed: BTreeMap<u64, Stmt>,
    staged_added: Vec<u64>,
    next: u64,
    referrer_overrides: BTreeMap<u64, BTreeSet<u64>>,
    seed: &'a [u8],
    minted: u64,
    affected_identity: BTreeSet<ElementRef>,
    affected_defs: BTreeSet<ElementRef>,
    project_name_after: Option<Option<String>>,
}

impl<'a> OverlayDraft<'a> {
    fn new(live: &'a IfcLive, seed: &'a [u8]) -> Self {
        Self {
            live,
            staged_changed: BTreeMap::new(),
            staged_added: Vec::new(),
            next: live.next,
            referrer_overrides: BTreeMap::new(),
            seed,
            minted: 0,
            affected_identity: BTreeSet::new(),
            affected_defs: BTreeSet::new(),
            project_name_after: None,
        }
    }

    fn stmt(&self, id: u64) -> Option<Stmt> {
        if let Some(stmt) = self.staged_changed.get(&id) {
            return Some(stmt.clone());
        }
        self.live.stmt(id)
    }

    fn kind(&self, id: u64) -> Vec<u8> {
        if let Some(stmt) = self.staged_changed.get(&id) {
            return stmt.name.clone();
        }
        if let Some(stmt) = self.live.changed.get(&id) {
            return stmt.name.clone();
        }
        self.live.base_kind_upper(id)
    }

    fn referrers(&self, id: u64) -> Vec<u64> {
        if let Some(ov) = self.referrer_overrides.get(&id) {
            return ov.iter().copied().collect();
        }
        self.live
            .referrers
            .get(&id)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    fn ensure_referrer_set(&mut self, target: u64) -> &mut BTreeSet<u64> {
        let live = self.live;
        self.referrer_overrides
            .entry(target)
            .or_insert_with(|| live.referrers.get(&target).cloned().unwrap_or_default())
    }

    fn put(&mut self, id: u64, stmt: Stmt) {
        if let Some(old) = self.stmt(id) {
            for target in refs_in(&old.args) {
                self.ensure_referrer_set(target).remove(&id);
            }
        }
        for target in refs_in(&stmt.args) {
            self.ensure_referrer_set(target).insert(id);
        }
        self.staged_changed.insert(id, stmt);
    }

    fn add(&mut self, stmt: Stmt) -> u64 {
        let id = self.next;
        self.next = self.next.saturating_add(1);
        self.staged_added.push(id);
        self.put(id, stmt);
        id
    }

    fn global_id(&mut self) -> Item {
        let mut input = Vec::with_capacity(self.seed.len() + 8);
        input.extend_from_slice(self.seed);
        input.extend_from_slice(&self.minted.to_le_bytes());
        self.minted = self.minted.saturating_add(1);
        let Hash(bytes) = Hash::document(&input);
        text(&GlobalId::from_uuid(bytes).to_string())
    }

    fn root(&self, element: u64, edit: &str, cx: &mut Cx) -> Option<(Stmt, Option<Category>)> {
        let stmt = self.stmt(element);
        let found = stmt.as_ref().and_then(|stmt| {
            let global_id = text_of(stmt.arg(1)?)?;
            (global_id.len() == 22).then_some(())?;
            Some(category(&stmt.name, &self.live.schema))
        });
        match (stmt, found) {
            (Some(stmt), Some(cat)) => Some((stmt, cat)),
            (stmt, _) => {
                let kind = stmt.map(|s| s.name).unwrap_or_default();
                cx.report(unsupported(edit, &kind));
                None
            }
        }
    }

    fn attribute(
        &mut self,
        element: u64,
        attribute: RootAttribute,
        value: Option<&str>,
        cx: &mut Cx,
    ) -> Result<bool, Error> {
        let edit = match attribute {
            RootAttribute::Name => "name",
            RootAttribute::Description => "description",
        };
        let Some((mut stmt, exact_cat)) = self.root(element, edit, cx) else {
            return Ok(false);
        };
        let at = match attribute {
            RootAttribute::Name => 3,
            RootAttribute::Description => 4,
        };
        if stmt.arg(at).is_none() {
            cx.report(unsupported(edit, &stmt.name));
            return Ok(false);
        }
        let encoded = value.map_or_else(|| Item::Raw(b"$".to_vec()), text);
        cx.check(Kind::DecodedBytes, item_heap_bytes(&encoded) as u64)
            .map_err(|stop| stopped(cx, stop, None))?;
        let stmt_name = stmt.name.clone();
        stmt.set(at, encoded);
        self.put(element, stmt);

        if exact_cat.is_some()
            && let Some(&elem_ref) = self.live.elements_by_instance.get(&element)
        {
            self.affected_identity.insert(elem_ref);
            if attribute == RootAttribute::Name && self.live.first_project_id == Some(element) {
                self.project_name_after = Some(value.map(str::to_owned));
            }
        } else if attribute == RootAttribute::Name
            && (stmt_name == b"IFCPROPERTYSET" || stmt_name == b"IFCELEMENTQUANTITY")
        {
            for holder in self.referrers(element) {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| stopped(cx, stop, None))?;
                if let Some(&elem_ref) = self.live.elements_by_instance.get(&holder) {
                    self.affected_defs.insert(elem_ref);
                } else if self.kind(holder) == b"IFCRELDEFINESBYPROPERTIES"
                    && let Some(rel_stmt) = self.stmt(holder)
                {
                    for rel_elem in refs_of(rel_stmt.arg(5)) {
                        if let Some(&elem_ref) = self.live.elements_by_instance.get(&rel_elem) {
                            self.affected_defs.insert(elem_ref);
                        }
                    }
                }
            }
        }
        Ok(true)
    }

    fn property(
        &mut self,
        element: u64,
        set: &str,
        name: &str,
        value: &NewValue,
        cx: &mut Cx,
    ) -> Result<bool, Error> {
        let Some((owner, exact_cat)) = self.root(element, "property", cx) else {
            return Ok(false);
        };
        let cat = exact_cat.unwrap_or(Category::Element);
        let Some(written) = typed(value) else {
            cx.report(unsupported("property", &owner.name));
            return Ok(false);
        };
        cx.check(
            Kind::DecodedBytes,
            (written.len() + set.len() + name.len() + 64) as u64,
        )
        .map_err(|stop| stopped(cx, stop, None))?;

        let typed_object = matches!(cat, Category::Type | Category::Style);
        let found = if typed_object {
            self.type_set(element, &owner, set)
        } else {
            self.occurrence_set(element, set, cx)?
        };
        let target = match found {
            Some((holder, pset)) => {
                let exclusive = self.referrers(pset) == [holder]
                    && (typed_object
                        || self
                            .stmt(holder)
                            .map(|rel| refs_of(rel.arg(5)) == [element])
                            .unwrap_or_default());
                if exclusive {
                    pset
                } else {
                    self.copy_set(element, &owner, holder, pset, typed_object)
                }
            }
            None => self.new_set(element, &owner, set, typed_object),
        };
        let ok = self.set_property(target, name, written, &owner.name, cx)?;
        if ok
            && exact_cat.is_some()
            && let Some(&elem_ref) = self.live.elements_by_instance.get(&element)
        {
            self.affected_defs.insert(elem_ref);
        }
        Ok(ok)
    }

    fn occurrence_set(
        &self,
        element: u64,
        set: &str,
        cx: &mut Cx,
    ) -> Result<Option<(u64, u64)>, Error> {
        for rel in self.referrers(element) {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, None))?;
            if self.kind(rel) != b"IFCRELDEFINESBYPROPERTIES" {
                continue;
            }
            let Some(stmt) = self.stmt(rel) else {
                continue;
            };
            if !refs_of(stmt.arg(5)).contains(&element) {
                continue;
            }
            for pset in refs_of(stmt.arg(6)) {
                if self.is_set(pset, set) {
                    return Ok(Some((rel, pset)));
                }
            }
        }
        Ok(None)
    }

    fn type_set(&self, element: u64, owner: &Stmt, set: &str) -> Option<(u64, u64)> {
        refs_of(owner.arg(6))
            .into_iter()
            .find(|&pset| self.is_set(pset, set))
            .map(|pset| (element, pset))
    }

    fn is_set(&self, pset: u64, set: &str) -> bool {
        self.kind(pset) == b"IFCPROPERTYSET"
            && self
                .stmt(pset)
                .and_then(|stmt| text_of(stmt.arg(3)?))
                .is_some_and(|name| name == set)
    }

    fn copy_set(
        &mut self,
        element: u64,
        owner: &Stmt,
        holder: u64,
        pset: u64,
        typed_object: bool,
    ) -> u64 {
        let Some(mut copy) = self.stmt(pset) else {
            return pset;
        };
        copy.set(1, self.global_id());
        let copy = self.add(copy);
        if typed_object {
            if let Some(mut stmt) = self.stmt(holder) {
                let sets = swap(stmt.arg(6), pset, copy);
                stmt.set(6, sets);
                self.put(holder, stmt);
            }
            return copy;
        }
        let Some(mut rel) = self.stmt(holder) else {
            return copy;
        };
        if refs_of(rel.arg(5)) == [element] {
            let sets = swap(rel.arg(6), pset, copy);
            rel.set(6, sets);
            self.put(holder, rel);
        } else {
            let siblings = refs_of(rel.arg(5));
            for sib in siblings {
                if sib != element
                    && let Some(&sib_ref) = self.live.elements_by_instance.get(&sib)
                {
                    self.affected_defs.insert(sib_ref);
                }
            }
            let related = match rel.arg(5) {
                Some(Item::List(items)) => Item::List(
                    items
                        .iter()
                        .filter(|item| **item != Item::Ref(element))
                        .cloned()
                        .collect(),
                ),
                other => other.cloned().unwrap_or(Item::List(Vec::new())),
            };
            rel.set(5, related);
            self.put(holder, rel);
            let history = owner.arg(2).cloned().unwrap_or(Item::Raw(b"$".to_vec()));
            self.relate(element, copy, history);
        }
        copy
    }

    fn new_set(&mut self, element: u64, owner: &Stmt, set: &str, typed_object: bool) -> u64 {
        let history = owner.arg(2).cloned().unwrap_or(Item::Raw(b"$".to_vec()));
        let stmt = Stmt {
            name: b"IFCPROPERTYSET".to_vec(),
            args: vec![
                self.global_id(),
                history.clone(),
                text(set),
                Item::Raw(b"$".to_vec()),
                Item::List(Vec::new()),
            ],
        };
        let pset = self.add(stmt);
        if typed_object {
            if let Some(mut stmt) = self.stmt(element) {
                let sets = match stmt.arg(6) {
                    Some(Item::List(items)) => {
                        let mut items = items.clone();
                        items.push(Item::Ref(pset));
                        Item::List(items)
                    }
                    _ => Item::List(vec![Item::Ref(pset)]),
                };
                stmt.set(6, sets);
                self.put(element, stmt);
            }
        } else {
            self.relate(element, pset, history);
        }
        pset
    }

    fn relate(&mut self, element: u64, pset: u64, history: Item) {
        let stmt = Stmt {
            name: b"IFCRELDEFINESBYPROPERTIES".to_vec(),
            args: vec![
                self.global_id(),
                history,
                Item::Raw(b"$".to_vec()),
                Item::Raw(b"$".to_vec()),
                Item::List(vec![Item::Ref(element)]),
                Item::Ref(pset),
            ],
        };
        self.add(stmt);
    }

    fn set_property(
        &mut self,
        pset: u64,
        name: &str,
        value: Vec<u8>,
        owner_kind: &[u8],
        cx: &mut Cx,
    ) -> Result<bool, Error> {
        let Some(mut set) = self.stmt(pset) else {
            cx.report(unsupported("property", owner_kind));
            return Ok(false);
        };
        let mut found = None;
        for property in refs_of(set.arg(5)) {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, None))?;
            let named = self
                .stmt(property)
                .and_then(|stmt| text_of(stmt.arg(1)?))
                .is_some_and(|text| text == name);
            if named {
                found = Some(property);
                break;
            }
        }
        match found {
            Some(property) => {
                if self.kind(property) != b"IFCPROPERTYSINGLEVALUE" {
                    let kind = self.kind(property);
                    cx.report(unsupported("property", &kind));
                    return Ok(false);
                }
                let Some(mut stmt) = self.stmt(property) else {
                    return Ok(false);
                };
                stmt.set(3, Item::Raw(value));
                if self.referrers(property) == [pset] {
                    self.put(property, stmt);
                } else {
                    let copy = self.add(stmt);
                    let properties = swap(set.arg(5), property, copy);
                    set.set(5, properties);
                    self.put(pset, set);
                }
            }
            None => {
                let stmt = Stmt {
                    name: b"IFCPROPERTYSINGLEVALUE".to_vec(),
                    args: vec![
                        text(name),
                        Item::Raw(b"$".to_vec()),
                        Item::Raw(value),
                        Item::Raw(b"$".to_vec()),
                    ],
                };
                let property = self.add(stmt);
                let properties = match set.arg(5) {
                    Some(Item::List(items)) => {
                        let mut items = items.clone();
                        items.push(Item::Ref(property));
                        Item::List(items)
                    }
                    _ => Item::List(vec![Item::Ref(property)]),
                };
                set.set(5, properties);
                self.put(pset, set);
            }
        }
        Ok(true)
    }

    fn statement_order(&self, id: u64) -> usize {
        if let Some(&pos) = self.live.index.get(&id) {
            return pos;
        }
        if let Some(pos) = self.live.added.iter().position(|&a| a == id) {
            return self.live.records.len() + pos;
        }
        if let Some(pos) = self.staged_added.iter().position(|&a| a == id) {
            return self.live.records.len() + self.live.added.len() + pos;
        }
        usize::MAX
    }

    fn with_parsed_or_base<R>(
        &self,
        id: u64,
        f: impl FnOnce(&Record, usize, &[u8]) -> R,
    ) -> Option<R> {
        if let Some(stmt) = self
            .staged_changed
            .get(&id)
            .or_else(|| self.live.changed.get(&id))
        {
            return parse_single_stmt(id, stmt, f);
        }
        let rec = self.live.base_instance(id)?;
        Some(
            self.live
                .src
                .with_range(rec.span(), |rs| f(rec, rs.base, rs.bytes)),
        )
    }

    fn read_definition_set(
        &self,
        set_id: u64,
        cx: &mut Cx,
    ) -> Result<Option<LiveDefinition>, Error> {
        cx.check(Kind::Work, 1)
            .map_err(|stop| stopped(cx, stop, None))?;
        let kind = self.kind(set_id);
        if kind == b"IFCPROPERTYSET" {
            let Some((name, prop_ids)) = self.with_parsed_or_base(set_id, |rec, base, slice| {
                let n = slice_text(arg(rec, 3), base, slice).unwrap_or_default();
                let ids = refs(arg(rec, 5));
                (n, ids)
            }) else {
                return Ok(None);
            };
            let mut properties = Vec::with_capacity(prop_ids.len());
            for prop_id in prop_ids {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| stopped(cx, stop, None))?;
                if self.kind(prop_id) != b"IFCPROPERTYSINGLEVALUE" {
                    continue;
                }
                if let Some(prop) = self.read_single_property(prop_id) {
                    properties.push(prop);
                }
            }
            Ok(Some(LiveDefinition::Properties(PropertySet {
                name,
                properties,
            })))
        } else if kind == b"IFCELEMENTQUANTITY" {
            let Some((name, method, qty_ids)) =
                self.with_parsed_or_base(set_id, |rec, base, slice| {
                    let n = slice_text(arg(rec, 3), base, slice).unwrap_or_default();
                    let m = slice_text(arg(rec, 5), base, slice);
                    let ids = refs(arg(rec, 6));
                    (n, m, ids)
                })
            else {
                return Ok(None);
            };
            let mut quantities = Vec::with_capacity(qty_ids.len());
            for qty_id in qty_ids {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| stopped(cx, stop, None))?;
                if let Some(qty) = self.read_single_quantity(qty_id) {
                    quantities.push(qty);
                }
            }
            Ok(Some(LiveDefinition::Quantities(QuantitySet {
                name,
                method,
                quantities,
            })))
        } else {
            Ok(None)
        }
    }

    fn read_single_property(&self, prop_id: u64) -> Option<Property> {
        self.with_parsed_or_base(prop_id, |rec, base, slice| {
            let name = slice_text(arg(rec, 1), base, slice).unwrap_or_default();
            let value = match arg(rec, 3) {
                None | Some(Value::Null) => None,
                Some(v) => self.slice_typed_value(v, base, slice),
            };
            let unit = match reference(arg(rec, 4)) {
                Some(unit_id) => self.live.units.get(&unit_id).and_then(Clone::clone),
                None => value
                    .as_ref()
                    .and_then(|v| v.type_name.as_deref())
                    .and_then(|tn| self.doc_measure_unit(tn)),
            };
            Property { name, value, unit }
        })
    }

    fn read_single_quantity(&self, qty_id: u64) -> Option<Quantity> {
        let kind = self.kind(qty_id);
        let (_, quantity_kind, unit_kind) = QUANTITIES
            .iter()
            .find(|(name, ..)| name.as_bytes() == kind.as_slice())?;
        self.with_parsed_or_base(qty_id, |rec, base, slice| {
            let value = slice_real(arg(rec, 4), base, slice)?;
            let unit = match reference(arg(rec, 3)) {
                Some(unit_id) => self.live.units.get(&unit_id).and_then(Clone::clone),
                None => unit_kind.and_then(|uk| self.doc_unit_by_kind(uk)),
            };
            Some(Quantity {
                name: slice_text(arg(rec, 1), base, slice).unwrap_or_default(),
                kind: *quantity_kind,
                value: Measured { value, unit },
                formula: match self.live.version {
                    Version::Ifc2x3 => None,
                    _ => slice_text(arg(rec, 5), base, slice),
                },
            })
        })
        .flatten()
    }

    fn doc_unit_by_kind(&self, kind: &str) -> Option<Unit> {
        self.live
            .building
            .document
            .units
            .iter()
            .find(|u| u.kind == kind)
            .cloned()
    }

    fn doc_measure_unit(&self, type_name: &str) -> Option<Unit> {
        let kind = MEASURES
            .iter()
            .find(|(name, _)| *name == type_name)
            .and_then(|(_, k)| *k)?;
        self.doc_unit_by_kind(kind)
    }

    fn slice_typed_value(&self, value: &Value, base: usize, slice: &[u8]) -> Option<TypedValue> {
        match value {
            Value::Typed(part) => {
                let written =
                    String::from_utf8_lossy(slice_at(base, slice, part.name())).into_owned();
                let type_name = MEASURES
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(&written))
                    .map_or(written, |(name, _)| (*name).to_owned());
                let [inner] = part.args() else {
                    return None;
                };
                let lit = self.slice_literal(&type_name, inner, base, slice)?;
                Some(TypedValue {
                    type_name: Some(type_name),
                    value: lit,
                })
            }
            other => Some(TypedValue {
                type_name: None,
                value: self.slice_literal("", other, base, slice)?,
            }),
        }
    }

    fn slice_literal(
        &self,
        type_name: &str,
        value: &Value,
        base: usize,
        slice: &[u8],
    ) -> Option<Literal> {
        match value {
            Value::List(values) => values
                .iter()
                .map(|v| match v {
                    Value::List(_) => None,
                    v => self.slice_literal(type_name, v, base, slice),
                })
                .collect::<Option<Vec<_>>>()
                .map(Literal::List),
            Value::Number(range) => {
                let text = core::str::from_utf8(slice_at(base, slice, range.clone())).ok()?;
                if text.contains(['.', 'E', 'e']) {
                    text.parse().ok().map(Literal::Real)
                } else {
                    text.parse().ok().map(Literal::Integer)
                }
            }
            Value::Text { .. } => slice_text(Some(value), base, slice).map(Literal::Text),
            Value::Enum(range) => {
                let name =
                    String::from_utf8_lossy(slice_at(base, slice, range.clone())).into_owned();
                match (type_name, name.as_str()) {
                    ("IfcBoolean", "T") => Some(Literal::Boolean(true)),
                    ("IfcBoolean", "F") => Some(Literal::Boolean(false)),
                    ("IfcLogical", "T") => Some(Literal::Logical(Some(true))),
                    ("IfcLogical", "F") => Some(Literal::Logical(Some(false))),
                    ("IfcLogical", "U") => Some(Literal::Logical(None)),
                    _ => Some(Literal::Enumeration(name)),
                }
            }
            Value::Ref(id) => self
                .live
                .elements_by_instance
                .get(id)
                .copied()
                .map(Literal::Element),
            _ => None,
        }
    }

    fn compute_defs_and_relations(
        &self,
        elem_ref: ElementRef,
        inst_id: u64,
        cx: &mut Cx,
    ) -> Result<ComputedElementDefs, Error> {
        let mut rel_ids: Vec<(usize, u64)> = self
            .referrers(inst_id)
            .into_iter()
            .filter(|&rel| self.kind(rel) == b"IFCRELDEFINESBYPROPERTIES")
            .map(|rel| (self.statement_order(rel), rel))
            .collect();
        rel_ids.sort_by_key(|&(ord, rel)| (ord, rel));

        let mut prop_sets = Vec::new();
        let mut qty_sets = Vec::new();
        let mut prop_relations: Vec<(usize, RelationSource)> = Vec::new();
        let mut def_cache: BTreeMap<u64, Option<LiveDefinition>> = BTreeMap::new();

        for (ord, rel_id) in rel_ids {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, None))?;
            let Some(rel_stmt) = self.stmt(rel_id) else {
                continue;
            };
            let positions =
                related_positions_for(rel_stmt.arg(5), inst_id, &self.live.elements_by_instance);
            if positions.is_empty() {
                continue;
            }
            let set_ids: Vec<u64> = refs_of(rel_stmt.arg(6))
                .into_iter()
                .filter(|&s| {
                    let k = self.kind(s);
                    k == b"IFCPROPERTYSET" || k == b"IFCELEMENTQUANTITY"
                })
                .collect();
            let rel_gid = rel_stmt.arg(1).and_then(text_of);

            for pos in positions {
                for &set_id in &set_ids {
                    let def = if let Some(cached) = def_cache.get(&set_id) {
                        cached.clone()
                    } else {
                        let resolved = self.read_definition_set(set_id, cx)?;
                        def_cache.insert(set_id, resolved.clone());
                        resolved
                    };
                    match def {
                        Some(LiveDefinition::Properties(ps)) => prop_sets.push(ps),
                        Some(LiveDefinition::Quantities(qs)) => qty_sets.push(qs),
                        None => {}
                    }
                }
                prop_relations.push((
                    ord,
                    RelationSource {
                        instance: rel_id,
                        global_id: rel_gid.clone(),
                        position: Some(pos),
                    },
                ));
            }
        }

        if let Some(owner_stmt) = self.stmt(inst_id)
            && matches!(
                category(&owner_stmt.name, &self.live.schema),
                Some(Category::Type | Category::Style)
            )
        {
            for set_id in refs_of(owner_stmt.arg(6)) {
                let def = if let Some(cached) = def_cache.get(&set_id) {
                    cached.clone()
                } else {
                    let resolved = self.read_definition_set(set_id, cx)?;
                    def_cache.insert(set_id, resolved.clone());
                    resolved
                };
                match def {
                    Some(LiveDefinition::Properties(ps)) => prop_sets.push(ps),
                    Some(LiveDefinition::Quantities(qs)) => qty_sets.push(qs),
                    None => {}
                }
            }
        }

        let base_rels = self
            .live
            .base_non_prop_relations
            .get(elem_ref.0 as usize)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let mut merged = Vec::with_capacity(base_rels.len() + prop_relations.len());
        let mut bi = 0usize;
        let mut pi = 0usize;
        while bi < base_rels.len() && pi < prop_relations.len() {
            let (Some(b_entry), Some(p_entry)) = (base_rels.get(bi), prop_relations.get(pi)) else {
                break;
            };
            if b_entry.0 <= p_entry.0 {
                merged.push(b_entry.1.clone());
                bi += 1;
            } else {
                merged.push(p_entry.1.clone());
                pi += 1;
            }
        }
        while let Some(b_entry) = base_rels.get(bi) {
            merged.push(b_entry.1.clone());
            bi += 1;
        }
        while let Some(p_entry) = prop_relations.get(pi) {
            merged.push(p_entry.1.clone());
            pi += 1;
        }

        let props = (!prop_sets.is_empty()).then_some(Properties { sets: prop_sets });
        let qtys = (!qty_sets.is_empty()).then_some(Quantities { sets: qty_sets });
        Ok((props, qtys, merged))
    }

    fn finish(self, cx: &mut Cx) -> Result<StagedOutcome, Error> {
        let changed_count = self.staged_changed.len();
        let mut all_affected: BTreeSet<ElementRef> = self.affected_identity.clone();
        all_affected.extend(self.affected_defs.iter().copied());

        let mut element_diffs = Vec::with_capacity(all_affected.len());
        for elem_ref in all_affected {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, None))?;
            let before = self.live.snapshot_element(elem_ref);
            let mut after = before.clone();
            let Some(inst_id) = self
                .live
                .building
                .element(elem_ref)
                .and_then(|e| e.source().instance)
            else {
                continue;
            };
            if self.affected_identity.contains(&elem_ref)
                && let Some(stmt) = self.stmt(inst_id)
            {
                let mut ident = after.identity.unwrap_or_default();
                ident.name = stmt.arg(3).and_then(text_of);
                ident.description = stmt.arg(4).and_then(text_of);
                if after.has_level {
                    after.level_name = ident.name.clone();
                }
                after.identity = (ident != Identity::default()).then_some(ident);
            }
            if self.affected_defs.contains(&elem_ref) {
                let (props, qtys, relations) =
                    self.compute_defs_and_relations(elem_ref, inst_id, cx)?;
                after.properties = props;
                after.quantities = qtys;
                after.relations = relations;
            }
            cx.check(Kind::DecodedBytes, after.heap_bytes() as u64)
                .map_err(|stop| stopped(cx, stop, None))?;
            element_diffs.push(ElementSnapDiff {
                element: elem_ref,
                before,
                after,
            });
        }

        let mut stmt_diffs = Vec::with_capacity(self.staged_changed.len());
        for (id, after) in self.staged_changed {
            let before = self.live.changed.get(&id).cloned();
            let old_refs = self
                .live
                .stmt(id)
                .map(|s| refs_in(&s.args))
                .unwrap_or_default();
            let new_refs = refs_in(&after.args);
            stmt_diffs.push(StmtDiff {
                id,
                before,
                after,
                old_refs,
                new_refs,
            });
        }

        let project_name = self
            .project_name_after
            .map(|after_name| (self.live.building.document.project_name.clone(), after_name));

        Ok(StagedOutcome {
            changed_count,
            patch: IfcPatch {
                stmt_diffs,
                added_ids: self.staged_added,
                before_next: self.live.next,
                after_next: self.next,
                project_name,
                element_diffs,
            },
        })
    }
}

type ComputedElementDefs = (Option<Properties>, Option<Quantities>, Vec<RelationSource>);

#[derive(Clone)]
enum LiveDefinition {
    Properties(PropertySet),
    Quantities(QuantitySet),
}

fn related_positions_for(
    arg5: Option<&Item>,
    target_inst: u64,
    elements: &BTreeMap<u64, ElementRef>,
) -> Vec<u32> {
    if !elements.contains_key(&target_inst) {
        return Vec::new();
    }
    match arg5 {
        Some(Item::List(items)) => items
            .iter()
            .enumerate()
            .filter_map(|(at, item)| match item {
                Item::Ref(id) if *id == target_inst => u32::try_from(at).ok(),
                _ => None,
            })
            .collect(),
        Some(Item::Ref(id)) if *id == target_inst => vec![0],
        _ => Vec::new(),
    }
}

fn slice_at(base: usize, slice: &[u8], range: Range<usize>) -> &[u8] {
    let Some(start) = range.start.checked_sub(base) else {
        return &[];
    };
    let Some(end) = range.end.checked_sub(base) else {
        return &[];
    };
    slice.get(start..end).unwrap_or_default()
}

fn slice_text(value: Option<&Value>, base: usize, slice: &[u8]) -> Option<String> {
    let Some(Value::Text { raw, .. }) = value else {
        return None;
    };
    Some(decode(slice_at(base, slice, raw.clone())).0)
}

fn slice_real(value: Option<&Value>, base: usize, slice: &[u8]) -> Option<f64> {
    match value? {
        Value::Number(range) => core::str::from_utf8(slice_at(base, slice, range.clone()))
            .ok()?
            .parse()
            .ok(),
        Value::Typed(part) => match part.args() {
            [only] => slice_real(Some(only), base, slice),
            _ => None,
        },
        _ => None,
    }
}

fn parse_single_stmt<R>(
    id: u64,
    stmt: &Stmt,
    f: impl FnOnce(&Record, usize, &[u8]) -> R,
) -> Option<R> {
    const STEP_PREFIX: &[u8] = b"ISO-10303-21;HEADER;ENDSEC;DATA;";
    const STEP_SUFFIX: &[u8] = b"ENDSEC;END-ISO-10303-21;";

    let mut buf = Vec::with_capacity(STEP_PREFIX.len() + 128 + STEP_SUFFIX.len());
    buf.extend_from_slice(STEP_PREFIX);
    write_stmt(id, stmt, &mut buf);
    buf.extend_from_slice(STEP_SUFFIX);

    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut qcx = Cx::new(&mut budget, &mut diag, &cancel);
    let parsed = zumen_step::read(&buf, &DIALECT, &mut qcx).ok()?;
    let rec = parsed.records.iter().find(|r| r.id() == Some(id))?;
    Some(f(rec, 0, &buf))
}

fn collect_literal_element_refs(lit: &Literal, out: &mut BTreeSet<RowId>) {
    match lit {
        Literal::Element(e) => {
            out.insert(RowId(e.0));
        }
        Literal::List(items) => {
            for item in items {
                collect_literal_element_refs(item, out);
            }
        }
        _ => {}
    }
}

fn collect_property_element_refs(props: Option<&Properties>, out: &mut BTreeSet<RowId>) {
    let Some(props) = props else {
        return;
    };
    for set in &props.sets {
        for prop in &set.properties {
            if let Some(tv) = &prop.value {
                collect_literal_element_refs(&tv.value, out);
            }
        }
    }
}

fn build_ifc_dep_graph(
    building: &Building,
    cx: &mut Cx,
) -> Result<(Vec<BTreeSet<RowId>>, DepGraph), Error> {
    let elem_count = building.elements().len();
    let mut base_sources: Vec<BTreeSet<RowId>> = vec![BTreeSet::new(); elem_count];
    for idx in 0..elem_count {
        cx.check(Kind::Work, 1)
            .map_err(|stop| stopped(cx, stop, None))?;
        let Ok(raw_idx) = u32::try_from(idx) else {
            continue;
        };
        let elem_ref = ElementRef(raw_idx);
        let row = RowId(raw_idx);
        if let Some(parent) = building.components.parent.get(elem_ref) {
            if let Some(m) = &parent.of
                && let Some(set) = base_sources.get_mut(idx)
            {
                set.insert(RowId(m.target.0));
            }
            if let Some(set) = base_sources.get_mut(idx) {
                for &r in &parent.referenced_in {
                    set.insert(RowId(r.0));
                }
            }
        }
        if let Some(pl) = building.components.placement.get(elem_ref)
            && let Some(rel) = pl.relative_to
            && let Some(set) = base_sources.get_mut(idx)
        {
            set.insert(RowId(rel.0));
        }
        if let Some(tp) = building.components.r#type.get(elem_ref)
            && let Some(set) = base_sources.get_mut(idx)
        {
            set.insert(RowId(tp.target.0));
        }
        if let Some(mat) = building.components.material.get(elem_ref)
            && let Some(set) = base_sources.get_mut(idx)
        {
            for part in &mat.parts {
                if let Some(m) = part.material {
                    set.insert(RowId(m.0));
                }
            }
        }
        if let Some(op) = building.components.opening.get(elem_ref) {
            match *op {
                Opening::Voids(host) => {
                    if let Some(host_set) = base_sources.get_mut(host.0 as usize) {
                        host_set.insert(row);
                    }
                }
                Opening::Fills(opening_elem) => {
                    if let Some(set) = base_sources.get_mut(idx) {
                        set.insert(RowId(opening_elem.0));
                    }
                }
            }
        }
        if let Some(grp) = building.components.group.get(elem_ref)
            && let Some(set) = base_sources.get_mut(idx)
        {
            for &g in &grp.groups {
                set.insert(RowId(g.0));
            }
        }
        if let Some(Grid::Grid { axes }) = building.components.grid.get(elem_ref)
            && let Some(set) = base_sources.get_mut(idx)
        {
            for &(_, axis) in axes {
                set.insert(RowId(axis.0));
            }
        }
        if let Some(zone) = building.components.zone.get(elem_ref)
            && let Some(set) = base_sources.get_mut(idx)
        {
            for &m in &zone.members {
                set.insert(RowId(m.0));
            }
        }
    }

    let mut dep = DepGraph::new();
    for (idx, base) in base_sources.iter().enumerate() {
        cx.check(Kind::Work, 1)
            .map_err(|stop| stopped(cx, stop, None))?;
        let Ok(raw_idx) = u32::try_from(idx) else {
            continue;
        };
        let elem_ref = ElementRef(raw_idx);
        let mut combined = base.clone();
        collect_property_element_refs(building.components.properties.get(elem_ref), &mut combined);
        if !combined.is_empty() {
            dep.set_sources(RowId(raw_idx), combined);
        }
    }
    Ok((base_sources, dep))
}

/// [`IfcLive`] 上の建物モデルを全文書き出しや再解析なしで IDS 照合する [`Lookup`]
#[cfg(feature = "ids")]
#[derive(Clone, Debug)]
pub struct LiveIdsLookup<'a> {
    live: &'a IfcLive,
    layouts: Option<RefCell<Layouts<'a>>>,
}

#[cfg(feature = "ids")]
impl<'a> LiveIdsLookup<'a> {
    /// 常駐文書エンジンと EXPRESS スキーマから照合器を作る
    pub fn new(live: &'a IfcLive, schema: Option<&'a Express>) -> Self {
        Self {
            live,
            layouts: schema.map(|s| RefCell::new(Layouts::new(s))),
        }
    }
}

#[cfg(feature = "ids")]
impl Lookup for LiveIdsLookup<'_> {
    fn missing(&self, what: Missing) -> bool {
        matches!(what, Missing::MaterialNames | Missing::PropertyKinds)
    }

    fn attributes(
        &self,
        building: &Building,
        element: ElementRef,
        cx: &mut Cx,
    ) -> Result<Option<Vec<(String, AttributeValue)>>, Stop> {
        let Some(layouts) = self.layouts.as_ref() else {
            return Ok(None);
        };
        let Some(instance) = building.element(element).and_then(|e| e.source().instance) else {
            return Ok(None);
        };
        if let Some(stmt) = self.live.changed.get(&instance) {
            return parse_single_stmt(instance, stmt, |record, base, slice| {
                attributes_from_record_slice(layouts, building, record, base, slice, cx)
            })
            .unwrap_or(Ok(None));
        }
        let Some(record) = self.live.base_instance(instance) else {
            return Ok(None);
        };
        self.live.src.with_range(record.span(), |rs| {
            attributes_from_record_slice(layouts, building, record, rs.base, rs.bytes, cx)
        })
    }
}
