// 生成したファイル。手で直さない。
// 元は schema/boundary.toml と schema/scene.toml で、`cargo xtask codegen` で作り直す。

#nullable enable

using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

namespace Zumen.Provisional
{
    /// <summary>版（決定 0015）</summary>
    public static class Versions
    {
        /// <summary>パッケージの版</summary>
        public const string Package = "0.0.0";

        /// <summary>ABI の大版</summary>
        public const uint Abi = 1;

        /// <summary>メッセージのスキーマの大版（要求と応答の v）</summary>
        public const uint SchemaMajor = 1;

        /// <summary>メッセージのスキーマの小版</summary>
        public const uint SchemaMinor = 39;
    }

    /// <summary>状態コード。知らない値も、そのまま持てる</summary>
    public enum Status
    {
        /// <summary>
        /// 成功した
        /// </summary>
        Ok = 0,
        /// <summary>
        /// 操作が失敗した。応答に error の診断がある
        /// </summary>
        Failed = 1,
        /// <summary>
        /// 引数が使えない（NULL、長さが合わない、使えない名前など）。応答は書かない
        /// </summary>
        Invalid = 2,
        /// <summary>
        /// 別のスレッドが、同じセッションを使っている
        /// </summary>
        Busy = 3,
        /// <summary>
        /// コアの不具合で止まった。セッションは閉じるしかない
        /// </summary>
        Panicked = 4,
    }

    /// <summary>状態コードの名前</summary>
    public static class StatusNames
    {
        /// <summary>値から名前を引く。知らない値は unknown(値)</summary>
        public static string Of(int code)
        {
            switch (code)
            {
                case 0: return "ok";
                case 1: return "failed";
                case 2: return "invalid";
                case 3: return "busy";
                case 4: return "panicked";
                default: return "unknown(" + code + ")";
            }
        }
    }

    /// <summary>コアが返すバッファー（zm_buf）。zm_free で返す</summary>
    [StructLayout(LayoutKind.Sequential)]
    internal unsafe struct Buf
    {
        public byte* Ptr;
        public nuint Len;
    }

    /// <summary>ネイティブのライブラリが出す関数の形。32 bit の Windows では stdcall、ほかでは cdecl（Winapi）</summary>
    internal static unsafe class NativeFunctions
    {
        /// <summary>
        /// ABI の大版を返す。この関数の形は変えない
        /// </summary>
        [UnmanagedFunctionPointer(CallingConvention.Winapi)]
        internal delegate uint ZmAbiVersion();

        /// <summary>
        /// セッションを作る。設定は JSON（schema/messages.json の session_options）で渡す
        /// </summary>
        [UnmanagedFunctionPointer(CallingConvention.Winapi)]
        internal delegate int ZmSessionNew(byte* @options, nuint @options_len, IntPtr* @session);

        /// <summary>
        /// 名前のついた入力に、バイト列を足す。同じ名前で何回か呼ぶと、後ろに足していく
        /// </summary>
        [UnmanagedFunctionPointer(CallingConvention.Winapi)]
        internal delegate int ZmPush(IntPtr @session, byte* @name, nuint @name_len, byte* @data, nuint @data_len);

        /// <summary>
        /// 要求のメッセージを渡し、応答のメッセージを受け取る。前の呼び出しのバイナリの結果は、ここで捨てる
        /// </summary>
        [UnmanagedFunctionPointer(CallingConvention.Winapi)]
        internal delegate int ZmCall(IntPtr @session, byte* @request, nuint @request_len, Buf* @response);

        /// <summary>
        /// 直前の zm_call が作ったバイナリの結果を取り出す。取り出したものは、セッションを閉じたあとも使える
        /// </summary>
        [UnmanagedFunctionPointer(CallingConvention.Winapi)]
        internal delegate int ZmTake(IntPtr @session, uint @blob, Buf* @out);

        /// <summary>
        /// 表の結果（種類が arrow のもの）を、Arrow の C Stream Interface で取り出す
        /// </summary>
        [UnmanagedFunctionPointer(CallingConvention.Winapi)]
        internal delegate int ZmTakeArrow(IntPtr @session, uint @blob, IntPtr @out);

        /// <summary>
        /// 動いている呼び出しを止める。どのスレッドからも呼べる。動いている呼び出しがなければ、何もしない
        /// </summary>
        [UnmanagedFunctionPointer(CallingConvention.Winapi)]
        internal delegate void ZmCancel(IntPtr @session);

