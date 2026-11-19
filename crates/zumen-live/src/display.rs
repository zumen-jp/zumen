//! 差分で更新する 2D の表示のバッファー（`docs/live.md`、#878）。
//!
//! 線分・円弧・文字・記号の挿入を、要素ごとの値のままページ配列 [`Paged`] に持つ（頂点への分割や字形、
//! 定義の中身を挿入ごとに展開することは描く側で行う）。点は `f64` を上位と下位の `f32` に分けて置き、
//! 視点からの相対の位置を GPU 側で作れるようにする。
//!
//! 変更のときは変わった要素の分だけを書き直し、GPU 13 列ごとの変更範囲を 64 要素の塊単位で [`DisplayDirty`] に
//! 記録する。除いた行やプリミティブ数の変わった行は、元の場所の持ち主を [`NO_OWNER`] にして後ろに置き直すため、
//! 配列の並びは開き直した直後とは変わりうるが、生きている行ごとの値（[`Display::row`]）と描画順は一致する。

use zumen_base::{Cx, Exceeded, Kind, Stop};

use crate::dirty::DirtyColumn;
use crate::paged::Paged;

/// 使っていない場所（除かれた行や置き直された旧スロット）の持ち主番号
pub const NO_OWNER: u32 = u32::MAX;

const TAG_MASK: u32 = 0xC000_0000;
const TAG_SEG_ARC: u32 = 0x0000_0000;
const TAG_TEXT_INST: u32 = 0x4000_0000;
const TAG_MIXED: u32 = 0x8000_0000;
const MAX_INLINE_B_COUNT: u32 = 0x3FFF_FFFF;

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

/// 線分・円弧と文字・挿入の両方を同時に持つ行（寸法・引出線・表など）の場所
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct MixedSlot {
    first: [u32; 4],
    count: [u32; 4],
}

/// 1 つの行が表示のバッファーの中で使う場所（16 バイト固定）。
///
/// 「線分＋円弧」または「文字＋挿入」だけの要素は間接参照なしでこの 16 バイトの中に収め、
/// 両方を併せ持つ要素（寸法・引出線・表など）だけを `Display` 内部の補助配列へ逃がす。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Slot {
    a_first: u32,
    a_count: u32,
    b_first: u32,
    b_count_tag: u32,
}

impl Slot {
    #[inline]
    const fn tag(self) -> u32 {
        self.b_count_tag & TAG_MASK
    }

    /// 線分・円弧と文字・挿入の両方を持つ複合スロットか
    #[inline]
    pub const fn is_mixed(self) -> bool {
        self.tag() == TAG_MIXED
    }
}

const _: () = assert!(core::mem::size_of::<Slot>() == 16);

/// 1 つの行が持つ線分・円弧・文字・挿入の値（[`Display::row`]）
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

/// 差分で更新する 2D の表示のバッファー
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
    /// 挿入が指す定義の番号と、列・行の数
    pub instance_ref: Paged<[u32; 3]>,
    /// 挿入の持ち主
    pub instance_owner: Paged<u32>,
    /// 挿入のスタイル
    pub instance_style: Paged<[u32; 2]>,
    /// 行ごとの場所
    pub slots: Paged<Slot>,
    /// 複合スロットの補助配列
    mixed_slots: Vec<MixedSlot>,
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
            mixed_slots: Vec::new(),
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
            && self.mixed_slots == other.mixed_slots
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

#[inline]
fn write_f32_checked<const N: usize>(
    col: &mut Paged<[f32; N]>,
    dirty: &mut DirtyColumn,
    track: bool,
    at: usize,
    val: [f32; N],
) {
    if let Some(place) = col.get_mut(at)
        && !same_f32(place, &val)
    {
        *place = val;
        if track {
            dirty.mark_one(at);
        }
    }
}

#[inline]
fn write_u32_arr_checked<const N: usize>(
    col: &mut Paged<[u32; N]>,
    dirty: &mut DirtyColumn,
    track: bool,
    at: usize,
    val: [u32; N],
) {
    if let Some(place) = col.get_mut(at)
        && *place != val
    {
        *place = val;
        if track {
            dirty.mark_one(at);
        }
    }
}

/// `f64` を上位と下位の `f32` に分ける
#[inline]
pub fn split_f64(value: f64) -> [f32; 2] {
    let high = value as f32;
    [high, (value - f64::from(high)) as f32]
}

/// 2 点を結ぶ線分の GPU 表現を作る
#[inline]
pub fn segment_f32([x0, y0]: [f64; 2], [x1, y1]: [f64; 2]) -> [f32; 8] {
    let ([a, b], [c, d], [e, f], [g, h]) =
        (split_f64(x0), split_f64(y0), split_f64(x1), split_f64(y1));
    [a, b, c, d, e, f, g, h]
}

/// 中心・半径・始角・掃引角（ラジアン）から円弧の GPU 表現を作る
#[inline]
pub fn arc_f32([x, y]: [f64; 2], radius: f64, start_rad: f64, sweep_rad: f64) -> [f32; 7] {
    let ([a, b], [c, d]) = (split_f64(x), split_f64(y));
    [
        a,
        b,
        c,
        d,
        radius as f32,
        start_rad as f32,
        sweep_rad as f32,
    ]
}

/// 文字の位置・高さ・回転（ラジアン）・幅係数・傾き（ラジアン）から文字の GPU 表現を作る
#[inline]
pub fn text_f32(
    [x, y]: [f64; 2],
    height: f64,
    rotation_rad: f64,
    width_factor: f64,
    oblique_rad: f64,
) -> [f32; 8] {
    let ([a, b], [c, d]) = (split_f64(x), split_f64(y));
    [
        a,
        b,
        c,
        d,
        height as f32,
        rotation_rad as f32,
        width_factor as f32,
        oblique_rad as f32,
    ]
}

