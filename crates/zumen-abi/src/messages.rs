// 生成したファイル。手で直さない。
// 元は schema/messages.json で、`cargo xtask codegen` で作り直す。

//! 境界のメッセージの型（決定 0012）。スキーマの定義ごとに 1 つの型がある。

use std::collections::BTreeMap;

use zumen_json::{self as json, FromJson, Invalid, ObjectReader, Problem, ToJson, Value, Writer};

/// 要求。op の値で形が決まる
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// 核の版と、使える op・形式を尋ねる
    Capabilities(CapabilitiesRequest),
    /// 入力を読んで、文書を作る。形式は入力から判定する
    Open(OpenRequest),
    /// 文書と入力を捨てる。名指ししたものが 1 つでもなければ、何も捨てない
    Close(CloseRequest),
    /// 診断の文を、カタログから組み立てる（決定 0009）
    Describe(DescribeRequest),
}

impl Request {
    /// 目印の欄（`op`）の値（スキーマの順）
    pub const TAGS: &[&str] = &["capabilities", "open", "close", "describe"];

    /// 目印の欄（`op`）の値
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Capabilities(_) => "capabilities",
            Self::Open(_) => "open",
            Self::Close(_) => "close",
            Self::Describe(_) => "describe",
        }
    }
}

impl FromJson for Request {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = match object.tag("op")? {
            "capabilities" => Self::Capabilities(CapabilitiesRequest::read_fields(&mut object)?),
            "open" => Self::Open(OpenRequest::read_fields(&mut object)?),
            "close" => Self::Close(CloseRequest::read_fields(&mut object)?),
            "describe" => Self::Describe(DescribeRequest::read_fields(&mut object)?),
            other => return Err(Invalid::new(Problem::Tag(other.to_string())).in_key("op")),
        };
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Request {
    fn to_json(&self, out: &mut Writer) {
        match self {
            Self::Capabilities(value) => value.to_json(out),
            Self::Open(value) => value.to_json(out),
            Self::Close(value) => value.to_json(out),
            Self::Describe(value) => value.to_json(out),
        }
    }
}

/// どの op の要求にも置ける欄
#[derive(Clone, Debug, PartialEq)]
pub struct RequestCommon {
    /// メッセージのスキーマの大版。今は 1
    pub v: u64,
    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    pub budget: Option<Budget>,
    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    pub partial: Option<bool>,
    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    pub max_places: Option<u64>,
    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    pub time_limit_ms: Option<u64>,
}

impl RequestCommon {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let v = object.required("v", |value| json::uint(value, 1, 9007199254740991))?;
        let budget = object.optional("budget", Budget::from_json)?;
        let partial = object.optional("partial", json::boolean)?;
        let max_places =
            object.optional("max_places", |value| json::uint(value, 0, 9007199254740991))?;
        let time_limit_ms = object.optional("time_limit_ms", |value| {
            json::uint(value, 1, 9007199254740991)
        })?;
        Ok(Self {
            v,
            budget,
            partial,
            max_places,
            time_limit_ms,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("v");
        out.uint(self.v);
        if let Some(value) = &self.budget {
            out.key("budget");
            value.to_json(out);
        }
        if let Some(value) = &self.partial {
            out.key("partial");
            out.bool(*value);
        }
        if let Some(value) = &self.max_places {
            out.key("max_places");
            out.uint(*value);
        }
        if let Some(value) = &self.time_limit_ms {
            out.key("time_limit_ms");
            out.uint(*value);
        }
    }
}

/// 予算の単位ごとの上限（決定 0010）
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Budget {
    /// 受け取ったバイト数
    pub input_bytes: Option<Limit>,
    /// 展開・復号で作ったバイト数。入れ子の分も足す
    pub decoded_bytes: Option<Limit>,
    /// L0・L1 で読んだレコードの数
    pub records: Option<Limit>,
    /// L2・L3 で作った要素の数
    pub elements: Option<Limit>,
    /// 入れ子と再帰の深さ
    pub depth: Option<Limit>,
    /// 幾何の分割で作った点・線・面の数
    pub segments: Option<Limit>,
    /// ほかの単位に入らない繰り返しの回数
    pub work: Option<Limit>,
}

impl Budget {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let input_bytes = object.optional("input_bytes", Limit::from_json)?;
        let decoded_bytes = object.optional("decoded_bytes", Limit::from_json)?;
        let records = object.optional("records", Limit::from_json)?;
        let elements = object.optional("elements", Limit::from_json)?;
        let depth = object.optional("depth", Limit::from_json)?;
        let segments = object.optional("segments", Limit::from_json)?;
        let work = object.optional("work", Limit::from_json)?;
        Ok(Self {
            input_bytes,
            decoded_bytes,
            records,
            elements,
            depth,
            segments,
            work,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.input_bytes {
            out.key("input_bytes");
            value.to_json(out);
        }
        if let Some(value) = &self.decoded_bytes {
            out.key("decoded_bytes");
            value.to_json(out);
        }
        if let Some(value) = &self.records {
            out.key("records");
            value.to_json(out);
        }
        if let Some(value) = &self.elements {
            out.key("elements");
            value.to_json(out);
        }
        if let Some(value) = &self.depth {
            out.key("depth");
            value.to_json(out);
        }
        if let Some(value) = &self.segments {
            out.key("segments");
            value.to_json(out);
        }
        if let Some(value) = &self.work {
            out.key("work");
            value.to_json(out);
        }
    }
}

impl FromJson for Budget {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Budget {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 1 つの単位の上限。上限をなくすときは unlimited と書く
#[derive(Clone, Debug, PartialEq)]
pub enum Limit {
    /// この量までは使える
    Integer(u64),
    /// 上限なし
    Unlimited,
}

impl FromJson for Limit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        match value {
            Value::Number(_) => json::uint(value, 0, 9007199254740991).map(Self::Integer),
            Value::String(text) => match text.as_str() {
                "unlimited" => Ok(Self::Unlimited),
                _ => Err(Invalid::new(Problem::Value)),
            },
            _ => Err(Invalid::new(Problem::Type)),
        }
    }
}

impl ToJson for Limit {
    fn to_json(&self, out: &mut Writer) {
        match self {
            Self::Integer(value) => out.uint(*value),
            Self::Unlimited => out.string("unlimited"),
        }
    }
}

/// 核の版と、使える op・形式を尋ねる
#[derive(Clone, Debug, PartialEq)]
pub struct CapabilitiesRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
}

impl CapabilitiesRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "capabilities")?;
        Ok(Self { request_common })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("capabilities");
    }
}

