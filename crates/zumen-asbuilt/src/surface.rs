//! 設計面の三角形と、点の真下の三角形を探す格子の索引。

use zumen_base::{Cx, Kind, Stop};
use zumen_civil::Tin;

use crate::{Range, Rule};

/// 格子の 1 辺の升の数の上限
const MAX_CELLS: usize = 2048;

/// 三角形の中とみなす重心座標の誤差
const INSIDE: f64 = 1e-9;

/// 平面の点（東・北）
type Plane = [f64; 2];

/// 設計面の三角形
#[derive(Clone, Copy, Debug)]
pub(crate) struct Face {
    /// 頂点の東・北
    corners: [Plane; 3],
    /// 頂点の標高
    heights: [f64; 3],
    /// 重心座標の分母（符号つきの面積の 2 倍）
    denominator: f64,
    /// 評価する規則の番号。どの規則の勾配にも入らなければ `None`
    pub rule: Option<usize>,
}

impl Face {
    /// 点の重心座標
    fn barycentric(&self, [e, n]: Plane) -> [f64; 3] {
        let [[ae, an], [be, bn], [ce, cn]] = self.corners;
        let l1 = ((bn - cn) * (e - ce) + (ce - be) * (n - cn)) / self.denominator;
        let l2 = ((cn - an) * (e - ce) + (ae - ce) * (n - cn)) / self.denominator;
        [l1, l2, 1.0 - l1 - l2]
    }

    /// 点が中にあれば、その位置の標高
    fn height(&self, point: Plane) -> Option<f64> {
        let weights = self.barycentric(point);
        if weights.iter().any(|w| *w < -INSIDE) {
            return None;
        }
        let [h1, h2, h3] = self.heights;
        let [l1, l2, l3] = weights;
        Some(l1 * h1 + l2 * h2 + l3 * h3)
    }

    /// 勾配（割、鉛直 1 に対する水平の長さ）。水平なら無限
    fn slope(&self) -> f64 {
        let [[ae, an], [be, bn], [ce, cn]] = self.corners;
        let [ha, hb, hc] = self.heights;
        let east = ((bn - cn) * ha + (cn - an) * hb + (an - bn) * hc) / self.denominator;
        let north = ((ce - be) * ha + (ae - ce) * hb + (be - ae) * hc) / self.denominator;
        let gradient = libm::sqrt(east * east + north * north);
        if gradient > 0.0 {
            1.0 / gradient
        } else {
            f64::INFINITY
        }
    }

    /// 平面に投影した面積
    fn area(&self) -> f64 {
        self.denominator.abs() / 2.0
    }

    /// 点から三角形までの平面の距離。中なら 0
    fn distance(&self, point: Plane) -> f64 {
        if self.height(point).is_some() {
            return 0.0;
        }
        let [a, b, c] = self.corners;
        [(a, b), (b, c), (c, a)]
            .into_iter()
            .map(|(from, to)| segment(point, from, to))
            .fold(f64::INFINITY, f64::min)
    }
}

