//! SFC（フィーチャコメント形式）の差分状態・単一フィーチャ往復検証・書き出し。

use core::ops::Range;
use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Cx, Key, Ref};
use zumen_live::{PatchPayload, SourceBytes};
use zumen_sdk::{Error, quietly};

use crate::drawing::apply::text::{self, is_string, sfc_string};
use crate::drawing::apply::{Arguments, line_end, newline, patched, patched_source};
use crate::drawing::{FeatureRecord, sfc_records, work};
use crate::{DIALECT, Marker, Mode, Quote, SfcRecord, SxfFile, Value};

use super::source::SourceMap;

/// SFC フィーチャの場所（開いた時点の基底フィーチャか、追加したフィーチャ）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum SfcSlot {
    Base(usize),
    Born(u32),
}

#[derive(Clone, Debug)]
struct BaseFeature {
    record_idx: usize,
    span: Range<usize>,
    line_end: usize,
    marker: Marker,
    instance: Option<u64>,
    name_lower: Box<str>,
    kind: Box<str>,
    grouped: bool,
    /// `span.start` からの相対バイト範囲（`Quote::Plain` または `Value::Token` の引数のみ `Some`）
    arg_ranges: Box<[Option<Range<u32>>]>,
    /// 囲みの引用符も含む、`span.start` からの相対バイト範囲（文字列か `Value::Token` の引数のみ `Some`）
    outer_ranges: Box<[Option<Range<u32>>]>,
}

/// 基底フィーチャの引数に書く値
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ArgValue {
    /// 囲みの内側に書く値
    Inner(String),
    /// 囲みの引用符も含めて書き直すバイト列（文字列）
    Outer(Vec<u8>),
}

impl ArgValue {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::Inner(text) => text.as_bytes(),
            Self::Outer(bytes) => bytes,
        }
    }
}

impl BaseFeature {
    /// 値を書く、`span.start` からの相対バイト範囲
    fn range(&self, at: usize, value: &ArgValue) -> Option<Range<usize>> {
        let ranges = match value {
            ArgValue::Inner(_) => &self.arg_ranges,
            ArgValue::Outer(_) => &self.outer_ranges,
        };
        let rel = ranges.get(at)?.as_ref()?;
        Some(rel.start as usize..rel.end as usize)
    }
}

#[derive(Clone, Debug)]
pub(super) struct BornFeature {
    pub(super) instance: u64,
    pub(super) marker: Marker,
    pub(super) name: &'static str,
    pub(super) args: Vec<String>,
}

/// 1 回のトランザクションで SFC 差分に当てた変更（`undo` / `redo` 用）
#[derive(Clone, Debug, Default)]
pub(super) struct SfcPatch {
    removed_base: Vec<usize>,
    removed_born: Vec<(u32, BornFeature)>,
    base_args: Vec<(usize, usize, Option<ArgValue>, ArgValue)>,
    born_args: Vec<(u32, usize, String, String)>,
    added_born: Vec<(u32, BornFeature)>,
}

impl PatchPayload for SfcPatch {
    const INLINE_BYTES: usize = {
        assert!(core::mem::size_of::<Self>() <= 128);
        128
    };

    fn heap_bytes(&self) -> usize {
        self.removed_base.capacity() * 8
            + self.removed_born.capacity() * 80
            + self.base_args.capacity() * 48
            + self.born_args.capacity() * 48
            + self.added_born.capacity() * 64
    }
}

/// 1 回の `apply` 中に仮置きする SFC 変更
pub(super) struct SfcStepTx {
    removed_base: BTreeSet<usize>,
    removed_born: BTreeSet<u32>,
    base_args: BTreeMap<(usize, usize), ArgValue>,
    born_args: BTreeMap<(u32, usize), String>,
    added_born: Vec<(u32, BornFeature)>,
    next_id: u64,
    next_born_seq: u32,
}

#[derive(Clone)]
pub(super) struct SfcState {
    src: SourceBytes,
    newline: &'static str,
    base_features: Vec<BaseFeature>,
    record_to_base: BTreeMap<usize, usize>,
    first_sheet_base: Option<usize>,
    fixed_max_id: u64,
    alive_id_counts: BTreeMap<u64, u32>,
    removed_base: BTreeSet<usize>,
    patched_base: BTreeMap<usize, BTreeMap<usize, ArgValue>>,
    born_features: BTreeMap<u32, BornFeature>,
    next_born_seq: u32,
}

