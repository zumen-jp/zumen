//! 文書と成分の型（決定 0207）。値は文書の単位のまま、単位とともに持つ。

use crate::ElementRef;
use crate::placement::Transform;

/// 単位。文書の単位の割り当てか、値に書いた単位
#[derive(Clone, Debug, PartialEq)]
pub struct Unit {
    /// 単位の種類（`LENGTHUNIT`・`AREAUNIT` など）
    pub kind: String,
    /// SI の接頭辞（`MILLI` など）
    pub prefix: Option<String>,
    /// 単位の名前（`METRE`、換算する単位なら `inch` など）
    pub name: String,
    /// 換算する単位の、SI の単位での大きさ。SI の単位は `None`
    pub scale: Option<f64>,
}

/// 単位とともに持つ値。単位が解けなければ `unit` は `None`
#[derive(Clone, Debug, PartialEq)]
pub struct Measured {
    /// 文書の単位のままの値
    pub value: f64,
    /// 単位
    pub unit: Option<Unit>,
}

/// プロパティなどの値
#[derive(Clone, Debug, PartialEq)]
pub enum Literal {
    /// 実数
    Real(f64),
    /// 整数
    Integer(i64),
    /// 文字列。ISO 10303-21 のエスケープを戻したもの
    Text(String),
    /// 真偽
    Boolean(bool),
    /// 真・偽・不明（`None`）
    Logical(Option<bool>),
    /// 列挙の値
    Enumeration(String),
    /// 要素への参照
    Element(ElementRef),
    /// 値の並び
    List(Vec<Literal>),
}

/// 文書。プロジェクト・単位・文脈・座標・書き出し・見出しの値（決定 0207 の「文書」）
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    /// プロジェクトの要素
    pub project: Option<ElementRef>,
    /// プロジェクトの名前
    pub project_name: Option<String>,
    /// プロジェクトの段階
    pub phase: Option<String>,
    /// 単位の割り当て
    pub units: Vec<Unit>,
    /// 表現の文脈
    pub contexts: Vec<Context>,
    /// 真北の向き（プロジェクトの空間の X と Y）
    pub true_north: Option<[f64; 2]>,
    /// 地図への変換（決定 0019）
    pub map: Option<MapConversion>,
    /// 書き出したアプリケーション
    pub application: Option<Application>,
    /// 見出しの値
    pub header: Header,
}

/// 表現の文脈
#[derive(Clone, Debug, PartialEq)]
pub struct Context {
    /// 文脈の識別（`Body`・`Axis` など）
    pub identifier: Option<String>,
    /// 文脈の種類（`Model`・`Plan` など）
    pub kind: Option<String>,
    /// 次元
    pub dimension: Option<u32>,
    /// 精度（決定 0018 の文書の許容差）
    pub precision: Option<f64>,
}

/// 地図への変換と座標系の名前（決定 0019）
#[derive(Clone, Debug, PartialEq)]
pub struct MapConversion {
    /// 座標系の名前（`EPSG:6677` など）
    pub crs_name: Option<String>,
    /// 東の向きの移動
    pub eastings: f64,
    /// 北の向きの移動
    pub northings: f64,
    /// 高さの移動
    pub height: f64,
    /// 地図の X 軸の向きの、横の成分
    pub x_axis_abscissa: Option<f64>,
    /// 地図の X 軸の向きの、縦の成分
    pub x_axis_ordinate: Option<f64>,
    /// 縮尺
    pub scale: Option<f64>,
}

/// 書き出したアプリケーション
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Application {
    /// 名前
    pub name: String,
    /// 版
    pub version: Option<String>,
}

/// 見出しの値（`FILE_DESCRIPTION`・`FILE_NAME`・`FILE_SCHEMA`）
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Header {
    /// ビューの定義などの説明
    pub description: Vec<String>,
    /// ファイルの名前
    pub name: Option<String>,
    /// 書いた日時
    pub time_stamp: Option<String>,
    /// 作者
    pub author: Vec<String>,
    /// 組織
    pub organization: Vec<String>,
    /// 前処理系
    pub preprocessor: Option<String>,
    /// 作成した系
    pub originating_system: Option<String>,
    /// 承認者
    pub authorization: Option<String>,
    /// スキーマの名前
    pub schemas: Vec<String>,
}

/// `identity`: 名前・説明・タグ・長い名前
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Identity {
    /// 名前
    pub name: Option<String>,
    /// 説明
    pub description: Option<String>,
    /// タグ
    pub tag: Option<String>,
    /// 長い名前
    pub long_name: Option<String>,
}

