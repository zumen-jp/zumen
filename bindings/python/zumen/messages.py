# 生成したファイル。手で直さない。
# 元は schema/messages.json で、`cargo xtask codegen` で作り直す。
#
# API のメッセージの型（決定 0012・0195）。スキーマの定義ごとに 1 つの型がある。
# 列挙は、知らない値も受け取れるように、ほかの文字列も許す（決定 0015）。
# 予約語のフィールド名（from など）を扱うため TypedDict を関数の形で書き、定義の名前は文字列で引く。

from typing import Any, Dict, List, Literal, TypedDict, Union

# 要求。op の値で形が決まる
Request = Union[
    "CapabilitiesRequest",
    "OpenRequest",
    "CloseRequest",
    "DescribeRequest",
    "DrawingRequest",
    "EntitiesRequest",
    "CalibrationsRequest",
    "MeasureDistanceRequest",
    "MeasureRequest",
    "IntersectRequest",
    "RegionsRequest",
    "ConstructRequest",
    "ApplyRequest",
    "CreateRequest",
    "ConvertRequest",
    "ConvertEstimateRequest",
    "TablesRequest",
    "SceneRequest",
    "Scene3dRequest",
    "CheckRequest",
    "ValidateRequest",
    "ReviewCarryRequest",
    "ReviewAdoptRequest",
    "ReviewSceneRequest",
    "BuildingRequest",
    "BuildingElementsRequest",
    "IdsCheckRequest",
    "ReviewAdoptIdsRequest",
    "BcfReadRequest",
    "BcfWriteRequest",
    "CivilRequest",
    "PointcloudRequest",
    "DeviationRequest",
    "CivilDrawingRequest",
    "GeoRequest",
    "DeliveryCheckRequest",
    "DeliveryApplyRequest",
    "PhotoRequest",
    "BoringRequest",
    "BuildingApplyRequest",
    "LiftRequest",
    "RenderRequest",
    "RasterRequest",
]


# どの op の要求にも置けるフィールド
_RequestCommonRequired = TypedDict(
    "_RequestCommonRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
    },
    total=False,
)


class RequestCommon(_RequestCommonRequired, _RequestCommonOptional):
    "どの op の要求にも置けるフィールド"


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


# コアのバージョンと、使える op・形式を尋ねる
_CapabilitiesRequestRequired = TypedDict(
    "_CapabilitiesRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
    },
    total=False,
)


class CapabilitiesRequest(_CapabilitiesRequestRequired, _CapabilitiesRequestOptional):
    "コアのバージョンと、使える op・形式を尋ねる"


# コアのバージョンと、使える op・形式（決定 0015）
CapabilitiesResult = TypedDict(
    "CapabilitiesResult",
    {
        # パッケージのバージョン（semver）
        "package": str,
        # ABI のメジャーバージョン。zm_abi_version と同じ
        "abi": int,
        # メッセージのスキーマのバージョン
        "schema": "SchemaVersion",
        # このコア（wasm ではこのモジュール）で使える op の名前
        "ops": List[str],
        # 読み書きできる形式
        "formats": List["Format"],
    },
)


# メッセージのスキーマのバージョン。メジャーバージョンは要求と応答の v、マイナーバージョンは op・フィールド・列挙の値を足すたびに上げる
SchemaVersion = TypedDict(
    "SchemaVersion",
    {
        # メジャーバージョン
        "major": int,
        # マイナーバージョン
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
        # create で空の文書を作れるか。マイナーバージョン 83 から
        "create": bool,
        # この形式の ID の規則のバージョン（決定 0015）
        "id_rules": int,
    },
)


# 入力を読んで、文書を作る。形式は入力から判定する
_OpenRequestRequired = TypedDict(
    "_OpenRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 作る文書の名前。書かなければ入力の名前
        "doc": str,
        # 入力の全体の大きさ（バイト）。渡した入力より大きければ、入力を全体の初めの部分とみなし、足りない範囲を needs で求める。範囲で読める形式（LAS・LAZ）だけが使う（決定 0232）。マイナーバージョン 48 から
        "size": int,
        # 見分けるマーカーのない形式（xyz・extdm・geojson）を名指して読む。書かなければ、入力から判定する（決定 0232）。マイナーバージョン 49 から。extdm はマイナーバージョン 62 から。geojson はマイナーバージョン 71 から
        "format": str,
        # SXF の属性ファイル（SAF）を入れた入力の名前。zm_push で渡したもの。属性ファイル用の属性付加機構（$$ATRF$$）の要素に、属性を sxf.saf の拡張で結ぶ（決定 0249）。書かなければ、P2Z・SFZ の中の、図面と同じ名前の SAF を使う。SXF でない入力には使えない。マイナーバージョン 63 から
        "saf": str,
        # IFC-SPF・ifcZIP の 8 ビットのエスケープを読む文字コード。省略時は auto。文書の building_apply 後も指定を保つ。IFC 以外の入力には使えない。マイナーバージョン 76 から
        "ifc_text_encoding": "IfcTextEncoding",
        # COPC を、この条件に掛かる八分木の節の範囲だけで読む。足りない範囲は needs で求める。作った文書は読んだ節の点だけを持ち、pointcloud・scene3d はその点を扱う。条件を変えるときは開き直す。書かなければ全体を読む。COPC でない入力には使えない（#899、決定 0232）。マイナーバージョン 104 から
        "pointcloud": "OpenPointcloud",
    },
    total=False,
)


class OpenRequest(_OpenRequestRequired, _OpenRequestOptional):
    "入力を読んで、文書を作る。形式は入力から判定する"


# COPC の八分木の節を選ぶ条件。pointcloud の op の同じ名前のフィールドと同じ規則で節を選ぶ
OpenPointcloud = TypedDict(
    "OpenPointcloud",
    {
        # 節を選ぶ範囲。書かなければ全体
        "region": "PointcloudBox",
        # 許容差（点の間隔、メートル）。これより細かい段の節を読まない。書かなければすべての段
        "tolerance": float,
        # 点の数の上限。節を浅い段から順に数え、範囲にすべて入る節の点の数の和が上限に届いたら、それより後の節を読まない。範囲に一部だけ掛かる節は数えない。書かなければ上限なし
        "limit": int,
    },
    total=False,
)

# IFC の文字列の読み方。auto はファイル内に CP932 の手がかりがあれば読み替える。iso_8859_1 は従来の ISO 8859-1 と Unicode、shift_jis は手がかりなしでも CP932 として読める文字列を読み替える。どの指定も元のバイト列を保つ
IfcTextEncoding = Union[
    Literal[
        "auto",
        "iso_8859_1",
        "shift_jis",
    ],
    str,
]


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
        # 読むのに足りない範囲。あれば文書はまだ作らない。範囲をその名前で zm_push してから、同じ要求をもう一度呼ぶ（決定 0232）。マイナーバージョン 48 から
        "needs": List["InputRange"],
        # 読み終えたので、close の inputs で捨ててよい範囲の入力。マイナーバージョン 48 から
        "done": List["InputRange"],
        # SXF を ZIP に入れたアーカイブ（P2Z・SFZ）なら、SXF の項目のほかの項目（属性ファイル・画像など）のアーカイブの中の名前。アーカイブの順。apply は、SXF の項目だけを置き換え、ほかの項目を写したアーカイブを書く（#385）。マイナーバージョン 54 から
        "entries": List[str],
        # open の pointcloud の上限のために、読まなかった節がある。COPC を条件で読んだときだけ書く（#899）。マイナーバージョン 104 から
        "truncated": bool,
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
        # メッセージのスキーマのメジャーバージョン。今は 1
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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
        # メッセージのスキーマのメジャーバージョン。今は 1
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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


# 開いた文書を図面モデル（L2、決定 0022）に変換し、その要約を返す。ネイティブと wasm の出力の一致を、適合テストで比べるためにも使う（決定 0007）
_DrawingRequestRequired = TypedDict(
    "_DrawingRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
    },
    total=False,
)


class DrawingRequest(_DrawingRequestRequired, _DrawingRequestOptional):
    "開いた文書を図面モデル（L2、決定 0022）に変換し、その要約を返す。ネイティブと wasm の出力の一致を、適合テストで比べるためにも使う（決定 0007）"


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
        # 文書の点の表し方（marker を持たない点の見え方、決定 0160）。point_style の変更で変える。文書が持たなければ（JWW・SXF）持たない。マイナーバージョン 39 から
        "point_style": "PointStyleSummary",
    },
    total=False,
)


class DrawingResult(_DrawingResultRequired, _DrawingResultOptional):
    "図面モデルの要約。移さなかった図形と値は、診断で返す"


# 2 次元の座標
Point2 = TypedDict(
    "Point2",
    {
        # X 座標
        "x": float,
        # Y 座標
        "y": float,
    },
)


# 原点と相対座標を分けた点。両成分の正確な和を位置とし、先に足して丸めない。座標の単位は使う所に従う（measure_distance では用紙の mm、measure では from の空間か用紙の単位）
SplitPoint2 = TypedDict(
    "SplitPoint2",
    {
        # 用紙の原点
        "origin": "Point2",
        # 原点からの相対座標
        "offset": "Point2",
    },
)


# 用紙の閉じた矩形で使う距離の校正。重なる領域は ID で明示的に選ぶ。マイナーバージョン 68 から
_CalibrationItemRequired = TypedDict(
    "_CalibrationItemRequired",
    {
        # 文書内で重複しない校正の ID
        "id": str,
        # 校正を使える公開用紙の ID。drawing が返すもの
        "from": str,
        # 用紙の mm の閉じた矩形。[X の最小, Y の最小, X の最大, Y の最大] の 4 値で、幅と高さは正
        "bounds": List[float],
        # 用紙の mm の変位から校正単位の変位へ写す行優先の 2×2 行列。[a, b, c, d] の 4 値で、[dx, dy] を [a*dx+b*dy, c*dx+d*dy] に写す
        "matrix": List[float],
        # 校正後の長さの単位。ラベルだけから推定しない
        "unit": "Unit",
        # 元の単位ラベル。未知の単位でも原文を保つ
        "unit_label": str,
    },
)
_CalibrationItemOptional = TypedDict(
    "_CalibrationItemOptional",
    {
        # 元の校正の名前。翻訳しない。なければ持たない
        "name": str,
        # 元の形式のレコード
        "source": "Source",
    },
    total=False,
)


class CalibrationItem(_CalibrationItemRequired, _CalibrationItemOptional):
    "用紙の閉じた矩形で使う距離の校正。重なる領域は ID で明示的に選ぶ。マイナーバージョン 68 から"


# 開いた文書を図面モデルに変換し、用紙の距離の校正をモデルの順に返す。マイナーバージョン 68 から
_CalibrationsRequestRequired = TypedDict(
    "_CalibrationsRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["calibrations"],
        # 文書の名前。open で作ったもの
        "doc": str,
    },
)
_CalibrationsRequestOptional = TypedDict(
    "_CalibrationsRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 公開用紙の ID。drawing が返すもの。書けばその用紙の校正だけを返し、書かなければすべての校正を返す
        "from": str,
        # 返す最初の校正の位置。from で絞った後の位置で、前の応答の next_offset を渡す。書かなければ 0
        "offset": int,
        # 返す校正の数の上限。書かなければ 1000
        "limit": int,
        # 期待する元の文書のハッシュ。open に渡したバイト列のハッシュと照合する。図面モデルへ変換した ZMDR 自身のハッシュではない
        "expected_hash": str,
    },
    total=False,
)


class CalibrationsRequest(_CalibrationsRequestRequired, _CalibrationsRequestOptional):
    "開いた文書を図面モデルに変換し、用紙の距離の校正をモデルの順に返す。マイナーバージョン 68 から"


# 距離の校正の一覧の一部
_CalibrationsResultRequired = TypedDict(
    "_CalibrationsResultRequired",
    {
        # 元の文書のハッシュ。ZMDR から開いたときは、埋め込まれた元の文書のハッシュ（Source.hash）
        "hash": str,
        # from で絞った一覧の offset から limit 個までの校正
        "items": List["CalibrationItem"],
        # from で絞った後の校正の総数
        "total": int,
    },
)
_CalibrationsResultOptional = TypedDict(
    "_CalibrationsResultOptional",
    {
        # 続きの最初の校正の位置。続きがなければ持たない
        "next_offset": int,
    },
    total=False,
)


class CalibrationsResult(_CalibrationsResultRequired, _CalibrationsResultOptional):
    "距離の校正の一覧の一部"


# 用紙と校正を明示的に選び、始点と終点の校正後の距離を求める。両点は校正の閉領域内にある必要がある。マイナーバージョン 68 から
_MeasureDistanceRequestRequired = TypedDict(
    "_MeasureDistanceRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["measure_distance"],
        # 文書の名前。open で作ったもの
        "doc": str,
        # 公開用紙の ID。drawing が返すもので、選んだ校正と同じ用紙
        "from": str,
        # 使う校正の ID。calibrations が返すもの
        "calibration": str,
        # 始点。原点と相対座標を分けた用紙の mm の点
        "start": "SplitPoint2",
        # 終点。原点と相対座標を分けた用紙の mm の点
        "end": "SplitPoint2",
    },
)
_MeasureDistanceRequestOptional = TypedDict(
    "_MeasureDistanceRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 期待する元の文書のハッシュ。open に渡したバイト列のハッシュと照合する。図面モデルへ変換した ZMDR 自身のハッシュではない
        "expected_hash": str,
    },
    total=False,
)


class MeasureDistanceRequest(_MeasureDistanceRequestRequired, _MeasureDistanceRequestOptional):
    "用紙と校正を明示的に選び、始点と終点の校正後の距離を求める。両点は校正の閉領域内にある必要がある。マイナーバージョン 68 から"


# 選んだ用紙と校正による距離
MeasureDistanceResult = TypedDict(
    "MeasureDistanceResult",
    {
        # 元の文書のハッシュ。ZMDR から開いたときは、埋め込まれた元の文書のハッシュ（Source.hash）
        "hash": str,
        # 計測した公開用紙の ID
        "from": str,
        # 使った校正の ID
        "calibration": str,
        # 校正後の距離。unit と unit_label が示す単位
        "distance": float,
        # 校正後の長さの単位
        "unit": "Unit",
        # 元の単位ラベル。未知の単位でも原文を保つ
        "unit_label": str,
    },
)


# モデル空間の要素の長さ・周長・面積、2 要素または点の間の最短距離を測る。線幅と表示用の変換は含めない
_MeasureRequestRequired = TypedDict(
    "_MeasureRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["measure"],
        # 開いた文書の名前
        "doc": str,
        # 実寸の空間の公開 ID。記号定義の空間は対象外。マイナーバージョン 102 から、distance で両方の対象が点なら公開用紙の ID も使え、校正しない用紙の座標の距離を返す
        "from": str,
        # 求める値
        "quantity": "MeasureQuantity",
        # 計測する要素または点
        "first": "MeasureTarget",
    },
)
_MeasureRequestOptional = TypedDict(
    "_MeasureRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 最短距離のもう一方の要素または点。distance のときだけ必須
        "second": "MeasureTarget",
        # 近似する曲線の分割で目標にする弦の誤差。空間の長さの単位で正の値。既定は 0.001。計測した長さ・面積・距離の誤差上限ではない
        "tolerance": float,
        # 期待する元の文書のハッシュ。open の元のバイト列のハッシュと照合する
        "expected_hash": str,
    },
    total=False,
)


class MeasureRequest(_MeasureRequestRequired, _MeasureRequestOptional):
    "モデル空間の要素の長さ・周長・面積、2 要素または点の間の最短距離を測る。線幅と表示用の変換は含めない"

# 長さ、閉じた経路の周長、向き付き線積分の絶対値の面積、境界間の最短距離。面積は自己交差を塗り規則で統合しない
MeasureQuantity = Union[
    Literal[
        "length",
        "perimeter",
        "area",
        "distance",
    ],
    str,
]

# 同じモデル空間内の要素または座標
MeasureTarget = Union[
    "MeasureElement",
    "MeasurePoint",
    "MeasureSplitPoint",
]


# 計測する要素。元の公開 ID を使う
MeasureElement = TypedDict(
    "MeasureElement",
    {
        # 対象の種類
        "kind": Literal["element"],
        # 同じ空間の entities が返した要素の ID
        "id": str,
    },
)


# 距離の計測で使う空間の座標
MeasurePoint = TypedDict(
    "MeasurePoint",
    {
        # 対象の種類
        "kind": Literal["point"],
        # 空間の長さの単位の点
        "position": "Point2",
    },
)


# 距離の計測で使う、原点と相対座標を分けた座標。distance で、もう一方も点のときだけ使える。マイナーバージョン 102 から
MeasureSplitPoint = TypedDict(
    "MeasureSplitPoint",
    {
        # 対象の種類
        "kind": Literal["split_point"],
        # 原点と相対座標を分けた点。成分ごとの差を正確に求めてから一度だけ丸める
        "position": "SplitPoint2",
    },
)


# 要素から求めた計測値。近似値かどうか、単位と出所の空間を明示する
_MeasureResultRequired = TypedDict(
    "_MeasureResultRequired",
    {
        # 元の文書のハッシュ
        "hash": str,
        # 計測した実寸の空間の公開 ID
        "from": str,
        # 求めた値の種類
        "quantity": "MeasureQuantity",
        # 計測値。0 以上の有限値
        "value": float,
        # 空間の長さの単位。不明な単位を補わない。from が用紙なら用紙の mm
        "unit": "Unit",
        # 単位の次数。長さと距離は 1、面積は 2
        "unit_power": int,
        # 計測に曲線の折れ線近似を使った。浮動小数の丸めとは区別する
        "approximate": bool,
        # 指定した、または既定の分割の目標。近似しないときも返す
        "tolerance": float,
    },
)
_MeasureResultOptional = TypedDict(
    "_MeasureResultOptional",
    {
        # from が用紙で、value が校正しない用紙の座標の距離なら true。実寸ではない。実寸の空間では返さない。マイナーバージョン 102 から
        "sheet": bool,
        # 最短距離を作る最初の対象上の点。distance のときだけ返す。split_point なら原点と相対座標の和を丸めた点
        "nearest_first": "Point2",
        # 最短距離を作るもう一方の対象上の点。近似した対象では折れ線上の点
        "nearest_second": "Point2",
    },
    total=False,
)


class MeasureResult(_MeasureResultRequired, _MeasureResultOptional):
    "要素から求めた計測値。近似値かどうか、単位と出所の空間を明示する"


# 同じ実寸の空間の 2 要素の交点・接点・重なりを求める。文書を変更しない。マイナーバージョン 90 から
_IntersectRequestRequired = TypedDict(
    "_IntersectRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["intersect"],
        # 開いた文書の名前
        "doc": str,
        # 実寸の空間の公開 ID。用紙と記号定義の空間は対象外
        "from": str,
        # 交わりを求める要素
        "first": "MeasureElement",
        # 交わりを求めるもう一方の要素。first と異なる ID
        "second": "MeasureElement",
    },
)
_IntersectRequestOptional = TypedDict(
    "_IntersectRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 許容差。空間の長さの単位で正の値。既定は 0.001。整数格子の間隔はこの 1/4 で、近似する曲線の分割の目標にも使う
        "tolerance": float,
        # 期待する元の文書のハッシュ。open の元のバイト列のハッシュと照合する
        "expected_hash": str,
    },
    total=False,
)


class IntersectRequest(_IntersectRequestRequired, _IntersectRequestOptional):
    "同じ実寸の空間の 2 要素の交点・接点・重なりを求める。文書を変更しない。マイナーバージョン 90 から"


# 交点または接点。格子点の座標
IntersectionPoint = TypedDict(
    "IntersectionPoint",
    {
        # 空間の長さの単位の点
        "position": "Point2",
        # 接点。同じ格子点にまとめたすべての交わりが接するときだけ真
        "tangent": bool,
        # 正確な交点が格子点でなく、近い格子点へ丸めた。まとめたすべての交わりが丸めたときだけ真
        "rounded": bool,
    },
)

# 重なりの形。segment は線分、arc は反時計回りの円弧、circle は円全体
IntersectionOverlapKind = Union[
    Literal[
        "segment",
        "arc",
        "circle",
    ],
    str,
]


# 2 要素が長さを持って重なる区間。区間の組ごとに返し、つながる区間をつながない
_IntersectionOverlapRequired = TypedDict(
    "_IntersectionOverlapRequired",
    {
        # 重なりの形
        "kind": "IntersectionOverlapKind",
    },
)
_IntersectionOverlapOptional = TypedDict(
    "_IntersectionOverlapOptional",
    {
        # 始点。segment は X、Y の辞書順で小さい端、arc は反時計回りの始点。circle では持たない
        "start": "Point2",
        # 終点。circle では持たない
        "end": "Point2",
        # 円弧と円の中心。segment では持たない
        "center": "Point2",
        # 円弧と円の格子上の半径。segment では持たない
        "radius": float,
    },
    total=False,
)


class IntersectionOverlap(_IntersectionOverlapRequired, _IntersectionOverlapOptional):
    "2 要素が長さを持って重なる区間。区間の組ごとに返し、つながる区間をつながない"


# 2 要素の交わり。first と second を入れ替えても同じ結果を返す
IntersectResult = TypedDict(
    "IntersectResult",
    {
        # 元の文書のハッシュ
        "hash": str,
        # 求めた実寸の空間の公開 ID
        "from": str,
        # 空間の長さの単位。不明な単位を補わない
        "unit": "Unit",
        # 指定した、または既定の許容差
        "tolerance": float,
        # 曲線を折れ線に近似した要素がある
        "approximate": bool,
        # 交点と接点。格子点の X、Y の辞書順。重なりの端点と同じ点は含めない
        "points": List["IntersectionPoint"],
        # 重なり。形・座標の順
        "overlaps": List["IntersectionOverlap"],
    },
)


# 同じ実寸の空間の要素の線が囲む閉領域を検出する、または閉じた要素どうしの和・積・差・排他的論理和を求める。文書を変更しない。マイナーバージョン 91 から
_RegionsRequestRequired = TypedDict(
    "_RegionsRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["regions"],
        # 開いた文書の名前
        "doc": str,
        # 実寸の空間の公開 ID。用紙と記号定義の空間は対象外
        "from": str,
        # 求め方
        "operation": "RegionOperation",
        # detect では線で面を囲む要素、ほかでは集合演算の左側の閉じた要素。1 つ以上で、同じ ID を繰り返さない
        "subject": List["MeasureElement"],
    },
)
_RegionsRequestOptional = TypedDict(
    "_RegionsRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 集合演算の右側の閉じた要素。書かなければ空。detect では書かない。同じ ID を繰り返さない
        "clip": List["MeasureElement"],
        # 閉じた要素の内部を決める規則。書かなければ even_odd。detect では書かない
        "fill": "RegionFill",
        # 許容差。空間の長さの単位で正の値。既定は 0.001。整数格子の間隔はこの 1/4 で、円弧を弦に分ける目標にも使う
        "tolerance": float,
        # 期待する元の文書のハッシュ。open の元のバイト列のハッシュと照合する
        "expected_hash": str,
    },
    total=False,
)


class RegionsRequest(_RegionsRequestRequired, _RegionsRequestOptional):
    "同じ実寸の空間の要素の線が囲む閉領域を検出する、または閉じた要素どうしの和・積・差・排他的論理和を求める。文書を変更しない。マイナーバージョン 91 から"

# 閉領域の求め方。detect は subject の線が囲むすべての有限の面、union・intersection・difference・xor は subject と clip の和・積・差（subject から clip を除く）・排他的論理和
RegionOperation = Union[
    Literal[
        "detect",
        "union",
        "intersection",
        "difference",
        "xor",
    ],
    str,
]

# 閉じた要素の内部を決める規則。even_odd は境界を横切る回数が奇数の部分、non_zero は符号を付けた周回数が 0 でない部分
RegionFill = Union[
    Literal[
        "even_odd",
        "non_zero",
    ],
    str,
]


# 外周と穴で表した閉領域。最後の点から最初の点へ閉じる
ClosedRegion = TypedDict(
    "ClosedRegion",
    {
        # 反時計回りの外周。格子点の座標で、辞書順で最小の点から始める
        "outer": List["Point2"],
        # 時計回りの穴。輪郭の辞書順
        "holes": List[List["Point2"]],
        # 外周から穴を除いた面積。空間の長さの単位の 2 乗
        "area": float,
    },
)


# 交点を格子点へ丸めて元の要素の線を曲げた点
RegionRounding = TypedDict(
    "RegionRounding",
    {
        # 曲げた要素の公開 ID
        "id": str,
        # 丸めた格子点
        "position": "Point2",
    },
)

# 閉領域を作らなかった入力の種類。degenerate は点や一直線に縮んだ要素か長さ 0 の線、open は閉領域の境界に使わなかった線の部分
RegionDiscardKind = Union[
    Literal[
        "degenerate",
        "open",
    ],
    str,
]


# 閉領域を作らなかった入力
_RegionDiscardRequired = TypedDict(
    "_RegionDiscardRequired",
    {
        # 元の要素の公開 ID
        "id": str,
        # 種類
        "kind": "RegionDiscardKind",
    },
)
_RegionDiscardOptional = TypedDict(
    "_RegionDiscardOptional",
    {
        # 使わなかった部分の始点。open だけが持つ
        "start": "Point2",
        # 使わなかった部分の終点。open だけが持つ
        "end": "Point2",
    },
    total=False,
)


class RegionDiscard(_RegionDiscardRequired, _RegionDiscardOptional):
    "閉領域を作らなかった入力"


# 閉領域。要素の順序を入れ替えても同じ領域を返す
RegionsResult = TypedDict(
    "RegionsResult",
    {
        # 元の文書のハッシュ
        "hash": str,
        # 求めた実寸の空間の公開 ID
        "from": str,
        # 空間の長さの単位。不明な単位を補わない
        "unit": "Unit",
        # 指定した、または既定の許容差
        "tolerance": float,
        # 曲線を折れ線に近似した要素がある
        "approximate": bool,
        # 閉領域。外周、穴の順の辞書順
        "regions": List["ClosedRegion"],
        # 丸めた分割点。要素の指定順、格子点の辞書順
        "rounded": List["RegionRounding"],
        # 閉領域を作らなかった入力。degenerate、続いて open を元の入力順で返す
        "discarded": List["RegionDiscard"],
    },
)


# 同じ実寸の空間の要素のオフセット・トリム・延長・面取り・フィレットの結果の形を求める。文書を変更しない。結果は apply の remove・replace・add で当てる。形の種類が変わるときは replace でなく remove と add にする。マイナーバージョン 92 から
_ConstructRequestRequired = TypedDict(
    "_ConstructRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["construct"],
        # 開いた文書の名前
        "doc": str,
        # 実寸の空間の公開 ID。用紙と記号定義の空間は対象外
        "from": str,
        # 作図の種類
        "operation": "ConstructOperation",
        # 作図する要素。線分・円・円弧。面取りとフィレットでは 1 本目の線分
        "target": "MeasureElement",
    },
)
_ConstructRequestOptional = TypedDict(
    "_ConstructRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 面取りとフィレットの 2 本目の線分。target と端点を共有し、target と異なる ID
        "second": "MeasureElement",
        # トリムと延長の境界の要素。線分・円・円弧・折れ線と、近似する曲線。1 つ以上で、同じ ID と target を含めない
        "boundaries": List["MeasureElement"],
        # トリムで削る区間を選ぶ点。線分は元の直線へ垂直に投影し、円・円弧は中心からの向きで選ぶ
        "pick": "Point2",
        # 延長する端点
        "end": "ConstructEnd",
        # オフセットの距離。線分は始点から終点へ向かって左、円・円弧は外側を正にする
        "distance": float,
        # 面取りで、共有する端点から target と second の上を進む正の距離 [target, second]
        "distances": List[float],
        # フィレットの円弧の正の半径
        "radius": float,
        # 許容差。空間の長さの単位で正の値。既定は 0.001。整数格子の間隔はこの 1/4 で、境界の曲線を近似する目標にも使う
        "tolerance": float,
        # 期待する元の文書のハッシュ。open の元のバイト列のハッシュと照合する
        "expected_hash": str,
    },
    total=False,
)


class ConstructRequest(_ConstructRequestRequired, _ConstructRequestOptional):
    "同じ実寸の空間の要素のオフセット・トリム・延長・面取り・フィレットの結果の形を求める。文書を変更しない。結果は apply の remove・replace・add で当てる。形の種類が変わるときは replace でなく remove と add にする。マイナーバージョン 92 から"

# offset は distance だけ平行に移した新しい形、trim は pick を含む区間を最初に当たる境界の間で削った残り、extend は end の端点を最初に当たる境界まで延ばした形、chamfer は target と second の角を distances で切る線分、fillet は radius の接する円弧
ConstructOperation = Union[
    Literal[
        "offset",
        "trim",
        "extend",
        "chamfer",
        "fillet",
    ],
    str,
]

# 延長する端点。start は始点を逆向きへ、end は終点を進む向きへ延ばす
ConstructEnd = Union[
    Literal[
        "start",
        "end",
    ],
    str,
]


# 元の要素を置き換える形。空なら要素を消す。2 つなら 1 つ目で置き換えて 2 つ目を足す。円をトリムした円弧のように種類が変わる形は、元の要素を消して足す
ConstructReplacement = TypedDict(
    "ConstructReplacement",
    {
        # 置き換える要素の公開 ID
        "id": str,
        # 置き換える形。元の向きの順
        "geometries": List["Geometry"],
    },
)


# トリムと延長で採用した境界と切断点
ConstructCut = TypedDict(
    "ConstructCut",
    {
        # 境界の要素の公開 ID
        "boundary": str,
        # 元の交点を丸めた格子点
        "position": "Point2",
        # 元の交点が格子点でなく、近い格子点へ丸めた
        "rounded": bool,
        # 丸めた点を元の線・円の上へ投影したため、正確な切断位置と異なる
        "adjusted": bool,
    },
)


