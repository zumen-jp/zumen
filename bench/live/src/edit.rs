//! 差分の変更（移動・スタイル変更・追加・削除）と、取り消し・やり直しの履歴。
//!
//! 変更の前後で変わらないタグは元のバイト範囲のまま残し、変わったタグだけを `diff_values`（決定 0024）と
//! 同じ規則で `overlay` に書く。トランザクションは途中で 1 つでも断られたら何も反映しない。

use std::collections::{BTreeMap, BTreeSet};

use zumen_acad::dxf::{
    TagEdit, advance_handseed, diff_values, entity_extents, merge_extents, widen_extents,
};
use zumen_acad::{Data, Value, single};
use zumen_drawing::Affine;

use crate::born::{Add, ColorSpec, Restyle, encode_born_records};
use crate::lift::{self, Geom, Kind, Lift};
use crate::paged::RecordSlice;
use crate::store::{Document, OTag, OverlayRecord, Row, bounds};

/// 試作が以前は範囲の外としていた分類（今はすべて差分で扱うため返さない）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outside {
    /// 表のレコード 1 つで決まらない名前
    Names,
    /// ATTRIB の付き方が単純でない図面での追加・削除
    Attachment,
    /// ATTDEF を持つ記号の追加
    DefinedAttributes,
    /// ASCII・UTF-8 以外の文字コード
    Charset,
    /// ENTITIES セクションが 1 つでない図面での追加
    Sections,
}

/// 変更を断った理由
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decline {
    /// 指定したハンドルの図形が見つからない
    NotFound,
    /// 同じトランザクションの中で同じ図形を 2 度指した
    Duplicate,
    /// 今の実装も断る未対応・無効な変更
    Unsupported,
    /// 読めない文字コード（現在は 0 件）
    Unreadable,
    /// 試作の範囲の外（現在は 0 件）
    Outside(Outside),
}

/// 1 つの編集操作
#[derive(Clone, Debug)]
pub enum Edit {
    /// ハンドルで指す図形をアフィン変換で動かす
    Transform {
        /// 対象のハンドル
        handle: u64,
        /// 2×3 行列 `[a, b, tx, c, d, ty]`
        matrix: [f64; 6],
    },
    /// ハンドルで指す図形の共通属性を変える
    Restyle {
        /// 対象のハンドル
        handle: u64,
        /// 変える属性
        style: Restyle,
    },
    /// モデル空間に図形を 1 つ足す
    Add(Box<Add>),
    /// ハンドルで指す図形を除く
    Remove {
        /// 対象のハンドル
        handle: u64,
    },
}

#[derive(Clone, Debug)]
enum AppliedTagOp {
    Set {
        at: usize,
        before: Option<Vec<u8>>,
        after: Vec<u8>,
    },
    Remove {
        at: usize,
        removed: OTag,
    },
    Insert {
        at: usize,
        inserted: OTag,
    },
}

impl AppliedTagOp {
    fn bytes(&self) -> usize {
        size_of::<Self>()
            + match self {
                Self::Set { before, after, .. } => {
                    before.as_ref().map_or(0, Vec::capacity) + after.capacity()
                }
                Self::Remove { removed, .. } => removed.heap_bytes(),
                Self::Insert { inserted, .. } => inserted.heap_bytes(),
            }
    }
}

#[derive(Clone, Debug)]
enum RecordUndo {
    Patched {
        record: u32,
        created: bool,
        ops: Vec<AppliedTagOp>,
    },
    Born {
        record: u32,
        tags: Vec<OTag>,
    },
}

impl RecordUndo {
    fn bytes(&self) -> usize {
        size_of::<Self>()
            + match self {
                Self::Patched { ops, .. } => {
                    ops.capacity() * size_of::<AppliedTagOp>()
                        + ops.iter().map(AppliedTagOp::bytes).sum::<usize>()
                }
                Self::Born { tags, .. } => {
                    tags.capacity() * size_of::<OTag>()
                        + tags.iter().map(OTag::heap_bytes).sum::<usize>()
                }
            }
    }
}

#[derive(Clone, Debug)]
struct AddedEntry {
    records: Vec<u32>,
}

#[derive(Clone, Debug)]
struct RemovedEntry {
    row: u32,
    records: Vec<u32>,
}

#[derive(Clone, Debug)]
struct Entry {
    overlays: Vec<RecordUndo>,
    touched_rows: Vec<u32>,
    added: Vec<AddedEntry>,
    removed: Vec<RemovedEntry>,
}

impl Entry {
    fn bytes(&self) -> usize {
        size_of::<Self>()
            + self.overlays.iter().map(RecordUndo::bytes).sum::<usize>()
            + self.touched_rows.capacity() * size_of::<u32>()
            + self
                .added
                .iter()
                .map(|a| size_of::<AddedEntry>() + a.records.capacity() * size_of::<u32>())
                .sum::<usize>()
            + self
                .removed
                .iter()
                .map(|r| size_of::<RemovedEntry>() + r.records.capacity() * size_of::<u32>())
                .sum::<usize>()
    }
}

/// 取り消し・やり直しの履歴
#[derive(Default)]
pub struct History {
    undo_stack: Vec<Entry>,
    redo_stack: Vec<Entry>,
}

impl History {
    /// 空の履歴を作る
    pub fn new() -> Self {
        Self::default()
    }

    /// 積まれている取り消し段数
    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    /// 履歴が保持するヒープバイト数
    pub fn bytes(&self) -> usize {
        self.undo_stack.iter().map(Entry::bytes).sum::<usize>()
            + self.redo_stack.iter().map(Entry::bytes).sum::<usize>()
    }
}

fn apply_tag_edits_in_place(
    tags: &mut Vec<OTag>,
    edits: Vec<TagEdit>,
) -> Option<Vec<AppliedTagOp>> {
    let mut ops = Vec::with_capacity(edits.len());
    for edit in edits {
        match edit {
            TagEdit::Set { at, value } => {
                let slot = tags.get_mut(at)?;
                let before = slot.edited.replace(value.clone());
                ops.push(AppliedTagOp::Set {
                    at,
                    before,
                    after: value,
                });
            }
            TagEdit::Remove { at } => {
                if at >= tags.len() {
                    return None;
                }
                let removed = tags.remove(at);
                ops.push(AppliedTagOp::Remove { at, removed });
            }
            TagEdit::Insert { at, code, value } => {
                if at > tags.len() {
                    return None;
                }
                let inserted = OTag::added(code, value);
                tags.insert(at, inserted.clone());
                ops.push(AppliedTagOp::Insert { at, inserted });
            }
        }
    }
    Some(ops)
}

type EncodedRecords = Vec<(Vec<u8>, Vec<OTag>, Vec<Value>)>;

struct StagedState {
    seen_handles: BTreeSet<u64>,
    overlay_ops: BTreeMap<u32, Vec<AppliedTagOp>>,
    overlay_now: BTreeMap<u32, (OverlayRecord, bool)>,
    header_edits: Vec<TagEdit>,
    touched_rows: Vec<u32>,
    added_plans: Vec<(u64, EncodedRecords)>,
    removed: Vec<RemovedEntry>,
    touched_extents: Option<[[f64; 3]; 2]>,
    next_handle_cursor: Option<u64>,
    live_max_after: Option<u64>,
}

impl StagedState {
    fn ensure_record(&mut self, doc: &Document, record: u32) -> &mut (OverlayRecord, bool) {
        self.overlay_now.entry(record).or_insert_with(|| {
            let ov = if let Some(existing) = doc.overlay.get(&record) {
                existing.clone()
            } else if record >= doc.counts.records {
                OverlayRecord::born(Vec::new())
            } else {
                doc.src
                    .with_range(doc.record_range(record), OverlayRecord::from_slice)
            };
            (ov, false)
        })
    }

    fn get_kind_and_values(
        &mut self,
        doc: &Document,
        record: u32,
    ) -> Option<(Vec<u8>, Vec<Value>)> {
        let (ov, _) = self.ensure_record(doc, record);
        let first = ov.tags.first()?;
        if first.code != 0 {
            return None;
        }
        if record >= doc.counts.records || !ov.raw.is_empty() {
            let rs = RecordSlice {
                base: ov.base as usize,
                bytes: &ov.raw,
            };
            let kind = first.raw_in(&rs).trim_ascii().to_vec();
            let values = ov
                .tags
                .get(1..)
                .unwrap_or_default()
                .iter()
                .map(|t| t.to_value_for_in(&kind, &rs))
                .collect();
            return Some((kind, values));
        }
        let tags = ov.tags.clone();
        doc.src.with_range(doc.record_range(record), |rs| {
            let first = tags.first()?;
            let kind = first.raw_in(&rs).trim_ascii().to_vec();
            let values = tags
                .get(1..)
                .unwrap_or_default()
                .iter()
                .map(|t| t.to_value_for_in(&kind, &rs))
                .collect();
            Some((kind, values))
        })
    }

    fn apply_diff(
        &mut self,
        doc: &Document,
        record: u32,
        old_values: &[Value],
        new_values: &[Value],
    ) -> Option<()> {
        let edits = diff_values(1, old_values, new_values).ok()?;
        if edits.is_empty() {
            return Some(());
        }
        let (ov, modified) = self.ensure_record(doc, record);
        let ops = apply_tag_edits_in_place(&mut ov.tags, edits)?;
        *modified = true;
        self.overlay_ops.entry(record).or_default().extend(ops);
        Some(())
    }

    fn is_gone(&self, doc: &Document, record: u32) -> bool {
        doc.gone.contains(&record) || self.removed.iter().any(|r| r.records.contains(&record))
    }
}

impl Document {
    fn resolve_target_row(&self, staged: &mut StagedState, handle: u64) -> Result<u32, Decline> {
        // `changes.rs` の `seen()` と同じく、同じ ID を 2 度指したら対象の有無より先に重複として断る
        if !staged.seen_handles.insert(handle) {
            return Err(Decline::Duplicate);
        }
        let idx = self
            .by_handle
            .get(handle, &self.rows)
            .ok_or(Decline::NotFound)?;
        let r = self.rows.get(idx as usize).ok_or(Decline::NotFound)?;
        if !r.alive || staged.is_gone(self, r.record) {
            return Err(Decline::NotFound);
        }
        Ok(idx)
    }