/// `class`: IFC のクラス
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Class {
    /// IFC4.3 のクラスの名前。寄せられなければ元の名前
    pub name: String,
    /// 定義された種類
    pub predefined: Option<String>,
    /// 利用者が付けた種類
    pub user_defined: Option<String>,
    /// 元のスキーマでのクラスの名前
    pub original: String,
}

/// `classification`: 分類の項目
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Classification {
    /// 分類の項目
    pub references: Vec<ClassificationReference>,
}

/// 分類の項目
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassificationReference {
    /// 分類体系の名前
    pub system: Option<String>,
    /// 分類体系の版
    pub edition: Option<String>,
    /// 項目のコード
    pub code: Option<String>,
    /// 項目の名前
    pub name: Option<String>,
}

/// 空間の種類
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpatialKind {
    /// 敷地
    Site,
    /// 建物
    Building,
    /// 階
    Storey,
    /// 室
    Space,
    /// 空間の区域
    SpatialZone,
    /// ほかの空間（IFC4.3 の施設など）。クラスの名前
    Other(String),
}

/// `spatial`: 空間の種類と高さ
#[derive(Clone, Debug, PartialEq)]
pub struct Spatial {
    /// 空間の種類
    pub kind: SpatialKind,
    /// 階の高さ・建物の標高など
    pub elevation: Option<Measured>,
    /// 敷地の標高
    pub ref_elevation: Option<Measured>,
}

/// 所属の種類
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParentKind {
    /// 集約（`IfcRelAggregates`）
    Aggregates,
    /// 入れ子（`IfcRelNests`）
    Nests,
    /// 空間への包含（`IfcRelContainedInSpatialStructure`）
    Contained,
}

/// 所属の先
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Membership {
    /// 所属の先の要素
    pub target: ElementRef,
    /// 所属の種類
    pub kind: ParentKind,
    /// 入れ子の順
    pub order: Option<u32>,
}

/// `parent`: 所属の先と、空間への参照
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Parent {
    /// 所属の先
    pub of: Option<Membership>,
    /// 複数の階にわたる要素の、空間への参照（`IfcRelReferencedInSpatialStructure`）
    pub referenced_in: Vec<ElementRef>,
}

/// 局所の配置
#[derive(Clone, Debug, PartialEq)]
pub enum Local {
    /// 局所の変換。軸は正規直交にしてある
    Transform(Transform),
    /// 解けない配置（`IfcGridPlacement`・`IfcLinearPlacement` など）。種類の名前
    Unsupported(String),
}

/// `placement`: 親の配置を持つ要素と、局所の変換
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    /// 親の配置を持つ要素。なければプロジェクトの空間に置く
    pub relative_to: Option<ElementRef>,
    /// 局所の配置
    pub local: Local,
}

/// 形の項目の木。項目の型は、試料で一覧を決めてから足す（決定 0207 の「試料を得てから決めること」の 1）
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShapeItem {
    /// 項目の種類（`IfcExtrudedAreaSolid` など）
    pub kind: String,
    /// 子の項目
    pub children: Vec<ShapeItem>,
}

/// 形状表現
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShapeRepresentation {
    /// 文脈（`Body`・`Axis`・`FootPrint` など）
    pub context: Option<String>,
    /// 表現の種類（`SweptSolid`・`Tessellation` など）
    pub kind: Option<String>,
    /// 形の項目
    pub items: Vec<ShapeItem>,
}

/// `representation`: 形状表現。評価しない
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Representation {
    /// 形状表現
    pub representations: Vec<ShapeRepresentation>,
}

/// `style`: 面と線の色、表示のレイヤ
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Style {
    /// 面の色（0〜1 の RGB）
    pub surface_color: Option<[f64; 3]>,
    /// 面の透明度（0〜1）
    pub transparency: Option<f64>,
    /// 線の色（0〜1 の RGB）
    pub curve_color: Option<[f64; 3]>,
    /// 表示のレイヤの名前
    pub layers: Vec<String>,
}

/// `type`: 型の要素
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeOf {
    /// 型の要素
    pub target: ElementRef,
}

/// IFC の値の型の名前と値
#[derive(Clone, Debug, PartialEq)]
pub struct TypedValue {
    /// 値の型の名前（`IfcLengthMeasure`・`IfcLabel` など）
    pub type_name: Option<String>,
    /// 値
    pub value: Literal,
}

/// プロパティ
#[derive(Clone, Debug, PartialEq)]
pub struct Property {
    /// 名前
    pub name: String,
    /// 値。値の並びは `Literal::List` にする
    pub value: Option<TypedValue>,
    /// 単位。プロパティに書いた単位か、文書の単位の割り当てから値の型に合うもの
    pub unit: Option<Unit>,
}

