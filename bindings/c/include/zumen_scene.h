/* 生成したファイル。手で直さない。
 * 元は schema/scene.toml で、`cargo xtask codegen` で作り直す。 */

/*
 * 2D のシーンの配置（決定 0137・0172・0178）。scene の op が返す blob の区画の番号・型・幅と、列に入れる値。
 *
 * - 見出しは ZMSC、大版（u16）、小版（u16）、塊の数（u32）、区画の数（u32）。続けて区画ごとに、始まりと長さ
 *   （u32、バイト）を置く。数はリトルエンディアン
 * - 区画と値は足すだけにする。読み手は、知らない区画を読み飛ばし、大版が違えば読まない（決定 0015）
 * - C++ の読み手は zumen.hpp の zumen::Scene にある
 */

#ifndef ZUMEN_SCENE_H
#define ZUMEN_SCENE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* 見出しの始まりの 4 バイト */
#define ZM_SCENE_MAGIC "ZMSC"
/* 大版。区画の意味を変えるときに上げる */
#define ZM_SCENE_MAJOR 1
/* 小版。区画か値を足すたびに上げる */
#define ZM_SCENE_MINOR 2
/* 番号の「なし」 */
#define ZM_SCENE_NONE UINT32_C(4294967295)

/* 区画の値の型 */
#define ZM_SCENE_TYPE_U8 0
#define ZM_SCENE_TYPE_U32 1
#define ZM_SCENE_TYPE_F32 2
#define ZM_SCENE_TYPE_F64 3

/* 区画の数 */
#define ZM_SCENE_SECTION_COUNT 69

