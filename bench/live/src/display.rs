//! 表示のバッファー。線分・円弧・文字を、要素ごとの値のまま配列に持つ（頂点への分割と字形は、描く側で行う）。
//! 点は f64 を上位と下位の f32 に分けて置き、視点からの相対の位置を描く側で作れるようにする。
//! 変更のときは、動いた要素の分だけを同じ場所に書き直す。開き直して作ったバッファーと同じ値になる。

use crate::lift::{Geom, POLY_CLOSED, Place, Pools};

/// 使っていない場所の持ち主
pub const NO_OWNER: u32 = u32::MAX;

/// 要素が使う場所
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Slot {
    /// 線分の最初の番号
    pub segment: u32,
    /// 線分の数
    pub segments: u32,
    /// 円弧の最初の番号
    pub arc: u32,
    /// 円弧の数
    pub arcs: u32,
    /// 文字の番号
    pub text: u32,
    /// 文字の数（0 か 1）
    pub texts: u32,
}

/// 1 つの行の、線分・円弧・文字の値（`Display::canonical`）
pub type RowBuffers = (u32, Vec<[f32; 8]>, Vec<[f32; 7]>, Vec<[f32; 8]>);

/// 表示のバッファー
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Display {
    /// 線分。始点の x（上位・下位）・y（上位・下位）、終点の x・y
    pub segments: Vec<[f32; 8]>,
    /// 線分の持ち主（行）
    pub segment_owner: Vec<u32>,
    /// 線分のスタイル（画層・色の指定）
    pub segment_style: Vec<[u32; 2]>,
    /// 円弧。中心の x（上位・下位）・y（上位・下位）、半径、始まりの角、回る角（ラジアン。正が反時計回り）
    pub arcs: Vec<[f32; 7]>,
    /// 円弧の持ち主
    pub arc_owner: Vec<u32>,
    /// 円弧のスタイル
    pub arc_style: Vec<[u32; 2]>,
    /// 文字。置く点の x（上位・下位）・y（上位・下位）、高さ、回転（ラジアン）、幅の係数、傾き（ラジアン）
    pub texts: Vec<[f32; 8]>,
    /// 文字の持ち主
    pub text_owner: Vec<u32>,
    /// 文字のスタイル
    pub text_style: Vec<[u32; 2]>,
    /// 行ごとの場所
    pub slots: Vec<Slot>,
    /// 置き直して使わなくなった場所の数
    pub dead: u32,
}

/// f64 を、上位と下位の f32 に分ける
fn split(value: f64) -> [f32; 2] {
    let high = value as f32;
    [high, (value - f64::from(high)) as f32]
}

fn segment([x0, y0]: [f64; 2], [x1, y1]: [f64; 2]) -> [f32; 8] {
    let ([a, b], [c, d], [e, f], [g, h]) = (split(x0), split(y0), split(x1), split(y1));
    [a, b, c, d, e, f, g, h]
}

fn arc([x, y]: [f64; 2], radius: f64, start: f64, sweep: f64) -> [f32; 7] {
    let ([a, b], [c, d]) = (split(x), split(y));
    [a, b, c, d, radius as f32, start as f32, sweep as f32]
}

const TURN: f64 = core::f64::consts::TAU;

fn radians(degrees: f64) -> f64 {
    degrees * (core::f64::consts::PI / 180.0)
}

