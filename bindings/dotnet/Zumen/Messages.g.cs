// 生成したファイル。手で直さない。
// 元は schema/messages.json で、`cargo xtask codegen` で作り直す。
//
// 境界のメッセージの型（決定 0012・0196）。スキーマの定義ごとに 1 つの型がある。
// 列挙は、知らない値も受け取れるように string で持ち、値を定数で出す（決定 0015）。
// 形が値で決まるもの（共用体）は JsonAny（net8.0 は JsonElement、netstandard2.0 は object）で持ち、
// 目印の値から型を引いて読み直す。

#nullable enable

using System.Collections.Generic;
#if NET
using System.Text.Json.Serialization;
using JsonAny = System.Text.Json.JsonElement;
#else
using JsonAny = System.Object;
#endif

namespace Zumen.Messages
{
#if !NET
    /// <summary>欄の JSON での名前。netstandard2.0 には System.Text.Json がないので、包みの中に代わりを置く</summary>
    [global::System.AttributeUsage(global::System.AttributeTargets.Property)]
    internal sealed class JsonPropertyNameAttribute : global::System.Attribute
    {
        public JsonPropertyNameAttribute(string name) => Name = name;

        public string Name { get; }
    }
#endif

    /// <summary>
    /// 要求。op の値で形が決まる
    /// </summary>
    public static class Request
    {
        /// <summary>目印の欄の名前</summary>
        public const string Tag = "op";

        /// <summary>目印の値と、その形の型</summary>
        public static readonly IReadOnlyDictionary<string, global::System.Type> Variants = new Dictionary<string, global::System.Type>
        {
            ["capabilities"] = typeof(CapabilitiesRequest),
            ["open"] = typeof(OpenRequest),
            ["close"] = typeof(CloseRequest),
            ["describe"] = typeof(DescribeRequest),
            ["drawing"] = typeof(DrawingRequest),
            ["entities"] = typeof(EntitiesRequest),
            ["apply"] = typeof(ApplyRequest),
            ["convert"] = typeof(ConvertRequest),
            ["tables"] = typeof(TablesRequest),
            ["scene"] = typeof(SceneRequest),
            ["check"] = typeof(CheckRequest),
            ["validate"] = typeof(ValidateRequest),
            ["review_carry"] = typeof(ReviewCarryRequest),
            ["review_adopt"] = typeof(ReviewAdoptRequest),
        };
    }

    /// <summary>
    /// どの op の要求にも置ける欄
    /// </summary>
    public sealed record RequestCommon
    {
        /// <summary>
        /// メッセージのスキーマの大版。今は 1
        /// </summary>
        [JsonPropertyName("v")]
        public ulong V { get; init; }

        /// <summary>
        /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        /// </summary>
        [JsonPropertyName("budget")]
        public Budget? Budget { get; init; }

        /// <summary>
        /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        /// </summary>
        [JsonPropertyName("partial")]
        public bool? Partial { get; init; }

        /// <summary>
        /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        /// </summary>
        [JsonPropertyName("max_places")]
        public ulong? MaxPlaces { get; init; }

        /// <summary>
        /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        /// </summary>
        [JsonPropertyName("time_limit_ms")]
        public ulong? TimeLimitMs { get; init; }
    }

    /// <summary>
    /// 予算の単位ごとの上限（決定 0010）
    /// </summary>
    public sealed record Budget
    {
        /// <summary>
        /// 受け取ったバイト数
        /// </summary>
        [JsonPropertyName("input_bytes")]
        public JsonAny? InputBytes { get; init; }

        /// <summary>
        /// 展開・復号で作ったバイト数。入れ子の分も足す
        /// </summary>
        [JsonPropertyName("decoded_bytes")]
        public JsonAny? DecodedBytes { get; init; }

        /// <summary>
        /// L0・L1 で読んだレコードの数
        /// </summary>
        [JsonPropertyName("records")]
        public JsonAny? Records { get; init; }

        /// <summary>
        /// L2・L3 で作った要素の数
        /// </summary>
        [JsonPropertyName("elements")]
        public JsonAny? Elements { get; init; }

        /// <summary>
        /// 入れ子と再帰の深さ
        /// </summary>
        [JsonPropertyName("depth")]
        public JsonAny? Depth { get; init; }

        /// <summary>
        /// 幾何の分割で作った点・線・面の数
        /// </summary>
        [JsonPropertyName("segments")]
        public JsonAny? Segments { get; init; }

        /// <summary>
        /// ほかの単位に入らない繰り返しの回数
        /// </summary>
        [JsonPropertyName("work")]
        public JsonAny? Work { get; init; }
    }

    /// <summary>
    /// 1 つの単位の上限。上限をなくすときは unlimited と書く
    /// - 整数: この量までは使える
    /// - "unlimited": 上限なし
    /// </summary>
    public static class Limit
    {
        /// <summary>
        /// 上限なし
        /// </summary>
        public const string Unlimited = "unlimited";
    }

    /// <summary>
    /// コアの版と、使える op・形式を尋ねる
    /// </summary>
    public sealed record CapabilitiesRequest
    {
        /// <summary>
        /// メッセージのスキーマの大版。今は 1
        /// </summary>
        [JsonPropertyName("v")]
        public ulong V { get; init; }

        /// <summary>
        /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        /// </summary>
        [JsonPropertyName("budget")]
        public Budget? Budget { get; init; }

        /// <summary>
        /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        /// </summary>
        [JsonPropertyName("partial")]
        public bool? Partial { get; init; }

        /// <summary>
        /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        /// </summary>
        [JsonPropertyName("max_places")]
        public ulong? MaxPlaces { get; init; }

        /// <summary>
        /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        /// </summary>
        [JsonPropertyName("time_limit_ms")]
        public ulong? TimeLimitMs { get; init; }

        /// <summary>
        /// op の名前
        /// </summary>
        [JsonPropertyName("op")]
        public string Op { get; init; } = "capabilities";
    }

    /// <summary>
    /// コアの版と、使える op・形式（決定 0015）
    /// </summary>
    public sealed record CapabilitiesResult
    {
        /// <summary>
        /// パッケージの版（semver）
        /// </summary>
        [JsonPropertyName("package")]
        public string Package { get; init; } = default!;

        /// <summary>
        /// ABI の大版。zm_abi_version と同じ
        /// </summary>
        [JsonPropertyName("abi")]
        public ulong Abi { get; init; }

        /// <summary>
        /// メッセージのスキーマの版
        /// </summary>
        [JsonPropertyName("schema")]
        public SchemaVersion Schema { get; init; } = default!;

        /// <summary>
        /// このコア（wasm ではこのモジュール）で使える op の名前
        /// </summary>
        [JsonPropertyName("ops")]
        public IReadOnlyList<string> Ops { get; init; } = default!;

        /// <summary>
        /// 読み書きできる形式
        /// </summary>
        [JsonPropertyName("formats")]
        public IReadOnlyList<Format> Formats { get; init; } = default!;
    }

    /// <summary>
    /// メッセージのスキーマの版。大版は要求と応答の v、小版は op・欄・列挙の値を足すたびに上げる
    /// </summary>
    public sealed record SchemaVersion
    {
        /// <summary>
        /// 大版
        /// </summary>
        [JsonPropertyName("major")]
        public ulong Major { get; init; }

        /// <summary>
        /// 小版
        /// </summary>
        [JsonPropertyName("minor")]
        public ulong Minor { get; init; }
    }

    /// <summary>
    /// 読み書きできる形式
    /// </summary>
    public sealed record Format
    {
        /// <summary>
        /// 形式の名前（dxf・sfc など）
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 形式の系統（autocad・sxf など、決定 0008）
        /// </summary>
        [JsonPropertyName("family")]
        public string Family { get; init; } = default!;

        /// <summary>
        /// 読めるか
        /// </summary>
        [JsonPropertyName("read")]
        public bool Read { get; init; }

        /// <summary>
        /// 書けるか
        /// </summary>
        [JsonPropertyName("write")]
        public bool Write { get; init; }

        /// <summary>
        /// この形式の ID の規則の版（決定 0015）
        /// </summary>
        [JsonPropertyName("id_rules")]
        public ulong IdRules { get; init; }
    }

    /// <summary>
    /// 入力を読んで、文書を作る。形式は入力から判定する
    /// </summary>
    public sealed record OpenRequest
    {
        /// <summary>
        /// メッセージのスキーマの大版。今は 1
        /// </summary>
        [JsonPropertyName("v")]
        public ulong V { get; init; }

        /// <summary>
        /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        /// </summary>
        [JsonPropertyName("budget")]
        public Budget? Budget { get; init; }

        /// <summary>
        /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        /// </summary>
        [JsonPropertyName("partial")]
        public bool? Partial { get; init; }

        /// <summary>
        /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        /// </summary>
        [JsonPropertyName("max_places")]
        public ulong? MaxPlaces { get; init; }

        /// <summary>
        /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        /// </summary>
        [JsonPropertyName("time_limit_ms")]
        public ulong? TimeLimitMs { get; init; }

        /// <summary>
        /// op の名前
        /// </summary>
        [JsonPropertyName("op")]
        public string Op { get; init; } = "open";

        /// <summary>
        /// 読む入力の名前。zm_push で渡したもの
        /// </summary>
        [JsonPropertyName("input")]
        public string Input { get; init; } = default!;

        /// <summary>
        /// 作る文書の名前。書かなければ入力の名前
        /// </summary>
        [JsonPropertyName("doc")]
        public string? Doc { get; init; }
    }

    /// <summary>
    /// 作った文書
    /// </summary>
    public sealed record OpenResult
    {
        /// <summary>
        /// 文書の名前
        /// </summary>
        [JsonPropertyName("doc")]
        public string Doc { get; init; } = default!;

        /// <summary>
        /// 判定した形式の名前
        /// </summary>
        [JsonPropertyName("format")]
        public string Format { get; init; } = default!;
    }

    /// <summary>
    /// 文書と入力を捨てる。名指ししたものが 1 つでもなければ、何も捨てない
    /// </summary>
    public sealed record CloseRequest
    {
        /// <summary>
        /// メッセージのスキーマの大版。今は 1
        /// </summary>
        [JsonPropertyName("v")]
        public ulong V { get; init; }

        /// <summary>
        /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        /// </summary>
        [JsonPropertyName("budget")]
        public Budget? Budget { get; init; }

        /// <summary>
        /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        /// </summary>
        [JsonPropertyName("partial")]
        public bool? Partial { get; init; }

        /// <summary>
        /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        /// </summary>
        [JsonPropertyName("max_places")]
        public ulong? MaxPlaces { get; init; }

        /// <summary>
        /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        /// </summary>
        [JsonPropertyName("time_limit_ms")]
        public ulong? TimeLimitMs { get; init; }

        /// <summary>
        /// op の名前
        /// </summary>
        [JsonPropertyName("op")]
        public string Op { get; init; } = "close";

        /// <summary>
        /// 捨てる文書の名前
        /// </summary>
        [JsonPropertyName("docs")]
        public IReadOnlyList<string>? Docs { get; init; }

        /// <summary>
        /// 捨てる入力の名前
        /// </summary>
        [JsonPropertyName("inputs")]
        public IReadOnlyList<string>? Inputs { get; init; }
    }

    /// <summary>
    /// 捨てたこと。中身はない
    /// </summary>
    public sealed record CloseResult
    {
    }

    /// <summary>
    /// 診断の文を、カタログから組み立てる（決定 0009）
    /// </summary>
    public sealed record DescribeRequest
    {
        /// <summary>
        /// メッセージのスキーマの大版。今は 1
        /// </summary>
        [JsonPropertyName("v")]
        public ulong V { get; init; }

        /// <summary>
        /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        /// </summary>
        [JsonPropertyName("budget")]
        public Budget? Budget { get; init; }

        /// <summary>
        /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        /// </summary>
        [JsonPropertyName("partial")]
        public bool? Partial { get; init; }

        /// <summary>
        /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        /// </summary>
        [JsonPropertyName("max_places")]
        public ulong? MaxPlaces { get; init; }

        /// <summary>
        /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        /// </summary>
        [JsonPropertyName("time_limit_ms")]
        public ulong? TimeLimitMs { get; init; }

        /// <summary>
        /// op の名前
        /// </summary>
        [JsonPropertyName("op")]
        public string Op { get; init; } = "describe";

        /// <summary>
        /// カタログの入力の名前。zm_push で渡したもの。書かなければ、code と値をそのまま文にする
        /// </summary>
        [JsonPropertyName("catalog")]
        public string? Catalog { get; init; }

        /// <summary>
        /// 文にする診断。応答で受け取ったもの
        /// </summary>
        [JsonPropertyName("diagnostics")]
        public IReadOnlyList<Diagnostic> Diagnostics { get; init; } = default!;
    }

    /// <summary>
    /// 診断の文
    /// </summary>
    public sealed record DescribeResult
    {
        /// <summary>
        /// 診断ごとの文。要求の diagnostics と同じ順。カタログにない code は、code と値をそのまま書く
        /// </summary>
        [JsonPropertyName("texts")]
        public IReadOnlyList<string> Texts { get; init; } = default!;
    }

    /// <summary>
    /// 開いた文書を図面モデル（L2、決定 0022）に変換し、その要約を返す。ネイティブと wasm の出力の一致を、適合試験で比べるためにも使う（決定 0007）
    /// </summary>
    public sealed record DrawingRequest
    {
        /// <summary>
        /// メッセージのスキーマの大版。今は 1
        /// </summary>
        [JsonPropertyName("v")]
        public ulong V { get; init; }

        /// <summary>
        /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        /// </summary>
        [JsonPropertyName("budget")]
        public Budget? Budget { get; init; }

        /// <summary>
        /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        /// </summary>
        [JsonPropertyName("partial")]
        public bool? Partial { get; init; }

        /// <summary>
        /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        /// </summary>
        [JsonPropertyName("max_places")]
        public ulong? MaxPlaces { get; init; }

        /// <summary>
        /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        /// </summary>
        [JsonPropertyName("time_limit_ms")]
        public ulong? TimeLimitMs { get; init; }

        /// <summary>
        /// op の名前
        /// </summary>
        [JsonPropertyName("op")]
        public string Op { get; init; } = "drawing";

        /// <summary>
        /// 文書の名前。open で作ったもの
        /// </summary>
        [JsonPropertyName("doc")]
        public string Doc { get; init; } = default!;
    }

    /// <summary>
    /// 図面モデルの要約。移さなかった図形と値は、診断で返す
    /// </summary>
    public sealed record DrawingResult
    {
        /// <summary>
        /// 文書のハッシュ。open に渡したバイト列から作る。変更の一覧の前提条件に使う（v2: と 32 桁の 16 進、決定 0156）
        /// </summary>
        [JsonPropertyName("hash")]
        public string? Hash { get; init; }

        /// <summary>
        /// 実寸の空間。モデルの順
        /// </summary>
        [JsonPropertyName("spaces")]
        public IReadOnlyList<SpaceSummary> Spaces { get; init; } = default!;

        /// <summary>
        /// 用紙。モデルの順
        /// </summary>
        [JsonPropertyName("sheets")]
        public IReadOnlyList<SheetSummary> Sheets { get; init; } = default!;

        /// <summary>
        /// 文書の点の表し方（marker を持たない点の見え方、決定 0160）。point_style の変更で変える。文書が持たなければ（JWW・SXF）持たない。小版 39 から
        /// </summary>
        [JsonPropertyName("point_style")]
        public PointStyleSummary? PointStyle { get; init; }
    }

    /// <summary>
    /// 文書の点の表し方。欄の意味は point_style_edit と同じ
    /// </summary>
    public sealed record PointStyleSummary
    {
        /// <summary>
        /// 基本の形
        /// </summary>
        [JsonPropertyName("mark")]
        public string Mark { get; init; } = default!;

        /// <summary>
        /// 円で囲む
        /// </summary>
        [JsonPropertyName("circle")]
        public bool Circle { get; init; }

        /// <summary>
        /// 四角で囲む
        /// </summary>
        [JsonPropertyName("square")]
        public bool Square { get; init; }

        /// <summary>
        /// 大きさ。意味は size_kind で決まる
        /// </summary>
        [JsonPropertyName("size")]
        public double Size { get; init; }

        /// <summary>
        /// 大きさの意味
        /// </summary>
        [JsonPropertyName("size_kind")]
        public string SizeKind { get; init; } = default!;
    }

    /// <summary>
    /// 実寸の空間の要約
    /// </summary>
    public sealed record SpaceSummary
    {
        /// <summary>
        /// 空間の ID（space/0）。entities の要素の ID の始まりと同じ（決定 0156）
        /// </summary>
        [JsonPropertyName("id")]
        public string? Id { get; init; }

        /// <summary>
        /// 文書の単位
        /// </summary>
        [JsonPropertyName("unit")]
        public string Unit { get; init; } = default!;

        /// <summary>
        /// 要素の数
        /// </summary>
        [JsonPropertyName("elements")]
        public ElementCounts Elements { get; init; } = default!;

        /// <summary>
        /// 線分の端点と文字の基点を囲む範囲。文書の単位。要素がなければ持たない
        /// </summary>
        [JsonPropertyName("extent")]
        public Extent? Extent { get; init; }
    }

    /// <summary>
    /// 用紙の要約
    /// </summary>
    public sealed record SheetSummary
    {
        /// <summary>
        /// 用紙の ID。LAYOUT がハンドルを持てば sheet/h22、なければ読んだ順の番号（sheet/0）。entities の要素の ID の始まりと同じ（決定 0156）
        /// </summary>
        [JsonPropertyName("id")]
        public string? Id { get; init; }

        /// <summary>
        /// 元の形式の用紙の名前（DXF・DWG のレイアウトの名前）。翻訳しない。なければ持たない
        /// </summary>
        [JsonPropertyName("name")]
        public string? Name { get; init; }

        /// <summary>
        /// 用紙の幅と高さ（mm）。分からなければ持たない
        /// </summary>
        [JsonPropertyName("size")]
        public IReadOnlyList<double>? Size { get; init; }

        /// <summary>
        /// 要素の数
        /// </summary>
        [JsonPropertyName("elements")]
        public ElementCounts Elements { get; init; } = default!;

        /// <summary>
        /// ビューの数
        /// </summary>
        [JsonPropertyName("views")]
        public ulong Views { get; init; }

        /// <summary>
        /// 線分の端点と文字の基点を囲む範囲。用紙の mm。要素がなければ持たない
        /// </summary>
        [JsonPropertyName("extent")]
        public Extent? Extent { get; init; }
    }

