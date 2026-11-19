//! 1 つの DWG オブジェクトの読み方と差分書き戻しの共通入口。
//!
//! `AcadDb`（全オブジェクトの `Vec<Object>`）を常駐させずに、表のレコード・クラス・ヘッダーだけを軽く索引化し、
//! 個々のレコードを並列に復号したり、1 図形の変更・追加・削除を差分で符号化したりするために使う。

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use zumen_base::{Cx, Diagnostic, Kind, Ref, codes};
use zumen_sdk::{Codec, Error, Transact, Transaction};

use super::bits::{Bits, modular_char, push_modular_char};
use super::header::{self, Header};
use super::kinds::{
    BLOCK, BLOCK_RECORD, DICTIONARY, DIMSTYLE, LAYER, LTYPE, MLINESTYLE, STYLE, VIEWPORT,
};
use super::objects::{
    BLOCK_FIELDS, COMMON_ENTITY, Fields, check_copy, check_values_copy, code_page_name,
    is_table_record, kind as builtin_kind, number_anonymous, read as read_fields, unmetered_copy,
};
use super::objects::{entity_data_entmode, skip_extended_data};
use super::write_back::{can_unlink_single, encode_born_entities, encode_single_entity};
use super::{COMMON_OBJECT, Dwg, DwgFile, Entry, Release, stopped, stopped_at};
use crate::db::{Data, Object, Origin, Section, Value};

/// 符号化した追加レコードの列と次の `HANDSEED`
pub type EncodedBorn = (Vec<(u64, Vec<u8>)>, u64);

/// 差分書き戻しで更新する 1 レコードの `(handle, section, kind, old_values, new_values)`
pub type UpdatedDwgRecord = (u64, Section, Vec<u8>, Vec<Value>, Vec<Value>);

/// 復号した 1 レコードの種類・セクション・値。
type DecodedRecord = (Vec<u8>, Section, Vec<Value>);

fn check_decode_input(bytes: usize, cx: &mut Cx<'_>) -> Result<(), Error> {
    let bytes = bytes as u64;
    cx.check(Kind::Work, bytes.saturating_mul(8).saturating_add(1))
        .map_err(|stop| stopped(cx, stop))?;
    // ビット列から作る値と一時領域の論理サイズ。表から補う値はコピー前にも検査する。
    cx.check(
        Kind::DecodedBytes,
        bytes.saturating_mul(128).saturating_add(1024),
    )
    .map_err(|stop| stopped(cx, stop))
}

fn check_decoded_values(kind: &[u8], values: &[Value], cx: &mut Cx<'_>) -> Result<(), Error> {
    cx.check(Kind::Work, values.len() as u64)
        .map_err(|stop| stopped(cx, stop))?;
    let payload = values.iter().fold(kind.len() as u64, |size, value| {
        size.saturating_add(match &value.data {
            Data::Text(bytes) | Data::Binary(bytes) => bytes.len() as u64,
            _ => 0,
        })
    });
    cx.check(Kind::Work, payload)
        .map_err(|stop| stopped(cx, stop))?;
    cx.check(Kind::DecodedBytes, payload)
        .map_err(|stop| stopped(cx, stop))
}

fn report_decode_error(error: Error, cx: &mut Cx<'_>) -> Error {
    match error {
        Error::Stopped(stop) => stopped(cx, stop),
        other => other,
    }
}

/// 書き戻したオブジェクトのハンドルと出所のバイト範囲。
#[cfg(any(feature = "drawing", test))]
pub(crate) type DwgSourceSpans = BTreeMap<u64, Range<usize>>;

/// 保存と出所の位置計算へ渡す、1 回分の変更。
pub(crate) struct DwgFastStep<'a> {
    pub updated: &'a [UpdatedDwgRecord],
    pub removed: &'a BTreeSet<u64>,
    pub removed_model: &'a BTreeSet<u64>,
    pub added: &'a [(u64, Vec<u8>, bool)],
    pub seed: Option<u64>,
    pub point_mode: Option<i16>,
    pub point_size: Option<f64>,
}

struct DwgMutation {
    changes: BTreeMap<usize, Vec<u8>>,
    removed: BTreeSet<usize>,
    added: Vec<(u64, Vec<u8>, bool)>,
    seed: Option<u64>,
    values: header::Values,
}

trait FastStepTarget {
    fn prepare_step(&self, _: &DwgFastStep<'_>, _: &mut Cx<'_>) -> Result<(), Error> {
        Ok(())
    }
    fn entries(&self) -> &[Entry];
    fn encoded(&self, single: &DwgSingle, record: usize, cx: &mut Cx<'_>)
    -> Result<Vec<u8>, Error>;
    #[allow(clippy::too_many_arguments)]
    fn patch_block(
        &self,
        record: usize,
        handle: u64,
        records: &BTreeMap<u64, usize>,
        changes: &mut BTreeMap<usize, Vec<u8>>,
        removed: &BTreeSet<u64>,
        added: &mut [(u64, Vec<u8>, bool)],
        cx: &mut Cx<'_>,
    ) -> Result<(), Error>;
    fn commit_step(&mut self, mutation: DwgMutation, cx: &mut Cx<'_>) -> Result<(), Error>;

    fn handle_map(&self) -> BTreeMap<u64, usize> {
        let mut records = BTreeMap::new();
        for (index, entry) in self.entries().iter().enumerate() {
            records.entry(entry.handle).or_insert(index);
        }
        records
    }
}

impl FastStepTarget for DwgFile {
    fn entries(&self) -> &[Entry] {
        &self.entries
    }

    fn encoded(&self, single: &DwgSingle, record: usize, _: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
        single.base_encoded(self, record).ok_or(Error::Rejected)
    }

    fn patch_block(
        &self,
        record: usize,
        handle: u64,
        records: &BTreeMap<u64, usize>,
        changes: &mut BTreeMap<usize, Vec<u8>>,
        removed: &BTreeSet<u64>,
        added: &mut [(u64, Vec<u8>, bool)],
        _: &mut Cx<'_>,
    ) -> Result<(), Error> {
        self.patch_model_block_and_added(
            record,
            handle,
            |h| records.get(&h).copied(),
            changes,
            removed,
            added,
        )
        .ok_or(Error::Rejected)
    }

    fn commit_step(&mut self, mutation: DwgMutation, cx: &mut Cx<'_>) -> Result<(), Error> {
        let mut txn = self.transaction();
        for (record, data) in mutation.changes {
            if !mutation.removed.contains(&record) {
                txn.set_data(record, &data)?;
            }
        }
        for record in mutation.removed {
            txn.remove(record)?;
        }
        for (handle, data, _) in mutation.added {
            txn.add(handle, &data)?;
        }
        if let Some(seed) = mutation.seed {
            txn.set_handle_seed(seed)?;
        }
        if let Some(mode) = mutation.values.point_mode {
            txn.set_point_mode(mode)?;
        }
        if let Some(size) = mutation.values.point_size {
            txn.set_point_size(size)?;
        }
        txn.commit(cx)
    }
}

/// 保存前の保持モデルに付ける出所の位置。元のバイト列は借用し、変更したデータだけを持つ。
#[cfg(any(feature = "drawing", test))]
pub(crate) struct DwgSourceLayout<'a> {
    base: &'a DwgFile,
    entries: Vec<Entry>,
    origins: Vec<Option<usize>>,
    changed: BTreeMap<usize, Vec<u8>>,
    header: Vec<u8>,
    cursor: usize,
}

#[cfg(any(feature = "drawing", test))]
impl<'a> DwgSourceLayout<'a> {
    pub fn new(base: &'a DwgFile, cx: &mut Cx<'_>) -> Result<Self, Error> {
        let count = base.entries.len() as u64;
        layout_check(cx, count, count.saturating_mul(112))?;
        let header = match base.header_section() {
            Some((bytes, range)) => {
                crate::snapshot::bytes(bytes.get(range).ok_or(Error::Rejected)?, cx)?
            }
            None => Vec::new(),
        };
        Ok(Self {
            base,
            entries: base.entries.clone(),
            origins: (0..base.entries.len()).map(Some).collect(),
            changed: BTreeMap::new(),
            header,
            cursor: base.body().len(),
        })
    }

    pub fn source_spans(&self, cx: &mut Cx<'_>) -> Result<DwgSourceSpans, Error> {
        let mut spans = BTreeMap::new();
        for entry in &self.entries {
            layout_check(cx, 1, 64)?;
            spans
                .entry(entry.handle)
                .or_insert_with(|| entry.span.clone());
        }
        Ok(spans)
    }

    pub fn base_span_shifts(
        &self,
        cx: &mut Cx<'_>,
    ) -> Result<BTreeMap<(u64, u64), Range<usize>>, Error> {
        let mut shifts = BTreeMap::new();
        for (entry, origin) in self.entries.iter().zip(&self.origins) {
            if let Some(orig_idx) = *origin
                && let Some(base_entry) = self.base.entries.get(orig_idx)
                && base_entry.span != entry.span
            {
                layout_check(cx, 1, 64)?;
                shifts
                    .entry((base_entry.span.start as u64, base_entry.span.end as u64))
                    .or_insert_with(|| entry.span.clone());
            }
        }
        Ok(shifts)
    }

    fn raw_data(&self, record: usize) -> Option<&[u8]> {
        let entry = self.entries.get(record)?;
        if let Some(encoded) = self.changed.get(&record) {
            encoded.get(encoded.len().checked_sub(entry.data.len())?..)
        } else {
            self.base
                .entry_raw_data(self.origins.get(record).copied().flatten()?)
        }
    }
}

#[cfg(any(feature = "drawing", test))]
fn layout_check(cx: &mut Cx<'_>, work: u64, bytes: u64) -> Result<(), Error> {
    cx.check(Kind::Work, work)
        .map_err(|stop| stopped(cx, stop))?;
    cx.check(Kind::DecodedBytes, bytes)
        .map_err(|stop| stopped(cx, stop))
}

#[cfg(any(feature = "drawing", test))]
impl FastStepTarget for DwgSourceLayout<'_> {
    fn prepare_step(&self, step: &DwgFastStep<'_>, cx: &mut Cx<'_>) -> Result<(), Error> {
        let count = self.entries.len() as u64;
        layout_check(
            cx,
            count
                .saturating_mul(count.checked_ilog2().unwrap_or(0).into())
                .saturating_add(1),
            count.saturating_mul(96),
        )?;
        for (_, _, kind, before, after) in step.updated {
            layout_check(cx, 1, kind.len() as u64)?;
            for value in before.iter().chain(after) {
                let bytes = match &value.data {
                    Data::Text(bytes) | Data::Binary(bytes) => bytes.len() as u64,
                    _ => 0,
                };
                layout_check(cx, 1, bytes.saturating_add(32).saturating_mul(8))?;
            }
        }
        for (_, data, _) in step.added {
            layout_check(
                cx,
                1,
                (data.len() as u64).saturating_add(64).saturating_mul(8),
            )?;
        }
        Ok(())
    }

