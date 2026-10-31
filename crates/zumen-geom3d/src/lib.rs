//! 3D の幾何（決定 0018）。点・ベクトル・配置、要素の ID と辺を持つメッシュ、形状表現を評価する
//! カーネルの trait と、平面での切断（→ 2D）の trait を持つ。
//!
//! 形式も領域も知らない（決定 0008）。IFC などの形の項目は、使う側が [`Shape`] に写してから評価する。
//! 長さの単位は、使う側が決める（建物モデルなら文書の長さの単位）。角度は度。三角関数は `libm` で
//! 求める（決定 0007）。
//!
//! カーネルは trait の裏に置き、日本の BIM ソフトの書き出しで測って選ぶ（決定 0018、#5）。
//! [`cases`] は、候補を試料なしで比べるための、合成の形と期待する値の組。

pub mod cases;
mod kernel;
mod mesh;
mod section;
mod shape;
mod transform;

pub use kernel::{Failure, Kernel, Section, Tolerance};
pub use mesh::{Mesh, Part};
pub use section::MeshSection;
pub use shape::{BooleanOp, Curve, Face, Profile, Shape};
pub use transform::{Plane, Transform};
pub use zumen_geom2d::Xy;

/// 点とベクトル
pub type Xyz = [f64; 3];

/// `a + b`
pub fn add(a: Xyz, b: Xyz) -> Xyz {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// `a − b`
pub fn sub(a: Xyz, b: Xyz) -> Xyz {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `a × s`
pub fn scale(a: Xyz, s: f64) -> Xyz {
    [a[0] * s, a[1] * s, a[2] * s]
}

/// 内積
pub fn dot(a: Xyz, b: Xyz) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// 外積
pub fn cross(a: Xyz, b: Xyz) -> Xyz {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// 長さ
pub fn length(a: Xyz) -> f64 {
    libm::sqrt(dot(a, a))
}

/// 長さを 1 にしたベクトル。長さが 0 か、有限でなければ `None`
pub fn normalize(a: Xyz) -> Option<Xyz> {
    let n = length(a);
    (n.is_finite() && n > 0.0).then(|| scale(a, 1.0 / n))
}

/// すべての成分が有限か
pub fn is_finite(a: Xyz) -> bool {
    a.iter().all(|v| v.is_finite())
}
