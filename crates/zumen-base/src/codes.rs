// 生成したファイル。手で直さない。
// 元は catalog/codes.toml で、`cargo xtask codegen` で作り直す。

//! 診断の code の定数（決定 0009）。台帳の名前の順に並べる。

use crate::code::{Code, Fidelity, Param, ParamType, Severity};

/// IFC の GlobalId でない要素の ID（`i` と番号）は BCF で指せないので、参照から除いた
///
/// `bcf.element.unsupported`（0.2.0 から）
pub const BCF_ELEMENT_UNSUPPORTED: Code = Code::new(
    "bcf.element.unsupported",
    Severity::Warning,
    None,
    &[
        Param::new("id", ParamType::Text),
        Param::new("topic", ParamType::Text),
    ],
    &[],
);

/// bcfzip の中の項目が、BCF 3.0 の形に合わない。name は項目の名前、near は合わなかった要素か属性
///
/// `bcf.file.invalid`（0.2.0 から）
pub const BCF_FILE_INVALID: Code = Code::new(
    "bcf.file.invalid",
    Severity::Error,
    None,
    &[
        Param::new("name", ParamType::Text),
        Param::new("near", ParamType::Text),
    ],
    &["name"],
);

/// BCF に表せない場所なので、書かなかった。reason は plane（2D の場所）・target（ほかの対象）・camera（カメラがない）
///
/// `bcf.location.unsupported`（0.2.0 から）
pub const BCF_LOCATION_UNSUPPORTED: Code = Code::new(
    "bcf.location.unsupported",
    Severity::Warning,
    None,
    &[
        Param::new("location", ParamType::Text),
        Param::new("reason", ParamType::Text),
        Param::new("topic", ParamType::Text),
    ],
    &["reason"],
);

/// 図形（赤入れ・変更雲・注記・写真）は BCF に表せないので、書かなかった
///
/// `bcf.markup.unsupported`（0.2.0 から）
pub const BCF_MARKUP_UNSUPPORTED: Code = Code::new(
    "bcf.markup.unsupported",
    Severity::Warning,
    None,
    &[Param::new("id", ParamType::Text)],
    &[],
);

/// 照合の結果の出所（規則と診断）は BCF に表せないので、落とした
///
/// `bcf.origin.dropped`（0.2.0 から）
pub const BCF_ORIGIN_DROPPED: Code = Code::new(
    "bcf.origin.dropped",
    Severity::Info,
    None,
    &[Param::new("topic", ParamType::Text)],
    &[],
);

/// 書く bcfzip が ZIP64 を要する大きさになる。ZIP64 は書かない（決定 0220）
///
/// `bcf.size.unsupported`（0.2.0 から）
pub const BCF_SIZE_UNSUPPORTED: Code =
    Code::new("bcf.size.unsupported", Severity::Error, None, &[], &[]);

/// BCF で必須の値が指摘にないので、引数の値を書いた。field は BCF の要素か属性の名前
///
/// `bcf.value.assumed`（0.2.0 から）
pub const BCF_VALUE_ASSUMED: Code = Code::new(
    "bcf.value.assumed",
    Severity::Info,
    None,
    &[
        Param::new("field", ParamType::Text),
        Param::new("topic", ParamType::Text),
    ],
    &["field"],
);

/// BCF で必須の値が、指摘にも引数にもない。field は BCF の要素か属性の名前
///
/// `bcf.value.missing`（0.2.0 から）
pub const BCF_VALUE_MISSING: Code = Code::new(
    "bcf.value.missing",
    Severity::Error,
    None,
    &[
        Param::new("field", ParamType::Text),
        Param::new("topic", ParamType::Text),
    ],
    &["field"],
);

/// 読まない BCF の版。読むのは 3.0（決定 0220）
///
/// `bcf.version.unsupported`（0.2.0 から）
pub const BCF_VERSION_UNSUPPORTED: Code = Code::new(
    "bcf.version.unsupported",
    Severity::Error,
    None,
    &[Param::new("version", ParamType::Text)],
    &[],
);

/// 展開・復号で作ったバイト数が上限を超えた
///
/// `budget.decoded-bytes.exceeded`（0.0.0 から）
pub const BUDGET_DECODED_BYTES_EXCEEDED: Code = Code::new(
    "budget.decoded-bytes.exceeded",
    Severity::Error,
    None,
    &[
        Param::new("limit", ParamType::UInt),
        Param::new("requested", ParamType::UInt),
    ],
    &[],
);

/// 入れ子と再帰の深さが上限を超えた
///
/// `budget.depth.exceeded`（0.0.0 から）
pub const BUDGET_DEPTH_EXCEEDED: Code = Code::new(
    "budget.depth.exceeded",
    Severity::Error,
    None,
    &[
        Param::new("limit", ParamType::UInt),
        Param::new("requested", ParamType::UInt),
    ],
    &[],
);

/// L2・L3 で作った要素の数が上限を超えた
///
/// `budget.elements.exceeded`（0.0.0 から）
pub const BUDGET_ELEMENTS_EXCEEDED: Code = Code::new(
    "budget.elements.exceeded",
    Severity::Error,
    None,
    &[
        Param::new("limit", ParamType::UInt),
        Param::new("requested", ParamType::UInt),
    ],
    &[],
);

/// 受け取ったバイト数が上限を超えた
///
/// `budget.input-bytes.exceeded`（0.0.0 から）
pub const BUDGET_INPUT_BYTES_EXCEEDED: Code = Code::new(
    "budget.input-bytes.exceeded",
    Severity::Error,
    None,
    &[
        Param::new("limit", ParamType::UInt),
        Param::new("requested", ParamType::UInt),
    ],
    &[],
);

/// L0・L1 で読んだレコードの数が上限を超えた
///
/// `budget.records.exceeded`（0.0.0 から）
pub const BUDGET_RECORDS_EXCEEDED: Code = Code::new(
    "budget.records.exceeded",
    Severity::Error,
    None,
    &[
        Param::new("limit", ParamType::UInt),
        Param::new("requested", ParamType::UInt),
    ],
    &[],
);

/// 幾何の分割で作った点・線・面の数が上限を超えた
///
/// `budget.segments.exceeded`（0.0.0 から）
pub const BUDGET_SEGMENTS_EXCEEDED: Code = Code::new(
    "budget.segments.exceeded",
    Severity::Error,
    None,
    &[
        Param::new("limit", ParamType::UInt),
        Param::new("requested", ParamType::UInt),
    ],
    &[],
);

/// 時間の上限に達した。ネイティブでの補助で、止まる所は環境で変わる
///
/// `budget.time.exceeded`（0.0.0 から）
pub const BUDGET_TIME_EXCEEDED: Code =
    Code::new("budget.time.exceeded", Severity::Error, None, &[], &[]);

/// ほかの単位に入らない繰り返しの回数が上限を超えた
///
/// `budget.work.exceeded`（0.0.0 から）
pub const BUDGET_WORK_EXCEEDED: Code = Code::new(
    "budget.work.exceeded",
    Severity::Error,
    None,
    &[
        Param::new("limit", ParamType::UInt),
        Param::new("requested", ParamType::UInt),
    ],
    &[],
);

/// 同じ GlobalId がもうあるので、後の要素はインスタンスの番号で ID を作った
///
/// `building.global-id.exists`（0.2.0 から）
pub const BUILDING_GLOBAL_ID_EXISTS: Code = Code::new(
    "building.global-id.exists",
    Severity::Warning,
    None,
    &[Param::new("global_id", ParamType::Text)],
    &[],
);

/// GlobalId が 22 文字でないか、使えない文字を含むので、インスタンスの番号で ID を作った
///
/// `building.global-id.invalid`（0.2.0 から）
pub const BUILDING_GLOBAL_ID_INVALID: Code = Code::new(
    "building.global-id.invalid",
    Severity::Warning,
    None,
    &[Param::new("global_id", ParamType::Text)],
    &[],
);

/// 同じ ID の要素がもうあるので、要素を足さなかった
///
/// `building.id.exists`（0.2.0 から）
pub const BUILDING_ID_EXISTS: Code = Code::new(
    "building.id.exists",
    Severity::Error,
    None,
    &[Param::new("id", ParamType::Text)],
    &[],
);

/// 配置の連鎖が循環するので、配置なしにした
///
/// `building.placement.invalid`（0.2.0 から）
pub const BUILDING_PLACEMENT_INVALID: Code = Code::new(
    "building.placement.invalid",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[],
    &[],
);

/// 配置の親の要素が配置を持たないので、配置なしにした
///
/// `building.placement.missing`（0.2.0 から）
pub const BUILDING_PLACEMENT_MISSING: Code = Code::new(
    "building.placement.missing",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("id", ParamType::Text)],
    &[],
);

/// 連鎖が解けない配置（通り芯や線形の配置など）を含むので、配置なしにした
///
/// `building.placement.unsupported`（0.2.0 から）
pub const BUILDING_PLACEMENT_UNSUPPORTED: Code = Code::new(
    "building.placement.unsupported",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("kind", ParamType::Text)],
    &[],
);

/// 開く文書の名前が、もう使われている
///
/// `call.document.exists`（0.0.0 から）
pub const CALL_DOCUMENT_EXISTS: Code = Code::new(
    "call.document.exists",
    Severity::Error,
    None,
    &[Param::new("name", ParamType::Text)],
    &["name"],
);

/// 名指しした文書がない
///
/// `call.document.missing`（0.0.0 から）
pub const CALL_DOCUMENT_MISSING: Code = Code::new(
    "call.document.missing",
    Severity::Error,
    None,
    &[Param::new("name", ParamType::Text)],
    &["name"],
);

/// 文書の種類で使えない op。図面の op に建物モデルの文書を、建物モデルの op に図面の文書を渡した
///
/// `call.document.unsupported`（0.2.0 から）
pub const CALL_DOCUMENT_UNSUPPORTED: Code = Code::new(
    "call.document.unsupported",
    Severity::Error,
    None,
    &[
        Param::new("name", ParamType::Text),
        Param::new("op", ParamType::Text),
    ],
    &[],
);

/// 要求に、必要な欄がない
///
/// `call.field.missing`（0.0.0 から）
pub const CALL_FIELD_MISSING: Code = Code::new(
    "call.field.missing",
    Severity::Error,
    None,
    &[Param::new("pointer", ParamType::Text)],
    &[],
);

/// 要求に、このコアが知らない欄か、この形（wasm など）では使えない欄がある
///
/// `call.field.unsupported`（0.0.0 から）
pub const CALL_FIELD_UNSUPPORTED: Code = Code::new(
    "call.field.unsupported",
    Severity::Error,
    None,
    &[Param::new("pointer", ParamType::Text)],
    &[],
);

/// 入力の形式を判定できない。または、このコアでは読めない形式
///
/// `call.format.unsupported`（0.0.0 から）
pub const CALL_FORMAT_UNSUPPORTED: Code = Code::new(
    "call.format.unsupported",
    Severity::Error,
    None,
    &[Param::new("name", ParamType::Text)],
    &[],
);

/// 名指しした入力がない
///
/// `call.input.missing`（0.0.0 から）
pub const CALL_INPUT_MISSING: Code = Code::new(
    "call.input.missing",
    Severity::Error,
    None,
    &[Param::new("name", ParamType::Text)],
    &["name"],
);

/// 要求が UTF-8 の JSON として読めない（文法・重なったキー・深すぎる入れ子・UTF-8 でないバイト）
///
/// `call.json.invalid`（0.0.0 から）
pub const CALL_JSON_INVALID: Code = Code::new(
    "call.json.invalid",
    Severity::Error,
    None,
    &[Param::new("offset", ParamType::UInt)],
    &[],
);

/// このコア（wasm ではこのモジュール）にない op
///
/// `call.op.unsupported`（0.0.0 から）
pub const CALL_OP_UNSUPPORTED: Code = Code::new(
    "call.op.unsupported",
    Severity::Error,
    None,
    &[Param::new("op", ParamType::Text)],
    &[],
);

/// 実行の形が process のとき、子プロセスの CLI が落ちたか、答えが読めなかった。子プロセスは捨てたので、セッションを作り直す（決定 0013・0200）
///
/// `call.process.caught`（0.0.0 から）
pub const CALL_PROCESS_CAUGHT: Code =
    Code::new("call.process.caught", Severity::Error, None, &[], &[]);

/// 中断の合図を受けて止めた
///
/// `call.request.cancelled`（0.0.0 から）
pub const CALL_REQUEST_CANCELLED: Code =
    Code::new("call.request.cancelled", Severity::Error, None, &[], &[]);

/// 要求の値の型か範囲が、スキーマに合わない
///
/// `call.value.invalid`（0.0.0 から）
pub const CALL_VALUE_INVALID: Code = Code::new(
    "call.value.invalid",
    Severity::Error,
    None,
    &[Param::new("pointer", ParamType::Text)],
    &[],
);

/// 要求の版 v を、このコアが扱えない
///
/// `call.version.unsupported`（0.0.0 から）
pub const CALL_VERSION_UNSUPPORTED: Code = Code::new(
    "call.version.unsupported",
    Severity::Error,
    None,
    &[Param::new("version", ParamType::UInt)],
    &[],
);

/// カタログの入力が、カタログの形（schema/messages.json の catalog）に合わない
///
/// `catalog.file.invalid`（0.0.0 から）
pub const CATALOG_FILE_INVALID: Code = Code::new(
    "catalog.file.invalid",
    Severity::Error,
    None,
    &[Param::new("name", ParamType::Text)],
    &[],
);

/// 2007 より前の版の DXF・DWG に書き戻す文字か名前に、見出しのコード・ページで表せない文字がある。符号化して戻すと別の文字になるもの（ANSI_932 の ¥・‾・− など）も含む（決定 0192）
///
/// `change.character.unsupported`（0.0.0 から）
pub const CHANGE_CHARACTER_UNSUPPORTED: Code = Code::new(
    "change.character.unsupported",
    Severity::Error,
    None,
    &[Param::new("character", ParamType::Text)],
    &["character"],
);

/// 変更の一覧が持つ文書のハッシュが、今の文書のハッシュと違う（決定 0156）
///
/// `change.document.stale`（0.0.0 から）
pub const CHANGE_DOCUMENT_STALE: Code =
    Code::new("change.document.stale", Severity::Error, None, &[], &[]);

/// この種類の変更を、この種類の要素について、元の形式のレコードに書き戻せない。扱う種類と欄は docs/formats/dxf.md に書く
///
/// `change.edit.unsupported`（0.0.0 から）
pub const CHANGE_EDIT_UNSUPPORTED: Code = Code::new(
    "change.edit.unsupported",
    Severity::Error,
    None,
    &[
        Param::new("edit", ParamType::Text),
        Param::new("format", ParamType::Text),
        Param::new("kind", ParamType::Text),
    ],
    &[],
);

/// 変更が指す ID の要素か、足す先の空間・用紙が、今の図面にない（決定 0156）
///
/// `change.element.missing`（0.0.0 から）
pub const CHANGE_ELEMENT_MISSING: Code = Code::new(
    "change.element.missing",
    Severity::Error,
    None,
    &[Param::new("id", ParamType::Text)],
    &["id"],
);

/// 変更が持つ要素のハッシュが、今の要素のハッシュと違う（決定 0156）
///
/// `change.element.stale`（0.0.0 から）
pub const CHANGE_ELEMENT_STALE: Code = Code::new(
    "change.element.stale",
    Severity::Error,
    None,
    &[Param::new("id", ParamType::Text)],
    &["id"],
);

/// この形式の文書には、まだ変更を当てられない。今は DXF・DWG・JWW・SXF に当てる（決定 0139・0167・0168・0169）
///
/// `change.format.unsupported`（0.0.0 から）
pub const CHANGE_FORMAT_UNSUPPORTED: Code = Code::new(
    "change.format.unsupported",
    Severity::Error,
    None,
    &[Param::new("format", ParamType::Text)],
    &[],
);

/// 記号の定義の中に足す記号が、その定義を直接か間接に置く（自分を含む）ので、定義の入れ子が輪になる（#333）
///
/// `change.symbol-cycle.invalid`（0.0.0 から）
pub const CHANGE_SYMBOL_CYCLE_INVALID: Code = Code::new(
    "change.symbol-cycle.invalid",
    Severity::Error,
    None,
    &[
        Param::new("id", ParamType::Text),
        Param::new("name", ParamType::Text),
    ],
    &["id", "name"],
);

/// 置き換える図形が、図形とともに動く XDATA（1011〜1013・1041・1042）を持つので、形の変更を書き戻せない（決定 0154）
///
/// `change.xdata.unsupported`（0.0.0 から）
pub const CHANGE_XDATA_UNSUPPORTED: Code = Code::new(
    "change.xdata.unsupported",
    Severity::Error,
    None,
    &[
        Param::new("code", ParamType::Int),
        Param::new("kind", ParamType::Text),
    ],
    &[],
);

