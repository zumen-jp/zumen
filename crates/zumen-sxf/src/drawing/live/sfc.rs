//! SFC（フィーチャコメント形式）の差分状態・単一フィーチャ往復検証・書き出し。

use core::ops::Range;
use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Cx, Key, Ref};
use zumen_live::{PatchPayload, SourceBytes};
use zumen_sdk::{Error, quietly};

use crate::drawing::apply::text::{self, is_string, sfc_string};
use crate::drawing::apply::{Arguments, line_end, newline, patched, patched_source};
use crate::drawing::{FeatureRecord, sfc_records, work};
use crate::{DIALECT, Marker, Mode, SfcRecord, SxfFile};

use super::source::SourceMap;

/// SFC フィーチャの場所（開いた時点の基底フィーチャか、追加したフィーチャ）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum SfcSlot {
    Base(usize),
    Born(u32),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct BaseFeature {
    pub(super) record_idx: usize,
    pub(super) span_start: usize,
    pub(super) span_end: usize,
    pub(super) line_end: usize,
    pub(super) instance: u64,
    pub(super) meta_idx: u16,
    pub(super) arg_count: u16,
    pub(super) marker: Marker,
    pub(super) grouped: bool,
    pub(super) has_instance: bool,
}

impl BaseFeature {
    #[inline]
    fn span(&self) -> Range<usize> {
        self.span_start..self.span_end
    }