    /// <summary>
    /// 実寸の単位
    /// </summary>
    public static class Unit
    {
        /// <summary>millimeter</summary>
        public const string Millimeter = "millimeter";
        /// <summary>centimeter</summary>
        public const string Centimeter = "centimeter";
        /// <summary>meter</summary>
        public const string Meter = "meter";
        /// <summary>inch</summary>
        public const string Inch = "inch";
        /// <summary>foot</summary>
        public const string Foot = "foot";
        /// <summary>unknown</summary>
        public const string Unknown = "unknown";
    }

    /// <summary>
    /// 種類ごとの要素の数
    /// </summary>
    public sealed record ElementCounts
    {
        /// <summary>
        /// 線分
        /// </summary>
        [JsonPropertyName("lines")]
        public ulong Lines { get; init; }

        /// <summary>
        /// 文字
        /// </summary>
        [JsonPropertyName("texts")]
        public ulong Texts { get; init; }
    }

    /// <summary>
    /// 点を囲む、軸に沿った長方形
    /// </summary>
    public sealed record Extent
    {
        /// <summary>
        /// X の最小
        /// </summary>
        [JsonPropertyName("min_x")]
        public double MinX { get; init; }

        /// <summary>
        /// Y の最小
        /// </summary>
        [JsonPropertyName("min_y")]
        public double MinY { get; init; }

        /// <summary>
        /// X の最大
        /// </summary>
        [JsonPropertyName("max_x")]
        public double MaxX { get; init; }

        /// <summary>
        /// Y の最大
        /// </summary>
        [JsonPropertyName("max_y")]
        public double MaxY { get; init; }
    }

    /// <summary>
    /// 開いた文書を図面モデル（L2、決定 0022）に変換し、要素の一覧を返す。実寸の空間、用紙の順に、それぞれの要素を入力の順に並べる（決定 0117）
    /// </summary>
    public sealed record EntitiesRequest
    {
        /// <summary>
        /// メッセージのスキーマの大版。今は 1
        /// </summary>
        [JsonPropertyName("v")]
        public ulong V { get; init; }

        /// <summary>
        /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        /// </summary>
        [JsonPropertyName("budget")]
        public Budget? Budget { get; init; }

        /// <summary>
        /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        /// </summary>
        [JsonPropertyName("partial")]
        public bool? Partial { get; init; }

        /// <summary>
        /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        /// </summary>
        [JsonPropertyName("max_places")]
        public ulong? MaxPlaces { get; init; }

        /// <summary>
        /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        /// </summary>
        [JsonPropertyName("time_limit_ms")]
        public ulong? TimeLimitMs { get; init; }

        /// <summary>
        /// op の名前
        /// </summary>
        [JsonPropertyName("op")]
        public string Op { get; init; } = "entities";

        /// <summary>
        /// 文書の名前。open で作ったもの
        /// </summary>
        [JsonPropertyName("doc")]
        public string Doc { get; init; } = default!;

        /// <summary>
        /// 返す最初の要素の位置。前の応答の next_offset を渡す。書かなければ 0
        /// </summary>
        [JsonPropertyName("offset")]
        public ulong? Offset { get; init; }

        /// <summary>
        /// 返す要素の数の上限。書かなければ 1000
        /// </summary>
        [JsonPropertyName("limit")]
        public uint? Limit { get; init; }

        /// <summary>
        /// 返す欄の名前。1 つ以上で、重ねない。id は書かなくても返す。書かなければ id・kind・layer・source を返す（決定 0118）
        /// </summary>
        [JsonPropertyName("fields")]
        public IReadOnlyList<string>? Fields { get; init; }

        /// <summary>
        /// [最小の X, 最小の Y, 最大の X, 最大の Y]。範囲がこの長方形と重なるか接する要素だけを返す。空間か用紙それぞれの座標で比べる（決定 0118）
        /// </summary>
        [JsonPropertyName("region")]
        public IReadOnlyList<double>? Region { get; init; }

        /// <summary>
        /// 空間か用紙の ID（space/0・sheet/h22 など）。書けば、その空間か用紙の要素だけを返す。記号の定義の空間（space/h1F など。BLOCK のハンドルから作る、決定 0156）も指せ、書かなければ定義の中の要素は返さない。ない ID なら要素を返さない。小版 31 から（決定 0177）
        /// </summary>
        [JsonPropertyName("place")]
        public string? Place { get; init; }
    }

    /// <summary>
    /// 要素の一覧の一部。移さなかった図形と値は、診断で返す
    /// </summary>
    public sealed record EntitiesResult
    {
        /// <summary>
        /// offset から limit 個までの要素
        /// </summary>
        [JsonPropertyName("items")]
        public IReadOnlyList<Entity> Items { get; init; } = default!;

        /// <summary>
        /// 全体の要素の数
        /// </summary>
        [JsonPropertyName("total")]
        public ulong Total { get; init; }

        /// <summary>
        /// 続きの最初の要素の位置。続きがなければ持たない
        /// </summary>
        [JsonPropertyName("next_offset")]
        public ulong? NextOffset { get; init; }
    }

    /// <summary>
    /// 図面モデルの要素
    /// </summary>
    public sealed record Entity
    {
        /// <summary>
        /// 空間か用紙と、出所で作る ID（space/0/h2F、space/0/r12、sheet/1/b4096 など）。出所の部分は、形式が持つ鍵（ハンドル）があれば鍵で作り、書き戻しても前の要素が増えても変わらない。同じバイト列には、どの版でも同じ ID を返す（決定 0117・0156）
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 形の種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string? Kind { get; init; }

        /// <summary>
        /// レイヤの元の名前。翻訳しない。図面モデルにレイヤがなければ持たない
        /// </summary>
        [JsonPropertyName("layer")]
        public string? Layer { get; init; }

        /// <summary>
        /// 元の形式のレコード
        /// </summary>
        [JsonPropertyName("source")]
        public Source? Source { get; init; }

        /// <summary>
        /// 形の定義から求めた範囲。文字の字形・線の太さ・記号の中身は含めない。囲む点がなければ持たない（決定 0118）
        /// </summary>
        [JsonPropertyName("bbox")]
        public Extent? Bbox { get; init; }

        /// <summary>
        /// 要素のレイヤ・スタイル・形のハッシュ。出所の位置は含めない。変更の前提条件に使う（v2: と 32 桁の 16 進、決定 0156）
        /// </summary>
        [JsonPropertyName("hash")]
        public string? Hash { get; init; }

        /// <summary>
        /// 形の値。線分・円・円弧・楕円・幅を持たない折れ線・1 点か 2 点の間に置く文字・書式つきの文字・ブロックの記号・制御点か当てはめる点で決まるスプライン・寸法・引出線（LEADER）だけが持つ。apply の replace と add に渡せる形（決定 0139）。小版 10 から
        /// </summary>
        [JsonPropertyName("geometry")]
        public JsonAny? Geometry { get; init; }
    }

    /// <summary>
    /// entities で返す要素の欄（決定 0118）
    /// </summary>
    public static class EntityField
    {
        /// <summary>id</summary>
        public const string Id = "id";
        /// <summary>kind</summary>
        public const string Kind = "kind";
        /// <summary>layer</summary>
        public const string Layer = "layer";
        /// <summary>source</summary>
        public const string Source = "source";
        /// <summary>bbox</summary>
        public const string Bbox = "bbox";
        /// <summary>hash</summary>
        public const string Hash = "hash";
        /// <summary>geometry</summary>
        public const string Geometry = "geometry";
    }

    /// <summary>
    /// 形の値。点は要素の空間か用紙の座標、長さは同じ単位（実寸の空間は文書の単位、用紙は mm）、角度は度で反時計回り。shape の値で形が決まる
    /// </summary>
    public static class Geometry
    {
        /// <summary>目印の欄の名前</summary>
        public const string Tag = "shape";

        /// <summary>目印の値と、その形の型</summary>
        public static readonly IReadOnlyDictionary<string, global::System.Type> Variants = new Dictionary<string, global::System.Type>
        {
            ["line"] = typeof(LineGeometry),
            ["circle"] = typeof(CircleGeometry),
            ["arc"] = typeof(ArcGeometry),
            ["ellipse"] = typeof(EllipseGeometry),
            ["polyline"] = typeof(PolylineGeometry),
            ["text"] = typeof(TextGeometry),
            ["formatted_text"] = typeof(FormattedTextGeometry),
            ["symbol"] = typeof(SymbolGeometry),
            ["spline"] = typeof(SplineGeometry),
            ["dimension"] = typeof(DimensionGeometry),
            ["leader"] = typeof(LeaderGeometry),
            ["point"] = typeof(PointGeometry),
            ["callout"] = typeof(CalloutGeometry),
        };
    }

    /// <summary>
    /// 線分
    /// </summary>
    public sealed record LineGeometry
    {
        /// <summary>
        /// 形の種類
        /// </summary>
        [JsonPropertyName("shape")]
        public string Shape { get; init; } = "line";

        /// <summary>
        /// 始点 [X, Y]
        /// </summary>
        [JsonPropertyName("start")]
        public IReadOnlyList<double> Start { get; init; } = default!;

        /// <summary>
        /// 終点 [X, Y]
        /// </summary>
        [JsonPropertyName("end")]
        public IReadOnlyList<double> End { get; init; } = default!;
    }

    /// <summary>
    /// 円
    /// </summary>
    public sealed record CircleGeometry
    {
        /// <summary>
        /// 形の種類
        /// </summary>
        [JsonPropertyName("shape")]
        public string Shape { get; init; } = "circle";

        /// <summary>
        /// 中心 [X, Y]
        /// </summary>
        [JsonPropertyName("center")]
        public IReadOnlyList<double> Center { get; init; } = default!;

        /// <summary>
        /// 半径。0 より大きい
        /// </summary>
        [JsonPropertyName("radius")]
        public double Radius { get; init; }
    }

    /// <summary>
    /// 円弧。始まりの角度から終わりの角度へ、反時計回りにたどる
    /// </summary>
    public sealed record ArcGeometry
    {
        /// <summary>
        /// 形の種類
        /// </summary>
        [JsonPropertyName("shape")]
        public string Shape { get; init; } = "arc";

        /// <summary>
        /// 中心 [X, Y]
        /// </summary>
        [JsonPropertyName("center")]
        public IReadOnlyList<double> Center { get; init; } = default!;

        /// <summary>
        /// 半径。0 より大きい
        /// </summary>
        [JsonPropertyName("radius")]
        public double Radius { get; init; }

        /// <summary>
        /// 始まりの角度（度）
        /// </summary>
        [JsonPropertyName("start_angle")]
        public double StartAngle { get; init; }

        /// <summary>
        /// 終わりの角度（度）
        /// </summary>
        [JsonPropertyName("end_angle")]
        public double EndAngle { get; init; }
    }

    /// <summary>
    /// 楕円か楕円の弧。始まりの角度から終わりの角度へ、反時計回りにたどる。閉じた楕円は 0 度から 360 度。小版 11 から
    /// </summary>
    public sealed record EllipseGeometry
    {
        /// <summary>
        /// 形の種類
        /// </summary>
        [JsonPropertyName("shape")]
        public string Shape { get; init; } = "ellipse";

        /// <summary>
        /// 中心 [X, Y]
        /// </summary>
        [JsonPropertyName("center")]
        public IReadOnlyList<double> Center { get; init; } = default!;

        /// <summary>
        /// 長軸の端の点 [X, Y]。中心と違う点
        /// </summary>
        [JsonPropertyName("major")]
        public IReadOnlyList<double> Major { get; init; } = default!;

        /// <summary>
        /// 短軸の長軸に対する比。0 より大きく 1 以下
        /// </summary>
        [JsonPropertyName("ratio")]
        public double Ratio { get; init; }

        /// <summary>
        /// 始まりの角度（度）。楕円を円に引き延ばしたときの角度
        /// </summary>
        [JsonPropertyName("start_angle")]
        public double StartAngle { get; init; }

        /// <summary>
        /// 終わりの角度（度）。楕円を円に引き延ばしたときの角度
        /// </summary>
        [JsonPropertyName("end_angle")]
        public double EndAngle { get; init; }
    }

    /// <summary>
    /// 頂点を結んだ折れ線。幅は持たない
    /// </summary>
    public sealed record PolylineGeometry
    {
        /// <summary>
        /// 形の種類
        /// </summary>
        [JsonPropertyName("shape")]
        public string Shape { get; init; } = "polyline";

        /// <summary>
        /// 頂点の [X, Y] の並び。2 つ以上
        /// </summary>
        [JsonPropertyName("points")]
        public IReadOnlyList<IReadOnlyList<double>> Points { get; init; } = default!;

        /// <summary>
        /// 頂点ごとの、次の頂点への弧の膨らみ（中心角の 4 分の 1 の正接。正は反時計回り）。points と同じ数。書かなければどれも 0
        /// </summary>
        [JsonPropertyName("bulges")]
        public IReadOnlyList<double>? Bulges { get; init; }

        /// <summary>
        /// 最後の頂点から最初の頂点へ結ぶ
        /// </summary>
        [JsonPropertyName("closed")]
        public bool Closed { get; init; }
    }

    /// <summary>
    /// 1 点か 2 点の間に置く文字。置き換えるときは、位置合わせ・幅の倍率・斜体・反転・文字スタイルを元の値のままにする。足すときは、2 点目がなければ左の基準線に合わせ、幅の倍率 1・斜体なし・反転なしにする
    /// </summary>
    public sealed record TextGeometry
    {
        /// <summary>
        /// 形の種類
        /// </summary>
        [JsonPropertyName("shape")]
        public string Shape { get; init; } = "text";

        /// <summary>
        /// 内容。1 行
        /// </summary>
        [JsonPropertyName("content")]
        public string Content { get; init; } = default!;

        /// <summary>
        /// 基点 [X, Y]。2 点の間に合わせる文字では 1 点目
        /// </summary>
        [JsonPropertyName("position")]
        public IReadOnlyList<double> Position { get; init; } = default!;

        /// <summary>
        /// 2 点の間に合わせる文字の 2 点目 [X, Y]。合わせ方（fit）と一緒に書く。置き換えでは、1 点に置く文字と 2 点の間に合わせる文字を入れ替えられない。小版 17 から
        /// </summary>
        [JsonPropertyName("end")]
        public IReadOnlyList<double>? End { get; init; }

        /// <summary>
        /// 2 点の間への合わせ方。2 点目（end）と一緒に書く。小版 17 から
        /// </summary>
        [JsonPropertyName("fit")]
        public string? Fit { get; init; }

        /// <summary>
        /// 高さ。0 より大きい
        /// </summary>
        [JsonPropertyName("height")]
        public double Height { get; init; }

        /// <summary>
        /// 回転（度）。2 点の間に合わせる文字では、1 点目から 2 点目への向きと同じでなければならない
        /// </summary>
        [JsonPropertyName("rotation")]
        public double Rotation { get; init; }

        /// <summary>
        /// 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。DXF に文字スタイルの定義がないときは Standard だけを受ける。置き換えるときは、元と同じ名前だけを受ける。style_id と一緒に書くなら、同じ文字スタイルを指す。小版 12 から
        /// </summary>
        [JsonPropertyName("style")]
        public string? Style { get; init; }

        /// <summary>
        /// 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。小版 23 から
        /// </summary>
        [JsonPropertyName("style_id")]
        public string? StyleId { get; init; }
    }

    /// <summary>
    /// 2 点の間への合わせ方。aligned は縦横の比を保って高さも変え、fit は高さを保って幅だけを変える
    /// </summary>
    public static class TextFit
    {
        /// <summary>aligned</summary>
        public const string Aligned = "aligned";
        /// <summary>fit</summary>
        public const string Fit = "fit";
    }

    /// <summary>
    /// 書式つきの文字（DXF の MTEXT）。置き換えるときは、枠の高さ・書き進める向き・行送り・背景・高さの意味・文字スタイルを元の値のままにする。足すときは、枠の高さなし・左から右・行送り 1 倍・背景なしにする。小版 13 から
    /// </summary>
    public sealed record FormattedTextGeometry
    {
        /// <summary>
        /// 形の種類
        /// </summary>
        [JsonPropertyName("shape")]
        public string Shape { get; init; } = "formatted_text";

        /// <summary>
        /// 内容。書式の符号（\P の改行など）を含めた、元の形式の内容のまま
        /// </summary>
        [JsonPropertyName("content")]
        public string Content { get; init; } = default!;

        /// <summary>
        /// 基点 [X, Y]
        /// </summary>
        [JsonPropertyName("position")]
        public IReadOnlyList<double> Position { get; init; } = default!;

        /// <summary>
        /// 高さ。0 より大きい
        /// </summary>
        [JsonPropertyName("height")]
        public double Height { get; init; }

        /// <summary>
        /// 回転（度）
        /// </summary>
        [JsonPropertyName("rotation")]
        public double Rotation { get; init; }

        /// <summary>
        /// 基点に合わせる枠の位置
        /// </summary>
        [JsonPropertyName("attachment")]
        public string? Attachment { get; init; }

        /// <summary>
        /// 折り返す枠の幅。0 より大きい。書かなければ折り返さない
        /// </summary>
        [JsonPropertyName("width")]
        public double? Width { get; init; }

        /// <summary>
        /// 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。置き換えるときは、元と同じ名前だけを受ける。style_id と一緒に書くなら、同じ文字スタイルを指す
        /// </summary>
        [JsonPropertyName("style")]
        public string? Style { get; init; }

        /// <summary>
        /// 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。小版 23 から
        /// </summary>
        [JsonPropertyName("style_id")]
        public string? StyleId { get; init; }
    }

    /// <summary>
    /// スプライン（NURBS）。制御点で決まるもの（小版 15 から）と、当てはめる点で決まるもの（小版 18 から）がある。当てはめる点で決まるものは、節点と制御点を空にし、当てはめる点（fit）と、あれば両端の接線を持つ。元の図形に制御点や節点もあるときは形の値を返さず、置き換えない。置き換えでは、制御点で決まるものと当てはめる点で決まるものを入れ替えない
    /// </summary>
    public sealed record SplineGeometry
    {
        /// <summary>
        /// 形の種類
        /// </summary>
        [JsonPropertyName("shape")]
        public string Shape { get; init; } = "spline";

        /// <summary>
        /// 次数。1 以上
        /// </summary>
        [JsonPropertyName("degree")]
        public uint Degree { get; init; }

        /// <summary>
        /// 節点の並び。周期的でなければ、制御点の数と次数と 1 の和の数。当てはめる点で決まるものは空
        /// </summary>
        [JsonPropertyName("knots")]
        public IReadOnlyList<double> Knots { get; init; } = default!;

        /// <summary>
        /// 制御点の [X, Y] の並び。次数より多い。当てはめる点で決まるものは空
        /// </summary>
        [JsonPropertyName("control")]
        public IReadOnlyList<IReadOnlyList<double>> Control { get; init; } = default!;