/* 区画の番号 */
/* 塊の原点（起点の座標の X・Y）。塊の座標は、ここからの相対の値（f64 × 2） */
#define ZM_SCENE_SECTION_CHUNK_ORIGINS 0
/* 塊の描くものの相対座標の範囲（X の最小・Y の最小・X の最大・Y の最大）。文字は基点だけを含む（f32 × 4） */
#define ZM_SCENE_SECTION_CHUNK_BOUNDS 1
/* 塊の線の始まり（line_vertices の行）（u32 × 1） */
#define ZM_SCENE_SECTION_CHUNK_LINES 2
/* 塊の面の始まり（face_rings の行）（u32 × 1） */
#define ZM_SCENE_SECTION_CHUNK_FACES 3
/* 塊の文字の始まり（text_strings の行）（u32 × 1） */
#define ZM_SCENE_SECTION_CHUNK_TEXTS 4
/* 塊の点の始まり（point_xy の行）（u32 × 1） */
#define ZM_SCENE_SECTION_CHUNK_POINTS 5
/* 塊の画像の始まり（image_names の行）（u32 × 1） */
#define ZM_SCENE_SECTION_CHUNK_IMAGES 6
/* 線（折れ線）の頂点の始まり（line_xy の行）（u32 × 1） */
#define ZM_SCENE_SECTION_LINE_VERTICES 7
/* 線の描く単位（unit_entities の行）（u32 × 1） */
#define ZM_SCENE_SECTION_LINE_UNITS 8
/* 線の頂点。塊の原点からの X・Y（f32 × 2） */
#define ZM_SCENE_SECTION_LINE_XY 9
/* 面の輪の始まり（ring_vertices の行）。輪は偶奇の規則で塗る（u32 × 1） */
#define ZM_SCENE_SECTION_FACE_RINGS 10
/* 面の描く単位（unit_entities の行）（u32 × 1） */
#define ZM_SCENE_SECTION_FACE_UNITS 11
/* 輪の頂点の始まり（face_xy の行）。輪は閉じているものとし、始点を繰り返さない（u32 × 1） */
#define ZM_SCENE_SECTION_RING_VERTICES 12
/* 面の輪の頂点。塊の原点からの X・Y（f32 × 2） */
#define ZM_SCENE_SECTION_FACE_XY 13
/* 文字の内容（strings の始まりと長さ）（u32 × 2） */
#define ZM_SCENE_SECTION_TEXT_STRINGS 14
/* 文字の基点。塊の原点からの X・Y（f32 × 2） */
#define ZM_SCENE_SECTION_TEXT_XY 15
/* 2 点の間に置く文字の 2 点目。塊の原点からの X・Y。2 点の間に置かなければ NaN（f32 × 2） */
#define ZM_SCENE_SECTION_TEXT_SECOND 16
/* 文字の高さ（起点の長さ）（f32 × 1） */
#define ZM_SCENE_SECTION_TEXT_HEIGHTS 17
/* 字形の幅の倍率（f32 × 1） */
#define ZM_SCENE_SECTION_TEXT_WIDTHS 18
/* 回転（度、反時計回り）（f32 × 1） */
#define ZM_SCENE_SECTION_TEXT_ROTATIONS 19
/* 斜体の角度（度）（f32 × 1） */
#define ZM_SCENE_SECTION_TEXT_OBLIQUES 20
/* 折り返す枠の幅（起点の長さ）。折り返さなければ 0（f32 × 1） */
#define ZM_SCENE_SECTION_TEXT_BOXES 21
/* 位置合わせ。水平（horizontal の値）と垂直（vertical の値）（u8 × 2） */
#define ZM_SCENE_SECTION_TEXT_ALIGNS 22
/* 印（text_flag の値の和）（u8 × 1） */
#define ZM_SCENE_SECTION_TEXT_FLAGS 23
/* 書式つきの文字の行の番号。0 から数える。1 行の文字は 0（u32 × 1） */
#define ZM_SCENE_SECTION_TEXT_LINES 24
/* フォント（font_names の行）。分からなければ「なし」（u32 × 1） */
#define ZM_SCENE_SECTION_TEXT_FONTS 25
/* 文字の描く単位（unit_entities の行）（u32 × 1） */
#define ZM_SCENE_SECTION_TEXT_UNITS 26
/* 点の位置。塊の原点からの X・Y（f32 × 2） */
#define ZM_SCENE_SECTION_POINT_XY 27
/* 点の回転（度、反時計回り）（f32 × 1） */
#define ZM_SCENE_SECTION_POINT_ROTATIONS 28
/* 既定義の印のキー。名前空間（strings の始まりと長さ）と、元のコード（strings の始まりと長さ）。文書の点の表し方に従う点は、長さが 0（u32 × 4） */
#define ZM_SCENE_SECTION_POINT_MARKERS 29
/* 既定義の印の尺度。文書の点の表し方に従う点は 1（f32 × 1） */
#define ZM_SCENE_SECTION_POINT_SCALES 30
/* 点の描く単位（unit_entities の行）（u32 × 1） */
#define ZM_SCENE_SECTION_POINT_UNITS 31
/* 文書の点の表し方（決定 0160）。形（point_mark の値）・囲み（point_frame の値の和）・大きさの意味（point_size の値）・大きさ。文書が持たなければ行がない（f32 × 4） */
#define ZM_SCENE_SECTION_POINT_STYLE 32
/* 下敷きの参照名（strings の始まりと長さ）。コアは開かない（u32 × 2） */
#define ZM_SCENE_SECTION_IMAGE_NAMES 33
/* 資源の中のページ（strings の始まりと長さ）。なければ長さが 0（u32 × 2） */
#define ZM_SCENE_SECTION_IMAGE_PAGES 34
/* 画像の基点（左下の角）。塊の原点からの X・Y（f32 × 2） */
#define ZM_SCENE_SECTION_IMAGE_XY 35
/* 画像の X の辺と Y の辺のベクトル（起点の長さ）。image_frames が corners なら内容の全体の辺、unit なら内容の 1 単位の辺（f32 × 4） */
#define ZM_SCENE_SECTION_IMAGE_EDGES 36
/* 資源の種類（image_kind の値）（u8 × 1） */
#define ZM_SCENE_SECTION_IMAGE_KINDS 37
/* 辺のベクトルの意味（image_frame の値）（u8 × 1） */
#define ZM_SCENE_SECTION_IMAGE_FRAMES 38
/* 画像の描く単位（unit_entities の行）（u32 × 1） */
#define ZM_SCENE_SECTION_IMAGE_UNITS 39
/* 描く単位の要素（entity_ids の行）（u32 × 1） */
#define ZM_SCENE_SECTION_UNIT_ENTITIES 40
/* 描く単位の色（color_systems の行）（u32 × 1） */
#define ZM_SCENE_SECTION_UNIT_COLORS 41
/* 線幅（用紙の mm）。0 は、いちばん細い線（f32 × 1） */
#define ZM_SCENE_SECTION_UNIT_WEIGHTS 42
/* 切り抜き（clip_polygons の行）。切り抜かなければ「なし」（u32 × 1） */
#define ZM_SCENE_SECTION_UNIT_CLIPS 43
/* 実効のレイヤ（layer_ids の行）（u32 × 1） */
#define ZM_SCENE_SECTION_UNIT_LAYERS 44
/* 要素の ID（strings の始まりと長さ、決定 0156）。記号の属性なら、属性を持つ記号の ID（u32 × 2） */
#define ZM_SCENE_SECTION_ENTITY_IDS 45
/* 要素の種類（entity_kind の値）（u8 × 1） */
#define ZM_SCENE_SECTION_ENTITY_KINDS 46
/* 配置の道筋の始まり（step_ids の行）。外側から内側へ（u32 × 1） */
#define ZM_SCENE_SECTION_ENTITY_PATHS 47
/* 配置の要素の ID（strings の始まりと長さ）（u32 × 2） */
#define ZM_SCENE_SECTION_STEP_IDS 48
/* 格子に並べた記号の列と行。格子でなければ 0 と 0（u32 × 2） */
#define ZM_SCENE_SECTION_STEP_CELLS 49
/* 色の符号の系統（color_system の値）（u8 × 1） */
#define ZM_SCENE_SECTION_COLOR_SYSTEMS 50
/* 系統の中の符号。系統がなければ 0（u32 × 1） */
#define ZM_SCENE_SECTION_COLOR_CODES 51
/* ファイルが持つ RGB と、持つかどうか（持てば 255、持たなければ 0）（u8 × 4） */
#define ZM_SCENE_SECTION_COLOR_FILES 52
/* 表示する RGBA。ファイルの RGB か、色の表（palette）から引いた値。未解決なら 0（u8 × 4） */
#define ZM_SCENE_SECTION_COLOR_RGBA 53
/* 印（color_flag の値の和）（u8 × 1） */
#define ZM_SCENE_SECTION_COLOR_FLAGS 54
/* 切り抜きの組の多角形の始まり（polygon_vertices の行）。描くものは、組のすべての多角形の内側だけを描く（u32 × 1） */
#define ZM_SCENE_SECTION_CLIP_POLYGONS 55
/* 多角形の頂点の始まり（clip_xy の行）（u32 × 1） */
#define ZM_SCENE_SECTION_POLYGON_VERTICES 56
/* 多角形の印。1 なら、内側を隠す（外側だけを描く）（u8 × 1） */
#define ZM_SCENE_SECTION_POLYGON_FLAGS 57
/* 切り抜きの多角形の頂点。起点の座標の X・Y（f64 × 2） */
#define ZM_SCENE_SECTION_CLIP_XY 58
/* レイヤの ID（strings の始まりと長さ）。tables の op が返すものと同じ形（u32 × 2） */
#define ZM_SCENE_SECTION_LAYER_IDS 59
/* フォントの元の名前（strings の始まりと長さ）（u32 × 2） */
#define ZM_SCENE_SECTION_FONT_NAMES 60
/* 文字列を並べた UTF-8（u8 × 1） */
#define ZM_SCENE_SECTION_STRINGS 61
/* 文字の字形の始まり（glyph_shapes の行）。字形を持つ文字は、文字列の代わりに字形を描く。持たなければ（フォントがない・置けない）文字列を描く（u32 × 1） */
#define ZM_SCENE_SECTION_TEXT_GLYPHS 62
/* 置いた字形の形（shape_rings の行）（u32 × 1） */
#define ZM_SCENE_SECTION_GLYPH_SHAPES 63
/* 形の座標（字形の単位）を写す変換の線形の部分 [a, b, c, d]。形の点 (u, v) は、x = a·u + c·v + e、y = b·u + d·v + f に置く（e・f は glyph_xy）（f32 × 4） */
#define ZM_SCENE_SECTION_GLYPH_AXES 64
/* 字形の原点（形の座標の (0, 0)）を置く点 [e, f]。塊の原点からの X・Y（f32 × 2） */
#define ZM_SCENE_SECTION_GLYPH_XY 65
/* 字形の形の輪の始まり（outline_vertices の行）。輪は非零の規則で塗る。空白などは輪を持たない（u32 × 1） */
#define ZM_SCENE_SECTION_SHAPE_RINGS 66
/* 字形の輪の頂点の始まり（outline_xy の行）。輪は閉じているものとし、始点を繰り返さない（u32 × 1） */
#define ZM_SCENE_SECTION_OUTLINE_VERTICES 67
/* 字形の輪の頂点（字形の単位）。曲線は、許容差を字形の単位に直して 2 の冪に切り下げた刻みで折れ線にする（f32 × 2） */
#define ZM_SCENE_SECTION_OUTLINE_XY 68

