//! 表示のバッファー。線分・円弧・文字・ブロックの挿入を、要素ごとの値のままページ配列に持つ
//! （頂点への分割と字形、定義の中身を挿入ごとに置くことは、描く側で行う）。
//! 点は f64 を上位と下位の f32 に分けて置き、視点からの相対の位置を描く側で作れるようにする。
//! 変更のときは、変わった要素の分だけを書き直す。動かす変更と属性を変える変更では、開き直して作った
//! バッファーと同じ値になる。除いた行は場所を残して持ち主を外すので、配列の並びは開き直したものと変わる。

use crate::lift::{Geom, POLY_CLOSED, Pools, TextRow};
use crate::paged::Paged;

/// 使っていない場所の持ち主
pub const NO_OWNER: u32 = u32::MAX;

const SLOT_TEXT_INST: u32 = 0x8000_0000;

/// 要素が使う場所（16 バイト。どの種類も「線分＋円弧」か「文字＋挿入」のどちらかだけを使う）
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Slot {
    a_first: u32,
    a_count: u32,
    b_first: u32,
    b_count_tag: u32,
}

impl Slot {
    #[inline]
    const fn is_text_inst(self) -> bool {
        self.b_count_tag & SLOT_TEXT_INST != 0
    }

    /// 線分の最初の番号
    #[inline]
    pub const fn segment(self) -> u32 {
        if self.is_text_inst() { 0 } else { self.a_first }
    }

    /// 線分の数
    #[inline]
    pub const fn segments(self) -> u32 {
        if self.is_text_inst() { 0 } else { self.a_count }
    }

    /// 円弧の最初の番号
    #[inline]
    pub const fn arc(self) -> u32 {
        if self.is_text_inst() { 0 } else { self.b_first }
    }

    /// 円弧の数
    #[inline]
    pub const fn arcs(self) -> u32 {
        if self.is_text_inst() {
            0
        } else {
            self.b_count_tag
        }
    }

    /// 文字の最初の番号
    #[inline]
    pub const fn text(self) -> u32 {
        if self.is_text_inst() { self.a_first } else { 0 }
    }

    /// 文字の数（文字は 1、挿入は表示する属性の数）
    #[inline]
    pub const fn texts(self) -> u32 {
        if self.is_text_inst() { self.a_count } else { 0 }
    }

    /// 挿入の番号
    #[inline]
    pub const fn instance(self) -> u32 {
        if self.is_text_inst() { self.b_first } else { 0 }
    }

    /// 挿入の数（0 か 1）
    #[inline]
    pub const fn instances(self) -> u32 {
        if self.is_text_inst() {
            self.b_count_tag & !SLOT_TEXT_INST
        } else {
            0
        }
    }

    #[inline]
    const fn counts(self) -> [u32; 4] {
        [self.segments(), self.arcs(), self.texts(), self.instances()]
    }

    #[inline]
    const fn total(self) -> u32 {
        self.a_count + (self.b_count_tag & !SLOT_TEXT_INST)
    }
}

/// 1 つの行の、線分・円弧・文字・挿入の値（`Display::row`）
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RowBuffers {
    /// 線分
    pub segments: Vec<[f32; 8]>,
    /// 円弧
    pub arcs: Vec<[f32; 7]>,
    /// 文字
    pub texts: Vec<[f32; 8]>,
    /// 挿入の値と、定義の番号・列の数・行の数
    pub instances: Vec<([f32; 9], [u32; 3])>,
}

/// 表示のバッファー
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Display {
    /// 線分。始点の x（上位・下位）・y（上位・下位）、終点の x・y
    pub segments: Paged<[f32; 8]>,
    /// 線分の持ち主（行）
    pub segment_owner: Paged<u32>,
    /// 線分のスタイル（画層・色の指定）
    pub segment_style: Paged<[u32; 2]>,
    /// 円弧。中心の x（上位・下位）・y（上位・下位）、半径、始まりの角、回る角（ラジアン。正が反時計回り）
    pub arcs: Paged<[f32; 7]>,
    /// 円弧の持ち主
    pub arc_owner: Paged<u32>,
    /// 円弧のスタイル
    pub arc_style: Paged<[u32; 2]>,
    /// 文字。置く点の x（上位・下位）・y（上位・下位）、高さ、回転（ラジアン）、幅の係数、傾き（ラジアン）
    pub texts: Paged<[f32; 8]>,
    /// 文字の持ち主
    pub text_owner: Paged<u32>,
    /// 文字のスタイル
    pub text_style: Paged<[u32; 2]>,
    /// 挿入。置く点の x（上位・下位）・y（上位・下位）、尺度の x・y、回転（ラジアン）、列・行の間隔
    pub instances: Paged<[f32; 9]>,
    /// 挿入が指す定義の番号（なければ `NO_DEFINITION`）と、列・行の数
    pub instance_ref: Paged<[u32; 3]>,
    /// 挿入の持ち主
    pub instance_owner: Paged<u32>,
    /// 挿入のスタイル
    pub instance_style: Paged<[u32; 2]>,
    /// 行ごとの場所
    pub slots: Paged<Slot>,
    /// 置き直して使わなくなった場所の数
    pub dead: u32,
}