        /// <summary>
        /// 制御点ごとの重み。0 より大きい。有理のスプラインだけが持ち、書かなければ有理でない
        /// </summary>
        [JsonPropertyName("weights")]
        public IReadOnlyList<double>? Weights { get; init; }

        /// <summary>
        /// 当てはめる点の [X, Y] の並び。2 つ以上。書けば、節点と制御点は空で、重みは書かない。小版 18 から
        /// </summary>
        [JsonPropertyName("fit")]
        public IReadOnlyList<IReadOnlyList<double>>? Fit { get; init; }

        /// <summary>
        /// 当てはめる点で決まるスプラインの、始まりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。小版 18 から
        /// </summary>
        [JsonPropertyName("start_tangent")]
        public IReadOnlyList<double>? StartTangent { get; init; }

        /// <summary>
        /// 当てはめる点で決まるスプラインの、終わりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。小版 18 から
        /// </summary>
        [JsonPropertyName("end_tangent")]
        public IReadOnlyList<double>? EndTangent { get; init; }

        /// <summary>
        /// 閉じている
        /// </summary>
        [JsonPropertyName("closed")]
        public bool Closed { get; init; }

        /// <summary>
        /// 周期的
        /// </summary>
        [JsonPropertyName("periodic")]
        public bool Periodic { get; init; }
    }

    /// <summary>
    /// 寸法。種類（kind）ごとに、その種類の定義点の欄を持つ。測定値（measurement）は元の形式が保存した値で、定義点から測り直さない。寸法を描いた部品（ブロック）は形の値に含めない。entities が返すだけで、置き換えと追加は change.edit.unsupported になる（寸法を描いた部品を作り直す必要があるため）。小版 19 から
    /// </summary>
    public sealed record DimensionGeometry
    {
        /// <summary>
        /// 形の種類
        /// </summary>
        [JsonPropertyName("shape")]
        public string Shape { get; init; } = "dimension";

        /// <summary>
        /// 寸法の種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = default!;

        /// <summary>
        /// 1 本目の補助線の点か、測る角度の 1 本目の線の点か、円弧の始めの点 [X, Y]。linear・aligned・angular・arc_length が持つ
        /// </summary>
        [JsonPropertyName("first")]
        public IReadOnlyList<double>? First { get; init; }

        /// <summary>
        /// 2 本目の補助線の点か、測る角度の 2 本目の線の点か、円弧の終わりの点 [X, Y]。linear・aligned・angular・arc_length が持つ
        /// </summary>
        [JsonPropertyName("second")]
        public IReadOnlyList<double>? Second { get; init; }

        /// <summary>
        /// 寸法線が通る点 [X, Y]。linear・aligned が持つ
        /// </summary>
        [JsonPropertyName("line")]
        public IReadOnlyList<double>? Line { get; init; }

        /// <summary>
        /// 測る向き（度）。linear が持つ
        /// </summary>
        [JsonPropertyName("rotation")]
        public double? Rotation { get; init; }

        /// <summary>
        /// 補助線の傾き（度）。傾けなければ 0。linear・aligned が持つ
        /// </summary>
        [JsonPropertyName("oblique")]
        public double? Oblique { get; init; }

        /// <summary>
        /// 角度の頂点 [X, Y]。angular が持つ
        /// </summary>
        [JsonPropertyName("vertex")]
        public IReadOnlyList<double>? Vertex { get; init; }

        /// <summary>
        /// 1 本目の線の始点と終点 [[X, Y], [X, Y]]。angular_lines が持つ
        /// </summary>
        [JsonPropertyName("first_line")]
        public IReadOnlyList<IReadOnlyList<double>>? FirstLine { get; init; }

        /// <summary>
        /// 2 本目の線の始点と終点 [[X, Y], [X, Y]]。angular_lines が持つ
        /// </summary>
        [JsonPropertyName("second_line")]
        public IReadOnlyList<IReadOnlyList<double>>? SecondLine { get; init; }

        /// <summary>
        /// 寸法の弧が通る点 [X, Y]。angular・angular_lines・arc_length が持つ
        /// </summary>
        [JsonPropertyName("arc")]
        public IReadOnlyList<double>? Arc { get; init; }

        /// <summary>
        /// 中心 [X, Y]。arc_length・radius が持つ
        /// </summary>
        [JsonPropertyName("center")]
        public IReadOnlyList<double>? Center { get; init; }

        /// <summary>
        /// 円の上の点 [X, Y]。radius・diameter が持つ
        /// </summary>
        [JsonPropertyName("chord")]
        public IReadOnlyList<double>? Chord { get; init; }

        /// <summary>
        /// chord と向かい合う円の上の点 [X, Y]。diameter が持つ
        /// </summary>
        [JsonPropertyName("opposite")]
        public IReadOnlyList<double>? Opposite { get; init; }

        /// <summary>
        /// 引出線の長さ。radius・diameter が持つ
        /// </summary>
        [JsonPropertyName("leader")]
        public double? Leader { get; init; }

        /// <summary>
        /// 原点 [X, Y]。ordinate が持つ
        /// </summary>
        [JsonPropertyName("origin")]
        public IReadOnlyList<double>? Origin { get; init; }

        /// <summary>
        /// 測る点 [X, Y]。ordinate が持つ
        /// </summary>
        [JsonPropertyName("feature")]
        public IReadOnlyList<double>? Feature { get; init; }

        /// <summary>
        /// 引出線の終点 [X, Y]。ordinate が持つ
        /// </summary>
        [JsonPropertyName("leader_end")]
        public IReadOnlyList<double>? LeaderEnd { get; init; }

        /// <summary>
        /// 測る座標の軸。ordinate が持つ
        /// </summary>
        [JsonPropertyName("axis")]
        public string? Axis { get; init; }

        /// <summary>
        /// 寸法値の文字の書き方
        /// </summary>
        [JsonPropertyName("label")]
        public string Label { get; init; } = default!;

        /// <summary>
        /// 利用者が書いた寸法値の文字の元の内容。label が override のときだけ持つ。測定値に置き換える符号（DXF の &lt;&gt;）と書式の符号もそのまま持つ
        /// </summary>
        [JsonPropertyName("content")]
        public string? Content { get; init; }

        /// <summary>
        /// 元の形式が保存した測定値。単位と倍率は measurement_key の欄に従う
        /// </summary>
        [JsonPropertyName("measurement")]
        public double? Measurement { get; init; }

        /// <summary>
        /// measurement の形式と欄を指すキー（DXF の DIMENSION の 42 なら dxf.DIMENSION.42）。measurement と一緒に持つ
        /// </summary>
        [JsonPropertyName("measurement_key")]
        public string? MeasurementKey { get; init; }

        /// <summary>
        /// 寸法値の文字の中央の点 [X, Y]。元の形式が持たなければ書かない
        /// </summary>
        [JsonPropertyName("text_position")]
        public IReadOnlyList<double>? TextPosition { get; init; }

        /// <summary>
        /// 寸法値の文字の回転（度）。寸法線に沿わせるなら書かない
        /// </summary>
        [JsonPropertyName("text_rotation")]
        public double? TextRotation { get; init; }

        /// <summary>
        /// 寸法スタイルの元の名前
        /// </summary>
        [JsonPropertyName("style")]
        public string Style { get; init; } = default!;

        /// <summary>
        /// 寸法スタイルの ID。tables が返したもの。名前で引いた寸法スタイルがなければ書かない。小版 23 から
        /// </summary>
        [JsonPropertyName("style_id")]
        public string? StyleId { get; init; }
    }

    /// <summary>
    /// 引出線（DXF・DWG の LEADER、決定 0177）。注記（MTEXT など）は別の要素で、annotation がその ID を指す。置き換えるときは寸法スタイルと注記を変えず、経路・曲線か・矢印を書き直す。足すときの注記は、同じ空間か用紙の MTEXT だけを指せる（小版 36 から。それまでは注記を持てなかった）。DWG は置き換えと追加を書き戻すが、矢印の印を必ず持つので、arrow を書かない引出線は書けない（小版 36 から。それまでは R2004 までの置き換えだけ）。小版 31 から
    /// </summary>
    public sealed record LeaderGeometry
    {
        /// <summary>
        /// 形の種類
        /// </summary>
        [JsonPropertyName("shape")]
        public string Shape { get; init; } = "leader";

        /// <summary>
        /// 経路の頂点 [[X, Y], ...]。矢印の側から並べる。2 つ以上
        /// </summary>
        [JsonPropertyName("path")]
        public IReadOnlyList<IReadOnlyList<double>> Path { get; init; } = default!;

        /// <summary>
        /// 頂点を通る曲線で結ぶか。偽なら折れ線
        /// </summary>
        [JsonPropertyName("spline")]
        public bool Spline { get; init; }

        /// <summary>
        /// 始点に矢印を付けるか。書かなければ寸法スタイルに従う
        /// </summary>
        [JsonPropertyName("arrow")]
        public bool? Arrow { get; init; }

        /// <summary>
        /// 注記の要素の ID。同じ空間か用紙の要素で、entities が返したもの。注記がなければ書かない。置き換えるときは元と同じ ID だけを受ける。足すときは、同じ空間か用紙の書式つきの文字（MTEXT）を指す
        /// </summary>
        [JsonPropertyName("annotation")]
        public string? Annotation { get; init; }

        /// <summary>
        /// 寸法スタイルの元の名前。足すときは図面にある寸法スタイルの名前を指す。置き換えるときは元と同じ名前だけを受ける
        /// </summary>
        [JsonPropertyName("style")]
        public string Style { get; init; } = default!;

        /// <summary>
        /// 寸法スタイルの ID。tables が返したもの。entities は、名前で引いた寸法スタイルの ID を返す。足す・置き換えるときは、名前と同じ寸法スタイルを指す ID だけを受ける
        /// </summary>
        [JsonPropertyName("style_id")]
        public string? StyleId { get; init; }
    }

    /// <summary>
    /// 引出線つきの注記（DXF・DWG の MULTILEADER、決定 0185）。注記は形の中にあり、content と text・symbol で種類と中身を示す。置き換えるときは枝の線の頂点・集まる点・折れ曲がりだけを変えられ、ほかの欄は元の値のままにする。DXF だけが置き換えを書き戻し、枝と線の数と、集まる点と折れ曲がりの有無は元と同じでなければならない。足すことと DWG の書き戻しは、まだしない。小版 36 から
    /// </summary>
    public sealed record CalloutGeometry
    {
        /// <summary>
        /// 形の種類
        /// </summary>
        [JsonPropertyName("shape")]
        public string Shape { get; init; } = "callout";

        /// <summary>
        /// 枝。入力の順
        /// </summary>
        [JsonPropertyName("branches")]
        public IReadOnlyList<CalloutBranch> Branches { get; init; } = default!;

        /// <summary>
        /// 線の引き方
        /// </summary>
        [JsonPropertyName("path")]
        public string Path { get; init; } = default!;

        /// <summary>
        /// 線を注記に、水平の線（ランディング）でつなぐか
        /// </summary>
        [JsonPropertyName("landing")]
        public bool Landing { get; init; }

        /// <summary>
        /// 端部記号の大きさ
        /// </summary>
        [JsonPropertyName("arrow_size")]
        public double ArrowSize { get; init; }

        /// <summary>
        /// 大きさに掛ける倍率
        /// </summary>
        [JsonPropertyName("scale")]
        public double Scale { get; init; }

        /// <summary>
        /// 注記の種類
        /// </summary>
        [JsonPropertyName("content")]
        public string Content { get; init; } = default!;

        /// <summary>
        /// 注記の書式つきの文字の元の内容。content が text のときだけ持つ
        /// </summary>
        [JsonPropertyName("text")]
        public string? Text { get; init; }

        /// <summary>
        /// 注記の記号のブロックの名前。content が symbol のときだけ持つ
        /// </summary>
        [JsonPropertyName("symbol")]
        public string? Symbol { get; init; }
    }

    /// <summary>
    /// 引出線つきの注記の枝。何本かの線が 1 点に集まり、折れ曲がりで注記につながる
    /// </summary>
    public sealed record CalloutBranch
    {
        /// <summary>
        /// 線ごとの頂点 [[[X, Y], ...], ...]。矢印の側から並べ、集まる点は含めない。線ごとに 1 つ以上
        /// </summary>
        [JsonPropertyName("lines")]
        public IReadOnlyList<IReadOnlyList<IReadOnlyList<double>>> Lines { get; init; } = default!;

        /// <summary>
        /// 線が集まる点 [X, Y]。元の形式が持たなければ書かない
        /// </summary>
        [JsonPropertyName("end")]
        public IReadOnlyList<double>? End { get; init; }

        /// <summary>
        /// 集まる点から注記への折れ曲がり。なければ書かない
        /// </summary>
        [JsonPropertyName("dogleg")]
        public CalloutDogleg? Dogleg { get; init; }
    }

    /// <summary>
    /// 枝の折れ曲がり
    /// </summary>
    public sealed record CalloutDogleg
    {
        /// <summary>
        /// 向き [X, Y]。長さは意味を持たない
        /// </summary>
        [JsonPropertyName("direction")]
        public IReadOnlyList<double> Direction { get; init; } = default!;

        /// <summary>
        /// 長さ
        /// </summary>
        [JsonPropertyName("length")]
        public double Length { get; init; }
    }

    /// <summary>
    /// 引出線つきの注記の線の引き方。straight は折れ線、spline は頂点を通る曲線、hidden は線を描かない
    /// </summary>
    public static class CalloutPath
    {
        /// <summary>straight</summary>
        public const string Straight = "straight";
        /// <summary>spline</summary>
        public const string Spline = "spline";
        /// <summary>hidden</summary>
        public const string Hidden = "hidden";
    }

    /// <summary>
    /// 引出線つきの注記の中身の種類。none は注記を持たず、text は書式つきの文字、symbol は記号（バルーンなどの囲み）
    /// </summary>
    public static class CalloutContent
    {
        /// <summary>none</summary>
        public const string None = "none";
        /// <summary>text</summary>
        public const string Text = "text";
        /// <summary>symbol</summary>
        public const string Symbol = "symbol";
    }

    /// <summary>
    /// 点（決定 0160・0183）。marker がなければ文書の点の表し方（DXF・DWG の $PDMODE・$PDSIZE）に従う点で、DXF・DWG の POINT と JWW の記号でない点が当たる。marker があれば既定義の印の点で、SFC の点マーカーと JWW の記号の点が当たる。置き換えるときは、描き方（marker があるか）を元のまま保つ。DXF・DWG は marker と construction を持つ点を書かず、JWW の記号でない点は角度 0 だけを、SXF は SFC の点マーカーだけを書く。小版 35 から
    /// </summary>
    public sealed record PointGeometry
    {
        /// <summary>
        /// 形の種類
        /// </summary>
        [JsonPropertyName("shape")]
        public string Shape { get; init; } = "point";

        /// <summary>
        /// 位置 [X, Y]
        /// </summary>
        [JsonPropertyName("position")]
        public IReadOnlyList<double> Position { get; init; } = default!;

        /// <summary>
        /// 回転（度）
        /// </summary>
        [JsonPropertyName("rotation")]
        public double Rotation { get; init; }

        /// <summary>
        /// 既定義の印。書かなければ文書の点の表し方に従う
        /// </summary>
        [JsonPropertyName("marker")]
        public PointMarker? Marker { get; init; }

        /// <summary>
        /// 作図の補助の点（JWW の仮点）か。書かなければ偽
        /// </summary>
        [JsonPropertyName("construction")]
        public bool? Construction { get; init; }
    }

    /// <summary>
    /// 既定義の印の表のキーと尺度。印の形は、表示する側がキーから引く
    /// </summary>
    public sealed record PointMarker
    {
        /// <summary>
        /// 印の表の名前空間。sxf.marker（SFC の点マーカー）か jww.point（JWW の記号の点）
        /// </summary>
        [JsonPropertyName("namespace")]
        public string Namespace { get; init; } = default!;

        /// <summary>
        /// 名前空間の中の元のコード
        /// </summary>
        [JsonPropertyName("code")]
        public string Code { get; init; } = default!;

        /// <summary>
        /// 尺度
        /// </summary>
        [JsonPropertyName("scale")]
        public double Scale { get; init; }
    }

    /// <summary>
    /// 寸法の種類。linear は 2 点を測る向きに、aligned は 2 点を結ぶ向きに測った長さ。angular は頂点から 2 点へ向かう線の、angular_lines は 2 本の線のなす角度。arc_length は円弧の長さ。radius は半径、diameter は直径。ordinate は原点からの X か Y の座標
    /// </summary>
    public static class DimensionKind
    {
        /// <summary>linear</summary>
        public const string Linear = "linear";
        /// <summary>aligned</summary>
        public const string Aligned = "aligned";
        /// <summary>angular</summary>
        public const string Angular = "angular";
        /// <summary>angular_lines</summary>
        public const string AngularLines = "angular_lines";
        /// <summary>arc_length</summary>
        public const string ArcLength = "arc_length";
        /// <summary>radius</summary>
        public const string Radius = "radius";
        /// <summary>diameter</summary>
        public const string Diameter = "diameter";
        /// <summary>ordinate</summary>
        public const string Ordinate = "ordinate";
    }

    /// <summary>
    /// 座標の軸
    /// </summary>
    public static class DimensionAxis
    {
        /// <summary>x</summary>
        public const string X = "x";
        /// <summary>y</summary>
        public const string Y = "y";
    }

    /// <summary>
    /// 寸法値の文字の書き方。measurement は測定値を書き、override は利用者が書いた文字（content）を書き、hidden は書かない
    /// </summary>
    public static class DimensionLabel
    {
        /// <summary>measurement</summary>
        public const string Measurement = "measurement";
        /// <summary>override</summary>
        public const string Override = "override";
        /// <summary>hidden</summary>
        public const string Hidden = "hidden";
    }

    /// <summary>
    /// 記号（DXF・DWG のブロックの INSERT）。定義の点を、尺度・回転・移動の順に写す。切り抜きは形の値に含めない。置き換えるときは切り抜きを元のままにし、属性は値だけを変えられる。属性を持つ記号の置く点・尺度・回転は変えない（属性ごと動かすなら transform を使う）。足すときは切り抜きを持たない。小版 14 から
    /// </summary>
    public sealed record SymbolGeometry
    {
        /// <summary>
        /// 形の種類
        /// </summary>
        [JsonPropertyName("shape")]
        public string Shape { get; init; } = "symbol";

        /// <summary>
        /// 記号の定義（ブロック）の元の名前。足すときは図面にある定義の名前を指す
        /// </summary>
        [JsonPropertyName("name")]
        public string Name { get; init; } = default!;

