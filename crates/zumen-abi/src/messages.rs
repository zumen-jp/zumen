// 生成したファイル。手で直さない。
// 元は schema/messages.json で、`cargo xtask codegen` で作り直す。

//! 境界のメッセージの型（決定 0012）。スキーマの定義ごとに 1 つの型がある。

use std::collections::BTreeMap;

use zumen_json::{self as json, FromJson, Invalid, ObjectReader, Problem, ToJson, Value, Writer};

/// 要求。op の値で形が決まる
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// コアの版と、使える op・形式を尋ねる
    Capabilities(CapabilitiesRequest),
    /// 入力を読んで、文書を作る。形式は入力から判定する
    Open(OpenRequest),
    /// 文書と入力を捨てる。名指ししたものが 1 つでもなければ、何も捨てない
    Close(CloseRequest),
    /// 診断の文を、カタログから組み立てる（決定 0009）
    Describe(DescribeRequest),
    /// 開いた文書を図面モデル（L2、決定 0022）に変換し、その要約を返す。ネイティブと wasm の出力の一致を、適合試験で比べるためにも使う（決定 0007）
    Drawing(DrawingRequest),
    /// 開いた文書を図面モデル（L2、決定 0022）に変換し、要素の一覧を返す。実寸の空間、用紙の順に、それぞれの要素を入力の順に並べる（決定 0117）
    Entities(EntitiesRequest),
    /// 開いた文書に、変更の一覧を当てて書き戻す（決定 0139・0156）。要素は、entities が返す ID とハッシュで指す。どれか 1 つでも当てられなければ、何も変えない。当てたら、文書を書き戻したものに替え、書き戻したファイルを blobs で返す。小版 7 から
    Apply(ApplyRequest),
    /// 開いた文書を図面モデル（L2）に変換し、レイヤと線種の表の項目を返す（決定 0156）。小版 8 から
    Tables(TablesRequest),
}

impl Request {
    /// 目印の欄（`op`）の値（スキーマの順）
    pub const TAGS: &[&str] = &[
        "capabilities",
        "open",
        "close",
        "describe",
        "drawing",
        "entities",
        "apply",
        "tables",
    ];

    /// 目印の欄（`op`）の値
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Capabilities(_) => "capabilities",
            Self::Open(_) => "open",
            Self::Close(_) => "close",
            Self::Describe(_) => "describe",
            Self::Drawing(_) => "drawing",
            Self::Entities(_) => "entities",
            Self::Apply(_) => "apply",
            Self::Tables(_) => "tables",
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
            "drawing" => Self::Drawing(DrawingRequest::read_fields(&mut object)?),
            "entities" => Self::Entities(EntitiesRequest::read_fields(&mut object)?),
            "apply" => Self::Apply(ApplyRequest::read_fields(&mut object)?),
            "tables" => Self::Tables(TablesRequest::read_fields(&mut object)?),
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
            Self::Drawing(value) => value.to_json(out),
            Self::Entities(value) => value.to_json(out),
            Self::Apply(value) => value.to_json(out),
            Self::Tables(value) => value.to_json(out),
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

/// コアの版と、使える op・形式を尋ねる
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

/// コアの版と、使える op・形式（決定 0015）
#[derive(Clone, Debug, PartialEq)]
pub struct CapabilitiesResult {
    /// パッケージの版（semver）
    pub package: String,
    /// ABI の大版。zm_abi_version と同じ
    pub abi: u64,
    /// メッセージのスキーマの版
    pub schema: SchemaVersion,
    /// このコア（wasm ではこのモジュール）で使える op の名前
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

/// 開いた文書を図面モデル（L2、決定 0022）に変換し、その要約を返す。ネイティブと wasm の出力の一致を、適合試験で比べるためにも使う（決定 0007）
#[derive(Clone, Debug, PartialEq)]
pub struct DrawingRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
    /// 文書の名前。open で作ったもの
    pub doc: String,
}

impl DrawingRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "drawing")?;
        let doc = object.required("doc", json::string)?;
        Ok(Self {
            request_common,
            doc,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("drawing");
        out.key("doc");
        out.string(&self.doc);
    }
}