/// f64 を、上位と下位の f32 に分ける
pub fn split_f64(value: f64) -> [f32; 2] {
    let high = value as f32;
    [high, (value - f64::from(high)) as f32]
}

fn segment([x0, y0]: [f64; 2], [x1, y1]: [f64; 2]) -> [f32; 8] {
    let ([a, b], [c, d], [e, f], [g, h]) =
        (split_f64(x0), split_f64(y0), split_f64(x1), split_f64(y1));
    [a, b, c, d, e, f, g, h]
}

fn arc([x, y]: [f64; 2], radius: f64, start: f64, sweep: f64) -> [f32; 7] {
    let ([a, b], [c, d]) = (split_f64(x), split_f64(y));
    [a, b, c, d, radius as f32, start as f32, sweep as f32]
}

const TURN: f64 = core::f64::consts::TAU;

fn radians(degrees: f64) -> f64 {
    degrees * (core::f64::consts::PI / 180.0)
}

/// 膨らみ `bulge` で `from` から `to` へ進む円弧
pub fn bulged(from: [f64; 2], to: [f64; 2], bulge: f64) -> [f32; 7] {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    // 中心は、弦の中点から左の法線の向きに (1 - b²) / 4b だけ離れる
    let offset = (1.0 - bulge * bulge) / (4.0 * bulge);
    let center = [
        (from[0] + to[0]) / 2.0 - dy * offset,
        (from[1] + to[1]) / 2.0 + dx * offset,
    ];
    let chord = (dx * dx + dy * dy).sqrt();
    let radius = chord * (1.0 + bulge * bulge) / (4.0 * bulge.abs());
    let start = libm::atan2(from[1] - center[1], from[0] - center[0]);
    arc(center, radius, start, 4.0 * libm::atan(bulge))
}

/// 文字の値
fn lettering(row: &TextRow, pools: &Pools) -> [f32; 8] {
    let at = row.point;
    let ([a, b], [c, d]) = (split_f64(at[0]), split_f64(at[1]));
    let rot_deg = match row.place(pools) {
        crate::lift::Place::Between { first, second, .. } if first != second => {
            libm::atan2(second[1] - first[1], second[0] - first[0]).to_degrees()
        }
        _ => row.rotation,
    };
    [
        a,
        b,
        c,
        d,
        row.height as f32,
        radians(rot_deg) as f32,
        row.width_factor(pools) as f32,
        radians(row.oblique(pools)) as f32,
    ]
}

/// 折れ線の辺（始まりの頂点・終わりの頂点・膨らみ）を順に返す
pub fn edges(
    vertices: &[[f64; 3]],
    closed: bool,
) -> impl Iterator<Item = ([f64; 2], [f64; 2], f64)> + '_ {
    let count = vertices.len();
    let last = if closed && count > 1 {
        count
    } else {
        count.saturating_sub(1)
    };
    (0..last).filter_map(move |index| {
        let [x0, y0, bulge] = *vertices.get(index)?;
        let [x1, y1, _] = *vertices.get((index + 1) % count)?;
        Some(([x0, y0], [x1, y1], bulge))
    })
}

impl Display {
    /// 行の数だけ、場所の表を用意する
    pub fn with_rows(rows: usize) -> Self {
        Display {
            slots: Paged::with_capacity(rows),
            ..Display::default()
        }
    }

