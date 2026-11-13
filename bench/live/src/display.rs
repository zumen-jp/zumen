//! 表示のバッファー。線分・円弧・文字・ブロックの挿入を、要素ごとの値のまま配列に持つ
//! （頂点への分割と字形、定義の中身を挿入ごとに置くことは、描く側で行う）。
//! 点は f64 を上位と下位の f32 に分けて置き、視点からの相対の位置を描く側で作れるようにする。
//! 変更のときは、変わった要素の分だけを書き直す。動かす変更と属性を変える変更では、開き直して作った
//! バッファーと同じ値になる。除いた行は場所を残して持ち主を外すので、配列の並びは開き直したものと変わる。

use crate::lift::{Geom, POLY_CLOSED, Place, Pools, TextRow};

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
    /// 文字の最初の番号
    pub text: u32,
    /// 文字の数（文字は 1、挿入は表示する属性の数）
    pub texts: u32,
    /// 挿入の番号
    pub instance: u32,
    /// 挿入の数（0 か 1）
    pub instances: u32,
}

impl Slot {
    fn total(self) -> u32 {
        self.segments + self.arcs + self.texts + self.instances
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
    /// 挿入。置く点の x（上位・下位）・y（上位・下位）、尺度の x・y、回転（ラジアン）、列・行の間隔
    pub instances: Vec<[f32; 9]>,
    /// 挿入が指す定義の番号（なければ `NO_DEFINITION`）と、列・行の数
    pub instance_ref: Vec<[u32; 3]>,
    /// 挿入の持ち主
    pub instance_owner: Vec<u32>,
    /// 挿入のスタイル
    pub instance_style: Vec<[u32; 2]>,
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

/// 文字の値
fn lettering(row: &TextRow) -> [f32; 8] {
    let at = match row.place {
        Place::Anchor { point, .. } => point,
        Place::Between { first, .. } => first,
    };
    let ([a, b], [c, d]) = (split(at[0]), split(at[1]));
    [
        a,
        b,
        c,
        d,
        row.height as f32,
        radians(row.rotation) as f32,
        row.width_factor as f32,
        radians(row.oblique) as f32,
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
            slots: Vec::with_capacity(rows),
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
                let vertices = pools
                    .vertices
                    .get(*first as usize..(*first + *count) as usize)
                    .unwrap_or_default();
                let mut counts = [0; 4];
                for (_, _, bulge) in edges(vertices, flags & POLY_CLOSED != 0) {
                    counts[usize::from(bulge != 0.0)] += 1;
                }
                counts
            }
            Geom::Text(_) => [0, 0, 1, 0],
            // 属性は、表示するものだけを文字として置く
            Geom::Insert(insert) => {
                let shown = pools
                    .attribs_of(insert)
                    .iter()
                    .filter(|attribute| !attribute.hidden)
                    .count();
                [0, 0, shown as u32, 1]
            }
        }
    }