/// 座標系（srsName）を書いていないか、扱わない座標系なので、座標系を不明にし、形を移さなかった
///
/// `citygml.crs.unsupported`（0.2.0 から）
pub const CITYGML_CRS_UNSUPPORTED: Code = Code::new(
    "citygml.crs.unsupported",
    Severity::Warning,
    None,
    &[Param::new("name", ParamType::Text)],
    &[],
);

/// 地理の領域モデルに移さない要素なので、読まなかった
///
/// `citygml.element.unsupported`（0.2.0 から）
pub const CITYGML_ELEMENT_UNSUPPORTED: Code = Code::new(
    "citygml.element.unsupported",
    Severity::Warning,
    None,
    &[Param::new("element", ParamType::Text)],
    &["element"],
);

/// 形の参照（xlink:href）が同じ文書の gml:id を指さないので、参照した形を移さなかった
///
/// `citygml.reference.unsupported`（0.2.0 から）
pub const CITYGML_REFERENCE_UNSUPPORTED: Code = Code::new(
    "citygml.reference.unsupported",
    Severity::Warning,
    None,
    &[Param::new("href", ParamType::Text)],
    &["href"],
);

/// 要素の値が読めないので、その値か形を地理の領域モデルに移さなかった
///
/// `citygml.value.invalid`（0.2.0 から）
pub const CITYGML_VALUE_INVALID: Code = Code::new(
    "citygml.value.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("element", ParamType::Text),
        Param::new("field", ParamType::Text),
    ],
    &["element", "field"],
);

/// 図面モデルの形のない緩和曲線を、許容差の内の折れ線にして図に出した（決定 0231）。curve は種類の名前
///
/// `civil.curve.approximated`（0.2.0 から）
pub const CIVIL_CURVE_APPROXIMATED: Code = Code::new(
    "civil.curve.approximated",
    Severity::Info,
    None,
    &[
        Param::new("curve", ParamType::Text),
        Param::new("index", ParamType::UInt),
    ],
    &["curve"],
);

/// 線形の区間の曲線の種類から、位置を計算できない。区間の値は保った。curve は種類の名前
///
/// `civil.curve.unsupported`（0.2.0 から）
pub const CIVIL_CURVE_UNSUPPORTED: Code = Code::new(
    "civil.curve.unsupported",
    Severity::Warning,
    None,
    &[
        Param::new("curve", ParamType::Text),
        Param::new("index", ParamType::UInt),
    ],
    &["curve"],
);

/// 追加距離が、平面線形の範囲の外にある
///
/// `civil.distance.missing`（0.2.0 から）
pub const CIVIL_DISTANCE_MISSING: Code = Code::new(
    "civil.distance.missing",
    Severity::Warning,
    None,
    &[
        Param::new("distance", ParamType::Float),
        Param::new("length", ParamType::Float),
    ],
    &[],
);

/// 追加距離が縦断線形の範囲の外にあるので、高さを返さなかった
///
/// `civil.profile.missing`（0.2.0 から）
pub const CIVIL_PROFILE_MISSING: Code = Code::new(
    "civil.profile.missing",
    Severity::Info,
    None,
    &[Param::new("distance", ParamType::Float)],
    &[],
);

/// 線形の区間の初めの点が、前の区間の終わりから計算した点と、許容差より離れている。区間の初めの点をそのまま使った。gap はメートル
///
/// `civil.segment-start.invalid`（0.2.0 から）
pub const CIVIL_SEGMENT_START_INVALID: Code = Code::new(
    "civil.segment-start.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("gap", ParamType::Float),
        Param::new("index", ParamType::UInt),
        Param::new("tolerance", ParamType::Float),
    ],
    &[],
);

/// 線形の区間の値が使えない（長さが正でない、半径が 0 か有限でないなど）。index は区間の番号、field は欄の名前
///
/// `civil.segment.invalid`（0.2.0 から）
pub const CIVIL_SEGMENT_INVALID: Code = Code::new(
    "civil.segment.invalid",
    Severity::Error,
    None,
    &[
        Param::new("field", ParamType::Text),
        Param::new("index", ParamType::UInt),
    ],
    &["field"],
);

/// 測点の値が、測点の不連続のために線形の places 個の所にあり、どれかを指定していないので、最初の所にした
///
/// `civil.station.assumed`（0.2.0 から）
pub const CIVIL_STATION_ASSUMED: Code = Code::new(
    "civil.station.assumed",
    Severity::Info,
    None,
    &[
        Param::new("places", ParamType::UInt),
        Param::new("station", ParamType::Text),
    ],
    &[],
);

/// 測点の表記を読めない
///
/// `civil.station.invalid`（0.2.0 から）
pub const CIVIL_STATION_INVALID: Code = Code::new(
    "civil.station.invalid",
    Severity::Warning,
    None,
    &[Param::new("station", ParamType::Text)],
    &[],
);

/// 測点の値が、線形のどの所にもない
///
/// `civil.station.missing`（0.2.0 から）
pub const CIVIL_STATION_MISSING: Code = Code::new(
    "civil.station.missing",
    Severity::Warning,
    None,
    &[Param::new("station", ParamType::Text)],
    &[],
);

/// 地表面の三角形が、範囲の外の点の番号を指すので、図に出さなかった（決定 0231）
///
/// `civil.triangle.invalid`（0.2.0 から）
pub const CIVIL_TRIANGLE_INVALID: Code = Code::new(
    "civil.triangle.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("surface", ParamType::Text),
        Param::new("triangle", ParamType::UInt),
    ],
    &["surface"],
);

/// 別の形式に書くとき、記号の属性を、記号から離した文字として書いた（決定 0206）
///
/// `convert.attribute.drawn`（0.0.0 から）
pub const CONVERT_ATTRIBUTE_DRAWN: Code = Code::new(
    "convert.attribute.drawn",
    Severity::Info,
    None,
    &[Param::new("format", ParamType::Text)],
    &[],
);

/// 別の形式に書くとき、この種類の要素か定義を書けないので落とした（決定 0206）。件数は種類ごとに数える
///
/// `convert.element.unsupported`（0.0.0 から）
pub const CONVERT_ELEMENT_UNSUPPORTED: Code = Code::new(
    "convert.element.unsupported",
    Severity::Warning,
    None,
    &[
        Param::new("format", ParamType::Text),
        Param::new("kind", ParamType::Key),
    ],
    &["kind"],
);

/// この形式には、まだ別の形式から書けない（決定 0206）
///
/// `convert.format.unsupported`（0.0.0 から）
pub const CONVERT_FORMAT_UNSUPPORTED: Code = Code::new(
    "convert.format.unsupported",
    Severity::Error,
    None,
    &[Param::new("format", ParamType::Text)],
    &[],
);

/// 別の形式に書くとき、横と縦の尺度の違うビューを、ビューの要素を用紙に写した要素にして書いた。曲線は用紙で 0.02 mm に収まる折れ線にした（決定 0238）
///
/// `convert.view.approximated`（0.2.0 から）
pub const CONVERT_VIEW_APPROXIMATED: Code = Code::new(
    "convert.view.approximated",
    Severity::Info,
    None,
    &[Param::new("format", ParamType::Text)],
    &[],
);

/// 渡された測地系の変換のパラメーター（TKY2JGD・PatchJGD の形）の行が壊れていて読めない
///
/// `crs.datum-grid.invalid`（0.2.0 から）
pub const CRS_DATUM_GRID_INVALID: Code = Code::new(
    "crs.datum-grid.invalid",
    Severity::Error,
    None,
    &[Param::new("line", ParamType::UInt)],
    &[],
);

/// 座標系を JGD2011 に直すのに要る変換のパラメーター（TKY2JGD・PatchJGD）が渡されていないので、直さなかった
///
/// `crs.datum-grid.missing`（0.2.0 から）
pub const CRS_DATUM_GRID_MISSING: Code = Code::new(
    "crs.datum-grid.missing",
    Severity::Warning,
    None,
    &[
        Param::new("crs", ParamType::UInt),
        Param::new("grid", ParamType::Text),
    ],
    &["crs", "grid"],
);

/// 点が変換のパラメーターの格子の範囲の外にあるか、囲む格子点が欠けているので、JGD2011 に直さなかった
///
/// `crs.datum-shift.missing`（0.2.0 から）
pub const CRS_DATUM_SHIFT_MISSING: Code = Code::new(
    "crs.datum-shift.missing",
    Severity::Warning,
    None,
    &[
        Param::new("grid", ParamType::Text),
        Param::new("latitude", ParamType::Float),
        Param::new("longitude", ParamType::Float),
    ],
    &["grid"],
);

/// 渡されたジオイドの格子（ISG format 2.0）が壊れていて読めない
///
/// `crs.geoid.invalid`（0.2.0 から）
pub const CRS_GEOID_INVALID: Code = Code::new(
    "crs.geoid.invalid",
    Severity::Error,
    None,
    &[Param::new("key", ParamType::Text)],
    &["key"],
);

/// 渡されたジオイドの格子は、扱わない形（点の並び・投影の座標・フィートの値・ほかの並びや版）なので読まなかった
///
/// `crs.geoid.unsupported`（0.2.0 から）
pub const CRS_GEOID_UNSUPPORTED: Code = Code::new(
    "crs.geoid.unsupported",
    Severity::Error,
    None,
    &[
        Param::new("key", ParamType::Text),
        Param::new("value", ParamType::Text),
    ],
    &["key", "value"],
);

/// 基準の違う高さ（標高と楕円体高など）の差は、引き算で決まらないので求めなかった
///
/// `crs.height-datum.unsupported`（0.2.0 から）
pub const CRS_HEIGHT_DATUM_UNSUPPORTED: Code = Code::new(
    "crs.height-datum.unsupported",
    Severity::Warning,
    None,
    &[
        Param::new("first", ParamType::Text),
        Param::new("second", ParamType::Text),
    ],
    &["first", "second"],
);

/// 形式が標高の成果（測地成果2011・2024）を書いていないので、成果を仮定した
///
/// `crs.height-realization.assumed`（0.2.0 から）
pub const CRS_HEIGHT_REALIZATION_ASSUMED: Code = Code::new(
    "crs.height-realization.assumed",
    Severity::Info,
    None,
    &[Param::new("realization", ParamType::Text)],
    &["realization"],
);

/// 成果の違う標高を一緒に使った。同じ地点でも数十 cm 違うことがある
///
/// `crs.height-realization.mixed`（0.2.0 から）
pub const CRS_HEIGHT_REALIZATION_MIXED: Code = Code::new(
    "crs.height-realization.mixed",
    Severity::Warning,
    None,
    &[
        Param::new("first", ParamType::Text),
        Param::new("second", ParamType::Text),
    ],
    &["first", "second"],
);

/// CSV に、その文字コードで直せないバイトがあるので、置き換えの字にした（#391）
///
/// `csv.encoding.invalid`（0.2.0 から）
pub const CSV_ENCODING_INVALID: Code = Code::new(
    "csv.encoding.invalid",
    Severity::Warning,
    None,
    &[Param::new("encoding", ParamType::Text)],
    &[],
);

/// CSV の引用符が閉じていないので、終わりまでを 1 つの値にした（#391）。line は引用符の始まりの行
///
/// `csv.quote.invalid`（0.2.0 から）
pub const CSV_QUOTE_INVALID: Code = Code::new(
    "csv.quote.invalid",
    Severity::Warning,
    None,
    &[Param::new("line", ParamType::UInt)],
    &[],
);

/// 管理ファイルを XML として読めないので、管理ファイルの記録に移さなかった
///
/// `delivery.file.invalid`（0.2.0 から）
pub const DELIVERY_FILE_INVALID: Code = Code::new(
    "delivery.file.invalid",
    Severity::Error,
    None,
    &[Param::new("name", ParamType::Text)],
    &[],
);

/// 成果品の中の名前に、空の部分・.・..・\ を含むか、/ で始まるので、そのファイルを読まなかった
///
/// `delivery.name.unsafe`（0.2.0 から）
pub const DELIVERY_NAME_UNSAFE: Code = Code::new(
    "delivery.name.unsafe",
    Severity::Warning,
    None,
    &[Param::new("name", ParamType::Text)],
    &[],
);

/// 点群と TIN の水平の座標系が違うので、標高の差を求めなかった
///
/// `deviation.crs.mixed`（0.2.0 から）
pub const DEVIATION_CRS_MIXED: Code = Code::new(
    "deviation.crs.mixed",
    Severity::Error,
    None,
    &[
        Param::new("points", ParamType::UInt),
        Param::new("surface", ParamType::UInt),
    ],
    &[],
);

/// 線種に添える文字の高さが有限の実数で表せず、結果に含めなかった
///
/// `drawing.mark-height.dropped`（0.0.0 から）
pub const DRAWING_MARK_HEIGHT_DROPPED: Code = Code::new(
    "drawing.mark-height.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("name", ParamType::Text)],
    &["name"],
);

/// 記号の配置が、道筋の外側の配置と同じ定義を置く（循環）ので、中をたどらなかった
///
/// `drawing.placement.invalid`（0.0.0 から）
pub const DRAWING_PLACEMENT_INVALID: Code = Code::new(
    "drawing.placement.invalid",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("id", ParamType::Text)],
    &["id"],
);

/// この DWG から読んでいないオブジェクトの変更や、まだ符号化できない値の変更は、DWG に書き戻せない
///
/// `dwg.change.unsupported`（0.0.0 から）
pub const DWG_CHANGE_UNSUPPORTED: Code = Code::new(
    "dwg.change.unsupported",
    Severity::Error,
    None,
    &[Param::new("kind", ParamType::Text)],
    &[],
);

/// DWG の CRC が、記録された値と合わない。読むのは続ける
///
/// `dwg.crc.invalid`（0.0.0 から）
pub const DWG_CRC_INVALID: Code = Code::new(
    "dwg.crc.invalid",
    Severity::Warning,
    None,
    &[Param::new("part", ParamType::Text)],
    &["part"],
);

/// DWG のオブジェクトに置くデータの、型を読めない
///
/// `dwg.data.invalid`（0.0.0 から）
pub const DWG_DATA_INVALID: Code = Code::new(
    "dwg.data.invalid",
    Severity::Error,
    None,
    &[Param::new("record", ParamType::UInt)],
    &[],
);

/// R2000（AC1015）でない版の DWG は、まだ変えられない
///
/// `dwg.edit.unsupported`（0.0.0 から）
pub const DWG_EDIT_UNSUPPORTED: Code = Code::new(
    "dwg.edit.unsupported",
    Severity::Error,
    None,
    &[Param::new("version", ParamType::Text)],
    &[],
);

/// DWG の見出し・オブジェクトの地図・オブジェクトの区切りを読めない
///
/// `dwg.file.invalid`（0.0.0 から）
pub const DWG_FILE_INVALID: Code = Code::new(
    "dwg.file.invalid",
    Severity::Error,
    None,
    &[Param::new("part", ParamType::Text)],
    &[],
);

/// DWG の図形が指す画層のハンドルに LAYER がないので、画層の名前（8）を置かなかった
///
/// `dwg.layer.missing`（0.0.0 から）
pub const DWG_LAYER_MISSING: Code = Code::new(
    "dwg.layer.missing",
    Severity::Warning,
    None,
    &[Param::new("type", ParamType::Int)],
    &["type"],
);

/// DWG のオブジェクトの欄を読めないので、ハンドルだけを表に置いた
///
/// `dwg.object.invalid`（0.0.0 から）
pub const DWG_OBJECT_INVALID: Code = Code::new(
    "dwg.object.invalid",
    Severity::Warning,
    None,
    &[Param::new("type", ParamType::Int)],
    &["type"],
);

/// DWG の変更で名指ししたオブジェクトがない
///
/// `dwg.object.missing`（0.0.0 から）
pub const DWG_OBJECT_MISSING: Code = Code::new(
    "dwg.object.missing",
    Severity::Error,
    None,
    &[Param::new("record", ParamType::UInt)],
    &[],
);

/// DWG の図形が指す文字の体裁・ブロックなどのハンドルに、その型の表のレコードがないので、名前を置かなかった
///
/// `dwg.record.missing`（0.0.0 から）
pub const DWG_RECORD_MISSING: Code = Code::new(
    "dwg.record.missing",
    Severity::Warning,
    None,
    &[
        Param::new("code", ParamType::Int),
        Param::new("type", ParamType::Int),
    ],
    &["type", "code"],
);

/// R2010 からの DWG の表のスタイル・セルのスタイル・内容の書式は、まだ読めないので、表とセルの書式と文字の回転を図面モデルに移さなかった
///
/// `dwg.table-format.unsupported`（0.0.0 から）
pub const DWG_TABLE_FORMAT_UNSUPPORTED: Code = Code::new(
    "dwg.table-format.unsupported",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[],
    &[],
);

/// まだ扱えない型の DWG のオブジェクトを、種類を空にして表に置いた
///
/// `dwg.type.unsupported`（0.0.0 から）
pub const DWG_TYPE_UNSUPPORTED: Code = Code::new(
    "dwg.type.unsupported",
    Severity::Warning,
    None,
    &[Param::new("type", ParamType::Int)],
    &["type"],
);