        /// <summary>
        /// セッションを閉じる。NULL なら何もしない。別のスレッドが使っている途中なら閉じない
        /// </summary>
        [UnmanagedFunctionPointer(CallingConvention.Winapi)]
        internal delegate void ZmSessionClose(IntPtr @session);

        /// <summary>
        /// コアが返したバッファーを返す。返したあとの buf は空になる。空のバッファーと NULL には何もしない
        /// </summary>
        [UnmanagedFunctionPointer(CallingConvention.Winapi)]
        internal delegate void ZmFree(Buf* @buf);

        /// <summary>出すべき関数の名前</summary>
        internal static readonly string[] Names =
        {
            "zm_abi_version",
            "zm_session_new",
            "zm_push",
            "zm_call",
            "zm_take",
            "zm_take_arrow",
            "zm_cancel",
            "zm_session_close",
            "zm_free",
        };
    }

    /// <summary>シーンの blob の配置（決定 0137）</summary>
    public static class SceneLayout
    {
        /// <summary>見出しの始まりの 4 バイト（ZMSC）</summary>
        public const uint Magic = 0x4353_4D5A;

        /// <summary>大版。区画の意味を変えるときに上げる</summary>
        public const ushort Major = 1;

        /// <summary>小版。区画か値を足すたびに上げる</summary>
        public const ushort Minor = 2;

        /// <summary>番号の「なし」</summary>
        public const uint None = uint.MaxValue;

