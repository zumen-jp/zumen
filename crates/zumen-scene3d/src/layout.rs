// 生成したファイル。手で直さない。
// 元は schema/scene3d.toml で、`cargo xtask codegen` で作り直す。

//! 3D のシーンの配置（#417・#421）。

/// ヘッダーの始まりの 4 バイト
pub const MAGIC: [u8; 4] = *b"ZMS3";
/// メジャーバージョン。セクションの意味を変えるときに上げる
pub const MAJOR: u16 = 1;
/// マイナーバージョン。セクションか値を足すたびに上げる
pub const MINOR: u16 = 1;
/// 番号の「なし」
pub const NONE: u32 = u32::MAX;

/// セクションの値の型
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    /// `u8`
    U8,
    /// `u32`
    U32,
    /// `f32`
    F32,
    /// `f64`
    F64,
    /// `u16`
    U16,
}

impl Type {
    /// 値のバイト数
    pub const fn size(self) -> usize {
        match self {
            Self::U8 => 1,
            Self::U32 => 4,
            Self::F32 => 4,
            Self::F64 => 8,
            Self::U16 => 2,
        }
    }
}

/// セクションの定義
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Section {
    /// 名前
    pub name: &'static str,
    /// 値の型
    pub ty: Type,
    /// 行ごとの値の数
    pub width: usize,
}

/// 塊の原点。プロジェクト空間の文書単位の X・Y・Z
pub const CHUNK_ORIGINS: usize = 0;
/// 塊の原点からの範囲。X・Y・Z の最小、X・Y・Z の最大
pub const CHUNK_BOUNDS: usize = 1;
/// 塊のインスタンスの始まり。1 要素を複数の塊に分けない
pub const CHUNK_INSTANCES: usize = 2;
/// メッシュの頂点の始まり。vertex_xyz の行
pub const MESH_VERTICES: usize = 3;
/// メッシュの三角形の始まり。triangle_vertices の行
pub const MESH_TRIANGLES: usize = 4;
/// メッシュの稜線の始まり。edge_vertices の行
pub const MESH_EDGES: usize = 5;
/// メッシュの局所座標。文書単位の X・Y・Z
pub const VERTEX_XYZ: usize = 6;
/// 三角形の頂点。所属するメッシュの中の頂点番号
pub const TRIANGLE_VERTICES: usize = 7;
/// 稜線の頂点。所属するメッシュの中の頂点番号。三角形の分け目は含めない
pub const EDGE_VERTICES: usize = 8;
/// 配置するメッシュ。mesh_vertices の行
pub const INSTANCE_MESHES: usize = 9;
/// 配置の元の要素。element_ids の行
pub const INSTANCE_ELEMENTS: usize = 10;
/// 配置の色。color_rgba の行
pub const INSTANCE_COLORS: usize = 11;
/// メッシュから塊の原点に対する座標への変換。行優先の 3 行 4 列
pub const INSTANCE_TRANSFORMS: usize = 12;
/// 建物モデルの要素の ID。strings の始まりと長さ
pub const ELEMENT_IDS: usize = 13;
/// 要素のクラス。strings の始まりと長さ
pub const ELEMENT_CLASSES: usize = 14;
/// 要素の名前。strings の始まりと長さ。名前がなければ長さ 0
pub const ELEMENT_NAMES: usize = 15;
/// 所属する階。element_ids の行。なければ番号の「なし」
pub const ELEMENT_STOREYS: usize = 16;
/// 色の赤・緑・青・不透明度。0〜1。未解決の RGB は 0
pub const COLOR_RGBA: usize = 17;
/// 色のフラグ。color_flag の値の和
pub const COLOR_FLAGS: usize = 18;
/// ID・クラス・名前の UTF-8 のバイト列
pub const STRINGS: usize = 19;
/// 塊の点の始まり。point_xyz の行。点を含むシーンで塊ごとに持つ
pub const CHUNK_POINTS: usize = 20;
/// 元の八分木のノード。段・北の位置・東の位置・高さの位置。段は0〜20。点のない塊は4値ともなし
pub const CHUNK_NODES: usize = 21;
/// 塊の原点からの点の東・北・高さ。メートル
pub const POINT_XYZ: usize = 22;
/// 点の赤・緑・青・不透明度。0〜65535。元の16ビットの色を保つ。属性がなければ空の列
pub const POINT_RGBA: usize = 23;
/// 点の元の強度。属性がなければ空の列
pub const POINT_INTENSITY: usize = 24;
/// 点の元の分類。属性がなければ空の列
pub const POINT_CLASSIFICATION: usize = 25;

/// セクションの定義。番号の順
pub const SECTIONS: [Section; 26] = [
    Section {
        name: "chunk_origins",
        ty: Type::F64,
        width: 3,
    },
    Section {
        name: "chunk_bounds",
        ty: Type::F32,
        width: 6,
    },
    Section {
        name: "chunk_instances",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "mesh_vertices",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "mesh_triangles",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "mesh_edges",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "vertex_xyz",
        ty: Type::F32,
        width: 3,
    },
    Section {
        name: "triangle_vertices",
        ty: Type::U32,
        width: 3,
    },
    Section {
        name: "edge_vertices",
        ty: Type::U32,
        width: 2,
    },
    Section {
        name: "instance_meshes",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "instance_elements",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "instance_colors",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "instance_transforms",
        ty: Type::F32,
        width: 12,
    },
    Section {
        name: "element_ids",
        ty: Type::U32,
        width: 2,
    },
    Section {
        name: "element_classes",
        ty: Type::U32,
        width: 2,
    },
    Section {
        name: "element_names",
        ty: Type::U32,
        width: 2,
    },
    Section {
        name: "element_storeys",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "color_rgba",
        ty: Type::F32,
        width: 4,
    },
    Section {
        name: "color_flags",
        ty: Type::U8,
        width: 1,
    },
    Section {
        name: "strings",
        ty: Type::U8,
        width: 1,
    },
    Section {
        name: "chunk_points",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "chunk_nodes",
        ty: Type::U32,
        width: 4,
    },
    Section {
        name: "point_xyz",
        ty: Type::F32,
        width: 3,
    },
    Section {
        name: "point_rgba",
        ty: Type::U16,
        width: 4,
    },
    Section {
        name: "point_intensity",
        ty: Type::U16,
        width: 1,
    },
    Section {
        name: "point_classification",
        ty: Type::U8,
        width: 1,
    },
];

/// 色の状態
pub mod color_flag {
    /// 色が未解決。読み手が代替色を決める
    pub const UNRESOLVED: u8 = 1;
}
