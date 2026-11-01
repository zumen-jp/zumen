//! 地理の領域モデル（L2、決定 0008・0233）。基盤地図情報・拡張 DM・CityGML の地物を、型・属性・LOD ごとの形で
//! 持つ。
//!
//! - 地物の型と属性の名前は、名前空間の接頭辞をつけた元の名前の文字で持つ。型とコードの語彙はプロファイルに置き、
//!   モデルは語彙を知らない。
//! - 座標は元の座標系の空間のまま持つ。使うときに、要求で名指した系に直す。
//! - 立体と面の集まりは、`zumen-geom3d` の面の集まり（`Shape::FaceSet`）にし、評価しないで持つ。

use zumen_base::Ref;
use zumen_crs::{Crs, GeoPoint, MapPoint, VerticalCrs};
use zumen_geom3d::Shape;

/// 地理の領域モデル
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GeoModel {
    /// 地物。順序は入力の順。子の地物は親の後に置く
    pub features: Vec<Feature>,
    /// 平面の座標系。元の形式から決まらなければ `None`。そのとき形は持たない
    pub crs: Option<Crs>,
    /// 高さの基準と成果。元の形式から決まらなければ `None`
    pub height: Option<VerticalCrs>,
}

/// 地物
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Feature {
    /// ID。元の ID（`gml:id`）か、親の ID と型と順の番号から作る
    pub id: String,
    /// 型。名前空間の接頭辞をつけた元の名前（`bldg:Building` など）
    pub kind: String,
    /// 分類コード。元の値の文字のまま
    pub code: Option<String>,
    /// 名前
    pub name: Option<String>,
    /// 属性。順序は入力の順
    pub attributes: Vec<Attribute>,
    /// 形。LOD の違う形をいくつも持てる
    pub geometries: Vec<Geometry>,
    /// 親の地物の番号（`GeoModel::features` の中の位置）
    pub parent: Option<usize>,
    /// 出所。元の要素のバイトの範囲
    pub source: Ref,
}

/// 属性
#[derive(Clone, Debug, PartialEq)]
pub struct Attribute {
    /// 名前。接頭辞をつけた元の名前。入れ子のデータ型の属性は、外から `/` でつなぐ
    pub name: String,
    /// 値
    pub value: Value,
}

/// 属性の値
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// 文字
    Text(String),
    /// 整数
    Integer(i64),
    /// 実数
    Real(f64),
    /// 真偽
    Bool(bool),
    /// 日付（`YYYY-MM-DD` の文字）
    Date(String),
    /// コード。コードの一覧の名前と、値の文字
    Code {
        /// コードの一覧（`codeSpace`）
        space: Option<String>,
        /// 値
        value: String,
    },
    /// 単位つきの量
    Measure {
        /// 値
        value: f64,
        /// 単位（`uom`）。元の文字のまま
        unit: Option<String>,
    },
}

/// 地物の形
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    /// LOD（0〜4）。2D の地物は `None`
    pub lod: Option<u8>,
    /// 形
    pub form: Form,
}

/// 形の種類
#[derive(Clone, Debug, PartialEq)]
pub enum Form {
    /// 点
    Point(Location),
    /// 線
    Line(Vec<Location>),
    /// 面。外周と穴
    Area {
        /// 外周
        outer: Vec<Location>,
        /// 穴
        holes: Vec<Vec<Location>>,
    },
    /// 立体と面の集まり。点（`[f64; 3]`）は、地理の空間なら（緯度・経度・標高）、地図の空間なら
    /// （`easting`・`northing`・標高）の順
    Solid(Shape),
}

/// 点の位置。座標系の空間の点と、標高
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Location {
    /// 平面の位置
    pub position: Position,
    /// 標高（m）。書かれていなければ `None`
    pub height: Option<f64>,
}

/// 平面の位置
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Position {
    /// 経緯度
    Geo(GeoPoint),
    /// 平面直角座標
    Map(MapPoint),
}