impl FromJson for DrawingRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DrawingRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 図面モデルの要約。移さなかった図形と値は、診断で返す
#[derive(Clone, Debug, PartialEq)]
pub struct DrawingResult {
    /// 文書のハッシュ。open に渡したバイト列から作る。変更の一覧の前提条件に使う（v1: と 32 桁の 16 進、決定 0156）
    pub hash: Option<String>,
    /// 実寸の空間。モデルの順
    pub spaces: Vec<SpaceSummary>,
    /// 用紙。モデルの順
    pub sheets: Vec<SheetSummary>,
}

impl DrawingResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let hash = object.optional("hash", json::string)?;
        let spaces = object.required("spaces", |value| {
            json::array(value, SpaceSummary::from_json)
        })?;
        let sheets = object.required("sheets", |value| {
            json::array(value, SheetSummary::from_json)
        })?;
        Ok(Self {
            hash,
            spaces,
            sheets,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.hash {
            out.key("hash");
            out.string(value);
        }
        out.key("spaces");
        out.array(&self.spaces, |out, item| item.to_json(out));
        out.key("sheets");
        out.array(&self.sheets, |out, item| item.to_json(out));
    }
}

impl FromJson for DrawingResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DrawingResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 実寸の空間の要約
#[derive(Clone, Debug, PartialEq)]
pub struct SpaceSummary {
    /// 空間の ID（space/0）。entities の要素の ID の始まりと同じ（決定 0156）
    pub id: Option<String>,
    /// 文書の単位
    pub unit: Unit,
    /// 要素の数
    pub elements: ElementCounts,
    /// 線分の端点と文字の基点を囲む範囲。文書の単位。要素がなければ持たない
    pub extent: Option<Extent>,
}