impl FromJson for CapabilitiesRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CapabilitiesRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 核の版と、使える op・形式（決定 0015）
#[derive(Clone, Debug, PartialEq)]
pub struct CapabilitiesResult {
    /// パッケージの版（semver）
    pub package: String,
    /// ABI の大版。zm_abi_version と同じ
    pub abi: u64,
    /// メッセージのスキーマの版
    pub schema: SchemaVersion,
    /// この核（wasm ではこのモジュール）で使える op の名前
    pub ops: Vec<String>,
    /// 読み書きできる形式
    pub formats: Vec<Format>,
}

impl CapabilitiesResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let package = object.required("package", json::string)?;
        let abi = object.required("abi", |value| json::uint(value, 1, 9007199254740991))?;
        let schema = object.required("schema", SchemaVersion::from_json)?;
        let ops = object.required("ops", |value| json::array(value, json::string))?;
        let formats = object.required("formats", |value| json::array(value, Format::from_json))?;
        Ok(Self {
            package,
            abi,
            schema,
            ops,
            formats,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("package");
        out.string(&self.package);
        out.key("abi");
        out.uint(self.abi);
        out.key("schema");
        self.schema.to_json(out);
        out.key("ops");
        out.array(&self.ops, |out, item| out.string(item));
        out.key("formats");
        out.array(&self.formats, |out, item| item.to_json(out));
    }
}