    #[inline]
    fn instance(&self) -> Option<u64> {
        self.has_instance.then_some(self.instance)
    }
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

fn is_sjis_lead(byte: u8) -> bool {
    matches!(byte, 0x81..=0x9f | 0xe0..=0xfc)
}

type ArgRangePair = (Option<Range<usize>>, Option<Range<usize>>);

/// 1 つの SFC コメント文字列から、各引数の `(内側範囲, 外側範囲)` を走査して取り出す（`zumen_step` と同じ規則）
fn scan_comment_arg_ranges(raw: &[u8]) -> Option<Vec<ArgRangePair>> {
    let close_len = Marker::ALL
        .iter()
        .find_map(|(_, open, close)| raw.starts_with(open).then_some(close.len()))?;
    let end = raw.len().checked_sub(close_len)?;
    let b_at = |i: usize| -> Option<u8> { (i < end).then(|| raw.get(i).copied()).flatten() };
    let mut pos = raw.iter().position(|&b| b == b'=')?.checked_add(1)?;
    while b_at(pos).is_some_and(|b| b.is_ascii_whitespace()) {
        pos += 1;
    }
    while b_at(pos).is_some_and(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'!')) {
        pos += 1;
    }
    while b_at(pos).is_some_and(|b| b.is_ascii_whitespace()) {
        pos += 1;
    }
    if raw.get(pos) != Some(&b'(') {
        return None;
    }
    pos += 1;
    let mut out = Vec::new();
    while b_at(pos).is_some_and(|b| b.is_ascii_whitespace()) {
        pos += 1;
    }
    if raw.get(pos) == Some(&b')') {
        return Some(out);
    }
    loop {
        while b_at(pos).is_some_and(|b| b.is_ascii_whitespace()) {
            pos += 1;
        }
        let byte = *raw.get(pos)?;
        let pair = if byte == b'(' {
            let mut depth = 1usize;
            pos += 1;
            while depth > 0 {
                let Some(b) = b_at(pos) else {
                    break;
                };
                if b == b'\'' {
                    pos += 1;
                    while b_at(pos).is_some_and(|c| c != b'\'') {
                        pos += 1;
                    }
                    if pos >= end {
                        return None;
                    }
                    pos += 1;
                } else if b == b'\\' && raw.get(pos + 1) == Some(&b'\'') {
                    pos += 2;
                    loop {
                        let cur = b_at(pos)?;
                        let next = raw.get(pos + 1).copied();
                        if is_sjis_lead(cur) && next.is_some() {
                            pos += 2;
                            continue;
                        }
                        if cur == b'\\' && next == Some(b'\\') {
                            pos += 2;
                            continue;
                        }
                        if cur == b'\\' && next == Some(b'\'') {
                            let mut after = pos + 2;
                            while b_at(after).is_some_and(|c| c.is_ascii_whitespace()) {
                                after += 1;
                            }
                            if b_at(after).is_none_or(|c| matches!(c, b',' | b')')) {
                                pos += 2;
                                break;
                            }
                        }
                        pos += 1;
                    }
                } else {
                    if b == b'(' {
                        depth += 1;
                    } else if b == b')' {
                        depth -= 1;
                    }
                    pos += 1;
                }
            }
            if depth != 0 {
                return None;
            }
            (None, None)
        } else if byte == b'\'' {
            let outer_start = pos;
            pos += 1;
            let inner_start = pos;
            while b_at(pos).is_some_and(|c| c != b'\'') {
                pos += 1;
            }
            if pos >= end {
                return None;
            }
            let inner_end = pos;
            pos += 1;
            (Some(inner_start..inner_end), Some(outer_start..pos))
        } else if byte == b'\\' && raw.get(pos + 1) == Some(&b'\'') {
            let outer_start = pos;
            pos += 2;
            loop {
                let cur = b_at(pos)?;
                let next = raw.get(pos + 1).copied();
                if is_sjis_lead(cur) && next.is_some() {
                    pos += 2;
                    continue;
                }
                if cur == b'\\' && next == Some(b'\\') {
                    pos += 2;
                    continue;
                }
                if cur == b'\\' && next == Some(b'\'') {
                    let mut after = pos + 2;
                    while b_at(after).is_some_and(|c| c.is_ascii_whitespace()) {
                        after += 1;
                    }
                    if b_at(after).is_none_or(|c| matches!(c, b',' | b')')) {
                        pos += 2;
                        break;
                    }
                }
                pos += 1;
            }
            (None, Some(outer_start..pos))
        } else {
            let start = pos;
            while b_at(pos).is_some_and(|c| c != b',' && c != b')') {
                pos += 1;
            }
            let mut trimmed = pos;
            while trimmed > start && raw.get(trimmed - 1).is_some_and(u8::is_ascii_whitespace) {
                trimmed -= 1;
            }
            (Some(start..trimmed), Some(start..trimmed))
        };
        out.push(pair);
        while b_at(pos).is_some_and(|b| b.is_ascii_whitespace()) {
            pos += 1;
        }
        match raw.get(pos) {
            Some(b',') => pos += 1,
            Some(b')') => return Some(out),
            _ => return None,
        }
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
    feature_meta: Vec<(Box<str>, Box<str>)>,
    base_features: Vec<BaseFeature>,
    first_sheet_base: Option<usize>,
    fixed_max_id: u64,
    sorted_base_ids: Vec<(u64, u32)>,
    removed_base: BTreeSet<usize>,
    patched_base: BTreeMap<usize, BTreeMap<usize, ArgValue>>,
    born_features: BTreeMap<u32, BornFeature>,
    next_born_seq: u32,
}

impl SfcState {
    pub(super) fn from_fast(
        src: SourceBytes,
        newline: &'static str,
        feature_meta: Vec<(Box<str>, Box<str>)>,
        base_features: Vec<BaseFeature>,
        first_sheet_base: Option<usize>,
    ) -> Self {
        let mut sorted_base_ids: Vec<(u64, u32)> = Vec::with_capacity(base_features.len());
        for (base_idx, bf) in base_features.iter().enumerate() {
            if bf.has_instance
                && let Ok(b_u32) = u32::try_from(base_idx)
            {
                sorted_base_ids.push((bf.instance, b_u32));
            }
        }
        if !sorted_base_ids.is_sorted_by(|a, b| a <= b) {
            sorted_base_ids.sort_unstable();
        }
        Self {
            src,
            newline,
            feature_meta,
            base_features,
            first_sheet_base,
            fixed_max_id: 0,
            sorted_base_ids,
            removed_base: BTreeSet::new(),
            patched_base: BTreeMap::new(),
            born_features: BTreeMap::new(),
            next_born_seq: 0,
        }
    }