        /// <summary>
        /// 定義の基点を置く点 [X, Y]
        /// </summary>
        [JsonPropertyName("position")]
        public IReadOnlyList<double> Position { get; init; } = default!;

        /// <summary>
        /// X と Y の尺度 [X, Y]。0 でない。負なら反転する
        /// </summary>
        [JsonPropertyName("scale")]
        public IReadOnlyList<double> Scale { get; init; } = default!;

        /// <summary>
        /// 回転（度）
        /// </summary>
        [JsonPropertyName("rotation")]
        public double Rotation { get; init; }

        /// <summary>
        /// 格子に並べて置く列と行。書かなければ 1 つだけ置く
        /// </summary>
        [JsonPropertyName("grid")]
        public SymbolGrid? Grid { get; init; }

        /// <summary>
        /// 属性のタグと値。entities は記号の属性を入力の順に返す。足すときは、定義の属性の定義（値が決まったものを除く）から属性を作り、ここに書いたタグの値で既定の値を替える。タグは大文字と小文字を区別せず、重複を受けない。必要な属性を作れなければ失敗する。置き換えるときは、元と同じ数・順・タグを受け、値を変えた属性の ATTRIB の値を書き直す（表題欄の項目の書き換えなど）。書式つきの値（複数行の属性）を持つ属性の値は変えられない。小版 16 から。置き換えで値を変えるのは小版 21 から
        /// </summary>
        [JsonPropertyName("attributes")]
        public IReadOnlyList<SymbolAttribute>? Attributes { get; init; }
    }

    /// <summary>
    /// 記号の属性のタグと値
    /// </summary>
    public sealed record SymbolAttribute
    {
        /// <summary>
        /// 元のタグ
        /// </summary>
        [JsonPropertyName("tag")]
        public string Tag { get; init; } = default!;

        /// <summary>
        /// 値。1 行
        /// </summary>
        [JsonPropertyName("value")]
        public string Value { get; init; } = default!;
    }

    /// <summary>
    /// 記号を格子に並べる列と行。間隔は、記号を回転する前の向きに測る。置き換えるときは、列と行の数を変えない
    /// </summary>
    public sealed record SymbolGrid
    {
        /// <summary>
        /// 列の数。1 以上
        /// </summary>
        [JsonPropertyName("columns")]
        public uint Columns { get; init; }

        /// <summary>
        /// 行の数。1 以上。列と行がどちらも 1 の格子は持たない
        /// </summary>
        [JsonPropertyName("rows")]
        public uint Rows { get; init; }

        /// <summary>
        /// 列の間隔
        /// </summary>
        [JsonPropertyName("column_spacing")]
        public double ColumnSpacing { get; init; }

        /// <summary>
        /// 行の間隔
        /// </summary>
        [JsonPropertyName("row_spacing")]
        public double RowSpacing { get; init; }
    }

    /// <summary>
    /// 基点に合わせる枠の位置。entities は返し、足すときに書かなければ左上、置き換えるときに書かなければ元のまま
    /// </summary>
    public static class Attachment
    {
        /// <summary>top_left</summary>
        public const string TopLeft = "top_left";
        /// <summary>top_center</summary>
        public const string TopCenter = "top_center";
        /// <summary>top_right</summary>
        public const string TopRight = "top_right";
        /// <summary>middle_left</summary>
        public const string MiddleLeft = "middle_left";
        /// <summary>middle_center</summary>
        public const string MiddleCenter = "middle_center";
        /// <summary>middle_right</summary>
        public const string MiddleRight = "middle_right";
        /// <summary>bottom_left</summary>
        public const string BottomLeft = "bottom_left";
        /// <summary>bottom_center</summary>
        public const string BottomCenter = "bottom_center";
        /// <summary>bottom_right</summary>
        public const string BottomRight = "bottom_right";
    }

    /// <summary>
    /// 要素の形の種類（決定 0022）。point（点、決定 0160）は小版 32 から。それまで点は symbol だった
    /// </summary>
    public static class EntityKind
    {
        /// <summary>line</summary>
        public const string Line = "line";
        /// <summary>circle</summary>
        public const string Circle = "circle";
        /// <summary>curve</summary>
        public const string Curve = "curve";
        /// <summary>polyline</summary>
        public const string Polyline = "polyline";
        /// <summary>text</summary>
        public const string Text = "text";
        /// <summary>formatted_text</summary>
        public const string FormattedText = "formatted_text";
        /// <summary>dimension</summary>
        public const string Dimension = "dimension";
        /// <summary>leader</summary>
        public const string Leader = "leader";
        /// <summary>callout</summary>
        public const string Callout = "callout";
        /// <summary>table</summary>
        public const string Table = "table";
        /// <summary>underlay</summary>
        public const string Underlay = "underlay";
        /// <summary>symbol</summary>
        public const string Symbol = "symbol";
        /// <summary>hatch</summary>
        public const string Hatch = "hatch";
        /// <summary>point</summary>
        public const string Point = "point";
    }

    /// <summary>
    /// 開いた文書に、変更の一覧を当てて書き戻す（決定 0139・0156）。要素は、entities が返す ID とハッシュで指す。どれか 1 つでも当てられなければ、何も変えない。当てたら、文書を書き戻したものに替え、書き戻したファイルを blobs で返す。小版 7 から
    /// </summary>
    public sealed record ApplyRequest
    {
        /// <summary>
        /// メッセージのスキーマの大版。今は 1
        /// </summary>
        [JsonPropertyName("v")]
        public ulong V { get; init; }

        /// <summary>
        /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        /// </summary>
        [JsonPropertyName("budget")]
        public Budget? Budget { get; init; }

        /// <summary>
        /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        /// </summary>
        [JsonPropertyName("partial")]
        public bool? Partial { get; init; }

        /// <summary>
        /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        /// </summary>
        [JsonPropertyName("max_places")]
        public ulong? MaxPlaces { get; init; }

        /// <summary>
        /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        /// </summary>
        [JsonPropertyName("time_limit_ms")]
        public ulong? TimeLimitMs { get; init; }

        /// <summary>
        /// op の名前
        /// </summary>
        [JsonPropertyName("op")]
        public string Op { get; init; } = "apply";

        /// <summary>
        /// 文書の名前。open で作ったもの
        /// </summary>
        [JsonPropertyName("doc")]
        public string Doc { get; init; } = default!;

        /// <summary>
        /// 読んだときの文書のハッシュ（drawing の hash）。書けば、今の文書と同じときだけ当てる
        /// </summary>
        [JsonPropertyName("hash")]
        public string? Hash { get; init; }

        /// <summary>
        /// 変更。並べた順に当てる
        /// </summary>
        [JsonPropertyName("edits")]
        public IReadOnlyList<JsonAny> Edits { get; init; } = default!;
    }

    /// <summary>
    /// 当てた変更の結果。書き戻したファイルは blobs の file で返す
    /// </summary>
    public sealed record ApplyResult
    {
        /// <summary>
        /// 変更ごとの結果。要求の edits と同じ順
        /// </summary>
        [JsonPropertyName("applied")]
        public IReadOnlyList<Applied> Applied { get; init; } = default!;

        /// <summary>
        /// 書き戻した文書のハッシュ。次の apply の前提条件に使う
        /// </summary>
        [JsonPropertyName("hash")]
        public string Hash { get; init; } = default!;

        /// <summary>
        /// true なら、変えていない要素の ID がずれた。鍵を持たない形式（JWW）で、前の図形を除くか足したとき。ID を使う前に entities で読み直す。ずれないときは持たない。小版 30 から
        /// </summary>
        [JsonPropertyName("ids_shifted")]
        public bool? IdsShifted { get; init; }
    }

    /// <summary>
    /// 開いた文書の図面モデルを、別の形式の新しいファイルに書き、新しい文書として開く（決定 0139・0206）。書けない要素は落とし、種類ごとの件数を診断で返す。書いたファイルは blobs で返す。今は DXF にだけ書ける。小版 41 から
    /// </summary>
    public sealed record ConvertRequest
    {
        /// <summary>
        /// メッセージのスキーマの大版。今は 1
        /// </summary>
        [JsonPropertyName("v")]
        public ulong V { get; init; }

        /// <summary>
        /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        /// </summary>
        [JsonPropertyName("budget")]
        public Budget? Budget { get; init; }

        /// <summary>
        /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        /// </summary>
        [JsonPropertyName("partial")]
        public bool? Partial { get; init; }

        /// <summary>
        /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        /// </summary>
        [JsonPropertyName("max_places")]
        public ulong? MaxPlaces { get; init; }

        /// <summary>
        /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        /// </summary>
        [JsonPropertyName("time_limit_ms")]
        public ulong? TimeLimitMs { get; init; }

        /// <summary>
        /// op の名前
        /// </summary>
        [JsonPropertyName("op")]
        public string Op { get; init; } = "convert";

        /// <summary>
        /// 書く元の文書の名前。open で作ったもの
        /// </summary>
        [JsonPropertyName("doc")]
        public string Doc { get; init; } = default!;

        /// <summary>
        /// 書く形式の名前（capabilities の formats の id）。今は dxf だけ
        /// </summary>
        [JsonPropertyName("format")]
        public string Format { get; init; } = default!;

        /// <summary>
        /// 作る文書の名前
        /// </summary>
        [JsonPropertyName("to")]
        public string To { get; init; } = default!;
    }

    /// <summary>
    /// 書いた文書。書いたファイルは blobs の file で返す
    /// </summary>
    public sealed record ConvertResult
    {
        /// <summary>
        /// 作った文書の名前
        /// </summary>
        [JsonPropertyName("doc")]
        public string Doc { get; init; } = default!;

        /// <summary>
        /// 書いた形式の名前
        /// </summary>
        [JsonPropertyName("format")]
        public string Format { get; init; } = default!;

        /// <summary>
        /// 書いたファイルのハッシュ。apply の前提条件に使う
        /// </summary>
        [JsonPropertyName("hash")]
        public string Hash { get; init; } = default!;

        /// <summary>
        /// 書いた要素の数。記号から離した属性の文字を含む
        /// </summary>
        [JsonPropertyName("elements")]
        public ulong Elements { get; init; }
    }

    /// <summary>
    /// 当てた変更 1 つの結果
    /// </summary>
    public sealed record Applied
    {
        /// <summary>
        /// 変えた要素（レイヤの変更では、そのレイヤ）の、書き戻した後の ID。除いたときと、ID を決められないときは持たない
        /// </summary>
        [JsonPropertyName("id")]
        public string? Id { get; init; }

        /// <summary>
        /// 変えた要素（レイヤ）の、書き戻した後のハッシュ。id があるときに持つ
        /// </summary>
        [JsonPropertyName("hash")]
        public string? Hash { get; init; }
    }

    /// <summary>
    /// 要素 1 つへの変更。kind の値で形が決まる
    /// </summary>
    public static class Edit
    {
        /// <summary>目印の欄の名前</summary>
        public const string Tag = "kind";

        /// <summary>目印の値と、その形の型</summary>
        public static readonly IReadOnlyDictionary<string, global::System.Type> Variants = new Dictionary<string, global::System.Type>
        {
            ["restyle"] = typeof(RestyleEdit),
            ["transform"] = typeof(TransformEdit),
            ["remove"] = typeof(RemoveEdit),
            ["replace"] = typeof(ReplaceEdit),
            ["add"] = typeof(AddEdit),
            ["layer"] = typeof(LayerEdit),
            ["add_layer"] = typeof(AddLayerEdit),
            ["remove_layer"] = typeof(RemoveLayerEdit),
            ["text_style"] = typeof(TextStyleEdit),
            ["add_text_style"] = typeof(AddTextStyleEdit),
            ["remove_text_style"] = typeof(RemoveTextStyleEdit),
            ["line_type"] = typeof(LineTypeEdit),
            ["add_line_type"] = typeof(AddLineTypeEdit),
            ["remove_line_type"] = typeof(RemoveLineTypeEdit),
            ["dimension_style"] = typeof(DimensionStyleEdit),
            ["add_dimension_style"] = typeof(AddDimensionStyleEdit),
            ["remove_dimension_style"] = typeof(RemoveDimensionStyleEdit),
            ["point_style"] = typeof(PointStyleEdit),
        };
    }

    /// <summary>
    /// 要素のレイヤ・色・線種・線種の倍率・線幅のうち、書いたものだけを変える。形は変えない
    /// </summary>
    public sealed record RestyleEdit
    {
        /// <summary>
        /// 変更の種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = "restyle";

        /// <summary>
        /// 変える要素の ID。entities が返したもの
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 読んだときの要素のハッシュ。entities が返したもの
        /// </summary>
        [JsonPropertyName("hash")]
        public string Hash { get; init; } = default!;

        /// <summary>
        /// レイヤの元の名前。図面にあるレイヤで、名前が 1 つに決まるもの。layer_id と一緒には書かない
        /// </summary>
        [JsonPropertyName("layer")]
        public string? Layer { get; init; }

        /// <summary>
        /// レイヤの ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。小版 8 から
        /// </summary>
        [JsonPropertyName("layer_id")]
        public string? LayerId { get; init; }

        /// <summary>
        /// 色
        /// </summary>
        [JsonPropertyName("color")]
        public ColorChange? Color { get; init; }

        /// <summary>
        /// 線種
        /// </summary>
        [JsonPropertyName("line_type")]
        public LineTypeChange? LineType { get; init; }

        /// <summary>
        /// 要素の線種の倍率
        /// </summary>
        [JsonPropertyName("line_type_scale")]
        public double? LineTypeScale { get; init; }

        /// <summary>
        /// 線幅
        /// </summary>
        [JsonPropertyName("line_weight")]
        public LineWeightChange? LineWeight { get; init; }
    }

    /// <summary>
    /// 要素を、アフィン変換で動かす。AutoCAD の系統では、図形とともに動く XDATA と記号の属性も同じ変換で直す。変換した形を元の種類で表せなければ当てない
    /// </summary>
    public sealed record TransformEdit
    {
        /// <summary>
        /// 変更の種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = "transform";

        /// <summary>
        /// 動かす要素の ID。entities が返したもの
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 読んだときの要素のハッシュ。entities が返したもの
        /// </summary>
        [JsonPropertyName("hash")]
        public string Hash { get; init; } = default!;

        /// <summary>
        /// [a, b, c, d, e, f]。要素の空間か用紙の点 (x, y) を (a x + b y + c, d x + e y + f) に写す
        /// </summary>
        [JsonPropertyName("matrix")]
        public IReadOnlyList<double> Matrix { get; init; } = default!;
    }

    /// <summary>
    /// 要素を除く
    /// </summary>
    public sealed record RemoveEdit
    {
        /// <summary>
        /// 変更の種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = "remove";

        /// <summary>
        /// 除く要素の ID。entities が返したもの
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 読んだときの要素のハッシュ。entities が返したもの
        /// </summary>
        [JsonPropertyName("hash")]
        public string Hash { get; init; } = default!;
    }

    /// <summary>
    /// 要素の形を、渡す値で置き換える。形の種類は変えない。レイヤとスタイルは変えない。小版 10 から
    /// </summary>
    public sealed record ReplaceEdit
    {
        /// <summary>
        /// 変更の種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = "replace";

        /// <summary>
        /// 置き換える要素の ID。entities が返したもの
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 読んだときの要素のハッシュ。entities が返したもの
        /// </summary>
        [JsonPropertyName("hash")]
        public string Hash { get; init; } = default!;

        /// <summary>
        /// 新しい形。entities の geometry と同じ種類
        /// </summary>
        [JsonPropertyName("geometry")]
        public JsonAny Geometry { get; init; } = default!;
    }

    /// <summary>
    /// 空間か用紙に要素を足す。書かない色・線種・線幅はレイヤに従う。足せる形は、形の値のどの種類も。小版 10 から
    /// </summary>
    public sealed record AddEdit
    {
        /// <summary>
        /// 変更の種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = "add";

        /// <summary>
        /// 足す先の空間か用紙の ID（space/0・sheet/h22 など）。entities の ID の先頭の部分。小版 31 から、記号の定義の空間（space/h1F など）も指せる（決定 0177）
        /// </summary>
        [JsonPropertyName("place")]
        public string Place { get; init; } = default!;

        /// <summary>
        /// レイヤの元の名前。図面にあるレイヤで、名前が 1 つに決まるもの。layer_id と一緒には書かない。どちらも書かなければレイヤ 0
        /// </summary>
        [JsonPropertyName("layer")]
        public string? Layer { get; init; }

        /// <summary>
        /// レイヤの ID。tables が返したもの
        /// </summary>
        [JsonPropertyName("layer_id")]
        public string? LayerId { get; init; }

        /// <summary>
        /// 色。SXF はレイヤに従う色を書けないので、書く。小版 29 から
        /// </summary>
        [JsonPropertyName("color")]
        public ColorChange? Color { get; init; }

        /// <summary>
        /// 線種。小版 29 から
        /// </summary>
        [JsonPropertyName("line_type")]
        public LineTypeChange? LineType { get; init; }

        /// <summary>
        /// 線幅。小版 29 から
        /// </summary>
        [JsonPropertyName("line_weight")]
        public LineWeightChange? LineWeight { get; init; }

        /// <summary>
        /// 足す形
        /// </summary>
        [JsonPropertyName("geometry")]
        public JsonAny Geometry { get; init; } = default!;
    }

    /// <summary>
    /// レイヤの名前・状態・色・線種・線幅のうち、書いたものだけを変える（決定 0162）。DXF は、名前を変えると、図形と今のレイヤ（$CLAYER）も新しい名前で指す。DWG は、今あるレイヤの線種と真の色を変えられない。小版 22 から
    /// </summary>
    public sealed record LayerEdit
    {
        /// <summary>
        /// 変更の種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = "layer";

        /// <summary>
        /// 変えるレイヤの ID。tables が返したもの
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 読んだときのレイヤのハッシュ。tables が返したもの
        /// </summary>
        [JsonPropertyName("hash")]
        public string Hash { get; init; } = default!;

        /// <summary>
        /// 新しい名前。ほかのレイヤと重ならず（大文字と小文字を区別しない）、&lt; &gt; / \ " : ; ? * | = ` を含まないもの。0 と Defpoints の名前は変えられない
        /// </summary>
        [JsonPropertyName("name")]
        public string? Name { get; init; }

        /// <summary>
        /// 表示する。凍結するなら偽
        /// </summary>
        [JsonPropertyName("visible")]
        public bool? Visible { get; init; }

        /// <summary>
        /// 印刷する
        /// </summary>
        [JsonPropertyName("printable")]
        public bool? Printable { get; init; }

