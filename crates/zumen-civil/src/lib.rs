//! 土木の領域モデル（L2、決定 0230）。J-LandXML と IFC4.3 の線形を同じ形で受ける。
//!
//! 線形は、平面・縦断・片勾配を区間を並べて持ち、曲線は種類のまま持つ（決定 0018）。点は地図の空間
//! （`northing`・`easting`、メートル。決定 0019）に置く。方向角は、北から時計回りのラジアンにする。
//! 線形の上の位置は、線形の初めからの平面の距離（追加距離）で表し、測点の値と表記は [`station`] で換算する。

use zumen_base::Ref;
use zumen_crs::{Crs, HeightDatum, MapPoint, Realization};

mod position;
pub mod station;

pub use position::{Placement, check, curvature, length, place};
pub use station::{StationBreak, Stationing};

/// 土木の領域モデル。線形・横断・地表面を持つ
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CivilModel {
    /// 線形
    pub alignments: Vec<Alignment>,
    /// 地表面
    pub surfaces: Vec<Tin>,
    /// 水平の座標系。分からなければ `None`
    pub crs: Option<Crs>,
    /// 高さの基準。分からなければ `None`
    pub height_datum: Option<HeightDatum>,
    /// 標高の成果。分からなければ `None`
    pub realization: Option<Realization>,
}

/// 道路・鉄道の中心の線
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Alignment {
    /// ID（#1 の規則。J-LandXML は名前、IFC は `GlobalId` から作る）
    pub id: String,
    /// 名前
    pub name: Option<String>,
    /// 用途。元の形式の値を文字のまま持つ
    pub usage: Option<String>,
    /// 平面線形の区間
    pub horizontal: Vec<Horizontal>,
    /// 縦断線形の区間。なければ空
    pub vertical: Vec<Vertical>,
    /// 片勾配とカントの区間。なければ空
    pub cant: Vec<Cant>,
    /// 測点の定め
    pub stations: Stationing,
    /// 横断
    pub cross_sections: Vec<CrossSection>,
    /// 出所
    pub source: Ref,
}

/// 平面線形の区間。初めの点と向きは、元の形式の値をそのまま持つ
#[derive(Clone, Debug, PartialEq)]
pub struct Horizontal {
    /// 初めの点
    pub start: MapPoint,
    /// 初めの方向角（北から時計回りのラジアン）
    pub direction: f64,
    /// 長さ（メートル）
    pub length: f64,
    /// 種類
    pub kind: HorizontalKind,
    /// 出所
    pub source: Ref,
}

/// 平面線形の区間の種類
#[derive(Clone, Debug, PartialEq)]
pub enum HorizontalKind {
    /// 直線
    Line,
    /// 円弧。半径は、右へ曲がるものを正にする
    Arc {
        /// 半径（メートル）
        radius: f64,
    },
    /// 緩和曲線。半径は、右へ曲がるものを正にし、直線の側は `None` にする
    Transition {
        /// 初めの半径
        start_radius: Option<f64>,
        /// 終わりの半径
        end_radius: Option<f64>,
        /// 曲線の種類
        curve: TransitionCurve,
    },
}

/// 緩和曲線の種類
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransitionCurve {
    /// クロソイド
    Clothoid,
    /// 3 次放物線
    CubicParabola,
    /// サイン半波長逓減曲線
    SineHalfWave,
    /// コサイン曲線
    Cosine,
    /// ブロス曲線
    Bloss,
    /// ヘルマート曲線
    Helmert,
    /// ウィーン曲線
    Viennese,
    /// ほかの曲線。元の名前のまま持つ
    Other(String),
}

/// 縦断線形の区間
#[derive(Clone, Debug, PartialEq)]
pub struct Vertical {
    /// 初めの追加距離（メートル）
    pub start_distance: f64,
    /// 初めの高さ（メートル）
    pub start_height: f64,
    /// 初めの勾配（高さ ÷ 平面の距離）
    pub start_gradient: f64,
    /// 平面の距離での長さ（メートル）
    pub length: f64,
    /// 種類
    pub kind: VerticalKind,
    /// 出所。J-LandXML では、区間の初めの縦断の変化点
    pub source: Ref,
}

/// 縦断線形の区間の種類
#[derive(Clone, Debug, PartialEq)]
pub enum VerticalKind {
    /// 一定の勾配
    ConstantGradient,
    /// 2 次放物線の縦断曲線
    Parabola {
        /// 終わりの勾配
        end_gradient: f64,
    },
    /// 円弧の縦断曲線。半径は、上に凸のものを正にする
    CircularArc {
        /// 半径（メートル）
        radius: f64,
    },
    /// クロソイドの縦断曲線
    Clothoid {
        /// 初めの半径
        start_radius: Option<f64>,
        /// 終わりの半径
        end_radius: Option<f64>,
    },
}

/// 片勾配（道路）とカント（鉄道）の区間
#[derive(Clone, Debug, PartialEq)]
pub struct Cant {
    /// 初めの追加距離（メートル）
    pub start_distance: f64,
    /// 長さ（メートル）
    pub length: f64,
    /// 値
    pub kind: CantKind,
    /// 変わり方。元の形式の値を文字のまま持つ（直線・3 次など）
    pub transition: Option<String>,
    /// 出所
    pub source: Ref,
}

/// 片勾配かカントか
#[derive(Clone, Debug, PartialEq)]
pub enum CantKind {
    /// 左右の横断勾配（%）。初めと終わり
    Superelevation {
        /// 初めの左と右
        start: [f64; 2],
        /// 終わりの左と右
        end: [f64; 2],
    },
    /// 鉄道のカント（高さの差、mm）。初めと終わり
    Rail {
        /// 初めの左と右
        start: [f64; 2],
        /// 終わりの左と右
        end: [f64; 2],
        /// 軌間（mm）
        gauge: Option<f64>,
    },
}

/// 横断
#[derive(Clone, Debug, PartialEq)]
pub struct CrossSection {
    /// 追加距離（メートル）
    pub distance: f64,
    /// 名前
    pub name: Option<String>,
    /// 横断の線
    pub lines: Vec<CrossSectionLine>,
    /// 出所
    pub source: Ref,
}

/// 横断の線。点は、線形の中心からの離れ（右を正、メートル）と高さ（メートル）
#[derive(Clone, Debug, PartialEq)]
pub struct CrossSectionLine {
    /// 名前
    pub name: Option<String>,
    /// 役割
    pub role: SurfaceRole,
    /// 点
    pub points: Vec<[f64; 2]>,
}

/// 地表面と横断の線の役割
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SurfaceRole {
    /// 現況（地盤）
    Existing,
    /// 計画
    Design,
    /// ほか。元の値を文字のまま持つ
    Other(String),
}

/// 地表面の TIN
#[derive(Clone, Debug, PartialEq)]
pub struct Tin {
    /// ID
    pub id: String,
    /// 名前
    pub name: Option<String>,
    /// 役割
    pub role: SurfaceRole,
    /// 点と標高（メートル）
    pub points: Vec<(MapPoint, f64)>,
    /// 三角形（点の番号の 3 つ組）
    pub triangles: Vec<[u32; 3]>,
    /// 境界線（点の番号の列）
    pub boundaries: Vec<Vec<u32>>,
    /// ブレーク線（点の番号の列）
    pub breaklines: Vec<Vec<u32>>,
    /// 出所
    pub source: Ref,
}