# 作図の結果の形。文書は変わらない
ConstructResult = TypedDict(
    "ConstructResult",
    {
        # 元の文書のハッシュ
        "hash": str,
        # 求めた実寸の空間の公開 ID
        "from": str,
        # 空間の長さの単位。不明な単位を補わない
        "unit": "Unit",
        # 指定した、または既定の許容差
        "tolerance": float,
        # 境界の曲線を折れ線に近似した
        "approximate": bool,
        # 置き換える要素。target、second の順
        "replace": List["ConstructReplacement"],
        # 足す形。オフセットの形、面取りの線分、フィレットの円弧
        "add": List["Geometry"],
        # トリムと延長で採用した切断点。元の向きの順
        "cuts": List["ConstructCut"],
    },
)


# 文書の点の表し方。フィールドの意味は point_style_edit と同じ
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
        # ビューの詳細。元の順に返す。マイナーバージョン 81 から
        "view_details": List["ViewDetail"],
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
        # 円。マイナーバージョン 81 から
        "circles": int,
        # 円弧・楕円弧・スプラインなどの曲線。マイナーバージョン 81 から
        "curves": int,
        # 折れ線。マイナーバージョン 81 から
        "polylines": int,
        # 書式つきの文字。マイナーバージョン 81 から
        "formatted_texts": int,
        # 寸法。マイナーバージョン 81 から
        "dimensions": int,
        # 引出線。マイナーバージョン 81 から
        "leaders": int,
        # 引出線つきの注記。マイナーバージョン 81 から
        "callouts": int,
        # 表。マイナーバージョン 81 から
        "tables": int,
        # アンダーレイ。マイナーバージョン 81 から
        "underlays": int,
        # 記号の配置。中の要素を展開しない。マイナーバージョン 81 から
        "symbols": int,
        # ハッチング。マイナーバージョン 81 から
        "hatches": int,
        # 点。マイナーバージョン 81 から
        "points": int,
    },
)


# 2D のビュー。変換行列と切り抜きを保ち、尺度・向き・見えるモデル範囲を決定的に求める
_ViewDetailRequired = TypedDict(
    "_ViewDetailRequired",
    {
        # 対象の実寸の空間の公開 ID
        "space": str,
        # モデル座標から用紙の mm への [a,b,c,d,e,f]。点を (ax+by+c,dx+ey+f) に写す
        "matrix": List[float],
        # モデルの X 軸・Y 軸の 1 単位に対応する用紙の長さ [SX,SY]（mm）。回転・せん断・反転でも各軸の正の長さを返す
        "scale": List[float],
        # 用紙の X 軸から見たモデルの X 軸の向き。度、反時計回り、-180〜180
        "rotation": float,
        # 変換が向きを反転する
        "mirrored": bool,
        # 用紙の mm での切り抜きの頂点。元の順を保ち、空なら切り抜かない
        "clip": List["Point2"],
        # 同じ頂点を逆変換したモデル座標。元の順を保つ
        "model_clip": List["Point2"],
        # ビューの出所
        "source": "Source",
    },
)
_ViewDetailOptional = TypedDict(
    "_ViewDetailOptional",
    {
        # モデル単位を mm に直した、各軸の縮尺の比 [SX,SY]。1:100 なら 0.01。単位不明なら持たない
        "ratio": List[float],
        # model_clip を囲む範囲。切り抜きが空なら持たない。モデルの単位
        "extent": "Extent",
    },
    total=False,
)


class ViewDetail(_ViewDetailRequired, _ViewDetailOptional):
    "2D のビュー。変換行列と切り抜きを保ち、尺度・向き・見えるモデル範囲を決定的に求める"


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
        # メッセージのスキーマのメジャーバージョン。今は 1
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # すべての条件で絞った後の、返す最初の要素の位置。前の応答の next_offset を渡す。書かなければ 0
        "offset": int,
        # 返す要素の数の上限。書かなければ 1000
        "limit": int,
        # 返すフィールドの名前。1 つ以上で、重ねない。id は書かなくても返す。書かなければ id・kind・layer・source を返す（決定 0118）
        "fields": List["EntityField"],
        # [最小の X, 最小の Y, 最大の X, 最大の Y]。範囲がこの長方形と重なるか接する要素だけを返す。空間か用紙それぞれの座標で比べる（決定 0118）
        "region": List[float],
        # 空間か用紙の ID（space/0・sheet/h22 など）。書けば、その空間か用紙の要素だけを返す。記号の定義の空間（space/h1F など。BLOCK のハンドルから作る、決定 0156）も指せ、書かなければ定義の中の要素は返さない。ない ID なら要素を返さない。マイナーバージョン 31 から（決定 0177）
        "place": str,
        # 要素の公開 ID の一覧。いずれかに一致する要素だけを返す。空なら要素を返さず、ない ID は無視する。重複しても要素を増やさない。ほかの条件とも合わせる。マイナーバージョン 80 から
        "ids": List[str],
        # tables.layers の公開 ID の一覧。名前ではない。いずれかのレイヤの要素だけを返す。空なら要素を返さず、ない ID は無視する。ほかの条件とも合わせる。マイナーバージョン 80 から
        "layers": List[str],
        # 要素の種類の一覧。いずれかの種類だけを返す。空なら要素を返さず、重複しても要素を増やさない。ほかの条件とも合わせる。マイナーバージョン 80 から
        "kinds": List["EntityKind"],
        # 出所の条件。指定したすべての項目を満たす要素だけを返す。ほかの条件とも合わせる。マイナーバージョン 80 から
        "source": "EntitySourceFilter",
    },
    total=False,
)


class EntitiesRequest(_EntitiesRequestRequired, _EntitiesRequestOptional):
    "開いた文書を図面モデル（L2、決定 0022）に変換し、要素の一覧を返す。実寸の空間、用紙の順に、それぞれの要素を入力の順に並べる（決定 0117）"


# 元の要素の出所による条件。空のオブジェクトなら制限しない。出所を持たない要素は指定した条件を満たさない
EntitySourceFilter = TypedDict(
    "EntitySourceFilter",
    {
        # L1 のレコード番号と完全に一致する
        "record": int,
        # L1 のバイト範囲との重なり。start を含み end を含まない、空でない範囲。境界が接するだけなら一致しない
        "bytes": "ByteRange",
        # AutoCAD のハンドル。接頭辞なし、1〜16 桁の 16 進の文字列。大文字と小文字を区別せず、数値で比べる。instance とは同時に指定できない
        "handle": str,
        # SXF のインスタンス番号。1〜20 桁の符号なし 64 bit 整数の 10 進文字列。数値で比べる。handle とは同時に指定できない
        "instance": str,
    },
    total=False,
)


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
        # 空間か用紙と、出所で作る ID（space/0/h2F、space/0/r12、sheet/1/b4096 など）。出所の部分は、形式が持つキー（ハンドル）があればキーで作り、書き戻しても前の要素が増えても変わらない。同じバイト列には、どのバージョンでも同じ ID を返す（決定 0117・0156）
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
        # 形の値。線分・円・円弧・楕円・幅を持たない折れ線・1 点か 2 点の間に置く文字・書式つきの文字・ブロックの記号・制御点かフィット点で決まるスプライン・寸法・引出線（LEADER）・点・引出線つきの注記・表（ACAD_TABLE）だけが持つ。apply の replace と add に渡せる形（決定 0139）。マイナーバージョン 10 から
        "geometry": "Geometry",
        # 要素自身のスタイル。fields に style を指定したときだけ返す。マイナーバージョン 79 から
        "style": "EntityStyle",
        # 要素に付く拡張の一覧。fields に extensions を指定したときだけ返し、拡張がなければ空の一覧。マイナーバージョン 79 から
        "extensions": List["EntityExtension"],
    },
    total=False,
)


class Entity(_EntityRequired, _EntityOptional):
    "図面モデルの要素"

# entities で返す要素のフィールド（決定 0118）
EntityField = Union[
    Literal[
        "id",
        "kind",
        "layer",
        "source",
        "bbox",
        "hash",
        "geometry",
        "style",
        "extensions",
    ],
    str,
]


# 要素自身が持つスタイル。継承は解かず、色表の RGB も補わない。色・線種・線幅の形は restyle と同じ。透過性は読むだけで、restyle の指定には含めない
_EntityStyleRequired = TypedDict(
    "_EntityStyleRequired",
    {
        # 色の符号・図面内 RGB、または継承元
        "color": "ColorChange",
        # 継承元か、tables が返す線種の公開 ID
        "line_type": "LineTypeChange",
        # 要素自身の線種の倍率。文書や配置の倍率を掛けない
        "line_type_scale": float,
        # 透過性の指定
        "transparency": "EntityTransparency",
    },
)
_EntityStyleOptional = TypedDict(
    "_EntityStyleOptional",
    {
        # 線幅の指定。元の形式から読めていなければ持たない
        "line_weight": "LineWeightChange",
    },
    total=False,
)


class EntityStyle(_EntityStyleRequired, _EntityStyleOptional):
    "要素自身が持つスタイル。継承は解かず、色表の RGB も補わない。色・線種・線幅の形は restyle と同じ。透過性は読むだけで、restyle の指定には含めない"


# 透過性。inherit または alpha の一方を返す
EntityTransparency = TypedDict(
    "EntityTransparency",
    {
        # 継承元
        "inherit": "Inherit",
        # 不透明度。0 は透明、255 は不透明
        "alpha": int,
    },
    total=False,
)


# 要素に付く形式固有の情報。元の順序を保ち、失効した値も current を偽にして返す。書き戻す操作ではない
_EntityExtensionRequired = TypedDict(
    "_EntityExtensionRequired",
    {
        # 値の型を定める名前空間
        "namespace": str,
        # 元の名前。翻訳しない
        "name": str,
        # 名前空間の値の型のバージョン
        "schema": int,
        # basis が現在の要素のハッシュと一致する
        "current": bool,
        # 元の順序と型を保つフィールド
        "fields": List["EntityExtensionField"],
        # 拡張の出所
        "source": "Source",
    },
)
_EntityExtensionOptional = TypedDict(
    "_EntityExtensionOptional",
    {
        # 元の形式のバージョン。不明なら持たない
        "version": str,
        # 拡張が依存する元の要素のハッシュ。要素に依存するハッシュがなければ持たない
        "basis": str,
    },
    total=False,
)


class EntityExtension(_EntityExtensionRequired, _EntityExtensionOptional):
    "要素に付く形式固有の情報。元の順序を保ち、失効した値も current を偽にして返す。書き戻す操作ではない"


# 拡張のフィールド。同じキーの重複と並びを保つ
_EntityExtensionFieldRequired = TypedDict(
    "_EntityExtensionFieldRequired",
    {
        # 元のキー
        "key": str,
        # 型付きの値
        "value": "EntityExtensionValue",
    },
)
_EntityExtensionFieldOptional = TypedDict(
    "_EntityExtensionFieldOptional",
    {
        # 元の形式の値の番号。番号がなければ持たない
        "code": int,
    },
    total=False,
)


class EntityExtensionField(_EntityExtensionFieldRequired, _EntityExtensionFieldOptional):
    "拡張のフィールド。同じキーの重複と並びを保つ"

# 拡張の値。整数・ハンドル・バイト列は文字列で正確に返し、実数と点は有限値だけを返す
EntityExtensionValue = Union[
    "EntityExtensionText",
    "EntityExtensionInteger",
    "EntityExtensionReal",
    "EntityExtensionPoint",
    "EntityExtensionHandle",
    "EntityExtensionBytes",
]


# 元の文字列
EntityExtensionText = TypedDict(
    "EntityExtensionText",
    {
        # 値の型
        "kind": Literal["text"],
        # 文字列
        "value": str,
    },
)


# 符号付き 64 bit 整数
EntityExtensionInteger = TypedDict(
    "EntityExtensionInteger",
    {
        # 値の型
        "kind": Literal["integer"],
        # 10 進の文字列。負数以外には符号を付けず、先頭の 0 を付けない
        "value": str,
    },
)


# 有限の実数
EntityExtensionReal = TypedDict(
    "EntityExtensionReal",
    {
        # 値の型
        "kind": Literal["real"],
        # 実数
        "value": float,
    },
)


# 3 次元の点やベクトル。単位と空間は名前空間が定める
EntityExtensionPoint = TypedDict(
    "EntityExtensionPoint",
    {
        # 値の型
        "kind": Literal["point"],
        # [X, Y, Z] の有限値
        "value": List[float],
    },
)


# 元の形式のオブジェクトを指す符号なし 64 bit の値
EntityExtensionHandle = TypedDict(
    "EntityExtensionHandle",
    {
        # 値の型
        "kind": Literal["handle"],
        # 大文字の 16 進の文字列。接頭辞と先頭の 0 を付けない
        "value": str,
    },
)


# 元のバイト列
EntityExtensionBytes = TypedDict(
    "EntityExtensionBytes",
    {
        # 値の型
        "kind": Literal["bytes"],
        # 1 バイトを小文字の 16 進 2 桁にした文字列。空も保つ
        "value": str,
    },
)

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
    "TableGeometry",
    "HatchGeometry",
]


# 非連動のハッチング。閉じた折れ線（膨らみ付き）と円を境界にできる。塗りは単色か、線の定義を明示した模様。参照・勾配・自動検出した境界は扱わない。DXF と DWG に追加できる。置き換え・変換はまだ扱わない。entities は用紙上の模様を返さない。用紙への模様の追加は、原点が 0 で 1 単位が 1 mm のときに限る。マイナーバージョン 75 から
_HatchGeometryRequired = TypedDict(
    "_HatchGeometryRequired",
    {
        # 形の種類
        "shape": Literal["hatch"],
        # 境界の輪。入力の順。1 つ以上
        "loops": List["HatchLoop"],
        # 輪の中の輪の埋め方
        "islands": "HatchIslands",
        # 中を埋めるもの
        "fill": "HatchFill",
    },
)
_HatchGeometryOptional = TypedDict(
    "_HatchGeometryOptional",
    {
        # fill が pattern のときだけ持つ模様の定義
        "pattern": "HatchPattern",
        # 境界を探した点 [[X, Y], ...]。書かなければ空
        "seeds": List[List[float]],
    },
    total=False,
)


class HatchGeometry(_HatchGeometryRequired, _HatchGeometryOptional):
    "非連動のハッチング。閉じた折れ線（膨らみ付き）と円を境界にできる。塗りは単色か、線の定義を明示した模様。参照・勾配・自動検出した境界は扱わない。DXF と DWG に追加できる。置き換え・変換はまだ扱わない。entities は用紙上の模様を返さない。用紙への模様の追加は、原点が 0 で 1 単位が 1 mm のときに限る。マイナーバージョン 75 から"


# ハッチングの境界の輪。参照を持たない
HatchLoop = TypedDict(
    "HatchLoop",
    {
        # 閉じた折れ線か円
        "boundary": "HatchBoundary",
        # 外側の輪
        "external": bool,
        # いちばん外の輪
        "outermost": bool,
    },
)

# 輪の形。折れ線は closed が真で頂点が 2 つ以上。最後の点から最初の点へ結ぶ
HatchBoundary = Union[
    "PolylineGeometry",
    "CircleGeometry",
]

# nested は外から奇数番目の内側、outer はいちばん外と次の輪の間、ignore は内側の輪を無視して埋める
HatchIslands = Union[
    Literal[
        "nested",
        "outer",
        "ignore",
    ],
    str,
]

# solid は要素の色で塗り、pattern は線の組で埋める
HatchFill = Union[
    Literal[
        "solid",
        "pattern",
    ],
    str,
]


# 模様の名前と線の定義。外部の定義ファイルを読まず、lines の値を使う
HatchPattern = TypedDict(
    "HatchPattern",
    {
        # 元の名前。空と制御文字を含むものは受けない
        "name": str,
        # 定義の出どころ
        "origin": "HatchPatternOrigin",
        # 定義に加えた回転（度）
        "angle": float,
        # 定義に掛けた尺度か、利用者が決めた線の間隔。0 より大きい
        "scale": float,
        # 線の組を 90 度回して重ねる
        "double": bool,
        # angle と scale を当てた後の線の組。1 つ以上
        "lines": List["HatchPatternLine"],
    },
)

# user_defined は利用者が決めた平行線、predefined は形式が名前で決めた定義、custom は利用者の定義
HatchPatternOrigin = Union[
    Literal[
        "user_defined",
        "predefined",
        "custom",
    ],
    str,
]


# 1 組の平行線。基点・ずれ・線の長さは要素の座標と単位に従う
HatchPatternLine = TypedDict(
    "HatchPatternLine",
    {
        # 線の向き（度）
        "angle": float,
        # 1 本の線が通る点 [X, Y]
        "base": List[float],
        # 隣の線までのずれ [X, Y]。両方 0 にはしない
        "offset": List[float],
        # 正は線、負は間、0 は点。空なら実線
        "dashes": List[float],
    },
)


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


# 楕円か楕円の弧。始まりの角度から終わりの角度へ、反時計回りにたどる。閉じた楕円は 0 度から 360 度。マイナーバージョン 11 から
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


# 1 点か 2 点の間に置く文字。置き換えるときは、位置合わせを元の値のままにし、書かない幅の倍率・斜体・反転も元の値を受け継ぐ。足すときは、2 点目がなければ書かない位置合わせを左の基準線にし、幅の倍率 1・斜体なし・反転なしにする
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
        # 2 点の間に合わせる文字の 2 点目 [X, Y]。合わせ方（fit）と一緒に書く。置き換えでは、1 点に置く文字と 2 点の間に合わせる文字を入れ替えられない。マイナーバージョン 17 から
        "end": List[float],
        # 2 点の間への合わせ方。2 点目（end）と一緒に書く。マイナーバージョン 17 から
        "fit": "TextFit",
        # 1 点に置く文字の水平の位置合わせ。left のときは entities が省き、足すときも省けば left。2 点の間に合わせる文字には書けない。置き換えるときは元と同じ値だけを受ける。マイナーバージョン 69 から
        "horizontal": "TextHorizontal",
        # 1 点に置く文字の垂直の位置合わせ。baseline のときは entities が省き、足すときも省けば baseline。2 点の間に合わせる文字には書けない。置き換えるときは元と同じ値だけを受ける。マイナーバージョン 69 から
        "vertical": "TextVertical",
        # 高さ（height）を用紙の mm で表すか。実寸の空間の文字が用紙の長さで高さを持つとき（JWW）、entities は真を返す。足す・置き換えるときに真を書けば、高さを用紙の mm として渡す。偽のときは entities が省き、省けば高さを点と同じ単位とする。マイナーバージョン 93 から
        "height_on_paper": bool,
        # 幅の倍率。0 より大きい。1 のときは entities が省き、足すときも省けば 1、置き換えるときは省けば元の値を受け継ぐ。マイナーバージョン 69 から
        "width_factor": float,
        # 斜体の角度（度）。0 のときは entities が省き、足すときも省けば 0、置き換えるときは省けば元の値を受け継ぐ。マイナーバージョン 69 から
        "oblique": float,
        # 左右を裏返すか。偽のときは entities が省き、足すときも省けば偽、置き換えるときは省けば元の値を受け継ぐ。マイナーバージョン 69 から
        "backward": bool,
        # 上下を反転するか。偽のときは entities が省き、足すときも省けば偽、置き換えるときは省けば元の値を受け継ぐ。マイナーバージョン 69 から
        "upside_down": bool,
        # 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。DXF に文字スタイルの定義がないときは Standard だけを受ける。置き換えるときは、省けば元の名前を保ち、DXF・DWG は定義のある別の文字スタイルも受ける。style_id と一緒に書くなら、同じ文字スタイルを指す。マイナーバージョン 12 から
        "style": str,
        # 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。マイナーバージョン 23 から
        "style_id": str,
    },
    total=False,
)


class TextGeometry(_TextGeometryRequired, _TextGeometryOptional):
    "1 点か 2 点の間に置く文字。置き換えるときは、位置合わせを元の値のままにし、書かない幅の倍率・斜体・反転も元の値を受け継ぐ。足すときは、2 点目がなければ書かない位置合わせを左の基準線にし、幅の倍率 1・斜体なし・反転なしにする"

# 1 点に置く文字の水平の位置合わせ。middle は水平と垂直の両方を外形の中央に合わせる（DXF の 72 = 4）
TextHorizontal = Union[
    Literal[
        "left",
        "center",
        "right",
        "middle",
    ],
    str,
]

# 1 点に置く文字の垂直の位置合わせ
TextVertical = Union[
    Literal[
        "baseline",
        "bottom",
        "middle",
        "top",
    ],
    str,
]

# 2 点の間への合わせ方。aligned は縦横の比を保って高さも変え、fit は高さを保って幅だけを変える
TextFit = Union[
    Literal[
        "aligned",
        "fit",
    ],
    str,
]


# 書式つきの文字（DXF の MTEXT）。置き換えるときは、枠の高さ・書き進める向き・行送り・背景・高さの意味を元の値のままにする。足すときは、枠の高さなし・左から右・行送り 1 倍・背景なしにする。マイナーバージョン 13 から
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
        # 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。置き換えるときは、省けば元の名前を保ち、DXF・DWG は定義のある別の文字スタイルも受ける。style_id と一緒に書くなら、同じ文字スタイルを指す
        "style": str,
        # 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。マイナーバージョン 23 から
        "style_id": str,
    },
    total=False,
)


class FormattedTextGeometry(_FormattedTextGeometryRequired, _FormattedTextGeometryOptional):
    "書式つきの文字（DXF の MTEXT）。置き換えるときは、枠の高さ・書き進める向き・行送り・背景・高さの意味を元の値のままにする。足すときは、枠の高さなし・左から右・行送り 1 倍・背景なしにする。マイナーバージョン 13 から"


# スプライン（NURBS）。制御点で決まるもの（マイナーバージョン 15 から）と、フィット点で決まるもの（マイナーバージョン 18 から）がある。フィット点で決まるものは、ノットと制御点を空にし、フィット点（fit）と、あれば両端の接線を持つ。元の図形に制御点やノットもあるときは形の値を返さず、置き換えない。置き換えでは、制御点で決まるものとフィット点で決まるものを入れ替えない
_SplineGeometryRequired = TypedDict(
    "_SplineGeometryRequired",
    {
        # 形の種類
        "shape": Literal["spline"],
        # 次数。1 以上
        "degree": int,
        # ノットの並び。周期的でなければ、制御点の数と次数と 1 の和の数。フィット点で決まるものは空
        "knots": List[float],
        # 制御点の [X, Y] の並び。次数より多い。フィット点で決まるものは空
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
        # フィット点の [X, Y] の並び。2 つ以上。書けば、ノットと制御点は空で、重みは書かない。マイナーバージョン 18 から
        "fit": List[List[float]],
        # フィット点で決まるスプラインの、始まりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。マイナーバージョン 18 から
        "start_tangent": List[float],
        # フィット点で決まるスプラインの、終わりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。マイナーバージョン 18 から
        "end_tangent": List[float],
    },
    total=False,
)


class SplineGeometry(_SplineGeometryRequired, _SplineGeometryOptional):
    "スプライン（NURBS）。制御点で決まるもの（マイナーバージョン 15 から）と、フィット点で決まるもの（マイナーバージョン 18 から）がある。フィット点で決まるものは、ノットと制御点を空にし、フィット点（fit）と、あれば両端の接線を持つ。元の図形に制御点やノットもあるときは形の値を返さず、置き換えない。置き換えでは、制御点で決まるものとフィット点で決まるものを入れ替えない"


# 寸法。種類（kind）ごとに、その種類の定義点のフィールドを持つ。測定値（measurement）は元の形式が保存した値で、定義点から測り直さない。寸法を描いた部品（ブロック）は形の値に含めない。置き換え（DXF と DWG の DIMENSION、DXF の ARC_DIMENSION）では定義点と文字を書き直し、測定値を測り直して表示の部品を作り直す（マイナーバージョン 74 から。それまでは置き換えも change.edit.unsupported だった）。measurement は省くか元と同じ値を書く。足すことは、まだしない。マイナーバージョン 19 から
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
        # 元の形式が保存した測定値。単位と倍率は measurement_key のフィールドに従う
        "measurement": float,
        # measurement の形式とフィールドを指すキー（DXF の DIMENSION の 42 なら dxf.DIMENSION.42）。measurement と一緒に持つ
        "measurement_key": str,
        # 寸法値の文字の中央の点 [X, Y]。元の形式が持たなければ書かない
        "text_position": List[float],
        # 寸法値の文字の回転（度）。寸法線に沿わせるなら書かない
        "text_rotation": float,
        # 寸法スタイルの ID。tables が返したもの。名前で引いた寸法スタイルがなければ書かない。置き換えるときは、名前と同じ寸法スタイルを指す ID だけを受ける。マイナーバージョン 23 から
        "style_id": str,
    },
    total=False,
)


class DimensionGeometry(_DimensionGeometryRequired, _DimensionGeometryOptional):
    "寸法。種類（kind）ごとに、その種類の定義点のフィールドを持つ。測定値（measurement）は元の形式が保存した値で、定義点から測り直さない。寸法を描いた部品（ブロック）は形の値に含めない。置き換え（DXF と DWG の DIMENSION、DXF の ARC_DIMENSION）では定義点と文字を書き直し、測定値を測り直して表示の部品を作り直す（マイナーバージョン 74 から。それまでは置き換えも change.edit.unsupported だった）。measurement は省くか元と同じ値を書く。足すことは、まだしない。マイナーバージョン 19 から"


# 引出線（DXF・DWG の LEADER、決定 0177）。注記（MTEXT など）は別の要素で、annotation がその ID を指す。置き換えるときは寸法スタイルと注記を変えず、経路・曲線か・矢印を書き直す。足すときの注記は、同じ空間か用紙の MTEXT だけを指せる（マイナーバージョン 36 から。それまでは注記を持てなかった）。DWG は置き換えと追加を書き戻すが、矢印のフラグを必ず持つので、arrow を書かない引出線は書けない（マイナーバージョン 36 から。それまでは R2004 までの置き換えだけ）。マイナーバージョン 31 から
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
    "引出線（DXF・DWG の LEADER、決定 0177）。注記（MTEXT など）は別の要素で、annotation がその ID を指す。置き換えるときは寸法スタイルと注記を変えず、経路・曲線か・矢印を書き直す。足すときの注記は、同じ空間か用紙の MTEXT だけを指せる（マイナーバージョン 36 から。それまでは注記を持てなかった）。DWG は置き換えと追加を書き戻すが、矢印のフラグを必ず持つので、arrow を書かない引出線は書けない（マイナーバージョン 36 から。それまでは R2004 までの置き換えだけ）。マイナーバージョン 31 から"


# 引出線つきの注記（DXF・DWG の MULTILEADER、決定 0185）。注記は形の中にあり、content と text・symbol で種類と中身を示す。置き換えるときは枝の線の頂点・集まる点・折れ曲がりだけを変えられ、ほかのフィールドは元の値のままにする。DXF だけが置き換えを書き戻し、枝と線の数と、集まる点と折れ曲がりの有無は元と同じでなければならない。足すことと DWG の書き戻しは、まだしない。マイナーバージョン 36 から
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
    "引出線つきの注記（DXF・DWG の MULTILEADER、決定 0185）。注記は形の中にあり、content と text・symbol で種類と中身を示す。置き換えるときは枝の線の頂点・集まる点・折れ曲がりだけを変えられ、ほかのフィールドは元の値のままにする。DXF だけが置き換えを書き戻し、枝と線の数と、集まる点と折れ曲がりの有無は元と同じでなければならない。足すことと DWG の書き戻しは、まだしない。マイナーバージョン 36 から"


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


# 表（DXF・DWG の ACAD_TABLE、決定 0113）。挿入点・行の向き・行の高さ・列の幅と、行ごとのセルの並びを持つ。表のスタイル・セルの書式・型のある値・表示の部品（ブロック）は形の値に含めない。置き換え（DXF の ACAD_TABLE）では、行と列の数・セルの結合・文字でないセル（記号）を変えず、挿入点・行の向き・行の高さ・列の幅・文字と空のセルの内容と回転を書き直し、表示の部品を作り直す。足すことと DWG の書き戻しは、まだしない。マイナーバージョン 74 から
TableGeometry = TypedDict(
    "TableGeometry",
    {
        # 形の種類
        "shape": Literal["table"],
        # 挿入点 [X, Y]
        "position": List[float],
        # 行の向き [X, Y]。0 でない
        "direction": List[float],
        # 上の行からの行の高さ。どれも 0 より大きい。1 つ以上
        "row_heights": List[float],
        # 左の列からの列の幅。どれも 0 より大きい。1 つ以上
        "column_widths": List[float],
        # 上の行から、行ごとに左の列から並べたセル。数は行の数と列の数の積
        "cells": List["TableCell"],
    },
)


# 表の 1 つのセル
_TableCellRequired = TypedDict(
    "_TableCellRequired",
    {
        # 中身の種類
        "content": "TableCellContent",
    },
)
_TableCellOptional = TypedDict(
    "_TableCellOptional",
    {
        # 書式の符号を含む元の文字列。空でない。content が text のときだけ持つ
        "text": str,
        # 記号のブロックの名前。content が symbol のときだけ持つ
        "symbol": str,
        # 結合して広げる列と行の数 [列, 行]。結合しなければ [1, 1]（省けば [1, 1]）
        "span": List[int],
        # ほかのセルに結合されて覆われるか。偽のときは省ける
        "covered": bool,
        # 文字か記号の回転。0 のときは省ける
        "rotation": float,
    },
    total=False,
)


class TableCell(_TableCellRequired, _TableCellOptional):
    "表の 1 つのセル"

# 表のセルの中身の種類。empty は空、text は書式つきの文字、symbol は記号（ブロック）
TableCellContent = Union[
    Literal[
        "empty",
        "text",
        "symbol",
    ],
    str,
]


