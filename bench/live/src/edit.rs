//! 変更（動かす・除く・属性を変える・足す）と、取り消し・やり直し。
//!
//! 変えた後のレコードの値・参照の除去・タグの差分・ヘッダーの範囲と `$HANDSEED` の更新は、
//! `zumen_acad`（`single`・`db`・`dxf`）の共通関数をそのまま呼んで一括の実装と 1 つに揃える。
//! 文書の全体は読み直さない。変えるレコードのタグを直し、その行の列・表示・箱・索引だけを作り直す。
//! 足す要素は行の終わりに足し、除く要素は行を残して印を付ける。

use std::collections::{BTreeMap, BTreeSet};

use zumen_acad::db::{forget_values, handle_of_values, owner_of_values};
use zumen_acad::dxf::{
    TagEdit, advance_handseed, diff_values, entity_extents, has_newline, merge_extents,
    widen_extents,
};
use zumen_acad::{Data, Value, single};
use zumen_drawing::{Affine, Element, Inherit, LayerId, Style};

use crate::born::{Add, Restyle};
use crate::lift::{
    self, AttribRow, Geom, InsertRow, Kind, Lift, POLY_WIDTHS, Pools, RawTag, TextRow, class,
};
use crate::scan::Span;
use crate::store::{
    Charset, Document, Locked, NO_BOX, OTag, Row, Shapes, Variable, bounds, range_of, tag_value,
    tokenize,
};
use crate::value::{value, value_bytes, value_of};

/// 文書への変更
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    /// 要素を動かす
    Transform {
        /// 要素のハンドル
        handle: u64,
        /// 変換の行列（x' = m0·x + m1·y + m2、y' = m3·x + m4·y + m5）
        matrix: [f64; 6],
    },
    /// 要素を除く
    Remove {
        /// 要素のハンドル
        handle: u64,
    },
    /// 要素の属性（画層・色・線種・線種の尺度・線の太さ）を変える
    Restyle {
        /// 要素のハンドル
        handle: u64,
        /// 変える属性
        style: Restyle,
    },
    /// モデル空間に要素を足す
    Add(Box<Add>),
}

/// 変更を受け付けなかった理由。受け付けなければ、文書は何も変わらない
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decline {
    /// 文書が変更を受け付けない
    Locked(Locked),
    /// ハンドルの要素がない
    Unknown,
    /// 同じ要素を 2 度指した
    Repeated,
    /// 種類を保って変えられないか、レコードに書けない（今の実装も拒む）
    Unsupported,
    /// 書いたレコードを、要素として読み直せない
    Unreadable,
    /// 試作の範囲の外。今の実装は受け付けることがある
    Outside(Outside),
}

/// 試作が範囲の外とする理由
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Outside {
    /// ATTRIB・VERTEX・SEQEND の付き方が、レコードの並びだけでは決まらない図面で、要素を足すか除く
    Attachment,
    /// 画層・線種・文字スタイル・ブロックの名前が、開くときに読んだ名前の表だけでは決まらない
    Names,
    /// 記号の定義が、名前で 1 つに決まらないか、属性の定義を持つ
    Definition,
    /// 試作が読まない符号化の文字を持つ
    Charset,
    /// ENTITIES のセクションが、ちょうど 1 つでない
    Section,
    /// 足したレコードを、試作が要素として読めない
    Unread,
}

impl Outside {
    /// すべての理由
    pub const ALL: [Outside; 6] = [
        Outside::Attachment,
        Outside::Names,
        Outside::Definition,
        Outside::Charset,
        Outside::Section,
        Outside::Unread,
    ];

    /// 記録に書く名前
    pub fn name(self) -> &'static str {
        match self {
            Outside::Attachment => "attachment",
            Outside::Names => "names",
            Outside::Definition => "definition",
            Outside::Charset => "charset",
            Outside::Section => "section",
            Outside::Unread => "unread",
        }
    }
}

/// 行った操作。取り消しとやり直しに使う
#[derive(Clone, Debug, PartialEq)]
enum TagOp {
    /// 値を置き換えた。`old` は、前に変えていた値
    Set {
        at: usize,
        old: Option<Box<[u8]>>,
        new: Box<[u8]>,
    },
    /// タグを足した
    Insert {
        at: usize,
        code: i16,
        value: Box<[u8]>,
    },
    /// タグを除いた
    Remove { at: usize, tag: OTag },
}

/// 1 つのレコードへの変更
#[derive(Clone, Debug, PartialEq)]
struct Change {
    record: u32,
    /// この変更で、レコードをタグの列に変えたか
    created: bool,
    ops: Vec<TagOp>,
}

/// 足したレコード
#[derive(Clone, Debug, PartialEq)]
struct Fresh {
    /// レコードの番号。元のレコードの数から先
    record: u32,
    /// 取り消している間だけ持つタグ。行があるときは、文書の `overlay` が持つ
    tags: Vec<OTag>,
}

/// 取り消せる 1 回の変更
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    /// タグを変えたレコード。HEADER のレコードも含む
    changes: Vec<Change>,
    /// 列・表示・箱・索引を作り直す行
    rows: Vec<u32>,
    /// 除いた行
    removed: Vec<u32>,
    /// 一緒に除いた OBJECTS のレコード
    gone_objects: Vec<u32>,
    /// 足したレコード。足した順
    added: Vec<Fresh>,
    /// 生きている元の行のハンドルの最大。変更の前と後
    row_largest: (Option<u64>, Option<u64>),
}

/// 足すタグ
fn fresh(code: i16, value: Box<[u8]>) -> OTag {
    OTag {
        code,
        span: Span {
            code,
            start: 0,
            value: 0,
            value_end: 0,
            end: 0,
        },
        fresh: true,
        edited: Some(value),
    }
}

