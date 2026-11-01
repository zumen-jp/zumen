// 生成したファイル。手で直さない。
// 元は schema/boundary.toml と schema/scene.toml で、`cargo xtask codegen` で作り直す。

package dev.zumen;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

/** 境界の版・状態コードと、シーンの配置（決定 0012・0137・0224） */
public final class Constants {
    private Constants() {}

    /** パッケージの版（決定 0015） */
    public static final String VERSION = "0.2.0";

    /** ABI の大版 */
    public static final int ABI_VERSION = 1;

    /** メッセージのスキーマの大版（要求と応答の v） */
    public static final int SCHEMA_MAJOR = 1;

    /** メッセージのスキーマの小版 */
    public static final int SCHEMA_MINOR = 57;

    /** 状態コード */
    public static final class Status {
        private Status() {}

        /** 成功した */
        public static final int OK = 0;

        /** 操作が失敗した。応答に error の診断がある */
        public static final int FAILED = 1;

        /** 引数が使えない（NULL、長さが合わない、使えない名前など）。応答は書かない */
        public static final int INVALID = 2;

        /** 別のスレッドが、同じセッションを使っている */
        public static final int BUSY = 3;

        /** コアの不具合で止まった。セッションは閉じるしかない */
        public static final int PANICKED = 4;

        /** 状態コードの名前。知らない値は {@code unknown(<値>)} */
        public static String name(int code) {
            switch (code) {
                case 0:
                    return "ok";
                case 1:
                    return "failed";
                case 2:
                    return "invalid";
                case 3:
                    return "busy";
                case 4:
                    return "panicked";
                default:
                    return "unknown(" + code + ")";
            }
        }
    }

    /** シーンの大版。区画の意味を変えるときに上げる */
    public static final int SCENE_MAJOR = 1;

    /** シーンの小版。区画か値を足すたびに上げる */
    public static final int SCENE_MINOR = 2;

    /** 番号の「なし」（u32 の最大）を int で読んだ値 */
    public static final int SCENE_NONE = -1;

    /** シーンの区画の値の型 */
    public enum ColumnType {
        /** u8 */
        U8(1),
        /** u32 */
        U32(4),
        /** f32 */
        F32(4),
        /** f64 */
        F64(8);

        /** 値のバイト数 */
        public final int size;

        ColumnType(int size) {
            this.size = size;
        }
    }

    /** シーンの区画の定義 */
    public static final class SceneSection {
        /** 名前 */
        public final String name;
        /** 値の型 */
        public final ColumnType type;
        /** 行ごとの値の数 */
        public final int width;

        SceneSection(String name, ColumnType type, int width) {
            this.name = name;
            this.type = type;
            this.width = width;
        }
    }