impl SfcState {
    pub(super) fn new(
        src: SourceBytes,
        file: &SxfFile,
        records: &[FeatureRecord],
        grouped: &BTreeSet<usize>,
    ) -> Self {
        let raw = file.as_bytes();
        let nl = newline(raw);
        let mut base_features = Vec::with_capacity(records.len());
        let mut record_to_base = BTreeMap::new();
        let mut first_sheet_base = None;
        let mut alive_id_counts: BTreeMap<u64, u32> = BTreeMap::new();

        for rec in records {
            let Some(entry) = file.records().get(rec.record) else {
                continue;
            };
            let Some(marker) = entry.marker() else {
                continue;
            };
            let span = entry.span();
            let le = line_end(raw, span.end);
            let instance = entry.id();
            if let Some(id) = instance {
                *alive_id_counts.entry(id).or_default() += 1;
            }
            let arg_ranges: Vec<Option<Range<u32>>> = entry
                .parts()
                .first()
                .map(|part| {
                    part.args()
                        .iter()
                        .map(|val| match val {
                            Value::Text {
                                raw,
                                quote: Quote::Plain,
                            }
                            | Value::Token(raw) => {
                                let s = u32::try_from(raw.start.checked_sub(span.start)?).ok()?;
                                let e = u32::try_from(raw.end.checked_sub(span.start)?).ok()?;
                                Some(s..e)
                            }
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default();
            let outer_ranges: Vec<Option<Range<u32>>> = entry
                .parts()
                .first()
                .map(|part| {
                    part.args()
                        .iter()
                        .map(|val| {
                            let raw = match val {
                                Value::Text { raw, quote } => {
                                    let width = if *quote == Quote::Backslash { 2 } else { 1 };
                                    raw.start.checked_sub(width)?..raw.end.checked_add(width)?
                                }
                                Value::Token(raw) => raw.clone(),
                                _ => return None,
                            };
                            let s = u32::try_from(raw.start.checked_sub(span.start)?).ok()?;
                            let e = u32::try_from(raw.end.checked_sub(span.start)?).ok()?;
                            Some(s..e)
                        })
                        .collect()
                })
                .unwrap_or_default();
            let name_lower = rec.name.to_ascii_lowercase().into_boxed_str();
            let base_idx = base_features.len();
            if first_sheet_base.is_none() && name_lower.as_ref() == "drawing_sheet" {
                first_sheet_base = Some(base_idx);
            }
            record_to_base.insert(rec.record, base_idx);
            base_features.push(BaseFeature {
                record_idx: rec.record,
                span,
                line_end: le,
                marker,
                instance,
                name_lower,
                kind: rec.kind.clone().into_boxed_str(),
                grouped: grouped.contains(&rec.record),
                arg_ranges: arg_ranges.into_boxed_slice(),
                outer_ranges: outer_ranges.into_boxed_slice(),
            });
        }

        let mut fixed_max_id = 0_u64;
        for (idx, entry) in file.records().iter().enumerate() {
            if !record_to_base.contains_key(&idx)
                && let Some(id) = entry.id()
                && id > fixed_max_id
            {
                fixed_max_id = id;
            }
        }

        Self {
            src,
            newline: nl,
            base_features,
            record_to_base,
            first_sheet_base,
            fixed_max_id,
            alive_id_counts,
            removed_base: BTreeSet::new(),
            patched_base: BTreeMap::new(),
            born_features: BTreeMap::new(),
            next_born_seq: 0,
        }
    }

    pub(super) fn resident_bytes(&self) -> usize {
        self.src.stored_bytes()
            + self.base_features.capacity() * core::mem::size_of::<BaseFeature>()
            + self.patched_base.len() * 64
            + self.born_features.len() * 96
    }

    pub(super) fn is_clean(&self) -> bool {
        self.removed_base.is_empty()
            && self.patched_base.is_empty()
            && self.born_features.is_empty()
    }

    pub(super) fn slot_for_record(&self, record_idx: usize) -> Option<SfcSlot> {
        self.record_to_base
            .get(&record_idx)
            .copied()
            .map(SfcSlot::Base)
    }

    pub(super) fn slot_meta(&self, slot: SfcSlot) -> Option<(&str, &str, bool, usize)> {
        match slot {
            SfcSlot::Base(idx) => {
                let bf = self.base_features.get(idx)?;
                Some((&bf.name_lower, &bf.kind, bf.grouped, bf.record_idx))
            }
            SfcSlot::Born(seq) => {
                let born = self.born_features.get(&seq)?;
                let kind = match born.name {
                    "line" => "line_feature",
                    "polyline" => "polyline_feature",
                    "circle" => "circle_feature",
                    "arc" => "arc_feature",
                    "point_marker" => "point_marker_feature",
                    text::NAME => "text_string_feature",
                    _ => "element",
                };
                Some((born.name, kind, false, 0))
            }
        }
    }

    pub(super) fn begin_step(&self) -> SfcStepTx {
        let max_feat_id = self
            .alive_id_counts
            .keys()
            .next_back()
            .copied()
            .unwrap_or(0);
        let next_id = max_feat_id.max(self.fixed_max_id).saturating_add(1);
        SfcStepTx {
            removed_base: BTreeSet::new(),
            removed_born: BTreeSet::new(),
            base_args: BTreeMap::new(),
            born_args: BTreeMap::new(),
            added_born: Vec::new(),
            next_id,
            next_born_seq: self.next_born_seq,
        }
    }

    fn current_insert_marker(&self) -> Option<Marker> {
        if let Some(sheet_idx) = self.first_sheet_base {
            return Some(self.base_features.get(sheet_idx)?.marker);
        }
        if let Some((_, last_born)) = self.born_features.iter().next_back() {
            return Some(last_born.marker);
        }
        self.base_features
            .iter()
            .enumerate()
            .rev()
            .find(|(idx, _)| !self.removed_base.contains(idx))
            .map(|(_, bf)| bf.marker)
    }

    pub(super) fn stage_codes(
        &self,
        tx: &mut SfcStepTx,
        slot: SfcSlot,
        codes: &[(usize, i64)],
    ) -> Option<()> {
        for &(at, code) in codes {
            self.stage_value(tx, slot, at, code.to_string())?;
        }
        Some(())
    }

    /// 囲みの内側の値を仮置きする。囲まない値か、`'…'` で囲んだ値だけ
    fn stage_value(
        &self,
        tx: &mut SfcStepTx,
        slot: SfcSlot,
        at: usize,
        value: String,
    ) -> Option<()> {
        match slot {
            SfcSlot::Base(idx) => {
                let bf = self.base_features.get(idx)?;
                bf.arg_ranges.get(at)?.as_ref()?;
                tx.base_args.insert((idx, at), ArgValue::Inner(value));
            }
            SfcSlot::Born(seq) => {
                let born = self.born_features.get(&seq)?;
                if at >= born.args.len() {
                    return None;
                }
                tx.born_args.insert((seq, at), value);
            }
        }
        Some(())
    }

    /// 文字列の引数を、囲みも含めて仮置きする。`suffix` は値の後に足すバイト列
    fn stage_string(
        &self,
        tx: &mut SfcStepTx,
        slot: SfcSlot,
        at: usize,
        value: &str,
        suffix: &[u8],
    ) -> Option<()> {
        match slot {
            SfcSlot::Base(idx) => {
                let bf = self.base_features.get(idx)?;
                bf.outer_ranges.get(at)?.as_ref()?;
                let mut bytes = sfc_string(value)?;
                bytes.extend_from_slice(suffix);
                tx.base_args.insert((idx, at), ArgValue::Outer(bytes));
            }
            SfcSlot::Born(seq) => {
                let born = self.born_features.get(&seq)?;
                if at >= born.args.len() || !suffix.is_empty() {
                    return None;
                }
                sfc_string(value)?;
                tx.born_args.insert((seq, at), value.to_owned());
            }
        }
        Some(())
    }

    fn arg_count(&self, slot: SfcSlot) -> Option<usize> {
        match slot {
            SfcSlot::Base(idx) => Some(self.base_features.get(idx)?.arg_ranges.len()),
            SfcSlot::Born(seq) => Some(self.born_features.get(&seq)?.args.len()),
        }
    }

    /// 形を仮置きする。種類は変えない（バッチの SFC の書き方と同じ引数を書く）
    pub(super) fn stage_shape(
        &self,
        tx: &mut SfcStepTx,
        slot: SfcSlot,
        args: &Arguments,
    ) -> Option<()> {
        let annotation = args.name == text::NAME;
        let first = if annotation { 0 } else { args.first() };
        let count = self.arg_count(slot)?;
        for (at, value) in args.values.iter().enumerate().skip(first) {
            if is_string(args, at) {
                self.stage_string(tx, slot, at, value, b"")?;
            } else if annotation && at == 12 && value == "1" && count == 12 {
                // 旧版の横書きは省略できる。
            } else {
                self.stage_value(tx, slot, at, value.clone())?;
            }
        }
        Some(())
    }

    /// レイヤフィーチャの名前と表示の可否を仮置きし、そのインスタンス番号を返す
    pub(super) fn stage_layer(
        &self,
        tx: &mut SfcStepTx,
        slot: SfcSlot,
        name: &str,
        visible: bool,
    ) -> Option<u64> {
        let flag = if visible { "1" } else { "0" };
        match slot {
            SfcSlot::Base(idx) => {
                let bf = self.base_features.get(idx)?;
                let id = bf.instance?;
                if bf.arg_ranges.len() > 1 {
                    self.stage_string(tx, slot, 0, name, b"")?;
                    self.stage_value(tx, slot, 1, flag.into())?;
                } else {
                    // 表示の可否を省いた定義は、隠すときだけ足す
                    let suffix: &[u8] = if visible { b"" } else { b",'0'" };
                    self.stage_string(tx, slot, 0, name, suffix)?;
                }
                Some(id)
            }
            SfcSlot::Born(seq) => {
                let id = self.born_features.get(&seq)?.instance;
                self.stage_string(tx, slot, 0, name, b"")?;
                self.stage_value(tx, slot, 1, flag.into())?;
                Some(id)
            }
        }
    }

    /// 出所の定義の場所。開いた時点のフィーチャはレコードの番号で、足したフィーチャ（仮置き中も含む）はインスタンス
    /// 番号で引く
    pub(super) fn slot_for_source(&self, tx: &SfcStepTx, source: &Ref) -> Option<SfcSlot> {
        if let Some(record) = source.record() {
            return self.slot_for_record(usize::try_from(record).ok()?);
        }
        let Some(Key::Instance(instance)) = source.key() else {
            return None;
        };
        tx.added_born
            .iter()
            .find(|(_, born)| born.instance == instance)
            .map(|(seq, _)| SfcSlot::Born(*seq))
            .or_else(|| {
                self.born_features
                    .iter()
                    .find(|(seq, born)| born.instance == instance && !tx.removed_born.contains(seq))
                    .map(|(seq, _)| SfcSlot::Born(*seq))
            })
    }

    pub(super) fn stage_remove(&self, tx: &mut SfcStepTx, slot: SfcSlot) -> Option<()> {
        match slot {
            SfcSlot::Base(idx) => {
                self.base_features.get(idx)?;
                tx.removed_base.insert(idx);
                Some(())
            }
            SfcSlot::Born(seq) => {
                self.born_features.get(&seq)?;
                tx.removed_born.insert(seq);
                Some(())
            }
        }
    }

    pub(super) fn stage_add(&self, tx: &mut SfcStepTx, args: &Arguments) -> Option<(u32, u64)> {
        let marker = self.current_insert_marker()?;
        let instance = tx.next_id;
        tx.next_id = tx.next_id.checked_add(1)?;
        let seq = tx.next_born_seq;
        tx.next_born_seq = tx.next_born_seq.checked_add(1)?;
        tx.added_born.push((
            seq,
            BornFeature {
                instance,
                marker,
                name: args.name,
                args: args.values.clone(),
            },
        ));
        Some((seq, instance))
    }

    /// 仮置き中の 1 つのフィーチャをバイト列に組み、`zumen_step` で読んで [`FeatureRecord`] を返す
    pub(super) fn parse_staged_feature(
        &self,
        tx: &SfcStepTx,
        slot: SfcSlot,
        cx: &mut Cx,
    ) -> Result<Option<(u64, FeatureRecord)>, Error> {
        let (comment_bytes, record_idx) = match slot {
            SfcSlot::Base(idx) => {
                let Some(bf) = self.base_features.get(idx) else {
                    return Ok(None);
                };
                let raw_comment = self.src.with_range(bf.span.clone(), |rs| rs.bytes.to_vec());
                let mut patches = Vec::new();
                for at in 0..bf.arg_ranges.len() {
                    let value = tx
                        .base_args
                        .get(&(idx, at))
                        .or_else(|| self.patched_base.get(&idx).and_then(|m| m.get(&at)));
                    if let Some(value) = value
                        && let Some(range) = bf.range(at, value)
                    {
                        patches.push((range, value.bytes().to_vec()));
                    }
                }
                let built = patched(patches, &raw_comment, cx)?;
                (built, bf.record_idx)
            }
            SfcSlot::Born(seq) => {
                let born = tx
                    .added_born
                    .iter()
                    .find(|(s, _)| *s == seq)
                    .map(|(_, b)| b)
                    .or_else(|| self.born_features.get(&seq));
                let Some(born) = born else {
                    return Ok(None);
                };
                let mut effective_args = born.args.clone();
                for (at, slot_val) in effective_args.iter_mut().enumerate() {
                    if let Some(override_val) = tx.born_args.get(&(seq, at)) {
                        *slot_val = override_val.clone();
                    }
                }
                let Some(text) = format_born_comment(
                    born.marker,
                    born.instance,
                    born.name,
                    &effective_args,
                    self.newline,
                ) else {
                    return Ok(None);
                };
                (text, 0)
            }
        };

        let mut mini = Vec::with_capacity(128 + comment_bytes.len());
        mini.extend_from_slice(
            b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('SCADEC level2 feature_mode'),'2;1');\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n",
        );
        mini.extend_from_slice(&comment_bytes);
        mini.extend_from_slice(b"\nENDSEC;\nEND-ISO-10303-21;\n");

        let exchange = quietly(cx, |cx| zumen_step::read(&mini, &DIALECT, cx))?;
        let mini_file = SxfFile::from_parts(mini, Mode::Sfc, exchange.records);
        let mut recs = sfc_records(&mini_file);
        if recs.len() != 1 {
            return Ok(None);
        }
        let Some(mut feat_rec) = recs.pop() else {
            return Ok(None);
        };
        let Some(inst) = mini_file
            .records()
            .get(feat_rec.record)
            .and_then(crate::Record::id)
        else {
            return Ok(None);
        };
        feat_rec.record = record_idx;
        Ok(Some((inst, feat_rec)))
    }

    /// インスタンス番号 `id` を持つ生存フィーチャが、この仮置き変更後に残っているか
    pub(super) fn has_surviving_feature_id(&self, tx: &SfcStepTx, id: u64) -> bool {
        let mut count = self.alive_id_counts.get(&id).copied().unwrap_or(0);
        for &idx in &tx.removed_base {
            if let Some(bf) = self.base_features.get(idx)
                && bf.instance == Some(id)
            {
                count = count.saturating_sub(1);
            }
        }
        for &seq in &tx.removed_born {
            if let Some(born) = self.born_features.get(&seq)
                && born.instance == id
            {
                count = count.saturating_sub(1);
            }
        }
        for (_, born) in &tx.added_born {
            if born.instance == id {
                count = count.saturating_add(1);
            }
        }
        count > 0
    }

    pub(super) fn commit_step(&mut self, tx: SfcStepTx) -> SfcPatch {
        let mut patch = SfcPatch::default();
        for idx in tx.removed_base {
            if self.removed_base.insert(idx) {
                if let Some(Some(id)) = self.base_features.get(idx).map(|b| b.instance) {
                    dec_id(&mut self.alive_id_counts, id);
                }
                patch.removed_base.push(idx);
            }
        }
        for seq in tx.removed_born {
            if let Some(born) = self.born_features.remove(&seq) {
                dec_id(&mut self.alive_id_counts, born.instance);
                patch.removed_born.push((seq, born));
            }
        }
        for ((idx, at), after) in tx.base_args {
            let map = self.patched_base.entry(idx).or_default();
            let before = map.insert(at, after.clone());
            patch.base_args.push((idx, at, before, after));
        }
        for ((seq, at), after) in tx.born_args {
            if let Some(born) = self.born_features.get_mut(&seq)
                && let Some(slot_ref) = born.args.get_mut(at)
            {
                let before = core::mem::replace(slot_ref, after.clone());
                patch.born_args.push((seq, at, before, after));
            }
        }
        for (seq, born) in tx.added_born {
            *self.alive_id_counts.entry(born.instance).or_default() += 1;
            self.born_features.insert(seq, born.clone());
            patch.added_born.push((seq, born));
        }
        self.next_born_seq = self.next_born_seq.max(tx.next_born_seq);
        patch
    }

    pub(super) fn revert_patch(&mut self, patch: &SfcPatch) {
        for (seq, _) in patch.added_born.iter().rev() {
            if let Some(born) = self.born_features.remove(seq) {
                dec_id(&mut self.alive_id_counts, born.instance);
            }
        }
        for (seq, at, before, _) in patch.born_args.iter().rev() {
            if let Some(born) = self.born_features.get_mut(seq)
                && let Some(slot_ref) = born.args.get_mut(*at)
            {
                *slot_ref = before.clone();
            }
        }
        for (idx, at, before, _) in patch.base_args.iter().rev() {
            if let Some(map) = self.patched_base.get_mut(idx) {
                match before {
                    Some(prev) => {
                        map.insert(*at, prev.clone());
                    }
                    None => {
                        map.remove(at);
                    }
                }
                if map.is_empty() {
                    self.patched_base.remove(idx);
                }
            }
        }
        for (seq, born) in patch.removed_born.iter().rev() {
            *self.alive_id_counts.entry(born.instance).or_default() += 1;
            self.born_features.insert(*seq, born.clone());
        }
        for &idx in patch.removed_base.iter().rev() {
            if self.removed_base.remove(&idx)
                && let Some(Some(id)) = self.base_features.get(idx).map(|b| b.instance)
            {
                *self.alive_id_counts.entry(id).or_default() += 1;
            }
        }
    }

    pub(super) fn reapply_patch(&mut self, patch: &SfcPatch) {
        for &idx in &patch.removed_base {
            if self.removed_base.insert(idx)
                && let Some(Some(id)) = self.base_features.get(idx).map(|b| b.instance)
            {
                dec_id(&mut self.alive_id_counts, id);
            }
        }
        for (seq, _) in &patch.removed_born {
            if let Some(born) = self.born_features.remove(seq) {
                dec_id(&mut self.alive_id_counts, born.instance);
            }
        }
        for (idx, at, _, after) in &patch.base_args {
            self.patched_base
                .entry(*idx)
                .or_default()
                .insert(*at, after.clone());
        }
        for (seq, at, _, after) in &patch.born_args {
            if let Some(born) = self.born_features.get_mut(seq)
                && let Some(slot_ref) = born.args.get_mut(*at)
            {
                *slot_ref = after.clone();
            }
        }
        for (seq, born) in &patch.added_born {
            *self.alive_id_counts.entry(born.instance).or_default() += 1;
            self.born_features.insert(*seq, born.clone());
        }
    }

    pub(super) fn source_map(&self, cx: &mut Cx<'_>) -> Result<SourceMap, Error> {
        let insertion = if self.born_features.is_empty() {
            None
        } else if let Some(sheet) = self
            .first_sheet_base
            .and_then(|index| self.base_features.get(index))
        {
            Some(sheet.record_idx as u64)
        } else {
            let mut insertion = None;
            for (index, feature) in self.base_features.iter().enumerate().rev() {
                work(cx, 1, &Ref::default())?;
                if !self.removed_base.contains(&index) {
                    insertion = Some(feature.record_idx.saturating_add(1) as u64);
                    break;
                }
            }
            insertion.or_else(|| {
                self.base_features
                    .first()
                    .map(|feature| feature.record_idx as u64)
            })
        };
        SourceMap::build(
            self.removed_base.iter().filter_map(|index| {
                self.base_features
                    .get(*index)
                    .map(|feature| feature.record_idx as u64)
            }),
            insertion,
            self.born_features.values().map(|feature| feature.instance),
            cx,
        )
    }

    pub(super) fn save(&self, cx: &mut Cx) -> Result<Vec<u8>, Error> {
        if self.is_clean() {
            work(cx, 1 + self.src.len() as u64 / 1024, &Ref::default())?;
            let mut out = Vec::with_capacity(self.src.len());
            self.src.append_range(&mut out, 0..self.src.len());
            return Ok(out);
        }

        let mut patches: Vec<(Range<usize>, Vec<u8>)> = Vec::new();
        for &idx in &self.removed_base {
            if let Some(bf) = self.base_features.get(idx) {
                patches.push((bf.span.start..bf.line_end, Vec::new()));
            }
        }
        for (&idx, arg_map) in &self.patched_base {
            if self.removed_base.contains(&idx) {
                continue;
            }
            let Some(bf) = self.base_features.get(idx) else {
                continue;
            };
            for (&at, value) in arg_map {
                if let Some(range) = bf.range(at, value) {
                    let start = bf.span.start + range.start;
                    let end = bf.span.start + range.end;
                    patches.push((start..end, value.bytes().to_vec()));
                }
            }
        }

        if !self.born_features.is_empty() {
            let nl = self.newline;
            if let Some(sheet_idx) = self.first_sheet_base {
                let Some(sheet_bf) = self.base_features.get(sheet_idx) else {
                    return Err(Error::Rejected);
                };
                let mut inserted = Vec::new();
                for born in self.born_features.values() {
                    let Some(comment) =
                        format_born_comment(born.marker, born.instance, born.name, &born.args, nl)
                    else {
                        return Err(Error::Rejected);
                    };
                    inserted.extend_from_slice(&comment);
                    inserted.extend_from_slice(nl.as_bytes());
                }
                let at = sheet_bf.span.start;
                patches.push((at..at, inserted));
            } else {
                // `drawing_sheet` がない場合は、生存している最後の基底フィーチャの直後、または全基底が削除済みなら最初の基底位置に置く
                let last_surviving = self
                    .base_features
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(idx, _)| !self.removed_base.contains(idx));
                if let Some((_, last_bf)) = last_surviving {
                    let mut inserted = Vec::new();
                    for born in self.born_features.values() {
                        let Some(comment) = format_born_comment(
                            born.marker,
                            born.instance,
                            born.name,
                            &born.args,
                            nl,
                        ) else {
                            return Err(Error::Rejected);
                        };
                        inserted.extend_from_slice(nl.as_bytes());
                        inserted.extend_from_slice(&comment);
                    }
                    let at = last_bf.span.end;
                    patches.push((at..at, inserted));
                } else if let Some(first_bf) = self.base_features.first() {
                    let mut inserted = Vec::new();
                    for born in self.born_features.values() {
                        let Some(comment) = format_born_comment(
                            born.marker,
                            born.instance,
                            born.name,
                            &born.args,
                            nl,
                        ) else {
                            return Err(Error::Rejected);
                        };
                        inserted.extend_from_slice(&comment);
                        inserted.extend_from_slice(nl.as_bytes());
                    }
                    let at = first_bf.span.start;
                    patches.push((at..at, inserted));
                }
            }
        }

        patched_source(patches, &self.src, cx)
    }
}

fn dec_id(counts: &mut BTreeMap<u64, u32>, id: u64) {
    if let Some(slot) = counts.get_mut(&id) {
        if *slot <= 1 {
            counts.remove(&id);
        } else {
            *slot -= 1;
        }
    }
}

fn format_born_comment(
    marker: Marker,
    instance: u64,
    name: &'static str,
    args: &[String],
    nl: &str,
) -> Option<Vec<u8>> {
    let (_, open, close) = Marker::ALL
        .iter()
        .find(|(found, _, _)| *found == marker)
        .copied()?;
    let arguments = Arguments {
        name,
        values: args.to_vec(),
    };
    let mut out = Vec::new();
    out.extend_from_slice(open);
    out.extend_from_slice(format!("{nl}#{instance} = {name}_feature(").as_bytes());
    for (at, value) in args.iter().enumerate() {
        if at > 0 {
            out.push(b',');
        }
        if is_string(&arguments, at) {
            out.extend(sfc_string(value)?);
        } else {
            out.extend_from_slice(format!("'{value}'").as_bytes());
        }
    }
    out.push(b')');
    out.extend_from_slice(nl.as_bytes());
    out.extend_from_slice(close);
    Some(out)
}
