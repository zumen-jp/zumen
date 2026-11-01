# 生成したファイル。手で直さない。
# 元は schema/messages.json で、`cargo xtask codegen` で作り直す。
#
# 境界のメッセージの型（決定 0012・0195）。スキーマの定義ごとに 1 つの型がある。
# 列挙は、知らない値も受け取れるように、ほかの文字列も許す（決定 0015）。
# Python 3.9 で読めるように、TypedDict を関数の形で書き、定義の名前は文字列で引く。

from typing import Any, Dict, List, Literal, TypedDict, Union

# 要求。op の値で形が決まる
Request = Union[
    "CapabilitiesRequest",
    "OpenRequest",
    "CloseRequest",
    "DescribeRequest",
    "DrawingRequest",
    "EntitiesRequest",
    "ApplyRequest",
    "ConvertRequest",
    "TablesRequest",
    "SceneRequest",
    "CheckRequest",
    "ValidateRequest",
    "ReviewCarryRequest",
    "ReviewAdoptRequest",
    "BuildingRequest",
    "BuildingElementsRequest",
    "IdsCheckRequest",
    "ReviewAdoptIdsRequest",
    "BcfReadRequest",
    "BcfWriteRequest",
    "CivilRequest",
    "PointcloudRequest",
]


# どの op の要求にも置ける欄
_RequestCommonRequired = TypedDict(
    "_RequestCommonRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
    },
)
_RequestCommonOptional = TypedDict(
    "_RequestCommonOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
    },
    total=False,
)


class RequestCommon(_RequestCommonRequired, _RequestCommonOptional):
    "どの op の要求にも置ける欄"


# 予算の単位ごとの上限（決定 0010）
Budget = TypedDict(
    "Budget",
    {
        # 受け取ったバイト数
        "input_bytes": "Limit",
        # 展開・復号で作ったバイト数。入れ子の分も足す
        "decoded_bytes": "Limit",
        # L0・L1 で読んだレコードの数
        "records": "Limit",
        # L2・L3 で作った要素の数
        "elements": "Limit",
        # 入れ子と再帰の深さ
        "depth": "Limit",
        # 幾何の分割で作った点・線・面の数
        "segments": "Limit",
        # ほかの単位に入らない繰り返しの回数
        "work": "Limit",
    },
    total=False,
)

# 1 つの単位の上限。上限をなくすときは unlimited と書く
Limit = Union[
    # この量までは使える
    int,
    # 上限なし
    Literal["unlimited"],
]


# コアの版と、使える op・形式を尋ねる
_CapabilitiesRequestRequired = TypedDict(
    "_CapabilitiesRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["capabilities"],
    },
)
_CapabilitiesRequestOptional = TypedDict(
    "_CapabilitiesRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
    },
    total=False,
)


class CapabilitiesRequest(_CapabilitiesRequestRequired, _CapabilitiesRequestOptional):
    "コアの版と、使える op・形式を尋ねる"


# コアの版と、使える op・形式（決定 0015）
CapabilitiesResult = TypedDict(
    "CapabilitiesResult",
    {
        # パッケージの版（semver）
        "package": str,
        # ABI の大版。zm_abi_version と同じ
        "abi": int,
        # メッセージのスキーマの版
        "schema": "SchemaVersion",
        # このコア（wasm ではこのモジュール）で使える op の名前
        "ops": List[str],
        # 読み書きできる形式
        "formats": List["Format"],
    },
)


# メッセージのスキーマの版。大版は要求と応答の v、小版は op・欄・列挙の値を足すたびに上げる
SchemaVersion = TypedDict(
    "SchemaVersion",
    {
        # 大版
        "major": int,
        # 小版
        "minor": int,
    },
)


# 読み書きできる形式
Format = TypedDict(
    "Format",
    {
        # 形式の名前（dxf・sfc など）
        "id": str,
        # 形式の系統（autocad・sxf など、決定 0008）
        "family": str,
        # 読めるか
        "read": bool,
        # 書けるか
        "write": bool,
        # この形式の ID の規則の版（決定 0015）
        "id_rules": int,
    },
)


# 入力を読んで、文書を作る。形式は入力から判定する
_OpenRequestRequired = TypedDict(
    "_OpenRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["open"],
        # 読む入力の名前。zm_push で渡したもの
        "input": str,
    },
)
_OpenRequestOptional = TypedDict(
    "_OpenRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 作る文書の名前。書かなければ入力の名前
        "doc": str,
        # 入力の全体の大きさ（バイト）。渡した入力より大きければ、入力を全体の初めの部分とみなし、足りない範囲を needs で求める。範囲で読める形式（LAS・LAZ）だけが使う（決定 0232）。小版 48 から
        "size": int,
    },
    total=False,
)


class OpenRequest(_OpenRequestRequired, _OpenRequestOptional):
    "入力を読んで、文書を作る。形式は入力から判定する"


# 作った文書
_OpenResultRequired = TypedDict(
    "_OpenResultRequired",
    {
        # 文書の名前
        "doc": str,
        # 判定した形式の名前
        "format": str,
    },
)
_OpenResultOptional = TypedDict(
    "_OpenResultOptional",
    {
        # 読むのに足りない範囲。あれば文書はまだ作らない。範囲をその名前で zm_push してから、同じ要求をもう一度呼ぶ（決定 0232）。小版 48 から
        "needs": List["InputRange"],
        # 読み終えたので、close の inputs で捨ててよい範囲の入力。小版 48 から
        "done": List["InputRange"],
    },
    total=False,
)


class OpenResult(_OpenResultRequired, _OpenResultOptional):
    "作った文書"


# 入力の範囲と、その範囲を渡す入力の名前
InputRange = TypedDict(
    "InputRange",
    {
        # 範囲を渡す入力の名前。<入力の名前>#<start>-<end>
        "input": str,
        # 範囲の初め（バイト）
        "start": int,
        # 範囲の終わり（バイト、含まない）
        "end": int,
    },
)


# 文書と入力を捨てる。名指ししたものが 1 つでもなければ、何も捨てない
_CloseRequestRequired = TypedDict(
    "_CloseRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["close"],
    },
)
_CloseRequestOptional = TypedDict(
    "_CloseRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 捨てる文書の名前
        "docs": List[str],
        # 捨てる入力の名前
        "inputs": List[str],
    },
    total=False,
)


class CloseRequest(_CloseRequestRequired, _CloseRequestOptional):
    "文書と入力を捨てる。名指ししたものが 1 つでもなければ、何も捨てない"


# 捨てたこと。中身はない
CloseResult = TypedDict("CloseResult", {})


# 診断の文を、カタログから組み立てる（決定 0009）
_DescribeRequestRequired = TypedDict(
    "_DescribeRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["describe"],
        # 文にする診断。応答で受け取ったもの
        "diagnostics": List["Diagnostic"],
    },
)
_DescribeRequestOptional = TypedDict(
    "_DescribeRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # カタログの入力の名前。zm_push で渡したもの。書かなければ、code と値をそのまま文にする
        "catalog": str,
    },
    total=False,
)


class DescribeRequest(_DescribeRequestRequired, _DescribeRequestOptional):
    "診断の文を、カタログから組み立てる（決定 0009）"


# 診断の文
DescribeResult = TypedDict(
    "DescribeResult",
    {
        # 診断ごとの文。要求の diagnostics と同じ順。カタログにない code は、code と値をそのまま書く
        "texts": List[str],
    },
)


# 開いた文書を図面モデル（L2、決定 0022）に変換し、その要約を返す。ネイティブと wasm の出力の一致を、適合試験で比べるためにも使う（決定 0007）
_DrawingRequestRequired = TypedDict(
    "_DrawingRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["drawing"],
        # 文書の名前。open で作ったもの
        "doc": str,
    },
)
_DrawingRequestOptional = TypedDict(
    "_DrawingRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
    },
    total=False,
)


class DrawingRequest(_DrawingRequestRequired, _DrawingRequestOptional):
    "開いた文書を図面モデル（L2、決定 0022）に変換し、その要約を返す。ネイティブと wasm の出力の一致を、適合試験で比べるためにも使う（決定 0007）"


# 図面モデルの要約。移さなかった図形と値は、診断で返す
_DrawingResultRequired = TypedDict(
    "_DrawingResultRequired",
    {
        # 実寸の空間。モデルの順
        "spaces": List["SpaceSummary"],
        # 用紙。モデルの順
        "sheets": List["SheetSummary"],
    },
)
_DrawingResultOptional = TypedDict(
    "_DrawingResultOptional",
    {
        # 文書のハッシュ。open に渡したバイト列から作る。変更の一覧の前提条件に使う（v2: と 32 桁の 16 進、決定 0156）
        "hash": str,
        # 文書の点の表し方（marker を持たない点の見え方、決定 0160）。point_style の変更で変える。文書が持たなければ（JWW・SXF）持たない。小版 39 から
        "point_style": "PointStyleSummary",
    },
    total=False,
)


class DrawingResult(_DrawingResultRequired, _DrawingResultOptional):
    "図面モデルの要約。移さなかった図形と値は、診断で返す"


# 文書の点の表し方。欄の意味は point_style_edit と同じ
PointStyleSummary = TypedDict(
    "PointStyleSummary",
    {
        # 基本の形
        "mark": "PointMark",
        # 円で囲む
        "circle": bool,
        # 四角で囲む
        "square": bool,
        # 大きさ。意味は size_kind で決まる
        "size": float,
        # 大きさの意味
        "size_kind": "PointSizeKind",
    },
)


# 実寸の空間の要約
_SpaceSummaryRequired = TypedDict(
    "_SpaceSummaryRequired",
    {
        # 文書の単位
        "unit": "Unit",
        # 要素の数
        "elements": "ElementCounts",
    },
)
_SpaceSummaryOptional = TypedDict(
    "_SpaceSummaryOptional",
    {
        # 空間の ID（space/0）。entities の要素の ID の始まりと同じ（決定 0156）
        "id": str,
        # 線分の端点と文字の基点を囲む範囲。文書の単位。要素がなければ持たない
        "extent": "Extent",
    },
    total=False,
)


class SpaceSummary(_SpaceSummaryRequired, _SpaceSummaryOptional):
    "実寸の空間の要約"


# 用紙の要約
_SheetSummaryRequired = TypedDict(
    "_SheetSummaryRequired",
    {
        # 要素の数
        "elements": "ElementCounts",
        # ビューの数
        "views": int,
    },
)
_SheetSummaryOptional = TypedDict(
    "_SheetSummaryOptional",
    {
        # 用紙の ID。LAYOUT がハンドルを持てば sheet/h22、なければ読んだ順の番号（sheet/0）。entities の要素の ID の始まりと同じ（決定 0156）
        "id": str,
        # 元の形式の用紙の名前（DXF・DWG のレイアウトの名前）。翻訳しない。なければ持たない
        "name": str,
        # 用紙の幅と高さ（mm）。分からなければ持たない
        "size": List[float],
        # 線分の端点と文字の基点を囲む範囲。用紙の mm。要素がなければ持たない
        "extent": "Extent",
    },
    total=False,
)


class SheetSummary(_SheetSummaryRequired, _SheetSummaryOptional):
    "用紙の要約"

# 実寸の単位
Unit = Union[
    Literal[
        "millimeter",
        "centimeter",
        "meter",
        "inch",
        "foot",
        "unknown",
    ],
    str,
]


# 種類ごとの要素の数
ElementCounts = TypedDict(
    "ElementCounts",
    {
        # 線分
        "lines": int,
        # 文字
        "texts": int,
    },
)


# 点を囲む、軸に沿った長方形
Extent = TypedDict(
    "Extent",
    {
        # X の最小
        "min_x": float,
        # Y の最小
        "min_y": float,
        # X の最大
        "max_x": float,
        # Y の最大
        "max_y": float,
    },
)


# 開いた文書を図面モデル（L2、決定 0022）に変換し、要素の一覧を返す。実寸の空間、用紙の順に、それぞれの要素を入力の順に並べる（決定 0117）
_EntitiesRequestRequired = TypedDict(
    "_EntitiesRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["entities"],
        # 文書の名前。open で作ったもの
        "doc": str,
    },
)
_EntitiesRequestOptional = TypedDict(
    "_EntitiesRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 返す最初の要素の位置。前の応答の next_offset を渡す。書かなければ 0
        "offset": int,
        # 返す要素の数の上限。書かなければ 1000
        "limit": int,
        # 返す欄の名前。1 つ以上で、重ねない。id は書かなくても返す。書かなければ id・kind・layer・source を返す（決定 0118）
        "fields": List["EntityField"],
        # [最小の X, 最小の Y, 最大の X, 最大の Y]。範囲がこの長方形と重なるか接する要素だけを返す。空間か用紙それぞれの座標で比べる（決定 0118）
        "region": List[float],
        # 空間か用紙の ID（space/0・sheet/h22 など）。書けば、その空間か用紙の要素だけを返す。記号の定義の空間（space/h1F など。BLOCK のハンドルから作る、決定 0156）も指せ、書かなければ定義の中の要素は返さない。ない ID なら要素を返さない。小版 31 から（決定 0177）
        "place": str,
    },
    total=False,
)


class EntitiesRequest(_EntitiesRequestRequired, _EntitiesRequestOptional):
    "開いた文書を図面モデル（L2、決定 0022）に変換し、要素の一覧を返す。実寸の空間、用紙の順に、それぞれの要素を入力の順に並べる（決定 0117）"


# 要素の一覧の一部。移さなかった図形と値は、診断で返す
_EntitiesResultRequired = TypedDict(
    "_EntitiesResultRequired",
    {
        # offset から limit 個までの要素
        "items": List["Entity"],
        # 全体の要素の数
        "total": int,
    },
)
_EntitiesResultOptional = TypedDict(
    "_EntitiesResultOptional",
    {
        # 続きの最初の要素の位置。続きがなければ持たない
        "next_offset": int,
    },
    total=False,
)


class EntitiesResult(_EntitiesResultRequired, _EntitiesResultOptional):
    "要素の一覧の一部。移さなかった図形と値は、診断で返す"


# 図面モデルの要素
_EntityRequired = TypedDict(
    "_EntityRequired",
    {
        # 空間か用紙と、出所で作る ID（space/0/h2F、space/0/r12、sheet/1/b4096 など）。出所の部分は、形式が持つ鍵（ハンドル）があれば鍵で作り、書き戻しても前の要素が増えても変わらない。同じバイト列には、どの版でも同じ ID を返す（決定 0117・0156）
        "id": str,
    },
)
_EntityOptional = TypedDict(
    "_EntityOptional",
    {
        # 形の種類
        "kind": "EntityKind",
        # レイヤの元の名前。翻訳しない。図面モデルにレイヤがなければ持たない
        "layer": str,
        # 元の形式のレコード
        "source": "Source",
        # 形の定義から求めた範囲。文字の字形・線の太さ・記号の中身は含めない。囲む点がなければ持たない（決定 0118）
        "bbox": "Extent",
        # 要素のレイヤ・スタイル・形のハッシュ。出所の位置は含めない。変更の前提条件に使う（v2: と 32 桁の 16 進、決定 0156）
        "hash": str,
        # 形の値。線分・円・円弧・楕円・幅を持たない折れ線・1 点か 2 点の間に置く文字・書式つきの文字・ブロックの記号・制御点か当てはめる点で決まるスプライン・寸法・引出線（LEADER）だけが持つ。apply の replace と add に渡せる形（決定 0139）。小版 10 から
        "geometry": "Geometry",
    },
    total=False,
)


class Entity(_EntityRequired, _EntityOptional):
    "図面モデルの要素"

# entities で返す要素の欄（決定 0118）
EntityField = Union[
    Literal[
        "id",
        "kind",
        "layer",
        "source",
        "bbox",
        "hash",
        "geometry",
    ],
    str,
]

# 形の値。点は要素の空間か用紙の座標、長さは同じ単位（実寸の空間は文書の単位、用紙は mm）、角度は度で反時計回り。shape の値で形が決まる
Geometry = Union[
    "LineGeometry",
    "CircleGeometry",
    "ArcGeometry",
    "EllipseGeometry",
    "PolylineGeometry",
    "TextGeometry",
    "FormattedTextGeometry",
    "SymbolGeometry",
    "SplineGeometry",
    "DimensionGeometry",
    "LeaderGeometry",
    "PointGeometry",
    "CalloutGeometry",
]


# 線分
LineGeometry = TypedDict(
    "LineGeometry",
    {
        # 形の種類
        "shape": Literal["line"],
        # 始点 [X, Y]
        "start": List[float],
        # 終点 [X, Y]
        "end": List[float],
    },
)


# 円
CircleGeometry = TypedDict(
    "CircleGeometry",
    {
        # 形の種類
        "shape": Literal["circle"],
        # 中心 [X, Y]
        "center": List[float],
        # 半径。0 より大きい
        "radius": float,
    },
)


# 円弧。始まりの角度から終わりの角度へ、反時計回りにたどる
ArcGeometry = TypedDict(
    "ArcGeometry",
    {
        # 形の種類
        "shape": Literal["arc"],
        # 中心 [X, Y]
        "center": List[float],
        # 半径。0 より大きい
        "radius": float,
        # 始まりの角度（度）
        "start_angle": float,
        # 終わりの角度（度）
        "end_angle": float,
    },
)


