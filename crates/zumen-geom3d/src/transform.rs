//! 配置（アフィン変換）と平面

use crate::{Xy, Xyz, add, cross, dot, is_finite, normalize, scale, sub};

/// アフィン変換。3 行 4 列で、列 0〜2 が軸、列 3 が原点の移動。点 `p` は `M·[p, 1]` に写る
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform(pub [[f64; 4]; 3]);

impl Transform {
    /// 恒等の変換
    pub const IDENTITY: Self = Self([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ]);

    /// 移動
    pub fn translation(v: Xyz) -> Self {
        Self([
            [1.0, 0.0, 0.0, v[0]],
            [0.0, 1.0, 0.0, v[1]],
            [0.0, 0.0, 1.0, v[2]],
        ])
    }

    /// 軸ごとの拡大縮小
    pub fn scaling(s: Xyz) -> Self {
        Self([
            [s[0], 0.0, 0.0, 0.0],
            [0.0, s[1], 0.0, 0.0],
            [0.0, 0.0, s[2], 0.0],
        ])
    }

    /// 局所の軸と原点から作る。`axis` が局所の Z、`reference` を Z に直交させたものが局所の X。
    /// `axis` がなければ Z、`reference` がないか `axis` と平行なら、`axis` に最も直交する座標軸を使う。
    /// 軸の長さが 0 か、有限でない値なら `None`
    pub fn from_axes(origin: Xyz, axis: Option<Xyz>, reference: Option<Xyz>) -> Option<Self> {
        if !is_finite(origin) {
            return None;
        }
        let z = normalize(axis.unwrap_or([0.0, 0.0, 1.0]))?;
        let x = reference
            .and_then(|r| normalize(sub(r, scale(z, dot(r, z)))))
            .or_else(|| {
                let fallback = if z[0].abs() < 0.9 {
                    [1.0, 0.0, 0.0]
                } else {
                    [0.0, 1.0, 0.0]
                };
                normalize(sub(fallback, scale(z, dot(fallback, z))))
            })?;
        let y = cross(z, x);
        Some(Self([
            [x[0], y[0], z[0], origin[0]],
            [x[1], y[1], z[1], origin[1]],
            [x[2], y[2], z[2], origin[2]],
        ]))
    }

    /// 点を写す
    pub fn point(&self, p: Xyz) -> Xyz {
        add(self.vector(p), self.column(3))
    }

    /// ベクトルを写す（移動を除く）
    pub fn vector(&self, v: Xyz) -> Xyz {
        let m = &self.0;
        [
            m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
            m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
            m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
        ]
    }

    /// 列（0〜2 は軸、3 は原点）。範囲の外は 0
    pub fn column(&self, j: usize) -> Xyz {
        let m = &self.0;
        let at = |i: usize| m.get(i).and_then(|row| row.get(j)).copied().unwrap_or(0.0);
        [at(0), at(1), at(2)]
    }

    /// `self` のあとに `outer` を写す変換（`outer · self`）
    pub fn then(&self, outer: &Self) -> Self {
        let x = outer.vector(self.column(0));
        let y = outer.vector(self.column(1));
        let z = outer.vector(self.column(2));
        let o = outer.point(self.column(3));
        Self([
            [x[0], y[0], z[0], o[0]],
            [x[1], y[1], z[1], o[1]],
            [x[2], y[2], z[2], o[2]],
        ])
    }

    /// 軸の部分の行列式。負なら裏返す
    pub fn determinant(&self) -> f64 {
        dot(self.column(0), cross(self.column(1), self.column(2)))
    }

    /// すべての成分が有限か
    pub fn is_finite(&self) -> bool {
        self.0.iter().flatten().all(|v| v.is_finite())
    }
}

/// 平面。`normal` の向きが表
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    /// 平面の上の点
    pub origin: Xyz,
    /// 法線。長さは問わない
    pub normal: Xyz,
}

impl Plane {
    /// 点の、平面からの符号つきの距離（表が正）。法線の長さが 0 なら `None`
    pub fn distance(&self, p: Xyz) -> Option<f64> {
        normalize(self.normal).map(|n| dot(sub(p, self.origin), n))
    }

    /// 平面の局所の座標（X・Y が平面の上、Z が法線）から世界への変換。平面の上の 2D の座標は、
    /// 表から見て反時計回りが正になる
    pub fn frame(&self) -> Option<Transform> {
        Transform::from_axes(self.origin, Some(self.normal), None)
    }

    /// 世界の点を、平面の上の 2D の座標に写す
    pub fn project(frame: &Transform, p: Xyz) -> Xy {
        let d = sub(p, frame.column(3));
        [dot(d, frame.column(0)), dot(d, frame.column(1))]
    }
}