/// 点から線分までの距離
fn segment([e, n]: Plane, [ae, an]: Plane, [be, bn]: Plane) -> f64 {
    let (de, dn) = (be - ae, bn - an);
    let length = de * de + dn * dn;
    let t = if length > 0.0 {
        (((e - ae) * de + (n - an) * dn) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (pe, pn) = (ae + t * de - e, an + t * dn - n);
    libm::sqrt(pe * pe + pn * pn)
}

/// 勾配の幅に入るか。`min` より大きく `max` 以下
fn within(slope: f64, range: Range) -> bool {
    range.min.is_none_or(|min| slope > min) && range.max.is_none_or(|max| slope <= max)
}

/// 三角形と格子の索引
pub(crate) struct Surface<'a> {
    faces: Vec<Face>,
    rules: &'a [Rule],
    /// 格子の始まり（東・北）
    origin: Plane,
    /// 升の大きさ（東・北）
    cell: Plane,
    /// 升の数（東・北）
    size: [usize; 2],
    /// 升ごとの三角形の番号
    cells: Vec<Vec<u32>>,
}

impl<'a> Surface<'a> {
    /// TIN の三角形に規則を当て、格子の索引を作る。面積のない三角形は使わない
    pub(crate) fn new(tin: &Tin, rules: &'a [Rule], cx: &mut Cx<'_>) -> Result<Surface<'a>, Stop> {
        cx.check(Kind::Elements, tin.triangles.len() as u64)?;
        let corner = |index: u32| {
            let (point, height) = tin.points.get(usize::try_from(index).ok()?)?;
            Some(([point.easting, point.northing], *height))
        };
        let mut faces = Vec::with_capacity(tin.triangles.len());
        for [a, b, c] in &tin.triangles {
            let (Some((pa, ha)), Some((pb, hb)), Some((pc, hc))) =
                (corner(*a), corner(*b), corner(*c))
            else {
                continue;
            };
            let [[ae, an], [be, bn], [ce, cn]] = [pa, pb, pc];
            let denominator = (bn - cn) * (ae - ce) + (ce - be) * (an - cn);
            if !(denominator.is_finite() && denominator != 0.0) {
                continue;
            }
            let mut face = Face {
                corners: [pa, pb, pc],
                heights: [ha, hb, hc],
                denominator,
                rule: None,
            };
            let slope = face.slope();
            face.rule = rules.iter().position(|rule| within(slope, rule.slope));
            faces.push(face);
        }
        let corners = faces.iter().flat_map(|face| face.corners);
        let (mut low, mut high) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for [e, n] in corners {
            low = [low[0].min(e), low[1].min(n)];
            high = [high[0].max(e), high[1].max(n)];
        }
        if faces.is_empty() {
            (low, high) = ([0.0; 2], [0.0; 2]);
        }
        // 升の数を三角形の数ほどにする
        let (width, depth) = ((high[0] - low[0]).max(0.0), (high[1] - low[1]).max(0.0));
        let target = libm::sqrt(faces.len().max(1) as f64);
        let side = libm::sqrt((width * depth).max(f64::MIN_POSITIVE)) / target;
        let count = |extent: f64| {
            if side > 0.0 && side.is_finite() {
                ((extent / side) as usize).clamp(1, MAX_CELLS)
            } else {
                1
            }
        };
        let size = [count(width), count(depth)];
        let cell = [
            (width / size[0] as f64).max(f64::MIN_POSITIVE),
            (depth / size[1] as f64).max(f64::MIN_POSITIVE),
        ];
        let mut surface = Surface {
            faces: Vec::new(),
            rules,
            origin: low,
            cell,
            size,
            cells: vec![Vec::new(); size[0] * size[1]],
        };
        for (number, face) in faces.iter().enumerate() {
            let [[ae, an], [be, bn], [ce, cn]] = face.corners;
            let from = surface.cell_of([ae.min(be).min(ce), an.min(bn).min(cn)]);
            let to = surface.cell_of([ae.max(be).max(ce), an.max(bn).max(cn)]);
            for y in from[1]..=to[1] {
                for x in from[0]..=to[0] {
                    cx.check(Kind::Work, 1)?;
                    if let Some(slot) = surface.cells.get_mut(y * size[0] + x) {
                        slot.push(number as u32);
                    }
                }
            }
        }
        surface.faces = faces;
        Ok(surface)
    }

    /// 点の升。格子の外なら端の升
    fn cell_of(&self, [e, n]: Plane) -> [usize; 2] {
        let at = |value: f64, origin: f64, cell: f64, size: usize| {
            let index = ((value - origin) / cell).max(0.0) as usize;
            index.min(size.saturating_sub(1))
        };
        [
            at(e, self.origin[0], self.cell[0], self.size[0]),
            at(n, self.origin[1], self.cell[1], self.size[1]),
        ]
    }

    /// 升の三角形
    fn faces_in(&self, [x, y]: [usize; 2]) -> impl Iterator<Item = &Face> {
        self.cells
            .get(y * self.size[0] + x)
            .into_iter()
            .flatten()
            .filter_map(|number| self.faces.get(*number as usize))
    }

    /// 点の真下の三角形と、その位置の標高
    pub(crate) fn locate(&self, e: f64, n: f64) -> Option<(&Face, f64)> {
        let point = [e, n];
        if !(e.is_finite() && n.is_finite()) {
            return None;
        }
        self.faces_in(self.cell_of(point))
            .find_map(|face| face.height(point).map(|height| (face, height)))
    }

    /// 点から `distance` の内に、`rule` と区分の違う三角形があるか
    pub(crate) fn near_other_part(&self, e: f64, n: f64, rule: usize, distance: f64) -> bool {
        let part = self.rules.get(rule).map(|rule| rule.part.as_str());
        let from = self.cell_of([e - distance, n - distance]);
        let to = self.cell_of([e + distance, n + distance]);
        (from[1]..=to[1]).any(|y| {
            (from[0]..=to[0]).any(|x| {
                self.faces_in([x, y]).any(|face| {
                    let other = face
                        .rule
                        .and_then(|at| self.rules.get(at))
                        .map(|rule| rule.part.as_str());
                    other != part && face.distance([e, n]) <= distance
                })
            })
        })
    }

    /// 規則ごとの、三角形の面積の和。規則の順
    pub(crate) fn areas(&self) -> Vec<f64> {
        let mut areas = vec![0.0; self.rules.len()];
        for face in &self.faces {
            if let Some(area) = face.rule.and_then(|at| areas.get_mut(at)) {
                *area += face.area();
            }
        }
        areas
    }
}