# 楕円か楕円の弧。始まりの角度から終わりの角度へ、反時計回りにたどる。閉じた楕円は 0 度から 360 度。小版 11 から
EllipseGeometry = TypedDict(
    "EllipseGeometry",
    {
        # 形の種類
        "shape": Literal["ellipse"],
        # 中心 [X, Y]
        "center": List[float],
        # 長軸の端の点 [X, Y]。中心と違う点
        "major": List[float],
        # 短軸の長軸に対する比。0 より大きく 1 以下
        "ratio": float,
        # 始まりの角度（度）。楕円を円に引き延ばしたときの角度
        "start_angle": float,
        # 終わりの角度（度）。楕円を円に引き延ばしたときの角度
        "end_angle": float,
    },
)


# 頂点を結んだ折れ線。幅は持たない
_PolylineGeometryRequired = TypedDict(
    "_PolylineGeometryRequired",
    {
        # 形の種類
        "shape": Literal["polyline"],
        # 頂点の [X, Y] の並び。2 つ以上
        "points": List[List[float]],
        # 最後の頂点から最初の頂点へ結ぶ
        "closed": bool,
    },
)
_PolylineGeometryOptional = TypedDict(
    "_PolylineGeometryOptional",
    {
        # 頂点ごとの、次の頂点への弧の膨らみ（中心角の 4 分の 1 の正接。正は反時計回り）。points と同じ数。書かなければどれも 0
        "bulges": List[float],
    },
    total=False,
)


class PolylineGeometry(_PolylineGeometryRequired, _PolylineGeometryOptional):
    "頂点を結んだ折れ線。幅は持たない"


# 1 点か 2 点の間に置く文字。置き換えるときは、位置合わせ・幅の倍率・斜体・反転・文字スタイルを元の値のままにする。足すときは、2 点目がなければ左の基準線に合わせ、幅の倍率 1・斜体なし・反転なしにする
_TextGeometryRequired = TypedDict(
    "_TextGeometryRequired",
    {
        # 形の種類
        "shape": Literal["text"],
        # 内容。1 行
        "content": str,
        # 基点 [X, Y]。2 点の間に合わせる文字では 1 点目
        "position": List[float],
        # 高さ。0 より大きい
        "height": float,
        # 回転（度）。2 点の間に合わせる文字では、1 点目から 2 点目への向きと同じでなければならない
        "rotation": float,
    },
)
_TextGeometryOptional = TypedDict(
    "_TextGeometryOptional",
    {
        # 2 点の間に合わせる文字の 2 点目 [X, Y]。合わせ方（fit）と一緒に書く。置き換えでは、1 点に置く文字と 2 点の間に合わせる文字を入れ替えられない。小版 17 から
        "end": List[float],
        # 2 点の間への合わせ方。2 点目（end）と一緒に書く。小版 17 から
        "fit": "TextFit",
        # 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。DXF に文字スタイルの定義がないときは Standard だけを受ける。置き換えるときは、元と同じ名前だけを受ける。style_id と一緒に書くなら、同じ文字スタイルを指す。小版 12 から
        "style": str,
        # 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。小版 23 から
        "style_id": str,
    },
    total=False,
)


class TextGeometry(_TextGeometryRequired, _TextGeometryOptional):
    "1 点か 2 点の間に置く文字。置き換えるときは、位置合わせ・幅の倍率・斜体・反転・文字スタイルを元の値のままにする。足すときは、2 点目がなければ左の基準線に合わせ、幅の倍率 1・斜体なし・反転なしにする"

# 2 点の間への合わせ方。aligned は縦横の比を保って高さも変え、fit は高さを保って幅だけを変える
TextFit = Union[
    Literal[
        "aligned",
        "fit",
    ],
    str,
]


# 書式つきの文字（DXF の MTEXT）。置き換えるときは、枠の高さ・書き進める向き・行送り・背景・高さの意味・文字スタイルを元の値のままにする。足すときは、枠の高さなし・左から右・行送り 1 倍・背景なしにする。小版 13 から
_FormattedTextGeometryRequired = TypedDict(
    "_FormattedTextGeometryRequired",
    {
        # 形の種類
        "shape": Literal["formatted_text"],
        # 内容。書式の符号（\P の改行など）を含めた、元の形式の内容のまま
        "content": str,
        # 基点 [X, Y]
        "position": List[float],
        # 高さ。0 より大きい
        "height": float,
        # 回転（度）
        "rotation": float,
    },
)
_FormattedTextGeometryOptional = TypedDict(
    "_FormattedTextGeometryOptional",
    {
        # 基点に合わせる枠の位置
        "attachment": "Attachment",
        # 折り返す枠の幅。0 より大きい。書かなければ折り返さない
        "width": float,
        # 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。置き換えるときは、元と同じ名前だけを受ける。style_id と一緒に書くなら、同じ文字スタイルを指す
        "style": str,
        # 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。小版 23 から
        "style_id": str,
    },
    total=False,
)


class FormattedTextGeometry(_FormattedTextGeometryRequired, _FormattedTextGeometryOptional):
    "書式つきの文字（DXF の MTEXT）。置き換えるときは、枠の高さ・書き進める向き・行送り・背景・高さの意味・文字スタイルを元の値のままにする。足すときは、枠の高さなし・左から右・行送り 1 倍・背景なしにする。小版 13 から"


# スプライン（NURBS）。制御点で決まるもの（小版 15 から）と、当てはめる点で決まるもの（小版 18 から）がある。当てはめる点で決まるものは、節点と制御点を空にし、当てはめる点（fit）と、あれば両端の接線を持つ。元の図形に制御点や節点もあるときは形の値を返さず、置き換えない。置き換えでは、制御点で決まるものと当てはめる点で決まるものを入れ替えない
_SplineGeometryRequired = TypedDict(
    "_SplineGeometryRequired",
    {
        # 形の種類
        "shape": Literal["spline"],
        # 次数。1 以上
        "degree": int,
        # 節点の並び。周期的でなければ、制御点の数と次数と 1 の和の数。当てはめる点で決まるものは空
        "knots": List[float],
        # 制御点の [X, Y] の並び。次数より多い。当てはめる点で決まるものは空
        "control": List[List[float]],
        # 閉じている
        "closed": bool,
        # 周期的
        "periodic": bool,
    },
)
_SplineGeometryOptional = TypedDict(
    "_SplineGeometryOptional",
    {
        # 制御点ごとの重み。0 より大きい。有理のスプラインだけが持ち、書かなければ有理でない
        "weights": List[float],
        # 当てはめる点の [X, Y] の並び。2 つ以上。書けば、節点と制御点は空で、重みは書かない。小版 18 から
        "fit": List[List[float]],
        # 当てはめる点で決まるスプラインの、始まりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。小版 18 から
        "start_tangent": List[float],
        # 当てはめる点で決まるスプラインの、終わりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。小版 18 から
        "end_tangent": List[float],
    },
    total=False,
)


class SplineGeometry(_SplineGeometryRequired, _SplineGeometryOptional):
    "スプライン（NURBS）。制御点で決まるもの（小版 15 から）と、当てはめる点で決まるもの（小版 18 から）がある。当てはめる点で決まるものは、節点と制御点を空にし、当てはめる点（fit）と、あれば両端の接線を持つ。元の図形に制御点や節点もあるときは形の値を返さず、置き換えない。置き換えでは、制御点で決まるものと当てはめる点で決まるものを入れ替えない"


# 寸法。種類（kind）ごとに、その種類の定義点の欄を持つ。測定値（measurement）は元の形式が保存した値で、定義点から測り直さない。寸法を描いた部品（ブロック）は形の値に含めない。entities が返すだけで、置き換えと追加は change.edit.unsupported になる（寸法を描いた部品を作り直す必要があるため）。小版 19 から
_DimensionGeometryRequired = TypedDict(
    "_DimensionGeometryRequired",
    {
        # 形の種類
        "shape": Literal["dimension"],
        # 寸法の種類
        "kind": "DimensionKind",
        # 寸法値の文字の書き方
        "label": "DimensionLabel",
        # 寸法スタイルの元の名前
        "style": str,
    },
)
_DimensionGeometryOptional = TypedDict(
    "_DimensionGeometryOptional",
    {
        # 1 本目の補助線の点か、測る角度の 1 本目の線の点か、円弧の始めの点 [X, Y]。linear・aligned・angular・arc_length が持つ
        "first": List[float],
        # 2 本目の補助線の点か、測る角度の 2 本目の線の点か、円弧の終わりの点 [X, Y]。linear・aligned・angular・arc_length が持つ
        "second": List[float],
        # 寸法線が通る点 [X, Y]。linear・aligned が持つ
        "line": List[float],
        # 測る向き（度）。linear が持つ
        "rotation": float,
        # 補助線の傾き（度）。傾けなければ 0。linear・aligned が持つ
        "oblique": float,
        # 角度の頂点 [X, Y]。angular が持つ
        "vertex": List[float],
        # 1 本目の線の始点と終点 [[X, Y], [X, Y]]。angular_lines が持つ
        "first_line": List[List[float]],
        # 2 本目の線の始点と終点 [[X, Y], [X, Y]]。angular_lines が持つ
        "second_line": List[List[float]],
        # 寸法の弧が通る点 [X, Y]。angular・angular_lines・arc_length が持つ
        "arc": List[float],
        # 中心 [X, Y]。arc_length・radius が持つ
        "center": List[float],
        # 円の上の点 [X, Y]。radius・diameter が持つ
        "chord": List[float],
        # chord と向かい合う円の上の点 [X, Y]。diameter が持つ
        "opposite": List[float],
        # 引出線の長さ。radius・diameter が持つ
        "leader": float,
        # 原点 [X, Y]。ordinate が持つ
        "origin": List[float],
        # 測る点 [X, Y]。ordinate が持つ
        "feature": List[float],
        # 引出線の終点 [X, Y]。ordinate が持つ
        "leader_end": List[float],
        # 測る座標の軸。ordinate が持つ
        "axis": "DimensionAxis",
        # 利用者が書いた寸法値の文字の元の内容。label が override のときだけ持つ。測定値に置き換える符号（DXF の <>）と書式の符号もそのまま持つ
        "content": str,
        # 元の形式が保存した測定値。単位と倍率は measurement_key の欄に従う
        "measurement": float,
        # measurement の形式と欄を指すキー（DXF の DIMENSION の 42 なら dxf.DIMENSION.42）。measurement と一緒に持つ
        "measurement_key": str,
        # 寸法値の文字の中央の点 [X, Y]。元の形式が持たなければ書かない
        "text_position": List[float],
        # 寸法値の文字の回転（度）。寸法線に沿わせるなら書かない
        "text_rotation": float,
        # 寸法スタイルの ID。tables が返したもの。名前で引いた寸法スタイルがなければ書かない。小版 23 から
        "style_id": str,
    },
    total=False,
)


class DimensionGeometry(_DimensionGeometryRequired, _DimensionGeometryOptional):
    "寸法。種類（kind）ごとに、その種類の定義点の欄を持つ。測定値（measurement）は元の形式が保存した値で、定義点から測り直さない。寸法を描いた部品（ブロック）は形の値に含めない。entities が返すだけで、置き換えと追加は change.edit.unsupported になる（寸法を描いた部品を作り直す必要があるため）。小版 19 から"


# 引出線（DXF・DWG の LEADER、決定 0177）。注記（MTEXT など）は別の要素で、annotation がその ID を指す。置き換えるときは寸法スタイルと注記を変えず、経路・曲線か・矢印を書き直す。足すときの注記は、同じ空間か用紙の MTEXT だけを指せる（小版 36 から。それまでは注記を持てなかった）。DWG は置き換えと追加を書き戻すが、矢印の印を必ず持つので、arrow を書かない引出線は書けない（小版 36 から。それまでは R2004 までの置き換えだけ）。小版 31 から
_LeaderGeometryRequired = TypedDict(
    "_LeaderGeometryRequired",
    {
        # 形の種類
        "shape": Literal["leader"],
        # 経路の頂点 [[X, Y], ...]。矢印の側から並べる。2 つ以上
        "path": List[List[float]],
        # 頂点を通る曲線で結ぶか。偽なら折れ線
        "spline": bool,
        # 寸法スタイルの元の名前。足すときは図面にある寸法スタイルの名前を指す。置き換えるときは元と同じ名前だけを受ける
        "style": str,
    },
)
_LeaderGeometryOptional = TypedDict(
    "_LeaderGeometryOptional",
    {
        # 始点に矢印を付けるか。書かなければ寸法スタイルに従う
        "arrow": bool,
        # 注記の要素の ID。同じ空間か用紙の要素で、entities が返したもの。注記がなければ書かない。置き換えるときは元と同じ ID だけを受ける。足すときは、同じ空間か用紙の書式つきの文字（MTEXT）を指す
        "annotation": str,
        # 寸法スタイルの ID。tables が返したもの。entities は、名前で引いた寸法スタイルの ID を返す。足す・置き換えるときは、名前と同じ寸法スタイルを指す ID だけを受ける
        "style_id": str,
    },
    total=False,
)


class LeaderGeometry(_LeaderGeometryRequired, _LeaderGeometryOptional):
    "引出線（DXF・DWG の LEADER、決定 0177）。注記（MTEXT など）は別の要素で、annotation がその ID を指す。置き換えるときは寸法スタイルと注記を変えず、経路・曲線か・矢印を書き直す。足すときの注記は、同じ空間か用紙の MTEXT だけを指せる（小版 36 から。それまでは注記を持てなかった）。DWG は置き換えと追加を書き戻すが、矢印の印を必ず持つので、arrow を書かない引出線は書けない（小版 36 から。それまでは R2004 までの置き換えだけ）。小版 31 から"


# 引出線つきの注記（DXF・DWG の MULTILEADER、決定 0185）。注記は形の中にあり、content と text・symbol で種類と中身を示す。置き換えるときは枝の線の頂点・集まる点・折れ曲がりだけを変えられ、ほかの欄は元の値のままにする。DXF だけが置き換えを書き戻し、枝と線の数と、集まる点と折れ曲がりの有無は元と同じでなければならない。足すことと DWG の書き戻しは、まだしない。小版 36 から
_CalloutGeometryRequired = TypedDict(
    "_CalloutGeometryRequired",
    {
        # 形の種類
        "shape": Literal["callout"],
        # 枝。入力の順
        "branches": List["CalloutBranch"],
        # 線の引き方
        "path": "CalloutPath",
        # 線を注記に、水平の線（ランディング）でつなぐか
        "landing": bool,
        # 端部記号の大きさ
        "arrow_size": float,
        # 大きさに掛ける倍率
        "scale": float,
        # 注記の種類
        "content": "CalloutContent",
    },
)
_CalloutGeometryOptional = TypedDict(
    "_CalloutGeometryOptional",
    {
        # 注記の書式つきの文字の元の内容。content が text のときだけ持つ
        "text": str,
        # 注記の記号のブロックの名前。content が symbol のときだけ持つ
        "symbol": str,
    },
    total=False,
)


class CalloutGeometry(_CalloutGeometryRequired, _CalloutGeometryOptional):
    "引出線つきの注記（DXF・DWG の MULTILEADER、決定 0185）。注記は形の中にあり、content と text・symbol で種類と中身を示す。置き換えるときは枝の線の頂点・集まる点・折れ曲がりだけを変えられ、ほかの欄は元の値のままにする。DXF だけが置き換えを書き戻し、枝と線の数と、集まる点と折れ曲がりの有無は元と同じでなければならない。足すことと DWG の書き戻しは、まだしない。小版 36 から"


# 引出線つきの注記の枝。何本かの線が 1 点に集まり、折れ曲がりで注記につながる
_CalloutBranchRequired = TypedDict(
    "_CalloutBranchRequired",
    {
        # 線ごとの頂点 [[[X, Y], ...], ...]。矢印の側から並べ、集まる点は含めない。線ごとに 1 つ以上
        "lines": List[List[List[float]]],
    },
)
_CalloutBranchOptional = TypedDict(
    "_CalloutBranchOptional",
    {
        # 線が集まる点 [X, Y]。元の形式が持たなければ書かない
        "end": List[float],
        # 集まる点から注記への折れ曲がり。なければ書かない
        "dogleg": "CalloutDogleg",
    },
    total=False,
)


class CalloutBranch(_CalloutBranchRequired, _CalloutBranchOptional):
    "引出線つきの注記の枝。何本かの線が 1 点に集まり、折れ曲がりで注記につながる"


# 枝の折れ曲がり
CalloutDogleg = TypedDict(
    "CalloutDogleg",
    {
        # 向き [X, Y]。長さは意味を持たない
        "direction": List[float],
        # 長さ
        "length": float,
    },
)

# 引出線つきの注記の線の引き方。straight は折れ線、spline は頂点を通る曲線、hidden は線を描かない
CalloutPath = Union[
    Literal[
        "straight",
        "spline",
        "hidden",
    ],
    str,
]

# 引出線つきの注記の中身の種類。none は注記を持たず、text は書式つきの文字、symbol は記号（バルーンなどの囲み）
CalloutContent = Union[
    Literal[
        "none",
        "text",
        "symbol",
    ],
    str,
]


# 点（決定 0160・0183）。marker がなければ文書の点の表し方（DXF・DWG の $PDMODE・$PDSIZE）に従う点で、DXF・DWG の POINT と JWW の記号でない点が当たる。marker があれば既定義の印の点で、SFC の点マーカーと JWW の記号の点が当たる。置き換えるときは、描き方（marker があるか）を元のまま保つ。DXF・DWG は marker と construction を持つ点を書かず、JWW の記号でない点は角度 0 だけを、SXF は SFC の点マーカーだけを書く。小版 35 から
_PointGeometryRequired = TypedDict(
    "_PointGeometryRequired",
    {
        # 形の種類
        "shape": Literal["point"],
        # 位置 [X, Y]
        "position": List[float],
        # 回転（度）
        "rotation": float,
    },
)
_PointGeometryOptional = TypedDict(
    "_PointGeometryOptional",
    {
        # 既定義の印。書かなければ文書の点の表し方に従う
        "marker": "PointMarker",
        # 作図の補助の点（JWW の仮点）か。書かなければ偽
        "construction": bool,
    },
    total=False,
)