impl FromJson for CapabilitiesResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CapabilitiesResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// メッセージのスキーマの版。大版は要求と応答の v、小版は op・欄・列挙の値を足すたびに上げる
#[derive(Clone, Debug, PartialEq)]
pub struct SchemaVersion {
    /// 大版
    pub major: u64,
    /// 小版
    pub minor: u64,
}

impl SchemaVersion {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let major = object.required("major", |value| json::uint(value, 1, 9007199254740991))?;
        let minor = object.required("minor", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self { major, minor })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("major");
        out.uint(self.major);
        out.key("minor");
        out.uint(self.minor);
    }
}

impl FromJson for SchemaVersion {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SchemaVersion {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 読み書きできる形式
#[derive(Clone, Debug, PartialEq)]
pub struct Format {
    /// 形式の名前（dxf・sfc など）
    pub id: String,
    /// 形式の系統（autocad・sxf など、決定 0008）
    pub family: String,
    /// 読めるか
    pub read: bool,
    /// 書けるか
    pub write: bool,
    /// この形式の ID の規則の版（決定 0015）
    pub id_rules: u64,
}

impl Format {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let family = object.required("family", json::string)?;
        let read = object.required("read", json::boolean)?;
        let write = object.required("write", json::boolean)?;
        let id_rules =
            object.required("id_rules", |value| json::uint(value, 1, 9007199254740991))?;
        Ok(Self {
            id,
            family,
            read,
            write,
            id_rules,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("family");
        out.string(&self.family);
        out.key("read");
        out.bool(self.read);
        out.key("write");
        out.bool(self.write);
        out.key("id_rules");
        out.uint(self.id_rules);
    }
}

impl FromJson for Format {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Format {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 入力を読んで、文書を作る。形式は入力から判定する
#[derive(Clone, Debug, PartialEq)]
pub struct OpenRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
    /// 読む入力の名前。zm_push で渡したもの
    pub input: String,
    /// 作る文書の名前。書かなければ入力の名前
    pub doc: Option<String>,
}

impl OpenRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "open")?;
        let input = object.required("input", json::string)?;
        let doc = object.optional("doc", json::string)?;
        Ok(Self {
            request_common,
            input,
            doc,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("open");
        out.key("input");
        out.string(&self.input);
        if let Some(value) = &self.doc {
            out.key("doc");
            out.string(value);
        }
    }
}

impl FromJson for OpenRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for OpenRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 作った文書
#[derive(Clone, Debug, PartialEq)]
pub struct OpenResult {
    /// 文書の名前
    pub doc: String,
    /// 判定した形式の名前
    pub format: String,
}

impl OpenResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let doc = object.required("doc", json::string)?;
        let format = object.required("format", json::string)?;
        Ok(Self { doc, format })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("doc");
        out.string(&self.doc);
        out.key("format");
        out.string(&self.format);
    }
}

impl FromJson for OpenResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for OpenResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 文書と入力を捨てる。名指ししたものが 1 つでもなければ、何も捨てない
#[derive(Clone, Debug, PartialEq)]
pub struct CloseRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
    /// 捨てる文書の名前
    pub docs: Option<Vec<String>>,
    /// 捨てる入力の名前
    pub inputs: Option<Vec<String>>,
}

impl CloseRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "close")?;
        let docs = object.optional("docs", |value| json::array(value, json::string))?;
        let inputs = object.optional("inputs", |value| json::array(value, json::string))?;
        Ok(Self {
            request_common,
            docs,
            inputs,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("close");
        if let Some(value) = &self.docs {
            out.key("docs");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.inputs {
            out.key("inputs");
            out.array(value, |out, item| out.string(item));
        }
    }
}

impl FromJson for CloseRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CloseRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 捨てたこと。中身はない
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CloseResult {}

impl CloseResult {
    fn read_fields(_object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        Ok(Self {})
    }

    fn write_fields(&self, _out: &mut Writer) {}
}

impl FromJson for CloseResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CloseResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 診断の文を、カタログから組み立てる（決定 0009）
#[derive(Clone, Debug, PartialEq)]
pub struct DescribeRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
    /// カタログの入力の名前。zm_push で渡したもの。書かなければ、code と値をそのまま文にする
    pub catalog: Option<String>,
    /// 文にする診断。応答で受け取ったもの
    pub diagnostics: Vec<Diagnostic>,
}

