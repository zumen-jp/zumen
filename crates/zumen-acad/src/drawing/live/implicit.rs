//! 暗黙の画層・線種（`TABLES` に定義がなく、図形の名前から足したもの）の参照の位置と、正規番号の並び。
//!
//! 読み直した図面では、暗黙の画層・線種は表の定義の後に、最初に参照された位置の順で並ぶ。位置は
//! `(段, 位置, 図形の中の順)` で表す。段 0 は `ENTITIES` のレコード番号（保存の順で、追加した図形は元の
//! 図形の後に追加の順で並ぶ）、段 1 はブロック定義の中の順（高速経路では変わらない）。
//!
//! 常駐ではモデル空間の行からの参照を多重集合で持ち、変更で動いた参照だけを足し引きする。正規番号の
//! 並びはスロットごとの先頭の位置を並べ替えて作るので、手間はスロットの数で決まる。モデル空間の行の外
//! （用紙・ブロック定義・読めなかった図形）からの参照は、先頭の位置がわかっていれば `Anchor::Fixed`、
//! わからなければ `Anchor::Unknown` とし、わからない位置に頼る変更は構造経路に任せる。

use std::collections::BTreeMap;

use zumen_drawing::{LayerId, LineTypeId};

use super::shape::SlotCatalog;

/// 参照の位置 `(段, 位置, 図形の中の順)`
pub type RefKey = (u32, u32, u32);

/// モデル空間の行からの参照の位置 `(レコード番号, 図形の中の順)`
pub type ModelKey = (u32, u32);

/// 生きていない暗黙スロットに置く正規番号の番兵
pub const INACTIVE: u32 = u32::MAX;

/// モデル空間の行の外からの参照
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Anchor {
    /// モデル空間の行の外からの参照はない
    #[default]
    None,
    /// 行の外からの参照があり、その先頭は確かにこの位置
    Fixed(RefKey),
    /// 行の外からの参照があるかもしれず、あればこの位置以降にある
    Unknown(RefKey),
}

/// 1 つのスロットへの参照
#[derive(Clone, Debug, Default)]
pub struct SlotRefs {
    /// モデル空間の行からの参照の位置と件数
    pub model: BTreeMap<ModelKey, u32>,
    /// 行の外からの参照
    pub anchor: Anchor,
}

/// 参照の 1 件の増減
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RefDelta {
    /// 線種なら真、画層なら偽
    pub line_type: bool,
    /// スロット番号
    pub slot: u32,
    /// 位置
    pub key: ModelKey,
    /// 件数の増減
    pub count: i32,
}

/// 画層か線種の一方の参照表
#[derive(Clone, Debug, Default)]
pub struct KindRefs {
    /// `TABLES` から作ったスロットの数（この前のスロットは常に生きていて、正規番号は動かない）
    pub table: u32,
    /// 暗黙スロット（`table` 以降）ごとの参照
    pub slots: Vec<SlotRefs>,
    /// 開いたときの参照から正規番号の並びを再現できたか
    pub usable: bool,
}

/// 暗黙の画層・線種の参照表
#[derive(Clone, Debug, Default)]
pub struct ImplicitRefs {
    /// 画層
    pub layers: KindRefs,
    /// 線種
    pub line_types: KindRefs,
}

/// 開くときに集める、1 種類分の材料
#[derive(Default)]
pub struct OpenNotes {
    /// 暗黙スロットごとに、最初に名前を足した位置
    pub first: Vec<RefKey>,
    /// 暗黙スロットごとに、行の外から参照しているかもしれない最初のレコード
    pub maybe: BTreeMap<u32, u32>,
    /// モデル空間の行が、行に残らない名前を参照した（属性を落としたなど）
    pub opaque: bool,
}

impl OpenNotes {
    /// レコード `record` で足されたスロットの最初の位置を記録する
    pub fn note_new(&mut self, table: u32, before: usize, after: usize, record: u32) {
        for (order, slot) in (before..after).enumerate() {
            if slot >= table as usize && self.first.len() == slot - table as usize {
                self.first.push((0, record, order as u32));
            }
        }
    }

    /// ブロック定義などで足されたスロットの最初の位置を記録する
    pub fn note_after_entities(&mut self, table: u32, before: usize, after: usize) {
        for (order, slot) in (before..after).enumerate() {
            if slot >= table as usize && self.first.len() == slot - table as usize {
                self.first.push((1, order as u32, 0));
            }
        }
    }

    /// 行の外の図形がスロット `slot` を参照しているかもしれないことを記録する
    pub fn note_maybe(&mut self, slot: u32, record: u32) {
        self.maybe
            .entry(slot)
            .and_modify(|at| *at = (*at).min(record))
            .or_insert(record);
    }
}

impl KindRefs {
    fn slot(&self, slot: u32) -> Option<&SlotRefs> {
        slot.checked_sub(self.table)
            .and_then(|idx| self.slots.get(idx as usize))
    }