/// プロパティのセット
#[derive(Clone, Debug, PartialEq)]
pub struct PropertySet {
    /// セットの名前
    pub name: String,
    /// プロパティ
    pub properties: Vec<Property>,
}

/// `properties`: プロパティのセット
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Properties {
    /// プロパティのセット
    pub sets: Vec<PropertySet>,
}

/// 数量の種類
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuantityKind {
    /// 長さ
    Length,
    /// 面積
    Area,
    /// 体積
    Volume,
    /// 個数
    Count,
    /// 重さ
    Weight,
    /// 時間
    Time,
}

/// 数量
#[derive(Clone, Debug, PartialEq)]
pub struct Quantity {
    /// 名前
    pub name: String,
    /// 種類
    pub kind: QuantityKind,
    /// 値と単位
    pub value: Measured,
    /// 式
    pub formula: Option<String>,
}

/// 数量のセット
#[derive(Clone, Debug, PartialEq)]
pub struct QuantitySet {
    /// セットの名前
    pub name: String,
    /// 測り方の名前
    pub method: Option<String>,
    /// 数量
    pub quantities: Vec<Quantity>,
}

/// `quantities`: ファイルに書かれた数量のセット
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Quantities {
    /// 数量のセット
    pub sets: Vec<QuantitySet>,
}

/// 材料の構成の種類
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialKind {
    /// 単一
    Single,
    /// 層
    Layers,
    /// 断面
    Profiles,
    /// 構成要素
    Constituents,
    /// 並び
    List,
}

/// 材料の構成の 1 つ（層・断面・構成要素）
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialPart {
    /// 材料の要素
    pub material: Option<ElementRef>,
    /// 名前
    pub name: Option<String>,
    /// 層の厚さ
    pub thickness: Option<Measured>,
}

/// `material`: 材料の構成
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    /// 構成の種類
    pub kind: MaterialKind,
    /// 構成の 1 つずつ
    pub parts: Vec<MaterialPart>,
    /// 層の向き（`AXIS2` など）
    pub direction: Option<String>,
    /// 層の基準からのずれ
    pub offset: Option<Measured>,
}

/// `opening`: 開口と充填
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opening {
    /// 開口の側。穴をあける要素（`IfcRelVoidsElement`）
    Voids(ElementRef),
    /// 建具の側。埋める開口（`IfcRelFillsElement`）
    Fills(ElementRef),
}

/// `group`: 所属するグループ・系統・区域
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Group {
    /// グループの要素
    pub groups: Vec<ElementRef>,
}

/// 接続
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    /// 関係の種類（`IfcRelConnectsPathElements`・`IfcRelSpaceBoundary` など）
    pub kind: String,
    /// 接続する要素・ポート
    pub target: ElementRef,
}

/// `connection`: 接続する要素・ポートと、空間の境界
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Connection {
    /// 接続
    pub links: Vec<Link>,
}

/// 通り芯の組
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridSet {
    /// U
    U,
    /// V
    V,
    /// W
    W,
}

/// `grid`: 通り芯と、その軸
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Grid {
    /// 通り芯。軸の要素と組
    Grid {
        /// 軸の要素と組
        axes: Vec<(GridSet, ElementRef)>,
    },
    /// 通り芯の軸
    Axis {
        /// 軸の名前（`X1` など）
        tag: Option<String>,
        /// 曲線の向きが軸と同じか
        same_sense: bool,
        /// 曲線
        curve: Option<ShapeItem>,
    },
}

/// `level`: レベル。どの実体とプロパティから作るかは、試料で決める
#[derive(Clone, Debug, PartialEq)]
pub struct Level {
    /// 名前
    pub name: Option<String>,
    /// 種類（GL・FL など。語彙はプロファイルに置く）
    pub kind: Option<String>,
    /// 高さ
    pub height: Measured,
    /// 高さの基準
    pub datum: Option<String>,
}

/// `zone`: 区画。どの実体とプロパティから作るかは、試料で決める
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Zone {
    /// 区画の種類（防火区画など。語彙はプロファイルに置く）
    pub kind: Option<String>,
    /// 構成する室
    pub members: Vec<ElementRef>,
}

/// 名前空間つきの型付きの値
#[derive(Clone, Debug, PartialEq)]
pub struct ExtensionValue {
    /// 名前空間
    pub namespace: String,
    /// 名前
    pub name: String,
    /// 値
    pub value: TypedValue,
}

/// `extension`: 形式に固有の値
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extension {
    /// 値
    pub values: Vec<ExtensionValue>,
}