impl DescribeRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "describe")?;
        let catalog = object.optional("catalog", json::string)?;
        let diagnostics = object.required("diagnostics", |value| {
            json::array(value, Diagnostic::from_json)
        })?;
        Ok(Self {
            request_common,
            catalog,
            diagnostics,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("describe");
        if let Some(value) = &self.catalog {
            out.key("catalog");
            out.string(value);
        }
        out.key("diagnostics");
        out.array(&self.diagnostics, |out, item| item.to_json(out));
    }
}

impl FromJson for DescribeRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DescribeRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 診断の文
#[derive(Clone, Debug, PartialEq)]
pub struct DescribeResult {
    /// 診断ごとの文。要求の diagnostics と同じ順。カタログにない code は、code と値をそのまま書く
    pub texts: Vec<String>,
}

impl DescribeResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let texts = object.required("texts", |value| json::array(value, json::string))?;
        Ok(Self { texts })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("texts");
        out.array(&self.texts, |out, item| out.string(item));
    }
}

impl FromJson for DescribeResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DescribeResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 診断。人向けの文は持たず、文は describe で組み立てる（決定 0009）
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    /// 変わらない名前（budget.records.exceeded など）。catalog/codes.toml にある
    pub code: String,
    /// 重さ。code ごとに決まる
    pub severity: Severity,
    /// 変換の結果の段。code ごとに決まる。変換の結果を表さない code では持たない
    pub fidelity: Option<Fidelity>,
    /// まとめた件数
    pub count: u64,
    /// 場所。入力の順に先頭の数件
    pub at: Vec<Place>,
    /// 仕様の節
    pub cite: Option<Cite>,
    /// 文に差し込む値。名前と型は code ごとに台帳が決める。2^53 − 1 を超える整数は文字列にする
    pub params: BTreeMap<String, ParamValue>,
}

