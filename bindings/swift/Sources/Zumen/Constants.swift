// 生成したファイル。手で直さない。
// 元は schema/boundary.toml と schema/scene.toml で、`cargo xtask codegen` で作り直す。

/// 境界の版・状態コードと、シーンの配置（決定 0012・0137・0225）
public enum Constants {
    /// パッケージの版（決定 0015）
    public static let version = "0.2.0"

    /// ABI の大版
    public static let abiVersion: UInt32 = 1

    /// メッセージのスキーマの大版（要求と応答の v）
    public static let schemaMajor: Int64 = 1

    /// メッセージのスキーマの小版
    public static let schemaMinor: Int64 = 49

    /// 状態コード
    public enum Status {
        /// 成功した
        public static let ok: Int32 = 0

        /// 操作が失敗した。応答に error の診断がある
        public static let failed: Int32 = 1

        /// 引数が使えない（NULL、長さが合わない、使えない名前など）。応答は書かない
        public static let invalid: Int32 = 2

        /// 別のスレッドが、同じセッションを使っている
        public static let busy: Int32 = 3

        /// コアの不具合で止まった。セッションは閉じるしかない
        public static let panicked: Int32 = 4

        /// 状態コードの名前。知らない値は `unknown(<値>)`
        public static func name(_ code: Int32) -> String {
            switch code {
            case 0: return "ok"
            case 1: return "failed"
            case 2: return "invalid"
            case 3: return "busy"
            case 4: return "panicked"
            default: return "unknown(\(code))"
            }
        }
    }

    /// シーンの大版。区画の意味を変えるときに上げる
    public static let sceneMajor: UInt16 = 1

    /// シーンの小版。区画か値を足すたびに上げる
    public static let sceneMinor: UInt16 = 2

    /// 番号の「なし」（u32 の最大）
    public static let sceneNone = UInt32.max

    /// シーンの区画の値の型
    public enum ColumnType: Sendable {
        case u8, u32, f32, f64

        /// 値のバイト数
        public var size: Int {
            switch self {
            case .u8: return 1
            case .u32, .f32: return 4
            case .f64: return 8
            }
        }
    }

    /// シーンの区画の定義
    public struct SceneSection: Sendable {
        /// 名前
        public let name: String
        /// 値の型
        public let type: ColumnType
        /// 行ごとの値の数
        public let width: Int
    }