/// R2000・R2004・R2010・R2013・R2018 でない版の DWG は、まだ読めない
///
/// `dwg.version.unsupported`（0.0.0 から）
pub const DWG_VERSION_UNSUPPORTED: Code = Code::new(
    "dwg.version.unsupported",
    Severity::Error,
    None,
    &[Param::new("version", ParamType::Text)],
    &[],
);

/// DWG のオブジェクトの拡張データ（EED）を読めないので、その登録アプリケーションの拡張データを置かなかった
///
/// `dwg.xdata.invalid`（0.0.0 から）
pub const DWG_XDATA_INVALID: Code = Code::new(
    "dwg.xdata.invalid",
    Severity::Warning,
    None,
    &[Param::new("type", ParamType::Int)],
    &["type"],
);

/// 弧長寸法（ARC_DIMENSION）の引出線を、図面モデルに移さなかった。寸法は移した
///
/// `dxf.arc-dimension-leader.dropped`（0.0.0 から）
pub const DXF_ARC_DIMENSION_LEADER_DROPPED: Code = Code::new(
    "dxf.arc-dimension-leader.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[],
    &[],
);

/// バイナリの DXF の変更で、値をグループ・コードの型のバイト列にできない
///
/// `dxf.binary-value.unsupported`（0.0.0 から）
pub const DXF_BINARY_VALUE_UNSUPPORTED: Code = Code::new(
    "dxf.binary-value.unsupported",
    Severity::Error,
    None,
    &[
        Param::new("code", ParamType::Int),
        Param::new("record", ParamType::UInt),
    ],
    &[],
);

/// バイナリの DXF の、`offset` バイト目から始まるタグが途中で切れているか、実数の値が有限でない
///
/// `dxf.binary.invalid`（0.0.0 から）
pub const DXF_BINARY_INVALID: Code = Code::new(
    "dxf.binary.invalid",
    Severity::Error,
    None,
    &[Param::new("offset", ParamType::UInt)],
    &[],
);

/// バイナリの DXF は、まだ読めない（読めるようにしたので使わない）
///
/// `dxf.binary.unsupported`（0.0.0 から）
#[deprecated = "廃止した code（catalog/codes.toml）"]
pub const DXF_BINARY_UNSUPPORTED: Code =
    Code::new("dxf.binary.unsupported", Severity::Error, None, &[], &[]);

/// 自分を直接か間接に配置するブロックの定義を、展開せずに図面モデルに移した
///
/// `dxf.block-cycle.invalid`（0.0.0 から）
pub const DXF_BLOCK_CYCLE_INVALID: Code = Code::new(
    "dxf.block-cycle.invalid",
    Severity::Warning,
    None,
    &[Param::new("name", ParamType::Text)],
    &["name"],
);

/// 配置が指すブロックの定義がないので、配置だけを図面モデルに移した
///
/// `dxf.block.missing`（0.0.0 から）
pub const DXF_BLOCK_MISSING: Code = Code::new(
    "dxf.block.missing",
    Severity::Warning,
    None,
    &[Param::new("name", ParamType::Text)],
    &["name"],
);

/// 表のセルの値のうち、図面モデルが持たない型（オブジェクトの参照・バイト列・結果の並び・一般の値）の値を移さなかった。表示する文字は移した
///
/// `dxf.cell-value.dropped`（0.0.0 から）
pub const DXF_CELL_VALUE_DROPPED: Code = Code::new(
    "dxf.cell-value.dropped",
    Severity::Info,
    Some(Fidelity::Dropped),
    &[Param::new("type", ParamType::Int)],
    &["type"],
);

/// 図面モデルの要素の変更を、元の図形のレコードに書き戻せない。扱う種類と欄は docs/formats/dxf.md に書く（決定 0139）（change.edit.unsupported に移したので使わない）
///
/// `dxf.change.unsupported`（0.0.0 から）
#[deprecated = "廃止した code（catalog/codes.toml）"]
pub const DXF_CHANGE_UNSUPPORTED: Code = Code::new(
    "dxf.change.unsupported",
    Severity::Error,
    None,
    &[Param::new("kind", ParamType::Text)],
    &[],
);

/// 記号の切り抜き（SPATIAL_FILTER）を読めず、切り抜かずに移した
///
/// `dxf.clip.dropped`（0.0.0 から）
pub const DXF_CLIP_DROPPED: Code = Code::new(
    "dxf.clip.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("code", ParamType::Int)],
    &["code"],
);

/// 2007 より前の版で、対応表のないコード・ページの ASCII でない文字を読めないので、その文字の図形を図面モデルに移さなかった
///
/// `dxf.code-page.unsupported`（0.0.0 から）
pub const DXF_CODE_PAGE_UNSUPPORTED: Code = Code::new(
    "dxf.code-page.unsupported",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("kind", ParamType::Text)],
    &["kind"],
);

/// 図面モデルの変更が指す出所の要素が、今のファイルの実寸の空間の図形にない（決定 0139）（change.element.missing に移したので使わない）
///
/// `dxf.element.missing`（0.0.0 から）
#[deprecated = "廃止した code（catalog/codes.toml）"]
pub const DXF_ELEMENT_MISSING: Code =
    Code::new("dxf.element.missing", Severity::Error, None, &[], &[]);

/// 出力先の DXF の版にない種類の図形を、書かなかった
///
/// `dxf.entity.dropped`（0.0.0 から）
pub const DXF_ENTITY_DROPPED: Code = Code::new(
    "dxf.entity.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("kind", ParamType::Text)],
    &["kind"],
);

/// 図形の拡張辞書の、XRECORD でない項目を、図面モデルに移さなかった
///
/// `dxf.extension-dictionary.dropped`（0.0.0 から）
pub const DXF_EXTENSION_DICTIONARY_DROPPED: Code = Code::new(
    "dxf.extension-dictionary.dropped",
    Severity::Info,
    Some(Fidelity::Dropped),
    &[
        Param::new("key", ParamType::Text),
        Param::new("kind", ParamType::Text),
    ],
    &["key", "kind"],
);

/// 図形・画層・線種の拡張辞書の項目のうち、図面モデルの欄にする値で欄がまだないものと、AutoCAD の版の往復の XRECORD を、図面モデルに移さなかった。元の形式の値は残る（決定 0157）
///
/// `dxf.extension-dictionary.unsupported`（0.0.0 から）
pub const DXF_EXTENSION_DICTIONARY_UNSUPPORTED: Code = Code::new(
    "dxf.extension-dictionary.unsupported",
    Severity::Info,
    Some(Fidelity::Dropped),
    &[Param::new("key", ParamType::Text)],
    &["key"],
);

/// 出力先の DXF の版にないグループ・コードのタグを、書かなかった
///
/// `dxf.group-code.dropped`（0.0.0 から）
pub const DXF_GROUP_CODE_DROPPED: Code = Code::new(
    "dxf.group-code.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[
        Param::new("code", ParamType::Int),
        Param::new("kind", ParamType::Text),
    ],
    &["kind", "code"],
);

/// DXF の変更で名指ししたグループ・コードのタグが、レコードにない
///
/// `dxf.group-code.missing`（0.0.0 から）
pub const DXF_GROUP_CODE_MISSING: Code = Code::new(
    "dxf.group-code.missing",
    Severity::Error,
    None,
    &[
        Param::new("code", ParamType::Int),
        Param::new("record", ParamType::UInt),
    ],
    &[],
);

/// DXF の見出しに $HANDSEED がないか、16 進のハンドルとして読めないので、新しいハンドルを決められない
///
/// `dxf.handle-seed.missing`（0.0.0 から）
pub const DXF_HANDLE_SEED_MISSING: Code =
    Code::new("dxf.handle-seed.missing", Severity::Error, None, &[], &[]);

/// SPLINE の辺を持つ HATCH を、図面モデルに移さなかった
///
/// `dxf.hatch-spline.unsupported`（0.0.0 から）
#[deprecated = "廃止した code（catalog/codes.toml）"]
pub const DXF_HATCH_SPLINE_UNSUPPORTED: Code = Code::new(
    "dxf.hatch-spline.unsupported",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[],
    &[],
);

/// 出力先の DXF の版で書けない値を持つ見出しの変数を、書かなかった
///
/// `dxf.header-variable.dropped`（0.0.0 から）
pub const DXF_HEADER_VARIABLE_DROPPED: Code = Code::new(
    "dxf.header-variable.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("name", ParamType::Text)],
    &["name"],
);

/// MULTILEADER の線ごとの色・線種・線幅・端部記号の上書きを、図面モデルに移さなかった。線は注記全体の指定で描く
///
/// `dxf.leader-line-style.dropped`（0.0.0 から）
pub const DXF_LEADER_LINE_STYLE_DROPPED: Code = Code::new(
    "dxf.leader-line-style.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[],
    &[],
);

/// 図面モデルにまだ移せない種類の図形を、移さなかった
///
/// `dxf.lift.dropped`（0.0.0 から）
pub const DXF_LIFT_DROPPED: Code = Code::new(
    "dxf.lift.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("kind", ParamType::Text)],
    &["kind"],
);

/// 値がないか読めないか範囲の外なので、その図形を図面モデルに移さなかった
///
/// `dxf.lift.invalid`（0.0.0 から）
pub const DXF_LIFT_INVALID: Code = Code::new(
    "dxf.lift.invalid",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[
        Param::new("code", ParamType::Int),
        Param::new("kind", ParamType::Text),
    ],
    &["kind", "code"],
);

/// 線種の要素の形か文字を読めないので、図面モデルに移さなかった。線と間の長さは移した
///
/// `dxf.line-type-element.dropped`（0.0.0 から）
pub const DXF_LINE_TYPE_ELEMENT_DROPPED: Code = Code::new(
    "dxf.line-type-element.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("name", ParamType::Text)],
    &["name"],
);

/// 段組みの MTEXT と複数行の属性を、図面モデルに移さなかった
///
/// `dxf.mtext-columns.unsupported`（0.0.0 から）
pub const DXF_MTEXT_COLUMNS_UNSUPPORTED: Code = Code::new(
    "dxf.mtext-columns.unsupported",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[],
    &[],
);

/// Z が 0 でないか、押し出しの向きが Z でない図形を、平面の図面モデルに移さなかった
///
/// `dxf.non-planar.dropped`（0.0.0 から）
pub const DXF_NON_PLANAR_DROPPED: Code = Code::new(
    "dxf.non-planar.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("kind", ParamType::Text)],
    &["kind"],
);

/// この DXF の同じ種類のレコードから読んでいないオブジェクトの変更は、DXF に書き戻せない
///
/// `dxf.object.unsupported`（0.0.0 から）
pub const DXF_OBJECT_UNSUPPORTED: Code = Code::new(
    "dxf.object.unsupported",
    Severity::Error,
    None,
    &[Param::new("kind", ParamType::Text)],
    &[],
);

/// 長さの単位が分からず用紙の mm に直せないので、ペーパー空間の図形を図面モデルに移さなかった
///
/// `dxf.paper-space.dropped`（0.0.0 から）
pub const DXF_PAPER_SPACE_DROPPED: Code = Code::new(
    "dxf.paper-space.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("kind", ParamType::Text)],
    &["kind"],
);

/// 有限でない実数は、DXF の値に書けない
///
/// `dxf.real.unsupported`（0.0.0 から）
pub const DXF_REAL_UNSUPPORTED: Code = Code::new(
    "dxf.real.unsupported",
    Severity::Error,
    None,
    &[
        Param::new("code", ParamType::Int),
        Param::new("record", ParamType::UInt),
    ],
    &[],
);

/// DXF の変更で名指ししたレコードがない
///
/// `dxf.record.missing`（0.0.0 から）
pub const DXF_RECORD_MISSING: Code = Code::new(
    "dxf.record.missing",
    Severity::Error,
    None,
    &[Param::new("record", ParamType::UInt)],
    &[],
);

/// DXF の構造を区切るレコード（SECTION・ENDSEC・EOF）は、追加・削除できない
///
/// `dxf.record.unsafe`（0.0.0 から）
pub const DXF_RECORD_UNSAFE: Code = Code::new(
    "dxf.record.unsafe",
    Severity::Error,
    None,
    &[Param::new("record", ParamType::UInt)],
    &[],
);

/// 出力先の DXF の版にないセクションを、書かなかった
///
/// `dxf.section.dropped`（0.0.0 から）
pub const DXF_SECTION_DROPPED: Code = Code::new(
    "dxf.section.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("name", ParamType::Text)],
    &["name"],
);

/// 出力先の DXF の版にない表を、書かなかった
///
/// `dxf.table.dropped`（0.0.0 から）
pub const DXF_TABLE_DROPPED: Code = Code::new(
    "dxf.table.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("name", ParamType::Text)],
    &["name"],
);

/// DXF のグループ・コードの行が整数でないか、値の行がない
///
/// `dxf.tag.invalid`（0.0.0 から）
pub const DXF_TAG_INVALID: Code = Code::new(
    "dxf.tag.invalid",
    Severity::Error,
    None,
    &[Param::new("line", ParamType::UInt)],
    &[],
);

/// DXF の変更で名指ししたタグが、レコードにない
///
/// `dxf.tag.missing`（0.0.0 から）
pub const DXF_TAG_MISSING: Code = Code::new(
    "dxf.tag.missing",
    Severity::Error,
    None,
    &[
        Param::new("record", ParamType::UInt),
        Param::new("tag", ParamType::UInt),
    ],
    &[],
);

/// コード・ページの版に書くため、ASCII でない文字を `\U+XXXX` の表記にした
///
/// `dxf.text.equivalent`（0.0.0 から）
pub const DXF_TEXT_EQUIVALENT: Code = Code::new(
    "dxf.text.equivalent",
    Severity::Info,
    Some(Fidelity::Equivalent),
    &[],
    &[],
);

/// UTF-8 の版の値が UTF-8 として読めないので、バイト列のまま書いた
///
/// `dxf.text.invalid`（0.0.0 から）
pub const DXF_TEXT_INVALID: Code = Code::new("dxf.text.invalid", Severity::Warning, None, &[], &[]);

/// 見出しの $INSUNITS がないか長さの単位でないので、実寸の単位を不明とした
///
/// `dxf.units.missing`（0.0.0 から）
pub const DXF_UNITS_MISSING: Code = Code::new("dxf.units.missing", Severity::Info, None, &[], &[]);

/// グループ・コードの型どおりに読めない値を、オブジェクトの表に文字列のまま置いた
///
/// `dxf.value.invalid`（0.0.0 から）
pub const DXF_VALUE_INVALID: Code = Code::new(
    "dxf.value.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("code", ParamType::Int),
        Param::new("kind", ParamType::Text),
    ],
    &["kind", "code"],
);

/// 書くとタグやレコードの区切りが変わる値（改行を含む値と、グループ・コード 0 の値の変更）を、DXF に書かなかった
///
/// `dxf.value.unsafe`（0.0.0 から）
pub const DXF_VALUE_UNSAFE: Code = Code::new(
    "dxf.value.unsafe",
    Severity::Error,
    None,
    &[
        Param::new("code", ParamType::Int),
        Param::new("record", ParamType::UInt),
    ],
    &[],
);

/// この組の DXF の版の変更は、まだ扱えない
///
/// `dxf.version.unsupported`（0.0.0 から）
pub const DXF_VERSION_UNSUPPORTED: Code = Code::new(
    "dxf.version.unsupported",
    Severity::Error,
    None,
    &[
        Param::new("from", ParamType::Text),
        Param::new("to", ParamType::Text),
    ],
    &[],
);

/// まだ扱えない設定を持つビューポートを、図面モデルに移さなかった
///
/// `dxf.viewport.unsupported`（0.0.0 から）
pub const DXF_VIEWPORT_UNSUPPORTED: Code = Code::new(
    "dxf.viewport.unsupported",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("code", ParamType::Int)],
    &["code"],
);

/// 置き換える図形が、図形とともに動く XDATA（1011〜1013・1041・1042）を持つので、形の変更を書き戻せない（決定 0154）（change.xdata.unsupported に移したので使わない）
///
/// `dxf.xdata.unsupported`（0.0.0 から）
#[deprecated = "廃止した code（catalog/codes.toml）"]
pub const DXF_XDATA_UNSUPPORTED: Code = Code::new(
    "dxf.xdata.unsupported",
    Severity::Error,
    None,
    &[
        Param::new("code", ParamType::Int),
        Param::new("kind", ParamType::Text),
    ],
    &[],
);

/// 点群の領域モデルに持たない点の欄か、一部のスキャンだけが持つ属性を、移さなかった（決定 0232）
///
/// `e57.attribute.dropped`（0.2.0 から）
pub const E57_ATTRIBUTE_DROPPED: Code = Code::new(
    "e57.attribute.dropped",
    Severity::Info,
    Some(Fidelity::Dropped),
    &[Param::new("attribute", ParamType::Text)],
    &["attribute"],
);

