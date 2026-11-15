//! P21（STEP AP202 インスタンス形式）の差分状態・ガベージ回収・単一要素往復検証・書き出し。

use core::ops::Range;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use zumen_base::{Cx, Diagnostic, Ref, codes};
use zumen_live::{PatchPayload, RecordSlice, SourceBytes};
use zumen_sdk::{Error, quietly};

use crate::drawing::apply::p21::{
    CONTAINERS, GARBAGE, RefCounts, collect, count_and_note_refs, count_arguments, count_number,
    count_values, for_each_ref, real, scan,
};
use crate::drawing::apply::{
    Arguments, MARKER, line_end, line_end_in_source, newline, patched_source,
};
use crate::drawing::{FeatureRecord, LINE_TYPES, WIDTHS, work};
use crate::p21::{COLORS, Codes, MARKERS, single_occurrence_feature};
use crate::{DIALECT, Form, Mode, Record, SxfFile, Value};

use super::source::SourceMap;

/// 引数の場所（文の通し番号・部分の番号・引数の番号）
type Slot = (usize, usize, usize);

const MINI_P21_HEADER: &[u8] = b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('AP202'),'2;1');\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n";
const MINI_P21_FOOTER: &[u8] = b"\nENDSEC;\nEND-ISO-10303-21;\n";

#[derive(Clone)]
struct InstanceIndex {
    dense: Vec<u32>,
    sparse: BTreeMap<u64, usize>,
}

impl InstanceIndex {
    fn with_capacity(dense_cap: usize) -> Self {
        Self {
            dense: vec![u32::MAX; dense_cap],
            sparse: BTreeMap::new(),
        }
    }

    fn insert_first(&mut self, id: u64, stmt: usize) -> bool {
        if let Ok(idx) = usize::try_from(id)
            && let Some(slot) = self.dense.get_mut(idx)
        {
            if *slot == u32::MAX {
                if let Ok(stmt_u32) = u32::try_from(stmt) {
                    *slot = stmt_u32;
                    return true;
                }
            } else {
                return false;
            }
        }
        if self.sparse.contains_key(&id) {
            return false;
        }
        self.sparse.insert(id, stmt);
        true
    }

    fn remove_exact(&mut self, id: u64, stmt: usize) {
        if let Ok(idx) = usize::try_from(id)
            && let Some(slot) = self.dense.get_mut(idx)
            && usize::try_from(*slot).ok() == Some(stmt)
        {
            *slot = u32::MAX;
            return;
        }
        if self.sparse.get(&id).copied() == Some(stmt) {
            self.sparse.remove(&id);
        }
    }

    fn get(&self, id: u64) -> Option<usize> {
        if let Ok(idx) = usize::try_from(id)
            && let Some(&raw) = self.dense.get(idx)
        {
            if raw != u32::MAX {
                return Some(raw as usize);
            }
            if !self.sparse.contains_key(&id) {
                return None;
            }
        }
        self.sparse.get(&id).copied()
    }
}

/// 要素の並びを持つコンテナー実体（用紙・レイヤ・非表示・複合図形定義）の `O(log K)` 差分リスト
#[derive(Clone, Debug)]
struct ContainerList {
    base_len: usize,
    /// 同じ `#id` が元の並びの中に 2 回以上現れるときだけその回数を覚える
    base_dups: BTreeMap<u64, u32>,
    removed_base: BTreeSet<u64>,
    removed_base_total: usize,
    tail_by_seq: BTreeMap<u64, u64>,
    tail_seq_of: BTreeMap<u64, u64>,
    next_seq: u64,
    dirty: bool,
}

#[derive(Clone, Debug)]
struct ContainerListSnapshot {
    removed_base_had: bool,
    prev_tail_seq: Option<u64>,
    prev_dirty: bool,
    after_in_tail: Option<u64>,
}

impl ContainerList {
    fn from_base(items: &[Value]) -> Option<(Self, Vec<u64>)> {
        let mut counts: BTreeMap<u64, u32> = BTreeMap::new();
        for item in items {
            let Value::Ref(id) = item else {
                return None;
            };
            *counts.entry(*id).or_default() += 1;
        }
        let unique: Vec<u64> = counts.keys().copied().collect();
        let base_dups: BTreeMap<u64, u32> = counts.into_iter().filter(|(_, c)| *c >= 2).collect();
        Some((
            Self {
                base_len: items.len(),
                base_dups,
                removed_base: BTreeSet::new(),
                removed_base_total: 0,
                tail_by_seq: BTreeMap::new(),
                tail_seq_of: BTreeMap::new(),
                next_seq: 0,
                dirty: false,
            },
            unique,
        ))
    }

    #[inline]
    fn len(&self) -> usize {
        self.base_len
            .saturating_sub(self.removed_base_total)
            .saturating_add(self.tail_by_seq.len())
    }

    #[inline]
    fn contains(&self, id: u64, in_base: bool) -> bool {
        if self.tail_seq_of.contains_key(&id) {
            return true;
        }
        in_base && !self.removed_base.contains(&id)
    }

    fn apply_op(
        &mut self,
        id: u64,
        in_base: bool,
        push_after_remove: bool,
    ) -> ContainerListSnapshot {
        let removed_base_had = self.removed_base.contains(&id);
        let prev_tail_seq = self.tail_seq_of.remove(&id);
        let prev_dirty = self.dirty;
        if let Some(seq) = prev_tail_seq {
            self.tail_by_seq.remove(&seq);
        }
        if in_base && !removed_base_had {
            let mult = self.base_dups.get(&id).copied().unwrap_or(1) as usize;
            self.removed_base.insert(id);
            self.removed_base_total = self.removed_base_total.saturating_add(mult);
        }
        let after_in_tail = if push_after_remove {
            let seq = self.next_seq;
            self.next_seq = self.next_seq.saturating_add(1);
            self.tail_seq_of.insert(id, seq);
            self.tail_by_seq.insert(seq, id);
            Some(seq)
        } else {
            None
        };
        self.dirty = true;
        ContainerListSnapshot {
            removed_base_had,
            prev_tail_seq,
            prev_dirty,
            after_in_tail,
        }
    }

    fn revert_op(&mut self, id: u64, in_base: bool, snap: &ContainerListSnapshot) {
        if let Some(seq) = snap.after_in_tail {
            self.tail_by_seq.remove(&seq);
            self.tail_seq_of.remove(&id);
        }
        if in_base && !snap.removed_base_had && self.removed_base.remove(&id) {
            let mult = self.base_dups.get(&id).copied().unwrap_or(1) as usize;
            self.removed_base_total = self.removed_base_total.saturating_sub(mult);
        }
        if let Some(seq) = snap.prev_tail_seq {
            self.tail_seq_of.insert(id, seq);
            self.tail_by_seq.insert(seq, id);
        }
        self.dirty = snap.prev_dirty;
    }

    fn reapply_op(&mut self, id: u64, in_base: bool, snap: &ContainerListSnapshot) {
        if let Some(seq) = snap.prev_tail_seq {
            self.tail_seq_of.remove(&id);
            self.tail_by_seq.remove(&seq);
        }
        if in_base && !snap.removed_base_had && self.removed_base.insert(id) {
            let mult = self.base_dups.get(&id).copied().unwrap_or(1) as usize;
            self.removed_base_total = self.removed_base_total.saturating_add(mult);
        }
        if let Some(seq) = snap.after_in_tail {
            self.tail_seq_of.insert(id, seq);
            self.tail_by_seq.insert(seq, id);
            self.next_seq = self.next_seq.max(seq.saturating_add(1));
        }
        self.dirty = true;
    }

    fn write_rendered(&self, base_items: &[Value], out: &mut Vec<u8>) {
        out.push(b'(');
        let mut first = true;
        for item in base_items {
            if let Value::Ref(id) = item
                && !self.removed_base.contains(id)
            {
                if !first {
                    out.push(b',');
                }
                first = false;
                out.extend_from_slice(format!("#{id}").as_bytes());
            }
        }
        for id in self.tail_by_seq.values() {
            if !first {
                out.push(b',');
            }
            first = false;
            out.extend_from_slice(format!("#{id}").as_bytes());
        }
        out.push(b')');
    }
}

#[derive(Clone, Debug)]
struct BornStmt {
    id: u64,
    body: Vec<u8>,
    parsed_buf: Arc<[u8]>,
    record: Record,
    extra_leading_nl: bool,
}

#[derive(Clone, Copy, Debug)]
enum StagedContainerOp {
    Remove,
    MoveToEnd,
}

/// 1 回のトランザクションで P21 状態に当てた変更（`undo` / `redo` 用）
#[derive(Clone, Debug, Default)]
pub(super) struct P21Patch {
    added_stmts: Vec<BornStmt>,
    removed_stmts: Vec<usize>,
    arg_diffs: Vec<(Slot, Option<String>, String)>,
    small_list_diffs: Vec<(Slot, Option<Vec<u64>>, Vec<u64>)>,
    container_ops: Vec<(Slot, u64, bool, ContainerListSnapshot)>,
    ref_deltas: Vec<(u64, i64)>,
    added_colors: Vec<(u64, i64)>,
    added_line_types: Vec<(u64, i64)>,
}

impl PatchPayload for P21Patch {
    const INLINE_BYTES: usize = {
        assert!(core::mem::size_of::<Self>() <= 256);
        256
    };

    fn heap_bytes(&self) -> usize {
        let born_bytes: usize = self
            .added_stmts
            .iter()
            .map(|b| b.body.capacity() + 64)
            .sum();
        born_bytes
            + self.removed_stmts.capacity() * 8
            + self.arg_diffs.capacity() * 64
            + self.small_list_diffs.capacity() * 64
            + self.container_ops.capacity() * 64
            + self.ref_deltas.capacity() * 16
    }
}

