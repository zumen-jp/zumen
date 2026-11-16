/* 生成したファイル。手で直さない。
 * 元は schema/scene3d.toml で、`cargo xtask codegen` で作り直す。 */

/*
 * 3D のシーンの配置（#417・#421）。scene3d の op が返す blob のセクションの番号・型・幅と、列に入れる値。
 *
 * - ヘッダーは ZMS3、メジャーバージョン（u16）、マイナーバージョン（u16）、塊の数（u32）、セクションの数（u32）。続けてセクションごとに、始まりと長さ
 *   （u32、バイト）を置く。数はリトルエンディアン
 * - セクションと値は足すだけにする。リーダーは、知らないセクションを読み飛ばし、メジャーバージョンが違えば読まない（決定 0015）
 * - C++ のリーダーは zumen.hpp の zumen::Scene3D にある
 */

#ifndef ZUMEN_SCENE3D_H
#define ZUMEN_SCENE3D_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ヘッダーの始まりの 4 バイト */
#define ZM_SCENE3D_MAGIC "ZMS3"
/* メジャーバージョン。セクションの意味を変えるときに上げる */
#define ZM_SCENE3D_MAJOR 1
/* マイナーバージョン。セクションか値を足すたびに上げる */
#define ZM_SCENE3D_MINOR 1
/* 番号の「なし」 */
#define ZM_SCENE3D_NONE UINT32_C(4294967295)

/* セクションの値の型 */
#define ZM_SCENE3D_TYPE_U8 0
#define ZM_SCENE3D_TYPE_U32 1
#define ZM_SCENE3D_TYPE_F32 2
#define ZM_SCENE3D_TYPE_F64 3
#define ZM_SCENE3D_TYPE_U16 4

/* セクションの数 */
#define ZM_SCENE3D_SECTION_COUNT 26

/* セクションの番号 */
/* 塊の原点。プロジェクト空間の文書単位の X・Y・Z（f64 × 3） */
#define ZM_SCENE3D_SECTION_CHUNK_ORIGINS 0
/* 塊の原点からの範囲。X・Y・Z の最小、X・Y・Z の最大（f32 × 6） */
#define ZM_SCENE3D_SECTION_CHUNK_BOUNDS 1
/* 塊のインスタンスの始まり。1 要素を複数の塊に分けない（u32 × 1） */
#define ZM_SCENE3D_SECTION_CHUNK_INSTANCES 2
/* メッシュの頂点の始まり。vertex_xyz の行（u32 × 1） */
#define ZM_SCENE3D_SECTION_MESH_VERTICES 3
/* メッシュの三角形の始まり。triangle_vertices の行（u32 × 1） */
#define ZM_SCENE3D_SECTION_MESH_TRIANGLES 4
/* メッシュの稜線の始まり。edge_vertices の行（u32 × 1） */
#define ZM_SCENE3D_SECTION_MESH_EDGES 5
/* メッシュの局所座標。文書単位の X・Y・Z（f32 × 3） */
#define ZM_SCENE3D_SECTION_VERTEX_XYZ 6
/* 三角形の頂点。所属するメッシュの中の頂点番号（u32 × 3） */
#define ZM_SCENE3D_SECTION_TRIANGLE_VERTICES 7
/* 稜線の頂点。所属するメッシュの中の頂点番号。三角形の分け目は含めない（u32 × 2） */
#define ZM_SCENE3D_SECTION_EDGE_VERTICES 8
/* 配置するメッシュ。mesh_vertices の行（u32 × 1） */
#define ZM_SCENE3D_SECTION_INSTANCE_MESHES 9
/* 配置の元の要素。element_ids の行（u32 × 1） */
#define ZM_SCENE3D_SECTION_INSTANCE_ELEMENTS 10
/* 配置の色。color_rgba の行（u32 × 1） */
#define ZM_SCENE3D_SECTION_INSTANCE_COLORS 11
/* メッシュから塊の原点に対する座標への変換。行優先の 3 行 4 列（f32 × 12） */
#define ZM_SCENE3D_SECTION_INSTANCE_TRANSFORMS 12
/* 建物モデルの要素の ID。strings の始まりと長さ（u32 × 2） */
#define ZM_SCENE3D_SECTION_ELEMENT_IDS 13
/* 要素のクラス。strings の始まりと長さ（u32 × 2） */
#define ZM_SCENE3D_SECTION_ELEMENT_CLASSES 14
/* 要素の名前。strings の始まりと長さ。名前がなければ長さ 0（u32 × 2） */
#define ZM_SCENE3D_SECTION_ELEMENT_NAMES 15
/* 所属する階。element_ids の行。なければ番号の「なし」（u32 × 1） */
#define ZM_SCENE3D_SECTION_ELEMENT_STOREYS 16
/* 色の赤・緑・青・不透明度。0〜1。未解決の RGB は 0（f32 × 4） */
#define ZM_SCENE3D_SECTION_COLOR_RGBA 17
/* 色のフラグ。color_flag の値の和（u8 × 1） */
#define ZM_SCENE3D_SECTION_COLOR_FLAGS 18
/* ID・クラス・名前の UTF-8 のバイト列（u8 × 1） */
#define ZM_SCENE3D_SECTION_STRINGS 19
/* 塊の点の始まり。point_xyz の行。点を含むシーンで塊ごとに持つ（u32 × 1） */
#define ZM_SCENE3D_SECTION_CHUNK_POINTS 20
/* 元の八分木のノード。段・北の位置・東の位置・高さの位置。段は0〜20。点のない塊は4値ともなし（u32 × 4） */
#define ZM_SCENE3D_SECTION_CHUNK_NODES 21
/* 塊の原点からの点の東・北・高さ。メートル（f32 × 3） */
#define ZM_SCENE3D_SECTION_POINT_XYZ 22
/* 点の赤・緑・青・不透明度。0〜65535。元の16ビットの色を保つ。属性がなければ空の列（u16 × 4） */
#define ZM_SCENE3D_SECTION_POINT_RGBA 23
/* 点の元の強度。属性がなければ空の列（u16 × 1） */
#define ZM_SCENE3D_SECTION_POINT_INTENSITY 24
/* 点の元の分類。属性がなければ空の列（u8 × 1） */
#define ZM_SCENE3D_SECTION_POINT_CLASSIFICATION 25