/* 文字の水平の位置合わせ（u8） */
/* 左 */
#define ZM_SCENE_VALUE_HORIZONTAL_LEFT 0
/* 中央 */
#define ZM_SCENE_VALUE_HORIZONTAL_CENTER 1
/* 右 */
#define ZM_SCENE_VALUE_HORIZONTAL_RIGHT 2
/* 文字の外形の中央 */
#define ZM_SCENE_VALUE_HORIZONTAL_MIDDLE 3
/* 2 点の間に、縦横の比を保って合わせる */
#define ZM_SCENE_VALUE_HORIZONTAL_ALIGNED 4
/* 2 点の間に、高さを保って幅だけを合わせる */
#define ZM_SCENE_VALUE_HORIZONTAL_FIT 5

/* 文字の垂直の位置合わせ（u8） */
/* 基準線 */
#define ZM_SCENE_VALUE_VERTICAL_BASELINE 0
/* 下 */
#define ZM_SCENE_VALUE_VERTICAL_BOTTOM 1
/* 中央 */
#define ZM_SCENE_VALUE_VERTICAL_MIDDLE 2
/* 上 */
#define ZM_SCENE_VALUE_VERTICAL_TOP 3

/* 文字の印。和で持つ（u8） */
/* 左右に反転する */
#define ZM_SCENE_VALUE_TEXT_FLAG_BACKWARD 1
/* 上下に反転する */
#define ZM_SCENE_VALUE_TEXT_FLAG_UPSIDE_DOWN 2
/* 縦書き */
#define ZM_SCENE_VALUE_TEXT_FLAG_VERTICAL 4
/* 書式つきの文字の区間。基点は枠の基点で、行の中の位置は表示する側が送る */
#define ZM_SCENE_VALUE_TEXT_FLAG_FORMATTED 8
/* 上下に重ねた文字の上 */
#define ZM_SCENE_VALUE_TEXT_FLAG_STACK_UPPER 16
/* 上下に重ねた文字の下 */
#define ZM_SCENE_VALUE_TEXT_FLAG_STACK_LOWER 32
/* 高さは文字枠の高さ（決定 0184）。height_frame と height_em のどちらもなければ、大文字の高さか、字形との関係が決まらない形式の宣言値 */
#define ZM_SCENE_VALUE_TEXT_FLAG_HEIGHT_FRAME 64
/* 高さは em（決定 0184） */
#define ZM_SCENE_VALUE_TEXT_FLAG_HEIGHT_EM 128