impl Diagnostic {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let code = object.required("code", json::string)?;
        let severity = object.required("severity", Severity::from_json)?;
        let fidelity = object.optional("fidelity", Fidelity::from_json)?;
        let count = object.required("count", |value| json::uint(value, 1, 9007199254740991))?;
        let at = object.required("at", |value| json::array(value, Place::from_json))?;
        let cite = object.optional("cite", Cite::from_json)?;
        let params = object.required("params", |value| json::map(value, ParamValue::from_json))?;
        Ok(Self {
            code,
            severity,
            fidelity,
            count,
            at,
            cite,
            params,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("code");
        out.string(&self.code);
        out.key("severity");
        self.severity.to_json(out);
        if let Some(value) = &self.fidelity {
            out.key("fidelity");
            value.to_json(out);
        }
        out.key("count");
        out.uint(self.count);
        out.key("at");
        out.array(&self.at, |out, item| item.to_json(out));
        if let Some(value) = &self.cite {
            out.key("cite");
            value.to_json(out);
        }
        out.key("params");
        out.map(&self.params, |out, item| item.to_json(out));
    }
}

impl FromJson for Diagnostic {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Diagnostic {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 診断の重さ（決定 0009）。info は何も失っていない、warning は結果を使える、error は失敗した
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// `info`
    Info,
    /// `warning`
    Warning,
    /// `error`
    Error,
}

impl Severity {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Info, Self::Warning, Self::Error];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "info" => Some(Self::Info),
            "warning" => Some(Self::Warning),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

impl FromJson for Severity {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for Severity {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 変換の結果の段のうち、`native` 以外のもの（決定 0009）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Fidelity {
    /// `equivalent`
    Equivalent,
    /// `approximated`
    Approximated,
    /// `drawn`
    Drawn,
    /// `dropped`
    Dropped,
}

impl Fidelity {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Equivalent,
        Self::Approximated,
        Self::Drawn,
        Self::Dropped,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Equivalent => "equivalent",
            Self::Approximated => "approximated",
            Self::Drawn => "drawn",
            Self::Dropped => "dropped",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "equivalent" => Some(Self::Equivalent),
            "approximated" => Some(Self::Approximated),
            "drawn" => Some(Self::Drawn),
            "dropped" => Some(Self::Dropped),
            _ => None,
        }
    }
}

impl FromJson for Fidelity {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for Fidelity {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 診断の場所。ID と、L1 のレコード・バイト範囲（決定 0008 の出所）
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Place {
    /// ID
    pub id: Option<String>,
    /// L1 のレコードの番号
    pub record: Option<u64>,
    /// L1 のバイト範囲
    pub bytes: Option<ByteRange>,
}

impl Place {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.optional("id", json::string)?;
        let record = object.optional("record", |value| json::uint(value, 0, 9007199254740991))?;
        let bytes = object.optional("bytes", ByteRange::from_json)?;
        Ok(Self { id, record, bytes })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.id {
            out.key("id");
            out.string(value);
        }
        if let Some(value) = &self.record {
            out.key("record");
            out.uint(*value);
        }
        if let Some(value) = &self.bytes {
            out.key("bytes");
            value.to_json(out);
        }
    }
}

impl FromJson for Place {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Place {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// バイト範囲（start から end の手前まで）
#[derive(Clone, Debug, PartialEq)]
pub struct ByteRange {
    /// 先頭の位置
    pub start: u64,
    /// 終わりの位置（含まない）
    pub end: u64,
}

impl ByteRange {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let start = object.required("start", |value| json::uint(value, 0, 9007199254740991))?;
        let end = object.required("end", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self { start, end })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("start");
        out.uint(self.start);
        out.key("end");
        out.uint(self.end);
    }
}

impl FromJson for ByteRange {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ByteRange {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 仕様の節。仕様の本文は写さない
#[derive(Clone, Debug, PartialEq)]
pub struct Cite {
    /// 仕様の一覧での名前
    pub spec: String,
    /// 節
    pub section: String,
}

impl Cite {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let spec = object.required("spec", json::string)?;
        let section = object.required("section", json::string)?;
        Ok(Self { spec, section })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("spec");
        out.string(&self.spec);
        out.key("section");
        out.string(&self.section);
    }
}

impl FromJson for Cite {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Cite {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 診断の値。数か文字列
#[derive(Clone, Debug, PartialEq)]
pub enum ParamValue {
    /// 数
    Number(f64),
    /// 文字列。図面に記録された名前、列挙の値のキー、2^53 − 1 を超える整数
    String(String),
}

impl FromJson for ParamValue {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        match value {
            Value::Number(_) => json::number(value).map(Self::Number),
            Value::String(text) => Ok(Self::String(text.clone())),
            _ => Err(Invalid::new(Problem::Type)),
        }
    }
}

impl ToJson for ParamValue {
    fn to_json(&self, out: &mut Writer) {
        match self {
            Self::Number(value) => out.float(*value),
            Self::String(value) => out.string(value),
        }
    }
}

/// バイナリの結果
#[derive(Clone, Debug, PartialEq)]
pub struct Blob {
    /// 番号。zm_take に渡す
    pub id: u64,
    /// 種類
    pub kind: BlobKind,
    /// バイト数
    pub size: u64,
}

impl Blob {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", |value| json::uint(value, 0, 4294967295))?;
        let kind = object.required("kind", BlobKind::from_json)?;
        let size = object.required("size", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self { id, kind, size })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.uint(self.id);
        out.key("kind");
        self.kind.to_json(out);
        out.key("size");
        out.uint(self.size);
    }
}

impl FromJson for Blob {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Blob {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// バイナリの結果の種類。file は書き出したファイル、image は描いた画像、scene はシーン（#6 で決める配置）、arrow は表（Arrow の IPC ストリーム。ネイティブでは zm_take_arrow でも取り出せる）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BlobKind {
    /// `file`
    File,
    /// `image`
    Image,
    /// `scene`
    Scene,
    /// `arrow`
    Arrow,
}

impl BlobKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::File, Self::Image, Self::Scene, Self::Arrow];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Image => "image",
            Self::Scene => "scene",
            Self::Arrow => "arrow",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "file" => Some(Self::File),
            "image" => Some(Self::Image),
            "scene" => Some(Self::Scene),
            "arrow" => Some(Self::Arrow),
            _ => None,
        }
    }
}

impl FromJson for BlobKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for BlobKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// zm_session_new に渡す設定。渡さなければ既定の設定
#[derive(Clone, Debug, PartialEq)]
pub struct SessionOptions {
    /// メッセージのスキーマの大版。今は 1
    pub v: u64,
    /// 実行の形。書かなければ、ネイティブのライブラリでは `native`、wasm のモジュールでは `wasm`（決定 0013）
    pub engine: Option<Engine>,
}

impl SessionOptions {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let v = object.required("v", |value| json::uint(value, 1, 9007199254740991))?;
        let engine = object.optional("engine", Engine::from_json)?;
        Ok(Self { v, engine })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("v");
        out.uint(self.v);
        if let Some(value) = &self.engine {
            out.key("engine");
            value.to_json(out);
        }
    }
}

impl FromJson for SessionOptions {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SessionOptions {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 実行の形（決定 0013）。`native` はプロセスの中で核を動かす、`process` は子プロセスの CLI に送る、`wasm` は wasm のモジュールで動かす
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Engine {
    /// `native`
    Native,
    /// `process`
    Process,
    /// `wasm`
    Wasm,
}

impl Engine {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Native, Self::Process, Self::Wasm];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Process => "process",
            Self::Wasm => "wasm",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "native" => Some(Self::Native),
            "process" => Some(Self::Process),
            "wasm" => Some(Self::Wasm),
            _ => None,
        }
    }
}

