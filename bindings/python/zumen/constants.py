# 生成したファイル。手で直さない。
# 元は schema/boundary.toml と schema/scene.toml で、`cargo xtask codegen` で作り直す。

import ctypes
from types import MappingProxyType
from typing import Literal, Mapping, NamedTuple, Tuple

# パッケージの版（決定 0015）
VERSION = "0.2.0"
# ABI の大版
ABI_VERSION = 1
# メッセージのスキーマの大版（要求と応答の v）
SCHEMA_MAJOR = 1
# メッセージのスキーマの小版
SCHEMA_MINOR = 54

# 状態コードの名前
StatusName = Literal[
    # 成功した
    "ok",
    # 操作が失敗した。応答に error の診断がある
    "failed",
    # 引数が使えない（NULL、長さが合わない、使えない名前など）。応答は書かない
    "invalid",
    # 別のスレッドが、同じセッションを使っている
    "busy",
    # コアの不具合で止まった。セッションは閉じるしかない
    "panicked",
]

# 状態コードの名前。値から引く
STATUS: Mapping[int, str] = MappingProxyType(
    {
        0: "ok",
        1: "failed",
        2: "invalid",
        3: "busy",
        4: "panicked",
    }
)


class Buf(ctypes.Structure):
    """コアが返すバッファー（zm_buf）。zm_free で返す"""

    _fields_ = [("ptr", ctypes.c_void_p), ("len", ctypes.c_size_t)]


# ネイティブのライブラリが出す関数の、戻り値と引数の ctypes の型
FUNCTIONS: Mapping[str, Tuple[object, Tuple[object, ...]]] = MappingProxyType(
    {
        "zm_abi_version": (ctypes.c_uint32, ()),
        "zm_session_new": (ctypes.c_int32, (ctypes.c_char_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_void_p))),
        "zm_push": (ctypes.c_int32, (ctypes.c_void_p, ctypes.c_char_p, ctypes.c_size_t, ctypes.c_char_p, ctypes.c_size_t)),
        "zm_call": (ctypes.c_int32, (ctypes.c_void_p, ctypes.c_char_p, ctypes.c_size_t, ctypes.POINTER(Buf))),
        "zm_take": (ctypes.c_int32, (ctypes.c_void_p, ctypes.c_uint32, ctypes.POINTER(Buf))),
        "zm_take_arrow": (ctypes.c_int32, (ctypes.c_void_p, ctypes.c_uint32, ctypes.c_void_p)),
        "zm_cancel": (None, (ctypes.c_void_p,)),
        "zm_session_close": (None, (ctypes.c_void_p,)),
        "zm_free": (None, (ctypes.POINTER(Buf),)),
    }
)

# シーンの見出しの始まりの 4 バイト
SCENE_MAGIC = b"ZMSC"
# シーンの大版。区画の意味を変えるときに上げる
SCENE_MAJOR = 1
# シーンの小版。区画か値を足すたびに上げる
SCENE_MINOR = 2
# 番号の「なし」
SCENE_NONE = 4294967295

# シーンの区画の名前
SceneSectionName = Literal[
    "chunk_origins",
    "chunk_bounds",
    "chunk_lines",
    "chunk_faces",
    "chunk_texts",
    "chunk_points",
    "chunk_images",
    "line_vertices",
    "line_units",
    "line_xy",
    "face_rings",
    "face_units",
    "ring_vertices",
    "face_xy",
    "text_strings",
    "text_xy",
    "text_second",
    "text_heights",
    "text_widths",
    "text_rotations",
    "text_obliques",
    "text_boxes",
    "text_aligns",
    "text_flags",
    "text_lines",
    "text_fonts",
    "text_units",
    "point_xy",
    "point_rotations",
    "point_markers",
    "point_scales",
    "point_units",
    "point_style",
    "image_names",
    "image_pages",
    "image_xy",
    "image_edges",
    "image_kinds",
    "image_frames",
    "image_units",
    "unit_entities",
    "unit_colors",
    "unit_weights",
    "unit_clips",
    "unit_layers",
    "entity_ids",
    "entity_kinds",
    "entity_paths",
    "step_ids",
    "step_cells",
    "color_systems",
    "color_codes",
    "color_files",
    "color_rgba",
    "color_flags",
    "clip_polygons",
    "polygon_vertices",
    "polygon_flags",
    "clip_xy",
    "layer_ids",
    "font_names",
    "strings",
    "text_glyphs",
    "glyph_shapes",
    "glyph_axes",
    "glyph_xy",
    "shape_rings",
    "outline_vertices",
    "outline_xy",
]


