// 生成したファイル。手で直さない。
// 元は schema/scene.toml で、`cargo xtask codegen` で作り直す。

//! 2D のシーンの配置（決定 0137・0172・0178）。

/// 見出しの始まりの 4 バイト
pub const MAGIC: [u8; 4] = *b"ZMSC";
/// 大版。区画の意味を変えるときに上げる
pub const MAJOR: u16 = 1;
/// 小版。区画か値を足すたびに上げる
pub const MINOR: u16 = 0;
/// 番号の「なし」
pub const NONE: u32 = u32::MAX;

/// 区画の値の型
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
}

impl Type {
    /// 値のバイト数
    pub const fn size(self) -> usize {
        match self {
            Self::U8 => 1,
            Self::U32 => 4,
            Self::F32 => 4,
            Self::F64 => 8,
        }
    }
}

/// 区画の定義
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Section {
    /// 名前
    pub name: &'static str,
    /// 値の型
    pub ty: Type,
    /// 行ごとの値の数
    pub width: usize,
}

/// 塊の原点（起点の座標の X・Y）。塊の座標は、ここからの相対の値
pub const CHUNK_ORIGINS: usize = 0;
/// 塊の描くものの相対座標の範囲（X の最小・Y の最小・X の最大・Y の最大）。文字は基点だけを含む
pub const CHUNK_BOUNDS: usize = 1;
/// 塊の線の始まり（line_vertices の行）
pub const CHUNK_LINES: usize = 2;
/// 塊の面の始まり（face_rings の行）
pub const CHUNK_FACES: usize = 3;
/// 塊の文字の始まり（text_strings の行）
pub const CHUNK_TEXTS: usize = 4;
/// 塊の点の始まり（point_xy の行）
pub const CHUNK_POINTS: usize = 5;
/// 塊の画像の始まり（image_names の行）
pub const CHUNK_IMAGES: usize = 6;
/// 線（折れ線）の頂点の始まり（line_xy の行）
pub const LINE_VERTICES: usize = 7;
/// 線の描く単位（unit_entities の行）
pub const LINE_UNITS: usize = 8;
/// 線の頂点。塊の原点からの X・Y
pub const LINE_XY: usize = 9;
/// 面の輪の始まり（ring_vertices の行）。輪は偶奇の規則で塗る
pub const FACE_RINGS: usize = 10;
/// 面の描く単位（unit_entities の行）
pub const FACE_UNITS: usize = 11;
/// 輪の頂点の始まり（face_xy の行）。輪は閉じているものとし、始点を繰り返さない
pub const RING_VERTICES: usize = 12;
/// 面の輪の頂点。塊の原点からの X・Y
pub const FACE_XY: usize = 13;
/// 文字の内容（strings の始まりと長さ）
pub const TEXT_STRINGS: usize = 14;
/// 文字の基点。塊の原点からの X・Y
pub const TEXT_XY: usize = 15;
/// 2 点の間に置く文字の 2 点目。塊の原点からの X・Y。2 点の間に置かなければ NaN
pub const TEXT_SECOND: usize = 16;
/// 文字の高さ（起点の長さ）
pub const TEXT_HEIGHTS: usize = 17;
/// 字形の幅の倍率
pub const TEXT_WIDTHS: usize = 18;
/// 回転（度、反時計回り）
pub const TEXT_ROTATIONS: usize = 19;
/// 斜体の角度（度）
pub const TEXT_OBLIQUES: usize = 20;
/// 折り返す枠の幅（起点の長さ）。折り返さなければ 0
pub const TEXT_BOXES: usize = 21;
/// 位置合わせ。水平（horizontal の値）と垂直（vertical の値）
pub const TEXT_ALIGNS: usize = 22;
/// 印（text_flag の値の和）
pub const TEXT_FLAGS: usize = 23;
/// 書式つきの文字の行の番号。0 から数える。1 行の文字は 0
pub const TEXT_LINES: usize = 24;
/// フォント（font_names の行）。分からなければ「なし」
pub const TEXT_FONTS: usize = 25;
/// 文字の描く単位（unit_entities の行）
pub const TEXT_UNITS: usize = 26;
/// 点の位置。塊の原点からの X・Y
pub const POINT_XY: usize = 27;
/// 点の回転（度、反時計回り）
pub const POINT_ROTATIONS: usize = 28;
/// 既定義の印のキー。名前空間（strings の始まりと長さ）と、元のコード（strings の始まりと長さ）。文書の点の表し方に従う点は、長さが 0
pub const POINT_MARKERS: usize = 29;
/// 既定義の印の尺度。文書の点の表し方に従う点は 1
pub const POINT_SCALES: usize = 30;
/// 点の描く単位（unit_entities の行）
pub const POINT_UNITS: usize = 31;
/// 文書の点の表し方（決定 0160）。形（point_mark の値）・囲み（point_frame の値の和）・大きさの意味（point_size の値）・大きさ。文書が持たなければ行がない
pub const POINT_STYLE: usize = 32;
/// 下敷きの参照名（strings の始まりと長さ）。コアは開かない
pub const IMAGE_NAMES: usize = 33;
/// 資源の中のページ（strings の始まりと長さ）。なければ長さが 0
pub const IMAGE_PAGES: usize = 34;
/// 画像の基点（左下の角）。塊の原点からの X・Y
pub const IMAGE_XY: usize = 35;
/// 画像の X の辺と Y の辺のベクトル（起点の長さ）。image_frames が corners なら内容の全体の辺、unit なら内容の 1 単位の辺
pub const IMAGE_EDGES: usize = 36;
/// 資源の種類（image_kind の値）
pub const IMAGE_KINDS: usize = 37;
/// 辺のベクトルの意味（image_frame の値）
pub const IMAGE_FRAMES: usize = 38;
/// 画像の描く単位（unit_entities の行）
pub const IMAGE_UNITS: usize = 39;
/// 描く単位の要素（entity_ids の行）
pub const UNIT_ENTITIES: usize = 40;
/// 描く単位の色（color_systems の行）
pub const UNIT_COLORS: usize = 41;
/// 線幅（用紙の mm）。0 は、いちばん細い線
pub const UNIT_WEIGHTS: usize = 42;
/// 切り抜き（clip_polygons の行）。切り抜かなければ「なし」
pub const UNIT_CLIPS: usize = 43;
/// 実効のレイヤ（layer_ids の行）
pub const UNIT_LAYERS: usize = 44;
/// 要素の ID（strings の始まりと長さ、決定 0156）。記号の属性なら、属性を持つ記号の ID
pub const ENTITY_IDS: usize = 45;
/// 要素の種類（entity_kind の値）
pub const ENTITY_KINDS: usize = 46;
/// 配置の道筋の始まり（step_ids の行）。外側から内側へ
pub const ENTITY_PATHS: usize = 47;
/// 配置の要素の ID（strings の始まりと長さ）
pub const STEP_IDS: usize = 48;
/// 格子に並べた記号の列と行。格子でなければ 0 と 0
pub const STEP_CELLS: usize = 49;
/// 色の符号の系統（color_system の値）
pub const COLOR_SYSTEMS: usize = 50;
/// 系統の中の符号。系統がなければ 0
pub const COLOR_CODES: usize = 51;
/// ファイルが持つ RGB と、持つかどうか（持てば 255、持たなければ 0）
pub const COLOR_FILES: usize = 52;
/// 表示する RGBA。ファイルの RGB か、色の表（palette）から引いた値。未解決なら 0
pub const COLOR_RGBA: usize = 53;
/// 印（color_flag の値の和）
pub const COLOR_FLAGS: usize = 54;
/// 切り抜きの組の多角形の始まり（polygon_vertices の行）。描くものは、組のすべての多角形の内側だけを描く
pub const CLIP_POLYGONS: usize = 55;
/// 多角形の頂点の始まり（clip_xy の行）
pub const POLYGON_VERTICES: usize = 56;
/// 多角形の印。1 なら、内側を隠す（外側だけを描く）
pub const POLYGON_FLAGS: usize = 57;
/// 切り抜きの多角形の頂点。起点の座標の X・Y
pub const CLIP_XY: usize = 58;
/// レイヤの ID（strings の始まりと長さ）。tables の op が返すものと同じ形
pub const LAYER_IDS: usize = 59;
/// フォントの元の名前（strings の始まりと長さ）
pub const FONT_NAMES: usize = 60;
/// 文字列を並べた UTF-8
pub const STRINGS: usize = 61;