    fn header_var_values(&self, (tag_start, len): (usize, usize)) -> Vec<Value> {
        if let Some(h_rec) = self.header.record
            && let Some(ov) = self.overlay.get(&h_rec)
        {
            let sub = ov.tags.get(tag_start..tag_start + len).unwrap_or_default();
            let mut min_b = usize::MAX;
            let mut max_b = 0usize;
            for t in sub {
                if t.edited.is_none() {
                    min_b = min_b.min(t.span.value as usize);
                    max_b = max_b.max(t.span.value_end as usize);
                }
            }
            if min_b == usize::MAX {
                let empty = RecordSlice {
                    base: 0,
                    bytes: &[],
                };
                return sub.iter().map(|t| t.to_value_in(&empty)).collect();
            }
            return self.src.with_range(min_b..max_b, |rs| {
                sub.iter().map(|t| t.to_value_in(&rs)).collect()
            });
        }
        let sub = self
            .header
            .spans
            .get(tag_start..tag_start + len)
            .unwrap_or_default();
        let (Some(first), Some(last)) = (sub.first(), sub.last()) else {
            return Vec::new();
        };
        self.src
            .with_range(first.value as usize..last.value_end as usize, |rs| {
                sub.iter()
                    .map(|s| crate::value::value(s.code, rs.slice(s.value, s.value_end)))
                    .collect()
            })
    }

    fn current_handseed(&self) -> Option<u64> {
        let range = self.header.handseed?;
        for v in self.header_var_values(range) {
            if v.code == 5 {
                return match v.data {
                    Data::Handle(h) => Some(h),
                    _ => None,
                };
            }
        }
        None
    }

    fn stage_transform(
        &self,
        staged: &mut StagedState,
        row_idx: u32,
        matrix: [f64; 6],
    ) -> Result<(), Decline> {
        if !matrix.iter().all(|v| v.is_finite()) {
            return Err(Decline::Unsupported);
        }
        let transform = Affine { m: matrix };
        let row = *self.rows.get(row_idx as usize).ok_or(Decline::NotFound)?;
        let was = self.element(row_idx).ok_or(Decline::Unsupported)?;
        let (kind, old_vals) = staged
            .get_kind_and_values(self, row.record)
            .ok_or(Decline::Unsupported)?;

        let mut att_records: Vec<u32> = Vec::new();
        let mut att_old_vals: Vec<Vec<Value>> = Vec::new();
        if row.kind == Kind::Insert {
            let ins = self
                .shapes
                .inserts
                .get(row.shape as usize)
                .ok_or(Decline::Unsupported)?;
            if ins.attached != ins.attrib_count {
                return Err(Decline::Unsupported);
            }
            for attr in self.shapes.pools.attribs_of(ins) {
                if attr.is_formatted() {
                    return Err(Decline::Unsupported);
                }
                att_records.push(attr.record);
                let (_, av) = staged
                    .get_kind_and_values(self, attr.record)
                    .ok_or(Decline::Unsupported)?;
                att_old_vals.push(av);
            }
        }

        let att_refs: Vec<&[Value]> = att_old_vals.iter().map(Vec::as_slice).collect();
        let (new_vals, new_att_vals) =
            single::move_model(&was, &kind, &old_vals, &att_refs, &transform, self.charset)
                .ok_or(Decline::Unsupported)?;

        if new_vals != old_vals {
            staged
                .apply_diff(self, row.record, &old_vals, &new_vals)
                .ok_or(Decline::Unsupported)?;
            staged.touched_extents = merge_extents(
                staged.touched_extents,
                entity_extents(&kind, true, &new_vals),
            );
            if !staged.touched_rows.contains(&row_idx) {
                staged.touched_rows.push(row_idx);
            }
        }
        for (&att_rec, (old_v, new_v)) in att_records
            .iter()
            .zip(att_old_vals.iter().zip(&new_att_vals))
        {
            if new_v != old_v {
                staged
                    .apply_diff(self, att_rec, old_v, new_v)
                    .ok_or(Decline::Unsupported)?;
                if !staged.touched_rows.contains(&row_idx) {
                    staged.touched_rows.push(row_idx);
                }
            }
        }
        Ok(())
    }

    fn stage_restyle(
        &self,
        staged: &mut StagedState,
        row_idx: u32,
        restyle: &Restyle,
    ) -> Result<(), Decline> {
        if let Some(scale) = restyle.line_type_scale
            && (!scale.is_finite() || scale <= 0.0)
        {
            return Err(Decline::Unsupported);
        }
        let line_weight = match restyle.line_weight {
            None => None,
            Some(w) => Some(w.to_inherit().ok_or(Decline::Unsupported)?),
        };
        let color = restyle.color.map(ColorSpec::to_inherit);

        let row = *self.rows.get(row_idx as usize).ok_or(Decline::NotFound)?;
        let current_layer = self
            .layers
            .get(row.layer as usize)
            .map(|l| l.name.as_str())
            .ok_or(Decline::Unsupported)?;

        let resolved_new_layer: Option<&str> = match &restyle.layer {
            None => None,
            Some(name) => {
                let lid = self.lookup_live_layer(name).ok_or(Decline::Unsupported)?;
                Some(
                    self.layers
                        .get(lid.0 as usize)
                        .ok_or(Decline::Unsupported)?
                        .name
                        .as_str(),
                )
            }
        };
        let resolved_new_lt = match &restyle.line_type {
            None => None,
            Some(spec) => Some(spec.as_restyle(self).ok_or(Decline::Unsupported)?),
        };

        let (kind, old_vals) = staged
            .get_kind_and_values(self, row.record)
            .ok_or(Decline::Unsupported)?;

        let current_line_type: Option<&str> = row
            .extra_style
            .checked_sub(1)
            .and_then(|idx| self.shapes.pools.extra_styles.get(idx as usize))
            .filter(|es| matches!(es.line_type, lift::LineTypeRef::Named(_)))
            .and_then(|es| self.line_types.get(es.line_type_id as usize))
            .map(|lt| lt.name.as_str());

        let mut tables = self.tables_cache.clone();
        tables.markers = self.live_markers();
        let new_vals = single::restyle_model(
            &tables,
            &kind,
            &old_vals,
            current_layer,
            current_line_type,
            resolved_new_layer,
            resolved_new_lt,
            color,
            restyle.line_type_scale,
            line_weight,
        )
        .ok_or(Decline::Unsupported)?;

        if new_vals != old_vals {
            staged
                .apply_diff(self, row.record, &old_vals, &new_vals)
                .ok_or(Decline::Unsupported)?;
            staged.touched_extents = merge_extents(
                staged.touched_extents,
                entity_extents(&kind, true, &new_vals),
            );
            if !staged.touched_rows.contains(&row_idx) {
                staged.touched_rows.push(row_idx);
            }
        }
        Ok(())
    }

    fn stage_remove(&self, staged: &mut StagedState, row_idx: u32) -> Result<(), Decline> {
        let row = *self.rows.get(row_idx as usize).ok_or(Decline::NotFound)?;
        let (kind, vals) = self
            .record_kind_and_values(row.record)
            .ok_or(Decline::Unsupported)?;

        if single::is_followed(&kind, &vals) {
            return Err(Decline::Unsupported);
        }
        if !vals.iter().any(|v| matches!(v.code, 102 | 360)) {
            staged.removed.push(RemovedEntry {
                row: row_idx,
                records: vec![row.record],
            });
            return Ok(());
        }

        let root_handle = row
            .handle()
            .filter(|&h| h != 0)
            .ok_or(Decline::Unsupported)?;
        let mut next = single::owned_children(&kind, &vals, true).ok_or(Decline::Unsupported)?;
        let mut parents: Vec<u64> = vec![root_handle; next.len()];
        let mut seen: BTreeSet<u64> = BTreeSet::from([root_handle]);
        let mut owned_map: BTreeMap<u32, u64> = BTreeMap::new();

        while let (Some((child, dictionary)), Some(parent)) = (next.pop(), parents.pop()) {
            if !seen.insert(child) {
                return Err(Decline::Unsupported);
            }
            let Some(&obj_rec) = self
                .objects_by_handle
                .get(&child)
                .filter(|&&r| !self.gone.contains(&r))
            else {
                return Err(Decline::Unsupported);
            };
            let (obj_kind, obj_vals) = self
                .record_kind_and_values(obj_rec)
                .ok_or(Decline::Unsupported)?;
            if zumen_acad::db::handle_of_values(&obj_kind, &obj_vals) != Some(child)
                || zumen_acad::db::owner_of_values(&obj_vals) != Some(parent)
                || (dictionary && obj_kind != b"DICTIONARY")
            {
                return Err(Decline::Unsupported);
            }
            let more =
                single::owned_children(&obj_kind, &obj_vals, false).ok_or(Decline::Unsupported)?;
            parents.extend(core::iter::repeat_n(child, more.len()));
            next.extend(more);
            owned_map.insert(obj_rec, child);
        }

        let owned_handles: BTreeSet<u64> = owned_map.values().copied().collect();
        let skip: BTreeSet<u32> = owned_map.keys().copied().chain([row.record]).collect();
        if !owned_handles.is_empty() && self.is_any_handle_pointed(&skip, &owned_handles) {
            return Err(Decline::Unsupported);
        }

        staged.removed.push(RemovedEntry {
            row: row_idx,
            records: skip.into_iter().collect(),
        });
        Ok(())
    }