/// 挿入の位置・尺度・回転（ラジアン）・列と行の間隔から挿入の GPU 表現を作る
#[inline]
pub fn instance_f32(
    [x, y]: [f64; 2],
    scale: [f64; 2],
    rotation_rad: f64,
    spacing: [f64; 2],
) -> [f32; 9] {
    let ([a, b], [c, d]) = (split_f64(x), split_f64(y));
    [
        a,
        b,
        c,
        d,
        scale[0] as f32,
        scale[1] as f32,
        rotation_rad as f32,
        spacing[0] as f32,
        spacing[1] as f32,
    ]
}

/// 度をラジアンに直す
#[inline]
pub fn radians_from_deg(degrees: f64) -> f64 {
    degrees * (core::f64::consts::PI / 180.0)
}

/// 始まりの角 `start_deg` から終わりの角 `end_deg` へ反時計回りに進む角（度、`(0.0, 360.0]`）
#[inline]
pub fn ccw_sweep_deg(start_deg: f64, end_deg: f64) -> f64 {
    match (end_deg - start_deg).rem_euclid(360.0) {
        0.0 => 360.0,
        sweep => sweep,
    }
}

/// 膨らみ `bulge` で `from` から `to` へ進む円弧の GPU 表現を作る
pub fn bulged(from: [f64; 2], to: [f64; 2], bulge: f64) -> [f32; 7] {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    // 中心は、弦の中点から左の法線の向きに (1 - b²) / 4b だけ離れる
    let offset = (1.0 - bulge * bulge) / (4.0 * bulge);
    let center = [
        (from[0] + to[0]) / 2.0 - dy * offset,
        (from[1] + to[1]) / 2.0 + dx * offset,
    ];
    let chord = libm::sqrt(dx * dx + dy * dy);
    let radius = chord * (1.0 + bulge * bulge) / (4.0 * bulge.abs());
    let start = libm::atan2(from[1] - center[1], from[0] - center[0]);
    arc_f32(center, radius, start, 4.0 * libm::atan(bulge))
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

/// 1 行分のプリミティブ列を所定のスロット範囲へ順に書き込む書き手
pub struct DisplayWriter<'a> {
    display: &'a mut Display,
    default_style: [u32; 2],
    at: [usize; 4],
    end: [usize; 4],
}