    /** シーンの区画の定義。番号の順。書き換えない */
    public static final SceneSection[] SCENE_SECTIONS = {
        /** 塊の原点（起点の座標の X・Y）。塊の座標は、ここからの相対の値（f64 × 2） */
        new SceneSection("chunk_origins", ColumnType.F64, 2),
        /** 塊の描くものの相対座標の範囲（X の最小・Y の最小・X の最大・Y の最大）。文字は基点だけを含む（f32 × 4） */
        new SceneSection("chunk_bounds", ColumnType.F32, 4),
        /** 塊の線の始まり（line_vertices の行）（u32 × 1） */
        new SceneSection("chunk_lines", ColumnType.U32, 1),
        /** 塊の面の始まり（face_rings の行）（u32 × 1） */
        new SceneSection("chunk_faces", ColumnType.U32, 1),
        /** 塊の文字の始まり（text_strings の行）（u32 × 1） */
        new SceneSection("chunk_texts", ColumnType.U32, 1),
        /** 塊の点の始まり（point_xy の行）（u32 × 1） */
        new SceneSection("chunk_points", ColumnType.U32, 1),
        /** 塊の画像の始まり（image_names の行）（u32 × 1） */
        new SceneSection("chunk_images", ColumnType.U32, 1),
        /** 線（折れ線）の頂点の始まり（line_xy の行）（u32 × 1） */
        new SceneSection("line_vertices", ColumnType.U32, 1),
        /** 線の描く単位（unit_entities の行）（u32 × 1） */
        new SceneSection("line_units", ColumnType.U32, 1),
        /** 線の頂点。塊の原点からの X・Y（f32 × 2） */
        new SceneSection("line_xy", ColumnType.F32, 2),
        /** 面の輪の始まり（ring_vertices の行）。輪は偶奇の規則で塗る（u32 × 1） */
        new SceneSection("face_rings", ColumnType.U32, 1),
        /** 面の描く単位（unit_entities の行）（u32 × 1） */
        new SceneSection("face_units", ColumnType.U32, 1),
        /** 輪の頂点の始まり（face_xy の行）。輪は閉じているものとし、始点を繰り返さない（u32 × 1） */
        new SceneSection("ring_vertices", ColumnType.U32, 1),
        /** 面の輪の頂点。塊の原点からの X・Y（f32 × 2） */
        new SceneSection("face_xy", ColumnType.F32, 2),
        /** 文字の内容（strings の始まりと長さ）（u32 × 2） */
        new SceneSection("text_strings", ColumnType.U32, 2),
        /** 文字の基点。塊の原点からの X・Y（f32 × 2） */
        new SceneSection("text_xy", ColumnType.F32, 2),
        /** 2 点の間に置く文字の 2 点目。塊の原点からの X・Y。2 点の間に置かなければ NaN（f32 × 2） */
        new SceneSection("text_second", ColumnType.F32, 2),
        /** 文字の高さ（起点の長さ）（f32 × 1） */
        new SceneSection("text_heights", ColumnType.F32, 1),
        /** 字形の幅の倍率（f32 × 1） */
        new SceneSection("text_widths", ColumnType.F32, 1),
        /** 回転（度、反時計回り）（f32 × 1） */
        new SceneSection("text_rotations", ColumnType.F32, 1),
        /** 斜体の角度（度）（f32 × 1） */
        new SceneSection("text_obliques", ColumnType.F32, 1),
        /** 折り返す枠の幅（起点の長さ）。折り返さなければ 0（f32 × 1） */
        new SceneSection("text_boxes", ColumnType.F32, 1),
        /** 位置合わせ。水平（horizontal の値）と垂直（vertical の値）（u8 × 2） */
        new SceneSection("text_aligns", ColumnType.U8, 2),
        /** 印（text_flag の値の和）（u8 × 1） */
        new SceneSection("text_flags", ColumnType.U8, 1),
        /** 書式つきの文字の行の番号。0 から数える。1 行の文字は 0（u32 × 1） */
        new SceneSection("text_lines", ColumnType.U32, 1),
        /** フォント（font_names の行）。分からなければ「なし」（u32 × 1） */
        new SceneSection("text_fonts", ColumnType.U32, 1),
        /** 文字の描く単位（unit_entities の行）（u32 × 1） */
        new SceneSection("text_units", ColumnType.U32, 1),
        /** 点の位置。塊の原点からの X・Y（f32 × 2） */
        new SceneSection("point_xy", ColumnType.F32, 2),
        /** 点の回転（度、反時計回り）（f32 × 1） */
        new SceneSection("point_rotations", ColumnType.F32, 1),
        /** 既定義の印のキー。名前空間（strings の始まりと長さ）と、元のコード（strings の始まりと長さ）。文書の点の表し方に従う点は、長さが 0（u32 × 4） */
        new SceneSection("point_markers", ColumnType.U32, 4),
        /** 既定義の印の尺度。文書の点の表し方に従う点は 1（f32 × 1） */
        new SceneSection("point_scales", ColumnType.F32, 1),
        /** 点の描く単位（unit_entities の行）（u32 × 1） */
        new SceneSection("point_units", ColumnType.U32, 1),
        /** 文書の点の表し方（決定 0160）。形（point_mark の値）・囲み（point_frame の値の和）・大きさの意味（point_size の値）・大きさ。文書が持たなければ行がない（f32 × 4） */
        new SceneSection("point_style", ColumnType.F32, 4),
        /** 下敷きの参照名（strings の始まりと長さ）。コアは開かない（u32 × 2） */
        new SceneSection("image_names", ColumnType.U32, 2),
        /** 資源の中のページ（strings の始まりと長さ）。なければ長さが 0（u32 × 2） */
        new SceneSection("image_pages", ColumnType.U32, 2),
        /** 画像の基点（左下の角）。塊の原点からの X・Y（f32 × 2） */
        new SceneSection("image_xy", ColumnType.F32, 2),
        /** 画像の X の辺と Y の辺のベクトル（起点の長さ）。image_frames が corners なら内容の全体の辺、unit なら内容の 1 単位の辺（f32 × 4） */
        new SceneSection("image_edges", ColumnType.F32, 4),
        /** 資源の種類（image_kind の値）（u8 × 1） */
        new SceneSection("image_kinds", ColumnType.U8, 1),
        /** 辺のベクトルの意味（image_frame の値）（u8 × 1） */
        new SceneSection("image_frames", ColumnType.U8, 1),
        /** 画像の描く単位（unit_entities の行）（u32 × 1） */
        new SceneSection("image_units", ColumnType.U32, 1),
        /** 描く単位の要素（entity_ids の行）（u32 × 1） */
        new SceneSection("unit_entities", ColumnType.U32, 1),
        /** 描く単位の色（color_systems の行）（u32 × 1） */
        new SceneSection("unit_colors", ColumnType.U32, 1),
        /** 線幅（用紙の mm）。0 は、いちばん細い線（f32 × 1） */
        new SceneSection("unit_weights", ColumnType.F32, 1),
        /** 切り抜き（clip_polygons の行）。切り抜かなければ「なし」（u32 × 1） */
        new SceneSection("unit_clips", ColumnType.U32, 1),
        /** 実効のレイヤ（layer_ids の行）（u32 × 1） */
        new SceneSection("unit_layers", ColumnType.U32, 1),
        /** 要素の ID（strings の始まりと長さ、決定 0156）。記号の属性なら、属性を持つ記号の ID（u32 × 2） */
        new SceneSection("entity_ids", ColumnType.U32, 2),
        /** 要素の種類（entity_kind の値）（u8 × 1） */
        new SceneSection("entity_kinds", ColumnType.U8, 1),
        /** 配置の道筋の始まり（step_ids の行）。外側から内側へ（u32 × 1） */
        new SceneSection("entity_paths", ColumnType.U32, 1),
        /** 配置の要素の ID（strings の始まりと長さ）（u32 × 2） */
        new SceneSection("step_ids", ColumnType.U32, 2),
        /** 格子に並べた記号の列と行。格子でなければ 0 と 0（u32 × 2） */
        new SceneSection("step_cells", ColumnType.U32, 2),
        /** 色の符号の系統（color_system の値）（u8 × 1） */
        new SceneSection("color_systems", ColumnType.U8, 1),
        /** 系統の中の符号。系統がなければ 0（u32 × 1） */
        new SceneSection("color_codes", ColumnType.U32, 1),
        /** ファイルが持つ RGB と、持つかどうか（持てば 255、持たなければ 0）（u8 × 4） */
        new SceneSection("color_files", ColumnType.U8, 4),
        /** 表示する RGBA。ファイルの RGB か、色の表（palette）から引いた値。未解決なら 0（u8 × 4） */
        new SceneSection("color_rgba", ColumnType.U8, 4),
        /** 印（color_flag の値の和）（u8 × 1） */
        new SceneSection("color_flags", ColumnType.U8, 1),
        /** 切り抜きの組の多角形の始まり（polygon_vertices の行）。描くものは、組のすべての多角形の内側だけを描く（u32 × 1） */
        new SceneSection("clip_polygons", ColumnType.U32, 1),
        /** 多角形の頂点の始まり（clip_xy の行）（u32 × 1） */
        new SceneSection("polygon_vertices", ColumnType.U32, 1),
        /** 多角形の印。1 なら、内側を隠す（外側だけを描く）（u8 × 1） */
        new SceneSection("polygon_flags", ColumnType.U8, 1),
        /** 切り抜きの多角形の頂点。起点の座標の X・Y（f64 × 2） */
        new SceneSection("clip_xy", ColumnType.F64, 2),
        /** レイヤの ID（strings の始まりと長さ）。tables の op が返すものと同じ形（u32 × 2） */
        new SceneSection("layer_ids", ColumnType.U32, 2),
        /** フォントの元の名前（strings の始まりと長さ）（u32 × 2） */
        new SceneSection("font_names", ColumnType.U32, 2),
        /** 文字列を並べた UTF-8（u8 × 1） */
        new SceneSection("strings", ColumnType.U8, 1),
        /** 文字の字形の始まり（glyph_shapes の行）。字形を持つ文字は、文字列の代わりに字形を描く。持たなければ（フォントがない・置けない）文字列を描く（u32 × 1） */
        new SceneSection("text_glyphs", ColumnType.U32, 1),
        /** 置いた字形の形（shape_rings の行）（u32 × 1） */
        new SceneSection("glyph_shapes", ColumnType.U32, 1),
        /** 形の座標（字形の単位）を写す変換の線形の部分 [a, b, c, d]。形の点 (u, v) は、x = a·u + c·v + e、y = b·u + d·v + f に置く（e・f は glyph_xy）（f32 × 4） */
        new SceneSection("glyph_axes", ColumnType.F32, 4),
        /** 字形の原点（形の座標の (0, 0)）を置く点 [e, f]。塊の原点からの X・Y（f32 × 2） */
        new SceneSection("glyph_xy", ColumnType.F32, 2),
        /** 字形の形の輪の始まり（outline_vertices の行）。輪は非零の規則で塗る。空白などは輪を持たない（u32 × 1） */
        new SceneSection("shape_rings", ColumnType.U32, 1),
        /** 字形の輪の頂点の始まり（outline_xy の行）。輪は閉じているものとし、始点を繰り返さない（u32 × 1） */
        new SceneSection("outline_vertices", ColumnType.U32, 1),
        /** 字形の輪の頂点（字形の単位）。曲線は、許容差を字形の単位に直して 2 の冪に切り下げた刻みで折れ線にする（f32 × 2） */
        new SceneSection("outline_xy", ColumnType.F32, 2),
    };