impl SpaceSummary {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.optional("id", json::string)?;
        let unit = object.required("unit", Unit::from_json)?;
        let elements = object.required("elements", ElementCounts::from_json)?;
        let extent = object.optional("extent", Extent::from_json)?;
        Ok(Self {
            id,
            unit,
            elements,
            extent,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.id {
            out.key("id");
            out.string(value);
        }
        out.key("unit");
        self.unit.to_json(out);
        out.key("elements");
        self.elements.to_json(out);
        if let Some(value) = &self.extent {
            out.key("extent");
            value.to_json(out);
        }
    }
}

impl FromJson for SpaceSummary {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SpaceSummary {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 用紙の要約
#[derive(Clone, Debug, PartialEq)]
pub struct SheetSummary {
    /// 用紙の ID。LAYOUT がハンドルを持てば sheet/h22、なければ読んだ順の番号（sheet/0）。entities の要素の ID の始まりと同じ（決定 0156）
    pub id: Option<String>,
    /// 元の形式の用紙の名前（DXF・DWG のレイアウトの名前）。翻訳しない。なければ持たない
    pub name: Option<String>,
    /// 用紙の幅と高さ（mm）。分からなければ持たない
    pub size: Option<Vec<f64>>,
    /// 要素の数
    pub elements: ElementCounts,
    /// ビューの数
    pub views: u64,
    /// 線分の端点と文字の基点を囲む範囲。用紙の mm。要素がなければ持たない
    pub extent: Option<Extent>,
}

impl SheetSummary {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.optional("id", json::string)?;
        let name = object.optional("name", json::string)?;
        let size = object.optional("size", |value| json::array(value, json::number))?;
        let elements = object.required("elements", ElementCounts::from_json)?;
        let views = object.required("views", |value| json::uint(value, 0, 9007199254740991))?;
        let extent = object.optional("extent", Extent::from_json)?;
        Ok(Self {
            id,
            name,
            size,
            elements,
            views,
            extent,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.id {
            out.key("id");
            out.string(value);
        }
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.size {
            out.key("size");
            out.array(value, |out, item| out.float(*item));
        }
        out.key("elements");
        self.elements.to_json(out);
        out.key("views");
        out.uint(self.views);
        if let Some(value) = &self.extent {
            out.key("extent");
            value.to_json(out);
        }
    }
}

impl FromJson for SheetSummary {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SheetSummary {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 実寸の単位
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Unit {
    /// `millimeter`
    Millimeter,
    /// `centimeter`
    Centimeter,
    /// `meter`
    Meter,
    /// `inch`
    Inch,
    /// `foot`
    Foot,
    /// `unknown`
    Unknown,
}

impl Unit {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Millimeter,
        Self::Centimeter,
        Self::Meter,
        Self::Inch,
        Self::Foot,
        Self::Unknown,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Millimeter => "millimeter",
            Self::Centimeter => "centimeter",
            Self::Meter => "meter",
            Self::Inch => "inch",
            Self::Foot => "foot",
            Self::Unknown => "unknown",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "millimeter" => Some(Self::Millimeter),
            "centimeter" => Some(Self::Centimeter),
            "meter" => Some(Self::Meter),
            "inch" => Some(Self::Inch),
            "foot" => Some(Self::Foot),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

impl FromJson for Unit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for Unit {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 種類ごとの要素の数
#[derive(Clone, Debug, PartialEq)]
pub struct ElementCounts {
    /// 線分
    pub lines: u64,
    /// 文字
    pub texts: u64,
}

impl ElementCounts {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let lines = object.required("lines", |value| json::uint(value, 0, 9007199254740991))?;
        let texts = object.required("texts", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self { lines, texts })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("lines");
        out.uint(self.lines);
        out.key("texts");
        out.uint(self.texts);
    }
}

impl FromJson for ElementCounts {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ElementCounts {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 点を囲む、軸に沿った長方形
#[derive(Clone, Debug, PartialEq)]
pub struct Extent {
    /// X の最小
    pub min_x: f64,
    /// Y の最小
    pub min_y: f64,
    /// X の最大
    pub max_x: f64,
    /// Y の最大
    pub max_y: f64,
}

impl Extent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let min_x = object.required("min_x", json::number)?;
        let min_y = object.required("min_y", json::number)?;
        let max_x = object.required("max_x", json::number)?;
        let max_y = object.required("max_y", json::number)?;
        Ok(Self {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("min_x");
        out.float(self.min_x);
        out.key("min_y");
        out.float(self.min_y);
        out.key("max_x");
        out.float(self.max_x);
        out.key("max_y");
        out.float(self.max_y);
    }
}

impl FromJson for Extent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Extent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開いた文書を図面モデル（L2、決定 0022）に変換し、要素の一覧を返す。実寸の空間、用紙の順に、それぞれの要素を入力の順に並べる（決定 0117）
#[derive(Clone, Debug, PartialEq)]
pub struct EntitiesRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
    /// 文書の名前。open で作ったもの
    pub doc: String,
    /// 返す最初の要素の位置。前の応答の next_offset を渡す。書かなければ 0
    pub offset: Option<u64>,
    /// 返す要素の数の上限。書かなければ 1000
    pub limit: Option<u64>,
    /// 返す欄の名前。1 つ以上で、重ねない。id は書かなくても返す。書かなければ id・kind・layer・source を返す（決定 0118）
    pub fields: Option<Vec<EntityField>>,
    /// [最小の X, 最小の Y, 最大の X, 最大の Y]。範囲がこの長方形と重なるか接する要素だけを返す。空間か用紙それぞれの座標で比べる（決定 0118）
    pub region: Option<Vec<f64>>,
}

impl EntitiesRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "entities")?;
        let doc = object.required("doc", json::string)?;
        let offset = object.optional("offset", |value| json::uint(value, 0, 9007199254740991))?;
        let limit = object.optional("limit", |value| json::uint(value, 1, 10000))?;
        let fields =
            object.optional("fields", |value| json::array(value, EntityField::from_json))?;
        let region = object.optional("region", |value| json::array(value, json::number))?;
        Ok(Self {
            request_common,
            doc,
            offset,
            limit,
            fields,
            region,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("entities");
        out.key("doc");
        out.string(&self.doc);
        if let Some(value) = &self.offset {
            out.key("offset");
            out.uint(*value);
        }
        if let Some(value) = &self.limit {
            out.key("limit");
            out.uint(*value);
        }
        if let Some(value) = &self.fields {
            out.key("fields");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.region {
            out.key("region");
            out.array(value, |out, item| out.float(*item));
        }
    }
}

impl FromJson for EntitiesRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for EntitiesRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 要素の一覧の一部。移さなかった図形と値は、診断で返す
#[derive(Clone, Debug, PartialEq)]
pub struct EntitiesResult {
    /// offset から limit 個までの要素
    pub items: Vec<Entity>,
    /// 全体の要素の数
    pub total: u64,
    /// 続きの最初の要素の位置。続きがなければ持たない
    pub next_offset: Option<u64>,
}

impl EntitiesResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let items = object.required("items", |value| json::array(value, Entity::from_json))?;
        let total = object.required("total", |value| json::uint(value, 0, 9007199254740991))?;
        let next_offset = object.optional("next_offset", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            items,
            total,
            next_offset,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("items");
        out.array(&self.items, |out, item| item.to_json(out));
        out.key("total");
        out.uint(self.total);
        if let Some(value) = &self.next_offset {
            out.key("next_offset");
            out.uint(*value);
        }
    }
}

impl FromJson for EntitiesResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for EntitiesResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 図面モデルの要素
#[derive(Clone, Debug, PartialEq)]
pub struct Entity {
    /// 空間か用紙と、出所で作る ID（space/0/h2F、space/0/r12、sheet/1/b4096 など）。出所の部分は、形式が持つ鍵（ハンドル）があれば鍵で作り、書き戻しても前の要素が増えても変わらない。同じバイト列には、どの版でも同じ ID を返す（決定 0117・0156）
    pub id: String,
    /// 形の種類
    pub kind: Option<EntityKind>,
    /// レイヤの元の名前。翻訳しない。図面モデルにレイヤがなければ持たない
    pub layer: Option<String>,
    /// 元の形式のレコード
    pub source: Option<Source>,
    /// 形の定義から求めた範囲。文字の字形・線の太さ・記号の中身は含めない。囲む点がなければ持たない（決定 0118）
    pub bbox: Option<Extent>,
    /// 要素のレイヤ・スタイル・形のハッシュ。出所の位置は含めない。変更の前提条件に使う（v1: と 32 桁の 16 進、決定 0156）
    pub hash: Option<String>,
}

impl Entity {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let kind = object.optional("kind", EntityKind::from_json)?;
        let layer = object.optional("layer", json::string)?;
        let source = object.optional("source", Source::from_json)?;
        let bbox = object.optional("bbox", Extent::from_json)?;
        let hash = object.optional("hash", json::string)?;
        Ok(Self {
            id,
            kind,
            layer,
            source,
            bbox,
            hash,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        if let Some(value) = &self.kind {
            out.key("kind");
            value.to_json(out);
        }
        if let Some(value) = &self.layer {
            out.key("layer");
            out.string(value);
        }
        if let Some(value) = &self.source {
            out.key("source");
            value.to_json(out);
        }
        if let Some(value) = &self.bbox {
            out.key("bbox");
            value.to_json(out);
        }
        if let Some(value) = &self.hash {
            out.key("hash");
            out.string(value);
        }
    }
}

impl FromJson for Entity {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Entity {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// entities で返す要素の欄（決定 0118）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntityField {
    /// `id`
    Id,
    /// `kind`
    Kind,
    /// `layer`
    Layer,
    /// `source`
    Source,
    /// `bbox`
    Bbox,
    /// `hash`
    Hash,
}

impl EntityField {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Id,
        Self::Kind,
        Self::Layer,
        Self::Source,
        Self::Bbox,
        Self::Hash,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Kind => "kind",
            Self::Layer => "layer",
            Self::Source => "source",
            Self::Bbox => "bbox",
            Self::Hash => "hash",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "id" => Some(Self::Id),
            "kind" => Some(Self::Kind),
            "layer" => Some(Self::Layer),
            "source" => Some(Self::Source),
            "bbox" => Some(Self::Bbox),
            "hash" => Some(Self::Hash),
            _ => None,
        }
    }
}

impl FromJson for EntityField {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for EntityField {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 要素の形の種類（決定 0022）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntityKind {
    /// `line`
    Line,
    /// `circle`
    Circle,
    /// `curve`
    Curve,
    /// `polyline`
    Polyline,
    /// `text`
    Text,
    /// `formatted_text`
    FormattedText,
    /// `dimension`
    Dimension,
    /// `leader`
    Leader,
    /// `callout`
    Callout,
    /// `table`
    Table,
    /// `underlay`
    Underlay,
    /// `symbol`
    Symbol,
    /// `hatch`
    Hatch,
}

impl EntityKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Line,
        Self::Circle,
        Self::Curve,
        Self::Polyline,
        Self::Text,
        Self::FormattedText,
        Self::Dimension,
        Self::Leader,
        Self::Callout,
        Self::Table,
        Self::Underlay,
        Self::Symbol,
        Self::Hatch,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Line => "line",
            Self::Circle => "circle",
            Self::Curve => "curve",
            Self::Polyline => "polyline",
            Self::Text => "text",
            Self::FormattedText => "formatted_text",
            Self::Dimension => "dimension",
            Self::Leader => "leader",
            Self::Callout => "callout",
            Self::Table => "table",
            Self::Underlay => "underlay",
            Self::Symbol => "symbol",
            Self::Hatch => "hatch",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "line" => Some(Self::Line),
            "circle" => Some(Self::Circle),
            "curve" => Some(Self::Curve),
            "polyline" => Some(Self::Polyline),
            "text" => Some(Self::Text),
            "formatted_text" => Some(Self::FormattedText),
            "dimension" => Some(Self::Dimension),
            "leader" => Some(Self::Leader),
            "callout" => Some(Self::Callout),
            "table" => Some(Self::Table),
            "underlay" => Some(Self::Underlay),
            "symbol" => Some(Self::Symbol),
            "hatch" => Some(Self::Hatch),
            _ => None,
        }
    }
}

impl FromJson for EntityKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for EntityKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 開いた文書に、変更の一覧を当てて書き戻す（決定 0139・0156）。要素は、entities が返す ID とハッシュで指す。どれか 1 つでも当てられなければ、何も変えない。当てたら、文書を書き戻したものに替え、書き戻したファイルを blobs で返す。小版 7 から
#[derive(Clone, Debug, PartialEq)]
pub struct ApplyRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
    /// 文書の名前。open で作ったもの
    pub doc: String,
    /// 読んだときの文書のハッシュ（drawing の hash）。書けば、今の文書と同じときだけ当てる
    pub hash: Option<String>,
    /// 変更。並べた順に当てる
    pub edits: Vec<Edit>,
}

impl ApplyRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "apply")?;
        let doc = object.required("doc", json::string)?;
        let hash = object.optional("hash", json::string)?;
        let edits = object.required("edits", |value| json::array(value, Edit::from_json))?;
        Ok(Self {
            request_common,
            doc,
            hash,
            edits,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("apply");
        out.key("doc");
        out.string(&self.doc);
        if let Some(value) = &self.hash {
            out.key("hash");
            out.string(value);
        }
        out.key("edits");
        out.array(&self.edits, |out, item| item.to_json(out));
    }
}

impl FromJson for ApplyRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ApplyRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 当てた変更の結果。書き戻したファイルは blobs の file で返す
#[derive(Clone, Debug, PartialEq)]
pub struct ApplyResult {
    /// 変更ごとの結果。要求の edits と同じ順
    pub applied: Vec<Applied>,
    /// 書き戻した文書のハッシュ。次の apply の前提条件に使う
    pub hash: String,
}

impl ApplyResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let applied = object.required("applied", |value| json::array(value, Applied::from_json))?;
        let hash = object.required("hash", json::string)?;
        Ok(Self { applied, hash })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("applied");
        out.array(&self.applied, |out, item| item.to_json(out));
        out.key("hash");
        out.string(&self.hash);
    }
}

impl FromJson for ApplyResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ApplyResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 当てた変更 1 つの結果
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Applied {
    /// 変えた要素の、書き戻した後の ID。除いたときと、ID を決められないときは持たない
    pub id: Option<String>,
    /// 変えた要素の、書き戻した後のハッシュ。id があるときに持つ
    pub hash: Option<String>,
}

impl Applied {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.optional("id", json::string)?;
        let hash = object.optional("hash", json::string)?;
        Ok(Self { id, hash })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.id {
            out.key("id");
            out.string(value);
        }
        if let Some(value) = &self.hash {
            out.key("hash");
            out.string(value);
        }
    }
}

impl FromJson for Applied {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Applied {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 要素 1 つへの変更。kind の値で形が決まる
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    /// 要素のレイヤ・色・線種・線種の倍率・線幅のうち、書いたものだけを変える。形は変えない
    Restyle(RestyleEdit),
    /// 要素を、アフィン変換で動かす。AutoCAD の系統では、図形とともに動く XDATA と記号の属性も同じ変換で直す。変換した形を元の種類で表せなければ当てない
    Transform(TransformEdit),
    /// 要素を除く
    Remove(RemoveEdit),
}

impl Edit {
    /// 目印の欄（`kind`）の値（スキーマの順）
    pub const TAGS: &[&str] = &["restyle", "transform", "remove"];