        /// <summary>
        /// 編集できる（ロックしない）
        /// </summary>
        [JsonPropertyName("editable")]
        public bool? Editable { get; init; }

        /// <summary>
        /// 凍結する
        /// </summary>
        [JsonPropertyName("frozen")]
        public bool? Frozen { get; init; }

        /// <summary>
        /// 色。aci が要る。inherit は書けない
        /// </summary>
        [JsonPropertyName("color")]
        public ColorChange? Color { get; init; }

        /// <summary>
        /// 線種。name か id で指す。inherit は書けない
        /// </summary>
        [JsonPropertyName("line_type")]
        public LineTypeChange? LineType { get; init; }

        /// <summary>
        /// 線幅。mm か default。inherit は書けない
        /// </summary>
        [JsonPropertyName("line_weight")]
        public LineWeightChange? LineWeight { get; init; }
    }

    /// <summary>
    /// レイヤを除く（決定 0162）。要素・ほかのオブジェクト・今のレイヤ（$CLAYER）が指すレイヤと、0・Defpoints・外部参照に依るレイヤは除けない。同じ apply の要素の変更が指すレイヤも除けない。後ろにハンドルのないレイヤがあれば、その ID が変わるので除けない。除いたレイヤより後ろのレイヤと、その上の要素のハッシュは変わるので、読み直す。小版 24 から
    /// </summary>
    public sealed record RemoveLayerEdit
    {
        /// <summary>
        /// 変更の種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = "remove_layer";

        /// <summary>
        /// 除くレイヤの ID。tables が返したもの
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 読んだときのレイヤのハッシュ。tables が返したもの
        /// </summary>
        [JsonPropertyName("hash")]
        public string Hash { get; init; } = default!;
    }

    /// <summary>
    /// 文字スタイルの名前・フォント・高さ・幅の倍率・傾き・裏返し・上下反転・縦書きのうち、書いたものだけを変える（決定 0163）。DXF は、名前を変えると、文字と今の文字スタイル（$TEXTSTYLE）も新しい名前で指す。表の書式などの図形でないオブジェクトが名前で指す文字スタイルの名前は変えられない。小版 25 から
    /// </summary>
    public sealed record TextStyleEdit
    {
        /// <summary>
        /// 変更の種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = "text_style";

        /// <summary>
        /// 変える文字スタイルの ID。tables が返したもの
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 読んだときの文字スタイルのハッシュ。tables が返したもの
        /// </summary>
        [JsonPropertyName("hash")]
        public string Hash { get; init; } = default!;

        /// <summary>
        /// 新しい名前。ほかの文字スタイルと重ならず（大文字と小文字を区別しない）、&lt; &gt; / \ " : ; ? * | = ` を含まないもの。Standard の名前は変えられない
        /// </summary>
        [JsonPropertyName("name")]
        public string? Name { get; init; }

        /// <summary>
        /// 主のフォント。形のファイル（shape）か、太字・斜体・文字集合・ピッチを持たない TrueType のファイル（named で、名前が .ttf・.ttc・.otf で終わるもの）。名前を空にすると除く。XDATA の書体の名前で指すフォントは変えられない
        /// </summary>
        [JsonPropertyName("font")]
        public Font? Font { get; init; }

        /// <summary>
        /// 大きいフォント。形のファイル（shape）だけ。名前を空にすると除く
        /// </summary>
        [JsonPropertyName("big_font")]
        public Font? BigFont { get; init; }

        /// <summary>
        /// 決まった高さ。文書の単位。0 なら決めない。負の数は書けない
        /// </summary>
        [JsonPropertyName("height")]
        public double? Height { get; init; }

        /// <summary>
        /// 幅の倍率。正の数
        /// </summary>
        [JsonPropertyName("width")]
        public double? Width { get; init; }

        /// <summary>
        /// 傾き（度）。-85 から 85 まで
        /// </summary>
        [JsonPropertyName("oblique")]
        public double? Oblique { get; init; }

        /// <summary>
        /// 左右を裏返す
        /// </summary>
        [JsonPropertyName("backward")]
        public bool? Backward { get; init; }

        /// <summary>
        /// 上下を反転する
        /// </summary>
        [JsonPropertyName("upside_down")]
        public bool? UpsideDown { get; init; }

        /// <summary>
        /// 縦書き
        /// </summary>
        [JsonPropertyName("vertical")]
        public bool? Vertical { get; init; }
    }

    /// <summary>
    /// 文字スタイルを足す（決定 0163）。書かない欄は Standard と同じ。同じ apply の要素は、足した文字スタイルを指せない。小版 25 から
    /// </summary>
    public sealed record AddTextStyleEdit
    {
        /// <summary>
        /// 変更の種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = "add_text_style";

        /// <summary>
        /// 名前。text_style の name と同じ決まり
        /// </summary>
        [JsonPropertyName("name")]
        public string Name { get; init; } = default!;

        /// <summary>
        /// 主のフォント。形のファイル（shape）か、太字・斜体・文字集合・ピッチを持たない TrueType のファイル（named で、名前が .ttf・.ttc・.otf で終わるもの）。名前を空にすると除く。XDATA の書体の名前で指すフォントは変えられない
        /// </summary>
        [JsonPropertyName("font")]
        public Font? Font { get; init; }

        /// <summary>
        /// 大きいフォント。形のファイル（shape）だけ。名前を空にすると除く
        /// </summary>
        [JsonPropertyName("big_font")]
        public Font? BigFont { get; init; }

        /// <summary>
        /// 決まった高さ。文書の単位。0 なら決めない。負の数は書けない
        /// </summary>
        [JsonPropertyName("height")]
        public double? Height { get; init; }

        /// <summary>
        /// 幅の倍率。正の数
        /// </summary>
        [JsonPropertyName("width")]
        public double? Width { get; init; }

        /// <summary>
        /// 傾き（度）。-85 から 85 まで
        /// </summary>
        [JsonPropertyName("oblique")]
        public double? Oblique { get; init; }

        /// <summary>
        /// 左右を裏返す
        /// </summary>
        [JsonPropertyName("backward")]
        public bool? Backward { get; init; }

        /// <summary>
        /// 上下を反転する
        /// </summary>
        [JsonPropertyName("upside_down")]
        public bool? UpsideDown { get; init; }

        /// <summary>
        /// 縦書き
        /// </summary>
        [JsonPropertyName("vertical")]
        public bool? Vertical { get; init; }
    }

    /// <summary>
    /// 文字スタイルを除く（決定 0163）。要素・表の書式や寸法スタイルなどのほかのオブジェクト・今の文字スタイル（$TEXTSTYLE）が指すものと、Standard・外部参照に依るものは除けない。同じ apply で足す要素が指すものも除けない。後ろにハンドルのない文字スタイルがあれば、その ID が変わるので除けない。小版 25 から
    /// </summary>
    public sealed record RemoveTextStyleEdit
    {
        /// <summary>
        /// 変更の種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = "remove_text_style";

        /// <summary>
        /// 除く文字スタイルの ID。tables が返したもの
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 読んだときの文字スタイルのハッシュ。tables が返したもの
        /// </summary>
        [JsonPropertyName("hash")]
        public string Hash { get; init; } = default!;
    }

    /// <summary>
    /// 線種の名前・説明・線と間の長さの列のうち、書いたものだけを変える（決定 0164）。DXF は、名前を変えると、要素・レイヤと今の線種（$CELTYPE）も新しい名前で指す。多重線のスタイルなどの図形でないオブジェクトが名前で指す線種の名前は変えられない。形か文字を添える線種と DWG の線種は、列の要素の数を変えられない。小版 26 から
    /// </summary>
    public sealed record LineTypeEdit
    {
        /// <summary>
        /// 変更の種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = "line_type";

        /// <summary>
        /// 変える線種の ID。tables が返したもの
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 読んだときの線種のハッシュ。tables が返したもの
        /// </summary>
        [JsonPropertyName("hash")]
        public string Hash { get; init; } = default!;

        /// <summary>
        /// 新しい名前。ほかの線種と重ならず（大文字と小文字を区別しない）、&lt; &gt; / \ " : ; ? * | = ` を含まないもの。ByLayer・ByBlock・Continuous と外部参照に依る線種の名前は変えられない
        /// </summary>
        [JsonPropertyName("name")]
        public string? Name { get; init; }

        /// <summary>
        /// 説明
        /// </summary>
        [JsonPropertyName("description")]
        public string? Description { get; init; }

        /// <summary>
        /// 線と間の長さの列。文書の単位。正は線、負は間、0 は点
        /// </summary>
        [JsonPropertyName("pattern")]
        public IReadOnlyList<double>? Pattern { get; init; }
    }

    /// <summary>
    /// 線種を足す（決定 0164）。書かない欄は Continuous と同じ。同じ apply の要素とレイヤは、足した線種を指せない。小版 26 から
    /// </summary>
    public sealed record AddLineTypeEdit
    {
        /// <summary>
        /// 変更の種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = "add_line_type";

        /// <summary>
        /// 名前。line_type の name と同じ決まり
        /// </summary>
        [JsonPropertyName("name")]
        public string Name { get; init; } = default!;

        /// <summary>
        /// 説明
        /// </summary>
        [JsonPropertyName("description")]
        public string? Description { get; init; }

        /// <summary>
        /// 線と間の長さの列。文書の単位。正は線、負は間、0 は点
        /// </summary>
        [JsonPropertyName("pattern")]
        public IReadOnlyList<double>? Pattern { get; init; }
    }

    /// <summary>
    /// 線種を除く（決定 0164）。要素・レイヤ・多重線のスタイルなどのほかのオブジェクト・今の線種（$CELTYPE）が指すものと、ByLayer・ByBlock・Continuous・外部参照に依るものは除けない。同じ apply で足す・変える要素とレイヤが指すものも除けない。後ろにハンドルのない線種があれば、その ID が変わるので除けない。小版 26 から
    /// </summary>
    public sealed record RemoveLineTypeEdit
    {
        /// <summary>
        /// 変更の種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = "remove_line_type";

        /// <summary>
        /// 除く線種の ID。tables が返したもの
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 読んだときの線種のハッシュ。tables が返したもの
        /// </summary>
        [JsonPropertyName("hash")]
        public string Hash { get; init; } = default!;
    }

    /// <summary>
    /// 寸法スタイルの名前・端部記号・その大きさ・斜線・倍率のうち、書いたものだけを変える（決定 0165）。DXF は、名前を変えると、寸法・引出線・公差と今の寸法スタイル（$DIMSTYLE）も新しい名前で指す。図形でないオブジェクトが名前で指す寸法スタイルの名前は変えられない。小版 27 から
    /// </summary>
    public sealed record DimensionStyleEdit
    {
        /// <summary>
        /// 変更の種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = "dimension_style";

        /// <summary>
        /// 変える寸法スタイルの ID。tables が返したもの
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 読んだときの寸法スタイルのハッシュ。tables が返したもの
        /// </summary>
        [JsonPropertyName("hash")]
        public string Hash { get; init; } = default!;

        /// <summary>
        /// 新しい名前。ほかの寸法スタイルと重ならず（大文字と小文字を区別しない）、$ と &lt; &gt; / \ " : ; ? * | = ` を含まないもの。Standard・子の寸法スタイル（名前に $ を持つもの）・子を持つ寸法スタイル・外部参照に依る寸法スタイルの名前は変えられない
        /// </summary>
        [JsonPropertyName("name")]
        public string? Name { get; init; }

        /// <summary>
        /// 寸法線の 1 本目と 2 本目の補助線の側の端部記号。2 つのブロックの名前で、空の名前は既定の矢印（塗りつぶした閉じた矢印）。DXF は R2000 から変えられる。DWG の端部記号は変えられない
        /// </summary>
        [JsonPropertyName("ends")]
        public IReadOnlyList<string>? Ends { get; init; }

        /// <summary>
        /// 引出線の端部記号。ブロックの名前で、空の名前は既定の矢印。ends と同じ決まり
        /// </summary>
        [JsonPropertyName("leader")]
        public string? Leader { get; init; }

        /// <summary>
        /// 端部記号の大きさ（DIMASZ）。文書の単位で、scale を掛ける前の値。負の数は書けない
        /// </summary>
        [JsonPropertyName("size")]
        public double? Size { get; init; }

        /// <summary>
        /// 端部記号の代わりに描く斜線の大きさ（DIMTSZ）。0 なら斜線を描かない。負の数は書けない
        /// </summary>
        [JsonPropertyName("tick")]
        public double? Tick { get; init; }

        /// <summary>
        /// 大きさに掛ける倍率（DIMSCALE）。0 はビューの縮尺から決める。負の数は書けない
        /// </summary>
        [JsonPropertyName("scale")]
        public double? Scale { get; init; }
    }

    /// <summary>
    /// 寸法スタイルを足す（決定 0165）。書かない欄は Standard と同じ。同じ apply の要素は、足した寸法スタイルを指せない。小版 27 から
    /// </summary>
    public sealed record AddDimensionStyleEdit
    {
        /// <summary>
        /// 変更の種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = "add_dimension_style";

        /// <summary>
        /// 名前。dimension_style の name と同じ決まり
        /// </summary>
        [JsonPropertyName("name")]
        public string Name { get; init; } = default!;

        /// <summary>
        /// 寸法線の 1 本目と 2 本目の補助線の側の端部記号。2 つのブロックの名前で、空の名前は既定の矢印（塗りつぶした閉じた矢印）。DXF は R2000 から変えられる。DWG の端部記号は変えられない
        /// </summary>
        [JsonPropertyName("ends")]
        public IReadOnlyList<string>? Ends { get; init; }

        /// <summary>
        /// 引出線の端部記号。ブロックの名前で、空の名前は既定の矢印。ends と同じ決まり
        /// </summary>
        [JsonPropertyName("leader")]
        public string? Leader { get; init; }

        /// <summary>
        /// 端部記号の大きさ（DIMASZ）。文書の単位で、scale を掛ける前の値。負の数は書けない
        /// </summary>
        [JsonPropertyName("size")]
        public double? Size { get; init; }

        /// <summary>
        /// 端部記号の代わりに描く斜線の大きさ（DIMTSZ）。0 なら斜線を描かない。負の数は書けない
        /// </summary>
        [JsonPropertyName("tick")]
        public double? Tick { get; init; }

        /// <summary>
        /// 大きさに掛ける倍率（DIMSCALE）。0 はビューの縮尺から決める。負の数は書けない
        /// </summary>
        [JsonPropertyName("scale")]
        public double? Scale { get; init; }
    }

    /// <summary>
    /// 寸法スタイルを除く（決定 0165）。寸法・引出線・公差などのほかのオブジェクト・今の寸法スタイル（$DIMSTYLE）が指すものと、Standard・子の寸法スタイル・子を持つ寸法スタイル・外部参照に依るものは除けない。後ろにハンドルのない寸法スタイルがあれば、その ID が変わるので除けない。小版 27 から
    /// </summary>
    public sealed record RemoveDimensionStyleEdit
    {
        /// <summary>
        /// 変更の種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = "remove_dimension_style";

        /// <summary>
        /// 除く寸法スタイルの ID。tables が返したもの
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 読んだときの寸法スタイルのハッシュ。tables が返したもの
        /// </summary>
        [JsonPropertyName("hash")]
        public string Hash { get; init; } = default!;
    }

    /// <summary>
    /// 文書の点の表し方（marker を持たない点の見え方）を変える（決定 0190）。文書全体の設定なので要素を指さない。読んだときの文書は、apply の hash で確かめる。DXF・DWG は見出しの $PDMODE・$PDSIZE に書く。DWG は、変数を書く幅が変わる値（$PDMODE の 0 とほかの値の間の変更、$PDSIZE の 0・1 とほかの値の間の変更）を、小版 39 から書ける（決定 0193）。JWW・SXF は文書の点の表し方を持たないので断る。applied は id と hash を持たない。小版 38 から
    /// </summary>
    public sealed record PointStyleEdit
    {
        /// <summary>
        /// 変更の種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = "point_style";

        /// <summary>
        /// 基本の形
        /// </summary>
        [JsonPropertyName("mark")]
        public string Mark { get; init; } = default!;

        /// <summary>
        /// 円で囲む。書かなければ偽
        /// </summary>
        [JsonPropertyName("circle")]
        public bool? Circle { get; init; }

        /// <summary>
        /// 四角で囲む。書かなければ偽
        /// </summary>
        [JsonPropertyName("square")]
        public bool? Square { get; init; }

        /// <summary>
        /// 大きさ。0 より大きい有限の数。意味は size_kind で決まる
        /// </summary>
        [JsonPropertyName("size")]
        public double Size { get; init; }

        /// <summary>
        /// 大きさの意味
        /// </summary>
        [JsonPropertyName("size_kind")]
        public string SizeKind { get; init; } = default!;
    }

    /// <summary>
    /// 文書の点の基本の形。dot は小さな点、none はなし、plus は十字、cross はバツ、tick は上向きの短い線
    /// </summary>
    public static class PointMark
    {
        /// <summary>dot</summary>
        public const string Dot = "dot";
        /// <summary>none</summary>
        public const string None = "none";
        /// <summary>plus</summary>
        public const string Plus = "plus";
        /// <summary>cross</summary>
        public const string Cross = "cross";
        /// <summary>tick</summary>
        public const string Tick = "tick";
    }

    /// <summary>
    /// 文書の点の大きさの意味。length は点のある空間の長さ、screen は画面の高さに対する百分率
    /// </summary>
    public static class PointSizeKind
    {
        /// <summary>length</summary>
        public const string Length = "length";
        /// <summary>screen</summary>
        public const string Screen = "screen";
    }

    /// <summary>
    /// レイヤを足す（決定 0162）。書かない欄は、状態なら表示・印刷・編集でき凍結せず、色・線種・線幅ならレイヤ 0 と同じ。同じ apply の要素は、足したレイヤを指せない。小版 22 から
    /// </summary>
    public sealed record AddLayerEdit
    {
        /// <summary>
        /// 変更の種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = "add_layer";

        /// <summary>
        /// 名前。layer の name と同じ決まり
        /// </summary>
        [JsonPropertyName("name")]
        public string Name { get; init; } = default!;

        /// <summary>
        /// 表示する。凍結するなら偽
        /// </summary>
        [JsonPropertyName("visible")]
        public bool? Visible { get; init; }

        /// <summary>
        /// 印刷する
        /// </summary>
        [JsonPropertyName("printable")]
        public bool? Printable { get; init; }

        /// <summary>
        /// 編集できる（ロックしない）
        /// </summary>
        [JsonPropertyName("editable")]
        public bool? Editable { get; init; }

        /// <summary>
        /// 凍結する
        /// </summary>
        [JsonPropertyName("frozen")]
        public bool? Frozen { get; init; }