    fn entries(&self) -> &[Entry] {
        &self.entries
    }

    fn encoded(
        &self,
        single: &DwgSingle,
        record: usize,
        cx: &mut Cx<'_>,
    ) -> Result<Vec<u8>, Error> {
        let entry = self.entries.get(record).ok_or(Error::Rejected)?;
        layout_check(
            cx,
            1,
            (entry.data.len() as u64)
                .saturating_add(16)
                .saturating_mul(8),
        )?;
        if let Some(encoded) = self.changed.get(&record) {
            Ok(encoded.clone())
        } else {
            let origin = self
                .origins
                .get(record)
                .copied()
                .flatten()
                .ok_or(Error::Rejected)?;
            single
                .base_encoded(self.base, origin)
                .ok_or(Error::Rejected)
        }
    }

    fn patch_block(
        &self,
        record: usize,
        handle: u64,
        records: &BTreeMap<u64, usize>,
        changes: &mut BTreeMap<usize, Vec<u8>>,
        removed: &BTreeSet<u64>,
        added: &mut [(u64, Vec<u8>, bool)],
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        // 既存のリンク更新器へ渡す一時の見え方。R2004 以降はブロックのデータだけ、
        // R2000 は前後リンクの走査に使うデータを置く。元のファイルや保存列は複製しない。
        layout_check(
            cx,
            self.entries.len() as u64,
            (self.entries.len() as u64).saturating_mul(96),
        )?;
        let mut entries = self.entries.clone();
        let mut src = Vec::new();
        for (index, entry) in entries.iter_mut().enumerate() {
            if index == record || self.base.release == Release::R2000 {
                let data = self.raw_data(index).ok_or(Error::Rejected)?;
                layout_check(cx, 1, (data.len() as u64).saturating_mul(8))?;
                let start = src.len();
                src.extend_from_slice(data);
                entry.data = start..src.len();
            } else {
                entry.data = 0..0;
            }
        }
        let view = DwgFile {
            src,
            release: self.base.release,
            code_page: self.base.code_page,
            locators: Vec::new(),
            container: None,
            r2007: None,
            classes: Vec::new(),
            entries,
        };
        view.patch_model_block_and_added(
            record,
            handle,
            |h| records.get(&h).copied(),
            changes,
            removed,
            added,
        )
        .ok_or(Error::Rejected)
    }

    fn commit_step(&mut self, mutation: DwgMutation, cx: &mut Cx<'_>) -> Result<(), Error> {
        use super::edit::{object_layout, object_map_len, rewrite_header};
        if mutation.changes.is_empty()
            && mutation.removed.is_empty()
            && mutation.added.is_empty()
            && mutation.seed.is_none()
            && !mutation.values.any()
        {
            return Ok(());
        }
        let count = self.entries.len() as u64;
        layout_check(
            cx,
            count,
            count
                .saturating_mul(160)
                .saturating_add(self.header.len() as u64 * 3),
        )?;
        let header_start = (!self.header.is_empty()).then_some(0);
        let rewritten = rewrite_header(
            &mut self.header,
            header_start,
            self.base.release,
            mutation.seed,
            mutation.values,
            cx,
        )?;
        for (record, data) in mutation.changes {
            if mutation.removed.contains(&record) {
                continue;
            }
            let entry = self.entries.get_mut(record).ok_or(Error::Rejected)?;
            *entry = object_layout(self.cursor, entry.handle, &data, self.base.release)
                .ok_or(Error::Rejected)?;
            self.cursor = entry.span.end;
            self.changed.insert(record, data);
        }
        for (handle, data, _) in mutation.added {
            let entry = object_layout(self.cursor, handle, &data, self.base.release)
                .ok_or(Error::Rejected)?;
            self.cursor = entry.span.end;
            self.changed.insert(self.entries.len(), data);
            self.entries.push(entry);
            self.origins.push(None);
        }
        let mut entries = Vec::with_capacity(self.entries.len());
        let mut origins = Vec::with_capacity(self.origins.len());
        let mut changed = BTreeMap::new();
        for (record, (entry, origin)) in self
            .entries
            .drain(..)
            .zip(self.origins.drain(..))
            .enumerate()
        {
            if !mutation.removed.contains(&record) {
                if let Some(data) = self.changed.remove(&record) {
                    changed.insert(entries.len(), data);
                }
                entries.push(entry);
                origins.push(origin);
            }
        }
        self.entries = entries;
        self.origins = origins;
        self.changed = changed;
        if let Some((span, section)) = rewritten {
            if self.base.release == Release::R2000 && span.len() != section.len() {
                self.cursor = self
                    .cursor
                    .checked_add(section.len())
                    .ok_or(Error::Rejected)?;
                self.header = section;
            } else {
                self.header.splice(span, section);
            }
        }
        if self.base.release == Release::R2000 {
            self.cursor = self
                .cursor
                .checked_add(object_map_len(&self.entries).ok_or(Error::Rejected)?)
                .ok_or(Error::Rejected)?;
        }
        Ok(())
    }
}

/// 図形レコードの所属空間の分類
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntitySpace {
    /// モデル空間の最上位の図形
    Model,
    /// ペーパー空間の最上位の図形（`entmode == 1`）
    Paper,
    /// 親の図形（INSERT や POLYLINE）に付く子レコード（モデル空間側）
    ModelFollower {
        /// 直接の所有者のハンドル
        owner: u64,
    },
    /// ブロック定義の中に属する図形（またはその子レコード）
    BlockDefinition,
}

/// 1 つのレコードの軽い見出し情報
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryMeta<'a> {
    /// オブジェクトのハンドル
    pub handle: u64,
    /// ファイル（または `AcDb:AcDbObjects`）内のバイト範囲
    pub span: Range<usize>,
    /// DXF の種類名（未知の型は空）
    pub kind: &'a [u8],
    /// 所属するセクション（図形がブロック定義に移る前の初期区分）
    pub section: Section,
}

/// `AcadDb` を常駐させずに DWG の個別レコードの復号と差分書き戻しを行う索引
#[derive(Clone, Debug)]
pub struct DwgSingle {
    release: Release,
    version: Vec<u8>,
    code_page: Option<&'static [u8]>,
    header: Option<Header>,
    handle_seed: Option<u64>,
    classes: BTreeMap<u16, (Vec<u8>, Section)>,
    names: BTreeMap<u64, (u16, Vec<u8>)>,
    by_table_name: BTreeMap<u16, BTreeMap<Vec<u8>, u64>>,
    books: BTreeMap<u64, Vec<Value>>,
    block_fields: BTreeMap<u64, Vec<Value>>,
    viewport_numbers: BTreeMap<usize, i64>,
    material_bylayer: Option<u64>,
    mtext_handles: BTreeSet<u64>,
    /// ハンドルからレコード番号を引くソート済み配列（重複時は最後のレコード番号）
    handle_to_record: Vec<(u64, u32)>,
    has_duplicate_handle: bool,
    /// 各レコードを直接の所有者（330）として指すオブジェクトの件数（255 以上は `owner_counts_large`）
    owner_counts: Vec<u8>,
    owner_counts_large: BTreeMap<usize, u32>,
    owner_counts_by_handle: BTreeMap<u64, u32>,
    /// モデル空間の `BLOCK_RECORD` の `(record, handle)`（追加・削除の書き戻しが可能なとき `Some`）
    model_block: Option<(usize, u64)>,
    /// モデル空間の `BLOCK_RECORD` の図形列に 1 度だけ現れるレコード番号のビット集合
    model_owned_bits: Vec<u64>,
}

fn bit_get(bits: &[u64], index: usize) -> bool {
    let word = index / 64;
    let bit = index % 64;
    bits.get(word).is_some_and(|w| (w >> bit) & 1 != 0)
}

fn bit_set(bits: &mut [u64], index: usize) {
    let word = index / 64;
    let bit = index % 64;
    if let Some(w) = bits.get_mut(word) {
        *w |= 1u64 << bit;
    }
}

fn bit_clear(bits: &mut [u64], index: usize) {
    let word = index / 64;
    let bit = index % 64;
    if let Some(w) = bits.get_mut(word) {
        *w &= !(1u64 << bit);
    }
}

fn table_code_of(name: &[u8]) -> Option<u16> {
    match name {
        b"LAYER" => Some(LAYER),
        b"LTYPE" => Some(LTYPE),
        b"STYLE" => Some(STYLE),
        b"DIMSTYLE" => Some(DIMSTYLE),
        b"BLOCK_RECORD" => Some(BLOCK_RECORD),
        _ => None,
    }
}

/// オブジェクトのデータから直接の所有者ハンドルだけを高速に読む
fn fast_direct_owner(
    data: &[u8],
    release: Release,
    entry: &Entry,
    entity: bool,
    header: Option<Header>,
) -> Option<u64> {
    let mut bits = Bits::with_release(data, release);
    let handles_at = match entry.handles {
        Some(at) => {
            bits.bot()?;
            bits.split_strings(at)?;
            at
        }
        None => {
            bits.bs()?;
            let at = usize::try_from(bits.rl()?).ok()?;
            if release == Release::R2007 {
                bits.split_strings(at)?;
            }
            at
        }
    };
    bits.handle()?;
    skip_extended_data(&mut bits)?;
    if entity {
        let entmode = entity_data_entmode(&mut bits, release)?;
        match entmode {
            0 => {
                bits.seek(handles_at);
                bits.handle()?.resolve(entry.handle)
            }
            1 => header.map(|h| h.paper),
            2 => header.map(|h| h.model),
            _ => None,
        }
    } else {
        bits.seek(handles_at);
        bits.handle()?.resolve(entry.handle)
    }
}