/// 直交の座標も球の座標も持たないスキャンなので、移さなかった
///
/// `e57.coordinates.missing`（0.2.0 から）
pub const E57_COORDINATES_MISSING: Code = Code::new(
    "e57.coordinates.missing",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("scan", ParamType::UInt)],
    &[],
);

/// 座標系の記述から EPSG のコードを読めないので、座標系を不明にした
///
/// `e57.crs.invalid`（0.2.0 から）
pub const E57_CRS_INVALID: Code = Code::new("e57.crs.invalid", Severity::Warning, None, &[], &[]);

/// 座標系の記述（coordinateMetadata）がないので、座標系を不明にした
///
/// `e57.crs.missing`（0.2.0 から）
pub const E57_CRS_MISSING: Code = Code::new("e57.crs.missing", Severity::Warning, None, &[], &[]);

/// 扱わない水平の座標系（平面直角座標系の JGD2011 でないもの）なので、座標系を不明にした
///
/// `e57.crs.unsupported`（0.2.0 から）
pub const E57_CRS_UNSUPPORTED: Code = Code::new(
    "e57.crs.unsupported",
    Severity::Warning,
    None,
    &[Param::new("epsg", ParamType::UInt)],
    &[],
);

/// ASTM-E57 の始まりか、ページ・XML・点のデータが読めない
///
/// `e57.file.invalid`（0.2.0 から）
pub const E57_FILE_INVALID: Code = Code::new(
    "e57.file.invalid",
    Severity::Error,
    None,
    &[Param::new("reason", ParamType::Text)],
    &[],
);

/// 画像（images2D）は点群の領域モデルに持たないので、移さなかった
///
/// `e57.image.dropped`（0.2.0 から）
pub const E57_IMAGE_DROPPED: Code = Code::new(
    "e57.image.dropped",
    Severity::Info,
    Some(Fidelity::Dropped),
    &[Param::new("images", ParamType::UInt)],
    &[],
);

/// 座標の状態が無効か、向きだけの点を、移さなかった
///
/// `e57.point.dropped`（0.2.0 から）
pub const E57_POINT_DROPPED: Code = Code::new(
    "e57.point.dropped",
    Severity::Info,
    Some(Fidelity::Dropped),
    &[Param::new("points", ParamType::UInt)],
    &[],
);

/// 点のデータが、スキャンの点の数より短いか壊れているので、読めた点だけを読んだ
///
/// `e57.points.invalid`（0.2.0 から）
pub const E57_POINTS_INVALID: Code = Code::new(
    "e57.points.invalid",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[
        Param::new("expected", ParamType::UInt),
        Param::new("found", ParamType::UInt),
    ],
    &[],
);

/// 扱わない高さの座標系なので、高さの座標系を不明にした
///
/// `e57.vertical-crs.unsupported`（0.2.0 から）
pub const E57_VERTICAL_CRS_UNSUPPORTED: Code = Code::new(
    "e57.vertical-crs.unsupported",
    Severity::Warning,
    None,
    &[Param::new("epsg", ParamType::UInt)],
    &[],
);

/// 抽象の実体を、そのままインスタンスにしている
///
/// `express.abstract.invalid`（0.2.0 から）
pub const EXPRESS_ABSTRACT_INVALID: Code = Code::new(
    "express.abstract.invalid",
    Severity::Warning,
    None,
    &[Param::new("kind", ParamType::Text)],
    &["kind"],
);

/// インスタンスの属性の数が、EXPRESS の実体の属性の数と違う
///
/// `express.attribute-count.invalid`（0.2.0 から）
pub const EXPRESS_ATTRIBUTE_COUNT_INVALID: Code = Code::new(
    "express.attribute-count.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("expected", ParamType::UInt),
        Param::new("found", ParamType::UInt),
        Param::new("kind", ParamType::Text),
    ],
    &["kind", "found"],
);

/// インスタンスの実体の名前が、EXPRESS にない
///
/// `express.entity.missing`（0.2.0 から）
pub const EXPRESS_ENTITY_MISSING: Code = Code::new(
    "express.entity.missing",
    Severity::Warning,
    None,
    &[Param::new("kind", ParamType::Text)],
    &["kind"],
);

/// 利用者が渡した EXPRESS を読めないか、宣言のない名前・循環する型を使っている
///
/// `express.file.invalid`（0.2.0 から）
pub const EXPRESS_FILE_INVALID: Code = Code::new(
    "express.file.invalid",
    Severity::Error,
    None,
    &[
        Param::new("line", ParamType::UInt),
        Param::new("near", ParamType::Text),
    ],
    &[],
);

/// EXPRESS の WHERE・UNIQUE・RULE の規則は評価しない
///
/// `express.rule.unsupported`（0.2.0 から）
pub const EXPRESS_RULE_UNSUPPORTED: Code = Code::new(
    "express.rule.unsupported",
    Severity::Info,
    None,
    &[Param::new("rules", ParamType::UInt)],
    &[],
);

/// IFC の FILE_SCHEMA の名前が、渡した EXPRESS のスキーマの名前と違う。そのままチェックする
///
/// `express.schema.mixed`（0.2.0 から）
pub const EXPRESS_SCHEMA_MIXED: Code = Code::new(
    "express.schema.mixed",
    Severity::Warning,
    None,
    &[
        Param::new("file_schema", ParamType::Text),
        Param::new("schema", ParamType::Text),
    ],
    &[],
);

/// 属性の値が、EXPRESS の型に合わない
///
/// `express.value.invalid`（0.2.0 から）
pub const EXPRESS_VALUE_INVALID: Code = Code::new(
    "express.value.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("attribute", ParamType::Text),
        Param::new("expected", ParamType::Text),
        Param::new("kind", ParamType::Text),
        Param::new("position", ParamType::UInt),
    ],
    &["kind", "position"],
);

/// 省略できない属性が $ になっている
///
/// `express.value.missing`（0.2.0 から）
pub const EXPRESS_VALUE_MISSING: Code = Code::new(
    "express.value.missing",
    Severity::Warning,
    None,
    &[
        Param::new("attribute", ParamType::Text),
        Param::new("kind", ParamType::Text),
        Param::new("position", ParamType::UInt),
    ],
    &["kind", "position"],
);

/// 禁じる仕様の、適用の条件に合う要素がある（指摘）
///
/// `ids.applicability.exists`（0.2.0 から）
pub const IDS_APPLICABILITY_EXISTS: Code = Code::new(
    "ids.applicability.exists",
    Severity::Error,
    None,
    &[Param::new("specification", ParamType::Text)],
    &[],
);

/// 必須の仕様に、適用の条件に合う要素がない（指摘）
///
/// `ids.applicability.missing`（0.2.0 から）
pub const IDS_APPLICABILITY_MISSING: Code = Code::new(
    "ids.applicability.missing",
    Severity::Error,
    None,
    &[Param::new("specification", ParamType::Text)],
    &[],
);

/// 要素が適用の条件に合うかを確かめられない（指摘）
///
/// `ids.applicability.unsupported`（0.2.0 から）
pub const IDS_APPLICABILITY_UNSUPPORTED: Code = Code::new(
    "ids.applicability.unsupported",
    Severity::Info,
    None,
    &[Param::new("specification", ParamType::Text)],
    &[],
);

/// 知らない facet。その facet は確かめられないとする
///
/// `ids.facet.unsupported`（0.2.0 から）
pub const IDS_FACET_UNSUPPORTED: Code = Code::new(
    "ids.facet.unsupported",
    Severity::Info,
    None,
    &[Param::new("kind", ParamType::Text)],
    &[],
);

/// IDS の XML の構造が、IDS 1.0 に合わない。near は合わなかった要素か属性
///
/// `ids.file.invalid`（0.2.0 から）
pub const IDS_FILE_INVALID: Code = Code::new(
    "ids.file.invalid",
    Severity::Error,
    None,
    &[Param::new("near", ParamType::Text)],
    &[],
);

/// 要素が要件の facet を満たさない（指摘）。任意の要件なら warning で返す
///
/// `ids.requirement.invalid`（0.2.0 から）
pub const IDS_REQUIREMENT_INVALID: Code = Code::new(
    "ids.requirement.invalid",
    Severity::Error,
    None,
    &[
        Param::new("cardinality", ParamType::Text),
        Param::new("facet", ParamType::Text),
        Param::new("specification", ParamType::Text),
    ],
    &[],
);

/// 要素が要件の facet を満たすかを確かめられない（指摘）
///
/// `ids.requirement.unsupported`（0.2.0 から）
pub const IDS_REQUIREMENT_UNSUPPORTED: Code = Code::new(
    "ids.requirement.unsupported",
    Severity::Info,
    None,
    &[
        Param::new("facet", ParamType::Text),
        Param::new("specification", ParamType::Text),
    ],
    &[],
);

/// 読まない値の制約か、読めない式。その制約は確かめられないとする
///
/// `ids.restriction.unsupported`（0.2.0 から）
pub const IDS_RESTRICTION_UNSUPPORTED: Code = Code::new(
    "ids.restriction.unsupported",
    Severity::Info,
    None,
    &[Param::new("kind", ParamType::Text)],
    &[],
);

/// 仕様の対象の IFC の版に、建物モデルのスキーマがない
///
/// `ids.version.mixed`（0.2.0 から）
pub const IDS_VERSION_MIXED: Code = Code::new(
    "ids.version.mixed",
    Severity::Warning,
    None,
    &[
        Param::new("schema", ParamType::Text),
        Param::new("specification", ParamType::Text),
    ],
    &[],
);

/// 線形の区間の種類が土木の領域モデルにないので、その区間を写さなかった
///
/// `ifc.alignment-segment.unsupported`（0.2.0 から）
pub const IFC_ALIGNMENT_SEGMENT_UNSUPPORTED: Code = Code::new(
    "ifc.alignment-segment.unsupported",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[
        Param::new("kind", ParamType::Key),
        Param::new("type", ParamType::Key),
    ],
    &["kind", "type"],
);

/// 意味を持つ位置の値が、期待した型でないので読まなかった
///
/// `ifc.attribute.invalid`（0.2.0 から）
pub const IFC_ATTRIBUTE_INVALID: Code = Code::new(
    "ifc.attribute.invalid",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[
        Param::new("kind", ParamType::Key),
        Param::new("position", ParamType::UInt),
    ],
    &["kind", "position"],
);

/// 地表面と横断の形は、まだ土木の領域モデルに写さない
///
/// `ifc.civil-geometry.dropped`（0.2.0 から）
pub const IFC_CIVIL_GEOMETRY_DROPPED: Code = Code::new(
    "ifc.civil-geometry.dropped",
    Severity::Info,
    Some(Fidelity::Dropped),
    &[Param::new("kind", ParamType::Key)],
    &["kind"],
);

/// プロジェクトの長さか平面角の単位が解けないので、メートルかラジアンとした
///
/// `ifc.civil-unit.assumed`（0.2.0 から）
pub const IFC_CIVIL_UNIT_ASSUMED: Code = Code::new(
    "ifc.civil-unit.assumed",
    Severity::Warning,
    None,
    &[Param::new("type", ParamType::Key)],
    &["type"],
);

/// 同じ番号のインスタンスが複数あるので、番号で引くときは初めのものを使う
///
/// `ifc.duplicate.dropped`（0.2.0 から）
pub const IFC_DUPLICATE_DROPPED: Code = Code::new(
    "ifc.duplicate.dropped",
    Severity::Warning,
    None,
    &[Param::new("id", ParamType::UInt)],
    &[],
);

/// 建物モデルに写さない実体を、L1 に残した。件数は種類ごとに数える
///
/// `ifc.entity.dropped`（0.2.0 から）
pub const IFC_ENTITY_DROPPED: Code = Code::new(
    "ifc.entity.dropped",
    Severity::Info,
    Some(Fidelity::Dropped),
    &[Param::new("kind", ParamType::Key)],
    &["kind"],
);

/// ifcZIP の書庫に、IFC-SPF（.ifc）の項目がない
///
/// `ifc.entry.missing`（0.2.0 から）
pub const IFC_ENTRY_MISSING: Code = Code::new("ifc.entry.missing", Severity::Error, None, &[], &[]);

/// IFC-SPF（.ifc）の項目が複数ある ifcZIP の書庫は読まない
///
/// `ifc.entry.unsupported`（0.2.0 から）
pub const IFC_ENTRY_UNSUPPORTED: Code =
    Code::new("ifc.entry.unsupported", Severity::Error, None, &[], &[]);

/// ISO 10303-21 の始まり・DATA の節・終わりのどれかがない
///
/// `ifc.file.invalid`（0.2.0 から）
pub const IFC_FILE_INVALID: Code = Code::new("ifc.file.invalid", Severity::Error, None, &[], &[]);

/// GlobalId のない実体なので、インスタンスの番号で ID を作った
///
/// `ifc.global-id.missing`（0.2.0 から）
pub const IFC_GLOBAL_ID_MISSING: Code =
    Code::new("ifc.global-id.missing", Severity::Warning, None, &[], &[]);

/// IfcProjectedCRS の VerticalDatum が扱える高さの基準でないので、高さの基準を不明にした
///
/// `ifc.height-datum.unsupported`（0.2.0 から）
pub const IFC_HEIGHT_DATUM_UNSUPPORTED: Code = Code::new(
    "ifc.height-datum.unsupported",
    Severity::Warning,
    None,
    &[Param::new("datum", ParamType::Text)],
    &[],
);

/// IfcMapConversion か IfcProjectedCRS がないので、座標系を不明にし、プロジェクトの空間の点を地図の点として読んだ
///
/// `ifc.map-crs.missing`（0.2.0 から）
pub const IFC_MAP_CRS_MISSING: Code =
    Code::new("ifc.map-crs.missing", Severity::Warning, None, &[], &[]);

/// IfcProjectedCRS の名前が扱える座標系でないので、座標系を不明にした
///
/// `ifc.map-crs.unsupported`（0.2.0 から）
pub const IFC_MAP_CRS_UNSUPPORTED: Code = Code::new(
    "ifc.map-crs.unsupported",
    Severity::Warning,
    None,
    &[Param::new("name", ParamType::Text)],
    &[],
);

/// 要素の親がもうあるので、後の集約・入れ子・空間への包含を写さなかった
///
/// `ifc.parent.exists`（0.2.0 から）
pub const IFC_PARENT_EXISTS: Code = Code::new(
    "ifc.parent.exists",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("id", ParamType::Text)],
    &[],
);

/// 配置の連鎖が循環するか、参照が欠けるか、軸を作れないので、配置なしにした
///
/// `ifc.placement.invalid`（0.2.0 から）
pub const IFC_PLACEMENT_INVALID: Code = Code::new(
    "ifc.placement.invalid",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("id", ParamType::Text)],
    &[],
);

/// 通り芯や線形の配置は、種類だけを持ち、解けない配置にした
///
/// `ifc.placement.unsupported`（0.2.0 から）
pub const IFC_PLACEMENT_UNSUPPORTED: Code = Code::new(
    "ifc.placement.unsupported",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("kind", ParamType::Key)],
    &["kind"],
);

/// 文の形が読めないので、レコードにせずに、次の文から読んだ
///
/// `ifc.record.invalid`（0.2.0 から）
pub const IFC_RECORD_INVALID: Code =
    Code::new("ifc.record.invalid", Severity::Warning, None, &[], &[]);

/// 値の中の参照が、ファイルにない番号を指す
///
/// `ifc.reference.missing`（0.2.0 から）
pub const IFC_REFERENCE_MISSING: Code = Code::new(
    "ifc.reference.missing",
    Severity::Warning,
    None,
    &[Param::new("id", ParamType::UInt)],
    &[],
);

/// FILE_SCHEMA がないか、最初のスキーマの名前が IFC で始まらない
///
/// `ifc.schema.unsupported`（0.2.0 から）
pub const IFC_SCHEMA_UNSUPPORTED: Code = Code::new(
    "ifc.schema.unsupported",
    Severity::Error,
    None,
    &[Param::new("schema", ParamType::Text)],
    &[],
);

/// 線形に測点の IfcReferent がないので、初めの測点を 0 にした
///
/// `ifc.station.missing`（0.2.0 から）
pub const IFC_STATION_MISSING: Code =
    Code::new("ifc.station.missing", Severity::Info, None, &[], &[]);

/// 文字列のエスケープか UTF-8 が読めない所を、置き換えの文字にした
///
/// `ifc.text.invalid`（0.2.0 から）
pub const IFC_TEXT_INVALID: Code = Code::new(
    "ifc.text.invalid",
    Severity::Warning,
    Some(Fidelity::Approximated),
    &[],
    &[],
);

/// 値の単位が解けないので、単位なしにした
///
/// `ifc.unit.missing`（0.2.0 から）
pub const IFC_UNIT_MISSING: Code = Code::new(
    "ifc.unit.missing",
    Severity::Warning,
    None,
    &[Param::new("type", ParamType::Key)],
    &["type"],
);