class SceneSection(NamedTuple):
    """シーンの区画の定義"""

    # 名前
    name: str
    # 値の型（u8・u32・f32・f64）
    type: str
    # memoryview と array の形式の文字
    format: str
    # 行ごとの値の数
    width: int


# シーンの区画の定義。番号の順
SCENE_SECTIONS: Tuple[SceneSection, ...] = (
    # 塊の原点（起点の座標の X・Y）。塊の座標は、ここからの相対の値（f64 × 2）
    SceneSection("chunk_origins", "f64", "d", 2),
    # 塊の描くものの相対座標の範囲（X の最小・Y の最小・X の最大・Y の最大）。文字は基点だけを含む（f32 × 4）
    SceneSection("chunk_bounds", "f32", "f", 4),
    # 塊の線の始まり（line_vertices の行）（u32 × 1）
    SceneSection("chunk_lines", "u32", "I", 1),
    # 塊の面の始まり（face_rings の行）（u32 × 1）
    SceneSection("chunk_faces", "u32", "I", 1),
    # 塊の文字の始まり（text_strings の行）（u32 × 1）
    SceneSection("chunk_texts", "u32", "I", 1),
    # 塊の点の始まり（point_xy の行）（u32 × 1）
    SceneSection("chunk_points", "u32", "I", 1),
    # 塊の画像の始まり（image_names の行）（u32 × 1）
    SceneSection("chunk_images", "u32", "I", 1),
    # 線（折れ線）の頂点の始まり（line_xy の行）（u32 × 1）
    SceneSection("line_vertices", "u32", "I", 1),
    # 線の描く単位（unit_entities の行）（u32 × 1）
    SceneSection("line_units", "u32", "I", 1),
    # 線の頂点。塊の原点からの X・Y（f32 × 2）
    SceneSection("line_xy", "f32", "f", 2),
    # 面の輪の始まり（ring_vertices の行）。輪は偶奇の規則で塗る（u32 × 1）
    SceneSection("face_rings", "u32", "I", 1),
    # 面の描く単位（unit_entities の行）（u32 × 1）
    SceneSection("face_units", "u32", "I", 1),
    # 輪の頂点の始まり（face_xy の行）。輪は閉じているものとし、始点を繰り返さない（u32 × 1）
    SceneSection("ring_vertices", "u32", "I", 1),
    # 面の輪の頂点。塊の原点からの X・Y（f32 × 2）
    SceneSection("face_xy", "f32", "f", 2),
    # 文字の内容（strings の始まりと長さ）（u32 × 2）
    SceneSection("text_strings", "u32", "I", 2),
    # 文字の基点。塊の原点からの X・Y（f32 × 2）
    SceneSection("text_xy", "f32", "f", 2),
    # 2 点の間に置く文字の 2 点目。塊の原点からの X・Y。2 点の間に置かなければ NaN（f32 × 2）
    SceneSection("text_second", "f32", "f", 2),
    # 文字の高さ（起点の長さ）（f32 × 1）
    SceneSection("text_heights", "f32", "f", 1),
    # 字形の幅の倍率（f32 × 1）
    SceneSection("text_widths", "f32", "f", 1),
    # 回転（度、反時計回り）（f32 × 1）
    SceneSection("text_rotations", "f32", "f", 1),
    # 斜体の角度（度）（f32 × 1）
    SceneSection("text_obliques", "f32", "f", 1),
    # 折り返す枠の幅（起点の長さ）。折り返さなければ 0（f32 × 1）
    SceneSection("text_boxes", "f32", "f", 1),
    # 位置合わせ。水平（horizontal の値）と垂直（vertical の値）（u8 × 2）
    SceneSection("text_aligns", "u8", "B", 2),
    # 印（text_flag の値の和）（u8 × 1）
    SceneSection("text_flags", "u8", "B", 1),
    # 書式つきの文字の行の番号。0 から数える。1 行の文字は 0（u32 × 1）
    SceneSection("text_lines", "u32", "I", 1),
    # フォント（font_names の行）。分からなければ「なし」（u32 × 1）
    SceneSection("text_fonts", "u32", "I", 1),
    # 文字の描く単位（unit_entities の行）（u32 × 1）
    SceneSection("text_units", "u32", "I", 1),
    # 点の位置。塊の原点からの X・Y（f32 × 2）
    SceneSection("point_xy", "f32", "f", 2),
    # 点の回転（度、反時計回り）（f32 × 1）
    SceneSection("point_rotations", "f32", "f", 1),
    # 既定義の印のキー。名前空間（strings の始まりと長さ）と、元のコード（strings の始まりと長さ）。文書の点の表し方に従う点は、長さが 0（u32 × 4）
    SceneSection("point_markers", "u32", "I", 4),
    # 既定義の印の尺度。文書の点の表し方に従う点は 1（f32 × 1）
    SceneSection("point_scales", "f32", "f", 1),
    # 点の描く単位（unit_entities の行）（u32 × 1）
    SceneSection("point_units", "u32", "I", 1),
    # 文書の点の表し方（決定 0160）。形（point_mark の値）・囲み（point_frame の値の和）・大きさの意味（point_size の値）・大きさ。文書が持たなければ行がない（f32 × 4）
    SceneSection("point_style", "f32", "f", 4),
    # 下敷きの参照名（strings の始まりと長さ）。コアは開かない（u32 × 2）
    SceneSection("image_names", "u32", "I", 2),
    # 資源の中のページ（strings の始まりと長さ）。なければ長さが 0（u32 × 2）
    SceneSection("image_pages", "u32", "I", 2),
    # 画像の基点（左下の角）。塊の原点からの X・Y（f32 × 2）
    SceneSection("image_xy", "f32", "f", 2),
    # 画像の X の辺と Y の辺のベクトル（起点の長さ）。image_frames が corners なら内容の全体の辺、unit なら内容の 1 単位の辺（f32 × 4）
    SceneSection("image_edges", "f32", "f", 4),
    # 資源の種類（image_kind の値）（u8 × 1）
    SceneSection("image_kinds", "u8", "B", 1),
    # 辺のベクトルの意味（image_frame の値）（u8 × 1）
    SceneSection("image_frames", "u8", "B", 1),
    # 画像の描く単位（unit_entities の行）（u32 × 1）
    SceneSection("image_units", "u32", "I", 1),
    # 描く単位の要素（entity_ids の行）（u32 × 1）
    SceneSection("unit_entities", "u32", "I", 1),
    # 描く単位の色（color_systems の行）（u32 × 1）
    SceneSection("unit_colors", "u32", "I", 1),
    # 線幅（用紙の mm）。0 は、いちばん細い線（f32 × 1）
    SceneSection("unit_weights", "f32", "f", 1),
    # 切り抜き（clip_polygons の行）。切り抜かなければ「なし」（u32 × 1）
    SceneSection("unit_clips", "u32", "I", 1),
    # 実効のレイヤ（layer_ids の行）（u32 × 1）
    SceneSection("unit_layers", "u32", "I", 1),
    # 要素の ID（strings の始まりと長さ、決定 0156）。記号の属性なら、属性を持つ記号の ID（u32 × 2）
    SceneSection("entity_ids", "u32", "I", 2),
    # 要素の種類（entity_kind の値）（u8 × 1）
    SceneSection("entity_kinds", "u8", "B", 1),
    # 配置の道筋の始まり（step_ids の行）。外側から内側へ（u32 × 1）
    SceneSection("entity_paths", "u32", "I", 1),
    # 配置の要素の ID（strings の始まりと長さ）（u32 × 2）
    SceneSection("step_ids", "u32", "I", 2),
    # 格子に並べた記号の列と行。格子でなければ 0 と 0（u32 × 2）
    SceneSection("step_cells", "u32", "I", 2),
    # 色の符号の系統（color_system の値）（u8 × 1）
    SceneSection("color_systems", "u8", "B", 1),
    # 系統の中の符号。系統がなければ 0（u32 × 1）
    SceneSection("color_codes", "u32", "I", 1),
    # ファイルが持つ RGB と、持つかどうか（持てば 255、持たなければ 0）（u8 × 4）
    SceneSection("color_files", "u8", "B", 4),
    # 表示する RGBA。ファイルの RGB か、色の表（palette）から引いた値。未解決なら 0（u8 × 4）
    SceneSection("color_rgba", "u8", "B", 4),
    # 印（color_flag の値の和）（u8 × 1）
    SceneSection("color_flags", "u8", "B", 1),
    # 切り抜きの組の多角形の始まり（polygon_vertices の行）。描くものは、組のすべての多角形の内側だけを描く（u32 × 1）
    SceneSection("clip_polygons", "u32", "I", 1),
    # 多角形の頂点の始まり（clip_xy の行）（u32 × 1）
    SceneSection("polygon_vertices", "u32", "I", 1),
    # 多角形の印。1 なら、内側を隠す（外側だけを描く）（u8 × 1）
    SceneSection("polygon_flags", "u8", "B", 1),
    # 切り抜きの多角形の頂点。起点の座標の X・Y（f64 × 2）
    SceneSection("clip_xy", "f64", "d", 2),
    # レイヤの ID（strings の始まりと長さ）。tables の op が返すものと同じ形（u32 × 2）
    SceneSection("layer_ids", "u32", "I", 2),
    # フォントの元の名前（strings の始まりと長さ）（u32 × 2）
    SceneSection("font_names", "u32", "I", 2),
    # 文字列を並べた UTF-8（u8 × 1）
    SceneSection("strings", "u8", "B", 1),
    # 文字の字形の始まり（glyph_shapes の行）。字形を持つ文字は、文字列の代わりに字形を描く。持たなければ（フォントがない・置けない）文字列を描く（u32 × 1）
    SceneSection("text_glyphs", "u32", "I", 1),
    # 置いた字形の形（shape_rings の行）（u32 × 1）
    SceneSection("glyph_shapes", "u32", "I", 1),
    # 形の座標（字形の単位）を写す変換の線形の部分 [a, b, c, d]。形の点 (u, v) は、x = a·u + c·v + e、y = b·u + d·v + f に置く（e・f は glyph_xy）（f32 × 4）
    SceneSection("glyph_axes", "f32", "f", 4),
    # 字形の原点（形の座標の (0, 0)）を置く点 [e, f]。塊の原点からの X・Y（f32 × 2）
    SceneSection("glyph_xy", "f32", "f", 2),
    # 字形の形の輪の始まり（outline_vertices の行）。輪は非零の規則で塗る。空白などは輪を持たない（u32 × 1）
    SceneSection("shape_rings", "u32", "I", 1),
    # 字形の輪の頂点の始まり（outline_xy の行）。輪は閉じているものとし、始点を繰り返さない（u32 × 1）
    SceneSection("outline_vertices", "u32", "I", 1),
    # 字形の輪の頂点（字形の単位）。曲線は、許容差を字形の単位に直して 2 の冪に切り下げた刻みで折れ線にする（f32 × 2）
    SceneSection("outline_xy", "f32", "f", 2),
)