impl DwgSingle {
    /// `DwgFile` から軽い索引を組み立てる
    pub fn from_file(file: &DwgFile, cx: &mut Cx<'_>) -> Result<Self, Error> {
        let header = file
            .header_section()
            .and_then(|(src, span)| header::header(src, span, file.release, cx));
        let mut classes: BTreeMap<u16, (Vec<u8>, Section)> = BTreeMap::new();
        for class in &file.classes {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            if let std::collections::btree_map::Entry::Vacant(slot) = classes.entry(class.number) {
                let (name, sec) = class.kind();
                let extra = name.len() as u64
                    + match &sec {
                        Section::Other(other) => other.len() as u64,
                        _ => 0,
                    };
                cx.check(Kind::DecodedBytes, extra.saturating_add(64))
                    .map_err(|stop| stopped(cx, stop))?;
                slot.insert((name.to_vec(), sec));
            }
        }

        let n = file.entries.len();
        cx.check(Kind::Work, n as u64)
            .and_then(|()| cx.check(Kind::DecodedBytes, (n as u64).saturating_mul(16)))
            .map_err(|stop| stopped(cx, stop))?;
        let mut handle_to_record: Vec<(u64, u32)> = Vec::with_capacity(n);
        let mut sorted = true;
        let mut prev_h = 0u64;
        for (idx, entry) in file.entries.iter().enumerate() {
            if idx > 0 && entry.handle <= prev_h {
                sorted = false;
            }
            prev_h = entry.handle;
            handle_to_record.push((entry.handle, u32::try_from(idx).unwrap_or(u32::MAX)));
        }
        let mut has_duplicate_handle = false;
        if !sorted {
            let compares =
                (n as u64).saturating_mul(u64::from(u64::BITS - (n as u64).leading_zeros()));
            cx.check(Kind::Work, compares)
                .and_then(|()| cx.check(Kind::DecodedBytes, (n as u64).saturating_mul(16)))
                .map_err(|stop| stopped(cx, stop))?;
            handle_to_record.sort_by_key(|&(h, _)| h);
            let mut deduped: Vec<(u64, u32)> = Vec::with_capacity(handle_to_record.len());
            for (h, rec) in handle_to_record {
                if let Some(last) = deduped.last_mut()
                    && last.0 == h
                {
                    last.1 = rec;
                    has_duplicate_handle = true;
                } else {
                    deduped.push((h, rec));
                }
            }
            handle_to_record = deduped;
        }

        // 表のレコード・DBCOLOR・DICTIONARY・VIEWPORT だけを先に読む
        let mut table_indices: Vec<usize> = Vec::new();
        let mut table_fields: Vec<Option<Fields>> = Vec::new();
        let mut dbcolor_entries: Vec<(usize, Fields)> = Vec::new();
        let mut dict_entries: Vec<(usize, Fields)> = Vec::new();
        let mut viewport_records: Vec<usize> = Vec::new();
        let mut mtext_handles: BTreeSet<u64> = BTreeSet::new();

        for (idx, entry) in file.entries.iter().enumerate() {
            let place = Ref::at_bytes(entry.span.start as u64, entry.span.end as u64);
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped_at(cx, stop, place))?;
            let resolved = builtin_kind(entry.type_code).or_else(|| {
                classes
                    .get(&entry.type_code)
                    .map(|(name, sec)| (name.as_slice(), sec.clone()))
            });
            let Some((kname, sec)) = resolved else {
                continue;
            };
            if kname == b"MTEXT" {
                mtext_handles.insert(entry.handle);
            }
            if entry.type_code == VIEWPORT {
                viewport_records.push(idx);
            }
            if is_table_record(entry.type_code) || entry.type_code == MLINESTYLE {
                let raw = file.data(entry);
                check_decode_input(raw.len(), cx)?;
                let entity = matches!(sec, Section::Entities | Section::Blocks);
                let fields = read_fields(raw, file.release, entry, kname, entity, header);
                if let Some(f) = &fields {
                    check_decoded_values(kname, &f.values, cx)?;
                }
                table_indices.push(idx);
                table_fields.push(fields);
            } else if kname == b"DBCOLOR" {
                let raw = file.data(entry);
                check_decode_input(raw.len(), cx)?;
                if let Some(fields) = read_fields(raw, file.release, entry, kname, false, header) {
                    check_decoded_values(kname, &fields.values, cx)?;
                    dbcolor_entries.push((idx, fields));
                }
            } else if entry.type_code == DICTIONARY || kname == b"ACDBDICTIONARYWDFLT" {
                let raw = file.data(entry);
                check_decode_input(raw.len(), cx)?;
                if let Some(fields) = read_fields(raw, file.release, entry, kname, false, header) {
                    check_decoded_values(kname, &fields.values, cx)?;
                    dict_entries.push((idx, fields));
                }
            }
        }

        if let Some(hdr) = header {
            let anon_iter = table_indices
                .iter()
                .zip(table_fields.iter_mut())
                .filter_map(|(&idx, fields)| {
                    let entry = file.entries.get(idx)?;
                    (entry.type_code == BLOCK_RECORD
                        && entry.handle != hdr.model
                        && entry.handle != hdr.paper)
                        .then_some(fields.as_mut())
                });
            number_anonymous(anon_iter);
        }

        let mut names: BTreeMap<u64, (u16, Vec<u8>)> = BTreeMap::new();
        let mut by_table_name: BTreeMap<u16, BTreeMap<Vec<u8>, u64>> = BTreeMap::new();
        for (&idx, fields_opt) in table_indices.iter().zip(&table_fields) {
            let Some(entry) = file.entries.get(idx) else {
                continue;
            };
            let Some(fields) = fields_opt.as_ref() else {
                continue;
            };
            let Some(name_val) = fields.values.iter().find(|v| v.code == 2) else {
                continue;
            };
            if let Data::Text(name_bytes) = &name_val.data {
                check_copy(2, (name_bytes.len() as u64).saturating_mul(2), cx)
                    .map_err(|e| report_decode_error(e, cx))?;
                names.insert(entry.handle, (entry.type_code, name_bytes.clone()));
                if is_table_record(entry.type_code) {
                    by_table_name
                        .entry(entry.type_code)
                        .or_default()
                        .entry(name_bytes.to_ascii_lowercase())
                        .or_insert(entry.handle);
                }
            }
        }

        let mut books: BTreeMap<u64, Vec<Value>> = BTreeMap::new();
        for (idx, fields) in dbcolor_entries {
            if let Some(entry) = file.entries.get(idx) {
                check_values_copy(&fields.values, cx, &mut check_copy)
                    .map_err(|e| report_decode_error(e, cx))?;
                let book: Vec<Value> = fields
                    .values
                    .iter()
                    .filter(|v| matches!(v.code, 420 | 430))
                    .cloned()
                    .collect();
                books.insert(entry.handle, book);
            }
        }

        // BLOCK_RECORD から BLOCK へ移すフィールドを先に分けておく
        let mut block_fields: BTreeMap<u64, Vec<Value>> = BTreeMap::new();
        let mut quiet_diag = zumen_base::Diagnostics::new();
        for (&idx, fields_opt) in table_indices.iter().zip(table_fields) {
            let Some(entry) = file.entries.get(idx) else {
                continue;
            };
            if entry.type_code != BLOCK_RECORD {
                continue;
            }
            let Some(fields) = fields_opt else {
                continue;
            };
            let at = Ref::at_bytes(entry.span.start as u64, entry.span.end as u64);
            let (own, _) = fields
                .place_in(
                    &names,
                    &books,
                    entry,
                    &mut Cx::new(cx.budget, &mut quiet_diag, cx.cancel),
                    &at,
                    &mut check_copy,
                )
                .map_err(|error| report_decode_error(error, cx))?;
            let mut seen_70 = false;
            let mut blk = Vec::new();
            for val in own {
                let first_70 = val.code != 70 || !seen_70;
                seen_70 |= val.code == 70;
                if first_70 && BLOCK_FIELDS.contains(&val.code) {
                    blk.push(val);
                }
            }
            block_fields.insert(entry.handle, blk);
        }

        // DICTIONARY から ACAD_MATERIAL -> ByLayer を引く
        let mut dicts: Vec<(u64, Vec<Value>)> = Vec::with_capacity(dict_entries.len());
        for (idx, fields) in dict_entries {
            if let Some(entry) = file.entries.get(idx) {
                let at = Ref::at_bytes(entry.span.start as u64, entry.span.end as u64);
                let (own, _) = fields
                    .place_in(
                        &names,
                        &books,
                        entry,
                        &mut Cx::new(cx.budget, &mut quiet_diag, cx.cancel),
                        &at,
                        &mut check_copy,
                    )
                    .map_err(|error| report_decode_error(error, cx))?;
                dicts.push((entry.handle, own));
            }
        }
        let dict_item = |vals: &[Value], target: &[u8]| -> Option<u64> {
            vals.windows(2).find_map(|pair| match pair {
                [
                    Value {
                        code: 3,
                        data: Data::Text(found),
                    },
                    Value {
                        code: 350 | 360,
                        data: Data::Handle(h),
                    },
                ] if found.eq_ignore_ascii_case(target) => Some(*h),
                _ => None,
            })
        };
        let material_bylayer = dicts
            .iter()
            .find_map(|(_, vals)| dict_item(vals, b"ACAD_MATERIAL"))
            .and_then(|grp| dicts.iter().find(|(h, _)| *h == grp))
            .and_then(|(_, vals)| dict_item(vals, b"ByLayer"));

        // VIEWPORT の連番（69）
        let mut viewport_numbers: BTreeMap<usize, i64> = BTreeMap::new();
        let mut vp_by_owner: BTreeMap<u64, i64> = BTreeMap::new();
        for idx in viewport_records {
            if let Some(entry) = file.entries.get(idx)
                && let Some(owner) =
                    fast_direct_owner(file.data(entry), file.release, entry, true, header)
            {
                let num = vp_by_owner.entry(owner).or_default();
                *num += 1;
                viewport_numbers.insert(idx, *num);
            }
        }