/// 1 回の `apply` 中に仮置きする P21 変更
pub(super) struct P21StepTx {
    args: BTreeMap<Slot, String>,
    small_lists: BTreeMap<Slot, Vec<u64>>,
    container_ops: BTreeMap<(Slot, u64), StagedContainerOp>,
    removed: BTreeSet<usize>,
    added: Vec<(u64, Vec<u8>)>,
    next: u64,
    loose: Vec<u64>,
    predefined: BTreeMap<(&'static str, &'static str), u64>,
    added_colors: Vec<(u64, i64)>,
    added_line_types: Vec<(u64, i64)>,
    ref_deltas: BTreeMap<u64, i64>,
    removed_last_alive: bool,
}

#[derive(Clone)]
pub(super) struct P21State {
    src: SourceBytes,
    newline: &'static str,
    base_records: Arc<[Record]>,
    last_base_instance: Option<(usize, Range<usize>, usize)>,
    born_stmts: Vec<BornStmt>,
    instances: InstanceIndex,
    alive_id_counts: BTreeMap<u64, u32>,
    ref_counts: RefCounts,
    container_lists: BTreeMap<Slot, ContainerList>,
    /// 元のコンテナーの要素列に `#id` が入っていた `Slot` の集合
    base_id_containers: BTreeMap<u64, BTreeSet<Slot>>,
    /// いま `#id` が入っているコンテナーの `Slot` の集合
    current_id_containers: BTreeMap<u64, BTreeSet<Slot>>,
    /// コンテナーの要素列以外の引数から `#id` を指している生存文の数
    non_container_refs: BTreeMap<u64, u32>,
    first_sheet: Option<(u64, Slot)>,
    length_measures: Vec<(u64, usize, usize)>,
    codes: Codes,
    base_widths: Vec<Option<f64>>,
    /// `base_widths` の各利用者定義線幅を持つ生存 `CURVE_STYLE` の数
    user_width_counts: Vec<u32>,
    args: BTreeMap<Slot, String>,
    small_lists: BTreeMap<Slot, Vec<u64>>,
    removed: BTreeSet<usize>,
}

impl P21State {
    pub(super) fn new(
        src: SourceBytes,
        file: &SxfFile,
        codes: Codes,
        base_widths: Vec<Option<f64>>,
        cx: &mut Cx,
    ) -> Result<Self, Error> {
        let records_slice = file.records();
        let dense_cap = records_slice.len().saturating_mul(4).saturating_add(64);
        let mut instances = InstanceIndex::with_capacity(dense_cap);
        let mut ref_counts = RefCounts::with_capacity(dense_cap);
        let mut alive_id_counts: BTreeMap<u64, u32> = BTreeMap::new();
        let mut first_sheet: Option<(u64, Slot)> = None;
        let mut length_measures = Vec::new();
        let mut last_base_instance: Option<(usize, Range<usize>, usize)> = None;
        let mut container_lists: BTreeMap<Slot, ContainerList> = BTreeMap::new();
        let mut base_id_containers: BTreeMap<u64, BTreeSet<Slot>> = BTreeMap::new();
        let mut current_id_containers: BTreeMap<u64, BTreeSet<Slot>> = BTreeMap::new();
        let mut non_container_refs: BTreeMap<u64, u32> = BTreeMap::new();
        let mut user_width_counts = vec![0_u32; base_widths.len()];

        let raw = file.as_bytes();
        for (record_idx, entry) in records_slice.iter().enumerate() {
            work(cx, 1, &Ref::default())?;
            if let Some(id) = entry.id() {
                *alive_id_counts.entry(id).or_default() += 1;
                let span = entry.span();
                let le = line_end(raw, span.end);
                if last_base_instance
                    .as_ref()
                    .is_none_or(|(_, prev_span, _)| span.end >= prev_span.end)
                {
                    last_base_instance = Some((record_idx, span, le));
                }
                if instances.insert_first(id, record_idx) {
                    for (part_idx, part) in entry.parts().iter().enumerate() {
                        let name = file.bytes(part.name());
                        if name.eq_ignore_ascii_case(b"DRAWING_SHEET_REVISION") {
                            if first_sheet.is_none_or(|(best_id, _)| id < best_id) {
                                first_sheet = Some((id, (record_idx, part_idx, 1)));
                            }
                        } else if name.eq_ignore_ascii_case(b"LENGTH_MEASURE_WITH_UNIT") {
                            length_measures.push((id, record_idx, part_idx));
                        }
                    }
                }
            }

            let container_slot = CONTAINERS.iter().find_map(|(cname, cat)| {
                let p_idx = entry
                    .parts()
                    .iter()
                    .position(|p| file.bytes(p.name()).eq_ignore_ascii_case(cname.as_bytes()))?;
                Some((record_idx, p_idx, *cat))
            });

            let mut ref_count = 0_u64;
            for (part_idx, part) in entry.parts().iter().enumerate() {
                work(cx, 1, &Ref::default())?;
                count_and_note_refs(part.args(), &mut ref_counts, &mut ref_count, cx)?;
                for (at, val) in part.args().iter().enumerate() {
                    let slot = (record_idx, part_idx, at);
                    if Some(slot) == container_slot
                        && let Value::List(items) = val
                        && let Some((clist, unique_ids)) = ContainerList::from_base(items)
                    {
                        for uid in unique_ids {
                            base_id_containers.entry(uid).or_default().insert(slot);
                            current_id_containers.entry(uid).or_default().insert(slot);
                        }
                        container_lists.insert(slot, clist);
                    } else {
                        for_each_ref(core::slice::from_ref(val), &mut |rid| {
                            *non_container_refs.entry(rid).or_default() += 1;
                        });
                    }
                }
            }
            if ref_count > 0 {
                work(cx, ref_count, &Ref::default())?;
            }
        }
        length_measures.sort_unstable_by_key(|(id, _, _)| *id);

        if !base_widths.is_empty() {
            for (record_idx, entry) in records_slice.iter().enumerate() {
                if let Some(id) = entry.id()
                    && instances.get(id) == Some(record_idx)
                    && let Some(part) = entry
                        .parts()
                        .iter()
                        .find(|p| file.bytes(p.name()).eq_ignore_ascii_case(b"CURVE_STYLE"))
                    && let Some(mm) = curve_style_mm(file, &instances, part.args().get(2))
                    && !WIDTHS.iter().any(|w| (w - mm).abs() < 1e-6)
                    && let Some(pos) = base_widths.iter().position(|w| *w == Some(mm))
                    && let Some(slot) = user_width_counts.get_mut(pos)
                {
                    *slot = slot.saturating_add(1);
                }
            }
        }

        let base_records: Arc<[Record]> = Arc::from(records_slice.to_vec());
        let nl = newline(raw);

        Ok(Self {
            src,
            newline: nl,
            base_records,
            last_base_instance,
            born_stmts: Vec::new(),
            instances,
            alive_id_counts,
            ref_counts,
            container_lists,
            base_id_containers,
            current_id_containers,
            non_container_refs,
            first_sheet,
            length_measures,
            codes,
            base_widths,
            user_width_counts,
            args: BTreeMap::new(),
            small_lists: BTreeMap::new(),
            removed: BTreeSet::new(),
        })
    }

    pub(super) fn resident_bytes(&self) -> usize {
        self.src.stored_bytes()
            + self.base_records.len() * core::mem::size_of::<Record>()
            + self.born_stmts.capacity() * core::mem::size_of::<BornStmt>()
            + self.args.len() * 64
            + self.small_lists.len() * 64
    }

    pub(super) fn is_clean(&self) -> bool {
        self.born_stmts.is_empty()
            && self.removed.is_empty()
            && self.args.is_empty()
            && self.small_lists.is_empty()
            && self.container_lists.values().all(|c| !c.dirty)
    }

    /// いま生存している利用者定義線幅の表（削除された `CURVE_STYLE` しか持たなかった線幅は除く）
    pub(super) fn active_widths(&self) -> Vec<Option<f64>> {
        if self.base_widths.is_empty() {
            return Vec::new();
        }
        self.base_widths
            .iter()
            .zip(&self.user_width_counts)
            .filter_map(|(&w, &cnt)| (w.is_none() || cnt > 0).then_some(w))
            .collect()
    }

    pub(super) fn stmt_for_instance(&self, id: u64) -> Option<usize> {
        let stmt = self.instances.get(id)?;
        (!self.removed.contains(&stmt)).then_some(stmt)
    }

    fn record_of(&self, stmt: usize) -> Option<&Record> {
        let base_len = self.base_records.len();
        if stmt < base_len {
            self.base_records.get(stmt)
        } else {
            self.born_stmts.get(stmt - base_len).map(|b| &b.record)
        }
    }