        /// <summary>
        /// 色。aci が要る。inherit は書けない
        /// </summary>
        [JsonPropertyName("color")]
        public ColorChange? Color { get; init; }

        /// <summary>
        /// 線種。name か id で指す。inherit は書けない
        /// </summary>
        [JsonPropertyName("line_type")]
        public LineTypeChange? LineType { get; init; }

        /// <summary>
        /// 線幅。mm か default。inherit は書けない
        /// </summary>
        [JsonPropertyName("line_weight")]
        public LineWeightChange? LineWeight { get; init; }
    }

    /// <summary>
    /// 値を継承する先。layer はレイヤから、block は要素を置いた記号の配置から
    /// </summary>
    public static class Inherit
    {
        /// <summary>layer</summary>
        public const string Layer = "layer";
        /// <summary>block</summary>
        public const string Block = "block";
    }

    /// <summary>
    /// 新しい色。inherit か、aci・sxf・jww・rgb の少なくとも 1 つのどちらかを書く。aci・sxf・jww は 1 つだけ書く
    /// </summary>
    public sealed record ColorChange
    {
        /// <summary>
        /// 継承する先
        /// </summary>
        [JsonPropertyName("inherit")]
        public string? Inherit { get; init; }

        /// <summary>
        /// AutoCAD の色の番号（1〜255）
        /// </summary>
        [JsonPropertyName("aci")]
        public uint? Aci { get; init; }

        /// <summary>
        /// SXF の色のコード。1〜16 は既定義色、17 からは利用者定義色を定義の順に数えたもの。小版 29 から
        /// </summary>
        [JsonPropertyName("sxf")]
        public ulong? Sxf { get; init; }

        /// <summary>
        /// Jw_cad のペンの番号。0〜9 と、版 420 からの SXF の拡張色 100〜356。小版 30 から
        /// </summary>
        [JsonPropertyName("jww")]
        public uint? Jww { get; init; }

        /// <summary>
        /// [赤, 緑, 青]。それぞれ 0〜255
        /// </summary>
        [JsonPropertyName("rgb")]
        public IReadOnlyList<uint>? Rgb { get; init; }
    }

    /// <summary>
    /// 新しい線種。inherit・name・id のどれか 1 つを書く
    /// </summary>
    public sealed record LineTypeChange
    {
        /// <summary>
        /// 継承する先
        /// </summary>
        [JsonPropertyName("inherit")]
        public string? Inherit { get; init; }

        /// <summary>
        /// 線種の元の名前。図面にある線種で、名前が 1 つに決まるもの
        /// </summary>
        [JsonPropertyName("name")]
        public string? Name { get; init; }

        /// <summary>
        /// 線種の ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。小版 8 から
        /// </summary>
        [JsonPropertyName("id")]
        public string? Id { get; init; }
    }

    /// <summary>
    /// 新しい線幅。inherit・mm・default のどれか 1 つを書く
    /// </summary>
    public sealed record LineWeightChange
    {
        /// <summary>
        /// 継承する先
        /// </summary>
        [JsonPropertyName("inherit")]
        public string? Inherit { get; init; }

        /// <summary>
        /// 用紙の上の幅（mm）。0 以上
        /// </summary>
        [JsonPropertyName("mm")]
        public double? Mm { get; init; }

        /// <summary>
        /// true なら、文書の既定の線幅
        /// </summary>
        [JsonPropertyName("default")]
        public bool? Default { get; init; }
    }

    /// <summary>
    /// 開いた文書を図面モデル（L2）に変換し、レイヤと線種の表の項目を返す（決定 0156）。小版 8 から
    /// </summary>
    public sealed record TablesRequest
    {
        /// <summary>
        /// メッセージのスキーマの大版。今は 1
        /// </summary>
        [JsonPropertyName("v")]
        public ulong V { get; init; }

        /// <summary>
        /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        /// </summary>
        [JsonPropertyName("budget")]
        public Budget? Budget { get; init; }

        /// <summary>
        /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        /// </summary>
        [JsonPropertyName("partial")]
        public bool? Partial { get; init; }

        /// <summary>
        /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        /// </summary>
        [JsonPropertyName("max_places")]
        public ulong? MaxPlaces { get; init; }

        /// <summary>
        /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        /// </summary>
        [JsonPropertyName("time_limit_ms")]
        public ulong? TimeLimitMs { get; init; }

        /// <summary>
        /// op の名前
        /// </summary>
        [JsonPropertyName("op")]
        public string Op { get; init; } = "tables";

        /// <summary>
        /// 文書の名前。open で作ったもの
        /// </summary>
        [JsonPropertyName("doc")]
        public string Doc { get; init; } = default!;
    }

    /// <summary>
    /// 表の項目。図面モデルの順
    /// </summary>
    public sealed record TablesResult
    {
        /// <summary>
        /// レイヤ
        /// </summary>
        [JsonPropertyName("layers")]
        public IReadOnlyList<LayerItem> Layers { get; init; } = default!;

        /// <summary>
        /// 線種。小版 26 から、定義を持つ
        /// </summary>
        [JsonPropertyName("line_types")]
        public IReadOnlyList<LineTypeItem> LineTypes { get; init; } = default!;

        /// <summary>
        /// 寸法スタイル。小版 27 から、端部記号と大きさを持つ
        /// </summary>
        [JsonPropertyName("dimension_styles")]
        public IReadOnlyList<DimensionStyleItem> DimensionStyles { get; init; } = default!;

        /// <summary>
        /// 文字スタイル（決定 0158）。要素が名前で指すだけで、ファイルに定義がないものは、定義の欄を持たない項目として後ろに並ぶ。小版 20 から
        /// </summary>
        [JsonPropertyName("text_styles")]
        public IReadOnlyList<TextStyleItem> TextStyles { get; init; } = default!;
    }

    /// <summary>
    /// 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、2D のシーンを blob で返す（決定 0172）。シーンの配置は schema/scene.toml にある。小版 33 から
    /// </summary>
    public sealed record SceneRequest
    {
        /// <summary>
        /// メッセージのスキーマの大版。今は 1
        /// </summary>
        [JsonPropertyName("v")]
        public ulong V { get; init; }

        /// <summary>
        /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        /// </summary>
        [JsonPropertyName("budget")]
        public Budget? Budget { get; init; }

        /// <summary>
        /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        /// </summary>
        [JsonPropertyName("partial")]
        public bool? Partial { get; init; }

        /// <summary>
        /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        /// </summary>
        [JsonPropertyName("max_places")]
        public ulong? MaxPlaces { get; init; }

        /// <summary>
        /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        /// </summary>
        [JsonPropertyName("time_limit_ms")]
        public ulong? TimeLimitMs { get; init; }

        /// <summary>
        /// op の名前
        /// </summary>
        [JsonPropertyName("op")]
        public string Op { get; init; } = "scene";

        /// <summary>
        /// 文書の名前。open で作ったもの
        /// </summary>
        [JsonPropertyName("doc")]
        public string Doc { get; init; } = default!;

        /// <summary>
        /// 起点。実寸の空間・記号の定義の空間・用紙の ID（drawing と entities が返すもの）
        /// </summary>
        [JsonPropertyName("from")]
        public string From { get; init; } = default!;

        /// <summary>
        /// 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した空間の要素だけを描く。from が用紙のときだけ書ける
        /// </summary>
        [JsonPropertyName("view")]
        public ulong? View { get; init; }

        /// <summary>
        /// [x の最小, y の最小, x の最大, y の最大]。起点の座標の長方形。範囲の重なる塊だけを返す
        /// </summary>
        [JsonPropertyName("region")]
        public IReadOnlyList<double>? Region { get; init; }

        /// <summary>
        /// 弦の誤差（起点の長さ）。0 より大きい。書かなければ、用紙からは 0.02 mm、空間からは要素の範囲の対角線の 1/50000
        /// </summary>
        [JsonPropertyName("tolerance")]
        public double? Tolerance { get; init; }

        /// <summary>
        /// true なら印刷する要素を、書かないか false なら表示する要素を描く
        /// </summary>
        [JsonPropertyName("print")]
        public bool? Print { get; init; }

        /// <summary>
        /// 色の表の入力の名前（zm_push で渡したもの）。プロファイルの色の語彙と同じ形の JSON で、values に rgb（#rrggbb）と aci・sxf・jww の符号を並べる（決定 0138・0178）。書かなければ、ファイルが RGB を持たない色は未解決の印を立てる
        /// </summary>
        [JsonPropertyName("palette")]
        public string? Palette { get; init; }

        /// <summary>
        /// 文字を置くフォント（決定 0188）。書けば、文字を字形で置き、シーンの字形の区画に形と変換を返す。文字スタイルと書式の区間のフォントの名前（シーンの font_names）で引き、同じ名前、なければ ASCII の大文字と小文字を区別しない名前で合わせる。合うものがなければ名前のないフォントで置く。小版 37 から
        /// </summary>
        [JsonPropertyName("fonts")]
        public IReadOnlyList<SceneFont>? Fonts { get; init; }

        /// <summary>
        /// 形式の宣言値の高さ（キーは dxf.TEXT.40・dxf.MTEXT.40・jww.moji.size_y など）が、フォントのどの寸法に当たるか（決定 0176・0188）。書かないキーは大文字の高さとみなす。fonts を書いたときだけ使う。小版 37 から
        /// </summary>
        [JsonPropertyName("heights")]
        public IReadOnlyDictionary<string, string>? Heights { get; init; }
    }

    /// <summary>
    /// シーンの文字を置くフォント（決定 0188）
    /// </summary>
    public sealed record SceneFont
    {
        /// <summary>
        /// 文字が指すフォントの名前（ファイルに書かれた元の名前）。書かなければ、名前の合わない文字に使う既定のフォント。同じ名前と、名前のないものは 1 つまで
        /// </summary>
        [JsonPropertyName("name")]
        public string? Name { get; init; }

        /// <summary>
        /// TrueType・OpenType（CFF・CFF2 を含む）のファイルの入力の名前（zm_push で渡したもの）
        /// </summary>
        [JsonPropertyName("input")]
        public string Input { get; init; } = default!;

        /// <summary>
        /// フォントのコレクション（TTC）の中の番号。書かなければ 0
        /// </summary>
        [JsonPropertyName("index")]
        public uint? Index { get; init; }
    }

    /// <summary>
    /// 宣言された高さが当たるフォントの寸法。cap_height は大文字の高さ、em は em、frame はアセンダーとディセンダーを合わせた高さ
    /// </summary>
    public static class HeightReference
    {
        /// <summary>cap_height</summary>
        public const string CapHeight = "cap_height";
        /// <summary>em</summary>
        public const string Em = "em";
        /// <summary>frame</summary>
        public const string Frame = "frame";
    }

    /// <summary>
    /// シーンの要約と、シーンの blob の番号
    /// </summary>
    public sealed record SceneResult
    {
        /// <summary>
        /// 起点の ID
        /// </summary>
        [JsonPropertyName("from")]
        public string From { get; init; } = default!;

        /// <summary>
        /// 起点のビューの番号。ビューでなければ持たない
        /// </summary>
        [JsonPropertyName("view")]
        public ulong? View { get; init; }

        /// <summary>
        /// 起点の座標の単位。用紙とビューからは millimeter
        /// </summary>
        [JsonPropertyName("unit")]
        public string Unit { get; init; } = default!;

        /// <summary>
        /// 使った弦の誤差（起点の長さ）
        /// </summary>
        [JsonPropertyName("tolerance")]
        public double Tolerance { get; init; }

        /// <summary>
        /// 返した描くものの数
        /// </summary>
        [JsonPropertyName("counts")]
        public SceneCounts Counts { get; init; } = default!;

        /// <summary>
        /// 返した塊の範囲。起点の座標。塊がなければ持たない
        /// </summary>
        [JsonPropertyName("extent")]
        public Extent? Extent { get; init; }

        /// <summary>
        /// シーンの blob の番号（種類は scene）。zm_take で取り出す
        /// </summary>
        [JsonPropertyName("blob")]
        public ulong Blob { get; init; }
    }

    /// <summary>
    /// シーンの描くものの種類ごとの数
    /// </summary>
    public sealed record SceneCounts
    {
        /// <summary>
        /// 塊
        /// </summary>
        [JsonPropertyName("chunks")]
        public ulong Chunks { get; init; }

        /// <summary>
        /// 線（折れ線）
        /// </summary>
        [JsonPropertyName("lines")]
        public ulong Lines { get; init; }

        /// <summary>
        /// 線と面の頂点
        /// </summary>
        [JsonPropertyName("vertices")]
        public ulong Vertices { get; init; }

        /// <summary>
        /// 面
        /// </summary>
        [JsonPropertyName("faces")]
        public ulong Faces { get; init; }

        /// <summary>
        /// 文字
        /// </summary>
        [JsonPropertyName("texts")]
        public ulong Texts { get; init; }

        /// <summary>
        /// 点
        /// </summary>
        [JsonPropertyName("points")]
        public ulong Points { get; init; }

        /// <summary>
        /// 画像
        /// </summary>
        [JsonPropertyName("images")]
        public ulong Images { get; init; }

        /// <summary>
        /// 置いた字形。fonts を書かなければ 0。小版 37 から
        /// </summary>
        [JsonPropertyName("glyphs")]
        public ulong Glyphs { get; init; }
    }

    /// <summary>
    /// 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、プロファイルの規則と語彙で照合する（決定 0138・0173・0179）。小版 34 から
    /// </summary>
    public sealed record CheckRequest
    {
        /// <summary>
        /// メッセージのスキーマの大版。今は 1
        /// </summary>
        [JsonPropertyName("v")]
        public ulong V { get; init; }

        /// <summary>
        /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        /// </summary>
        [JsonPropertyName("budget")]
        public Budget? Budget { get; init; }

        /// <summary>
        /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        /// </summary>
        [JsonPropertyName("partial")]
        public bool? Partial { get; init; }

        /// <summary>
        /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        /// </summary>
        [JsonPropertyName("max_places")]
        public ulong? MaxPlaces { get; init; }

        /// <summary>
        /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        /// </summary>
        [JsonPropertyName("time_limit_ms")]
        public ulong? TimeLimitMs { get; init; }

        /// <summary>
        /// op の名前
        /// </summary>
        [JsonPropertyName("op")]
        public string Op { get; init; } = "check";

        /// <summary>
        /// 文書の名前。open で作ったもの
        /// </summary>
        [JsonPropertyName("doc")]
        public string Doc { get; init; } = default!;

        /// <summary>
        /// プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す
        /// </summary>
        [JsonPropertyName("profiles")]
        public IReadOnlyList<string> Profiles { get; init; } = default!;

        /// <summary>
        /// 照合に使うプロファイル。profiles の中から選ぶ
        /// </summary>
        [JsonPropertyName("profile")]
        public ProfileRef Profile { get; init; } = default!;

        /// <summary>
        /// たどる起点。実寸の空間・記号の定義の空間・用紙の ID。書かなければ、すべての用紙から、用紙がなければ実寸の空間からたどる
        /// </summary>
        [JsonPropertyName("from")]
        public string? From { get; init; }

        /// <summary>
        /// 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した要素と、そのビューの縮尺だけを照合する。from が用紙のときだけ書ける
        /// </summary>
        [JsonPropertyName("view")]
        public ulong? View { get; init; }

        /// <summary>
        /// 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。小版 40 から
        /// </summary>
        [JsonPropertyName("file_name")]
        public string? FileName { get; init; }

        /// <summary>
        /// 返す最初の指摘の位置。前の応答の next_offset を渡す。書かなければ 0
        /// </summary>
        [JsonPropertyName("offset")]
        public ulong? Offset { get; init; }

        /// <summary>
        /// 返す指摘の数の上限。書かなければ 1000
        /// </summary>
        [JsonPropertyName("limit")]
        public uint? Limit { get; init; }

        /// <summary>
        /// 色の表の入力の名前（zm_push で渡したもの）。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く
        /// </summary>
        [JsonPropertyName("palette")]
        public string? Palette { get; init; }
    }

    /// <summary>
    /// プロファイルの識別（決定 0138）
    /// </summary>
    public sealed record ProfileRef
    {
        /// <summary>
        /// 基準の系統（mlit-cad など）
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 基準の版（2025-12 など）
        /// </summary>
        [JsonPropertyName("edition")]
        public string Edition { get; init; } = default!;
    }

    /// <summary>
    /// 照合の結果。指摘は offset から limit 個まで
    /// </summary>
    public sealed record CheckResult
    {
        /// <summary>
        /// 照合に使ったプロファイルの鎖。extends の基から順
        /// </summary>
        [JsonPropertyName("chain")]
        public IReadOnlyList<ProfileIdentity> Chain { get; init; } = default!;

        /// <summary>
        /// 照合した規則ごとの数。プロファイルの順。使えない規則は持たず、profile.rule.unsupported の診断で返す
        /// </summary>
        [JsonPropertyName("rules")]
        public IReadOnlyList<CheckRule> Rules { get; init; } = default!;

        /// <summary>
        /// 指摘。規則の順に、規則ごとにたどった順
        /// </summary>
        [JsonPropertyName("findings")]
        public IReadOnlyList<CheckFinding> Findings { get; init; } = default!;

        /// <summary>
        /// 全体の指摘の数
        /// </summary>
        [JsonPropertyName("total")]
        public ulong Total { get; init; }

        /// <summary>
        /// 続きの最初の指摘の位置。続きがなければ持たない
        /// </summary>
        [JsonPropertyName("next_offset")]
        public ulong? NextOffset { get; init; }
    }

    /// <summary>
    /// 照合に使ったプロファイルの識別と、データの直し
    /// </summary>
    public sealed record ProfileIdentity
    {
        /// <summary>
        /// 基準の系統
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 基準の版
        /// </summary>
        [JsonPropertyName("edition")]
        public string Edition { get; init; } = default!;

        /// <summary>
        /// このデータの直し。1 から
        /// </summary>
        [JsonPropertyName("revision")]
        public ulong Revision { get; init; }
    }

    /// <summary>
    /// 規則ごとの数
    /// </summary>
    public sealed record CheckRule
    {
        /// <summary>
        /// 規則の id
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 照合の種類（layer-name など）
        /// </summary>
        [JsonPropertyName("check")]
        public string Check { get; init; } = default!;

        /// <summary>
        /// 指摘の数。照合できなかったものを除く
        /// </summary>
        [JsonPropertyName("findings")]
        public ulong Findings { get; init; }

        /// <summary>
        /// 照合できなかった数
        /// </summary>
        [JsonPropertyName("unsupported")]
        public ulong Unsupported { get; init; }
    }