        // 所有者の参照カウント（entmode == 1・2 は空間の BLOCK_RECORD を指すので飛ばす）
        let word_len = n.div_ceil(64);
        cx.check(Kind::Work, n as u64)
            .and_then(|()| {
                cx.check(
                    Kind::DecodedBytes,
                    (n as u64).saturating_add((word_len as u64).saturating_mul(16)),
                )
            })
            .map_err(|stop| stopped(cx, stop))?;
        let mut owner_counts = vec![0u8; n];
        let mut owner_counts_large: BTreeMap<usize, u32> = BTreeMap::new();
        let mut owner_counts_by_handle: BTreeMap<u64, u32> = BTreeMap::new();
        let lookup_rec = |h: u64| -> Option<usize> {
            handle_to_record
                .binary_search_by_key(&h, |&(k, _)| k)
                .ok()
                .and_then(|i| handle_to_record.get(i))
                .map(|&(_, r)| r as usize)
        };
        let space_handles = (header.map(|h| h.model), header.map(|h| h.paper));
        for entry in &file.entries {
            cx.check(Kind::Work, 1).map_err(|stop| {
                stopped_at(
                    cx,
                    stop,
                    Ref::at_bytes(entry.span.start as u64, entry.span.end as u64),
                )
            })?;
            let sec_opt = builtin_kind(entry.type_code)
                .map(|(_, s)| s)
                .or_else(|| classes.get(&entry.type_code).map(|(_, s)| s.clone()));
            let Some(sec) = sec_opt else {
                continue;
            };
            let entity = matches!(sec, Section::Entities | Section::Blocks);
            if entity {
                let mut bits = Bits::with_release(file.data(entry), file.release);
                let handles_at = match entry.handles {
                    Some(at) => {
                        if bits.bot().is_none() || bits.split_strings(at).is_none() {
                            continue;
                        }
                        at
                    }
                    None => {
                        if bits.bs().is_none() {
                            continue;
                        }
                        let Some(at) = bits.rl().and_then(|v| usize::try_from(v).ok()) else {
                            continue;
                        };
                        if file.release == Release::R2007 && bits.split_strings(at).is_none() {
                            continue;
                        }
                        at
                    }
                };
                if bits.handle().is_none() || skip_extended_data(&mut bits).is_none() {
                    continue;
                }
                let Some(entmode) = entity_data_entmode(&mut bits, file.release) else {
                    continue;
                };
                if entmode != 0 {
                    continue;
                }
                bits.seek(handles_at);
                let Some(owner) = bits.handle().and_then(|h| h.resolve(entry.handle)) else {
                    continue;
                };
                if Some(owner) == space_handles.0 || Some(owner) == space_handles.1 {
                    continue;
                }
                if has_duplicate_handle {
                    *owner_counts_by_handle.entry(owner).or_default() += 1;
                } else if let Some(parent_rec) = lookup_rec(owner)
                    && let Some(slot) = owner_counts.get_mut(parent_rec)
                {
                    if *slot < u8::MAX {
                        *slot += 1;
                    } else {
                        let large = owner_counts_large.entry(parent_rec).or_insert(255);
                        *large = large.saturating_add(1);
                    }
                }
            } else if let Some(owner) =
                fast_direct_owner(file.data(entry), file.release, entry, false, header)
            {
                if Some(owner) == space_handles.0 || Some(owner) == space_handles.1 {
                    continue;
                }
                if has_duplicate_handle {
                    *owner_counts_by_handle.entry(owner).or_default() += 1;
                } else if let Some(parent_rec) = lookup_rec(owner)
                    && let Some(slot) = owner_counts.get_mut(parent_rec)
                {
                    if *slot < u8::MAX {
                        *slot += 1;
                    } else {
                        let large = owner_counts_large.entry(parent_rec).or_insert(255);
                        *large = large.saturating_add(1);
                    }
                }
            }
        }

        // モデル空間の BLOCK_RECORD の図形列を検証する
        let mut model_owned_bits = vec![0u64; word_len];
        let mut model_owned_seen = vec![0u64; word_len];
        let model_block = header.and_then(|hdr| {
            let rec = lookup_rec(hdr.model)?;
            let entry = file.entries.get(rec)?;
            if entry.type_code != BLOCK_RECORD {
                return None;
            }
            let (_, name) = names.get(&hdr.model)?;
            if !name.eq_ignore_ascii_case(b"*Model_Space") {
                return None;
            }
            let owned_handles = file.model_block_owned_handles(rec, hdr.model, lookup_rec)?;
            for h in owned_handles {
                if let Some(ent_rec) = lookup_rec(h) {
                    if bit_get(&model_owned_seen, ent_rec) {
                        bit_clear(&mut model_owned_bits, ent_rec);
                    } else {
                        bit_set(&mut model_owned_seen, ent_rec);
                        bit_set(&mut model_owned_bits, ent_rec);
                    }
                }
            }
            Some((rec, hdr.model))
        });