    /// 要素が要る線分・円弧・文字・挿入の数
    fn counts(geom: &Geom, pools: &Pools) -> [u32; 4] {
        match geom {
            Geom::Line(_) => [1, 0, 0, 0],
            Geom::Circle(_) | Geom::Arc(_) => [0, 1, 0, 0],
            Geom::Polyline {
                first,
                count,
                flags,
                ..
            } => {
                let mut counts = [0; 4];
                for (_, _, bulge) in pools.edges_of(*first, *count, flags & POLY_CLOSED != 0) {
                    counts[usize::from(bulge != 0.0)] += 1;
                }
                counts
            }
            Geom::Text(_) => [0, 0, 1, 0],
            // 属性は、表示するものだけを文字として置く
            Geom::Insert(insert) => {
                let shown = pools
                    .attribs_of(insert)
                    .filter(|attribute| !attribute.hidden())
                    .count();
                [0, 0, shown as u32, 1]
            }
        }
    }

    /// 要素の値とスタイルを、場所 `slot` に書く。場所は、要る数だけあること
    fn write(&mut self, slot: Slot, geom: &Geom, pools: &Pools, style: [u32; 2]) {
        if slot.is_text_inst() {
            let (t0, tn) = (slot.a_first as usize, slot.a_count as usize);
            if tn > 0 {
                self.text_style.fill_range(t0, t0 + tn, style);
            }
            let (i0, in_) = (
                slot.b_first as usize,
                (slot.b_count_tag & !SLOT_TEXT_INST) as usize,
            );
            if in_ > 0 {
                self.instance_style.fill_range(i0, i0 + in_, style);
            }
        } else {
            let (s0, sn) = (slot.a_first as usize, slot.a_count as usize);
            if sn > 0 {
                self.segment_style.fill_range(s0, s0 + sn, style);
            }
            let (a0, an) = (slot.b_first as usize, slot.b_count_tag as usize);
            if an > 0 {
                self.arc_style.fill_range(a0, a0 + an, style);
            }
        }
        match geom {
            Geom::Line([x0, y0, x1, y1]) => {
                if let Some(place) = self.segments.get_mut(slot.segment() as usize) {
                    *place = segment([*x0, *y0], [*x1, *y1]);
                }
            }
            Geom::Circle([x, y, radius]) => {
                if let Some(place) = self.arcs.get_mut(slot.arc() as usize) {
                    *place = arc([*x, *y], *radius, 0.0, TURN);
                }
            }
            Geom::Arc([x, y, radius, start, end]) => {
                // 始まりと終わりが同じ角なら、ひと回りとする
                let sweep = match (end - start).rem_euclid(360.0) {
                    0.0 => 360.0,
                    sweep => sweep,
                };
                if let Some(place) = self.arcs.get_mut(slot.arc() as usize) {
                    *place = arc([*x, *y], *radius, radians(*start), radians(sweep));
                }
            }
            Geom::Polyline {
                first,
                count,
                flags,
                ..
            } => {
                let (mut straight, mut round) = (slot.segment() as usize, slot.arc() as usize);
                for (from, to, bulge) in pools.edges_of(*first, *count, flags & POLY_CLOSED != 0) {
                    if bulge == 0.0 {
                        if let Some(place) = self.segments.get_mut(straight) {
                            *place = segment(from, to);
                        }
                        straight += 1;
                    } else {
                        if let Some(place) = self.arcs.get_mut(round) {
                            *place = bulged(from, to, bulge);
                        }
                        round += 1;
                    }
                }
            }
            Geom::Text(row) => {
                if let Some(place) = self.texts.get_mut(slot.text() as usize) {
                    *place = lettering(row, pools);
                }
            }
            Geom::Insert(insert) => {
                let at = slot.instance() as usize;
                let ([a, b], [c, d]) =
                    (split_f64(insert.position[0]), split_f64(insert.position[1]));
                let grid = insert.grid(pools);
                let spacing = insert.spacing(pools);
                if let Some(place) = self.instances.get_mut(at) {
                    *place = [
                        a,
                        b,
                        c,
                        d,
                        insert.scale[0] as f32,
                        insert.scale[1] as f32,
                        radians(insert.rotation) as f32,
                        spacing[0] as f32,
                        spacing[1] as f32,
                    ];
                }
                if let Some(place) = self.instance_ref.get_mut(at) {
                    *place = [insert.definition, grid[0], grid[1]];
                }
                let shown = pools
                    .attribs_of(insert)
                    .filter(|attribute| !attribute.hidden());
                for (at, attribute) in (slot.text() as usize..).zip(shown) {
                    if let Some(place) = self.texts.get_mut(at) {
                        *place = lettering(&attribute.text, pools);
                    }
                    if let Some(place) = self.text_style.get_mut(at) {
                        *place = [attribute.layer, attribute.color];
                    }
                }
            }
        }
    }