    /// <summary>
    /// 指摘。diagnostic は診断と同じ形で、describe で文にできる。severity は規則の level で、照合できない値（profile.&lt;種類&gt;.unsupported）は info。count は 1
    /// </summary>
    public sealed record CheckFinding
    {
        /// <summary>
        /// 規則の id
        /// </summary>
        [JsonPropertyName("rule")]
        public string Rule { get; init; } = default!;

        /// <summary>
        /// code・重要度・場所（要素・実効のレイヤ・用紙の ID）・見つけた値
        /// </summary>
        [JsonPropertyName("diagnostic")]
        public Diagnostic Diagnostic { get; init; } = default!;
    }

    /// <summary>
    /// zm_push で渡した IFC（IFC-SPF か ifcZIP）を、zm_push で渡した EXPRESS のスキーマに照らしてチェックする（決定 0207・0216）。インスタンスの実体の名前・抽象・属性の数・値の型・省略を見る。WHERE・UNIQUE・RULE の規則は評価せず、数を返す。zumen は EXPRESS を同梱しない。小版 42 から
    /// </summary>
    public sealed record ValidateRequest
    {
        /// <summary>
        /// メッセージのスキーマの大版。今は 1
        /// </summary>
        [JsonPropertyName("v")]
        public ulong V { get; init; }

        /// <summary>
        /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        /// </summary>
        [JsonPropertyName("budget")]
        public Budget? Budget { get; init; }

        /// <summary>
        /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        /// </summary>
        [JsonPropertyName("partial")]
        public bool? Partial { get; init; }

        /// <summary>
        /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        /// </summary>
        [JsonPropertyName("max_places")]
        public ulong? MaxPlaces { get; init; }

        /// <summary>
        /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        /// </summary>
        [JsonPropertyName("time_limit_ms")]
        public ulong? TimeLimitMs { get; init; }

        /// <summary>
        /// op の名前
        /// </summary>
        [JsonPropertyName("op")]
        public string Op { get; init; } = "validate";

        /// <summary>
        /// IFC の入力の名前（zm_push で渡した .ifc か .ifczip）
        /// </summary>
        [JsonPropertyName("input")]
        public string Input { get; init; } = default!;

        /// <summary>
        /// EXPRESS の入力の名前（zm_push で渡した .exp）
        /// </summary>
        [JsonPropertyName("express")]
        public string Express { get; init; } = default!;

        /// <summary>
        /// 返す最初の指摘の位置。前の応答の next_offset を渡す。書かなければ 0
        /// </summary>
        [JsonPropertyName("offset")]
        public ulong? Offset { get; init; }

        /// <summary>
        /// 返す指摘の数の上限。書かなければ 1000
        /// </summary>
        [JsonPropertyName("limit")]
        public uint? Limit { get; init; }
    }

    /// <summary>
    /// スキーマに照らしたチェックの結果。指摘は offset から limit 個まで。スキーマの名前の違いと評価しない規則は、診断で返す
    /// </summary>
    public sealed record ValidateResult
    {
        /// <summary>
        /// EXPRESS のスキーマの名前（大文字）
        /// </summary>
        [JsonPropertyName("schema")]
        public string Schema { get; init; } = default!;

        /// <summary>
        /// IFC の FILE_SCHEMA の名前
        /// </summary>
        [JsonPropertyName("file_schema")]
        public string FileSchema { get; init; } = default!;

        /// <summary>
        /// EXPRESS のバイト列のハッシュ（v2: と 32 桁の 16 進）。どの EXPRESS でチェックしたかを残す
        /// </summary>
        [JsonPropertyName("express_hash")]
        public string ExpressHash { get; init; } = default!;

        /// <summary>
        /// 評価しなかった規則（WHERE・UNIQUE・RULE）の数。確認できなかった項目
        /// </summary>
        [JsonPropertyName("rules_not_evaluated")]
        public ulong RulesNotEvaluated { get; init; }

        /// <summary>
        /// 指摘。インスタンスの番号の順に、属性の順。診断と同じ形で、describe で文にできる。count は 1
        /// </summary>
        [JsonPropertyName("findings")]
        public IReadOnlyList<Diagnostic> Findings { get; init; } = default!;

        /// <summary>
        /// 全体の指摘の数
        /// </summary>
        [JsonPropertyName("total")]
        public ulong Total { get; init; }

        /// <summary>
        /// 続きの最初の指摘の位置。続きがなければ持たない
        /// </summary>
        [JsonPropertyName("next_offset")]
        public ulong? NextOffset { get; init; }
    }

    /// <summary>
    /// レビューの文書（決定 0214・0218）の 1 つの対象を、開いた図面の文書の版へ移す。要素の参照を図面の要素の ID で引き、参照ごとに unchanged・changed・removed を返す。場所と図形に元の版のハッシュと状態を書き、対象のハッシュを文書のハッシュにする。2D の点は動かさない。移した文書は blobs の file で返す。小版 43 から
    /// </summary>
    public sealed record ReviewCarryRequest
    {
        /// <summary>
        /// メッセージのスキーマの大版。今は 1
        /// </summary>
        [JsonPropertyName("v")]
        public ulong V { get; init; }

        /// <summary>
        /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        /// </summary>
        [JsonPropertyName("budget")]
        public Budget? Budget { get; init; }

        /// <summary>
        /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        /// </summary>
        [JsonPropertyName("partial")]
        public bool? Partial { get; init; }

        /// <summary>
        /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        /// </summary>
        [JsonPropertyName("max_places")]
        public ulong? MaxPlaces { get; init; }

        /// <summary>
        /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        /// </summary>
        [JsonPropertyName("time_limit_ms")]
        public ulong? TimeLimitMs { get; init; }

        /// <summary>
        /// op の名前
        /// </summary>
        [JsonPropertyName("op")]
        public string Op { get; init; } = "review_carry";

        /// <summary>
        /// 新しい版の文書の名前。open で作ったもの
        /// </summary>
        [JsonPropertyName("doc")]
        public string Doc { get; init; } = default!;

        /// <summary>
        /// レビューの文書の入力の名前（zm_push で渡した JSON）
        /// </summary>
        [JsonPropertyName("review")]
        public string Review { get; init; } = default!;

        /// <summary>
        /// 移す対象の ID。レビューの文書の targets の id
        /// </summary>
        [JsonPropertyName("target")]
        public string Target { get; init; } = default!;

        /// <summary>
        /// 返す最初の参照の位置。前の応答の next_offset を渡す。書かなければ 0
        /// </summary>
        [JsonPropertyName("offset")]
        public ulong? Offset { get; init; }

        /// <summary>
        /// 返す参照の数の上限。書かなければ 1000
        /// </summary>
        [JsonPropertyName("limit")]
        public uint? Limit { get; init; }
    }

    /// <summary>
    /// 移した結果。参照は offset から limit 個まで。移した文書は blobs の file で返す
    /// </summary>
    public sealed record ReviewCarryResult
    {
        /// <summary>
        /// 移す前の対象のハッシュ
        /// </summary>
        [JsonPropertyName("from")]
        public string From { get; init; } = default!;

        /// <summary>
        /// 移した後の対象のハッシュ。文書のハッシュ
        /// </summary>
        [JsonPropertyName("hash")]
        public string Hash { get; init; } = default!;

        /// <summary>
        /// 要素の参照ごとの状態。指摘の順に、場所の順、参照の順
        /// </summary>
        [JsonPropertyName("references")]
        public IReadOnlyList<CarriedReference> References { get; init; } = default!;

        /// <summary>
        /// 全体の参照の数
        /// </summary>
        [JsonPropertyName("total")]
        public ulong Total { get; init; }

        /// <summary>
        /// 続きの最初の参照の位置。続きがなければ持たない
        /// </summary>
        [JsonPropertyName("next_offset")]
        public ulong? NextOffset { get; init; }
    }

    /// <summary>
    /// 移した要素の参照
    /// </summary>
    public sealed record CarriedReference
    {
        /// <summary>
        /// 指摘の ID
        /// </summary>
        [JsonPropertyName("topic")]
        public string Topic { get; init; } = default!;

        /// <summary>
        /// 場所の ID
        /// </summary>
        [JsonPropertyName("location")]
        public string Location { get; init; } = default!;

        /// <summary>
        /// 要素の ID
        /// </summary>
        [JsonPropertyName("element")]
        public string Element { get; init; } = default!;

        /// <summary>
        /// 状態
        /// </summary>
        [JsonPropertyName("state")]
        public string State { get; init; } = default!;
    }

    /// <summary>
    /// 移したときの状態（決定 0214）。unchanged は要素のハッシュが同じ、changed は違うか元のハッシュがない、removed は新しい版に要素がない
    /// </summary>
    public static class CarriedState
    {
        /// <summary>unchanged</summary>
        public const string Unchanged = "unchanged";
        /// <summary>changed</summary>
        public const string Changed = "changed";
        /// <summary>removed</summary>
        public const string Removed = "removed";
    }

    /// <summary>
    /// 開いた文書を check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0214・0218）に足す。指摘の ID は対象のハッシュ・規則・診断から決め、取り込み済みの指摘は足さない。足した文書は blobs の file で返す。小版 43 から
    /// </summary>
    public sealed record ReviewAdoptRequest
    {
        /// <summary>
        /// メッセージのスキーマの大版。今は 1
        /// </summary>
        [JsonPropertyName("v")]
        public ulong V { get; init; }

        /// <summary>
        /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        /// </summary>
        [JsonPropertyName("budget")]
        public Budget? Budget { get; init; }

        /// <summary>
        /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        /// </summary>
        [JsonPropertyName("partial")]
        public bool? Partial { get; init; }

        /// <summary>
        /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        /// </summary>
        [JsonPropertyName("max_places")]
        public ulong? MaxPlaces { get; init; }

        /// <summary>
        /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        /// </summary>
        [JsonPropertyName("time_limit_ms")]
        public ulong? TimeLimitMs { get; init; }

        /// <summary>
        /// op の名前
        /// </summary>
        [JsonPropertyName("op")]
        public string Op { get; init; } = "review_adopt";

        /// <summary>
        /// 照合する文書の名前。open で作ったもの
        /// </summary>
        [JsonPropertyName("doc")]
        public string Doc { get; init; } = default!;

        /// <summary>
        /// プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す
        /// </summary>
        [JsonPropertyName("profiles")]
        public IReadOnlyList<string> Profiles { get; init; } = default!;

        /// <summary>
        /// 照合に使うプロファイル。profiles の中から選ぶ
        /// </summary>
        [JsonPropertyName("profile")]
        public ProfileRef Profile { get; init; } = default!;

        /// <summary>
        /// たどる起点。実寸の空間・記号の定義の空間・用紙の ID。書かなければ、すべての用紙から、用紙がなければ実寸の空間からたどる
        /// </summary>
        [JsonPropertyName("from")]
        public string? From { get; init; }

        /// <summary>
        /// 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した要素と、そのビューの縮尺だけを照合する。from が用紙のときだけ書ける
        /// </summary>
        [JsonPropertyName("view")]
        public ulong? View { get; init; }

        /// <summary>
        /// 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。小版 40 から
        /// </summary>
        [JsonPropertyName("file_name")]
        public string? FileName { get; init; }

        /// <summary>
        /// 色の表の入力の名前（zm_push で渡したもの）。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く
        /// </summary>
        [JsonPropertyName("palette")]
        public string? Palette { get; init; }

        /// <summary>
        /// 足す先のレビューの文書の入力の名前（zm_push で渡した JSON）。書かなければ空の文書に足す
        /// </summary>
        [JsonPropertyName("review")]
        public string? Review { get; init; }

        /// <summary>
        /// 指摘が指す対象の ID。文書になければ、文書の形式とハッシュで足す
        /// </summary>
        [JsonPropertyName("target")]
        public string Target { get; init; } = default!;

        /// <summary>
        /// 取り込む指摘の番号の列。check の指摘の並びの順で、0 から数える
        /// </summary>
        [JsonPropertyName("findings")]
        public IReadOnlyList<ulong> Findings { get; init; } = default!;
    }

    /// <summary>
    /// 取り込んだ結果。足した文書は blobs の file で返す
    /// </summary>
    public sealed record ReviewAdoptResult
    {
        /// <summary>
        /// 足した指摘の ID。取り込み済みで足さなかった指摘は持たない
        /// </summary>
        [JsonPropertyName("added")]
        public IReadOnlyList<string> Added { get; init; } = default!;

        /// <summary>
        /// 足した後の文書の指摘の数
        /// </summary>
        [JsonPropertyName("topics")]
        public ulong Topics { get; init; }
    }

    /// <summary>
    /// 表の項目
    /// </summary>
    public sealed record TableItem
    {
        /// <summary>
        /// ID。表のレコードがハンドルを持てば layer/h10・line-type/h14、なければ読んだ順の番号（layer/2）。寸法スタイルは dimension-style/h1A か、寸法スタイルの並びの中の順（dimension-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 元の名前。翻訳しない
        /// </summary>
        [JsonPropertyName("name")]
        public string Name { get; init; } = default!;

        /// <summary>
        /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
        /// </summary>
        [JsonPropertyName("hash")]
        public string Hash { get; init; } = default!;
    }

    /// <summary>
    /// レイヤの表の項目（決定 0162）。小版 22 から、状態とスタイルを持つ
    /// </summary>
    public sealed record LayerItem
    {
        /// <summary>
        /// ID。表のレコードがハンドルを持てば layer/h10、なければ読んだ順の番号（layer/2）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 元の名前。翻訳しない
        /// </summary>
        [JsonPropertyName("name")]
        public string Name { get; init; } = default!;

        /// <summary>
        /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
        /// </summary>
        [JsonPropertyName("hash")]
        public string Hash { get; init; } = default!;

        /// <summary>
        /// ファイルに定義がある。偽なら、要素が名前で指すだけのレイヤで、ほかの欄を持たない。小版 22 から
        /// </summary>
        [JsonPropertyName("defined")]
        public bool Defined { get; init; }

        /// <summary>
        /// 表示する。小版 22 から
        /// </summary>
        [JsonPropertyName("visible")]
        public bool? Visible { get; init; }

        /// <summary>
        /// 印刷する。小版 22 から
        /// </summary>
        [JsonPropertyName("printable")]
        public bool? Printable { get; init; }

        /// <summary>
        /// 編集できる。小版 22 から
        /// </summary>
        [JsonPropertyName("editable")]
        public bool? Editable { get; init; }

        /// <summary>
        /// 凍結する。小版 22 から
        /// </summary>
        [JsonPropertyName("frozen")]
        public bool? Frozen { get; init; }

        /// <summary>
        /// 色。inherit を持たない。定義が色を持たなければ書かない。小版 22 から
        /// </summary>
        [JsonPropertyName("color")]
        public ColorChange? Color { get; init; }

        /// <summary>
        /// 線種の ID。定義が線種を持たなければ書かない。小版 22 から
        /// </summary>
        [JsonPropertyName("line_type_id")]
        public string? LineTypeId { get; init; }

        /// <summary>
        /// 線幅。inherit を持たない。定義が線幅を持たなければ書かない。小版 22 から
        /// </summary>
        [JsonPropertyName("line_weight")]
        public LineWeightChange? LineWeight { get; init; }
    }

    /// <summary>
    /// 線種の表の項目（決定 0164）。小版 26 から、定義を持つ
    /// </summary>
    public sealed record LineTypeItem
    {
        /// <summary>
        /// ID。表のレコードがハンドルを持てば line-type/h14、なければ読んだ順の番号（line-type/2）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 元の名前。翻訳しない
        /// </summary>
        [JsonPropertyName("name")]
        public string Name { get; init; } = default!;

        /// <summary>
        /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
        /// </summary>
        [JsonPropertyName("hash")]
        public string Hash { get; init; } = default!;

        /// <summary>
        /// ファイルに定義がある。偽なら、要素が名前で指すだけの線種で、ほかの欄を持たない。小版 26 から
        /// </summary>
        [JsonPropertyName("defined")]
        public bool Defined { get; init; }

        /// <summary>
        /// 元の説明。小版 26 から
        /// </summary>
        [JsonPropertyName("description")]
        public string? Description { get; init; }

        /// <summary>
        /// 線と間の長さの列。正は線、負は間、0 は点。空なら実線。長さの単位は paper が決める。小版 26 から
        /// </summary>
        [JsonPropertyName("pattern")]
        public IReadOnlyList<double>? Pattern { get; init; }

        /// <summary>
        /// 列の長さが用紙の mm なら真、文書の単位なら偽。用紙の mm の列は line_type で変えられない。小版 26 から
        /// </summary>
        [JsonPropertyName("paper")]
        public bool? Paper { get; init; }

        /// <summary>
        /// 列の要素に形か文字を添える。添えれば、line_type で列の要素の数を変えられない。小版 26 から
        /// </summary>
        [JsonPropertyName("marked")]
        public bool? Marked { get; init; }

        /// <summary>
        /// 列の要素に添える形と文字（決定 0150・0166）。添えなければ空。小版 28 から
        /// </summary>
        [JsonPropertyName("marks")]
        public IReadOnlyList<LineTypeMark>? Marks { get; init; }
    }

    /// <summary>
    /// 寸法スタイルの表の項目（決定 0165）。小版 27 から、端部記号と大きさを持つ
    /// </summary>
    public sealed record DimensionStyleItem
    {
        /// <summary>
        /// ID。寸法スタイルのレコードがハンドルを持てば dimension-style/h1A、なければ寸法スタイルの並びの中の順（dimension-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 元の名前。翻訳しない
        /// </summary>
        [JsonPropertyName("name")]
        public string Name { get; init; } = default!;

        /// <summary>
        /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
        /// </summary>
        [JsonPropertyName("hash")]
        public string Hash { get; init; } = default!;

        /// <summary>
        /// 寸法線の 1 本目と 2 本目の補助線の側の端部記号の、ブロックの名前。空の名前は既定の矢印。小版 27 から
        /// </summary>
        [JsonPropertyName("ends")]
        public IReadOnlyList<string>? Ends { get; init; }

        /// <summary>
        /// 引出線の端部記号の、ブロックの名前。空の名前は既定の矢印。小版 27 から
        /// </summary>
        [JsonPropertyName("leader")]
        public string? Leader { get; init; }

        /// <summary>
        /// 端部記号の大きさ。文書の単位で、scale を掛ける前の値。小版 27 から
        /// </summary>
        [JsonPropertyName("size")]
        public double? Size { get; init; }

        /// <summary>
        /// 端部記号の代わりに描く斜線の大きさ。0 なら斜線を描かない。小版 27 から
        /// </summary>
        [JsonPropertyName("tick")]
        public double? Tick { get; init; }