    /// シーンの区画の定義。番号の順
    public static let sceneSections: [SceneSection] = [
        /// 塊の原点（起点の座標の X・Y）。塊の座標は、ここからの相対の値（f64 × 2）
        SceneSection(name: "chunk_origins", type: .f64, width: 2),
        /// 塊の描くものの相対座標の範囲（X の最小・Y の最小・X の最大・Y の最大）。文字は基点だけを含む（f32 × 4）
        SceneSection(name: "chunk_bounds", type: .f32, width: 4),
        /// 塊の線の始まり（line_vertices の行）（u32 × 1）
        SceneSection(name: "chunk_lines", type: .u32, width: 1),
        /// 塊の面の始まり（face_rings の行）（u32 × 1）
        SceneSection(name: "chunk_faces", type: .u32, width: 1),
        /// 塊の文字の始まり（text_strings の行）（u32 × 1）
        SceneSection(name: "chunk_texts", type: .u32, width: 1),
        /// 塊の点の始まり（point_xy の行）（u32 × 1）
        SceneSection(name: "chunk_points", type: .u32, width: 1),
        /// 塊の画像の始まり（image_names の行）（u32 × 1）
        SceneSection(name: "chunk_images", type: .u32, width: 1),
        /// 線（折れ線）の頂点の始まり（line_xy の行）（u32 × 1）
        SceneSection(name: "line_vertices", type: .u32, width: 1),
        /// 線の描く単位（unit_entities の行）（u32 × 1）
        SceneSection(name: "line_units", type: .u32, width: 1),
        /// 線の頂点。塊の原点からの X・Y（f32 × 2）
        SceneSection(name: "line_xy", type: .f32, width: 2),
        /// 面の輪の始まり（ring_vertices の行）。輪は偶奇の規則で塗る（u32 × 1）
        SceneSection(name: "face_rings", type: .u32, width: 1),
        /// 面の描く単位（unit_entities の行）（u32 × 1）
        SceneSection(name: "face_units", type: .u32, width: 1),
        /// 輪の頂点の始まり（face_xy の行）。輪は閉じているものとし、始点を繰り返さない（u32 × 1）
        SceneSection(name: "ring_vertices", type: .u32, width: 1),
        /// 面の輪の頂点。塊の原点からの X・Y（f32 × 2）
        SceneSection(name: "face_xy", type: .f32, width: 2),
        /// 文字の内容（strings の始まりと長さ）（u32 × 2）
        SceneSection(name: "text_strings", type: .u32, width: 2),
        /// 文字の基点。塊の原点からの X・Y（f32 × 2）
        SceneSection(name: "text_xy", type: .f32, width: 2),
        /// 2 点の間に置く文字の 2 点目。塊の原点からの X・Y。2 点の間に置かなければ NaN（f32 × 2）
        SceneSection(name: "text_second", type: .f32, width: 2),
        /// 文字の高さ（起点の長さ）（f32 × 1）
        SceneSection(name: "text_heights", type: .f32, width: 1),
        /// 字形の幅の倍率（f32 × 1）
        SceneSection(name: "text_widths", type: .f32, width: 1),
        /// 回転（度、反時計回り）（f32 × 1）
        SceneSection(name: "text_rotations", type: .f32, width: 1),
        /// 斜体の角度（度）（f32 × 1）
        SceneSection(name: "text_obliques", type: .f32, width: 1),
        /// 折り返す枠の幅（起点の長さ）。折り返さなければ 0（f32 × 1）
        SceneSection(name: "text_boxes", type: .f32, width: 1),
        /// 位置合わせ。水平（horizontal の値）と垂直（vertical の値）（u8 × 2）
        SceneSection(name: "text_aligns", type: .u8, width: 2),
        /// 印（text_flag の値の和）（u8 × 1）
        SceneSection(name: "text_flags", type: .u8, width: 1),
        /// 書式つきの文字の行の番号。0 から数える。1 行の文字は 0（u32 × 1）
        SceneSection(name: "text_lines", type: .u32, width: 1),
        /// フォント（font_names の行）。分からなければ「なし」（u32 × 1）
        SceneSection(name: "text_fonts", type: .u32, width: 1),
        /// 文字の描く単位（unit_entities の行）（u32 × 1）
        SceneSection(name: "text_units", type: .u32, width: 1),
        /// 点の位置。塊の原点からの X・Y（f32 × 2）
        SceneSection(name: "point_xy", type: .f32, width: 2),
        /// 点の回転（度、反時計回り）（f32 × 1）
        SceneSection(name: "point_rotations", type: .f32, width: 1),
        /// 既定義の印のキー。名前空間（strings の始まりと長さ）と、元のコード（strings の始まりと長さ）。文書の点の表し方に従う点は、長さが 0（u32 × 4）
        SceneSection(name: "point_markers", type: .u32, width: 4),
        /// 既定義の印の尺度。文書の点の表し方に従う点は 1（f32 × 1）
        SceneSection(name: "point_scales", type: .f32, width: 1),
        /// 点の描く単位（unit_entities の行）（u32 × 1）
        SceneSection(name: "point_units", type: .u32, width: 1),
        /// 文書の点の表し方（決定 0160）。形（point_mark の値）・囲み（point_frame の値の和）・大きさの意味（point_size の値）・大きさ。文書が持たなければ行がない（f32 × 4）
        SceneSection(name: "point_style", type: .f32, width: 4),
        /// 下敷きの参照名（strings の始まりと長さ）。コアは開かない（u32 × 2）
        SceneSection(name: "image_names", type: .u32, width: 2),
        /// 資源の中のページ（strings の始まりと長さ）。なければ長さが 0（u32 × 2）
        SceneSection(name: "image_pages", type: .u32, width: 2),
        /// 画像の基点（左下の角）。塊の原点からの X・Y（f32 × 2）
        SceneSection(name: "image_xy", type: .f32, width: 2),
        /// 画像の X の辺と Y の辺のベクトル（起点の長さ）。image_frames が corners なら内容の全体の辺、unit なら内容の 1 単位の辺（f32 × 4）
        SceneSection(name: "image_edges", type: .f32, width: 4),
        /// 資源の種類（image_kind の値）（u8 × 1）
        SceneSection(name: "image_kinds", type: .u8, width: 1),
        /// 辺のベクトルの意味（image_frame の値）（u8 × 1）
        SceneSection(name: "image_frames", type: .u8, width: 1),
        /// 画像の描く単位（unit_entities の行）（u32 × 1）
        SceneSection(name: "image_units", type: .u32, width: 1),
        /// 描く単位の要素（entity_ids の行）（u32 × 1）
        SceneSection(name: "unit_entities", type: .u32, width: 1),
        /// 描く単位の色（color_systems の行）（u32 × 1）
        SceneSection(name: "unit_colors", type: .u32, width: 1),
        /// 線幅（用紙の mm）。0 は、いちばん細い線（f32 × 1）
        SceneSection(name: "unit_weights", type: .f32, width: 1),
        /// 切り抜き（clip_polygons の行）。切り抜かなければ「なし」（u32 × 1）
        SceneSection(name: "unit_clips", type: .u32, width: 1),
        /// 実効のレイヤ（layer_ids の行）（u32 × 1）
        SceneSection(name: "unit_layers", type: .u32, width: 1),
        /// 要素の ID（strings の始まりと長さ、決定 0156）。記号の属性なら、属性を持つ記号の ID（u32 × 2）
        SceneSection(name: "entity_ids", type: .u32, width: 2),
        /// 要素の種類（entity_kind の値）（u8 × 1）
        SceneSection(name: "entity_kinds", type: .u8, width: 1),
        /// 配置の道筋の始まり（step_ids の行）。外側から内側へ（u32 × 1）
        SceneSection(name: "entity_paths", type: .u32, width: 1),
        /// 配置の要素の ID（strings の始まりと長さ）（u32 × 2）
        SceneSection(name: "step_ids", type: .u32, width: 2),
        /// 格子に並べた記号の列と行。格子でなければ 0 と 0（u32 × 2）
        SceneSection(name: "step_cells", type: .u32, width: 2),
        /// 色の符号の系統（color_system の値）（u8 × 1）
        SceneSection(name: "color_systems", type: .u8, width: 1),
        /// 系統の中の符号。系統がなければ 0（u32 × 1）
        SceneSection(name: "color_codes", type: .u32, width: 1),
        /// ファイルが持つ RGB と、持つかどうか（持てば 255、持たなければ 0）（u8 × 4）
        SceneSection(name: "color_files", type: .u8, width: 4),
        /// 表示する RGBA。ファイルの RGB か、色の表（palette）から引いた値。未解決なら 0（u8 × 4）
        SceneSection(name: "color_rgba", type: .u8, width: 4),
        /// 印（color_flag の値の和）（u8 × 1）
        SceneSection(name: "color_flags", type: .u8, width: 1),
        /// 切り抜きの組の多角形の始まり（polygon_vertices の行）。描くものは、組のすべての多角形の内側だけを描く（u32 × 1）
        SceneSection(name: "clip_polygons", type: .u32, width: 1),
        /// 多角形の頂点の始まり（clip_xy の行）（u32 × 1）
        SceneSection(name: "polygon_vertices", type: .u32, width: 1),
        /// 多角形の印。1 なら、内側を隠す（外側だけを描く）（u8 × 1）
        SceneSection(name: "polygon_flags", type: .u8, width: 1),
        /// 切り抜きの多角形の頂点。起点の座標の X・Y（f64 × 2）
        SceneSection(name: "clip_xy", type: .f64, width: 2),
        /// レイヤの ID（strings の始まりと長さ）。tables の op が返すものと同じ形（u32 × 2）
        SceneSection(name: "layer_ids", type: .u32, width: 2),
        /// フォントの元の名前（strings の始まりと長さ）（u32 × 2）
        SceneSection(name: "font_names", type: .u32, width: 2),
        /// 文字列を並べた UTF-8（u8 × 1）
        SceneSection(name: "strings", type: .u8, width: 1),
        /// 文字の字形の始まり（glyph_shapes の行）。字形を持つ文字は、文字列の代わりに字形を描く。持たなければ（フォントがない・置けない）文字列を描く（u32 × 1）
        SceneSection(name: "text_glyphs", type: .u32, width: 1),
        /// 置いた字形の形（shape_rings の行）（u32 × 1）
        SceneSection(name: "glyph_shapes", type: .u32, width: 1),
        /// 形の座標（字形の単位）を写す変換の線形の部分 [a, b, c, d]。形の点 (u, v) は、x = a·u + c·v + e、y = b·u + d·v + f に置く（e・f は glyph_xy）（f32 × 4）
        SceneSection(name: "glyph_axes", type: .f32, width: 4),
        /// 字形の原点（形の座標の (0, 0)）を置く点 [e, f]。塊の原点からの X・Y（f32 × 2）
        SceneSection(name: "glyph_xy", type: .f32, width: 2),
        /// 字形の形の輪の始まり（outline_vertices の行）。輪は非零の規則で塗る。空白などは輪を持たない（u32 × 1）
        SceneSection(name: "shape_rings", type: .u32, width: 1),
        /// 字形の輪の頂点の始まり（outline_xy の行）。輪は閉じているものとし、始点を繰り返さない（u32 × 1）
        SceneSection(name: "outline_vertices", type: .u32, width: 1),
        /// 字形の輪の頂点（字形の単位）。曲線は、許容差を字形の単位に直して 2 の冪に切り下げた刻みで折れ線にする（f32 × 2）
        SceneSection(name: "outline_xy", type: .f32, width: 2),
    ]

