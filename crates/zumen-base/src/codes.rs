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

/// 要求に、この核が知らない欄か、この形（wasm など）では使えない欄がある
///
/// `call.field.unsupported`（0.0.0 から）
pub const CALL_FIELD_UNSUPPORTED: Code = Code::new(
    "call.field.unsupported",
    Severity::Error,
    None,
    &[Param::new("pointer", ParamType::Text)],
    &[],
);

/// 入力の形式を判定できない。または、この核では読めない形式
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

/// この核（wasm ではこのモジュール）にない op
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

/// 要求の版 v を、この核が扱えない
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

/// バイナリの DXF は、まだ読めない
///
/// `dxf.binary.unsupported`（0.0.0 から）
pub const DXF_BINARY_UNSUPPORTED: Code =
    Code::new("dxf.binary.unsupported", Severity::Error, None, &[], &[]);

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

/// ネイティブの境界でパニックを捕まえた。不具合なので直す
///
/// `internal.panic.caught`（0.0.0 から）
pub const INTERNAL_PANIC_CAUGHT: Code =
    Code::new("internal.panic.caught", Severity::Error, None, &[], &[]);

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
    DXF_BINARY_UNSUPPORTED,
    DXF_ENTITY_DROPPED,
    DXF_GROUP_CODE_DROPPED,
    DXF_GROUP_CODE_MISSING,
    DXF_HEADER_VARIABLE_DROPPED,
    DXF_REAL_UNSUPPORTED,
    DXF_RECORD_MISSING,
    DXF_SECTION_DROPPED,
    DXF_TABLE_DROPPED,
    DXF_TAG_INVALID,
    DXF_TAG_MISSING,
    DXF_TEXT_EQUIVALENT,
    DXF_TEXT_INVALID,
    DXF_VALUE_UNSAFE,
    DXF_VERSION_UNSUPPORTED,
    INTERNAL_PANIC_CAUGHT,
    ZIP_PATH_UNSAFE,
];