class PointGeometry(_PointGeometryRequired, _PointGeometryOptional):
    "点（決定 0160・0183）。marker がなければ文書の点の表し方（DXF・DWG の $PDMODE・$PDSIZE）に従う点で、DXF・DWG の POINT と JWW の記号でない点が当たる。marker があれば既定義の印の点で、SFC の点マーカーと JWW の記号の点が当たる。置き換えるときは、描き方（marker があるか）を元のまま保つ。DXF・DWG は marker と construction を持つ点を書かず、JWW の記号でない点は角度 0 だけを、SXF は SFC の点マーカーだけを書く。小版 35 から"


# 既定義の印の表のキーと尺度。印の形は、表示する側がキーから引く
PointMarker = TypedDict(
    "PointMarker",
    {
        # 印の表の名前空間。sxf.marker（SFC の点マーカー）か jww.point（JWW の記号の点）
        "namespace": str,
        # 名前空間の中の元のコード
        "code": str,
        # 尺度
        "scale": float,
    },
)

# 寸法の種類。linear は 2 点を測る向きに、aligned は 2 点を結ぶ向きに測った長さ。angular は頂点から 2 点へ向かう線の、angular_lines は 2 本の線のなす角度。arc_length は円弧の長さ。radius は半径、diameter は直径。ordinate は原点からの X か Y の座標
DimensionKind = Union[
    Literal[
        "linear",
        "aligned",
        "angular",
        "angular_lines",
        "arc_length",
        "radius",
        "diameter",
        "ordinate",
    ],
    str,
]

# 座標の軸
DimensionAxis = Union[
    Literal[
        "x",
        "y",
    ],
    str,
]

# 寸法値の文字の書き方。measurement は測定値を書き、override は利用者が書いた文字（content）を書き、hidden は書かない
DimensionLabel = Union[
    Literal[
        "measurement",
        "override",
        "hidden",
    ],
    str,
]


# 記号（DXF・DWG のブロックの INSERT）。定義の点を、尺度・回転・移動の順に写す。切り抜きは形の値に含めない。置き換えるときは切り抜きを元のままにし、属性は値だけを変えられる。属性を持つ記号の置く点・尺度・回転は変えない（属性ごと動かすなら transform を使う）。足すときは切り抜きを持たない。小版 14 から
_SymbolGeometryRequired = TypedDict(
    "_SymbolGeometryRequired",
    {
        # 形の種類
        "shape": Literal["symbol"],
        # 記号の定義（ブロック）の元の名前。足すときは図面にある定義の名前を指す
        "name": str,
        # 定義の基点を置く点 [X, Y]
        "position": List[float],
        # X と Y の尺度 [X, Y]。0 でない。負なら反転する
        "scale": List[float],
        # 回転（度）
        "rotation": float,
    },
)
_SymbolGeometryOptional = TypedDict(
    "_SymbolGeometryOptional",
    {
        # 格子に並べて置く列と行。書かなければ 1 つだけ置く
        "grid": "SymbolGrid",
        # 属性のタグと値。entities は記号の属性を入力の順に返す。足すときは、定義の属性の定義（値が決まったものを除く）から属性を作り、ここに書いたタグの値で既定の値を替える。タグは大文字と小文字を区別せず、重複を受けない。必要な属性を作れなければ失敗する。置き換えるときは、元と同じ数・順・タグを受け、値を変えた属性の ATTRIB の値を書き直す（表題欄の項目の書き換えなど）。書式つきの値（複数行の属性）を持つ属性の値は変えられない。小版 16 から。置き換えで値を変えるのは小版 21 から
        "attributes": List["SymbolAttribute"],
    },
    total=False,
)


class SymbolGeometry(_SymbolGeometryRequired, _SymbolGeometryOptional):
    "記号（DXF・DWG のブロックの INSERT）。定義の点を、尺度・回転・移動の順に写す。切り抜きは形の値に含めない。置き換えるときは切り抜きを元のままにし、属性は値だけを変えられる。属性を持つ記号の置く点・尺度・回転は変えない（属性ごと動かすなら transform を使う）。足すときは切り抜きを持たない。小版 14 から"


# 記号の属性のタグと値
SymbolAttribute = TypedDict(
    "SymbolAttribute",
    {
        # 元のタグ
        "tag": str,
        # 値。1 行
        "value": str,
    },
)


# 記号を格子に並べる列と行。間隔は、記号を回転する前の向きに測る。置き換えるときは、列と行の数を変えない
SymbolGrid = TypedDict(
    "SymbolGrid",
    {
        # 列の数。1 以上
        "columns": int,
        # 行の数。1 以上。列と行がどちらも 1 の格子は持たない
        "rows": int,
        # 列の間隔
        "column_spacing": float,
        # 行の間隔
        "row_spacing": float,
    },
)

# 基点に合わせる枠の位置。entities は返し、足すときに書かなければ左上、置き換えるときに書かなければ元のまま
Attachment = Union[
    Literal[
        "top_left",
        "top_center",
        "top_right",
        "middle_left",
        "middle_center",
        "middle_right",
        "bottom_left",
        "bottom_center",
        "bottom_right",
    ],
    str,
]

# 要素の形の種類（決定 0022）。point（点、決定 0160）は小版 32 から。それまで点は symbol だった
EntityKind = Union[
    Literal[
        "line",
        "circle",
        "curve",
        "polyline",
        "text",
        "formatted_text",
        "dimension",
        "leader",
        "callout",
        "table",
        "underlay",
        "symbol",
        "hatch",
        "point",
    ],
    str,
]


# 開いた文書に、変更の一覧を当てて書き戻す（決定 0139・0156）。要素は、entities が返す ID とハッシュで指す。どれか 1 つでも当てられなければ、何も変えない。当てたら、文書を書き戻したものに替え、書き戻したファイルを blobs で返す。小版 7 から
_ApplyRequestRequired = TypedDict(
    "_ApplyRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["apply"],
        # 文書の名前。open で作ったもの
        "doc": str,
        # 変更。並べた順に当てる
        "edits": List["Edit"],
    },
)
_ApplyRequestOptional = TypedDict(
    "_ApplyRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 読んだときの文書のハッシュ（drawing の hash）。書けば、今の文書と同じときだけ当てる
        "hash": str,
    },
    total=False,
)


class ApplyRequest(_ApplyRequestRequired, _ApplyRequestOptional):
    "開いた文書に、変更の一覧を当てて書き戻す（決定 0139・0156）。要素は、entities が返す ID とハッシュで指す。どれか 1 つでも当てられなければ、何も変えない。当てたら、文書を書き戻したものに替え、書き戻したファイルを blobs で返す。小版 7 から"


# 当てた変更の結果。書き戻したファイルは blobs の file で返す
_ApplyResultRequired = TypedDict(
    "_ApplyResultRequired",
    {
        # 変更ごとの結果。要求の edits と同じ順
        "applied": List["Applied"],
        # 書き戻した文書のハッシュ。次の apply の前提条件に使う
        "hash": str,
    },
)
_ApplyResultOptional = TypedDict(
    "_ApplyResultOptional",
    {
        # true なら、変えていない要素の ID がずれた。鍵を持たない形式（JWW）で、前の図形を除くか足したとき。ID を使う前に entities で読み直す。ずれないときは持たない。小版 30 から
        "ids_shifted": bool,
    },
    total=False,
)


class ApplyResult(_ApplyResultRequired, _ApplyResultOptional):
    "当てた変更の結果。書き戻したファイルは blobs の file で返す"


# 開いた文書の図面モデルを、別の形式の新しいファイルに書き、新しい文書として開く（決定 0139・0206）。書けない要素は落とし、種類ごとの件数を診断で返す。書いたファイルは blobs で返す。今は DXF にだけ書ける。小版 41 から
_ConvertRequestRequired = TypedDict(
    "_ConvertRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["convert"],
        # 書く元の文書の名前。open で作ったもの
        "doc": str,
        # 書く形式の名前（capabilities の formats の id）。今は dxf だけ
        "format": str,
        # 作る文書の名前
        "to": str,
    },
)
_ConvertRequestOptional = TypedDict(
    "_ConvertRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
    },
    total=False,
)


class ConvertRequest(_ConvertRequestRequired, _ConvertRequestOptional):
    "開いた文書の図面モデルを、別の形式の新しいファイルに書き、新しい文書として開く（決定 0139・0206）。書けない要素は落とし、種類ごとの件数を診断で返す。書いたファイルは blobs で返す。今は DXF にだけ書ける。小版 41 から"


# 書いた文書。書いたファイルは blobs の file で返す
ConvertResult = TypedDict(
    "ConvertResult",
    {
        # 作った文書の名前
        "doc": str,
        # 書いた形式の名前
        "format": str,
        # 書いたファイルのハッシュ。apply の前提条件に使う
        "hash": str,
        # 書いた要素の数。記号から離した属性の文字を含む
        "elements": int,
    },
)


# 当てた変更 1 つの結果
Applied = TypedDict(
    "Applied",
    {
        # 変えた要素（レイヤの変更では、そのレイヤ）の、書き戻した後の ID。除いたときと、ID を決められないときは持たない
        "id": str,
        # 変えた要素（レイヤ）の、書き戻した後のハッシュ。id があるときに持つ
        "hash": str,
    },
    total=False,
)

# 要素 1 つへの変更。kind の値で形が決まる
Edit = Union[
    "RestyleEdit",
    "TransformEdit",
    "RemoveEdit",
    "ReplaceEdit",
    "AddEdit",
    "LayerEdit",
    "AddLayerEdit",
    "RemoveLayerEdit",
    "TextStyleEdit",
    "AddTextStyleEdit",
    "RemoveTextStyleEdit",
    "LineTypeEdit",
    "AddLineTypeEdit",
    "RemoveLineTypeEdit",
    "DimensionStyleEdit",
    "AddDimensionStyleEdit",
    "RemoveDimensionStyleEdit",
    "PointStyleEdit",
]


# 要素のレイヤ・色・線種・線種の倍率・線幅のうち、書いたものだけを変える。形は変えない
_RestyleEditRequired = TypedDict(
    "_RestyleEditRequired",
    {
        # 変更の種類
        "kind": Literal["restyle"],
        # 変える要素の ID。entities が返したもの
        "id": str,
        # 読んだときの要素のハッシュ。entities が返したもの
        "hash": str,
    },
)
_RestyleEditOptional = TypedDict(
    "_RestyleEditOptional",
    {
        # レイヤの元の名前。図面にあるレイヤで、名前が 1 つに決まるもの。layer_id と一緒には書かない
        "layer": str,
        # レイヤの ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。小版 8 から
        "layer_id": str,
        # 色
        "color": "ColorChange",
        # 線種
        "line_type": "LineTypeChange",
        # 要素の線種の倍率
        "line_type_scale": float,
        # 線幅
        "line_weight": "LineWeightChange",
    },
    total=False,
)


class RestyleEdit(_RestyleEditRequired, _RestyleEditOptional):
    "要素のレイヤ・色・線種・線種の倍率・線幅のうち、書いたものだけを変える。形は変えない"


# 要素を、アフィン変換で動かす。AutoCAD の系統では、図形とともに動く XDATA と記号の属性も同じ変換で直す。変換した形を元の種類で表せなければ当てない
TransformEdit = TypedDict(
    "TransformEdit",
    {
        # 変更の種類
        "kind": Literal["transform"],
        # 動かす要素の ID。entities が返したもの
        "id": str,
        # 読んだときの要素のハッシュ。entities が返したもの
        "hash": str,
        # [a, b, c, d, e, f]。要素の空間か用紙の点 (x, y) を (a x + b y + c, d x + e y + f) に写す
        "matrix": List[float],
    },
)


# 要素を除く
RemoveEdit = TypedDict(
    "RemoveEdit",
    {
        # 変更の種類
        "kind": Literal["remove"],
        # 除く要素の ID。entities が返したもの
        "id": str,
        # 読んだときの要素のハッシュ。entities が返したもの
        "hash": str,
    },
)


# 要素の形を、渡す値で置き換える。形の種類は変えない。レイヤとスタイルは変えない。小版 10 から
ReplaceEdit = TypedDict(
    "ReplaceEdit",
    {
        # 変更の種類
        "kind": Literal["replace"],
        # 置き換える要素の ID。entities が返したもの
        "id": str,
        # 読んだときの要素のハッシュ。entities が返したもの
        "hash": str,
        # 新しい形。entities の geometry と同じ種類
        "geometry": "Geometry",
    },
)


# 空間か用紙に要素を足す。書かない色・線種・線幅はレイヤに従う。足せる形は、形の値のどの種類も。小版 10 から
_AddEditRequired = TypedDict(
    "_AddEditRequired",
    {
        # 変更の種類
        "kind": Literal["add"],
        # 足す先の空間か用紙の ID（space/0・sheet/h22 など）。entities の ID の先頭の部分。小版 31 から、記号の定義の空間（space/h1F など）も指せる（決定 0177）
        "place": str,
        # 足す形
        "geometry": "Geometry",
    },
)
_AddEditOptional = TypedDict(
    "_AddEditOptional",
    {
        # レイヤの元の名前。図面にあるレイヤで、名前が 1 つに決まるもの。layer_id と一緒には書かない。どちらも書かなければレイヤ 0
        "layer": str,
        # レイヤの ID。tables が返したもの
        "layer_id": str,
        # 色。SXF はレイヤに従う色を書けないので、書く。小版 29 から
        "color": "ColorChange",
        # 線種。小版 29 から
        "line_type": "LineTypeChange",
        # 線幅。小版 29 から
        "line_weight": "LineWeightChange",
    },
    total=False,
)


class AddEdit(_AddEditRequired, _AddEditOptional):
    "空間か用紙に要素を足す。書かない色・線種・線幅はレイヤに従う。足せる形は、形の値のどの種類も。小版 10 から"


# レイヤの名前・状態・色・線種・線幅のうち、書いたものだけを変える（決定 0162）。DXF は、名前を変えると、図形と今のレイヤ（$CLAYER）も新しい名前で指す。DWG は、今あるレイヤの線種と真の色を変えられない。小版 22 から
_LayerEditRequired = TypedDict(
    "_LayerEditRequired",
    {
        # 変更の種類
        "kind": Literal["layer"],
        # 変えるレイヤの ID。tables が返したもの
        "id": str,
        # 読んだときのレイヤのハッシュ。tables が返したもの
        "hash": str,
    },
)
_LayerEditOptional = TypedDict(
    "_LayerEditOptional",
    {
        # 新しい名前。ほかのレイヤと重ならず（大文字と小文字を区別しない）、< > / \ " : ; ? * | = ` を含まないもの。0 と Defpoints の名前は変えられない
        "name": str,
        # 表示する。凍結するなら偽
        "visible": bool,
        # 印刷する
        "printable": bool,
        # 編集できる（ロックしない）
        "editable": bool,
        # 凍結する
        "frozen": bool,
        # 色。aci が要る。inherit は書けない
        "color": "ColorChange",
        # 線種。name か id で指す。inherit は書けない
        "line_type": "LineTypeChange",
        # 線幅。mm か default。inherit は書けない
        "line_weight": "LineWeightChange",
    },
    total=False,
)


class LayerEdit(_LayerEditRequired, _LayerEditOptional):
    "レイヤの名前・状態・色・線種・線幅のうち、書いたものだけを変える（決定 0162）。DXF は、名前を変えると、図形と今のレイヤ（$CLAYER）も新しい名前で指す。DWG は、今あるレイヤの線種と真の色を変えられない。小版 22 から"


# レイヤを除く（決定 0162）。要素・ほかのオブジェクト・今のレイヤ（$CLAYER）が指すレイヤと、0・Defpoints・外部参照に依るレイヤは除けない。同じ apply の要素の変更が指すレイヤも除けない。後ろにハンドルのないレイヤがあれば、その ID が変わるので除けない。除いたレイヤより後ろのレイヤと、その上の要素のハッシュは変わるので、読み直す。小版 24 から
RemoveLayerEdit = TypedDict(
    "RemoveLayerEdit",
    {
        # 変更の種類
        "kind": Literal["remove_layer"],
        # 除くレイヤの ID。tables が返したもの
        "id": str,
        # 読んだときのレイヤのハッシュ。tables が返したもの
        "hash": str,
    },
)


# 文字スタイルの名前・フォント・高さ・幅の倍率・傾き・裏返し・上下反転・縦書きのうち、書いたものだけを変える（決定 0163）。DXF は、名前を変えると、文字と今の文字スタイル（$TEXTSTYLE）も新しい名前で指す。表の書式などの図形でないオブジェクトが名前で指す文字スタイルの名前は変えられない。小版 25 から
_TextStyleEditRequired = TypedDict(
    "_TextStyleEditRequired",
    {
        # 変更の種類
        "kind": Literal["text_style"],
        # 変える文字スタイルの ID。tables が返したもの
        "id": str,
        # 読んだときの文字スタイルのハッシュ。tables が返したもの
        "hash": str,
    },
)
_TextStyleEditOptional = TypedDict(
    "_TextStyleEditOptional",
    {
        # 新しい名前。ほかの文字スタイルと重ならず（大文字と小文字を区別しない）、< > / \ " : ; ? * | = ` を含まないもの。Standard の名前は変えられない
        "name": str,
        # 主のフォント。形のファイル（shape）か、太字・斜体・文字集合・ピッチを持たない TrueType のファイル（named で、名前が .ttf・.ttc・.otf で終わるもの）。名前を空にすると除く。XDATA の書体の名前で指すフォントは変えられない
        "font": "Font",
        # 大きいフォント。形のファイル（shape）だけ。名前を空にすると除く
        "big_font": "Font",
        # 決まった高さ。文書の単位。0 なら決めない。負の数は書けない
        "height": float,
        # 幅の倍率。正の数
        "width": float,
        # 傾き（度）。-85 から 85 まで
        "oblique": float,
        # 左右を裏返す
        "backward": bool,
        # 上下を反転する
        "upside_down": bool,
        # 縦書き
        "vertical": bool,
    },
    total=False,
)