    /// 目印の欄（`kind`）の値
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Restyle(_) => "restyle",
            Self::Transform(_) => "transform",
            Self::Remove(_) => "remove",
        }
    }
}

impl FromJson for Edit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = match object.tag("kind")? {
            "restyle" => Self::Restyle(RestyleEdit::read_fields(&mut object)?),
            "transform" => Self::Transform(TransformEdit::read_fields(&mut object)?),
            "remove" => Self::Remove(RemoveEdit::read_fields(&mut object)?),
            other => return Err(Invalid::new(Problem::Tag(other.to_string())).in_key("kind")),
        };
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Edit {
    fn to_json(&self, out: &mut Writer) {
        match self {
            Self::Restyle(value) => value.to_json(out),
            Self::Transform(value) => value.to_json(out),
            Self::Remove(value) => value.to_json(out),
        }
    }
}

/// 要素のレイヤ・色・線種・線種の倍率・線幅のうち、書いたものだけを変える。形は変えない
#[derive(Clone, Debug, PartialEq)]
pub struct RestyleEdit {
    /// 変える要素の ID。entities が返したもの
    pub id: String,
    /// 読んだときの要素のハッシュ。entities が返したもの
    pub hash: String,
    /// レイヤの元の名前。図面にあるレイヤで、名前が 1 つに決まるもの。layer_id と一緒には書かない
    pub layer: Option<String>,
    /// レイヤの ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。小版 8 から
    pub layer_id: Option<String>,
    /// 色
    pub color: Option<ColorChange>,
    /// 線種
    pub line_type: Option<LineTypeChange>,
    /// 要素の線種の倍率
    pub line_type_scale: Option<f64>,
    /// 線幅
    pub line_weight: Option<LineWeightChange>,
}

