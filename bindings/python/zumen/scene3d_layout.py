# 生成したファイル。元は schema/scene3d.toml。cargo xtask codegen で作り直す。

from types import MappingProxyType
from .constants import SceneSection

SCENE3D_MAGIC = b"ZMS3"
SCENE3D_MAJOR = 1
SCENE3D_MINOR = 1
SCENE3D_NONE = 4294967295

SCENE3D_SECTIONS = (
    # 塊の原点。プロジェクト空間の文書単位の X・Y・Z
    SceneSection("chunk_origins", "f64", "d", 3),
    # 塊の原点からの範囲。X・Y・Z の最小、X・Y・Z の最大
    SceneSection("chunk_bounds", "f32", "f", 6),
    # 塊のインスタンスの始まり。1 要素を複数の塊に分けない
    SceneSection("chunk_instances", "u32", "I", 1),
    # メッシュの頂点の始まり。vertex_xyz の行
    SceneSection("mesh_vertices", "u32", "I", 1),
    # メッシュの三角形の始まり。triangle_vertices の行
    SceneSection("mesh_triangles", "u32", "I", 1),
    # メッシュの稜線の始まり。edge_vertices の行
    SceneSection("mesh_edges", "u32", "I", 1),
    # メッシュの局所座標。文書単位の X・Y・Z
    SceneSection("vertex_xyz", "f32", "f", 3),
    # 三角形の頂点。所属するメッシュの中の頂点番号
    SceneSection("triangle_vertices", "u32", "I", 3),
    # 稜線の頂点。所属するメッシュの中の頂点番号。三角形の分け目は含めない
    SceneSection("edge_vertices", "u32", "I", 2),
    # 配置するメッシュ。mesh_vertices の行
    SceneSection("instance_meshes", "u32", "I", 1),
    # 配置の元の要素。element_ids の行
    SceneSection("instance_elements", "u32", "I", 1),
    # 配置の色。color_rgba の行
    SceneSection("instance_colors", "u32", "I", 1),
    # メッシュから塊の原点に対する座標への変換。行優先の 3 行 4 列
    SceneSection("instance_transforms", "f32", "f", 12),
    # 建物モデルの要素の ID。strings の始まりと長さ
    SceneSection("element_ids", "u32", "I", 2),
    # 要素のクラス。strings の始まりと長さ
    SceneSection("element_classes", "u32", "I", 2),
    # 要素の名前。strings の始まりと長さ。名前がなければ長さ 0
    SceneSection("element_names", "u32", "I", 2),
    # 所属する階。element_ids の行。なければ番号の「なし」
    SceneSection("element_storeys", "u32", "I", 1),
    # 色の赤・緑・青・不透明度。0〜1。未解決の RGB は 0
    SceneSection("color_rgba", "f32", "f", 4),
    # 色のフラグ。color_flag の値の和
    SceneSection("color_flags", "u8", "B", 1),
    # ID・クラス・名前の UTF-8 のバイト列
    SceneSection("strings", "u8", "B", 1),
    # 塊の点の始まり。point_xyz の行。点を含むシーンで塊ごとに持つ
    SceneSection("chunk_points", "u32", "I", 1),
    # 元の八分木のノード。段・北の位置・東の位置・高さの位置。段は0〜20。点のない塊は4値ともなし
    SceneSection("chunk_nodes", "u32", "I", 4),
    # 塊の原点からの点の東・北・高さ。メートル
    SceneSection("point_xyz", "f32", "f", 3),
    # 点の赤・緑・青・不透明度。0〜65535。元の16ビットの色を保つ。属性がなければ空の列
    SceneSection("point_rgba", "u16", "H", 4),
    # 点の元の強度。属性がなければ空の列
    SceneSection("point_intensity", "u16", "H", 1),
    # 点の元の分類。属性がなければ空の列
    SceneSection("point_classification", "u8", "B", 1),
)

SCENE3D_VALUES = MappingProxyType({
    "color_flag": MappingProxyType({
        "unresolved": 1,
    }),
})