    fn is_any_handle_pointed(&self, skip: &BTreeSet<u32>, handles: &BTreeSet<u64>) -> bool {
        let is_handle_code =
            |code: i16| matches!(code, 5 | 105 | 320..=369 | 390..=399 | 480..=481 | 1005);
        for &(sk, ref range) in &self.sections {
            if !sk.has_db_objects() {
                continue;
            }
            for rec in range.start..range.end {
                if self.gone.contains(&rec) || skip.contains(&rec) {
                    continue;
                }
                let tags = self.record_tags(rec);
                let Some(first) = tags.first() else {
                    continue;
                };
                if first.code != 0 {
                    continue;
                }
                let hit = self.with_record_slice(rec, |rs| {
                    let kind = first.raw_in(&rs).trim_ascii();
                    if matches!(kind, b"SECTION" | b"ENDSEC" | b"EOF") {
                        return false;
                    }
                    tags.iter().skip(1).any(|tag| {
                        is_handle_code(tag.code)
                            && matches!(
                                crate::value::typed_of(kind, tag.code, tag.raw_in(&rs)),
                                crate::value::Val::Handle(h) if handles.contains(&h)
                            )
                    })
                });
                if hit {
                    return true;
                }
            }
        }
        let empty_rs = RecordSlice {
            base: 0,
            bytes: &[],
        };
        for (&rec, ov) in self.overlay.range(self.counts.records..) {
            if self.gone.contains(&rec) || skip.contains(&rec) {
                continue;
            }
            let Some(first) = ov.tags.first() else {
                continue;
            };
            let kind = first.raw_in(&empty_rs).trim_ascii();
            for tag in ov.tags.iter().skip(1) {
                if is_handle_code(tag.code)
                    && let crate::value::Val::Handle(h) =
                        crate::value::typed_of(kind, tag.code, tag.raw_in(&empty_rs))
                    && handles.contains(&h)
                {
                    return true;
                }
            }
        }
        if handles.iter().any(|h| self.header.handles.contains(h)) {
            return true;
        }
        if let Some(range) = self.header.handseed {
            for v in self.header_var_values(range) {
                if let Data::Handle(h) = v.data
                    && handles.contains(&h)
                {
                    return true;
                }
            }
        }
        false
    }

    fn apply_forget_refs(&self, staged: &mut StagedState) -> Result<(), Decline> {
        if staged.removed.is_empty() || self.forget_refs.is_empty() {
            return Ok(());
        }
        let mut removing_records: BTreeSet<u32> = BTreeSet::new();
        let mut removed_handles: BTreeSet<u64> = BTreeSet::new();
        for rem in &staged.removed {
            for &rec in &rem.records {
                removing_records.insert(rec);
                if let Some(h) = self.record_handle(rec) {
                    removed_handles.insert(h);
                }
            }
        }
        for h in removed_handles {
            if h == 0 && self.has_surviving_zero_handle(&removing_records) {
                continue;
            }
            let start = self.forget_refs.partition_point(|&(th, _)| th < h);
            for &(th, rec) in self.forget_refs.get(start..).unwrap_or_default() {
                if th != h {
                    break;
                }
                if self.gone.contains(&rec) || removing_records.contains(&rec) {
                    continue;
                }
                let (kind, old_vals) = staged
                    .get_kind_and_values(self, rec)
                    .ok_or(Decline::Unsupported)?;
                let new_vals = zumen_acad::db::forget_values(&kind, old_vals.clone(), h);
                if new_vals != old_vals {
                    staged
                        .apply_diff(self, rec, &old_vals, &new_vals)
                        .ok_or(Decline::Unsupported)?;
                    let in_entities = self.entities.iter().any(|r| r.contains(&rec))
                        || rec >= self.counts.records;
                    staged.touched_extents = merge_extents(
                        staged.touched_extents,
                        entity_extents(&kind, in_entities, &new_vals),
                    );
                    if let Some(r_idx) = self.row_by_record(rec)
                        && !staged.touched_rows.contains(&r_idx)
                    {
                        staged.touched_rows.push(r_idx);
                    }
                }
            }
        }
        Ok(())
    }

    fn has_surviving_zero_handle(&self, removing_records: &BTreeSet<u32>) -> bool {
        for &(sk, ref range) in &self.sections {
            if !sk.has_db_objects() {
                continue;
            }
            for rec in range.start..range.end {
                if self.gone.contains(&rec) || removing_records.contains(&rec) {
                    continue;
                }
                if self.record_handle(rec) == Some(0) {
                    return true;
                }
            }
        }
        false
    }

    fn compute_batch_initial_handle(&self) -> Result<u64, Decline> {
        let seed = self.current_handseed();
        single::next_handle(self.live_max_handle(), seed).ok_or(Decline::Unsupported)
    }

    fn stage_add(&mut self, staged: &mut StagedState, add: &Add) -> Result<u64, Decline> {
        let handle = match staged.next_handle_cursor {
            Some(next) => next,
            None => self.compute_batch_initial_handle()?,
        };
        let born_records = self
            .build_born_records(add, handle)
            .ok_or(Decline::Unsupported)?;
        let count = born_records.len() as u64;
        let last_minted = handle
            .checked_add(count.saturating_sub(1))
            .ok_or(Decline::Unsupported)?;
        let next_after = handle.checked_add(count).ok_or(Decline::Unsupported)?;
        staged.next_handle_cursor = Some(next_after);
        staged.live_max_after = Some(
            staged
                .live_max_after
                .map_or(last_minted, |m| m.max(last_minted)),
        );

        let encoded = encode_born_records(born_records).ok_or(Decline::Unsupported)?;
        for (kind, _, vals) in &encoded {
            staged.touched_extents =
                merge_extents(staged.touched_extents, entity_extents(kind, true, vals));
        }
        staged.added_plans.push((handle, encoded));
        Ok(handle)
    }

    fn stage_header_updates(&self, staged: &mut StagedState) -> Result<(), Decline> {
        if self.header.record.is_none() {
            return Ok(());
        }
        if self.header.handseed.is_none()
            && self.header.extmin.is_none()
            && self.header.extmax.is_none()
        {
            return Ok(());
        }

        let live_max = if self.header.handseed.is_none() {
            None
        } else if staged.removed.is_empty() {
            match (self.live_max_handle(), staged.live_max_after) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (a, b) => a.or(b),
            }
        } else {
            let mut removed_handles = BTreeSet::new();
            for rem in &staged.removed {
                for &rec in &rem.records {
                    if let Some(h) = self.record_handle(rec) {
                        removed_handles.insert(h);
                    }
                }
            }
            [
                self.fixed_max_handle,
                self.by_handle
                    .max_live_excluding(&self.rows, &removed_handles),
                self.live_object_handles
                    .iter()
                    .rev()
                    .find(|h| !removed_handles.contains(h))
                    .copied(),
                self.added_follower_handles
                    .iter()
                    .rev()
                    .find(|h| !removed_handles.contains(h))
                    .copied(),
                staged.live_max_after,
            ]
            .into_iter()
            .flatten()
            .max()
        };

        let mut replacements: Vec<(usize, Vec<Value>, Vec<Value>)> = Vec::new();
        if let Some(ext) = staged.touched_extents {
            for (var, is_max) in [(self.header.extmin, false), (self.header.extmax, true)] {
                if let Some(range) = var {
                    let old_v = self.header_var_values(range);
                    if let Some(new_v) = widen_extents(is_max, &old_v, ext) {
                        replacements.push((range.0, old_v, new_v));
                    }
                }
            }
        }
        if let (Some(range), Some(max_h)) = (self.header.handseed, live_max) {
            let old_v = self.header_var_values(range);
            if let Some(new_v) = advance_handseed(&old_v, max_h) {
                replacements.push((range.0, old_v, new_v));
            }
        }

