//! 要素が持つ評価前の形・表示・通り芯。出所の番号と意味値を分け、L1 を借用しない。

use crate::{
    BooleanOperation, CurveGraph, DiskSweep, FaceGraph, GridAxisDefinition, GridLayout,
    IndexedFaceSet, PlaneHalfSpace, PresentationLayer, ProfileExtrusion, ProfileRevolution,
    RepresentationContext, ShapeMapping3, SurfaceStyle, Unit,
};
use zumen_base::{Cx, Stop};

/// 形の参照を検査する途中のエラー。幾何の妥当性の検査ではない
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometryError {
    /// このグラフにない項目を参照した
    Reference,
    /// 子から親へ戻る循環がある
    Cycle,
    /// 根から到達しない項目がある
    Unreachable,
    /// 定義が要求する曲線などの値がない
    MissingDefinition,
    /// 予算または中断による停止
    Stopped(Stop),
}

impl From<Stop> for GeometryError {
    fn from(value: Stop) -> Self {
        Self::Stopped(value)
    }
}

/// 参照を検査済みの所有する形。値の変更には作り直しが必要で、取得によってハッシュが変わらない
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    data: GeometryData,
    fingerprint: crate::Hash,
}

impl Geometry {
    /// 参照・循環・到達性を検査し、出所を含めない意味値のハッシュを作る
    pub fn new(data: GeometryData, cx: &mut Cx) -> Result<Self, GeometryError> {
        let fingerprint = crate::hash::geometry_digest(&data, cx)?;
        Ok(Self { data, fingerprint })
    }

    /// 所有する元の値。位置・単位・向きは未適用
    pub fn data(&self) -> &GeometryData {
        &self.data
    }

    /// 出所を除いた値と共有関係のハッシュ
    pub fn fingerprint(&self) -> crate::Hash {
        self.fingerprint
    }
}

/// 取得した値。未対応を空の値や省略と区別する
#[derive(Clone, Debug, PartialEq)]
pub enum Acquired<T> {
    /// 取得できた値
    Value(T),
    /// 未対応。種類と出所は同じ項目と診断に残す
    Unsupported,
}

/// 同じ GeometryData の nodes 内でだけ使う参照。永続 ID ではない
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeometryRef(pub u32);

/// 値の列に対応する出所の番号。ハッシュには含めない
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeometrySourceGroup {
    /// 出所の列の役割。points・faces・units など
    pub role: String,
    /// 対応する値と同じ並びの元の番号
    pub instances: Vec<u64>,
}

/// 形や表示の出所。形式のレコードそのものは持たない
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GeometrySource {
    /// 項目の元の番号。利用側で作った値なら None
    pub instance: Option<u64>,
    /// 値に対応する出所を役割ごとに保持する
    pub groups: Vec<GeometrySourceGroup>,
}

/// 形の構造。子の所有権は再帰せず、共有する項目を参照する
#[derive(Clone, Debug, PartialEq)]
pub enum GeometryLinks {
    /// 要素の表現の一覧
    Definition(Vec<GeometryRef>),
    /// 1 つの表現。文脈の値は同じ項目の definition に持つ
    Representation {
        /// 元の表現の識別名
        identifier: Option<String>,
        /// 元の表現の種類の名
        kind: Option<String>,
        /// 項目の並び
        items: Vec<GeometryRef>,
    },
    /// 共有写像の適用。原点・軸・尺度は同じ項目の definition に持つ
    Mapped(GeometryRef),
    /// 共有写像の元の表現。基準位置は参照する Mapped の定義が保持する
    Map(GeometryRef),
    /// 幾何の集合
    Set {
        /// 曲線だけを許す集合か
        curves_only: bool,
        /// 元の並びの項目
        items: Vec<GeometryRef>,
    },
    /// 集合演算。対象の向きと順序を保持する
    Boolean {
        /// 演算
        operation: BooleanOperation,
        /// 第 1 の対象
        first: GeometryRef,
        /// 第 2 の対象
        second: GeometryRef,
    },
    /// 値を持つ葉、または未対応の項目
    Leaf,
}