# 点（決定 0160・0183）。marker がなければ文書の点の表し方（DXF・DWG の $PDMODE・$PDSIZE）に従う点で、DXF・DWG の POINT と JWW の記号でない点が当たる。marker があれば既定義のマーカーの点で、SFC の点マーカーと JWW の記号の点が当たる。置き換えるときは、描き方（marker があるか）を元のまま保つ。DXF・DWG は marker と construction を持つ点を書かず、JWW の記号でない点は角度 0 だけを、SXF は SFC の点マーカーだけを書く。マイナーバージョン 35 から
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
        # 既定義のマーカー。書かなければ文書の点の表し方に従う
        "marker": "PointMarker",
        # 作図の補助の点（JWW の仮点）か。書かなければ偽
        "construction": bool,
    },
    total=False,
)


class PointGeometry(_PointGeometryRequired, _PointGeometryOptional):
    "点（決定 0160・0183）。marker がなければ文書の点の表し方（DXF・DWG の $PDMODE・$PDSIZE）に従う点で、DXF・DWG の POINT と JWW の記号でない点が当たる。marker があれば既定義のマーカーの点で、SFC の点マーカーと JWW の記号の点が当たる。置き換えるときは、描き方（marker があるか）を元のまま保つ。DXF・DWG は marker と construction を持つ点を書かず、JWW の記号でない点は角度 0 だけを、SXF は SFC の点マーカーだけを書く。マイナーバージョン 35 から"


# 既定義のマーカーの表のキーと尺度。マーカーの形は、表示する側がキーから引く
PointMarker = TypedDict(
    "PointMarker",
    {
        # マーカーの表の名前空間。sxf.marker（SFC の点マーカー）か jww.point（JWW の記号の点）
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


# 記号（DXF・DWG のブロックの INSERT）。定義の点を、尺度・回転・移動の順に写す。切り抜きは形の値に含めない。置き換えるときは切り抜きを元のままにし、属性は値だけを変えられる。属性を持つ記号の置く点・尺度・回転は変えない（属性ごと動かすなら transform を使う）。足すときは切り抜きを持たない。マイナーバージョン 14 から
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
        # 属性のタグと値。entities は記号の属性を入力の順に返す。足すときは、定義の属性の定義（値が決まったものを除く）から属性を作り、ここに書いたタグの値で既定の値を替える。タグは大文字と小文字を区別せず、重複を受けない。必要な属性を作れなければ失敗する。置き換えるときは、元と同じ数・順・タグを受け、値を変えた属性の ATTRIB の値を書き直す（表題欄の項目の書き換えなど）。書式つきの値（複数行の属性）を持つ属性の値は変えられない。マイナーバージョン 16 から。置き換えで値を変えるのはマイナーバージョン 21 から
        "attributes": List["SymbolAttribute"],
    },
    total=False,
)


class SymbolGeometry(_SymbolGeometryRequired, _SymbolGeometryOptional):
    "記号（DXF・DWG のブロックの INSERT）。定義の点を、尺度・回転・移動の順に写す。切り抜きは形の値に含めない。置き換えるときは切り抜きを元のままにし、属性は値だけを変えられる。属性を持つ記号の置く点・尺度・回転は変えない（属性ごと動かすなら transform を使う）。足すときは切り抜きを持たない。マイナーバージョン 14 から"


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

# 要素の形の種類（決定 0022）。point（点、決定 0160）はマイナーバージョン 32 から。それまで点は symbol だった
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


# 開いた文書に、変更の一覧を当てて書き戻す（決定 0139・0156）。要素は、entities が返す ID とハッシュで指す。どれか 1 つでも当てられなければ、何も変えない。当てたら、文書を書き戻したものに替え、書き戻したファイルを blobs で返す。マイナーバージョン 7 から
_ApplyRequestRequired = TypedDict(
    "_ApplyRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 読んだときの文書のハッシュ（drawing の hash）。書けば、今の文書と同じときだけ当てる
        "hash": str,
    },
    total=False,
)


class ApplyRequest(_ApplyRequestRequired, _ApplyRequestOptional):
    "開いた文書に、変更の一覧を当てて書き戻す（決定 0139・0156）。要素は、entities が返す ID とハッシュで指す。どれか 1 つでも当てられなければ、何も変えない。当てたら、文書を書き戻したものに替え、書き戻したファイルを blobs で返す。マイナーバージョン 7 から"


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
        # true なら、変えていない要素の ID がずれた。キーを持たない形式（JWW）か、ハンドルを持たない DXF（R12 など）で、前の図形か表のレコードを除くか足したとき。ID を使う前に entities で読み直す。ずれないときは持たない。マイナーバージョン 30 から
        "ids_shifted": bool,
    },
    total=False,
)


class ApplyResult(_ApplyResultRequired, _ApplyResultOptional):
    "当てた変更の結果。書き戻したファイルは blobs の file で返す"


# 入力なしで空の文書を作り、開く。dxf は R2004 の DXF で、既定のレイヤとスタイル、空のモデル空間と Layout1 を持つ。ifc は IFC のひな形で、プロジェクト・長さとラジアンの単位・3 次元の Model の文脈と Body の下位の文脈を持つ（#395）。書いたファイルは blobs で返す。マイナーバージョン 83 から。ifc はマイナーバージョン 109 から
_CreateRequestRequired = TypedDict(
    "_CreateRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["create"],
        # 作る形式の名前。dxf か ifc
        "format": str,
        # 作る文書の名前。既存の文書と重ねない
        "to": str,
    },
)
_CreateRequestOptional = TypedDict(
    "_CreateRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 実寸の空間の長さの単位。省略すると millimeter。unknown は単位なしで、ifc では書けない
        "unit": "Unit",
        # IFC を書く設定。format が ifc のときに必須で、ほかの形式では書けない。マイナーバージョン 109 から
        "ifc": "IfcOutput",
    },
    total=False,
)


class CreateRequest(_CreateRequestRequired, _CreateRequestOptional):
    "入力なしで空の文書を作り、開く。dxf は R2004 の DXF で、既定のレイヤとスタイル、空のモデル空間と Layout1 を持つ。ifc は IFC のひな形で、プロジェクト・長さとラジアンの単位・3 次元の Model の文脈と Body の下位の文脈を持つ（#395）。書いたファイルは blobs で返す。マイナーバージョン 83 から。ifc はマイナーバージョン 109 から"


# 作った空の文書。書いたファイルは blobs の file で返す
CreateResult = TypedDict(
    "CreateResult",
    {
        # 作った文書の名前
        "doc": str,
        # 書いた形式の名前
        "format": str,
        # 書いたファイルのハッシュ。apply の前提条件に使う
        "hash": str,
        # 書いた要素の数。空なので 0
        "elements": int,
    },
)


# 開いた文書から別の形式のファイルを作り、新しい文書として開く。図面からは DXF（対応する R2004 DWG の弧長寸法は共有オブジェクト表から、それ以外は図面モデルから。決定 0139・0206・0281）または SIMA（.sim）、土木モデル（J-LandXML・IFC4.3・SIMA）からは SIMA（.sim）、地理の領域モデルの文書（CityGML・基盤地図情報・拡張 DM・GeoJSON・SIMA・GeoTIFF）からは GeoJSON または SIMA（.sim）を書く。建物モデルの文書（IFC・ST-Bridge）からは、ifc で指定したバージョンの新しい IFC を書く（#395）。書けないデータは種類ごとの件数を診断で返す。書いたファイルは blobs で返す。マイナーバージョン 41 から。geojson はマイナーバージョン 71 から。sima はマイナーバージョン 72 から。ifc はマイナーバージョン 109 から
_ConvertRequestRequired = TypedDict(
    "_ConvertRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["convert"],
        # 書く元の文書の名前。open で作ったもの
        "doc": str,
        # 書く形式の名前（capabilities の formats の id）。dxf・sxf（P21）・jww・geojson・sima・ifc のいずれか。sxf・jww はマイナーバージョン 85 から
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # IFC を書く設定。format が ifc のときに必須で、ほかの形式では書けない。マイナーバージョン 109 から
        "ifc": "IfcOutput",
    },
    total=False,
)


class ConvertRequest(_ConvertRequestRequired, _ConvertRequestOptional):
    "開いた文書から別の形式のファイルを作り、新しい文書として開く。図面からは DXF（対応する R2004 DWG の弧長寸法は共有オブジェクト表から、それ以外は図面モデルから。決定 0139・0206・0281）または SIMA（.sim）、土木モデル（J-LandXML・IFC4.3・SIMA）からは SIMA（.sim）、地理の領域モデルの文書（CityGML・基盤地図情報・拡張 DM・GeoJSON・SIMA・GeoTIFF）からは GeoJSON または SIMA（.sim）を書く。建物モデルの文書（IFC・ST-Bridge）からは、ifc で指定したバージョンの新しい IFC を書く（#395）。書けないデータは種類ごとの件数を診断で返す。書いたファイルは blobs で返す。マイナーバージョン 41 から。geojson はマイナーバージョン 71 から。sima はマイナーバージョン 72 から。ifc はマイナーバージョン 109 から"


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


# 開いた文書を変換したときの忠実度と欠落を、convert と同じ書き手で見積もる。文書もファイルの blob も登録しない。図面から DXF・SXF（P21）・JWW、建物モデルの文書から IFC への変換に対応する。マイナーバージョン 66 から（SXF・JWW は 85 から、IFC は 109 から）
_ConvertEstimateRequestRequired = TypedDict(
    "_ConvertEstimateRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["convert_estimate"],
        # 書く元の文書の名前
        "doc": str,
        # 書く形式の名前。dxf・sxf・jww・ifc
        "format": str,
        # 予定する文書の名前。convert と同じ条件で検査するが、名前を確保しない
        "to": str,
    },
)
_ConvertEstimateRequestOptional = TypedDict(
    "_ConvertEstimateRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # IFC を書く設定。format が ifc のときに必須で、ほかの形式では書けない。マイナーバージョン 109 から
        "ifc": "IfcOutput",
        # 書き手と読み取り段階のそれぞれで返す明細の最大件数。省略時は 100。総数と全件集計はこの上限によらない
        "limit": int,
    },
    total=False,
)


class ConvertEstimateRequest(_ConvertEstimateRequestRequired, _ConvertEstimateRequestOptional):
    "開いた文書を変換したときの忠実度と欠落を、convert と同じ書き手で見積もる。文書もファイルの blob も登録しない。図面から DXF・SXF（P21）・JWW、建物モデルの文書から IFC への変換に対応する。マイナーバージョン 66 から（SXF・JWW は 85 から、IFC は 109 から）"


# 新しい IFC を書く設定（#395）。コアは時計を読まないので、時刻は呼び出し元が渡す
_IfcOutputRequired = TypedDict(
    "_IfcOutputRequired",
    {
        # 書くバージョン
        "version": "IfcVersion",
    },
)
_IfcOutputOptional = TypedDict(
    "_IfcOutputOptional",
    {
        # ヘッダー（FILE_DESCRIPTION と FILE_NAME）の値。書かなければ空の値で書く
        "header": "IfcHeader",
        # 所有履歴（IfcOwnerHistory）の値。ifc2x3 では必須。ifc4 からは書いたときだけ書く
        "owner": "IfcOwner",
        # GlobalId を導く名前空間。書かなければ、convert は元の IfcProject の GlobalId（なければ元の文書のハッシュ）、create は作る文書の名前を使う。元の文書のハッシュでない名前空間なら、元を一部直して書き直しても、直していない要素の GlobalId は変わらない
        "namespace": str,
    },
    total=False,
)


class IfcOutput(_IfcOutputRequired, _IfcOutputOptional):
    "新しい IFC を書く設定（#395）。コアは時計を読まないので、時刻は呼び出し元が渡す"

# IFC のバージョン。ifc2x3 は IFC2X3 TC1、ifc4 は IFC4 ADD2 TC1、ifc4x3 は IFC4X3 ADD2
IfcVersion = Union[
    Literal[
        "ifc2x3",
        "ifc4",
        "ifc4x3",
    ],
    str,
]


# IFC のヘッダーの値。書かない値は空で書く
IfcHeader = TypedDict(
    "IfcHeader",
    {
        # FILE_DESCRIPTION に ViewDefinition [<値>] として書く MVD の名前。書かなければ書かない
        "view_definition": str,
        # FILE_NAME の name
        "name": str,
        # FILE_NAME の time_stamp。呼び出し元が書いた時刻
        "time_stamp": str,
        # FILE_NAME の author
        "author": List[str],
        # FILE_NAME の organization
        "organization": List[str],
        # FILE_NAME の preprocessor_version
        "preprocessor_version": str,
        # FILE_NAME の originating_system
        "originating_system": str,
        # FILE_NAME の authorization
        "authorization": str,
    },
    total=False,
)


# 所有履歴の値
IfcOwner = TypedDict(
    "IfcOwner",
    {
        # 所有する人の IfcPerson の FamilyName
        "person": str,
        # 所有する組織の IfcOrganization の Name
        "organization": str,
        # 作ったソフトの開発元の IfcOrganization の Name
        "developer": str,
        # 作ったソフトの名前（IfcApplication の ApplicationFullName）
        "application": str,
        # 作ったソフトのバージョン（IfcApplication の Version）
        "version": str,
        # 作ったソフトの識別子（IfcApplication の ApplicationIdentifier）
        "identifier": str,
        # 作った日時。1970-01-01T00:00:00Z からの秒（IfcTimeStamp）
        "created": int,
    },
)


# 予定する変換の結果。元の文書・ID を保ち、ファイルの blob を返さない
ConvertEstimateResult = TypedDict(
    "ConvertEstimateResult",
    {
        # 予定する出力と忠実度の全件集計・上限付き明細
        "estimate": "ConversionEstimate",
    },
)


# 実変換と同じ処理で準備した出力。doc と hash は未登録の予定値。元の入力・設定・予算が同じなら convert の値と一致する
_ConversionEstimateRequired = TypedDict(
    "_ConversionEstimateRequired",
    {
        # 予定する文書の名前
        "doc": str,
        # 予定する形式の名前
        "format": str,
        # 予定する出力バイト列のハッシュ。未登録なので apply の前提条件には使えない
        "hash": str,
        # 予定する出力の要素数。記号から離す属性の文字を含む
        "elements": int,
        # 処理対象の出現数。同じ出所を持つ別の入力は別々に数え、診断の数では増やさない
        "total": int,
        # 種類と忠実度ごとの全件集計
        "counts": List["ConversionCount"],
        # 処理順の明細。limit 件まで返す
        "details": List["ConversionDetail"],
        # 明細の上限で返さなかった対象がある
        "truncated": bool,
    },
)
_ConversionEstimateOptional = TypedDict(
    "_ConversionEstimateOptional",
    {
        # 入力の表の読み取りと lift 中の、集約前の診断発生記録。書き手の項目の集計には含めない。マイナーバージョン 67 から
        "input_diagnostics": "ConversionInputDiagnostics",
    },
    total=False,
)


class ConversionEstimate(_ConversionEstimateRequired, _ConversionEstimateOptional):
    "実変換と同じ処理で準備した出力。doc と hash は未登録の予定値。元の入力・設定・予算が同じなら convert の値と一致する"


# 読み取り段階の診断発生数。失った要素数ではなく、同じ要素の複数の診断もそれぞれ数える。忠実度を持たない診断も含む。open 時や書き手の診断は含まない
ConversionInputDiagnostics = TypedDict(
    "ConversionInputDiagnostics",
    {
        # 診断の発生件数の合計。max_places と limit によらない
        "total": int,
        # 診断の code とカタログの group_by の値による全件集計。場所は持たず、値は最も早い入力位置の報告から取る
        "counts": List["Diagnostic"],
        # 集約前の報告を入力位置順に limit 件まで返す。同じ位置は報告順。診断の場所は max_places で制限しない
        "details": List["ConversionInputDiagnostic"],
        # 明細の上限で返さなかった報告がある
        "truncated": bool,
    },
)


# 集約前の診断と、その場所と同じ順序の出所。種類や失った値は診断の code と params からたどる
ConversionInputDiagnostic = TypedDict(
    "ConversionInputDiagnostic",
    {
        # 集約前の診断。元の値と場所を保つ
        "diagnostic": "Diagnostic",
        # diagnostic.at と同じ順序のレコード・バイト範囲・ハンドル・インスタンス番号。ID は diagnostic.at にある
        "sources": List["Source"],
    },
)


# 処理対象の種類と忠実度ごとの集計
ConversionCount = TypedDict(
    "ConversionCount",
    {
        # 処理対象の種類のキー
        "kind": str,
        # その対象を移した結果の忠実度
        "fidelity": "ConversionFidelity",
        # 入力の出現数。1 つの入力が複数の出力になっても 1 件
        "count": int,
        # この対象から書いた図形の数。表や参照だけを移した場合は 0
        "output_elements": int,
    },
)


# 処理対象の出現 1 つ。複数の欠落の診断が出ても 1 件として数える
_ConversionDetailRequired = TypedDict(
    "_ConversionDetailRequired",
    {
        # 処理対象の種類のキー
        "kind": str,
        # その対象を移した結果の忠実度
        "fidelity": "ConversionFidelity",
        # 値や参照を変えた理由のキー。text_height_meaning は文字高の意味、relocated_shape は形の値、style_default はスタイルの既定値、detached_attribute は属性の文字への分離、baked_view はビューの焼き込み、reference_dropped は参照の欠落
        "reasons": List[str],
        # 元の対象のレコード・バイト範囲・キー
        "source": "Source",
        # この対象から書いた図形の数。表や参照だけを移した場合は 0
        "output_elements": int,
    },
)
_ConversionDetailOptional = TypedDict(
    "_ConversionDetailOptional",
    {
        # 元の要素の ID。持つ対象だけで返す
        "id": str,
    },
    total=False,
)


class ConversionDetail(_ConversionDetailRequired, _ConversionDetailOptional):
    "処理対象の出現 1 つ。複数の欠落の診断が出ても 1 件として数える"

# 忠実度。`native` は元の値・参照を保持、`equivalent` は別の表現で同じ意味、`approximated` は近似、`drawn` は見た目だけ、`dropped` は欠落
ConversionFidelity = Union[
    Literal[
        "native",
        "equivalent",
        "approximated",
        "drawn",
        "dropped",
    ],
    str,
]


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
    "CopyEdit",
    "RemoveEdit",
    "ReplaceEdit",
    "AddEdit",
    "LayerEdit",
    "AddLayerEdit",
    "AddSymbolEdit",
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


# 要素のレイヤ・色・線種・線種の倍率・線幅のうち、書いたものだけを変える。形は変えない。RGB の色は、DXF と DWG の R2004 から、線幅は R2000 から書ける（DWG の今ある図形のレイヤ・線種・線幅・RGB の色は、決定 0293 から変えられる）
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
        # レイヤの ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。マイナーバージョン 8 から
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
    "要素のレイヤ・色・線種・線種の倍率・線幅のうち、書いたものだけを変える。形は変えない。RGB の色は、DXF と DWG の R2004 から、線幅は R2000 から書ける（DWG の今ある図形のレイヤ・線種・線幅・RGB の色は、決定 0293 から変えられる）"


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


# 要素を複写し、写しをアフィン変換で動かす。元の要素は変えない。写しは、元のレイヤ・色・線種・線種の倍率・線幅・透過性と形式固有の値（AutoCAD の系統の XDATA、JWW のフラグ・文字種・字間、SXF の文字幅・字間など）を保ち、図形とともに動く XDATA と記号の属性も同じ変換で直す。DXF の寸法・表は新しい匿名ブロックを作る。place で配置先の空間・用紙を、from_doc で複写元の文書を指定でき（マイナーバージョン 110 から）、単位・画層・線種・文字スタイル・寸法スタイル・記号定義・APPID の対応と名前の衝突を確かめる。保てない値・参照・対応を持つ要素は change.copy.unsupported で全体を拒む。applied は写しの ID とハッシュを返す。マイナーバージョン 100 から
_CopyEditRequired = TypedDict(
    "_CopyEditRequired",
    {
        # 変更の種類
        "kind": Literal["copy"],
        # 複写する要素の ID。entities が返したもの
        "id": str,
        # 読んだときの要素のハッシュ。entities が返したもの
        "hash": str,
        # [a, b, c, d, e, f]。写しの点 (x, y) を (a x + b y + c, d x + e y + f) に写す。元の位置に写すなら [1, 0, 0, 0, 1, 0]
        "matrix": List[float],
    },
)
_CopyEditOptional = TypedDict(
    "_CopyEditOptional",
    {
        # 配置先の空間か用紙の ID。省けば元の要素と同じ場所 ID。マイナーバージョン 110 から
        "place": str,
        # 複写元の文書の名前。open で作ったもの。省けば doc と同じ文書。マイナーバージョン 110 から
        "from_doc": str,
    },
    total=False,
)


class CopyEdit(_CopyEditRequired, _CopyEditOptional):
    "要素を複写し、写しをアフィン変換で動かす。元の要素は変えない。写しは、元のレイヤ・色・線種・線種の倍率・線幅・透過性と形式固有の値（AutoCAD の系統の XDATA、JWW のフラグ・文字種・字間、SXF の文字幅・字間など）を保ち、図形とともに動く XDATA と記号の属性も同じ変換で直す。DXF の寸法・表は新しい匿名ブロックを作る。place で配置先の空間・用紙を、from_doc で複写元の文書を指定でき（マイナーバージョン 110 から）、単位・画層・線種・文字スタイル・寸法スタイル・記号定義・APPID の対応と名前の衝突を確かめる。保てない値・参照・対応を持つ要素は change.copy.unsupported で全体を拒む。applied は写しの ID とハッシュを返す。マイナーバージョン 100 から"


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


# 要素の形を、渡す値で置き換える。形の種類は変えない。レイヤと線のスタイルは変えない。DXF・DWG の文字と書式つきの文字は、定義のある文字スタイルへ指し替えられる。マイナーバージョン 10 から
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


# 空間か用紙に要素を足す。書かない色・線種・線幅はレイヤに従う。JWW と SXF の図形は色・線種・線幅を自分で持ち、レイヤかブロックに従う値を持たないので、JWW は色と線種を書き（点は色だけ）、SXF は色・線種・線幅を書く（点マーカーは色だけ）。書かないか、レイヤかブロックに従う値なら change.style.unsupported で断る。JWW の書かないかレイヤに従う線幅は、ペンの印刷の線幅に従う。足せる形は、形の値のどの種類も。マイナーバージョン 10 から
_AddEditRequired = TypedDict(
    "_AddEditRequired",
    {
        # 変更の種類
        "kind": Literal["add"],
        # 足す先の空間か用紙の ID（space/0・sheet/h22 など）。entities の ID の先頭の部分。マイナーバージョン 31 から、記号の定義の空間（space/h1F など）も指せる（決定 0177）
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
        # 色。SXF と JWW はレイヤに従う色を書けないので、書く。SXF は、sxf の番号か、利用者定義色と同じ RGB を受ける。JWW は、jww の番号（0〜9 と、バージョン 420 からの拡張色 100〜356）か、ペン 1〜9 と、バージョン 420 からの 100〜356 のどれかと同じ RGB を受ける。書かないか、受けられない色なら断る。マイナーバージョン 29 から
        "color": "ColorChange",
        # 線種。マイナーバージョン 29 から
        "line_type": "LineTypeChange",
        # 線幅。マイナーバージョン 29 から
        "line_weight": "LineWeightChange",
    },
    total=False,
)


class AddEdit(_AddEditRequired, _AddEditOptional):
    "空間か用紙に要素を足す。書かない色・線種・線幅はレイヤに従う。JWW と SXF の図形は色・線種・線幅を自分で持ち、レイヤかブロックに従う値を持たないので、JWW は色と線種を書き（点は色だけ）、SXF は色・線種・線幅を書く（点マーカーは色だけ）。書かないか、レイヤかブロックに従う値なら change.style.unsupported で断る。JWW の書かないかレイヤに従う線幅は、ペンの印刷の線幅に従う。足せる形は、形の値のどの種類も。マイナーバージョン 10 から"


# レイヤの名前・状態・色・線種・線幅のうち、書いたものだけを変える（決定 0162）。DXF は、名前を変えると、図形と今のレイヤ（$CLAYER）も新しい名前で指す。DWG の真の色は R2004 から書ける（今あるレイヤの線種と真の色は、決定 0293 から変えられる）。マイナーバージョン 22 から
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
    "レイヤの名前・状態・色・線種・線幅のうち、書いたものだけを変える（決定 0162）。DXF は、名前を変えると、図形と今のレイヤ（$CLAYER）も新しい名前で指す。DWG の真の色は R2004 から書ける（今あるレイヤの線種と真の色は、決定 0293 から変えられる）。マイナーバージョン 22 から"


# レイヤを除く（決定 0162）。要素・ほかのオブジェクト・今のレイヤ（$CLAYER）が指すレイヤと、0・Defpoints・外部参照に依るレイヤは除けない。同じ apply の要素の変更が指すレイヤも除けない。後ろにハンドルのないレイヤがあれば、その ID が変わるので除けない。除いたレイヤより後ろのレイヤと、その上の要素のハッシュは変わるので、読み直す。マイナーバージョン 24 から
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


# 文字スタイルの名前・フォント・高さ・幅の倍率・傾き・裏返し・上下反転・縦書きのうち、書いたものだけを変える（決定 0163）。DXF は、名前を変えると、文字と今の文字スタイル（$TEXTSTYLE）も新しい名前で指す。表の書式などの図形でないオブジェクトが名前で指す文字スタイルの名前は変えられない。マイナーバージョン 25 から
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
    "文字スタイルの名前・フォント・高さ・幅の倍率・傾き・裏返し・上下反転・縦書きのうち、書いたものだけを変える（決定 0163）。DXF は、名前を変えると、文字と今の文字スタイル（$TEXTSTYLE）も新しい名前で指す。表の書式などの図形でないオブジェクトが名前で指す文字スタイルの名前は変えられない。マイナーバージョン 25 から"


# 文字スタイルを足す（決定 0163）。書かないフィールドは Standard と同じ。同じ apply の後ろの文字の追加・置き換えから、足した文字スタイルを名前で指せる。マイナーバージョン 25 から
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
    "文字スタイルを足す（決定 0163）。書かないフィールドは Standard と同じ。同じ apply の後ろの文字の追加・置き換えから、足した文字スタイルを名前で指せる。マイナーバージョン 25 から"


# 文字スタイルを除く（決定 0163）。要素・表の書式や寸法スタイルなどのほかのオブジェクト・今の文字スタイル（$TEXTSTYLE）が指すものと、Standard・外部参照に依るものは除けない。同じ apply で足す要素が指すものも除けない。後ろにハンドルのない文字スタイルがあれば、その ID が変わるので除けない。マイナーバージョン 25 から
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


# 線種の名前・説明・線と間の長さの列のうち、書いたものだけを変える（決定 0164）。DXF は、名前を変えると、要素・レイヤと今の線種（$CELTYPE）も新しい名前で指す。マルチラインスタイルなどの図形でないオブジェクトが名前で指す線種の名前は変えられない。形か文字を添える線種と DWG の線種は、列の要素の数を変えられない。マイナーバージョン 26 から
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
    "線種の名前・説明・線と間の長さの列のうち、書いたものだけを変える（決定 0164）。DXF は、名前を変えると、要素・レイヤと今の線種（$CELTYPE）も新しい名前で指す。マルチラインスタイルなどの図形でないオブジェクトが名前で指す線種の名前は変えられない。形か文字を添える線種と DWG の線種は、列の要素の数を変えられない。マイナーバージョン 26 から"


# 線種を足す（決定 0164）。書かないフィールドは Continuous と同じ。同じ apply の後ろの要素とレイヤの変更から、足した線種を名前で指せる。マイナーバージョン 26 から
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
    "線種を足す（決定 0164）。書かないフィールドは Continuous と同じ。同じ apply の後ろの要素とレイヤの変更から、足した線種を名前で指せる。マイナーバージョン 26 から"


# 線種を除く（決定 0164）。要素・レイヤ・マルチラインスタイルなどのほかのオブジェクト・今の線種（$CELTYPE）が指すものと、ByLayer・ByBlock・Continuous・外部参照に依るものは除けない。同じ apply で足す・変える要素とレイヤが指すものも除けない。後ろにハンドルのない線種があれば、その ID が変わるので除けない。マイナーバージョン 26 から
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


# 寸法スタイルの名前・端部記号・その大きさ・斜線・倍率のうち、書いたものだけを変える（決定 0165）。DXF は、名前を変えると、寸法・引出線・公差と今の寸法スタイル（$DIMSTYLE）も新しい名前で指す。図形でないオブジェクトが名前で指す寸法スタイルの名前は変えられない。マイナーバージョン 27 から
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
    "寸法スタイルの名前・端部記号・その大きさ・斜線・倍率のうち、書いたものだけを変える（決定 0165）。DXF は、名前を変えると、寸法・引出線・公差と今の寸法スタイル（$DIMSTYLE）も新しい名前で指す。図形でないオブジェクトが名前で指す寸法スタイルの名前は変えられない。マイナーバージョン 27 から"


# 寸法スタイルを足す（決定 0165）。書かないフィールドは Standard と同じ。同じ apply の後ろの要素の追加から、足した寸法スタイルを名前で指せる。マイナーバージョン 27 から
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
    "寸法スタイルを足す（決定 0165）。書かないフィールドは Standard と同じ。同じ apply の後ろの要素の追加から、足した寸法スタイルを名前で指せる。マイナーバージョン 27 から"


# 寸法スタイルを除く（決定 0165）。寸法・引出線・公差などのほかのオブジェクト・今の寸法スタイル（$DIMSTYLE）が指すものと、Standard・子の寸法スタイル・子を持つ寸法スタイル・外部参照に依るものは除けない。後ろにハンドルのない寸法スタイルがあれば、その ID が変わるので除けない。マイナーバージョン 27 から
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