/// IFC2X3・IFC4・IFC4X3 でない版のファイルからは、建物モデルを作らない
///
/// `ifc.version.unsupported`（0.2.0 から）
pub const IFC_VERSION_UNSUPPORTED: Code = Code::new(
    "ifc.version.unsupported",
    Severity::Error,
    None,
    &[Param::new("schema", ParamType::Text)],
    &[],
);

/// ネイティブの境界でパニックを捕まえた。不具合なので直す
///
/// `internal.panic.caught`（0.0.0 から）
pub const INTERNAL_PANIC_CAUGHT: Code =
    Code::new("internal.panic.caught", Severity::Error, None, &[], &[]);

/// Exif の項目の値か構造が読めないので、その値を返さなかった（#387）。tag は項目の番号で、0 は構造
///
/// `jpeg.exif.invalid`（0.2.0 から）
pub const JPEG_EXIF_INVALID: Code = Code::new(
    "jpeg.exif.invalid",
    Severity::Warning,
    None,
    &[Param::new("tag", ParamType::UInt)],
    &[],
);

/// JPEG の始まりの印（SOI）がないので、写真として読まなかった（#387）
///
/// `jpeg.file.invalid`（0.2.0 から）
pub const JPEG_FILE_INVALID: Code = Code::new("jpeg.file.invalid", Severity::Error, None, &[], &[]);

/// JPEG のセグメントの長さか印が壊れているので、その先を読まなかった（#387）
///
/// `jpeg.segment.invalid`（0.2.0 から）
pub const JPEG_SEGMENT_INVALID: Code = Code::new(
    "jpeg.segment.invalid",
    Severity::Warning,
    None,
    &[Param::new("offset", ParamType::UInt)],
    &[],
);

/// XMP の区画を XML として読めないので、XMP の性質を返さなかった（#387）
///
/// `jpeg.xmp.invalid`（0.2.0 から）
pub const JPEG_XMP_INVALID: Code = Code::new("jpeg.xmp.invalid", Severity::Warning, None, &[], &[]);

/// 図形の一覧とブロックの定義の一覧を、終わりまで読めない
///
/// `jww.body.invalid`（0.0.0 から）
pub const JWW_BODY_INVALID: Code = Code::new("jww.body.invalid", Severity::Error, None, &[], &[]);

/// JwwData. の始まりか版がない
///
/// `jww.file.invalid`（0.0.0 から）
pub const JWW_FILE_INVALID: Code = Code::new("jww.file.invalid", Severity::Error, None, &[], &[]);

/// 見出しの欄を読めず、レイヤの名前と状態・色・線幅・縮尺を既定にした
///
/// `jww.header.invalid`（0.0.0 から）
pub const JWW_HEADER_INVALID: Code =
    Code::new("jww.header.invalid", Severity::Warning, None, &[], &[]);

/// 図面モデルにまだ移せない種類の図形を、移さなかった
///
/// `jww.lift.dropped`（0.0.0 から）
pub const JWW_LIFT_DROPPED: Code = Code::new(
    "jww.lift.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("kind", ParamType::Text)],
    &["kind"],
);

/// 中身を読めない図形を、移さなかった
///
/// `jww.lift.invalid`（0.0.0 から）
pub const JWW_LIFT_INVALID: Code = Code::new(
    "jww.lift.invalid",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("kind", ParamType::Text)],
    &["kind"],
);

/// 図形の一覧の後ろのバイト列（版 700 からは画像の一覧）を読めず、そのまま残した
///
/// `jww.trailer.invalid`（0.0.0 から）
pub const JWW_TRAILER_INVALID: Code =
    Code::new("jww.trailer.invalid", Severity::Warning, None, &[], &[]);

/// 横断の線の点が、計画高からの高さか、勾配と距離で書いてあるので、横断の線を移さなかった
///
/// `landxml.cross-section-line.unsupported`（0.2.0 から）
pub const LANDXML_CROSS_SECTION_LINE_UNSUPPORTED: Code = Code::new(
    "landxml.cross-section-line.unsupported",
    Severity::Warning,
    None,
    &[
        Param::new("form", ParamType::Text),
        Param::new("name", ParamType::Text),
    ],
    &["name", "form"],
);

/// 座標系を書いていないか、扱わない測地系・座標系なので、CRS を不明にした
///
/// `landxml.crs.unsupported`（0.2.0 から）
pub const LANDXML_CRS_UNSUPPORTED: Code = Code::new(
    "landxml.crs.unsupported",
    Severity::Warning,
    None,
    &[
        Param::new("datum", ParamType::Text),
        Param::new("system", ParamType::Text),
    ],
    &[],
);

/// 土木の領域モデルに移さない要素なので、読まなかった
///
/// `landxml.element.unsupported`（0.2.0 から）
pub const LANDXML_ELEMENT_UNSUPPORTED: Code = Code::new(
    "landxml.element.unsupported",
    Severity::Warning,
    None,
    &[Param::new("element", ParamType::Text)],
    &["element"],
);

/// TIN の、見えない面（F の i が 1）を移さなかった
///
/// `landxml.face.dropped`（0.2.0 から）
pub const LANDXML_FACE_DROPPED: Code = Code::new(
    "landxml.face.dropped",
    Severity::Info,
    None,
    &[
        Param::new("faces", ParamType::UInt),
        Param::new("surface", ParamType::Text),
    ],
    &[],
);

/// 高さの基準を書いていないか、東京湾平均海面（T.P.）でないので、高さの基準を不明にした
///
/// `landxml.height-datum.unsupported`（0.2.0 から）
pub const LANDXML_HEIGHT_DATUM_UNSUPPORTED: Code = Code::new(
    "landxml.height-datum.unsupported",
    Severity::Warning,
    None,
    &[Param::new("datum", ParamType::Text)],
    &[],
);

/// 要素の値が読めないので、その要素を土木の領域モデルに移さなかった
///
/// `landxml.value.invalid`（0.2.0 から）
pub const LANDXML_VALUE_INVALID: Code = Code::new(
    "landxml.value.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("element", ParamType::Text),
        Param::new("field", ParamType::Text),
    ],
    &["element", "field"],
);

/// 点群の領域モデルに持たない点の属性を、移さなかった（決定 0232）
///
/// `las.attribute.dropped`（0.2.0 から）
pub const LAS_ATTRIBUTE_DROPPED: Code = Code::new(
    "las.attribute.dropped",
    Severity::Info,
    Some(Fidelity::Dropped),
    &[Param::new("attribute", ParamType::Text)],
    &["attribute"],
);

/// 座標系のレコードがないので、座標系を不明にした
///
/// `las.crs.missing`（0.2.0 から）
pub const LAS_CRS_MISSING: Code = Code::new("las.crs.missing", Severity::Warning, None, &[], &[]);

/// 扱わない水平の座標系（平面直角座標系の JGD2011 でないもの）なので、座標系を不明にした
///
/// `las.crs.unsupported`（0.2.0 から）
pub const LAS_CRS_UNSUPPORTED: Code = Code::new(
    "las.crs.unsupported",
    Severity::Warning,
    None,
    &[Param::new("epsg", ParamType::UInt)],
    &[],
);

/// LASF の始まりか見出しがないか、見出しの値が合わない
///
/// `las.file.invalid`（0.2.0 から）
pub const LAS_FILE_INVALID: Code = Code::new("las.file.invalid", Severity::Error, None, &[], &[]);

/// LAZ の圧縮の記述（laszip の VLR）がないか、点を展開できなかった
///
/// `las.laz.invalid`（0.2.0 から）
pub const LAS_LAZ_INVALID: Code = Code::new(
    "las.laz.invalid",
    Severity::Error,
    None,
    &[Param::new("reason", ParamType::Text)],
    &[],
);

/// 点の形式が 0〜10 のどれでもない
///
/// `las.point-format.unsupported`（0.2.0 から）
pub const LAS_POINT_FORMAT_UNSUPPORTED: Code = Code::new(
    "las.point-format.unsupported",
    Severity::Error,
    None,
    &[Param::new("format", ParamType::UInt)],
    &[],
);

/// 点のレコードが、見出しの点の数より短いので、読めた点だけを読んだ
///
/// `las.points.invalid`（0.2.0 から）
pub const LAS_POINTS_INVALID: Code = Code::new(
    "las.points.invalid",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[
        Param::new("expected", ParamType::UInt),
        Param::new("found", ParamType::UInt),
    ],
    &[],
);

/// 扱わない高さの座標系なので、高さの座標系を不明にした
///
/// `las.vertical-crs.unsupported`（0.2.0 から）
pub const LAS_VERTICAL_CRS_UNSUPPORTED: Code = Code::new(
    "las.vertical-crs.unsupported",
    Severity::Warning,
    None,
    &[Param::new("epsg", ParamType::UInt)],
    &[],
);

/// 可変長のレコード（VLR・EVLR）が範囲から外れるので、残りを読まなかった
///
/// `las.vlr.invalid`（0.2.0 から）
pub const LAS_VLR_INVALID: Code = Code::new(
    "las.vlr.invalid",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[
        Param::new("expected", ParamType::UInt),
        Param::new("found", ParamType::UInt),
    ],
    &[],
);

/// PLY は座標系を書かないので、座標系を不明にした
///
/// `ply.crs.missing`（0.2.0 から）
pub const PLY_CRS_MISSING: Code = Code::new("ply.crs.missing", Severity::Warning, None, &[], &[]);

/// 点群の領域モデルに持たない要素（vertex でないもの）を、移さなかった
///
/// `ply.element.dropped`（0.2.0 から）
pub const PLY_ELEMENT_DROPPED: Code = Code::new(
    "ply.element.dropped",
    Severity::Info,
    Some(Fidelity::Dropped),
    &[
        Param::new("element", ParamType::Text),
        Param::new("found", ParamType::UInt),
    ],
    &["element"],
);

/// ply の始まりか見出しがないか、本体が見出しの要素と合わない
///
/// `ply.file.invalid`（0.2.0 から）
pub const PLY_FILE_INVALID: Code = Code::new("ply.file.invalid", Severity::Error, None, &[], &[]);

/// 点群の領域モデルに持たない点の性質を、移さなかった
///
/// `ply.property.dropped`（0.2.0 から）
pub const PLY_PROPERTY_DROPPED: Code = Code::new(
    "ply.property.dropped",
    Severity::Info,
    Some(Fidelity::Dropped),
    &[Param::new("property", ParamType::Text)],
    &["property"],
);

/// 文字の内容が、語彙の範囲の文字を含む（指摘）。character は U+XXXX の形
///
/// `profile.characters.invalid`（0.0.0 から）
pub const PROFILE_CHARACTERS_INVALID: Code = Code::new(
    "profile.characters.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("character", ParamType::Text),
        Param::new("rule", ParamType::Text),
        Param::new("vocabulary", ParamType::Text),
    ],
    &[],
);

/// 要素の色が、語彙にない（指摘）。色は系統と符号（sxf:17 など）か #rrggbb
///
/// `profile.color.invalid`（0.0.0 から）
pub const PROFILE_COLOR_INVALID: Code = Code::new(
    "profile.color.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("color", ParamType::Text),
        Param::new("rule", ParamType::Text),
        Param::new("vocabulary", ParamType::Text),
    ],
    &[],
);

/// 要素の色が、語彙の符号の列でも RGB でも決まらず、照合できない（指摘）
///
/// `profile.color.unsupported`（0.0.0 から）
pub const PROFILE_COLOR_UNSUPPORTED: Code = Code::new(
    "profile.color.unsupported",
    Severity::Info,
    None,
    &[Param::new("rule", ParamType::Text)],
    &[],
);

/// 管理ファイルの値に、規則の文字の範囲（JIS X 0208 など）の外の文字がある（指摘、決定 0241）
///
/// `profile.delivery-characters.invalid`（0.2.0 から）
pub const PROFILE_DELIVERY_CHARACTERS_INVALID: Code = Code::new(
    "profile.delivery-characters.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("character", ParamType::Text),
        Param::new("element", ParamType::Text),
        Param::new("managed", ParamType::Text),
        Param::new("rule", ParamType::Text),
    ],
    &[],
);

/// 管理ファイルの DTD の名前が、規則の名前のどれとも違うか、文書型宣言がない（指摘、決定 0241）
///
/// `profile.delivery-doctype.invalid`（0.2.0 から）
pub const PROFILE_DELIVERY_DOCTYPE_INVALID: Code = Code::new(
    "profile.delivery-doctype.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("found", ParamType::Text),
        Param::new("managed", ParamType::Text),
        Param::new("rule", ParamType::Text),
    ],
    &[],
);

/// 管理ファイルが指すファイルが成果品の中にない（指摘、決定 0241）
///
/// `profile.delivery-link.invalid`（0.2.0 から）
pub const PROFILE_DELIVERY_LINK_INVALID: Code = Code::new(
    "profile.delivery-link.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("managed", ParamType::Text),
        Param::new("name", ParamType::Text),
        Param::new("rule", ParamType::Text),
    ],
    &[],
);

/// 成果品の中のフォルダーかファイルの名前が、規則の文字・長さ・深さを外れる（指摘、決定 0241）
///
/// `profile.delivery-name.invalid`（0.2.0 から）
pub const PROFILE_DELIVERY_NAME_INVALID: Code = Code::new(
    "profile.delivery-name.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("name", ParamType::Text),
        Param::new("reason", ParamType::Text),
        Param::new("rule", ParamType::Text),
    ],
    &[],
);

/// 写真の管理ファイルの撮影年月日が、写真の Exif の撮影の日と違う（指摘、#388）
///
/// `profile.delivery-photo-date.invalid`（0.2.0 から）
pub const PROFILE_DELIVERY_PHOTO_DATE_INVALID: Code = Code::new(
    "profile.delivery-photo-date.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("found", ParamType::Text),
        Param::new("managed", ParamType::Text),
        Param::new("name", ParamType::Text),
        Param::new("rule", ParamType::Text),
        Param::new("taken", ParamType::Text),
    ],
    &[],
);

/// 撮影年月日の要素か、写真の Exif の撮影の日時がないので、確認できない（指摘、#388）
///
/// `profile.delivery-photo-date.unsupported`（0.2.0 から）
pub const PROFILE_DELIVERY_PHOTO_DATE_UNSUPPORTED: Code = Code::new(
    "profile.delivery-photo-date.unsupported",
    Severity::Info,
    None,
    &[
        Param::new("managed", ParamType::Text),
        Param::new("name", ParamType::Text),
        Param::new("rule", ParamType::Text),
    ],
    &[],
);

/// 写真の画素数が、規則の範囲の外にある（指摘、#388）
///
/// `profile.delivery-photo-pixels.invalid`（0.2.0 から）
pub const PROFILE_DELIVERY_PHOTO_PIXELS_INVALID: Code = Code::new(
    "profile.delivery-photo-pixels.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("name", ParamType::Text),
        Param::new("pixels", ParamType::UInt),
        Param::new("rule", ParamType::Text),
    ],
    &[],
);

/// 写真が JPEG でないか、画素数を読めないので、確認できない（指摘、#388）
///
/// `profile.delivery-photo-pixels.unsupported`（0.2.0 から）
pub const PROFILE_DELIVERY_PHOTO_PIXELS_UNSUPPORTED: Code = Code::new(
    "profile.delivery-photo-pixels.unsupported",
    Severity::Info,
    None,
    &[
        Param::new("name", ParamType::Text),
        Param::new("rule", ParamType::Text),
    ],
    &[],
);

/// 規則が求めるファイルが成果品の中にない（指摘、決定 0241）
///
/// `profile.delivery-required.invalid`（0.2.0 から）
pub const PROFILE_DELIVERY_REQUIRED_INVALID: Code = Code::new(
    "profile.delivery-required.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("name", ParamType::Text),
        Param::new("rule", ParamType::Text),
    ],
    &[],
);

/// 管理ファイルの要素の値（適用要領基準など）が、規則の値のどれとも違う（指摘、決定 0241）
///
/// `profile.delivery-standard.invalid`（0.2.0 から）
pub const PROFILE_DELIVERY_STANDARD_INVALID: Code = Code::new(
    "profile.delivery-standard.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("element", ParamType::Text),
        Param::new("found", ParamType::Text),
        Param::new("managed", ParamType::Text),
        Param::new("rule", ParamType::Text),
    ],
    &[],
);

/// 管理ファイルに、規則が比べる要素がないので、確認できない（指摘、決定 0241）
///
/// `profile.delivery-standard.unsupported`（0.2.0 から）
pub const PROFILE_DELIVERY_STANDARD_UNSUPPORTED: Code = Code::new(
    "profile.delivery-standard.unsupported",
    Severity::Info,
    None,
    &[
        Param::new("element", ParamType::Text),
        Param::new("managed", ParamType::Text),
        Param::new("rule", ParamType::Text),
    ],
    &[],
);

