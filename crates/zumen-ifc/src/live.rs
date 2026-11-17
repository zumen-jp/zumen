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

use zumen_base::{Diagnostic, Diagnostics, Kind, codes};
use zumen_building::{
    Building, DetailedProperty, DetailedPropertySet, ElementRef, GlobalId, Grid, Hash, Identity,
    Literal, Measured, Opening, Properties, Property, PropertyDetailValue, PropertyDetails,
    PropertySet, Quantities, Quantity, QuantitySet, RelationSource, TypedValue, Unit,
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
    Category, QUANTITIES, Version, arg, category, decode, lift_with_units, measure_types,
    predefined_property_fields, reference, refs,
};
use crate::edit::{
    Edit, Item, NewValue, RootAttribute, Stmt, refs_in, refs_of, stmt_from_slice, swap, text,
    text_of, typed, unsupported, write_stmt,
};
#[cfg(feature = "ids")]
use crate::express::{Express, Layouts};
#[cfg(feature = "ids")]
use crate::ids::attributes_from_record_slice;
use crate::{
    DIALECT, Ifc, IfcFile, IfcZip, IfcZipFile, Record, TextEncoding, Value, at_bytes, stopped,
};

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
    const INLINE_BYTES: usize = {
        assert!(core::mem::size_of::<Self>() <= 32);
        32
    };

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
    before_parsed: Option<Arc<ParsedStmt>>,
    after_parsed: Arc<ParsedStmt>,
    old_refs: Vec<u64>,
    new_refs: Vec<u64>,
}

impl StmtDiff {
    fn heap_bytes(&self) -> usize {
        // 予算にも使うため、可変幅の値は 64 bit の論理バイト数で見積もる。
        stmt_heap_bytes(&self.after)
            + self
                .before_parsed
                .as_ref()
                .map_or(0, |parsed| parsed.heap_bytes)
            + self.after_parsed.heap_bytes
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
            items.capacity() * 32 + items.iter().map(item_heap_bytes).sum::<usize>()
        }
    }
}

#[derive(Clone, Debug)]
struct ElementSnap {
    copy_bytes: usize,
    identity: Option<Identity>,
    has_level: bool,
    level_name: Option<String>,
    properties: Option<Properties>,
    property_details: Option<PropertyDetails>,
    quantities: Option<Quantities>,
    relations: Vec<RelationSource>,
}

impl ElementSnap {
    fn heap_bytes(&self) -> usize {
        self.copy_bytes
    }

    fn check_copy(&self, cx: &mut Cx) -> Result<usize, Error> {
        ElementSnapRef {
            identity: self.identity.as_ref(),
            level_name: self.level_name.as_deref(),
            properties: self.properties.as_ref(),
            property_details: self.property_details.as_ref(),
            quantities: self.quantities.as_ref(),
            relations: &self.relations,
        }
        .check_copy(cx)
    }
}

struct ElementSnapRef<'a> {
    identity: Option<&'a Identity>,
    level_name: Option<&'a str>,
    properties: Option<&'a Properties>,
    property_details: Option<&'a PropertyDetails>,
    quantities: Option<&'a Quantities>,
    relations: &'a [RelationSource],
}

impl ElementSnapRef<'_> {
    fn check_copy(&self, cx: &mut Cx) -> Result<usize, Error> {
        let mut copy = ModelCopy::new(cx);
        copy.add(512)?;
        if let Some(id) = self.identity {
            for text in [&id.name, &id.description, &id.tag, &id.long_name] {
                copy.text(text.as_deref())?;
            }
        }
        copy.text(self.level_name)?;
        if let Some(props) = self.properties {
            for set in &props.sets {
                copy.property_set(set)?;
            }
        }
        if let Some(details) = self.property_details {
            for set in &details.sets {
                copy.detail_set(set)?;
            }
        }
        if let Some(qtys) = self.quantities {
            for set in &qtys.sets {
                copy.quantity_set(set)?;
            }
        }
        for relation in self.relations {
            copy.add(64)?;
            copy.text(relation.global_id.as_deref())?;
        }
        Ok(copy.bytes)
    }
}

/// モデルのコピー前に、容量やポインター幅によらない論理サイズと作業量を数える。
/// 履歴には検査済みのサイズを持ち、履歴の集計で値の木を走査し直さない。
struct ModelCopy<'a, 'b> {
    cx: &'a mut Cx<'b>,
    bytes: usize,
}

impl<'a, 'b> ModelCopy<'a, 'b> {
    fn new(cx: &'a mut Cx<'b>) -> Self {
        Self { cx, bytes: 0 }
    }

    fn add(&mut self, bytes: usize) -> Result<(), Error> {
        self.cx
            .check(Kind::Work, 1 + bytes.div_ceil(1024) as u64)
            .map_err(|stop| stopped(self.cx, stop, None))?;
        self.cx
            .check(Kind::DecodedBytes, bytes as u64)
            .map_err(|stop| stopped(self.cx, stop, None))?;
        self.bytes = self.bytes.saturating_add(bytes);
        Ok(())
    }

    fn text(&mut self, text: Option<&str>) -> Result<(), Error> {
        if let Some(text) = text {
            self.add(text.len())?;
        }
        Ok(())
    }

    fn unit(&mut self, unit: Option<&Unit>) -> Result<(), Error> {
        if let Some(unit) = unit {
            self.add(128)?;
            self.text(Some(&unit.kind))?;
            self.text(unit.prefix.as_deref())?;
            self.text(Some(&unit.name))?;
        }
        Ok(())
    }

    fn typed(&mut self, value: &TypedValue) -> Result<(), Error> {
        self.add(128)?;
        self.text(value.type_name.as_deref())?;
        self.literal(&value.value)
    }

    fn literal(&mut self, value: &Literal) -> Result<(), Error> {
        self.add(64)?;
        match value {
            Literal::Text(text) | Literal::Enumeration(text) => self.text(Some(text)),
            Literal::List(values) => {
                self.cx
                    .enter()
                    .map_err(|stop| stopped(self.cx, stop, None))?;
                let result = values.iter().try_for_each(|value| self.literal(value));
                self.cx.leave();
                result
            }
            _ => Ok(()),
        }
    }

    fn property_set(&mut self, set: &PropertySet) -> Result<(), Error> {
        self.add(128)?;
        self.text(Some(&set.name))?;
        for property in &set.properties {
            self.add(256)?;
            self.text(Some(&property.name))?;
            if let Some(value) = &property.value {
                self.typed(value)?;
            }
            self.unit(property.unit.as_ref())?;
        }
        Ok(())
    }

    fn detail_set(&mut self, set: &DetailedPropertySet) -> Result<(), Error> {
        self.add(128)?;
        self.text(Some(&set.name))?;
        self.text(set.description.as_deref())?;
        for property in &set.properties {
            self.detail_property(property)?;
        }
        Ok(())
    }

    fn detail_property(&mut self, property: &DetailedProperty) -> Result<(), Error> {
        self.add(512)?;
        self.text(Some(&property.name))?;
        self.text(property.description.as_deref())?;
        match &property.value {
            PropertyDetailValue::Single { value, unit } => {
                if let Some(value) = value {
                    self.typed(value)?;
                }
                self.unit(unit.as_ref())?;
            }
            PropertyDetailValue::List { values, unit } => {
                for value in values.iter().flatten() {
                    self.typed(value)?;
                }
                self.unit(unit.as_ref())?;
            }
            PropertyDetailValue::Enumerated { values, choices } => {
                for value in values.iter().flatten() {
                    self.typed(value)?;
                }
                if let Some(choices) = choices {
                    self.add(256)?;
                    self.text(Some(&choices.name))?;
                    for value in &choices.values {
                        self.typed(value)?;
                    }
                    self.unit(choices.unit.as_ref())?;
                }
            }
            PropertyDetailValue::Bounded {
                lower,
                upper,
                set_point,
                unit,
            } => {
                for value in [lower, upper, set_point].into_iter().flatten() {
                    self.typed(value)?;
                }
                self.unit(unit.as_ref())?;
            }
            PropertyDetailValue::Complex { usage_name } => self.text(Some(usage_name))?,
            PropertyDetailValue::Unsupported { kind } => self.text(Some(kind))?,
        }
        Ok(())
    }