class TextStyleEdit(_TextStyleEditRequired, _TextStyleEditOptional):
    "文字スタイルの名前・フォント・高さ・幅の倍率・傾き・裏返し・上下反転・縦書きのうち、書いたものだけを変える（決定 0163）。DXF は、名前を変えると、文字と今の文字スタイル（$TEXTSTYLE）も新しい名前で指す。表の書式などの図形でないオブジェクトが名前で指す文字スタイルの名前は変えられない。小版 25 から"


# 文字スタイルを足す（決定 0163）。書かない欄は Standard と同じ。同じ apply の要素は、足した文字スタイルを指せない。小版 25 から
_AddTextStyleEditRequired = TypedDict(
    "_AddTextStyleEditRequired",
    {
        # 変更の種類
        "kind": Literal["add_text_style"],
        # 名前。text_style の name と同じ決まり
        "name": str,
    },
)
_AddTextStyleEditOptional = TypedDict(
    "_AddTextStyleEditOptional",
    {
        # 主のフォント。形のファイル（shape）か、太字・斜体・文字集合・ピッチを持たない TrueType のファイル（named で、名前が .ttf・.ttc・.otf で終わるもの）。名前を空にすると除く。XDATA の書体の名前で指すフォントは変えられない
        "font": "Font",
        # 大きいフォント。形のファイル（shape）だけ。名前を空にすると除く
        "big_font": "Font",
        # 決まった高さ。文書の単位。0 なら決めない。負の数は書けない
        "height": float,
        # 幅の倍率。正の数
        "width": float,
        # 傾き（度）。-85 から 85 まで
        "oblique": float,
        # 左右を裏返す
        "backward": bool,
        # 上下を反転する
        "upside_down": bool,
        # 縦書き
        "vertical": bool,
    },
    total=False,
)


class AddTextStyleEdit(_AddTextStyleEditRequired, _AddTextStyleEditOptional):
    "文字スタイルを足す（決定 0163）。書かない欄は Standard と同じ。同じ apply の要素は、足した文字スタイルを指せない。小版 25 から"


# 文字スタイルを除く（決定 0163）。要素・表の書式や寸法スタイルなどのほかのオブジェクト・今の文字スタイル（$TEXTSTYLE）が指すものと、Standard・外部参照に依るものは除けない。同じ apply で足す要素が指すものも除けない。後ろにハンドルのない文字スタイルがあれば、その ID が変わるので除けない。小版 25 から
RemoveTextStyleEdit = TypedDict(
    "RemoveTextStyleEdit",
    {
        # 変更の種類
        "kind": Literal["remove_text_style"],
        # 除く文字スタイルの ID。tables が返したもの
        "id": str,
        # 読んだときの文字スタイルのハッシュ。tables が返したもの
        "hash": str,
    },
)


# 線種の名前・説明・線と間の長さの列のうち、書いたものだけを変える（決定 0164）。DXF は、名前を変えると、要素・レイヤと今の線種（$CELTYPE）も新しい名前で指す。多重線のスタイルなどの図形でないオブジェクトが名前で指す線種の名前は変えられない。形か文字を添える線種と DWG の線種は、列の要素の数を変えられない。小版 26 から
_LineTypeEditRequired = TypedDict(
    "_LineTypeEditRequired",
    {
        # 変更の種類
        "kind": Literal["line_type"],
        # 変える線種の ID。tables が返したもの
        "id": str,
        # 読んだときの線種のハッシュ。tables が返したもの
        "hash": str,
    },
)
_LineTypeEditOptional = TypedDict(
    "_LineTypeEditOptional",
    {
        # 新しい名前。ほかの線種と重ならず（大文字と小文字を区別しない）、< > / \ " : ; ? * | = ` を含まないもの。ByLayer・ByBlock・Continuous と外部参照に依る線種の名前は変えられない
        "name": str,
        # 説明
        "description": str,
        # 線と間の長さの列。文書の単位。正は線、負は間、0 は点
        "pattern": List[float],
    },
    total=False,
)


class LineTypeEdit(_LineTypeEditRequired, _LineTypeEditOptional):
    "線種の名前・説明・線と間の長さの列のうち、書いたものだけを変える（決定 0164）。DXF は、名前を変えると、要素・レイヤと今の線種（$CELTYPE）も新しい名前で指す。多重線のスタイルなどの図形でないオブジェクトが名前で指す線種の名前は変えられない。形か文字を添える線種と DWG の線種は、列の要素の数を変えられない。小版 26 から"


# 線種を足す（決定 0164）。書かない欄は Continuous と同じ。同じ apply の要素とレイヤは、足した線種を指せない。小版 26 から
_AddLineTypeEditRequired = TypedDict(
    "_AddLineTypeEditRequired",
    {
        # 変更の種類
        "kind": Literal["add_line_type"],
        # 名前。line_type の name と同じ決まり
        "name": str,
    },
)
_AddLineTypeEditOptional = TypedDict(
    "_AddLineTypeEditOptional",
    {
        # 説明
        "description": str,
        # 線と間の長さの列。文書の単位。正は線、負は間、0 は点
        "pattern": List[float],
    },
    total=False,
)


class AddLineTypeEdit(_AddLineTypeEditRequired, _AddLineTypeEditOptional):
    "線種を足す（決定 0164）。書かない欄は Continuous と同じ。同じ apply の要素とレイヤは、足した線種を指せない。小版 26 から"


# 線種を除く（決定 0164）。要素・レイヤ・多重線のスタイルなどのほかのオブジェクト・今の線種（$CELTYPE）が指すものと、ByLayer・ByBlock・Continuous・外部参照に依るものは除けない。同じ apply で足す・変える要素とレイヤが指すものも除けない。後ろにハンドルのない線種があれば、その ID が変わるので除けない。小版 26 から
RemoveLineTypeEdit = TypedDict(
    "RemoveLineTypeEdit",
    {
        # 変更の種類
        "kind": Literal["remove_line_type"],
        # 除く線種の ID。tables が返したもの
        "id": str,
        # 読んだときの線種のハッシュ。tables が返したもの
        "hash": str,
    },
)


# 寸法スタイルの名前・端部記号・その大きさ・斜線・倍率のうち、書いたものだけを変える（決定 0165）。DXF は、名前を変えると、寸法・引出線・公差と今の寸法スタイル（$DIMSTYLE）も新しい名前で指す。図形でないオブジェクトが名前で指す寸法スタイルの名前は変えられない。小版 27 から
_DimensionStyleEditRequired = TypedDict(
    "_DimensionStyleEditRequired",
    {
        # 変更の種類
        "kind": Literal["dimension_style"],
        # 変える寸法スタイルの ID。tables が返したもの
        "id": str,
        # 読んだときの寸法スタイルのハッシュ。tables が返したもの
        "hash": str,
    },
)
_DimensionStyleEditOptional = TypedDict(
    "_DimensionStyleEditOptional",
    {
        # 新しい名前。ほかの寸法スタイルと重ならず（大文字と小文字を区別しない）、$ と < > / \ " : ; ? * | = ` を含まないもの。Standard・子の寸法スタイル（名前に $ を持つもの）・子を持つ寸法スタイル・外部参照に依る寸法スタイルの名前は変えられない
        "name": str,
        # 寸法線の 1 本目と 2 本目の補助線の側の端部記号。2 つのブロックの名前で、空の名前は既定の矢印（塗りつぶした閉じた矢印）。DXF は R2000 から変えられる。DWG の端部記号は変えられない
        "ends": List[str],
        # 引出線の端部記号。ブロックの名前で、空の名前は既定の矢印。ends と同じ決まり
        "leader": str,
        # 端部記号の大きさ（DIMASZ）。文書の単位で、scale を掛ける前の値。負の数は書けない
        "size": float,
        # 端部記号の代わりに描く斜線の大きさ（DIMTSZ）。0 なら斜線を描かない。負の数は書けない
        "tick": float,
        # 大きさに掛ける倍率（DIMSCALE）。0 はビューの縮尺から決める。負の数は書けない
        "scale": float,
    },
    total=False,
)


class DimensionStyleEdit(_DimensionStyleEditRequired, _DimensionStyleEditOptional):
    "寸法スタイルの名前・端部記号・その大きさ・斜線・倍率のうち、書いたものだけを変える（決定 0165）。DXF は、名前を変えると、寸法・引出線・公差と今の寸法スタイル（$DIMSTYLE）も新しい名前で指す。図形でないオブジェクトが名前で指す寸法スタイルの名前は変えられない。小版 27 から"


# 寸法スタイルを足す（決定 0165）。書かない欄は Standard と同じ。同じ apply の要素は、足した寸法スタイルを指せない。小版 27 から
_AddDimensionStyleEditRequired = TypedDict(
    "_AddDimensionStyleEditRequired",
    {
        # 変更の種類
        "kind": Literal["add_dimension_style"],
        # 名前。dimension_style の name と同じ決まり
        "name": str,
    },
)
_AddDimensionStyleEditOptional = TypedDict(
    "_AddDimensionStyleEditOptional",
    {
        # 寸法線の 1 本目と 2 本目の補助線の側の端部記号。2 つのブロックの名前で、空の名前は既定の矢印（塗りつぶした閉じた矢印）。DXF は R2000 から変えられる。DWG の端部記号は変えられない
        "ends": List[str],
        # 引出線の端部記号。ブロックの名前で、空の名前は既定の矢印。ends と同じ決まり
        "leader": str,
        # 端部記号の大きさ（DIMASZ）。文書の単位で、scale を掛ける前の値。負の数は書けない
        "size": float,
        # 端部記号の代わりに描く斜線の大きさ（DIMTSZ）。0 なら斜線を描かない。負の数は書けない
        "tick": float,
        # 大きさに掛ける倍率（DIMSCALE）。0 はビューの縮尺から決める。負の数は書けない
        "scale": float,
    },
    total=False,
)


class AddDimensionStyleEdit(_AddDimensionStyleEditRequired, _AddDimensionStyleEditOptional):
    "寸法スタイルを足す（決定 0165）。書かない欄は Standard と同じ。同じ apply の要素は、足した寸法スタイルを指せない。小版 27 から"


# 寸法スタイルを除く（決定 0165）。寸法・引出線・公差などのほかのオブジェクト・今の寸法スタイル（$DIMSTYLE）が指すものと、Standard・子の寸法スタイル・子を持つ寸法スタイル・外部参照に依るものは除けない。後ろにハンドルのない寸法スタイルがあれば、その ID が変わるので除けない。小版 27 から
RemoveDimensionStyleEdit = TypedDict(
    "RemoveDimensionStyleEdit",
    {
        # 変更の種類
        "kind": Literal["remove_dimension_style"],
        # 除く寸法スタイルの ID。tables が返したもの
        "id": str,
        # 読んだときの寸法スタイルのハッシュ。tables が返したもの
        "hash": str,
    },
)


# 文書の点の表し方（marker を持たない点の見え方）を変える（決定 0190）。文書全体の設定なので要素を指さない。読んだときの文書は、apply の hash で確かめる。DXF・DWG は見出しの $PDMODE・$PDSIZE に書く。DWG は、変数を書く幅が変わる値（$PDMODE の 0 とほかの値の間の変更、$PDSIZE の 0・1 とほかの値の間の変更）を、小版 39 から書ける（決定 0193）。JWW・SXF は文書の点の表し方を持たないので断る。applied は id と hash を持たない。小版 38 から
_PointStyleEditRequired = TypedDict(
    "_PointStyleEditRequired",
    {
        # 変更の種類
        "kind": Literal["point_style"],
        # 基本の形
        "mark": "PointMark",
        # 大きさ。0 より大きい有限の数。意味は size_kind で決まる
        "size": float,
        # 大きさの意味
        "size_kind": "PointSizeKind",
    },
)
_PointStyleEditOptional = TypedDict(
    "_PointStyleEditOptional",
    {
        # 円で囲む。書かなければ偽
        "circle": bool,
        # 四角で囲む。書かなければ偽
        "square": bool,
    },
    total=False,
)


class PointStyleEdit(_PointStyleEditRequired, _PointStyleEditOptional):
    "文書の点の表し方（marker を持たない点の見え方）を変える（決定 0190）。文書全体の設定なので要素を指さない。読んだときの文書は、apply の hash で確かめる。DXF・DWG は見出しの $PDMODE・$PDSIZE に書く。DWG は、変数を書く幅が変わる値（$PDMODE の 0 とほかの値の間の変更、$PDSIZE の 0・1 とほかの値の間の変更）を、小版 39 から書ける（決定 0193）。JWW・SXF は文書の点の表し方を持たないので断る。applied は id と hash を持たない。小版 38 から"

# 文書の点の基本の形。dot は小さな点、none はなし、plus は十字、cross はバツ、tick は上向きの短い線
PointMark = Union[
    Literal[
        "dot",
        "none",
        "plus",
        "cross",
        "tick",
    ],
    str,
]

# 文書の点の大きさの意味。length は点のある空間の長さ、screen は画面の高さに対する百分率
PointSizeKind = Union[
    Literal[
        "length",
        "screen",
    ],
    str,
]


# レイヤを足す（決定 0162）。書かない欄は、状態なら表示・印刷・編集でき凍結せず、色・線種・線幅ならレイヤ 0 と同じ。同じ apply の要素は、足したレイヤを指せない。小版 22 から
_AddLayerEditRequired = TypedDict(
    "_AddLayerEditRequired",
    {
        # 変更の種類
        "kind": Literal["add_layer"],
        # 名前。layer の name と同じ決まり
        "name": str,
    },
)
_AddLayerEditOptional = TypedDict(
    "_AddLayerEditOptional",
    {
        # 表示する。凍結するなら偽
        "visible": bool,
        # 印刷する
        "printable": bool,
        # 編集できる（ロックしない）
        "editable": bool,
        # 凍結する
        "frozen": bool,
        # 色。aci が要る。inherit は書けない
        "color": "ColorChange",
        # 線種。name か id で指す。inherit は書けない
        "line_type": "LineTypeChange",
        # 線幅。mm か default。inherit は書けない
        "line_weight": "LineWeightChange",
    },
    total=False,
)


class AddLayerEdit(_AddLayerEditRequired, _AddLayerEditOptional):
    "レイヤを足す（決定 0162）。書かない欄は、状態なら表示・印刷・編集でき凍結せず、色・線種・線幅ならレイヤ 0 と同じ。同じ apply の要素は、足したレイヤを指せない。小版 22 から"

# 値を継承する先。layer はレイヤから、block は要素を置いた記号の配置から
Inherit = Union[
    Literal[
        "layer",
        "block",
    ],
    str,
]


# 新しい色。inherit か、aci・sxf・jww・rgb の少なくとも 1 つのどちらかを書く。aci・sxf・jww は 1 つだけ書く
ColorChange = TypedDict(
    "ColorChange",
    {
        # 継承する先
        "inherit": "Inherit",
        # AutoCAD の色の番号（1〜255）
        "aci": int,
        # SXF の色のコード。1〜16 は既定義色、17 からは利用者定義色を定義の順に数えたもの。小版 29 から
        "sxf": int,
        # Jw_cad のペンの番号。0〜9 と、版 420 からの SXF の拡張色 100〜356。小版 30 から
        "jww": int,
        # [赤, 緑, 青]。それぞれ 0〜255
        "rgb": List[int],
    },
    total=False,
)


# 新しい線種。inherit・name・id のどれか 1 つを書く
LineTypeChange = TypedDict(
    "LineTypeChange",
    {
        # 継承する先
        "inherit": "Inherit",
        # 線種の元の名前。図面にある線種で、名前が 1 つに決まるもの
        "name": str,
        # 線種の ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。小版 8 から
        "id": str,
    },
    total=False,
)


# 新しい線幅。inherit・mm・default のどれか 1 つを書く
LineWeightChange = TypedDict(
    "LineWeightChange",
    {
        # 継承する先
        "inherit": "Inherit",
        # 用紙の上の幅（mm）。0 以上
        "mm": float,
        # true なら、文書の既定の線幅
        "default": bool,
    },
    total=False,
)


# 開いた文書を図面モデル（L2）に変換し、レイヤと線種の表の項目を返す（決定 0156）。小版 8 から
_TablesRequestRequired = TypedDict(
    "_TablesRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["tables"],
        # 文書の名前。open で作ったもの
        "doc": str,
    },
)
_TablesRequestOptional = TypedDict(
    "_TablesRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
    },
    total=False,
)


class TablesRequest(_TablesRequestRequired, _TablesRequestOptional):
    "開いた文書を図面モデル（L2）に変換し、レイヤと線種の表の項目を返す（決定 0156）。小版 8 から"


# 表の項目。図面モデルの順
TablesResult = TypedDict(
    "TablesResult",
    {
        # レイヤ
        "layers": List["LayerItem"],
        # 線種。小版 26 から、定義を持つ
        "line_types": List["LineTypeItem"],
        # 寸法スタイル。小版 27 から、端部記号と大きさを持つ
        "dimension_styles": List["DimensionStyleItem"],
        # 文字スタイル（決定 0158）。要素が名前で指すだけで、ファイルに定義がないものは、定義の欄を持たない項目として後ろに並ぶ。小版 20 から
        "text_styles": List["TextStyleItem"],
    },
)