impl RestyleEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "restyle")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        let layer = object.optional("layer", json::string)?;
        let layer_id = object.optional("layer_id", json::string)?;
        let color = object.optional("color", ColorChange::from_json)?;
        let line_type = object.optional("line_type", LineTypeChange::from_json)?;
        let line_type_scale = object.optional("line_type_scale", json::number)?;
        let line_weight = object.optional("line_weight", LineWeightChange::from_json)?;
        Ok(Self {
            id,
            hash,
            layer,
            layer_id,
            color,
            line_type,
            line_type_scale,
            line_weight,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("restyle");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
        if let Some(value) = &self.layer {
            out.key("layer");
            out.string(value);
        }
        if let Some(value) = &self.layer_id {
            out.key("layer_id");
            out.string(value);
        }
        if let Some(value) = &self.color {
            out.key("color");
            value.to_json(out);
        }
        if let Some(value) = &self.line_type {
            out.key("line_type");
            value.to_json(out);
        }
        if let Some(value) = &self.line_type_scale {
            out.key("line_type_scale");
            out.float(*value);
        }
        if let Some(value) = &self.line_weight {
            out.key("line_weight");
            value.to_json(out);
        }
    }
}

impl FromJson for RestyleEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RestyleEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 要素を、アフィン変換で動かす。AutoCAD の系統では、図形とともに動く XDATA と記号の属性も同じ変換で直す。変換した形を元の種類で表せなければ当てない
#[derive(Clone, Debug, PartialEq)]
pub struct TransformEdit {
    /// 動かす要素の ID。entities が返したもの
    pub id: String,
    /// 読んだときの要素のハッシュ。entities が返したもの
    pub hash: String,
    /// [a, b, c, d, e, f]。要素の空間か用紙の点 (x, y) を (a x + b y + c, d x + e y + f) に写す
    pub matrix: Vec<f64>,
}