# 文書の点の表し方（marker を持たない点の見え方）を変える（決定 0190）。文書全体の設定なので要素を指さない。読んだときの文書は、apply の hash で確かめる。DXF・DWG はヘッダーの $PDMODE・$PDSIZE に書く。DWG は、変数を書く幅が変わる値（$PDMODE の 0 とほかの値の間の変更、$PDSIZE の 0・1 とほかの値の間の変更）を、マイナーバージョン 39 から書ける（決定 0193）。JWW・SXF は文書の点の表し方を持たないので断る。applied は id と hash を持たない。マイナーバージョン 38 から
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
    "文書の点の表し方（marker を持たない点の見え方）を変える（決定 0190）。文書全体の設定なので要素を指さない。読んだときの文書は、apply の hash で確かめる。DXF・DWG はヘッダーの $PDMODE・$PDSIZE に書く。DWG は、変数を書く幅が変わる値（$PDMODE の 0 とほかの値の間の変更、$PDSIZE の 0・1 とほかの値の間の変更）を、マイナーバージョン 39 から書ける（決定 0193）。JWW・SXF は文書の点の表し方を持たないので断る。applied は id と hash を持たない。マイナーバージョン 38 から"

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


# レイヤを足す（決定 0162）。書かないフィールドは、状態なら表示・印刷・編集でき凍結せず、色・線種・線幅ならレイヤ 0 と同じ。同じ apply の後ろの要素の変更から、足したレイヤを名前で指せる。マイナーバージョン 22 から
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
    "レイヤを足す（決定 0162）。書かないフィールドは、状態なら表示・印刷・編集でき凍結せず、色・線種・線幅ならレイヤ 0 と同じ。同じ apply の後ろの要素の変更から、足したレイヤを名前で指せる。マイナーバージョン 22 から"


# 記号の定義（DXF・DWG のブロック）を足す（#684）。中の要素は、定義の空間に足す。足した定義は、同じ apply の後ろの add の記号から名前で指せる。属性の定義・名前のない定義・外部参照は足せない。結果は id と hash を持たない。足した定義は tables の symbols から名前で引く。マイナーバージョン 99 から
_AddSymbolEditRequired = TypedDict(
    "_AddSymbolEditRequired",
    {
        # 変更の種類
        "kind": Literal["add_symbol"],
        # 定義の名前。図面にない名前で、layer の name と同じ決まり（DXF・DWG のブロックの名前で書けない文字を含まない）。大文字と小文字は区別しない
        "name": str,
        # 中の要素。点は定義の空間の座標で書く。並べた順に足す
        "elements": List["SymbolElement"],
    },
)
_AddSymbolEditOptional = TypedDict(
    "_AddSymbolEditOptional",
    {
        # 定義の基点 [X, Y]。記号を置く点に重ねる。書かなければ [0, 0]
        "base": List[float],
    },
    total=False,
)


class AddSymbolEdit(_AddSymbolEditRequired, _AddSymbolEditOptional):
    "記号の定義（DXF・DWG のブロック）を足す（#684）。中の要素は、定義の空間に足す。足した定義は、同じ apply の後ろの add の記号から名前で指せる。属性の定義・名前のない定義・外部参照は足せない。結果は id と hash を持たない。足した定義は tables の symbols から名前で引く。マイナーバージョン 99 から"


# 足す記号の定義の中の要素。フィールドは add と同じ意味で、place と kind を持たない。引出線の注記は指せない。マイナーバージョン 99 から
_SymbolElementRequired = TypedDict(
    "_SymbolElementRequired",
    {
        # 足す形
        "geometry": "Geometry",
    },
)
_SymbolElementOptional = TypedDict(
    "_SymbolElementOptional",
    {
        # レイヤの元の名前。layer_id と一緒には書かない。どちらも書かなければレイヤ 0
        "layer": str,
        # レイヤの ID。tables が返したもの
        "layer_id": str,
        # 色
        "color": "ColorChange",
        # 線種
        "line_type": "LineTypeChange",
        # 線幅
        "line_weight": "LineWeightChange",
    },
    total=False,
)


class SymbolElement(_SymbolElementRequired, _SymbolElementOptional):
    "足す記号の定義の中の要素。フィールドは add と同じ意味で、place と kind を持たない。引出線の注記は指せない。マイナーバージョン 99 から"

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
        # SXF の色のコード。1〜16 は既定義色、17 からは利用者定義色を定義の順に数えたもの。マイナーバージョン 29 から
        "sxf": int,
        # Jw_cad のペンの番号。0〜9 と、バージョン 420 からの SXF の拡張色 100〜356。マイナーバージョン 30 から
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
        # 線種の ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。マイナーバージョン 8 から
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


# 開いた文書を図面モデル（L2）に変換し、レイヤと線種の表の項目を返す（決定 0156）。マイナーバージョン 8 から
_TablesRequestRequired = TypedDict(
    "_TablesRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
    },
    total=False,
)


class TablesRequest(_TablesRequestRequired, _TablesRequestOptional):
    "開いた文書を図面モデル（L2）に変換し、レイヤと線種の表の項目を返す（決定 0156）。マイナーバージョン 8 から"


# 表の項目。図面モデルの順
TablesResult = TypedDict(
    "TablesResult",
    {
        # レイヤ
        "layers": List["LayerItem"],
        # 線種。マイナーバージョン 26 から、定義を持つ
        "line_types": List["LineTypeItem"],
        # 寸法スタイル。マイナーバージョン 27 から、端部記号と大きさを持つ
        "dimension_styles": List["DimensionStyleItem"],
        # 文字スタイル（決定 0158）。要素が名前で指すだけで、ファイルに定義がないものは、定義のフィールドを持たない項目として後ろに並ぶ。マイナーバージョン 20 から
        "text_styles": List["TextStyleItem"],
        # 記号の定義。配置されていないものも含む。マイナーバージョン 81 から
        "symbols": List["SymbolDefinitionItem"],
    },
)


# 記号の定義。要素と属性の配置や書式は元の定義に保ち、名前と属性の既定値を一覧にする
_SymbolDefinitionItemRequired = TypedDict(
    "_SymbolDefinitionItemRequired",
    {
        # 定義の空間の公開 ID。entities.place と同じ
        "id": str,
        # 定義を識別する名前空間
        "namespace": str,
        # 元の名前。add の記号の name に使う
        "name": str,
        # 属性のタグ・既定値・入力の指定。元の順を保つ
        "attributes": List["SymbolAttributeDefinition"],
        # 寸法などが作った無名の定義である
        "anonymous": bool,
        # 定義の出所
        "source": "Source",
    },
)
_SymbolDefinitionItemOptional = TypedDict(
    "_SymbolDefinitionItemOptional",
    {
        # 定義の基点。定義の空間の座標と単位。不明なら持たない
        "base": "Point2",
        # 外部参照の元のパス。参照先を開かない。外部参照でなければ持たない
        "external": str,
    },
    total=False,
)


class SymbolDefinitionItem(_SymbolDefinitionItemRequired, _SymbolDefinitionItemOptional):
    "記号の定義。要素と属性の配置や書式は元の定義に保ち、名前と属性の既定値を一覧にする"


# 記号の属性の定義。タグと値は翻訳しない。配置・書式・スタイルは一覧には含めない
_SymbolAttributeDefinitionRequired = TypedDict(
    "_SymbolAttributeDefinitionRequired",
    {
        # 元のタグ
        "tag": str,
        # 既定の 1 行の値
        "value": str,
        # 値を尋ねる文
        "prompt": str,
        # 値が決まっていて、配置ごとに変えない
        "constant": bool,
        # 入力した値を確かめる指定
        "verify": bool,
        # 尋ねずに既定の値を使う指定
        "preset": bool,
        # 属性を表示しない
        "hidden": bool,
        # 属性の出所
        "source": "Source",
    },
)
_SymbolAttributeDefinitionOptional = TypedDict(
    "_SymbolAttributeDefinitionOptional",
    {
        # 複数行の値があるとき、その元の書式つきの文字列。読むための値
        "formatted_value": str,
    },
    total=False,
)


class SymbolAttributeDefinition(_SymbolAttributeDefinitionRequired, _SymbolAttributeDefinitionOptional):
    "記号の属性の定義。タグと値は翻訳しない。配置・書式・スタイルは一覧には含めない"


# 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、2D のシーンを blob で返す（決定 0172）。シーンの配置は schema/scene.toml にある。マイナーバージョン 33 から
_SceneRequestRequired = TypedDict(
    "_SceneRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した空間の要素だけを描く。from が用紙のときだけ書ける
        "view": int,
        # [x の最小, y の最小, x の最大, y の最大]。起点の座標の長方形。範囲の重なる塊だけを返す
        "region": List[float],
        # 弦の誤差（起点の長さ）。0 より大きい。書かなければ、用紙からは 0.02 mm、空間からは要素の範囲の対角線の 1/50000
        "tolerance": float,
        # true なら印刷する要素を、書かないか false なら表示する要素を描く
        "print": bool,
        # 色の表の入力の名前（zm_push で渡したもの）、または組み込みの表 builtin:cad-v1。プロファイルの色の語彙と同じ形の JSON で、values に rgb（#rrggbb）と aci・sxf・jww の符号を並べる（決定 0138・0178）。書かなければ、ファイルが RGB を持たない色は未解決のフラグを立てる
        "palette": str,
        # 文字を置くフォント（決定 0188）。書けば、文字を字形で置き、シーンの字形のセクションに形と変換を返す。文字スタイルと書式の区間のフォントの名前（シーンの font_names）で引き、同じ名前、なければ ASCII の大文字と小文字を区別しない名前で合わせる。合うものがなければ名前のないフォントで置く。マイナーバージョン 37 から
        "fonts": List["SceneFont"],
        # 形式の宣言値の高さ（キーは dxf.TEXT.40・dxf.MTEXT.40・jww.moji.size_y など）が、フォントのどの寸法に当たるか（決定 0176・0188）。書かないキーは、表示と比べて決めた意味（dxf.TEXT.40 は capital_a。マイナーバージョン 59 から）にし、それもなければ大文字の高さとみなす。fonts を書いたときだけ使う。マイナーバージョン 37 から
        "heights": Dict[str, "HeightReference"],
    },
    total=False,
)


class SceneRequest(_SceneRequestRequired, _SceneRequestOptional):
    "開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、2D のシーンを blob で返す（決定 0172）。シーンの配置は schema/scene.toml にある。マイナーバージョン 33 から"


# 建物モデルを開口付きの 3D シーンにし、ZMS3 の blob で返す。86 から地物の面と高さのある線・面も扱う。階・クラス・ID の各条件の中は和、条件どうしは積。箱は要素の重なりで選び、メッシュを切らない。88 から点群を元の八分木の節ごとの点の区画（ZMS3 1.1）で返す。点群の点は pointcloud と同じ節の順で選び、座標系と高さの基準を変換しない。94 から平面直角座標系の GeoTIFF の DEM も、外枠の地物でなく標高格子を点群と同じ点の区画で返す。105 から surface で、DEM を地形の面（セル代表点を頂点にした三角形のメッシュ）で返す。マイナーバージョン 82 から
_Scene3dRequestRequired = TypedDict(
    "_Scene3dRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["scene3d"],
        # open で作った建物・地物・点群の文書名。点群は 88 から
        "doc": str,
    },
)
_Scene3dRequestOptional = TypedDict(
    "_Scene3dRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 所属する階の要素の ID。省略は全階、空の配列は何も選ばない。地物と点群には指定できない
        "storeys": List[str],
        # 建物のクラス名か地物の型。大文字・小文字を区別する。省略は全種類、空の配列は何も選ばない。点群には指定できない
        "classes": List[str],
        # 建物の要素か元の地物の ID。省略は全要素、空の配列は何も選ばない。建物では参照する階の行も返す。点群には指定できない
        "ids": List[str],
        # 地物だけに指定する投影先の平面直角座標系の系（1〜19）。省略時は元の系、経緯度では範囲の中心から計算用の系を仮定して診断する。86 から
        "zone": int,
        # 地物だけに指定する LOD（0〜4）。省略時は選択した地物にある最大の LOD を 1 段選ぶ。LOD のない形は含める。86 から
        "lod": int,
        # プロジェクト空間の文書単位の箱。重なる要素を選び、形のない要素も残す。切り抜きは読み手が行う
        "box": "Scene3dBox",
        # 点群だけに指定する返す点の数の上限。省略時は選んだすべての点。88 から。DEM を surface で返すときは、頂点にする格子点の数の上限。超えれば間引く間隔を 2 倍ずつ広げ、それでも超えれば何も返さない
        "limit": int,
        # 平面直角座標系の GeoTIFF の DEM だけに指定する。真なら標高格子を点でなく地形の面で返す。セル代表点を頂点にし、隣り合う 4 つの格子点が点にできれば 2 個、3 つなら 1 個の三角形を作る。nodata などのセルは面に含めない。面は 128 間隔四方の区画ごとの要素（ID は区画の初めの行と列の r<行>c<列>、クラスは dem）に分け、隣の区画と縁の頂点を共有する。座標系と高さの基準は変換しない。偽か省略なら点で返す。105 から
        "surface": bool,
        # 文書単位の正の有限値。点群では点の間隔（メートル）で、これより細かい段の節を選ばない。DEM の面では頂点の間隔（メートル）で、セル代表点の間隔の整数倍のうち、これ以上でもっとも細かい間隔に間引く。建物では曲線を分ける弦の誤差。地物では f32 の座標の丸めをこの値の 1/4 までに制限し、投影した辺全体の誤差は含めない。省略時は対象範囲の対角線の 1/50000、空か点だけなら 1 mm 相当。建物の範囲は 1 mm の予備評価で求める
        "tolerance": float,
    },
    total=False,
)


class Scene3dRequest(_Scene3dRequestRequired, _Scene3dRequestOptional):
    "建物モデルを開口付きの 3D シーンにし、ZMS3 の blob で返す。86 から地物の面と高さのある線・面も扱う。階・クラス・ID の各条件の中は和、条件どうしは積。箱は要素の重なりで選び、メッシュを切らない。88 から点群を元の八分木の節ごとの点の区画（ZMS3 1.1）で返す。点群の点は pointcloud と同じ節の順で選び、座標系と高さの基準を変換しない。94 から平面直角座標系の GeoTIFF の DEM も、外枠の地物でなく標高格子を点群と同じ点の区画で返す。105 から surface で、DEM を地形の面（セル代表点を頂点にした三角形のメッシュ）で返す。マイナーバージョン 82 から"


# 建物ではプロジェクト空間の文書単位、地物では投影先の、点群では元の座標系の東・北・高さのメートルの箱。Z が上
Scene3dBox = TypedDict(
    "Scene3dBox",
    {
        # 最小の X・Y・Z の 3 値
        "min": List[float],
        # 最大の X・Y・Z の 3 値
        "max": List[float],
    },
)


# 建物では文書が持つ地図への変換の元値で、シーンの座標へ適用していない。地物では投影先の EPSG 名と恒等変換を返し、シーンの X・Y は既に東・北のメートル
_Scene3dMapRequired = TypedDict(
    "_Scene3dMapRequired",
    {
        # 東への移動
        "eastings": float,
        # 北への移動
        "northings": float,
        # 高さの移動
        "height": float,
    },
)
_Scene3dMapOptional = TypedDict(
    "_Scene3dMapOptional",
    {
        # 地図の座標系の名前。未指定なら持たない
        "crs_name": str,
        # 地図の X 軸の横の成分の元値
        "x_axis_abscissa": float,
        # 地図の X 軸の縦の成分の元値
        "x_axis_ordinate": float,
        # 地図への尺度の元値
        "scale": float,
    },
    total=False,
)


class Scene3dMap(_Scene3dMapRequired, _Scene3dMapOptional):
    "建物では文書が持つ地図への変換の元値で、シーンの座標へ適用していない。地物では投影先の EPSG 名と恒等変換を返し、シーンの X・Y は既に東・北のメートル"


# 返した 3D シーンの列の行数。共有するメッシュの点と面は 1 回だけ数える
_Scene3dCountsRequired = TypedDict(
    "_Scene3dCountsRequired",
    {
        # 塊
        "chunks": int,
        # 共有するメッシュ
        "meshes": int,
        # メッシュの配置
        "instances": int,
        # 要素と参照先の階。形のない行も含む
        "elements": int,
        # 色
        "colors": int,
        # 局所の頂点
        "vertices": int,
        # 三角形
        "triangles": int,
        # 形の稜線
        "edges": int,
    },
)
_Scene3dCountsOptional = TypedDict(
    "_Scene3dCountsOptional",
    {
        # 点群の点。点群のシーンだけが持つ。88 から
        "points": int,
    },
    total=False,
)


class Scene3dCounts(_Scene3dCountsRequired, _Scene3dCountsOptional):
    "返した 3D シーンの列の行数。共有するメッシュの点と面は 1 回だけ数える"


# 3D シーンの座標・許容差・行数と blob の番号
_Scene3dResultRequired = TypedDict(
    "_Scene3dResultRequired",
    {
        # 元の建物か地物の文書名
        "doc": str,
        # シーンの座標と許容差の長さの単位
        "unit": "BuildingUnit",
        # 使った文書単位の許容差。建物では曲線の弦の誤差、地物では f32 の丸めの上限の 4 倍。点群では要求の点の間隔、省略時は返したもっとも深い段の点の間隔。DEM の面では使った頂点の間隔。丸めのずれは別の診断で返す
        "tolerance": float,
        # シーンの行数
        "counts": "Scene3dCounts",
        # 種類が scene3d の blob の番号。zm_take で取り出す
        "blob": int,
    },
)
_Scene3dResultOptional = TypedDict(
    "_Scene3dResultOptional",
    {
        # 文書の地図への変換。なければ持たない
        "map": "Scene3dMap",
        # 地物の高さの基準と成果を表す EPSG。元が不明なら持たない。86 から
        "height_crs": int,
        # 地物で選択した LOD。LOD のない形だけなら持たない。86 から
        "lod": int,
        # 点群で上限のために返さなかった点がある。DEM の面では、上限のために面を返さなかった。点群と DEM のシーンだけが持つ。88 から
        "truncated": bool,
        # 要求の断面の箱。なければ持たない
        "box": "Scene3dBox",
        # 返した塊の文書単位の範囲。形がなければ持たない
        "extent": "Scene3dBox",
    },
    total=False,
)


class Scene3dResult(_Scene3dResultRequired, _Scene3dResultOptional):
    "3D シーンの座標・許容差・行数と blob の番号"


# 開いた文書を scene と同じ起点・配置・フォントで 2D のシーンにし、画像を blob で返す（決定 0272・0275・0277）。字形の輪郭だけを描き、字形のない空でない文字・外部画像・名前付きの点記号は元の ID と unit を診断して描かない。シーン生成と描画で同じ予算と中断の合図を使い、失敗では画像を返さない。PNG は最大 16777216 画素で、変換後の座標は絶対値 2^31 ピクセル以内。SVG はマイナーバージョン 64、PNG は 65 から
_RenderRequestRequired = TypedDict(
    "_RenderRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["render"],
        # 文書の名前。open で作ったもの
        "doc": str,
        # scene と同じ起点。実寸の空間・記号の定義の空間・用紙の ID
        "from": str,
        # 画像の出力形式
        "format": "RenderFormat",
        # 出力の幅（ピクセル）
        "width": int,
        # 出力の高さ（ピクセル）
        "height": int,
        # [x の最小, y の最小, x の最大, y の最大]。起点の座標で、幅と高さが正の有限値になる範囲を明示する。scene の範囲の絞り込みにも使い、縦横同率で中央に収めて範囲外を切る。縦横比の違いによる余白は背景色にする
        "region": List[float],
        # 用紙 mm の線幅をピクセルへ直す DPI。正の有限値。座標の単位を mm と推測しない。線幅 0 は 1 ピクセル
        "dpi": float,
        # 背景の sRGB と alpha。[r, g, b, a] の 4 値を 0〜255 で明示する
        "background": List[int],
        # ACI 7 と未解決の色に使う sRGB と alpha。[r, g, b, a] の 4 値を 0〜255 で明示する。未解決の色は診断する
        "foreground": List[int],
    },
)
_RenderRequestOptional = TypedDict(
    "_RenderRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 起点の用紙のビューの番号。0 から数える。from が用紙のときだけ書ける。scene と同じ意味
        "view": int,
        # scene と同じ弦の誤差（起点の長さ）。0 より大きい。書かなければ用紙は 0.02 mm、空間は要素の範囲の対角線の 1/50000
        "tolerance": float,
        # scene と同じ指定。true は印刷する要素、書かないか false は表示する要素を描く
        "print": bool,
        # 隠すレイヤの公開 ID の一覧。scene の layer_ids と同じ値で描画単位を除く。未知の ID は失敗、省略または空なら従来どおり描く
        "hidden_layers": List[str],
        # scene と同じ色の表の入力の名前、または組み込みの表 builtin:cad-v1。zm_push で渡した JSON の values に rgb と aci・sxf・jww の符号を並べる
        "palette": str,
        # scene と同じフォントの指定。名前を合わせ、なければ名前のないフォントを使う。字形の輪郭を作るため、zm_push でフォントのバイト列を渡す。OS のフォントは探索しない
        "fonts": List["SceneFont"],
        # scene と同じ宣言値の高さの意味。fonts を書いたときだけ使う。書かないキーは既定の意味に従う（決定 0176・0188）
        "heights": Dict[str, "HeightReference"],
    },
    total=False,
)


class RenderRequest(_RenderRequestRequired, _RenderRequestOptional):
    "開いた文書を scene と同じ起点・配置・フォントで 2D のシーンにし、画像を blob で返す（決定 0272・0275・0277）。字形の輪郭だけを描き、字形のない空でない文字・外部画像・名前付きの点記号は元の ID と unit を診断して描かない。シーン生成と描画で同じ予算と中断の合図を使い、失敗では画像を返さない。PNG は最大 16777216 画素で、変換後の座標は絶対値 2^31 ピクセル以内。SVG はマイナーバージョン 64、PNG は 65 から"

# 画像の出力形式。svg は SVG 1.1（image/svg+xml）、png は RGBA8・非インターレースの PNG（image/png、マイナーバージョン 65 から）
RenderFormat = Union[
    Literal[
        "svg",
        "png",
    ],
    str,
]


# 画像の形式・サイズと、シーンの起点・単位・許容差、画像の blob の番号
_RenderResultRequired = TypedDict(
    "_RenderResultRequired",
    {
        # 画像の出力形式
        "format": "RenderFormat",
        # 出力の幅（ピクセル）
        "width": int,
        # 出力の高さ（ピクセル）
        "height": int,
        # シーンの起点の ID
        "from": str,
        # 起点の座標の単位。用紙とビューからは millimeter
        "unit": "Unit",
        # シーンで使った弦の誤差（起点の長さ）
        "tolerance": float,
        # 画像の blob の番号（種類は image）。zm_take で取り出す
        "blob": int,
    },
)
_RenderResultOptional = TypedDict(
    "_RenderResultOptional",
    {
        # 起点のビューの番号。ビューでなければ持たない
        "view": int,
    },
    total=False,
)


class RenderResult(_RenderResultRequired, _RenderResultOptional):
    "画像の形式・サイズと、シーンの起点・単位・許容差、画像の blob の番号"


# 開いた TIFF・GeoTIFF の指定ページの画素を RGBA8・非インターレースの PNG に復号し、画像の blob で返す（#877）。複数ページの TIFF は page で 1 から数えるページ番号を選ぶ（書かなければ 1）。max_side を指定すると、長辺が max_side 以下になるよう縦横比を保って縮小して返す（最大 16777216 画素）。マイナーバージョン 73 から
_RasterRequestRequired = TypedDict(
    "_RasterRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["raster"],
        # 文書の名前。open で作ったもの
        "doc": str,
    },
)
_RasterRequestOptional = TypedDict(
    "_RasterRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # ページ番号（IFD の 1 から始まる番号）。書かなければ 1
        "page": int,
        # 出力の長辺の最大ピクセル数。元の長辺がこれを超えるときだけ縦横比を保って縮小する
        "max_side": int,
    },
    total=False,
)


class RasterRequest(_RasterRequestRequired, _RasterRequestOptional):
    "開いた TIFF・GeoTIFF の指定ページの画素を RGBA8・非インターレースの PNG に復号し、画像の blob で返す（#877）。複数ページの TIFF は page で 1 から数えるページ番号を選ぶ（書かなければ 1）。max_side を指定すると、長辺が max_side 以下になるよう縦横比を保って縮小して返す（最大 16777216 画素）。マイナーバージョン 73 から"


# 復号した PNG 画像のサイズ・元のページサイズ・総ページ数と、画像の blob の番号
RasterResult = TypedDict(
    "RasterResult",
    {
        # 出力した PNG の幅（ピクセル）
        "width": int,
        # 出力した PNG の高さ（ピクセル）
        "height": int,
        # 元のページの幅（ピクセル）
        "source_width": int,
        # 元のページの高さ（ピクセル）
        "source_height": int,
        # 復号したページ番号（1 から）
        "page": int,
        # 文書に含まれる総ページ（IFD）数
        "pages": int,
        # PNG 画像の blob の番号（種類は image）。zm_take で取り出す
        "blob": int,
    },
)


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

# 宣言された高さが当たるフォントの寸法。cap_height は大文字の高さ、em は em、frame はアセンダーとディセンダーを合わせた高さ、capital_a は A の字形の外形の上（A がなければ大文字の高さ。マイナーバージョン 59 から）
HeightReference = Union[
    Literal[
        "cap_height",
        "em",
        "frame",
        "capital_a",
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
        # 置いた字形。fonts を書かなければ 0。マイナーバージョン 37 から
        "glyphs": int,
    },
)


# 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、プロファイルの規則と語彙で照合する（決定 0138・0173・0179）。マイナーバージョン 34 から
_CheckRequestRequired = TypedDict(
    "_CheckRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # たどる起点。実寸の空間・記号の定義の空間・用紙の ID。書かなければ、すべての用紙から、用紙がなければ実寸の空間からたどる
        "from": str,
        # 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した要素と、そのビューの縮尺だけを照合する。from が用紙のときだけ書ける
        "view": int,
        # 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。マイナーバージョン 40 から
        "file_name": str,
        # 返す最初の指摘の位置。前の応答の next_offset を渡す。書かなければ 0
        "offset": int,
        # 返す指摘の数の上限。書かなければ 1000
        "limit": int,
        # 色の表の入力の名前（zm_push で渡したもの）、または組み込みの表 builtin:cad-v1。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く
        "palette": str,
    },
    total=False,
)


class CheckRequest(_CheckRequestRequired, _CheckRequestOptional):
    "開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、プロファイルの規則と語彙で照合する（決定 0138・0173・0179）。マイナーバージョン 34 から"


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


# zm_push で渡した IFC（IFC-SPF か ifcZIP）を、zm_push で渡した EXPRESS のスキーマに照らしてチェックする（決定 0207・0216）。インスタンスの実体の名前・抽象・属性の数・値の型・省略を見る。WHERE・UNIQUE・RULE の規則は評価せず、数を返す。zumen は EXPRESS を同梱しない。マイナーバージョン 42 から
_ValidateRequestRequired = TypedDict(
    "_ValidateRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 返す最初の指摘の位置。前の応答の next_offset を渡す。書かなければ 0
        "offset": int,
        # 返す指摘の数の上限。書かなければ 1000
        "limit": int,
    },
    total=False,
)


class ValidateRequest(_ValidateRequestRequired, _ValidateRequestOptional):
    "zm_push で渡した IFC（IFC-SPF か ifcZIP）を、zm_push で渡した EXPRESS のスキーマに照らしてチェックする（決定 0207・0216）。インスタンスの実体の名前・抽象・属性の数・値の型・省略を見る。WHERE・UNIQUE・RULE の規則は評価せず、数を返す。zumen は EXPRESS を同梱しない。マイナーバージョン 42 から"


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


# レビューの文書（決定 0214・0218）の 1 つの対象を、開いた図面の文書のバージョンへ移す。要素の参照を図面の要素の ID で引き、参照ごとに unchanged・changed・removed を返す。場所と図形に元のバージョンのハッシュと状態を書き、対象のハッシュを文書のハッシュにする。2D の点は動かさない。移した文書は blobs の file で返す。マイナーバージョン 43 から
_ReviewCarryRequestRequired = TypedDict(
    "_ReviewCarryRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["review_carry"],
        # 新しいバージョンの文書の名前。open で作ったもの。マイナーバージョン 44 から、IFC の建物モデルの文書も渡せ、要素の参照を建物モデルの要素の ID とハッシュで引く
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 返す最初の参照の位置。前の応答の next_offset を渡す。書かなければ 0
        "offset": int,
        # 返す参照の数の上限。書かなければ 1000
        "limit": int,
    },
    total=False,
)


class ReviewCarryRequest(_ReviewCarryRequestRequired, _ReviewCarryRequestOptional):
    "レビューの文書（決定 0214・0218）の 1 つの対象を、開いた図面の文書のバージョンへ移す。要素の参照を図面の要素の ID で引き、参照ごとに unchanged・changed・removed を返す。場所と図形に元のバージョンのハッシュと状態を書き、対象のハッシュを文書のハッシュにする。2D の点は動かさない。移した文書は blobs の file で返す。マイナーバージョン 43 から"


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

# 移したときの状態（決定 0214）。unchanged は要素のハッシュが同じ、changed は違うか元のハッシュがない、removed は新しいバージョンに要素がない
CarriedState = Union[
    Literal[
        "unchanged",
        "changed",
        "removed",
    ],
    str,
]