# 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、2D のシーンを blob で返す（決定 0172）。シーンの配置は schema/scene.toml にある。小版 33 から
_SceneRequestRequired = TypedDict(
    "_SceneRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["scene"],
        # 文書の名前。open で作ったもの
        "doc": str,
        # 起点。実寸の空間・記号の定義の空間・用紙の ID（drawing と entities が返すもの）
        "from": str,
    },
)
_SceneRequestOptional = TypedDict(
    "_SceneRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した空間の要素だけを描く。from が用紙のときだけ書ける
        "view": int,
        # [x の最小, y の最小, x の最大, y の最大]。起点の座標の長方形。範囲の重なる塊だけを返す
        "region": List[float],
        # 弦の誤差（起点の長さ）。0 より大きい。書かなければ、用紙からは 0.02 mm、空間からは要素の範囲の対角線の 1/50000
        "tolerance": float,
        # true なら印刷する要素を、書かないか false なら表示する要素を描く
        "print": bool,
        # 色の表の入力の名前（zm_push で渡したもの）。プロファイルの色の語彙と同じ形の JSON で、values に rgb（#rrggbb）と aci・sxf・jww の符号を並べる（決定 0138・0178）。書かなければ、ファイルが RGB を持たない色は未解決の印を立てる
        "palette": str,
        # 文字を置くフォント（決定 0188）。書けば、文字を字形で置き、シーンの字形の区画に形と変換を返す。文字スタイルと書式の区間のフォントの名前（シーンの font_names）で引き、同じ名前、なければ ASCII の大文字と小文字を区別しない名前で合わせる。合うものがなければ名前のないフォントで置く。小版 37 から
        "fonts": List["SceneFont"],
        # 形式の宣言値の高さ（キーは dxf.TEXT.40・dxf.MTEXT.40・jww.moji.size_y など）が、フォントのどの寸法に当たるか（決定 0176・0188）。書かないキーは大文字の高さとみなす。fonts を書いたときだけ使う。小版 37 から
        "heights": Dict[str, "HeightReference"],
    },
    total=False,
)


class SceneRequest(_SceneRequestRequired, _SceneRequestOptional):
    "開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、2D のシーンを blob で返す（決定 0172）。シーンの配置は schema/scene.toml にある。小版 33 から"


# シーンの文字を置くフォント（決定 0188）
_SceneFontRequired = TypedDict(
    "_SceneFontRequired",
    {
        # TrueType・OpenType（CFF・CFF2 を含む）のファイルの入力の名前（zm_push で渡したもの）
        "input": str,
    },
)
_SceneFontOptional = TypedDict(
    "_SceneFontOptional",
    {
        # 文字が指すフォントの名前（ファイルに書かれた元の名前）。書かなければ、名前の合わない文字に使う既定のフォント。同じ名前と、名前のないものは 1 つまで
        "name": str,
        # フォントのコレクション（TTC）の中の番号。書かなければ 0
        "index": int,
    },
    total=False,
)


class SceneFont(_SceneFontRequired, _SceneFontOptional):
    "シーンの文字を置くフォント（決定 0188）"

# 宣言された高さが当たるフォントの寸法。cap_height は大文字の高さ、em は em、frame はアセンダーとディセンダーを合わせた高さ
HeightReference = Union[
    Literal[
        "cap_height",
        "em",
        "frame",
    ],
    str,
]


# シーンの要約と、シーンの blob の番号
_SceneResultRequired = TypedDict(
    "_SceneResultRequired",
    {
        # 起点の ID
        "from": str,
        # 起点の座標の単位。用紙とビューからは millimeter
        "unit": "Unit",
        # 使った弦の誤差（起点の長さ）
        "tolerance": float,
        # 返した描くものの数
        "counts": "SceneCounts",
        # シーンの blob の番号（種類は scene）。zm_take で取り出す
        "blob": int,
    },
)
_SceneResultOptional = TypedDict(
    "_SceneResultOptional",
    {
        # 起点のビューの番号。ビューでなければ持たない
        "view": int,
        # 返した塊の範囲。起点の座標。塊がなければ持たない
        "extent": "Extent",
    },
    total=False,
)


class SceneResult(_SceneResultRequired, _SceneResultOptional):
    "シーンの要約と、シーンの blob の番号"


# シーンの描くものの種類ごとの数
SceneCounts = TypedDict(
    "SceneCounts",
    {
        # 塊
        "chunks": int,
        # 線（折れ線）
        "lines": int,
        # 線と面の頂点
        "vertices": int,
        # 面
        "faces": int,
        # 文字
        "texts": int,
        # 点
        "points": int,
        # 画像
        "images": int,
        # 置いた字形。fonts を書かなければ 0。小版 37 から
        "glyphs": int,
    },
)


# 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、プロファイルの規則と語彙で照合する（決定 0138・0173・0179）。小版 34 から
_CheckRequestRequired = TypedDict(
    "_CheckRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["check"],
        # 文書の名前。open で作ったもの
        "doc": str,
        # プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す
        "profiles": List[str],
        # 照合に使うプロファイル。profiles の中から選ぶ
        "profile": "ProfileRef",
    },
)
_CheckRequestOptional = TypedDict(
    "_CheckRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # たどる起点。実寸の空間・記号の定義の空間・用紙の ID。書かなければ、すべての用紙から、用紙がなければ実寸の空間からたどる
        "from": str,
        # 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した要素と、そのビューの縮尺だけを照合する。from が用紙のときだけ書ける
        "view": int,
        # 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。小版 40 から
        "file_name": str,
        # 返す最初の指摘の位置。前の応答の next_offset を渡す。書かなければ 0
        "offset": int,
        # 返す指摘の数の上限。書かなければ 1000
        "limit": int,
        # 色の表の入力の名前（zm_push で渡したもの）。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く
        "palette": str,
    },
    total=False,
)


class CheckRequest(_CheckRequestRequired, _CheckRequestOptional):
    "開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、プロファイルの規則と語彙で照合する（決定 0138・0173・0179）。小版 34 から"


# プロファイルの識別（決定 0138）
ProfileRef = TypedDict(
    "ProfileRef",
    {
        # 基準の系統（mlit-cad など）
        "id": str,
        # 基準の版（2025-12 など）
        "edition": str,
    },
)


# 照合の結果。指摘は offset から limit 個まで
_CheckResultRequired = TypedDict(
    "_CheckResultRequired",
    {
        # 照合に使ったプロファイルの鎖。extends の基から順
        "chain": List["ProfileIdentity"],
        # 照合した規則ごとの数。プロファイルの順。使えない規則は持たず、profile.rule.unsupported の診断で返す
        "rules": List["CheckRule"],
        # 指摘。規則の順に、規則ごとにたどった順
        "findings": List["CheckFinding"],
        # 全体の指摘の数
        "total": int,
    },
)
_CheckResultOptional = TypedDict(
    "_CheckResultOptional",
    {
        # 続きの最初の指摘の位置。続きがなければ持たない
        "next_offset": int,
    },
    total=False,
)


class CheckResult(_CheckResultRequired, _CheckResultOptional):
    "照合の結果。指摘は offset から limit 個まで"


# 照合に使ったプロファイルの識別と、データの直し
ProfileIdentity = TypedDict(
    "ProfileIdentity",
    {
        # 基準の系統
        "id": str,
        # 基準の版
        "edition": str,
        # このデータの直し。1 から
        "revision": int,
    },
)


# 規則ごとの数
CheckRule = TypedDict(
    "CheckRule",
    {
        # 規則の id
        "id": str,
        # 照合の種類（layer-name など）
        "check": str,
        # 指摘の数。照合できなかったものを除く
        "findings": int,
        # 照合できなかった数
        "unsupported": int,
    },
)


# 指摘。diagnostic は診断と同じ形で、describe で文にできる。severity は規則の level で、照合できない値（profile.<種類>.unsupported）は info。count は 1
CheckFinding = TypedDict(
    "CheckFinding",
    {
        # 規則の id
        "rule": str,
        # code・重要度・場所（要素・実効のレイヤ・用紙の ID）・見つけた値
        "diagnostic": "Diagnostic",
    },
)


# zm_push で渡した IFC（IFC-SPF か ifcZIP）を、zm_push で渡した EXPRESS のスキーマに照らしてチェックする（決定 0207・0216）。インスタンスの実体の名前・抽象・属性の数・値の型・省略を見る。WHERE・UNIQUE・RULE の規則は評価せず、数を返す。zumen は EXPRESS を同梱しない。小版 42 から
_ValidateRequestRequired = TypedDict(
    "_ValidateRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["validate"],
        # IFC の入力の名前（zm_push で渡した .ifc か .ifczip）
        "input": str,
        # EXPRESS の入力の名前（zm_push で渡した .exp）
        "express": str,
    },
)
_ValidateRequestOptional = TypedDict(
    "_ValidateRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 返す最初の指摘の位置。前の応答の next_offset を渡す。書かなければ 0
        "offset": int,
        # 返す指摘の数の上限。書かなければ 1000
        "limit": int,
    },
    total=False,
)


class ValidateRequest(_ValidateRequestRequired, _ValidateRequestOptional):
    "zm_push で渡した IFC（IFC-SPF か ifcZIP）を、zm_push で渡した EXPRESS のスキーマに照らしてチェックする（決定 0207・0216）。インスタンスの実体の名前・抽象・属性の数・値の型・省略を見る。WHERE・UNIQUE・RULE の規則は評価せず、数を返す。zumen は EXPRESS を同梱しない。小版 42 から"


# スキーマに照らしたチェックの結果。指摘は offset から limit 個まで。スキーマの名前の違いと評価しない規則は、診断で返す
_ValidateResultRequired = TypedDict(
    "_ValidateResultRequired",
    {
        # EXPRESS のスキーマの名前（大文字）
        "schema": str,
        # IFC の FILE_SCHEMA の名前
        "file_schema": str,
        # EXPRESS のバイト列のハッシュ（v2: と 32 桁の 16 進）。どの EXPRESS でチェックしたかを残す
        "express_hash": str,
        # 評価しなかった規則（WHERE・UNIQUE・RULE）の数。確認できなかった項目
        "rules_not_evaluated": int,
        # 指摘。インスタンスの番号の順に、属性の順。診断と同じ形で、describe で文にできる。count は 1
        "findings": List["Diagnostic"],
        # 全体の指摘の数
        "total": int,
    },
)
_ValidateResultOptional = TypedDict(
    "_ValidateResultOptional",
    {
        # 続きの最初の指摘の位置。続きがなければ持たない
        "next_offset": int,
    },
    total=False,
)


class ValidateResult(_ValidateResultRequired, _ValidateResultOptional):
    "スキーマに照らしたチェックの結果。指摘は offset から limit 個まで。スキーマの名前の違いと評価しない規則は、診断で返す"


# レビューの文書（決定 0214・0218）の 1 つの対象を、開いた図面の文書の版へ移す。要素の参照を図面の要素の ID で引き、参照ごとに unchanged・changed・removed を返す。場所と図形に元の版のハッシュと状態を書き、対象のハッシュを文書のハッシュにする。2D の点は動かさない。移した文書は blobs の file で返す。小版 43 から
_ReviewCarryRequestRequired = TypedDict(
    "_ReviewCarryRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["review_carry"],
        # 新しい版の文書の名前。open で作ったもの。小版 44 から、IFC の建物モデルの文書も渡せ、要素の参照を建物モデルの要素の ID とハッシュで引く
        "doc": str,
        # レビューの文書の入力の名前（zm_push で渡した JSON）
        "review": str,
        # 移す対象の ID。レビューの文書の targets の id
        "target": str,
    },
)
_ReviewCarryRequestOptional = TypedDict(
    "_ReviewCarryRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 返す最初の参照の位置。前の応答の next_offset を渡す。書かなければ 0
        "offset": int,
        # 返す参照の数の上限。書かなければ 1000
        "limit": int,
    },
    total=False,
)


class ReviewCarryRequest(_ReviewCarryRequestRequired, _ReviewCarryRequestOptional):
    "レビューの文書（決定 0214・0218）の 1 つの対象を、開いた図面の文書の版へ移す。要素の参照を図面の要素の ID で引き、参照ごとに unchanged・changed・removed を返す。場所と図形に元の版のハッシュと状態を書き、対象のハッシュを文書のハッシュにする。2D の点は動かさない。移した文書は blobs の file で返す。小版 43 から"


# 移した結果。参照は offset から limit 個まで。移した文書は blobs の file で返す
_ReviewCarryResultRequired = TypedDict(
    "_ReviewCarryResultRequired",
    {
        # 移す前の対象のハッシュ
        "from": str,
        # 移した後の対象のハッシュ。文書のハッシュ
        "hash": str,
        # 要素の参照ごとの状態。指摘の順に、場所の順、参照の順
        "references": List["CarriedReference"],
        # 全体の参照の数
        "total": int,
    },
)
_ReviewCarryResultOptional = TypedDict(
    "_ReviewCarryResultOptional",
    {
        # 続きの最初の参照の位置。続きがなければ持たない
        "next_offset": int,
    },
    total=False,
)


class ReviewCarryResult(_ReviewCarryResultRequired, _ReviewCarryResultOptional):
    "移した結果。参照は offset から limit 個まで。移した文書は blobs の file で返す"


# 移した要素の参照
CarriedReference = TypedDict(
    "CarriedReference",
    {
        # 指摘の ID
        "topic": str,
        # 場所の ID
        "location": str,
        # 要素の ID
        "element": str,
        # 状態
        "state": "CarriedState",
    },
)

# 移したときの状態（決定 0214）。unchanged は要素のハッシュが同じ、changed は違うか元のハッシュがない、removed は新しい版に要素がない
CarriedState = Union[
    Literal[
        "unchanged",
        "changed",
        "removed",
    ],
    str,
]


# 開いた文書を check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0214・0218）に足す。指摘の ID は対象のハッシュ・規則・診断から決め、取り込み済みの指摘は足さない。足した文書は blobs の file で返す。小版 43 から
_ReviewAdoptRequestRequired = TypedDict(
    "_ReviewAdoptRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["review_adopt"],
        # 照合する文書の名前。open で作ったもの
        "doc": str,
        # プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す
        "profiles": List[str],
        # 照合に使うプロファイル。profiles の中から選ぶ
        "profile": "ProfileRef",
        # 指摘が指す対象の ID。文書になければ、文書の形式とハッシュで足す
        "target": str,
        # 取り込む指摘の番号の列。check の指摘の並びの順で、0 から数える
        "findings": List[int],
    },
)
_ReviewAdoptRequestOptional = TypedDict(
    "_ReviewAdoptRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # たどる起点。実寸の空間・記号の定義の空間・用紙の ID。書かなければ、すべての用紙から、用紙がなければ実寸の空間からたどる
        "from": str,
        # 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した要素と、そのビューの縮尺だけを照合する。from が用紙のときだけ書ける
        "view": int,
        # 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。小版 40 から
        "file_name": str,
        # 色の表の入力の名前（zm_push で渡したもの）。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く
        "palette": str,
        # 足す先のレビューの文書の入力の名前（zm_push で渡した JSON）。書かなければ空の文書に足す
        "review": str,
    },
    total=False,
)


class ReviewAdoptRequest(_ReviewAdoptRequestRequired, _ReviewAdoptRequestOptional):
    "開いた文書を check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0214・0218）に足す。指摘の ID は対象のハッシュ・規則・診断から決め、取り込み済みの指摘は足さない。足した文書は blobs の file で返す。小版 43 から"


# 取り込んだ結果。足した文書は blobs の file で返す
ReviewAdoptResult = TypedDict(
    "ReviewAdoptResult",
    {
        # 足した指摘の ID。取り込み済みで足さなかった指摘は持たない
        "added": List[str],
        # 足した後の文書の指摘の数
        "topics": int,
    },
)


# 開いた IFC の建物モデル（決定 0207・0219）の要約を返す。小版 44 から
_BuildingRequestRequired = TypedDict(
    "_BuildingRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["building"],
        # 文書の名前。open で IFC を開いたもの
        "doc": str,
    },
)
_BuildingRequestOptional = TypedDict(
    "_BuildingRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
    },
    total=False,
)


class BuildingRequest(_BuildingRequestRequired, _BuildingRequestOptional):
    "開いた IFC の建物モデル（決定 0207・0219）の要約を返す。小版 44 から"


# 建物モデルの要約
_BuildingResultRequired = TypedDict(
    "_BuildingResultRequired",
    {
        # IFC のスキーマの名前（FILE_SCHEMA の最初）
        "schema": str,
        # 入力のバイト列のハッシュ。review_carry の対象の版に使う
        "hash": str,
        # プロジェクトの単位
        "units": List["BuildingUnit"],
        # 要素の数
        "elements": int,
        # クラスごとの要素の数。クラスの名前の順
        "classes": List["ClassCount"],
    },
)
_BuildingResultOptional = TypedDict(
    "_BuildingResultOptional",
    {
        # プロジェクトの要素の ID。なければ持たない
        "project": str,
        # プロジェクトの名前。なければ持たない
        "project_name": str,
    },
    total=False,
)


class BuildingResult(_BuildingResultRequired, _BuildingResultOptional):
    "建物モデルの要約"