    /** シーンの列に入れる値の組。組の名前から、値の名前と値の表を引く */
    public static final Map<String, Map<String, Integer>> SCENE_VALUES;

    static {
        Map<String, Map<String, Integer>> values = new LinkedHashMap<String, Map<String, Integer>>();
        Map<String, Integer> items;
        // 文字の水平の位置合わせ
        items = new LinkedHashMap<String, Integer>();
        items.put("left", 0);
        items.put("center", 1);
        items.put("right", 2);
        items.put("middle", 3);
        items.put("aligned", 4);
        items.put("fit", 5);
        values.put("horizontal", Collections.unmodifiableMap(items));
        // 文字の垂直の位置合わせ
        items = new LinkedHashMap<String, Integer>();
        items.put("baseline", 0);
        items.put("bottom", 1);
        items.put("middle", 2);
        items.put("top", 3);
        values.put("vertical", Collections.unmodifiableMap(items));
        // 文字の印。和で持つ
        items = new LinkedHashMap<String, Integer>();
        items.put("backward", 1);
        items.put("upside_down", 2);
        items.put("vertical", 4);
        items.put("formatted", 8);
        items.put("stack_upper", 16);
        items.put("stack_lower", 32);
        items.put("height_frame", 64);
        items.put("height_em", 128);
        values.put("text_flag", Collections.unmodifiableMap(items));
        // 文書の点の基本の形
        items = new LinkedHashMap<String, Integer>();
        items.put("dot", 0);
        items.put("none", 1);
        items.put("plus", 2);
        items.put("cross", 3);
        items.put("tick", 4);
        values.put("point_mark", Collections.unmodifiableMap(items));
        // 文書の点の囲み。和で持つ
        items = new LinkedHashMap<String, Integer>();
        items.put("circle", 1);
        items.put("square", 2);
        values.put("point_frame", Collections.unmodifiableMap(items));
        // 文書の点の大きさの意味
        items = new LinkedHashMap<String, Integer>();
        items.put("length", 0);
        items.put("screen", 1);
        values.put("point_size", Collections.unmodifiableMap(items));
        // 下敷きの資源の種類
        items = new LinkedHashMap<String, Integer>();
        items.put("image", 0);
        items.put("pdf", 1);
        items.put("dwf", 2);
        items.put("dgn", 3);
        values.put("image_kind", Collections.unmodifiableMap(items));
        // 画像の辺のベクトルの意味
        items = new LinkedHashMap<String, Integer>();
        items.put("corners", 0);
        items.put("unit", 1);
        values.put("image_frame", Collections.unmodifiableMap(items));
        // 要素の種類（entities の op の kind と同じ）
        items = new LinkedHashMap<String, Integer>();
        items.put("line", 0);
        items.put("circle", 1);
        items.put("curve", 2);
        items.put("polyline", 3);
        items.put("text", 4);
        items.put("formatted", 5);
        items.put("dimension", 6);
        items.put("leader", 7);
        items.put("callout", 8);
        items.put("table", 9);
        items.put("underlay", 10);
        items.put("symbol", 11);
        items.put("hatch", 12);
        items.put("point", 13);
        items.put("attribute", 14);
        values.put("entity_kind", Collections.unmodifiableMap(items));
        // 色の符号の系統
        items = new LinkedHashMap<String, Integer>();
        items.put("none", 0);
        items.put("aci", 1);
        items.put("sxf", 2);
        items.put("jww", 3);
        values.put("color_system", Collections.unmodifiableMap(items));
        // 色の印。和で持つ
        items = new LinkedHashMap<String, Integer>();
        items.put("unresolved", 1);
        items.put("foreground", 2);
        values.put("color_flag", Collections.unmodifiableMap(items));
        SCENE_VALUES = Collections.unmodifiableMap(values);
    }
}
