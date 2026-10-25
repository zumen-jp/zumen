// 生成したファイル。手で直さない。
// 元は catalog/codes.toml で、`cargo xtask codegen` で作り直す。

//! 診断の code の定数（決定 0009）。台帳の名前の順に並べる。

use crate::code::{Code, Param, ParamType, Severity};

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
    DXF_TAG_INVALID,
    INTERNAL_PANIC_CAUGHT,
    ZIP_PATH_UNSAFE,
];