    /// 後ろに場所を取る
    fn reserve(&mut self, row: u32, counts: [u32; 4]) -> Slot {
        let slot = if counts[2] > 0 || counts[3] > 0 {
            debug_assert!(counts[0] == 0 && counts[1] == 0 && counts[3] < SLOT_TEXT_INST);
            Slot {
                a_first: self.texts.len() as u32,
                a_count: counts[2],
                b_first: self.instances.len() as u32,
                b_count_tag: counts[3] | SLOT_TEXT_INST,
            }
        } else {
            debug_assert!(counts[1] < SLOT_TEXT_INST);
            Slot {
                a_first: self.segments.len() as u32,
                a_count: counts[0],
                b_first: self.arcs.len() as u32,
                b_count_tag: counts[1],
            }
        };
        let grow = |len: usize, count: u32| len + count as usize;
        if counts[0] > 0 {
            let n = grow(self.segments.len(), counts[0]);
            self.segments.resize(n, [0.0; 8]);
            self.segment_owner.resize(n, row);
            self.segment_style.resize(n, [0; 2]);
        }
        if counts[1] > 0 {
            let n = grow(self.arcs.len(), counts[1]);
            self.arcs.resize(n, [0.0; 7]);
            self.arc_owner.resize(n, row);
            self.arc_style.resize(n, [0; 2]);
        }
        if counts[2] > 0 {
            let n = grow(self.texts.len(), counts[2]);
            self.texts.resize(n, [0.0; 8]);
            self.text_owner.resize(n, row);
            self.text_style.resize(n, [0; 2]);
        }
        if counts[3] > 0 {
            let n = grow(self.instances.len(), counts[3]);
            self.instances.resize(n, [0.0; 9]);
            self.instance_ref.resize(n, [0; 3]);
            self.instance_owner.resize(n, row);
            self.instance_style.resize(n, [0; 2]);
        }
        slot
    }

    /// 場所の持ち主を書く
    fn own(&mut self, slot: Slot, owner: u32) {
        if slot.is_text_inst() {
            let (t0, tn) = (slot.a_first as usize, slot.a_count as usize);
            if tn > 0 {
                self.text_owner.fill_range(t0, t0 + tn, owner);
            }
            let (i0, in_) = (
                slot.b_first as usize,
                (slot.b_count_tag & !SLOT_TEXT_INST) as usize,
            );
            if in_ > 0 {
                self.instance_owner.fill_range(i0, i0 + in_, owner);
            }
        } else {
            let (s0, sn) = (slot.a_first as usize, slot.a_count as usize);
            if sn > 0 {
                self.segment_owner.fill_range(s0, s0 + sn, owner);
            }
            let (a0, an) = (slot.b_first as usize, slot.b_count_tag as usize);
            if an > 0 {
                self.arc_owner.fill_range(a0, a0 + an, owner);
            }
        }
    }

    /// 次の行の要素を足す。行は、0 から順に足すこと
    pub fn push(&mut self, geom: &Geom, pools: &Pools, style: [u32; 2]) {
        let row = self.slots.len() as u32;
        let slot = self.reserve(row, Self::counts(geom, pools));
        self.write(slot, geom, pools, style);
        self.slots.push(slot);
    }

    /// 終わりの行を除く。足した行を取り消すときに使う。
    /// 場所が配列の終わりにあれば切り詰め、なければ使わない場所として残す
    pub fn pop(&mut self) {
        let Some(slot) = self.slots.pop() else {
            return;
        };
        let at_end =
            |first: u32, count: u32, len: usize| count == 0 || (first + count) as usize == len;
        if at_end(slot.segment(), slot.segments(), self.segments.len())
            && at_end(slot.arc(), slot.arcs(), self.arcs.len())
            && at_end(slot.text(), slot.texts(), self.texts.len())
            && at_end(slot.instance(), slot.instances(), self.instances.len())
        {
            if slot.segments() > 0 {
                let n = slot.segment() as usize;
                self.segments.truncate(n);
                self.segment_owner.truncate(n);
                self.segment_style.truncate(n);
            }
            if slot.arcs() > 0 {
                let n = slot.arc() as usize;
                self.arcs.truncate(n);
                self.arc_owner.truncate(n);
                self.arc_style.truncate(n);
            }
            if slot.texts() > 0 {
                let n = slot.text() as usize;
                self.texts.truncate(n);
                self.text_owner.truncate(n);
                self.text_style.truncate(n);
            }
            if slot.instances() > 0 {
                let n = slot.instance() as usize;
                self.instances.truncate(n);
                self.instance_ref.truncate(n);
                self.instance_owner.truncate(n);
                self.instance_style.truncate(n);
            }
        } else {
            self.own(slot, NO_OWNER);
            self.dead += slot.total();
        }
    }