impl TransformEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "transform")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        let matrix = object.required("matrix", |value| json::array(value, json::number))?;
        Ok(Self { id, hash, matrix })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("transform");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
        out.key("matrix");
        out.array(&self.matrix, |out, item| out.float(*item));
    }
}

impl FromJson for TransformEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for TransformEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 要素を除く
#[derive(Clone, Debug, PartialEq)]
pub struct RemoveEdit {
    /// 除く要素の ID。entities が返したもの
    pub id: String,
    /// 読んだときの要素のハッシュ。entities が返したもの
    pub hash: String,
}

impl RemoveEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "remove")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        Ok(Self { id, hash })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("remove");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
    }
}

impl FromJson for RemoveEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RemoveEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 値を継承する先。layer はレイヤから、block は要素を置いた記号の配置から
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Inherit {
    /// `layer`
    Layer,
    /// `block`
    Block,
}

impl Inherit {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Layer, Self::Block];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Layer => "layer",
            Self::Block => "block",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "layer" => Some(Self::Layer),
            "block" => Some(Self::Block),
            _ => None,
        }
    }
}

impl FromJson for Inherit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for Inherit {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 新しい色。inherit か、aci と rgb の少なくとも一方のどちらかを書く
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColorChange {
    /// 継承する先
    pub inherit: Option<Inherit>,
    /// AutoCAD の色の番号（1〜255）
    pub aci: Option<u64>,
    /// [赤, 緑, 青]。それぞれ 0〜255
    pub rgb: Option<Vec<u64>>,
}

impl ColorChange {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let inherit = object.optional("inherit", Inherit::from_json)?;
        let aci = object.optional("aci", |value| json::uint(value, 1, 255))?;
        let rgb = object.optional("rgb", |value| {
            json::array(value, |value| json::uint(value, 0, 255))
        })?;
        Ok(Self { inherit, aci, rgb })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.inherit {
            out.key("inherit");
            value.to_json(out);
        }
        if let Some(value) = &self.aci {
            out.key("aci");
            out.uint(*value);
        }
        if let Some(value) = &self.rgb {
            out.key("rgb");
            out.array(value, |out, item| out.uint(*item));
        }
    }
}