    /// 要素の値とスタイルを、場所 `slot` に書く。場所は、要る数だけあること
    fn write(&mut self, slot: Slot, geom: &Geom, pools: &Pools, style: [u32; 2]) {
        let fill = |styles: &mut [[u32; 2]], first: u32, count: u32| {
            let range = first as usize..(first + count) as usize;
            styles.get_mut(range).unwrap_or_default().fill(style);
        };
        fill(&mut self.segment_style, slot.segment, slot.segments);
        fill(&mut self.arc_style, slot.arc, slot.arcs);
        fill(&mut self.text_style, slot.text, slot.texts);
        fill(&mut self.instance_style, slot.instance, slot.instances);
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
                if let Some(place) = self.texts.get_mut(slot.text as usize) {
                    *place = lettering(row);
                }
            }
            Geom::Insert(insert) => {
                let at = slot.instance as usize;
                let ([a, b], [c, d]) = (split(insert.position[0]), split(insert.position[1]));
                if let Some(place) = self.instances.get_mut(at) {
                    *place = [
                        a,
                        b,
                        c,
                        d,
                        insert.scale[0] as f32,
                        insert.scale[1] as f32,
                        radians(insert.rotation) as f32,
                        insert.spacing[0] as f32,
                        insert.spacing[1] as f32,
                    ];
                }
                if let Some(place) = self.instance_ref.get_mut(at) {
                    *place = [insert.definition, insert.grid[0], insert.grid[1]];
                }
                let shown = pools
                    .attribs_of(insert)
                    .iter()
                    .filter(|attribute| !attribute.hidden);
                for (at, attribute) in (slot.text as usize..).zip(shown) {
                    if let Some(place) = self.texts.get_mut(at) {
                        *place = lettering(&attribute.text);
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
        let slot = Slot {
            segment: self.segments.len() as u32,
            segments: counts[0],
            arc: self.arcs.len() as u32,
            arcs: counts[1],
            text: self.texts.len() as u32,
            texts: counts[2],
            instance: self.instances.len() as u32,
            instances: counts[3],
        };
        let grow = |len: usize, count: u32| len + count as usize;
        self.segments
            .resize(grow(self.segments.len(), counts[0]), [0.0; 8]);
        self.segment_owner.resize(self.segments.len(), row);
        self.segment_style.resize(self.segments.len(), [0; 2]);
        self.arcs.resize(grow(self.arcs.len(), counts[1]), [0.0; 7]);
        self.arc_owner.resize(self.arcs.len(), row);
        self.arc_style.resize(self.arcs.len(), [0; 2]);
        self.texts
            .resize(grow(self.texts.len(), counts[2]), [0.0; 8]);
        self.text_owner.resize(self.texts.len(), row);
        self.text_style.resize(self.texts.len(), [0; 2]);
        self.instances
            .resize(grow(self.instances.len(), counts[3]), [0.0; 9]);
        self.instance_ref.resize(self.instances.len(), [0; 3]);
        self.instance_owner.resize(self.instances.len(), row);
        self.instance_style.resize(self.instances.len(), [0; 2]);
        slot
    }

    /// 場所の持ち主を書く
    fn own(&mut self, slot: Slot, owner: u32) {
        let fill = |owners: &mut [u32], first: u32, count: u32| {
            let range = first as usize..(first + count) as usize;
            owners.get_mut(range).unwrap_or_default().fill(owner);
        };
        fill(&mut self.segment_owner, slot.segment, slot.segments);
        fill(&mut self.arc_owner, slot.arc, slot.arcs);
        fill(&mut self.text_owner, slot.text, slot.texts);
        fill(&mut self.instance_owner, slot.instance, slot.instances);
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
        let at_end = |first: u32, count: u32, len: usize| (first + count) as usize == len;
        if at_end(slot.segment, slot.segments, self.segments.len())
            && at_end(slot.arc, slot.arcs, self.arcs.len())
            && at_end(slot.text, slot.texts, self.texts.len())
            && at_end(slot.instance, slot.instances, self.instances.len())
        {
            self.segments.truncate(slot.segment as usize);
            self.segment_owner.truncate(slot.segment as usize);
            self.segment_style.truncate(slot.segment as usize);
            self.arcs.truncate(slot.arc as usize);
            self.arc_owner.truncate(slot.arc as usize);
            self.arc_style.truncate(slot.arc as usize);
            self.texts.truncate(slot.text as usize);
            self.text_owner.truncate(slot.text as usize);
            self.text_style.truncate(slot.text as usize);
            self.instances.truncate(slot.instance as usize);
            self.instance_ref.truncate(slot.instance as usize);
            self.instance_owner.truncate(slot.instance as usize);
            self.instance_style.truncate(slot.instance as usize);
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
        if [slot.segments, slot.arcs, slot.texts, slot.instances] != counts {
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
        let Some(slot) = self.slots.get(row as usize) else {
            return RowBuffers::default();
        };
        let part = |first: u32, count: u32| first as usize..(first + count) as usize;
        let instances = part(slot.instance, slot.instances);
        RowBuffers {
            segments: self
                .segments
                .get(part(slot.segment, slot.segments))
                .unwrap_or_default()
                .to_vec(),
            arcs: self
                .arcs
                .get(part(slot.arc, slot.arcs))
                .unwrap_or_default()
                .to_vec(),
            texts: self
                .texts
                .get(part(slot.text, slot.texts))
                .unwrap_or_default()
                .to_vec(),
            instances: self
                .instances
                .get(instances.clone())
                .unwrap_or_default()
                .iter()
                .copied()
                .zip(
                    self.instance_ref
                        .get(instances)
                        .unwrap_or_default()
                        .iter()
                        .copied(),
                )
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
    use zumen_drawing::{Horizontal, Vertical};

    use super::*;
    use crate::lift::{AttribRow, InsertRow};

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
        display.update(1, &line, &pools, [3, 4]);
        assert_eq!(display.slots, before);
        assert_eq!(display.dead, 0);
        // 同じ場所に書くときも、スタイルを直す
        assert_eq!(display.segment_style, [[0, 0], [3, 4], [3, 4]]);
        // 辺の種類の数が変われば、後ろに置き直す
        pools.vertices[1][2] = 0.0;
        display.update(1, &line, &pools, [0, 0]);
        assert_eq!(display.dead, 3);
        assert_eq!(display.slots[1].segments, 3);
        assert_eq!(display.segment_owner, [0, NO_OWNER, NO_OWNER, 1, 1, 1]);
    }

    fn text(x: f64) -> TextRow {
        TextRow {
            place: Place::Anchor {
                point: [x, 0.0],
                horizontal: Horizontal::Left,
                vertical: Vertical::Baseline,
            },
            height: 2.5,
            rotation: 0.0,
            width_factor: 1.0,
            oblique: 0.0,
            flags: 0,
            content: (0, 0),
            style: None,
        }
    }

    #[test]
    fn an_insert_is_one_instance_and_its_shown_attributes() {
        let attribute = |x: f64, hidden: bool| AttribRow {
            record: 0,
            text: text(x),
            tag: (0, 0),
            hidden,
            layer_name: (0, 0),
            layer: 7,
            color: 9,
            class: 0,
        };
        let pools = Pools {
            attribs: vec![
                attribute(1.0, false),
                attribute(2.0, true),
                attribute(3.0, false),
            ],
            ..Pools::default()
        };
        let insert = Geom::Insert(InsertRow {
            name: (0, 0),
            definition: 4,
            position: [10.0, 20.0],
            scale: [2.0, 2.0],
            rotation: 90.0,
            grid: [3, 2],
            spacing: [5.0, 6.0],
            attribs: (0, 3),
            attached: 3,
            followed: true,
        });
        let mut display = Display::default();
        display.push(&insert, &pools, [1, 2]);
        assert_eq!(display.instance_ref, [[4, 3, 2]]);
        assert_eq!(display.instance_style, [[1, 2]]);
        // 表示しない属性は置かない。属性のスタイルは、属性のもの
        assert_eq!(display.texts.len(), 2);
        assert_eq!(display.text_style, [[7, 9], [7, 9]]);
        assert_eq!(display.texts[1][0], 3.0);
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
        // 隠した行は、場所を残して持ち主だけを外す。戻せば、元と同じになる
        display.hide(0);
        assert_eq!(display.segment_owner, [NO_OWNER]);
        assert_eq!(display.segments, opened.segments);
        display.show(0);
        assert_eq!(display, opened);
        // 終わりに足した行は、除けば元と同じになる
        display.push(&Geom::Line([2.0, 2.0, 3.0, 3.0]), &pools, [0, 0]);
        display.pop();
        assert_eq!(display, opened);
    }
}
