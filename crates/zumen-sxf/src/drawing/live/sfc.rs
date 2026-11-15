//! SFC（フィーチャコメント形式）の差分状態・単一フィーチャ往復検証・書き出し。

use core::ops::Range;
use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Cx, Ref};
use zumen_live::{PatchPayload, SourceBytes};
use zumen_sdk::{Error, quietly};

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
    base_args: Vec<(usize, usize, Option<String>, String)>,
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
    base_args: BTreeMap<(usize, usize), String>,
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
    patched_base: BTreeMap<usize, BTreeMap<usize, String>>,
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
        match slot {
            SfcSlot::Base(idx) => {
                let bf = self.base_features.get(idx)?;
                for &(at, code) in codes {
                    bf.arg_ranges.get(at)?.as_ref()?;
                    tx.base_args.insert((idx, at), code.to_string());
                }
                Some(())
            }
            SfcSlot::Born(seq) => {
                let born = self.born_features.get(&seq)?;
                for &(at, code) in codes {
                    if at >= born.args.len() {
                        return None;
                    }
                    tx.born_args.insert((seq, at), code.to_string());
                }
                Some(())
            }
        }
    }

    pub(super) fn stage_shape(
        &self,
        tx: &mut SfcStepTx,
        slot: SfcSlot,
        args: &Arguments,
    ) -> Option<()> {
        let first = args.first();
        match slot {
            SfcSlot::Base(idx) => {
                let bf = self.base_features.get(idx)?;
                for (at, text) in args.values.iter().enumerate().skip(first) {
                    bf.arg_ranges.get(at)?.as_ref()?;
                    tx.base_args.insert((idx, at), text.clone());
                }
                Some(())
            }
            SfcSlot::Born(seq) => {
                let born = self.born_features.get(&seq)?;
                for (at, text) in args.values.iter().enumerate().skip(first) {
                    if at >= born.args.len() {
                        return None;
                    }
                    tx.born_args.insert((seq, at), text.clone());
                }
                Some(())
            }
        }
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
                for (at, maybe_rel) in bf.arg_ranges.iter().enumerate() {
                    let text = tx
                        .base_args
                        .get(&(idx, at))
                        .or_else(|| self.patched_base.get(&idx).and_then(|m| m.get(&at)));
                    if let (Some(rel), Some(t)) = (maybe_rel, text) {
                        patches.push((
                            (rel.start as usize)..(rel.end as usize),
                            t.as_bytes().to_vec(),
                        ));
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
                (text.into_bytes(), 0)
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
            for (&at, text) in arg_map {
                if let Some(Some(rel)) = bf.arg_ranges.get(at) {
                    let start = bf.span.start + rel.start as usize;
                    let end = bf.span.start + rel.end as usize;
                    patches.push((start..end, text.as_bytes().to_vec()));
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
                    inserted.extend_from_slice(comment.as_bytes());
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
                        inserted.extend_from_slice(comment.as_bytes());
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
                        inserted.extend_from_slice(comment.as_bytes());
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
    name: &str,
    args: &[String],
    nl: &str,
) -> Option<String> {
    let (_, open, close) = Marker::ALL
        .iter()
        .find(|(found, _, _)| *found == marker)
        .copied()?;
    let values: Vec<String> = args.iter().map(|v| format!("'{v}'")).collect();
    Some(format!(
        "{open}{nl}#{instance} = {name}_feature({}){nl}{close}",
        values.join(","),
        open = String::from_utf8_lossy(open),
        close = String::from_utf8_lossy(close),
    ))
}