# 開いた文書を check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0214・0218）に足す。指摘の ID は対象のハッシュ・規則・診断から決め、取り込み済みの指摘は足さない。足した文書は blobs の file で返す。マイナーバージョン 43 から
_ReviewAdoptRequestRequired = TypedDict(
    "_ReviewAdoptRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # たどる起点。実寸の空間・記号の定義の空間・用紙の ID。書かなければ、すべての用紙から、用紙がなければ実寸の空間からたどる
        "from": str,
        # 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した要素と、そのビューの縮尺だけを照合する。from が用紙のときだけ書ける
        "view": int,
        # 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。マイナーバージョン 40 から
        "file_name": str,
        # 色の表の入力の名前（zm_push で渡したもの）、または組み込みの表 builtin:cad-v1。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く
        "palette": str,
        # 足す先のレビューの文書の入力の名前（zm_push で渡した JSON）。書かなければ空の文書に足す
        "review": str,
    },
    total=False,
)


class ReviewAdoptRequest(_ReviewAdoptRequestRequired, _ReviewAdoptRequestOptional):
    "開いた文書を check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0214・0218）に足す。指摘の ID は対象のハッシュ・規則・診断から決め、取り込み済みの指摘は足さない。足した文書は blobs の file で返す。マイナーバージョン 43 から"


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


# レビューの文書（決定 0214・0295）の 1 つの対象の図形と指摘の場所を、開いた図面の起点で、図面とは別の 2D のシーンにして blob で返す。対象の形式とハッシュは開いた文書から作り、文書の対象と合わなければ描かない。シーンの配置は scene と同じ schema/scene.toml。色・線幅・寸法は style で明示し、文書の値から推測しない。レビューの文書と図面は変えない。マイナーバージョン 95 から
_ReviewSceneRequestRequired = TypedDict(
    "_ReviewSceneRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["review_scene"],
        # 対象の図面の文書の名前。open で作ったもの
        "doc": str,
        # レビューの文書の入力の名前（zm_push で渡した JSON）
        "review": str,
        # 描く対象の ID。レビューの文書の targets の id
        "target": str,
        # 起点。実寸の空間・記号の定義の空間・用紙の ID（scene と同じ）。文書の図形・場所の view がこの起点を指すものだけを描く
        "from": str,
        # 弦の誤差（起点の長さ）。0 より大きい有限値。既定値を補わない
        "tolerance": float,
        # 選んだ項目に共通で当てる色と寸法
        "style": "ReviewSceneStyle",
    },
)
_ReviewSceneRequestOptional = TypedDict(
    "_ReviewSceneRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 起点の用紙のビューの番号。0 から数える。from が用紙のときだけ書ける。書けば、文書の view が用紙の ID に /view/<番号> を足した値の項目だけを描く
        "view": int,
        # [x の最小, y の最小, x の最大, y の最大]。起点の座標の長方形。範囲の重なる塊だけを返す
        "region": List[float],
        # 文字を置くフォント（scene と同じ）。style の font の名前で引く。書かなければ字形を置かず、注記の枠を文字高と字数から概算して診断する
        "fonts": List["SceneFont"],
    },
    total=False,
)


class ReviewSceneRequest(_ReviewSceneRequestRequired, _ReviewSceneRequestOptional):
    "レビューの文書（決定 0214・0295）の 1 つの対象の図形と指摘の場所を、開いた図面の起点で、図面とは別の 2D のシーンにして blob で返す。対象の形式とハッシュは開いた文書から作り、文書の対象と合わなければ描かない。シーンの配置は scene と同じ schema/scene.toml。色・線幅・寸法は style で明示し、文書の値から推測しない。レビューの文書と図面は変えない。マイナーバージョン 95 から"


# レビューの図形の色と寸法。幾何の寸法は起点の単位、線幅だけは用紙の mm。文書には保存しない
_ReviewSceneStyleRequired = TypedDict(
    "_ReviewSceneStyleRequired",
    {
        # 線・面・文字の色。#rrggbb
        "color": str,
        # 用紙の mm の線幅。0 以上の有限値。0 はもっとも細い線
        "line_weight": float,
        # 起点の単位での文字の高さ。正の有限値
        "text_height": float,
        # 起点の単位での注記の枠の余白。0 以上の有限値
        "text_padding": float,
        # 起点の単位での矢じりの長さ。正の有限値
        "arrow_length": float,
        # 赤線の矢じりを置く端
        "arrows": "ReviewArrows",
        # 雲の円弧の弦の長さの上限。起点の単位の正の有限値
        "cloud_pitch": float,
        # 写真と指摘の位置の記号の直径。起点の単位の正の有限値
        "marker_size": float,
    },
)
_ReviewSceneStyleOptional = TypedDict(
    "_ReviewSceneStyleOptional",
    {
        # fonts から引くフォントの名前。書かなければ名前のないフォント
        "font": str,
    },
    total=False,
)


class ReviewSceneStyle(_ReviewSceneStyleRequired, _ReviewSceneStyleOptional):
    "レビューの図形の色と寸法。幾何の寸法は起点の単位、線幅だけは用紙の mm。文書には保存しない"

# 赤線の矢じりを置く端。none は置かない、start は最初の端点、end は最後の端点、both は両端
ReviewArrows = Union[
    Literal[
        "none",
        "start",
        "end",
        "both",
    ],
    str,
]


# レビューのシーンの要約と、シーンの blob の番号、シーンの ID から元の図形・場所への対応
_ReviewSceneResultRequired = TypedDict(
    "_ReviewSceneResultRequired",
    {
        # 起点の ID
        "from": str,
        # 起点の座標の単位。用紙とビューからは millimeter。単位の不明な空間は unknown のまま
        "unit": "Unit",
        # 使った弦の誤差（起点の長さ）
        "tolerance": float,
        # 返した描くものの数
        "counts": "SceneCounts",
        # 描いた図形・場所ごとの対応。描く順（文書の markups の順、続いて topics と各指摘の locations の順）
        "sources": List["ReviewSceneSource"],
        # シーンの blob の番号（種類は scene）。zm_take で取り出す
        "blob": int,
    },
)
_ReviewSceneResultOptional = TypedDict(
    "_ReviewSceneResultOptional",
    {
        # 起点のビューの番号。ビューでなければ持たない
        "view": int,
        # 返した塊の範囲。起点の座標。塊がなければ持たない
        "extent": "Extent",
    },
    total=False,
)


class ReviewSceneResult(_ReviewSceneResultRequired, _ReviewSceneResultOptional):
    "レビューのシーンの要約と、シーンの blob の番号、シーンの ID から元の図形・場所への対応"


# シーンの ID（シーンの entity_ids）から、元のレビューの文書の図形か指摘の場所へ戻す対応
_ReviewSceneSourceRequired = TypedDict(
    "_ReviewSceneSourceRequired",
    {
        # シーンの ID。review/markup/<hex> か review/location/<hex>/<hex>
        "id": str,
        # 元の項目の種類
        "kind": "ReviewSceneSourceKind",
    },
)
_ReviewSceneSourceOptional = TypedDict(
    "_ReviewSceneSourceOptional",
    {
        # 元の図形の ID。kind が markup のときだけ持つ
        "markup": str,
        # 元の指摘の ID。kind が location のときだけ持つ
        "topic": str,
        # 元の場所の ID。kind が location のときだけ持つ
        "location": str,
    },
    total=False,
)


class ReviewSceneSource(_ReviewSceneSourceRequired, _ReviewSceneSourceOptional):
    "シーンの ID（シーンの entity_ids）から、元のレビューの文書の図形か指摘の場所へ戻す対応"

# シーンの ID の元の項目の種類。markup はレビューの文書の図形、location は指摘の中の場所
ReviewSceneSourceKind = Union[
    Literal[
        "markup",
        "location",
    ],
    str,
]


# 開いた IFC の建物モデル（決定 0207・0219）の要約を返す。マイナーバージョン 44 から
_BuildingRequestRequired = TypedDict(
    "_BuildingRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
    },
    total=False,
)


class BuildingRequest(_BuildingRequestRequired, _BuildingRequestOptional):
    "開いた IFC の建物モデル（決定 0207・0219）の要約を返す。マイナーバージョン 44 から"