    /// 行 `row` の要素を描かないようにする。場所は残す（取り消しで戻す）
    pub fn hide(&mut self, row: u32) {
        if let Some(&slot) = self.slots.get(row as usize) {
            self.own(slot, NO_OWNER);
        }
    }

    /// `hide` で隠した行を戻す
    pub fn show(&mut self, row: u32) {
        if let Some(&slot) = self.slots.get(row as usize) {
            self.own(slot, row);
        }
    }

    /// 行 `row` の要素を書き直す。線分・円弧・文字・挿入の数が変わらなければ同じ場所に書き、変われば後ろに置き直す
    pub fn update(&mut self, row: u32, geom: &Geom, pools: &Pools, style: [u32; 2]) {
        let Some(&slot) = self.slots.get(row as usize) else {
            return;
        };
        let counts = Self::counts(geom, pools);
        let mut slot = slot;
        if slot.counts() != counts {
            self.own(slot, NO_OWNER);
            self.dead += slot.total();
            slot = self.reserve(row, counts);
            if let Some(place) = self.slots.get_mut(row as usize) {
                *place = slot;
            }
        }
        self.write(slot, geom, pools, style);
    }

    /// 行 `row` の値
    pub fn row(&self, row: u32) -> RowBuffers {
        let Some(&slot) = self.slots.get(row as usize) else {
            return RowBuffers::default();
        };
        let inst_start = slot.instance() as usize;
        let inst_end = (slot.instance() + slot.instances()) as usize;
        RowBuffers {
            segments: self.segments.slice_to_vec(
                slot.segment() as usize,
                (slot.segment() + slot.segments()) as usize,
            ),
            arcs: self
                .arcs
                .slice_to_vec(slot.arc() as usize, (slot.arc() + slot.arcs()) as usize),
            texts: self
                .texts
                .slice_to_vec(slot.text() as usize, (slot.text() + slot.texts()) as usize),
            instances: self
                .instances
                .slice_to_vec(inst_start, inst_end)
                .into_iter()
                .zip(self.instance_ref.slice_to_vec(inst_start, inst_end))
                .collect(),
        }
    }

    /// すべての行の値を、行の順に並べたもの
    pub fn canonical(&self) -> Vec<RowBuffers> {
        (0..self.slots.len() as u32)
            .map(|row| self.row(row))
            .collect()
    }

