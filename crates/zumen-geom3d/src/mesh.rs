//! 要素の ID と辺を持つメッシュ

use std::collections::BTreeMap;

use crate::{Failure, Transform, Xyz, cross, dot, is_finite, sub};

/// 三角形のメッシュ
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    /// 点
    pub positions: Vec<Xyz>,
    /// 三角形（点の番号、外から見て反時計回り）
    pub triangles: Vec<[u32; 3]>,
    /// 描く辺（形の稜線。三角形に分けた線は入れない）
    pub edges: Vec<[u32; 2]>,
}

/// 要素の形。`E` は使う側の要素の ID（建物モデルなら要素の ID、決定 0207）
#[derive(Clone, Debug, PartialEq)]
pub struct Part<E> {
    /// 要素
    pub element: E,
    /// 形
    pub mesh: Mesh,
}

impl Mesh {
    /// 点の番号が範囲の内にあり、点が有限か
    pub fn validate(&self) -> Result<(), Failure> {
        if !self.positions.iter().all(|p| is_finite(*p)) {
            return Err(Failure::Invalid("position"));
        }
        let n = self.positions.len();
        let inside = |i: &u32| (*i as usize) < n;
        if !self.triangles.iter().flatten().all(inside) || !self.edges.iter().flatten().all(inside)
        {
            return Err(Failure::Invalid("index"));
        }
        Ok(())
    }

    /// 三角形の 3 点。番号が範囲の外なら `None`
    pub fn triangle(&self, t: [u32; 3]) -> Option<[Xyz; 3]> {
        let p = |i: u32| self.positions.get(i as usize).copied();
        Some([p(t[0])?, p(t[1])?, p(t[2])?])
    }

    /// 符号つきの体積。閉じた外向きのメッシュなら正
    pub fn volume(&self) -> f64 {
        let sum: f64 = self
            .triangles
            .iter()
            .filter_map(|t| self.triangle(*t))
            .map(|[a, b, c]| dot(a, cross(b, c)))
            .sum();
        sum / 6.0
    }

    /// 表面積
    pub fn area(&self) -> f64 {
        let sum: f64 = self
            .triangles
            .iter()
            .filter_map(|t| self.triangle(*t))
            .map(|[a, b, c]| crate::length(cross(sub(b, a), sub(c, a))))
            .sum();
        sum / 2.0
    }

    /// 点を囲む箱（最小と最大）。点がなければ `None`
    pub fn bounds(&self) -> Option<[Xyz; 2]> {
        let mut points = self.positions.iter();
        let first = *points.next()?;
        Some(points.fold([first, first], |[lo, hi], p| {
            [
                [lo[0].min(p[0]), lo[1].min(p[1]), lo[2].min(p[2])],
                [hi[0].max(p[0]), hi[1].max(p[1]), hi[2].max(p[2])],
            ]
        }))
    }

    /// 同じ座標（ビットが同じ）の点を 1 つにしたメッシュ。点は最初に出た順に並べる
    pub fn welded(&self) -> Mesh {
        let mut index: BTreeMap<[u64; 3], u32> = BTreeMap::new();
        let mut positions = Vec::new();
        let remap: Vec<u32> = self
            .positions
            .iter()
            .map(|p| {
                *index.entry(p.map(f64::to_bits)).or_insert_with(|| {
                    positions.push(*p);
                    (positions.len() - 1) as u32
                })
            })
            .collect();
        let at = |i: u32| remap.get(i as usize).copied().unwrap_or(i);
        Mesh {
            positions,
            triangles: self.triangles.iter().map(|t| t.map(at)).collect(),
            edges: self.edges.iter().map(|e| e.map(at)).collect(),
        }
    }

    /// 閉じた 2 様体か。どの向きつきの辺も 1 度だけ使われ、その逆向きの辺も 1 度だけ使われる。
    /// 点の番号で比べるので、座標の同じ点が分かれていれば、先に [`Mesh::welded`] で 1 つにする
    pub fn is_closed(&self) -> bool {
        let mut count: BTreeMap<(u32, u32), u32> = BTreeMap::new();
        for t in &self.triangles {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *count.entry((a, b)).or_default() += 1;
            }
        }
        !count.is_empty()
            && count
                .iter()
                .all(|(&(a, b), &n)| n == 1 && count.get(&(b, a)) == Some(&1))
    }

    /// 変換したメッシュ。変換が裏返すなら、三角形の向きを戻す
    pub fn transformed(&self, transform: &Transform) -> Mesh {
        let flip = transform.determinant() < 0.0;
        Mesh {
            positions: self.positions.iter().map(|p| transform.point(*p)).collect(),
            triangles: self
                .triangles
                .iter()
                .map(|t| if flip { [t[0], t[2], t[1]] } else { *t })
                .collect(),
            edges: self.edges.clone(),
        }
    }

    /// ほかのメッシュを足す
    pub fn append(&mut self, other: &Mesh) {
        let base = self.positions.len() as u32;
        self.positions.extend_from_slice(&other.positions);
        self.triangles
            .extend(other.triangles.iter().map(|t| t.map(|i| i + base)));
        self.edges
            .extend(other.edges.iter().map(|e| e.map(|i| i + base)));
    }
}
