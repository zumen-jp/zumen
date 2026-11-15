//! 表示のバッファー。線分・円弧・文字・ブロックの挿入を、要素ごとの値のままページ配列に持つ
//! （頂点への分割と字形、定義の中身を挿入ごとに置くことは、描く側で行う）。
//! 点は f64 を上位と下位の f32 に分けて置き、視点からの相対の位置を描く側で作れるようにする。
//! 変更のときは、変わった要素の分だけを書き直す。動かす変更と属性を変える変更では、開き直して作った
//! バッファーと同じ値になる。除いた行は場所を残して持ち主を外すので、配列の並びは開き直したものと変わる。

use crate::lift::{Geom, POLY_CLOSED, Pools, TextRow};
use crate::paged::Paged;

/// 使っていない場所の持ち主
pub const NO_OWNER: u32 = u32::MAX;

pub use zumen_live::dirty::{DIRTY_BLOCK, DirtyColumn};

/// 表示のバッファーの GPU 13 列ごとの変更追跡
#[derive(Clone, Debug, Default)]
pub struct DisplayDirty {
    /// 線分の座標
    pub segments: DirtyColumn,
    /// 線分の持ち主
    pub segment_owner: DirtyColumn,
    /// 線分のスタイル
    pub segment_style: DirtyColumn,
    /// 円弧の座標と角
    pub arcs: DirtyColumn,
    /// 円弧の持ち主
    pub arc_owner: DirtyColumn,
    /// 円弧のスタイル
    pub arc_style: DirtyColumn,
    /// 文字の位置と姿勢
    pub texts: DirtyColumn,
    /// 文字の持ち主
    pub text_owner: DirtyColumn,
    /// 文字のスタイル
    pub text_style: DirtyColumn,
    /// 挿入の位置と尺度
    pub instances: DirtyColumn,
    /// 挿入の定義参照と格子数
    pub instance_ref: DirtyColumn,
    /// 挿入の持ち主
    pub instance_owner: DirtyColumn,
    /// 挿入のスタイル
    pub instance_style: DirtyColumn,
}

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
#[derive(Clone, Debug)]
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
    /// GPU 転送用の 64 要素ブロック単位の変更追跡
    pub dirty: DisplayDirty,
    /// 変更追跡を行うか（初回の全体構築中は止め、`clear_dirty` で有効にする）
    track_dirty: bool,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            segments: Paged::new(),
            segment_owner: Paged::new(),
            segment_style: Paged::new(),
            arcs: Paged::new(),
            arc_owner: Paged::new(),
            arc_style: Paged::new(),
            texts: Paged::new(),
            text_owner: Paged::new(),
            text_style: Paged::new(),
            instances: Paged::new(),
            instance_ref: Paged::new(),
            instance_owner: Paged::new(),
            instance_style: Paged::new(),
            slots: Paged::new(),
            dead: 0,
            dirty: DisplayDirty::default(),
            track_dirty: true,
        }
    }
}

impl PartialEq for Display {
    fn eq(&self, other: &Self) -> bool {
        self.dead == other.dead
            && self.slots == other.slots
            && self.segments == other.segments
            && self.segment_owner == other.segment_owner
            && self.segment_style == other.segment_style
            && self.arcs == other.arcs
            && self.arc_owner == other.arc_owner
            && self.arc_style == other.arc_style
            && self.texts == other.texts
            && self.text_owner == other.text_owner
            && self.text_style == other.text_style
            && self.instances == other.instances
            && self.instance_ref == other.instance_ref
            && self.instance_owner == other.instance_owner
            && self.instance_style == other.instance_style
    }
}

#[inline]
fn same_f32<const N: usize>(a: &[f32; N], b: &[f32; N]) -> bool {
    a.map(f32::to_bits) == b.map(f32::to_bits)
}