/* 文書の点の基本の形（u8） */
/* 小さな点 */
#define ZM_SCENE_VALUE_POINT_MARK_DOT 0
/* なし */
#define ZM_SCENE_VALUE_POINT_MARK_NONE 1
/* 十字 */
#define ZM_SCENE_VALUE_POINT_MARK_PLUS 2
/* バツ */
#define ZM_SCENE_VALUE_POINT_MARK_CROSS 3
/* 上向きの短い線 */
#define ZM_SCENE_VALUE_POINT_MARK_TICK 4

/* 文書の点の囲み。和で持つ（u8） */
/* 円で囲む */
#define ZM_SCENE_VALUE_POINT_FRAME_CIRCLE 1
/* 四角で囲む */
#define ZM_SCENE_VALUE_POINT_FRAME_SQUARE 2

/* 文書の点の大きさの意味（u8） */
/* 起点の長さ */
#define ZM_SCENE_VALUE_POINT_SIZE_LENGTH 0
/* 画面の高さに対する百分率 */
#define ZM_SCENE_VALUE_POINT_SIZE_SCREEN 1

/* 下敷きの資源の種類（u8） */
/* 画素の画像 */
#define ZM_SCENE_VALUE_IMAGE_KIND_IMAGE 0
/* PDF */
#define ZM_SCENE_VALUE_IMAGE_KIND_PDF 1
/* DWF（決定 0184） */
#define ZM_SCENE_VALUE_IMAGE_KIND_DWF 2
/* DGN（決定 0184） */
#define ZM_SCENE_VALUE_IMAGE_KIND_DGN 3

/* 画像の辺のベクトルの意味（u8） */
/* 内容の全体を、2 つの辺の平行四辺形に収める */
#define ZM_SCENE_VALUE_IMAGE_FRAME_CORNERS 0
/* 辺は内容の 1 単位。内容の大きさを掛ける */
#define ZM_SCENE_VALUE_IMAGE_FRAME_UNIT 1