# 建物モデルの要約
_BuildingResultRequired = TypedDict(
    "_BuildingResultRequired",
    {
        # IFC のスキーマの名前（FILE_SCHEMA の最初）
        "schema": str,
        # 入力のバイト列のハッシュ。review_carry の対象のバージョンに使う
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


# 開いた IFC の建物モデルの要素を、書いた順に返す。コンポーネントは components で選ぶ（決定 0207・0219）。マイナーバージョン 44 から
_BuildingElementsRequestRequired = TypedDict(
    "_BuildingElementsRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 返す要素の ID。書けば、その要素だけを、書いた順に返す。ない ID は返さない
        "ids": List[str],
        # IFC のクラスの名前。書けば、そのクラスの要素だけを返す。大文字と小文字を区別しない
        "class": str,
        # 返すコンポーネントの名前。1 つ以上で、重ねない。書かなければ class と identity を返す
        "components": List["BuildingComponent"],
        # 返す最初の位置。前の応答の next_offset を渡す。書かなければ 0
        "offset": int,
        # 返す数の上限。書かなければ 1000
        "limit": int,
    },
    total=False,
)


class BuildingElementsRequest(_BuildingElementsRequestRequired, _BuildingElementsRequestOptional):
    "開いた IFC の建物モデルの要素を、書いた順に返す。コンポーネントは components で選ぶ（決定 0207・0219）。マイナーバージョン 44 から"

# building_elements で返すコンポーネント（決定 0207）。relations は索引から求める子・参照・型の実例・開口・埋める要素・構成員
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


# 建物モデルの要素。コンポーネントは、選んだもののうち要素が持つものだけを持つ
_BuildingElementRequired = TypedDict(
    "_BuildingElementRequired",
    {
        # 要素の ID（GlobalId か、i と番号）
        "id": str,
        # 要素のコンポーネントのハッシュ（v1: と 32 桁の 16 進、決定 0207）
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
    "建物モデルの要素。コンポーネントは、選んだもののうち要素が持つものだけを持つ"


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
        # 体系のバージョン
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


# 開いた IFC に、変更の一覧を当てて書き戻す（決定 0207・0244）。要素は、building_elements が返す ID とハッシュで指す。どれか 1 つでも当てられなければ、何も変えない。当てたら、文書を書き戻したものに替え、書き戻したファイル（ifcZIP で開いたものは ifcZIP）を blobs の file で返す。名前を挙げないインスタンスは、元のバイト列のまま書く。マイナーバージョン 58 から
_BuildingApplyRequestRequired = TypedDict(
    "_BuildingApplyRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["building_apply"],
        # 文書の名前。open で IFC を開いたもの
        "doc": str,
        # 変更。並べた順に当てる
        "edits": List["BuildingEdit"],
    },
)
_BuildingApplyRequestOptional = TypedDict(
    "_BuildingApplyRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 読んだときの文書のハッシュ（building の hash）。書けば、今の文書と同じときだけ当てる
        "hash": str,
    },
    total=False,
)


class BuildingApplyRequest(_BuildingApplyRequestRequired, _BuildingApplyRequestOptional):
    "開いた IFC に、変更の一覧を当てて書き戻す（決定 0207・0244）。要素は、building_elements が返す ID とハッシュで指す。どれか 1 つでも当てられなければ、何も変えない。当てたら、文書を書き戻したものに替え、書き戻したファイル（ifcZIP で開いたものは ifcZIP）を blobs の file で返す。名前を挙げないインスタンスは、元のバイト列のまま書く。マイナーバージョン 58 から"

# 建物の要素 1 つへの変更。kind の値で形が決まる。add_space・add_element・remove・place・contain・type・void・fill はマイナーバージョン 96 から、add_wall はマイナーバージョン 97 から、add_slab・add_column・add_beam・add_opening・add_door・add_window はマイナーバージョン 98 から
BuildingEdit = Union[
    "BuildingPropertyEdit",
    "BuildingAttributeEdit",
    "BuildingAddSpaceEdit",
    "BuildingAddElementEdit",
    "BuildingRemoveEdit",
    "BuildingPlaceEdit",
    "BuildingContainEdit",
    "BuildingTypeEdit",
    "BuildingVoidEdit",
    "BuildingFillEdit",
    "BuildingAddWallEdit",
    "BuildingAddSlabEdit",
    "BuildingAddColumnEdit",
    "BuildingAddBeamEdit",
    "BuildingAddOpeningEdit",
    "BuildingAddDoorEdit",
    "BuildingAddWindowEdit",
    "BuildingAddGridEdit",
    "BuildingElevationEdit",
    "BuildingWallShapeEdit",
    "BuildingOpeningShapeEdit",
    "BuildingAddTypeEdit",
    "BuildingQuantityEdit",
    "BuildingAddUnitEdit",
    "BuildingMaterialEdit",
    "BuildingShapeEdit",
]


# 要素のプロパティのセットの、プロパティ（IfcPropertySingleValue）の値を変える。セットかプロパティがなければ足す。ほかの要素と共有するセットは、写しを作って、この要素だけに付け替える。型の要素では、型のセット（HasPropertySets）を変える
BuildingPropertyEdit = TypedDict(
    "BuildingPropertyEdit",
    {
        # 変更の種類
        "kind": Literal["property"],
        # 変える要素の ID。building_elements が返したもの
        "id": str,
        # 読んだときの要素のハッシュ。building_elements が返したもの
        "hash": str,
        # プロパティのセットの名前（Pset_WallCommon など）
        "set": str,
        # プロパティの名前
        "name": str,
        # 値。type_name は要る。kind は real・integer・text・boolean・logical のどれか
        "value": "BuildingValue",
    },
)


# 要素（IfcRoot）の名前か説明を変える
_BuildingAttributeEditRequired = TypedDict(
    "_BuildingAttributeEditRequired",
    {
        # 変更の種類
        "kind": Literal["attribute"],
        # 変える要素の ID。building_elements が返したもの
        "id": str,
        # 読んだときの要素のハッシュ。building_elements が返したもの
        "hash": str,
        # 変える属性
        "attribute": "BuildingAttribute",
    },
)
_BuildingAttributeEditOptional = TypedDict(
    "_BuildingAttributeEditOptional",
    {
        # 値。書かなければ、値をなくす
        "value": str,
    },
    total=False,
)


class BuildingAttributeEdit(_BuildingAttributeEditRequired, _BuildingAttributeEditOptional):
    "要素（IfcRoot）の名前か説明を変える"

# 変えられる IfcRoot の属性。name は Name、description は Description
BuildingAttribute = Union[
    Literal[
        "name",
        "description",
    ],
    str,
]


# 変更が指す要素。id と hash を書くか、added だけを書く
BuildingRef = TypedDict(
    "BuildingRef",
    {
        # 要素の ID。building_elements が返したもの
        "id": str,
        # 読んだときの要素のハッシュ。building_elements が返したもの
        "hash": str,
        # 同じ要求の edits の、前にある要素を足す変更（add_ で始まる種類）の位置（0 から）。その変更で足した要素を指す
        "added": int,
    },
    total=False,
)


# 要素の配置（IfcLocalPlacement）。relative_to の要素の配置からの相対で、書かなければプロジェクトの空間に置く
_BuildingPlacementChangeRequired = TypedDict(
    "_BuildingPlacementChangeRequired",
    {
        # 局所の変換。3 行 4 列（回転と移動。building_elements の placement の world と同じ形）。回転は正規直交で行列式が 1、移動は文書の長さの単位
        "matrix": List[List[float]],
    },
)
_BuildingPlacementChangeOptional = TypedDict(
    "_BuildingPlacementChangeOptional",
    {
        # 配置の親の要素
        "relative_to": "BuildingRef",
    },
    total=False,
)


class BuildingPlacementChange(_BuildingPlacementChangeRequired, _BuildingPlacementChangeOptional):
    "要素の配置（IfcLocalPlacement）。relative_to の要素の配置からの相対で、書かなければプロジェクトの空間に置く"

# 足す空間の種類。site は IfcSite、building は IfcBuilding、storey は IfcBuildingStorey
BuildingSpaceKind = Union[
    Literal[
        "site",
        "building",
        "storey",
    ],
    str,
]


# 敷地・建物・階を足し、IfcRelAggregates で親に結ぶ。結果の id は足した空間の ID。マイナーバージョン 96 から
_BuildingAddSpaceEditRequired = TypedDict(
    "_BuildingAddSpaceEditRequired",
    {
        # 変更の種類
        "kind": Literal["add_space"],
        # 空間の種類
        "space": "BuildingSpaceKind",
        # 親。敷地はプロジェクトか敷地、建物はプロジェクト・敷地・建物、階は建物か階。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "parent": "BuildingRef",
    },
)
_BuildingAddSpaceEditOptional = TypedDict(
    "_BuildingAddSpaceEditOptional",
    {
        # 名前
        "name": str,
        # 配置。書かなければ、親の配置に重ねる
        "placement": "BuildingPlacementChange",
        # 高さ。敷地は RefElevation、建物は ElevationOfRefHeight、階は Elevation に書く。文書の長さの単位
        "elevation": float,
        # GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
        "global_id": str,
    },
    total=False,
)


class BuildingAddSpaceEdit(_BuildingAddSpaceEditRequired, _BuildingAddSpaceEditOptional):
    "敷地・建物・階を足し、IfcRelAggregates で親に結ぶ。結果の id は足した空間の ID。マイナーバージョン 96 から"


# 要素を足し、container があれば IfcRelContainedInSpatialStructure で空間に結ぶ。足せるのは、壁・床・柱・梁・部材・板・被覆・手すり・屋根・階段・斜路・基礎・杭・扉・窓・開口・代理要素（IfcBuildingElementProxy）。結果の id は足した要素の ID。マイナーバージョン 96 から
_BuildingAddElementEditRequired = TypedDict(
    "_BuildingAddElementEditRequired",
    {
        # 変更の種類
        "kind": Literal["add_element"],
        # IFC の実体の名前（IfcWall など）。文書のバージョンにあるもの
        "class": str,
    },
)
_BuildingAddElementEditOptional = TypedDict(
    "_BuildingAddElementEditOptional",
    {
        # PredefinedType の列挙の値（SOLIDWALL など）。バージョンが持たない実体では書けない
        "predefined": str,
        # 名前
        "name": str,
        # 包含する空間（敷地・建物・階・室）。開口には書けない。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "container": "BuildingRef",
        # 配置。書かなければ、空間の配置に重ねる（空間もなければ原点）
        "placement": "BuildingPlacementChange",
        # GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
        "global_id": str,
    },
    total=False,
)


class BuildingAddElementEdit(_BuildingAddElementEditRequired, _BuildingAddElementEditOptional):
    "要素を足し、container があれば IfcRelContainedInSpatialStructure で空間に結ぶ。足せるのは、壁・床・柱・梁・部材・板・被覆・手すり・屋根・階段・斜路・基礎・杭・扉・窓・開口・代理要素（IfcBuildingElementProxy）。結果の id は足した要素の ID。マイナーバージョン 96 から"


# 要素を除く。関係からも外し、空になった関係を除く。壁の開口は一緒に除く。子のある空間と、関係のほかから指される要素は除けない。結果は id を持たない。マイナーバージョン 96 から
BuildingRemoveEdit = TypedDict(
    "BuildingRemoveEdit",
    {
        # 変更の種類
        "kind": Literal["remove"],
        # 除く要素の ID。building_elements が返したもの
        "id": str,
        # 読んだときの要素のハッシュ。building_elements が返したもの
        "hash": str,
    },
)


# 要素の配置を置き直す。ほかの要素と共有しない配置はその場で書き換え、共有する配置は新しく作る。配置の親をたどって輪になる配置は断る。マイナーバージョン 96 から
BuildingPlaceEdit = TypedDict(
    "BuildingPlaceEdit",
    {
        # 変更の種類
        "kind": Literal["place"],
        # 置き直す要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "element": "BuildingRef",
        # 新しい配置
        "placement": "BuildingPlacementChange",
    },
)


# 要素を空間に移す。空間は IfcRelAggregates、要素は IfcRelContainedInSpatialStructure で結び、前の関係から外す。マイナーバージョン 96 から
BuildingContainEdit = TypedDict(
    "BuildingContainEdit",
    {
        # 変更の種類
        "kind": Literal["contain"],
        # 移す要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "element": "BuildingRef",
        # 移す先の空間。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "space": "BuildingRef",
    },
)


# 要素の型を IfcRelDefinesByType で付け替える。element_type を書かなければ、型を外す。マイナーバージョン 96 から
_BuildingTypeEditRequired = TypedDict(
    "_BuildingTypeEditRequired",
    {
        # 変更の種類
        "kind": Literal["type"],
        # 型を変える要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "element": "BuildingRef",
    },
)
_BuildingTypeEditOptional = TypedDict(
    "_BuildingTypeEditOptional",
    {
        # 型の要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "element_type": "BuildingRef",
    },
    total=False,
)


class BuildingTypeEdit(_BuildingTypeEditRequired, _BuildingTypeEditOptional):
    "要素の型を IfcRelDefinesByType で付け替える。element_type を書かなければ、型を外す。マイナーバージョン 96 から"


# 開口（IfcOpeningElement）を要素に IfcRelVoidsElement で結ぶ。1 つの開口は 1 つの要素だけに結ぶ。マイナーバージョン 96 から
BuildingVoidEdit = TypedDict(
    "BuildingVoidEdit",
    {
        # 変更の種類
        "kind": Literal["void"],
        # 開口を持つ要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "host": "BuildingRef",
        # 開口。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "opening": "BuildingRef",
    },
)


# 材料の層
BuildingWallLayer = TypedDict(
    "BuildingWallLayer",
    {
        # 材料の名前（IfcMaterial の Name）。同じ要素の中で同じ名前の層は、同じ材料を指す
        "material": str,
        # 層の厚さ。文書の長さの単位で、正の値
        "thickness": float,
    },
)


# 芯の 2 点・高さ・材料の層で壁を足し、空間に IfcRelContainedInSpatialStructure で結ぶ。配置（芯の始点を原点、芯の向きを x 軸）・Axis と Body の表現・IfcMaterialLayerSetUsage はコアが書く。IFC2X3 は IfcWallStandardCase、IFC4 と IFC4X3 は IfcWall。結果の id は足した壁の ID。マイナーバージョン 97 から
_BuildingAddWallEditRequired = TypedDict(
    "_BuildingAddWallEditRequired",
    {
        # 変更の種類
        "kind": Literal["add_wall"],
        # 所属する空間（階など）。壁はこの空間の配置からの相対に置く。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "storey": "BuildingRef",
        # 芯の始点。空間の配置の [x, y] で、文書の長さの単位
        "start": List[float],
        # 芯の終点。始点と違う [x, y]
        "end": List[float],
        # 高さ。文書の長さの単位で、正の値
        "height": float,
        # 材料の層。始点から終点を見て、芯の右から左へ並べる。1 つ以上
        "layers": List["BuildingWallLayer"],
    },
)
_BuildingAddWallEditOptional = TypedDict(
    "_BuildingAddWallEditOptional",
    {
        # 芯から最初の層の面までのずれ（OffsetFromReferenceLine、左が正）。書かなければ厚さの合計の半分を負にして、芯を厚さの中央にする
        "offset": float,
        # 名前
        "name": str,
        # GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
        "global_id": str,
    },
    total=False,
)


class BuildingAddWallEdit(_BuildingAddWallEditRequired, _BuildingAddWallEditOptional):
    "芯の 2 点・高さ・材料の層で壁を足し、空間に IfcRelContainedInSpatialStructure で結ぶ。配置（芯の始点を原点、芯の向きを x 軸）・Axis と Body の表現・IfcMaterialLayerSetUsage はコアが書く。IFC2X3 は IfcWallStandardCase、IFC4 と IFC4X3 は IfcWall。結果の id は足した壁の ID。マイナーバージョン 97 から"


# 矩形の断面
BuildingSection = TypedDict(
    "BuildingSection",
    {
        # 幅。文書の長さの単位で、正の値
        "width": float,
        # せい。文書の長さの単位で、正の値
        "depth": float,
    },
)


# 外形・材料の層で床を足し、空間に包含で結ぶ。上の面を配置の原点にし、層の厚さの合計だけ下へ押し出す Body と、IfcSlab の FLOOR、IfcMaterialLayerSetUsage（AXIS3・NEGATIVE）をコアが書く。結果の id は足した床の ID。マイナーバージョン 98 から
_BuildingAddSlabEditRequired = TypedDict(
    "_BuildingAddSlabEditRequired",
    {
        # 変更の種類
        "kind": Literal["add_slab"],
        # 所属する空間（階など）。床はこの空間の配置からの相対に置く。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "storey": "BuildingRef",
        # 外形の頂点。空間の配置の [x, y] の並びで、3 つ以上。最後の点が最初の点と同じなら、その点は除く
        "outline": List[List[float]],
        # 材料の層。上から下へ並べる。1 つ以上
        "layers": List["BuildingWallLayer"],
    },
)
_BuildingAddSlabEditOptional = TypedDict(
    "_BuildingAddSlabEditOptional",
    {
        # 上の面の高さ（空間の配置の z）。書かなければ 0
        "elevation": float,
        # 名前
        "name": str,
        # GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
        "global_id": str,
    },
    total=False,
)


class BuildingAddSlabEdit(_BuildingAddSlabEditRequired, _BuildingAddSlabEditOptional):
    "外形・材料の層で床を足し、空間に包含で結ぶ。上の面を配置の原点にし、層の厚さの合計だけ下へ押し出す Body と、IfcSlab の FLOOR、IfcMaterialLayerSetUsage（AXIS3・NEGATIVE）をコアが書く。結果の id は足した床の ID。マイナーバージョン 98 から"


# 位置・高さ・矩形の断面で柱を足し、空間に包含で結ぶ。断面の中心を配置の原点にして上へ押し出す Body と IfcColumn をコアが書く。結果の id は足した柱の ID。マイナーバージョン 98 から
_BuildingAddColumnEditRequired = TypedDict(
    "_BuildingAddColumnEditRequired",
    {
        # 変更の種類
        "kind": Literal["add_column"],
        # 所属する空間（階など）。柱はこの空間の配置からの相対に置く。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "storey": "BuildingRef",
        # 断面の中心。空間の配置の [x, y]
        "position": List[float],
        # 高さ。文書の長さの単位で、正の値
        "height": float,
        # 断面。幅は柱の x 軸、せいは y 軸の向き
        "section": "BuildingSection",
    },
)
_BuildingAddColumnEditOptional = TypedDict(
    "_BuildingAddColumnEditOptional",
    {
        # 柱の脚の高さ（空間の配置の z）。書かなければ 0
        "base": float,
        # 柱の x 軸の向き。空間の配置の [x, y]。書かなければ空間の x 軸
        "direction": List[float],
        # 材料の名前（IfcMaterial の Name）。書けば IfcRelAssociatesMaterial で結ぶ
        "material": str,
        # 名前
        "name": str,
        # GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
        "global_id": str,
    },
    total=False,
)


class BuildingAddColumnEdit(_BuildingAddColumnEditRequired, _BuildingAddColumnEditOptional):
    "位置・高さ・矩形の断面で柱を足し、空間に包含で結ぶ。断面の中心を配置の原点にして上へ押し出す Body と IfcColumn をコアが書く。結果の id は足した柱の ID。マイナーバージョン 98 から"


# 天端の芯の 2 点・矩形の断面で梁を足し、空間に包含で結ぶ。始点を配置の原点、芯の向きを x 軸、水平で芯の左を y 軸にし、断面を芯に沿って押し出す Body と IfcBeam をコアが書く。鉛直の芯は断る。結果の id は足した梁の ID。マイナーバージョン 98 から
_BuildingAddBeamEditRequired = TypedDict(
    "_BuildingAddBeamEditRequired",
    {
        # 変更の種類
        "kind": Literal["add_beam"],
        # 所属する空間（階など）。梁はこの空間の配置からの相対に置く。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "storey": "BuildingRef",
        # 天端の芯の始点。空間の配置の [x, y, z]
        "start": List[float],
        # 天端の芯の終点。始点と x か y が違う [x, y, z]
        "end": List[float],
        # 断面。幅は芯の左右、せいは天端から下へ
        "section": "BuildingSection",
    },
)
_BuildingAddBeamEditOptional = TypedDict(
    "_BuildingAddBeamEditOptional",
    {
        # 材料の名前（IfcMaterial の Name）。書けば IfcRelAssociatesMaterial で結ぶ
        "material": str,
        # 名前
        "name": str,
        # GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
        "global_id": str,
    },
    total=False,
)


class BuildingAddBeamEdit(_BuildingAddBeamEditRequired, _BuildingAddBeamEditOptional):
    "天端の芯の 2 点・矩形の断面で梁を足し、空間に包含で結ぶ。始点を配置の原点、芯の向きを x 軸、水平で芯の左を y 軸にし、断面を芯に沿って押し出す Body と IfcBeam をコアが書く。鉛直の芯は断る。結果の id は足した梁の ID。マイナーバージョン 98 から"


# 壁の芯に沿う位置・下端・矩形の大きさで開口を足し、IfcRelVoidsElement で壁に穴をあける。開口は壁の配置からの相対に置き、壁の材料の層の使い方（AXIS2）の厚さだけ押し出す Body と IfcOpeningElement をコアが書く。空間には所属させない。層の使い方を持たない要素は断る。結果の id は足した開口の ID。マイナーバージョン 98 から
_BuildingAddOpeningEditRequired = TypedDict(
    "_BuildingAddOpeningEditRequired",
    {
        # 変更の種類
        "kind": Literal["add_opening"],
        # 穴をあける壁。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "host": "BuildingRef",
        # 壁の配置の x 軸に沿う、壁の原点から開口の端までの距離
        "distance": float,
        # 壁の下端から開口の下端までの高さ
        "sill": float,
        # 幅。文書の長さの単位で、正の値
        "width": float,
        # 高さ。文書の長さの単位で、正の値
        "height": float,
    },
)
_BuildingAddOpeningEditOptional = TypedDict(
    "_BuildingAddOpeningEditOptional",
    {
        # 名前
        "name": str,
        # GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
        "global_id": str,
    },
    total=False,
)


class BuildingAddOpeningEdit(_BuildingAddOpeningEditRequired, _BuildingAddOpeningEditOptional):
    "壁の芯に沿う位置・下端・矩形の大きさで開口を足し、IfcRelVoidsElement で壁に穴をあける。開口は壁の配置からの相対に置き、壁の材料の層の使い方（AXIS2）の厚さだけ押し出す Body と IfcOpeningElement をコアが書く。空間には所属させない。層の使い方を持たない要素は断る。結果の id は足した開口の ID。マイナーバージョン 98 から"


# 開口を扉で埋める。扉は開口の配置に置き、IfcDoor（IFC4 と IFC4X3 は PredefinedType の DOOR）を IfcRelFillsElement で開口に、包含で壁と同じ空間に結ぶ。形と型はまだ書かない。もう埋まった開口は断る。結果の id は足した扉の ID。マイナーバージョン 98 から
_BuildingAddDoorEditRequired = TypedDict(
    "_BuildingAddDoorEditRequired",
    {
        # 変更の種類
        "kind": Literal["add_door"],
        # 埋める開口（IfcOpeningElement）。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "opening": "BuildingRef",
    },
)
_BuildingAddDoorEditOptional = TypedDict(
    "_BuildingAddDoorEditOptional",
    {
        # 全体の幅（OverallWidth）。文書の長さの単位で、正の値
        "width": float,
        # 全体の高さ（OverallHeight）。文書の長さの単位で、正の値
        "height": float,
        # 名前
        "name": str,
        # GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
        "global_id": str,
    },
    total=False,
)


class BuildingAddDoorEdit(_BuildingAddDoorEditRequired, _BuildingAddDoorEditOptional):
    "開口を扉で埋める。扉は開口の配置に置き、IfcDoor（IFC4 と IFC4X3 は PredefinedType の DOOR）を IfcRelFillsElement で開口に、包含で壁と同じ空間に結ぶ。形と型はまだ書かない。もう埋まった開口は断る。結果の id は足した扉の ID。マイナーバージョン 98 から"


# 開口を窓で埋める。窓は開口の配置に置き、IfcWindow（IFC4 と IFC4X3 は PredefinedType の WINDOW）を IfcRelFillsElement で開口に、包含で壁と同じ空間に結ぶ。形と型はまだ書かない。もう埋まった開口は断る。結果の id は足した窓の ID。マイナーバージョン 98 から
_BuildingAddWindowEditRequired = TypedDict(
    "_BuildingAddWindowEditRequired",
    {
        # 変更の種類
        "kind": Literal["add_window"],
        # 埋める開口（IfcOpeningElement）。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "opening": "BuildingRef",
    },
)
_BuildingAddWindowEditOptional = TypedDict(
    "_BuildingAddWindowEditOptional",
    {
        # 全体の幅（OverallWidth）。文書の長さの単位で、正の値
        "width": float,
        # 全体の高さ（OverallHeight）。文書の長さの単位で、正の値
        "height": float,
        # 名前
        "name": str,
        # GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
        "global_id": str,
    },
    total=False,
)


class BuildingAddWindowEdit(_BuildingAddWindowEditRequired, _BuildingAddWindowEditOptional):
    "開口を窓で埋める。窓は開口の配置に置き、IfcWindow（IFC4 と IFC4X3 は PredefinedType の WINDOW）を IfcRelFillsElement で開口に、包含で壁と同じ空間に結ぶ。形と型はまだ書かない。もう埋まった開口は断る。結果の id は足した窓の ID。マイナーバージョン 98 から"


# 通り芯の軸。2 点の折れ線の IfcGridAxis に書く。マイナーバージョン 101 から
BuildingGridAxis = TypedDict(
    "BuildingGridAxis",
    {
        # 軸の名前（AxisTag）。同じ通り芯の中で重ならない
        "tag": str,
        # 始点。通り芯の配置の [x, y]
        "start": List[float],
        # 終点。始点と違う [x, y]
        "end": List[float],
    },
)


# U と V の軸で通り芯（IfcGrid）を足し、空間に包含で結ぶ。通り芯は空間の配置からの相対に置く。U か V が空・同じ名前の軸・始点と終点が同じ軸は断る。結果の id は足した通り芯の ID。マイナーバージョン 101 から
_BuildingAddGridEditRequired = TypedDict(
    "_BuildingAddGridEditRequired",
    {
        # 変更の種類
        "kind": Literal["add_grid"],
        # 所属する空間（階など）。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "storey": "BuildingRef",
        # U の軸。1 つ以上
        "u": List["BuildingGridAxis"],
        # V の軸。1 つ以上
        "v": List["BuildingGridAxis"],
    },
)
_BuildingAddGridEditOptional = TypedDict(
    "_BuildingAddGridEditOptional",
    {
        # 高さ（空間の配置の z）。書かなければ 0
        "elevation": float,
        # 名前
        "name": str,
        # GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
        "global_id": str,
    },
    total=False,
)


class BuildingAddGridEdit(_BuildingAddGridEditRequired, _BuildingAddGridEditOptional):
    "U と V の軸で通り芯（IfcGrid）を足し、空間に包含で結ぶ。通り芯は空間の配置からの相対に置く。U か V が空・同じ名前の軸・始点と終点が同じ軸は断る。結果の id は足した通り芯の ID。マイナーバージョン 101 から"


# 階の高さ（IfcBuildingStorey の Elevation）を直し、階の配置の z を高さの差だけ動かす。今の高さがなければ、配置の z を今の高さとみなす。階に属する要素は配置の相対で付いてくる。結果の id は直した階の ID。マイナーバージョン 103 から
BuildingElevationEdit = TypedDict(
    "BuildingElevationEdit",
    {
        # 変更の種類
        "kind": Literal["elevation"],
        # 直す階。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "storey": "BuildingRef",
        # 新しい高さ。文書の長さの単位
        "elevation": float,
    },
)


# 壁の芯・高さ・材料の層を、add_wall と同じ値で直す。配置・表現・材料の層の使い方を書き直し、同じ名前の層は元の IfcMaterial を使う。開口と建具は壁の配置からの相対のまま付いてきて、開口の Body の押し出しを新しい層の範囲に合わせる。合わせられない開口があれば断る。結果の id は直した壁の ID。マイナーバージョン 103 から
_BuildingWallShapeEditRequired = TypedDict(
    "_BuildingWallShapeEditRequired",
    {
        # 変更の種類
        "kind": Literal["wall_shape"],
        # 直す壁（IfcWall か IfcWallStandardCase）。材料の層の使い方（AXIS2）を持つもの。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "wall": "BuildingRef",
        # 芯の始点。壁の配置の親の配置の [x, y]
        "start": List[float],
        # 芯の終点。始点と違う [x, y]
        "end": List[float],
        # 高さ。文書の長さの単位で、正の値
        "height": float,
        # 材料の層。始点から終点を見て、芯の右から左へ並べる。1 つ以上
        "layers": List["BuildingWallLayer"],
    },
)
_BuildingWallShapeEditOptional = TypedDict(
    "_BuildingWallShapeEditOptional",
    {
        # 芯から最初の層の面までのずれ（左が正）。書かなければ厚さの合計の半分を負にして、芯を壁の厚さの中央にする
        "offset": float,
    },
    total=False,
)


class BuildingWallShapeEdit(_BuildingWallShapeEditRequired, _BuildingWallShapeEditOptional):
    "壁の芯・高さ・材料の層を、add_wall と同じ値で直す。配置・表現・材料の層の使い方を書き直し、同じ名前の層は元の IfcMaterial を使う。開口と建具は壁の配置からの相対のまま付いてきて、開口の Body の押し出しを新しい層の範囲に合わせる。合わせられない開口があれば断る。結果の id は直した壁の ID。マイナーバージョン 103 から"


# 開口の位置と大きさを、add_opening と同じ値で直す。配置と Body を書き直し、埋める扉と窓の全体の高さと幅（OverallHeight・OverallWidth）は、書いてあれば合わせる。結果の id は直した開口の ID。マイナーバージョン 103 から
BuildingOpeningShapeEdit = TypedDict(
    "BuildingOpeningShapeEdit",
    {
        # 変更の種類
        "kind": Literal["opening_shape"],
        # 直す開口（IfcOpeningElement）。材料の層の使い方を持つ要素に穴をあけるもの。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "opening": "BuildingRef",
        # 穴をあけた要素の配置の x 軸に沿う、要素の原点から開口の端までの距離
        "distance": float,
        # 要素の下端から開口の下端までの高さ
        "sill": float,
        # 幅。文書の長さの単位で、正の値
        "width": float,
        # 高さ。文書の長さの単位で、正の値
        "height": float,
    },
)


# 型を足す。配置と表現は持たず、要素とは type で結ぶ。足せるのは、壁・床・柱・梁・部材・板・被覆・手すり・代理要素の型と、IFC4 からの屋根・階段・斜路・基礎・杭・扉・窓の型。PredefinedType を書かなければ NOTDEFINED を書き、扉と窓の型の OperationType・PartitioningType も NOTDEFINED にする。結果の id は足した型の ID。マイナーバージョン 106 から
_BuildingAddTypeEditRequired = TypedDict(
    "_BuildingAddTypeEditRequired",
    {
        # 変更の種類
        "kind": Literal["add_type"],
        # IFC の型の実体の名前（IfcWallType など）。文書のバージョンにあるもの
        "class": str,
    },
)
_BuildingAddTypeEditOptional = TypedDict(
    "_BuildingAddTypeEditOptional",
    {
        # PredefinedType の列挙の値（SOLIDWALL など）
        "predefined": str,
        # 利用者が決めた種類（ElementType）
        "element_type": str,
        # 名前
        "name": str,
        # GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
        "global_id": str,
    },
    total=False,
)


class BuildingAddTypeEdit(_BuildingAddTypeEditRequired, _BuildingAddTypeEditOptional):
    "型を足す。配置と表現は持たず、要素とは type で結ぶ。足せるのは、壁・床・柱・梁・部材・板・被覆・手すり・代理要素の型と、IFC4 からの屋根・階段・斜路・基礎・杭・扉・窓の型。PredefinedType を書かなければ NOTDEFINED を書き、扉と窓の型の OperationType・PartitioningType も NOTDEFINED にする。結果の id は足した型の ID。マイナーバージョン 106 から"


# 要素の数量のセット（IfcElementQuantity）の数量の値を変える。セットか数量がなければ足す。ほかの要素と共有するセットは、写しを作って、この要素だけに付け替える。型の要素では、型のセット（HasPropertySets）を変える。単位は書かず、文書の単位の割り当てに従う。結果の id は要素の ID。マイナーバージョン 106 から
_BuildingQuantityEditRequired = TypedDict(
    "_BuildingQuantityEditRequired",
    {
        # 変更の種類
        "kind": Literal["quantity"],
        # 変える要素の ID。building_elements が返したもの
        "id": str,
        # 読んだときの要素のハッシュ。building_elements が返したもの
        "hash": str,
        # 数量のセットの名前（Qto_WallBaseQuantities など）
        "set": str,
        # 数量の名前（Length など）
        "name": str,
        # 数量の種類。IfcQuantityLength・IfcQuantityArea・IfcQuantityVolume・IfcQuantityCount・IfcQuantityWeight・IfcQuantityTime のどれを書くか
        "quantity": "QuantityKind",
        # 値。文書の単位で、有限の負でない値。IFC4X3 の個数は整数
        "value": float,
    },
)
_BuildingQuantityEditOptional = TypedDict(
    "_BuildingQuantityEditOptional",
    {
        # セットを足すときの測り方（MethodOfMeasurement）
        "method": str,
        # 式（Formula）。IFC2X3 は持たないので、書けば断る
        "formula": str,
    },
    total=False,
)


class BuildingQuantityEdit(_BuildingQuantityEditRequired, _BuildingQuantityEditOptional):
    "要素の数量のセット（IfcElementQuantity）の数量の値を変える。セットか数量がなければ足す。ほかの要素と共有するセットは、写しを作って、この要素だけに付け替える。型の要素では、型のセット（HasPropertySets）を変える。単位は書かず、文書の単位の割り当てに従う。結果の id は要素の ID。マイナーバージョン 106 から"


# プロジェクトの単位の割り当て（IfcUnitAssignment）に SI の単位（IfcSIUnit）を足す。長さと平面角の単位は受けず、同じ種類の単位があれば断る。結果は id を持たない。マイナーバージョン 106 から
_BuildingAddUnitEditRequired = TypedDict(
    "_BuildingAddUnitEditRequired",
    {
        # 変更の種類
        "kind": Literal["add_unit"],
        # 単位の種類（IfcUnitEnum の AREAUNIT など）
        "unit": str,
        # SI の単位の名前（IfcSIUnitName の SQUARE_METRE など）。単位の種類に合うもの
        "name": str,
    },
)
_BuildingAddUnitEditOptional = TypedDict(
    "_BuildingAddUnitEditOptional",
    {
        # SI の接頭辞（IfcSIPrefix の MILLI など）
        "prefix": str,
    },
    total=False,
)


class BuildingAddUnitEdit(_BuildingAddUnitEditRequired, _BuildingAddUnitEditOptional):
    "プロジェクトの単位の割り当て（IfcUnitAssignment）に SI の単位（IfcSIUnit）を足す。長さと平面角の単位は受けず、同じ種類の単位があれば断る。結果は id を持たない。マイナーバージョン 106 から"


# 要素か型に材料の構成を IfcRelAssociatesMaterial で結ぶ。結んであった材料は外す。同じ名前の材料（IfcMaterial）は文書のものを使う。結果の id は要素の ID。マイナーバージョン 106 から
BuildingMaterialEdit = TypedDict(
    "BuildingMaterialEdit",
    {
        # 変更の種類
        "kind": Literal["material"],
        # 材料を結ぶ要素か型。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "element": "BuildingRef",
        # 材料の構成
        "material": "BuildingMaterialChange",
    },
)


# 結ぶ材料の構成。kind に合うフィールドだけを書く。single は name、list は names、layers は layers と usage、constituents は constituents
_BuildingMaterialChangeRequired = TypedDict(
    "_BuildingMaterialChangeRequired",
    {
        # 構成の種類
        "kind": "BuildingMaterialChangeKind",
    },
)
_BuildingMaterialChangeOptional = TypedDict(
    "_BuildingMaterialChangeOptional",
    {
        # 単一の材料の名前
        "name": str,
        # 材料の並びの名前。1 つ以上
        "names": List[str],
        # 材料の層。1 つ以上
        "layers": List["BuildingMaterialLayer"],
        # 層の使い方（IfcMaterialLayerSetUsage）。書かなければ、層の組（IfcMaterialLayerSet）をそのまま結ぶ
        "usage": "BuildingLayerUsage",
        # 構成要素。1 つ以上
        "constituents": List["BuildingMaterialConstituent"],
    },
    total=False,
)


class BuildingMaterialChange(_BuildingMaterialChangeRequired, _BuildingMaterialChangeOptional):
    "結ぶ材料の構成。kind に合うフィールドだけを書く。single は name、list は names、layers は layers と usage、constituents は constituents"

# 結ぶ材料の構成の種類。single は IfcMaterial、list は IfcMaterialList（IFC2X3 だけ。IFC4 からは非推奨なので断る）、layers は IfcMaterialLayerSet、constituents は IfcMaterialConstituentSet（IFC4 から）
BuildingMaterialChangeKind = Union[
    Literal[
        "single",
        "list",
        "layers",
        "constituents",
    ],
    str,
]


# 材料の層
_BuildingMaterialLayerRequired = TypedDict(
    "_BuildingMaterialLayerRequired",
    {
        # 層の厚さ。文書の長さの単位で、IFC2X3 は正、IFC4 からは負でない値
        "thickness": float,
    },
)
_BuildingMaterialLayerOptional = TypedDict(
    "_BuildingMaterialLayerOptional",
    {
        # 材料の名前。書かなければ、材料のない層（空気層など）
        "material": str,
        # 層の名前。IFC2X3 は持たないので、書けば断る
        "name": str,
    },
    total=False,
)


class BuildingMaterialLayer(_BuildingMaterialLayerRequired, _BuildingMaterialLayerOptional):
    "材料の層"


# 材料の層の使い方
_BuildingLayerUsageRequired = TypedDict(
    "_BuildingLayerUsageRequired",
    {
        # 層を重ねる向き
        "direction": "BuildingLayerDirection",
        # 基準線からのずれ。文書の長さの単位
        "offset": float,
    },
)
_BuildingLayerUsageOptional = TypedDict(
    "_BuildingLayerUsageOptional",
    {
        # true なら負の向きに重ねる（NEGATIVE）。書かなければ正の向き
        "negative": bool,
    },
    total=False,
)


class BuildingLayerUsage(_BuildingLayerUsageRequired, _BuildingLayerUsageOptional):
    "材料の層の使い方"

# 層を重ねる向き（IfcLayerSetDirectionEnum）。axis1 は AXIS1、axis2 は AXIS2、axis3 は AXIS3
BuildingLayerDirection = Union[
    Literal[
        "axis1",
        "axis2",
        "axis3",
    ],
    str,
]


# 材料の構成要素
_BuildingMaterialConstituentRequired = TypedDict(
    "_BuildingMaterialConstituentRequired",
    {
        # 材料の名前
        "material": str,
    },
)
_BuildingMaterialConstituentOptional = TypedDict(
    "_BuildingMaterialConstituentOptional",
    {
        # 構成要素の名前
        "name": str,
    },
    total=False,
)


class BuildingMaterialConstituent(_BuildingMaterialConstituentRequired, _BuildingMaterialConstituentOptional):
    "材料の構成要素"


# 要素の形（Axis と Body の表現）を置き換える。項目は items に並べ、Body に置く項目を body の位置で挙げる。集合演算と写像は、items の前にある項目を位置で指し、同じ項目を何度指してもよい（それぞれに写して書く）。表現の種類（SweptSolid・AdvancedSweptSolid・Brep・SolidModel・Tessellation・Clipping・CSG・MappedRepresentation）はコアが決め、決まらない組み合わせ・表現に直接置く半空間・16 段を超える入れ子・写して 10,000 項目を超える Body は断る。使われなくなった元の表現は除く。要素は配置を持つこと。結果の id は要素の ID。マイナーバージョン 107 から
_BuildingShapeEditRequired = TypedDict(
    "_BuildingShapeEditRequired",
    {
        # 変更の種類
        "kind": Literal["shape"],
        # 形を置き換える要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "element": "BuildingRef",
        # 形の項目の表
        "items": List["BuildingShapeItem"],
        # Body の表現に置く項目の、items の位置（0 から）。空なら Body を書かない
        "body": List[int],
    },
)
_BuildingShapeEditOptional = TypedDict(
    "_BuildingShapeEditOptional",
    {
        # 芯の線（Axis の IfcPolyline）。要素の配置の [x, y] を 2 点以上。書かなければ Axis を書かない
        "axis": List[List[float]],
    },
    total=False,
)


class BuildingShapeEdit(_BuildingShapeEditRequired, _BuildingShapeEditOptional):
    "要素の形（Axis と Body の表現）を置き換える。項目は items に並べ、Body に置く項目を body の位置で挙げる。集合演算と写像は、items の前にある項目を位置で指し、同じ項目を何度指してもよい（それぞれに写して書く）。表現の種類（SweptSolid・AdvancedSweptSolid・Brep・SolidModel・Tessellation・Clipping・CSG・MappedRepresentation）はコアが決め、決まらない組み合わせ・表現に直接置く半空間・16 段を超える入れ子・写して 10,000 項目を超える Body は断る。使われなくなった元の表現は除く。要素は配置を持つこと。結果の id は要素の ID。マイナーバージョン 107 から"


# 形の項目。kind に合うフィールドだけを書く。extrusion は profile・position・direction・depth、revolution は profile・position・axis・angle、swept_disk は directrix・radius・inner_radius・start・end、faces は points・faces・closed、boolean は operation・first・second、half_space は plane・agreement・box か polygon、mapped は origin・target・members。長さは文書の長さの単位
_BuildingShapeItemRequired = TypedDict(
    "_BuildingShapeItemRequired",
    {
        # 項目の種類
        "kind": "BuildingShapeItemKind",
    },
)
_BuildingShapeItemOptional = TypedDict(
    "_BuildingShapeItemOptional",
    {
        # 押し出す・回す断面
        "profile": "BuildingShapeProfile",
        # 押し出し・回転の局所の配置。書かなければ要素の配置のまま
        "position": "BuildingAxis3",
        # 局所の押し出す向き [x, y, z]。z は 0 にできない。書かなければ [0, 0, 1]
        "direction": List[float],
        # 押し出す深さ。正の値
        "depth": float,
        # 断面を回す軸。断面の平面（局所の XY 平面）にある。マイナーバージョン 108 から
        "axis": "BuildingShapeRevolutionAxis",
        # 回す角度（ラジアン）。0 より大きく 2π 以下。文書の平面角の単位に直して書くので、文書は平面角の単位を持つこと。マイナーバージョン 108 から
        "angle": float,
        # 円盤を沿わせる折れ線の [x, y, z] を 2 点以上。続く点は重ならない。マイナーバージョン 108 から
        "directrix": List[List[float]],
        # 円盤の半径。正の値。マイナーバージョン 108 から
        "radius": float,
        # 管の内側の半径。正で radius より小さい。書かなければ中実。マイナーバージョン 108 から
        "inner_radius": float,
        # 掃引を始める折れ線のパラメーター（1 区間で 1 進む）。書かなければ始点。IFC2X3 では書かなければ 0 を書く。マイナーバージョン 108 から
        "start": float,
        # 掃引を終える折れ線のパラメーター。start より大きく区間の数以下。書かなければ終点。IFC2X3 では書かなければ区間の数を書く。マイナーバージョン 108 から
        "end": float,
        # 面集合の点 [x, y, z]
        "points": List[List[float]],
        # 面集合の面
        "faces": List["BuildingShapeFace"],
        # true なら閉殻（立体。IfcFacetedBrep）。false か書かなければ開殻の面（IFC2X3 は IfcShellBasedSurfaceModel、IFC4 からは IfcPolygonalFaceSet）
        "closed": bool,
        # 集合演算
        "operation": "BuildingBooleanOperation",
        # 集合演算の第 1 の対象。items の前にある項目の位置
        "first": int,
        # 集合演算の第 2 の対象。差では第 1 から取り除く。items の前にある項目の位置
        "second": int,
        # 半空間の基準面の配置。局所の z 軸が面の法線
        "plane": "BuildingAxis3",
        # true なら法線と逆の側、false か書かなければ法線の向く側を内部とする
        "agreement": bool,
        # 半空間の計算用の囲みの箱（IfcBoxedHalfSpace）。座標系は IFC2X3 では基準面、IFC4 からは要素
        "box": "BuildingShapeBox",
        # 半空間を制限する多角形（IfcPolygonalBoundedHalfSpace）
        "polygon": "BuildingShapePolygon",
        # 写像元の基準位置。書かなければ原点
        "origin": "BuildingAxis3",
        # 写像先の変換
        "target": "BuildingShapeMapping",
        # 写像元の表現の項目。items の前にある項目の位置で、1 つ以上
        "members": List[int],
    },
    total=False,
)


class BuildingShapeItem(_BuildingShapeItemRequired, _BuildingShapeItemOptional):
    "形の項目。kind に合うフィールドだけを書く。extrusion は profile・position・direction・depth、revolution は profile・position・axis・angle、swept_disk は directrix・radius・inner_radius・start・end、faces は points・faces・closed、boolean は operation・first・second、half_space は plane・agreement・box か polygon、mapped は origin・target・members。長さは文書の長さの単位"

# 形の項目の種類。extrusion は断面の押し出し（IfcExtrudedAreaSolid）、faces は面集合、boolean は集合演算（IfcBooleanResult。差で第 2 が半空間なら IfcBooleanClippingResult）、half_space は平面の半空間（集合演算の対象にだけ置く）、mapped は写像（IfcMappedItem）。revolution は断面の回転（IfcRevolvedAreaSolid）、swept_disk は円盤の掃引（IfcSweptDiskSolid）で、マイナーバージョン 108 から
BuildingShapeItemKind = Union[
    Literal[
        "extrusion",
        "faces",
        "boolean",
        "half_space",
        "mapped",
        "revolution",
        "swept_disk",
    ],
    str,
]


# 断面を回す軸（IfcAxis1Placement）。断面の平面にある
BuildingShapeRevolutionAxis = TypedDict(
    "BuildingShapeRevolutionAxis",
    {
        # 軸の通る点 [x, y]
        "location": List[float],
        # 軸の向き [x, y]。0 にできない
        "direction": List[float],
    },
)

# 集合演算。union は和、intersection は積、difference は差
BuildingBooleanOperation = Union[
    Literal[
        "union",
        "intersection",
        "difference",
    ],
    str,
]


# 3 次元の局所の配置
_BuildingAxis3Required = TypedDict(
    "_BuildingAxis3Required",
    {
        # 原点 [x, y, z]
        "location": List[float],
    },
)
_BuildingAxis3Optional = TypedDict(
    "_BuildingAxis3Optional",
    {
        # 局所の z 軸 [x, y, z]
        "axis": List[float],
        # 局所の x 軸の参照方向 [x, y, z]
        "reference": List[float],
    },
    total=False,
)


class BuildingAxis3(_BuildingAxis3Required, _BuildingAxis3Optional):
    "3 次元の局所の配置"


# 押し出す断面。rectangle は x と y、circle は radius、polygon は outer と holes を書く
_BuildingShapeProfileRequired = TypedDict(
    "_BuildingShapeProfileRequired",
    {
        # 断面の種類
        "kind": "BuildingShapeProfileKind",
    },
)
_BuildingShapeProfileOptional = TypedDict(
    "_BuildingShapeProfileOptional",
    {
        # 断面の名前
        "name": str,
        # 矩形の局所の x の全幅。正の値
        "x": float,
        # 矩形の局所の y の全高。正の値
        "y": float,
        # 円の半径。正の値
        "radius": float,
        # 矩形と円の中心 [x, y]。書かなければ原点
        "location": List[float],
        # 矩形と円の局所の x 軸の参照方向 [x, y]。location と一緒に書く
        "reference": List[float],
        # 多角形の外輪の [x, y] を 3 点以上。始点に戻る点は書かない
        "outer": List[List[float]],
        # 多角形の内輪
        "holes": List[List[List[float]]],
    },
    total=False,
)


class BuildingShapeProfile(_BuildingShapeProfileRequired, _BuildingShapeProfileOptional):
    "押し出す断面。rectangle は x と y、circle は radius、polygon は outer と holes を書く"

# 断面の種類。rectangle は IfcRectangleProfileDef、circle は IfcCircleProfileDef、polygon は IfcArbitraryClosedProfileDef（内輪があれば IfcArbitraryProfileDefWithVoids）
BuildingShapeProfileKind = Union[
    Literal[
        "rectangle",
        "circle",
        "polygon",
    ],
    str,
]


# 面集合の面。points の位置（0 から）で外輪と内輪を書く
_BuildingShapeFaceRequired = TypedDict(
    "_BuildingShapeFaceRequired",
    {
        # 外輪の点の位置。3 つ以上
        "boundary": List[int],
    },
)
_BuildingShapeFaceOptional = TypedDict(
    "_BuildingShapeFaceOptional",
    {
        # 内輪ごとの点の位置
        "holes": List[List[int]],
    },
    total=False,
)


class BuildingShapeFace(_BuildingShapeFaceRequired, _BuildingShapeFaceOptional):
    "面集合の面。points の位置（0 から）で外輪と内輪を書く"


# 囲みの箱
BuildingShapeBox = TypedDict(
    "BuildingShapeBox",
    {
        # 箱の隅 [x, y, z]
        "corner": List[float],
        # 各軸の長さ [x, y, z]。正の値
        "dimensions": List[float],
    },
)


# 半空間を制限する多角形
BuildingShapePolygon = TypedDict(
    "BuildingShapePolygon",
    {
        # 境界の配置
        "position": "BuildingAxis3",
        # 境界の局所の XY 平面の [x, y] を 3 点以上。始点に戻る点は書かない
        "boundary": List[List[float]],
    },
)


# 写像先の変換（IfcCartesianTransformationOperator3D）。scales を書けば軸ごとの尺度（NonUniform）
_BuildingShapeMappingRequired = TypedDict(
    "_BuildingShapeMappingRequired",
    {
        # 写像先の原点 [x, y, z]。要素の座標系
        "origin": List[float],
    },
)
_BuildingShapeMappingOptional = TypedDict(
    "_BuildingShapeMappingOptional",
    {
        # x 軸を導く方向 [x, y, z]
        "axis1": List[float],
        # y 軸を導く方向 [x, y, z]
        "axis2": List[float],
        # z 軸を定める方向 [x, y, z]
        "axis3": List[float],
        # 一様な尺度。正の値で、書かなければ 1
        "scale": float,
        # 軸ごとの尺度 [x, y, z]。正の値。scale とは一緒に書けない
        "scales": List[float],
    },
    total=False,
)


class BuildingShapeMapping(_BuildingShapeMappingRequired, _BuildingShapeMappingOptional):
    "写像先の変換（IfcCartesianTransformationOperator3D）。scales を書けば軸ごとの尺度（NonUniform）"


# 扉や窓を開口（IfcOpeningElement）に IfcRelFillsElement で結ぶ。1 つの開口は 1 つの要素だけで埋める。マイナーバージョン 96 から
BuildingFillEdit = TypedDict(
    "BuildingFillEdit",
    {
        # 変更の種類
        "kind": Literal["fill"],
        # 開口。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "opening": "BuildingRef",
        # 埋める要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
        "filler": "BuildingRef",
    },
)


# 当てた変更の結果。書き戻したファイルは blobs の file で返す
BuildingApplyResult = TypedDict(
    "BuildingApplyResult",
    {
        # 変更ごとの結果。要求の edits と同じ順
        "applied": List["Applied"],
        # 書き戻した文書のハッシュ。次の building_apply の前提条件に使う
        "hash": str,
    },
)


# 開いた図面の文書を図面モデル（L2）に変換し、図面モデルのバイナリ（決定 0247）を blob で返す。配置は schema/drawing.toml にあり、元の文書の形式・系統・ハッシュも持つ。プラグインのモジュールが読んだ図面は、これを図面のモジュールの open に渡すと、元の形式とハッシュの読むだけの文書になる。マイナーバージョン 60 から
_LiftRequestRequired = TypedDict(
    "_LiftRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["lift"],
        # 文書の名前。open で図面を開いたもの
        "doc": str,
    },
)
_LiftRequestOptional = TypedDict(
    "_LiftRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
    },
    total=False,
)