impl DisplayWriter<'_> {
    /// 既定スタイルの線分を 1 本書く
    #[inline]
    pub fn push_segment(&mut self, from: [f64; 2], to: [f64; 2]) {
        self.push_segment_styled(from, to, self.default_style);
    }

    /// スタイル指定付きの線分を 1 本書く
    #[inline]
    pub fn push_segment_styled(&mut self, from: [f64; 2], to: [f64; 2], style: [u32; 2]) {
        self.push_segment_raw(segment_f32(from, to), style);
    }

    /// 変換済みの線分値を 1 本書く
    #[inline]
    pub fn push_segment_raw(&mut self, val: [f32; 8], style: [u32; 2]) {
        if self.at[0] >= self.end[0] {
            return;
        }
        let at = self.at[0];
        self.at[0] += 1;
        let track = self.display.track_dirty;
        write_f32_checked(
            &mut self.display.segments,
            &mut self.display.dirty.segments,
            track,
            at,
            val,
        );
        write_u32_arr_checked(
            &mut self.display.segment_style,
            &mut self.display.dirty.segment_style,
            track,
            at,
            style,
        );
    }

    /// 既定スタイルの円（全周の円弧）を 1 つ書く
    #[inline]
    pub fn push_circle(&mut self, center: [f64; 2], radius: f64) {
        self.push_arc_rad_styled(
            center,
            radius,
            0.0,
            core::f64::consts::TAU,
            self.default_style,
        );
    }

    /// 既定スタイルの円弧（始角・終角とも度、反時計回り）を 1 つ書く
    #[inline]
    pub fn push_arc_deg(&mut self, center: [f64; 2], radius: f64, start_deg: f64, end_deg: f64) {
        let sweep_deg = ccw_sweep_deg(start_deg, end_deg);
        self.push_arc_rad_styled(
            center,
            radius,
            radians_from_deg(start_deg),
            radians_from_deg(sweep_deg),
            self.default_style,
        );
    }

    /// 既定スタイルの円弧（始角・掃引角ともラジアン）を 1 つ書く
    #[inline]
    pub fn push_arc_rad(&mut self, center: [f64; 2], radius: f64, start_rad: f64, sweep_rad: f64) {
        self.push_arc_rad_styled(center, radius, start_rad, sweep_rad, self.default_style);
    }

    /// スタイル指定付きの円弧（始角・掃引角ともラジアン）を 1 つ書く
    #[inline]
    pub fn push_arc_rad_styled(
        &mut self,
        center: [f64; 2],
        radius: f64,
        start_rad: f64,
        sweep_rad: f64,
        style: [u32; 2],
    ) {
        self.push_arc_raw(arc_f32(center, radius, start_rad, sweep_rad), style);
    }

    /// 既定スタイルの膨らみ付き辺（円弧）を 1 つ書く
    #[inline]
    pub fn push_bulged(&mut self, from: [f64; 2], to: [f64; 2], bulge: f64) {
        self.push_arc_raw(bulged(from, to, bulge), self.default_style);
    }

    /// 変換済みの円弧値を 1 つ書く
    #[inline]
    pub fn push_arc_raw(&mut self, val: [f32; 7], style: [u32; 2]) {
        if self.at[1] >= self.end[1] {
            return;
        }
        let at = self.at[1];
        self.at[1] += 1;
        let track = self.display.track_dirty;
        write_f32_checked(
            &mut self.display.arcs,
            &mut self.display.dirty.arcs,
            track,
            at,
            val,
        );
        write_u32_arr_checked(
            &mut self.display.arc_style,
            &mut self.display.dirty.arc_style,
            track,
            at,
            style,
        );
    }

    /// 既定スタイルの文字を 1 つ書く
    #[inline]
    pub fn push_text(
        &mut self,
        point: [f64; 2],
        height: f64,
        rotation_rad: f64,
        width_factor: f64,
        oblique_rad: f64,
    ) {
        self.push_text_styled(
            point,
            height,
            rotation_rad,
            width_factor,
            oblique_rad,
            self.default_style,
        );
    }

    /// スタイル指定付きの文字を 1 つ書く
    #[inline]
    pub fn push_text_styled(
        &mut self,
        point: [f64; 2],
        height: f64,
        rotation_rad: f64,
        width_factor: f64,
        oblique_rad: f64,
        style: [u32; 2],
    ) {
        self.push_text_raw(
            text_f32(point, height, rotation_rad, width_factor, oblique_rad),
            style,
        );
    }

    /// 変換済みの文字値を 1 つ書く
    #[inline]
    pub fn push_text_raw(&mut self, val: [f32; 8], style: [u32; 2]) {
        if self.at[2] >= self.end[2] {
            return;
        }
        let at = self.at[2];
        self.at[2] += 1;
        let track = self.display.track_dirty;
        write_f32_checked(
            &mut self.display.texts,
            &mut self.display.dirty.texts,
            track,
            at,
            val,
        );
        write_u32_arr_checked(
            &mut self.display.text_style,
            &mut self.display.dirty.text_style,
            track,
            at,
            style,
        );
    }

    /// 既定スタイルの挿入を 1 つ書く
    #[inline]
    pub fn push_instance(
        &mut self,
        position: [f64; 2],
        scale: [f64; 2],
        rotation_rad: f64,
        spacing: [f64; 2],
        definition: u32,
        grid: [u32; 2],
    ) {
        self.push_instance_styled(
            position,
            scale,
            rotation_rad,
            spacing,
            definition,
            grid,
            self.default_style,
        );
    }

    /// スタイル指定付きの挿入を 1 つ書く
    #[inline]
    #[expect(clippy::too_many_arguments)]
    pub fn push_instance_styled(
        &mut self,
        position: [f64; 2],
        scale: [f64; 2],
        rotation_rad: f64,
        spacing: [f64; 2],
        definition: u32,
        grid: [u32; 2],
        style: [u32; 2],
    ) {
        self.push_instance_raw(
            instance_f32(position, scale, rotation_rad, spacing),
            [definition, grid[0], grid[1]],
            style,
        );
    }

    /// 変換済みの挿入値と定義参照を 1 つ書く
    #[inline]
    pub fn push_instance_raw(&mut self, val: [f32; 9], iref: [u32; 3], style: [u32; 2]) {
        if self.at[3] >= self.end[3] {
            return;
        }
        let at = self.at[3];
        self.at[3] += 1;
        let track = self.display.track_dirty;
        write_f32_checked(
            &mut self.display.instances,
            &mut self.display.dirty.instances,
            track,
            at,
            val,
        );
        write_u32_arr_checked(
            &mut self.display.instance_ref,
            &mut self.display.dirty.instance_ref,
            track,
            at,
            iref,
        );
        write_u32_arr_checked(
            &mut self.display.instance_style,
            &mut self.display.dirty.instance_style,
            track,
            at,
            style,
        );
    }

    /// 書き残したスロットがあれば既定スタイルで埋める
    fn finish(self) {
        let track = self.display.track_dirty;
        let style = self.default_style;
        for at in self.at[0]..self.end[0] {
            write_u32_arr_checked(
                &mut self.display.segment_style,
                &mut self.display.dirty.segment_style,
                track,
                at,
                style,
            );
        }
        for at in self.at[1]..self.end[1] {
            write_u32_arr_checked(
                &mut self.display.arc_style,
                &mut self.display.dirty.arc_style,
                track,
                at,
                style,
            );
        }
        for at in self.at[2]..self.end[2] {
            write_u32_arr_checked(
                &mut self.display.text_style,
                &mut self.display.dirty.text_style,
                track,
                at,
                style,
            );
        }
        for at in self.at[3]..self.end[3] {
            write_u32_arr_checked(
                &mut self.display.instance_style,
                &mut self.display.dirty.instance_style,
                track,
                at,
                style,
            );
        }
    }
}

impl Display {
    /// 行の数だけ場所の表を用意する（初回構築中は変更追跡を止め、終わったら [`Self::clear_dirty`] を呼ぶ）
    pub fn with_rows(rows: usize) -> Self {
        Self {
            slots: Paged::with_capacity(rows),
            track_dirty: false,
            ..Self::default()
        }
    }

    /// 予算と中断を確かめながら、行の数だけ場所の表を用意する
    pub fn with_rows_with_cx(cx: &mut Cx<'_>, rows: usize) -> Result<Self, Stop> {
        if rows > NO_OWNER as usize {
            return Err(Stop::Exceeded(Exceeded {
                kind: Kind::Elements,
                limit: u64::from(NO_OWNER),
                requested: rows as u64,
            }));
        }
        let slot_bytes = (rows as u64).saturating_mul(core::mem::size_of::<Slot>() as u64);
        cx.check(Kind::Work, 1)?;
        cx.check(Kind::DecodedBytes, slot_bytes)?;
        Ok(Self::with_rows(rows))
    }