        /// <summary>区画の定義。番号の順</summary>
        public static readonly IReadOnlyList<SceneSection> Sections = new[]
        {
            // 塊の原点（起点の座標の X・Y）。塊の座標は、ここからの相対の値（f64 × 2）
            new SceneSection(0, "chunk_origins", typeof(double), 2),
            // 塊の描くものの相対座標の範囲（X の最小・Y の最小・X の最大・Y の最大）。文字は基点だけを含む（f32 × 4）
            new SceneSection(1, "chunk_bounds", typeof(float), 4),
            // 塊の線の始まり（line_vertices の行）（u32 × 1）
            new SceneSection(2, "chunk_lines", typeof(uint), 1),
            // 塊の面の始まり（face_rings の行）（u32 × 1）
            new SceneSection(3, "chunk_faces", typeof(uint), 1),
            // 塊の文字の始まり（text_strings の行）（u32 × 1）
            new SceneSection(4, "chunk_texts", typeof(uint), 1),
            // 塊の点の始まり（point_xy の行）（u32 × 1）
            new SceneSection(5, "chunk_points", typeof(uint), 1),
            // 塊の画像の始まり（image_names の行）（u32 × 1）
            new SceneSection(6, "chunk_images", typeof(uint), 1),
            // 線（折れ線）の頂点の始まり（line_xy の行）（u32 × 1）
            new SceneSection(7, "line_vertices", typeof(uint), 1),
            // 線の描く単位（unit_entities の行）（u32 × 1）
            new SceneSection(8, "line_units", typeof(uint), 1),
            // 線の頂点。塊の原点からの X・Y（f32 × 2）
            new SceneSection(9, "line_xy", typeof(float), 2),
            // 面の輪の始まり（ring_vertices の行）。輪は偶奇の規則で塗る（u32 × 1）
            new SceneSection(10, "face_rings", typeof(uint), 1),
            // 面の描く単位（unit_entities の行）（u32 × 1）
            new SceneSection(11, "face_units", typeof(uint), 1),
            // 輪の頂点の始まり（face_xy の行）。輪は閉じているものとし、始点を繰り返さない（u32 × 1）
            new SceneSection(12, "ring_vertices", typeof(uint), 1),
            // 面の輪の頂点。塊の原点からの X・Y（f32 × 2）
            new SceneSection(13, "face_xy", typeof(float), 2),
            // 文字の内容（strings の始まりと長さ）（u32 × 2）
            new SceneSection(14, "text_strings", typeof(uint), 2),
            // 文字の基点。塊の原点からの X・Y（f32 × 2）
            new SceneSection(15, "text_xy", typeof(float), 2),
            // 2 点の間に置く文字の 2 点目。塊の原点からの X・Y。2 点の間に置かなければ NaN（f32 × 2）
            new SceneSection(16, "text_second", typeof(float), 2),
            // 文字の高さ（起点の長さ）（f32 × 1）
            new SceneSection(17, "text_heights", typeof(float), 1),
            // 字形の幅の倍率（f32 × 1）
            new SceneSection(18, "text_widths", typeof(float), 1),
            // 回転（度、反時計回り）（f32 × 1）
            new SceneSection(19, "text_rotations", typeof(float), 1),
            // 斜体の角度（度）（f32 × 1）
            new SceneSection(20, "text_obliques", typeof(float), 1),
            // 折り返す枠の幅（起点の長さ）。折り返さなければ 0（f32 × 1）
            new SceneSection(21, "text_boxes", typeof(float), 1),
            // 位置合わせ。水平（horizontal の値）と垂直（vertical の値）（u8 × 2）
            new SceneSection(22, "text_aligns", typeof(byte), 2),
            // 印（text_flag の値の和）（u8 × 1）
            new SceneSection(23, "text_flags", typeof(byte), 1),
            // 書式つきの文字の行の番号。0 から数える。1 行の文字は 0（u32 × 1）
            new SceneSection(24, "text_lines", typeof(uint), 1),
            // フォント（font_names の行）。分からなければ「なし」（u32 × 1）
            new SceneSection(25, "text_fonts", typeof(uint), 1),
            // 文字の描く単位（unit_entities の行）（u32 × 1）
            new SceneSection(26, "text_units", typeof(uint), 1),
            // 点の位置。塊の原点からの X・Y（f32 × 2）
            new SceneSection(27, "point_xy", typeof(float), 2),
            // 点の回転（度、反時計回り）（f32 × 1）
            new SceneSection(28, "point_rotations", typeof(float), 1),
            // 既定義の印のキー。名前空間（strings の始まりと長さ）と、元のコード（strings の始まりと長さ）。文書の点の表し方に従う点は、長さが 0（u32 × 4）
            new SceneSection(29, "point_markers", typeof(uint), 4),
            // 既定義の印の尺度。文書の点の表し方に従う点は 1（f32 × 1）
            new SceneSection(30, "point_scales", typeof(float), 1),
            // 点の描く単位（unit_entities の行）（u32 × 1）
            new SceneSection(31, "point_units", typeof(uint), 1),
            // 文書の点の表し方（決定 0160）。形（point_mark の値）・囲み（point_frame の値の和）・大きさの意味（point_size の値）・大きさ。文書が持たなければ行がない（f32 × 4）
            new SceneSection(32, "point_style", typeof(float), 4),
            // 下敷きの参照名（strings の始まりと長さ）。コアは開かない（u32 × 2）
            new SceneSection(33, "image_names", typeof(uint), 2),
            // 資源の中のページ（strings の始まりと長さ）。なければ長さが 0（u32 × 2）
            new SceneSection(34, "image_pages", typeof(uint), 2),
            // 画像の基点（左下の角）。塊の原点からの X・Y（f32 × 2）
            new SceneSection(35, "image_xy", typeof(float), 2),
            // 画像の X の辺と Y の辺のベクトル（起点の長さ）。image_frames が corners なら内容の全体の辺、unit なら内容の 1 単位の辺（f32 × 4）
            new SceneSection(36, "image_edges", typeof(float), 4),
            // 資源の種類（image_kind の値）（u8 × 1）
            new SceneSection(37, "image_kinds", typeof(byte), 1),
            // 辺のベクトルの意味（image_frame の値）（u8 × 1）
            new SceneSection(38, "image_frames", typeof(byte), 1),
            // 画像の描く単位（unit_entities の行）（u32 × 1）
            new SceneSection(39, "image_units", typeof(uint), 1),
            // 描く単位の要素（entity_ids の行）（u32 × 1）
            new SceneSection(40, "unit_entities", typeof(uint), 1),
            // 描く単位の色（color_systems の行）（u32 × 1）
            new SceneSection(41, "unit_colors", typeof(uint), 1),
            // 線幅（用紙の mm）。0 は、いちばん細い線（f32 × 1）
            new SceneSection(42, "unit_weights", typeof(float), 1),
            // 切り抜き（clip_polygons の行）。切り抜かなければ「なし」（u32 × 1）
            new SceneSection(43, "unit_clips", typeof(uint), 1),
            // 実効のレイヤ（layer_ids の行）（u32 × 1）
            new SceneSection(44, "unit_layers", typeof(uint), 1),
            // 要素の ID（strings の始まりと長さ、決定 0156）。記号の属性なら、属性を持つ記号の ID（u32 × 2）
            new SceneSection(45, "entity_ids", typeof(uint), 2),
            // 要素の種類（entity_kind の値）（u8 × 1）
            new SceneSection(46, "entity_kinds", typeof(byte), 1),
            // 配置の道筋の始まり（step_ids の行）。外側から内側へ（u32 × 1）
            new SceneSection(47, "entity_paths", typeof(uint), 1),
            // 配置の要素の ID（strings の始まりと長さ）（u32 × 2）
            new SceneSection(48, "step_ids", typeof(uint), 2),
            // 格子に並べた記号の列と行。格子でなければ 0 と 0（u32 × 2）
            new SceneSection(49, "step_cells", typeof(uint), 2),
            // 色の符号の系統（color_system の値）（u8 × 1）
            new SceneSection(50, "color_systems", typeof(byte), 1),
            // 系統の中の符号。系統がなければ 0（u32 × 1）
            new SceneSection(51, "color_codes", typeof(uint), 1),
            // ファイルが持つ RGB と、持つかどうか（持てば 255、持たなければ 0）（u8 × 4）
            new SceneSection(52, "color_files", typeof(byte), 4),
            // 表示する RGBA。ファイルの RGB か、色の表（palette）から引いた値。未解決なら 0（u8 × 4）
            new SceneSection(53, "color_rgba", typeof(byte), 4),
            // 印（color_flag の値の和）（u8 × 1）
            new SceneSection(54, "color_flags", typeof(byte), 1),
            // 切り抜きの組の多角形の始まり（polygon_vertices の行）。描くものは、組のすべての多角形の内側だけを描く（u32 × 1）
            new SceneSection(55, "clip_polygons", typeof(uint), 1),
            // 多角形の頂点の始まり（clip_xy の行）（u32 × 1）
            new SceneSection(56, "polygon_vertices", typeof(uint), 1),
            // 多角形の印。1 なら、内側を隠す（外側だけを描く）（u8 × 1）
            new SceneSection(57, "polygon_flags", typeof(byte), 1),
            // 切り抜きの多角形の頂点。起点の座標の X・Y（f64 × 2）
            new SceneSection(58, "clip_xy", typeof(double), 2),
            // レイヤの ID（strings の始まりと長さ）。tables の op が返すものと同じ形（u32 × 2）
            new SceneSection(59, "layer_ids", typeof(uint), 2),
            // フォントの元の名前（strings の始まりと長さ）（u32 × 2）
            new SceneSection(60, "font_names", typeof(uint), 2),
            // 文字列を並べた UTF-8（u8 × 1）
            new SceneSection(61, "strings", typeof(byte), 1),
            // 文字の字形の始まり（glyph_shapes の行）。字形を持つ文字は、文字列の代わりに字形を描く。持たなければ（フォントがない・置けない）文字列を描く（u32 × 1）
            new SceneSection(62, "text_glyphs", typeof(uint), 1),
            // 置いた字形の形（shape_rings の行）（u32 × 1）
            new SceneSection(63, "glyph_shapes", typeof(uint), 1),
            // 形の座標（字形の単位）を写す変換の線形の部分 [a, b, c, d]。形の点 (u, v) は、x = a·u + c·v + e、y = b·u + d·v + f に置く（e・f は glyph_xy）（f32 × 4）
            new SceneSection(64, "glyph_axes", typeof(float), 4),
            // 字形の原点（形の座標の (0, 0)）を置く点 [e, f]。塊の原点からの X・Y（f32 × 2）
            new SceneSection(65, "glyph_xy", typeof(float), 2),
            // 字形の形の輪の始まり（outline_vertices の行）。輪は非零の規則で塗る。空白などは輪を持たない（u32 × 1）
            new SceneSection(66, "shape_rings", typeof(uint), 1),
            // 字形の輪の頂点の始まり（outline_xy の行）。輪は閉じているものとし、始点を繰り返さない（u32 × 1）
            new SceneSection(67, "outline_vertices", typeof(uint), 1),
            // 字形の輪の頂点（字形の単位）。曲線は、許容差を字形の単位に直して 2 の冪に切り下げた刻みで折れ線にする（f32 × 2）
            new SceneSection(68, "outline_xy", typeof(float), 2),
        };