class LiftRequest(_LiftRequestRequired, _LiftRequestOptional):
    "開いた図面の文書を図面モデル（L2）に変換し、図面モデルのバイナリ（決定 0247）を blob で返す。配置は schema/drawing.toml にあり、元の文書の形式・系統・ハッシュも持つ。プラグインのモジュールが読んだ図面は、これを図面のモジュールの open に渡すと、元の形式とハッシュの読むだけの文書になる。マイナーバージョン 60 から"


# 図面モデルのバイナリの blob の番号
LiftResult = TypedDict(
    "LiftResult",
    {
        # 図面モデルのバイナリの blob の番号（種類は drawing）。zm_take で取り出す
        "blob": int,
    },
)


# 開いた IFC の建物モデルを、zm_push で渡した IDS で照合する（決定 0217・0219）。確かめられない facet は、指摘と分けて info の指摘で返す。マイナーバージョン 44 から
_IdsCheckRequestRequired = TypedDict(
    "_IdsCheckRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # EXPRESS の入力の名前（zm_push で渡した .exp）。渡せば、IFC の属性の全体を EXPRESS の名前で照らす。渡さなければ、建物モデルのコンポーネントにある属性だけを照らす
        "express": str,
        # 返す最初の位置。前の応答の next_offset を渡す。書かなければ 0
        "offset": int,
        # 返す数の上限。書かなければ 1000
        "limit": int,
    },
    total=False,
)


class IdsCheckRequest(_IdsCheckRequestRequired, _IdsCheckRequestOptional):
    "開いた IFC の建物モデルを、zm_push で渡した IDS で照合する（決定 0217・0219）。確かめられない facet は、指摘と分けて info の指摘で返す。マイナーバージョン 44 から"


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


# 開いた IFC を ids_check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0218・0219）に足す。足した文書は blobs の file で返す。マイナーバージョン 44 から
_ReviewAdoptIdsRequestRequired = TypedDict(
    "_ReviewAdoptIdsRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # EXPRESS の入力の名前（zm_push で渡した .exp）。渡せば、IFC の属性の全体を EXPRESS の名前で照らす。渡さなければ、建物モデルのコンポーネントにある属性だけを照らす
        "express": str,
        # 足す先のレビューの文書の入力の名前（zm_push で渡した JSON）。書かなければ空の文書に足す
        "review": str,
    },
    total=False,
)


class ReviewAdoptIdsRequest(_ReviewAdoptIdsRequestRequired, _ReviewAdoptIdsRequestOptional):
    "開いた IFC を ids_check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0218・0219）に足す。足した文書は blobs の file で返す。マイナーバージョン 44 から"


# zm_push で渡した BCF 2.1 か 3.0 の bcfzip を読み、開いた IFC を対象にしたレビューの文書（決定 0214・0220）にする。topic は指摘に、comment はコメントに、viewpoint は 3D の場所に写し、要素の参照のハッシュは建物モデルの要素のハッシュで埋める。文書は blobs の file で返す。マイナーバージョン 45 から
_BcfReadRequestRequired = TypedDict(
    "_BcfReadRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 指摘の状態と BCF の TopicStatus の文字列の対応の表。書かなければ Open・Resolved・Closed。読むとき、表にない文字列は open にし、元の文字列を status_label に残す
        "statuses": List["BcfStatus"],
    },
    total=False,
)


class BcfReadRequest(_BcfReadRequestRequired, _BcfReadRequestOptional):
    "zm_push で渡した BCF 2.1 か 3.0 の bcfzip を読み、開いた IFC を対象にしたレビューの文書（決定 0214・0220）にする。topic は指摘に、comment はコメントに、viewpoint は 3D の場所に写し、要素の参照のハッシュは建物モデルの要素のハッシュで埋める。文書は blobs の file で返す。マイナーバージョン 45 から"


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
        # BCF のバージョン
        "version": str,
        # 指摘の数
        "topics": int,
    },
)


# zm_push で渡したレビューの文書を、BCF の bcfzip に書く（決定 0214・0220）。バージョンは version で選ぶ（マイナーバージョン 61 から。それまでは 3.0 だけ）。target を対象にした 3D の場所を viewpoint にし、2D の場所と図形は書かずに診断する。original を渡せば、変えていない topic を元のバイト列のまま書き、変えた topic でも写さない要素と知らない要素を保つ。bcfzip は blobs の file で返す。マイナーバージョン 45 から
_BcfWriteRequestRequired = TypedDict(
    "_BcfWriteRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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
        # 書く BCF のバージョン。2.1 か 3.0。書かなければ、original のバージョンか 3.0。original と違うバージョンでは書かず、bcf.conversion.unsupported で失敗する。ほかの値は call.value.invalid。マイナーバージョン 61 から
        "version": str,
    },
    total=False,
)


class BcfWriteRequest(_BcfWriteRequestRequired, _BcfWriteRequestOptional):
    "zm_push で渡したレビューの文書を、BCF の bcfzip に書く（決定 0214・0220）。バージョンは version で選ぶ（マイナーバージョン 61 から。それまでは 3.0 だけ）。target を対象にした 3D の場所を viewpoint にし、2D の場所と図形は書かずに診断する。original を渡せば、変えていない topic を元のバイト列のまま書き、変えた topic でも写さない要素と知らない要素を保つ。bcfzip は blobs の file で返す。マイナーバージョン 45 から"


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


# レイヤの表の項目（決定 0162）。マイナーバージョン 22 から、状態とスタイルを持つ
_LayerItemRequired = TypedDict(
    "_LayerItemRequired",
    {
        # ID。表のレコードがハンドルを持てば layer/h10、なければ読んだ順の番号（layer/2）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
        "id": str,
        # 元の名前。翻訳しない
        "name": str,
        # 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
        "hash": str,
        # ファイルに定義がある。偽なら、要素が名前で指すだけのレイヤで、ほかのフィールドを持たない。マイナーバージョン 22 から
        "defined": bool,
    },
)
_LayerItemOptional = TypedDict(
    "_LayerItemOptional",
    {
        # 表示する。マイナーバージョン 22 から
        "visible": bool,
        # 印刷する。マイナーバージョン 22 から
        "printable": bool,
        # 編集できる。マイナーバージョン 22 から
        "editable": bool,
        # 凍結する。マイナーバージョン 22 から
        "frozen": bool,
        # 色。inherit を持たない。定義が色を持たなければ書かない。マイナーバージョン 22 から
        "color": "ColorChange",
        # 線種の ID。定義が線種を持たなければ書かない。マイナーバージョン 22 から
        "line_type_id": str,
        # 線幅。inherit を持たない。定義が線幅を持たなければ書かない。マイナーバージョン 22 から
        "line_weight": "LineWeightChange",
    },
    total=False,
)


class LayerItem(_LayerItemRequired, _LayerItemOptional):
    "レイヤの表の項目（決定 0162）。マイナーバージョン 22 から、状態とスタイルを持つ"


# 線種の表の項目（決定 0164）。マイナーバージョン 26 から、定義を持つ
_LineTypeItemRequired = TypedDict(
    "_LineTypeItemRequired",
    {
        # ID。表のレコードがハンドルを持てば line-type/h14、なければ読んだ順の番号（line-type/2）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
        "id": str,
        # 元の名前。翻訳しない
        "name": str,
        # 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
        "hash": str,
        # ファイルに定義がある。偽なら、要素が名前で指すだけの線種で、ほかのフィールドを持たない。マイナーバージョン 26 から
        "defined": bool,
    },
)
_LineTypeItemOptional = TypedDict(
    "_LineTypeItemOptional",
    {
        # 元の説明。マイナーバージョン 26 から
        "description": str,
        # 線と間の長さの列。正は線、負は間、0 は点。空なら実線。長さの単位は paper が決める。マイナーバージョン 26 から
        "pattern": List[float],
        # 列の長さが用紙の mm なら真、文書の単位なら偽。用紙の mm の列は line_type で変えられない。マイナーバージョン 26 から
        "paper": bool,
        # 列の要素に形か文字を添える。添えれば、line_type で列の要素の数を変えられない。マイナーバージョン 26 から
        "marked": bool,
        # 列の要素に添える形と文字（決定 0150・0166）。添えなければ空。マイナーバージョン 28 から
        "marks": List["LineTypeMark"],
    },
    total=False,
)


class LineTypeItem(_LineTypeItemRequired, _LineTypeItemOptional):
    "線種の表の項目（決定 0164）。マイナーバージョン 26 から、定義を持つ"


# 寸法スタイルの表の項目（決定 0165）。マイナーバージョン 27 から、端部記号と大きさを持つ
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
        # 寸法線の 1 本目と 2 本目の補助線の側の端部記号の、ブロックの名前。空の名前は既定の矢印。マイナーバージョン 27 から
        "ends": List[str],
        # 引出線の端部記号の、ブロックの名前。空の名前は既定の矢印。マイナーバージョン 27 から
        "leader": str,
        # 端部記号の大きさ。文書の単位で、scale を掛ける前の値。マイナーバージョン 27 から
        "size": float,
        # 端部記号の代わりに描く斜線の大きさ。0 なら斜線を描かない。マイナーバージョン 27 から
        "tick": float,
        # 大きさに掛ける倍率。0 はビューの縮尺から決める。マイナーバージョン 27 から
        "scale": float,
    },
    total=False,
)


class DimensionStyleItem(_DimensionStyleItemRequired, _DimensionStyleItemOptional):
    "寸法スタイルの表の項目（決定 0165）。マイナーバージョン 27 から、端部記号と大きさを持つ"


# 線種の列の要素に添える形か文字（決定 0150・0166）。要素の始まりに、線の向きとずれで置く。マイナーバージョン 28 から
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
    "線種の列の要素に添える形か文字（決定 0150・0166）。要素の始まりに、線の向きとずれで置く。マイナーバージョン 28 から"

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


# 文字スタイルの表の項目（決定 0158）。フォントと縦書きは、この文字スタイルを指す文字に効く。高さ・幅の倍率・傾き・裏返し・上下反転は、新しく作る文字の既定の値で、今の文字はそれぞれの値を持つ。マイナーバージョン 20 から
_TextStyleItemRequired = TypedDict(
    "_TextStyleItemRequired",
    {
        # ID。表のレコードがハンドルを持てば text-style/h11、なければ文字スタイルの並びの中の順（text-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
        "id": str,
        # 元の名前。翻訳しない
        "name": str,
        # 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
        "hash": str,
        # ファイルに定義がある。偽なら、ほかのフィールドを持たない
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
    "文字スタイルの表の項目（決定 0158）。フォントと縦書きは、この文字スタイルを指す文字に効く。高さ・幅の倍率・傾き・裏返し・上下反転は、新しく作る文字の既定の値で、今の文字はそれぞれの値を持つ。マイナーバージョン 20 から"


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


# 要素の出所。L1 のレコードの番号かバイト範囲（決定 0008）と、形式が持つキー（決定 0156）。ファイルから読んでいない要素は、レコードの番号もバイト範囲も持たない
Source = TypedDict(
    "Source",
    {
        # L1 のレコードの番号
        "record": int,
        # L1 のバイト範囲
        "bytes": "ByteRange",
        # AutoCAD の系統のハンドル。大文字の 16 進。書き戻しても変わらない
        "handle": str,
        # SXF のインスタンスの番号（#n の n）。10 進の文字列。マイナーバージョン 6 から
        "instance": str,
    },
    total=False,
)


# 応答。失敗したときも同じ形で、result を持たない
_ResponseRequired = TypedDict(
    "_ResponseRequired",
    {
        # メッセージのスキーマのメジャーバージョン
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

# バイナリの結果の種類。file は書き出したファイル、image は描いた画像、scene は 2D のシーン（schema/scene.toml の配置、決定 0137・0172）、scene3d は 3D のシーン（schema/scene3d.toml の配置）、arrow は表（Arrow の IPC ストリーム。ネイティブでは zm_take_arrow でも取り出せる）、drawing は図面モデルのバイナリ（schema/drawing.toml の配置、決定 0247。マイナーバージョン 60 から）
BlobKind = Union[
    Literal[
        "file",
        "image",
        "scene",
        "arrow",
        "drawing",
        "scene3d",
    ],
    str,
]


# zm_session_new に渡す設定。渡さなければ既定の設定
_SessionOptionsRequired = TypedDict(
    "_SessionOptionsRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
    },
)
_SessionOptionsOptional = TypedDict(
    "_SessionOptionsOptional",
    {
        # 実行の形態。書かなければ、ネイティブのライブラリでは `native`、wasm のモジュールでは `wasm`（決定 0013）
        "engine": "Engine",
        # engine が `process` のときの、子プロセスにする CLI（zumen）の実行ファイルのパス。書かなければ、ライブラリと同じフォルダーの zumen を使う（決定 0013・0200）
        "process_path": str,
    },
    total=False,
)


class SessionOptions(_SessionOptionsRequired, _SessionOptionsOptional):
    "zm_session_new に渡す設定。渡さなければ既定の設定"

# 実行の形態（決定 0013）。`native` はプロセスの中でコアを動かす、`process` は子プロセスの CLI に送る、`wasm` は wasm のモジュールで動かす
Engine = Union[
    Literal[
        "native",
        "process",
        "wasm",
    ],
    str,
]


# 線形（決定 0230）の上の所を、追加距離か測点の表記から求め、点・方向角・高さ・勾配を返す。線形は、要求に書く（alignment）か、open で開いた J-LandXML・IFC4.3・SIMA の文書（doc）の線形を id で指す。distances・stations・interval の所を、この順に並べて返す。求められない所は、理由を診断して除く。doc を渡せば、文書の線形と地表面の要約も返す。図面・BIM・土木地理・点群のモジュールに置く（決定 0253）。マイナーバージョン 46 から（doc と id はマイナーバージョン 47 から）
_CivilRequestRequired = TypedDict(
    "_CivilRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # doc の座標点・高さ基準と、選んだ線形の IP・地盤縦断・片勾配・横断の値を返す。複数の線形があれば id が必要。省略は false。マイナーバージョン 87 から
        "details": bool,
        # 線形。doc と一緒には書かない
        "alignment": "CivilAlignment",
        # open で開いた J-LandXML・IFC4.3・SIMA の文書の名前。マイナーバージョン 47 から
        "doc": str,
        # doc の線形の ID。文書に線形が 1 つなら書かなくてよい。マイナーバージョン 47 から
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
    "線形（決定 0230）の上の所を、追加距離か測点の表記から求め、点・方向角・高さ・勾配を返す。線形は、要求に書く（alignment）か、open で開いた J-LandXML・IFC4.3・SIMA の文書（doc）の線形を id で指す。distances・stations・interval の所を、この順に並べて返す。求められない所は、理由を診断して除く。doc を渡せば、文書の線形と地表面の要約も返す。図面・BIM・土木地理・点群のモジュールに置く（決定 0253）。マイナーバージョン 46 から（doc と id はマイナーバージョン 47 から）"


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
        # details を求めた場合の保持値。マイナーバージョン 87 から
        "details": "CivilDetails",
        # 所を求めた doc の線形の ID。マイナーバージョン 47 から
        "alignment": str,
        # doc の線形の要約。doc を渡したときに返す。マイナーバージョン 47 から
        "alignments": List["CivilAlignmentSummary"],
        # doc の地表面（TIN）の要約。doc を渡したときに返す。マイナーバージョン 47 から
        "surfaces": List["CivilSurfaceSummary"],
        # doc の水平の座標系の EPSG のコード。分からなければ書かない。マイナーバージョン 47 から
        "crs": int,
    },
    total=False,
)


class CivilResult(_CivilResultRequired, _CivilResultOptional):
    "線形の上の所"


# 土木モデルの保持値。スキーマ 1.87 から
_CivilDetailsRequired = TypedDict(
    "_CivilDetailsRequired",
    {
        # 文書の座標点セット
        "point_sets": List["CivilPointSet"],
    },
)
_CivilDetailsOptional = TypedDict(
    "_CivilDetailsOptional",
    {
        # 選んだ線形の値。線形がなければ書かない
        "alignment": "CivilAlignmentValues",
        # 文書の高さの基準。値は自動で T.P. に直さない
        "height_reference": "CivilHeightReference",
    },
    total=False,
)


class CivilDetails(_CivilDetailsRequired, _CivilDetailsOptional):
    "土木モデルの保持値。スキーマ 1.87 から"


# 元の高さの基準
_CivilHeightReferenceRequired = TypedDict(
    "_CivilHeightReferenceRequired",
    {
        # 元の位置
        "source": "Source",
    },
)
_CivilHeightReferenceOptional = TypedDict(
    "_CivilHeightReferenceOptional",
    {
        # 元の基準名
        "name": str,
        # 元の高さに足すと T.P. の高さになる差（メートル）。不明なら書かない
        "tokyo_bay_offset": float,
    },
    total=False,
)


class CivilHeightReference(_CivilHeightReferenceRequired, _CivilHeightReferenceOptional):
    "元の高さの基準"


# 線形の付随する値。元の順序を保つ
_CivilAlignmentValuesRequired = TypedDict(
    "_CivilAlignmentValuesRequired",
    {
        # 同じ名前の縦断が複数あり、名前で参照先を一意に定められない
        "vertical_name_ambiguous": bool,
        # 選んだ線形の ID
        "id": str,
        # IP の列。始点・終点を含む
        "intersection_points": List["CivilSurveyPoint"],
        # 地盤の縦断の折れ線
        "ground_profiles": List["CivilGroundProfile"],
        # 片勾配とカントの区間
        "cant": List["CivilCant"],
        # 横断の線と点
        "cross_sections": List["CivilCrossSection"],
    },
)
_CivilAlignmentValuesOptional = TypedDict(
    "_CivilAlignmentValuesOptional",
    {
        # 保持した縦断線形の元の名前
        "vertical_name": str,
    },
    total=False,
)


class CivilAlignmentValues(_CivilAlignmentValuesRequired, _CivilAlignmentValuesOptional):
    "線形の付随する値。元の順序を保つ"


# IP・基準点などの座標点
_CivilSurveyPointRequired = TypedDict(
    "_CivilSurveyPointRequired",
    {
        # 北の座標（メートル）
        "northing": float,
        # 東の座標（メートル）
        "easting": float,
        # 元の位置
        "source": "Source",
    },
)
_CivilSurveyPointOptional = TypedDict(
    "_CivilSurveyPointOptional",
    {
        # 元の名前。永続 ID と同一視しない
        "name": str,
        # 注記
        "description": str,
        # 文書の基準での標高（メートル）。省略は 0 にしない
        "height": float,
        # 基準点・水準点の種類
        "class": str,
    },
    total=False,
)


class CivilSurveyPoint(_CivilSurveyPointRequired, _CivilSurveyPointOptional):
    "IP・基準点などの座標点"


# 元の順序を持つ座標点セット
_CivilPointSetRequired = TypedDict(
    "_CivilPointSetRequired",
    {
        # 点
        "points": List["CivilSurveyPoint"],
        # 元の位置
        "source": "Source",
    },
)
_CivilPointSetOptional = TypedDict(
    "_CivilPointSetOptional",
    {
        # 元の名前
        "name": str,
        # 注記
        "description": str,
    },
    total=False,
)


class CivilPointSet(_CivilPointSetRequired, _CivilPointSetOptional):
    "元の順序を持つ座標点セット"


# 地盤の縦断線
_CivilGroundProfileRequired = TypedDict(
    "_CivilGroundProfileRequired",
    {
        # 追加距離と高さ
        "points": List["CivilProfilePoint"],
        # 元の位置
        "source": "Source",
    },
)
_CivilGroundProfileOptional = TypedDict(
    "_CivilGroundProfileOptional",
    {
        # 元の名前
        "name": str,
        # 注記
        "description": str,
    },
    total=False,
)


class CivilGroundProfile(_CivilGroundProfileRequired, _CivilGroundProfileOptional):
    "地盤の縦断線"


# 地盤縦断の点
CivilProfilePoint = TypedDict(
    "CivilProfilePoint",
    {
        # 線形の初めからの追加距離（メートル）
        "distance": float,
        # 文書の基準での標高（メートル）
        "height": float,
    },
)


# 片勾配またはカントの区間
_CivilCantRequired = TypedDict(
    "_CivilCantRequired",
    {
        # 初めの追加距離（メートル）
        "start_distance": float,
        # 区間の長さ（メートル）
        "length": float,
        # superelevation は道路の左右の勾配（%）、rail は鉄道の左右の高さの差（mm）
        "kind": str,
        # 初めの左と右
        "start": List[float],
        # 終わりの左と右
        "end": List[float],
        # 元の位置
        "source": "Source",
    },
)
_CivilCantOptional = TypedDict(
    "_CivilCantOptional",
    {
        # 鉄道の軌間（mm）。不明なら書かない
        "gauge": float,
        # 変わり方。元の値
        "transition": str,
    },
    total=False,
)


class CivilCant(_CivilCantRequired, _CivilCantOptional):
    "片勾配またはカントの区間"


# 横断
_CivilCrossSectionRequired = TypedDict(
    "_CivilCrossSectionRequired",
    {
        # 追加距離（メートル）
        "distance": float,
        # 横断の線
        "lines": List["CivilCrossSectionLine"],
        # 元の位置
        "source": "Source",
    },
)
_CivilCrossSectionOptional = TypedDict(
    "_CivilCrossSectionOptional",
    {
        # 名前
        "name": str,
        # 参照する縦断線形の元の名前
        "profile": str,
    },
    total=False,
)


class CivilCrossSection(_CivilCrossSectionRequired, _CivilCrossSectionOptional):
    "横断"


# 横断の線。点の元の値を保つ
_CivilCrossSectionLineRequired = TypedDict(
    "_CivilCrossSectionLineRequired",
    {
        # existing、design または元の値
        "role": str,
        # elevation は文書の標高、formation_offset はその所の計画高からの差
        "height_reference": str,
        # 離れと元の高さ
        "points": List["CivilCrossSectionPoint"],
        # 元の位置
        "source": "Source",
    },
)
_CivilCrossSectionLineOptional = TypedDict(
    "_CivilCrossSectionLineOptional",
    {
        # 名前
        "name": str,
        # 注記
        "description": str,
        # 幅員中心から見た側。元の値。離れの符号は変えない
        "side": str,
    },
    total=False,
)


class CivilCrossSectionLine(_CivilCrossSectionLineRequired, _CivilCrossSectionLineOptional):
    "横断の線。点の元の値を保つ"


# 横断の点
CivilCrossSectionPoint = TypedDict(
    "CivilCrossSectionPoint",
    {
        # 中心線からの離れ（右を正、メートル）
        "offset": float,
        # 線の height_reference に従う高さ（メートル）
        "height": float,
    },
)


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


# open で開いた点群（LAS・LAZ・PLY・XYZ・E57・GeoTIFF の DEM）の要約と、条件に合う点を返す。点は八分木の浅い段のノードから順に選ぶ（決定 0232）。点群のモジュールに置く（決定 0253）。マイナーバージョン 48 から
_PointcloudRequestRequired = TypedDict(
    "_PointcloudRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
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
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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
    "open で開いた点群（LAS・LAZ・PLY・XYZ・E57・GeoTIFF の DEM）の要約と、条件に合う点を返す。点は八分木の浅い段のノードから順に選ぶ（決定 0232）。点群のモジュールに置く（決定 0253）。マイナーバージョン 48 から"


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
        # 八分木のノードの数
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


# 管理 XML の項目を変更し、file blob を返す。入力は変更しない。すべての経路は元の文書で解き、失敗時は結果を返さない。図面のモジュールに置く。マイナーバージョン 84 から
_DeliveryApplyRequestRequired = TypedDict(
    "_DeliveryApplyRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["delivery_apply"],
        # 管理 XML の入力の名前
        "input": str,
        # 指定する順の変更。同じ親への追加は順序を保つ
        "edits": List["DeliveryEdit"],
    },
)
_DeliveryApplyRequestOptional = TypedDict(
    "_DeliveryApplyRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 元の XML の文書ハッシュ。違えば変更しない。省略すれば検査しない
        "hash": str,
    },
    total=False,
)


class DeliveryApplyRequest(_DeliveryApplyRequestRequired, _DeliveryApplyRequestOptional):
    "管理 XML の項目を変更し、file blob を返す。入力は変更しない。すべての経路は元の文書で解き、失敗時は結果を返さない。図面のモジュールに置く。マイナーバージョン 84 から"


# 変更後の管理 XML の情報。file blob は 1 個。入力は変更しない
DeliveryApplyResult = TypedDict(
    "DeliveryApplyResult",
    {
        # 元の入力の名前
        "input": str,
        # 変更後のバイト列の文書ハッシュ
        "hash": str,
        # 適用した変更の数
        "applied": int,
    },
)


# 根からたどる子。接頭辞を含む名前で区別する
_DeliveryPathStepRequired = TypedDict(
    "_DeliveryPathStepRequired",
    {
        # 接頭辞を含む元の要素名
        "name": str,
    },
)
_DeliveryPathStepOptional = TypedDict(
    "_DeliveryPathStepOptional",
    {
        # 同名の兄弟の中の位置。0 から数え、省略すると 0
        "index": int,
    },
    total=False,
)


class DeliveryPathStep(_DeliveryPathStepRequired, _DeliveryPathStepOptional):
    "根からたどる子。接頭辞を含む名前で区別する"


# 親の名前空間を引き継ぐ追加要素。属性と名前空間宣言は追加しない
_DeliveryNewElementRequired = TypedDict(
    "_DeliveryNewElementRequired",
    {
        # 接頭辞を含む要素名。接頭辞は親で宣言済みのものに限る
        "name": str,
    },
)
_DeliveryNewElementOptional = TypedDict(
    "_DeliveryNewElementOptional",
    {
        # 子の前に書く文字。文字参照にする前の値
        "value": str,
        # 子の要素。書く順。省略すると空
        "children": List["DeliveryNewElement"],
    },
    total=False,
)


class DeliveryNewElement(_DeliveryNewElementRequired, _DeliveryNewElementOptional):
    "親の名前空間を引き継ぐ追加要素。属性と名前空間宣言は追加しない"

# 元の文書の項目の更新か、子の記録の追加。空の path は根を指す。同じ項目の更新を重ねたり、更新と追加を混ぜたりしない
DeliveryEdit = Union[
    "DeliverySetText",
    "DeliveryAppend",
]


# 子の要素を持たない項目の文字を置き換える。注記と処理命令は保つ
DeliverySetText = TypedDict(
    "DeliverySetText",
    {
        # 変更の種類
        "kind": Literal["set_text"],
        # 根からの経路
        "path": List["DeliveryPathStep"],
        # 文字参照にする前の値
        "value": str,
    },
)


# 親の末尾に子の記録を足す。名前・文字コード・予算を検査する
DeliveryAppend = TypedDict(
    "DeliveryAppend",
    {
        # 変更の種類
        "kind": Literal["append"],
        # 親の根からの経路
        "path": List["DeliveryPathStep"],
        # 追加する記録の木
        "child": "DeliveryNewElement",
    },
)


# 電子納品の成果品（決定 0241）を読み、管理ファイルの要約を返す。成果品は、zm_push で渡した入力のうち root の下にある名前の集まりか、ZIP の入力（archive）で渡す。profiles と profile を渡せば、プロファイルの delivery- で始まる照合の種類の規則と照らし、指摘を返す（#384）。ほかの種類の規則は飛ばす。図面のモジュールに置く。マイナーバージョン 53 から
_DeliveryCheckRequestRequired = TypedDict(
    "_DeliveryCheckRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["delivery_check"],
    },
)
_DeliveryCheckRequestOptional = TypedDict(
    "_DeliveryCheckRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 成果品の起点。/ で終わる入力の名前の前の部分（納品/ など）。archive と一緒には書かない
        "root": str,
        # 成果品の ZIP の入力の名前。root と一緒には書かない
        "archive": str,
        # プロファイルの入力の名前の列。extends で指す先も、すべて渡す
        "profiles": List[str],
        # 照合に使うプロファイル。profiles の中から選ぶ
        "profile": "ProfileRef",
        # 返す指摘の初めの位置
        "offset": int,
        # 返す指摘の数の上限。書かなければ 10000
        "limit": int,
    },
    total=False,
)


class DeliveryCheckRequest(_DeliveryCheckRequestRequired, _DeliveryCheckRequestOptional):
    "電子納品の成果品（決定 0241）を読み、管理ファイルの要約を返す。成果品は、zm_push で渡した入力のうち root の下にある名前の集まりか、ZIP の入力（archive）で渡す。profiles と profile を渡せば、プロファイルの delivery- で始まる照合の種類の規則と照らし、指摘を返す（#384）。ほかの種類の規則は飛ばす。図面のモジュールに置く。マイナーバージョン 53 から"