# シーンの列に入れる値の組
SCENE_VALUES: Mapping[str, Mapping[str, int]] = MappingProxyType(
    {
        # 文字の水平の位置合わせ
        "horizontal": MappingProxyType(
            {
                # 左
                "left": 0,
                # 中央
                "center": 1,
                # 右
                "right": 2,
                # 文字の外形の中央
                "middle": 3,
                # 2 点の間に、縦横の比を保って合わせる
                "aligned": 4,
                # 2 点の間に、高さを保って幅だけを合わせる
                "fit": 5,
            }
        ),
        # 文字の垂直の位置合わせ
        "vertical": MappingProxyType(
            {
                # 基準線
                "baseline": 0,
                # 下
                "bottom": 1,
                # 中央
                "middle": 2,
                # 上
                "top": 3,
            }
        ),
        # 文字の印。和で持つ
        "text_flag": MappingProxyType(
            {
                # 左右に反転する
                "backward": 1,
                # 上下に反転する
                "upside_down": 2,
                # 縦書き
                "vertical": 4,
                # 書式つきの文字の区間。基点は枠の基点で、行の中の位置は表示する側が送る
                "formatted": 8,
                # 上下に重ねた文字の上
                "stack_upper": 16,
                # 上下に重ねた文字の下
                "stack_lower": 32,
                # 高さは文字枠の高さ（決定 0184）。height_frame と height_em のどちらもなければ、大文字の高さか、字形との関係が決まらない形式の宣言値
                "height_frame": 64,
                # 高さは em（決定 0184）
                "height_em": 128,
            }
        ),
        # 文書の点の基本の形
        "point_mark": MappingProxyType(
            {
                # 小さな点
                "dot": 0,
                # なし
                "none": 1,
                # 十字
                "plus": 2,
                # バツ
                "cross": 3,
                # 上向きの短い線
                "tick": 4,
            }
        ),
        # 文書の点の囲み。和で持つ
        "point_frame": MappingProxyType(
            {
                # 円で囲む
                "circle": 1,
                # 四角で囲む
                "square": 2,
            }
        ),
        # 文書の点の大きさの意味
        "point_size": MappingProxyType(
            {
                # 起点の長さ
                "length": 0,
                # 画面の高さに対する百分率
                "screen": 1,
            }
        ),
        # 下敷きの資源の種類
        "image_kind": MappingProxyType(
            {
                # 画素の画像
                "image": 0,
                # PDF
                "pdf": 1,
                # DWF（決定 0184）
                "dwf": 2,
                # DGN（決定 0184）
                "dgn": 3,
            }
        ),
        # 画像の辺のベクトルの意味
        "image_frame": MappingProxyType(
            {
                # 内容の全体を、2 つの辺の平行四辺形に収める
                "corners": 0,
                # 辺は内容の 1 単位。内容の大きさを掛ける
                "unit": 1,
            }
        ),
        # 要素の種類（entities の op の kind と同じ）
        "entity_kind": MappingProxyType(
            {
                # 線分
                "line": 0,
                # 円
                "circle": 1,
                # 曲線
                "curve": 2,
                # 折れ線
                "polyline": 3,
                # 文字
                "text": 4,
                # 書式つきの文字
                "formatted": 5,
                # 寸法
                "dimension": 6,
                # 引出線
                "leader": 7,
                # 引出線つきの注記
                "callout": 8,
                # 表
                "table": 9,
                # 下敷き
                "underlay": 10,
                # 記号
                "symbol": 11,
                # ハッチング
                "hatch": 12,
                # 点
                "point": 13,
                # 記号の属性
                "attribute": 14,
            }
        ),
        # 色の符号の系統
        "color_system": MappingProxyType(
            {
                # 符号を持たない
                "none": 0,
                # AutoCAD の色の番号
                "aci": 1,
                # SXF の色のコード
                "sxf": 2,
                # Jw_cad のペンの番号
                "jww": 3,
            }
        ),
        # 色の印。和で持つ
        "color_flag": MappingProxyType(
            {
                # 表示する RGBA が決まらない。表示する側が決める
                "unresolved": 1,
                # 背景で変わる色（ACI の 7）。表示する側が、背景に対する前景の色にする
                "foreground": 2,
            }
        ),
    }
)
