// 生成したファイル。手で直さない。
// 元は catalog/codes.toml で、`cargo xtask codegen` で作り直す。

//! 診断の code の定数（決定 0009）。台帳の名前の順に並べる。

use crate::code::{Code, Fidelity, Param, ParamType, Severity};

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

/// 図面モデルの要素の変更を、元の図形のレコードに書き戻せない。今は、LINE・CIRCLE の形の置き換えと、続くレコードを持たない図形の除去だけを扱う（決定 0139）
///
/// `dxf.change.unsupported`（0.0.0 から）
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

/// コード・ページの版の ASCII でない文字を、まだ読めないので、その文字の図形を図面モデルに移さなかった
///
/// `dxf.code-page.unsupported`（0.0.0 から）
pub const DXF_CODE_PAGE_UNSUPPORTED: Code = Code::new(
    "dxf.code-page.unsupported",
    Severity::Warning,
    Some(Fidelity::Dropped),
    &[Param::new("kind", ParamType::Text)],
    &["kind"],
);

/// 図面モデルの変更が指す出所の要素が、今のファイルの実寸の空間の図形にない（決定 0139）
///
/// `dxf.element.missing`（0.0.0 から）
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

/// ネイティブの境界でパニックを捕まえた。不具合なので直す
///
/// `internal.panic.caught`（0.0.0 から）
pub const INTERNAL_PANIC_CAUGHT: Code =
    Code::new("internal.panic.caught", Severity::Error, None, &[], &[]);

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

/// 台帳のすべての code（名前の順）
#[allow(deprecated)]
pub const ALL: &[Code] = &[
    BUDGET_DECODED_BYTES_EXCEEDED,
    BUDGET_DEPTH_EXCEEDED,
    BUDGET_ELEMENTS_EXCEEDED,
    BUDGET_INPUT_BYTES_EXCEEDED,
    BUDGET_RECORDS_EXCEEDED,
    BUDGET_SEGMENTS_EXCEEDED,
    BUDGET_TIME_EXCEEDED,
    BUDGET_WORK_EXCEEDED,
    CALL_DOCUMENT_EXISTS,
    CALL_DOCUMENT_MISSING,
    CALL_FIELD_MISSING,
    CALL_FIELD_UNSUPPORTED,
    CALL_FORMAT_UNSUPPORTED,
    CALL_INPUT_MISSING,
    CALL_JSON_INVALID,
    CALL_OP_UNSUPPORTED,
    CALL_REQUEST_CANCELLED,
    CALL_VALUE_INVALID,
    CALL_VERSION_UNSUPPORTED,
    CATALOG_FILE_INVALID,
    DWG_CHANGE_UNSUPPORTED,
    DWG_CRC_INVALID,
    DWG_DATA_INVALID,
    DWG_EDIT_UNSUPPORTED,
    DWG_FILE_INVALID,
    DWG_LAYER_MISSING,
    DWG_OBJECT_INVALID,
    DWG_OBJECT_MISSING,
    DWG_RECORD_MISSING,
    DWG_TYPE_UNSUPPORTED,
    DWG_VERSION_UNSUPPORTED,
    DWG_XDATA_INVALID,
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
    INTERNAL_PANIC_CAUGHT,
    JWW_BODY_INVALID,
    JWW_FILE_INVALID,
    JWW_HEADER_INVALID,
    JWW_LIFT_DROPPED,
    JWW_LIFT_INVALID,
    JWW_TRAILER_INVALID,
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
    ZIP_PATH_UNSAFE,
];