    pub(super) fn new(
        src: SourceBytes,
        file: &SxfFile,
        records: &[FeatureRecord],
        grouped: &BTreeSet<usize>,
    ) -> Self {
        let raw = file.as_bytes();
        let nl = newline(raw);
        let mut feature_meta: Vec<(Box<str>, Box<str>)> = Vec::new();
        let mut base_features = Vec::with_capacity(records.len());
        let mut first_sheet_base = None;
        let mut sorted_base_ids: Vec<(u64, u32)> = Vec::with_capacity(records.len());

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
            let arg_count = entry
                .parts()
                .first()
                .and_then(|p| u16::try_from(p.args().len()).ok())
                .unwrap_or(u16::MAX);
            let meta_idx = if let Some(pos) = feature_meta.iter().position(|(nl_str, kind_str)| {
                nl_str.eq_ignore_ascii_case(&rec.name) && **kind_str == rec.kind
            }) {
                pos as u16
            } else {
                let idx = feature_meta.len() as u16;
                feature_meta.push((
                    rec.name.to_ascii_lowercase().into_boxed_str(),
                    rec.kind.clone().into_boxed_str(),
                ));
                idx
            };
            let base_idx = base_features.len();
            if first_sheet_base.is_none() && rec.name.eq_ignore_ascii_case("drawing_sheet") {
                first_sheet_base = Some(base_idx);
            }
            if let Some(id) = instance
                && let Ok(b_u32) = u32::try_from(base_idx)
            {
                sorted_base_ids.push((id, b_u32));
            }
            base_features.push(BaseFeature {
                record_idx: rec.record,
                span_start: span.start,
                span_end: span.end,
                line_end: le,
                instance: instance.unwrap_or(0),
                meta_idx,
                arg_count,
                marker,
                grouped: grouped.contains(&rec.record),
                has_instance: instance.is_some(),
            });
        }

        if !sorted_base_ids.is_sorted_by(|a, b| a <= b) {
            sorted_base_ids.sort_unstable();
        }

        let mut fixed_max_id = 0_u64;
        let mut bf_cursor = 0usize;
        for (idx, entry) in file.records().iter().enumerate() {
            while base_features
                .get(bf_cursor)
                .is_some_and(|bf| bf.record_idx < idx)
            {
                bf_cursor += 1;
            }
            let in_base = base_features
                .get(bf_cursor)
                .is_some_and(|bf| bf.record_idx == idx);
            if !in_base
                && let Some(id) = entry.id()
                && id > fixed_max_id
            {
                fixed_max_id = id;
            }
        }

        Self {
            src,
            newline: nl,
            feature_meta,
            base_features,
            first_sheet_base,
            fixed_max_id,
            sorted_base_ids,
            removed_base: BTreeSet::new(),
            patched_base: BTreeMap::new(),
            born_features: BTreeMap::new(),
            next_born_seq: 0,
        }
    }

    pub(super) fn resident_bytes(&self) -> usize {
        self.src.stored_bytes()
            + self.base_features.capacity() * core::mem::size_of::<BaseFeature>()
            + self.sorted_base_ids.capacity() * core::mem::size_of::<(u64, u32)>()
            + self.patched_base.len() * 64
            + self.born_features.len() * 96
    }

    pub(super) fn is_clean(&self) -> bool {
        self.removed_base.is_empty()
            && self.patched_base.is_empty()
            && self.born_features.is_empty()
    }

    pub(super) fn slot_for_record(&self, record_idx: usize) -> Option<SfcSlot> {
        self.base_features
            .binary_search_by_key(&record_idx, |bf| bf.record_idx)
            .ok()
            .map(SfcSlot::Base)
    }

    fn base_ranges(&self, bf: &BaseFeature) -> Option<Vec<ArgRangePair>> {
        self.src.with_range(bf.span(), |rs| {
            let start = bf.span_start.checked_sub(rs.base)?;
            let end = bf.span_end.checked_sub(rs.base)?;
            scan_comment_arg_ranges(rs.bytes.get(start..end)?)
        })
    }