        /// <summary>列に入れる値の組。組の名前から、値の名前と値を引く</summary>
        public static readonly IReadOnlyDictionary<string, IReadOnlyDictionary<string, uint>> Values =
            new Dictionary<string, IReadOnlyDictionary<string, uint>>
            {
                // 文字の水平の位置合わせ
                ["horizontal"] = new Dictionary<string, uint>
                {
                    // 左
                    ["left"] = 0,
                    // 中央
                    ["center"] = 1,
                    // 右
                    ["right"] = 2,
                    // 文字の外形の中央
                    ["middle"] = 3,
                    // 2 点の間に、縦横の比を保って合わせる
                    ["aligned"] = 4,
                    // 2 点の間に、高さを保って幅だけを合わせる
                    ["fit"] = 5,
                },
                // 文字の垂直の位置合わせ
                ["vertical"] = new Dictionary<string, uint>
                {
                    // 基準線
                    ["baseline"] = 0,
                    // 下
                    ["bottom"] = 1,
                    // 中央
                    ["middle"] = 2,
                    // 上
                    ["top"] = 3,
                },
                // 文字の印。和で持つ
                ["text_flag"] = new Dictionary<string, uint>
                {
                    // 左右に反転する
                    ["backward"] = 1,
                    // 上下に反転する
                    ["upside_down"] = 2,
                    // 縦書き
                    ["vertical"] = 4,
                    // 書式つきの文字の区間。基点は枠の基点で、行の中の位置は表示する側が送る
                    ["formatted"] = 8,
                    // 上下に重ねた文字の上
                    ["stack_upper"] = 16,
                    // 上下に重ねた文字の下
                    ["stack_lower"] = 32,
                    // 高さは文字枠の高さ（決定 0184）。height_frame と height_em のどちらもなければ、大文字の高さか、字形との関係が決まらない形式の宣言値
                    ["height_frame"] = 64,
                    // 高さは em（決定 0184）
                    ["height_em"] = 128,
                },
                // 文書の点の基本の形
                ["point_mark"] = new Dictionary<string, uint>
                {
                    // 小さな点
                    ["dot"] = 0,
                    // なし
                    ["none"] = 1,
                    // 十字
                    ["plus"] = 2,
                    // バツ
                    ["cross"] = 3,
                    // 上向きの短い線
                    ["tick"] = 4,
                },
                // 文書の点の囲み。和で持つ
                ["point_frame"] = new Dictionary<string, uint>
                {
                    // 円で囲む
                    ["circle"] = 1,
                    // 四角で囲む
                    ["square"] = 2,
                },
                // 文書の点の大きさの意味
                ["point_size"] = new Dictionary<string, uint>
                {
                    // 起点の長さ
                    ["length"] = 0,
                    // 画面の高さに対する百分率
                    ["screen"] = 1,
                },
                // 下敷きの資源の種類
                ["image_kind"] = new Dictionary<string, uint>
                {
                    // 画素の画像
                    ["image"] = 0,
                    // PDF
                    ["pdf"] = 1,
                    // DWF（決定 0184）
                    ["dwf"] = 2,
                    // DGN（決定 0184）
                    ["dgn"] = 3,
                },
                // 画像の辺のベクトルの意味
                ["image_frame"] = new Dictionary<string, uint>
                {
                    // 内容の全体を、2 つの辺の平行四辺形に収める
                    ["corners"] = 0,
                    // 辺は内容の 1 単位。内容の大きさを掛ける
                    ["unit"] = 1,
                },
                // 要素の種類（entities の op の kind と同じ）
                ["entity_kind"] = new Dictionary<string, uint>
                {
                    // 線分
                    ["line"] = 0,
                    // 円
                    ["circle"] = 1,
                    // 曲線
                    ["curve"] = 2,
                    // 折れ線
                    ["polyline"] = 3,
                    // 文字
                    ["text"] = 4,
                    // 書式つきの文字
                    ["formatted"] = 5,
                    // 寸法
                    ["dimension"] = 6,
                    // 引出線
                    ["leader"] = 7,
                    // 引出線つきの注記
                    ["callout"] = 8,
                    // 表
                    ["table"] = 9,
                    // 下敷き
                    ["underlay"] = 10,
                    // 記号
                    ["symbol"] = 11,
                    // ハッチング
                    ["hatch"] = 12,
                    // 点
                    ["point"] = 13,
                    // 記号の属性
                    ["attribute"] = 14,
                },
                // 色の符号の系統
                ["color_system"] = new Dictionary<string, uint>
                {
                    // 符号を持たない
                    ["none"] = 0,
                    // AutoCAD の色の番号
                    ["aci"] = 1,
                    // SXF の色のコード
                    ["sxf"] = 2,
                    // Jw_cad のペンの番号
                    ["jww"] = 3,
                },
                // 色の印。和で持つ
                ["color_flag"] = new Dictionary<string, uint>
                {
                    // 表示する RGBA が決まらない。表示する側が決める
                    ["unresolved"] = 1,
                    // 背景で変わる色（ACI の 7）。表示する側が、背景に対する前景の色にする
                    ["foreground"] = 2,
                },
            };
    }
}