/// 区画の定義。番号の順
pub const SECTIONS: [Section; 62] = [
    Section {
        name: "chunk_origins",
        ty: Type::F64,
        width: 2,
    },
    Section {
        name: "chunk_bounds",
        ty: Type::F32,
        width: 4,
    },
    Section {
        name: "chunk_lines",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "chunk_faces",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "chunk_texts",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "chunk_points",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "chunk_images",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "line_vertices",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "line_units",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "line_xy",
        ty: Type::F32,
        width: 2,
    },
    Section {
        name: "face_rings",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "face_units",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "ring_vertices",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "face_xy",
        ty: Type::F32,
        width: 2,
    },
    Section {
        name: "text_strings",
        ty: Type::U32,
        width: 2,
    },
    Section {
        name: "text_xy",
        ty: Type::F32,
        width: 2,
    },
    Section {
        name: "text_second",
        ty: Type::F32,
        width: 2,
    },
    Section {
        name: "text_heights",
        ty: Type::F32,
        width: 1,
    },
    Section {
        name: "text_widths",
        ty: Type::F32,
        width: 1,
    },
    Section {
        name: "text_rotations",
        ty: Type::F32,
        width: 1,
    },
    Section {
        name: "text_obliques",
        ty: Type::F32,
        width: 1,
    },
    Section {
        name: "text_boxes",
        ty: Type::F32,
        width: 1,
    },
    Section {
        name: "text_aligns",
        ty: Type::U8,
        width: 2,
    },
    Section {
        name: "text_flags",
        ty: Type::U8,
        width: 1,
    },
    Section {
        name: "text_lines",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "text_fonts",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "text_units",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "point_xy",
        ty: Type::F32,
        width: 2,
    },
    Section {
        name: "point_rotations",
        ty: Type::F32,
        width: 1,
    },
    Section {
        name: "point_markers",
        ty: Type::U32,
        width: 4,
    },
    Section {
        name: "point_scales",
        ty: Type::F32,
        width: 1,
    },
    Section {
        name: "point_units",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "point_style",
        ty: Type::F32,
        width: 4,
    },
    Section {
        name: "image_names",
        ty: Type::U32,
        width: 2,
    },
    Section {
        name: "image_pages",
        ty: Type::U32,
        width: 2,
    },
    Section {
        name: "image_xy",
        ty: Type::F32,
        width: 2,
    },
    Section {
        name: "image_edges",
        ty: Type::F32,
        width: 4,
    },
    Section {
        name: "image_kinds",
        ty: Type::U8,
        width: 1,
    },
    Section {
        name: "image_frames",
        ty: Type::U8,
        width: 1,
    },
    Section {
        name: "image_units",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "unit_entities",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "unit_colors",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "unit_weights",
        ty: Type::F32,
        width: 1,
    },
    Section {
        name: "unit_clips",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "unit_layers",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "entity_ids",
        ty: Type::U32,
        width: 2,
    },
    Section {
        name: "entity_kinds",
        ty: Type::U8,
        width: 1,
    },
    Section {
        name: "entity_paths",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "step_ids",
        ty: Type::U32,
        width: 2,
    },
    Section {
        name: "step_cells",
        ty: Type::U32,
        width: 2,
    },
    Section {
        name: "color_systems",
        ty: Type::U8,
        width: 1,
    },
    Section {
        name: "color_codes",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "color_files",
        ty: Type::U8,
        width: 4,
    },
    Section {
        name: "color_rgba",
        ty: Type::U8,
        width: 4,
    },
    Section {
        name: "color_flags",
        ty: Type::U8,
        width: 1,
    },
    Section {
        name: "clip_polygons",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "polygon_vertices",
        ty: Type::U32,
        width: 1,
    },
    Section {
        name: "polygon_flags",
        ty: Type::U8,
        width: 1,
    },
    Section {
        name: "clip_xy",
        ty: Type::F64,
        width: 2,
    },
    Section {
        name: "layer_ids",
        ty: Type::U32,
        width: 2,
    },
    Section {
        name: "font_names",
        ty: Type::U32,
        width: 2,
    },
    Section {
        name: "strings",
        ty: Type::U8,
        width: 1,
    },
];