    /// 開いたときの材料から参照表を組む。`pinned` はスロットが用紙・ブロック定義の要素から参照されて
    /// いるか、`in_blocks` は BLOCKS のレコード（持ち上げに失敗したものを含む）が名前で指しているか
    pub fn build(
        table: u32,
        slot_count: usize,
        pinned: impl Fn(u32) -> bool,
        in_blocks: impl Fn(u32) -> bool,
        model: Vec<BTreeMap<ModelKey, u32>>,
        notes: &OpenNotes,
        current: &[u32],
    ) -> Self {
        let implicit = slot_count.saturating_sub(table as usize);
        let mut slots = Vec::with_capacity(implicit);
        let mut model = model.into_iter();
        let mut usable = notes.first.len() == implicit && (implicit == 0 || !notes.opaque);
        for idx in 0..implicit {
            let slot = table + idx as u32;
            let model = model.next().unwrap_or_default();
            let first = notes.first.get(idx).copied().unwrap_or((u32::MAX, 0, 0));
            let model_first = model
                .first_key_value()
                .map(|(&(record, order), _)| (0, record, order));
            let first_from_model = model_first.is_some_and(|key| key.1 == first.1 && first.0 == 0);
            let maybe = notes.maybe.get(&slot).map(|&record| (0, record, 0));
            let anchor = if !first_from_model {
                // 最初に名前を足したのは行の外の図形で、その参照は変更で動かない
                Anchor::Fixed(first)
            } else if pinned(slot) {
                // 行の外の参照は、最初の行より後のどこか
                let bound = (0, first.1.saturating_add(1), 0);
                Anchor::Unknown(maybe.map_or(bound, |at| at.min(bound)))
            } else if let Some(at) = maybe {
                Anchor::Unknown(at)
            } else if in_blocks(slot) {
                // ブロック定義の名前は ENTITIES のすべての参照の後で足すので、モデル空間の参照が残る
                // 間だけ先頭がわかる
                Anchor::Unknown((1, 0, 0))
            } else {
                Anchor::None
            };
            slots.push(SlotRefs { model, anchor });
        }
        let mut refs = Self {
            table,
            slots,
            usable,
        };
        // 開いたときの並び（名前を足した順）を参照の位置から作り直せるかを確かめる
        if usable {
            usable = refs
                .order(&BTreeMap::new(), slot_count)
                .is_some_and(|order| order == current.get(table as usize..).unwrap_or_default());
        }
        refs.usable = usable;
        refs
    }

    /// 増減 `deltas`（スロットごと）を当てたときの、スロットの先頭の位置。生きていなければ
    /// `Ok(None)`、位置がわからなければ `Err(())`
    fn head(
        &self,
        slot: u32,
        deltas: Option<&BTreeMap<ModelKey, i64>>,
    ) -> Result<Option<RefKey>, ()> {
        let empty = SlotRefs::default();
        let refs = self.slot(slot).unwrap_or(&empty);
        let mut model_head = None;
        for (key, &count) in &refs.model {
            let delta = deltas.and_then(|d| d.get(key)).copied().unwrap_or(0);
            if i64::from(count) + delta > 0 {
                model_head = Some(*key);
                break;
            }
        }
        if let Some(deltas) = deltas {
            for (key, &delta) in deltas {
                if delta > 0
                    && !refs.model.contains_key(key)
                    && model_head.is_none_or(|head| *key < head)
                {
                    model_head = Some(*key);
                }
            }
        }
        let model_head = model_head.map(|(record, order)| (0, record, order));
        match refs.anchor {
            Anchor::None => Ok(model_head),
            Anchor::Fixed(at) => Ok(Some(model_head.map_or(at, |head| head.min(at)))),
            Anchor::Unknown(bound) => match model_head {
                Some(head) if head < bound => Ok(Some(head)),
                _ => Err(()),
            },
        }
    }

    /// 暗黙スロットを正規の順に並べる（`slot_count` は増やした後のスロット数）
    fn order(
        &self,
        deltas: &BTreeMap<u32, BTreeMap<ModelKey, i64>>,
        slot_count: usize,
    ) -> Option<Vec<u32>> {
        let mut keyed = Vec::new();
        for slot in self.table..slot_count as u32 {
            if let Some(head) = self.head(slot, deltas.get(&slot)).ok()? {
                keyed.push((head, slot));
            }
        }
        keyed.sort_unstable();
        Some(keyed.into_iter().map(|(_, slot)| slot).collect())
    }

    /// 増減を当てる（`forward` が偽なら逆向き）
    fn apply(&mut self, delta: &RefDelta, forward: bool) {
        let Some(idx) = delta.slot.checked_sub(self.table) else {
            return;
        };
        let idx = idx as usize;
        if self.slots.len() <= idx {
            self.slots.resize_with(idx + 1, SlotRefs::default);
        }
        let Some(refs) = self.slots.get_mut(idx) else {
            return;
        };
        let change = if forward { delta.count } else { -delta.count };
        let now = i64::from(refs.model.get(&delta.key).copied().unwrap_or(0)) + i64::from(change);
        if now > 0 {
            refs.model
                .insert(delta.key, u32::try_from(now).unwrap_or(u32::MAX));
        } else {
            refs.model.remove(&delta.key);
        }
    }
}