/* 色の状態（u8） */
/* 色が未解決。読み手が代替色を決める */
#define ZM_SCENE3D_VALUE_COLOR_FLAG_UNRESOLVED 1

/* セクションの定義 */
typedef struct zm_scene3d_section {
    /* 名前（schema/scene3d.toml の name） */
    const char *name;
    /* 値の型（ZM_SCENE3D_TYPE_*） */
    int32_t type;
    /* 行ごとの値の数 */
    uint32_t width;
} zm_scene3d_section;

/* セクションの定義を番号から引く。知らない番号なら NULL */
static inline const zm_scene3d_section *zm_scene3d_section_at(uint32_t index) {
    static const zm_scene3d_section sections[26] = {
        {"chunk_origins", ZM_SCENE3D_TYPE_F64, 3},
        {"chunk_bounds", ZM_SCENE3D_TYPE_F32, 6},
        {"chunk_instances", ZM_SCENE3D_TYPE_U32, 1},
        {"mesh_vertices", ZM_SCENE3D_TYPE_U32, 1},
        {"mesh_triangles", ZM_SCENE3D_TYPE_U32, 1},
        {"mesh_edges", ZM_SCENE3D_TYPE_U32, 1},
        {"vertex_xyz", ZM_SCENE3D_TYPE_F32, 3},
        {"triangle_vertices", ZM_SCENE3D_TYPE_U32, 3},
        {"edge_vertices", ZM_SCENE3D_TYPE_U32, 2},
        {"instance_meshes", ZM_SCENE3D_TYPE_U32, 1},
        {"instance_elements", ZM_SCENE3D_TYPE_U32, 1},
        {"instance_colors", ZM_SCENE3D_TYPE_U32, 1},
        {"instance_transforms", ZM_SCENE3D_TYPE_F32, 12},
        {"element_ids", ZM_SCENE3D_TYPE_U32, 2},
        {"element_classes", ZM_SCENE3D_TYPE_U32, 2},
        {"element_names", ZM_SCENE3D_TYPE_U32, 2},
        {"element_storeys", ZM_SCENE3D_TYPE_U32, 1},
        {"color_rgba", ZM_SCENE3D_TYPE_F32, 4},
        {"color_flags", ZM_SCENE3D_TYPE_U8, 1},
        {"strings", ZM_SCENE3D_TYPE_U8, 1},
        {"chunk_points", ZM_SCENE3D_TYPE_U32, 1},
        {"chunk_nodes", ZM_SCENE3D_TYPE_U32, 4},
        {"point_xyz", ZM_SCENE3D_TYPE_F32, 3},
        {"point_rgba", ZM_SCENE3D_TYPE_U16, 4},
        {"point_intensity", ZM_SCENE3D_TYPE_U16, 1},
        {"point_classification", ZM_SCENE3D_TYPE_U8, 1},
    };
    return index < ZM_SCENE3D_SECTION_COUNT ? &sections[index] : NULL;
}

/* 値の型のバイト数。知らない型なら 0 */
static inline uint32_t zm_scene3d_type_size(int32_t type) {
    switch (type) {
    case 0:
        return 1;
    case 1:
        return 4;
    case 2:
        return 4;
    case 3:
        return 8;
    case 4:
        return 2;
    default:
        return 0;
    }
}

#ifdef __cplusplus
}
#endif

#endif /* ZUMEN_SCENE3D_H */
