//! 形状表現の文脈。座標系・精度・表示尺度を、適用前の値として保持する。

use crate::{AxisPlacement2, AxisPlacement3};

/// 表現の座標系。位置・方向の省略は補わず、元の次元を保つ
#[derive(Clone, Debug, PartialEq)]
pub enum ContextPlacement {
    /// 平面の座標系
    Plane(AxisPlacement2),
    /// 空間の座標系
    Space(AxisPlacement3),
}

/// 複数の形状表現が共有する幾何の文脈
#[derive(Clone, Debug, PartialEq)]
pub struct GeometricContext {
    /// 元の識別名
    pub identifier: Option<String>,
    /// 元の種類の名
    pub kind: Option<String>,
    /// 宣言した座標空間の次元
    pub dimension: u8,
    /// 元のモデル精度。省略時に許容差を補わない
    pub precision: Option<f64>,
    /// 工学座標系の原点と軸。地図への変換は含めない
    pub world_coordinate_system: ContextPlacement,
    /// XY 平面の真北の方向比。正規化も省略時の既定値の適用も行わない
    pub true_north: Option<[f64; 2]>,
}

/// 表示する投影の種類。投影や切断そのものは行わない
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetView {
    /// 接続を示す図
    Graph,
    /// 略図
    Sketch,
    /// 空間のモデル
    Model,
    /// 上から見た平面図
    Plan,
    /// 下から見た平面図
    ReflectedPlan,
    /// 断面図
    Section,
    /// 立面図
    Elevation,
    /// 利用側が名前を定めた投影
    UserDefined,
    /// 投影を指定しない
    NotDefined,
}

/// 親の幾何の文脈を使う表示の文脈
#[derive(Clone, Debug, PartialEq)]
pub struct SubContext {
    /// 子の識別名。親の識別名と分けて保持する
    pub identifier: Option<String>,
    /// 子の種類の名
    pub kind: Option<String>,
    /// 元の表示尺度。1:100 は 0.01。省略と明示した値を区別する
    pub target_scale: Option<f64>,
    /// 投影の種類
    pub target_view: TargetView,
    /// 利用側が定めた投影の名前
    pub user_defined_target_view: Option<String>,
}

/// 表現から参照する文脈。子でなければ view は None
#[derive(Clone, Debug, PartialEq)]
pub struct RepresentationContext {
    /// 直接の文脈、または子が参照する親の文脈
    pub geometric: GeometricContext,
    /// 子に固有の表示の指定
    pub view: Option<SubContext>,
}
