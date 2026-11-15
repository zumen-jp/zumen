//! 評価前の多角形の面集合。点・ループ・境界・面の共有と元の向きを保持する。
//! 出所の番号、三角形化、平面性・交差・閉殻の検査結果は含めない。

/// 同じ FaceGraph 内の点への参照
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FacePointRef(pub u32);
/// 同じ FaceGraph 内の閉じた多角形への参照
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FaceLoopRef(pub u32);
/// 同じ FaceGraph 内の面の境界への参照
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FaceBoundRef(pub u32);
/// 同じ FaceGraph 内の面への参照
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FaceRef(pub u32);
/// 同じ FaceGraph 内の連結した面集合への参照
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FaceSetRef(pub u32);

/// 面集合で表した形の種類。幾何の検査を済ませたことは示さない
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaceModelKind {
    /// 閉じた外殻で表す立体
    FacetedSolid,
    /// 開殻または閉殻の集合で表す面
    ShellSurface,
    /// 連結した面集合で表す面
    FaceSurface,
}

/// 連結した面集合。並びは取得時のまま保持する
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectedFaces {
    /// 面への参照
    pub faces: Vec<FaceRef>,
    /// 閉殻は Some(true)、開殻は Some(false)、指定がなければ None。検証結果ではない
    pub closed: Option<bool>,
}

/// 多角形の境界で表す面。外輪の指定がなくても先頭を外輪と決めない
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolygonFace {
    /// 面の境界への参照
    pub bounds: Vec<FaceBoundRef>,
}

/// 面の境界。ループの点の順を変えず、向きと外輪の指定を保持する
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FaceBoundary {
    /// 境界の多角形
    pub polygon: FaceLoopRef,
    /// ループと同じ向きで面の境界をたどるか
    pub orientation: bool,
    /// 外輪と明示されているか
    pub outer: bool,
}

/// 閉じた多角形。最後から最初へ結ぶ。閉じる点を末尾に足さない
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FacePolygon {
    /// 元の順序の点への参照
    pub points: Vec<FacePointRef>,
}

/// 面集合の定義。各表は最初に参照した順で、所有権は再帰しない。
/// 参照はこのグラフ内だけで有効。要素の ID や意味値のハッシュではない
#[derive(Clone, Debug, PartialEq)]
pub struct FaceGraph {
    /// 形の種類
    pub kind: FaceModelKind,
    /// 形を構成する面集合
    pub roots: Vec<FaceSetRef>,
    /// 連結した面集合
    pub sets: Vec<ConnectedFaces>,
    /// 面
    pub faces: Vec<PolygonFace>,
    /// 境界
    pub bounds: Vec<FaceBoundary>,
    /// 多角形のループ
    pub loops: Vec<FacePolygon>,
    /// 3D の座標。文書の単位のまま保持する
    pub points: Vec<[f64; 3]>,
}