/// 操作を行う。今の実装のトランザクションと同じく、レコードの最初のタグ・ないタグ・グループ・コード 0 は扱わない
fn perform(tags: &mut Vec<OTag>, op: TagEdit) -> Option<TagOp> {
    match op {
        TagEdit::Set { at, value } => {
            if at == 0 || has_newline(&value) {
                return None;
            }
            let tag = tags.get_mut(at).filter(|tag| tag.code != 0)?;
            let new: Box<[u8]> = value.into_boxed_slice();
            let old = tag.edited.replace(new.clone());
            Some(TagOp::Set { at, old, new })
        }
        TagEdit::Remove { at } => {
            if at == 0 || at >= tags.len() {
                return None;
            }
            Some(TagOp::Remove {
                at,
                tag: tags.remove(at),
            })
        }
        TagEdit::Insert { at, code, value } => {
            if at == 0 || at > tags.len() || code == 0 || has_newline(&value) {
                return None;
            }
            let value: Box<[u8]> = value.into_boxed_slice();
            tags.insert(at, fresh(code, value.clone()));
            Some(TagOp::Insert { at, code, value })
        }
    }
}

/// 行った操作を、もう一度行う
fn forward(tags: &mut Vec<OTag>, op: &TagOp) {
    match op {
        TagOp::Set { at, new, .. } => {
            if let Some(tag) = tags.get_mut(*at) {
                tag.edited = Some(new.clone());
            }
        }
        TagOp::Insert { at, code, value } => {
            if *at <= tags.len() {
                tags.insert(*at, fresh(*code, value.clone()));
            }
        }
        TagOp::Remove { at, .. } => {
            if *at < tags.len() {
                tags.remove(*at);
            }
        }
    }
}

/// 行った操作を、元に戻す
fn backward(tags: &mut Vec<OTag>, op: &TagOp) {
    match op {
        TagOp::Set { at, old, .. } => {
            if let Some(tag) = tags.get_mut(*at) {
                tag.edited = old.clone();
            }
        }
        TagOp::Insert { at, .. } => {
            if *at < tags.len() {
                tags.remove(*at);
            }
        }
        TagOp::Remove { at, tag } => {
            if *at <= tags.len() {
                tags.insert(*at, tag.clone());
            }
        }
    }
}

/// 足すレコードのタグ。値の行に書けない値があれば `None`
fn record_tags(kind: &[u8], values: &[Value]) -> Option<Vec<OTag>> {
    let mut tags = Vec::with_capacity(values.len() + 1);
    tags.push(fresh(0, kind.into()));
    for value in values {
        let bytes = value_bytes(&value.data)?;
        if value.code == 0 || has_newline(&bytes) {
            return None;
        }
        tags.push(fresh(value.code, bytes.into_boxed_slice()));
    }
    Some(tags)
}

/// 読み直した頂点（と頂点ごとの幅）を、同じ場所に書く。場所がなければ、何も変えずに `false`
fn overwrite(
    pools: &mut Pools,
    first: u32,
    vertices: &[[f64; 3]],
    widths: Option<&[[f64; 2]]>,
) -> bool {
    let range = first as usize..first as usize + vertices.len();
    if pools.vertices.get(range.clone()).is_none() {
        return false;
    }
    if let Some(widths) = widths {
        let place = pools.widths_mut(first, vertices.len() as u32);
        if place.len() != widths.len() {
            return false;
        }
        place.copy_from_slice(widths);
    }
    if let Some(place) = pools.vertices.get_mut(range) {
        place.copy_from_slice(vertices);
    }
    true
}

/// 読み直した形を、行の形の場所に置く。置いた形（折れ線と文字は、文書の置き場を指すもの）を返す。
/// 動かすだけなら、頂点の数も文字の中身も変わらないので、置き場は増えない
fn store_geom(shapes: &mut Shapes, at: u32, geom: &Geom, scratch: &Pools) -> Option<Geom> {
    let at = at as usize;
    Some(match *geom {
        Geom::Line(line) => {
            *shapes.lines.get_mut(at)? = line;
            Geom::Line(line)
        }
        Geom::Circle(circle) => {
            *shapes.circles.get_mut(at)? = circle;
            Geom::Circle(circle)
        }
        Geom::Arc(arc) => {
            *shapes.arcs.get_mut(at)? = arc;
            Geom::Arc(arc)
        }
        Geom::Polyline {
            first,
            count,
            flags,
            width,
        } => {
            let vertices = scratch
                .vertices
                .get(first as usize..first as usize + count as usize)?;
            let wide = flags & POLY_WIDTHS != 0;
            let widths = scratch.widths_of(first, count);
            if wide && widths.len() != vertices.len() {
                return None;
            }
            let pools = &mut shapes.pools;
            let poly = shapes.polys.get_mut(at)?;
            let same = poly.count == count && (poly.flags & POLY_WIDTHS != 0) == wide;
            if !(same && overwrite(pools, poly.first, vertices, wide.then_some(widths))) {
                // 頂点の数か、幅を持つかが変わった。後ろに置き直す（前の場所は使わなくなる）
                let start = u32::try_from(pools.vertices.len()).ok()?;
                let width_start = u32::try_from(pools.widths.len()).ok()?;
                pools.vertices.extend_from_slice(vertices);
                if wide {
                    pools.width_first.push((start, width_start));
                    pools.widths.extend_from_slice(widths);
                }
                poly.first = start;
            }
            poly.count = count;
            poly.flags = flags;
            poly.width = width;
            Geom::Polyline {
                first: poly.first,
                count,
                flags,
                width,
            }
        }
        Geom::Text(text) => {
            let old = *shapes.texts.get(at)?;
            let pools = &mut shapes.pools;
            let content = scratch.text_of(text.content);
            let content = if pools.text_of(old.content) == content {
                old.content
            } else {
                pools.push_text(content)?
            };
            let style = match (text.style, old.style) {
                (None, _) => None,
                (Some(range), Some(kept)) if pools.text_of(kept) == scratch.text_of(range) => {
                    Some(kept)
                }
                (Some(range), _) => Some(pools.push_text(scratch.text_of(range))?),
            };
            let stored = TextRow {
                content,
                style,
                ..text
            };
            *shapes.texts.get_mut(at)? = stored;
            Geom::Text(stored)
        }
        Geom::Insert(insert) => {
            // 定義と、付いた ATTRIB の置き場は、開いたときのまま
            let old = *shapes.inserts.get(at)?;
            let pools = &mut shapes.pools;
            let name = if pools.text_of(old.name) == scratch.text_of(insert.name) {
                old.name
            } else {
                pools.push_text(scratch.text_of(insert.name))?
            };
            let stored = InsertRow {
                name,
                definition: old.definition,
                attribs: old.attribs,
                attached: old.attached,
                ..insert
            };
            *shapes.inserts.get_mut(at)? = stored;
            Geom::Insert(stored)
        }
    })
}