    fn quantity_set(&mut self, set: &QuantitySet) -> Result<(), Error> {
        self.add(128)?;
        self.text(Some(&set.name))?;
        self.text(set.method.as_deref())?;
        for quantity in &set.quantities {
            self.add(256)?;
            self.text(Some(&quantity.name))?;
            self.text(quantity.formula.as_deref())?;
            self.unit(quantity.value.unit.as_ref())?;
        }
        Ok(())
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

impl IfcPatch {
    fn check_model_restore(&self, forward: bool, cx: &mut Cx) -> Result<(), Error> {
        // 履歴や版を変える前に、後処理で戻すモデルのコピーを検査する。
        for diff in &self.element_diffs {
            if forward { &diff.after } else { &diff.before }.check_copy(cx)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct SharedIfcPatch(Arc<IfcPatch>);

impl PatchPayload for SharedIfcPatch {
    const INLINE_BYTES: usize = {
        assert!(core::mem::size_of::<Self>() <= 8);
        8
    };

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
    /// 開いたファイルの文字コードの指定
    text_encoding: TextEncoding,
    /// 開いたときに、エスケープの 8 ビット値を CP932 として読んだか。セットとプロパティの名前を、
    /// 今までの経路（[`crate::apply`]）と同じ読み方で比べるのに使う
    shift_jis: bool,

    /// 書き直したインスタンスおよび追加したインスタンス
    changed: BTreeMap<u64, Stmt>,
    /// 確定前に共有予算で読んだ変更文。属性取得では解析し直さない。
    parsed_changed: BTreeMap<u64, Arc<ParsedStmt>>,
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
        let text_encoding = file.text_encoding();
        let shift_jis = file.shift_jis();
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
            text_encoding,
            shift_jis,
            changed: BTreeMap::new(),
            parsed_changed: BTreeMap::new(),
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
        cx.check(Kind::Work, (changed.len() as u64).max(1))
            .map_err(|stop| stopped(cx, stop, None))?;
        cx.check(Kind::DecodedBytes, (changed.len() as u64).saturating_mul(4))
            .map_err(|stop| stopped(cx, stop, None))?;
        let roots: Vec<RowId> = changed.iter().map(|e| RowId(e.0)).collect();
        let rows = self
            .dep
            .affected_rows_with_cx(&roots, cx)
            .map_err(|stop| stopped(cx, stop, None))?;
        cx.check(Kind::Work, rows.len() as u64)
            .map_err(|stop| stopped(cx, stop, None))?;
        cx.check(Kind::DecodedBytes, (rows.len() as u64).saturating_mul(4))
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

    /// 開いたファイルの文字コードの指定（[`IfcFile::text_encoding`]）
    #[inline]
    pub fn text_encoding(&self) -> TextEncoding {
        self.text_encoding
    }

    /// エスケープの 8 ビット値を CP932 として読むか。指定が [`TextEncoding::Auto`] なら、開いたファイルの
    /// 手がかりで決まる。変更で手がかりの文字列を書き換えても、この値は変えない（今までの経路は、書き戻したファイルを
    /// 読み直すので、手がかりを読み直す）
    #[inline]
    pub fn reads_shift_jis(&self) -> bool {
        self.shift_jis
    }

    /// `file` から開いたときの [`Self::reads_shift_jis`]。文書エンジンを作る前に、読み方を確かめるのに使う
    #[inline]
    pub fn file_reads_shift_jis(file: &IfcFile) -> bool {
        file.shift_jis()
    }

    /// 文字列の値の中身を、開いたファイルの文字コードで読む。今までの経路（[`crate::apply`]）が、セットと
    /// プロパティの名前を比べる読み方と同じ
    fn text_value(&self, item: &Item) -> Option<String> {
        let Item::Raw(bytes) = item else {
            return None;
        };
        let inner = bytes.strip_prefix(b"'")?.strip_suffix(b"'")?;
        Some(crate::text::decode(inner, self.shift_jis).text)
    }

    /// 取り消し・やり直しの履歴を空にする。常駐する状態は変えない
    pub fn clear_history(&mut self) {
        self.history.clear();
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
        cx.check(
            Kind::DecodedBytes,
            (shared_patch.0.element_diffs.len() as u64).saturating_mul(32),
        )
        .map_err(|stop| stopped(cx, stop, None))?;
        let mut updated_rows: Vec<(ElementRef, Option<Vec<RowId>>)> =
            Vec::with_capacity(shared_patch.0.element_diffs.len());
        for diff in &shared_patch.0.element_diffs {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, None))?;
            let new_srcs = if diff.before.properties != diff.after.properties
                || diff.before.property_details != diff.after.property_details
            {
                let mut combined =
                    copy_dep_sources(self.base_dep_sources.get(diff.element.0 as usize), cx)?;
                collect_property_element_refs(diff.after.properties.as_ref(), &mut combined, cx)?;
                collect_detail_element_refs(
                    diff.after.property_details.as_ref(),
                    &mut combined,
                    cx,
                )?;
                cx.check(Kind::Work, combined.len() as u64)
                    .map_err(|stop| stopped(cx, stop, None))?;
                cx.check(
                    Kind::DecodedBytes,
                    (combined.len() as u64).saturating_mul(4),
                )
                .map_err(|stop| stopped(cx, stop, None))?;
                Some(combined.into_iter().collect())
            } else {
                None
            };
            updated_rows.push((diff.element, new_srcs));
        }

        shared_patch.0.check_model_restore(true, cx)?;
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
        if let Some(txn) = self.history.last_undo() {
            for patch in txn.patches() {
                patch.0.check_model_restore(false, cx)?;
            }
        }
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
        if let Some(txn) = self.history.last_redo() {
            for patch in txn.patches() {
                patch.0.check_model_restore(true, cx)?;
            }
        }
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

    fn snapshot_element(&self, element: ElementRef, cx: &mut Cx) -> Result<ElementSnap, Error> {
        let level_entry = self.building.components.level.get(element);
        let view = ElementSnapRef {
            identity: self.building.components.identity.get(element),
            level_name: level_entry.and_then(|level| level.name.as_deref()),
            properties: self.building.components.properties.get(element),
            property_details: self.building.components.property_details.get(element),
            quantities: self.building.components.quantities.get(element),
            relations: self
                .building
                .element(element)
                .map_or(&[], |e| &e.source().relations),
        };
        let copy_bytes = view.check_copy(cx)?;
        Ok(ElementSnap {
            copy_bytes,
            identity: self.building.components.identity.get(element).cloned(),
            has_level: level_entry.is_some(),
            level_name: level_entry.and_then(|l| l.name.clone()),
            properties: self.building.components.properties.get(element).cloned(),
            property_details: self
                .building
                .components
                .property_details
                .get(element)
                .cloned(),
            quantities: self.building.components.quantities.get(element).cloned(),
            relations: self
                .building
                .element(element)
                .map(|e| e.source().relations.clone())
                .unwrap_or_default(),
        })
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
        match &snap.property_details {
            Some(details) if !details.sets.is_empty() => {
                self.building
                    .components
                    .property_details
                    .insert(element, details.clone());
            }
            _ => {
                self.building.components.property_details.remove(element);
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
            self.parsed_changed
                .insert(diff.id, Arc::clone(&diff.after_parsed));
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
            match &diff.before_parsed {
                Some(parsed) => {
                    self.parsed_changed.insert(diff.id, Arc::clone(parsed));
                }
                None => {
                    self.parsed_changed.remove(&diff.id);
                }
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
    staged_parsed: BTreeMap<u64, Arc<ParsedStmt>>,
    staged_added: Vec<u64>,
    next: u64,
    referrer_overrides: BTreeMap<u64, BTreeSet<u64>>,
    seed: &'a [u8],
    minted: u64,
    affected_identity: BTreeSet<ElementRef>,
    affected_defs: BTreeSet<ElementRef>,
    copied_sets: BTreeMap<u64, u64>,
    copied_properties: BTreeMap<u64, u64>,
    project_name_after: Option<Option<String>>,
}

impl<'a> OverlayDraft<'a> {
    fn new(live: &'a IfcLive, seed: &'a [u8]) -> Self {
        Self {
            live,
            staged_changed: BTreeMap::new(),
            staged_parsed: BTreeMap::new(),
            staged_added: Vec::new(),
            next: live.next,
            referrer_overrides: BTreeMap::new(),
            seed,
            minted: 0,
            affected_identity: BTreeSet::new(),
            affected_defs: BTreeSet::new(),
            copied_sets: BTreeMap::new(),
            copied_properties: BTreeMap::new(),
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
        } else if stmt_name == b"IFCPROPERTYSET"
            || (attribute == RootAttribute::Name
                && (stmt_name == b"IFCELEMENTQUANTITY"
                    || predefined_property_fields(&stmt_name, self.live.version).is_some()))
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
                    self.copy_set(element, &owner, holder, pset, typed_object, cx)?
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
                .and_then(|stmt| self.live.text_value(stmt.arg(3)?))
                .is_some_and(|name| name == set)
    }

    fn copy_set(
        &mut self,
        element: u64,
        owner: &Stmt,
        holder: u64,
        pset: u64,
        typed_object: bool,
        cx: &mut Cx,
    ) -> Result<u64, Error> {
        let Some(mut copy) = self.stmt(pset) else {
            return Ok(pset);
        };
        cx.check(Kind::Work, 1)
            .map_err(|stop| stopped(cx, stop, None))?;
        cx.check(Kind::DecodedBytes, 64)
            .map_err(|stop| stopped(cx, stop, None))?;
        let original = self.copied_sets.get(&pset).copied().unwrap_or(pset);
        copy.set(1, self.global_id());
        let copy = self.add(copy);
        self.copied_sets.insert(copy, original);
        if typed_object {
            if let Some(mut stmt) = self.stmt(holder) {
                let sets = swap(stmt.arg(6), pset, copy);
                stmt.set(6, sets);
                self.put(holder, stmt);
            }
            return Ok(copy);
        }
        let Some(mut rel) = self.stmt(holder) else {
            return Ok(copy);
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
        Ok(copy)
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
                .and_then(|stmt| self.live.text_value(stmt.arg(1)?))
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
                    cx.check(Kind::Work, 1)
                        .map_err(|stop| stopped(cx, stop, None))?;
                    cx.check(Kind::DecodedBytes, 64)
                        .map_err(|stop| stopped(cx, stop, None))?;
                    let original = self
                        .copied_properties
                        .get(&property)
                        .copied()
                        .unwrap_or(property);
                    let copy = self.add(stmt);
                    self.copied_properties.insert(copy, original);
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
        if let Some(parsed) = self
            .staged_parsed
            .get(&id)
            .or_else(|| self.live.parsed_changed.get(&id))
        {
            return parsed.with_record(f);
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
        element: ElementRef,
        cx: &mut Cx,
    ) -> Result<Option<LiveDefinition>, Error> {
        cx.check(Kind::Work, 1)
            .map_err(|stop| stopped(cx, stop, None))?;
        let kind = self.kind(set_id);
        if kind == b"IFCPROPERTYSET" {
            let Some((name, prop_ids)) = self
                .with_parsed_or_base(set_id, |rec, base, slice| {
                    check_model_record(rec, slice, cx)?;
                    let n = slice_text(arg(rec, 3), base, slice).unwrap_or_default();
                    let ids = refs(arg(rec, 5));
                    Ok::<_, Error>((n, ids))
                })
                .transpose()?
            else {
                return Ok(None);
            };
            let mut properties = Vec::with_capacity(prop_ids.len());
            for &prop_id in &prop_ids {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| stopped(cx, stop, None))?;
                if !matches!(
                    self.kind(prop_id).as_slice(),
                    b"IFCPROPERTYSINGLEVALUE"
                        | b"IFCPROPERTYLISTVALUE"
                        | b"IFCPROPERTYENUMERATEDVALUE"
                ) {
                    continue;
                }
                if let Some(prop) = self.read_property(prop_id, cx)? {
                    properties.push(prop);
                }
            }
            let details = self.read_property_details(set_id, element, &name, &prop_ids, cx)?;
            Ok(Some(LiveDefinition::Properties(
                PropertySet { name, properties },
                details,
            )))
        } else if kind == b"IFCELEMENTQUANTITY" {
            let Some((name, method, qty_ids)) = self
                .with_parsed_or_base(set_id, |rec, base, slice| {
                    check_model_record(rec, slice, cx)?;
                    let n = slice_text(arg(rec, 3), base, slice).unwrap_or_default();
                    let m = slice_text(arg(rec, 5), base, slice);
                    let ids = refs(arg(rec, 6));
                    Ok::<_, Error>((n, m, ids))
                })
                .transpose()?
            else {
                return Ok(None);
            };
            let mut quantities = Vec::with_capacity(qty_ids.len());
            for qty_id in qty_ids {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| stopped(cx, stop, None))?;
                if let Some(qty) = self.read_single_quantity(qty_id, cx)? {
                    quantities.push(qty);
                }
            }
            Ok(Some(LiveDefinition::Quantities(QuantitySet {
                name,
                method,
                quantities,
            })))
        } else if let Some(fields) = predefined_property_fields(&kind, self.live.version) {
            self.with_parsed_or_base(set_id, |record, base, bytes| {
                check_model_record(record, bytes, cx)?;
                ModelCopy::new(cx).add(128)?;
                let name = slice_text(arg(record, 3), base, bytes).unwrap_or_default();
                let mut properties = Vec::new();
                for (at, name, type_name) in fields {
                    cx.check(Kind::Work, 1)
                        .map_err(|stop| stopped(cx, stop, None))?;
                    let Some(value) = arg(record, at).filter(|v| !matches!(v, Value::Null)) else {
                        continue;
                    };
                    ModelCopy::new(cx).add(256 + name.len() + type_name.len())?;
                    let value = if type_name.ends_with("Enum") {
                        match value {
                            Value::Enum(range) => Some(Literal::Enumeration(
                                String::from_utf8_lossy(slice_at(base, bytes, range.clone()))
                                    .into_owned(),
                            )),
                            _ => None,
                        }
                    } else {
                        slice_real(Some(value), base, bytes)
                            .filter(|value| value.is_finite())
                            .map(Literal::Real)
                    };
                    let Some(value) = value else {
                        continue;
                    };
                    let unit = type_name
                        .ends_with("LengthMeasure")
                        .then(|| self.doc_unit_by_kind("LENGTHUNIT"))
                        .flatten();
                    ModelCopy::new(cx).unit(unit)?;
                    properties.push(Property {
                        name: name.into(),
                        value: Some(TypedValue {
                            type_name: Some(type_name.into()),
                            value,
                        }),
                        unit: unit.cloned(),
                    });
                }
                Ok(Some(LiveDefinition::Properties(
                    PropertySet { name, properties },
                    None,
                )))
            })
            .transpose()
            .map(Option::flatten)
        } else {
            Ok(None)
        }
    }

    fn read_property(&self, prop_id: u64, cx: &mut Cx) -> Result<Option<Property>, Error> {
        let kind = self.kind(prop_id);
        let enumerated = kind == b"IFCPROPERTYENUMERATEDVALUE";
        let list = enumerated || kind == b"IFCPROPERTYLISTVALUE";
        self.with_parsed_or_base(prop_id, |rec, base, slice| {
            check_model_record(rec, slice, cx)?;
            let name = slice_text(arg(rec, 1), base, slice).unwrap_or_default();
            let value = match arg(rec, 3) {
                None | Some(Value::Null) => None,
                Some(v) if list => self.slice_property_values(v, base, slice),
                Some(v) => self.slice_typed_value(v, base, slice),
            };
            let unit_id = if enumerated {
                match arg(rec, 4) {
                    None | Some(Value::Null) => Ok(None),
                    other => reference(other)
                        .filter(|&id| self.kind(id) == b"IFCPROPERTYENUMERATION")
                        .and_then(|id| {
                            self.with_parsed_or_base(id, |rec, _, _| match arg(rec, 3) {
                                Some(Value::Null) => Ok(None),
                                other => reference(other).map(Some).ok_or(()),
                            })
                        })
                        .unwrap_or(Err(())),
                }
            } else {
                match arg(rec, 4) {
                    None | Some(Value::Null) => Ok(None),
                    other => reference(other).map(Some).ok_or(()),
                }
            };
            let unit = match unit_id {
                Err(()) => None,
                Ok(Some(unit_id)) => self.live.units.get(&unit_id).and_then(Option::as_ref),
                Ok(None) => value
                    .as_ref()
                    .and_then(|v| v.type_name.as_deref())
                    .and_then(|tn| self.doc_measure_unit(tn)),
            };
            ModelCopy::new(cx).unit(unit)?;
            Ok(Property {
                name,
                value,
                unit: unit.cloned(),
            })
        })
        .transpose()
    }

    fn slice_property_values(
        &self,
        value: &Value,
        base: usize,
        slice: &[u8],
    ) -> Option<TypedValue> {
        let Value::List(values) = value else {
            return None;
        };
        if values.is_empty() {
            return None;
        }
        let mut result = Vec::with_capacity(values.len());
        let mut type_name = None;
        for (index, value) in values.iter().enumerate() {
            let value = self.slice_typed_value(value, base, slice)?;
            if matches!(value.value, Literal::Real(real) if !real.is_finite()) {
                return None;
            }
            if index == 0 {
                type_name = value.type_name;
            } else if type_name != value.type_name {
                return None;
            }
            result.push(value.value);
        }
        Some(TypedValue {
            type_name,
            value: Literal::List(result),
        })
    }

    fn read_property_details(
        &self,
        set_id: u64,
        element: ElementRef,
        name: &str,
        roots: &[u64],
        cx: &mut Cx,
    ) -> Result<Option<DetailedPropertySet>, Error> {
        let original = self.copied_sets.get(&set_id).copied().unwrap_or(set_id);
        cx.check(
            Kind::Work,
            self.live
                .building
                .components
                .property_details
                .get(element)
                .map_or(0, |details| details.sets.len()) as u64,
        )
        .map_err(|stop| stopped(cx, stop, None))?;
        let Some(template) = self
            .live
            .building
            .components
            .property_details
            .get(element)
            .and_then(|details| {
                details
                    .sets
                    .iter()
                    .find(|set| set.source_instance == Some(original))
            })
        else {
            return Ok(None);
        };
        cx.check(
            Kind::Work,
            (template.properties.len() as u64).saturating_mul(2),
        )
        .map_err(|stop| stopped(cx, stop, None))?;
        let mut groups = BTreeMap::new();
        for (start, property) in template.properties.iter().enumerate() {
            if property.parent.is_some() {
                continue;
            }
            let end = template
                .properties
                .get(start + 1..)
                .unwrap_or_default()
                .iter()
                .position(|property| property.parent.is_none())
                .map_or(template.properties.len(), |next| start + next + 1);
            if let Some(instance) = property.source_instance {
                cx.check(Kind::DecodedBytes, 64)
                    .map_err(|stop| stopped(cx, stop, None))?;
                groups.entry(instance).or_insert(start..end);
            }
        }
        let description = self
            .with_parsed_or_base(set_id, |record, base, bytes| {
                check_model_record(record, bytes, cx)?;
                Ok::<_, Error>(slice_text(arg(record, 4), base, bytes))
            })
            .transpose()?
            .flatten();
        let mut copy = ModelCopy::new(cx);
        copy.add(128)?;
        copy.text(Some(name))?;
        let mut result = DetailedPropertySet {
            name: name.into(),
            description,
            properties: Vec::new(),
            source_instance: Some(set_id),
        };
        for &root in roots {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, None))?;
            let original = self.copied_properties.get(&root).copied().unwrap_or(root);
            if let Some(range) = groups.get(&original) {
                let new_start = result.properties.len();
                for (offset, property) in template
                    .properties
                    .get(range.clone())
                    .unwrap_or_default()
                    .iter()
                    .enumerate()
                {
                    ModelCopy::new(cx).detail_property(property)?;
                    let mut property = property.clone();
                    property.parent = property
                        .parent
                        .map(|parent| {
                            (parent as usize)
                                .checked_sub(range.start)
                                .and_then(|relative| relative.checked_add(new_start))
                                .and_then(|index| u32::try_from(index).ok())
                                .ok_or(Error::Invalid)
                        })
                        .transpose()?;
                    if offset == 0 {
                        property.source_instance = Some(root);
                    }
                    if let Some(instance) = property.source_instance
                        && self.staged_changed.contains_key(&instance)
                        && matches!(property.value, PropertyDetailValue::Single { .. })
                        && let Some(value) = self.read_property(instance, cx)?
                    {
                        property.value = PropertyDetailValue::Single {
                            value: value.value,
                            unit: value.unit,
                        };
                    }
                    result.properties.push(property);
                }
            } else if self.kind(root) == b"IFCPROPERTYSINGLEVALUE"
                && let Some(value) = self.read_property(root, cx)?
            {
                let description = self
                    .with_parsed_or_base(root, |record, base, bytes| {
                        check_model_record(record, bytes, cx)?;
                        Ok::<_, Error>(slice_text(arg(record, 2), base, bytes))
                    })
                    .transpose()?
                    .flatten();
                ModelCopy::new(cx).add(512)?;
                result.properties.push(DetailedProperty {
                    parent: None,
                    name: value.name,
                    description,
                    value: PropertyDetailValue::Single {
                        value: value.value,
                        unit: value.unit,
                    },
                    source_instance: Some(root),
                });
            }
        }
        Ok(Some(result))
    }

    fn read_single_quantity(&self, qty_id: u64, cx: &mut Cx) -> Result<Option<Quantity>, Error> {
        let kind = self.kind(qty_id);
        let Some((_, quantity_kind, unit_kind)) = QUANTITIES
            .iter()
            .find(|(name, ..)| name.as_bytes() == kind.as_slice())
        else {
            return Ok(None);
        };
        self.with_parsed_or_base(qty_id, |rec, base, slice| {
            check_model_record(rec, slice, cx)?;
            let Some(value) = slice_real(arg(rec, 4), base, slice) else {
                return Ok(None);
            };
            let unit = match reference(arg(rec, 3)) {
                Some(unit_id) => self.live.units.get(&unit_id).and_then(Option::as_ref),
                None => unit_kind.and_then(|uk| self.doc_unit_by_kind(uk)),
            };
            ModelCopy::new(cx).unit(unit)?;
            Ok(Some(Quantity {
                name: slice_text(arg(rec, 1), base, slice).unwrap_or_default(),
                kind: *quantity_kind,
                value: Measured {
                    value,
                    unit: unit.cloned(),
                },
                formula: match self.live.version {
                    Version::Ifc2x3 => None,
                    _ => slice_text(arg(rec, 5), base, slice),
                },
            }))
        })
        .transpose()
        .map(Option::flatten)
    }

    fn doc_unit_by_kind(&self, kind: &str) -> Option<&Unit> {
        self.live
            .building
            .document
            .units
            .iter()
            .find(|u| u.kind == kind)
    }

    fn doc_measure_unit(&self, type_name: &str) -> Option<&Unit> {
        let kind = measure_types()
            .find(|(name, _)| *name == type_name)
            .and_then(|(_, k)| *k)?;
        self.doc_unit_by_kind(kind)
    }

    fn slice_typed_value(&self, value: &Value, base: usize, slice: &[u8]) -> Option<TypedValue> {
        match value {
            Value::Typed(part) => {
                let written =
                    String::from_utf8_lossy(slice_at(base, slice, part.name())).into_owned();
                let type_name = measure_types()
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

    fn copy_definition(
        &self,
        set_id: u64,
        element: ElementRef,
        cache: &mut BTreeMap<u64, Option<LiveDefinition>>,
        cx: &mut Cx,
    ) -> Result<Option<LiveDefinition>, Error> {
        if let std::collections::btree_map::Entry::Vacant(entry) = cache.entry(set_id) {
            ModelCopy::new(cx).add(128)?;
            entry.insert(self.read_definition_set(set_id, element, cx)?);
        }
        let definition = cache.get(&set_id).and_then(Option::as_ref);
        let mut copy = ModelCopy::new(cx);
        match definition {
            Some(LiveDefinition::Properties(set, details)) => {
                copy.property_set(set)?;
                if let Some(details) = details {
                    copy.detail_set(details)?;
                }
            }
            Some(LiveDefinition::Quantities(set)) => copy.quantity_set(set)?,
            None => {}
        }
        Ok(definition.cloned())
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
        let mut detail_sets = Vec::new();
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
                    k == b"IFCPROPERTYSET"
                        || k == b"IFCELEMENTQUANTITY"
                        || predefined_property_fields(&k, self.live.version).is_some()
                })
                .collect();
            let rel_gid = rel_stmt.arg(1).and_then(text_of);

            for pos in positions {
                for &set_id in &set_ids {
                    let def = self.copy_definition(set_id, elem_ref, &mut def_cache, cx)?;
                    match def {
                        Some(LiveDefinition::Properties(ps, details)) => {
                            prop_sets.push(ps);
                            detail_sets.extend(details);
                        }
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
                let def = self.copy_definition(set_id, elem_ref, &mut def_cache, cx)?;
                match def {
                    Some(LiveDefinition::Properties(ps, details)) => {
                        prop_sets.push(ps);
                        detail_sets.extend(details);
                    }
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
        let details = (!detail_sets.is_empty()).then_some(PropertyDetails { sets: detail_sets });
        Ok(ComputedElementDefs {
            properties: props,
            property_details: details,
            quantities: qtys,
            relations: merged,
        })
    }

    fn finish(mut self, cx: &mut Cx) -> Result<StagedOutcome, Error> {
        // 建物モデルと IDS の属性取得は同じ解析結果を使い、取り消しも文と一緒に戻す。
        for (&id, stmt) in &self.staged_changed {
            let parsed = parse_single_stmt(id, stmt, cx)?;
            self.staged_parsed.insert(id, Arc::new(parsed));
        }
        let changed_count = self.staged_changed.len();
        let mut all_affected: BTreeSet<ElementRef> = self.affected_identity.clone();
        all_affected.extend(self.affected_defs.iter().copied());

        let mut element_diffs = Vec::with_capacity(all_affected.len());
        for elem_ref in all_affected {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, None))?;
            let before = self.live.snapshot_element(elem_ref, cx)?;
            before.check_copy(cx)?;
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
                let definitions = self.compute_defs_and_relations(elem_ref, inst_id, cx)?;
                after.properties = definitions.properties;
                after.property_details = definitions.property_details;
                after.quantities = definitions.quantities;
                after.relations = definitions.relations;
            }
            after.copy_bytes = after.check_copy(cx)?;
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
            let before_parsed = self.live.parsed_changed.get(&id).cloned();
            let after_parsed = self.staged_parsed.remove(&id).ok_or(Error::Invalid)?;
            stmt_diffs.push(StmtDiff {
                id,
                before,
                after,
                before_parsed,
                after_parsed,
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

struct ComputedElementDefs {
    properties: Option<Properties>,
    property_details: Option<PropertyDetails>,
    quantities: Option<Quantities>,
    relations: Vec<RelationSource>,
}

#[derive(Clone)]
enum LiveDefinition {
    Properties(PropertySet, Option<DetailedPropertySet>),
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

/// 書き直した 1 文の読み取り結果。元の文との出所位置の比較には使わない。
#[derive(Clone, Debug)]
struct ParsedStmt {
    bytes: Vec<u8>,
    record: Option<Record>,
    heap_bytes: usize,
}

impl ParsedStmt {
    fn with_record<R>(&self, f: impl FnOnce(&Record, usize, &[u8]) -> R) -> Option<R> {
        self.record.as_ref().map(|record| f(record, 0, &self.bytes))
    }
}

fn stmt_items_size(items: &[Item], cx: &mut Cx) -> Result<usize, Error> {
    cx.enter().map_err(|stop| stopped(cx, stop, None))?;
    let result = (|| {
        let mut size = 2usize.saturating_add(items.len().saturating_sub(1));
        for item in items {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, None))?;
            let bytes = match item {
                Item::Ref(_) => 21,
                Item::Raw(bytes) => {
                    cx.check(Kind::Work, bytes.len().div_ceil(1024) as u64)
                        .map_err(|stop| stopped(cx, stop, None))?;
                    bytes.len()
                }
                Item::List(nested) => stmt_items_size(nested, cx)?,
            };
            size = size.saturating_add(bytes);
        }
        Ok(size)
    })();
    cx.leave();
    result
}

fn parsed_values_size(values: &[Value], cx: &mut Cx) -> Result<usize, Error> {
    cx.enter().map_err(|stop| stopped(cx, stop, None))?;
    let result = (|| {
        // ネイティブと wasm で同じ論理バイト数を数える。確保時の余裕も含む。
        let mut size = if values.is_empty() {
            0
        } else {
            values.len().saturating_add(3).saturating_mul(128)
        };
        for value in values {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, None))?;
            let nested = match value {
                Value::List(values) => parsed_values_size(values, cx)?,
                Value::Typed(part) => parsed_values_size(part.args(), cx)?,
                _ => 0,
            };
            size = size.saturating_add(nested);
        }
        Ok(size)
    })();
    cx.leave();
    result
}

fn check_model_record(record: &Record, bytes: &[u8], cx: &mut Cx) -> Result<(), Error> {
    // 型名・文字列・値の列を作る前に、解析済みの木と文字の展開分を数える。
    let mut size = bytes.len().saturating_mul(4).saturating_add(512);
    for part in record.parts() {
        size = size.saturating_add(parsed_values_size(part.args(), cx)?.saturating_mul(4));
    }
    ModelCopy::new(cx).add(size)
}

fn parse_single_stmt(id: u64, stmt: &Stmt, cx: &mut Cx) -> Result<ParsedStmt, Error> {
    const STEP_PREFIX: &[u8] = b"ISO-10303-21;HEADER;ENDSEC;DATA;";
    const STEP_SUFFIX: &[u8] = b"ENDSEC;END-ISO-10303-21;";
    cx.check(Kind::Work, 1 + stmt.name.len().div_ceil(1024) as u64)
        .map_err(|stop| stopped(cx, stop, None))?;
    let capacity = STEP_PREFIX
        .len()
        .saturating_add(STEP_SUFFIX.len())
        .saturating_add(23)
        .saturating_add(stmt.name.len())
        .saturating_add(stmt_items_size(&stmt.args, cx)?);
    cx.check(Kind::DecodedBytes, capacity as u64)
        .map_err(|stop| stopped(cx, stop, None))?;
    let mut bytes = Vec::with_capacity(capacity);
    bytes.extend_from_slice(STEP_PREFIX);
    write_stmt(id, stmt, &mut bytes);
    bytes.extend_from_slice(STEP_SUFFIX);
    let record = match zumen_sdk::quietly(cx, |cx| zumen_step::read(&bytes, &DIALECT, cx)) {
        Ok(parsed) => parsed
            .records
            .into_iter()
            .find(|record| record.id() == Some(id)),
        Err(Error::Stopped(stop)) => return Err(Error::Stopped(stop)),
        Err(_) => None,
    };
    let mut metadata = 256usize;
    if let Some(record) = &record {
        for part in record.parts() {
            metadata = metadata
                .saturating_add(128)
                .saturating_add(parsed_values_size(part.args(), cx)?);
        }
    }
    cx.check(Kind::DecodedBytes, metadata as u64)
        .map_err(|stop| stopped(cx, stop, None))?;
    Ok(ParsedStmt {
        bytes,
        record,
        heap_bytes: capacity.saturating_add(metadata),
    })
}

fn insert_dep_source(out: &mut BTreeSet<RowId>, row: RowId, cx: &mut Cx) -> Result<(), Error> {
    cx.check(Kind::Work, 1)
        .map_err(|stop| stopped(cx, stop, None))?;
    if !out.contains(&row) {
        cx.check(Kind::DecodedBytes, 128)
            .map_err(|stop| stopped(cx, stop, None))?;
        out.insert(row);
    }
    Ok(())
}

fn copy_dep_sources(
    sources: Option<&BTreeSet<RowId>>,
    cx: &mut Cx,
) -> Result<BTreeSet<RowId>, Error> {
    let mut out = BTreeSet::new();
    if let Some(sources) = sources {
        for &source in sources {
            insert_dep_source(&mut out, source, cx)?;
        }
    }
    Ok(out)
}

fn collect_literal_element_refs(
    lit: &Literal,
    out: &mut BTreeSet<RowId>,
    cx: &mut Cx,
) -> Result<(), Error> {
    cx.check(Kind::Work, 1)
        .map_err(|stop| stopped(cx, stop, None))?;
    match lit {
        Literal::Element(e) => {
            insert_dep_source(out, RowId(e.0), cx)?;
        }
        Literal::List(items) => {
            cx.enter().map_err(|stop| stopped(cx, stop, None))?;
            let result = items
                .iter()
                .try_for_each(|item| collect_literal_element_refs(item, out, cx));
            cx.leave();
            result?;
        }
        _ => {}
    }
    Ok(())
}

fn collect_property_element_refs(
    props: Option<&Properties>,
    out: &mut BTreeSet<RowId>,
    cx: &mut Cx,
) -> Result<(), Error> {
    cx.check(Kind::Work, 1)
        .map_err(|stop| stopped(cx, stop, None))?;
    let Some(props) = props else {
        return Ok(());
    };
    for set in &props.sets {
        cx.check(Kind::Work, 1)
            .map_err(|stop| stopped(cx, stop, None))?;
        for prop in &set.properties {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, None))?;
            if let Some(tv) = &prop.value {
                collect_literal_element_refs(&tv.value, out, cx)?;
            }
        }
    }
    Ok(())
}

fn collect_detail_element_refs(
    details: Option<&PropertyDetails>,
    out: &mut BTreeSet<RowId>,
    cx: &mut Cx,
) -> Result<(), Error> {
    let Some(details) = details else {
        return Ok(());
    };
    for set in &details.sets {
        cx.check(Kind::Work, 1)
            .map_err(|stop| stopped(cx, stop, None))?;
        for property in &set.properties {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, None))?;
            match &property.value {
                PropertyDetailValue::Single { value, .. } => {
                    if let Some(value) = value {
                        collect_literal_element_refs(&value.value, out, cx)?;
                    }
                }
                PropertyDetailValue::List { values, .. }
                | PropertyDetailValue::Enumerated { values, .. } => {
                    for value in values.iter().flatten() {
                        collect_literal_element_refs(&value.value, out, cx)?;
                    }
                    if let PropertyDetailValue::Enumerated {
                        choices: Some(choices),
                        ..
                    } = &property.value
                    {
                        for value in &choices.values {
                            collect_literal_element_refs(&value.value, out, cx)?;
                        }
                    }
                }
                PropertyDetailValue::Bounded {
                    lower,
                    upper,
                    set_point,
                    ..
                } => {
                    for value in [lower, upper, set_point].into_iter().flatten() {
                        collect_literal_element_refs(&value.value, out, cx)?;
                    }
                }
                PropertyDetailValue::Complex { .. } | PropertyDetailValue::Unsupported { .. } => {}
            }
        }
    }
    Ok(())
}

fn build_ifc_dep_graph(
    building: &Building,
    cx: &mut Cx,
) -> Result<(Vec<BTreeSet<RowId>>, DepGraph), Error> {
    let elem_count = building.elements().len();
    cx.check(Kind::DecodedBytes, (elem_count as u64).saturating_mul(32))
        .map_err(|stop| stopped(cx, stop, None))?;
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
                insert_dep_source(set, RowId(m.target.0), cx)?;
            }
            if let Some(set) = base_sources.get_mut(idx) {
                for &r in &parent.referenced_in {
                    insert_dep_source(set, RowId(r.0), cx)?;
                }
            }
        }
        if let Some(pl) = building.components.placement.get(elem_ref)
            && let Some(rel) = pl.relative_to
            && let Some(set) = base_sources.get_mut(idx)
        {
            insert_dep_source(set, RowId(rel.0), cx)?;
        }
        if let Some(tp) = building.components.r#type.get(elem_ref)
            && let Some(set) = base_sources.get_mut(idx)
        {
            insert_dep_source(set, RowId(tp.target.0), cx)?;
        }
        if let Some(mat) = building.components.material.get(elem_ref)
            && let Some(set) = base_sources.get_mut(idx)
        {
            for part in &mat.parts {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| stopped(cx, stop, None))?;
                if let Some(m) = part.material {
                    insert_dep_source(set, RowId(m.0), cx)?;
                }
            }
        }
        if let Some(op) = building.components.opening.get(elem_ref) {
            match *op {
                Opening::Voids(host) => {
                    if let Some(host_set) = base_sources.get_mut(host.0 as usize) {
                        insert_dep_source(host_set, row, cx)?;
                    }
                }
                Opening::Fills(opening_elem) => {
                    if let Some(set) = base_sources.get_mut(idx) {
                        insert_dep_source(set, RowId(opening_elem.0), cx)?;
                    }
                }
            }
        }
        if let Some(grp) = building.components.group.get(elem_ref)
            && let Some(set) = base_sources.get_mut(idx)
        {
            for &g in &grp.groups {
                insert_dep_source(set, RowId(g.0), cx)?;
            }
        }
        if let Some(Grid::Grid { axes }) = building.components.grid.get(elem_ref)
            && let Some(set) = base_sources.get_mut(idx)
        {
            for &(_, axis) in axes {
                insert_dep_source(set, RowId(axis.0), cx)?;
            }
        }
        if let Some(zone) = building.components.zone.get(elem_ref)
            && let Some(set) = base_sources.get_mut(idx)
        {
            for &m in &zone.members {
                insert_dep_source(set, RowId(m.0), cx)?;
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
        let mut combined = copy_dep_sources(Some(base), cx)?;
        collect_property_element_refs(
            building.components.properties.get(elem_ref),
            &mut combined,
            cx,
        )?;
        collect_detail_element_refs(
            building.components.property_details.get(elem_ref),
            &mut combined,
            cx,
        )?;
        if !combined.is_empty() {
            cx.check(
                Kind::Work,
                (combined.len() as u64).saturating_mul(3).saturating_add(1),
            )
            .map_err(|stop| stopped(cx, stop, None))?;
            cx.check(
                Kind::DecodedBytes,
                (combined.len() as u64)
                    .saturating_mul(384)
                    .saturating_add(256),
            )
            .map_err(|stop| stopped(cx, stop, None))?;
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
        if let Some(parsed) = self.live.parsed_changed.get(&instance) {
            return parsed
                .with_record(|record, base, slice| {
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

#[cfg(test)]
mod shared_budget_tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Limit, Limits, Stop};
    use zumen_sdk::Lift;

    fn run<T>(f: impl FnOnce(&mut Cx) -> T) -> (T, Vec<Diagnostic>) {
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let result = f(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
        (result, diagnostics.into_vec())
    }

    fn record_budget<T: core::fmt::Debug>(
        transcript: &mut String,
        f: impl FnOnce(&mut Cx) -> T,
    ) -> T {
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let result = f(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
        for kind in Kind::ALL {
            transcript.push_str(&format!("{kind:?}: {}\n", budget.used(kind)));
        }
        transcript.push_str(&format!("{:?}\n", diagnostics.into_vec()));
        result
    }

    #[test]
    fn changed_statement_uses_the_shared_budget_and_cancel_token() {
        let stmt = Stmt {
            name: b"IFCPROPERTYSINGLEVALUE".to_vec(),
            args: vec![
                text("Value"),
                Item::Raw(b"$".to_vec()),
                Item::Raw(b"IFCLABEL('changed')".to_vec()),
                Item::List(vec![Item::Ref(2)]),
            ],
        };
        let mut measured = Budget::default();
        let cancel = CancelToken::new();
        let parsed = parse_single_stmt(
            10,
            &stmt,
            &mut Cx::new(&mut measured, &mut Diagnostics::new(), &cancel),
        )
        .unwrap();
        assert_eq!(parsed.record.as_ref().and_then(Record::id), Some(10));
        for kind in [Kind::Work, Kind::DecodedBytes, Kind::Records] {
            let amount = measured.used(kind);
            assert!(amount > 0);
            for (limit, spent) in [(0, 0), (amount - 1, 0), (amount, 1), (amount, 0)] {
                let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
                budget.spend(kind, spent).unwrap();
                let mut diagnostics = Diagnostics::new();
                let result = parse_single_stmt(
                    10,
                    &stmt,
                    &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
                );
                if limit == amount && spent == 0 {
                    assert_eq!(result.unwrap().record, parsed.record);
                    assert!(diagnostics.is_empty());
                } else {
                    assert!(matches!(result, Err(Error::Stopped(Stop::Exceeded(_)))));
                    assert_eq!(
                        diagnostics
                            .iter()
                            .filter(|d| d.code() == kind.exceeded_code())
                            .count(),
                        1
                    );
                }
                assert_eq!(budget.used(Kind::Depth), 0);
            }
        }
        let mut budget = Budget::new(Limits::default().with(Kind::Depth, Limit::Finite(1)));
        assert!(matches!(
            parse_single_stmt(
                10,
                &stmt,
                &mut Cx::new(&mut budget, &mut Diagnostics::new(), &cancel)
            ),
            Err(Error::Stopped(_))
        ));
        assert_eq!(budget.used(Kind::Depth), 0);
        cancel.cancel();
        assert!(matches!(
            run(|cx| parse_single_stmt(10, &stmt, &mut Cx::new(cx.budget, cx.diag, &cancel))).0,
            Err(Error::Stopped(Stop::Cancelled))
        ));
    }

    fn fixture() -> Vec<u8> {
        concat!(
            "ISO-10303-21;HEADER;FILE_DESCRIPTION((''),'2;1');",
            "FILE_SCHEMA(('IFC4'));ENDSEC;DATA;",
            "#1=IFCPROJECT('0000000000000000000001',$,'Project',$,$,$,$,$,#3);",
            "#3=IFCUNITASSIGNMENT((#4));#4=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);",
            "#10=IFCWALL('0000000000000000000010',$,'A',$,$,$,$,$,$);",
            "#11=IFCWALL('0000000000000000000011',$,'B',$,$,$,$,$,$);",
            "#40=IFCPROPERTYSET('0000000000000000000040',$,'Pset_Test',$,(#41));",
            "#41=IFCPROPERTYSINGLEVALUE('Value',$,IFCLABEL('before'),$);",
            "#42=IFCRELDEFINESBYPROPERTIES('0000000000000000000042',$,$,$,(#10,#11),#40);",
            "#60=IFCELEMENTQUANTITY('0000000000000000000060',$,'Qto_Test',$,$,(#61));",
            "#61=IFCQUANTITYLENGTH('Width',$,$,1.,$);",
            "#62=IFCRELDEFINESBYPROPERTIES('0000000000000000000062',$,$,$,(#10),#60);",
            "ENDSEC;END-ISO-10303-21;",
        )
        .as_bytes()
        .to_vec()
    }

    #[test]
    fn rich_property_sets_follow_single_value_edits_and_history() {
        for extra in [
            "#70=IFCPROPERTYBOUNDEDVALUE('Bounds',$,IFCLENGTHMEASURE(4.),IFCLENGTHMEASURE(1.),$,IFCLENGTHMEASURE(2.));",
            "#70=IFCPROPERTYLISTVALUE('Choices',$,(IFCLABEL('a'),IFCLABEL('b')),$);",
            "#70=IFCPROPERTYENUMERATEDVALUE('Mode',$,(IFCLABEL('a')),#71);#71=IFCPROPERTYENUMERATION('Modes',(IFCLABEL('a'),IFCLABEL('b')),$);",
            "#70=IFCCOMPLEXPROPERTY('Complex',$,'Use',(#41,#71));#71=IFCPROPERTYSINGLEVALUE('Child',$,IFCLABEL('child'),$);",
            "#70=IFCPROPERTYLISTVALUE('Velocity',$,(IFCLINEARVELOCITYMEASURE(1.),IFCLINEARVELOCITYMEASURE(2.)),$);",
        ] {
            let bytes = String::from_utf8(fixture())
                .unwrap()
                .replace("'Pset_Test',$,(#41)", "'Pset_Test',$,(#41,#70)")
                .replace(
                    "ENDSEC;END-ISO-10303-21;",
                    &format!("{extra}ENDSEC;END-ISO-10303-21;"),
                )
                .into_bytes();
            for threads in [1, 4] {
                let mut live = run(|cx| IfcLive::open_with_threads(&bytes, threads, cx))
                    .0
                    .unwrap();
                check_model(&live, &bytes);
                let change = [Edit::Property {
                    element: 10,
                    set: "Pset_Test".into(),
                    name: "Value".into(),
                    value: NewValue {
                        type_name: "IfcLabel".into(),
                        literal: crate::NewLiteral::Text("changed".into()),
                    },
                }];
                let file = run(|cx| Ifc::read(&bytes, cx)).0.unwrap();
                let expected = run(|cx| crate::apply(&file, &change, b"rich-property", cx));
                let actual = run(|cx| live.apply(&change, b"rich-property", cx));
                assert_eq!(actual.1, expected.1);
                let expected = expected.0.unwrap();
                assert_eq!(actual.0.unwrap(), expected.changed);
                assert_eq!(run(|cx| live.save_spf(cx)).0.unwrap(), expected.bytes);
                check_model(&live, &expected.bytes);
                assert!(run(|cx| live.undo(cx)).0.unwrap());
                check_model(&live, &bytes);
                assert!(run(|cx| live.redo(cx)).0.unwrap());
                check_model(&live, &expected.bytes);
            }
        }
    }

    fn rich_fixture() -> Vec<u8> {
        String::from_utf8(fixture()).unwrap()
            .replace("'Pset_Test',$,(#41)", "'Pset_Test','set description',(#41,#70,#70,#71,#72,#73,#77)")
            .replace("IFCUNITASSIGNMENT((#4))", "IFCUNITASSIGNMENT((#4,#80))")
            .replace("ENDSEC;END-ISO-10303-21;", concat!(
                "#20=IFCWALLTYPE('0000000000000000000020',$,'Type A',$,$,(#40),$,$,$,.STANDARD.);",
                "#21=IFCWALLTYPE('0000000000000000000021',$,'Type B',$,$,(#40),$,$,$,.STANDARD.);",
                "#70=IFCCOMPLEXPROPERTY('Complex','complex description','Use',(#41,#76));",
                "#71=IFCPROPERTYLISTVALUE('Choices',$,(IFCLABEL('a'),IFCLABEL('b')),$);",
                "#72=IFCPROPERTYBOUNDEDVALUE('Bounds',$,IFCLENGTHMEASURE(4.),IFCLENGTHMEASURE(1.),#4,IFCLENGTHMEASURE(2.));",
                "#73=IFCPROPERTYENUMERATEDVALUE('Mode',$,(IFCLABEL('a')),#74);",
                "#74=IFCPROPERTYENUMERATION('Modes',(IFCLABEL('a'),IFCLABEL('b')),$);",
                "#75=IFCPROPERTYSINGLEVALUE('ChildRef',$,#11,$);",
                "#76=IFCCOMPLEXPROPERTY('Nested',$,'Nested Use',(#75,#71));",
                "#77=IFCPROPERTYLISTVALUE('Velocity',$,(IFCLINEARVELOCITYMEASURE(1.),IFCLINEARVELOCITYMEASURE(2.)),$);",
                "#80=IFCDERIVEDUNIT((#81,#82),.LINEARVELOCITYUNIT.,$);",
                "#81=IFCDERIVEDUNITELEMENT(#4,1);#82=IFCDERIVEDUNITELEMENT(#83,-1);",
                "#83=IFCSIUNIT(*,.TIMEUNIT.,$,.SECOND.);",
                "ENDSEC;END-ISO-10303-21;",
            )).into_bytes()
    }

    #[test]
    fn predefined_property_sets_survive_unrelated_property_edits() {
        let bytes = String::from_utf8(fixture()).unwrap().replace(
            "ENDSEC;END-ISO-10303-21;",
            concat!(
                "#20=IFCDOORTYPE('0000000000000000000020',$,'Door Type',$,$,(#70,#40),$,$,$,.DOOR.,.SINGLE_SWING_LEFT.,.F.,$);",
                "#70=IFCDOORLININGPROPERTIES('0000000000000000000070',$,'Lining',$,0.3,0.1,$,$,$,$,$,$,$,$,$,$,$);",
                "#71=IFCRELDEFINESBYPROPERTIES('0000000000000000000071',$,$,$,(#10),#70);",
                "ENDSEC;END-ISO-10303-21;",
            ),
        ).into_bytes();
        for threads in [1, 4] {
            for element in [10, 20] {
                let mut live = run(|cx| IfcLive::open_with_threads(&bytes, threads, cx))
                    .0
                    .unwrap();
                check_model(&live, &bytes);
                let edits = [rich_edit(element, "Pset_Test", "Value", "changed")];
                let file = run(|cx| Ifc::read(&bytes, cx)).0.unwrap();
                let expected = run(|cx| crate::apply(&file, &edits, b"predefined", cx));
                let actual = run(|cx| live.apply(&edits, b"predefined", cx));
                assert_eq!(actual.1, expected.1);
                let expected = expected.0.unwrap();
                assert_eq!(actual.0.unwrap(), expected.changed);
                check_model(&live, &expected.bytes);
                assert!(run(|cx| live.undo(cx)).0.unwrap());
                check_model(&live, &bytes);
                assert!(run(|cx| live.redo(cx)).0.unwrap());
                check_model(&live, &expected.bytes);
            }
        }
    }

    fn predefined_fixture(schema: &str, variant: u8) -> Vec<u8> {
        let mut bytes = String::from_utf8(fixture())
            .unwrap()
            .replace("'IFC4'", &format!("'{schema}'"));
        bytes = bytes.replace("ENDSEC;END-ISO-10303-21;", concat!(
            "#20=IFCDOORTYPE('0000000000000000000020',$,'Type',$,$,(#70,#40,#71,#72,#73,#70),$,$,$,.DOOR.,.SINGLE_SWING_LEFT.,.F.,$);",
            "#70=ifcdoorliningproperties('0000000000000000000070',$,'Door Lining',$,IFCLENGTHMEASURE(3),0.,1.,0.2,0.3,0.4,0.5,0.6,0.7,0.8,#999,0.9,1.);",
            "#71=IFCDOORPANELPROPERTIES('0000000000000000000071',$,'Door Panel',$,IFCLENGTHMEASURE(2),.SWINGING.,0.5,.LEFT.,#999);",
            "#72=IFCWINDOWLININGPROPERTIES('0000000000000000000072',$,'Window Lining',$,1.,0.,0.2,0.3,0.25,0.5,0.3,0.4,#999,0.6,0.7,0.8);",
            "#73=IFCWINDOWPANELPROPERTIES('0000000000000000000073',$,'Window Panel',$,.ROTATING.,.LEFT.,0.4,IFCLENGTHMEASURE(0.1),#999);",
            "#80=IFCRELDEFINESBYPROPERTIES('0000000000000000000080',$,$,$,(#10,#10,#20),(#70,#40,#71,#72,#73,#70,#999));",
            "ENDSEC;END-ISO-10303-21;",
        ));
        if schema == "IFC2X3" {
            bytes = bytes.replace("IFCDOORTYPE", "IFCDOORSTYLE");
        }
        if variant == 1 {
            bytes = bytes
                .replace("IFCLENGTHMEASURE(3)", "'bad'")
                .replace(".SWINGING.", "3");
        } else if variant == 2 {
            bytes = bytes.replace("'Project',$,$,$,$,$,#3", "'Project',$,$,$,$,$,$");
        }
        bytes.into_bytes()
    }

    #[test]
    fn predefined_properties_follow_versions_units_renames_and_history() {
        let mut transcript = String::new();
        for schema in ["IFC2X3", "IFC4", "IFC4X3"] {
            for variant in 0..3 {
                let bytes = predefined_fixture(schema, variant);
                let mut thread_record = None;
                for threads in [1, 4] {
                    let mut record = String::new();
                    for element in [10, 20] {
                        let mut live = record_budget(&mut record, |cx| {
                            IfcLive::open_with_threads(&bytes, threads, cx)
                        })
                        .unwrap();
                        record.push_str(&check_model(&live, &bytes));
                        let owner = live.element_by_instance(element).unwrap();
                        let sets = &live.building.components.properties.get(owner).unwrap().sets;
                        let lining = sets.iter().find(|s| s.name == "Door Lining").unwrap();
                        let thickness = lining
                            .properties
                            .iter()
                            .find(|p| p.name == "LiningThickness")
                            .unwrap();
                        assert_eq!(
                            thickness.value.as_ref().unwrap().type_name.as_deref(),
                            Some(if schema == "IFC2X3" {
                                "IfcPositiveLengthMeasure"
                            } else {
                                "IfcNonNegativeLengthMeasure"
                            })
                        );
                        assert_eq!(thickness.unit.is_some(), variant != 2);
                        assert_eq!(
                            lining
                                .properties
                                .iter()
                                .any(|p| p.name == "LiningToPanelOffsetX"),
                            schema != "IFC2X3"
                        );
                        let panel = sets.iter().find(|s| s.name == "Window Panel").unwrap();
                        assert!(matches!(
                            panel.properties[0].value.as_ref().unwrap().value,
                            Literal::Enumeration(_)
                        ));
                        assert!(panel.properties[0].unit.is_none());
                        let rename = |element, value: Option<&str>| Edit::Attribute {
                            element,
                            attribute: RootAttribute::Name,
                            value: value.map(str::to_owned),
                        };
                        let changes = [
                            vec![rich_edit(element, "Pset_Test", "Value", "changed")],
                            vec![rename(70, Some("枠 'A'"))],
                            vec![Edit::Attribute {
                                element: 70,
                                attribute: RootAttribute::Description,
                                value: Some("description".into()),
                            }],
                            vec![rename(71, None)],
                            vec![rename(72, Some("Window")), rename(73, Some("Window"))],
                            vec![rich_edit(element, "Added", "Value", "new")],
                            vec![rename(40, Some("Renamed"))],
                        ];
                        let mut history = vec![bytes.clone()];
                        for edits in changes {
                            let file = run(|cx| Ifc::read(history.last().unwrap(), cx)).0.unwrap();
                            let expected =
                                run(|cx| crate::apply(&file, &edits, b"predefined-history", cx));
                            let actual = record_budget(&mut record, |cx| {
                                let result = live.apply(&edits, b"predefined-history", cx);
                                (result, cx.diag.iter().cloned().collect::<Vec<_>>())
                            });
                            assert_eq!(actual.1, expected.1);
                            record.push_str(&format!("{:?}\n", actual.1));
                            let expected = expected.0.unwrap();
                            assert_eq!(actual.0.unwrap(), expected.changed);
                            record.push_str(&check_model(&live, &expected.bytes));
                            history.push(expected.bytes);
                        }
                        for version in history.iter().rev().skip(1) {
                            assert!(record_budget(&mut record, |cx| live.undo(cx)).unwrap());
                            record.push_str(&check_model(&live, version));
                        }
                        for version in history.iter().skip(1) {
                            assert!(record_budget(&mut record, |cx| live.redo(cx)).unwrap());
                            record.push_str(&check_model(&live, version));
                        }
                    }
                    if let Some(reference) = &thread_record {
                        assert_eq!(&record, reference);
                    } else {
                        thread_record = Some(record.clone());
                    }
                    transcript.push_str(&record);
                }
            }
        }
        #[expect(clippy::print_stdout, reason = "テストの比較記録だけを出力する")]
        {
            println!("ifc-predefined {:?}", Hash::document(transcript.as_bytes()));
        }
    }

    fn rich_edit(element: u64, set: &str, name: &str, text: &str) -> Edit {
        Edit::Property {
            element,
            set: set.into(),
            name: name.into(),
            value: NewValue {
                type_name: "IfcLabel".into(),
                literal: crate::NewLiteral::Text(text.into()),
            },
        }
    }

    #[test]
    fn rich_property_sets_follow_multiple_edits_types_and_parent_positions() {
        let bytes = rich_fixture();
        let mut transcript = String::new();
        for threads in [1, 4] {
            for element in [10, 20] {
                let mut live = run(|cx| IfcLive::open_with_threads(&bytes, threads, cx))
                    .0
                    .unwrap();
                let mut versions = vec![bytes.clone()];
                transcript.push_str(&check_model(&live, &bytes));
                for step in 0..4 {
                    let target_set = if step > 0 {
                        let draft = OverlayDraft::new(&live, b"rich-multi");
                        if element == 10 {
                            run(|cx| draft.occurrence_set(element, "Pset_Test", cx))
                                .0
                                .unwrap()
                                .unwrap()
                                .1
                        } else {
                            draft
                                .type_set(element, &draft.stmt(element).unwrap(), "Pset_Test")
                                .unwrap()
                                .1
                        }
                    } else {
                        40
                    };
                    let edits = match step {
                        0 => vec![
                            rich_edit(element, "Pset_Test", "Value", "first"),
                            rich_edit(element, "Pset_Test", "Value", "second"),
                        ],
                        1 => vec![Edit::Property {
                            element,
                            set: "Pset_Test".into(),
                            name: "Added".into(),
                            value: NewValue {
                                type_name: "IfcLinearVelocityMeasure".into(),
                                literal: crate::NewLiteral::Real(3.5),
                            },
                        }],
                        2 => vec![Edit::Attribute {
                            element: target_set,
                            attribute: RootAttribute::Description,
                            value: Some("new description".into()),
                        }],
                        _ => vec![
                            Edit::Attribute {
                                element: target_set,
                                attribute: RootAttribute::Name,
                                value: Some("Renamed".into()),
                            },
                            rich_edit(element, "Renamed", "Value", "third"),
                        ],
                    };
                    let current = versions.last().unwrap();
                    let file = run(|cx| Ifc::read(current, cx)).0.unwrap();
                    let expected = run(|cx| crate::apply(&file, &edits, b"rich-multi", cx));
                    let actual =
                        record_budget(&mut transcript, |cx| live.apply(&edits, b"rich-multi", cx));
                    let expected_value = expected.0.unwrap();
                    assert_eq!(actual.unwrap(), expected_value.changed);
                    assert!(expected.1.is_empty());
                    transcript.push_str(&check_model(&live, &expected_value.bytes));
                    versions.push(expected_value.bytes);
                }
                for version in versions.iter().rev().skip(1) {
                    assert!(record_budget(&mut transcript, |cx| live.undo(cx)).unwrap());
                    transcript.push_str(&check_model(&live, version));
                }
                for version in versions.iter().skip(1) {
                    assert!(record_budget(&mut transcript, |cx| live.redo(cx)).unwrap());
                    transcript.push_str(&check_model(&live, version));
                }
            }
        }
        // stdout の比較記録で、ネイティブと WASI の全モデル・保存・履歴・予算を照らす。
        #[expect(clippy::print_stdout, reason = "テストの比較記録だけを出力する")]
        {
            println!("ifc-rich {:?}", Hash::document(transcript.as_bytes()));
        }
    }

    #[test]
    fn rich_model_copies_stop_before_apply_undo_and_redo_mutation() {
        let bytes = rich_fixture();
        let edits = [rich_edit(10, "Pset_Test", "Value", "changed")];
        check_model_copy_stops(&bytes, &edits, true);
    }

    #[test]
    fn predefined_model_copies_stop_before_apply_undo_and_redo_mutation() {
        let edits = [
            Edit::Attribute {
                element: 70,
                attribute: RootAttribute::Name,
                value: Some("Renamed".into()),
            },
            rich_edit(10, "Pset_Test", "Value", "changed"),
        ];
        for schema in ["IFC2X3", "IFC4", "IFC4X3"] {
            check_model_copy_stops(&predefined_fixture(schema, 0), &edits, false);
        }
    }

    fn check_model_copy_stops(bytes: &[u8], edits: &[Edit], check_depth: bool) {
        let perform = |live: &mut IfcLive, phase: u8, cx: &mut Cx| match phase {
            0 => live.apply(edits, b"rich-stop", cx).map(|count| count > 0),
            1 => live.undo(cx),
            _ => live.redo(cx),
        };
        for threads in [1, 4] {
            let mut live = run(|cx| IfcLive::open_with_threads(bytes, threads, cx))
                .0
                .unwrap();
            live.take_dirty_element_spans();
            for phase in 0..3 {
                let before = format!("{live:?}");
                let mut measured = Budget::default();
                let cancel = CancelToken::new();
                perform(
                    &mut live.clone(),
                    phase,
                    &mut Cx::new(&mut measured, &mut Diagnostics::new(), &cancel),
                )
                .unwrap();
                for kind in [Kind::Work, Kind::DecodedBytes, Kind::Records] {
                    let used = measured.used(kind);
                    if used == 0 {
                        continue;
                    }
                    for (limit, spent) in
                        [(0, 0), (used / 2, 0), (used - 1, 0), (used, 1), (used, 0)]
                    {
                        let mut budget =
                            Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
                        budget.spend(kind, spent).unwrap();
                        let mut candidate = live.clone();
                        let result = perform(
                            &mut candidate,
                            phase,
                            &mut Cx::new(&mut budget, &mut Diagnostics::new(), &cancel),
                        );
                        assert_eq!(
                            result.is_ok(),
                            limit >= used + spent,
                            "{phase} {kind:?} {limit} {spent}"
                        );
                        if result.is_err() {
                            assert_eq!(format!("{candidate:?}"), before);
                        }
                    }
                }
                if check_depth || phase == 0 {
                    let mut depth_budget =
                        Budget::new(Limits::default().with(Kind::Depth, Limit::Finite(0)));
                    let mut candidate = live.clone();
                    assert!(matches!(
                        perform(
                            &mut candidate,
                            phase,
                            &mut Cx::new(&mut depth_budget, &mut Diagnostics::new(), &cancel)
                        ),
                        Err(Error::Stopped(_))
                    ));
                    assert_eq!(format!("{candidate:?}"), before);
                    assert_eq!(depth_budget.used(Kind::Depth), 0);
                }
                cancel.cancel();
                let result = perform(
                    &mut live,
                    phase,
                    &mut Cx::new(&mut Budget::default(), &mut Diagnostics::new(), &cancel),
                );
                assert_eq!(result, Err(Error::Stopped(Stop::Cancelled)));
                assert_eq!(format!("{live:?}"), before);
                assert!(run(|cx| perform(&mut live, phase, cx)).0.unwrap());
            }
        }
    }

    #[test]
    fn rich_dependencies_include_complex_children_bounds_and_choices() {
        let mut live = run(|cx| IfcLive::open(&rich_fixture(), cx)).0.unwrap();
        let source = live.element_by_instance(11).unwrap();
        let target = live.element_by_instance(10).unwrap();
        assert!(
            run(|cx| live.affected_elements(&[source], cx))
                .0
                .unwrap()
                .contains(&target)
        );
        let graph = live.dep.clone();
        run(|cx| {
            live.apply(
                &[rich_edit(10, "Pset_Test", "Value", "changed")],
                b"rich-dep",
                cx,
            )
        })
        .0
        .unwrap();
        assert_eq!(live.dep, graph);
        assert!(run(|cx| live.undo(cx)).0.unwrap());
        assert_eq!(live.dep, graph);
        assert!(run(|cx| live.redo(cx)).0.unwrap());
        assert_eq!(live.dep, graph);
        let ref_value = || TypedValue {
            type_name: None,
            value: Literal::Element(source),
        };
        let details = PropertyDetails {
            sets: vec![DetailedPropertySet {
                name: "refs".into(),
                description: None,
                source_instance: None,
                properties: vec![
                    DetailedProperty {
                        parent: None,
                        name: "bounds".into(),
                        description: None,
                        source_instance: None,
                        value: PropertyDetailValue::Bounded {
                            lower: Some(ref_value()),
                            upper: None,
                            set_point: None,
                            unit: None,
                        },
                    },
                    DetailedProperty {
                        parent: None,
                        name: "choice".into(),
                        description: None,
                        source_instance: None,
                        value: PropertyDetailValue::Enumerated {
                            values: None,
                            choices: Some(zumen_building::PropertyChoices {
                                name: "refs".into(),
                                values: vec![ref_value()],
                                unit: None,
                                source_instance: None,
                            }),
                        },
                    },
                ],
            }],
        };
        let mut sources = BTreeSet::new();
        run(|cx| collect_detail_element_refs(Some(&details), &mut sources, cx))
            .0
            .unwrap();
        assert_eq!(sources, BTreeSet::from([RowId(source.0)]));
    }

    #[test]
    fn rich_model_copy_size_ignores_capacity_and_limits_nested_values() {
        let mut property = DetailedProperty {
            parent: None,
            name: "payload".into(),
            description: Some("description".into()),
            source_instance: Some(1),
            value: PropertyDetailValue::Single {
                value: Some(TypedValue {
                    type_name: Some("IfcLabel".into()),
                    value: Literal::List(vec![Literal::List(vec![Literal::Text(
                        "x".repeat(131_072),
                    )])]),
                }),
                unit: Some(Unit {
                    kind: "LENGTHUNIT".into(),
                    prefix: Some("MILLI".into()),
                    name: "METRE".into(),
                    scale: None,
                }),
            },
        };
        let check = |property: &DetailedProperty, cx: &mut Cx| {
            let mut copy = ModelCopy::new(cx);
            copy.detail_property(property)?;
            Ok::<_, Error>(copy.bytes)
        };
        let cancel = CancelToken::new();
        let mut measured = Budget::default();
        let bytes = check(
            &property,
            &mut Cx::new(&mut measured, &mut Diagnostics::new(), &cancel),
        )
        .unwrap();
        assert!(bytes > 131_072);
        property.name.reserve(1024);
        property.description.as_mut().unwrap().reserve(2048);
        if let PropertyDetailValue::Single {
            value: Some(value),
            unit: Some(unit),
        } = &mut property.value
        {
            value.type_name.as_mut().unwrap().reserve(512);
            unit.name.reserve(128);
            if let Literal::List(values) = &mut value.value {
                values.reserve(128);
            }
        }
        let mut expanded = Budget::default();
        assert_eq!(
            check(
                &property,
                &mut Cx::new(&mut expanded, &mut Diagnostics::new(), &cancel)
            )
            .unwrap(),
            bytes
        );
        for kind in Kind::ALL {
            assert_eq!(measured.used(kind), expanded.used(kind));
        }
        for (kind, used) in [
            (Kind::Work, measured.used(Kind::Work)),
            (Kind::DecodedBytes, measured.used(Kind::DecodedBytes)),
            (Kind::Depth, 2),
        ] {
            for (limit, spent) in [(0, 0), (used - 1, 0), (used, 0), (used, 1), (used + 1, 1)] {
                let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
                budget.spend(kind, spent).unwrap();
                let result = check(
                    &property,
                    &mut Cx::new(&mut budget, &mut Diagnostics::new(), &cancel),
                );
                assert_eq!(result.is_ok(), limit >= used + spent);
                assert_eq!(
                    budget.used(Kind::Depth),
                    if kind == Kind::Depth { spent } else { 0 }
                );
            }
        }
        cancel.cancel();
        assert_eq!(
            check(
                &property,
                &mut Cx::new(&mut Budget::default(), &mut Diagnostics::new(), &cancel)
            ),
            Err(Error::Stopped(Stop::Cancelled))
        );
    }

    #[test]
    fn property_dependency_lists_share_work_allocation_and_depth_limits() {
        let literal = Literal::List(vec![
            Literal::Element(ElementRef(1)),
            Literal::List(vec![
                Literal::Element(ElementRef(2)),
                Literal::Element(ElementRef(1)),
            ]),
        ]);
        let mut measured = Budget::default();
        let cancel = CancelToken::new();
        let mut complete = BTreeSet::new();
        collect_literal_element_refs(
            &literal,
            &mut complete,
            &mut Cx::new(&mut measured, &mut Diagnostics::new(), &cancel),
        )
        .unwrap();
        assert_eq!(complete, BTreeSet::from([RowId(1), RowId(2)]));
        for (kind, used) in [
            (Kind::Work, measured.used(Kind::Work)),
            (Kind::DecodedBytes, measured.used(Kind::DecodedBytes)),
            (Kind::Depth, 2),
        ] {
            for (limit, spent) in [(0, 0), (used - 1, 0), (used, 0), (used, 1), (used + 1, 1)] {
                let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
                budget.spend(kind, spent).unwrap();
                let mut out = BTreeSet::new();
                let mut diagnostics = Diagnostics::new();
                let result = collect_literal_element_refs(
                    &literal,
                    &mut out,
                    &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
                );
                assert_eq!(
                    result.is_ok(),
                    limit >= used + spent,
                    "{kind:?}={limit} spent={spent}"
                );
                if result.is_ok() {
                    assert_eq!(out, complete);
                } else {
                    assert!(matches!(result, Err(Error::Stopped(_))));
                }
                assert_eq!(
                    budget.used(Kind::Depth),
                    if kind == Kind::Depth { spent } else { 0 }
                );
            }
        }
        cancel.cancel();
        let mut out = BTreeSet::new();
        let result = collect_literal_element_refs(
            &literal,
            &mut out,
            &mut Cx::new(&mut Budget::default(), &mut Diagnostics::new(), &cancel),
        );
        assert_eq!(result, Err(Error::Stopped(zumen_base::Stop::Cancelled)));
        assert!(out.is_empty());
    }

    #[test]
    fn property_dependency_changes_keep_model_bytes_history_and_stop_atomically() {
        let bytes = String::from_utf8(fixture())
            .unwrap()
            .replace("IFCLABEL('before')", "#11")
            .into_bytes();
        let mut transcript = String::new();
        for threads in [1, 4] {
            let mut live = record_budget(&mut transcript, |cx| {
                IfcLive::open_with_threads(&bytes, threads, cx)
            })
            .unwrap();
            let dependent = live.element_by_instance(10).unwrap();
            let source = live.element_by_instance(11).unwrap();
            assert_eq!(
                run(|cx| live.affected_elements(&[source], cx)).0.unwrap(),
                [dependent]
            );
            transcript.push_str(&check_model(&live, &bytes));
            let before = live.building().clone();
            let graph = live.dep.clone();
            let change = [Edit::Property {
                element: 10,
                set: "Pset_Test".into(),
                name: "Value".into(),
                value: NewValue {
                    type_name: "IfcLabel".into(),
                    literal: crate::NewLiteral::Text("no reference".into()),
                },
            }];
            let mut measured = Budget::default();
            let cancel = CancelToken::new();
            live.clone()
                .apply(
                    &change,
                    b"dependency",
                    &mut Cx::new(&mut measured, &mut Diagnostics::new(), &cancel),
                )
                .unwrap();
            for kind in [Kind::Work, Kind::DecodedBytes, Kind::Records] {
                let used = measured.used(kind);
                assert!(used > 0);
                for limit in [0, used / 2, used - 1] {
                    let mut budget =
                        Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
                    let result = live.apply(
                        &change,
                        b"dependency",
                        &mut Cx::new(&mut budget, &mut Diagnostics::new(), &cancel),
                    );
                    assert!(matches!(result, Err(Error::Stopped(_))), "{kind:?} {limit}");
                    assert_eq!(live.building(), &before);
                    assert_eq!(live.dep, graph);
                    assert_eq!((live.undo_len(), live.redo_len()), (0, 0));
                    assert_eq!(run(|cx| live.save_spf(cx)).0.unwrap(), bytes);
                }
            }
            let file = run(|cx| Ifc::read(&bytes, cx)).0.unwrap();
            let expected = run(|cx| crate::apply(&file, &change, b"dependency", cx));
            let actual = run(|cx| live.apply(&change, b"dependency", cx));
            assert_eq!(actual.1, expected.1);
            let expected = expected.0.unwrap();
            assert_eq!(actual.0.unwrap(), expected.changed);
            transcript.push_str(&check_model(&live, &expected.bytes));
            let affected =
                record_budget(&mut transcript, |cx| live.affected_elements(&[source], cx)).unwrap();
            assert!(affected.is_empty());
            assert!(record_budget(&mut transcript, |cx| live.undo(cx)).unwrap());
            assert_eq!(live.dep, graph);
            transcript.push_str(&check_model(&live, &bytes));
            let affected =
                record_budget(&mut transcript, |cx| live.affected_elements(&[source], cx)).unwrap();
            assert_eq!(affected, [dependent]);
            assert!(record_budget(&mut transcript, |cx| live.redo(cx)).unwrap());
            transcript.push_str(&check_model(&live, &expected.bytes));
            assert!(
                run(|cx| live.affected_elements(&[source], cx))
                    .0
                    .unwrap()
                    .is_empty()
            );
        }
        #[allow(
            clippy::disallowed_methods,
            reason = "テストの比較記録の出力先だけを読む"
        )]
        let destination = std::env::var("ZUMEN_IFC_LIVE_DEP_OUT");
        if let Ok(path) = destination {
            std::fs::write(path, transcript).unwrap();
        }
    }

    fn check_model(live: &IfcLive, bytes: &[u8]) -> String {
        let mut transcript = format!(
            "{bytes:?}\n{:?}\n{} {:?} {} {}\n",
            live.building(),
            live.history.bytes(),
            live.revision(),
            live.undo_len(),
            live.redo_len()
        );
        let file = run(|cx| Ifc::read(bytes, cx)).0.unwrap();
        let expected: Building = run(|cx| file.lift(cx)).0.unwrap();
        assert_eq!(live.building(), &expected);
        assert_eq!(run(|cx| live.save_spf(cx)).0.unwrap(), bytes);
        for (index, _) in expected.elements().iter().enumerate() {
            let element = ElementRef(index as u32);
            let current = run(|cx| Hash::element_checked(live.building(), element, cx));
            let reference = run(|cx| Hash::element_checked(&expected, element, cx));
            assert_eq!(current, reference);
            transcript.push_str(&format!("{current:?}\n"));
        }
        #[cfg(feature = "ids")]
        {
            // このテスト専用の最小宣言。配布用の IFC スキーマではない。
            let schema = run(|cx| Express::read(b"SCHEMA IFC4; TYPE IfcLabel = STRING; END_TYPE; ENTITY IfcWall; GlobalId : IfcLabel; OwnerHistory : OPTIONAL IfcWall; Name : OPTIONAL IfcLabel; Description : OPTIONAL IfcLabel; END_ENTITY; END_SCHEMA;", cx)).0.unwrap();
            let current = live.ids_lookup(Some(&schema));
            let reference = crate::IdsLookup::new(&file, Some(&schema));
            for (index, _) in expected.elements().iter().enumerate() {
                let element = ElementRef(index as u32);
                let collect = |lookup: &dyn Lookup, model: &Building| {
                    let mut budget = Budget::default();
                    let mut diagnostics = Diagnostics::new();
                    let cancel = CancelToken::new();
                    let values = lookup.attributes(
                        model,
                        element,
                        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
                    );
                    (
                        values,
                        diagnostics.into_vec(),
                        budget.used(Kind::Work),
                        budget.used(Kind::Records),
                    )
                };
                let values = collect(&current, live.building());
                assert_eq!(values, collect(&reference, &expected));
                transcript.push_str(&format!("{values:?}\n"));
            }
        }
        transcript
    }

    #[test]
    fn changed_statement_cache_is_atomic_and_follows_multi_step_history() {
        let edits = [
            Edit::Property {
                element: 10,
                set: "Pset_Test".to_owned(),
                name: "Value".to_owned(),
                value: NewValue {
                    type_name: "IfcLabel".to_owned(),
                    literal: crate::NewLiteral::Text("after".to_owned()),
                },
            },
            Edit::Property {
                element: 10,
                set: "Pset_Test".to_owned(),
                name: "Value".to_owned(),
                value: NewValue {
                    type_name: "IfcLabel".to_owned(),
                    literal: crate::NewLiteral::Text("updated again".to_owned()),
                },
            },
            Edit::Attribute {
                element: 60,
                attribute: RootAttribute::Name,
                value: Some("Qto_Renamed".to_owned()),
            },
            Edit::Attribute {
                element: 10,
                attribute: RootAttribute::Name,
                value: Some("壁 'A'".to_owned()),
            },
            Edit::Attribute {
                element: 10,
                attribute: RootAttribute::Name,
                value: Some("壁 'B'".to_owned()),
            },
        ];
        let mut transcript = String::new();
        for threads in [1, 4] {
            let bytes = fixture();
            let mut live = record_budget(&mut transcript, |cx| {
                IfcLive::open_with_threads(&bytes, threads, cx)
            })
            .unwrap();
            let mut history = vec![bytes];
            transcript.push_str(&check_model(&live, &history[0]));
            for edit in &edits {
                let changes = std::slice::from_ref(edit);
                let before = live.building().clone();
                let revision = live.revision();
                let state = live.state_id();
                let cache = live.parsed_changed.clone();
                let undo = live.undo_len();
                let mut measured = Budget::default();
                let cancel = CancelToken::new();
                let mut probe = live.clone();
                probe
                    .apply(
                        changes,
                        b"budget",
                        &mut Cx::new(&mut measured, &mut Diagnostics::new(), &cancel),
                    )
                    .unwrap();
                for kind in Kind::ALL {
                    transcript.push_str(&format!("{kind:?}: {}\n", measured.used(kind)));
                }
                for kind in [Kind::Work, Kind::DecodedBytes, Kind::Records] {
                    let amount = measured.used(kind);
                    assert!(amount > 0);
                    for limit in [0, 1, amount / 2, amount - 1] {
                        let mut budget =
                            Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
                        let result = live.apply(
                            changes,
                            b"budget",
                            &mut Cx::new(&mut budget, &mut Diagnostics::new(), &cancel),
                        );
                        assert!(matches!(result, Err(Error::Stopped(_))), "{kind:?} {limit}");
                        assert_eq!(live.building(), &before);
                        assert_eq!(live.revision(), revision);
                        assert_eq!(live.state_id(), state);
                        assert_eq!(live.undo_len(), undo);
                        assert_eq!(live.redo_len(), 0);
                        assert_eq!(live.parsed_changed.len(), cache.len());
                        for (id, parsed) in &cache {
                            assert!(Arc::ptr_eq(parsed, &live.parsed_changed[id]));
                        }
                        assert_eq!(
                            run(|cx| live.save_spf(cx)).0.unwrap(),
                            *history.last().unwrap()
                        );
                    }
                }
                let file = run(|cx| Ifc::read(history.last().unwrap(), cx)).0.unwrap();
                let (expected, reference_diagnostics) =
                    run(|cx| crate::apply(&file, changes, b"budget", cx));
                let expected = expected.unwrap();
                let (actual, actual_diagnostics) = run(|cx| live.apply(changes, b"budget", cx));
                assert_eq!(actual.unwrap(), expected.changed);
                assert_eq!(actual_diagnostics, reference_diagnostics);
                transcript.push_str(&format!("{actual_diagnostics:?}\n"));
                transcript.push_str(&check_model(&live, &expected.bytes));
                history.push(expected.bytes);
            }
            for bytes in history.iter().rev().skip(1) {
                assert!(record_budget(&mut transcript, |cx| live.undo(cx)).unwrap());
                transcript.push_str(&check_model(&live, bytes));
            }
            assert!(live.parsed_changed.is_empty());
            for bytes in history.iter().skip(1) {
                assert!(record_budget(&mut transcript, |cx| live.redo(cx)).unwrap());
                transcript.push_str(&check_model(&live, bytes));
            }
        }
        #[allow(
            clippy::disallowed_methods,
            reason = "テストの比較記録の出力先だけを読む"
        )]
        let destination = std::env::var("ZUMEN_IFC_LIVE_BUDGET_OUT");
        if let Ok(path) = destination {
            std::fs::write(path, transcript).unwrap();
        }
    }
}