    /// スロットが指す 4 列それぞれの先頭位置と個数を返す
    #[inline]
    fn slot_ranges(&self, slot: Slot) -> ([u32; 4], [u32; 4]) {
        match slot.tag() {
            TAG_SEG_ARC => (
                [slot.a_first, slot.b_first, 0, 0],
                [slot.a_count, slot.b_count_tag & MAX_INLINE_B_COUNT, 0, 0],
            ),
            TAG_TEXT_INST => (
                [0, 0, slot.a_first, slot.b_first],
                [0, 0, slot.a_count, slot.b_count_tag & MAX_INLINE_B_COUNT],
            ),
            TAG_MIXED => self
                .mixed_slots
                .get(slot.b_first as usize)
                .map_or(([0; 4], [0; 4]), |m| (m.first, m.count)),
            _ => ([0; 4], [0; 4]),
        }
    }

    /// 行 `row` が確保している線分・円弧・文字・挿入の数を返す
    pub fn slot_counts(&self, row: u32) -> [u32; 4] {
        self.slots
            .get(row as usize)
            .map_or([0; 4], |&slot| self.slot_ranges(slot).1)
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

    /// すべての GPU 列を未転送（ダーティ）にして、全体同期を要求する
    pub fn mark_all_dirty(&mut self) {
        self.dirty.segments.mark_range(0, self.segments.len());
        self.dirty
            .segment_owner
            .mark_range(0, self.segment_owner.len());
        self.dirty
            .segment_style
            .mark_range(0, self.segment_style.len());
        self.dirty.arcs.mark_range(0, self.arcs.len());
        self.dirty.arc_owner.mark_range(0, self.arc_owner.len());
        self.dirty.arc_style.mark_range(0, self.arc_style.len());
        self.dirty.texts.mark_range(0, self.texts.len());
        self.dirty.text_owner.mark_range(0, self.text_owner.len());
        self.dirty.text_style.mark_range(0, self.text_style.len());
        self.dirty.instances.mark_range(0, self.instances.len());
        self.dirty
            .instance_ref
            .mark_range(0, self.instance_ref.len());
        self.dirty
            .instance_owner
            .mark_range(0, self.instance_owner.len());
        self.dirty
            .instance_style
            .mark_range(0, self.instance_style.len());
        self.track_dirty = true;
    }

    /// 前回の [`Self::clear_dirty`] から GPU 列に変化があったか
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
        mirror.mixed_slots.clone_from(&self.mixed_slots);
        mirror.dead = self.dead;
        self.clear_dirty();
    }

    /// `counts` 個分の追加が `u32` 上限内に収まるかを確かめ、新しい長さと追加バイト数を返す
    fn check_reserve_limits(
        &self,
        counts: [u32; 4],
        include_slot: bool,
    ) -> Result<([usize; 4], bool, u64), Stop> {
        let cur = [
            self.segments.len(),
            self.arcs.len(),
            self.texts.len(),
            self.instances.len(),
        ];
        let mut next = [0usize; 4];
        for ((dst, &old_len), &c) in next.iter_mut().zip(&cur).zip(&counts) {
            let requested = (old_len as u64).saturating_add(u64::from(c));
            let n = old_len
                .checked_add(c as usize)
                .ok_or(Stop::Exceeded(Exceeded {
                    kind: Kind::Elements,
                    limit: u64::from(u32::MAX),
                    requested,
                }))?;
            if n > u32::MAX as usize {
                return Err(Stop::Exceeded(Exceeded {
                    kind: Kind::Elements,
                    limit: u64::from(u32::MAX),
                    requested,
                }));
            }
            *dst = n;
        }
        let inline = (counts[2] == 0 && counts[3] == 0 && counts[1] <= MAX_INLINE_B_COUNT)
            || (counts[0] == 0 && counts[1] == 0 && counts[3] <= MAX_INLINE_B_COUNT);
        let needs_mixed = if inline {
            false
        } else {
            if self.mixed_slots.len() >= u32::MAX as usize {
                return Err(Stop::Exceeded(Exceeded {
                    kind: Kind::Elements,
                    limit: u64::from(u32::MAX),
                    requested: (self.mixed_slots.len() as u64).saturating_add(1),
                }));
            }
            true
        };
        let mut bytes = u64::from(counts[0]) * (32 + 4 + 8)
            + u64::from(counts[1]) * (28 + 4 + 8)
            + u64::from(counts[2]) * (32 + 4 + 8)
            + u64::from(counts[3]) * (36 + 12 + 4 + 8);
        if include_slot {
            bytes = bytes.saturating_add(core::mem::size_of::<Slot>() as u64);
        }
        if needs_mixed {
            bytes = bytes.saturating_add(core::mem::size_of::<MixedSlot>() as u64);
        }
        Ok((next, needs_mixed, bytes))
    }