fn fill_style_checked(
    col: &mut Paged<[u32; 2]>,
    dirty: &mut DirtyColumn,
    track: bool,
    start: usize,
    end: usize,
    style: [u32; 2],
) {
    if start >= end {
        return;
    }
    if end - start == 1 {
        if let Some(place) = col.get_mut(start)
            && *place != style
        {
            *place = style;
            if track {
                dirty.mark_one(start);
            }
        }
        return;
    }
    let mut changed = false;
    for at in start..end {
        if let Some(place) = col.get_mut(at)
            && *place != style
        {
            *place = style;
            changed = true;
        }
    }
    if changed && track {
        dirty.mark_range(start, end);
    }
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
    /// 行の数だけ、場所の表を用意する（初回構築中は変更追跡を止め、終わったら `clear_dirty` を呼ぶ）
    pub fn with_rows(rows: usize) -> Self {
        Display {
            slots: Paged::with_capacity(rows),
            track_dirty: false,
            ..Display::default()
        }
    }

    /// GPU 側のバッファーと同期し終えた状態にし、以降の差分追跡を有効にする
    pub fn clear_dirty(&mut self) {
        self.dirty.segments.clear(self.segments.len());
        self.dirty.segment_owner.clear(self.segment_owner.len());
        self.dirty.segment_style.clear(self.segment_style.len());
        self.dirty.arcs.clear(self.arcs.len());
        self.dirty.arc_owner.clear(self.arc_owner.len());
        self.dirty.arc_style.clear(self.arc_style.len());
        self.dirty.texts.clear(self.texts.len());
        self.dirty.text_owner.clear(self.text_owner.len());
        self.dirty.text_style.clear(self.text_style.len());
        self.dirty.instances.clear(self.instances.len());
        self.dirty.instance_ref.clear(self.instance_ref.len());
        self.dirty.instance_owner.clear(self.instance_owner.len());
        self.dirty.instance_style.clear(self.instance_style.len());
        self.track_dirty = true;
    }

    /// 前回の `clear_dirty` から GPU 列に変化があったか
    pub fn has_dirty(&self) -> bool {
        self.dirty.segments.is_dirty(self.segments.len())
            || self.dirty.segment_owner.is_dirty(self.segment_owner.len())
            || self.dirty.segment_style.is_dirty(self.segment_style.len())
            || self.dirty.arcs.is_dirty(self.arcs.len())
            || self.dirty.arc_owner.is_dirty(self.arc_owner.len())
            || self.dirty.arc_style.is_dirty(self.arc_style.len())
            || self.dirty.texts.is_dirty(self.texts.len())
            || self.dirty.text_owner.is_dirty(self.text_owner.len())
            || self.dirty.text_style.is_dirty(self.text_style.len())
            || self.dirty.instances.is_dirty(self.instances.len())
            || self.dirty.instance_ref.is_dirty(self.instance_ref.len())
            || self
                .dirty
                .instance_owner
                .is_dirty(self.instance_owner.len())
            || self
                .dirty
                .instance_style
                .is_dirty(self.instance_style.len())
    }

    /// 差分で GPU に転送する合計バイト数（64 要素ブロック単位）
    pub fn dirty_bytes(&self) -> usize {
        self.dirty.segments.dirty_elements(self.segments.len()) * 32
            + self
                .dirty
                .segment_owner
                .dirty_elements(self.segment_owner.len())
                * 4
            + self
                .dirty
                .segment_style
                .dirty_elements(self.segment_style.len())
                * 8
            + self.dirty.arcs.dirty_elements(self.arcs.len()) * 28
            + self.dirty.arc_owner.dirty_elements(self.arc_owner.len()) * 4
            + self.dirty.arc_style.dirty_elements(self.arc_style.len()) * 8
            + self.dirty.texts.dirty_elements(self.texts.len()) * 32
            + self.dirty.text_owner.dirty_elements(self.text_owner.len()) * 4
            + self.dirty.text_style.dirty_elements(self.text_style.len()) * 8
            + self.dirty.instances.dirty_elements(self.instances.len()) * 36
            + self
                .dirty
                .instance_ref
                .dirty_elements(self.instance_ref.len())
                * 12
            + self
                .dirty
                .instance_owner
                .dirty_elements(self.instance_owner.len())
                * 4
            + self
                .dirty
                .instance_style
                .dirty_elements(self.instance_style.len())
                * 8
    }

    /// 記録されている差分スライスだけを別の `Display` の GPU 13 列に反映する（GPU 側ミラーの検証用）
    pub fn sync_gpu_mirror(&mut self, mirror: &mut Self) {
        fn sync_col<T: Copy>(src: &Paged<T>, dirty: &DirtyColumn, dst: &mut Paged<T>, zero: T) {
            if dst.len() != src.len() {
                dst.resize(src.len(), zero);
            }
            for span in dirty.spans(src.len()) {
                if let Some(slice) = src.page_slice(span.start, span.end) {
                    dst.copy_range_from_slice(span.start, slice);
                }
            }
        }
        sync_col(
            &self.segments,
            &self.dirty.segments,
            &mut mirror.segments,
            [0.0; 8],
        );
        sync_col(
            &self.segment_owner,
            &self.dirty.segment_owner,
            &mut mirror.segment_owner,
            0,
        );
        sync_col(
            &self.segment_style,
            &self.dirty.segment_style,
            &mut mirror.segment_style,
            [0; 2],
        );
        sync_col(&self.arcs, &self.dirty.arcs, &mut mirror.arcs, [0.0; 7]);
        sync_col(
            &self.arc_owner,
            &self.dirty.arc_owner,
            &mut mirror.arc_owner,
            0,
        );
        sync_col(
            &self.arc_style,
            &self.dirty.arc_style,
            &mut mirror.arc_style,
            [0; 2],
        );
        sync_col(&self.texts, &self.dirty.texts, &mut mirror.texts, [0.0; 8]);
        sync_col(
            &self.text_owner,
            &self.dirty.text_owner,
            &mut mirror.text_owner,
            0,
        );
        sync_col(
            &self.text_style,
            &self.dirty.text_style,
            &mut mirror.text_style,
            [0; 2],
        );
        sync_col(
            &self.instances,
            &self.dirty.instances,
            &mut mirror.instances,
            [0.0; 9],
        );
        sync_col(
            &self.instance_ref,
            &self.dirty.instance_ref,
            &mut mirror.instance_ref,
            [0; 3],
        );
        sync_col(
            &self.instance_owner,
            &self.dirty.instance_owner,
            &mut mirror.instance_owner,
            0,
        );
        sync_col(
            &self.instance_style,
            &self.dirty.instance_style,
            &mut mirror.instance_style,
            [0; 2],
        );
        mirror.slots = self.slots.clone();
        mirror.dead = self.dead;
        self.clear_dirty();
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
        let track = self.track_dirty;
        if slot.is_text_inst() {
            let (t0, tn) = (slot.a_first as usize, slot.a_count as usize);
            fill_style_checked(
                &mut self.text_style,
                &mut self.dirty.text_style,
                track,
                t0,
                t0 + tn,
                style,
            );
            let (i0, in_) = (
                slot.b_first as usize,
                (slot.b_count_tag & !SLOT_TEXT_INST) as usize,
            );
            fill_style_checked(
                &mut self.instance_style,
                &mut self.dirty.instance_style,
                track,
                i0,
                i0 + in_,
                style,
            );
        } else {
            let (s0, sn) = (slot.a_first as usize, slot.a_count as usize);
            fill_style_checked(
                &mut self.segment_style,
                &mut self.dirty.segment_style,
                track,
                s0,
                s0 + sn,
                style,
            );
            let (a0, an) = (slot.b_first as usize, slot.b_count_tag as usize);
            fill_style_checked(
                &mut self.arc_style,
                &mut self.dirty.arc_style,
                track,
                a0,
                a0 + an,
                style,
            );
        }
        match geom {
            Geom::Line([x0, y0, x1, y1]) => {
                let at = slot.segment() as usize;
                let val = segment([*x0, *y0], [*x1, *y1]);
                if let Some(place) = self.segments.get_mut(at)
                    && !same_f32(place, &val)
                {
                    *place = val;
                    if track {
                        self.dirty.segments.mark_one(at);
                    }
                }
            }
            Geom::Circle([x, y, radius]) => {
                let at = slot.arc() as usize;
                let val = arc([*x, *y], *radius, 0.0, TURN);
                if let Some(place) = self.arcs.get_mut(at)
                    && !same_f32(place, &val)
                {
                    *place = val;
                    if track {
                        self.dirty.arcs.mark_one(at);
                    }
                }
            }
            Geom::Arc([x, y, radius, start, end]) => {
                // 始まりと終わりが同じ角なら、ひと回りとする
                let sweep = match (end - start).rem_euclid(360.0) {
                    0.0 => 360.0,
                    sweep => sweep,
                };
                let at = slot.arc() as usize;
                let val = arc([*x, *y], *radius, radians(*start), radians(sweep));
                if let Some(place) = self.arcs.get_mut(at)
                    && !same_f32(place, &val)
                {
                    *place = val;
                    if track {
                        self.dirty.arcs.mark_one(at);
                    }
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
                        let val = segment(from, to);
                        if let Some(place) = self.segments.get_mut(straight)
                            && !same_f32(place, &val)
                        {
                            *place = val;
                            if track {
                                self.dirty.segments.mark_one(straight);
                            }
                        }
                        straight += 1;
                    } else {
                        let val = bulged(from, to, bulge);
                        if let Some(place) = self.arcs.get_mut(round)
                            && !same_f32(place, &val)
                        {
                            *place = val;
                            if track {
                                self.dirty.arcs.mark_one(round);
                            }
                        }
                        round += 1;
                    }
                }
            }
            Geom::Text(row) => {
                let at = slot.text() as usize;
                let val = lettering(row, pools);
                if let Some(place) = self.texts.get_mut(at)
                    && !same_f32(place, &val)
                {
                    *place = val;
                    if track {
                        self.dirty.texts.mark_one(at);
                    }
                }
            }
            Geom::Insert(insert) => {
                let at = slot.instance() as usize;
                let ([a, b], [c, d]) =
                    (split_f64(insert.position[0]), split_f64(insert.position[1]));
                let grid = insert.grid(pools);
                let spacing = insert.spacing(pools);
                let val = [
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
                if let Some(place) = self.instances.get_mut(at)
                    && !same_f32(place, &val)
                {
                    *place = val;
                    if track {
                        self.dirty.instances.mark_one(at);
                    }
                }
                let iref = [insert.definition, grid[0], grid[1]];
                if let Some(place) = self.instance_ref.get_mut(at)
                    && *place != iref
                {
                    *place = iref;
                    if track {
                        self.dirty.instance_ref.mark_one(at);
                    }
                }
                let shown = pools
                    .attribs_of(insert)
                    .filter(|attribute| !attribute.hidden());
                for (at, attribute) in (slot.text() as usize..).zip(shown) {
                    let tval = lettering(&attribute.text, pools);
                    if let Some(place) = self.texts.get_mut(at)
                        && !same_f32(place, &tval)
                    {
                        *place = tval;
                        if track {
                            self.dirty.texts.mark_one(at);
                        }
                    }
                    let tstyle = [attribute.layer, attribute.color];
                    if let Some(place) = self.text_style.get_mut(at)
                        && *place != tstyle
                    {
                        *place = tstyle;
                        if track {
                            self.dirty.text_style.mark_one(at);
                        }
                    }
                }
            }
        }
    }

    /// 後ろに場所を取る
    fn reserve(&mut self, row: u32, counts: [u32; 4]) -> Slot {
        let track = self.track_dirty;
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
            let s0 = self.segments.len();
            let n = grow(s0, counts[0]);
            self.segments.resize(n, [0.0; 8]);
            self.segment_owner.resize(n, row);
            self.segment_style.resize(n, [0; 2]);
            if track {
                self.dirty.segments.mark_range(s0, n);
                self.dirty.segment_owner.mark_range(s0, n);
                self.dirty.segment_style.mark_range(s0, n);
            }
        }
        if counts[1] > 0 {
            let a0 = self.arcs.len();
            let n = grow(a0, counts[1]);
            self.arcs.resize(n, [0.0; 7]);
            self.arc_owner.resize(n, row);
            self.arc_style.resize(n, [0; 2]);
            if track {
                self.dirty.arcs.mark_range(a0, n);
                self.dirty.arc_owner.mark_range(a0, n);
                self.dirty.arc_style.mark_range(a0, n);
            }
        }
        if counts[2] > 0 {
            let t0 = self.texts.len();
            let n = grow(t0, counts[2]);
            self.texts.resize(n, [0.0; 8]);
            self.text_owner.resize(n, row);
            self.text_style.resize(n, [0; 2]);
            if track {
                self.dirty.texts.mark_range(t0, n);
                self.dirty.text_owner.mark_range(t0, n);
                self.dirty.text_style.mark_range(t0, n);
            }
        }
        if counts[3] > 0 {
            let i0 = self.instances.len();
            let n = grow(i0, counts[3]);
            self.instances.resize(n, [0.0; 9]);
            self.instance_ref.resize(n, [0; 3]);
            self.instance_owner.resize(n, row);
            self.instance_style.resize(n, [0; 2]);
            if track {
                self.dirty.instances.mark_range(i0, n);
                self.dirty.instance_ref.mark_range(i0, n);
                self.dirty.instance_owner.mark_range(i0, n);
                self.dirty.instance_style.mark_range(i0, n);
            }
        }
        slot
    }

    /// 場所の持ち主を書く
    fn own(&mut self, slot: Slot, owner: u32) {
        let track = self.track_dirty;
        if slot.is_text_inst() {
            let (t0, tn) = (slot.a_first as usize, slot.a_count as usize);
            if tn > 0 {
                self.text_owner.fill_range(t0, t0 + tn, owner);
                if track {
                    self.dirty.text_owner.mark_range(t0, t0 + tn);
                }
            }
            let (i0, in_) = (
                slot.b_first as usize,
                (slot.b_count_tag & !SLOT_TEXT_INST) as usize,
            );
            if in_ > 0 {
                self.instance_owner.fill_range(i0, i0 + in_, owner);
                if track {
                    self.dirty.instance_owner.mark_range(i0, i0 + in_);
                }
            }
        } else {
            let (s0, sn) = (slot.a_first as usize, slot.a_count as usize);
            if sn > 0 {
                self.segment_owner.fill_range(s0, s0 + sn, owner);
                if track {
                    self.dirty.segment_owner.mark_range(s0, s0 + sn);
                }
            }
            let (a0, an) = (slot.b_first as usize, slot.b_count_tag as usize);
            if an > 0 {
                self.arc_owner.fill_range(a0, a0 + an, owner);
                if track {
                    self.dirty.arc_owner.mark_range(a0, a0 + an);
                }
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
                if self.track_dirty {
                    self.dirty.segments.truncate(n);
                    self.dirty.segment_owner.truncate(n);
                    self.dirty.segment_style.truncate(n);
                }
            }
            if slot.arcs() > 0 {
                let n = slot.arc() as usize;
                self.arcs.truncate(n);
                self.arc_owner.truncate(n);
                self.arc_style.truncate(n);
                if self.track_dirty {
                    self.dirty.arcs.truncate(n);
                    self.dirty.arc_owner.truncate(n);
                    self.dirty.arc_style.truncate(n);
                }
            }
            if slot.texts() > 0 {
                let n = slot.text() as usize;
                self.texts.truncate(n);
                self.text_owner.truncate(n);
                self.text_style.truncate(n);
                if self.track_dirty {
                    self.dirty.texts.truncate(n);
                    self.dirty.text_owner.truncate(n);
                    self.dirty.text_style.truncate(n);
                }
            }
            if slot.instances() > 0 {
                let n = slot.instance() as usize;
                self.instances.truncate(n);
                self.instance_ref.truncate(n);
                self.instance_owner.truncate(n);
                self.instance_style.truncate(n);
                if self.track_dirty {
                    self.dirty.instances.truncate(n);
                    self.dirty.instance_ref.truncate(n);
                    self.dirty.instance_owner.truncate(n);
                    self.dirty.instance_style.truncate(n);
                }
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

    #[test]
    fn dirty_blocks_sync_gpu_mirror_with_minimal_bytes() {
        let pools = Pools::default();
        let mut display = Display::with_rows(5000);
        for i in 0..5000 {
            let x = f64::from(i);
            display.push(&Geom::Line([x, 0.0, x + 1.0, 1.0]), &pools, [0, 1]);
        }
        let mut mirror = display.clone();
        display.clear_dirty();
        assert!(!display.has_dirty());
        assert_eq!(display.dirty_bytes(), 0);

        // 1 本だけ動かす: `segments` の 64 要素ブロック 1 個（64 × 32 ＝ 2,048 バイト）だけが汚れる
        display.update(2500, &Geom::Line([99.0, 5.0, 100.0, 6.0]), &pools, [0, 1]);
        assert!(display.has_dirty());
        assert_eq!(display.dirty_bytes(), DIRTY_BLOCK * 32);
        display.sync_gpu_mirror(&mut mirror);
        assert_eq!(display, mirror);
        assert!(!display.has_dirty());

        // 1 本の色だけ変える: `segment_style` の 64 要素ブロック 1 個（64 × 8 ＝ 512 バイト）だけが汚れる
        display.update(2500, &Geom::Line([99.0, 5.0, 100.0, 6.0]), &pools, [0, 5]);
        assert_eq!(display.dirty_bytes(), DIRTY_BLOCK * 8);
        display.sync_gpu_mirror(&mut mirror);
        assert_eq!(display, mirror);

        // 1 本を隠す: `segment_owner` の 64 要素ブロック 1 個（64 × 4 ＝ 256 バイト）だけが汚れる
        display.hide(1200);
        assert_eq!(display.dirty_bytes(), DIRTY_BLOCK * 4);
        display.sync_gpu_mirror(&mut mirror);
        assert_eq!(display, mirror);

        // 追加して取り消す（pop）もミラーと完全一致する
        display.push(&Geom::Circle([10.0, 20.0, 3.0]), &pools, [2, 4]);
        assert!(display.has_dirty());
        display.sync_gpu_mirror(&mut mirror);
        assert_eq!(display, mirror);

        display.pop();
        assert!(display.has_dirty());
        display.sync_gpu_mirror(&mut mirror);
        assert_eq!(display, mirror);
    }
}