        Ok(Self {
            release: file.release,
            version: file.version().to_vec(),
            code_page: Some(code_page_name(file.code_page)),
            header,
            handle_seed: file.handle_seed(),
            classes,
            names,
            by_table_name,
            books,
            block_fields,
            viewport_numbers,
            material_bylayer,
            mtext_handles,
            handle_to_record,
            has_duplicate_handle,
            owner_counts,
            owner_counts_large,
            owner_counts_by_handle,
            model_block,
            model_owned_bits,
        })
    }

    /// バージョンのバイト列（`AC1015`・`AC1032` など）
    pub fn version(&self) -> &[u8] {
        &self.version
    }

    /// DXF の `$DWGCODEPAGE` に対応するコード・ページ名
    pub fn code_page_name(&self) -> Option<&'static [u8]> {
        self.code_page
    }

    /// ヘッダーの `HANDSEED`
    pub fn handle_seed(&self) -> Option<u64> {
        self.handle_seed
    }

    /// モデル空間の `BLOCK_RECORD` のハンドル
    pub fn model_owner(&self) -> Option<u64> {
        self.header.map(|h| h.model)
    }

    /// 現在のペーパー空間の `BLOCK_RECORD` のハンドル
    pub fn paper_owner(&self) -> Option<u64> {
        self.header.map(|h| h.paper)
    }

    /// ヘッダーの `$INSUNITS`
    pub fn insunits(&self) -> Option<i64> {
        self.header.map(|h| i64::from(h.units))
    }

    /// ヘッダーの `$PLIMMIN`
    pub fn plimmin(&self) -> Option<[f64; 2]> {
        self.header.map(|h| [h.limits[0], h.limits[1]])
    }

    /// ヘッダーの `PDMODE`
    pub fn point_mode(&self) -> Option<i16> {
        self.header.map(|h| h.point_mode)
    }

    /// ヘッダーの `PDSIZE`
    pub fn point_size(&self) -> Option<f64> {
        self.header.map(|h| h.point_size)
    }

    /// `DwgFile::objects` と同じ規則でヘッダー変数を `db` に設定する
    pub fn populate_header_variables(&self, db: &mut crate::db::AcadDb) {
        db.set_variable(
            b"$ACADVER",
            vec![Value {
                code: 1,
                data: Data::Text(self.version.clone()),
            }],
        );
        if let Some(name) = self.code_page {
            db.set_variable(
                b"$DWGCODEPAGE",
                vec![Value {
                    code: 3,
                    data: Data::Text(name.to_vec()),
                }],
            );
        }
        if let Some(header) = self.header {
            let [x0, y0, x1, y1] = header.limits;
            db.set_variable(
                b"$INSUNITS",
                vec![Value {
                    code: 70,
                    data: Data::Int(i64::from(header.units)),
                }],
            );
            db.set_variable(
                b"$PLIMMIN",
                vec![
                    Value {
                        code: 10,
                        data: Data::Real(x0),
                    },
                    Value {
                        code: 20,
                        data: Data::Real(y0),
                    },
                ],
            );
            db.set_variable(
                b"$PLIMMAX",
                vec![
                    Value {
                        code: 10,
                        data: Data::Real(x1),
                    },
                    Value {
                        code: 20,
                        data: Data::Real(y1),
                    },
                ],
            );
            db.set_variable(
                b"$LTSCALE",
                vec![Value {
                    code: 40,
                    data: Data::Real(header.line_type_scale),
                }],
            );
            db.set_variable(
                b"$PDMODE",
                vec![Value {
                    code: 70,
                    data: Data::Int(i64::from(header.point_mode)),
                }],
            );
            db.set_variable(
                b"$PDSIZE",
                vec![Value {
                    code: 40,
                    data: Data::Real(header.point_size),
                }],
            );
        }
    }

    /// ハンドル重複が元のオブジェクト・マップにあったか
    pub fn has_duplicate_handle(&self) -> bool {
        self.has_duplicate_handle
    }

    /// `DwgFile` とこの索引が保持しているおおよそのヒープ・バイト数
    pub fn backing_bytes(&self, file: &DwgFile) -> usize {
        file.heap_bytes()
            + self.handle_to_record.capacity() * core::mem::size_of::<(u64, u32)>()
            + self.owner_counts.capacity()
            + self.model_owned_bits.capacity() * core::mem::size_of::<u64>()
    }

    /// ハンドルからレコード番号を引く
    pub fn record_of_handle(&self, handle: u64) -> Option<usize> {
        self.handle_to_record
            .binary_search_by_key(&handle, |&(k, _)| k)
            .ok()
            .and_then(|i| self.handle_to_record.get(i))
            .map(|&(_, r)| r as usize)
    }

    /// ハンドルから表レコードの名前（生のバイト列）と型を引く
    pub fn table_names(&self) -> impl Iterator<Item = (&'static [u8], &[u8])> + '_ {
        self.names.values().filter_map(|(code, name)| {
            let table: &'static [u8] = match *code {
                LAYER => b"LAYER",
                LTYPE => b"LTYPE",
                STYLE => b"STYLE",
                BLOCK_RECORD => b"BLOCK_RECORD",
                DIMSTYLE => b"DIMSTYLE",
                _ => return None,
            };
            Some((table, name.as_slice()))
        })
    }

    /// 表の種類（`b"LAYER"` など）と名前（ASCII 大文字・小文字無視）から最初のハンドルを引く
    pub fn lookup_table_handle(&self, table: &[u8], name: &[u8]) -> Option<u64> {
        let code = table_code_of(table)?;
        let map = self.by_table_name.get(&code)?;
        if !name.iter().any(u8::is_ascii_uppercase) {
            return map.get(name).copied();
        }
        map.get(&name.to_ascii_lowercase()).copied()
    }

    /// `record` 番目のレコードの見出し情報を返す
    pub fn entry_meta<'a>(&'a self, file: &'a DwgFile, record: usize) -> Option<EntryMeta<'a>> {
        let entry = file.entries.get(record)?;
        let (kind, section) = match builtin_kind(entry.type_code) {
            Some((k, s)) => (k, s),
            None => match self.classes.get(&entry.type_code) {
                Some((k, s)) => (k.as_slice(), s.clone()),
                None => (&b""[..], Section::Other(Vec::new())),
            },
        };
        Some(EntryMeta {
            handle: entry.handle,
            span: entry.span.clone(),
            kind,
            section,
        })
    }

    /// `Section::Entities` のレコードの所属空間（モデル・ペーパー・モデルの子・ブロック定義）を高速に判定する
    pub fn classify_entity(&self, file: &DwgFile, record: usize) -> Option<EntitySpace> {
        let entry = file.entries.get(record)?;
        let mut bits = Bits::with_release(file.data(entry), self.release);
        let handles_at = match entry.handles {
            Some(at) => {
                bits.bot()?;
                bits.split_strings(at)?;
                at
            }
            None => {
                bits.bs()?;
                let at = usize::try_from(bits.rl()?).ok()?;
                if self.release == Release::R2007 {
                    bits.split_strings(at)?;
                }
                at
            }
        };
        bits.handle()?;
        skip_extended_data(&mut bits)?;
        let entmode = entity_data_entmode(&mut bits, self.release)?;
        match entmode {
            2 => Some(EntitySpace::Model),
            1 => Some(EntitySpace::Paper),
            0 => {
                bits.seek(handles_at);
                let direct = bits.handle()?.resolve(entry.handle)?;
                let Some(hdr) = self.header else {
                    return Some(EntitySpace::Model);
                };
                if direct == hdr.model {
                    return Some(EntitySpace::Model);
                }
                if direct == hdr.paper {
                    // entmode == 0 で paper を直接指す図形は 67 = 1 を持たないためモデル空間に残る
                    return Some(EntitySpace::Model);
                }
                if self.in_block_definition(file, Some(direct)) {
                    Some(EntitySpace::BlockDefinition)
                } else {
                    Some(EntitySpace::ModelFollower { owner: direct })
                }
            }
            _ => None,
        }
    }

    fn in_block_definition(&self, file: &DwgFile, start_owner: Option<u64>) -> bool {
        let Some(hdr) = self.header else {
            return false;
        };
        let mut owner = start_owner;
        for _ in 0..file.entries.len() {
            let Some(o) = owner else {
                break;
            };
            let Some(rec) = self.record_of_handle(o) else {
                break;
            };
            let Some(entry) = file.entries.get(rec) else {
                break;
            };
            let sec = builtin_kind(entry.type_code)
                .map(|(_, s)| s)
                .or_else(|| self.classes.get(&entry.type_code).map(|(_, s)| s.clone()));
            if sec != Some(Section::Entities) {
                break;
            }
            owner = fast_direct_owner(file.data(entry), self.release, entry, true, self.header);
        }
        owner.is_some_and(|o| o != hdr.model && o != hdr.paper)
    }

    /// `record` 番目のレコードを `DwgFile::objects` と同じ規則で 1 件だけ復号する（スレッド安全）
    pub fn decode_record(&self, file: &DwgFile, record: usize, cx: &mut Cx<'_>) -> Option<Object> {
        match self.decode_record_in(file, record, cx, &mut unmetered_copy) {
            Ok(object) => object,
            Err(never) => match never {},
        }
    }

    fn decode_kind_in<E>(
        &self,
        type_code: u16,
        cx: &mut Cx<'_>,
        before_copy: &mut impl FnMut(u64, u64, &mut Cx) -> Result<(), E>,
    ) -> Result<Option<(&[u8], Section)>, E> {
        if let Some(kind) = builtin_kind(type_code) {
            return Ok(Some(kind));
        }
        let Some((kind, section)) = self.classes.get(&type_code) else {
            return Ok(None);
        };
        if let Section::Other(name) = section {
            before_copy(1, name.len() as u64, cx)?;
        }
        Ok(Some((kind, section.clone())))
    }

    fn decode_record_in<E>(
        &self,
        file: &DwgFile,
        record: usize,
        cx: &mut Cx<'_>,
        before_copy: &mut impl FnMut(u64, u64, &mut Cx) -> Result<(), E>,
    ) -> Result<Option<Object>, E> {
        let Some(entry) = file.entries.get(record) else {
            return Ok(None);
        };
        let at = Ref::at_bytes(entry.span.start as u64, entry.span.end as u64);
        let resolved = self.decode_kind_in(entry.type_code, cx, before_copy)?;
        let handle_code = if entry.type_code == DIMSTYLE { 105 } else { 5 };
        let mut values = vec![Value {
            code: handle_code,
            data: Data::Handle(entry.handle),
        }];
        let mut extra = Vec::new();
        let (kind_vec, mut section) = match resolved {
            Some((kname, sec)) => {
                let entity = matches!(sec, Section::Entities | Section::Blocks);
                let mut fields = read_fields(
                    file.data(entry),
                    self.release,
                    entry,
                    kname,
                    entity,
                    self.header,
                );
                if let (BLOCK_RECORD, Some(hdr), Some(f)) =
                    (entry.type_code, self.header, fields.as_mut())
                    && entry.handle != hdr.model
                    && entry.handle != hdr.paper
                    && let Some((_, numbered)) = self.names.get(&entry.handle)
                    && let Some(v2) = f.values.iter_mut().find(|v| v.code == 2)
                {
                    before_copy(1, numbered.len() as u64, cx)?;
                    v2.data = Data::Text(numbered.clone());
                }
                match fields {
                    Some(f) => {
                        let direct_owner = f.owner;
                        let (mut own, xdata) =
                            f.place_in(&self.names, &self.books, entry, cx, &at, before_copy)?;
                        if entry.type_code == BLOCK_RECORD {
                            let mut seen_70 = false;
                            own.retain(|val| {
                                let first_70 = val.code != 70 || !seen_70;
                                seen_70 |= val.code == 70;
                                !(first_70 && BLOCK_FIELDS.contains(&val.code))
                            });
                        }
                        values.extend(own);
                        if entry.type_code == BLOCK
                            && let Some(owner) = direct_owner
                            && let Some(moved) = self.block_fields.get(&owner)
                        {
                            check_values_copy(moved, cx, before_copy)?;
                            values.extend(moved.iter().cloned());
                        }
                        extra = xdata;
                    }
                    None => cx.report(
                        Diagnostic::new(codes::DWG_OBJECT_INVALID)
                            .with("type", i64::from(entry.type_code))
                            .at(at)
                            .cite(if entity { COMMON_ENTITY } else { COMMON_OBJECT }),
                    ),
                }
                before_copy(1, kname.len() as u64, cx)?;
                (kname.to_vec(), sec)
            }
            None => {
                cx.report(
                    Diagnostic::new(codes::DWG_TYPE_UNSUPPORTED)
                        .with("type", i64::from(entry.type_code))
                        .at(at),
                );
                (Vec::new(), Section::Other(Vec::new()))
            }
        };
        if let Some(&vp_num) = self.viewport_numbers.get(&record) {
            values.push(Value {
                code: 69,
                data: Data::Int(vp_num),
            });
        }
        if section == Section::Entities {
            // 所有者は、リアクターなどの 102 の組の外の初めの 330
            let owner = crate::db::owner_of_values(&values);
            if self.in_block_definition(file, owner) {
                section = Section::Blocks;
            }
        }
        values.extend(extra);
        Ok(Some(Object {
            kind: kind_vec,
            section,
            values,
            origin: Origin::Dwg {
                offset: entry.span.start,
                end: entry.span.end,
            },
        }))
    }

    /// 開くときに元レコードを共有予算で復号し、変換前の診断も呼び出し元へ報告する。
    #[cfg(any(feature = "drawing", test))]
    pub(crate) fn decode_record_open(
        &self,
        file: &DwgFile,
        record: usize,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Object>, Error> {
        let Some(entry) = file.entries.get(record) else {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            return Ok(None);
        };
        let at = Ref::at_bytes(entry.span.start as u64, entry.span.end as u64);
        cx.check(Kind::Work, 1)
            .map_err(|stop| stopped_at(cx, stop, at.clone()))?;
        check_decode_input(file.data(entry).len(), cx)?;
        let result = self
            .decode_record_in(file, record, cx, &mut check_copy)
            .map_err(|error| report_decode_error(error, cx))?;
        cx.check(Kind::Work, 0)
            .map_err(|stop| stopped_at(cx, stop, at))?;
        if let Some(object) = &result {
            check_decoded_values(&object.kind, &object.values, cx)?;
        }
        Ok(result)
    }

    /// 編集対象の元レコードを共有予算で復号する。変換診断は開いたときに報告済み。
    #[cfg(any(feature = "drawing", test))]
    pub(crate) fn decode_record_with_cx(
        &self,
        file: &DwgFile,
        record: usize,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Object>, Error> {
        cx.check(Kind::Work, 0).map_err(|stop| stopped(cx, stop))?;
        let Some(entry) = file.entries.get(record) else {
            return Ok(None);
        };
        check_decode_input(file.data(entry).len(), cx)?;
        let mut quiet = zumen_base::Diagnostics::new();
        let result = self
            .decode_record_in(
                file,
                record,
                &mut Cx::new(cx.budget, &mut quiet, cx.cancel),
                &mut check_copy,
            )
            .map_err(|error| report_decode_error(error, cx))?;
        cx.check(Kind::Work, 0).map_err(|stop| stopped(cx, stop))?;
        if let Some(object) = &result {
            check_decoded_values(&object.kind, &object.values, cx)?;
        }
        Ok(result)
    }

    /// 元のファイル `file` の `record` 番目のオブジェクトを `DwgTxn::set_data` と同じバイト列で取り出す
    pub fn base_encoded(&self, file: &DwgFile, record: usize) -> Option<Vec<u8>> {
        let entry = file.entries.get(record)?;
        let data = file.data(entry);
        if self.release >= Release::R2010 {
            let bits =
                i64::try_from(data.len().checked_mul(8)?.checked_sub(entry.handles?)?).ok()?;
            let mut out = Vec::with_capacity(data.len() + 6);
            push_modular_char(&mut out, bits, false);
            out.extend_from_slice(data);
            Some(out)
        } else {
            Some(data.to_vec())
        }
    }

    /// `DwgTxn::set_data` / `DwgTxn::add` と同じ形の符号化済みバイト列を 1 件だけ復号する
    pub fn decode_encoded(&self, handle: u64, encoded: &[u8]) -> Option<DecodedRecord> {
        let mut budget = zumen_base::Budget::default();
        let mut diag = zumen_base::Diagnostics::new();
        let cancel = zumen_base::CancelToken::new();
        match self.decode_encoded_in(
            handle,
            encoded,
            &mut Cx::new(&mut budget, &mut diag, &cancel),
            &mut unmetered_copy,
        ) {
            Ok(record) => record,
            Err(never) => match never {},
        }
    }

    /// 編集後のレコードを、呼び出し元の予算と中断を使って復号する。
    #[cfg(any(feature = "drawing", test))]
    pub(crate) fn decode_encoded_with_cx(
        &self,
        handle: u64,
        encoded: &[u8],
        cx: &mut Cx<'_>,
    ) -> Result<Option<DecodedRecord>, Error> {
        check_decode_input(encoded.len(), cx)?;
        let mut quiet = zumen_base::Diagnostics::new();
        let result = self
            .decode_encoded_in(
                handle,
                encoded,
                &mut Cx::new(cx.budget, &mut quiet, cx.cancel),
                &mut check_copy,
            )
            .map_err(|error| report_decode_error(error, cx))?;
        cx.check(Kind::Work, 0).map_err(|stop| stopped(cx, stop))?;
        if let Some((kind, _, values)) = &result {
            check_decoded_values(kind, values, cx)?;
        }
        Ok(result)
    }

    fn decode_encoded_in<E>(
        &self,
        handle: u64,
        encoded: &[u8],
        cx: &mut Cx<'_>,
        before_copy: &mut impl FnMut(u64, u64, &mut Cx) -> Result<(), E>,
    ) -> Result<Option<DecodedRecord>, E> {
        let parts = || {
            if self.release >= Release::R2010 {
                let mut skip = 0;
                let bits = usize::try_from(modular_char(encoded, &mut skip, false)?).ok()?;
                let data = encoded.get(skip..)?;
                let at = data.len().checked_mul(8)?.checked_sub(bits)?;
                let code = u16::try_from(Bits::new(data).bot()?).ok()?;
                Some((data, Some(at), code))
            } else {
                let code = u16::try_from(Bits::new(encoded).bs()?).ok()?;
                Some((encoded, None, code))
            }
        };
        let Some((raw_data, handles, type_code)) = parts() else {
            return Ok(None);
        };
        let Some((kname, sec)) = self.decode_kind_in(type_code, cx, before_copy)? else {
            return Ok(None);
        };
        let entity = matches!(sec, Section::Entities | Section::Blocks);
        let entry = Entry {
            handle,
            span: 0..0,
            data: 0..raw_data.len(),
            handles,
            type_code,
        };
        let Some(fields) = read_fields(raw_data, self.release, &entry, kname, entity, self.header)
        else {
            return Ok(None);
        };
        let at = Ref::default();
        let (own, xdata) =
            fields.place_in(&self.names, &self.books, &entry, cx, &at, before_copy)?;
        let handle_code = if type_code == DIMSTYLE { 105 } else { 5 };
        let mut values = vec![Value {
            code: handle_code,
            data: Data::Handle(handle),
        }];
        values.extend(own);
        values.extend(xdata);
        before_copy(1, kname.len() as u64, cx)?;
        Ok(Some((kname.to_vec(), sec, values)))
    }

    /// 1 図形の変更を符号化し直す
    pub fn encode_entity_edit(
        &self,
        current_encoded: &[u8],
        kind: &[u8],
        old_values: &[Value],
        new_values: &[Value],
    ) -> Option<Vec<u8>> {
        self.encode_record_edit(
            current_encoded,
            Section::Entities,
            kind,
            old_values,
            new_values,
        )
    }

    /// 1 レコード（図形またはオブジェクト）の変更を符号化し直す
    pub fn encode_record_edit(
        &self,
        current_encoded: &[u8],
        section: Section,
        kind: &[u8],
        old_values: &[Value],
        new_values: &[Value],
    ) -> Option<Vec<u8>> {
        encode_single_entity(
            self.release,
            current_encoded,
            section,
            kind,
            old_values,
            new_values,
            &|table, name| self.lookup_table_handle(table, name),
        )
    }

    /// モデル空間に足す 1 図形（と続く ATTRIB・SEQEND）を符号化する
    pub fn encode_born_model(
        &self,
        records: &[(&[u8], Vec<Value>)],
        seed: u64,
        is_removed: impl Fn(usize) -> bool,
    ) -> Option<EncodedBorn> {
        self.handle_seed?;
        let (_, model_owner) = self.model_block?;
        let objects: Vec<Object> = records
            .iter()
            .map(|(kind, values)| Object {
                kind: kind.to_vec(),
                section: Section::Entities,
                values: values.clone(),
                origin: Origin::New,
            })
            .collect();
        let (encoded, next_seed) = encode_born_entities(
            self.release,
            &objects,
            seed,
            model_owner,
            &|table, name| self.lookup_table_handle(table, name),
            self.material_bylayer,
            &|h| self.mtext_handles.contains(&h),
        )?;
        // 生きている既存のハンドルと衝突しないことを確かめる
        if encoded
            .iter()
            .any(|&(h, _)| self.record_of_handle(h).is_some_and(|r| !is_removed(r)))
        {
            return None;
        }
        Some((encoded, next_seed))
    }

    /// レコード `record` が持つ直接の子オブジェクトの総数
    fn total_children_of(&self, record: usize, handle: u64) -> u32 {
        if self.has_duplicate_handle {
            return self
                .owner_counts_by_handle
                .get(&handle)
                .copied()
                .unwrap_or(0);
        }
        match self.owner_counts.get(record).copied() {
            Some(u8::MAX) => self.owner_counts_large.get(&record).copied().unwrap_or(255),
            Some(c) => u32::from(c),
            None => 0,
        }
    }

    /// 既存のモデル空間図形 `record`（および一緒に除く所有オブジェクト `group_records`）を削除できるか
    pub fn can_unlink_model_entity(
        &self,
        file: &DwgFile,
        record: usize,
        current_encoded: &[u8],
        kind: &[u8],
        values: &[Value],
        group_records: &[(usize, u64, Option<u64>)],
    ) -> bool {
        let Some((_, model_owner)) = self.model_block else {
            return false;
        };
        if !bit_get(&self.model_owned_bits, record) {
            return false;
        }
        let Some(entry) = file.entries.get(record) else {
            return false;
        };
        let ent_owner = crate::db::owner_of_values(values);
        if ent_owner != Some(model_owner) {
            return false;
        }
        let mut group_handles = BTreeSet::new();
        for &(_, h, _) in group_records {
            group_handles.insert(h);
        }
        // `group_records` 内の各ハンドルについて、残る子オブジェクトが 1 件もないことを確かめる
        for &(rec, h, parent_opt) in group_records {
            if rec != record {
                let Some(p) = parent_opt else {
                    return false;
                };
                if !group_handles.contains(&p) {
                    return false;
                }
            }
            let total_children = self.total_children_of(rec, h);
            if total_children > 0 {
                let removed_children = group_records
                    .iter()
                    .filter(|&&(other_rec, _, other_parent)| {
                        other_rec != rec && other_parent == Some(h)
                    })
                    .count() as u32;
                if total_children > removed_children {
                    return false;
                }
            }
        }
        can_unlink_single(
            self.release,
            current_encoded,
            entry.handle,
            kind,
            values,
            |h| group_handles.contains(&h),
        )
    }

    /// 追加したモデル空間の図形が、保存後にも単独で削除できる種類と参照を持つか。
    #[cfg(feature = "drawing")]
    pub(crate) fn can_unlink_added_model_entity(
        &self,
        handle: u64,
        current_encoded: &[u8],
        kind: &[u8],
        values: &[Value],
    ) -> bool {
        let Some((_, owner)) = self.model_block else {
            return false;
        };
        let current_owner = crate::db::owner_of_values(values);
        current_owner == Some(owner)
            && can_unlink_single(self.release, current_encoded, handle, kind, values, |_| {
                false
            })
    }

    /// 差分の変更を `DwgFile` の複製に反映して書き出したバイト列を返す
    #[allow(clippy::too_many_arguments)]
    pub fn save_file(
        &self,
        file: &DwgFile,
        staged: &BTreeMap<usize, Vec<u8>>,
        removed_records: &BTreeSet<usize>,
        removed_model_handles: &BTreeSet<u64>,
        added: &[(u64, Vec<u8>, bool)],
        new_seed: Option<u64>,
        cx: &mut Cx<'_>,
    ) -> Result<Vec<u8>, Error> {
        self.save_file_with_headers(
            file,
            staged,
            removed_records,
            removed_model_handles,
            added,
            new_seed,
            None,
            None,
            cx,
        )
    }

    /// ヘッダー変数（`PDMODE`・`PDSIZE`）の変更も含め、差分を 1 つの `DwgTxn` で反映して書き出す
    #[allow(clippy::too_many_arguments)]
    pub fn save_file_with_headers(
        &self,
        file: &DwgFile,
        staged: &BTreeMap<usize, Vec<u8>>,
        removed_records: &BTreeSet<usize>,
        removed_model_handles: &BTreeSet<u64>,
        added: &[(u64, Vec<u8>, bool)],
        new_seed: Option<u64>,
        point_mode: Option<i16>,
        point_size: Option<f64>,
        cx: &mut Cx<'_>,
    ) -> Result<Vec<u8>, Error> {
        if staged.is_empty()
            && removed_records.is_empty()
            && added.is_empty()
            && new_seed.is_none()
            && point_mode.is_none()
            && point_size.is_none()
        {
            let mut out = Vec::new();
            Dwg::write(file, &mut out, cx)?;
            return Ok(out);
        }
        let mut changes = staged.clone();
        let mut added_copy: Vec<(u64, Vec<u8>, bool)> = added.to_vec();
        let reuses_removed_handle = added_copy.iter().any(|(h, _, _)| {
            self.record_of_handle(*h)
                .is_some_and(|r| removed_records.contains(&r))
        });
        if reuses_removed_handle {
            if !removed_model_handles.is_empty() {
                let (block_record, block_handle) = self.model_block.ok_or(Error::Rejected)?;
                let mut no_added = Vec::new();
                file.patch_model_block_and_added(
                    block_record,
                    block_handle,
                    |h| self.record_of_handle(h),
                    &mut changes,
                    removed_model_handles,
                    &mut no_added,
                )
                .ok_or(Error::Rejected)?;
            }
            let mut updated = file.snapshot(cx)?;
            {
                let mut txn = updated.transaction();
                for (&rec, data) in &changes {
                    if !removed_records.contains(&rec) {
                        txn.set_data(rec, data)?;
                    }
                }
                for &rec in removed_records {
                    txn.remove(rec)?;
                }
                txn.commit(cx)?;
            }
            let mut add_changes = BTreeMap::new();
            if !added_copy.is_empty() {
                let (block_record, block_handle) = self.model_block.ok_or(Error::Rejected)?;
                let shifted_block = block_record - removed_records.range(..block_record).count();
                let handle_map: BTreeMap<u64, usize> = updated
                    .entries
                    .iter()
                    .enumerate()
                    .map(|(i, e)| (e.handle, i))
                    .collect();
                updated
                    .patch_model_block_and_added(
                        shifted_block,
                        block_handle,
                        |h| handle_map.get(&h).copied(),
                        &mut add_changes,
                        &BTreeSet::new(),
                        &mut added_copy,
                    )
                    .ok_or(Error::Rejected)?;
            }
            {
                let mut txn = updated.transaction();
                for (&rec, data) in &add_changes {
                    txn.set_data(rec, data)?;
                }
                for (handle, data, _) in &added_copy {
                    txn.add(*handle, data)?;
                }
                if let Some(seed) = new_seed {
                    txn.set_handle_seed(seed)?;
                }
                if let Some(mode) = point_mode {
                    txn.set_point_mode(mode)?;
                }
                if let Some(size) = point_size {
                    txn.set_point_size(size)?;
                }
                txn.commit(cx)?;
            }
            let mut out = Vec::new();
            Dwg::write(&updated, &mut out, cx)?;
            return Ok(out);
        }
        if !removed_model_handles.is_empty() || !added_copy.is_empty() {
            let (block_record, block_handle) = self.model_block.ok_or(Error::Rejected)?;
            file.patch_model_block_and_added(
                block_record,
                block_handle,
                |h| self.record_of_handle(h),
                &mut changes,
                removed_model_handles,
                &mut added_copy,
            )
            .ok_or(Error::Rejected)?;
        }
        let mut updated = file.snapshot(cx)?;
        let mut txn = updated.transaction();
        for (&rec, data) in &changes {
            if !removed_records.contains(&rec) {
                txn.set_data(rec, data)?;
            }
        }
        for &rec in removed_records {
            txn.remove(rec)?;
        }
        for (handle, data, _) in &added_copy {
            txn.add(*handle, data)?;
        }
        if let Some(seed) = new_seed {
            txn.set_handle_seed(seed)?;
        }
        if let Some(mode) = point_mode {
            txn.set_point_mode(mode)?;
        }
        if let Some(size) = point_size {
            txn.set_point_size(size)?;
        }
        txn.commit(cx)?;
        let mut out = Vec::new();
        Dwg::write(&updated, &mut out, cx)?;
        Ok(out)
    }

    /// 1 回の `apply` に相当する差分を `file` に確定させる（全体を書く保存で変更列を順に再現するのに使う）
    #[allow(clippy::too_many_arguments)]
    pub fn apply_fast_step(
        &self,
        file: &mut DwgFile,
        updated_records: &[UpdatedDwgRecord],
        removed_handles: &BTreeSet<u64>,
        removed_model_handles: &BTreeSet<u64>,
        added: &[(u64, Vec<u8>, bool)],
        new_seed: Option<u64>,
        point_mode: Option<i16>,
        point_size: Option<f64>,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        self.apply_step_to(
            file,
            DwgFastStep {
                updated: updated_records,
                removed: removed_handles,
                removed_model: removed_model_handles,
                added,
                seed: new_seed,
                point_mode,
                point_size,
            },
            cx,
        )
    }

    /// 全体のバイト列を作らず、保存時のオブジェクトの位置へ変更を当てる。
    #[cfg(any(feature = "drawing", test))]
    pub(crate) fn apply_layout_step(
        &self,
        layout: &mut DwgSourceLayout<'_>,
        step: DwgFastStep<'_>,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        self.apply_step_to(layout, step, cx)
    }

    fn apply_step_to(
        &self,
        file: &mut impl FastStepTarget,
        step: DwgFastStep<'_>,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        file.prepare_step(&step, cx)?;
        let DwgFastStep {
            updated,
            removed,
            removed_model,
            added,
            seed,
            point_mode,
            point_size,
        } = step;
        if updated.is_empty()
            && removed.is_empty()
            && added.is_empty()
            && seed.is_none()
            && point_mode.is_none()
            && point_size.is_none()
        {
            return Ok(());
        }
        let handle_map = file.handle_map();
        let mut changes = BTreeMap::new();
        for (handle, section, kind, old_values, new_values) in updated {
            if old_values == new_values {
                continue;
            }
            let rec = *handle_map.get(handle).ok_or(Error::Rejected)?;
            let encoded = file.encoded(self, rec, cx)?;
            let bytes = self
                .encode_record_edit(&encoded, section.clone(), kind, old_values, new_values)
                .ok_or(Error::Rejected)?;
            changes.insert(rec, bytes);
        }
        let removed_records = removed
            .iter()
            .map(|handle| handle_map.get(handle).copied().ok_or(Error::Rejected))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let mut added = added.to_vec();
        let reused = added.iter().any(|(handle, _, _)| removed.contains(handle));
        if reused {
            if !removed_model.is_empty() {
                let (_, block) = self.model_block.ok_or(Error::Rejected)?;
                let record = *handle_map.get(&block).ok_or(Error::Rejected)?;
                file.patch_block(
                    record,
                    block,
                    &handle_map,
                    &mut changes,
                    removed_model,
                    &mut [],
                    cx,
                )?;
            }
            file.commit_step(
                DwgMutation {
                    changes,
                    removed: removed_records,
                    added: Vec::new(),
                    seed: None,
                    values: header::Values::default(),
                },
                cx,
            )?;
            let post_map = file.handle_map();
            let mut changes = BTreeMap::new();
            if !added.is_empty() {
                let (_, block) = self.model_block.ok_or(Error::Rejected)?;
                let record = *post_map.get(&block).ok_or(Error::Rejected)?;
                file.patch_block(
                    record,
                    block,
                    &post_map,
                    &mut changes,
                    &BTreeSet::new(),
                    &mut added,
                    cx,
                )?;
            }
            return file.commit_step(
                DwgMutation {
                    changes,
                    removed: BTreeSet::new(),
                    added,
                    seed,
                    values: header::Values {
                        point_mode,
                        point_size,
                        ..header::Values::default()
                    },
                },
                cx,
            );
        }
        if !removed_model.is_empty() || !added.is_empty() {
            let (_, block) = self.model_block.ok_or(Error::Rejected)?;
            let record = *handle_map.get(&block).ok_or(Error::Rejected)?;
            file.patch_block(
                record,
                block,
                &handle_map,
                &mut changes,
                removed_model,
                &mut added,
                cx,
            )?;
        }
        file.commit_step(
            DwgMutation {
                changes,
                removed: removed_records,
                added,
                seed,
                values: header::Values {
                    point_mode,
                    point_size,
                    ..header::Values::default()
                },
            },
            cx,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Stop};
    use zumen_sdk::{Codec, Error, FileModel};

    use super::{DwgFastStep, DwgSingle, DwgSourceLayout, bit_get};
    use crate::Dwg;

    fn shared_decode<T: core::fmt::Debug + PartialEq>(
        expected: Option<T>,
        mut decode: impl FnMut(&mut Cx<'_>) -> Result<Option<T>, Error>,
    ) {
        let cancel = CancelToken::new();
        let mut measured = Budget::new(Limits::default().with(Kind::InputBytes, Limit::Finite(0)));
        let mut diagnostics = Diagnostics::new();
        assert_eq!(
            decode(&mut Cx::new(&mut measured, &mut diagnostics, &cancel)).unwrap(),
            expected
        );
        assert!(diagnostics.is_empty());
        for kind in [Kind::Work, Kind::DecodedBytes] {
            let needed = measured.used(kind);
            assert!(needed > 0);
            for available in [0, needed / 2, needed - 1, needed] {
                let mut budget =
                    Budget::new(Limits::default().with(kind, Limit::Finite(available + 11)));
                budget.spend(kind, 11).unwrap();
                let mut diagnostics = Diagnostics::new();
                let result = decode(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
                if available < needed {
                    assert!(matches!(result, Err(Error::Stopped(_))));
                    assert!(diagnostics.iter().any(|d| d.code() == kind.exceeded_code()));
                } else {
                    assert_eq!(result.unwrap(), expected);
                    assert_eq!(budget.used(kind), needed + 11);
                }
            }
        }
        cancel.cancel();
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        assert_eq!(
            decode(&mut Cx::new(&mut budget, &mut diagnostics, &cancel)),
            Err(Error::Stopped(Stop::Cancelled))
        );
        assert_eq!(budget.used(Kind::Work), 0);
        assert_eq!(budget.used(Kind::DecodedBytes), 0);
    }

    #[test]
    fn source_records_share_budget_and_cancellation() {
        for bytes in [
            include_bytes!("../../../../samples/dwg/zumen-layout-r2000.dwg").as_slice(),
            include_bytes!("../../../../samples/dwg/zumen-layout-r2004.dwg").as_slice(),
            include_bytes!("../../../../samples/dwg/zumen-layout-r2018.dwg").as_slice(),
        ] {
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
            let file = Dwg::read(bytes, &mut cx).unwrap();
            let single = DwgSingle::from_file(&file, &mut cx).unwrap();
            for record in 0..file.entries.len() {
                let expected = single.decode_record(&file, record, &mut cx);
                shared_decode(expected, |cx| {
                    single.decode_record_with_cx(&file, record, cx)
                });
            }
            assert_eq!(
                single.decode_record_with_cx(&file, usize::MAX, &mut cx),
                Ok(None)
            );
        }
    }

    #[test]
    fn encoded_records_share_budget_and_cancellation() {
        for bytes in [
            include_bytes!("../../../../samples/dwg/zumen-layout-r2000.dwg").as_slice(),
            include_bytes!("../../../../samples/dwg/zumen-layout-r2004.dwg").as_slice(),
            include_bytes!("../../../../samples/dwg/zumen-layout-r2018.dwg").as_slice(),
        ] {
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
            let file = Dwg::read(bytes, &mut cx).unwrap();
            let single = DwgSingle::from_file(&file, &mut cx).unwrap();
            for (record, entry) in file.entries.iter().enumerate() {
                let encoded = single.base_encoded(&file, record).unwrap();
                let expected = single.decode_encoded(entry.handle, &encoded);
                let mut measured = Budget::default();
                let mut diagnostics = Diagnostics::new();
                assert_eq!(
                    single
                        .decode_encoded_with_cx(
                            entry.handle,
                            &encoded,
                            &mut Cx::new(&mut measured, &mut diagnostics, &cancel),
                        )
                        .unwrap(),
                    expected
                );
                assert!(diagnostics.is_empty());
                for kind in [Kind::Work, Kind::DecodedBytes] {
                    let needed = measured.used(kind);
                    assert!(needed > 0);
                    for available in [0, needed / 2, needed - 1, needed] {
                        let mut budget = Budget::new(
                            Limits::default().with(kind, Limit::Finite(available + 11)),
                        );
                        budget.spend(kind, 11).unwrap();
                        let mut diagnostics = Diagnostics::new();
                        let result = single.decode_encoded_with_cx(
                            entry.handle,
                            &encoded,
                            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
                        );
                        if available < needed {
                            assert!(matches!(result, Err(Error::Stopped(_))));
                            assert!(diagnostics.iter().any(|d| d.code() == kind.exceeded_code()));
                        } else {
                            assert_eq!(result.unwrap(), expected);
                            assert_eq!(budget.used(kind), needed + 11);
                        }
                    }
                }
                let cancelled = CancelToken::new();
                cancelled.cancel();
                let mut budget = Budget::default();
                let mut diagnostics = Diagnostics::new();
                assert_eq!(
                    single.decode_encoded_with_cx(
                        entry.handle,
                        &encoded,
                        &mut Cx::new(&mut budget, &mut diagnostics, &cancelled),
                    ),
                    Err(Error::Stopped(Stop::Cancelled))
                );
                assert_eq!(budget.used(Kind::Work), 0);
                assert_eq!(budget.used(Kind::DecodedBytes), 0);
            }
            for bytes in [b"".as_slice(), &[0], &[255; 32]] {
                let mut budget = Budget::default();
                let mut diagnostics = Diagnostics::new();
                assert_eq!(
                    single
                        .decode_encoded_with_cx(
                            1,
                            bytes,
                            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
                        )
                        .unwrap(),
                    single.decode_encoded(1, bytes)
                );
            }
        }
    }

    #[test]
    fn table_payloads_use_shared_limits_in_source_and_encoded_records() {
        for bytes in [
            include_bytes!("../../../../samples/dwg/zumen-layout-r2000.dwg").as_slice(),
            include_bytes!("../../../../samples/dwg/zumen-layout-r2004.dwg").as_slice(),
            include_bytes!("../../../../samples/dwg/zumen-layout-r2018.dwg").as_slice(),
        ] {
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
            let file = Dwg::read(bytes, &mut cx).unwrap();
            let mut single = DwgSingle::from_file(&file, &mut cx).unwrap();
            let (record, entry) = file
                .entries
                .iter()
                .enumerate()
                .find(|(record, _)| {
                    single
                        .decode_record(&file, *record, &mut cx)
                        .is_some_and(|object| {
                            object.section == crate::db::Section::Entities
                                && object.values.iter().any(|value| value.code == 8)
                        })
                })
                .unwrap();
            let encoded = single.base_encoded(&file, record).unwrap();
            for (kind, name) in single.names.values_mut() {
                if *kind == super::LAYER {
                    *name = vec![b'L'; 131_072];
                }
            }
            let expected = single.decode_record(&file, record, &mut cx).unwrap();
            assert!(expected.values.iter().any(|value| value.code == 8
                && matches!(&value.data, crate::db::Data::Text(name) if name.len() == 131_072)));
            shared_decode(Some(expected), |cx| {
                single.decode_record_with_cx(&file, record, cx)
            });
            shared_decode(single.decode_encoded(entry.handle, &encoded), |cx| {
                single.decode_encoded_with_cx(entry.handle, &encoded, cx)
            });

            // クラス名とは別に、出所のセクション名もコピー前に検査する。
            single.classes.insert(
                65_000,
                (
                    b"CUSTOM".to_vec(),
                    crate::db::Section::Other(vec![b'S'; 131_072]),
                ),
            );
            let mut budget =
                Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(131_071)));
            let mut diagnostics = Diagnostics::new();
            assert!(matches!(
                single.decode_kind_in(
                    65_000,
                    &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
                    &mut super::check_copy
                ),
                Err(Error::Stopped(_))
            ));
            assert_eq!(budget.used(Kind::DecodedBytes), 0);
        }
    }

    #[test]
    fn source_layout_matches_saved_offsets_with_handle_reuse_and_header_growth() {
        for name in [
            "zumen-layout-r2000.dwg",
            "zumen-layout-r2004.dwg",
            "zumen-layout-r2018.dwg",
        ] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../samples/dwg")
                .join(name);
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
            let base = Dwg::read(&std::fs::read(path).unwrap(), &mut cx).unwrap();
            let single = DwgSingle::from_file(&base, &mut cx).unwrap();
            let mut layout = DwgSourceLayout::new(&base, &mut cx).unwrap();
            let mut reference = base.snapshot(&mut cx).unwrap();
            let record = (0..base.entries.len())
                .find(|&i| bit_get(&single.model_owned_bits, i))
                .unwrap();
            let handle = base.entries[record].handle;
            let removed = BTreeSet::from([handle]);
            for (seed, mode, size) in [(1 << 40, 35, 2.5), (1 << 48, 0, 0.0)] {
                let current = reference
                    .entries
                    .iter()
                    .position(|entry| entry.handle == handle)
                    .unwrap();
                let object = single.decode_record(&reference, current, &mut cx).unwrap();
                let unchanged = vec![(
                    handle,
                    object.section,
                    object.kind,
                    object.values.clone(),
                    object.values,
                )];
                let empty = BTreeSet::new();
                single
                    .apply_layout_step(
                        &mut layout,
                        DwgFastStep {
                            updated: &unchanged,
                            removed: &empty,
                            removed_model: &empty,
                            added: &[],
                            seed: None,
                            point_mode: None,
                            point_size: None,
                        },
                        &mut cx,
                    )
                    .unwrap();
                single
                    .apply_fast_step(
                        &mut reference,
                        &unchanged,
                        &empty,
                        &empty,
                        &[],
                        None,
                        None,
                        None,
                        &mut cx,
                    )
                    .unwrap();
                assert_eq!(
                    layout.cursor,
                    reference.body().len(),
                    "no-op changed the append position"
                );
                let added = vec![(
                    handle,
                    single.base_encoded(&reference, current).unwrap(),
                    true,
                )];
                let step = DwgFastStep {
                    updated: &[],
                    removed: &removed,
                    removed_model: &removed,
                    added: &added,
                    seed: Some(seed),
                    point_mode: Some(mode),
                    point_size: Some(size),
                };
                single
                    .apply_layout_step(&mut layout, step, &mut cx)
                    .unwrap();
                single
                    .apply_fast_step(
                        &mut reference,
                        &[],
                        &removed,
                        &removed,
                        &added,
                        Some(seed),
                        Some(mode),
                        Some(size),
                        &mut cx,
                    )
                    .unwrap();
                let mut saved = Vec::new();
                Dwg::write(&reference, &mut saved, &mut cx).unwrap();
                let reloaded = Dwg::read(&saved, &mut cx).unwrap();
                let expected = reloaded
                    .entries
                    .iter()
                    .map(|entry| (entry.handle, entry.span.clone()))
                    .collect();
                assert_eq!(
                    layout.source_spans(&mut cx).unwrap(),
                    expected,
                    "{name}; seed={seed}"
                );
            }
        }
    }

    #[test]
    fn decode_record_matches_full_objects_across_releases() {
        let samples = [
            "../../samples/dwg/zumen-layout-r2000.dwg",
            "../../samples/dwg/zumen-minsert-r2000.dwg",
            "../../samples/dwg/zumen-points-a-r2000.dwg",
            "../../samples/dwg/zumen-layout-r2004.dwg",
            "../../samples/dwg/zumen-xclip-r2013.dwg",
            "../../samples/dwg/zumen-arcdim-r2018.dwg",
            "../../samples/dwg/zumen-layout-r2018.dwg",
            "../../samples/dwg/zumen-minsert-r2018.dwg",
            "../../samples/dwg/zumen-xclip-r2018.dwg",
        ];
        for rel in samples {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let mut budget = Budget::new(Limits::default());
            let mut diag = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
            let Ok(file) = Dwg::read(&bytes, &mut cx) else {
                continue;
            };

            let mut d1 = Diagnostics::new();
            let Ok(full) = file.objects(&mut Cx::new(&mut budget, &mut d1, &cancel)) else {
                continue;
            };
            let mut d2 = Diagnostics::new();
            let Ok(single) =
                DwgSingle::from_file(&file, &mut Cx::new(&mut budget, &mut d2, &cancel))
            else {
                continue;
            };
            assert_eq!(full.objects().len(), file.record_count());
            for rec in 0..file.record_count() {
                let decoded =
                    single.decode_record(&file, rec, &mut Cx::new(&mut budget, &mut d2, &cancel));
                assert_eq!(
                    decoded.as_ref(),
                    full.objects().get(rec),
                    "record {rec} mismatch in {rel}"
                );
            }
            assert_eq!(d1.len(), d2.len(), "diagnostics mismatch in {rel}");
        }
    }
}