    /// 検証済みの長さ `next` に従って各列の末尾に場所を確保し、`(Slot, firsts)` を返す
    fn reserve_unchecked(
        &mut self,
        row: u32,
        counts: [u32; 4],
        next: [usize; 4],
        needs_mixed: bool,
    ) -> (Slot, [u32; 4]) {
        let firsts = [
            self.segments.len() as u32,
            self.arcs.len() as u32,
            self.texts.len() as u32,
            self.instances.len() as u32,
        ];
        let slot = if needs_mixed {
            let idx = self.mixed_slots.len() as u32;
            self.mixed_slots.push(MixedSlot {
                first: firsts,
                count: counts,
            });
            Slot {
                a_first: 0,
                a_count: 0,
                b_first: idx,
                b_count_tag: TAG_MIXED,
            }
        } else if counts[2] > 0 || counts[3] > 0 {
            Slot {
                a_first: firsts[2],
                a_count: counts[2],
                b_first: firsts[3],
                b_count_tag: counts[3] | TAG_TEXT_INST,
            }
        } else {
            Slot {
                a_first: firsts[0],
                a_count: counts[0],
                b_first: firsts[1],
                b_count_tag: counts[1] | TAG_SEG_ARC,
            }
        };
        let track = self.track_dirty;
        if counts[0] > 0 {
            let s0 = firsts[0] as usize;
            let n = next[0];
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
            let a0 = firsts[1] as usize;
            let n = next[1];
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
            let t0 = firsts[2] as usize;
            let n = next[2];
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
            let i0 = firsts[3] as usize;
            let n = next[3];
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
        (slot, firsts)
    }

    fn write_at(
        &mut self,
        firsts: [u32; 4],
        counts: [u32; 4],
        default_style: [u32; 2],
        write_fn: impl FnOnce(&mut DisplayWriter<'_>),
    ) {
        let at = [
            firsts[0] as usize,
            firsts[1] as usize,
            firsts[2] as usize,
            firsts[3] as usize,
        ];
        let end = [
            at[0] + counts[0] as usize,
            at[1] + counts[1] as usize,
            at[2] + counts[2] as usize,
            at[3] + counts[3] as usize,
        ];
        let mut writer = DisplayWriter {
            display: self,
            default_style,
            at,
            end,
        };
        write_fn(&mut writer);
        writer.finish();
    }

    /// 指定した範囲の持ち主を書き換える
    fn own_ranges(&mut self, firsts: [u32; 4], counts: [u32; 4], owner: u32) {
        let track = self.track_dirty;
        if counts[0] > 0 {
            let (s0, s1) = (firsts[0] as usize, firsts[0] as usize + counts[0] as usize);
            self.segment_owner.fill_range(s0, s1, owner);
            if track {
                self.dirty.segment_owner.mark_range(s0, s1);
            }
        }
        if counts[1] > 0 {
            let (a0, a1) = (firsts[1] as usize, firsts[1] as usize + counts[1] as usize);
            self.arc_owner.fill_range(a0, a1, owner);
            if track {
                self.dirty.arc_owner.mark_range(a0, a1);
            }
        }
        if counts[2] > 0 {
            let (t0, t1) = (firsts[2] as usize, firsts[2] as usize + counts[2] as usize);
            self.text_owner.fill_range(t0, t1, owner);
            if track {
                self.dirty.text_owner.mark_range(t0, t1);
            }
        }
        if counts[3] > 0 {
            let (i0, i1) = (firsts[3] as usize, firsts[3] as usize + counts[3] as usize);
            self.instance_owner.fill_range(i0, i1, owner);
            if track {
                self.dirty.instance_owner.mark_range(i0, i1);
            }
        }
    }

    /// 予算と中断を確かめながら、次の行の要素を末尾に足し、割り当てた行番号を返す
    pub fn push_with_cx(
        &mut self,
        cx: &mut Cx<'_>,
        counts: [u32; 4],
        style: [u32; 2],
        write_fn: impl FnOnce(&mut DisplayWriter<'_>),
    ) -> Result<u32, Stop> {
        if self.slots.len() >= NO_OWNER as usize {
            return Err(Stop::Exceeded(Exceeded {
                kind: Kind::Elements,
                limit: u64::from(NO_OWNER),
                requested: (self.slots.len() as u64).saturating_add(1),
            }));
        }
        let row = self.slots.len() as u32;
        let total = u64::from(counts[0])
            + u64::from(counts[1])
            + u64::from(counts[2])
            + u64::from(counts[3]);
        let (next, needs_mixed, bytes) = self.check_reserve_limits(counts, true)?;
        cx.check(Kind::Work, total.saturating_add(1))?;
        cx.check(Kind::DecodedBytes, bytes)?;
        let (slot, firsts) = self.reserve_unchecked(row, counts, next, needs_mixed);
        self.write_at(firsts, counts, style, write_fn);
        self.slots.push(slot);
        Ok(row)
    }

    /// 次の行の要素を末尾に足す（上限なしの簡易版。`u32` の添字上限だけは保つ）
    pub fn push_with(
        &mut self,
        counts: [u32; 4],
        style: [u32; 2],
        write_fn: impl FnOnce(&mut DisplayWriter<'_>),
    ) {
        if self.slots.len() >= NO_OWNER as usize {
            return;
        }
        let Ok((next, needs_mixed, _)) = self.check_reserve_limits(counts, true) else {
            return;
        };
        let row = self.slots.len() as u32;
        let (slot, firsts) = self.reserve_unchecked(row, counts, next, needs_mixed);
        self.write_at(firsts, counts, style, write_fn);
        self.slots.push(slot);
    }

    /// 終わりの行を除く（足した行を取り消すときに使う）。
    /// 場所が配列の末尾にあれば切り詰め、なければ使わない場所として残す
    pub fn pop(&mut self) {
        let Some(slot) = self.slots.pop() else {
            return;
        };
        let (firsts, counts) = self.slot_ranges(slot);
        let at_end = |first: u32, count: u32, len: usize| {
            count == 0 || (first as usize).saturating_add(count as usize) == len
        };
        if at_end(firsts[0], counts[0], self.segments.len())
            && at_end(firsts[1], counts[1], self.arcs.len())
            && at_end(firsts[2], counts[2], self.texts.len())
            && at_end(firsts[3], counts[3], self.instances.len())
        {
            if counts[0] > 0 {
                let n = firsts[0] as usize;
                self.segments.truncate(n);
                self.segment_owner.truncate(n);
                self.segment_style.truncate(n);
                if self.track_dirty {
                    self.dirty.segments.truncate(n);
                    self.dirty.segment_owner.truncate(n);
                    self.dirty.segment_style.truncate(n);
                }
            }
            if counts[1] > 0 {
                let n = firsts[1] as usize;
                self.arcs.truncate(n);
                self.arc_owner.truncate(n);
                self.arc_style.truncate(n);
                if self.track_dirty {
                    self.dirty.arcs.truncate(n);
                    self.dirty.arc_owner.truncate(n);
                    self.dirty.arc_style.truncate(n);
                }
            }
            if counts[2] > 0 {
                let n = firsts[2] as usize;
                self.texts.truncate(n);
                self.text_owner.truncate(n);
                self.text_style.truncate(n);
                if self.track_dirty {
                    self.dirty.texts.truncate(n);
                    self.dirty.text_owner.truncate(n);
                    self.dirty.text_style.truncate(n);
                }
            }
            if counts[3] > 0 {
                let n = firsts[3] as usize;
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
            if slot.is_mixed()
                && (slot.b_first as usize).saturating_add(1) == self.mixed_slots.len()
            {
                self.mixed_slots.pop();
            }
        } else {
            self.own_ranges(firsts, counts, NO_OWNER);
            let total = counts.iter().fold(0u32, |acc, &c| acc.saturating_add(c));
            self.dead = self.dead.saturating_add(total);
        }
    }

    /// 行 `row` の要素を描かないようにする。場所は残す（取り消しで戻す）
    pub fn hide(&mut self, row: u32) {
        if let Some(&slot) = self.slots.get(row as usize) {
            let (firsts, counts) = self.slot_ranges(slot);
            self.own_ranges(firsts, counts, NO_OWNER);
        }
    }

    /// [`Self::hide`] で隠した行を戻す
    pub fn show(&mut self, row: u32) {
        if let Some(&slot) = self.slots.get(row as usize) {
            let (firsts, counts) = self.slot_ranges(slot);
            self.own_ranges(firsts, counts, row);
        }
    }

    /// 予算と中断を確かめながら、行 `row` の要素を書き直す。
    /// 線分・円弧・文字・挿入の数が変わらなければ同じ場所に書き、変われば後ろに置き直す
    pub fn update_with_cx(
        &mut self,
        cx: &mut Cx<'_>,
        row: u32,
        counts: [u32; 4],
        style: [u32; 2],
        write_fn: impl FnOnce(&mut DisplayWriter<'_>),
    ) -> Result<(), Stop> {
        let Some(&old_slot) = self.slots.get(row as usize) else {
            return Ok(());
        };
        let (old_firsts, old_counts) = self.slot_ranges(old_slot);
        let total = u64::from(counts[0])
            + u64::from(counts[1])
            + u64::from(counts[2])
            + u64::from(counts[3]);
        cx.check(Kind::Work, total.saturating_add(1))?;
        let firsts = if old_counts == counts {
            old_firsts
        } else {
            let (next, needs_mixed, bytes) = self.check_reserve_limits(counts, false)?;
            cx.check(Kind::DecodedBytes, bytes)?;
            self.own_ranges(old_firsts, old_counts, NO_OWNER);
            let old_total = old_counts
                .iter()
                .fold(0u32, |acc, &c| acc.saturating_add(c));
            self.dead = self.dead.saturating_add(old_total);
            let (new_slot, new_firsts) = self.reserve_unchecked(row, counts, next, needs_mixed);
            if let Some(place) = self.slots.get_mut(row as usize) {
                *place = new_slot;
            }
            new_firsts
        };
        self.write_at(firsts, counts, style, write_fn);
        Ok(())
    }

    /// 行 `row` の要素を書き直す（上限なしの簡易版。`u32` の添字上限だけは保つ）
    pub fn update_with(
        &mut self,
        row: u32,
        counts: [u32; 4],
        style: [u32; 2],
        write_fn: impl FnOnce(&mut DisplayWriter<'_>),
    ) {
        let Some(&old_slot) = self.slots.get(row as usize) else {
            return;
        };
        let (old_firsts, old_counts) = self.slot_ranges(old_slot);
        let firsts = if old_counts == counts {
            old_firsts
        } else {
            let Ok((next, needs_mixed, _)) = self.check_reserve_limits(counts, false) else {
                return;
            };
            self.own_ranges(old_firsts, old_counts, NO_OWNER);
            let old_total = old_counts
                .iter()
                .fold(0u32, |acc, &c| acc.saturating_add(c));
            self.dead = self.dead.saturating_add(old_total);
            let (new_slot, new_firsts) = self.reserve_unchecked(row, counts, next, needs_mixed);
            if let Some(place) = self.slots.get_mut(row as usize) {
                *place = new_slot;
            }
            new_firsts
        };
        self.write_at(firsts, counts, style, write_fn);
    }

    /// 行 `row` の値
    pub fn row(&self, row: u32) -> RowBuffers {
        let Some(&slot) = self.slots.get(row as usize) else {
            return RowBuffers::default();
        };
        let (firsts, counts) = self.slot_ranges(slot);
        let seg_start = firsts[0] as usize;
        let seg_end = seg_start + counts[0] as usize;
        let arc_start = firsts[1] as usize;
        let arc_end = arc_start + counts[1] as usize;
        let text_start = firsts[2] as usize;
        let text_end = text_start + counts[2] as usize;
        let inst_start = firsts[3] as usize;
        let inst_end = inst_start + counts[3] as usize;
        RowBuffers {
            segments: self.segments.slice_to_vec(seg_start, seg_end),
            arcs: self.arcs.slice_to_vec(arc_start, arc_end),
            texts: self.texts.slice_to_vec(text_start, text_end),
            instances: self
                .instances
                .slice_to_vec(inst_start, inst_end)
                .into_iter()
                .zip(self.instance_ref.slice_to_vec(inst_start, inst_end))
                .collect(),
        }
    }

    /// 行 `row` の線分・円弧・文字・挿入のスタイル（画層・色の指定）
    pub fn row_styles(&self, row: u32) -> [Vec<[u32; 2]>; 4] {
        let Some(&slot) = self.slots.get(row as usize) else {
            return Default::default();
        };
        let (firsts, counts) = self.slot_ranges(slot);
        let [seg_first, arc_first, text_first, inst_first] = firsts.map(|first| first as usize);
        let [seg_count, arc_count, text_count, inst_count] = counts.map(|count| count as usize);
        let (segments, arcs, texts, instances) = (
            (seg_first, seg_first + seg_count),
            (arc_first, arc_first + arc_count),
            (text_first, text_first + text_count),
            (inst_first, inst_first + inst_count),
        );
        [
            self.segment_style.slice_to_vec(segments.0, segments.1),
            self.arc_style.slice_to_vec(arcs.0, arcs.1),
            self.text_style.slice_to_vec(texts.0, texts.1),
            self.instance_style.slice_to_vec(instances.0, instances.1),
        ]
    }

    /// すべての行の値を行の順に並べたもの
    pub fn canonical(&self) -> Vec<RowBuffers> {
        (0..self.slots.len() as u32)
            .map(|row| self.row(row))
            .collect()
    }

    /// バッファーが使うバイト数
    pub fn bytes(&self) -> usize {
        self.segments.len() * (32 + 4 + 8)
            + self.arcs.len() * (28 + 4 + 8)
            + self.texts.len() * (32 + 4 + 8)
            + self.instances.len() * (36 + 12 + 4 + 8)
            + self.slots.len() * core::mem::size_of::<Slot>()
            + self.mixed_slots.len() * core::mem::size_of::<MixedSlot>()
    }
}

/// 表示のスタイルの 2 つ目の値（色）の符号化。どの形式も同じ読み方にする。
///
/// 上位 8 bit が種類で、0 は画層の色（または指定なし）、1 はブロックの色、2 は色番号、3 は下位 24 bit の RGB
/// （`0xRRGGBB`）。色番号は、bit 16〜23 に体系（[`ACI`]・[`SXF`]・[`JWW`]。Layers の表と同じ値）、
/// 下位 16 bit に番号を置く
pub mod color {
    /// 画層の色（または指定なし）
    pub const LAYER: u32 = 0;
    /// ブロックの色
    pub const BLOCK: u32 = 1 << 24;
    /// 色番号の種類
    pub const CODE: u32 = 2 << 24;
    /// RGB の種類
    pub const RGB: u32 = 3 << 24;
    /// 色番号の体系: AutoCAD の色の番号（ACI）
    pub const ACI: u8 = 1;
    /// 色番号の体系: SXF の色のコード
    pub const SXF: u8 = 2;
    /// 色番号の体系: Jw_cad のペンの番号
    pub const JWW: u8 = 3;

    /// 体系 `system` の番号 `value` の色。番号が 16 bit に収まらなければ、画層の色（[`LAYER`]）
    pub const fn code(system: u8, value: u32) -> u32 {
        if value > 0xFFFF {
            return LAYER;
        }
        CODE | ((system as u32) << 16) | value
    }

    /// RGB の色
    pub const fn rgb([r, g, b]: [u8; 3]) -> u32 {
        RGB | ((r as u32) << 16) | ((g as u32) << 8) | b as u32
    }

    /// 色番号なら、その体系と番号
    pub const fn code_of(bits: u32) -> Option<(u8, u32)> {
        if bits & 0xFF00_0000 == CODE {
            Some((((bits >> 16) & 0xFF) as u8, bits & 0xFFFF))
        } else {
            None
        }
    }

    /// RGB なら、その値
    pub const fn rgb_of(bits: u32) -> Option<[u8; 3]> {
        if bits & 0xFF00_0000 == RGB {
            Some([(bits >> 16) as u8, (bits >> 8) as u8, bits as u8])
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};

    use super::*;
    use crate::dirty::DIRTY_BLOCK;

    #[test]
    fn split_f64_preserves_sub_f32_precision() {
        let value = 123_456_789.123_456_79_f64;
        let [high, low] = split_f64(value);
        let error = (f64::from(high) + f64::from(low) - value).abs();
        assert!(error <= value / (1u64 << 47) as f64, "{error}");
        assert!((f64::from(value as f32) - value).abs() > 1.0);
        let [high, low] = split_f64(1_234_567.25);
        assert_eq!(f64::from(high) + f64::from(low), 1_234_567.25);
    }

    #[test]
    fn bulged_one_is_half_circle() {
        let [.., radius, start, sweep] = bulged([0.0, 0.0], [2.0, 0.0], 1.0);
        assert_eq!(radius, 1.0);
        assert!((start - core::f32::consts::PI).abs() < 1e-6);
        assert!((sweep - core::f32::consts::PI).abs() < 1e-6);
    }

    #[test]
    fn mixed_slots_support_all_four_primitive_kinds_and_clean_pop() {
        let mut display = Display::with_rows(4);
        display.push_with([1, 0, 0, 0], [1, 2], |w| {
            w.push_segment([0.0, 0.0], [10.0, 0.0]);
        });
        // 寸法のように線分・円弧・文字・矢印の挿入をすべて持つ複合要素
        display.push_with([2, 1, 1, 1], [3, 4], |w| {
            w.push_segment([0.0, 0.0], [0.0, 5.0]);
            w.push_segment([10.0, 0.0], [10.0, 5.0]);
            w.push_arc_deg([5.0, 5.0], 5.0, 0.0, 180.0);
            w.push_text_styled([5.0, 6.0], 2.5, 0.0, 1.0, 0.0, [7, 8]);
            w.push_instance([0.0, 5.0], [1.0, 1.0], 0.0, [0.0, 0.0], 2, [1, 1]);
        });
        assert!(display.slots.get(1).expect("slot").is_mixed());
        assert_eq!(display.slot_counts(1), [2, 1, 1, 1]);
        let row1 = display.row(1);
        assert_eq!(row1.segments.len(), 2);
        assert_eq!(row1.arcs.len(), 1);
        assert_eq!(row1.texts.len(), 1);
        assert_eq!(row1.instances.len(), 1);
        assert_eq!(display.text_style.slice_to_vec(0, 1), [[7, 8]]);

        let mut mirror = display.clone();
        display.clear_dirty();

        // 同じ個数のまま更新しても場所を動かさず、文字の個別スタイルを汚さない
        display.update_with(1, [2, 1, 1, 1], [3, 4], |w| {
            w.push_segment([0.0, 0.0], [0.0, 6.0]);
            w.push_segment([10.0, 0.0], [10.0, 6.0]);
            w.push_arc_deg([5.0, 6.0], 5.0, 0.0, 180.0);
            w.push_text_styled([5.0, 7.0], 2.5, 0.0, 1.0, 0.0, [7, 8]);
            w.push_instance([0.0, 6.0], [1.0, 1.0], 0.0, [0.0, 0.0], 2, [1, 1]);
        });
        assert_eq!(display.dead, 0);
        assert!(!display.dirty.text_style.is_dirty(display.text_style.len()));
        display.sync_gpu_mirror(&mut mirror);
        assert_eq!(display, mirror);

        // 隠して戻す
        display.hide(1);
        assert_eq!(
            display.segment_owner.slice_to_vec(0, 3),
            [0, NO_OWNER, NO_OWNER]
        );
        assert_eq!(display.arc_owner.slice_to_vec(0, 1), [NO_OWNER]);
        assert_eq!(display.text_owner.slice_to_vec(0, 1), [NO_OWNER]);
        assert_eq!(display.instance_owner.slice_to_vec(0, 1), [NO_OWNER]);
        display.show(1);
        display.sync_gpu_mirror(&mut mirror);
        assert_eq!(display, mirror);

        // 末尾の複合行を pop すると mixed_slots も含めて完全に巻き戻る
        let before_extra = display.clone();
        display.push_with([1, 0, 1, 0], [5, 6], |w| {
            w.push_segment([1.0, 1.0], [2.0, 2.0]);
            w.push_text([1.5, 1.5], 2.0, 0.0, 1.0, 0.0);
        });
        display.pop();
        display.clear_dirty();
        assert_eq!(display, before_extra);
    }

    #[test]
    fn dirty_blocks_sync_gpu_mirror_with_minimal_bytes() {
        let mut display = Display::with_rows(200);
        for i in 0..200 {
            let x = f64::from(i);
            display.push_with([1, 0, 0, 0], [0, 1], |w| {
                w.push_segment([x, 0.0], [x + 1.0, 1.0]);
            });
        }
        let mut mirror = display.clone();
        display.clear_dirty();
        assert!(!display.has_dirty());
        assert_eq!(display.dirty_bytes(), 0);

        display.update_with(100, [1, 0, 0, 0], [0, 1], |w| {
            w.push_segment([99.0, 5.0], [100.0, 6.0]);
        });
        assert_eq!(display.dirty_bytes(), DIRTY_BLOCK * 32);
        display.sync_gpu_mirror(&mut mirror);
        assert_eq!(display, mirror);

        display.update_with(100, [1, 0, 0, 0], [0, 5], |w| {
            w.push_segment([99.0, 5.0], [100.0, 6.0]);
        });
        assert_eq!(display.dirty_bytes(), DIRTY_BLOCK * 8);
        display.sync_gpu_mirror(&mut mirror);
        assert_eq!(display, mirror);
    }

    #[test]
    fn push_and_update_respect_decoded_bytes_budget_atomically() {
        let mut display = Display::default();
        display.push_with([1, 0, 0, 0], [0, 0], |w| {
            w.push_segment([0.0, 0.0], [1.0, 1.0]);
        });
        let snapshot = display.clone();

        let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(32));
        let mut budget = Budget::new(limits);
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let err1 = display
            .push_with_cx(&mut cx, [1, 0, 0, 0], [0, 0], |w| {
                w.push_segment([2.0, 2.0], [3.0, 3.0]);
            })
            .unwrap_err();
        assert!(matches!(err1, Stop::Exceeded(e) if e.kind == Kind::DecodedBytes));
        assert_eq!(display, snapshot);

        let mut budget2 = Budget::new(limits);
        let mut diag2 = Diagnostics::new();
        let mut cx2 = Cx::new(&mut budget2, &mut diag2, &cancel);
        let err2 = display
            .update_with_cx(&mut cx2, 0, [2, 0, 0, 0], [0, 0], |w| {
                w.push_segment([0.0, 0.0], [1.0, 1.0]);
                w.push_segment([1.0, 1.0], [2.0, 2.0]);
            })
            .unwrap_err();
        assert!(matches!(err2, Stop::Exceeded(e) if e.kind == Kind::DecodedBytes));
        assert_eq!(display, snapshot);
    }

    /// 色は種類・体系・番号・RGB を往復し、16 bit に収まらない番号は画層の色になる
    #[test]
    fn colors_share_one_encoding() {
        assert_eq!(color::code(color::ACI, 3), 0x0201_0003);
        assert_eq!(color::code(color::JWW, 356), 0x0203_0164);
        assert_eq!(color::code(color::SXF, 0x1_0000), color::LAYER);
        assert_eq!(color::rgb([0x12, 0x34, 0x56]), 0x0312_3456);
        assert_eq!(color::code_of(0x0202_0011), Some((color::SXF, 17)));
        assert_eq!(color::rgb_of(0x0312_3456), Some([0x12, 0x34, 0x56]));
        for other in [color::LAYER, color::BLOCK, color::rgb([1, 2, 3])] {
            assert_eq!(color::code_of(other), None);
        }
        for other in [color::LAYER, color::BLOCK, color::code(color::ACI, 7)] {
            assert_eq!(color::rgb_of(other), None);
        }
    }
}