    pub(super) fn slot_meta(&self, slot: SfcSlot) -> Option<(&str, &str, bool, usize)> {
        match slot {
            SfcSlot::Base(idx) => {
                let bf = self.base_features.get(idx)?;
                let (name_lower, kind) = self.feature_meta.get(usize::from(bf.meta_idx))?;
                Some((name_lower, kind, bf.grouped, bf.record_idx))
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
        let max_base = self
            .sorted_base_ids
            .iter()
            .rev()
            .find(|&&(_, b_idx)| !self.removed_base.contains(&(b_idx as usize)))
            .map_or(0, |&(id, _)| id);
        let max_born = self
            .born_features
            .values()
            .map(|b| b.instance)
            .max()
            .unwrap_or(0);
        let max_feat_id = max_base.max(max_born);
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
                let ranges = self.base_ranges(bf)?;
                ranges.get(at)?.0.as_ref()?;
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
                let ranges = self.base_ranges(bf)?;
                ranges.get(at)?.1.as_ref()?;
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
            SfcSlot::Base(idx) => {
                let bf = self.base_features.get(idx)?;
                if bf.arg_count < u16::MAX {
                    Some(usize::from(bf.arg_count))
                } else {
                    Some(self.base_ranges(bf)?.len())
                }
            }
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
                let id = bf.instance()?;
                if self.arg_count(slot)? > 1 {
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
                let raw_comment = self.src.with_range(bf.span(), |rs| rs.bytes.to_vec());
                let Some(ranges) = scan_comment_arg_ranges(&raw_comment) else {
                    return Ok(None);
                };
                let mut patches = Vec::new();
                for (at, (inner, outer)) in ranges.into_iter().enumerate() {
                    let value = tx
                        .base_args
                        .get(&(idx, at))
                        .or_else(|| self.patched_base.get(&idx).and_then(|m| m.get(&at)));
                    if let Some(value) = value {
                        let range = match value {
                            ArgValue::Inner(_) => inner,
                            ArgValue::Outer(_) => outer,
                        };
                        if let Some(range) = range {
                            patches.push((range, value.bytes().to_vec()));
                        }
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
        let lo = self.sorted_base_ids.partition_point(|&(k, _)| k < id);
        let hi = self.sorted_base_ids.partition_point(|&(k, _)| k <= id);
        let mut count = 0u32;
        for &(_, b_idx) in self.sorted_base_ids.get(lo..hi).unwrap_or(&[]) {
            let idx = b_idx as usize;
            if !self.removed_base.contains(&idx) && !tx.removed_base.contains(&idx) {
                count = count.saturating_add(1);
            }
        }
        for (&seq, born) in &self.born_features {
            if born.instance == id && !tx.removed_born.contains(&seq) {
                count = count.saturating_add(1);
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
                patch.removed_base.push(idx);
            }
        }
        for seq in tx.removed_born {
            if let Some(born) = self.born_features.remove(&seq) {
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
            self.born_features.insert(seq, born.clone());
            patch.added_born.push((seq, born));
        }
        self.next_born_seq = self.next_born_seq.max(tx.next_born_seq);
        patch
    }

    pub(super) fn revert_patch(&mut self, patch: &SfcPatch) {
        for (seq, _) in patch.added_born.iter().rev() {
            self.born_features.remove(seq);
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
            self.born_features.insert(*seq, born.clone());
        }
        for &idx in patch.removed_base.iter().rev() {
            self.removed_base.remove(&idx);
        }
    }

    pub(super) fn reapply_patch(&mut self, patch: &SfcPatch) {
        for &idx in &patch.removed_base {
            self.removed_base.insert(idx);
        }
        for (seq, _) in &patch.removed_born {
            self.born_features.remove(seq);
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
                patches.push((bf.span_start..bf.line_end, Vec::new()));
            }
        }
        for (&idx, arg_map) in &self.patched_base {
            if self.removed_base.contains(&idx) {
                continue;
            }
            let Some(bf) = self.base_features.get(idx) else {
                continue;
            };
            let Some(ranges) = self.base_ranges(bf) else {
                return Err(Error::Rejected);
            };
            for (&at, value) in arg_map {
                let rel = ranges.get(at).and_then(|(inner, outer)| match value {
                    ArgValue::Inner(_) => inner.clone(),
                    ArgValue::Outer(_) => outer.clone(),
                });
                if let Some(range) = rel {
                    let start = bf.span_start + range.start;
                    let end = bf.span_start + range.end;
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
                let at = sheet_bf.span_start;
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
                    let at = last_bf.span_end;
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
                    let at = first_bf.span_start;
                    patches.push((at..at, inserted));
                }
            }
        }

        patched_source(patches, &self.src, cx)
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
