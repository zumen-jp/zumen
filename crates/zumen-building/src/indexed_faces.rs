//! 点の添字で定義した評価前の面集合。元の点の対応・法線・タグを保持する。
//! 添字は 1 から始まり、変換・三角形化・法線の正規化は行わない。

/// 同じ IndexedFaces::Polygons の faces 内で共有する面への参照。0 から始まる
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexedPolygonRef(pub u32);

/// 点の添字で定義した多角形。向きと並びは元のまま
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedPolygon {
    /// 外輪の点。1 から始まる添字
    pub boundary: Vec<u32>,
    /// 内輪ごとの点。ないときは空。1 から始まる添字
    pub holes: Vec<Vec<u32>>,
}

/// 添字で定義した面の種類。三角形と多角形の元の構造を区別する
#[derive(Clone, Debug, PartialEq)]
pub enum IndexedFaces {
    /// 3 個の添字で定義した三角形
    Triangles {
        /// 三角形ごとの 3 個の添字。点と法線の両方を指す
        indices: Vec<[u32; 3]>,
        /// 頂点の法線。元の 3 成分を正規化せず持つ。点の対応表は適用しない
        normals: Option<Vec<[f64; 3]>>,
    },
    /// 共有する多角形の面
    Polygons {
        /// 面を並べる順。共有する面への参照も元の位置に保持する
        indices: Vec<IndexedPolygonRef>,
        /// 初めて参照した順の面
        faces: Vec<IndexedPolygon>,
    },
}

/// 添字で定義した面集合。点の単位と出所は取得結果に分けて持つ
#[derive(Clone, Debug, PartialEq)]
pub struct IndexedFaceSet {
    /// 3D の点。文書の単位のまま
    pub points: Vec<[f64; 3]>,
    /// 点ごとの元のタグ。省略時は None
    pub tags: Option<Vec<String>>,
    /// 面の添字から points の添字への対応。1 から始まる。None なら直接 points を指す
    pub point_index: Option<Vec<u32>>,
    /// 閉殻・開殻の申告。指定がなければ None。幾何の検査結果ではない
    pub closed: Option<bool>,
    /// 三角形または共有する多角形
    pub faces: IndexedFaces,
}