/// 膨らみ `bulge` で `from` から `to` へ進む円弧
fn bulged(from: [f64; 2], to: [f64; 2], bulge: f64) -> [f32; 7] {
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
            slots: Vec::with_capacity(rows),
            ..Display::default()
        }
    }

    /// 要素が要る線分・円弧・文字の数
    fn counts(geom: &Geom, pools: &Pools) -> [u32; 3] {
        match geom {
            Geom::Line(_) => [1, 0, 0],
            Geom::Circle(_) | Geom::Arc(_) => [0, 1, 0],
            Geom::Polyline {
                first,
                count,
                flags,
                ..
            } => {
                let vertices = pools
                    .vertices
                    .get(*first as usize..(*first + *count) as usize)
                    .unwrap_or_default();
                let mut counts = [0; 3];
                for (_, _, bulge) in edges(vertices, flags & POLY_CLOSED != 0) {
                    counts[usize::from(bulge != 0.0)] += 1;
                }
                counts
            }
            Geom::Text(_) => [0, 0, 1],
        }
    }

    /// 要素の値を、場所 `slot` に書く。場所は、要る数だけあること
    fn write(&mut self, slot: Slot, geom: &Geom, pools: &Pools) {
        match geom {
            Geom::Line([x0, y0, x1, y1]) => {
                if let Some(place) = self.segments.get_mut(slot.segment as usize) {
                    *place = segment([*x0, *y0], [*x1, *y1]);
                }
            }
            Geom::Circle([x, y, radius]) => {
                if let Some(place) = self.arcs.get_mut(slot.arc as usize) {
                    *place = arc([*x, *y], *radius, 0.0, TURN);
                }
            }
            Geom::Arc([x, y, radius, start, end]) => {
                // 始まりと終わりが同じ角なら、ひと回りとする
                let sweep = match (end - start).rem_euclid(360.0) {
                    0.0 => 360.0,
                    sweep => sweep,
                };
                if let Some(place) = self.arcs.get_mut(slot.arc as usize) {
                    *place = arc([*x, *y], *radius, radians(*start), radians(sweep));
                }
            }
            Geom::Polyline {
                first,
                count,
                flags,
                ..
            } => {
                let vertices = pools
                    .vertices
                    .get(*first as usize..(*first + *count) as usize)
                    .unwrap_or_default();
                let (mut straight, mut round) = (slot.segment as usize, slot.arc as usize);
                for (from, to, bulge) in edges(vertices, flags & POLY_CLOSED != 0) {
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
                let at = match row.place {
                    Place::Anchor { point, .. } => point,
                    Place::Between { first, .. } => first,
                };
                let ([a, b], [c, d]) = (split(at[0]), split(at[1]));
                if let Some(place) = self.texts.get_mut(slot.text as usize) {
                    *place = [
                        a,
                        b,
                        c,
                        d,
                        row.height as f32,
                        radians(row.rotation) as f32,
                        row.width_factor as f32,
                        radians(row.oblique) as f32,
                    ];
                }
            }
        }
    }

    /// 後ろに場所を取る
    fn reserve(&mut self, row: u32, counts: [u32; 3], style: [u32; 2]) -> Slot {
        let slot = Slot {
            segment: self.segments.len() as u32,
            segments: counts[0],
            arc: self.arcs.len() as u32,
            arcs: counts[1],
            text: self.texts.len() as u32,
            texts: counts[2],
        };
        let grow = |len: usize, count: u32| len + count as usize;
        self.segments
            .resize(grow(self.segments.len(), counts[0]), [0.0; 8]);
        self.segment_owner.resize(self.segments.len(), row);
        self.segment_style.resize(self.segments.len(), style);
        self.arcs.resize(grow(self.arcs.len(), counts[1]), [0.0; 7]);
        self.arc_owner.resize(self.arcs.len(), row);
        self.arc_style.resize(self.arcs.len(), style);
        self.texts
            .resize(grow(self.texts.len(), counts[2]), [0.0; 8]);
        self.text_owner.resize(self.texts.len(), row);
        self.text_style.resize(self.texts.len(), style);
        slot
    }

    /// 次の行の要素を足す。行は、0 から順に足すこと
    pub fn push(&mut self, geom: &Geom, pools: &Pools, style: [u32; 2]) {
        let row = self.slots.len() as u32;
        let slot = self.reserve(row, Self::counts(geom, pools), style);
        self.write(slot, geom, pools);
        self.slots.push(slot);
    }

    /// 行 `row` の要素を書き直す。線分・円弧の数が変わらなければ同じ場所に書き、変われば後ろに置き直す
    pub fn update(&mut self, row: u32, geom: &Geom, pools: &Pools, style: [u32; 2]) {
        let Some(&slot) = self.slots.get(row as usize) else {
            return;
        };
        let counts = Self::counts(geom, pools);
        let mut slot = slot;
        if [slot.segments, slot.arcs, slot.texts] != counts {
            let clear = |owners: &mut [u32], first: u32, count: u32| {
                let range = first as usize..(first + count) as usize;
                owners.get_mut(range).unwrap_or_default().fill(NO_OWNER);
            };
            clear(&mut self.segment_owner, slot.segment, slot.segments);
            clear(&mut self.arc_owner, slot.arc, slot.arcs);
            clear(&mut self.text_owner, slot.text, slot.texts);
            self.dead += slot.segments + slot.arcs + slot.texts;
            slot = self.reserve(row, counts, style);
            if let Some(place) = self.slots.get_mut(row as usize) {
                *place = slot;
            }
        }
        self.write(slot, geom, pools);
    }

    /// 生きている値だけを、行の順に並べたもの。開き直して作ったバッファーと比べるのに使う
    pub fn canonical(&self) -> Vec<RowBuffers> {
        self.slots
            .iter()
            .enumerate()
            .map(|(row, slot)| {
                let part = |first: u32, count: u32| first as usize..(first + count) as usize;
                (
                    row as u32,
                    self.segments
                        .get(part(slot.segment, slot.segments))
                        .unwrap_or_default()
                        .to_vec(),
                    self.arcs
                        .get(part(slot.arc, slot.arcs))
                        .unwrap_or_default()
                        .to_vec(),
                    self.texts
                        .get(part(slot.text, slot.texts))
                        .unwrap_or_default()
                        .to_vec(),
                )
            })
            .collect()
    }

    /// バッファーのバイト数
    pub fn bytes(&self) -> usize {
        self.segments.len() * (32 + 4 + 8)
            + self.arcs.len() * (28 + 4 + 8)
            + self.texts.len() * (32 + 4 + 8)
            + self.slots.len() * core::mem::size_of::<Slot>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_splits_into_high_and_low() {
        // 上位と下位を合わせると、48 ビットほどの精度になる。f32 だけでは、この値は 1 より大きくずれる
        let value = 123_456_789.123_456_79_f64;
        let [high, low] = split(value);
        let error = (f64::from(high) + f64::from(low) - value).abs();
        assert!(error <= value / (1u64 << 47) as f64, "{error}");
        assert!((f64::from(value as f32) - value).abs() > 1.0);
        // f32 2 つで表せる値は、そのまま戻る
        let [high, low] = split(1_234_567.25);
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
        let mut pools = Pools {
            vertices: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.5], [1.0, 1.0, 0.0]],
            ..Pools::default()
        };
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
        pools.vertices[2] = [2.0, 2.0, 0.0];
        display.update(1, &line, &pools, [0, 0]);
        assert_eq!(display.slots, before);
        assert_eq!(display.dead, 0);
        // 辺の種類の数が変われば、後ろに置き直す
        pools.vertices[1][2] = 0.0;
        display.update(1, &line, &pools, [0, 0]);
        assert_eq!(display.dead, 3);
        assert_eq!(display.slots[1].segments, 3);
        assert_eq!(display.segment_owner, [0, NO_OWNER, NO_OWNER, 1, 1, 1]);
    }
}