/// フォルダーの中のファイルを、どの管理ファイルも指さない（指摘、決定 0241）
///
/// `profile.delivery-unreferenced.invalid`（0.2.0 から）
pub const PROFILE_DELIVERY_UNREFERENCED_INVALID: Code = Code::new(
    "profile.delivery-unreferenced.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("name", ParamType::Text),
        Param::new("rule", ParamType::Text),
    ],
    &[],
);

/// extends の鎖が輪になっている
///
/// `profile.extends.invalid`（0.0.0 から）
pub const PROFILE_EXTENDS_INVALID: Code = Code::new(
    "profile.extends.invalid",
    Severity::Error,
    None,
    &[
        Param::new("edition", ParamType::Text),
        Param::new("id", ParamType::Text),
    ],
    &["id", "edition"],
);

/// extends で指すプロファイルが、渡した入力にない
///
/// `profile.extends.missing`（0.0.0 から）
pub const PROFILE_EXTENDS_MISSING: Code = Code::new(
    "profile.extends.missing",
    Severity::Error,
    None,
    &[
        Param::new("edition", ParamType::Text),
        Param::new("id", ParamType::Text),
    ],
    &["id", "edition"],
);

/// ファイルの名前が規則に合わない（指摘）。reason は length（全体の文字の数）・extension（拡張子）・part（決まった部分）・user-length（利用者が決める部分の文字の数）・user-characters（利用者が決める部分の半角の文字）
///
/// `profile.file-name.invalid`（0.0.0 から）
pub const PROFILE_FILE_NAME_INVALID: Code = Code::new(
    "profile.file-name.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("name", ParamType::Text),
        Param::new("reason", ParamType::Key),
        Param::new("rule", ParamType::Text),
    ],
    &[],
);

/// check の要求に file_name がなく、ファイルの名前を照合できない（指摘）
///
/// `profile.file-name.unsupported`（0.0.0 から）
pub const PROFILE_FILE_NAME_UNSUPPORTED: Code = Code::new(
    "profile.file-name.unsupported",
    Severity::Info,
    None,
    &[Param::new("rule", ParamType::Text)],
    &[],
);

/// プロファイルの入力を読めないか、書式の大版を読めない（決定 0138）
///
/// `profile.format.unsupported`（0.0.0 から）
pub const PROFILE_FORMAT_UNSUPPORTED: Code = Code::new(
    "profile.format.unsupported",
    Severity::Error,
    None,
    &[Param::new("name", ParamType::Text)],
    &["name"],
);

/// レイヤの名前が、規則の区切りと語彙に合わないか、長すぎる（指摘）
///
/// `profile.layer-name.invalid`（0.0.0 から）
pub const PROFILE_LAYER_NAME_INVALID: Code = Code::new(
    "profile.layer-name.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("layer", ParamType::Text),
        Param::new("rule", ParamType::Text),
    ],
    &[],
);

/// 要素の線種の名前が、語彙の形式の列にない（指摘）
///
/// `profile.line-type.invalid`（0.0.0 から）
pub const PROFILE_LINE_TYPE_INVALID: Code = Code::new(
    "profile.line-type.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("line_type", ParamType::Text),
        Param::new("rule", ParamType::Text),
        Param::new("vocabulary", ParamType::Text),
    ],
    &[],
);

/// 線種が決まらないか、語彙が図面の形式の列を持たず、照合できない（指摘）
///
/// `profile.line-type.unsupported`（0.0.0 から）
pub const PROFILE_LINE_TYPE_UNSUPPORTED: Code = Code::new(
    "profile.line-type.unsupported",
    Severity::Info,
    None,
    &[Param::new("rule", ParamType::Text)],
    &[],
);

/// 要素の線幅（用紙の mm）が、起点の用紙の大きさの行のどの線幅とも許容差の内で一致しない（指摘）。sheet は語彙の用紙の名前
///
/// `profile.line-width-by-sheet.invalid`（0.0.0 から）
pub const PROFILE_LINE_WIDTH_BY_SHEET_INVALID: Code = Code::new(
    "profile.line-width-by-sheet.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("rule", ParamType::Text),
        Param::new("sheet", ParamType::Text),
        Param::new("vocabulary", ParamType::Text),
        Param::new("width", ParamType::Float),
    ],
    &[],
);

/// 線幅を読まなかったか、決まらず、用紙の大きさごとの線幅と照合できない（指摘）
///
/// `profile.line-width-by-sheet.unsupported`（0.0.0 から）
pub const PROFILE_LINE_WIDTH_BY_SHEET_UNSUPPORTED: Code = Code::new(
    "profile.line-width-by-sheet.unsupported",
    Severity::Info,
    None,
    &[Param::new("rule", ParamType::Text)],
    &[],
);

/// 要素の線幅（用紙の mm）が、語彙のどの値とも許容差の内で一致しない（指摘）
///
/// `profile.line-width.invalid`（0.0.0 から）
pub const PROFILE_LINE_WIDTH_INVALID: Code = Code::new(
    "profile.line-width.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("rule", ParamType::Text),
        Param::new("vocabulary", ParamType::Text),
        Param::new("width", ParamType::Float),
    ],
    &[],
);

/// 線幅を読まなかったか、決まらず、照合できない（指摘）
///
/// `profile.line-width.unsupported`（0.0.0 から）
pub const PROFILE_LINE_WIDTH_UNSUPPORTED: Code = Code::new(
    "profile.line-width.unsupported",
    Severity::Info,
    None,
    &[Param::new("rule", ParamType::Text)],
    &[],
);

/// 用紙の向きが、規則の向きと違う（指摘）
///
/// `profile.orientation.invalid`（0.0.0 から）
pub const PROFILE_ORIENTATION_INVALID: Code = Code::new(
    "profile.orientation.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("expected", ParamType::Text),
        Param::new("orientation", ParamType::Key),
        Param::new("rule", ParamType::Text),
    ],
    &[],
);

/// 大きさのない用紙で、向きが決まらず、照合できない（指摘）
///
/// `profile.orientation.unsupported`（0.0.0 から）
pub const PROFILE_ORIENTATION_UNSUPPORTED: Code = Code::new(
    "profile.orientation.unsupported",
    Severity::Info,
    None,
    &[Param::new("rule", ParamType::Text)],
    &[],
);

/// 照合の種類を知らないか、欄か語彙を使えない規則を、照合しなかった
///
/// `profile.rule.unsupported`（0.0.0 から）
pub const PROFILE_RULE_UNSUPPORTED: Code = Code::new(
    "profile.rule.unsupported",
    Severity::Warning,
    None,
    &[
        Param::new("check", ParamType::Text),
        Param::new("rule", ParamType::Text),
    ],
    &["rule"],
);

/// ビューの縮尺の分母の仮数が、語彙にない（指摘）。view は用紙のビューの番号
///
/// `profile.scale.invalid`（0.0.0 から）
pub const PROFILE_SCALE_INVALID: Code = Code::new(
    "profile.scale.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("denominator", ParamType::Float),
        Param::new("rule", ParamType::Text),
        Param::new("view", ParamType::UInt),
        Param::new("vocabulary", ParamType::Text),
    ],
    &[],
);

/// 空間の単位が分からないか、縦横の尺度が違うビューで、縮尺が決まらず、照合できない（指摘）
///
/// `profile.scale.unsupported`（0.0.0 から）
pub const PROFILE_SCALE_UNSUPPORTED: Code = Code::new(
    "profile.scale.unsupported",
    Severity::Info,
    None,
    &[
        Param::new("rule", ParamType::Text),
        Param::new("view", ParamType::UInt),
    ],
    &[],
);

/// 用紙の大きさが、向きを問わず、語彙のどの大きさとも許容差の内で一致しない（指摘）
///
/// `profile.sheet-size.invalid`（0.0.0 から）
pub const PROFILE_SHEET_SIZE_INVALID: Code = Code::new(
    "profile.sheet-size.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("height", ParamType::Float),
        Param::new("rule", ParamType::Text),
        Param::new("vocabulary", ParamType::Text),
        Param::new("width", ParamType::Float),
    ],
    &[],
);

/// 大きさのない用紙で、照合できない（指摘）
///
/// `profile.sheet-size.unsupported`（0.0.0 から）
pub const PROFILE_SHEET_SIZE_UNSUPPORTED: Code = Code::new(
    "profile.sheet-size.unsupported",
    Severity::Info,
    None,
    &[Param::new("rule", ParamType::Text)],
    &[],
);

/// 区分の標高較差の平均値か、個々の計測値の最小・最大が、規則の幅（mm）を外れる（指摘、決定 0234）
///
/// `profile.surface-elevation.invalid`（0.2.0 から）
pub const PROFILE_SURFACE_ELEVATION_INVALID: Code = Code::new(
    "profile.surface-elevation.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("found", ParamType::Float),
        Param::new("limit", ParamType::Float),
        Param::new("rule", ParamType::Text),
        Param::new("statistic", ParamType::Text),
    ],
    &[],
);

/// 区分に評価した点がないか、計測の密度（点／m²）が規則の下限に足りず、確認できない（指摘、決定 0234）
///
/// `profile.surface-elevation.unsupported`（0.2.0 から）
pub const PROFILE_SURFACE_ELEVATION_UNSUPPORTED: Code = Code::new(
    "profile.surface-elevation.unsupported",
    Severity::Info,
    None,
    &[
        Param::new("density", ParamType::Float),
        Param::new("required", ParamType::Float),
        Param::new("rule", ParamType::Text),
    ],
    &[],
);

/// 文字の高さ（用紙の mm）が、語彙のどの値とも許容差の内で一致しない（指摘）。meaning は高さの意味（決定 0022）
///
/// `profile.text-height.invalid`（0.0.0 から）
pub const PROFILE_TEXT_HEIGHT_INVALID: Code = Code::new(
    "profile.text-height.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("height", ParamType::Float),
        Param::new("meaning", ParamType::Key),
        Param::new("rule", ParamType::Text),
        Param::new("vocabulary", ParamType::Text),
    ],
    &[],
);

/// 用紙を通さない実寸の文字か、縦横の尺度が違う配置の文字で、用紙の高さが決まらず、照合できない（指摘）
///
/// `profile.text-height.unsupported`（0.0.0 から）
pub const PROFILE_TEXT_HEIGHT_UNSUPPORTED: Code = Code::new(
    "profile.text-height.unsupported",
    Severity::Info,
    None,
    &[Param::new("rule", ParamType::Text)],
    &[],
);

/// プロファイルの語彙の値が、使える形でない（決定 0239）
///
/// `profile.vocabulary.invalid`（0.2.0 から）
pub const PROFILE_VOCABULARY_INVALID: Code = Code::new(
    "profile.vocabulary.invalid",
    Severity::Error,
    None,
    &[
        Param::new("field", ParamType::Text),
        Param::new("vocabulary", ParamType::Text),
    ],
    &[],
);

/// 指摘の文書の JSON の形が合わない。pointer は合わなかった欄の JSON Pointer か、構文の誤りのバイト位置
///
/// `review.file.invalid`（0.2.0 から）
pub const REVIEW_FILE_INVALID: Code = Code::new(
    "review.file.invalid",
    Severity::Error,
    None,
    &[Param::new("pointer", ParamType::Text)],
    &[],
);

/// 指摘の文書ではない JSON。format は書いてあった形式の名
///
/// `review.format.unsupported`（0.2.0 から）
pub const REVIEW_FORMAT_UNSUPPORTED: Code = Code::new(
    "review.format.unsupported",
    Severity::Error,
    None,
    &[Param::new("format", ParamType::Text)],
    &[],
);

/// 指摘の文書の中で、同じ種類の ID が重なる
///
/// `review.id.exists`（0.2.0 から）
pub const REVIEW_ID_EXISTS: Code = Code::new(
    "review.id.exists",
    Severity::Error,
    None,
    &[Param::new("id", ParamType::Text)],
    &[],
);

/// 指摘の文書の参照の先（対象・トピック・場所）が、文書の中にない
///
/// `review.reference.missing`（0.2.0 から）
pub const REVIEW_REFERENCE_MISSING: Code = Code::new(
    "review.reference.missing",
    Severity::Error,
    None,
    &[Param::new("id", ParamType::Text)],
    &[],
);

/// 引き継ぐ・取り込む対象が、指摘の文書にない
///
/// `review.target.missing`（0.2.0 から）
pub const REVIEW_TARGET_MISSING: Code = Code::new(
    "review.target.missing",
    Severity::Error,
    None,
    &[Param::new("target", ParamType::Text)],
    &[],
);

/// 取り込む指摘の対象の形式かハッシュが、文書の同じ ID の対象と違う。先に引き継ぐ
///
/// `review.target.stale`（0.2.0 から）
pub const REVIEW_TARGET_STALE: Code = Code::new(
    "review.target.stale",
    Severity::Error,
    None,
    &[Param::new("target", ParamType::Text)],
    &[],
);

/// 読めない版の指摘の文書
///
/// `review.version.unsupported`（0.2.0 から）
pub const REVIEW_VERSION_UNSUPPORTED: Code = Code::new(
    "review.version.unsupported",
    Severity::Error,
    None,
    &[Param::new("version", ParamType::UInt)],
    &[],
);

/// 引出線と引出線つきの注記の、記号の定義で決まるか大きさの決まらない端部記号を描かなかった。線は描いた（決定 0184）
///
/// `scene.arrowhead.dropped`（0.0.0 から）
pub const SCENE_ARROWHEAD_DROPPED: Code = Code::new(
    "scene.arrowhead.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[],
    &[],
);

/// 引出線つきの注記の記号の、属性の値を描かなかった。記号の中身は描いた（決定 0184）
///
/// `scene.callout-value.dropped`（0.0.0 から）
pub const SCENE_CALLOUT_VALUE_DROPPED: Code = Code::new(
    "scene.callout-value.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[],
    &[],
);

/// 引出線つきの注記を、シーンに描かなかった（決定 0172）（決定 0184 で、引出線つきの注記を描くので使わない）
///
/// `scene.callout.dropped`（0.0.0 から）
#[deprecated = "廃止した code（catalog/codes.toml）"]
pub const SCENE_CALLOUT_DROPPED: Code = Code::new(
    "scene.callout.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[],
    &[],
);

/// 曲線・ふくらみのある折れ線を、弦の誤差が許容差以下の折れ線にした（決定 0172）。許容差は起点の長さ
///
/// `scene.curve.approximated`（0.0.0 から）
pub const SCENE_CURVE_APPROXIMATED: Code = Code::new(
    "scene.curve.approximated",
    Severity::Info,
    Some(Fidelity::Approximated),
    &[Param::new("tolerance", ParamType::Float)],
    &["tolerance"],
);

/// 表示の部品がない寸法を、定義点から補助線・寸法線と、書式の高さを持つ寸法値の文字で描いた。端部記号は描かない（決定 0184）
///
/// `scene.dimension.approximated`（0.0.0 から）
pub const SCENE_DIMENSION_APPROXIMATED: Code = Code::new(
    "scene.dimension.approximated",
    Severity::Info,
    Some(Fidelity::Approximated),
    &[],
    &[],
);

/// 表示の部品から描けない寸法を、シーンに描かなかった（決定 0172）
///
/// `scene.dimension.dropped`（0.0.0 から）
pub const SCENE_DIMENSION_DROPPED: Code = Code::new(
    "scene.dimension.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[],
    &[],
);

/// 当てはめる点を許容差で近づけるスプラインを、点を通る曲線で描いた（決定 0184）
///
/// `scene.fit-tolerance.approximated`（0.0.0 から）
pub const SCENE_FIT_TOLERANCE_APPROXIMATED: Code = Code::new(
    "scene.fit-tolerance.approximated",
    Severity::Info,
    Some(Fidelity::Approximated),
    &[],
    &[],
);

/// 名前の合うフォントがないので、既定のフォント（書式の区間では文字のフォント）で字形を置いた（決定 0188）
///
/// `scene.font.assumed`（0.0.0 から）
pub const SCENE_FONT_ASSUMED: Code = Code::new(
    "scene.font.assumed",
    Severity::Info,
    Some(Fidelity::Approximated),
    &[Param::new("name", ParamType::Text)],
    &["name"],
);

/// 渡したフォントに、文字のフォントの名前に合うものも既定のものもないので、字形を置かずに文字列だけを返した（決定 0188）
///
/// `scene.font.missing`（0.0.0 から）
pub const SCENE_FONT_MISSING: Code = Code::new(
    "scene.font.missing",
    Severity::Warning,
    None,
    &[Param::new("name", ParamType::Text)],
    &["name"],
);

/// 字形の輪郭が壊れているか上限を超えて読めないので、その字形の形を空にした（決定 0188）
///
/// `scene.glyph-outline.invalid`（0.0.0 から）
pub const SCENE_GLYPH_OUTLINE_INVALID: Code = Code::new(
    "scene.glyph-outline.invalid",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[],
    &[],
);