# 単位
_BuildingUnitRequired = TypedDict(
    "_BuildingUnitRequired",
    {
        # 単位の種類（LENGTHUNIT・AREAUNIT など）
        "kind": str,
        # 単位の名前（METRE、換算する単位なら inch など）
        "name": str,
    },
)
_BuildingUnitOptional = TypedDict(
    "_BuildingUnitOptional",
    {
        # SI の接頭辞（MILLI など）
        "prefix": str,
        # 換算する単位の、SI の単位での大きさ。SI の単位は持たない
        "scale": float,
    },
    total=False,
)


class BuildingUnit(_BuildingUnitRequired, _BuildingUnitOptional):
    "単位"


# クラスの要素の数
ClassCount = TypedDict(
    "ClassCount",
    {
        # IFC のクラスの名前（IfcWall など）
        "name": str,
        # 要素の数
        "count": int,
    },
)


# 開いた IFC の建物モデルの要素を、書いた順に返す。成分は components で選ぶ（決定 0207・0219）。小版 44 から
_BuildingElementsRequestRequired = TypedDict(
    "_BuildingElementsRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["building_elements"],
        # 文書の名前。open で IFC を開いたもの
        "doc": str,
    },
)
_BuildingElementsRequestOptional = TypedDict(
    "_BuildingElementsRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 返す要素の ID。書けば、その要素だけを、書いた順に返す。ない ID は返さない
        "ids": List[str],
        # IFC のクラスの名前。書けば、そのクラスの要素だけを返す。大文字と小文字を区別しない
        "class": str,
        # 返す成分の名前。1 つ以上で、重ねない。書かなければ class と identity を返す
        "components": List["BuildingComponent"],
        # 返す最初の位置。前の応答の next_offset を渡す。書かなければ 0
        "offset": int,
        # 返す数の上限。書かなければ 1000
        "limit": int,
    },
    total=False,
)


class BuildingElementsRequest(_BuildingElementsRequestRequired, _BuildingElementsRequestOptional):
    "開いた IFC の建物モデルの要素を、書いた順に返す。成分は components で選ぶ（決定 0207・0219）。小版 44 から"

# building_elements で返す成分（決定 0207）。relations は索引から求める子・参照・型の実例・開口・埋める要素・構成員
BuildingComponent = Union[
    Literal[
        "identity",
        "class",
        "classification",
        "spatial",
        "parent",
        "placement",
        "type_of",
        "properties",
        "quantities",
        "material",
        "opening",
        "group",
        "connection",
        "level",
        "zone",
        "style",
        "relations",
    ],
    str,
]


# 要素の一覧。offset から limit 個まで
_BuildingElementsResultRequired = TypedDict(
    "_BuildingElementsResultRequired",
    {
        # 要素
        "elements": List["BuildingElement"],
        # 選んだ要素の数
        "total": int,
    },
)
_BuildingElementsResultOptional = TypedDict(
    "_BuildingElementsResultOptional",
    {
        # 続きの最初の位置。続きがなければ持たない
        "next_offset": int,
    },
    total=False,
)


class BuildingElementsResult(_BuildingElementsResultRequired, _BuildingElementsResultOptional):
    "要素の一覧。offset から limit 個まで"


# 建物モデルの要素。成分は、選んだもののうち要素が持つものだけを持つ
_BuildingElementRequired = TypedDict(
    "_BuildingElementRequired",
    {
        # 要素の ID（GlobalId か、i と番号）
        "id": str,
        # 要素の成分のハッシュ（v1: と 32 桁の 16 進、決定 0207）
        "hash": str,
    },
)
_BuildingElementOptional = TypedDict(
    "_BuildingElementOptional",
    {
        # 名前・説明・タグ・長い名前
        "identity": "IdentityComponent",
        # クラス
        "class": "ClassComponent",
        # 分類の参照
        "classification": List["ClassificationReference"],
        # 空間の要素の種類と高さ
        "spatial": "SpatialComponent",
        # 親と、参照される空間
        "parent": "ParentComponent",
        # 配置
        "placement": "PlacementComponent",
        # 型の要素の ID
        "type_of": str,
        # プロパティのセット
        "properties": List["PropertySet"],
        # 数量のセット
        "quantities": List["QuantitySet"],
        # 材料
        "material": "MaterialComponent",
        # 開口と、埋める要素
        "opening": "OpeningComponent",
        # 属するグループの要素の ID
        "group": List[str],
        # 接続
        "connection": List["ConnectionLink"],
        # レベル
        "level": "LevelComponent",
        # 区画
        "zone": "ZoneComponent",
        # 表示のスタイル
        "style": "StyleComponent",
        # 索引から求める関係
        "relations": "RelationsComponent",
    },
    total=False,
)


class BuildingElement(_BuildingElementRequired, _BuildingElementOptional):
    "建物モデルの要素。成分は、選んだもののうち要素が持つものだけを持つ"


# 名前・説明・タグ・長い名前
IdentityComponent = TypedDict(
    "IdentityComponent",
    {
        # 名前
        "name": str,
        # 説明
        "description": str,
        # タグ
        "tag": str,
        # 長い名前
        "long_name": str,
    },
    total=False,
)


# クラス
_ClassComponentRequired = TypedDict(
    "_ClassComponentRequired",
    {
        # IFC4 のクラスの名前。IFC2X3 の StandardCase などは、元の名前を original に残して直す
        "name": str,
        # ファイルに書いた実体の名前
        "original": str,
    },
)
_ClassComponentOptional = TypedDict(
    "_ClassComponentOptional",
    {
        # PredefinedType
        "predefined": str,
        # ObjectType など、USERDEFINED のときの名前
        "user_defined": str,
    },
    total=False,
)


class ClassComponent(_ClassComponentRequired, _ClassComponentOptional):
    "クラス"


# 分類の参照
ClassificationReference = TypedDict(
    "ClassificationReference",
    {
        # 分類の体系の名前
        "system": str,
        # 体系の版
        "edition": str,
        # 分類の記号
        "code": str,
        # 分類の名前
        "name": str,
    },
    total=False,
)


# 単位とともに持つ値
_MeasuredRequired = TypedDict(
    "_MeasuredRequired",
    {
        # 値
        "value": float,
    },
)
_MeasuredOptional = TypedDict(
    "_MeasuredOptional",
    {
        # 単位。解けなければ持たない
        "unit": "BuildingUnit",
    },
    total=False,
)


class Measured(_MeasuredRequired, _MeasuredOptional):
    "単位とともに持つ値"


# 空間の要素
_SpatialComponentRequired = TypedDict(
    "_SpatialComponentRequired",
    {
        # site・building・storey・space・spatial_zone か、元の実体の名前
        "kind": str,
    },
)
_SpatialComponentOptional = TypedDict(
    "_SpatialComponentOptional",
    {
        # 高さ
        "elevation": "Measured",
        # 基準の高さ
        "ref_elevation": "Measured",
    },
    total=False,
)


class SpatialComponent(_SpatialComponentRequired, _SpatialComponentOptional):
    "空間の要素"


# 親
_ParentComponentRequired = TypedDict(
    "_ParentComponentRequired",
    {
        # 参照される空間の要素の ID
        "referenced_in": List[str],
    },
)
_ParentComponentOptional = TypedDict(
    "_ParentComponentOptional",
    {
        # 親の要素
        "of": "Membership",
    },
    total=False,
)


class ParentComponent(_ParentComponentRequired, _ParentComponentOptional):
    "親"


# 親との関係
_MembershipRequired = TypedDict(
    "_MembershipRequired",
    {
        # 親の要素の ID
        "target": str,
        # 関係の種類
        "kind": "ParentKind",
    },
)
_MembershipOptional = TypedDict(
    "_MembershipOptional",
    {
        # 入れ子の順。0 から
        "order": int,
    },
    total=False,
)


class Membership(_MembershipRequired, _MembershipOptional):
    "親との関係"


# 配置
PlacementComponent = TypedDict(
    "PlacementComponent",
    {
        # 配置の親の要素の ID
        "relative_to": str,
        # 親をたどった世界の変換。3 行 4 列（回転と移動）。解けなければ持たない
        "world": List[List[float]],
        # 解けなかった配置の実体の名前
        "unsupported": str,
    },
    total=False,
)


# 値
_BuildingValueRequired = TypedDict(
    "_BuildingValueRequired",
    {
        # 値の種類
        "kind": "BuildingValueKind",
    },
)
_BuildingValueOptional = TypedDict(
    "_BuildingValueOptional",
    {
        # 値の型の名前（IfcLengthMeasure・IfcLabel など）
        "type_name": str,
        # real と、2^53 − 1 までの integer の値
        "number": float,
        # text と enumeration の値と、2^53 − 1 を超える integer の値
        "text": str,
        # boolean と logical の値。logical の不明は持たない
        "boolean": bool,
        # element の要素の ID
        "element": str,
        # list の値
        "items": List["BuildingValue"],
    },
    total=False,
)


class BuildingValue(_BuildingValueRequired, _BuildingValueOptional):
    "値"


# プロパティのセット
PropertySet = TypedDict(
    "PropertySet",
    {
        # 名前
        "name": str,
        # プロパティ
        "properties": List["Property"],
    },
)


# プロパティ
_PropertyRequired = TypedDict(
    "_PropertyRequired",
    {
        # 名前
        "name": str,
    },
)
_PropertyOptional = TypedDict(
    "_PropertyOptional",
    {
        # 値。値がなければ持たない
        "value": "BuildingValue",
        # 単位
        "unit": "BuildingUnit",
    },
    total=False,
)


class Property(_PropertyRequired, _PropertyOptional):
    "プロパティ"


# 数量のセット
_QuantitySetRequired = TypedDict(
    "_QuantitySetRequired",
    {
        # 名前
        "name": str,
        # 数量
        "quantities": List["Quantity"],
    },
)
_QuantitySetOptional = TypedDict(
    "_QuantitySetOptional",
    {
        # 計測の方法
        "method": str,
    },
    total=False,
)


class QuantitySet(_QuantitySetRequired, _QuantitySetOptional):
    "数量のセット"


# 数量
_QuantityRequired = TypedDict(
    "_QuantityRequired",
    {
        # 名前
        "name": str,
        # 種類
        "kind": "QuantityKind",
        # 値
        "value": "Measured",
    },
)
_QuantityOptional = TypedDict(
    "_QuantityOptional",
    {
        # 式
        "formula": str,
    },
    total=False,
)


class Quantity(_QuantityRequired, _QuantityOptional):
    "数量"


# 材料
_MaterialComponentRequired = TypedDict(
    "_MaterialComponentRequired",
    {
        # 材料の組み方
        "kind": "MaterialKind",
        # 部分
        "parts": List["MaterialPart"],
    },
)
_MaterialComponentOptional = TypedDict(
    "_MaterialComponentOptional",
    {
        # 層の向き
        "direction": str,
        # 層の基準からのずれ
        "offset": "Measured",
    },
    total=False,
)


class MaterialComponent(_MaterialComponentRequired, _MaterialComponentOptional):
    "材料"


# 材料の部分
MaterialPart = TypedDict(
    "MaterialPart",
    {
        # 材料の要素の ID
        "material": str,
        # 名前
        "name": str,
        # 厚さ
        "thickness": "Measured",
    },
    total=False,
)


# 開口
OpeningComponent = TypedDict(
    "OpeningComponent",
    {
        # この開口が穴を開ける要素の ID
        "voids": str,
        # この要素が埋める開口の要素の ID
        "fills": str,
    },
    total=False,
)


# 接続
ConnectionLink = TypedDict(
    "ConnectionLink",
    {
        # 関係の実体の名前
        "kind": str,
        # 接続する要素の ID
        "target": str,
    },
)


# レベル
_LevelComponentRequired = TypedDict(
    "_LevelComponentRequired",
    {
        # 高さ
        "height": "Measured",
    },
)
_LevelComponentOptional = TypedDict(
    "_LevelComponentOptional",
    {
        # 名前
        "name": str,
        # 種類
        "kind": str,
        # 基準
        "datum": str,
    },
    total=False,
)


class LevelComponent(_LevelComponentRequired, _LevelComponentOptional):
    "レベル"


# 区画
_ZoneComponentRequired = TypedDict(
    "_ZoneComponentRequired",
    {
        # 構成する要素の ID
        "members": List[str],
    },
)
_ZoneComponentOptional = TypedDict(
    "_ZoneComponentOptional",
    {
        # 区画の種類
        "kind": str,
    },
    total=False,
)


class ZoneComponent(_ZoneComponentRequired, _ZoneComponentOptional):
    "区画"


# 表示のスタイル
_StyleComponentRequired = TypedDict(
    "_StyleComponentRequired",
    {
        # 提示のレイヤの名前
        "layers": List[str],
    },
)
_StyleComponentOptional = TypedDict(
    "_StyleComponentOptional",
    {
        # 面の色の RGB（0 から 1）
        "surface_color": List[float],
        # 透明度（0 から 1）
        "transparency": float,
        # 線の色の RGB（0 から 1）
        "curve_color": List[float],
    },
    total=False,
)


class StyleComponent(_StyleComponentRequired, _StyleComponentOptional):
    "表示のスタイル"


# 索引から求める関係。要素の ID の列
RelationsComponent = TypedDict(
    "RelationsComponent",
    {
        # 子（集約・入れ子・包含）
        "children": List[str],
        # 参照する要素
        "referenced": List[str],
        # 型の実例
        "instances": List[str],
        # 開口
        "openings": List[str],
        # 開口を埋める要素
        "fillings": List[str],
        # グループ・システム・区画の構成員
        "members": List[str],
    },
)

# 親との関係の種類。集約・入れ子・包含
ParentKind = Union[
    Literal[
        "aggregates",
        "nests",
        "contained",
    ],
    str,
]

# 値の種類
BuildingValueKind = Union[
    Literal[
        "real",
        "integer",
        "text",
        "boolean",
        "logical",
        "enumeration",
        "element",
        "list",
    ],
    str,
]

# 数量の種類
QuantityKind = Union[
    Literal[
        "length",
        "area",
        "volume",
        "count",
        "weight",
        "time",
    ],
    str,
]

# 材料の組み方。単一・層・断面・構成・一覧
MaterialKind = Union[
    Literal[
        "single",
        "layers",
        "profiles",
        "constituents",
        "list",
    ],
    str,
]

# 仕様の結果。unknown は確かめられない facet があった
IdsStatus = Union[
    Literal[
        "passed",
        "failed",
        "unknown",
    ],
    str,
]


# 開いた IFC の建物モデルを、zm_push で渡した IDS で照合する（決定 0217・0219）。確かめられない facet は、指摘と分けて info の指摘で返す。小版 44 から
_IdsCheckRequestRequired = TypedDict(
    "_IdsCheckRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["ids_check"],
        # 文書の名前。open で IFC を開いたもの
        "doc": str,
        # IDS の入力の名前（zm_push で渡した .ids）
        "ids": str,
    },
)
_IdsCheckRequestOptional = TypedDict(
    "_IdsCheckRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # EXPRESS の入力の名前（zm_push で渡した .exp）。渡せば、IFC の属性の全体を EXPRESS の名前で照らす。渡さなければ、建物モデルの成分にある属性だけを照らす
        "express": str,
        # 返す最初の位置。前の応答の next_offset を渡す。書かなければ 0
        "offset": int,
        # 返す数の上限。書かなければ 1000
        "limit": int,
    },
    total=False,
)


class IdsCheckRequest(_IdsCheckRequestRequired, _IdsCheckRequestOptional):
    "開いた IFC の建物モデルを、zm_push で渡した IDS で照合する（決定 0217・0219）。確かめられない facet は、指摘と分けて info の指摘で返す。小版 44 から"


# 照合の結果。指摘は offset から limit 個まで
_IdsCheckResultRequired = TypedDict(
    "_IdsCheckResultRequired",
    {
        # 仕様ごとの結果。IDS の順
        "specifications": List["IdsSpecification"],
        # 指摘。仕様の順
        "findings": List["IdsFinding"],
        # 全体の指摘の数
        "total": int,
    },
)
_IdsCheckResultOptional = TypedDict(
    "_IdsCheckResultOptional",
    {
        # IDS の題
        "title": str,
        # 続きの最初の位置。続きがなければ持たない
        "next_offset": int,
    },
    total=False,
)


class IdsCheckResult(_IdsCheckResultRequired, _IdsCheckResultOptional):
    "照合の結果。指摘は offset から limit 個まで"


# 仕様の結果
_IdsSpecificationRequired = TypedDict(
    "_IdsSpecificationRequired",
    {
        # 名前
        "name": str,
        # 結果
        "status": "IdsStatus",
        # 適用の条件に合う要素の数
        "applicable": int,
        # 要件を満たさない要素の数
        "failed": int,
        # 確かめられなかった要素の数
        "unsupported": int,
    },
)
_IdsSpecificationOptional = TypedDict(
    "_IdsSpecificationOptional",
    {
        # 識別子
        "identifier": str,
    },
    total=False,
)


class IdsSpecification(_IdsSpecificationRequired, _IdsSpecificationOptional):
    "仕様の結果"


# 指摘。diagnostic は診断と同じ形で、describe で文にできる
IdsFinding = TypedDict(
    "IdsFinding",
    {
        # 仕様の名前
        "specification": str,
        # code・重要度・場所（要素の ID）・値
        "diagnostic": "Diagnostic",
    },
)