/// 評価前の形の元値。単位換算・写像・形の評価は行わない
#[derive(Clone, Debug, PartialEq)]
pub enum GeometryDefinition {
    /// 構造だけを持つ項目
    Links,
    /// 表現の文脈
    Context {
        /// 座標系・精度・表示尺度
        definition: RepresentationContext,
        /// 座標と精度の元の単位
        length_unit: Option<Unit>,
    },
    /// 曲線
    Curve(CurveGraph),
    /// 断面の押し出し
    Extrusion {
        /// 断面と押し出し
        definition: ProfileExtrusion,
        /// 任意断面の外輪と内輪
        boundary: Option<CurveGraph>,
        /// 元の長さの単位
        length_unit: Option<Unit>,
    },
    /// 断面の回転
    Revolution {
        /// 断面・軸・角度
        definition: ProfileRevolution,
        /// 任意断面の外輪と内輪
        boundary: Option<CurveGraph>,
        /// 元の長さの単位
        length_unit: Option<Unit>,
    },
    /// 円盤の掃引
    SweptDisk {
        /// 半径・区間・経路の参照
        definition: DiskSweep,
        /// 経路と元の単位
        directrix: CurveGraph,
    },
    /// 多角形の面集合
    Faces {
        /// 点・ループ・境界・面・面集合
        definition: FaceGraph,
        /// 元の長さの単位
        length_unit: Option<Unit>,
    },
    /// 添字付きの面集合
    Tessellation {
        /// 点・法線・添字・内外輪
        definition: IndexedFaceSet,
        /// 元の長さの単位
        length_unit: Option<Unit>,
    },
    /// 平面の半空間
    HalfSpace {
        /// 基準面・向き・囲み
        definition: PlaneHalfSpace,
        /// 多角形の囲みが持つ境界
        boundary: Option<CurveGraph>,
        /// 元の長さの単位
        length_unit: Option<Unit>,
    },
    /// 共有する形の写像
    Mapping {
        /// 写像元の位置と写像先の変換
        definition: ShapeMapping3,
        /// 元の長さの単位
        length_unit: Option<Unit>,
    },
    /// 未対応の定義。元の属性は L1 だけが保持する
    Unsupported(String),
}

/// 共有する形の項目
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryNode {
    /// 子の参照
    pub links: GeometryLinks,
    /// 元の値
    pub definition: GeometryDefinition,
    /// 値から分けた出所
    pub source: GeometrySource,
}

/// レイヤを直接割り当てた位置
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerAssignmentLevel {
    /// 割り当てなし
    None,
    /// 表現に割り当てた
    Representation,
    /// 項目に割り当てた
    Item,
}

/// 面の表示を選ぶ規則の結果
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceStyleSelection {
    /// 指定なし
    None,
    /// 直接の指定
    Direct,
    /// 同じ表示の layers 内の位置
    Layer(u32),
    /// 複数のレイヤの指定が競合する。並びで勝者を決めない
    MultipleLayers,
}

/// 面の表示と出所
#[derive(Clone, Debug, PartialEq)]
pub struct GeometrySurfaceStyle {
    /// 元の値
    pub definition: SurfaceStyle,
    /// 表示・色などの出所
    pub source: GeometrySource,
}

/// レイヤの表示
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryLayer {
    /// 元の名前と表示・固定・編集の指定
    pub definition: PresentationLayer,
    /// 面の表示
    pub styles: Vec<GeometrySurfaceStyle>,
    /// 割り当てた位置
    pub level: LayerAssignmentLevel,
    /// 元のレイヤ
    pub source: GeometrySource,
}

/// 直接とレイヤから取得した表示。上書きされた指定も保持する
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryPresentation {
    /// 直接の割り当ての名前
    pub name: Option<String>,
    /// 直接の面の表示
    pub direct: Vec<GeometrySurfaceStyle>,
    /// 関係する全レイヤ
    pub layers: Vec<GeometryLayer>,
    /// レイヤの適用元
    pub layer_source: LayerAssignmentLevel,
    /// 面の表示の適用元
    pub surface_source: SurfaceStyleSelection,
    /// 直接の割り当ての出所
    pub source: GeometrySource,
}

/// 表示を使う位置。同じ形でも直接含む表現ごとに区別する
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryStyle {
    /// 形の項目
    pub item: GeometryRef,
    /// 直接含む表現。集合や演算の子だけの場合は None
    pub representation: Option<GeometryRef>,
    /// 取得した表示
    pub presentation: Acquired<GeometryPresentation>,
}

/// 通り芯の 1 軸
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryGridAxis {
    /// 所属・名前・向き
    pub definition: GridAxisDefinition,
    /// 元の曲線
    pub curve: CurveGraph,
    /// 軸の出所
    pub source: GeometrySource,
}

/// 通り芯。配置と向きは未適用
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryGrid {
    /// U・V・W の順の軸
    pub axes: Vec<GeometryGridAxis>,
    /// 元の配置の種類
    pub layout: Option<GridLayout>,
    /// 通り芯・配置・表現の出所
    pub source: GeometrySource,
}

/// 1 要素の所有する形・表示・通り芯。参照はこの値の nodes 内に閉じる
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryData {
    /// 形の定義。省略と空の表現を区別する
    pub root: Option<GeometryRef>,
    /// 共有する項目
    pub nodes: Vec<GeometryNode>,
    /// 項目と表現の組ごとの表示
    pub styles: Vec<GeometryStyle>,
    /// 通り芯でなければ None
    pub grid: Option<Acquired<GeometryGrid>>,
    /// 要素と配置の出所。配置の意味値は Building の placement が持つ
    pub source: GeometrySource,
}