/// フォントにない文字を、代わりの字形（.notdef）で置いた（決定 0188）
///
/// `scene.glyph.missing`（0.0.0 から）
pub const SCENE_GLYPH_MISSING: Code = Code::new(
    "scene.glyph.missing",
    Severity::Warning,
    Some(Fidelity::Approximated),
    &[],
    &[],
);

/// グラデーションの塗りを、最初の色で塗った（決定 0172）
///
/// `scene.gradient.approximated`（0.0.0 から）
pub const SCENE_GRADIENT_APPROXIMATED: Code = Code::new(
    "scene.gradient.approximated",
    Severity::Info,
    Some(Fidelity::Approximated),
    &[],
    &[],
);

/// 線の間隔が許容差以下のパターンの線の組を、塗りで描いた（決定 0184）
///
/// `scene.hatch-pattern.approximated`（0.0.0 から）
pub const SCENE_HATCH_PATTERN_APPROXIMATED: Code = Code::new(
    "scene.hatch-pattern.approximated",
    Severity::Info,
    Some(Fidelity::Approximated),
    &[],
    &[],
);

/// ハッチングのパターンの線を、シーンに描かなかった。境界も描かない（決定 0184 で、パターンの線を描くので使わない）
///
/// `scene.hatch-pattern.dropped`（0.0.0 から）
#[deprecated = "廃止した code（catalog/codes.toml）"]
pub const SCENE_HATCH_PATTERN_DROPPED: Code = Code::new(
    "scene.hatch-pattern.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[],
    &[],
);

/// 引出線を、シーンに描かなかった（決定 0172）
///
/// `scene.leader.dropped`（0.0.0 から）
pub const SCENE_LEADER_DROPPED: Code = Code::new(
    "scene.leader.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[],
    &[],
);

/// 線種に添える形と文字を、シーンに描かなかった。線と間は描いた（決定 0184 で、文字を描き、形は scene.line-type-shape.dropped で返すので使わない）
///
/// `scene.line-type-mark.dropped`（0.0.0 から）
#[deprecated = "廃止した code（catalog/codes.toml）"]
pub const SCENE_LINE_TYPE_MARK_DROPPED: Code = Code::new(
    "scene.line-type-mark.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[],
    &[],
);

/// 線種に添える形（形のファイルの字形）を、シーンに描かなかった。線と間と文字は描いた（決定 0184）
///
/// `scene.line-type-shape.dropped`（0.0.0 から）
pub const SCENE_LINE_TYPE_SHAPE_DROPPED: Code = Code::new(
    "scene.line-type-shape.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[],
    &[],
);

/// 当てはめる点だけで決まるスプラインを、当てはめる点を結ぶ折れ線で描いた
///
/// `scene.spline.approximated`（0.0.0 から）
pub const SCENE_SPLINE_APPROXIMATED: Code = Code::new(
    "scene.spline.approximated",
    Severity::Warning,
    Some(Fidelity::Approximated),
    &[],
    &[],
);

/// 描いた部品がない表を、セルの枠の線と、高さの決まるセルの文字で描いた。枠の表示と色・セルの塗り・記号のセルは描かない（決定 0184）
///
/// `scene.table.approximated`（0.0.0 から）
pub const SCENE_TABLE_APPROXIMATED: Code = Code::new(
    "scene.table.approximated",
    Severity::Info,
    Some(Fidelity::Approximated),
    &[],
    &[],
);

/// 表を、シーンに描かなかった（決定 0172）（決定 0184 で、表は描いた部品か行と列から描くので使わない）
///
/// `scene.table.dropped`（0.0.0 から）
#[deprecated = "廃止した code（catalog/codes.toml）"]
pub const SCENE_TABLE_DROPPED: Code = Code::new(
    "scene.table.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[],
    &[],
);

/// 形式の宣言値の高さが、フォントのどの寸法に当たるかが渡されないので、大文字の高さとみなして字形を置いた（決定 0188）
///
/// `scene.text-height.assumed`（0.0.0 から）
pub const SCENE_TEXT_HEIGHT_ASSUMED: Code = Code::new(
    "scene.text-height.assumed",
    Severity::Info,
    None,
    &[Param::new("key", ParamType::Text)],
    &["key"],
);

/// 文字（縦書きの 1 行の文字、測定値の区間、寸法の決まらない文字など）を字形で置けないので、その部分は文字列だけを返した（決定 0188）
///
/// `scene.text-layout.unsupported`（0.0.0 から）
pub const SCENE_TEXT_LAYOUT_UNSUPPORTED: Code = Code::new(
    "scene.text-layout.unsupported",
    Severity::Warning,
    None,
    &[],
    &[],
);

/// 下敷きの切り抜きを当てずに、下敷きの全体を描いた
///
/// `scene.underlay-clip.dropped`（0.0.0 から）
pub const SCENE_UNDERLAY_CLIP_DROPPED: Code = Code::new(
    "scene.underlay-clip.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[],
    &[],
);

/// 画像と PDF でない下敷き（DWF・DGN）を、シーンに描かなかった（決定 0184 で、DWF・DGN も画像として返すので使わない）
///
/// `scene.underlay.dropped`（0.0.0 から）
#[deprecated = "廃止した code（catalog/codes.toml）"]
pub const SCENE_UNDERLAY_DROPPED: Code = Code::new(
    "scene.underlay.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[],
    &[],
);

/// 書き戻す P2Z・SFZ の書庫が ZIP64 を要する大きさになる。ZIP64 は書かない（#385）
///
/// `sxf.archive.unsupported`（0.2.0 から）
pub const SXF_ARCHIVE_UNSUPPORTED: Code =
    Code::new("sxf.archive.unsupported", Severity::Error, None, &[], &[]);

/// 要素が指すレイヤ・色・線種・線幅のコードに定義がない
///
/// `sxf.code.missing`（0.0.0 から）
pub const SXF_CODE_MISSING: Code = Code::new(
    "sxf.code.missing",
    Severity::Warning,
    None,
    &[
        Param::new("code", ParamType::Int),
        Param::new("kind", ParamType::Text),
    ],
    &["kind", "code"],
);

/// ISO 10303-21 の始まり・DATA の節・終わりのどれかがない
///
/// `sxf.file.invalid`（0.0.0 から）
pub const SXF_FILE_INVALID: Code = Code::new("sxf.file.invalid", Severity::Error, None, &[], &[]);

/// 図面モデルにまだ移せない種類のフィーチャを、移さなかった
///
/// `sxf.lift.dropped`（0.0.0 から）
pub const SXF_LIFT_DROPPED: Code = Code::new(
    "sxf.lift.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("kind", ParamType::Text)],
    &["kind"],
);

/// 引数が足りないか、読めないか、範囲の外のフィーチャを、移さなかった
///
/// `sxf.lift.invalid`（0.0.0 から）
pub const SXF_LIFT_INVALID: Code = Code::new(
    "sxf.lift.invalid",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("kind", ParamType::Text)],
    &["kind"],
);

/// 用紙のフィーチャより後に書いた要素を、用紙の要素とした
///
/// `sxf.member.assumed`（0.0.0 から）
pub const SXF_MEMBER_ASSUMED: Code =
    Code::new("sxf.member.assumed", Severity::Info, None, &[], &[]);

/// P21 の実体が、ない実体を参照している
///
/// `sxf.p21-reference.missing`（0.0.0 から）
pub const SXF_P21_REFERENCE_MISSING: Code = Code::new(
    "sxf.p21-reference.missing",
    Severity::Warning,
    None,
    &[Param::new("id", ParamType::UInt)],
    &[],
);

/// P21 に用紙（DRAWING_SHEET_REVISION）がないので、要素を図面モデルに移さなかった
///
/// `sxf.p21-sheet.missing`（0.0.0 から）
pub const SXF_P21_SHEET_MISSING: Code = Code::new(
    "sxf.p21-sheet.missing",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[],
    &[],
);

/// P21 の図面モデルへの変換はまだないので、空の図面にした
///
/// `sxf.p21.unsupported`（0.0.0 から）
#[deprecated = "廃止した code（catalog/codes.toml）"]
pub const SXF_P21_UNSUPPORTED: Code = Code::new(
    "sxf.p21.unsupported",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[],
    &[],
);

/// 画像の作図グループに、範囲を示す折れ線がないので、画像を移さなかった
///
/// `sxf.raster-frame.missing`（0.0.0 から）
pub const SXF_RASTER_FRAME_MISSING: Code = Code::new(
    "sxf.raster-frame.missing",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("name", ParamType::Text)],
    &["name"],
);

/// 文かフィーチャのコメントの形が読めないので、レコードにせずに、次の文から読んだ
///
/// `sxf.record.invalid`（0.0.0 から）
pub const SXF_RECORD_INVALID: Code =
    Code::new("sxf.record.invalid", Severity::Warning, None, &[], &[]);

/// FILE_SCHEMA が SXF の図面のスキーマ（ASSOCIATIVE_DRAUGHTING）でない
///
/// `sxf.schema.unsupported`（0.0.0 から）
pub const SXF_SCHEMA_UNSUPPORTED: Code = Code::new(
    "sxf.schema.unsupported",
    Severity::Error,
    None,
    &[Param::new("schema", ParamType::Text)],
    &[],
);

/// 自分を直接か間接に配置する作図グループの配置を、展開しなかった
///
/// `sxf.sfig-cycle.invalid`（0.0.0 から）
pub const SXF_SFIG_CYCLE_INVALID: Code = Code::new(
    "sxf.sfig-cycle.invalid",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("name", ParamType::Text)],
    &["name"],
);

/// 配置が指す複合図形の定義がないので、配置を移さなかった
///
/// `sxf.sfig.missing`（0.0.0 から）
pub const SXF_SFIG_MISSING: Code = Code::new(
    "sxf.sfig.missing",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("name", ParamType::Text)],
    &["name"],
);

/// セルの参照か共有文字列の番号が読めないので、そのセルか結合セルを飛ばした（#391）
///
/// `xlsx.cell.invalid`（0.2.0 から）
pub const XLSX_CELL_INVALID: Code = Code::new(
    "xlsx.cell.invalid",
    Severity::Warning,
    None,
    &[
        Param::new("reference", ParamType::Text),
        Param::new("sheet", ParamType::Text),
    ],
    &[],
);

/// xlsx のブックの部品（workbook.xml）がないか読めないので、表として読まなかった（#391）
///
/// `xlsx.file.invalid`（0.2.0 から）
pub const XLSX_FILE_INVALID: Code = Code::new("xlsx.file.invalid", Severity::Error, None, &[], &[]);

/// xlsx の部品がないか XML として読めないので、その部品を飛ばした（#391）。name は書庫の中の名前かシートの名前
///
/// `xlsx.part.invalid`（0.2.0 から）
pub const XLSX_PART_INVALID: Code = Code::new(
    "xlsx.part.invalid",
    Severity::Warning,
    None,
    &[Param::new("name", ParamType::Text)],
    &[],
);

/// ワークシートでないシート（グラフのシートなど）なので、読まなかった（#391）
///
/// `xlsx.sheet.unsupported`（0.2.0 から）
pub const XLSX_SHEET_UNSUPPORTED: Code = Code::new(
    "xlsx.sheet.unsupported",
    Severity::Info,
    None,
    &[Param::new("name", ParamType::Text)],
    &[],
);

/// XML の形になっていないので読めない
///
/// `xml.document.invalid`（0.2.0 から）
pub const XML_DOCUMENT_INVALID: Code =
    Code::new("xml.document.invalid", Severity::Error, None, &[], &[]);

/// 文書型宣言（DTD）を処理しなかった。外部の DTD と実体も解決しない
///
/// `xml.dtd.dropped`（0.2.0 から）
pub const XML_DTD_DROPPED: Code = Code::new("xml.dtd.dropped", Severity::Info, None, &[], &[]);

/// UTF-8 と Shift_JIS のほかの文字コードなので、読まなかった
///
/// `xml.encoding.unsupported`（0.2.0 から）
pub const XML_ENCODING_UNSUPPORTED: Code = Code::new(
    "xml.encoding.unsupported",
    Severity::Error,
    None,
    &[Param::new("encoding", ParamType::Text)],
    &["encoding"],
);

/// DTD で宣言する実体の参照なので、戻さずにそのままの文字にした
///
/// `xml.entity.unsupported`（0.2.0 から）
pub const XML_ENTITY_UNSUPPORTED: Code = Code::new(
    "xml.entity.unsupported",
    Severity::Warning,
    None,
    &[Param::new("name", ParamType::Text)],
    &["name"],
);

/// 宣言されていない名前空間の接頭辞なので、名前空間のない名前にした
///
/// `xml.namespace.missing`（0.2.0 から）
pub const XML_NAMESPACE_MISSING: Code = Code::new(
    "xml.namespace.missing",
    Severity::Warning,
    None,
    &[Param::new("prefix", ParamType::Text)],
    &["prefix"],
);

/// 列の数から属性を推定できないので、4 列目からを移さなかった
///
/// `xyz.column.dropped`（0.2.0 から）
pub const XYZ_COLUMN_DROPPED: Code = Code::new(
    "xyz.column.dropped",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("columns", ParamType::UInt)],
    &[],
);

/// XYZ は座標系を書かないので、座標系を不明にした
///
/// `xyz.crs.missing`（0.2.0 から）
pub const XYZ_CRS_MISSING: Code = Code::new("xyz.crs.missing", Severity::Warning, None, &[], &[]);

/// 3 つ以上の数を並べた行がない
///
/// `xyz.file.invalid`（0.2.0 から）
pub const XYZ_FILE_INVALID: Code = Code::new("xyz.file.invalid", Severity::Error, None, &[], &[]);

/// 数を読めない行か、列の数が最初の行と違う行を、読まなかった
///
/// `xyz.line.invalid`（0.2.0 から）
pub const XYZ_LINE_INVALID: Code = Code::new(
    "xyz.line.invalid",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[
        Param::new("line", ParamType::UInt),
        Param::new("lines", ParamType::UInt),
    ],
    &[],
);

/// 中央ディレクトリの終わりの記録がないか、中央ディレクトリが壊れていて、ZIP の書庫として読めない
///
/// `zip.archive.invalid`（0.2.0 から）
pub const ZIP_ARCHIVE_INVALID: Code =
    Code::new("zip.archive.invalid", Severity::Error, None, &[], &[]);

/// 展開した項目の CRC-32 が、記録した値と合わない
///
/// `zip.crc.invalid`（0.2.0 から）
pub const ZIP_CRC_INVALID: Code = Code::new(
    "zip.crc.invalid",
    Severity::Error,
    None,
    &[Param::new("name", ParamType::Text)],
    &[],
);

/// 項目のデータを展開できない（deflate が壊れている・大きさが記録と合わない）
///
/// `zip.data.invalid`（0.2.0 から）
pub const ZIP_DATA_INVALID: Code = Code::new(
    "zip.data.invalid",
    Severity::Error,
    None,
    &[Param::new("name", ParamType::Text)],
    &[],
);

/// 同じ名前の項目が複数あるので、初めのものを使った
///
/// `zip.duplicate.dropped`（0.2.0 から）
pub const ZIP_DUPLICATE_DROPPED: Code = Code::new(
    "zip.duplicate.dropped",
    Severity::Warning,
    None,
    &[Param::new("name", ParamType::Text)],
    &[],
);

/// 暗号化した項目は読まない（決定 0210）
///
/// `zip.encryption.unsupported`（0.2.0 から）
pub const ZIP_ENCRYPTION_UNSUPPORTED: Code = Code::new(
    "zip.encryption.unsupported",
    Severity::Warning,
    None,
    &[Param::new("name", ParamType::Text)],
    &[],
);

/// 局所の見出しが壊れているか、データが書庫の外にはみ出す項目を読まなかった
///
/// `zip.entry.invalid`（0.2.0 から）
pub const ZIP_ENTRY_INVALID: Code = Code::new(
    "zip.entry.invalid",
    Severity::Warning,
    None,
    &[Param::new("name", ParamType::Text)],
    &[],
);

/// stored と deflate でない圧縮の方式の項目は読まない（決定 0210）
///
/// `zip.method.unsupported`（0.2.0 から）
pub const ZIP_METHOD_UNSUPPORTED: Code = Code::new(
    "zip.method.unsupported",
    Severity::Warning,
    None,
    &[
        Param::new("method", ParamType::UInt),
        Param::new("name", ParamType::Text),
    ],
    &[],
);

/// ZIP の項目の名前が、パスとして安全でない（`..`・絶対パス・ドライブ名・NUL を含む）
///
/// `zip.path.unsafe`（0.0.0 から）
pub const ZIP_PATH_UNSAFE: Code = Code::new(
    "zip.path.unsafe",
    Severity::Warning,
    None,
    &[Param::new("name", ParamType::Text)],
    &[],
);

/// データの範囲がほかの項目と重なる項目を読まなかった（決定 0210）
///
/// `zip.range.unsafe`（0.2.0 から）
pub const ZIP_RANGE_UNSAFE: Code = Code::new(
    "zip.range.unsafe",
    Severity::Warning,
    None,
    &[Param::new("name", ParamType::Text)],
    &[],
);