/* 要素の種類（entities の op の kind と同じ）（u8） */
/* 線分 */
#define ZM_SCENE_VALUE_ENTITY_KIND_LINE 0
/* 円 */
#define ZM_SCENE_VALUE_ENTITY_KIND_CIRCLE 1
/* 曲線 */
#define ZM_SCENE_VALUE_ENTITY_KIND_CURVE 2
/* 折れ線 */
#define ZM_SCENE_VALUE_ENTITY_KIND_POLYLINE 3
/* 文字 */
#define ZM_SCENE_VALUE_ENTITY_KIND_TEXT 4
/* 書式つきの文字 */
#define ZM_SCENE_VALUE_ENTITY_KIND_FORMATTED 5
/* 寸法 */
#define ZM_SCENE_VALUE_ENTITY_KIND_DIMENSION 6
/* 引出線 */
#define ZM_SCENE_VALUE_ENTITY_KIND_LEADER 7
/* 引出線つきの注記 */
#define ZM_SCENE_VALUE_ENTITY_KIND_CALLOUT 8
/* 表 */
#define ZM_SCENE_VALUE_ENTITY_KIND_TABLE 9
/* 下敷き */
#define ZM_SCENE_VALUE_ENTITY_KIND_UNDERLAY 10
/* 記号 */
#define ZM_SCENE_VALUE_ENTITY_KIND_SYMBOL 11
/* ハッチング */
#define ZM_SCENE_VALUE_ENTITY_KIND_HATCH 12
/* 点 */
#define ZM_SCENE_VALUE_ENTITY_KIND_POINT 13
/* 記号の属性 */
#define ZM_SCENE_VALUE_ENTITY_KIND_ATTRIBUTE 14

/* 色の符号の系統（u8） */
/* 符号を持たない */
#define ZM_SCENE_VALUE_COLOR_SYSTEM_NONE 0
/* AutoCAD の色の番号 */
#define ZM_SCENE_VALUE_COLOR_SYSTEM_ACI 1
/* SXF の色のコード */
#define ZM_SCENE_VALUE_COLOR_SYSTEM_SXF 2
/* Jw_cad のペンの番号 */
#define ZM_SCENE_VALUE_COLOR_SYSTEM_JWW 3

/* 色の印。和で持つ（u8） */
/* 表示する RGBA が決まらない。表示する側が決める */
#define ZM_SCENE_VALUE_COLOR_FLAG_UNRESOLVED 1
/* 背景で変わる色（ACI の 7）。表示する側が、背景に対する前景の色にする */
#define ZM_SCENE_VALUE_COLOR_FLAG_FOREGROUND 2

/* 区画の定義 */
typedef struct zm_scene_section {
    /* 名前（schema/scene.toml の name） */
    const char *name;
    /* 値の型（ZM_SCENE_TYPE_*） */
    int32_t type;
    /* 行ごとの値の数 */
    uint32_t width;
} zm_scene_section;