    fn with_stmt_bytes<R>(
        &self,
        stmt: usize,
        f: impl FnOnce(RecordSlice<'_>, &Record) -> R,
    ) -> Option<R> {
        let base_len = self.base_records.len();
        if stmt < base_len {
            let rec = self.base_records.get(stmt)?;
            Some(self.src.with_range(rec.span(), |rs| f(rs, rec)))
        } else {
            let born = self.born_stmts.get(stmt - base_len)?;
            Some(f(
                RecordSlice {
                    base: 0,
                    bytes: &born.parsed_buf,
                },
                &born.record,
            ))
        }
    }

    fn last_alive_stmt(&self) -> Option<usize> {
        let base_len = self.base_records.len();
        for idx in (0..self.born_stmts.len()).rev() {
            let stmt = base_len + idx;
            if !self.removed.contains(&stmt) {
                return Some(stmt);
            }
        }
        for (idx, entry) in self.base_records.iter().enumerate().rev() {
            if entry.id().is_some() && !self.removed.contains(&idx) {
                return Some(idx);
            }
        }
        None
    }

    pub(super) fn begin_step(&self) -> P21StepTx {
        let next = match self.alive_id_counts.keys().next_back().copied() {
            Some(max_id) => max_id.saturating_add(1),
            None => 1,
        };
        P21StepTx {
            args: BTreeMap::new(),
            small_lists: BTreeMap::new(),
            container_ops: BTreeMap::new(),
            removed: BTreeSet::new(),
            added: Vec::new(),
            next,
            loose: Vec::new(),
            predefined: BTreeMap::new(),
            added_colors: Vec::new(),
            added_line_types: Vec::new(),
            ref_deltas: BTreeMap::new(),
            removed_last_alive: false,
        }
    }

    fn is_removed(&self, tx: &P21StepTx, stmt: usize) -> bool {
        self.removed.contains(&stmt) || tx.removed.contains(&stmt)
    }

    fn part(&self, stmt: usize, name: &str) -> Option<usize> {
        self.with_stmt_bytes(stmt, |rs, rec| {
            rec.parts().iter().position(|p| {
                slice_in(rs, p.name()).is_some_and(|n| n.eq_ignore_ascii_case(name.as_bytes()))
            })
        })
        .flatten()
    }

    fn value_ref(&self, (stmt, part, at): Slot) -> Option<&Value> {
        self.record_of(stmt)?.parts().get(part)?.args().get(at)
    }

    fn number_at(&self, (stmt, part, at): Slot) -> Option<f64> {
        self.with_stmt_bytes(stmt, |rs, rec| {
            let val = rec.parts().get(part)?.args().get(at)?;
            number_in_slice(rs, val)
        })
        .flatten()
    }

    fn slot(&self, tx: &P21StepTx, id: u64, name: &str, at: usize) -> Option<Slot> {
        let stmt = self.instances.get(id)?;
        if self.is_removed(tx, stmt) {
            return None;
        }
        Some((stmt, self.part(stmt, name)?, at))
    }

    fn small_list(&self, tx: &P21StepTx, slot: Slot) -> Option<Vec<u64>> {
        if let Some(list) = tx.small_lists.get(&slot) {
            return Some(list.clone());
        }
        if let Some(list) = self.small_lists.get(&slot) {
            return Some(list.clone());
        }
        match self.value_ref(slot)? {
            Value::List(items) => items
                .iter()
                .map(|item| match item {
                    Value::Ref(id) => Some(*id),
                    _ => None,
                })
                .collect(),
            _ => None,
        }
    }

    fn reference(&self, tx: &P21StepTx, slot: Slot) -> Option<u64> {
        if let Some(text) = tx.args.get(&slot).or_else(|| self.args.get(&slot)) {
            return text.trim().strip_prefix('#')?.parse::<u64>().ok();
        }
        match self.value_ref(slot)? {
            Value::Ref(id) => Some(*id),
            _ => None,
        }
    }

    fn occurrence(&self, stmt: usize) -> Option<(Slot, Slot)> {
        self.part(stmt, "ANNOTATION_CURVE_OCCURRENCE")
            .or_else(|| self.part(stmt, "ANNOTATION_SYMBOL_OCCURRENCE"))?;
        let part = self.part(stmt, "STYLED_ITEM").or_else(|| {
            let rec = self.record_of(stmt)?;
            (rec.parts().len() == 1).then_some(0)
        })?;
        let rec = self.record_of(stmt)?;
        let count = rec.parts().get(part)?.args().len();
        let at = usize::from(count >= 3);
        Some(((stmt, part, at), (stmt, part, at + 1)))
    }

    fn alloc_instance(&self, tx: &mut P21StepTx, body: impl Into<Vec<u8>>) -> Option<u64> {
        let id = tx.next;
        tx.next = tx.next.checked_add(1)?;
        tx.added.push((id, body.into()));
        Some(id)
    }

    fn alloc_point(&self, tx: &mut P21StepTx, x: f64, y: f64) -> Option<u64> {
        let body = format!("CARTESIAN_POINT(' ',({},{}))", real(x)?, real(y)?);
        self.alloc_instance(tx, body)
    }

    fn alloc_predefined(
        &self,
        tx: &mut P21StepTx,
        entity: &'static str,
        name: &'static str,
    ) -> Option<(u64, bool)> {
        if let Some(&id) = tx.predefined.get(&(entity, name)) {
            return Some((id, false));
        }
        let id = self.alloc_instance(tx, format!("{entity}('{name}')"))?;
        tx.predefined.insert((entity, name), id);
        Some((id, true))
    }

    fn resolve_color(&self, tx: &mut P21StepTx, code: i64) -> Option<u64> {
        if let Some((&id, _)) = self.codes.colors.iter().find(|(_, known)| **known == code) {
            return Some(id);
        }
        let name = COLORS.get(usize::try_from(code.checked_sub(1)?).ok()?)?;
        let (id, newly_created) =
            self.alloc_predefined(tx, "DRAUGHTING_PRE_DEFINED_COLOUR", name)?;
        if newly_created {
            tx.added_colors.push((id, code));
        }
        Some(id)
    }

    fn resolve_line_type(&self, tx: &mut P21StepTx, code: i64) -> Option<u64> {
        if let Some((&id, _)) = self
            .codes
            .line_types
            .iter()
            .find(|(_, known)| **known == code)
        {
            return Some(id);
        }
        let name = LINE_TYPES.get(usize::try_from(code.checked_sub(1)?).ok()?)?;
        let (id, newly_created) =
            self.alloc_predefined(tx, "DRAUGHTING_PRE_DEFINED_CURVE_FONT", name)?;
        if newly_created {
            tx.added_line_types.push((id, code));
        }
        Some(id)
    }

    fn resolve_width(&self, active_widths: &[Option<f64>], code: i64) -> Option<String> {
        let mm = match usize::try_from(code.checked_sub(1)?).ok()? {
            at if at < WIDTHS.len() => *WIDTHS.get(at)?,
            _ => (*active_widths.get(usize::try_from(code.checked_sub(11)?).ok()?)?)?,
        };
        for &(id, stmt, part) in &self.length_measures {
            if self.removed.contains(&stmt) {
                continue;
            }
            if self.number_at((stmt, part, 0)) == Some(mm) {
                return Some(format!("#{id}"));
            }
        }
        Some(format!("POSITIVE_LENGTH_MEASURE({})", real(mm)?))
    }

    fn build_curve_style(
        &self,
        tx: &mut P21StepTx,
        active_widths: &[Option<f64>],
        was: Option<u64>,
        codes: &[(usize, i64)],
    ) -> Option<u64> {
        let original: Option<[Vec<u8>; 4]> = match was {
            Some(was_id) => {
                let stmt = self.instances.get(was_id)?;
                if self.is_removed(tx, stmt) {
                    return None;
                }
                let part = self.part(stmt, "CURVE_STYLE")?;
                self.with_stmt_bytes(stmt, |rs, rec| {
                    let [v0, v1, v2, v3] = rec.parts().get(part)?.args().first_chunk::<4>()?;
                    let render_one = |val: &Value| {
                        let mut tmp = Vec::new();
                        write_value_slice(rs, val, &mut tmp);
                        tmp
                    };
                    Some([
                        render_one(v0),
                        render_one(v1),
                        render_one(v2),
                        render_one(v3),
                    ])
                })
                .flatten()
                .map(Some)?
            }
            None => None,
        };
        let mut changed: [Option<String>; 4] = Default::default();
        for (at, code) in codes {
            let value = match at {
                1 => format!("#{}", self.resolve_color(tx, *code)?),
                2 => format!("#{}", self.resolve_line_type(tx, *code)?),
                3 => self.resolve_width(active_widths, *code)?,
                _ => continue,
            };
            let dst = match at {
                1 => 3,
                other => *other - 1,
            };
            *changed.get_mut(dst)? = Some(value);
        }
        let mut body = b"CURVE_STYLE(".to_vec();
        for (at, change) in changed.iter().enumerate() {
            if at > 0 {
                body.push(b',');
            }
            if let Some(value) = change {
                body.extend_from_slice(value.as_bytes());
            } else if let Some(values) = &original {
                body.extend_from_slice(values.get(at)?);
            } else if at == 0 {
                body.extend_from_slice(b"' '");
            } else {
                return None;
            }
        }
        body.push(b')');
        let style = self.alloc_instance(tx, body)?;
        self.alloc_instance(tx, format!("PRESENTATION_STYLE_ASSIGNMENT((#{style}))"))
    }

    fn build_circle(&self, tx: &mut P21StepTx, x: f64, y: f64, radius: f64) -> Option<u64> {
        let center = self.alloc_point(tx, x, y)?;
        let placement = self.alloc_instance(tx, format!("AXIS2_PLACEMENT_2D(' ',#{center},$)"))?;
        self.alloc_instance(tx, format!("CIRCLE(' ',#{placement},{})", real(radius)?))
    }

    fn build_geometry(&self, tx: &mut P21StepTx, args: &Arguments) -> Option<u64> {
        let value = |at: usize| args.values.get(at)?.parse::<f64>().ok();
        match args.name {
            "line" => {
                let (x1, y1, x2, y2) = (value(4)?, value(5)?, value(6)?, value(7)?);
                let length = libm::hypot(x2 - x1, y2 - y1);
                if !(length > 0.0 && length.is_finite()) {
                    return None;
                }
                let start = self.alloc_point(tx, x1, y1)?;
                let end = self.alloc_point(tx, x2, y2)?;
                let (dx, dy) = ((x2 - x1) / length, (y2 - y1) / length);
                let direction = self
                    .alloc_instance(tx, format!("DIRECTION(' ',({},{}))", real(dx)?, real(dy)?))?;
                let vector =
                    self.alloc_instance(tx, format!("VECTOR(' ',#{direction},{})", real(length)?))?;
                let line = self.alloc_instance(tx, format!("LINE(' ',#{start},#{vector})"))?;
                self.alloc_instance(
                    tx,
                    format!("TRIMMED_CURVE(' ',#{line},(#{start}),(#{end}),.T.,.CARTESIAN.)"),
                )
            }
            "polyline" => {
                let coordinates = |at: usize| -> Option<Vec<f64>> {
                    let list = args.values.get(at)?;
                    let inner = list.strip_prefix('(')?.strip_suffix(')')?;
                    inner.split(',').map(|item| item.parse().ok()).collect()
                };
                let (xs, ys) = (coordinates(5)?, coordinates(6)?);
                if xs.len() != ys.len() || xs.len() < 2 {
                    return None;
                }
                let mut points = Vec::new();
                for (x, y) in xs.into_iter().zip(ys) {
                    points.push(format!("#{}", self.alloc_point(tx, x, y)?));
                }
                self.alloc_instance(tx, format!("POLYLINE(' ',({}))", points.join(",")))
            }
            "circle" => self.build_circle(tx, value(4)?, value(5)?, value(6)?),
            "arc" => {
                let circle = self.build_circle(tx, value(4)?, value(5)?, value(6)?)?;
                let sense = if args.values.get(7)? == "0" { "T" } else { "F" };
                let start = real(value(8)?.to_radians())?;
                let end = real(value(9)?.to_radians())?;
                self.alloc_instance(
                    tx,
                    format!(
                        "TRIMMED_CURVE(' ',#{circle},(PARAMETER_VALUE({start})),(PARAMETER_VALUE({end})),.{sense}.,.PARAMETER.)"
                    ),
                )
            }
            _ => None,
        }
    }

    fn build_marker(&self, tx: &mut P21StepTx, args: &Arguments) -> Option<u64> {
        let value = |at: usize| args.values.get(at)?.parse::<f64>().ok();
        let (x, y, angle, scale) = (value(2)?, value(3)?, value(5)?, value(6)?);
        let code: usize = args.values.get(4)?.parse().ok()?;
        let name = MARKERS.get(code.checked_sub(1)?)?;
        if scale == 0.0 {
            return None;
        }
        let origin = self.alloc_point(tx, x, y)?;
        let radians = angle.to_radians();
        let direction = self.alloc_instance(
            tx,
            format!(
                "DIRECTION(' ',({},{}))",
                real(libm::cos(radians))?,
                real(libm::sin(radians))?
            ),
        )?;
        let placement = self.alloc_instance(
            tx,
            format!("AXIS2_PLACEMENT_2D(' ',#{origin},#{direction})"),
        )?;
        let scale_s = real(scale)?;
        let target = self.alloc_instance(
            tx,
            format!("SYMBOL_TARGET(' ',#{placement},{scale_s},{scale_s})"),
        )?;
        let marker =
            self.alloc_instance(tx, format!("PRE_DEFINED_POINT_MARKER_SYMBOL('{name}')"))?;
        self.alloc_instance(tx, format!("DEFINED_SYMBOL(' ',#{marker},#{target})"))
    }

    fn build_symbol_colour_style(&self, tx: &mut P21StepTx, code: i64) -> Option<u64> {
        let colour = self.resolve_color(tx, code)?;
        let symbol = self.alloc_instance(tx, format!("SYMBOL_COLOUR(#{colour})"))?;
        self.alloc_instance(tx, format!("SYMBOL_STYLE(' ',#{symbol})"))
    }

    fn build_symbol_style(&self, tx: &mut P21StepTx, code: i64) -> Option<u64> {
        let style = self.build_symbol_colour_style(tx, code)?;
        self.alloc_instance(tx, format!("PRESENTATION_STYLE_ASSIGNMENT((#{style}))"))
    }

    fn container_contains(&self, tx: &P21StepTx, slot: Slot, id: u64) -> Option<bool> {
        let clist = self.container_lists.get(&slot)?;
        if let Some(&op) = tx.container_ops.get(&(slot, id)) {
            return Some(match op {
                StagedContainerOp::Remove => false,
                StagedContainerOp::MoveToEnd => true,
            });
        }
        let in_base = self
            .base_id_containers
            .get(&id)
            .is_some_and(|s| s.contains(&slot));
        Some(clist.contains(id, in_base))
    }

    fn container_len_after(&self, tx: &P21StepTx, slot: Slot) -> Option<usize> {
        let clist = self.container_lists.get(&slot)?;
        let mut len = clist.len() as isize;
        let range = (slot, 0)..=(slot, u64::MAX);
        for (&(_, id), &op) in tx.container_ops.range(range) {
            let in_base = self
                .base_id_containers
                .get(&id)
                .is_some_and(|s| s.contains(&slot));
            let was_in = clist.contains(id, in_base);
            let mult = if was_in && in_base && !clist.removed_base.contains(&id) {
                clist.base_dups.get(&id).copied().unwrap_or(1) as isize
            } else {
                isize::from(was_in)
            };
            match op {
                StagedContainerOp::Remove => {
                    len -= mult;
                }
                StagedContainerOp::MoveToEnd => {
                    len = len - mult + 1;
                }
            }
        }
        usize::try_from(len).ok()
    }

    fn assign_layer(&self, tx: &mut P21StepTx, id: u64, code: i64) -> Option<()> {
        let target = *self
            .codes
            .layers
            .get(usize::try_from(code.checked_sub(1)?).ok()?)?;
        let mut pending: Vec<(Slot, StagedContainerOp)> = Vec::new();
        for &layer_ent in &self.codes.layers {
            let slot = self.slot(tx, layer_ent, "PRESENTATION_LAYER_ASSIGNMENT", 2)?;
            let had = self.container_contains(tx, slot, id)?;
            if layer_ent == target {
                pending.push((slot, StagedContainerOp::MoveToEnd));
            } else if had {
                pending.push((slot, StagedContainerOp::Remove));
            }
        }
        for &(slot, op) in &pending {
            tx.container_ops.insert((slot, id), op);
        }
        for &(slot, _) in &pending {
            if self.container_len_after(tx, slot)? == 0 {
                return None;
            }
        }
        Some(())
    }

    pub(super) fn stage_codes(
        &self,
        tx: &mut P21StepTx,
        active_widths: &[Option<f64>],
        stmt: usize,
        codes: &[(usize, i64)],
        cx: &mut Cx,
    ) -> Result<Option<()>, Error> {
        self.count_codes_work(stmt, codes, active_widths, cx)?;
        Ok((|| {
            let (styles, _) = self.occurrence(stmt)?;
            let rec = self.record_of(stmt)?;
            let id = rec.id()?;
            for (at, code) in codes {
                if *at == 0 {
                    self.assign_layer(tx, id, *code)?;
                }
            }
            let style: Vec<(usize, i64)> =
                codes.iter().copied().filter(|(at, _)| *at > 0).collect();
            if style.is_empty() {
                return Some(());
            }
            if self.part(stmt, "ANNOTATION_SYMBOL_OCCURRENCE").is_some() {
                let [(1, code)] = style.as_slice() else {
                    return None;
                };
                let mut list = self.small_list(tx, styles)?;
                let found = list.iter().position(|assignment| {
                    self.slot(tx, *assignment, "PRESENTATION_STYLE_ASSIGNMENT", 0)
                        .and_then(|s| self.small_list(tx, s))
                        .is_some_and(|items| {
                            items
                                .iter()
                                .any(|st| self.slot(tx, *st, "SYMBOL_STYLE", 0).is_some())
                        })
                });
                match found {
                    Some(index) => {
                        let was = *list.get(index)?;
                        let new_sym_style = self.build_symbol_colour_style(tx, *code)?;
                        let was_slot = self.slot(tx, was, "PRESENTATION_STYLE_ASSIGNMENT", 0)?;
                        let mut merged = Vec::new();
                        for item in self.small_list(tx, was_slot)? {
                            let item = if self.slot(tx, item, "SYMBOL_STYLE", 0).is_some() {
                                new_sym_style
                            } else {
                                item
                            };
                            if !merged.contains(&item) {
                                merged.push(item);
                            }
                        }
                        let formatted: Vec<String> =
                            merged.iter().map(|it| format!("#{it}")).collect();
                        let assignment = self.alloc_instance(
                            tx,
                            format!("PRESENTATION_STYLE_ASSIGNMENT(({}))", formatted.join(",")),
                        )?;
                        *list.get_mut(index)? = assignment;
                        tx.loose.push(was);
                    }
                    None => list.push(self.build_symbol_style(tx, *code)?),
                }
                tx.small_lists.insert(styles, list);
                return Some(());
            }

            let mut list = self.small_list(tx, styles)?;
            let (index, was, curve) = list.iter().enumerate().find_map(|(index, assignment)| {
                let s = self.slot(tx, *assignment, "PRESENTATION_STYLE_ASSIGNMENT", 0)?;
                let curve = self
                    .small_list(tx, s)?
                    .into_iter()
                    .find(|st| self.slot(tx, *st, "CURVE_STYLE", 0).is_some())?;
                Some((index, *assignment, curve))
            })?;
            let was_slot = self.slot(tx, was, "PRESENTATION_STYLE_ASSIGNMENT", 0)?;
            if self.small_list(tx, was_slot)?.len() != 1 {
                return None;
            }
            let assignment = self.build_curve_style(tx, active_widths, Some(curve), &style)?;
            *list.get_mut(index)? = assignment;
            tx.small_lists.insert(styles, list);
            tx.loose.push(was);
            Some(())
        })())
    }

    pub(super) fn stage_shape(
        &self,
        tx: &mut P21StepTx,
        stmt: usize,
        args: &Arguments,
        cx: &mut Cx,
    ) -> Result<Option<()>, Error> {
        if let Some(rec) = self.record_of(stmt) {
            work(cx, rec.parts().len() as u64, &Ref::default())?;
        }
        count_arguments(args, cx)?;
        Ok((|| {
            let (_, item) = self.occurrence(stmt)?;
            let was = self.reference(tx, item)?;
            let geometry = if args.name == MARKER {
                self.build_marker(tx, args)?
            } else {
                self.build_geometry(tx, args)?
            };
            tx.args.insert(item, format!("#{geometry}"));
            tx.loose.push(was);
            Some(())
        })())
    }

    pub(super) fn stage_remove(
        &self,
        tx: &mut P21StepTx,
        stmt: usize,
        cx: &mut Cx,
    ) -> Result<Option<()>, Error> {
        if let Some(rec) = self.record_of(stmt) {
            for part in rec.parts() {
                work(cx, 1, &Ref::default())?;
                count_values(part.args(), cx)?;
            }
        }
        Ok((|| {
            let rec = self.record_of(stmt)?;
            let id = rec.id()?;
            if self.instances.get(id) != Some(stmt) || self.is_removed(tx, stmt) {
                return None;
            }
            if self.non_container_refs.get(&id).copied().unwrap_or(0) > 0 {
                return None;
            }
            let slots: Vec<Slot> = self
                .current_id_containers
                .get(&id)
                .map(|set| {
                    set.iter()
                        .copied()
                        .filter(|s| !self.is_removed(tx, s.0))
                        .collect()
                })
                .unwrap_or_default();
            for &slot in &slots {
                tx.container_ops
                    .insert((slot, id), StagedContainerOp::Remove);
                if self.container_len_after(tx, slot)? == 0 {
                    self.part(slot.0, "INVISIBILITY")?;
                    tx.removed.insert(slot.0);
                }
            }
            tx.removed.insert(stmt);
            let mut outgoing = Vec::new();
            self.collect_stmt_effective_refs(stmt, &mut outgoing);
            tx.loose.extend(outgoing);
            Some(())
        })())
    }

    pub(super) fn stage_add(
        &self,
        tx: &mut P21StepTx,
        active_widths: &[Option<f64>],
        args: &Arguments,
        cx: &mut Cx,
    ) -> Result<Option<u64>, Error> {
        count_arguments(args, cx)?;
        work(cx, 1 + self.codes.layers.len() as u64, &Ref::default())?;
        Ok((|| {
            let code = |at: usize| args.values.get(at)?.parse::<i64>().ok();
            let (_, sheet_slot) = self.first_sheet?;
            self.container_lists.get(&sheet_slot)?;
            if args.name == MARKER {
                let (layer, color) = (code(0)?, code(1)?);
                let symbol = self.build_marker(tx, args)?;
                let assignment = self.build_symbol_style(tx, color)?;
                let id = self.alloc_instance(
                    tx,
                    format!("ANNOTATION_SYMBOL_OCCURRENCE(' ',(#{assignment}),#{symbol})"),
                )?;
                tx.container_ops
                    .insert((sheet_slot, id), StagedContainerOp::MoveToEnd);
                self.assign_layer(tx, id, layer)?;
                return Some(id);
            }
            let (layer, color, line_type, width) = (code(0)?, code(1)?, code(2)?, code(3)?);
            let geometry = self.build_geometry(tx, args)?;
            let font = self.resolve_line_type(tx, line_type)?;
            let colour = self.resolve_color(tx, color)?;
            let width_s = self.resolve_width(active_widths, width)?;
            let style =
                self.alloc_instance(tx, format!("CURVE_STYLE(' ',#{font},{width_s},#{colour})"))?;
            let assignment =
                self.alloc_instance(tx, format!("PRESENTATION_STYLE_ASSIGNMENT((#{style}))"))?;
            let id = self.alloc_instance(
                tx,
                format!("ANNOTATION_CURVE_OCCURRENCE(' ',(#{assignment}),#{geometry})"),
            )?;
            tx.container_ops
                .insert((sheet_slot, id), StagedContainerOp::MoveToEnd);
            self.assign_layer(tx, id, layer)?;
            Some(id)
        })())
    }

    fn count_codes_work(
        &self,
        stmt: usize,
        codes: &[(usize, i64)],
        active_widths: &[Option<f64>],
        cx: &mut Cx,
    ) -> Result<(), Error> {
        if let Some(rec) = self.record_of(stmt) {
            work(cx, rec.parts().len() as u64, &Ref::default())?;
        }
        if codes.iter().any(|(at, _)| *at == 0) {
            work(cx, 1 + self.codes.layers.len() as u64 * 2, &Ref::default())?;
        }
        if codes.iter().any(|(at, _)| *at > 0) {
            work(cx, 4, &Ref::default())?;
            if let Some((_, code)) = codes.iter().find(|(at, _)| *at == 3) {
                let valid = code
                    .checked_sub(1)
                    .and_then(|at| usize::try_from(at).ok())
                    .and_then(|at| {
                        if at < WIDTHS.len() {
                            WIDTHS.get(at).copied()
                        } else {
                            code.checked_sub(11)
                                .and_then(|at| usize::try_from(at).ok())
                                .and_then(|at| active_widths.get(at).copied().flatten())
                        }
                    });
                if valid.is_some() {
                    for &(_, l_stmt, l_part) in &self.length_measures {
                        work(cx, 1, &Ref::default())?;
                        if let Some(val) = self.value_ref((l_stmt, l_part, 0)) {
                            count_values(core::slice::from_ref(val), cx)?;
                            count_number(val, cx)?;
                            if self.number_at((l_stmt, l_part, 0)) == valid {
                                break;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn collect_slot_effective_refs(&self, slot: Slot, out: &mut Vec<u64>) {
        if let Some(text) = self.args.get(&slot) {
            out.extend(scan(text.as_bytes()));
            return;
        }
        if let Some(list) = self.small_lists.get(&slot) {
            out.extend_from_slice(list);
            return;
        }
        if let Some(clist) = self.container_lists.get(&slot) {
            if let Some(Value::List(items)) = self.value_ref(slot) {
                for item in items {
                    if let Value::Ref(id) = item
                        && !clist.removed_base.contains(id)
                    {
                        out.push(*id);
                    }
                }
            }
            out.extend(clist.tail_by_seq.values().copied());
            return;
        }
        if let Some(val) = self.value_ref(slot) {
            collect(core::slice::from_ref(val), out);
        }
    }

    fn collect_stmt_effective_refs(&self, stmt: usize, out: &mut Vec<u64>) {
        let Some(rec) = self.record_of(stmt) else {
            return;
        };
        for (p_idx, part) in rec.parts().iter().enumerate() {
            for at in 0..part.args().len() {
                self.collect_slot_effective_refs((stmt, p_idx, at), out);
            }
        }
    }

    /// どこからも指されなくなった幾何と見え方の実体を回収し、`tx.ref_deltas` を確定する
    pub(super) fn collect_garbage(&self, tx: &mut P21StepTx, cx: &mut Cx) -> Result<(), Error> {
        work(cx, 2, &Ref::default())?;
        let last_alive_before = self.last_alive_stmt();
        let mut deltas: BTreeMap<u64, i64> = BTreeMap::new();

        for &stmt in &tx.removed {
            if self.removed.contains(&stmt) {
                continue;
            }
            work(cx, 1, &Ref::default())?;
            let mut refs = Vec::new();
            self.collect_stmt_effective_refs(stmt, &mut refs);
            work(cx, refs.len() as u64, &Ref::default())?;
            for id in refs {
                *deltas.entry(id).or_default() -= 1;
            }
        }

        let modified_non_container: BTreeSet<Slot> = tx
            .args
            .keys()
            .chain(tx.small_lists.keys())
            .copied()
            .filter(|(stmt, _, _)| !self.is_removed(tx, *stmt))
            .collect();

        for slot in modified_non_container {
            work(cx, 1, &Ref::default())?;
            let mut old_refs = Vec::new();
            self.collect_slot_effective_refs(slot, &mut old_refs);
            for id in old_refs {
                *deltas.entry(id).or_default() -= 1;
            }
            if let Some(text) = tx.args.get(&slot) {
                let new_refs = scan(text.as_bytes());
                work(cx, 1 + new_refs.len() as u64, &Ref::default())?;
                for id in new_refs {
                    *deltas.entry(id).or_default() += 1;
                }
            } else if let Some(list) = tx.small_lists.get(&slot) {
                work(cx, 1 + list.len() as u64, &Ref::default())?;
                for &id in list {
                    *deltas.entry(id).or_default() += 1;
                }
            }
        }

        for (&(slot, id), &op) in &tx.container_ops {
            if self.is_removed(tx, slot.0) {
                continue;
            }
            let Some(clist) = self.container_lists.get(&slot) else {
                continue;
            };
            let in_base = self
                .base_id_containers
                .get(&id)
                .is_some_and(|s| s.contains(&slot));
            let was_count = if clist.tail_seq_of.contains_key(&id) {
                1_i64
            } else if in_base && !clist.removed_base.contains(&id) {
                i64::from(clist.base_dups.get(&id).copied().unwrap_or(1))
            } else {
                0
            };
            let now_count = match op {
                StagedContainerOp::Remove => 0,
                StagedContainerOp::MoveToEnd => 1,
            };
            if now_count != was_count {
                *deltas.entry(id).or_default() += now_count - was_count;
            }
        }

        for (_, body) in &tx.added {
            work(cx, 1 + body.len() as u64, &Ref::default())?;
            let refs = scan(body);
            work(cx, refs.len() as u64, &Ref::default())?;
            for id in refs {
                *deltas.entry(id).or_default() += 1;
            }
        }

        while let Some(id) = tx.loose.pop() {
            work(cx, 1, &Ref::default())?;
            let Some(stmt) = self.instances.get(id) else {
                continue;
            };
            if self.is_removed(tx, stmt) {
                continue;
            }
            let is_garbage = self
                .with_stmt_bytes(stmt, |rs, rec| {
                    rec.parts().len() == 1
                        && rec
                            .parts()
                            .first()
                            .and_then(|p| slice_in(rs, p.name()))
                            .is_some_and(|n| {
                                GARBAGE.iter().any(|g| n.eq_ignore_ascii_case(g.as_bytes()))
                            })
                })
                .unwrap_or(false);
            let effective = (self.ref_counts.get(id) as i64)
                .saturating_add(deltas.get(&id).copied().unwrap_or(0));
            if !is_garbage || effective > 0 {
                continue;
            }
            tx.removed.insert(stmt);
            let mut children = Vec::new();
            if let Some(rec) = self.record_of(stmt) {
                for part in rec.parts() {
                    work(cx, 1, &Ref::default())?;
                    count_values(part.args(), cx)?;
                    collect(part.args(), &mut children);
                }
            }
            for child in children {
                work(cx, 1, &Ref::default())?;
                *deltas.entry(child).or_default() -= 1;
                tx.loose.push(child);
            }
        }

        tx.removed_last_alive =
            !tx.added.is_empty() && last_alive_before.is_some_and(|s| tx.removed.contains(&s));
        tx.ref_deltas = deltas.into_iter().filter(|(_, d)| *d != 0).collect();
        Ok(())
    }

    fn render_stmt_with_tx(&self, tx: Option<&P21StepTx>, stmt: usize) -> Option<Vec<u8>> {
        self.with_stmt_bytes(stmt, |rs, rec| {
            let nl = self.newline.as_bytes();
            let mut out = format!("#{}=", rec.id()?).into_bytes();
            let complex = rec.form() == Form::Complex;
            if complex {
                out.push(b'(');
                out.extend_from_slice(nl);
            }
            for (p_idx, part) in rec.parts().iter().enumerate() {
                out.extend_from_slice(slice_in(rs, part.name())?);
                out.push(b'(');
                for (at, val) in part.args().iter().enumerate() {
                    if at > 0 {
                        out.push(b',');
                    }
                    let slot = (stmt, p_idx, at);
                    if let Some(text) = tx
                        .and_then(|t| t.args.get(&slot))
                        .or_else(|| self.args.get(&slot))
                    {
                        out.extend_from_slice(text.as_bytes());
                    } else if let Some(list) = tx
                        .and_then(|t| t.small_lists.get(&slot))
                        .or_else(|| self.small_lists.get(&slot))
                    {
                        let items: Vec<String> = list.iter().map(|id| format!("#{id}")).collect();
                        out.extend_from_slice(format!("({})", items.join(",")).as_bytes());
                    } else if let Some(clist) = self.container_lists.get(&slot)
                        && clist.dirty
                        && let Value::List(base_items) = val
                    {
                        clist.write_rendered(base_items, &mut out);
                    } else {
                        write_value_slice(rs, val, &mut out);
                    }
                }
                out.push(b')');
                if complex {
                    out.extend_from_slice(nl);
                }
            }
            if complex {
                out.push(b')');
            }
            out.push(b';');
            Some(out)
        })
        .flatten()
    }

    /// 仮置き中の表記実体（`occ_id`）とその下流参照だけを小さな P21 構造に組んで [`FeatureRecord`] に変換する
    pub(super) fn parse_staged_occurrence(
        &self,
        tx: &P21StepTx,
        occ_id: u64,
        active_widths: &[Option<f64>],
        record_index: usize,
        cx: &mut Cx,
    ) -> Result<Option<FeatureRecord>, Error> {
        let mut layer_code = 0_i64;
        for (idx, &layer_ent) in self.codes.layers.iter().enumerate() {
            if let Some(slot) = self.slot(tx, layer_ent, "PRESENTATION_LAYER_ASSIGNMENT", 2)
                && self.container_contains(tx, slot, occ_id) == Some(true)
            {
                layer_code = (idx + 1) as i64;
                break;
            }
        }

        let mut effective_codes = self.codes.clone();
        for &(cid, ccode) in &tx.added_colors {
            effective_codes.colors.insert(cid, ccode);
        }
        for &(lid, lcode) in &tx.added_line_types {
            effective_codes.line_types.insert(lid, lcode);
        }

        let added_map: BTreeMap<u64, &[u8]> = tx
            .added
            .iter()
            .map(|(id, body)| (*id, body.as_slice()))
            .collect();

        let mut visited = BTreeSet::new();
        let mut queue = vec![occ_id];
        let mut stmts_bytes: Vec<(u64, Vec<u8>)> = Vec::new();

        while let Some(cur_id) = queue.pop() {
            if !visited.insert(cur_id) {
                continue;
            }
            work(cx, 1, &Ref::default())?;
            if let Some(&body) = added_map.get(&cur_id) {
                let mut line = format!("#{cur_id}=").into_bytes();
                line.extend_from_slice(body);
                line.push(b';');
                let child_refs = scan(body);
                for child in child_refs {
                    if !visited.contains(&child) {
                        queue.push(child);
                    }
                }
                stmts_bytes.push((cur_id, line));
            } else if let Some(stmt) = self.instances.get(cur_id)
                && !self.is_removed(tx, stmt)
                && let Some(rendered) = self.render_stmt_with_tx(Some(tx), stmt)
            {
                let child_refs = scan(&rendered);
                for child in child_refs {
                    if child != cur_id && !visited.contains(&child) {
                        queue.push(child);
                    }
                }
                stmts_bytes.push((cur_id, rendered));
            }
        }

        stmts_bytes.sort_by_key(|(id, _)| *id);
        let body_len: usize = stmts_bytes.iter().map(|(_, b)| b.len() + 1).sum();
        let mut mini = Vec::with_capacity(MINI_P21_HEADER.len() + body_len + MINI_P21_FOOTER.len());
        mini.extend_from_slice(MINI_P21_HEADER);
        for (i, (_, line)) in stmts_bytes.into_iter().enumerate() {
            if i > 0 {
                mini.push(b'\n');
            }
            mini.extend_from_slice(&line);
        }
        mini.extend_from_slice(MINI_P21_FOOTER);

        let user_widths: Vec<f64> = active_widths.iter().copied().flatten().collect();
        quietly(cx, |cx| {
            let exchange = zumen_step::read(&mini, &DIALECT, cx)?;
            let mini_file = SxfFile::from_parts(mini, Mode::P21, exchange.records);
            single_occurrence_feature(
                &mini_file,
                occ_id,
                layer_code,
                &effective_codes,
                &user_widths,
                record_index,
                cx,
            )
        })
    }

    pub(super) fn has_surviving_id(&self, tx: &P21StepTx, id: u64) -> bool {
        if tx.added.iter().any(|(aid, _)| *aid == id) {
            return true;
        }
        let mut count = self.alive_id_counts.get(&id).copied().unwrap_or(0);
        for &stmt in &tx.removed {
            if !self.removed.contains(&stmt)
                && let Some(rec) = self.record_of(stmt)
                && rec.id() == Some(id)
            {
                count = count.saturating_sub(1);
            }
        }
        count > 0
    }

    pub(super) fn commit_step(&mut self, tx: P21StepTx, cx: &mut Cx) -> Result<P21Patch, Error> {
        let mut patch = P21Patch::default();

        // 1. 新規追加文を一括パースして `BornStmt` にする
        if !tx.added.is_empty() {
            let total_body: usize = tx.added.iter().map(|(_, b)| b.len() + 24).sum();
            let mut buf =
                Vec::with_capacity(MINI_P21_HEADER.len() + total_body + MINI_P21_FOOTER.len());
            buf.extend_from_slice(MINI_P21_HEADER);
            for (i, (id, body)) in tx.added.iter().enumerate() {
                if i > 0 {
                    buf.push(b'\n');
                }
                buf.extend_from_slice(format!("#{id}=").as_bytes());
                buf.extend_from_slice(body);
                buf.push(b';');
            }
            buf.extend_from_slice(MINI_P21_FOOTER);
            let exchange = quietly(cx, |cx| zumen_step::read(&buf, &DIALECT, cx))?;
            let parsed_buf: Arc<[u8]> = Arc::from(buf);
            let inst_records: Vec<Record> = exchange
                .records
                .into_iter()
                .filter(|r| r.id().is_some())
                .collect();
            if inst_records.len() != tx.added.len() {
                return Err(Error::Rejected);
            }
            for (idx, ((id, body), record)) in tx.added.into_iter().zip(inst_records).enumerate() {
                let born = BornStmt {
                    id,
                    body,
                    parsed_buf: Arc::clone(&parsed_buf),
                    record,
                    extra_leading_nl: idx == 0 && tx.removed_last_alive,
                };
                let stmt_idx = self.base_records.len() + self.born_stmts.len();
                self.instances.insert_first(id, stmt_idx);
                *self.alive_id_counts.entry(id).or_default() += 1;
                self.note_stmt_aux_added(stmt_idx, &born.parsed_buf, &born.record);
                self.born_stmts.push(born.clone());
                patch.added_stmts.push(born);
            }
        }

        // 2. 削除された文（ガベージ回収分を含む）
        for stmt in tx.removed {
            if self.removed.insert(stmt) {
                self.note_stmt_removed_delta(stmt, true);
                patch.removed_stmts.push(stmt);
            }
        }

        // 3. スカラー引数と小リストの書き換え
        for (slot, after) in tx.args {
            let before = self.args.insert(slot, after.clone());
            patch.arg_diffs.push((slot, before, after));
        }
        for (slot, after) in tx.small_lists {
            let before = self.small_lists.insert(slot, after.clone());
            patch.small_list_diffs.push((slot, before, after));
        }

        // 4. コンテナーの要素列の差分適用
        for ((slot, id), op) in tx.container_ops {
            let in_base = self
                .base_id_containers
                .get(&id)
                .is_some_and(|s| s.contains(&slot));
            let push_after_remove = matches!(op, StagedContainerOp::MoveToEnd);
            if let Some(clist) = self.container_lists.get_mut(&slot) {
                let snap = clist.apply_op(id, in_base, push_after_remove);
                if push_after_remove {
                    self.current_id_containers
                        .entry(id)
                        .or_default()
                        .insert(slot);
                } else if let Some(set) = self.current_id_containers.get_mut(&id) {
                    set.remove(&slot);
                    if set.is_empty() {
                        self.current_id_containers.remove(&id);
                    }
                }
                patch.container_ops.push((slot, id, in_base, snap));
            }
        }

        // 5. 参照カウント・既定義色・既定義線種の更新
        for (id, delta) in tx.ref_deltas {
            apply_ref_delta(&mut self.ref_counts, id, delta);
            patch.ref_deltas.push((id, delta));
        }
        for (cid, ccode) in tx.added_colors {
            self.codes.colors.insert(cid, ccode);
            patch.added_colors.push((cid, ccode));
        }
        for (lid, lcode) in tx.added_line_types {
            self.codes.line_types.insert(lid, lcode);
            patch.added_line_types.push((lid, lcode));
        }

        Ok(patch)
    }

    pub(super) fn revert_patch(&mut self, patch: &P21Patch) {
        for (lid, _) in patch.added_line_types.iter().rev() {
            self.codes.line_types.remove(lid);
        }
        for (cid, _) in patch.added_colors.iter().rev() {
            self.codes.colors.remove(cid);
        }
        for &(id, delta) in patch.ref_deltas.iter().rev() {
            apply_ref_delta(&mut self.ref_counts, id, -delta);
        }
        for (slot, id, in_base, snap) in patch.container_ops.iter().rev() {
            if let Some(clist) = self.container_lists.get_mut(slot) {
                clist.revert_op(*id, *in_base, snap);
                let now_present = clist.contains(*id, *in_base);
                if now_present {
                    self.current_id_containers
                        .entry(*id)
                        .or_default()
                        .insert(*slot);
                } else if let Some(set) = self.current_id_containers.get_mut(id) {
                    set.remove(slot);
                    if set.is_empty() {
                        self.current_id_containers.remove(id);
                    }
                }
            }
        }
        for (slot, before, _) in patch.small_list_diffs.iter().rev() {
            match before {
                Some(prev) => {
                    self.small_lists.insert(*slot, prev.clone());
                }
                None => {
                    self.small_lists.remove(slot);
                }
            }
        }
        for (slot, before, _) in patch.arg_diffs.iter().rev() {
            match before {
                Some(prev) => {
                    self.args.insert(*slot, prev.clone());
                }
                None => {
                    self.args.remove(slot);
                }
            }
        }
        for &stmt in patch.removed_stmts.iter().rev() {
            if self.removed.remove(&stmt) {
                self.note_stmt_removed_delta(stmt, false);
            }
        }
        for _ in 0..patch.added_stmts.len() {
            if let Some(born) = self.born_stmts.pop() {
                let stmt_idx = self.base_records.len() + self.born_stmts.len();
                self.note_stmt_aux_unadded(stmt_idx, &born.parsed_buf, &born.record);
                self.instances.remove_exact(born.id, stmt_idx);
                dec_u32(&mut self.alive_id_counts, born.id);
            }
        }
    }

    pub(super) fn reapply_patch(&mut self, patch: &P21Patch) {
        for born in &patch.added_stmts {
            let stmt_idx = self.base_records.len() + self.born_stmts.len();
            self.instances.insert_first(born.id, stmt_idx);
            *self.alive_id_counts.entry(born.id).or_default() += 1;
            self.note_stmt_aux_added(stmt_idx, &born.parsed_buf, &born.record);
            self.born_stmts.push(born.clone());
        }
        for &stmt in &patch.removed_stmts {
            if self.removed.insert(stmt) {
                self.note_stmt_removed_delta(stmt, true);
            }
        }
        for (slot, _, after) in &patch.arg_diffs {
            self.args.insert(*slot, after.clone());
        }
        for (slot, _, after) in &patch.small_list_diffs {
            self.small_lists.insert(*slot, after.clone());
        }
        for (slot, id, in_base, snap) in &patch.container_ops {
            if let Some(clist) = self.container_lists.get_mut(slot) {
                clist.reapply_op(*id, *in_base, snap);
                if clist.contains(*id, *in_base) {
                    self.current_id_containers
                        .entry(*id)
                        .or_default()
                        .insert(*slot);
                } else if let Some(set) = self.current_id_containers.get_mut(id) {
                    set.remove(slot);
                    if set.is_empty() {
                        self.current_id_containers.remove(id);
                    }
                }
            }
        }
        for &(id, delta) in &patch.ref_deltas {
            apply_ref_delta(&mut self.ref_counts, id, delta);
        }
        for &(cid, ccode) in &patch.added_colors {
            self.codes.colors.insert(cid, ccode);
        }
        for &(lid, lcode) in &patch.added_line_types {
            self.codes.line_types.insert(lid, lcode);
        }
    }

    fn note_stmt_aux_added(&mut self, stmt_idx: usize, bytes: &[u8], rec: &Record) {
        for part in rec.parts() {
            for val in part.args() {
                for_each_ref(core::slice::from_ref(val), &mut |rid| {
                    *self.non_container_refs.entry(rid).or_default() += 1;
                });
            }
        }
        let rs = RecordSlice { base: 0, bytes };
        if !self.base_widths.is_empty()
            && let Some(id) = rec.id()
            && self.instances.get(id) == Some(stmt_idx)
            && let Some(mm) = self.stmt_curve_style_mm(rs, rec)
            && !WIDTHS.iter().any(|w| (w - mm).abs() < 1e-6)
            && let Some(pos) = self.base_widths.iter().position(|w| *w == Some(mm))
            && let Some(slot) = self.user_width_counts.get_mut(pos)
        {
            *slot = slot.saturating_add(1);
        }
    }

    fn note_stmt_aux_unadded(&mut self, stmt_idx: usize, bytes: &[u8], rec: &Record) {
        for part in rec.parts() {
            for val in part.args() {
                for_each_ref(core::slice::from_ref(val), &mut |rid| {
                    dec_u32(&mut self.non_container_refs, rid);
                });
            }
        }
        let rs = RecordSlice { base: 0, bytes };
        if !self.base_widths.is_empty()
            && let Some(id) = rec.id()
            && self.instances.get(id) == Some(stmt_idx)
            && let Some(mm) = self.stmt_curve_style_mm(rs, rec)
            && !WIDTHS.iter().any(|w| (w - mm).abs() < 1e-6)
            && let Some(pos) = self.base_widths.iter().position(|w| *w == Some(mm))
            && let Some(slot) = self.user_width_counts.get_mut(pos)
        {
            *slot = slot.saturating_sub(1);
        }
    }

    fn note_stmt_removed_delta(&mut self, stmt: usize, removing: bool) {
        let Some((id_opt, container_slot, mm_opt, rec_clone)) =
            self.with_stmt_bytes(stmt, |rs, rec| {
                let c_slot = CONTAINERS.iter().find_map(|(cname, cat)| {
                    let p_idx = rec.parts().iter().position(|p| {
                        slice_in(rs, p.name())
                            .is_some_and(|n| n.eq_ignore_ascii_case(cname.as_bytes()))
                    })?;
                    Some((stmt, p_idx, *cat))
                });
                let mm = if !self.base_widths.is_empty()
                    && let Some(id) = rec.id()
                    && self.instances.get(id) == Some(stmt)
                {
                    self.stmt_curve_style_mm(rs, rec)
                } else {
                    None
                };
                (rec.id(), c_slot, mm, rec.clone())
            })
        else {
            return;
        };
        if let Some(id) = id_opt {
            if removing {
                dec_u32(&mut self.alive_id_counts, id);
            } else {
                *self.alive_id_counts.entry(id).or_default() += 1;
            }
        }

        for (p_idx, part) in rec_clone.parts().iter().enumerate() {
            for (at, val) in part.args().iter().enumerate() {
                let slot = (stmt, p_idx, at);
                if Some(slot) == container_slot && self.container_lists.contains_key(&slot) {
                    continue;
                }
                for_each_ref(core::slice::from_ref(val), &mut |rid| {
                    if removing {
                        dec_u32(&mut self.non_container_refs, rid);
                    } else {
                        *self.non_container_refs.entry(rid).or_default() += 1;
                    }
                });
            }
        }

        if let Some(mm) = mm_opt
            && !WIDTHS.iter().any(|w| (w - mm).abs() < 1e-6)
            && let Some(pos) = self.base_widths.iter().position(|w| *w == Some(mm))
            && let Some(slot) = self.user_width_counts.get_mut(pos)
        {
            if removing {
                *slot = slot.saturating_sub(1);
            } else {
                *slot = slot.saturating_add(1);
            }
        }
    }

    fn stmt_curve_style_mm(&self, rs: RecordSlice<'_>, rec: &Record) -> Option<f64> {
        let part = rec.parts().iter().find(|p| {
            slice_in(rs, p.name()).is_some_and(|n| n.eq_ignore_ascii_case(b"CURVE_STYLE"))
        })?;
        let arg = part.args().get(2)?;
        match arg {
            Value::Ref(mid) => {
                let m_stmt = self.instances.get(*mid)?;
                let m_part = self.part(m_stmt, "LENGTH_MEASURE_WITH_UNIT")?;
                self.number_at((m_stmt, m_part, 0))
            }
            _ => number_in_slice(rs, arg),
        }
    }

    pub(super) fn diagnostic_current(
        &self,
        event: &Diagnostic,
        cx: &mut Cx<'_>,
    ) -> Result<bool, Error> {
        let Some(record) = event
            .places()
            .first()
            .and_then(Ref::record)
            .and_then(|record| usize::try_from(record).ok())
        else {
            return Ok(true);
        };
        let Some(zumen_base::Value::UInt(id)) = event.param("id") else {
            return Ok(true);
        };
        if event.code() == codes::SXF_P21_REFERENCE_MISSING {
            if self
                .instances
                .get(*id)
                .is_some_and(|record| !self.removed.contains(&record))
            {
                return Ok(false);
            }
            let Some(statement) = self.record_of(record) else {
                return Ok(false);
            };
            for (part_index, part) in statement.parts().iter().enumerate() {
                work(cx, 1, &Ref::at_record(record as u64))?;
                for (argument, value) in part.args().iter().enumerate() {
                    let slot = (record, part_index, argument);
                    work(cx, 1, &Ref::at_record(record as u64))?;
                    let present = if let Some(text) = self.args.get(&slot) {
                        work(cx, text.len() as u64, &Ref::default())?;
                        cx.check(
                            zumen_base::Kind::DecodedBytes,
                            (text.len() as u64).saturating_mul(16),
                        )
                        .map_err(|stop| {
                            cx.report(stop.to_diagnostic());
                            Error::Stopped(stop)
                        })?;
                        scan(text.as_bytes()).contains(id)
                    } else if let Some(list) = self.small_lists.get(&slot) {
                        work(cx, list.len() as u64, &Ref::default())?;
                        list.contains(id)
                    } else {
                        count_values(core::slice::from_ref(value), cx)?;
                        let mut in_base = false;
                        for_each_ref(core::slice::from_ref(value), &mut |reference| {
                            in_base |= reference == *id
                        });
                        self.container_lists
                            .get(&slot)
                            .map_or(in_base, |list| list.contains(*id, in_base))
                    };
                    if present {
                        return Ok(true);
                    }
                }
            }
            return Ok(false);
        }
        if event.code() == codes::SXF_P21_LAYER_MEMBER_DROPPED {
            let Some(containers) = self.current_id_containers.get(id) else {
                return Ok(false);
            };
            let mut assigned = false;
            for layer in &self.codes.layers {
                work(cx, 1, &Ref::at_record(record as u64))?;
                let Some(statement) = self.instances.get(*layer) else {
                    continue;
                };
                work(cx, containers.len() as u64, &Ref::default())?;
                if containers.iter().any(|slot| slot.0 == statement) {
                    if statement == record {
                        return Ok(assigned);
                    }
                    assigned = true;
                }
            }
            return Ok(false);
        }
        Ok(true)
    }

    pub(super) fn source_map(&self, cx: &mut Cx<'_>) -> Result<SourceMap, Error> {
        work(
            cx,
            self.removed.len().saturating_add(self.born_stmts.len()) as u64,
            &Ref::default(),
        )?;
        let base_len = self.base_records.len();
        SourceMap::build(
            self.removed.range(..base_len).map(|record| *record as u64),
            self.last_base_instance
                .as_ref()
                .map(|(record, _, _)| (*record as u64).saturating_add(1)),
            self.born_stmts
                .iter()
                .enumerate()
                .filter(|(index, _)| !self.removed.contains(&base_len.saturating_add(*index)))
                .map(|(_, statement)| statement.id),
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

        let base_len = self.base_records.len();
        let mut patches: Vec<(Range<usize>, Vec<u8>)> = Vec::new();

        for &stmt in &self.removed {
            if stmt < base_len
                && let Some(entry) = self.base_records.get(stmt)
            {
                work(cx, 1, &Ref::default())?;
                let span = entry.span();
                patches.push((
                    span.start..line_end_in_source(&self.src, span.end),
                    Vec::new(),
                ));
            }
        }

        let mut rewritten_base: BTreeSet<usize> = self
            .args
            .keys()
            .chain(self.small_lists.keys())
            .map(|(s, _, _)| *s)
            .filter(|&s| s < base_len && !self.removed.contains(&s))
            .collect();
        for (&(s, _, _), clist) in &self.container_lists {
            if clist.dirty && s < base_len && !self.removed.contains(&s) {
                rewritten_base.insert(s);
            }
        }

        for stmt in rewritten_base {
            let Some(entry) = self.base_records.get(stmt) else {
                return Err(Error::Rejected);
            };
            let span = entry.span();
            work(
                cx,
                1 + span.end.saturating_sub(span.start) as u64 / 64,
                &Ref::default(),
            )?;
            let Some(rendered) = self.render_stmt_with_tx(None, stmt) else {
                return Err(Error::Rejected);
            };
            patches.push((span, rendered));
        }

        let surviving_born: Vec<(usize, &BornStmt)> = self
            .born_stmts
            .iter()
            .enumerate()
            .map(|(i, b)| (base_len + i, b))
            .filter(|(stmt, _)| !self.removed.contains(stmt))
            .collect();

        if !surviving_born.is_empty() {
            let Some((last_idx, last_span, last_le)) = self.last_base_instance.clone() else {
                return Err(Error::Rejected);
            };
            let last_removed = self.removed.contains(&last_idx);
            let mut last_nl = Vec::with_capacity(last_le.saturating_sub(last_span.end));
            self.src.append_range(&mut last_nl, last_span.end..last_le);
            let nl = self.newline.as_bytes();
            let mut block = Vec::new();

            for (pos, (stmt, born)) in surviving_born.into_iter().enumerate() {
                work(cx, 1 + born.body.len() as u64 / 64, &Ref::default())?;
                if pos == 0 {
                    if !last_removed && last_nl.is_empty() {
                        block.extend_from_slice(nl);
                    }
                    if born.extra_leading_nl {
                        block.extend_from_slice(nl);
                    }
                } else {
                    block.extend_from_slice(nl);
                    if born.extra_leading_nl {
                        block.extend_from_slice(nl);
                    }
                }
                let has_override = self
                    .args
                    .range((stmt, 0, 0)..=(stmt, usize::MAX, usize::MAX))
                    .next()
                    .is_some()
                    || self
                        .small_lists
                        .range((stmt, 0, 0)..=(stmt, usize::MAX, usize::MAX))
                        .next()
                        .is_some();
                if has_override {
                    let Some(rendered) = self.render_stmt_with_tx(None, stmt) else {
                        return Err(Error::Rejected);
                    };
                    block.extend_from_slice(&rendered);
                } else {
                    block.extend_from_slice(format!("#{}=", born.id).as_bytes());
                    block.extend_from_slice(&born.body);
                    block.push(b';');
                }
            }
            block.extend_from_slice(&last_nl);
            patches.push((last_le..last_le, block));
        }

        patched_source(patches, &self.src, cx)
    }
}

fn slice_in<'a>(rs: RecordSlice<'a>, range: Range<usize>) -> Option<&'a [u8]> {
    let s = range.start.checked_sub(rs.base)?;
    let e = range.end.checked_sub(rs.base)?;
    rs.bytes.get(s..e)
}

fn number_in_slice(rs: RecordSlice<'_>, value: &Value) -> Option<f64> {
    match value {
        Value::Number(raw) => core::str::from_utf8(slice_in(rs, raw.clone())?)
            .ok()?
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite()),
        Value::Typed(part) => number_in_slice(rs, part.args().first()?),
        _ => None,
    }
}

fn curve_style_mm(file: &SxfFile, instances: &InstanceIndex, arg: Option<&Value>) -> Option<f64> {
    let val = arg?;
    match val {
        Value::Ref(mid) => {
            let m_rec_idx = instances.get(*mid)?;
            let m_entry = file.records().get(m_rec_idx)?;
            let m_part = m_entry.parts().iter().find(|p| {
                file.bytes(p.name())
                    .eq_ignore_ascii_case(b"LENGTH_MEASURE_WITH_UNIT")
            })?;
            number_from_file(file, m_part.args().first()?)
        }
        _ => number_from_file(file, val),
    }
}

fn number_from_file(file: &SxfFile, value: &Value) -> Option<f64> {
    match value {
        Value::Number(raw) => core::str::from_utf8(file.bytes(raw.clone()))
            .ok()?
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite()),
        Value::Typed(part) => number_from_file(file, part.args().first()?),
        _ => None,
    }
}

fn apply_ref_delta(counts: &mut RefCounts, id: u64, delta: i64) {
    if delta > 0 {
        for _ in 0..delta {
            counts.add(id);
        }
    } else if delta < 0 {
        for _ in 0..delta.unsigned_abs() {
            counts.sub(id);
        }
    }
}

fn dec_u32(map: &mut BTreeMap<u64, u32>, key: u64) {
    if let Some(slot) = map.get_mut(&key) {
        if *slot <= 1 {
            map.remove(&key);
        } else {
            *slot -= 1;
        }
    }
}

fn write_value_slice(rs: RecordSlice<'_>, value: &Value, out: &mut Vec<u8>) {
    let bytes = |r: Range<usize>| slice_in(rs, r).unwrap_or_default();
    let mut delimited = |mark: u8, raw: &[u8]| {
        out.push(mark);
        out.extend_from_slice(raw);
        out.push(mark);
    };
    match value {
        Value::Null => out.push(b'$'),
        Value::Derived => out.push(b'*'),
        Value::Ref(id) => out.extend_from_slice(format!("#{id}").as_bytes()),
        Value::Enum(range) => delimited(b'.', bytes(range.clone())),
        Value::Text { raw, .. } => delimited(b'\'', bytes(raw.clone())),
        Value::Number(range) | Value::Token(range) => out.extend_from_slice(bytes(range.clone())),
        Value::Binary(range) => delimited(b'"', bytes(range.clone())),
        Value::Typed(part) => {
            out.extend_from_slice(bytes(part.name()));
            write_list_slice(rs, part.args(), out);
        }
        Value::List(items) => write_list_slice(rs, items, out),
    }
}

fn write_list_slice(rs: RecordSlice<'_>, items: &[Value], out: &mut Vec<u8>) {
    out.push(b'(');
    for (at, item) in items.iter().enumerate() {
        if at > 0 {
            out.push(b',');
        }
        write_value_slice(rs, item, out);
    }
    out.push(b')');
}