/// 複数の部分に分けた ZIP の書庫は、読まない（決定 0210）
///
/// `zip.split.unsupported`（0.2.0 から）
pub const ZIP_SPLIT_UNSUPPORTED: Code =
    Code::new("zip.split.unsupported", Severity::Error, None, &[], &[]);

/// 台帳のすべての code（名前の順）
#[allow(deprecated)]
pub const ALL: &[Code] = &[
    BCF_ELEMENT_UNSUPPORTED,
    BCF_FILE_INVALID,
    BCF_LOCATION_UNSUPPORTED,
    BCF_MARKUP_UNSUPPORTED,
    BCF_ORIGIN_DROPPED,
    BCF_SIZE_UNSUPPORTED,
    BCF_VALUE_ASSUMED,
    BCF_VALUE_MISSING,
    BCF_VERSION_UNSUPPORTED,
    BUDGET_DECODED_BYTES_EXCEEDED,
    BUDGET_DEPTH_EXCEEDED,
    BUDGET_ELEMENTS_EXCEEDED,
    BUDGET_INPUT_BYTES_EXCEEDED,
    BUDGET_RECORDS_EXCEEDED,
    BUDGET_SEGMENTS_EXCEEDED,
    BUDGET_TIME_EXCEEDED,
    BUDGET_WORK_EXCEEDED,
    BUILDING_GLOBAL_ID_EXISTS,
    BUILDING_GLOBAL_ID_INVALID,
    BUILDING_ID_EXISTS,
    BUILDING_PLACEMENT_INVALID,
    BUILDING_PLACEMENT_MISSING,
    BUILDING_PLACEMENT_UNSUPPORTED,
    CALL_DOCUMENT_EXISTS,
    CALL_DOCUMENT_MISSING,
    CALL_DOCUMENT_UNSUPPORTED,
    CALL_FIELD_MISSING,
    CALL_FIELD_UNSUPPORTED,
    CALL_FORMAT_UNSUPPORTED,
    CALL_INPUT_MISSING,
    CALL_JSON_INVALID,
    CALL_OP_UNSUPPORTED,
    CALL_PROCESS_CAUGHT,
    CALL_REQUEST_CANCELLED,
    CALL_VALUE_INVALID,
    CALL_VERSION_UNSUPPORTED,
    CATALOG_FILE_INVALID,
    CHANGE_CHARACTER_UNSUPPORTED,
    CHANGE_DOCUMENT_STALE,
    CHANGE_EDIT_UNSUPPORTED,
    CHANGE_ELEMENT_MISSING,
    CHANGE_ELEMENT_STALE,
    CHANGE_FORMAT_UNSUPPORTED,
    CHANGE_SYMBOL_CYCLE_INVALID,
    CHANGE_XDATA_UNSUPPORTED,
    CITYGML_CRS_UNSUPPORTED,
    CITYGML_ELEMENT_UNSUPPORTED,
    CITYGML_REFERENCE_UNSUPPORTED,
    CITYGML_VALUE_INVALID,
    CIVIL_CURVE_APPROXIMATED,
    CIVIL_CURVE_UNSUPPORTED,
    CIVIL_DISTANCE_MISSING,
    CIVIL_PROFILE_MISSING,
    CIVIL_SEGMENT_START_INVALID,
    CIVIL_SEGMENT_INVALID,
    CIVIL_STATION_ASSUMED,
    CIVIL_STATION_INVALID,
    CIVIL_STATION_MISSING,
    CIVIL_TRIANGLE_INVALID,
    CONVERT_ATTRIBUTE_DRAWN,
    CONVERT_ELEMENT_UNSUPPORTED,
    CONVERT_FORMAT_UNSUPPORTED,
    CONVERT_VIEW_APPROXIMATED,
    CRS_DATUM_GRID_INVALID,
    CRS_DATUM_GRID_MISSING,
    CRS_DATUM_SHIFT_MISSING,
    CRS_GEOID_INVALID,
    CRS_GEOID_UNSUPPORTED,
    CRS_HEIGHT_DATUM_UNSUPPORTED,
    CRS_HEIGHT_REALIZATION_ASSUMED,
    CRS_HEIGHT_REALIZATION_MIXED,
    CSV_ENCODING_INVALID,
    CSV_QUOTE_INVALID,
    DELIVERY_FILE_INVALID,
    DELIVERY_NAME_UNSAFE,
    DEVIATION_CRS_MIXED,
    DRAWING_MARK_HEIGHT_DROPPED,
    DRAWING_PLACEMENT_INVALID,
    DWG_CHANGE_UNSUPPORTED,
    DWG_CRC_INVALID,
    DWG_DATA_INVALID,
    DWG_EDIT_UNSUPPORTED,
    DWG_FILE_INVALID,
    DWG_LAYER_MISSING,
    DWG_OBJECT_INVALID,
    DWG_OBJECT_MISSING,
    DWG_RECORD_MISSING,
    DWG_TABLE_FORMAT_UNSUPPORTED,
    DWG_TYPE_UNSUPPORTED,
    DWG_VERSION_UNSUPPORTED,
    DWG_XDATA_INVALID,
    DXF_ARC_DIMENSION_LEADER_DROPPED,
    DXF_BINARY_VALUE_UNSUPPORTED,
    DXF_BINARY_INVALID,
    DXF_BINARY_UNSUPPORTED,
    DXF_BLOCK_CYCLE_INVALID,
    DXF_BLOCK_MISSING,
    DXF_CELL_VALUE_DROPPED,
    DXF_CHANGE_UNSUPPORTED,
    DXF_CLIP_DROPPED,
    DXF_CODE_PAGE_UNSUPPORTED,
    DXF_ELEMENT_MISSING,
    DXF_ENTITY_DROPPED,
    DXF_EXTENSION_DICTIONARY_DROPPED,
    DXF_EXTENSION_DICTIONARY_UNSUPPORTED,
    DXF_GROUP_CODE_DROPPED,
    DXF_GROUP_CODE_MISSING,
    DXF_HANDLE_SEED_MISSING,
    DXF_HATCH_SPLINE_UNSUPPORTED,
    DXF_HEADER_VARIABLE_DROPPED,
    DXF_LEADER_LINE_STYLE_DROPPED,
    DXF_LIFT_DROPPED,
    DXF_LIFT_INVALID,
    DXF_LINE_TYPE_ELEMENT_DROPPED,
    DXF_MTEXT_COLUMNS_UNSUPPORTED,
    DXF_NON_PLANAR_DROPPED,
    DXF_OBJECT_UNSUPPORTED,
    DXF_PAPER_SPACE_DROPPED,
    DXF_REAL_UNSUPPORTED,
    DXF_RECORD_MISSING,
    DXF_RECORD_UNSAFE,
    DXF_SECTION_DROPPED,
    DXF_TABLE_DROPPED,
    DXF_TAG_INVALID,
    DXF_TAG_MISSING,
    DXF_TEXT_EQUIVALENT,
    DXF_TEXT_INVALID,
    DXF_UNITS_MISSING,
    DXF_VALUE_INVALID,
    DXF_VALUE_UNSAFE,
    DXF_VERSION_UNSUPPORTED,
    DXF_VIEWPORT_UNSUPPORTED,
    DXF_XDATA_UNSUPPORTED,
    E57_ATTRIBUTE_DROPPED,
    E57_COORDINATES_MISSING,
    E57_CRS_INVALID,
    E57_CRS_MISSING,
    E57_CRS_UNSUPPORTED,
    E57_FILE_INVALID,
    E57_IMAGE_DROPPED,
    E57_POINT_DROPPED,
    E57_POINTS_INVALID,
    E57_VERTICAL_CRS_UNSUPPORTED,
    EXPRESS_ABSTRACT_INVALID,
    EXPRESS_ATTRIBUTE_COUNT_INVALID,
    EXPRESS_ENTITY_MISSING,
    EXPRESS_FILE_INVALID,
    EXPRESS_RULE_UNSUPPORTED,
    EXPRESS_SCHEMA_MIXED,
    EXPRESS_VALUE_INVALID,
    EXPRESS_VALUE_MISSING,
    IDS_APPLICABILITY_EXISTS,
    IDS_APPLICABILITY_MISSING,
    IDS_APPLICABILITY_UNSUPPORTED,
    IDS_FACET_UNSUPPORTED,
    IDS_FILE_INVALID,
    IDS_REQUIREMENT_INVALID,
    IDS_REQUIREMENT_UNSUPPORTED,
    IDS_RESTRICTION_UNSUPPORTED,
    IDS_VERSION_MIXED,
    IFC_ALIGNMENT_SEGMENT_UNSUPPORTED,
    IFC_ATTRIBUTE_INVALID,
    IFC_CIVIL_GEOMETRY_DROPPED,
    IFC_CIVIL_UNIT_ASSUMED,
    IFC_DUPLICATE_DROPPED,
    IFC_ENTITY_DROPPED,
    IFC_ENTRY_MISSING,
    IFC_ENTRY_UNSUPPORTED,
    IFC_FILE_INVALID,
    IFC_GLOBAL_ID_MISSING,
    IFC_HEIGHT_DATUM_UNSUPPORTED,
    IFC_MAP_CRS_MISSING,
    IFC_MAP_CRS_UNSUPPORTED,
    IFC_PARENT_EXISTS,
    IFC_PLACEMENT_INVALID,
    IFC_PLACEMENT_UNSUPPORTED,
    IFC_RECORD_INVALID,
    IFC_REFERENCE_MISSING,
    IFC_SCHEMA_UNSUPPORTED,
    IFC_STATION_MISSING,
    IFC_TEXT_INVALID,
    IFC_UNIT_MISSING,
    IFC_VERSION_UNSUPPORTED,
    INTERNAL_PANIC_CAUGHT,
    JPEG_EXIF_INVALID,
    JPEG_FILE_INVALID,
    JPEG_SEGMENT_INVALID,
    JPEG_XMP_INVALID,
    JWW_BODY_INVALID,
    JWW_FILE_INVALID,
    JWW_HEADER_INVALID,
    JWW_LIFT_DROPPED,
    JWW_LIFT_INVALID,
    JWW_TRAILER_INVALID,
    LANDXML_CROSS_SECTION_LINE_UNSUPPORTED,
    LANDXML_CRS_UNSUPPORTED,
    LANDXML_ELEMENT_UNSUPPORTED,
    LANDXML_FACE_DROPPED,
    LANDXML_HEIGHT_DATUM_UNSUPPORTED,
    LANDXML_VALUE_INVALID,
    LAS_ATTRIBUTE_DROPPED,
    LAS_CRS_MISSING,
    LAS_CRS_UNSUPPORTED,
    LAS_FILE_INVALID,
    LAS_LAZ_INVALID,
    LAS_POINT_FORMAT_UNSUPPORTED,
    LAS_POINTS_INVALID,
    LAS_VERTICAL_CRS_UNSUPPORTED,
    LAS_VLR_INVALID,
    PLY_CRS_MISSING,
    PLY_ELEMENT_DROPPED,
    PLY_FILE_INVALID,
    PLY_PROPERTY_DROPPED,
    PROFILE_CHARACTERS_INVALID,
    PROFILE_COLOR_INVALID,
    PROFILE_COLOR_UNSUPPORTED,
    PROFILE_DELIVERY_CHARACTERS_INVALID,
    PROFILE_DELIVERY_DOCTYPE_INVALID,
    PROFILE_DELIVERY_LINK_INVALID,
    PROFILE_DELIVERY_NAME_INVALID,
    PROFILE_DELIVERY_PHOTO_DATE_INVALID,
    PROFILE_DELIVERY_PHOTO_DATE_UNSUPPORTED,
    PROFILE_DELIVERY_PHOTO_PIXELS_INVALID,
    PROFILE_DELIVERY_PHOTO_PIXELS_UNSUPPORTED,
    PROFILE_DELIVERY_REQUIRED_INVALID,
    PROFILE_DELIVERY_STANDARD_INVALID,
    PROFILE_DELIVERY_STANDARD_UNSUPPORTED,
    PROFILE_DELIVERY_UNREFERENCED_INVALID,
    PROFILE_EXTENDS_INVALID,
    PROFILE_EXTENDS_MISSING,
    PROFILE_FILE_NAME_INVALID,
    PROFILE_FILE_NAME_UNSUPPORTED,
    PROFILE_FORMAT_UNSUPPORTED,
    PROFILE_LAYER_NAME_INVALID,
    PROFILE_LINE_TYPE_INVALID,
    PROFILE_LINE_TYPE_UNSUPPORTED,
    PROFILE_LINE_WIDTH_BY_SHEET_INVALID,
    PROFILE_LINE_WIDTH_BY_SHEET_UNSUPPORTED,
    PROFILE_LINE_WIDTH_INVALID,
    PROFILE_LINE_WIDTH_UNSUPPORTED,
    PROFILE_ORIENTATION_INVALID,
    PROFILE_ORIENTATION_UNSUPPORTED,
    PROFILE_RULE_UNSUPPORTED,
    PROFILE_SCALE_INVALID,
    PROFILE_SCALE_UNSUPPORTED,
    PROFILE_SHEET_SIZE_INVALID,
    PROFILE_SHEET_SIZE_UNSUPPORTED,
    PROFILE_SURFACE_ELEVATION_INVALID,
    PROFILE_SURFACE_ELEVATION_UNSUPPORTED,
    PROFILE_TEXT_HEIGHT_INVALID,
    PROFILE_TEXT_HEIGHT_UNSUPPORTED,
    PROFILE_VOCABULARY_INVALID,
    REVIEW_FILE_INVALID,
    REVIEW_FORMAT_UNSUPPORTED,
    REVIEW_ID_EXISTS,
    REVIEW_REFERENCE_MISSING,
    REVIEW_TARGET_MISSING,
    REVIEW_TARGET_STALE,
    REVIEW_VERSION_UNSUPPORTED,
    SCENE_ARROWHEAD_DROPPED,
    SCENE_CALLOUT_VALUE_DROPPED,
    SCENE_CALLOUT_DROPPED,
    SCENE_CURVE_APPROXIMATED,
    SCENE_DIMENSION_APPROXIMATED,
    SCENE_DIMENSION_DROPPED,
    SCENE_FIT_TOLERANCE_APPROXIMATED,
    SCENE_FONT_ASSUMED,
    SCENE_FONT_MISSING,
    SCENE_GLYPH_OUTLINE_INVALID,
    SCENE_GLYPH_MISSING,
    SCENE_GRADIENT_APPROXIMATED,
    SCENE_HATCH_PATTERN_APPROXIMATED,
    SCENE_HATCH_PATTERN_DROPPED,
    SCENE_LEADER_DROPPED,
    SCENE_LINE_TYPE_MARK_DROPPED,
    SCENE_LINE_TYPE_SHAPE_DROPPED,
    SCENE_SPLINE_APPROXIMATED,
    SCENE_TABLE_APPROXIMATED,
    SCENE_TABLE_DROPPED,
    SCENE_TEXT_HEIGHT_ASSUMED,
    SCENE_TEXT_LAYOUT_UNSUPPORTED,
    SCENE_UNDERLAY_CLIP_DROPPED,
    SCENE_UNDERLAY_DROPPED,
    SXF_ARCHIVE_UNSUPPORTED,
    SXF_CODE_MISSING,
    SXF_FILE_INVALID,
    SXF_LIFT_DROPPED,
    SXF_LIFT_INVALID,
    SXF_MEMBER_ASSUMED,
    SXF_P21_REFERENCE_MISSING,
    SXF_P21_SHEET_MISSING,
    SXF_P21_UNSUPPORTED,
    SXF_RASTER_FRAME_MISSING,
    SXF_RECORD_INVALID,
    SXF_SCHEMA_UNSUPPORTED,
    SXF_SFIG_CYCLE_INVALID,
    SXF_SFIG_MISSING,
    XLSX_CELL_INVALID,
    XLSX_FILE_INVALID,
    XLSX_PART_INVALID,
    XLSX_SHEET_UNSUPPORTED,
    XML_DOCUMENT_INVALID,
    XML_DTD_DROPPED,
    XML_ENCODING_UNSUPPORTED,
    XML_ENTITY_UNSUPPORTED,
    XML_NAMESPACE_MISSING,
    XYZ_COLUMN_DROPPED,
    XYZ_CRS_MISSING,
    XYZ_FILE_INVALID,
    XYZ_LINE_INVALID,
    ZIP_ARCHIVE_INVALID,
    ZIP_CRC_INVALID,
    ZIP_DATA_INVALID,
    ZIP_DUPLICATE_DROPPED,
    ZIP_ENCRYPTION_UNSUPPORTED,
    ZIP_ENTRY_INVALID,
    ZIP_METHOD_UNSUPPORTED,
    ZIP_PATH_UNSAFE,
    ZIP_RANGE_UNSAFE,
    ZIP_SPLIT_UNSUPPORTED,
];