/// 文字の水平の位置合わせ
pub mod horizontal {
    /// 左
    pub const LEFT: u8 = 0;
    /// 中央
    pub const CENTER: u8 = 1;
    /// 右
    pub const RIGHT: u8 = 2;
    /// 文字の外形の中央
    pub const MIDDLE: u8 = 3;
    /// 2 点の間に、縦横の比を保って合わせる
    pub const ALIGNED: u8 = 4;
    /// 2 点の間に、高さを保って幅だけを合わせる
    pub const FIT: u8 = 5;
}

/// 文字の垂直の位置合わせ
pub mod vertical {
    /// 基準線
    pub const BASELINE: u8 = 0;
    /// 下
    pub const BOTTOM: u8 = 1;
    /// 中央
    pub const MIDDLE: u8 = 2;
    /// 上
    pub const TOP: u8 = 3;
}

/// 文字の印。和で持つ
pub mod text_flag {
    /// 左右に反転する
    pub const BACKWARD: u8 = 1;
    /// 上下に反転する
    pub const UPSIDE_DOWN: u8 = 2;
    /// 縦書き
    pub const VERTICAL: u8 = 4;
    /// 書式つきの文字の区間。基点は枠の基点で、行の中の位置は表示する側が送る
    pub const FORMATTED: u8 = 8;
    /// 上下に重ねた文字の上
    pub const STACK_UPPER: u8 = 16;
    /// 上下に重ねた文字の下
    pub const STACK_LOWER: u8 = 32;
}