        /// <summary>
        /// 大きさに掛ける倍率。0 はビューの縮尺から決める。小版 27 から
        /// </summary>
        [JsonPropertyName("scale")]
        public double? Scale { get; init; }
    }

    /// <summary>
    /// 線種の列の要素に添える形か文字（決定 0150・0166）。要素の始まりに、線の向きとずれで置く。小版 28 から
    /// </summary>
    public sealed record LineTypeMark
    {
        /// <summary>
        /// 添える列の要素の番号（pattern の添字）
        /// </summary>
        [JsonPropertyName("index")]
        public ulong Index { get; init; }

        /// <summary>
        /// 形（shape）か文字（text）
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = default!;

        /// <summary>
        /// 形の形のファイル。元の形式が指す文字スタイルがなければ書かない
        /// </summary>
        [JsonPropertyName("font")]
        public Font? Font { get; init; }

        /// <summary>
        /// 形の番号
        /// </summary>
        [JsonPropertyName("number")]
        public ulong? Number { get; init; }

        /// <summary>
        /// 文字の文字列
        /// </summary>
        [JsonPropertyName("text")]
        public string? Text { get; init; }

        /// <summary>
        /// 文字の文字スタイルの元の名前。元の形式が指す文字スタイルがなければ書かない
        /// </summary>
        [JsonPropertyName("style")]
        public string? Style { get; init; }

        /// <summary>
        /// 文字の文字スタイルの ID。tables の text_styles の ID で、名前で引いたもの（大文字と小文字を区別せず、同じ名前があれば先の項目）
        /// </summary>
        [JsonPropertyName("style_id")]
        public string? StyleId { get; init; }

        /// <summary>
        /// 文字の高さ。文字スタイルが決まった高さを持てば、それに scale を掛けたもの。持たなければ scale。単位は pattern と同じ
        /// </summary>
        [JsonPropertyName("height")]
        public double? Height { get; init; }

        /// <summary>
        /// 倍率。文字なら、文字スタイルの高さへの倍率
        /// </summary>
        [JsonPropertyName("scale")]
        public double Scale { get; init; }

        /// <summary>
        /// 回転（度、反時計回り）
        /// </summary>
        [JsonPropertyName("rotation")]
        public double Rotation { get; init; }

        /// <summary>
        /// 回転の基準
        /// </summary>
        [JsonPropertyName("rotation_basis")]
        public string RotationBasis { get; init; } = default!;

        /// <summary>
        /// 置く位置のずれ（x・y）。線の向きを x とする。単位は pattern と同じ
        /// </summary>
        [JsonPropertyName("offset")]
        public IReadOnlyList<double> Offset { get; init; } = default!;
    }

    /// <summary>
    /// 線種に添えるものの種類。shape は形のファイルの形、text は文字
    /// </summary>
    public static class LineTypeMarkKind
    {
        /// <summary>shape</summary>
        public const string Shape = "shape";
        /// <summary>text</summary>
        public const string Text = "text";
    }

    /// <summary>
    /// 回転の基準。relative は線の向きから、absolute は文書の x 軸から測る。upright は線の向きから測り、文字が逆さに見えるなら半回転する
    /// </summary>
    public static class MarkRotation
    {
        /// <summary>relative</summary>
        public const string Relative = "relative";
        /// <summary>absolute</summary>
        public const string Absolute = "absolute";
        /// <summary>upright</summary>
        public const string Upright = "upright";
    }

    /// <summary>
    /// 文字スタイルの表の項目（決定 0158）。フォントと縦書きは、この文字スタイルを指す文字に効く。高さ・幅の倍率・傾き・裏返し・上下反転は、新しく作る文字の既定の値で、今の文字はそれぞれの値を持つ。小版 20 から
    /// </summary>
    public sealed record TextStyleItem
    {
        /// <summary>
        /// ID。表のレコードがハンドルを持てば text-style/h11、なければ文字スタイルの並びの中の順（text-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
        /// </summary>
        [JsonPropertyName("id")]
        public string Id { get; init; } = default!;

        /// <summary>
        /// 元の名前。翻訳しない
        /// </summary>
        [JsonPropertyName("name")]
        public string Name { get; init; } = default!;

        /// <summary>
        /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
        /// </summary>
        [JsonPropertyName("hash")]
        public string Hash { get; init; } = default!;

        /// <summary>
        /// ファイルに定義がある。偽なら、ほかの欄を持たない
        /// </summary>
        [JsonPropertyName("defined")]
        public bool Defined { get; init; }

        /// <summary>
        /// 主のフォント。ファイルが決めていなければ書かない
        /// </summary>
        [JsonPropertyName("font")]
        public Font? Font { get; init; }

        /// <summary>
        /// 大きいフォント（漢字などの形のファイル）。なければ書かない
        /// </summary>
        [JsonPropertyName("big_font")]
        public Font? BigFont { get; init; }

        /// <summary>
        /// 決まった高さ。文書の単位。決めなければ書かない
        /// </summary>
        [JsonPropertyName("height")]
        public double? Height { get; init; }

        /// <summary>
        /// 幅の倍率
        /// </summary>
        [JsonPropertyName("width")]
        public double? Width { get; init; }

        /// <summary>
        /// 傾き（度）
        /// </summary>
        [JsonPropertyName("oblique")]
        public double? Oblique { get; init; }

        /// <summary>
        /// 左右を裏返す
        /// </summary>
        [JsonPropertyName("backward")]
        public bool? Backward { get; init; }

        /// <summary>
        /// 上下を反転する
        /// </summary>
        [JsonPropertyName("upside_down")]
        public bool? UpsideDown { get; init; }

        /// <summary>
        /// 縦書き
        /// </summary>
        [JsonPropertyName("vertical")]
        public bool? Vertical { get; init; }
    }

    /// <summary>
    /// フォントの指定。取得と字形の評価は使い手が行う
    /// </summary>
    public sealed record Font
    {
        /// <summary>
        /// 元の名前。形のファイルならファイルの名前、名前で指すフォントなら書体の名前。翻訳しない
        /// </summary>
        [JsonPropertyName("name")]
        public string Name { get; init; } = default!;

        /// <summary>
        /// 種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = default!;

        /// <summary>
        /// 太字。名前で指すフォントだけ
        /// </summary>
        [JsonPropertyName("bold")]
        public bool? Bold { get; init; }

        /// <summary>
        /// 斜体。名前で指すフォントだけ
        /// </summary>
        [JsonPropertyName("italic")]
        public bool? Italic { get; init; }

        /// <summary>
        /// 文字集合の番号（128 は Shift_JIS）。名前で指すフォントで、元の形式が決めていれば
        /// </summary>
        [JsonPropertyName("charset")]
        public ulong? Charset { get; init; }

        /// <summary>
        /// ピッチと系統の番号。名前で指すフォントで、元の形式が決めていれば
        /// </summary>
        [JsonPropertyName("pitch")]
        public ulong? Pitch { get; init; }
    }

    /// <summary>
    /// フォントの種類。shape は形のファイル（SHX など）、named は名前で指すフォント（TrueType など）
    /// </summary>
    public static class FontKind
    {
        /// <summary>shape</summary>
        public const string Shape = "shape";
        /// <summary>named</summary>
        public const string Named = "named";
    }

    /// <summary>
    /// 要素の出所。L1 のレコードの番号かバイト範囲（決定 0008）と、形式が持つ鍵（決定 0156）。ファイルから読んでいない要素は、レコードの番号もバイト範囲も持たない
    /// </summary>
    public sealed record Source
    {
        /// <summary>
        /// L1 のレコードの番号
        /// </summary>
        [JsonPropertyName("record")]
        public ulong? Record { get; init; }

        /// <summary>
        /// L1 のバイト範囲
        /// </summary>
        [JsonPropertyName("bytes")]
        public ByteRange? Bytes { get; init; }

        /// <summary>
        /// AutoCAD の系統のハンドル。大文字の 16 進。書き戻しても変わらない
        /// </summary>
        [JsonPropertyName("handle")]
        public string? Handle { get; init; }

        /// <summary>
        /// SXF のインスタンスの番号（#n の n）。10 進の文字列。小版 6 から
        /// </summary>
        [JsonPropertyName("instance")]
        public string? Instance { get; init; }
    }

    /// <summary>
    /// 応答。失敗したときも同じ形で、result を持たない
    /// </summary>
    public sealed record Response<TResult>
    {
        /// <summary>
        /// メッセージのスキーマの大版
        /// </summary>
        [JsonPropertyName("v")]
        public uint V { get; init; }

        /// <summary>
        /// 診断。入力の順
        /// </summary>
        [JsonPropertyName("diagnostics")]
        public IReadOnlyList<Diagnostic> Diagnostics { get; init; } = default!;

        /// <summary>
        /// バイナリの結果。Take で取り出す。次の Call で捨てる
        /// </summary>
        [JsonPropertyName("blobs")]
        public IReadOnlyList<Blob> Blobs { get; init; } = default!;

        /// <summary>
        /// op ごとの結果（Operations が引く結果の型）。失敗したときは持たない
        /// </summary>
        [JsonPropertyName("result")]
        public TResult? Result { get; init; }
    }

    /// <summary>
    /// 診断。人向けの文は持たず、文は describe で組み立てる（決定 0009）
    /// </summary>
    public sealed record Diagnostic
    {
        /// <summary>
        /// 変わらない名前（budget.records.exceeded など）。catalog/codes.toml にある
        /// </summary>
        [JsonPropertyName("code")]
        public string Code { get; init; } = default!;

        /// <summary>
        /// 重さ。code ごとに決まる
        /// </summary>
        [JsonPropertyName("severity")]
        public string Severity { get; init; } = default!;

        /// <summary>
        /// 変換の結果の段。code ごとに決まる。変換の結果を表さない code では持たない
        /// </summary>
        [JsonPropertyName("fidelity")]
        public string? Fidelity { get; init; }

        /// <summary>
        /// まとめた件数
        /// </summary>
        [JsonPropertyName("count")]
        public ulong Count { get; init; }

        /// <summary>
        /// 場所。入力の順に先頭の数件
        /// </summary>
        [JsonPropertyName("at")]
        public IReadOnlyList<Place> At { get; init; } = default!;

        /// <summary>
        /// 仕様の節
        /// </summary>
        [JsonPropertyName("cite")]
        public Cite? Cite { get; init; }

        /// <summary>
        /// 文に差し込む値。名前と型は code ごとに台帳が決める。2^53 − 1 を超える整数は文字列にする
        /// </summary>
        [JsonPropertyName("params")]
        public IReadOnlyDictionary<string, JsonAny> Params { get; init; } = default!;
    }

    /// <summary>
    /// 診断の重さ（決定 0009）。info は何も失っていない、warning は結果を使える、error は失敗した
    /// </summary>
    public static class Severity
    {
        /// <summary>info</summary>
        public const string Info = "info";
        /// <summary>warning</summary>
        public const string Warning = "warning";
        /// <summary>error</summary>
        public const string Error = "error";
    }

    /// <summary>
    /// 変換の結果の段のうち、`native` 以外のもの（決定 0009）
    /// </summary>
    public static class Fidelity
    {
        /// <summary>equivalent</summary>
        public const string Equivalent = "equivalent";
        /// <summary>approximated</summary>
        public const string Approximated = "approximated";
        /// <summary>drawn</summary>
        public const string Drawn = "drawn";
        /// <summary>dropped</summary>
        public const string Dropped = "dropped";
    }

    /// <summary>
    /// 診断の場所。ID と、L1 のレコード・バイト範囲（決定 0008 の出所）
    /// </summary>
    public sealed record Place
    {
        /// <summary>
        /// ID
        /// </summary>
        [JsonPropertyName("id")]
        public string? Id { get; init; }

        /// <summary>
        /// L1 のレコードの番号
        /// </summary>
        [JsonPropertyName("record")]
        public ulong? Record { get; init; }

        /// <summary>
        /// L1 のバイト範囲
        /// </summary>
        [JsonPropertyName("bytes")]
        public ByteRange? Bytes { get; init; }
    }

    /// <summary>
    /// バイト範囲（start から end の手前まで）
    /// </summary>
    public sealed record ByteRange
    {
        /// <summary>
        /// 先頭の位置
        /// </summary>
        [JsonPropertyName("start")]
        public ulong Start { get; init; }

        /// <summary>
        /// 終わりの位置（含まない）
        /// </summary>
        [JsonPropertyName("end")]
        public ulong End { get; init; }
    }

    /// <summary>
    /// 仕様の節。仕様の本文は写さない
    /// </summary>
    public sealed record Cite
    {
        /// <summary>
        /// 仕様の一覧での名前
        /// </summary>
        [JsonPropertyName("spec")]
        public string Spec { get; init; } = default!;

        /// <summary>
        /// 節
        /// </summary>
        [JsonPropertyName("section")]
        public string Section { get; init; } = default!;
    }

    /// <summary>
    /// 診断の値。数か文字列
    /// - 数: 数
    /// - 文字列: 文字列。図面に記録された名前、列挙の値のキー、2^53 − 1 を超える整数
    /// </summary>
    public static class ParamValue
    {
    }

    /// <summary>
    /// バイナリの結果
    /// </summary>
    public sealed record Blob
    {
        /// <summary>
        /// 番号。zm_take に渡す
        /// </summary>
        [JsonPropertyName("id")]
        public uint Id { get; init; }

        /// <summary>
        /// 種類
        /// </summary>
        [JsonPropertyName("kind")]
        public string Kind { get; init; } = default!;

        /// <summary>
        /// バイト数
        /// </summary>
        [JsonPropertyName("size")]
        public ulong Size { get; init; }
    }

    /// <summary>
    /// バイナリの結果の種類。file は書き出したファイル、image は描いた画像、scene は 2D のシーン（schema/scene.toml の配置、決定 0137・0172）、arrow は表（Arrow の IPC ストリーム。ネイティブでは zm_take_arrow でも取り出せる）
    /// </summary>
    public static class BlobKind
    {
        /// <summary>file</summary>
        public const string File = "file";
        /// <summary>image</summary>
        public const string Image = "image";
        /// <summary>scene</summary>
        public const string Scene = "scene";
        /// <summary>arrow</summary>
        public const string Arrow = "arrow";
    }

    /// <summary>
    /// zm_session_new に渡す設定。渡さなければ既定の設定
    /// </summary>
    public sealed record SessionOptions
    {
        /// <summary>
        /// メッセージのスキーマの大版。今は 1
        /// </summary>
        [JsonPropertyName("v")]
        public ulong V { get; init; }

        /// <summary>
        /// 実行の形。書かなければ、ネイティブのライブラリでは `native`、wasm のモジュールでは `wasm`（決定 0013）
        /// </summary>
        [JsonPropertyName("engine")]
        public string? Engine { get; init; }

        /// <summary>
        /// engine が `process` のときの、子プロセスにする CLI（zumen）の実行ファイルのパス。書かなければ、ライブラリと同じフォルダーの zumen を使う（決定 0013・0200）
        /// </summary>
        [JsonPropertyName("process_path")]
        public string? ProcessPath { get; init; }
    }

    /// <summary>
    /// 実行の形（決定 0013）。`native` はプロセスの中でコアを動かす、`process` は子プロセスの CLI に送る、`wasm` は wasm のモジュールで動かす
    /// </summary>
    public static class Engine
    {
        /// <summary>native</summary>
        public const string Native = "native";
        /// <summary>process</summary>
        public const string Process = "process";
        /// <summary>wasm</summary>
        public const string Wasm = "wasm";
    }

    /// <summary>
    /// describe に使うカタログのファイル。catalog/&lt;言語&gt;.toml から生成し、zm_push で渡す（決定 0009）
    /// </summary>
    public sealed record Catalog
    {
        /// <summary>
        /// カタログの形の大版。今は 1
        /// </summary>
        [JsonPropertyName("v")]
        public ulong V { get; init; }

        /// <summary>
        /// 言語（ja・en）
        /// </summary>
        [JsonPropertyName("lang")]
        public string Lang { get; init; } = default!;

        /// <summary>
        /// パッケージの版。コアと同じ版で出す
        /// </summary>
        [JsonPropertyName("package")]
        public string Package { get; init; } = default!;

        /// <summary>
        /// 数の書き方
        /// </summary>
        [JsonPropertyName("number")]
        public NumberFormat Number { get; init; } = default!;

        /// <summary>
        /// code ごとの文のひな形。{名前} に値が、{count} にまとめた件数が入る
        /// </summary>
        [JsonPropertyName("codes")]
        public IReadOnlyDictionary<string, string> Codes { get; init; } = default!;
    }

    /// <summary>
    /// 文に差し込む数の書き方。OS のロケールは使わない（決定 0009）
    /// </summary>
    public sealed record NumberFormat
    {
        /// <summary>
        /// 整数部を 3 桁ごとに区切る文字。空なら区切らない
        /// </summary>
        [JsonPropertyName("group_separator")]
        public string GroupSeparator { get; init; } = default!;

        /// <summary>
        /// 小数点の文字
        /// </summary>
        [JsonPropertyName("decimal_separator")]
        public string DecimalSeparator { get; init; } = default!;
    }

    /// <summary>op ごとの要求と結果の型。結果は Response の Result の型</summary>
    public static class Operations
    {
        /// <summary>op の名前と、要求と結果の型</summary>
        public static readonly IReadOnlyDictionary<string, (global::System.Type Request, global::System.Type Result)> Types =
            new Dictionary<string, (global::System.Type Request, global::System.Type Result)>
            {
                ["capabilities"] = (typeof(CapabilitiesRequest), typeof(CapabilitiesResult)),
                ["open"] = (typeof(OpenRequest), typeof(OpenResult)),
                ["close"] = (typeof(CloseRequest), typeof(CloseResult)),
                ["describe"] = (typeof(DescribeRequest), typeof(DescribeResult)),
                ["drawing"] = (typeof(DrawingRequest), typeof(DrawingResult)),
                ["entities"] = (typeof(EntitiesRequest), typeof(EntitiesResult)),
                ["apply"] = (typeof(ApplyRequest), typeof(ApplyResult)),
                ["convert"] = (typeof(ConvertRequest), typeof(ConvertResult)),
                ["tables"] = (typeof(TablesRequest), typeof(TablesResult)),
                ["scene"] = (typeof(SceneRequest), typeof(SceneResult)),
                ["check"] = (typeof(CheckRequest), typeof(CheckResult)),
                ["validate"] = (typeof(ValidateRequest), typeof(ValidateResult)),
                ["review_carry"] = (typeof(ReviewCarryRequest), typeof(ReviewCarryResult)),
                ["review_adopt"] = (typeof(ReviewAdoptRequest), typeof(ReviewAdoptResult)),
            };
    }
}