impl FromJson for Engine {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for Engine {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// describe に使うカタログのファイル。catalog/<言語>.toml から生成し、zm_push で渡す（決定 0009）
#[derive(Clone, Debug, PartialEq)]
pub struct Catalog {
    /// カタログの形の大版。今は 1
    pub v: u64,
    /// 言語（ja・en）
    pub lang: String,
    /// パッケージの版。核と同じ版で出す
    pub package: String,
    /// 数の書き方
    pub number: NumberFormat,
    /// code ごとの文のひな形。{名前} に値が、{count} にまとめた件数が入る
    pub codes: BTreeMap<String, String>,
}

impl Catalog {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let v = object.required("v", |value| json::uint(value, 1, 9007199254740991))?;
        let lang = object.required("lang", json::string)?;
        let package = object.required("package", json::string)?;
        let number = object.required("number", NumberFormat::from_json)?;
        let codes = object.required("codes", |value| json::map(value, json::string))?;
        Ok(Self {
            v,
            lang,
            package,
            number,
            codes,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("v");
        out.uint(self.v);
        out.key("lang");
        out.string(&self.lang);
        out.key("package");
        out.string(&self.package);
        out.key("number");
        self.number.to_json(out);
        out.key("codes");
        out.map(&self.codes, |out, item| out.string(item));
    }
}

impl FromJson for Catalog {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Catalog {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 文に差し込む数の書き方。OS のロケールは使わない（決定 0009）
#[derive(Clone, Debug, PartialEq)]
pub struct NumberFormat {
    /// 整数部を 3 桁ごとに区切る文字。空なら区切らない
    pub group_separator: String,
    /// 小数点の文字
    pub decimal_separator: String,
}

impl NumberFormat {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let group_separator = object.required("group_separator", json::string)?;
        let decimal_separator = object.required("decimal_separator", json::string)?;
        Ok(Self {
            group_separator,
            decimal_separator,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("group_separator");
        out.string(&self.group_separator);
        out.key("decimal_separator");
        out.string(&self.decimal_separator);
    }
}

impl FromJson for NumberFormat {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for NumberFormat {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}