        if replacements.is_empty() {
            return Ok(());
        }
        replacements.sort_by_key(|(at, _, _)| core::cmp::Reverse(*at));
        let total_len = self.header.spans.len();
        let mut edits = Vec::new();
        for (tag_start, old_v, new_v) in replacements {
            let part = diff_values(tag_start, &old_v, &new_v).map_err(|_| Decline::Unsupported)?;
            for e in &part {
                match *e {
                    TagEdit::Set { at, .. } | TagEdit::Remove { at } if at >= total_len => {
                        return Err(Decline::Unsupported);
                    }
                    TagEdit::Insert { at, .. } if at > total_len => {
                        return Err(Decline::Unsupported);
                    }
                    _ => {}
                }
            }
            edits.extend(part);
        }
        staged.header_edits = edits;
        Ok(())
    }

    /// 変更の列を 1 つのトランザクションとして当て、操作ごとの新しいハンドル（足した変更のみ `Some`）を返す
    pub fn apply(&mut self, edits: &[Edit]) -> Result<Vec<Option<u64>>, Decline> {
        if edits.is_empty() {
            return Ok(Vec::new());
        }
        if self.has_duplicate_object_handle {
            return Err(Decline::Unsupported);
        }
        let mut staged = StagedState {
            seen_handles: BTreeSet::new(),
            overlay_ops: BTreeMap::new(),
            overlay_now: BTreeMap::new(),
            header_edits: Vec::new(),
            touched_rows: Vec::new(),
            added_plans: Vec::new(),
            removed: Vec::new(),
            touched_extents: None,
            next_handle_cursor: None,
            live_max_after: None,
        };
        let mut results = Vec::with_capacity(edits.len());

        for edit in edits {
            match edit {
                Edit::Transform { handle, matrix } => {
                    let row = self.resolve_target_row(&mut staged, *handle)?;
                    self.stage_transform(&mut staged, row, *matrix)?;
                    results.push(None);
                }
                Edit::Restyle { handle, style } => {
                    let row = self.resolve_target_row(&mut staged, *handle)?;
                    self.stage_restyle(&mut staged, row, style)?;
                    results.push(None);
                }
                Edit::Add(add) => {
                    let h = self.stage_add(&mut staged, add)?;
                    results.push(Some(h));
                }
                Edit::Remove { handle } => {
                    let row = self.resolve_target_row(&mut staged, *handle)?;
                    self.stage_remove(&mut staged, row)?;
                    results.push(None);
                }
            }
        }

        self.apply_forget_refs(&mut staged)?;
        // `Today` は `apply` のたびに `write_back` を走らせ、`$HANDSEED` が既存の最大ハンドル以下なら進める
        self.stage_header_updates(&mut staged)?;

        let mut overlays = Vec::with_capacity(
            staged.overlay_now.len()
                + usize::from(!staged.header_edits.is_empty())
                + staged.added_plans.len() * 2,
        );
        if !staged.header_edits.is_empty()
            && let Some(h_rec) = self.header.record
        {
            let created = !self.overlay.contains_key(&h_rec);
            let spans = &self.header.spans;
            let ov = self.overlay.entry(h_rec).or_insert_with(|| OverlayRecord {
                base: 0,
                raw: Box::new([]),
                tags: spans
                    .iter()
                    .map(|&span| OTag {
                        code: span.code,
                        span,
                        edited: None,
                        fresh: false,
                    })
                    .collect(),
            });
            let ops = apply_tag_edits_in_place(&mut ov.tags, staged.header_edits)
                .ok_or(Decline::Unsupported)?;
            overlays.push(RecordUndo::Patched {
                record: h_rec,
                created,
                ops,
            });
        }

        for (rec, (after_ov, modified)) in staged.overlay_now {
            if !modified {
                continue;
            }
            let created = !self.overlay.contains_key(&rec);
            let ops = staged.overlay_ops.remove(&rec).unwrap_or_default();
            self.overlay.insert(rec, after_ov);
            overlays.push(RecordUndo::Patched {
                record: rec,
                created,
                ops,
            });
        }

        for rem in &staged.removed {
            self.kill_row(rem.row, &rem.records);
        }

        for &row_idx in &staged.touched_rows {
            if self.rows.get(row_idx as usize).is_some_and(|r| r.alive) {
                let _ = self.refresh_row_from_record(row_idx, None);
            }
        }

        let mut added_entries = Vec::with_capacity(staged.added_plans.len());
        for (_, encoded) in staged.added_plans {
            let mut rec_ids = Vec::with_capacity(encoded.len());
            for (_, tags, _) in encoded {
                let rec = self.next_added_record;
                self.next_added_record += 1;
                self.overlay.insert(rec, OverlayRecord::born(tags.clone()));
                overlays.push(RecordUndo::Born { record: rec, tags });
                rec_ids.push(rec);
            }
            let _ = self.push_born_row(&rec_ids)?;
            added_entries.push(AddedEntry { records: rec_ids });
        }

        if !self.plain && (!staged.removed.is_empty() || !added_entries.is_empty()) {
            self.reattach_live();
        }

        self.history.redo_stack.clear();
        self.history.undo_stack.push(Entry {
            overlays,
            touched_rows: staged.touched_rows,
            added: added_entries,
            removed: staged.removed,
        });
        Ok(results)
    }

    fn push_born_row(&mut self, rec_ids: &[u32]) -> Result<u32, Decline> {
        let &main_rec = rec_ids.first().ok_or(Decline::Unsupported)?;
        for &frec in rec_ids.get(1..).unwrap_or_default() {
            if let Some(fh) = self.record_handle(frec) {
                self.added_follower_handles.insert(fh);
            }
        }
        let empty_rs = RecordSlice {
            base: 0,
            bytes: &[],
        };
        let tags = self.record_tags(main_rec);
        let first = tags.first().ok_or(Decline::Unsupported)?;
        let slice_kind =
            Kind::of(first.raw_in(&empty_rs).trim_ascii()).ok_or(Decline::Unsupported)?;
        let body = tags.get(1..).unwrap_or_default();
        let raw_tags: Vec<single::RawTag<'_>> = body
            .iter()
            .map(|t| single::RawTag {
                code: t.code,
                value: t.raw_in(&empty_rs),
            })
            .collect();
        let Lift::Element(mut lifted) =
            lift::lift(self.charset, slice_kind, &raw_tags, &mut self.shapes.pools)
        else {
            return Err(Decline::Unsupported);
        };
        let layer_id = self.bind_lifted_ids(&mut lifted);

        if let Geom::Insert(ref mut ins) = lifted.geom {
            let mut att_recs = Vec::new();
            for &frec in rec_ids.get(1..).unwrap_or_default() {
                if let Some((fk, _)) = self.record_kind_and_values(frec)
                    && fk == b"ATTRIB"
                {
                    att_recs.push(frec);
                }
            }
            ins.attached = att_recs.len() as u32;
            ins.attrib_first = self.shapes.pools.attribs.len() as u32;
            let mut cnt = 0u32;
            for frec in att_recs {
                if let Some(arow) = self.lift_single_attrib_record(frec) {
                    self.shapes.pools.attribs.push(arow);
                    cnt += 1;
                }
            }
            ins.attrib_count = cnt;
        }

        let bx = bounds(&self.shapes.pools, &lifted.geom);
        let geom_copy = lifted.geom;
        let (kind, shape_idx) = self.shapes.push(lifted.geom);
        let row_idx = self.rows.len() as u32;
        let row = Row::new(
            main_rec,
            lifted.handle,
            kind,
            shape_idx,
            layer_id,
            lifted.color,
            lifted.extra_style,
            true,
        );
        self.rows.push(row);
        self.boxes.push(bx);
        self.grid.push(bx);
        self.display
            .push(&geom_copy, &self.shapes.pools, [row.layer, row.color]);
        self.adjust_row_uses(row, 1);
        if let Some(h) = row.handle()
            && !self.duplicate_handles.contains(&h)
        {
            self.by_handle.insert_added(h, row_idx);
        }
        self.added_by_record.insert(main_rec, row_idx);
        self.counts.elements += 1;
        Ok(row_idx)
    }

    fn kill_row(&mut self, row_idx: u32, records: &[u32]) {
        let main_rec = self.rows.get(row_idx as usize).map(|r| r.record);
        for &rec in records {
            if let Some(h) = self.record_handle(rec) {
                if rec >= self.counts.records && Some(rec) != main_rec {
                    self.added_follower_handles.remove(&h);
                } else if self.objects_by_handle.get(&h) == Some(&rec) {
                    self.live_object_handles.remove(&h);
                }
            }
            self.gone.insert(rec);
        }
        if let Some(row) = self.rows.get(row_idx as usize).copied()
            && row.alive
        {
            self.adjust_row_uses(row, -1);
            if let Some(slot) = self.rows.get_mut(row_idx as usize) {
                slot.alive = false;
            }
            self.grid.update(
                row_idx,
                [
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                ],
            );
            self.display.hide(row_idx);
            if let Some(h) = row.handle() {
                self.by_handle.remove_row(
                    h,
                    row_idx,
                    row.record >= self.counts.records,
                    &self.rows,
                );
            }
            if row.record >= self.counts.records {
                self.added_by_record.remove(&row.record);
            }
            self.counts.elements = self.counts.elements.saturating_sub(1);
        }
    }

    fn revive_row(&mut self, row_idx: u32, records: &[u32]) {
        let main_rec = self.rows.get(row_idx as usize).map(|r| r.record);
        for &rec in records {
            self.gone.remove(&rec);
            if let Some(h) = self.record_handle(rec) {
                if rec >= self.counts.records && Some(rec) != main_rec {
                    self.added_follower_handles.insert(h);
                } else if self.objects_by_handle.get(&h) == Some(&rec) {
                    self.live_object_handles.insert(h);
                }
            }
        }
        if let Some(row) = self.rows.get(row_idx as usize).copied()
            && !row.alive
        {
            if let Some(slot) = self.rows.get_mut(row_idx as usize) {
                slot.alive = true;
            }
            let mut revived = row;
            revived.alive = true;
            self.adjust_row_uses(revived, 1);
            if let Some(bx) = self.boxes.get(row_idx as usize).copied() {
                self.grid.update(row_idx, bx);
            }
            self.display.show(row_idx);
            if let Some(h) = row.handle()
                && !self.duplicate_handles.contains(&h)
            {
                if row.record >= self.counts.records {
                    self.by_handle.insert_added(h, row_idx);
                } else {
                    self.by_handle.revive_base(h);
                }
            }
            if row.record >= self.counts.records {
                self.added_by_record.insert(row.record, row_idx);
            }
            self.counts.elements += 1;
        }
    }

    fn pop_born_row(&mut self, added: &AddedEntry) {
        let main_rec = added.records.first().copied();
        for &rec in &added.records {
            if rec >= self.counts.records
                && Some(rec) != main_rec
                && let Some(h) = self.record_handle(rec)
            {
                self.added_follower_handles.remove(&h);
            }
        }
        let popped_idx = self.rows.len().saturating_sub(1) as u32;
        if let Some(row) = self.rows.pop() {
            if row.alive {
                self.adjust_row_uses(row, -1);
                if let Some(h) = row.handle() {
                    self.by_handle.remove_row(h, popped_idx, true, &self.rows);
                }
                self.added_by_record.remove(&row.record);
                self.counts.elements = self.counts.elements.saturating_sub(1);
            }
            self.shapes.pop(row.kind);
            self.boxes.pop();
            self.grid.pop();
            self.display.pop();
        }
        self.next_added_record = self
            .next_added_record
            .saturating_sub(added.records.len() as u32);
    }

    /// 直前のトランザクションを取り消す。取り消せたら `true`
    pub fn undo(&mut self) -> bool {
        let Some(entry) = self.history.undo_stack.pop() else {
            return false;
        };
        for added in entry.added.iter().rev() {
            self.pop_born_row(added);
        }
        for ov_undo in entry.overlays.iter().rev() {
            match ov_undo {
                RecordUndo::Born { record, .. } => {
                    self.overlay.remove(record);
                }
                RecordUndo::Patched { record, ops, .. } => {
                    if let Some(ov) = self.overlay.get_mut(record) {
                        for op in ops.iter().rev() {
                            match op {
                                AppliedTagOp::Set { at, before, .. } => {
                                    if let Some(slot) = ov.tags.get_mut(*at) {
                                        slot.edited.clone_from(before);
                                    }
                                }
                                AppliedTagOp::Remove { at, removed } => {
                                    if *at <= ov.tags.len() {
                                        ov.tags.insert(*at, removed.clone());
                                    }
                                }
                                AppliedTagOp::Insert { at, .. } => {
                                    if *at < ov.tags.len() {
                                        ov.tags.remove(*at);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        for rem in entry.removed.iter().rev() {
            self.revive_row(rem.row, &rem.records);
        }
        for &row_idx in &entry.touched_rows {
            if self.rows.get(row_idx as usize).is_some_and(|r| r.alive) {
                let _ = self.refresh_row_from_record(row_idx, None);
            }
        }
        for ov_undo in entry.overlays.iter().rev() {
            if let RecordUndo::Patched {
                record,
                created: true,
                ..
            } = ov_undo
            {
                self.overlay.remove(record);
            }
        }
        if !self.plain && (!entry.removed.is_empty() || !entry.added.is_empty()) {
            self.reattach_live();
        }
        self.history.redo_stack.push(entry);
        true
    }

    /// 直前に取り消したトランザクションをやり直す。やり直せたら `true`
    pub fn redo(&mut self) -> bool {
        let Some(entry) = self.history.redo_stack.pop() else {
            return false;
        };
        for ov_undo in &entry.overlays {
            match ov_undo {
                RecordUndo::Born { record, tags } => {
                    self.overlay
                        .insert(*record, OverlayRecord::born(tags.clone()));
                }
                RecordUndo::Patched {
                    record,
                    created,
                    ops,
                } => {
                    if *created {
                        let fresh = if self.header.record == Some(*record) {
                            OverlayRecord {
                                base: 0,
                                raw: Box::new([]),
                                tags: self
                                    .header
                                    .spans
                                    .iter()
                                    .map(|&span| OTag {
                                        code: span.code,
                                        span,
                                        edited: None,
                                        fresh: false,
                                    })
                                    .collect(),
                            }
                        } else {
                            self.src
                                .with_range(self.record_range(*record), OverlayRecord::from_slice)
                        };
                        self.overlay.insert(*record, fresh);
                    }
                    if let Some(ov) = self.overlay.get_mut(record) {
                        for op in ops {
                            match op {
                                AppliedTagOp::Set { at, after, .. } => {
                                    if let Some(slot) = ov.tags.get_mut(*at) {
                                        slot.edited = Some(after.clone());
                                    }
                                }
                                AppliedTagOp::Remove { at, .. } => {
                                    if *at < ov.tags.len() {
                                        ov.tags.remove(*at);
                                    }
                                }
                                AppliedTagOp::Insert { at, inserted } => {
                                    if *at <= ov.tags.len() {
                                        ov.tags.insert(*at, inserted.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        for rem in &entry.removed {
            self.kill_row(rem.row, &rem.records);
        }
        for &row_idx in &entry.touched_rows {
            if self.rows.get(row_idx as usize).is_some_and(|r| r.alive) {
                let _ = self.refresh_row_from_record(row_idx, None);
            }
        }
        for added in &entry.added {
            if let Some(&last_rec) = added.records.last() {
                self.next_added_record = self.next_added_record.max(last_rec + 1);
            }
            let _ = self.push_born_row(&added.records);
        }
        if !self.plain && (!entry.removed.is_empty() || !entry.added.is_empty()) {
            self.reattach_live();
        }
        self.history.undo_stack.push(entry);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::born::{ColorSpec, LineTypeSpec, NewShape, NewSymbol, NewText};
    use crate::inspect;
    use crate::lift::Geom;
    use zumen::messages::{BlobKind, EntitiesResult, Extent, TablesResult};
    use zumen::{CancelToken, Response, Session};
    use zumen_acad::Charset;

    const HEADER: &str = "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n  9\n$HANDSEED\n  5\nFFFF\n\
                          \x20 9\n$EXTMIN\n 10\n0.0\n 20\n0.0\n 30\n0.0\n  9\n$EXTMAX\n 10\n10.0\n 20\n10.0\n 30\n0.0\n\
                          \x20 0\nENDSEC\n";

    /// 線種と画層の表
    const TABLES: &str = "  0\nSECTION\n  2\nTABLES\n  0\nTABLE\n  2\nLTYPE\n  5\n1\n 70\n1\n\
                          \x20 0\nLTYPE\n  5\n10\n  2\nDASHED\n 70\n0\n  0\nENDTAB\n\
                          \x20 0\nTABLE\n  2\nLAYER\n  5\n2\n 70\n2\n\
                          \x20 0\nLAYER\n  5\n11\n  2\n0\n 70\n0\n 62\n7\n  6\nCONTINUOUS\n\
                          \x20 0\nLAYER\n  5\n12\n  2\nWalls\n 70\n0\n 62\n1\n  6\nCONTINUOUS\n\
                          \x20 0\nENDTAB\n  0\nENDSEC\n";

    fn dxf(entities: &str) -> Vec<u8> {
        format!("{HEADER}  0\nSECTION\n  2\nENTITIES\n{entities}  0\nENDSEC\n  0\nEOF\n")
            .into_bytes()
    }

    fn tabled(entities: &str) -> Vec<u8> {
        format!("{HEADER}{TABLES}  0\nSECTION\n  2\nENTITIES\n{entities}  0\nENDSEC\n  0\nEOF\n")
            .into_bytes()
    }

    fn open(entities: &str) -> Document {
        Document::open(dxf(entities), 1).expect("開く")
    }

    fn text(bytes: &[u8]) -> String {
        String::from_utf8(bytes.to_vec()).expect("ASCII")
    }

    fn geom_of(document: &Document, row_index: usize) -> Option<Geom> {
        let row = document.rows.get(row_index)?;
        row.alive.then(|| document.shapes.geom(row.kind, row.shape))
    }

    const LINE: &str =
        "  0\nLINE\n  5\n2A\n  8\n0\n 10\n1.0\n 20\n2.0\n 30\n0.0\n 11\n3.0\n 21\n4.0\n 31\n0.0\n";

    const CIRCLE: &str = "  0\nCIRCLE\n  5\n2B\n  8\n0\n 10\n5.0\n 20\n5.0\n 40\n1.5\n";

    fn translation(handle: u64, dx: f64, dy: f64) -> Edit {
        Edit::Transform {
            handle,
            matrix: [1.0, 0.0, dx, 0.0, 1.0, dy],
        }
    }

    fn session_call(session: &mut Session, request: &str) -> Response {
        let cancel = CancelToken::new();
        let reply = session
            .call_bytes(request.as_bytes(), &cancel)
            .expect("Session::call_bytes");
        Response::parse(&reply).expect("Response::parse")
    }

    fn take_file(session: &mut Session, response: &Response) -> Vec<u8> {
        let blob = response
            .blobs
            .iter()
            .find(|b| matches!(b.kind, BlobKind::File))
            .expect("BlobKind::File");
        session
            .take(u32::try_from(blob.id).expect("blob id"))
            .expect("session.take")
    }

    #[test]
    fn moving_a_line_rewrites_its_points_and_widens_the_extents() {
        let mut document = open(LINE);
        let before = document.save();
        assert_eq!(before, dxf(LINE));
        assert_eq!(
            document.apply(&[translation(0x2A, 10.0, 0.5)]),
            Ok(vec![None])
        );
        let after = text(&document.save());
        assert!(
            after.contains(" 10\n11.0\n 20\n2.5\n 30\n0.0\n 11\n13.0\n 21\n4.5\n"),
            "{after}"
        );
        assert!(after.contains("$EXTMIN\n 10\n0.0\n 20\n0.0\n"), "{after}");
        assert!(after.contains("$EXTMAX\n 10\n13.0\n 20\n10.0\n"), "{after}");
        assert_eq!(
            geom_of(&document, 0),
            Some(Geom::Line([11.0, 2.5, 13.0, 4.5]))
        );
        assert_eq!(document.live_boxes(), [[11.0, 2.5, 13.0, 4.5]]);
        assert!(document.undo());
        assert_eq!(document.save(), before);
        assert_eq!(
            geom_of(&document, 0),
            Some(Geom::Line([1.0, 2.0, 3.0, 4.0]))
        );
        assert_eq!(document.edited_records(), 0);
        assert!(!document.undo());
        assert!(document.redo());
        assert_eq!(text(&document.save()), after);
        assert!(!document.redo());
    }

    #[test]
    fn the_edited_document_equals_a_reopened_one() {
        let entities = format!(
            "{LINE}  0\nCIRCLE\n  5\n2B\n  8\nA\n 10\n5.0\n 20\n5.0\n 40\n1.5\n\
             \x20 0\nARC\n  5\n2C\n  8\nA\n 10\n5.0\n 20\n5.0\n 40\n2.0\n 50\n0.0\n 51\n90.0\n\
             \x20 0\nLWPOLYLINE\n  5\n2D\n  8\n0\n 90\n3\n 70\n1\n 43\n0.0\n 10\n0.0\n 20\n0.0\n 10\n4.0\n 20\n0.0\n 42\n0.5\n 10\n4.0\n 20\n3.0\n\
             \x20 0\nTEXT\n  5\n2E\n  8\n0\n 10\n1.0\n 20\n1.0\n 40\n0.5\n  1\nabc\n 72\n1\n 11\n2.0\n 21\n1.0\n"
        );
        let mut document = open(&entities);
        assert_eq!(document.rows.len(), 5);
        let quarter = Affine::rotation(1.0, 1.0, 90.0).m;
        let edits: Vec<Edit> = [0x2A, 0x2B, 0x2C, 0x2D, 0x2E]
            .into_iter()
            .map(|handle| Edit::Transform {
                handle,
                matrix: quarter,
            })
            .collect();
        assert_eq!(document.apply(&edits), Ok(vec![None; 5]));
        assert_eq!(
            document.apply(&[translation(0x2D, 0.25, 0.0)]),
            Ok(vec![None])
        );
        let saved = document.save();
        let reopened = Document::open(saved.clone(), 1).expect("開き直す");
        assert!(document.same_content(&reopened));
        assert_eq!(document.live_display(), reopened.live_display());
        assert_eq!(document.live_boxes(), reopened.live_boxes());
        let saved = text(&saved);
        assert!(!saved.contains(" 43\n"), "{saved}");
        assert!(document.undo());
        assert!(document.undo());
        assert_eq!(document.save(), dxf(&entities));
    }

    #[test]
    fn declined_edits_change_nothing() {
        let circle_2c = CIRCLE.replace("2B", "2C");
        let entities = format!("{LINE}{CIRCLE}{circle_2c}");
        let mut document = open(&entities);
        let before = document.save();
        assert_eq!(
            document.apply(&[translation(0x99, 1.0, 0.0)]),
            Err(Decline::NotFound)
        );
        assert_eq!(
            document.apply(&[translation(0x2A, 1.0, 0.0), translation(0x2A, 2.0, 0.0)]),
            Err(Decline::Duplicate)
        );
        assert_eq!(
            document.apply(&[translation(0x2A, 1.0, 0.0), Edit::Remove { handle: 0x2A }]),
            Err(Decline::Duplicate)
        );
        let flat = Edit::Transform {
            handle: 0x2A,
            matrix: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        };
        assert_eq!(document.apply(&[flat]), Err(Decline::Unsupported));
        let stretch = [2.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let ellipse = Edit::Transform {
            handle: 0x2C,
            matrix: stretch,
        };
        assert_eq!(
            document.apply(&[
                translation(0x2A, 1.0, 0.0),
                Edit::Remove { handle: 0x2B },
                ellipse
            ]),
            Err(Decline::Unsupported)
        );
        // 表にも図形にも存在しない画層は、今の実装と同じく受け付けない
        let restyle = Edit::Restyle {
            handle: 0x2A,
            style: Restyle {
                layer: Some("Walls".to_string()),
                ..Restyle::default()
            },
        };
        assert_eq!(document.apply(&[restyle]), Err(Decline::Unsupported));
        assert_eq!(document.save(), before);
        assert_eq!(document.undo_len(), 0);
        let line = Edit::Transform {
            handle: 0x2A,
            matrix: stretch,
        };
        assert_eq!(document.apply(&[line]), Ok(vec![None]));
        assert_eq!(
            geom_of(&document, 0),
            Some(Geom::Line([2.0, 2.0, 6.0, 4.0]))
        );
    }

    #[test]
    fn a_seed_not_above_the_largest_handle_is_raised_by_any_edit() {
        let entities = LINE.replace("2A", "FFFF");
        let mut document = open(&entities);
        assert!(document.duplicate_handles.is_empty());
        assert_eq!(
            document.apply(&[translation(0xFFFF, 1.0, 0.0)]),
            Ok(vec![None])
        );
        let after = text(&document.save());
        assert!(after.contains("$HANDSEED\n  5\n10000\n"), "{after}");
        assert!(document.undo());
        assert_eq!(document.save(), dxf(&entities));
    }

    #[test]
    fn restyling_rewrites_the_common_values_after_the_layer() {
        let entities = format!("{LINE}{CIRCLE}");
        let source = tabled(&entities);
        let mut document = Document::open(source.clone(), 1).expect("開く");
        let restyle = Edit::Restyle {
            handle: 0x2A,
            style: Restyle {
                layer: Some("Walls".to_string()),
                color: Some(ColorSpec::Aci(3)),
                line_type: Some(LineTypeSpec::Name("DASHED".to_string())),
                ..Restyle::default()
            },
        };
        assert_eq!(document.apply(&[restyle]), Ok(vec![None]));
        let saved = document.save();
        let after = text(&saved);
        assert!(
            after.contains("  5\n2A\n  8\nWalls\n  6\nDASHED\n 62\n3\n 10\n1.0\n"),
            "{after}"
        );
        let reopened = Document::open(saved, 1).expect("開き直す");
        assert!(document.same_content(&reopened));
        assert_eq!(document.live_display(), reopened.live_display());
        let same = Edit::Restyle {
            handle: 0x2A,
            style: Restyle {
                layer: Some("Walls".to_string()),
                color: Some(ColorSpec::Aci(3)),
                ..Restyle::default()
            },
        };
        assert_eq!(document.apply(&[same]), Ok(vec![None]));
        assert_eq!(text(&document.save()), after);
        let zero = Edit::Restyle {
            handle: 0x2A,
            style: Restyle {
                color: Some(ColorSpec::Aci(0)),
                ..Restyle::default()
            },
        };
        assert_eq!(document.apply(&[zero]), Err(Decline::Unsupported));
        assert!(document.undo());
        assert!(document.undo());
        assert_eq!(document.save(), source);
    }

    #[test]
    fn removing_and_adding_elements_can_be_undone_and_redone() {
        let entities = format!("{LINE}{CIRCLE}");
        let source = tabled(&entities);
        let mut document = Document::open(source.clone(), 1).expect("開く");
        let circle = Edit::Add(Box::new(Add {
            layer: Some("Walls".to_string()),
            color: Some(ColorSpec::Aci(1)),
            line_type: None,
            line_weight: None,
            shape: NewShape::Circle {
                center: [9.0, 6.0],
                radius: 2.0,
            },
        }));
        assert_eq!(
            document.apply(&[Edit::Remove { handle: 0x2A }, circle]),
            Ok(vec![None, Some(0xFFFF)])
        );
        assert_eq!(document.row_of(0x2A), None);
        assert_eq!(document.row_of(0xFFFF), Some(2));
        let saved = document.save();
        let after = text(&saved);
        assert!(!after.contains("LINE"), "{after}");
        assert!(
            after.contains(
                "  0\nCIRCLE\n  5\nFFFF\n100\nAcDbEntity\n  8\nWalls\n 62\n1\n100\nAcDbCircle\n \
                 10\n9.0\n 20\n6.0\n 30\n0.0\n 40\n2.0\n  0\nENDSEC\n  0\nEOF\n"
            ),
            "{after}"
        );
        assert!(after.contains("$HANDSEED\n  5\n10000\n"), "{after}");
        assert!(after.contains("$EXTMAX\n 10\n11.0\n 20\n10.0\n"), "{after}");
        let reopened = Document::open(saved.clone(), 1).expect("開き直す");
        assert!(document.same_content(&reopened));
        let mut found = Vec::new();
        document.select([8.0, 5.0, 9.0, 6.0], &mut found);
        assert_eq!(found, [2]);
        assert_eq!(
            document.apply(&[translation(0xFFFF, 1.0, 0.0)]),
            Ok(vec![None])
        );
        assert_eq!(
            document.apply(&[Edit::Remove { handle: 0xFFFF }]),
            Ok(vec![None])
        );
        assert_eq!(document.row_of(0xFFFF), None);
        let emptied = document.save();
        assert!(!text(&emptied).contains("FFFF"), "{}", text(&emptied));
        for _ in 0..3 {
            assert!(document.undo());
        }
        assert!(!document.undo());
        assert_eq!(document.save(), source);
        // ページ配列のスロットは残し、生きている要素だけ 2 件に戻る
        assert_eq!(document.live_boxes().len(), 2);
        assert_eq!(document.row_of(0x2A), Some(0));
        assert!(document.redo());
        assert_eq!(document.save(), saved);
        assert!(document.redo());
        assert!(document.redo());
        assert_eq!(document.save(), emptied);
    }

    #[test]
    fn removing_an_element_drops_the_references_to_it() {
        let circle = "  0\nCIRCLE\n  5\n2B\n102\n{ACAD_REACTORS\n330\n2A\n102\n}\n  8\n0\n 10\n5.0\n 20\n5.0\n 40\n1.5\n";
        let entities = format!("{LINE}{circle}");
        let source = tabled(&entities);
        let mut document = Document::open(source.clone(), 1).expect("開く");
        assert_eq!(
            document.apply(&[Edit::Remove { handle: 0x2B }]),
            Err(Decline::Unsupported)
        );
        assert_eq!(
            document.apply(&[Edit::Remove { handle: 0x2A }]),
            Ok(vec![None])
        );
        let after = text(&document.save());
        assert!(
            after.contains("  0\nCIRCLE\n  5\n2B\n  8\n0\n 10\n5.0\n"),
            "{after}"
        );
        assert!(document.undo());
        assert_eq!(document.save(), source);
    }

    #[test]
    fn removing_an_element_with_xdictionary_removes_owned_objects_and_restores_on_undo() {
        let line_with_xdict = "  0\nLINE\n  5\n2A\n102\n{ACAD_XDICTIONARY\n360\n30\n102\n}\n  8\n0\n 10\n1.0\n 20\n2.0\n 11\n3.0\n 21\n4.0\n";
        let objects = "  0\nSECTION\n  2\nOBJECTS\n\
                       \x20 0\nDICTIONARY\n  5\n30\n330\n2A\n  3\nK\n350\n31\n\
                       \x20 0\nXRECORD\n  5\n31\n330\n30\n  1\nnote\n\
                       \x20 0\nENDSEC\n";
        let source = format!(
            "{HEADER}{TABLES}  0\nSECTION\n  2\nENTITIES\n{line_with_xdict}{CIRCLE}  0\nENDSEC\n{objects}  0\nEOF\n"
        )
        .into_bytes();
        let mut document = Document::open(source.clone(), 1).expect("開く");
        assert_eq!(
            document.apply(&[Edit::Remove { handle: 0x2A }]),
            Ok(vec![None])
        );
        let after = text(&document.save());
        assert!(!after.contains("LINE"), "{after}");
        assert!(!after.contains("DICTIONARY"), "{after}");
        assert!(!after.contains("XRECORD"), "{after}");
        assert!(document.undo());
        assert_eq!(document.save(), source);
        assert!(document.redo());
        assert_eq!(text(&document.save()), after);
    }

    #[test]
    fn shift_jis_japanese_layers_text_and_attribs_match_today() {
        let sjis = |s: &str| Charset::ShiftJis.encode(s).expect("Shift_JIS");
        let mut raw = Vec::new();
        raw.extend_from_slice(
            b"  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n  9\n$DWGCODEPAGE\n  3\nANSI_932\n\
              \x20 9\n$HANDSEED\n  5\n100\n  0\nENDSEC\n\
              \x20 0\nSECTION\n  2\nTABLES\n  0\nTABLE\n  2\nLAYER\n  5\n2\n 70\n2\n\
              \x20 0\nLAYER\n  5\n10\n  2\n0\n 70\n0\n 62\n7\n  6\nCONTINUOUS\n\
              \x20 0\nLAYER\n  5\n11\n  2\n",
        );
        raw.extend_from_slice(&sjis("壁"));
        raw.extend_from_slice(
            b"\n 70\n0\n 62\n1\n  6\nCONTINUOUS\n  0\nENDTAB\n  0\nENDSEC\n\
              \x20 0\nSECTION\n  2\nBLOCKS\n  0\nBLOCK\n  5\n20\n  8\n0\n  2\nROOM\n 70\n2\n 10\n0.0\n 20\n0.0\n\
              \x20 0\nLINE\n  5\n21\n  8\n0\n 10\n0.0\n 20\n0.0\n 11\n2.0\n 21\n0.0\n\
              \x20 0\nENDBLK\n  5\n22\n  8\n0\n  0\nENDSEC\n\
              \x20 0\nSECTION\n  2\nENTITIES\n\
              \x20 0\nTEXT\n  5\n30\n  8\n",
        );
        raw.extend_from_slice(&sjis("壁"));
        raw.extend_from_slice(b"\n 10\n1.0\n 20\n2.0\n 40\n0.5\n  1\n");
        raw.extend_from_slice(&sjis("日本語"));
        raw.extend_from_slice(
            b"\n  0\nINSERT\n  5\n31\n  8\n0\n 66\n1\n  2\nROOM\n 10\n5.0\n 20\n5.0\n\
              \x20 0\nATTRIB\n  5\n32\n330\n31\n  8\n0\n 10\n5.5\n 20\n5.5\n 40\n0.4\n  1\n",
        );
        raw.extend_from_slice(&sjis("部屋1"));
        raw.extend_from_slice(
            b"\n  2\nNAME\n 70\n0\n  0\nSEQEND\n  5\n33\n330\n31\n  8\n0\n  0\nENDSEC\n  0\nEOF\n",
        );

        let mut doc = Document::open(raw.clone(), 1).expect("Shift_JIS を開く");
        assert_eq!(doc.charset, Charset::ShiftJis);
        let h31_hash = inspect::live_elements(&doc)
            .into_iter()
            .find(|e| e.id == "space/0/h31")
            .expect("h31")
            .hash;

        let mut session = Session::open().expect("Session::open");
        session.push("a.dxf", &raw).expect("push");
        session_call(
            &mut session,
            r#"{"v":1,"op":"open","input":"a.dxf","doc":"a.dxf"}"#,
        );

        // 1) 日本語 ATTRIB 付き INSERT の移動と、日本語画層への変更、日本語文字の追加
        let edits = [
            translation(0x31, 3.0, -1.0),
            Edit::Add(Box::new(Add {
                layer: Some("壁".to_string()),
                color: None,
                line_type: None,
                line_weight: None,
                shape: NewShape::Text(NewText {
                    content: "追加文字".to_string(),
                    position: [2.0, 8.0],
                    end: None,
                    horizontal: None,
                    vertical: None,
                    height: 1.0,
                    rotation: 0.0,
                    width_factor: None,
                    oblique: None,
                    backward: false,
                    upside_down: false,
                    style: None,
                }),
            })),
        ];
        assert_eq!(doc.apply(&edits), Ok(vec![None, Some(0x100)]));

        let apply_resp = session_call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[
                    {{"kind":"transform","id":"space/0/h31","hash":"{h31_hash}","matrix":[1.0,0.0,3.0,0.0,1.0,-1.0]}},
                    {{"kind":"add","place":"space/0","layer":"壁","geometry":{{"shape":"text","content":"追加文字","position":[2.0,8.0],"height":1.0,"rotation":0.0}}}}
                ]}}"#
            ),
        );
        let today_bytes = take_file(&mut session, &apply_resp);
        assert_eq!(doc.save(), today_bytes);

        // 2) 日本語画層への変更も一致する
        let h31_hash2 = inspect::live_elements(&doc)
            .into_iter()
            .find(|e| e.id == "space/0/h31")
            .expect("h31")
            .hash;
        assert_eq!(
            doc.apply(&[Edit::Restyle {
                handle: 0x31,
                style: Restyle {
                    layer: Some("壁".to_string()),
                    ..Restyle::default()
                },
            }]),
            Ok(vec![None])
        );
        let apply_resp2 = session_call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[
                    {{"kind":"restyle","id":"space/0/h31","hash":"{h31_hash2}","layer":"壁"}}
                ]}}"#
            ),
        );
        let today_bytes2 = take_file(&mut session, &apply_resp2);
        assert_eq!(doc.save(), today_bytes2);

        // 保持している要素・ハッシュ・箱・表も今の実装と一致する
        session.push("b.dxf", &today_bytes2).expect("push b");
        session_call(
            &mut session,
            r#"{"v":1,"op":"open","input":"b.dxf","doc":"b.dxf"}"#,
        );
        let ents: EntitiesResult = session_call(
            &mut session,
            r#"{"v":1,"op":"entities","doc":"b.dxf","place":"space/0","fields":["kind","layer","hash","bbox"]}"#,
        )
        .result_as()
        .expect("result")
        .expect("EntitiesResult");
        let live_ents = inspect::live_elements(&doc);
        assert_eq!(live_ents.len(), ents.items.len());
        for (l, t) in live_ents.iter().zip(&ents.items) {
            assert_eq!(l.id, t.id);
            assert_eq!(Some(l.layer.as_str()), t.layer.as_deref());
            assert_eq!(Some(l.hash.as_str()), t.hash.as_deref());
            assert_eq!(
                Some(Extent {
                    min_x: l.bounds[0],
                    min_y: l.bounds[1],
                    max_x: l.bounds[2],
                    max_y: l.bounds[3],
                }),
                t.bbox
            );
        }
        let tables: TablesResult =
            session_call(&mut session, r#"{"v":1,"op":"tables","doc":"b.dxf"}"#)
                .result_as()
                .expect("result")
                .expect("TablesResult");
        let (live_layers, _) = inspect::live_tables(&doc);
        assert_eq!(live_layers.len(), tables.layers.len());
        for (ll, tl) in live_layers.iter().zip(&tables.layers) {
            assert_eq!(ll.id, tl.id);
            assert_eq!(ll.name, tl.name);
            assert_eq!(ll.hash, tl.hash);
        }

        assert!(doc.undo());
        assert!(doc.undo());
        assert_eq!(doc.save(), raw);
        assert!(doc.redo());
        assert!(doc.redo());
        assert_eq!(doc.save(), today_bytes2);
    }

    #[test]
    fn adding_a_symbol_with_attdefs_mints_attrib_and_seqend_like_today() {
        // 非定数の ATTDEF (ROOM) と定数の ATTDEF (CONST, 70=2) を持つブロック定義
        let blocks = "  0\nSECTION\n  2\nBLOCKS\n\
                      \x20 0\nBLOCK\n  5\n20\n  8\n0\n  2\nTAGMARK\n 70\n0\n 10\n1.0\n 20\n2.0\n\
                      \x20 0\nCIRCLE\n  5\n21\n  8\n0\n 10\n1.0\n 20\n2.0\n 40\n0.5\n\
                      \x20 0\nATTDEF\n  5\n22\n  8\n0\n 10\n2.0\n 20\n3.0\n 40\n0.4\n  1\nA1\n  3\nRoom\n  2\nROOM\n 70\n0\n\
                      \x20 0\nATTDEF\n  5\n23\n  8\n0\n 10\n2.0\n 20\n4.0\n 40\n0.4\n  1\nFIXED\n  3\nConst\n  2\nCONST\n 70\n2\n\
                      \x20 0\nENDBLK\n  5\n24\n  8\n0\n  0\nENDSEC\n";
        let src = format!(
            "{HEADER}{TABLES}{blocks}  0\nSECTION\n  2\nENTITIES\n{LINE}  0\nENDSEC\n  0\nEOF\n"
        )
        .into_bytes();

        let mut doc = Document::open(src.clone(), 1).expect("開く");
        let add = Edit::Add(Box::new(Add {
            layer: Some("Walls".to_string()),
            color: None,
            line_type: None,
            line_weight: None,
            shape: NewShape::Symbol(NewSymbol {
                name: "TAGMARK".to_string(),
                position: [10.0, 20.0],
                scale: [2.0, 2.0],
                rotation: 90.0,
                grid: None,
            }),
        }));
        // INSERT (FFFF) + 非定数 ATTRIB 1 個 (10000) + SEQEND (10001) -> $HANDSEED は 10002
        assert_eq!(doc.apply(&[add]), Ok(vec![Some(0xFFFF)]));
        let saved = doc.save();
        assert!(text(&saved).contains("$HANDSEED\n  5\n10002\n"));

        let mut session = Session::open().expect("Session::open");
        session.push("a.dxf", &src).expect("push");
        session_call(
            &mut session,
            r#"{"v":1,"op":"open","input":"a.dxf","doc":"a.dxf"}"#,
        );
        let apply_resp = session_call(
            &mut session,
            r#"{"v":1,"op":"apply","doc":"a.dxf","edits":[
                {"kind":"add","place":"space/0","layer":"Walls","geometry":{"shape":"symbol","name":"TAGMARK","position":[10.0,20.0],"scale":[2.0,2.0],"rotation":90.0}}
            ]}"#,
        );
        let today_bytes = take_file(&mut session, &apply_resp);
        assert_eq!(saved, today_bytes);

        assert!(doc.undo());
        assert_eq!(doc.save(), src);
        assert!(doc.redo());
        assert_eq!(doc.save(), today_bytes);
    }

    #[test]
    fn loose_attrib_and_seqend_attachment_supports_add_remove_undo_redo() {
        // 330 で離れた INSERT (30) を指す ATTRIB (32) と、孤立 ATTRIB (35) を混ぜた loose 図面
        let blocks = "  0\nSECTION\n  2\nBLOCKS\n\
                      \x20 0\nBLOCK\n  5\n20\n  8\n0\n  2\nMARK\n 70\n0\n 10\n0.0\n 20\n0.0\n\
                      \x20 0\nPOINT\n  5\n21\n  8\n0\n 10\n0.0\n 20\n0.0\n\
                      \x20 0\nENDBLK\n  5\n22\n  8\n0\n  0\nENDSEC\n";
        let entities = "\
              \x20 0\nINSERT\n  5\n30\n  8\n0\n 66\n1\n  2\nMARK\n 10\n1.0\n 20\n1.0\n\
              \x20 0\nLINE\n  5\n31\n  8\n0\n 10\n0.0\n 20\n0.0\n 11\n2.0\n 21\n2.0\n\
              \x20 0\nATTRIB\n  5\n32\n330\n30\n  8\n0\n 10\n1.5\n 20\n1.5\n 40\n0.3\n  1\nV\n  2\nT\n 70\n0\n\
              \x20 0\nSEQEND\n  5\n33\n330\n30\n  8\n0\n\
              \x20 0\nATTRIB\n  5\n35\n  8\n0\n 10\n9.0\n 20\n9.0\n 40\n0.3\n  1\nORPHAN\n  2\nO\n 70\n0\n";
        let src = format!(
            "{HEADER}{TABLES}{blocks}  0\nSECTION\n  2\nENTITIES\n{entities}  0\nENDSEC\n  0\nEOF\n"
        )
        .into_bytes();

        let mut doc = Document::open(src.clone(), 1).expect("loose を開く");
        assert!(!doc.plain);
        let h31_hash = inspect::live_elements(&doc)
            .into_iter()
            .find(|e| e.id == "space/0/h31")
            .expect("h31")
            .hash;

        let edits = [
            Edit::Remove { handle: 0x31 },
            Edit::Add(Box::new(Add {
                layer: Some("0".to_string()),
                color: None,
                line_type: None,
                line_weight: None,
                shape: NewShape::Line {
                    start: [3.0, 3.0],
                    end: [4.0, 5.0],
                },
            })),
        ];
        assert_eq!(doc.apply(&edits), Ok(vec![None, Some(0xFFFF)]));
        let saved = doc.save();

        let mut session = Session::open().expect("Session::open");
        session.push("a.dxf", &src).expect("push");
        session_call(
            &mut session,
            r#"{"v":1,"op":"open","input":"a.dxf","doc":"a.dxf"}"#,
        );
        let apply_resp = session_call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[
                    {{"kind":"remove","id":"space/0/h31","hash":"{h31_hash}"}},
                    {{"kind":"add","place":"space/0","layer":"0","geometry":{{"shape":"line","start":[3.0,3.0],"end":[4.0,5.0]}}}}
                ]}}"#
            ),
        );
        let today_bytes = take_file(&mut session, &apply_resp);
        assert_eq!(saved, today_bytes);

        assert!(doc.undo());
        assert_eq!(doc.save(), src);
        assert!(doc.redo());
        assert_eq!(doc.save(), today_bytes);
    }

    #[test]
    fn two_entities_sections_insert_at_first_endsec_and_keep_element_order() {
        let src = format!(
            "{HEADER}{TABLES}\
             \x20 0\nSECTION\n  2\nENTITIES\n{LINE}  0\nENDSEC\n\
             \x20 0\nSECTION\n  2\nENTITIES\n{CIRCLE}  0\nENDSEC\n\
             \x20 0\nEOF\n"
        )
        .into_bytes();

        let mut doc = Document::open(src.clone(), 1).expect("2 セクションを開く");
        assert_eq!(doc.entities.len(), 2);
        let add = Edit::Add(Box::new(Add {
            layer: Some("0".to_string()),
            color: None,
            line_type: None,
            line_weight: None,
            shape: NewShape::Line {
                start: [7.0, 7.0],
                end: [8.0, 8.0],
            },
        }));
        assert_eq!(doc.apply(&[add]), Ok(vec![Some(0xFFFF)]));
        let saved = doc.save();

        let mut session = Session::open().expect("Session::open");
        session.push("a.dxf", &src).expect("push");
        session_call(
            &mut session,
            r#"{"v":1,"op":"open","input":"a.dxf","doc":"a.dxf"}"#,
        );
        let apply_resp = session_call(
            &mut session,
            r#"{"v":1,"op":"apply","doc":"a.dxf","edits":[
                {"kind":"add","place":"space/0","layer":"0","geometry":{"shape":"line","start":[7.0,7.0],"end":[8.0,8.0]}}
            ]}"#,
        );
        let today_bytes = take_file(&mut session, &apply_resp);
        assert_eq!(saved, today_bytes);

        let ids: Vec<String> = inspect::live_elements(&doc)
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(ids, ["space/0/h2A", "space/0/hFFFF", "space/0/h2B"]);
    }

    #[test]
    fn insert_xclip_and_multiline_attrib_hashes_bounds_and_diagnostics_match_today() {
        let blocks = "  0\nSECTION\n  2\nBLOCKS\n\
                      \x20 0\nBLOCK\n  5\n20\n  8\n0\n  2\nCLIPBLK\n 70\n0\n 10\n0.0\n 20\n0.0\n\
                      \x20 0\nLINE\n  5\n21\n  8\n0\n 10\n0.0\n 20\n0.0\n 11\n10.0\n 21\n10.0\n\
                      \x20 0\nENDBLK\n  5\n22\n  8\n0\n  0\nENDSEC\n";
        let identity = "\
             40\n1.0\n 40\n0.0\n 40\n0.0\n 40\n0.0\n\
             40\n0.0\n 40\n1.0\n 40\n0.0\n 40\n0.0\n\
             40\n0.0\n 40\n0.0\n 40\n1.0\n 40\n0.0\n";
        let entities = "\
              \x20 0\nINSERT\n  5\n40\n102\n{ACAD_XDICTIONARY\n360\n50\n102\n}\n  8\n0\n 66\n1\n  2\nCLIPBLK\n 10\n2.0\n 20\n3.0\n\
              \x20 0\nATTRIB\n  5\n41\n330\n40\n  8\n0\n 10\n2.5\n 20\n3.5\n 40\n0.5\n  1\nA\\PB\n  2\nTAG\n 70\n0\n\
              101\nEmbedded Object\n 10\n12.0\n 20\n15.0\n 40\n0.5\n  1\nA\\PB\n\
              \x20 0\nSEQEND\n  5\n42\n330\n40\n  8\n0\n\
              \x20 0\nINSERT\n  5\n43\n102\n{ACAD_XDICTIONARY\n360\n60\n102\n}\n  8\n0\n  2\nCLIPBLK\n 10\n20.0\n 20\n30.0\n";
        let objects = format!(
            "\
              \x20 0\nSECTION\n  2\nOBJECTS\n\
              \x20 0\nDICTIONARY\n  5\n50\n330\n40\n  3\nACAD_FILTER\n360\n51\n\
              \x20 0\nDICTIONARY\n  5\n51\n330\n50\n  3\nSPATIAL\n360\n52\n\
              \x20 0\nSPATIAL_FILTER\n  5\n52\n330\n51\n100\nAcDbFilter\n100\nAcDbSpatialFilter\n\
               70\n2\n 10\n0.0\n 20\n0.0\n 10\n5.0\n 20\n5.0\n\
              210\n0.0\n220\n0.0\n230\n1.0\n 11\n0.0\n 21\n0.0\n 31\n0.0\n\
               71\n1\n 72\n0\n 73\n0\n{identity}{identity}\
              \x20 0\nDICTIONARY\n  5\n60\n330\n43\n  3\nACAD_FILTER\n360\n61\n\
              \x20 0\nDICTIONARY\n  5\n61\n330\n60\n  3\nSPATIAL\n360\n62\n\
              \x20 0\nSPATIAL_FILTER\n  5\n62\n330\n61\n100\nAcDbFilter\n100\nAcDbSpatialFilter\n\
               70\n1\n\
              \x20 0\nENDSEC\n"
        );
        let src = format!(
            "{HEADER}{TABLES}{blocks}  0\nSECTION\n  2\nENTITIES\n{entities}  0\nENDSEC\n{objects}  0\nEOF\n"
        )
        .into_bytes();

        let doc = Document::open(src.clone(), 1).expect("XCLIP + Embedded Object を開く");
        let mut session = Session::open().expect("Session::open");
        session.push("a.dxf", &src).expect("push");
        let open_resp = session_call(
            &mut session,
            r#"{"v":1,"op":"open","input":"a.dxf","doc":"a.dxf"}"#,
        );
        let ents_resp = session_call(
            &mut session,
            r#"{"v":1,"op":"entities","doc":"a.dxf","place":"space/0","fields":["hash","bbox"]}"#,
        );
        let ents: EntitiesResult = ents_resp
            .result_as()
            .expect("result")
            .expect("EntitiesResult");

        let live_ents = inspect::live_elements(&doc);
        assert_eq!(live_ents.len(), 2);
        assert_eq!(ents.items.len(), 2);
        for (live_item, today_item) in live_ents.iter().zip(&ents.items) {
            assert_eq!(Some(live_item.hash.as_str()), today_item.hash.as_deref());
            assert_eq!(
                Some(Extent {
                    min_x: live_item.bounds[0],
                    min_y: live_item.bounds[1],
                    max_x: live_item.bounds[2],
                    max_y: live_item.bounds[3],
                }),
                today_item.bbox
            );
        }

        let live_counts = inspect::open_diagnostic_counts(&doc);
        assert_eq!(live_counts.get("dxf.clip.dropped"), Some(&1));
        let mut today_counts = BTreeMap::new();
        for d in open_resp.diagnostics.iter().chain(&ents_resp.diagnostics) {
            if live_counts.contains_key(d.code.as_str()) || d.code.starts_with("dxf.") {
                *today_counts.entry(d.code.as_str()).or_insert(0usize) += d.count as usize;
            }
        }
        assert_eq!(live_counts, today_counts);
    }

    #[test]
    fn first_add_after_open_does_not_reallocate_existing_pages() {
        let mut entities = String::new();
        for i in 0..2500u32 {
            let h = 0x100 + i;
            entities.push_str(&format!(
                "  0\nLINE\n  5\n{h:X}\n  8\n0\n 10\n{i}.0\n 20\n0.0\n 11\n{i}.5\n 21\n1.0\n"
            ));
        }
        let mut doc = Document::open(tabled(&entities), 1).expect("2500 要素を開く");
        assert_eq!(doc.rows.pages().len(), 3);
        let rows_p0 = doc.rows.pages()[0].as_ptr();
        let rows_p1 = doc.rows.pages()[1].as_ptr();
        let boxes_p0 = doc.boxes.pages()[0].as_ptr();
        let lines_p0 = doc.shapes.lines.pages()[0].as_ptr();
        let disp_p0 = doc.display.segments.pages()[0].as_ptr();

        let add = Edit::Add(Box::new(Add {
            layer: Some("0".to_string()),
            color: None,
            line_type: None,
            line_weight: None,
            shape: NewShape::Line {
                start: [0.0, 10.0],
                end: [5.0, 10.0],
            },
        }));
        assert_eq!(doc.apply(&[add]), Ok(vec![Some(0xFFFF)]));

        assert_eq!(doc.rows.pages()[0].as_ptr(), rows_p0);
        assert_eq!(doc.rows.pages()[1].as_ptr(), rows_p1);
        assert_eq!(doc.boxes.pages()[0].as_ptr(), boxes_p0);
        assert_eq!(doc.shapes.lines.pages()[0].as_ptr(), lines_p0);
        assert_eq!(doc.display.segments.pages()[0].as_ptr(), disp_p0);
    }
}
