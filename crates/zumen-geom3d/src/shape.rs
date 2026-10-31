//! 評価する形。形状表現の形の項目（決定 0207）を、形式を知らない形にしたもの

use crate::{Plane, Transform, Xy, Xyz};

/// 断面の輪郭の曲線。閉じた輪とし、最後の点から最初の点へも辺がある
#[derive(Clone, Debug, PartialEq)]
pub enum Curve {
    /// 折れ線
    Polygon(Vec<Xy>),
    /// 円
    Circle {
        /// 中心
        center: Xy,
        /// 半径
        radius: f64,
    },
}

/// 断面（局所の XY 平面の上の領域）
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    /// 外の輪
    pub outer: Curve,
    /// 穴の輪
    pub holes: Vec<Curve>,
}

/// 面。外の輪と穴の輪を、表から見て反時計回りに持つ
#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    /// 外の輪
    pub outer: Vec<Xyz>,
    /// 穴の輪
    pub holes: Vec<Vec<Xyz>>,
}

/// ブーリアンの演算
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BooleanOp {
    /// 和
    Union,
    /// 差（1 つめから 2 つめを除く）
    Difference,
    /// 積
    Intersection,
}

/// 評価する形
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// 断面の押し出し。断面を `position` の局所の XY に置き、局所の `direction` に `depth` だけ押し出す
    Extrusion {
        /// 断面
        profile: Profile,
        /// 断面を置く配置
        position: Transform,
        /// 押し出す向き（局所）
        direction: Xyz,
        /// 押し出す長さ
        depth: f64,
    },
    /// 断面の回転。局所の XY の断面を、局所の軸のまわりに `angle` 度だけ回す
    Revolution {
        /// 断面
        profile: Profile,
        /// 断面を置く配置
        position: Transform,
        /// 軸の上の点（局所）
        axis_origin: Xyz,
        /// 軸の向き（局所、XY 平面の上）
        axis_direction: Xyz,
        /// 回す角度（度）。正なら軸の向きに右ねじ
        angle: f64,
    },
    /// 円の断面の掃引。折れ線の通り道に沿って、円か円環の断面を動かす
    SweptDisk {
        /// 通り道
        directrix: Vec<Xyz>,
        /// 外の半径
        radius: f64,
        /// 内の半径（管）
        inner_radius: Option<f64>,
    },
    /// 面の集合
    FaceSet {
        /// 面
        faces: Vec<Face>,
        /// 閉じた立体か
        closed: bool,
    },
    /// 三角形の集合
    Triangles {
        /// 点
        positions: Vec<Xyz>,
        /// 三角形（点の番号、表から見て反時計回り）
        triangles: Vec<[u32; 3]>,
        /// 閉じた立体か
        closed: bool,
    },
    /// ブーリアン
    Boolean {
        /// 演算
        op: BooleanOp,
        /// 1 つめ
        first: Box<Shape>,
        /// 2 つめ
        second: Box<Shape>,
    },
    /// 半空間。平面の裏（法線と逆の側）を中身とする。ブーリアンの 2 つめに使う
    HalfSpace {
        /// 境の平面
        plane: Plane,
    },
    /// 写像。形を変換して置く
    Mapped {
        /// 変換
        transform: Transform,
        /// 形
        shape: Box<Shape>,
    },
    /// 形の組（1 つの形状表現の複数の項目）
    Group(Vec<Shape>),
}

impl Shape {
    /// 形の種類の名前。診断に使う
    pub fn kind(&self) -> &'static str {
        match self {
            Shape::Extrusion { .. } => "extrusion",
            Shape::Revolution { .. } => "revolution",
            Shape::SweptDisk { .. } => "swept_disk",
            Shape::FaceSet { .. } => "face_set",
            Shape::Triangles { .. } => "triangles",
            Shape::Boolean { .. } => "boolean",
            Shape::HalfSpace { .. } => "half_space",
            Shape::Mapped { .. } => "mapped",
            Shape::Group(_) => "group",
        }
    }
}