/// 文書の点の基本の形
pub mod point_mark {
    /// 小さな点
    pub const DOT: u8 = 0;
    /// なし
    pub const NONE: u8 = 1;
    /// 十字
    pub const PLUS: u8 = 2;
    /// バツ
    pub const CROSS: u8 = 3;
    /// 上向きの短い線
    pub const TICK: u8 = 4;
}

/// 文書の点の囲み。和で持つ
pub mod point_frame {
    /// 円で囲む
    pub const CIRCLE: u8 = 1;
    /// 四角で囲む
    pub const SQUARE: u8 = 2;
}

/// 文書の点の大きさの意味
pub mod point_size {
    /// 起点の長さ
    pub const LENGTH: u8 = 0;
    /// 画面の高さに対する百分率
    pub const SCREEN: u8 = 1;
}

/// 下敷きの資源の種類
pub mod image_kind {
    /// 画素の画像
    pub const IMAGE: u8 = 0;
    /// PDF
    pub const PDF: u8 = 1;
}

/// 画像の辺のベクトルの意味
pub mod image_frame {
    /// 内容の全体を、2 つの辺の平行四辺形に収める
    pub const CORNERS: u8 = 0;
    /// 辺は内容の 1 単位。内容の大きさを掛ける
    pub const UNIT: u8 = 1;
}

/// 要素の種類（entities の op の kind と同じ）
pub mod entity_kind {
    /// 線分
    pub const LINE: u8 = 0;
    /// 円
    pub const CIRCLE: u8 = 1;
    /// 曲線
    pub const CURVE: u8 = 2;
    /// 折れ線
    pub const POLYLINE: u8 = 3;
    /// 文字
    pub const TEXT: u8 = 4;
    /// 書式つきの文字
    pub const FORMATTED: u8 = 5;
    /// 寸法
    pub const DIMENSION: u8 = 6;
    /// 引出線
    pub const LEADER: u8 = 7;
    /// 引出線つきの注記
    pub const CALLOUT: u8 = 8;
    /// 表
    pub const TABLE: u8 = 9;
    /// 下敷き
    pub const UNDERLAY: u8 = 10;
    /// 記号
    pub const SYMBOL: u8 = 11;
    /// ハッチング
    pub const HATCH: u8 = 12;
    /// 点
    pub const POINT: u8 = 13;
    /// 記号の属性
    pub const ATTRIBUTE: u8 = 14;
}

/// 色の符号の系統
pub mod color_system {
    /// 符号を持たない
    pub const NONE: u8 = 0;
    /// AutoCAD の色の番号
    pub const ACI: u8 = 1;
    /// SXF の色のコード
    pub const SXF: u8 = 2;
    /// Jw_cad のペンの番号
    pub const JWW: u8 = 3;
}

/// 色の印。和で持つ
pub mod color_flag {
    /// 表示する RGBA が決まらない。表示する側が決める
    pub const UNRESOLVED: u8 = 1;
    /// 背景で変わる色（ACI の 7）。表示する側が、背景に対する前景の色にする
    pub const FOREGROUND: u8 = 2;
}