/// 増減を当てた後の正規番号の並び
pub struct Projection {
    /// 正規順の生きている画層スロット
    pub active_layer_slots: Vec<u32>,
    /// 画層スロットから正規番号
    pub layer_remap: Vec<LayerId>,
    /// 正規順の生きている線種スロット
    pub active_line_type_slots: Vec<u32>,
    /// 線種スロットから正規番号
    pub line_type_remap: Vec<LineTypeId>,
}

/// 増減をスロットごとにまとめる（打ち消し合うものは除く）
pub fn net_deltas(deltas: &[RefDelta], line_type: bool) -> BTreeMap<u32, BTreeMap<ModelKey, i64>> {
    let mut net: BTreeMap<u32, BTreeMap<ModelKey, i64>> = BTreeMap::new();
    for delta in deltas.iter().filter(|d| d.line_type == line_type) {
        *net.entry(delta.slot)
            .or_default()
            .entry(delta.key)
            .or_default() += i64::from(delta.count);
    }
    for per_slot in net.values_mut() {
        per_slot.retain(|_, count| *count != 0);
    }
    net.retain(|_, per_slot| !per_slot.is_empty());
    net
}

impl ImplicitRefs {
    /// 増減を当てる
    pub fn apply(&mut self, deltas: &[RefDelta], forward: bool) {
        for delta in deltas {
            if delta.line_type {
                self.line_types.apply(delta, forward);
            } else {
                self.layers.apply(delta, forward);
            }
        }
    }

    /// 増減を当てた後の並びを作る。並びが決まらない、または `check` が拒む変化なら `None`
    ///
    /// `catalog` は変更で増える名前を足し終えたカタログ。`deltas` は表のスロットの増減を含まない。
    ///
    /// `born_from` はこの変更で足す図形の最初のレコード番号（それより前のレコードはすべて元からある）。
    pub fn project(
        &self,
        before: &SlotCatalog,
        catalog: &SlotCatalog,
        deltas: &[RefDelta],
        born_from: u32,
    ) -> Option<Projection> {
        let layer_net = net_deltas(deltas, false);
        let line_type_net = net_deltas(deltas, true);
        let layers = project_kind(
            &self.layers,
            &layer_net,
            before.layers.len(),
            catalog.layers.len(),
            &before.active_layer_slots,
            born_from,
            |slot| {
                catalog
                    .layers
                    .get(slot as usize)
                    .map(|e| e.spelled_otherwise)
            },
        )?;
        let line_types = project_kind(
            &self.line_types,
            &line_type_net,
            before.line_types.len(),
            catalog.line_types.len(),
            &before.active_line_type_slots,
            born_from,
            |slot| {
                catalog
                    .line_types
                    .get(slot as usize)
                    .map(|e| e.spelled_otherwise)
            },
        )?;
        let remap = |active: &[u32], len: usize| {
            let mut remap = vec![INACTIVE; len];
            for (id, &slot) in active.iter().enumerate() {
                if let Some(at) = remap.get_mut(slot as usize) {
                    *at = id as u32;
                }
            }
            remap
        };
        let layer_remap = remap(&layers, catalog.layers.len())
            .into_iter()
            .map(LayerId)
            .collect();
        let line_type_remap = remap(&line_types, catalog.line_types.len())
            .into_iter()
            .map(LineTypeId)
            .collect();
        Some(Projection {
            active_layer_slots: layers,
            layer_remap,
            active_line_type_slots: line_types,
            line_type_remap,
        })
    }
}

fn project_kind(
    refs: &KindRefs,
    net: &BTreeMap<u32, BTreeMap<ModelKey, i64>>,
    before_count: usize,
    slot_count: usize,
    current_active: &[u32],
    born_from: u32,
    spelled_otherwise: impl Fn(u32) -> Option<bool>,
) -> Option<Vec<u32>> {
    let grew = slot_count > before_count;
    let implicit_net = net.range(refs.table..);
    if !grew && implicit_net.clone().next().is_none() {
        return Some(current_active.to_vec());
    }
    if !refs.usable {
        // 並びを作り直せない文書では、モデル空間の参照が残るスロットへ、この変更で足す図形（保存で
        // 元の図形の後ろに並ぶ）から参照を足すだけの変更に限る。先頭の参照は元の図形のどれかのまま動かない
        let only_appended = implicit_net.into_iter().all(|(&slot, per_slot)| {
            refs.slot(slot).is_some_and(|r| !r.model.is_empty())
                && per_slot
                    .iter()
                    .all(|(&(record, _), &count)| count > 0 && record >= born_from)
        });
        return (!grew && only_appended).then(|| current_active.to_vec());
    }
    for (&slot, per_slot) in implicit_net {
        // 綴りの違う参照があるスロットは、先頭の参照が変わると名前が変わりうる
        if spelled_otherwise(slot)?
            && refs.head(slot, None).ok()? != refs.head(slot, Some(per_slot)).ok()?
        {
            return None;
        }
    }
    let implicit = refs.order(net, slot_count)?;
    let mut active = Vec::with_capacity(refs.table as usize + implicit.len());
    active.extend(0..refs.table);
    active.extend(implicit);
    Some(active)
}