    /// バッファーのバイト数
    pub fn bytes(&self) -> usize {
        self.segments.len() * (32 + 4 + 8)
            + self.arcs.len() * (28 + 4 + 8)
            + self.texts.len() * (32 + 4 + 8)
            + self.instances.len() * (36 + 12 + 4 + 8)
            + self.slots.len() * core::mem::size_of::<Slot>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lift::{ATTRIB_HIDDEN, AttribRow, InsertExtra, InsertRow};
    use crate::paged::TextSpan;

    #[test]
    fn a_point_splits_into_high_and_low() {
        let value = 123_456_789.123_456_79_f64;
        let [high, low] = split_f64(value);
        let error = (f64::from(high) + f64::from(low) - value).abs();
        assert!(error <= value / (1u64 << 47) as f64, "{error}");
        assert!((f64::from(value as f32) - value).abs() > 1.0);
        let [high, low] = split_f64(1_234_567.25);
        assert_eq!(f64::from(high) + f64::from(low), 1_234_567.25);
    }

    #[test]
    fn a_bulge_of_one_is_a_half_circle() {
        let [.., radius, start, sweep] = bulged([0.0, 0.0], [2.0, 0.0], 1.0);
        assert_eq!(radius, 1.0);
        assert!((start - core::f32::consts::PI).abs() < 1e-6);
        assert!((sweep - core::f32::consts::PI).abs() < 1e-6);
    }

    #[test]
    fn an_update_keeps_the_place_when_counts_match() {
        let mut pools = Pools::default();
        pools
            .vertices
            .extend_from_slice(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.5], [1.0, 1.0, 0.0]]);
        let line = Geom::Polyline {
            first: 0,
            count: 3,
            flags: POLY_CLOSED,
            width: 0.0,
        };
        let mut display = Display::default();
        display.push(&Geom::Line([0.0, 0.0, 1.0, 1.0]), &pools, [0, 0]);
        display.push(&line, &pools, [0, 0]);
        assert_eq!(display.segments.len(), 3);
        assert_eq!(display.arcs.len(), 1);
        let before = display.slots.clone();
        *pools.vertices.get_mut(2).expect("v") = [2.0, 2.0, 0.0];
        display.update(1, &line, &pools, [3, 4]);
        assert_eq!(display.slots, before);
        assert_eq!(display.dead, 0);
        assert_eq!(
            display.segment_style.slice_to_vec(0, 3),
            [[0, 0], [3, 4], [3, 4]]
        );
        pools.vertices.get_mut(1).expect("v")[2] = 0.0;
        display.update(1, &line, &pools, [0, 0]);
        assert_eq!(display.dead, 3);
        assert_eq!(display.slots.get(1).expect("s").segments(), 3);
        assert_eq!(core::mem::size_of::<Slot>(), 16);
        assert_eq!(
            display.segment_owner.slice_to_vec(0, 6),
            [0, NO_OWNER, NO_OWNER, 1, 1, 1]
        );
    }

    fn text(x: f64, flags: u8) -> TextRow {
        TextRow {
            point: [x, 0.0],
            height: 2.5,
            rotation: 0.0,
            content: TextSpan::EMPTY,
            style: TextSpan::STANDARD,
            extra: 0,
            h_align: 0,
            v_align: 0,
            flags,
            pad: 0,
        }
    }

    #[test]
    fn an_insert_is_one_instance_and_its_shown_attributes() {
        let attribute = |x: f64, hidden: bool| AttribRow {
            handle: 0,
            text: text(x, if hidden { ATTRIB_HIDDEN } else { 0 }),
            tag: TextSpan::EMPTY,
            record: 0,
            layer: 7,
            color: 9,
            extra_style: 0,
        };
        let mut pools = Pools::default();
        pools.attribs.extend([
            attribute(1.0, false),
            attribute(2.0, true),
            attribute(3.0, false),
        ]);
        pools.insert_extras.push(InsertExtra {
            grid: [3, 2],
            spacing: [5.0, 6.0],
            clip: 0,
        });
        let insert = Geom::Insert(InsertRow {
            position: [10.0, 20.0],
            scale: [2.0, 2.0],
            rotation: 90.0,
            name: TextSpan::EMPTY,
            definition: 4,
            attrib_first: 0,
            attrib_count: 3,
            attached: 3,
            extra: 1,
            flags: 1,
            pad: [0; 3],
        });
        let mut display = Display::default();
        display.push(&insert, &pools, [1, 2]);
        assert_eq!(display.instance_ref.slice_to_vec(0, 1), [[4, 3, 2]]);
        assert_eq!(display.instance_style.slice_to_vec(0, 1), [[1, 2]]);
        assert_eq!(display.texts.len(), 2);
        assert_eq!(display.text_style.slice_to_vec(0, 2), [[7, 9], [7, 9]]);
        assert_eq!(display.texts.get(1).expect("t")[0], 3.0);
        let row = display.row(0);
        assert_eq!(row.instances.len(), 1);
        assert_eq!(
            row.instances[0].0[4..],
            [2.0, 2.0, core::f32::consts::FRAC_PI_2, 5.0, 6.0]
        );
    }

    #[test]
    fn hiding_and_popping_rows() {
        let pools = Pools::default();
        let mut display = Display::default();
        display.push(&Geom::Line([0.0, 0.0, 1.0, 1.0]), &pools, [0, 0]);
        display.push(&Geom::Circle([0.0, 0.0, 1.0]), &pools, [0, 0]);
        let opened = display.clone();
        display.hide(0);
        assert_eq!(display.segment_owner.slice_to_vec(0, 1), [NO_OWNER]);
        assert_eq!(display.segments, opened.segments);
        display.show(0);
        assert_eq!(display, opened);
        display.push(&Geom::Line([2.0, 2.0, 3.0, 3.0]), &pools, [0, 0]);
        display.pop();
        assert_eq!(display, opened);
    }
}