# 開いた IFC を ids_check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0218・0219）に足す。足した文書は blobs の file で返す。小版 44 から
_ReviewAdoptIdsRequestRequired = TypedDict(
    "_ReviewAdoptIdsRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["review_adopt_ids"],
        # 文書の名前。open で IFC を開いたもの
        "doc": str,
        # IDS の入力の名前（zm_push で渡した .ids）
        "ids": str,
        # 指摘が指す対象の ID。文書になければ、文書の形式とハッシュで足す
        "target": str,
        # 取り込む指摘の番号の列。ids_check の指摘の並びの順で、0 から数える
        "findings": List[int],
    },
)
_ReviewAdoptIdsRequestOptional = TypedDict(
    "_ReviewAdoptIdsRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # EXPRESS の入力の名前（zm_push で渡した .exp）。渡せば、IFC の属性の全体を EXPRESS の名前で照らす。渡さなければ、建物モデルの成分にある属性だけを照らす
        "express": str,
        # 足す先のレビューの文書の入力の名前（zm_push で渡した JSON）。書かなければ空の文書に足す
        "review": str,
    },
    total=False,
)


class ReviewAdoptIdsRequest(_ReviewAdoptIdsRequestRequired, _ReviewAdoptIdsRequestOptional):
    "開いた IFC を ids_check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0218・0219）に足す。足した文書は blobs の file で返す。小版 44 から"


# zm_push で渡した BCF 3.0 の bcfzip を読み、開いた IFC を対象にしたレビューの文書（決定 0214・0220）にする。topic は指摘に、comment はコメントに、viewpoint は 3D の場所に写し、要素の参照のハッシュは建物モデルの要素のハッシュで埋める。文書は blobs の file で返す。小版 45 から
_BcfReadRequestRequired = TypedDict(
    "_BcfReadRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["bcf_read"],
        # BCF が指す建物モデルの文書の名前。open で IFC を開いたもの
        "doc": str,
        # bcfzip の入力の名前（zm_push で渡した .bcf）
        "input": str,
        # レビューの文書の対象の ID
        "target": str,
    },
)
_BcfReadRequestOptional = TypedDict(
    "_BcfReadRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 指摘の状態と BCF の TopicStatus の文字列の対応の表。書かなければ Open・Resolved・Closed。読むとき、表にない文字列は open にし、元の文字列を status_label に残す
        "statuses": List["BcfStatus"],
    },
    total=False,
)


class BcfReadRequest(_BcfReadRequestRequired, _BcfReadRequestOptional):
    "zm_push で渡した BCF 3.0 の bcfzip を読み、開いた IFC を対象にしたレビューの文書（決定 0214・0220）にする。topic は指摘に、comment はコメントに、viewpoint は 3D の場所に写し、要素の参照のハッシュは建物モデルの要素のハッシュで埋める。文書は blobs の file で返す。小版 45 から"


# 状態の対応
BcfStatus = TypedDict(
    "BcfStatus",
    {
        # BCF の TopicStatus の文字列
        "text": str,
        # 指摘の状態
        "status": "ReviewStatus",
    },
)

# 指摘の状態（決定 0214）
ReviewStatus = Union[
    Literal[
        "open",
        "resolved",
        "closed",
    ],
    str,
]


# 読んだ bcfzip
BcfReadResult = TypedDict(
    "BcfReadResult",
    {
        # BCF の版
        "version": str,
        # 指摘の数
        "topics": int,
    },
)


# zm_push で渡したレビューの文書を、BCF 3.0 の bcfzip に書く（決定 0214・0220）。target を対象にした 3D の場所を viewpoint にし、2D の場所と図形は書かずに診断する。original を渡せば、変えていない topic を元のバイト列のまま書き、変えた topic でも写さない要素と知らない要素を保つ。bcfzip は blobs の file で返す。小版 45 から
_BcfWriteRequestRequired = TypedDict(
    "_BcfWriteRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["bcf_write"],
        # レビューの文書の入力の名前（zm_push で渡した JSON）
        "review": str,
        # viewpoint にする 3D の場所の対象の ID
        "target": str,
    },
)
_BcfWriteRequestOptional = TypedDict(
    "_BcfWriteRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 元の bcfzip の入力の名前（zm_push で渡した .bcf）。書かなければ新しく書く
        "original": str,
        # 指摘の状態と BCF の TopicStatus の文字列の対応の表。書かなければ Open・Resolved・Closed。読むとき、表にない文字列は open にし、元の文字列を status_label に残す
        "statuses": List["BcfStatus"],
        # 種類のない指摘に書く TopicType
        "kind": str,
        # 作者のない指摘とコメントに書く作者
        "author": str,
        # 日時のない指摘とコメントに書く日時（RFC 3339）
        "date": str,
    },
    total=False,
)


class BcfWriteRequest(_BcfWriteRequestRequired, _BcfWriteRequestOptional):
    "zm_push で渡したレビューの文書を、BCF 3.0 の bcfzip に書く（決定 0214・0220）。target を対象にした 3D の場所を viewpoint にし、2D の場所と図形は書かずに診断する。original を渡せば、変えていない topic を元のバイト列のまま書き、変えた topic でも写さない要素と知らない要素を保つ。bcfzip は blobs の file で返す。小版 45 から"


# 書いた bcfzip
BcfWriteResult = TypedDict(
    "BcfWriteResult",
    {
        # 書いた指摘の数
        "topics": int,
    },
)


# 表の項目
TableItem = TypedDict(
    "TableItem",
    {
        # ID。表のレコードがハンドルを持てば layer/h10・line-type/h14、なければ読んだ順の番号（layer/2）。寸法スタイルは dimension-style/h1A か、寸法スタイルの並びの中の順（dimension-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
        "id": str,
        # 元の名前。翻訳しない
        "name": str,
        # 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
        "hash": str,
    },
)


# レイヤの表の項目（決定 0162）。小版 22 から、状態とスタイルを持つ
_LayerItemRequired = TypedDict(
    "_LayerItemRequired",
    {
        # ID。表のレコードがハンドルを持てば layer/h10、なければ読んだ順の番号（layer/2）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
        "id": str,
        # 元の名前。翻訳しない
        "name": str,
        # 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
        "hash": str,
        # ファイルに定義がある。偽なら、要素が名前で指すだけのレイヤで、ほかの欄を持たない。小版 22 から
        "defined": bool,
    },
)
_LayerItemOptional = TypedDict(
    "_LayerItemOptional",
    {
        # 表示する。小版 22 から
        "visible": bool,
        # 印刷する。小版 22 から
        "printable": bool,
        # 編集できる。小版 22 から
        "editable": bool,
        # 凍結する。小版 22 から
        "frozen": bool,
        # 色。inherit を持たない。定義が色を持たなければ書かない。小版 22 から
        "color": "ColorChange",
        # 線種の ID。定義が線種を持たなければ書かない。小版 22 から
        "line_type_id": str,
        # 線幅。inherit を持たない。定義が線幅を持たなければ書かない。小版 22 から
        "line_weight": "LineWeightChange",
    },
    total=False,
)


class LayerItem(_LayerItemRequired, _LayerItemOptional):
    "レイヤの表の項目（決定 0162）。小版 22 から、状態とスタイルを持つ"


# 線種の表の項目（決定 0164）。小版 26 から、定義を持つ
_LineTypeItemRequired = TypedDict(
    "_LineTypeItemRequired",
    {
        # ID。表のレコードがハンドルを持てば line-type/h14、なければ読んだ順の番号（line-type/2）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
        "id": str,
        # 元の名前。翻訳しない
        "name": str,
        # 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
        "hash": str,
        # ファイルに定義がある。偽なら、要素が名前で指すだけの線種で、ほかの欄を持たない。小版 26 から
        "defined": bool,
    },
)
_LineTypeItemOptional = TypedDict(
    "_LineTypeItemOptional",
    {
        # 元の説明。小版 26 から
        "description": str,
        # 線と間の長さの列。正は線、負は間、0 は点。空なら実線。長さの単位は paper が決める。小版 26 から
        "pattern": List[float],
        # 列の長さが用紙の mm なら真、文書の単位なら偽。用紙の mm の列は line_type で変えられない。小版 26 から
        "paper": bool,
        # 列の要素に形か文字を添える。添えれば、line_type で列の要素の数を変えられない。小版 26 から
        "marked": bool,
        # 列の要素に添える形と文字（決定 0150・0166）。添えなければ空。小版 28 から
        "marks": List["LineTypeMark"],
    },
    total=False,
)


class LineTypeItem(_LineTypeItemRequired, _LineTypeItemOptional):
    "線種の表の項目（決定 0164）。小版 26 から、定義を持つ"


# 寸法スタイルの表の項目（決定 0165）。小版 27 から、端部記号と大きさを持つ
_DimensionStyleItemRequired = TypedDict(
    "_DimensionStyleItemRequired",
    {
        # ID。寸法スタイルのレコードがハンドルを持てば dimension-style/h1A、なければ寸法スタイルの並びの中の順（dimension-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
        "id": str,
        # 元の名前。翻訳しない
        "name": str,
        # 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
        "hash": str,
    },
)
_DimensionStyleItemOptional = TypedDict(
    "_DimensionStyleItemOptional",
    {
        # 寸法線の 1 本目と 2 本目の補助線の側の端部記号の、ブロックの名前。空の名前は既定の矢印。小版 27 から
        "ends": List[str],
        # 引出線の端部記号の、ブロックの名前。空の名前は既定の矢印。小版 27 から
        "leader": str,
        # 端部記号の大きさ。文書の単位で、scale を掛ける前の値。小版 27 から
        "size": float,
        # 端部記号の代わりに描く斜線の大きさ。0 なら斜線を描かない。小版 27 から
        "tick": float,
        # 大きさに掛ける倍率。0 はビューの縮尺から決める。小版 27 から
        "scale": float,
    },
    total=False,
)


class DimensionStyleItem(_DimensionStyleItemRequired, _DimensionStyleItemOptional):
    "寸法スタイルの表の項目（決定 0165）。小版 27 から、端部記号と大きさを持つ"


# 線種の列の要素に添える形か文字（決定 0150・0166）。要素の始まりに、線の向きとずれで置く。小版 28 から
_LineTypeMarkRequired = TypedDict(
    "_LineTypeMarkRequired",
    {
        # 添える列の要素の番号（pattern の添字）
        "index": int,
        # 形（shape）か文字（text）
        "kind": "LineTypeMarkKind",
        # 倍率。文字なら、文字スタイルの高さへの倍率
        "scale": float,
        # 回転（度、反時計回り）
        "rotation": float,
        # 回転の基準
        "rotation_basis": "MarkRotation",
        # 置く位置のずれ（x・y）。線の向きを x とする。単位は pattern と同じ
        "offset": List[float],
    },
)
_LineTypeMarkOptional = TypedDict(
    "_LineTypeMarkOptional",
    {
        # 形の形のファイル。元の形式が指す文字スタイルがなければ書かない
        "font": "Font",
        # 形の番号
        "number": int,
        # 文字の文字列
        "text": str,
        # 文字の文字スタイルの元の名前。元の形式が指す文字スタイルがなければ書かない
        "style": str,
        # 文字の文字スタイルの ID。tables の text_styles の ID で、名前で引いたもの（大文字と小文字を区別せず、同じ名前があれば先の項目）
        "style_id": str,
        # 文字の高さ。文字スタイルが決まった高さを持てば、それに scale を掛けたもの。持たなければ scale。単位は pattern と同じ
        "height": float,
    },
    total=False,
)


class LineTypeMark(_LineTypeMarkRequired, _LineTypeMarkOptional):
    "線種の列の要素に添える形か文字（決定 0150・0166）。要素の始まりに、線の向きとずれで置く。小版 28 から"

# 線種に添えるものの種類。shape は形のファイルの形、text は文字
LineTypeMarkKind = Union[
    Literal[
        "shape",
        "text",
    ],
    str,
]

# 回転の基準。relative は線の向きから、absolute は文書の x 軸から測る。upright は線の向きから測り、文字が逆さに見えるなら半回転する
MarkRotation = Union[
    Literal[
        "relative",
        "absolute",
        "upright",
    ],
    str,
]


# 文字スタイルの表の項目（決定 0158）。フォントと縦書きは、この文字スタイルを指す文字に効く。高さ・幅の倍率・傾き・裏返し・上下反転は、新しく作る文字の既定の値で、今の文字はそれぞれの値を持つ。小版 20 から
_TextStyleItemRequired = TypedDict(
    "_TextStyleItemRequired",
    {
        # ID。表のレコードがハンドルを持てば text-style/h11、なければ文字スタイルの並びの中の順（text-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
        "id": str,
        # 元の名前。翻訳しない
        "name": str,
        # 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
        "hash": str,
        # ファイルに定義がある。偽なら、ほかの欄を持たない
        "defined": bool,
    },
)
_TextStyleItemOptional = TypedDict(
    "_TextStyleItemOptional",
    {
        # 主のフォント。ファイルが決めていなければ書かない
        "font": "Font",
        # 大きいフォント（漢字などの形のファイル）。なければ書かない
        "big_font": "Font",
        # 決まった高さ。文書の単位。決めなければ書かない
        "height": float,
        # 幅の倍率
        "width": float,
        # 傾き（度）
        "oblique": float,
        # 左右を裏返す
        "backward": bool,
        # 上下を反転する
        "upside_down": bool,
        # 縦書き
        "vertical": bool,
    },
    total=False,
)


class TextStyleItem(_TextStyleItemRequired, _TextStyleItemOptional):
    "文字スタイルの表の項目（決定 0158）。フォントと縦書きは、この文字スタイルを指す文字に効く。高さ・幅の倍率・傾き・裏返し・上下反転は、新しく作る文字の既定の値で、今の文字はそれぞれの値を持つ。小版 20 から"


# フォントの指定。取得と字形の評価は使い手が行う
_FontRequired = TypedDict(
    "_FontRequired",
    {
        # 元の名前。形のファイルならファイルの名前、名前で指すフォントなら書体の名前。翻訳しない
        "name": str,
        # 種類
        "kind": "FontKind",
    },
)
_FontOptional = TypedDict(
    "_FontOptional",
    {
        # 太字。名前で指すフォントだけ
        "bold": bool,
        # 斜体。名前で指すフォントだけ
        "italic": bool,
        # 文字集合の番号（128 は Shift_JIS）。名前で指すフォントで、元の形式が決めていれば
        "charset": int,
        # ピッチと系統の番号。名前で指すフォントで、元の形式が決めていれば
        "pitch": int,
    },
    total=False,
)


class Font(_FontRequired, _FontOptional):
    "フォントの指定。取得と字形の評価は使い手が行う"

# フォントの種類。shape は形のファイル（SHX など）、named は名前で指すフォント（TrueType など）
FontKind = Union[
    Literal[
        "shape",
        "named",
    ],
    str,
]


# 要素の出所。L1 のレコードの番号かバイト範囲（決定 0008）と、形式が持つ鍵（決定 0156）。ファイルから読んでいない要素は、レコードの番号もバイト範囲も持たない
Source = TypedDict(
    "Source",
    {
        # L1 のレコードの番号
        "record": int,
        # L1 のバイト範囲
        "bytes": "ByteRange",
        # AutoCAD の系統のハンドル。大文字の 16 進。書き戻しても変わらない
        "handle": str,
        # SXF のインスタンスの番号（#n の n）。10 進の文字列。小版 6 から
        "instance": str,
    },
    total=False,
)


# 応答。失敗したときも同じ形で、result を持たない
_ResponseRequired = TypedDict(
    "_ResponseRequired",
    {
        # メッセージのスキーマの大版
        "v": int,
        # 診断。入力の順
        "diagnostics": List["Diagnostic"],
        # バイナリの結果。take で取り出す。次の call で捨てる
        "blobs": List["Blob"],
    },
)
_ResponseOptional = TypedDict(
    "_ResponseOptional",
    {
        # op ごとの結果（<op>_result の形）。失敗したときは持たない
        "result": Dict[str, Any],
    },
    total=False,
)


class Response(_ResponseRequired, _ResponseOptional):
    "応答。失敗したときも同じ形で、result を持たない"


# 診断。人向けの文は持たず、文は describe で組み立てる（決定 0009）
_DiagnosticRequired = TypedDict(
    "_DiagnosticRequired",
    {
        # 変わらない名前（budget.records.exceeded など）。catalog/codes.toml にある
        "code": str,
        # 重さ。code ごとに決まる
        "severity": "Severity",
        # まとめた件数
        "count": int,
        # 場所。入力の順に先頭の数件
        "at": List["Place"],
        # 文に差し込む値。名前と型は code ごとに台帳が決める。2^53 − 1 を超える整数は文字列にする
        "params": Dict[str, "ParamValue"],
    },
)
_DiagnosticOptional = TypedDict(
    "_DiagnosticOptional",
    {
        # 変換の結果の段。code ごとに決まる。変換の結果を表さない code では持たない
        "fidelity": "Fidelity",
        # 仕様の節
        "cite": "Cite",
    },
    total=False,
)


class Diagnostic(_DiagnosticRequired, _DiagnosticOptional):
    "診断。人向けの文は持たず、文は describe で組み立てる（決定 0009）"

# 診断の重さ（決定 0009）。info は何も失っていない、warning は結果を使える、error は失敗した
Severity = Union[
    Literal[
        "info",
        "warning",
        "error",
    ],
    str,
]

# 変換の結果の段のうち、`native` 以外のもの（決定 0009）
Fidelity = Union[
    Literal[
        "equivalent",
        "approximated",
        "drawn",
        "dropped",
    ],
    str,
]


# 診断の場所。ID と、L1 のレコード・バイト範囲（決定 0008 の出所）
Place = TypedDict(
    "Place",
    {
        # ID
        "id": str,
        # L1 のレコードの番号
        "record": int,
        # L1 のバイト範囲
        "bytes": "ByteRange",
    },
    total=False,
)


# バイト範囲（start から end の手前まで）
ByteRange = TypedDict(
    "ByteRange",
    {
        # 先頭の位置
        "start": int,
        # 終わりの位置（含まない）
        "end": int,
    },
)