impl FromJson for ColorChange {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ColorChange {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 新しい線種。inherit・name・id のどれか 1 つを書く
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LineTypeChange {
    /// 継承する先
    pub inherit: Option<Inherit>,
    /// 線種の元の名前。図面にある線種で、名前が 1 つに決まるもの
    pub name: Option<String>,
    /// 線種の ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。小版 8 から
    pub id: Option<String>,
}

impl LineTypeChange {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let inherit = object.optional("inherit", Inherit::from_json)?;
        let name = object.optional("name", json::string)?;
        let id = object.optional("id", json::string)?;
        Ok(Self { inherit, name, id })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.inherit {
            out.key("inherit");
            value.to_json(out);
        }
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.id {
            out.key("id");
            out.string(value);
        }
    }
}

impl FromJson for LineTypeChange {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LineTypeChange {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 新しい線幅。inherit・mm・default のどれか 1 つを書く
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LineWeightChange {
    /// 継承する先
    pub inherit: Option<Inherit>,
    /// 用紙の上の幅（mm）。0 以上
    pub mm: Option<f64>,
    /// true なら、文書の既定の線幅
    pub default: Option<bool>,
}

impl LineWeightChange {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let inherit = object.optional("inherit", Inherit::from_json)?;
        let mm = object.optional("mm", json::number)?;
        let default = object.optional("default", json::boolean)?;
        Ok(Self {
            inherit,
            mm,
            default,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.inherit {
            out.key("inherit");
            value.to_json(out);
        }
        if let Some(value) = &self.mm {
            out.key("mm");
            out.float(*value);
        }
        if let Some(value) = &self.default {
            out.key("default");
            out.bool(*value);
        }
    }
}

impl FromJson for LineWeightChange {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LineWeightChange {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開いた文書を図面モデル（L2）に変換し、レイヤと線種の表の項目を返す（決定 0156）。小版 8 から
#[derive(Clone, Debug, PartialEq)]
pub struct TablesRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
    /// 文書の名前。open で作ったもの
    pub doc: String,
}

impl TablesRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "tables")?;
        let doc = object.required("doc", json::string)?;
        Ok(Self {
            request_common,
            doc,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("tables");
        out.key("doc");
        out.string(&self.doc);
    }
}

impl FromJson for TablesRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for TablesRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 表の項目。図面モデルの順
#[derive(Clone, Debug, PartialEq)]
pub struct TablesResult {
    /// レイヤ
    pub layers: Vec<TableItem>,
    /// 線種
    pub line_types: Vec<TableItem>,
    /// 寸法スタイル
    pub dimension_styles: Vec<TableItem>,
}

impl TablesResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let layers = object.required("layers", |value| json::array(value, TableItem::from_json))?;
        let line_types = object.required("line_types", |value| {
            json::array(value, TableItem::from_json)
        })?;
        let dimension_styles = object.required("dimension_styles", |value| {
            json::array(value, TableItem::from_json)
        })?;
        Ok(Self {
            layers,
            line_types,
            dimension_styles,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("layers");
        out.array(&self.layers, |out, item| item.to_json(out));
        out.key("line_types");
        out.array(&self.line_types, |out, item| item.to_json(out));
        out.key("dimension_styles");
        out.array(&self.dimension_styles, |out, item| item.to_json(out));
    }
}

impl FromJson for TablesResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for TablesResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 表の項目
#[derive(Clone, Debug, PartialEq)]
pub struct TableItem {
    /// ID。表のレコードがハンドルを持てば layer/h10・line-type/h14、なければ読んだ順の番号（layer/2）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
    pub id: String,
    /// 元の名前。翻訳しない
    pub name: String,
    /// 項目のハッシュ（v1: と 32 桁の 16 進、決定 0156）
    pub hash: String,
}

impl TableItem {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let name = object.required("name", json::string)?;
        let hash = object.required("hash", json::string)?;
        Ok(Self { id, name, hash })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("name");
        out.string(&self.name);
        out.key("hash");
        out.string(&self.hash);
    }
}

impl FromJson for TableItem {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for TableItem {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 要素の出所。L1 のレコードの番号かバイト範囲（決定 0008）と、形式が持つ鍵（決定 0156）。ファイルから読んでいない要素は、レコードの番号もバイト範囲も持たない
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Source {
    /// L1 のレコードの番号
    pub record: Option<u64>,
    /// L1 のバイト範囲
    pub bytes: Option<ByteRange>,
    /// AutoCAD の系統のハンドル。大文字の 16 進。書き戻しても変わらない
    pub handle: Option<String>,
    /// SXF のインスタンスの番号（#n の n）。10 進の文字列。小版 6 から
    pub instance: Option<String>,
}

impl Source {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let record = object.optional("record", |value| json::uint(value, 0, 9007199254740991))?;
        let bytes = object.optional("bytes", ByteRange::from_json)?;
        let handle = object.optional("handle", json::string)?;
        let instance = object.optional("instance", json::string)?;
        Ok(Self {
            record,
            bytes,
            handle,
            instance,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.record {
            out.key("record");
            out.uint(*value);
        }
        if let Some(value) = &self.bytes {
            out.key("bytes");
            value.to_json(out);
        }
        if let Some(value) = &self.handle {
            out.key("handle");
            out.string(value);
        }
        if let Some(value) = &self.instance {
            out.key("instance");
            out.string(value);
        }
    }
}

impl FromJson for Source {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Source {
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

/// 実行の形（決定 0013）。`native` はプロセスの中でコアを動かす、`process` は子プロセスの CLI に送る、`wasm` は wasm のモジュールで動かす
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
    /// パッケージの版。コアと同じ版で出す
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