# 成果品の要約と、照合の結果。照合の結果は check と同じ形
_DeliveryCheckResultRequired = TypedDict(
    "_DeliveryCheckResultRequired",
    {
        # 成果品のファイルの数
        "files": int,
        # 管理ファイルの要約。名前の順
        "managed": List["DeliveryManaged"],
        # 照合に使ったプロファイルの鎖。基から順に。プロファイルを渡さなければ空
        "chain": List["ProfileIdentity"],
        # 規則ごとの指摘の数
        "rules": List["CheckRule"],
        # 指摘
        "findings": List["CheckFinding"],
        # 指摘の数の全体
        "total": int,
    },
)
_DeliveryCheckResultOptional = TypedDict(
    "_DeliveryCheckResultOptional",
    {
        # 続きの指摘の位置。続きがなければ書かない
        "next_offset": int,
    },
    total=False,
)


class DeliveryCheckResult(_DeliveryCheckResultRequired, _DeliveryCheckResultOptional):
    "成果品の要約と、照合の結果。照合の結果は check と同じ形"


# 工事写真（JPEG）の Exif と XMP を読み、画素数・向き・撮影の日時・位置などを返す（#387）。画像は復号しない。図面のモジュールに置く。マイナーバージョン 55 から
_PhotoRequestRequired = TypedDict(
    "_PhotoRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["photo"],
        # JPEG の入力の名前
        "input": str,
    },
)
_PhotoRequestOptional = TypedDict(
    "_PhotoRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
    },
    total=False,
)


class PhotoRequest(_PhotoRequestRequired, _PhotoRequestOptional):
    "工事写真（JPEG）の Exif と XMP を読み、画素数・向き・撮影の日時・位置などを返す（#387）。画像は復号しない。図面のモジュールに置く。マイナーバージョン 55 から"


# 写真の情報。ファイルが持たない値は書かない
_PhotoResultRequired = TypedDict(
    "_PhotoResultRequired",
    {
        # XMP の性質。文書の順
        "xmp": List["PhotoProperty"],
    },
)
_PhotoResultOptional = TypedDict(
    "_PhotoResultOptional",
    {
        # 画素の幅（フレームのヘッダー）
        "width": int,
        # 画素の高さ（フレームのヘッダー）
        "height": int,
        # 向き（Exif の Orientation、1〜8）
        "orientation": int,
        # 撮影の日時（YYYY-MM-DDTHH:MM:SS、時差があれば +09:00 などを足す）
        "taken": str,
        # 機器の作り手
        "make": str,
        # 機器の型
        "model": str,
        # 撮った位置（GPS の緯度・経度、度）。測地系は換算しない
        "position": "PhotoPosition",
        # 撮った向き（度）
        "direction": float,
        # 向きの基準（T は真北、M は磁北）
        "direction_reference": str,
        # GPS の測地系の名前（GPSMapDatum）
        "datum": str,
    },
    total=False,
)


class PhotoResult(_PhotoResultRequired, _PhotoResultOptional):
    "写真の情報。ファイルが持たない値は書かない"


# 撮った位置
_PhotoPositionRequired = TypedDict(
    "_PhotoPositionRequired",
    {
        # 緯度（度、北が正）
        "latitude": float,
        # 経度（度、東が正）
        "longitude": float,
    },
)
_PhotoPositionOptional = TypedDict(
    "_PhotoPositionOptional",
    {
        # 高さ（m、海面より下は負）
        "altitude": float,
    },
    total=False,
)


class PhotoPosition(_PhotoPositionRequired, _PhotoPositionOptional):
    "撮った位置"


# XMP の性質
PhotoProperty = TypedDict(
    "PhotoProperty",
    {
        # 名前空間の URI
        "namespace": str,
        # 接頭辞を除いた名前
        "name": str,
        # 値。配列は最初の項目の値
        "value": str,
    },
)


# 地質・土質調査のボーリング交換用データ（BED0500.DTD の XML）を読み、ボーリングの位置・標高と、地層・色調・観察記事・標準貫入試験・孔内水位を返す（#390）。ほかの試験と判定表は読まない。図面のモジュールに置く。マイナーバージョン 57 から
_BoringRequestRequired = TypedDict(
    "_BoringRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["boring"],
        # ボーリング交換用データの入力の名前
        "input": str,
    },
)
_BoringRequestOptional = TypedDict(
    "_BoringRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
    },
    total=False,
)


class BoringRequest(_BoringRequestRequired, _BoringRequestOptional):
    "地質・土質調査のボーリング交換用データ（BED0500.DTD の XML）を読み、ボーリングの位置・標高と、地層・色調・観察記事・標準貫入試験・孔内水位を返す（#390）。ほかの試験と判定表は読まない。図面のモジュールに置く。マイナーバージョン 57 から"


# 読んだボーリング。深さは地表からの m（GL.-m）
_BoringResultRequired = TypedDict(
    "_BoringResultRequired",
    {
        # 適用規格。文書の順
        "standards": List[str],
        # 工学的地質区分名・現場土質名。文書の順
        "layers": List["BoringLayer"],
        # 色調。文書の順
        "colors": List["BoringColor"],
        # 観察記事。文書の順
        "observations": List["BoringObservation"],
        # 標準貫入試験。文書の順
        "penetrations": List["BoringPenetration"],
        # 孔内水位。文書の順
        "water_levels": List["BoringWaterLevel"],
    },
)
_BoringResultOptional = TypedDict(
    "_BoringResultOptional",
    {
        # 公開フラグのコード
        "publication": str,
        # 事業工事名
        "project": str,
        # 調査名
        "survey": str,
        # ボーリング名
        "name": str,
        # ボーリング総数
        "count": int,
        # ボーリング連番
        "serial": int,
        # 孔口の位置。測地系は換算しない
        "position": "BoringPosition",
        # 位置の測地系
        "datum": "BoringDatum",
        # 調査位置の住所
        "address": str,
        # 調査期間の始まりの日（YYYY-MM-DD）
        "start": str,
        # 調査期間の終わりの日（YYYY-MM-DD）
        "end": str,
        # 孔口標高
        "elevation": "BoringHeight",
        # 総削孔長（m）
        "length": float,
        # 削孔の角度（度）
        "angle": float,
        # 削孔の方位（度）
        "direction": float,
    },
    total=False,
)


class BoringResult(_BoringResultRequired, _BoringResultOptional):
    "読んだボーリング。深さは地表からの m（GL.-m）"


# 経度・緯度（度）。度・分・秒から求める
BoringPosition = TypedDict(
    "BoringPosition",
    {
        # 緯度（度、北が正）
        "latitude": float,
        # 経度（度、東が正）
        "longitude": float,
    },
)

# 測地系。tokyo は日本測地系（コード 00）、jgd2000・jgd2011・jgd2024 は世界測地系（コード 01〜03）
BoringDatum = Union[
    Literal[
        "tokyo",
        "jgd2000",
        "jgd2011",
        "jgd2024",
    ],
    str,
]


# 標高と、その基準と成果（決定 0019）
BoringHeight = TypedDict(
    "BoringHeight",
    {
        # 標高（m）
        "value": float,
        # 高さの基準。孔口標高は tokyo_bay（東京湾平均海面）とする
        "datum": "HeightDatum",
        # 成果。測地系が jgd2024 なら survey_2024、そうでなければ調査の日から仮定する
        "realization": "HeightRealization",
    },
)

# 高さの基準（決定 0019）。tokyo_bay は東京湾平均海面（T.P.）、island は離島の独自の平均海面、ellipsoid は楕円体（GRS80）
HeightDatum = Union[
    Literal[
        "tokyo_bay",
        "island",
        "ellipsoid",
    ],
    str,
]

# 標高の成果（決定 0019）。survey_2011 は測地成果2011（2025-03-31 まで）、survey_2024 は測地成果2024（2025-04-01 から）
HeightRealization = Union[
    Literal[
        "survey_2011",
        "survey_2024",
    ],
    str,
]


# 地層の区間。上端は 1 つ前の区間の下端（最初は 0）
_BoringLayerRequired = TypedDict(
    "_BoringLayerRequired",
    {
        # 下端の深さ（m）
        "bottom": float,
        # 工学的地質区分名・現場土質名
        "name": str,
    },
)
_BoringLayerOptional = TypedDict(
    "_BoringLayerOptional",
    {
        # 記号
        "symbol": str,
    },
    total=False,
)


class BoringLayer(_BoringLayerRequired, _BoringLayerOptional):
    "地層の区間。上端は 1 つ前の区間の下端（最初は 0）"


# 色調の区間
_BoringColorRequired = TypedDict(
    "_BoringColorRequired",
    {
        # 下端の深さ（m）
        "bottom": float,
    },
)
_BoringColorOptional = TypedDict(
    "_BoringColorOptional",
    {
        # 色調の名前
        "name": str,
    },
    total=False,
)


class BoringColor(_BoringColorRequired, _BoringColorOptional):
    "色調の区間"


# 観察記事
_BoringObservationRequired = TypedDict(
    "_BoringObservationRequired",
    {
        # 上端の深さ（m）
        "top": float,
        # 下端の深さ（m）
        "bottom": float,
    },
)
_BoringObservationOptional = TypedDict(
    "_BoringObservationOptional",
    {
        # 記事
        "text": str,
    },
    total=False,
)


class BoringObservation(_BoringObservationRequired, _BoringObservationOptional):
    "観察記事"


# 標準貫入試験
BoringPenetration = TypedDict(
    "BoringPenetration",
    {
        # 開始の深さ（m）
        "depth": float,
        # 合計の打撃回数
        "blows": int,
        # 合計の貫入量（mm）
        "penetration": float,
    },
)


# 孔内水位
BoringWaterLevel = TypedDict(
    "BoringWaterLevel",
    {
        # 測定の日（YYYY-MM-DD）
        "date": str,
        # 削孔状況のコード
        "state": str,
        # 水位の深さ（m）。地表より高ければ負。水位がなければ書かない
        "depth": float,
        # 水位種別・備考
        "note": str,
    },
    total=False,
)


# 管理ファイルの要約
_DeliveryManagedRequired = TypedDict(
    "_DeliveryManagedRequired",
    {
        # 成果品の中の名前
        "name": str,
        # 根の要素の名前
        "root": str,
        # 要素の記録の数
        "records": int,
        # ファイルの名前の形の値の数
        "links": int,
        # 成果品の中に見つからないファイルの名前。書いた順
        "missing": List[str],
    },
)
_DeliveryManagedOptional = TypedDict(
    "_DeliveryManagedOptional",
    {
        # 文書型宣言の DTD の名前
        "doctype": str,
    },
    total=False,
)


class DeliveryManaged(_DeliveryManagedRequired, _DeliveryManagedOptional):
    "管理ファイルの要約"


# open で開いた CityGML・基盤地図情報（GML）・拡張 DM・GeoJSON・SIMA・GeoTIFF の文書（決定 0233）の地理の領域モデルの要約と、id で指した地物の中身を返す。土木・地理のモジュールに置く（決定 0253）。マイナーバージョン 52 から
_GeoRequestRequired = TypedDict(
    "_GeoRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["geo"],
        # open で開いた CityGML・基盤地図情報（GML）・拡張 DM・GeoJSON・SIMA・GeoTIFF の文書の名前
        "doc": str,
    },
)
_GeoRequestOptional = TypedDict(
    "_GeoRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 中身を返す地物の ID
        "id": str,
    },
    total=False,
)


class GeoRequest(_GeoRequestRequired, _GeoRequestOptional):
    "open で開いた CityGML・基盤地図情報（GML）・拡張 DM・GeoJSON・SIMA・GeoTIFF の文書（決定 0233）の地理の領域モデルの要約と、id で指した地物の中身を返す。土木・地理のモジュールに置く（決定 0253）。マイナーバージョン 52 から"


# 地理の領域モデルの要約
_GeoResultRequired = TypedDict(
    "_GeoResultRequired",
    {
        # 地物の数
        "features": int,
        # 地物の型ごとの数。型の名前の順
        "kinds": List["GeoKind"],
    },
)
_GeoResultOptional = TypedDict(
    "_GeoResultOptional",
    {
        # 平面の座標系の EPSG のコード。分からなければ書かない
        "crs": int,
        # 高さの座標系の EPSG のコード。分からなければ書かない
        "height": int,
        # id で指した地物の中身
        "feature": "GeoFeature",
    },
    total=False,
)


class GeoResult(_GeoResultRequired, _GeoResultOptional):
    "地理の領域モデルの要約"


# 地物の型と数
GeoKind = TypedDict(
    "GeoKind",
    {
        # 型。名前空間の接頭辞をつけた元の名前
        "kind": str,
        # 地物の数
        "count": int,
    },
)


# 地物の中身
_GeoFeatureRequired = TypedDict(
    "_GeoFeatureRequired",
    {
        # ID
        "id": str,
        # 型
        "kind": str,
        # 子の地物の数
        "children": int,
        # 属性。入力の順
        "attributes": List["GeoAttribute"],
        # 形。入力の順
        "geometries": List["GeoGeometry"],
    },
)
_GeoFeatureOptional = TypedDict(
    "_GeoFeatureOptional",
    {
        # 分類コード。元の値の文字のまま
        "code": str,
        # 名前
        "name": str,
        # 親の地物の ID
        "parent": str,
    },
    total=False,
)


class GeoFeature(_GeoFeatureRequired, _GeoFeatureOptional):
    "地物の中身"


# 属性
_GeoAttributeRequired = TypedDict(
    "_GeoAttributeRequired",
    {
        # 名前。接頭辞をつけた元の名前
        "name": str,
        # 値の型
        "kind": "GeoValueKind",
        # 値の文字
        "value": str,
    },
)
_GeoAttributeOptional = TypedDict(
    "_GeoAttributeOptional",
    {
        # 単位つきの量の単位
        "unit": str,
        # コードの一覧（codeSpace）
        "space": str,
    },
    total=False,
)


class GeoAttribute(_GeoAttributeRequired, _GeoAttributeOptional):
    "属性"

# 属性の値の型
GeoValueKind = Union[
    Literal[
        "text",
        "integer",
        "real",
        "bool",
        "date",
        "code",
        "measure",
    ],
    str,
]

# 形の種類
GeoForm = Union[
    Literal[
        "point",
        "line",
        "area",
        "solid",
    ],
    str,
]


# 形の要約
_GeoGeometryRequired = TypedDict(
    "_GeoGeometryRequired",
    {
        # 形の種類
        "form": "GeoForm",
        # 点の数。立体と面の集まりは、面の輪の点の数の和
        "points": int,
    },
)
_GeoGeometryOptional = TypedDict(
    "_GeoGeometryOptional",
    {
        # LOD。2D の地物は書かない
        "lod": int,
        # 立体と面の集まりの面の数
        "faces": int,
    },
    total=False,
)


class GeoGeometry(_GeoGeometryRequired, _GeoGeometryOptional):
    "形の要約"


# open で開いた J-LandXML・IFC4.3・SIMA の文書の線形から、平面図・縦断図・横断図の図面モデルを作り（決定 0231）、用紙を 1 枚選んで新しい DXF の文書として開く（決定 0239）。尺度・文字の高さ・測点の間隔は、プロファイルを渡せば語彙 civil-drawing と text-height から読み、渡さなければ作図の慣習の値にする。縦断図は用紙の要素に焼き込む（決定 0238）。書いたファイルは blobs で返す。作った文書は scene と check に渡せる。図面のモジュールに置く。マイナーバージョン 51 から
_CivilDrawingRequestRequired = TypedDict(
    "_CivilDrawingRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["civil_drawing"],
        # open で開いた J-LandXML・IFC4.3・SIMA の文書の名前
        "doc": str,
        # 作る文書の名前
        "to": str,
    },
)
_CivilDrawingRequestOptional = TypedDict(
    "_CivilDrawingRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # doc の線形の ID。文書に線形が 1 つなら書かなくてよい
        "id": str,
        # 書く用紙の名前（結果の sheets の 1 つ）。書かなければ平面図
        "sheet": str,
        # プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す
        "profiles": List[str],
        # 作図の値に使うプロファイル。profiles の中から選ぶ。profiles と一緒に書く
        "profile": "ProfileRef",
    },
    total=False,
)


class CivilDrawingRequest(_CivilDrawingRequestRequired, _CivilDrawingRequestOptional):
    "open で開いた J-LandXML・IFC4.3・SIMA の文書の線形から、平面図・縦断図・横断図の図面モデルを作り（決定 0231）、用紙を 1 枚選んで新しい DXF の文書として開く（決定 0239）。尺度・文字の高さ・測点の間隔は、プロファイルを渡せば語彙 civil-drawing と text-height から読み、渡さなければ作図の慣習の値にする。縦断図は用紙の要素に焼き込む（決定 0238）。書いたファイルは blobs で返す。作った文書は scene と check に渡せる。図面のモジュールに置く。マイナーバージョン 51 から"


# 作った文書。書いたファイルは blobs の file で返す
_CivilDrawingResultRequired = TypedDict(
    "_CivilDrawingResultRequired",
    {
        # 作った文書の名前
        "doc": str,
        # 書いた形式の名前。今は dxf
        "format": str,
        # 書いたファイルのハッシュ。apply の前提条件に使う
        "hash": str,
        # 書いた要素の数
        "elements": int,
        # 図を作った線形の ID
        "alignment": str,
        # 作った用紙の名前。図を描ける順（平面図・縦断図・横断図）
        "sheets": List[str],
        # 書いた用紙の名前
        "sheet": str,
        # 使った作図の値
        "options": "CivilDrawingOptions",
    },
)
_CivilDrawingResultOptional = TypedDict(
    "_CivilDrawingResultOptional",
    {
        # 作図の値を読んだプロファイルの、extends を解いた鎖。基から順。プロファイルを渡さなければ書かない
        "profile": List["ProfileIdentity"],
    },
    total=False,
)


class CivilDrawingResult(_CivilDrawingResultRequired, _CivilDrawingResultOptional):
    "作った文書。書いたファイルは blobs の file で返す"


# 土木の図の作図の値
CivilDrawingOptions = TypedDict(
    "CivilDrawingOptions",
    {
        # 平面図の尺度の分母
        "plan": float,
        # 縦断図の横の尺度の分母
        "horizontal": float,
        # 縦断図の縦の尺度の分母
        "vertical": float,
        # 横断図の尺度の分母
        "cross_section": float,
        # 文字の高さ（mm）
        "text_height": float,
        # 測点の間隔（メートル）
        "station_pitch": float,
    },
)


# 開いた点群と、土木の文書の TIN の標高の差を求め、プロファイルの surface-elevation の規則の区分ごとに集計して、規格値と比べる（決定 0234）。点群のモジュールに置く（決定 0253）。マイナーバージョン 50 から
_DeviationRequestRequired = TypedDict(
    "_DeviationRequestRequired",
    {
        # メッセージのスキーマのメジャーバージョン。今は 1
        "v": int,
        # op の名前
        "op": Literal["deviation"],
        # open で開いた点群の文書の名前
        "doc": str,
        # 設計面の TIN
        "surface": "DeviationSurface",
        # プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す
        "profiles": List[str],
        # 規格値に使うプロファイル。profiles の中から選ぶ
        "profile": "ProfileRef",
        # 工種の名前。規則の work と比べ、同じものだけを使う
        "work": str,
    },
)
_DeviationRequestOptional = TypedDict(
    "_DeviationRequestOptional",
    {
        # 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
        "budget": "Budget",
        # 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
        "partial": bool,
        # 診断ごとに返す場所の数。書かなければ 16（決定 0009）
        "max_places": int,
        # 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
        "time_limit_ms": int,
        # 返す最初の評価した点の位置。前の応答の next_offset を渡す。書かなければ 0
        "offset": int,
        # 返す評価した点の数の上限。書かなければ点を返さない
        "limit": int,
        # 差の色の段で点を塗った 3D のシーンも返す。点の選び方は点群の scene3d と同じ。89 から
        "scene3d": "DeviationScene3d",
    },
    total=False,
)


class DeviationRequest(_DeviationRequestRequired, _DeviationRequestOptional):
    "開いた点群と、土木の文書の TIN の標高の差を求め、プロファイルの surface-elevation の規則の区分ごとに集計して、規格値と比べる（決定 0234）。点群のモジュールに置く（決定 0253）。マイナーバージョン 50 から"


# 設計面の TIN を持つ文書と、TIN の ID
DeviationSurface = TypedDict(
    "DeviationSurface",
    {
        # open で開いた土木の文書の名前
        "doc": str,
        # TIN の ID。civil の surfaces の id
        "id": str,
    },
)


# 標高の差の、規則の区分ごとの集計と指摘。合格・適合とは言わない
_DeviationResultRequired = TypedDict(
    "_DeviationResultRequired",
    {
        # 使ったプロファイルの鎖。extends の基から順
        "chain": List["ProfileIdentity"],
        # 点群の点の数
        "points": int,
        # TIN の外なので評価しなかった点の数
        "outside": int,
        # どの規則の勾配にも入らない三角形の上なので評価しなかった点の数
        "unmatched": int,
        # 区分の境の近く（規則の exclude の内）なので除いた点の数
        "excluded": int,
        # 規則ごとの集計。プロファイルの順。使えない規則は持たず、profile.rule.unsupported の診断で返す
        "rules": List["DeviationRule"],
        # 指摘。規則の順。規格値を外れれば profile.surface-elevation.invalid、確認できなければ profile.surface-elevation.unsupported
        "findings": List["CheckFinding"],
        # 評価した点の数
        "evaluated": int,
        # 評価した点。点群の順に、offset から limit 個まで
        "differences": List["DeviationPoint"],
    },
)
_DeviationResultOptional = TypedDict(
    "_DeviationResultOptional",
    {
        # 続きの最初の評価した点の位置。続きがなければ持たない
        "next_offset": int,
        # 要求に scene3d を書いたときの 3D のシーン。点の色は bands の段の色で、元の色は持たない。強度と分類は元の値。89 から
        "scene3d": "Scene3dResult",
        # 要求に scene3d を書いたときの色の段。規則の順に、その規則で起こりうる段を、差の小さい順に並べ、最後に評価しなかった点の段を置く。89 から
        "bands": List["DeviationBand"],
    },
    total=False,
)


class DeviationResult(_DeviationResultRequired, _DeviationResultOptional):
    "標高の差の、規則の区分ごとの集計と指摘。合格・適合とは言わない"


# 差の色分けの 3D のシーンの点の選び方。意味は点群の scene3d の box・tolerance・limit と同じ
DeviationScene3d = TypedDict(
    "DeviationScene3d",
    {
        # 点群の元の座標系の東・北・高さのメートルの箱
        "box": "Scene3dBox",
        # 点の間隔（メートル）の正の有限値。これより細かい段の節を選ばない
        "tolerance": float,
        # 返す点の数の上限。省略時は選んだすべての点
        "limit": int,
    },
    total=False,
)


# 差の色の段。差は mm で、規則の each（個々の計測値の規格値）から決める。下限より小さい差は below、上限より大きい差は above で、規格値ちょうどは内側に入れる。内側は 0 から規格値の半分までと、半分から規格値までに分け、境の値は 0 に近い段に入れる。0 は high_inner。規格値のない側は inner の 1 段だけにする
_DeviationBandRequired = TypedDict(
    "_DeviationBandRequired",
    {
        # 段の名前
        "band": "DeviationBandKind",
        # シーンの点の色。16 ビットの赤・緑・青・不透明度
        "color": List[int],
    },
)
_DeviationBandOptional = TypedDict(
    "_DeviationBandOptional",
    {
        # 規則の id。評価しなかった点の段は持たない
        "rule": str,
        # 段の下の端の差（mm）。下に端がなければ持たない
        "lower": float,
        # 段の上の端の差（mm）。上に端がなければ持たない
        "upper": float,
    },
    total=False,
)


class DeviationBand(_DeviationBandRequired, _DeviationBandOptional):
    "差の色の段。差は mm で、規則の each（個々の計測値の規格値）から決める。下限より小さい差は below、上限より大きい差は above で、規格値ちょうどは内側に入れる。内側は 0 から規格値の半分までと、半分から規格値までに分け、境の値は 0 に近い段に入れる。0 は high_inner。規格値のない側は inner の 1 段だけにする"

# 差の色の段。not_evaluated は TIN の外・どの規則の勾配にも入らない・区分の境の近くで除いた点
DeviationBandKind = Union[
    Literal[
        "below",
        "low_outer",
        "low_inner",
        "high_inner",
        "high_outer",
        "above",
        "not_evaluated",
    ],
    str,
]


# 規則の区分ごとの集計。差はメートル
_DeviationRuleRequired = TypedDict(
    "_DeviationRuleRequired",
    {
        # 規則の id
        "rule": str,
        # 区分の名前
        "part": str,
        # 評価した点の数
        "points": int,
        # 区分の三角形の、平面に投影した面積（m²）
        "area": float,
        # 指摘の数（確認できなかったものを除く）
        "findings": int,
        # 確認できなかった数
        "unsupported": int,
    },
)
_DeviationRuleOptional = TypedDict(
    "_DeviationRuleOptional",
    {
        # 計測の密度（点／m²）。面積がなければ書かない
        "density": float,
        # 差の平均。点がなければ書かない
        "mean": float,
        # 差の最小。点がなければ書かない
        "min": float,
        # 差の最大。点がなければ書かない
        "max": float,
    },
    total=False,
)


class DeviationRule(_DeviationRuleRequired, _DeviationRuleOptional):
    "規則の区分ごとの集計。差はメートル"


# 評価した点
DeviationPoint = TypedDict(
    "DeviationPoint",
    {
        # 点群の中の番号
        "index": int,
        # 北・東・標高（メートル）
        "position": List[float],
        # 点の標高から設計面の標高を引いた差（メートル）
        "difference": float,
        # 評価した規則の id
        "rule": str,
    },
)


# describe に使うカタログのファイル。catalog/<言語>.toml から生成し、zm_push で渡す（決定 0009）
Catalog = TypedDict(
    "Catalog",
    {
        # カタログの形のメジャーバージョン。今は 1
        "v": int,
        # 言語（ja・en）
        "lang": str,
        # パッケージのバージョン。コアと同じバージョンで出す
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
    "calibrations",
    "measure_distance",
    "measure",
    "intersect",
    "regions",
    "construct",
    "apply",
    "create",
    "convert",
    "convert_estimate",
    "tables",
    "scene",
    "scene3d",
    "check",
    "validate",
    "review_carry",
    "review_adopt",
    "review_scene",
    "building",
    "building_elements",
    "ids_check",
    "review_adopt_ids",
    "bcf_read",
    "bcf_write",
    "civil",
    "pointcloud",
    "deviation",
    "civil_drawing",
    "geo",
    "delivery_check",
    "delivery_apply",
    "photo",
    "boring",
    "building_apply",
    "lift",
    "render",
    "raster",
]

# op ごとの要求と結果の型の名前。結果は応答の result を typing.cast で読む型
OPERATIONS: Dict[str, Any] = {
    "capabilities": ("CapabilitiesRequest", "CapabilitiesResult"),
    "open": ("OpenRequest", "OpenResult"),
    "close": ("CloseRequest", "CloseResult"),
    "describe": ("DescribeRequest", "DescribeResult"),
    "drawing": ("DrawingRequest", "DrawingResult"),
    "entities": ("EntitiesRequest", "EntitiesResult"),
    "calibrations": ("CalibrationsRequest", "CalibrationsResult"),
    "measure_distance": ("MeasureDistanceRequest", "MeasureDistanceResult"),
    "measure": ("MeasureRequest", "MeasureResult"),
    "intersect": ("IntersectRequest", "IntersectResult"),
    "regions": ("RegionsRequest", "RegionsResult"),
    "construct": ("ConstructRequest", "ConstructResult"),
    "apply": ("ApplyRequest", "ApplyResult"),
    "create": ("CreateRequest", "CreateResult"),
    "convert": ("ConvertRequest", "ConvertResult"),
    "convert_estimate": ("ConvertEstimateRequest", "ConvertEstimateResult"),
    "tables": ("TablesRequest", "TablesResult"),
    "scene": ("SceneRequest", "SceneResult"),
    "scene3d": ("Scene3dRequest", "Scene3dResult"),
    "check": ("CheckRequest", "CheckResult"),
    "validate": ("ValidateRequest", "ValidateResult"),
    "review_carry": ("ReviewCarryRequest", "ReviewCarryResult"),
    "review_adopt": ("ReviewAdoptRequest", "ReviewAdoptResult"),
    "review_scene": ("ReviewSceneRequest", "ReviewSceneResult"),
    "building": ("BuildingRequest", "BuildingResult"),
    "building_elements": ("BuildingElementsRequest", "BuildingElementsResult"),
    "ids_check": ("IdsCheckRequest", "IdsCheckResult"),
    "review_adopt_ids": ("ReviewAdoptIdsRequest", "Dict[str, Any]"),
    "bcf_read": ("BcfReadRequest", "BcfReadResult"),
    "bcf_write": ("BcfWriteRequest", "BcfWriteResult"),
    "civil": ("CivilRequest", "CivilResult"),
    "pointcloud": ("PointcloudRequest", "PointcloudResult"),
    "deviation": ("DeviationRequest", "DeviationResult"),
    "civil_drawing": ("CivilDrawingRequest", "CivilDrawingResult"),
    "geo": ("GeoRequest", "GeoResult"),
    "delivery_check": ("DeliveryCheckRequest", "DeliveryCheckResult"),
    "delivery_apply": ("DeliveryApplyRequest", "DeliveryApplyResult"),
    "photo": ("PhotoRequest", "PhotoResult"),
    "boring": ("BoringRequest", "BoringResult"),
    "building_apply": ("BuildingApplyRequest", "BuildingApplyResult"),
    "lift": ("LiftRequest", "LiftResult"),
    "render": ("RenderRequest", "RenderResult"),
    "raster": ("RasterRequest", "RasterResult"),
}