# 仕様の節。仕様の本文は写さない
Cite = TypedDict(
    "Cite",
    {
        # 仕様の一覧での名前
        "spec": str,
        # 節
        "section": str,
    },
)

# 診断の値。数か文字列
ParamValue = Union[
    # 数
    float,
    # 文字列。図面に記録された名前、列挙の値のキー、2^53 − 1 を超える整数
    str,
]


# バイナリの結果
Blob = TypedDict(
    "Blob",
    {
        # 番号。zm_take に渡す
        "id": int,
        # 種類
        "kind": "BlobKind",
        # バイト数
        "size": int,
    },
)

# バイナリの結果の種類。file は書き出したファイル、image は描いた画像、scene は 2D のシーン（schema/scene.toml の配置、決定 0137・0172）、arrow は表（Arrow の IPC ストリーム。ネイティブでは zm_take_arrow でも取り出せる）
BlobKind = Union[
    Literal[
        "file",
        "image",
        "scene",
        "arrow",
    ],
    str,
]


# zm_session_new に渡す設定。渡さなければ既定の設定
_SessionOptionsRequired = TypedDict(
    "_SessionOptionsRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
    },
)
_SessionOptionsOptional = TypedDict(
    "_SessionOptionsOptional",
    {
        # 実行の形。書かなければ、ネイティブのライブラリでは `native`、wasm のモジュールでは `wasm`（決定 0013）
        "engine": "Engine",
        # engine が `process` のときの、子プロセスにする CLI（zumen）の実行ファイルのパス。書かなければ、ライブラリと同じフォルダーの zumen を使う（決定 0013・0200）
        "process_path": str,
    },
    total=False,
)


class SessionOptions(_SessionOptionsRequired, _SessionOptionsOptional):
    "zm_session_new に渡す設定。渡さなければ既定の設定"

# 実行の形（決定 0013）。`native` はプロセスの中でコアを動かす、`process` は子プロセスの CLI に送る、`wasm` は wasm のモジュールで動かす
Engine = Union[
    Literal[
        "native",
        "process",
        "wasm",
    ],
    str,
]


# 線形（決定 0230）の上の所を、追加距離か測点の表記から求め、点・方向角・高さ・勾配を返す。線形は、要求に書く（alignment）か、open で開いた J-LandXML の文書（doc）の線形を id で指す。distances・stations・interval の所を、この順に並べて返す。求められない所は、理由を診断して除く。doc を渡せば、文書の線形と地表面の要約も返す。図面のモジュールに置く。小版 46 から（doc と id は小版 47 から）
_CivilRequestRequired = TypedDict(
    "_CivilRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["civil"],
    },
)
_CivilRequestOptional = TypedDict(
    "_CivilRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 線形。doc と一緒には書かない
        "alignment": "CivilAlignment",
        # open で開いた J-LandXML の文書の名前。小版 47 から
        "doc": str,
        # doc の線形の ID。文書に線形が 1 つなら書かなくてよい。小版 47 から
        "id": str,
        # 追加距離（メートル）の列
        "distances": List[float],
        # 測点の表記（No.12+5.000・No.12+5・No.12）の列。測点の不連続のために同じ値が何か所にもあれば、最初の所にして診断する
        "stations": List[str],
        # 線形の初めから終わりまで、この間隔（メートル）ごとの所を足す。終わりの所も足す
        "interval": float,
        # 測点の間隔（メートル）。書かなければ 20
        "pitch": float,
        # 測点の表記の端数の小数の桁数。書かなければ 3
        "digits": int,
    },
    total=False,
)


class CivilRequest(_CivilRequestRequired, _CivilRequestOptional):
    "線形（決定 0230）の上の所を、追加距離か測点の表記から求め、点・方向角・高さ・勾配を返す。線形は、要求に書く（alignment）か、open で開いた J-LandXML の文書（doc）の線形を id で指す。distances・stations・interval の所を、この順に並べて返す。求められない所は、理由を診断して除く。doc を渡せば、文書の線形と地表面の要約も返す。図面のモジュールに置く。小版 46 から（doc と id は小版 47 から）"


# 線形。点は地図の空間（北と東、メートル）に置き、方向角は北から時計回りのラジアンにする。半径は、右へ曲がるものを正にする（決定 0230）
_CivilAlignmentRequired = TypedDict(
    "_CivilAlignmentRequired",
    {
        # 平面線形の区間。線形の初めから順に並べる
        "horizontal": List["CivilHorizontal"],
    },
)
_CivilAlignmentOptional = TypedDict(
    "_CivilAlignmentOptional",
    {
        # 縦断線形の区間。書かなければ高さを返さない
        "vertical": List["CivilVertical"],
        # 線形の初めの測点の値（メートル）。書かなければ 0
        "start_station": float,
        # 測点の不連続。追加距離の順に並べる
        "breaks": List["CivilStationBreak"],
    },
    total=False,
)


class CivilAlignment(_CivilAlignmentRequired, _CivilAlignmentOptional):
    "線形。点は地図の空間（北と東、メートル）に置き、方向角は北から時計回りのラジアンにする。半径は、右へ曲がるものを正にする（決定 0230）"


# 平面線形の区間
_CivilHorizontalRequired = TypedDict(
    "_CivilHorizontalRequired",
    {
        # 区間の種類
        "kind": "CivilHorizontalKind",
        # 初めの点の北（メートル）
        "northing": float,
        # 初めの点の東（メートル）
        "easting": float,
        # 初めの方向角（北から時計回りのラジアン）
        "direction": float,
        # 長さ（メートル）
        "length": float,
    },
)
_CivilHorizontalOptional = TypedDict(
    "_CivilHorizontalOptional",
    {
        # 円弧の半径（メートル）。arc に書く
        "radius": float,
        # 緩和曲線の初めの半径（メートル）。直線の側は書かない
        "start_radius": float,
        # 緩和曲線の終わりの半径（メートル）。直線の側は書かない
        "end_radius": float,
        # 緩和曲線の種類。transition に書き、書かなければ clothoid
        "curve": "CivilTransitionCurve",
    },
    total=False,
)


class CivilHorizontal(_CivilHorizontalRequired, _CivilHorizontalOptional):
    "平面線形の区間"


# 縦断線形の区間
_CivilVerticalRequired = TypedDict(
    "_CivilVerticalRequired",
    {
        # 区間の種類
        "kind": "CivilVerticalKind",
        # 初めの追加距離（メートル）
        "start_distance": float,
        # 初めの高さ（メートル）
        "start_height": float,
        # 初めの勾配（高さ ÷ 平面の距離）
        "start_gradient": float,
        # 平面の距離での長さ（メートル）
        "length": float,
    },
)
_CivilVerticalOptional = TypedDict(
    "_CivilVerticalOptional",
    {
        # 放物線の終わりの勾配。parabola に書く
        "end_gradient": float,
        # 円弧の半径（メートル）。上に凸のものを正にする。circular_arc に書く
        "radius": float,
    },
    total=False,
)


class CivilVertical(_CivilVerticalRequired, _CivilVerticalOptional):
    "縦断線形の区間"

# 平面線形の区間の種類。直線・円弧・緩和曲線
CivilHorizontalKind = Union[
    Literal[
        "line",
        "arc",
        "transition",
    ],
    str,
]

# 緩和曲線の種類。クロソイド・3 次放物線・サイン半波長逓減曲線・コサイン曲線・ブロス曲線・ヘルマート曲線・ウィーン曲線
CivilTransitionCurve = Union[
    Literal[
        "clothoid",
        "cubic_parabola",
        "sine_half_wave",
        "cosine",
        "bloss",
        "helmert",
        "viennese",
    ],
    str,
]

# 縦断線形の区間の種類。一定の勾配・2 次放物線・円弧
CivilVerticalKind = Union[
    Literal[
        "constant_gradient",
        "parabola",
        "circular_arc",
    ],
    str,
]


# 測点の不連続
CivilStationBreak = TypedDict(
    "CivilStationBreak",
    {
        # 追加距離（メートル）
        "distance": float,
        # 戻りの測点の値（不連続の手前から数えた値）
        "back": float,
        # 進みの測点の値（不連続の先で数え直す値）
        "ahead": float,
    },
)


# 線形の上の所
_CivilResultRequired = TypedDict(
    "_CivilResultRequired",
    {
        # 平面線形の長さ（メートル）
        "length": float,
        # 求めた所
        "positions": List["CivilPosition"],
    },
)
_CivilResultOptional = TypedDict(
    "_CivilResultOptional",
    {
        # 所を求めた doc の線形の ID。小版 47 から
        "alignment": str,
        # doc の線形の要約。doc を渡したときに返す。小版 47 から
        "alignments": List["CivilAlignmentSummary"],
        # doc の地表面（TIN）の要約。doc を渡したときに返す。小版 47 から
        "surfaces": List["CivilSurfaceSummary"],
        # doc の水平の座標系の EPSG のコード。分からなければ書かない。小版 47 から
        "crs": int,
    },
    total=False,
)


class CivilResult(_CivilResultRequired, _CivilResultOptional):
    "線形の上の所"


# 線形の要約
_CivilAlignmentSummaryRequired = TypedDict(
    "_CivilAlignmentSummaryRequired",
    {
        # 線形の ID
        "id": str,
        # 平面線形の長さ（メートル）
        "length": float,
        # 線形の初めの測点の値（メートル）
        "start_station": float,
        # 平面線形の区間の数
        "horizontal": int,
        # 縦断線形の区間の数
        "vertical": int,
        # 測点の不連続の数
        "breaks": int,
        # 横断の数
        "cross_sections": int,
    },
)
_CivilAlignmentSummaryOptional = TypedDict(
    "_CivilAlignmentSummaryOptional",
    {
        # 名前
        "name": str,
    },
    total=False,
)


class CivilAlignmentSummary(_CivilAlignmentSummaryRequired, _CivilAlignmentSummaryOptional):
    "線形の要約"


# 地表面（TIN）の要約
_CivilSurfaceSummaryRequired = TypedDict(
    "_CivilSurfaceSummaryRequired",
    {
        # 地表面の ID
        "id": str,
        # 役割。existing（現況）・design（計画）か、元の形式の値
        "role": str,
        # 点の数
        "points": int,
        # 三角形の数
        "triangles": int,
    },
)
_CivilSurfaceSummaryOptional = TypedDict(
    "_CivilSurfaceSummaryOptional",
    {
        # 名前
        "name": str,
    },
    total=False,
)


class CivilSurfaceSummary(_CivilSurfaceSummaryRequired, _CivilSurfaceSummaryOptional):
    "地表面（TIN）の要約"


# 線形の上の 1 つの所
_CivilPositionRequired = TypedDict(
    "_CivilPositionRequired",
    {
        # 追加距離（メートル）
        "distance": float,
        # 測点の値（メートル）
        "station": float,
        # 点の北（メートル）
        "northing": float,
        # 点の東（メートル）
        "easting": float,
        # 接線の方向角（北から時計回りのラジアン）
        "direction": float,
    },
)
_CivilPositionOptional = TypedDict(
    "_CivilPositionOptional",
    {
        # 測点の表記。表記にできなければ書かない
        "label": str,
        # 縦断線形の高さ（メートル）。求められなければ書かない
        "height": float,
        # 縦断線形の勾配。高さと同じく、求められなければ書かない
        "gradient": float,
    },
    total=False,
)


class CivilPosition(_CivilPositionRequired, _CivilPositionOptional):
    "線形の上の 1 つの所"


# open で開いた点群（LAS・LAZ）の要約と、条件に合う点を返す。点は八分木の浅い段の節から順に選ぶ（決定 0232）。図面のモジュールに置く。小版 48 から
_PointcloudRequestRequired = TypedDict(
    "_PointcloudRequestRequired",
    {
        # メッセージのスキーマの大版。今は 1
        "v": int,
        # op の名前
        "op": Literal["pointcloud"],
        # open で開いた点群の文書の名前
        "doc": str,
    },
)
_PointcloudRequestOptional = TypedDict(
    "_PointcloudRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 点を選ぶ範囲。書かなければ全体
        "region": "PointcloudBox",
        # 許容差（点の間隔、メートル）。これより細かい段の点を選ばない。書かなければすべての段
        "tolerance": float,
        # 返す点の数の上限。書かなければ点を返さず、要約だけを返す
        "limit": int,
    },
    total=False,
)


class PointcloudRequest(_PointcloudRequestRequired, _PointcloudRequestOptional):
    "open で開いた点群（LAS・LAZ）の要約と、条件に合う点を返す。点は八分木の浅い段の節から順に選ぶ（決定 0232）。図面のモジュールに置く。小版 48 から"


# 地図の空間の箱。北・東・標高（メートル）の最小と最大
PointcloudBox = TypedDict(
    "PointcloudBox",
    {
        # 最小の北・東・標高
        "min": List[float],
        # 最大の北・東・標高
        "max": List[float],
    },
)


# 点群の要約と、選んだ点
_PointcloudResultRequired = TypedDict(
    "_PointcloudResultRequired",
    {
        # 点の数
        "points": int,
        # 点が持つ属性（position・intensity・classification・color・returns・time）
        "attributes": List[str],
        # 八分木のいちばん深い段
        "depth": int,
        # 八分木の節の数
        "nodes": int,
    },
)
_PointcloudResultOptional = TypedDict(
    "_PointcloudResultOptional",
    {
        # 点の範囲。点がなければ書かない
        "bounds": "PointcloudBox",
        # 水平の座標系の EPSG のコード。分からなければ書かない
        "crs": int,
        # 高さの座標系の EPSG のコード。分からなければ書かない
        "vertical": int,
        # 選んだ点。limit を書いたときに返す
        "selected": "PointcloudPoints",
    },
    total=False,
)


class PointcloudResult(_PointcloudResultRequired, _PointcloudResultOptional):
    "点群の要約と、選んだ点"


# 選んだ点。どの列も、選んだ点の順で同じ長さにする
_PointcloudPointsRequired = TypedDict(
    "_PointcloudPointsRequired",
    {
        # 北・東・標高（メートル）
        "positions": List[List[float]],
        # limit のために選ばなかった点がある
        "truncated": bool,
        # 選んだいちばん深い段
        "level": int,
    },
)
_PointcloudPointsOptional = TypedDict(
    "_PointcloudPointsOptional",
    {
        # 強度。点群が持つときに返す
        "intensity": List[int],
        # 分類。点群が持つときに返す
        "classification": List[int],
        # 色（赤・緑・青。16 ビット）。点群が持つときに返す
        "color": List[List[int]],
    },
    total=False,
)


class PointcloudPoints(_PointcloudPointsRequired, _PointcloudPointsOptional):
    "選んだ点。どの列も、選んだ点の順で同じ長さにする"


# describe に使うカタログのファイル。catalog/<言語>.toml から生成し、zm_push で渡す（決定 0009）
Catalog = TypedDict(
    "Catalog",
    {
        # カタログの形の大版。今は 1
        "v": int,
        # 言語（ja・en）
        "lang": str,
        # パッケージの版。コアと同じ版で出す
        "package": str,
        # 数の書き方
        "number": "NumberFormat",
        # code ごとの文のひな形。{名前} に値が、{count} にまとめた件数が入る
        "codes": Dict[str, str],
    },
)


# 文に差し込む数の書き方。OS のロケールは使わない（決定 0009）
NumberFormat = TypedDict(
    "NumberFormat",
    {
        # 整数部を 3 桁ごとに区切る文字。空なら区切らない
        "group_separator": str,
        # 小数点の文字
        "decimal_separator": str,
    },
)

# op の名前
Operation = Literal[
    "capabilities",
    "open",
    "close",
    "describe",
    "drawing",
    "entities",
    "apply",
    "convert",
    "tables",
    "scene",
    "check",
    "validate",
    "review_carry",
    "review_adopt",
    "building",
    "building_elements",
    "ids_check",
    "review_adopt_ids",
    "bcf_read",
    "bcf_write",
    "civil",
    "pointcloud",
]

# op ごとの要求と結果の型の名前。結果は応答の result を typing.cast で読む型
OPERATIONS: Dict[str, Any] = {
    "capabilities": ("CapabilitiesRequest", "CapabilitiesResult"),
    "open": ("OpenRequest", "OpenResult"),
    "close": ("CloseRequest", "CloseResult"),
    "describe": ("DescribeRequest", "DescribeResult"),
    "drawing": ("DrawingRequest", "DrawingResult"),
    "entities": ("EntitiesRequest", "EntitiesResult"),
    "apply": ("ApplyRequest", "ApplyResult"),
    "convert": ("ConvertRequest", "ConvertResult"),
    "tables": ("TablesRequest", "TablesResult"),
    "scene": ("SceneRequest", "SceneResult"),
    "check": ("CheckRequest", "CheckResult"),
    "validate": ("ValidateRequest", "ValidateResult"),
    "review_carry": ("ReviewCarryRequest", "ReviewCarryResult"),
    "review_adopt": ("ReviewAdoptRequest", "ReviewAdoptResult"),
    "building": ("BuildingRequest", "BuildingResult"),
    "building_elements": ("BuildingElementsRequest", "BuildingElementsResult"),
    "ids_check": ("IdsCheckRequest", "IdsCheckResult"),
    "review_adopt_ids": ("ReviewAdoptIdsRequest", "Dict[str, Any]"),
    "bcf_read": ("BcfReadRequest", "BcfReadResult"),
    "bcf_write": ("BcfWriteRequest", "BcfWriteResult"),
    "civil": ("CivilRequest", "CivilResult"),
    "pointcloud": ("PointcloudRequest", "PointcloudResult"),
}