    /// シーンの列に入れる値の組。組の名前から、値の名前と値の表を引く
    public static let sceneValues: [String: [String: UInt32]] = [
        // 文字の水平の位置合わせ
        "horizontal": [
            "left": 0,
            "center": 1,
            "right": 2,
            "middle": 3,
            "aligned": 4,
            "fit": 5,
        ],
        // 文字の垂直の位置合わせ
        "vertical": [
            "baseline": 0,
            "bottom": 1,
            "middle": 2,
            "top": 3,
        ],
        // 文字の印。和で持つ
        "text_flag": [
            "backward": 1,
            "upside_down": 2,
            "vertical": 4,
            "formatted": 8,
            "stack_upper": 16,
            "stack_lower": 32,
            "height_frame": 64,
            "height_em": 128,
        ],
        // 文書の点の基本の形
        "point_mark": [
            "dot": 0,
            "none": 1,
            "plus": 2,
            "cross": 3,
            "tick": 4,
        ],
        // 文書の点の囲み。和で持つ
        "point_frame": [
            "circle": 1,
            "square": 2,
        ],
        // 文書の点の大きさの意味
        "point_size": [
            "length": 0,
            "screen": 1,
        ],
        // 下敷きの資源の種類
        "image_kind": [
            "image": 0,
            "pdf": 1,
            "dwf": 2,
            "dgn": 3,
        ],
        // 画像の辺のベクトルの意味
        "image_frame": [
            "corners": 0,
            "unit": 1,
        ],
        // 要素の種類（entities の op の kind と同じ）
        "entity_kind": [
            "line": 0,
            "circle": 1,
            "curve": 2,
            "polyline": 3,
            "text": 4,
            "formatted": 5,
            "dimension": 6,
            "leader": 7,
            "callout": 8,
            "table": 9,
            "underlay": 10,
            "symbol": 11,
            "hatch": 12,
            "point": 13,
            "attribute": 14,
        ],
        // 色の符号の系統
        "color_system": [
            "none": 0,
            "aci": 1,
            "sxf": 2,
            "jww": 3,
        ],
        // 色の印。和で持つ
        "color_flag": [
            "unresolved": 1,
            "foreground": 2,
        ],
    ]
}