impl Document {
    /// 変更を当てる。変更は、すべて受け付けるか、すべて受け付けないかのどちらか。
    /// 受け付けた 1 回の変更は、1 回の取り消しで戻る。変更ごとに、足した要素のハンドルを返す（ほかの変更は `None`）
    pub fn apply(&mut self, edits: &[Edit]) -> Result<Vec<Option<u64>>, Decline> {
        if let Some(locked) = self.locked {
            return Err(Decline::Locked(locked));
        }
        // 足す要素のハンドルは、変更の前の、ハンドルの最大と種から決める。足す変更があるときだけ求める
        let mut next: Option<Option<u64>> = None;
        // 先に、すべての変更を決める。ここでは文書を変えない。レコードごとに、変更の前の値と後の値を持つ
        let mut values: BTreeMap<u32, (Vec<Value>, Vec<Value>)> = BTreeMap::new();
        let mut targets: Vec<u32> = Vec::new();
        let mut rows: Vec<u32> = Vec::new();
        let mut removed: Vec<u32> = Vec::new();
        let mut owned_removed: Vec<(u32, u64)> = Vec::new();
        let mut born: Vec<(&'static [u8], Vec<Value>)> = Vec::new();
        let mut handles: Vec<Option<u64>> = Vec::with_capacity(edits.len());
        for edit in edits {
            let mut added = None;
            match edit {
                Edit::Transform { handle, matrix } => {
                    let at = self.row_of(*handle).ok_or(Decline::Unknown)?;
                    self.plan_move(at, &Affine { m: *matrix }, &mut values)?;
                    targets.push(at);
                    rows.push(at);
                }
                Edit::Restyle { handle, style } => {
                    let at = self.row_of(*handle).ok_or(Decline::Unknown)?;
                    let row = self.rows.get(at as usize).ok_or(Decline::Unknown)?;
                    // 属性を変えた行は、付いた ATTRIB も読み直す
                    if self.foreign.contains(&row.record) {
                        return Err(Decline::Outside(Outside::Charset));
                    }
                    let layer = self
                        .layers
                        .list
                        .get(row.layer as usize)
                        .ok_or(Decline::Unsupported)?;
                    let old = self.values(row.record);
                    let new = self.restyled(row.kind.dxf_name(), &layer.name, &old, style)?;
                    values.insert(row.record, (old, new));
                    targets.push(at);
                    rows.push(at);
                }
                Edit::Remove { handle } => {
                    let at = self.row_of(*handle).ok_or(Decline::Unknown)?;
                    // 今の実装は、ATTRIB が続く INSERT を除かない
                    if let Some(Geom::Insert(insert)) = self.geom(at)
                        && insert.followed
                    {
                        return Err(Decline::Unsupported);
                    }
                    if !self.plain {
                        return Err(Decline::Outside(Outside::Attachment));
                    }
                    let extra = self.plan_remove(at)?;
                    targets.push(at);
                    removed.push(at);
                    owned_removed.extend(extra);
                }
                Edit::Add(add) => {
                    if !self.plain {
                        return Err(Decline::Outside(Outside::Attachment));
                    }
                    if self.entities_end.is_none() {
                        return Err(Decline::Outside(Outside::Section));
                    }
                    let handle = next
                        .get_or_insert_with(|| self.next_handle())
                        .and_then(|next| next.checked_add(born.len() as u64))
                        .filter(|handle| handle.checked_add(1).is_some())
                        .ok_or(Decline::Unsupported)?;
                    born.push(self.born(add, handle)?);
                    added = Some(handle);
                }
            }
            handles.push(added);
        }
        targets.sort_unstable();
        if targets.windows(2).any(|pair| pair.first() == pair.get(1)) {
            return Err(Decline::Repeated);
        }

        // 除く要素と、一緒に除く拡張辞書の木（#881）への参照を、残るレコードから除く
        removed.sort_unstable();
        owned_removed.sort_unstable();
        owned_removed.dedup();
        let mut leaving_items: Vec<(u32, u64)> = removed
            .iter()
            .filter_map(|at| self.rows.get(*at as usize))
            .map(|row| (row.record, row.handle))
            .chain(owned_removed.iter().copied())
            .collect();
        leaving_items.sort_unstable();
        leaving_items.dedup();
        let leaving: BTreeSet<u32> = leaving_items.iter().map(|&(record, _)| record).collect();
        for &(_, handle) in leaving_items.iter().rev() {
            if handle == 0 {
                continue;
            }
            let from = self.refs.partition_point(|(target, _)| *target < handle);
            let pointing = self
                .refs
                .iter()
                .skip(from)
                .take_while(|(target, _)| *target == handle);
            for (_, record) in pointing {
                if self.gone.contains(record) || leaving.contains(record) {
                    continue;
                }
                let kind = self.kind_name(*record);
                match values.get_mut(record) {
                    Some((_, new)) => *new = forget_values(kind, core::mem::take(new), handle),
                    None => {
                        let old = self.values(*record);
                        let new = forget_values(kind, old.clone(), handle);
                        if new != old {
                            values.insert(*record, (old, new));
                        }
                    }
                }
            }
        }

        // 値が変わるレコードの、タグの操作と、図面の範囲に足す箱を決める。レコードは、ファイルの順に並ぶ
        let mut found = None;
        let mut plans: Vec<(u32, Vec<TagEdit>)> = Vec::with_capacity(values.len());
        for (record, (old, new)) in &values {
            if new == old {
                continue;
            }
            let ops = diff_values(1, old, new).map_err(|_| Decline::Unsupported)?;
            let kind = self.kind_name(*record);
            found = merge_extents(found, entity_extents(kind, self.in_entities(*record), new));
            plans.push((*record, ops));
        }
        for (kind, new) in &born {
            found = merge_extents(found, entity_extents(kind, true, new));
        }

        let gone_objects: Vec<u32> = owned_removed
            .into_iter()
            .map(|(record, _)| record)
            .collect();
        let before = self.row_largest;
        let mut step = Step {
            changes: Vec::with_capacity(plans.len() + 1),
            rows,
            removed: Vec::with_capacity(removed.len()),
            gone_objects: Vec::with_capacity(gone_objects.len()),
            added: Vec::with_capacity(born.len()),
            row_largest: (before, before),
        };
        if let Err(decline) = self.carry_out(&mut step, plans, &removed, &gone_objects, born, found)
        {
            self.revert(&mut step);
            return Err(decline);
        }
        step.row_largest.1 = self.row_largest;
        self.undo.push(step);
        self.redo.clear();
        Ok(handles)
    }

    /// 決めた変更を行う。途中で行えなくなったら、そこまでに行ったことを `step` に残して、理由を返す
    fn carry_out(
        &mut self,
        step: &mut Step,
        plans: Vec<(u32, Vec<TagEdit>)>,
        removed: &[u32],
        gone_objects: &[u32],
        born: Vec<(&'static [u8], Vec<Value>)>,
        found: Option<[[f64; 3]; 2]>,
    ) -> Result<(), Decline> {
        for (record, ops) in plans {
            let change = self.rewrite(record, ops).ok_or(Decline::Unsupported)?;
            step.changes.push(change);
        }
        let mut scratch = Pools::default();
        for &at in &step.rows {
            if !self.refresh_row(at, &mut scratch) {
                return Err(Decline::Unreadable);
            }
        }
        for &at in removed {
            self.kill(at, true);
            step.removed.push(at);
        }
        for &record in gone_objects {
            if self.gone.insert(record) {
                step.gone_objects.push(record);
            }
        }
        for (kind, values) in born {
            let tags = record_tags(kind, &values).ok_or(Decline::Unsupported)?;
            let record = self.next_record().ok_or(Decline::Unsupported)?;
            if !self.push_row(record, tags) {
                return Err(Decline::Outside(Outside::Unread));
            }
            step.added.push(Fresh {
                record,
                tags: Vec::new(),
            });
        }
        // ヘッダーの変数は、除いて足した後のハンドルの最大から直す
        if let Some((record, ops)) = self.header_ops(found)? {
            let change = self.rewrite(record, ops).ok_or(Decline::Unsupported)?;
            step.changes.push(change);
        }
        Ok(())
    }

    /// 直前の変更を取り消す。取り消す変更がなければ `false`
    pub fn undo(&mut self) -> bool {
        let Some(mut step) = self.undo.pop() else {
            return false;
        };
        self.revert(&mut step);
        self.redo.push(step);
        true
    }

    /// 取り消した変更をやり直す。やり直す変更がなければ `false`
    pub fn redo(&mut self) -> bool {
        let Some(mut step) = self.redo.pop() else {
            return false;
        };
        self.replay(&mut step);
        self.undo.push(step);
        true
    }

    /// 変えたレコードのタグと、取り消し・やり直しの列の、およそのバイト数
    pub fn edit_bytes(&self) -> usize {
        let held = |value: &Option<Box<[u8]>>| value.as_ref().map_or(0, |value| value.len());
        let tags = |tags: &[OTag]| -> usize {
            tags.iter()
                .map(|tag| core::mem::size_of::<OTag>() + held(&tag.edited))
                .sum()
        };
        let overlay: usize = self.overlay.values().map(|record| tags(record)).sum();
        let op = |op: &TagOp| {
            core::mem::size_of::<TagOp>()
                + match op {
                    TagOp::Set { old, new, .. } => held(old) + new.len(),
                    TagOp::Insert { value, .. } => value.len(),
                    TagOp::Remove { tag, .. } => held(&tag.edited),
                }
        };
        let steps = |steps: &[Step]| -> usize {
            steps
                .iter()
                .map(|step| {
                    let changes: usize = step
                        .changes
                        .iter()
                        .map(|change| {
                            core::mem::size_of::<Change>()
                                + change.ops.iter().map(op).sum::<usize>()
                        })
                        .sum();
                    let added: usize = step
                        .added
                        .iter()
                        .map(|fresh| core::mem::size_of::<Fresh>() + tags(&fresh.tags))
                        .sum();
                    core::mem::size_of::<Step>()
                        + changes
                        + added
                        + (step.rows.len() + step.removed.len() + step.gone_objects.len()) * 4
                })
                .sum()
        };
        overlay + steps(&self.undo) + steps(&self.redo)
    }

    /// レコードの、グループ・コード 0 の次からの値
    fn values(&self, record: u32) -> Vec<Value> {
        let kind = self.kind_name(record);
        let read = |tags: &[OTag]| -> Vec<Value> {
            tags.iter()
                .skip(1)
                .map(|tag| value_of(kind, tag.code, tag_value(&self.src, tag)))
                .collect()
        };
        match self.overlay.get(&record) {
            Some(tags) => read(tags),
            None => read(&tokenize(&self.src, self.record_range(record))),
        }
    }

    /// ヘッダーの変数の、今の値
    fn variable_values(&self, record: u32, variable: &Variable) -> Vec<Value> {
        match self.overlay.get(&record) {
            Some(tags) => tags
                .iter()
                .skip(variable.start)
                .take(variable.values.len())
                .map(|tag| value(tag.code, tag_value(&self.src, tag)))
                .collect(),
            None => variable.values.clone(),
        }
    }

    /// ハンドルの種（`$HANDSEED`）の、変数の値の中の場所と、今の値
    fn seed(&self) -> Option<(usize, u64)> {
        let header = self.header.as_ref()?;
        let variable = header.seed.as_ref()?;
        self.variable_values(header.record, variable)
            .iter()
            .enumerate()
            .find_map(|(at, value)| match value.data {
                Data::Handle(seed) if value.code == 5 => Some((at, seed)),
                _ => None,
            })
    }

    /// 次に足す要素のハンドル（`zumen_acad::single::next_handle`）。ハンドルの最大の次と、種の、大きいほう。
    /// ハンドルも種もない図面には足せない
    fn next_handle(&self) -> Option<u64> {
        let seed = self.seed().map(|(_, seed)| seed);
        single::next_handle(self.largest_handle(), seed)
    }

    /// 次に足すレコードの番号。元のレコードの数から先を、足した順に使う
    fn next_record(&self) -> Option<u32> {
        let first = self.source_records();
        match self.overlay.last_key_value() {
            Some((last, _)) if *last >= first => last.checked_add(1),
            _ => Some(first),
        }
    }

    /// 行を除くときに一緒に除く OBJECTS のレコードとハンドルを調べる（#881・`entity_removal`）
    fn plan_remove(&self, at: u32) -> Result<Vec<(u32, u64)>, Decline> {
        let row = self.rows.get(at as usize).ok_or(Decline::Unknown)?;
        let unedited = !self.overlay.contains_key(&row.record);
        let has_102_or_360 = self
            .flags
            .get(row.record as usize)
            .is_some_and(|&flag| flag & 2 != 0);
        if unedited && !has_102_or_360 {
            return Ok(Vec::new());
        }
        let kind = row.kind.dxf_name();
        let old = self.values(row.record);
        if single::is_followed(kind, &old) {
            return Err(Decline::Unsupported);
        }
        if !old.iter().any(|v| matches!(v.code, 102 | 360)) {
            return Ok(Vec::new());
        }
        if row.handle == 0 {
            return Err(Decline::Unsupported);
        }
        let mut next = single::owned_children(kind, &old, true).ok_or(Decline::Unsupported)?;
        let mut parents: Vec<u64> = vec![row.handle; next.len()];
        let mut seen = BTreeSet::from([row.handle]);
        let mut owned: BTreeMap<u32, u64> = BTreeMap::new();
        while let (Some((child, dictionary)), Some(parent)) = (next.pop(), parents.pop()) {
            if !seen.insert(child) {
                return Err(Decline::Unsupported);
            }
            let Some(&record) = self.objects_by_handle.get(&child) else {
                return Err(Decline::Unsupported);
            };
            if self.gone.contains(&record) {
                return Err(Decline::Unsupported);
            }
            let child_kind = self.kind_name(record);
            if dictionary && child_kind != b"DICTIONARY" {
                return Err(Decline::Unsupported);
            }
            let child_values = self.values(record);
            if handle_of_values(child_kind, &child_values) != Some(child)
                || owner_of_values(&child_values) != Some(parent)
            {
                return Err(Decline::Unsupported);
            }
            let more = single::owned_children(child_kind, &child_values, false)
                .ok_or(Decline::Unsupported)?;
            parents.extend(core::iter::repeat_n(child, more.len()));
            next.extend(more);
            owned.insert(record, child);
        }
        let handles: BTreeSet<u64> = owned.values().copied().collect();
        let skip: BTreeSet<u32> = owned.keys().copied().chain([row.record]).collect();
        if self.pointed(&skip, &handles) {
            return Err(Decline::Unsupported);
        }
        Ok(owned.into_iter().collect())
    }

    /// `skip` 以外の残っているオブジェクトかヘッダー変数が、`handles` のどれかを指すか（`owned::pointed`）
    fn pointed(&self, skip: &BTreeSet<u32>, handles: &BTreeSet<u64>) -> bool {
        if handles.is_empty() {
            return false;
        }
        if handles.iter().any(|h| self.header_handles.contains(h)) {
            return true;
        }
        let header_record = self.header.as_ref().map(|h| h.record);
        if let Some(header) = &self.header
            && let Some(seed) = &header.seed
            && self
                .variable_values(header.record, seed)
                .iter()
                .any(|v| matches!(v.data, Data::Handle(h) if handles.contains(&h)))
        {
            return true;
        }
        for &record in self.overlay.keys() {
            if Some(record) == header_record
                || self.gone.contains(&record)
                || skip.contains(&record)
                || !self.in_section(record)
            {
                continue;
            }
            if self
                .values(record)
                .iter()
                .any(|v| matches!(v.data, Data::Handle(h) if handles.contains(&h)))
            {
                return true;
            }
        }
        for &handle in handles {
            let from = self
                .object_refs
                .partition_point(|&(target, _)| target < handle);
            let hit = self
                .object_refs
                .iter()
                .skip(from)
                .take_while(|&&(target, _)| target == handle)
                .any(|&(_, record)| {
                    !self.gone.contains(&record)
                        && !skip.contains(&record)
                        && !self.overlay.contains_key(&record)
                });
            if hit {
                return true;
            }
        }
        false
    }

    /// 行を `transform` で動かした後の、レコードの値を決める。記号は、付いた ATTRIB のレコードも動かす
    fn plan_move(
        &self,
        at: u32,
        transform: &Affine,
        out: &mut BTreeMap<u32, (Vec<Value>, Vec<Value>)>,
    ) -> Result<(), Decline> {
        const UNSUPPORTED: Decline = Decline::Unsupported;
        let row = self.rows.get(at as usize).ok_or(Decline::Unknown)?;
        let geom = self.shapes.geom(row.kind, row.geom).ok_or(UNSUPPORTED)?;
        let attrib_records: Vec<u32> = match &geom {
            Geom::Insert(insert) => {
                if self.foreign.contains(&row.record) {
                    return Err(Decline::Outside(Outside::Charset));
                }
                // 今の実装は、付いた ATTRIB のうち、属性として読めないものがある記号を動かさない
                if insert.attached != insert.attribs.1 {
                    return Err(UNSUPPORTED);
                }
                self.shapes
                    .pools
                    .attribs_of(insert)
                    .iter()
                    .map(|attrib| attrib.record)
                    .collect()
            }
            _ => Vec::new(),
        };
        let shape = lift::shape(&geom, &self.shapes.pools).ok_or(UNSUPPORTED)?;
        let element = Element {
            layer: LayerId(row.layer),
            style: Style {
                color: Inherit::Layer,
                line_type: Inherit::Layer,
                line_type_scale: 1.0,
                line_weight: None,
                transparency: Inherit::Layer,
            },
            shape,
            source: Default::default(),
        };
        let old = self.values(row.record);
        let old_attribs: Vec<Vec<Value>> = attrib_records
            .iter()
            .map(|&record| self.values(record))
            .collect();
        let attrib_slices: Vec<&[Value]> = old_attribs.iter().map(Vec::as_slice).collect();
        let (new, new_attribs) = single::move_model(
            &element,
            row.kind.dxf_name(),
            &old,
            &attrib_slices,
            transform,
            self.charset,
        )
        .ok_or(UNSUPPORTED)?;
        out.insert(row.record, (old, new));
        for ((record, old_val), new_val) in
            attrib_records.into_iter().zip(old_attribs).zip(new_attribs)
        {
            out.insert(record, (old_val, new_val));
        }
        Ok(())
    }

    /// ヘッダーの変数を直す操作（`zumen_acad::dxf::widen_extents`・`advance_handseed`）
    fn header_ops(
        &self,
        found: Option<[[f64; 3]; 2]>,
    ) -> Result<Option<(u32, Vec<TagEdit>)>, Decline> {
        let Some(header) = &self.header else {
            return Ok(None);
        };
        let mut updates: Vec<(usize, Vec<Value>, Vec<Value>)> = Vec::new();
        if let Some(found) = found {
            for (variable, is_max) in [(&header.extmin, false), (&header.extmax, true)] {
                let Some(variable) = variable else {
                    continue;
                };
                let old = self.variable_values(header.record, variable);
                if let Some(new) = widen_extents(is_max, &old, found) {
                    updates.push((variable.start, old, new));
                }
            }
        }
        if let Some(variable) = &header.seed
            && let Some(largest) = self.largest_handle()
        {
            let old = self.variable_values(header.record, variable);
            if let Some(new) = advance_handseed(&old, largest) {
                updates.push((variable.start, old, new));
            }
        }
        updates.sort_by_key(|&(start, ..)| core::cmp::Reverse(start));
        let mut ops = Vec::new();
        for (start, old, new) in updates {
            ops.extend(diff_values(start, &old, &new).map_err(|_| Decline::Unsupported)?);
        }
        Ok((!ops.is_empty()).then_some((header.record, ops)))
    }

    /// レコードのタグに操作を行う。1 つでも行えなければ、行った分を戻して `None`
    fn rewrite(&mut self, record: u32, ops: Vec<TagEdit>) -> Option<Change> {
        let created = !self.overlay.contains_key(&record);
        let range = range_of(&self.starts, record);
        let src = &self.src;
        let tags = self
            .overlay
            .entry(record)
            .or_insert_with(|| tokenize(src, range));
        let mut done = Vec::with_capacity(ops.len());
        for op in ops {
            let Some(op) = perform(tags, op) else {
                for op in done.iter().rev() {
                    backward(tags, op);
                }
                if created {
                    self.overlay.remove(&record);
                }
                return None;
            };
            done.push(op);
        }
        Some(Change {
            record,
            created,
            ops: done,
        })
    }

    /// 行を除く。行と形の場所は残し、表示・箱・索引・ハンドルの表から外す。
    /// `lower` なら、生きている元の行のハンドルの最大を、たどり直す
    fn kill(&mut self, at: u32, lower: bool) {
        let Some(row) = self.rows.get_mut(at as usize) else {
            return;
        };
        row.alive = false;
        let (handle, record) = (row.handle, row.record);
        self.gone.insert(record);
        self.display.hide(at);
        if let Some(place) = self.boxes.get_mut(at as usize) {
            *place = NO_BOX;
        }
        self.grid.update(at, NO_BOX);
        if self.added.get(&handle) == Some(&at) {
            self.added.remove(&handle);
        } else if lower && self.row_largest == Some(handle) {
            self.lower_row_largest();
        }
    }

    /// 除いた行を戻す。生きている元の行のハンドルの最大は、呼ぶ側が戻す
    fn revive(&mut self, at: u32) {
        let Some(row) = self.rows.get_mut(at as usize) else {
            return;
        };
        row.alive = true;
        let row = *row;
        self.gone.remove(&row.record);
        self.display.show(at);
        if let Some(geom) = self.shapes.geom(row.kind, row.geom)
            && let Some(place) = self.boxes.get_mut(at as usize)
        {
            *place = bounds(&self.shapes.pools, &self.definitions, &geom);
            self.grid.update(at, *place);
        }
        if row.record >= self.source_records() {
            self.added.insert(row.handle, at);
        }
    }

    /// 足したレコードを、行の終わりに足す。開くときと同じ読み方（`lift`）を通す。
    /// 要素として読めなければ、何も足さずに `false`
    fn push_row(&mut self, record: u32, tags: Vec<OTag>) -> bool {
        let src: &[u8] = &self.src;
        let Some(kind) = tags
            .first()
            .and_then(|tag| Kind::of(tag_value(src, tag).trim_ascii()))
        else {
            return false;
        };
        let marks = self.shapes.pools.marks();
        let read = {
            let raw: Vec<RawTag<'_>> = tags
                .iter()
                .skip(1)
                .map(|tag| RawTag {
                    code: tag.code,
                    value: tag_value(src, tag),
                })
                .collect();
            match lift::lift(kind, &raw, &mut self.shapes.pools) {
                Lift::Element(lifted) => {
                    let readable = lifted.class == class::ASCII
                        || (lifted.class == class::UTF8 && self.charset == Charset::Utf8);
                    match lifted.handle {
                        Some(handle) if readable && handle != 0 => Some((
                            lifted.geom,
                            handle,
                            self.layers.intern(lifted.layer),
                            lifted.color,
                            lifted.class,
                        )),
                        _ => None,
                    }
                }
                _ => None,
            }
        };
        let Some((geom, handle, layer, color, class)) = read else {
            self.shapes.pools.truncate(marks);
            return false;
        };
        let geom = match geom {
            Geom::Insert(insert) => Geom::Insert(InsertRow {
                definition: self.definition_of(&insert),
                ..insert
            }),
            other => other,
        };
        let Ok(at) = u32::try_from(self.rows.len()) else {
            self.shapes.pools.truncate(marks);
            return false;
        };
        let index = self.shapes.push(geom);
        self.rows.push(Row {
            handle,
            record,
            geom: index,
            layer,
            color,
            kind,
            class,
            alive: true,
        });
        self.display.push(&geom, &self.shapes.pools, [layer, color]);
        let place = bounds(&self.shapes.pools, &self.definitions, &geom);
        self.boxes.push(place);
        self.grid.push(place);
        self.added.insert(handle, at);
        self.overlay.insert(record, tags);
        true
    }

    /// 終わりの行（足した行）を除いて、レコードのタグを返す。頂点と文字の置き場は詰め直さない
    fn pop_row(&mut self, record: u32) -> Vec<OTag> {
        if let Some(row) = self.rows.pop() {
            let at = self.rows.len() as u32;
            self.shapes.pop(row.kind);
            self.display.pop();
            self.boxes.pop();
            self.grid.pop();
            if self.added.get(&row.handle) == Some(&at) {
                self.added.remove(&row.handle);
            }
        }
        self.overlay.remove(&record).unwrap_or_default()
    }

    /// 変更を、後ろから元に戻す。足したレコードのタグは、やり直しのために `step` へ移す
    fn revert(&mut self, step: &mut Step) {
        for fresh in step.added.iter_mut().rev() {
            fresh.tags = self.pop_row(fresh.record);
        }
        for &record in step.gone_objects.iter().rev() {
            self.gone.remove(&record);
        }
        for &at in step.removed.iter().rev() {
            self.revive(at);
        }
        for change in step.changes.iter().rev() {
            if let Some(tags) = self.overlay.get_mut(&change.record) {
                for op in change.ops.iter().rev() {
                    backward(tags, op);
                }
            }
            if change.created {
                self.overlay.remove(&change.record);
            }
        }
        // 記号は、INSERT と ATTRIB のレコードがそろってから読み直す
        let mut scratch = Pools::default();
        for &at in &step.rows {
            self.refresh_row(at, &mut scratch);
        }
        self.row_largest = step.row_largest.0;
    }

    /// 変更を、前からもう一度行う
    fn replay(&mut self, step: &mut Step) {
        for change in &step.changes {
            let range = range_of(&self.starts, change.record);
            let src = &self.src;
            let tags = self
                .overlay
                .entry(change.record)
                .or_insert_with(|| tokenize(src, range));
            for op in &change.ops {
                forward(tags, op);
            }
        }
        let mut scratch = Pools::default();
        for &at in &step.rows {
            self.refresh_row(at, &mut scratch);
        }
        for &at in &step.removed {
            self.kill(at, false);
        }
        for &record in &step.gone_objects {
            self.gone.insert(record);
        }
        for fresh in &mut step.added {
            self.push_row(fresh.record, core::mem::take(&mut fresh.tags));
        }
        self.row_largest = step.row_largest.1;
    }

    /// 記号に付いた ATTRIB を、今のタグから読み直す。文字が前と同じなら、前の置き場を使う
    fn reread_attribs(&mut self, insert: &InsertRow) -> Option<Vec<AttribRow>> {
        let (first, count) = insert.attribs;
        let mut out = Vec::with_capacity(count as usize);
        for index in first..first.checked_add(count)? {
            let old = *self.shapes.pools.attribs.get(index as usize)?;
            let marks = self.shapes.pools.marks();
            let mut new = self.read_attrib(old.record)?;
            let pools = &mut self.shapes.pools;
            let kept = {
                let same = |a: (u32, u32), b: (u32, u32)| pools.text_of(a) == pools.text_of(b);
                let style = match (new.text.style, old.text.style) {
                    (None, None) => true,
                    (Some(a), Some(b)) => same(a, b),
                    _ => false,
                };
                style
                    && same(new.text.content, old.text.content)
                    && same(new.tag, old.tag)
                    && same(new.layer_name, old.layer_name)
            };
            if kept {
                new.text.content = old.text.content;
                new.text.style = old.text.style;
                new.tag = old.tag;
                new.layer_name = old.layer_name;
                pools.truncate(marks);
            }
            out.push(new);
        }
        Some(out)
    }

    /// 行の列・表示・箱・索引を、レコードの今のタグから作り直す。開くときと同じ読み方（`lift`）を通すので、
    /// 開き直した文書と同じ値になる。要素として読めなければ、何も変えずに `false` を返す
    fn refresh_row(&mut self, at: u32, scratch: &mut Pools) -> bool {
        // 記号は、付いた ATTRIB を先に読み直す
        let attribs = match self.geom(at) {
            Some(Geom::Insert(insert)) => match self.reread_attribs(&insert) {
                Some(attribs) => attribs,
                None => return false,
            },
            Some(_) => Vec::new(),
            None => return false,
        };
        let Document {
            src,
            starts,
            rows,
            shapes,
            layers,
            charset,
            overlay,
            display,
            boxes,
            grid,
            definitions,
            ..
        } = self;
        let src: &[u8] = src;
        let Some(row) = rows.get_mut(at as usize) else {
            return false;
        };
        let pristine;
        let tags: &[OTag] = match overlay.get(&row.record) {
            Some(tags) => tags,
            None => {
                pristine = tokenize(src, range_of(starts, row.record));
                &pristine
            }
        };
        let raw: Vec<RawTag<'_>> = tags
            .iter()
            .skip(1)
            .map(|tag| RawTag {
                code: tag.code,
                value: tag_value(src, tag),
            })
            .collect();
        scratch.clear();
        let Lift::Element(lifted) = lift::lift(row.kind, &raw, scratch) else {
            return false;
        };
        let readable = lifted.class == class::ASCII
            || (lifted.class == class::UTF8 && *charset == Charset::Utf8);
        if !readable || lifted.handle.unwrap_or(0) != row.handle {
            return false;
        }
        let Some(geom) = store_geom(shapes, row.geom, &lifted.geom, scratch) else {
            return false;
        };
        if let Geom::Insert(insert) = &geom {
            let places = shapes
                .pools
                .attribs
                .iter_mut()
                .skip(insert.attribs.0 as usize);
            for (place, attrib) in places.zip(attribs) {
                *place = attrib;
            }
        }
        let same_layer = layers
            .list
            .get(row.layer as usize)
            .is_some_and(|layer| layer.name.eq_ignore_ascii_case(lifted.layer));
        if !same_layer {
            row.layer = layers.intern(lifted.layer);
        }
        row.color = lifted.color;
        row.class = lifted.class;
        display.update(at, &geom, &shapes.pools, [row.layer, row.color]);
        if let Some(place) = boxes.get_mut(at as usize) {
            *place = bounds(&shapes.pools, definitions, &geom);
            grid.update(at, *place);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::born::{ColorSpec, LineTypeSpec, NewShape};

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

    /// ハンドルを持つ最小の DXF。`entities` は、ENTITIES のセクションのレコード
    fn dxf(entities: &str) -> Vec<u8> {
        format!("{HEADER}  0\nSECTION\n  2\nENTITIES\n{entities}  0\nENDSEC\n  0\nEOF\n")
            .into_bytes()
    }

    /// 線種と画層の表も持つ DXF
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

    const LINE: &str =
        "  0\nLINE\n  5\n2A\n  8\n0\n 10\n1.0\n 20\n2.0\n 30\n0.0\n 11\n3.0\n 21\n4.0\n 31\n0.0\n";

    const CIRCLE: &str = "  0\nCIRCLE\n  5\n2B\n  8\n0\n 10\n5.0\n 20\n5.0\n 40\n1.5\n";

    fn translation(handle: u64, dx: f64, dy: f64) -> Edit {
        Edit::Transform {
            handle,
            matrix: [1.0, 0.0, dx, 0.0, 1.0, dy],
        }
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
        // 範囲は、広がる向きの値だけが変わる
        assert!(after.contains("$EXTMIN\n 10\n0.0\n 20\n0.0\n"), "{after}");
        assert!(after.contains("$EXTMAX\n 10\n13.0\n 20\n10.0\n"), "{after}");
        assert_eq!(document.geom(0), Some(Geom::Line([11.0, 2.5, 13.0, 4.5])));
        assert_eq!(document.boxes, [[11.0, 2.5, 13.0, 4.5]]);
        // 取り消すと、元のバイト列と列に戻る
        assert!(document.undo());
        assert_eq!(document.save(), before);
        assert_eq!(document.geom(0), Some(Geom::Line([1.0, 2.0, 3.0, 4.0])));
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
        assert_eq!(document.display.canonical(), reopened.display.canonical());
        assert_eq!(document.boxes, reopened.boxes);
        // 一定の幅が 0 の折れ線は、43 を除いて頂点を書き直す
        let saved = text(&saved);
        assert!(!saved.contains(" 43\n"), "{saved}");
        assert!(document.undo());
        assert!(document.undo());
        assert_eq!(document.save(), dxf(&entities));
    }

    #[test]
    fn declined_edits_change_nothing() {
        let entities = format!("{LINE}{CIRCLE}");
        let mut document = open(&entities);
        let before = document.save();
        assert_eq!(
            document.apply(&[translation(0x99, 1.0, 0.0)]),
            Err(Decline::Unknown)
        );
        assert_eq!(
            document.apply(&[translation(0x2A, 1.0, 0.0), translation(0x2A, 2.0, 0.0)]),
            Err(Decline::Repeated)
        );
        assert_eq!(
            document.apply(&[translation(0x2A, 1.0, 0.0), Edit::Remove { handle: 0x2A }]),
            Err(Decline::Repeated)
        );
        // 逆のない変換と、円を楕円にする変換は、種類を保てない。まとめて受け付けない
        let flat = Edit::Transform {
            handle: 0x2A,
            matrix: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        };
        assert_eq!(document.apply(&[flat]), Err(Decline::Unsupported));
        let stretch = [2.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let ellipse = Edit::Transform {
            handle: 0x2B,
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
        // 表にない画層へは、試作は変えない（今の実装は、図形が指すだけの画層も一覧に持つ）
        let restyle = Edit::Restyle {
            handle: 0x2A,
            style: Restyle {
                layer: Some("Walls".to_string()),
                ..Restyle::default()
            },
        };
        assert_eq!(
            document.apply(&[restyle]),
            Err(Decline::Outside(Outside::Names))
        );
        assert_eq!(document.save(), before);
        assert_eq!(document.undo_len(), 0);
        // 線は、相似でない変換でも動かせる
        let line = Edit::Transform {
            handle: 0x2A,
            matrix: stretch,
        };
        assert_eq!(document.apply(&[line]), Ok(vec![None]));
        assert_eq!(document.geom(0), Some(Geom::Line([2.0, 2.0, 6.0, 4.0])));
    }

    #[test]
    fn a_seed_not_above_the_largest_handle_is_raised_by_any_edit() {
        let entities = LINE.replace("2A", "FFFF");
        let mut document = open(&entities);
        assert_eq!(document.locked, None);
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
        assert_eq!(document.display.canonical(), reopened.display.canonical());
        // 同じ値への変更は、受け付けて、何も書かない
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
        // 色番号 0 と、表にない線種は、受け付けない
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
        // 足す要素のハンドルは、種と、ハンドルの最大の次の、大きいほう
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
        // 足した要素も、動かして除ける
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
        assert_eq!(document.rows.len(), 2);
        assert_eq!(document.row_of(0x2A), Some(0));
        assert!(document.redo());
        assert_eq!(document.save(), saved);
        assert!(document.redo());
        assert!(document.redo());
        assert_eq!(document.save(), emptied);
    }

    #[test]
    fn removing_an_element_drops_the_references_to_it() {
        // 円が、リアクターの組で線を指す。線を除くと、組ごとなくなる。
        // 逆にリアクターの組を持つ円は、#881 で削除を拒む
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
}