/* 区画の定義を番号から引く。知らない番号なら NULL */
static inline const zm_scene_section *zm_scene_section_at(uint32_t index) {
    static const zm_scene_section sections[69] = {
        {"chunk_origins", ZM_SCENE_TYPE_F64, 2},
        {"chunk_bounds", ZM_SCENE_TYPE_F32, 4},
        {"chunk_lines", ZM_SCENE_TYPE_U32, 1},
        {"chunk_faces", ZM_SCENE_TYPE_U32, 1},
        {"chunk_texts", ZM_SCENE_TYPE_U32, 1},
        {"chunk_points", ZM_SCENE_TYPE_U32, 1},
        {"chunk_images", ZM_SCENE_TYPE_U32, 1},
        {"line_vertices", ZM_SCENE_TYPE_U32, 1},
        {"line_units", ZM_SCENE_TYPE_U32, 1},
        {"line_xy", ZM_SCENE_TYPE_F32, 2},
        {"face_rings", ZM_SCENE_TYPE_U32, 1},
        {"face_units", ZM_SCENE_TYPE_U32, 1},
        {"ring_vertices", ZM_SCENE_TYPE_U32, 1},
        {"face_xy", ZM_SCENE_TYPE_F32, 2},
        {"text_strings", ZM_SCENE_TYPE_U32, 2},
        {"text_xy", ZM_SCENE_TYPE_F32, 2},
        {"text_second", ZM_SCENE_TYPE_F32, 2},
        {"text_heights", ZM_SCENE_TYPE_F32, 1},
        {"text_widths", ZM_SCENE_TYPE_F32, 1},
        {"text_rotations", ZM_SCENE_TYPE_F32, 1},
        {"text_obliques", ZM_SCENE_TYPE_F32, 1},
        {"text_boxes", ZM_SCENE_TYPE_F32, 1},
        {"text_aligns", ZM_SCENE_TYPE_U8, 2},
        {"text_flags", ZM_SCENE_TYPE_U8, 1},
        {"text_lines", ZM_SCENE_TYPE_U32, 1},
        {"text_fonts", ZM_SCENE_TYPE_U32, 1},
        {"text_units", ZM_SCENE_TYPE_U32, 1},
        {"point_xy", ZM_SCENE_TYPE_F32, 2},
        {"point_rotations", ZM_SCENE_TYPE_F32, 1},
        {"point_markers", ZM_SCENE_TYPE_U32, 4},
        {"point_scales", ZM_SCENE_TYPE_F32, 1},
        {"point_units", ZM_SCENE_TYPE_U32, 1},
        {"point_style", ZM_SCENE_TYPE_F32, 4},
        {"image_names", ZM_SCENE_TYPE_U32, 2},
        {"image_pages", ZM_SCENE_TYPE_U32, 2},
        {"image_xy", ZM_SCENE_TYPE_F32, 2},
        {"image_edges", ZM_SCENE_TYPE_F32, 4},
        {"image_kinds", ZM_SCENE_TYPE_U8, 1},
        {"image_frames", ZM_SCENE_TYPE_U8, 1},
        {"image_units", ZM_SCENE_TYPE_U32, 1},
        {"unit_entities", ZM_SCENE_TYPE_U32, 1},
        {"unit_colors", ZM_SCENE_TYPE_U32, 1},
        {"unit_weights", ZM_SCENE_TYPE_F32, 1},
        {"unit_clips", ZM_SCENE_TYPE_U32, 1},
        {"unit_layers", ZM_SCENE_TYPE_U32, 1},
        {"entity_ids", ZM_SCENE_TYPE_U32, 2},
        {"entity_kinds", ZM_SCENE_TYPE_U8, 1},
        {"entity_paths", ZM_SCENE_TYPE_U32, 1},
        {"step_ids", ZM_SCENE_TYPE_U32, 2},
        {"step_cells", ZM_SCENE_TYPE_U32, 2},
        {"color_systems", ZM_SCENE_TYPE_U8, 1},
        {"color_codes", ZM_SCENE_TYPE_U32, 1},
        {"color_files", ZM_SCENE_TYPE_U8, 4},
        {"color_rgba", ZM_SCENE_TYPE_U8, 4},
        {"color_flags", ZM_SCENE_TYPE_U8, 1},
        {"clip_polygons", ZM_SCENE_TYPE_U32, 1},
        {"polygon_vertices", ZM_SCENE_TYPE_U32, 1},
        {"polygon_flags", ZM_SCENE_TYPE_U8, 1},
        {"clip_xy", ZM_SCENE_TYPE_F64, 2},
        {"layer_ids", ZM_SCENE_TYPE_U32, 2},
        {"font_names", ZM_SCENE_TYPE_U32, 2},
        {"strings", ZM_SCENE_TYPE_U8, 1},
        {"text_glyphs", ZM_SCENE_TYPE_U32, 1},
        {"glyph_shapes", ZM_SCENE_TYPE_U32, 1},
        {"glyph_axes", ZM_SCENE_TYPE_F32, 4},
        {"glyph_xy", ZM_SCENE_TYPE_F32, 2},
        {"shape_rings", ZM_SCENE_TYPE_U32, 1},
        {"outline_vertices", ZM_SCENE_TYPE_U32, 1},
        {"outline_xy", ZM_SCENE_TYPE_F32, 2},
    };
    return index < ZM_SCENE_SECTION_COUNT ? &sections[index] : NULL;
}

/* 値の型のバイト数。知らない型なら 0 */
static inline uint32_t zm_scene_type_size(int32_t type) {
    switch (type) {
    case 0:
        return 1;
    case 1:
        return 4;
    case 2:
        return 4;
    case 3:
        return 8;
    default:
        return 0;
    }
}

#ifdef __cplusplus
}
#endif

#endif /* ZUMEN_SCENE_H */
