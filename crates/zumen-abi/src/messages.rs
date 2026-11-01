// 生成したファイル。手で直さない。
// 元は schema/messages.json で、`cargo xtask codegen` で作り直す。

//! 境界のメッセージの型（決定 0012）。スキーマの定義ごとに 1 つの型がある。

use std::collections::BTreeMap;

use zumen_json::{self as json, FromJson, Invalid, ObjectReader, Problem, ToJson, Value, Writer};

/// 要求。op の値で形が決まる
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
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
    /// 開いた文書の図面モデルを、別の形式の新しいファイルに書き、新しい文書として開く（決定 0139・0206）。書けない要素は落とし、種類ごとの件数を診断で返す。書いたファイルは blobs で返す。今は DXF にだけ書ける。小版 41 から
    Convert(ConvertRequest),
    /// 開いた文書を図面モデル（L2）に変換し、レイヤと線種の表の項目を返す（決定 0156）。小版 8 から
    Tables(TablesRequest),
    /// 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、2D のシーンを blob で返す（決定 0172）。シーンの配置は schema/scene.toml にある。小版 33 から
    Scene(SceneRequest),
    /// 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、プロファイルの規則と語彙で照合する（決定 0138・0173・0179）。小版 34 から
    Check(CheckRequest),
    /// zm_push で渡した IFC（IFC-SPF か ifcZIP）を、zm_push で渡した EXPRESS のスキーマに照らしてチェックする（決定 0207・0216）。インスタンスの実体の名前・抽象・属性の数・値の型・省略を見る。WHERE・UNIQUE・RULE の規則は評価せず、数を返す。zumen は EXPRESS を同梱しない。小版 42 から
    Validate(ValidateRequest),
    /// レビューの文書（決定 0214・0218）の 1 つの対象を、開いた図面の文書の版へ移す。要素の参照を図面の要素の ID で引き、参照ごとに unchanged・changed・removed を返す。場所と図形に元の版のハッシュと状態を書き、対象のハッシュを文書のハッシュにする。2D の点は動かさない。移した文書は blobs の file で返す。小版 43 から
    ReviewCarry(ReviewCarryRequest),
    /// 開いた文書を check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0214・0218）に足す。指摘の ID は対象のハッシュ・規則・診断から決め、取り込み済みの指摘は足さない。足した文書は blobs の file で返す。小版 43 から
    ReviewAdopt(ReviewAdoptRequest),
    /// 開いた IFC の建物モデル（決定 0207・0219）の要約を返す。小版 44 から
    Building(BuildingRequest),
    /// 開いた IFC の建物モデルの要素を、書いた順に返す。成分は components で選ぶ（決定 0207・0219）。小版 44 から
    BuildingElements(BuildingElementsRequest),
    /// 開いた IFC の建物モデルを、zm_push で渡した IDS で照合する（決定 0217・0219）。確かめられない facet は、指摘と分けて info の指摘で返す。小版 44 から
    IdsCheck(IdsCheckRequest),
    /// 開いた IFC を ids_check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0218・0219）に足す。足した文書は blobs の file で返す。小版 44 から
    ReviewAdoptIds(ReviewAdoptIdsRequest),
    /// zm_push で渡した BCF 3.0 の bcfzip を読み、開いた IFC を対象にしたレビューの文書（決定 0214・0220）にする。topic は指摘に、comment はコメントに、viewpoint は 3D の場所に写し、要素の参照のハッシュは建物モデルの要素のハッシュで埋める。文書は blobs の file で返す。小版 45 から
    BcfRead(BcfReadRequest),
    /// zm_push で渡したレビューの文書を、BCF 3.0 の bcfzip に書く（決定 0214・0220）。target を対象にした 3D の場所を viewpoint にし、2D の場所と図形は書かずに診断する。original を渡せば、変えていない topic を元のバイト列のまま書き、変えた topic でも写さない要素と知らない要素を保つ。bcfzip は blobs の file で返す。小版 45 から
    BcfWrite(BcfWriteRequest),
    /// 線形（決定 0230）の上の所を、追加距離か測点の表記から求め、点・方向角・高さ・勾配を返す。線形は、要求に書く（alignment）か、open で開いた J-LandXML の文書（doc）の線形を id で指す。distances・stations・interval の所を、この順に並べて返す。求められない所は、理由を診断して除く。doc を渡せば、文書の線形と地表面の要約も返す。図面のモジュールに置く。小版 46 から（doc と id は小版 47 から）
    Civil(CivilRequest),
    /// open で開いた点群（LAS・LAZ）の要約と、条件に合う点を返す。点は八分木の浅い段の節から順に選ぶ（決定 0232）。図面のモジュールに置く。小版 48 から
    Pointcloud(PointcloudRequest),
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
            Self::Convert(_) => "convert",
            Self::Tables(_) => "tables",
            Self::Scene(_) => "scene",
            Self::Check(_) => "check",
            Self::Validate(_) => "validate",
            Self::ReviewCarry(_) => "review_carry",
            Self::ReviewAdopt(_) => "review_adopt",
            Self::Building(_) => "building",
            Self::BuildingElements(_) => "building_elements",
            Self::IdsCheck(_) => "ids_check",
            Self::ReviewAdoptIds(_) => "review_adopt_ids",
            Self::BcfRead(_) => "bcf_read",
            Self::BcfWrite(_) => "bcf_write",
            Self::Civil(_) => "civil",
            Self::Pointcloud(_) => "pointcloud",
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
            "convert" => Self::Convert(ConvertRequest::read_fields(&mut object)?),
            "tables" => Self::Tables(TablesRequest::read_fields(&mut object)?),
            "scene" => Self::Scene(SceneRequest::read_fields(&mut object)?),
            "check" => Self::Check(CheckRequest::read_fields(&mut object)?),
            "validate" => Self::Validate(ValidateRequest::read_fields(&mut object)?),
            "review_carry" => Self::ReviewCarry(ReviewCarryRequest::read_fields(&mut object)?),
            "review_adopt" => Self::ReviewAdopt(ReviewAdoptRequest::read_fields(&mut object)?),
            "building" => Self::Building(BuildingRequest::read_fields(&mut object)?),
            "building_elements" => {
                Self::BuildingElements(BuildingElementsRequest::read_fields(&mut object)?)
            }
            "ids_check" => Self::IdsCheck(IdsCheckRequest::read_fields(&mut object)?),
            "review_adopt_ids" => {
                Self::ReviewAdoptIds(ReviewAdoptIdsRequest::read_fields(&mut object)?)
            }
            "bcf_read" => Self::BcfRead(BcfReadRequest::read_fields(&mut object)?),
            "bcf_write" => Self::BcfWrite(BcfWriteRequest::read_fields(&mut object)?),
            "civil" => Self::Civil(CivilRequest::read_fields(&mut object)?),
            "pointcloud" => Self::Pointcloud(PointcloudRequest::read_fields(&mut object)?),
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
            Self::Convert(value) => value.to_json(out),
            Self::Tables(value) => value.to_json(out),
            Self::Scene(value) => value.to_json(out),
            Self::Check(value) => value.to_json(out),
            Self::Validate(value) => value.to_json(out),
            Self::ReviewCarry(value) => value.to_json(out),
            Self::ReviewAdopt(value) => value.to_json(out),
            Self::Building(value) => value.to_json(out),
            Self::BuildingElements(value) => value.to_json(out),
            Self::IdsCheck(value) => value.to_json(out),
            Self::ReviewAdoptIds(value) => value.to_json(out),
            Self::BcfRead(value) => value.to_json(out),
            Self::BcfWrite(value) => value.to_json(out),
            Self::Civil(value) => value.to_json(out),
            Self::Pointcloud(value) => value.to_json(out),
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
    /// 入力の全体の大きさ（バイト）。渡した入力より大きければ、入力を全体の初めの部分とみなし、足りない範囲を needs で求める。範囲で読める形式（LAS・LAZ）だけが使う（決定 0232）。小版 48 から
    pub size: Option<u64>,
    /// 見分ける印のない形式（xyz）を名指して読む。書かなければ、入力から判定する（決定 0232）。小版 49 から
    pub format: Option<String>,
}

impl OpenRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "open")?;
        let input = object.required("input", json::string)?;
        let doc = object.optional("doc", json::string)?;
        let size = object.optional("size", |value| json::uint(value, 0, 9007199254740991))?;
        let format = object.optional("format", json::string)?;
        Ok(Self {
            request_common,
            input,
            doc,
            size,
            format,
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
        if let Some(value) = &self.size {
            out.key("size");
            out.uint(*value);
        }
        if let Some(value) = &self.format {
            out.key("format");
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
    /// 読むのに足りない範囲。あれば文書はまだ作らない。範囲をその名前で zm_push してから、同じ要求をもう一度呼ぶ（決定 0232）。小版 48 から
    pub needs: Option<Vec<InputRange>>,
    /// 読み終えたので、close の inputs で捨ててよい範囲の入力。小版 48 から
    pub done: Option<Vec<InputRange>>,
}

impl OpenResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let doc = object.required("doc", json::string)?;
        let format = object.required("format", json::string)?;
        let needs = object.optional("needs", |value| json::array(value, InputRange::from_json))?;
        let done = object.optional("done", |value| json::array(value, InputRange::from_json))?;
        Ok(Self {
            doc,
            format,
            needs,
            done,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("doc");
        out.string(&self.doc);
        out.key("format");
        out.string(&self.format);
        if let Some(value) = &self.needs {
            out.key("needs");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.done {
            out.key("done");
            out.array(value, |out, item| item.to_json(out));
        }
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

/// 入力の範囲と、その範囲を渡す入力の名前
#[derive(Clone, Debug, PartialEq)]
pub struct InputRange {
    /// 範囲を渡す入力の名前。<入力の名前>#<start>-<end>
    pub input: String,
    /// 範囲の初め（バイト）
    pub start: u64,
    /// 範囲の終わり（バイト、含まない）
    pub end: u64,
}

impl InputRange {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let input = object.required("input", json::string)?;
        let start = object.required("start", |value| json::uint(value, 0, 9007199254740991))?;
        let end = object.required("end", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self { input, start, end })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("input");
        out.string(&self.input);
        out.key("start");
        out.uint(self.start);
        out.key("end");
        out.uint(self.end);
    }
}

impl FromJson for InputRange {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for InputRange {
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
    /// 文書のハッシュ。open に渡したバイト列から作る。変更の一覧の前提条件に使う（v2: と 32 桁の 16 進、決定 0156）
    pub hash: Option<String>,
    /// 実寸の空間。モデルの順
    pub spaces: Vec<SpaceSummary>,
    /// 用紙。モデルの順
    pub sheets: Vec<SheetSummary>,
    /// 文書の点の表し方（marker を持たない点の見え方、決定 0160）。point_style の変更で変える。文書が持たなければ（JWW・SXF）持たない。小版 39 から
    pub point_style: Option<PointStyleSummary>,
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
        let point_style = object.optional("point_style", PointStyleSummary::from_json)?;
        Ok(Self {
            hash,
            spaces,
            sheets,
            point_style,
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
        if let Some(value) = &self.point_style {
            out.key("point_style");
            value.to_json(out);
        }
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

/// 文書の点の表し方。欄の意味は point_style_edit と同じ
#[derive(Clone, Debug, PartialEq)]
pub struct PointStyleSummary {
    /// 基本の形
    pub mark: PointMark,
    /// 円で囲む
    pub circle: bool,
    /// 四角で囲む
    pub square: bool,
    /// 大きさ。意味は size_kind で決まる
    pub size: f64,
    /// 大きさの意味
    pub size_kind: PointSizeKind,
}

impl PointStyleSummary {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let mark = object.required("mark", PointMark::from_json)?;
        let circle = object.required("circle", json::boolean)?;
        let square = object.required("square", json::boolean)?;
        let size = object.required("size", json::number)?;
        let size_kind = object.required("size_kind", PointSizeKind::from_json)?;
        Ok(Self {
            mark,
            circle,
            square,
            size,
            size_kind,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("mark");
        self.mark.to_json(out);
        out.key("circle");
        out.bool(self.circle);
        out.key("square");
        out.bool(self.square);
        out.key("size");
        out.float(self.size);
        out.key("size_kind");
        self.size_kind.to_json(out);
    }
}

impl FromJson for PointStyleSummary {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PointStyleSummary {
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
    /// 空間か用紙の ID（space/0・sheet/h22 など）。書けば、その空間か用紙の要素だけを返す。記号の定義の空間（space/h1F など。BLOCK のハンドルから作る、決定 0156）も指せ、書かなければ定義の中の要素は返さない。ない ID なら要素を返さない。小版 31 から（決定 0177）
    pub place: Option<String>,
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
        let place = object.optional("place", json::string)?;
        Ok(Self {
            request_common,
            doc,
            offset,
            limit,
            fields,
            region,
            place,
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
        if let Some(value) = &self.place {
            out.key("place");
            out.string(value);
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
    /// 要素のレイヤ・スタイル・形のハッシュ。出所の位置は含めない。変更の前提条件に使う（v2: と 32 桁の 16 進、決定 0156）
    pub hash: Option<String>,
    /// 形の値。線分・円・円弧・楕円・幅を持たない折れ線・1 点か 2 点の間に置く文字・書式つきの文字・ブロックの記号・制御点か当てはめる点で決まるスプライン・寸法・引出線（LEADER）だけが持つ。apply の replace と add に渡せる形（決定 0139）。小版 10 から
    pub geometry: Option<Geometry>,
}

impl Entity {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let kind = object.optional("kind", EntityKind::from_json)?;
        let layer = object.optional("layer", json::string)?;
        let source = object.optional("source", Source::from_json)?;
        let bbox = object.optional("bbox", Extent::from_json)?;
        let hash = object.optional("hash", json::string)?;
        let geometry = object.optional("geometry", Geometry::from_json)?;
        Ok(Self {
            id,
            kind,
            layer,
            source,
            bbox,
            hash,
            geometry,
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
        if let Some(value) = &self.geometry {
            out.key("geometry");
            value.to_json(out);
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
    /// `geometry`
    Geometry,
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
        Self::Geometry,
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
            Self::Geometry => "geometry",
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
            "geometry" => Some(Self::Geometry),
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

/// 形の値。点は要素の空間か用紙の座標、長さは同じ単位（実寸の空間は文書の単位、用紙は mm）、角度は度で反時計回り。shape の値で形が決まる
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Geometry {
    /// 線分
    Line(LineGeometry),
    /// 円
    Circle(CircleGeometry),
    /// 円弧。始まりの角度から終わりの角度へ、反時計回りにたどる
    Arc(ArcGeometry),
    /// 楕円か楕円の弧。始まりの角度から終わりの角度へ、反時計回りにたどる。閉じた楕円は 0 度から 360 度。小版 11 から
    Ellipse(EllipseGeometry),
    /// 頂点を結んだ折れ線。幅は持たない
    Polyline(PolylineGeometry),
    /// 1 点か 2 点の間に置く文字。置き換えるときは、位置合わせ・幅の倍率・斜体・反転・文字スタイルを元の値のままにする。足すときは、2 点目がなければ左の基準線に合わせ、幅の倍率 1・斜体なし・反転なしにする
    Text(TextGeometry),
    /// 書式つきの文字（DXF の MTEXT）。置き換えるときは、枠の高さ・書き進める向き・行送り・背景・高さの意味・文字スタイルを元の値のままにする。足すときは、枠の高さなし・左から右・行送り 1 倍・背景なしにする。小版 13 から
    FormattedText(FormattedTextGeometry),
    /// 記号（DXF・DWG のブロックの INSERT）。定義の点を、尺度・回転・移動の順に写す。切り抜きは形の値に含めない。置き換えるときは切り抜きを元のままにし、属性は値だけを変えられる。属性を持つ記号の置く点・尺度・回転は変えない（属性ごと動かすなら transform を使う）。足すときは切り抜きを持たない。小版 14 から
    Symbol(SymbolGeometry),
    /// スプライン（NURBS）。制御点で決まるもの（小版 15 から）と、当てはめる点で決まるもの（小版 18 から）がある。当てはめる点で決まるものは、節点と制御点を空にし、当てはめる点（fit）と、あれば両端の接線を持つ。元の図形に制御点や節点もあるときは形の値を返さず、置き換えない。置き換えでは、制御点で決まるものと当てはめる点で決まるものを入れ替えない
    Spline(SplineGeometry),
    /// 寸法。種類（kind）ごとに、その種類の定義点の欄を持つ。測定値（measurement）は元の形式が保存した値で、定義点から測り直さない。寸法を描いた部品（ブロック）は形の値に含めない。entities が返すだけで、置き換えと追加は change.edit.unsupported になる（寸法を描いた部品を作り直す必要があるため）。小版 19 から
    Dimension(DimensionGeometry),
    /// 引出線（DXF・DWG の LEADER、決定 0177）。注記（MTEXT など）は別の要素で、annotation がその ID を指す。置き換えるときは寸法スタイルと注記を変えず、経路・曲線か・矢印を書き直す。足すときの注記は、同じ空間か用紙の MTEXT だけを指せる（小版 36 から。それまでは注記を持てなかった）。DWG は置き換えと追加を書き戻すが、矢印の印を必ず持つので、arrow を書かない引出線は書けない（小版 36 から。それまでは R2004 までの置き換えだけ）。小版 31 から
    Leader(LeaderGeometry),
    /// 点（決定 0160・0183）。marker がなければ文書の点の表し方（DXF・DWG の $PDMODE・$PDSIZE）に従う点で、DXF・DWG の POINT と JWW の記号でない点が当たる。marker があれば既定義の印の点で、SFC の点マーカーと JWW の記号の点が当たる。置き換えるときは、描き方（marker があるか）を元のまま保つ。DXF・DWG は marker と construction を持つ点を書かず、JWW の記号でない点は角度 0 だけを、SXF は SFC の点マーカーだけを書く。小版 35 から
    Point(PointGeometry),
    /// 引出線つきの注記（DXF・DWG の MULTILEADER、決定 0185）。注記は形の中にあり、content と text・symbol で種類と中身を示す。置き換えるときは枝の線の頂点・集まる点・折れ曲がりだけを変えられ、ほかの欄は元の値のままにする。DXF だけが置き換えを書き戻し、枝と線の数と、集まる点と折れ曲がりの有無は元と同じでなければならない。足すことと DWG の書き戻しは、まだしない。小版 36 から
    Callout(CalloutGeometry),
}

impl Geometry {
    /// 目印の欄（`shape`）の値（スキーマの順）
    pub const TAGS: &[&str] = &[
        "line",
        "circle",
        "arc",
        "ellipse",
        "polyline",
        "text",
        "formatted_text",
        "symbol",
        "spline",
        "dimension",
        "leader",
        "point",
        "callout",
    ];

    /// 目印の欄（`shape`）の値
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Line(_) => "line",
            Self::Circle(_) => "circle",
            Self::Arc(_) => "arc",
            Self::Ellipse(_) => "ellipse",
            Self::Polyline(_) => "polyline",
            Self::Text(_) => "text",
            Self::FormattedText(_) => "formatted_text",
            Self::Symbol(_) => "symbol",
            Self::Spline(_) => "spline",
            Self::Dimension(_) => "dimension",
            Self::Leader(_) => "leader",
            Self::Point(_) => "point",
            Self::Callout(_) => "callout",
        }
    }
}

impl FromJson for Geometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = match object.tag("shape")? {
            "line" => Self::Line(LineGeometry::read_fields(&mut object)?),
            "circle" => Self::Circle(CircleGeometry::read_fields(&mut object)?),
            "arc" => Self::Arc(ArcGeometry::read_fields(&mut object)?),
            "ellipse" => Self::Ellipse(EllipseGeometry::read_fields(&mut object)?),
            "polyline" => Self::Polyline(PolylineGeometry::read_fields(&mut object)?),
            "text" => Self::Text(TextGeometry::read_fields(&mut object)?),
            "formatted_text" => {
                Self::FormattedText(FormattedTextGeometry::read_fields(&mut object)?)
            }
            "symbol" => Self::Symbol(SymbolGeometry::read_fields(&mut object)?),
            "spline" => Self::Spline(SplineGeometry::read_fields(&mut object)?),
            "dimension" => Self::Dimension(DimensionGeometry::read_fields(&mut object)?),
            "leader" => Self::Leader(LeaderGeometry::read_fields(&mut object)?),
            "point" => Self::Point(PointGeometry::read_fields(&mut object)?),
            "callout" => Self::Callout(CalloutGeometry::read_fields(&mut object)?),
            other => return Err(Invalid::new(Problem::Tag(other.to_string())).in_key("shape")),
        };
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Geometry {
    fn to_json(&self, out: &mut Writer) {
        match self {
            Self::Line(value) => value.to_json(out),
            Self::Circle(value) => value.to_json(out),
            Self::Arc(value) => value.to_json(out),
            Self::Ellipse(value) => value.to_json(out),
            Self::Polyline(value) => value.to_json(out),
            Self::Text(value) => value.to_json(out),
            Self::FormattedText(value) => value.to_json(out),
            Self::Symbol(value) => value.to_json(out),
            Self::Spline(value) => value.to_json(out),
            Self::Dimension(value) => value.to_json(out),
            Self::Leader(value) => value.to_json(out),
            Self::Point(value) => value.to_json(out),
            Self::Callout(value) => value.to_json(out),
        }
    }
}

/// 線分
#[derive(Clone, Debug, PartialEq)]
pub struct LineGeometry {
    /// 始点 [X, Y]
    pub start: Vec<f64>,
    /// 終点 [X, Y]
    pub end: Vec<f64>,
}

impl LineGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "line")?;
        let start = object.required("start", |value| json::array(value, json::number))?;
        let end = object.required("end", |value| json::array(value, json::number))?;
        Ok(Self { start, end })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("line");
        out.key("start");
        out.array(&self.start, |out, item| out.float(*item));
        out.key("end");
        out.array(&self.end, |out, item| out.float(*item));
    }
}

impl FromJson for LineGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LineGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 円
#[derive(Clone, Debug, PartialEq)]
pub struct CircleGeometry {
    /// 中心 [X, Y]
    pub center: Vec<f64>,
    /// 半径。0 より大きい
    pub radius: f64,
}

impl CircleGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "circle")?;
        let center = object.required("center", |value| json::array(value, json::number))?;
        let radius = object.required("radius", json::number)?;
        Ok(Self { center, radius })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("circle");
        out.key("center");
        out.array(&self.center, |out, item| out.float(*item));
        out.key("radius");
        out.float(self.radius);
    }
}

impl FromJson for CircleGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CircleGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 円弧。始まりの角度から終わりの角度へ、反時計回りにたどる
#[derive(Clone, Debug, PartialEq)]
pub struct ArcGeometry {
    /// 中心 [X, Y]
    pub center: Vec<f64>,
    /// 半径。0 より大きい
    pub radius: f64,
    /// 始まりの角度（度）
    pub start_angle: f64,
    /// 終わりの角度（度）
    pub end_angle: f64,
}

impl ArcGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "arc")?;
        let center = object.required("center", |value| json::array(value, json::number))?;
        let radius = object.required("radius", json::number)?;
        let start_angle = object.required("start_angle", json::number)?;
        let end_angle = object.required("end_angle", json::number)?;
        Ok(Self {
            center,
            radius,
            start_angle,
            end_angle,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("arc");
        out.key("center");
        out.array(&self.center, |out, item| out.float(*item));
        out.key("radius");
        out.float(self.radius);
        out.key("start_angle");
        out.float(self.start_angle);
        out.key("end_angle");
        out.float(self.end_angle);
    }
}

impl FromJson for ArcGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ArcGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 楕円か楕円の弧。始まりの角度から終わりの角度へ、反時計回りにたどる。閉じた楕円は 0 度から 360 度。小版 11 から
#[derive(Clone, Debug, PartialEq)]
pub struct EllipseGeometry {
    /// 中心 [X, Y]
    pub center: Vec<f64>,
    /// 長軸の端の点 [X, Y]。中心と違う点
    pub major: Vec<f64>,
    /// 短軸の長軸に対する比。0 より大きく 1 以下
    pub ratio: f64,
    /// 始まりの角度（度）。楕円を円に引き延ばしたときの角度
    pub start_angle: f64,
    /// 終わりの角度（度）。楕円を円に引き延ばしたときの角度
    pub end_angle: f64,
}

impl EllipseGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "ellipse")?;
        let center = object.required("center", |value| json::array(value, json::number))?;
        let major = object.required("major", |value| json::array(value, json::number))?;
        let ratio = object.required("ratio", json::number)?;
        let start_angle = object.required("start_angle", json::number)?;
        let end_angle = object.required("end_angle", json::number)?;
        Ok(Self {
            center,
            major,
            ratio,
            start_angle,
            end_angle,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("ellipse");
        out.key("center");
        out.array(&self.center, |out, item| out.float(*item));
        out.key("major");
        out.array(&self.major, |out, item| out.float(*item));
        out.key("ratio");
        out.float(self.ratio);
        out.key("start_angle");
        out.float(self.start_angle);
        out.key("end_angle");
        out.float(self.end_angle);
    }
}

impl FromJson for EllipseGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for EllipseGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 頂点を結んだ折れ線。幅は持たない
#[derive(Clone, Debug, PartialEq)]
pub struct PolylineGeometry {
    /// 頂点の [X, Y] の並び。2 つ以上
    pub points: Vec<Vec<f64>>,
    /// 頂点ごとの、次の頂点への弧の膨らみ（中心角の 4 分の 1 の正接。正は反時計回り）。points と同じ数。書かなければどれも 0
    pub bulges: Option<Vec<f64>>,
    /// 最後の頂点から最初の頂点へ結ぶ
    pub closed: bool,
}

impl PolylineGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "polyline")?;
        let points = object.required("points", |value| {
            json::array(value, |value| json::array(value, json::number))
        })?;
        let bulges = object.optional("bulges", |value| json::array(value, json::number))?;
        let closed = object.required("closed", json::boolean)?;
        Ok(Self {
            points,
            bulges,
            closed,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("polyline");
        out.key("points");
        out.array(&self.points, |out, item| {
            out.array(item, |out, item| out.float(*item))
        });
        if let Some(value) = &self.bulges {
            out.key("bulges");
            out.array(value, |out, item| out.float(*item));
        }
        out.key("closed");
        out.bool(self.closed);
    }
}

impl FromJson for PolylineGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PolylineGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 1 点か 2 点の間に置く文字。置き換えるときは、位置合わせ・幅の倍率・斜体・反転・文字スタイルを元の値のままにする。足すときは、2 点目がなければ左の基準線に合わせ、幅の倍率 1・斜体なし・反転なしにする
#[derive(Clone, Debug, PartialEq)]
pub struct TextGeometry {
    /// 内容。1 行
    pub content: String,
    /// 基点 [X, Y]。2 点の間に合わせる文字では 1 点目
    pub position: Vec<f64>,
    /// 2 点の間に合わせる文字の 2 点目 [X, Y]。合わせ方（fit）と一緒に書く。置き換えでは、1 点に置く文字と 2 点の間に合わせる文字を入れ替えられない。小版 17 から
    pub end: Option<Vec<f64>>,
    /// 2 点の間への合わせ方。2 点目（end）と一緒に書く。小版 17 から
    pub fit: Option<TextFit>,
    /// 高さ。0 より大きい
    pub height: f64,
    /// 回転（度）。2 点の間に合わせる文字では、1 点目から 2 点目への向きと同じでなければならない
    pub rotation: f64,
    /// 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。DXF に文字スタイルの定義がないときは Standard だけを受ける。置き換えるときは、元と同じ名前だけを受ける。style_id と一緒に書くなら、同じ文字スタイルを指す。小版 12 から
    pub style: Option<String>,
    /// 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。小版 23 から
    pub style_id: Option<String>,
}

impl TextGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "text")?;
        let content = object.required("content", json::string)?;
        let position = object.required("position", |value| json::array(value, json::number))?;
        let end = object.optional("end", |value| json::array(value, json::number))?;
        let fit = object.optional("fit", TextFit::from_json)?;
        let height = object.required("height", json::number)?;
        let rotation = object.required("rotation", json::number)?;
        let style = object.optional("style", json::string)?;
        let style_id = object.optional("style_id", json::string)?;
        Ok(Self {
            content,
            position,
            end,
            fit,
            height,
            rotation,
            style,
            style_id,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("text");
        out.key("content");
        out.string(&self.content);
        out.key("position");
        out.array(&self.position, |out, item| out.float(*item));
        if let Some(value) = &self.end {
            out.key("end");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.fit {
            out.key("fit");
            value.to_json(out);
        }
        out.key("height");
        out.float(self.height);
        out.key("rotation");
        out.float(self.rotation);
        if let Some(value) = &self.style {
            out.key("style");
            out.string(value);
        }
        if let Some(value) = &self.style_id {
            out.key("style_id");
            out.string(value);
        }
    }
}

impl FromJson for TextGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for TextGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 2 点の間への合わせ方。aligned は縦横の比を保って高さも変え、fit は高さを保って幅だけを変える
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TextFit {
    /// `aligned`
    Aligned,
    /// `fit`
    Fit,
}

impl TextFit {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Aligned, Self::Fit];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Aligned => "aligned",
            Self::Fit => "fit",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "aligned" => Some(Self::Aligned),
            "fit" => Some(Self::Fit),
            _ => None,
        }
    }
}

impl FromJson for TextFit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for TextFit {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 書式つきの文字（DXF の MTEXT）。置き換えるときは、枠の高さ・書き進める向き・行送り・背景・高さの意味・文字スタイルを元の値のままにする。足すときは、枠の高さなし・左から右・行送り 1 倍・背景なしにする。小版 13 から
#[derive(Clone, Debug, PartialEq)]
pub struct FormattedTextGeometry {
    /// 内容。書式の符号（\P の改行など）を含めた、元の形式の内容のまま
    pub content: String,
    /// 基点 [X, Y]
    pub position: Vec<f64>,
    /// 高さ。0 より大きい
    pub height: f64,
    /// 回転（度）
    pub rotation: f64,
    /// 基点に合わせる枠の位置
    pub attachment: Option<Attachment>,
    /// 折り返す枠の幅。0 より大きい。書かなければ折り返さない
    pub width: Option<f64>,
    /// 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。置き換えるときは、元と同じ名前だけを受ける。style_id と一緒に書くなら、同じ文字スタイルを指す
    pub style: Option<String>,
    /// 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。小版 23 から
    pub style_id: Option<String>,
}

impl FormattedTextGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "formatted_text")?;
        let content = object.required("content", json::string)?;
        let position = object.required("position", |value| json::array(value, json::number))?;
        let height = object.required("height", json::number)?;
        let rotation = object.required("rotation", json::number)?;
        let attachment = object.optional("attachment", Attachment::from_json)?;
        let width = object.optional("width", json::number)?;
        let style = object.optional("style", json::string)?;
        let style_id = object.optional("style_id", json::string)?;
        Ok(Self {
            content,
            position,
            height,
            rotation,
            attachment,
            width,
            style,
            style_id,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("formatted_text");
        out.key("content");
        out.string(&self.content);
        out.key("position");
        out.array(&self.position, |out, item| out.float(*item));
        out.key("height");
        out.float(self.height);
        out.key("rotation");
        out.float(self.rotation);
        if let Some(value) = &self.attachment {
            out.key("attachment");
            value.to_json(out);
        }
        if let Some(value) = &self.width {
            out.key("width");
            out.float(*value);
        }
        if let Some(value) = &self.style {
            out.key("style");
            out.string(value);
        }
        if let Some(value) = &self.style_id {
            out.key("style_id");
            out.string(value);
        }
    }
}

impl FromJson for FormattedTextGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for FormattedTextGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// スプライン（NURBS）。制御点で決まるもの（小版 15 から）と、当てはめる点で決まるもの（小版 18 から）がある。当てはめる点で決まるものは、節点と制御点を空にし、当てはめる点（fit）と、あれば両端の接線を持つ。元の図形に制御点や節点もあるときは形の値を返さず、置き換えない。置き換えでは、制御点で決まるものと当てはめる点で決まるものを入れ替えない
#[derive(Clone, Debug, PartialEq)]
pub struct SplineGeometry {
    /// 次数。1 以上
    pub degree: u64,
    /// 節点の並び。周期的でなければ、制御点の数と次数と 1 の和の数。当てはめる点で決まるものは空
    pub knots: Vec<f64>,
    /// 制御点の [X, Y] の並び。次数より多い。当てはめる点で決まるものは空
    pub control: Vec<Vec<f64>>,
    /// 制御点ごとの重み。0 より大きい。有理のスプラインだけが持ち、書かなければ有理でない
    pub weights: Option<Vec<f64>>,
    /// 当てはめる点の [X, Y] の並び。2 つ以上。書けば、節点と制御点は空で、重みは書かない。小版 18 から
    pub fit: Option<Vec<Vec<f64>>>,
    /// 当てはめる点で決まるスプラインの、始まりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。小版 18 から
    pub start_tangent: Option<Vec<f64>>,
    /// 当てはめる点で決まるスプラインの、終わりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。小版 18 から
    pub end_tangent: Option<Vec<f64>>,
    /// 閉じている
    pub closed: bool,
    /// 周期的
    pub periodic: bool,
}

impl SplineGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "spline")?;
        let degree = object.required("degree", |value| json::uint(value, 1, 32767))?;
        let knots = object.required("knots", |value| json::array(value, json::number))?;
        let control = object.required("control", |value| {
            json::array(value, |value| json::array(value, json::number))
        })?;
        let weights = object.optional("weights", |value| json::array(value, json::number))?;
        let fit = object.optional("fit", |value| {
            json::array(value, |value| json::array(value, json::number))
        })?;
        let start_tangent =
            object.optional("start_tangent", |value| json::array(value, json::number))?;
        let end_tangent =
            object.optional("end_tangent", |value| json::array(value, json::number))?;
        let closed = object.required("closed", json::boolean)?;
        let periodic = object.required("periodic", json::boolean)?;
        Ok(Self {
            degree,
            knots,
            control,
            weights,
            fit,
            start_tangent,
            end_tangent,
            closed,
            periodic,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("spline");
        out.key("degree");
        out.uint(self.degree);
        out.key("knots");
        out.array(&self.knots, |out, item| out.float(*item));
        out.key("control");
        out.array(&self.control, |out, item| {
            out.array(item, |out, item| out.float(*item))
        });
        if let Some(value) = &self.weights {
            out.key("weights");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.fit {
            out.key("fit");
            out.array(value, |out, item| {
                out.array(item, |out, item| out.float(*item))
            });
        }
        if let Some(value) = &self.start_tangent {
            out.key("start_tangent");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.end_tangent {
            out.key("end_tangent");
            out.array(value, |out, item| out.float(*item));
        }
        out.key("closed");
        out.bool(self.closed);
        out.key("periodic");
        out.bool(self.periodic);
    }
}

impl FromJson for SplineGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SplineGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 寸法。種類（kind）ごとに、その種類の定義点の欄を持つ。測定値（measurement）は元の形式が保存した値で、定義点から測り直さない。寸法を描いた部品（ブロック）は形の値に含めない。entities が返すだけで、置き換えと追加は change.edit.unsupported になる（寸法を描いた部品を作り直す必要があるため）。小版 19 から
#[derive(Clone, Debug, PartialEq)]
pub struct DimensionGeometry {
    /// 寸法の種類
    pub kind: DimensionKind,
    /// 1 本目の補助線の点か、測る角度の 1 本目の線の点か、円弧の始めの点 [X, Y]。linear・aligned・angular・arc_length が持つ
    pub first: Option<Vec<f64>>,
    /// 2 本目の補助線の点か、測る角度の 2 本目の線の点か、円弧の終わりの点 [X, Y]。linear・aligned・angular・arc_length が持つ
    pub second: Option<Vec<f64>>,
    /// 寸法線が通る点 [X, Y]。linear・aligned が持つ
    pub line: Option<Vec<f64>>,
    /// 測る向き（度）。linear が持つ
    pub rotation: Option<f64>,
    /// 補助線の傾き（度）。傾けなければ 0。linear・aligned が持つ
    pub oblique: Option<f64>,
    /// 角度の頂点 [X, Y]。angular が持つ
    pub vertex: Option<Vec<f64>>,
    /// 1 本目の線の始点と終点 [[X, Y], [X, Y]]。angular_lines が持つ
    pub first_line: Option<Vec<Vec<f64>>>,
    /// 2 本目の線の始点と終点 [[X, Y], [X, Y]]。angular_lines が持つ
    pub second_line: Option<Vec<Vec<f64>>>,
    /// 寸法の弧が通る点 [X, Y]。angular・angular_lines・arc_length が持つ
    pub arc: Option<Vec<f64>>,
    /// 中心 [X, Y]。arc_length・radius が持つ
    pub center: Option<Vec<f64>>,
    /// 円の上の点 [X, Y]。radius・diameter が持つ
    pub chord: Option<Vec<f64>>,
    /// chord と向かい合う円の上の点 [X, Y]。diameter が持つ
    pub opposite: Option<Vec<f64>>,
    /// 引出線の長さ。radius・diameter が持つ
    pub leader: Option<f64>,
    /// 原点 [X, Y]。ordinate が持つ
    pub origin: Option<Vec<f64>>,
    /// 測る点 [X, Y]。ordinate が持つ
    pub feature: Option<Vec<f64>>,
    /// 引出線の終点 [X, Y]。ordinate が持つ
    pub leader_end: Option<Vec<f64>>,
    /// 測る座標の軸。ordinate が持つ
    pub axis: Option<DimensionAxis>,
    /// 寸法値の文字の書き方
    pub label: DimensionLabel,
    /// 利用者が書いた寸法値の文字の元の内容。label が override のときだけ持つ。測定値に置き換える符号（DXF の <>）と書式の符号もそのまま持つ
    pub content: Option<String>,
    /// 元の形式が保存した測定値。単位と倍率は measurement_key の欄に従う
    pub measurement: Option<f64>,
    /// measurement の形式と欄を指すキー（DXF の DIMENSION の 42 なら dxf.DIMENSION.42）。measurement と一緒に持つ
    pub measurement_key: Option<String>,
    /// 寸法値の文字の中央の点 [X, Y]。元の形式が持たなければ書かない
    pub text_position: Option<Vec<f64>>,
    /// 寸法値の文字の回転（度）。寸法線に沿わせるなら書かない
    pub text_rotation: Option<f64>,
    /// 寸法スタイルの元の名前
    pub style: String,
    /// 寸法スタイルの ID。tables が返したもの。名前で引いた寸法スタイルがなければ書かない。小版 23 から
    pub style_id: Option<String>,
}

impl DimensionGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "dimension")?;
        let kind = object.required("kind", DimensionKind::from_json)?;
        let first = object.optional("first", |value| json::array(value, json::number))?;
        let second = object.optional("second", |value| json::array(value, json::number))?;
        let line = object.optional("line", |value| json::array(value, json::number))?;
        let rotation = object.optional("rotation", json::number)?;
        let oblique = object.optional("oblique", json::number)?;
        let vertex = object.optional("vertex", |value| json::array(value, json::number))?;
        let first_line = object.optional("first_line", |value| {
            json::array(value, |value| json::array(value, json::number))
        })?;
        let second_line = object.optional("second_line", |value| {
            json::array(value, |value| json::array(value, json::number))
        })?;
        let arc = object.optional("arc", |value| json::array(value, json::number))?;
        let center = object.optional("center", |value| json::array(value, json::number))?;
        let chord = object.optional("chord", |value| json::array(value, json::number))?;
        let opposite = object.optional("opposite", |value| json::array(value, json::number))?;
        let leader = object.optional("leader", json::number)?;
        let origin = object.optional("origin", |value| json::array(value, json::number))?;
        let feature = object.optional("feature", |value| json::array(value, json::number))?;
        let leader_end = object.optional("leader_end", |value| json::array(value, json::number))?;
        let axis = object.optional("axis", DimensionAxis::from_json)?;
        let label = object.required("label", DimensionLabel::from_json)?;
        let content = object.optional("content", json::string)?;
        let measurement = object.optional("measurement", json::number)?;
        let measurement_key = object.optional("measurement_key", json::string)?;
        let text_position =
            object.optional("text_position", |value| json::array(value, json::number))?;
        let text_rotation = object.optional("text_rotation", json::number)?;
        let style = object.required("style", json::string)?;
        let style_id = object.optional("style_id", json::string)?;
        Ok(Self {
            kind,
            first,
            second,
            line,
            rotation,
            oblique,
            vertex,
            first_line,
            second_line,
            arc,
            center,
            chord,
            opposite,
            leader,
            origin,
            feature,
            leader_end,
            axis,
            label,
            content,
            measurement,
            measurement_key,
            text_position,
            text_rotation,
            style,
            style_id,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("dimension");
        out.key("kind");
        self.kind.to_json(out);
        if let Some(value) = &self.first {
            out.key("first");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.second {
            out.key("second");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.line {
            out.key("line");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.rotation {
            out.key("rotation");
            out.float(*value);
        }
        if let Some(value) = &self.oblique {
            out.key("oblique");
            out.float(*value);
        }
        if let Some(value) = &self.vertex {
            out.key("vertex");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.first_line {
            out.key("first_line");
            out.array(value, |out, item| {
                out.array(item, |out, item| out.float(*item))
            });
        }
        if let Some(value) = &self.second_line {
            out.key("second_line");
            out.array(value, |out, item| {
                out.array(item, |out, item| out.float(*item))
            });
        }
        if let Some(value) = &self.arc {
            out.key("arc");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.center {
            out.key("center");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.chord {
            out.key("chord");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.opposite {
            out.key("opposite");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.leader {
            out.key("leader");
            out.float(*value);
        }
        if let Some(value) = &self.origin {
            out.key("origin");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.feature {
            out.key("feature");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.leader_end {
            out.key("leader_end");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.axis {
            out.key("axis");
            value.to_json(out);
        }
        out.key("label");
        self.label.to_json(out);
        if let Some(value) = &self.content {
            out.key("content");
            out.string(value);
        }
        if let Some(value) = &self.measurement {
            out.key("measurement");
            out.float(*value);
        }
        if let Some(value) = &self.measurement_key {
            out.key("measurement_key");
            out.string(value);
        }
        if let Some(value) = &self.text_position {
            out.key("text_position");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.text_rotation {
            out.key("text_rotation");
            out.float(*value);
        }
        out.key("style");
        out.string(&self.style);
        if let Some(value) = &self.style_id {
            out.key("style_id");
            out.string(value);
        }
    }
}

impl FromJson for DimensionGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DimensionGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 引出線（DXF・DWG の LEADER、決定 0177）。注記（MTEXT など）は別の要素で、annotation がその ID を指す。置き換えるときは寸法スタイルと注記を変えず、経路・曲線か・矢印を書き直す。足すときの注記は、同じ空間か用紙の MTEXT だけを指せる（小版 36 から。それまでは注記を持てなかった）。DWG は置き換えと追加を書き戻すが、矢印の印を必ず持つので、arrow を書かない引出線は書けない（小版 36 から。それまでは R2004 までの置き換えだけ）。小版 31 から
#[derive(Clone, Debug, PartialEq)]
pub struct LeaderGeometry {
    /// 経路の頂点 [[X, Y], ...]。矢印の側から並べる。2 つ以上
    pub path: Vec<Vec<f64>>,
    /// 頂点を通る曲線で結ぶか。偽なら折れ線
    pub spline: bool,
    /// 始点に矢印を付けるか。書かなければ寸法スタイルに従う
    pub arrow: Option<bool>,
    /// 注記の要素の ID。同じ空間か用紙の要素で、entities が返したもの。注記がなければ書かない。置き換えるときは元と同じ ID だけを受ける。足すときは、同じ空間か用紙の書式つきの文字（MTEXT）を指す
    pub annotation: Option<String>,
    /// 寸法スタイルの元の名前。足すときは図面にある寸法スタイルの名前を指す。置き換えるときは元と同じ名前だけを受ける
    pub style: String,
    /// 寸法スタイルの ID。tables が返したもの。entities は、名前で引いた寸法スタイルの ID を返す。足す・置き換えるときは、名前と同じ寸法スタイルを指す ID だけを受ける
    pub style_id: Option<String>,
}

impl LeaderGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "leader")?;
        let path = object.required("path", |value| {
            json::array(value, |value| json::array(value, json::number))
        })?;
        let spline = object.required("spline", json::boolean)?;
        let arrow = object.optional("arrow", json::boolean)?;
        let annotation = object.optional("annotation", json::string)?;
        let style = object.required("style", json::string)?;
        let style_id = object.optional("style_id", json::string)?;
        Ok(Self {
            path,
            spline,
            arrow,
            annotation,
            style,
            style_id,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("leader");
        out.key("path");
        out.array(&self.path, |out, item| {
            out.array(item, |out, item| out.float(*item))
        });
        out.key("spline");
        out.bool(self.spline);
        if let Some(value) = &self.arrow {
            out.key("arrow");
            out.bool(*value);
        }
        if let Some(value) = &self.annotation {
            out.key("annotation");
            out.string(value);
        }
        out.key("style");
        out.string(&self.style);
        if let Some(value) = &self.style_id {
            out.key("style_id");
            out.string(value);
        }
    }
}

impl FromJson for LeaderGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LeaderGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 引出線つきの注記（DXF・DWG の MULTILEADER、決定 0185）。注記は形の中にあり、content と text・symbol で種類と中身を示す。置き換えるときは枝の線の頂点・集まる点・折れ曲がりだけを変えられ、ほかの欄は元の値のままにする。DXF だけが置き換えを書き戻し、枝と線の数と、集まる点と折れ曲がりの有無は元と同じでなければならない。足すことと DWG の書き戻しは、まだしない。小版 36 から
#[derive(Clone, Debug, PartialEq)]
pub struct CalloutGeometry {
    /// 枝。入力の順
    pub branches: Vec<CalloutBranch>,
    /// 線の引き方
    pub path: CalloutPath,
    /// 線を注記に、水平の線（ランディング）でつなぐか
    pub landing: bool,
    /// 端部記号の大きさ
    pub arrow_size: f64,
    /// 大きさに掛ける倍率
    pub scale: f64,
    /// 注記の種類
    pub content: CalloutContent,
    /// 注記の書式つきの文字の元の内容。content が text のときだけ持つ
    pub text: Option<String>,
    /// 注記の記号のブロックの名前。content が symbol のときだけ持つ
    pub symbol: Option<String>,
}

impl CalloutGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "callout")?;
        let branches = object.required("branches", |value| {
            json::array(value, CalloutBranch::from_json)
        })?;
        let path = object.required("path", CalloutPath::from_json)?;
        let landing = object.required("landing", json::boolean)?;
        let arrow_size = object.required("arrow_size", json::number)?;
        let scale = object.required("scale", json::number)?;
        let content = object.required("content", CalloutContent::from_json)?;
        let text = object.optional("text", json::string)?;
        let symbol = object.optional("symbol", json::string)?;
        Ok(Self {
            branches,
            path,
            landing,
            arrow_size,
            scale,
            content,
            text,
            symbol,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("callout");
        out.key("branches");
        out.array(&self.branches, |out, item| item.to_json(out));
        out.key("path");
        self.path.to_json(out);
        out.key("landing");
        out.bool(self.landing);
        out.key("arrow_size");
        out.float(self.arrow_size);
        out.key("scale");
        out.float(self.scale);
        out.key("content");
        self.content.to_json(out);
        if let Some(value) = &self.text {
            out.key("text");
            out.string(value);
        }
        if let Some(value) = &self.symbol {
            out.key("symbol");
            out.string(value);
        }
    }
}

impl FromJson for CalloutGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CalloutGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 引出線つきの注記の枝。何本かの線が 1 点に集まり、折れ曲がりで注記につながる
#[derive(Clone, Debug, PartialEq)]
pub struct CalloutBranch {
    /// 線ごとの頂点 [[[X, Y], ...], ...]。矢印の側から並べ、集まる点は含めない。線ごとに 1 つ以上
    pub lines: Vec<Vec<Vec<f64>>>,
    /// 線が集まる点 [X, Y]。元の形式が持たなければ書かない
    pub end: Option<Vec<f64>>,
    /// 集まる点から注記への折れ曲がり。なければ書かない
    pub dogleg: Option<CalloutDogleg>,
}

impl CalloutBranch {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let lines = object.required("lines", |value| {
            json::array(value, |value| {
                json::array(value, |value| json::array(value, json::number))
            })
        })?;
        let end = object.optional("end", |value| json::array(value, json::number))?;
        let dogleg = object.optional("dogleg", CalloutDogleg::from_json)?;
        Ok(Self { lines, end, dogleg })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("lines");
        out.array(&self.lines, |out, item| {
            out.array(item, |out, item| {
                out.array(item, |out, item| out.float(*item))
            })
        });
        if let Some(value) = &self.end {
            out.key("end");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.dogleg {
            out.key("dogleg");
            value.to_json(out);
        }
    }
}

impl FromJson for CalloutBranch {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CalloutBranch {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 枝の折れ曲がり
#[derive(Clone, Debug, PartialEq)]
pub struct CalloutDogleg {
    /// 向き [X, Y]。長さは意味を持たない
    pub direction: Vec<f64>,
    /// 長さ
    pub length: f64,
}

impl CalloutDogleg {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let direction = object.required("direction", |value| json::array(value, json::number))?;
        let length = object.required("length", json::number)?;
        Ok(Self { direction, length })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("direction");
        out.array(&self.direction, |out, item| out.float(*item));
        out.key("length");
        out.float(self.length);
    }
}

impl FromJson for CalloutDogleg {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CalloutDogleg {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 引出線つきの注記の線の引き方。straight は折れ線、spline は頂点を通る曲線、hidden は線を描かない
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CalloutPath {
    /// `straight`
    Straight,
    /// `spline`
    Spline,
    /// `hidden`
    Hidden,
}

impl CalloutPath {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Straight, Self::Spline, Self::Hidden];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Straight => "straight",
            Self::Spline => "spline",
            Self::Hidden => "hidden",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "straight" => Some(Self::Straight),
            "spline" => Some(Self::Spline),
            "hidden" => Some(Self::Hidden),
            _ => None,
        }
    }
}

impl FromJson for CalloutPath {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for CalloutPath {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 引出線つきの注記の中身の種類。none は注記を持たず、text は書式つきの文字、symbol は記号（バルーンなどの囲み）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CalloutContent {
    /// `none`
    None,
    /// `text`
    Text,
    /// `symbol`
    Symbol,
}

impl CalloutContent {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::None, Self::Text, Self::Symbol];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Text => "text",
            Self::Symbol => "symbol",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "none" => Some(Self::None),
            "text" => Some(Self::Text),
            "symbol" => Some(Self::Symbol),
            _ => None,
        }
    }
}

impl FromJson for CalloutContent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for CalloutContent {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 点（決定 0160・0183）。marker がなければ文書の点の表し方（DXF・DWG の $PDMODE・$PDSIZE）に従う点で、DXF・DWG の POINT と JWW の記号でない点が当たる。marker があれば既定義の印の点で、SFC の点マーカーと JWW の記号の点が当たる。置き換えるときは、描き方（marker があるか）を元のまま保つ。DXF・DWG は marker と construction を持つ点を書かず、JWW の記号でない点は角度 0 だけを、SXF は SFC の点マーカーだけを書く。小版 35 から
#[derive(Clone, Debug, PartialEq)]
pub struct PointGeometry {
    /// 位置 [X, Y]
    pub position: Vec<f64>,
    /// 回転（度）
    pub rotation: f64,
    /// 既定義の印。書かなければ文書の点の表し方に従う
    pub marker: Option<PointMarker>,
    /// 作図の補助の点（JWW の仮点）か。書かなければ偽
    pub construction: Option<bool>,
}

impl PointGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "point")?;
        let position = object.required("position", |value| json::array(value, json::number))?;
        let rotation = object.required("rotation", json::number)?;
        let marker = object.optional("marker", PointMarker::from_json)?;
        let construction = object.optional("construction", json::boolean)?;
        Ok(Self {
            position,
            rotation,
            marker,
            construction,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("point");
        out.key("position");
        out.array(&self.position, |out, item| out.float(*item));
        out.key("rotation");
        out.float(self.rotation);
        if let Some(value) = &self.marker {
            out.key("marker");
            value.to_json(out);
        }
        if let Some(value) = &self.construction {
            out.key("construction");
            out.bool(*value);
        }
    }
}

impl FromJson for PointGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PointGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 既定義の印の表のキーと尺度。印の形は、表示する側がキーから引く
#[derive(Clone, Debug, PartialEq)]
pub struct PointMarker {
    /// 印の表の名前空間。sxf.marker（SFC の点マーカー）か jww.point（JWW の記号の点）
    pub namespace: String,
    /// 名前空間の中の元のコード
    pub code: String,
    /// 尺度
    pub scale: f64,
}

impl PointMarker {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let namespace = object.required("namespace", json::string)?;
        let code = object.required("code", json::string)?;
        let scale = object.required("scale", json::number)?;
        Ok(Self {
            namespace,
            code,
            scale,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("namespace");
        out.string(&self.namespace);
        out.key("code");
        out.string(&self.code);
        out.key("scale");
        out.float(self.scale);
    }
}

impl FromJson for PointMarker {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PointMarker {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 寸法の種類。linear は 2 点を測る向きに、aligned は 2 点を結ぶ向きに測った長さ。angular は頂点から 2 点へ向かう線の、angular_lines は 2 本の線のなす角度。arc_length は円弧の長さ。radius は半径、diameter は直径。ordinate は原点からの X か Y の座標
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DimensionKind {
    /// `linear`
    Linear,
    /// `aligned`
    Aligned,
    /// `angular`
    Angular,
    /// `angular_lines`
    AngularLines,
    /// `arc_length`
    ArcLength,
    /// `radius`
    Radius,
    /// `diameter`
    Diameter,
    /// `ordinate`
    Ordinate,
}

impl DimensionKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Linear,
        Self::Aligned,
        Self::Angular,
        Self::AngularLines,
        Self::ArcLength,
        Self::Radius,
        Self::Diameter,
        Self::Ordinate,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Aligned => "aligned",
            Self::Angular => "angular",
            Self::AngularLines => "angular_lines",
            Self::ArcLength => "arc_length",
            Self::Radius => "radius",
            Self::Diameter => "diameter",
            Self::Ordinate => "ordinate",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "linear" => Some(Self::Linear),
            "aligned" => Some(Self::Aligned),
            "angular" => Some(Self::Angular),
            "angular_lines" => Some(Self::AngularLines),
            "arc_length" => Some(Self::ArcLength),
            "radius" => Some(Self::Radius),
            "diameter" => Some(Self::Diameter),
            "ordinate" => Some(Self::Ordinate),
            _ => None,
        }
    }
}

impl FromJson for DimensionKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for DimensionKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 座標の軸
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DimensionAxis {
    /// `x`
    X,
    /// `y`
    Y,
}

impl DimensionAxis {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::X, Self::Y];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "x" => Some(Self::X),
            "y" => Some(Self::Y),
            _ => None,
        }
    }
}

impl FromJson for DimensionAxis {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for DimensionAxis {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 寸法値の文字の書き方。measurement は測定値を書き、override は利用者が書いた文字（content）を書き、hidden は書かない
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DimensionLabel {
    /// `measurement`
    Measurement,
    /// `override`
    Override,
    /// `hidden`
    Hidden,
}

impl DimensionLabel {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Measurement, Self::Override, Self::Hidden];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Measurement => "measurement",
            Self::Override => "override",
            Self::Hidden => "hidden",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "measurement" => Some(Self::Measurement),
            "override" => Some(Self::Override),
            "hidden" => Some(Self::Hidden),
            _ => None,
        }
    }
}

impl FromJson for DimensionLabel {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for DimensionLabel {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 記号（DXF・DWG のブロックの INSERT）。定義の点を、尺度・回転・移動の順に写す。切り抜きは形の値に含めない。置き換えるときは切り抜きを元のままにし、属性は値だけを変えられる。属性を持つ記号の置く点・尺度・回転は変えない（属性ごと動かすなら transform を使う）。足すときは切り抜きを持たない。小版 14 から
#[derive(Clone, Debug, PartialEq)]
pub struct SymbolGeometry {
    /// 記号の定義（ブロック）の元の名前。足すときは図面にある定義の名前を指す
    pub name: String,
    /// 定義の基点を置く点 [X, Y]
    pub position: Vec<f64>,
    /// X と Y の尺度 [X, Y]。0 でない。負なら反転する
    pub scale: Vec<f64>,
    /// 回転（度）
    pub rotation: f64,
    /// 格子に並べて置く列と行。書かなければ 1 つだけ置く
    pub grid: Option<SymbolGrid>,
    /// 属性のタグと値。entities は記号の属性を入力の順に返す。足すときは、定義の属性の定義（値が決まったものを除く）から属性を作り、ここに書いたタグの値で既定の値を替える。タグは大文字と小文字を区別せず、重複を受けない。必要な属性を作れなければ失敗する。置き換えるときは、元と同じ数・順・タグを受け、値を変えた属性の ATTRIB の値を書き直す（表題欄の項目の書き換えなど）。書式つきの値（複数行の属性）を持つ属性の値は変えられない。小版 16 から。置き換えで値を変えるのは小版 21 から
    pub attributes: Option<Vec<SymbolAttribute>>,
}

impl SymbolGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "symbol")?;
        let name = object.required("name", json::string)?;
        let position = object.required("position", |value| json::array(value, json::number))?;
        let scale = object.required("scale", |value| json::array(value, json::number))?;
        let rotation = object.required("rotation", json::number)?;
        let grid = object.optional("grid", SymbolGrid::from_json)?;
        let attributes = object.optional("attributes", |value| {
            json::array(value, SymbolAttribute::from_json)
        })?;
        Ok(Self {
            name,
            position,
            scale,
            rotation,
            grid,
            attributes,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("symbol");
        out.key("name");
        out.string(&self.name);
        out.key("position");
        out.array(&self.position, |out, item| out.float(*item));
        out.key("scale");
        out.array(&self.scale, |out, item| out.float(*item));
        out.key("rotation");
        out.float(self.rotation);
        if let Some(value) = &self.grid {
            out.key("grid");
            value.to_json(out);
        }
        if let Some(value) = &self.attributes {
            out.key("attributes");
            out.array(value, |out, item| item.to_json(out));
        }
    }
}

impl FromJson for SymbolGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SymbolGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 記号の属性のタグと値
#[derive(Clone, Debug, PartialEq)]
pub struct SymbolAttribute {
    /// 元のタグ
    pub tag: String,
    /// 値。1 行
    pub value: String,
}

impl SymbolAttribute {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let tag = object.required("tag", json::string)?;
        let value = object.required("value", json::string)?;
        Ok(Self { tag, value })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("tag");
        out.string(&self.tag);
        out.key("value");
        out.string(&self.value);
    }
}

impl FromJson for SymbolAttribute {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SymbolAttribute {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 記号を格子に並べる列と行。間隔は、記号を回転する前の向きに測る。置き換えるときは、列と行の数を変えない
#[derive(Clone, Debug, PartialEq)]
pub struct SymbolGrid {
    /// 列の数。1 以上
    pub columns: u64,
    /// 行の数。1 以上。列と行がどちらも 1 の格子は持たない
    pub rows: u64,
    /// 列の間隔
    pub column_spacing: f64,
    /// 行の間隔
    pub row_spacing: f64,
}

impl SymbolGrid {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let columns = object.required("columns", |value| json::uint(value, 1, 32767))?;
        let rows = object.required("rows", |value| json::uint(value, 1, 32767))?;
        let column_spacing = object.required("column_spacing", json::number)?;
        let row_spacing = object.required("row_spacing", json::number)?;
        Ok(Self {
            columns,
            rows,
            column_spacing,
            row_spacing,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("columns");
        out.uint(self.columns);
        out.key("rows");
        out.uint(self.rows);
        out.key("column_spacing");
        out.float(self.column_spacing);
        out.key("row_spacing");
        out.float(self.row_spacing);
    }
}

impl FromJson for SymbolGrid {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SymbolGrid {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 基点に合わせる枠の位置。entities は返し、足すときに書かなければ左上、置き換えるときに書かなければ元のまま
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Attachment {
    /// `top_left`
    TopLeft,
    /// `top_center`
    TopCenter,
    /// `top_right`
    TopRight,
    /// `middle_left`
    MiddleLeft,
    /// `middle_center`
    MiddleCenter,
    /// `middle_right`
    MiddleRight,
    /// `bottom_left`
    BottomLeft,
    /// `bottom_center`
    BottomCenter,
    /// `bottom_right`
    BottomRight,
}

impl Attachment {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::TopLeft,
        Self::TopCenter,
        Self::TopRight,
        Self::MiddleLeft,
        Self::MiddleCenter,
        Self::MiddleRight,
        Self::BottomLeft,
        Self::BottomCenter,
        Self::BottomRight,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::TopLeft => "top_left",
            Self::TopCenter => "top_center",
            Self::TopRight => "top_right",
            Self::MiddleLeft => "middle_left",
            Self::MiddleCenter => "middle_center",
            Self::MiddleRight => "middle_right",
            Self::BottomLeft => "bottom_left",
            Self::BottomCenter => "bottom_center",
            Self::BottomRight => "bottom_right",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "top_left" => Some(Self::TopLeft),
            "top_center" => Some(Self::TopCenter),
            "top_right" => Some(Self::TopRight),
            "middle_left" => Some(Self::MiddleLeft),
            "middle_center" => Some(Self::MiddleCenter),
            "middle_right" => Some(Self::MiddleRight),
            "bottom_left" => Some(Self::BottomLeft),
            "bottom_center" => Some(Self::BottomCenter),
            "bottom_right" => Some(Self::BottomRight),
            _ => None,
        }
    }
}

impl FromJson for Attachment {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for Attachment {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 要素の形の種類（決定 0022）。point（点、決定 0160）は小版 32 から。それまで点は symbol だった
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
    /// `point`
    Point,
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
        Self::Point,
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
            Self::Point => "point",
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
            "point" => Some(Self::Point),
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
    /// true なら、変えていない要素の ID がずれた。鍵を持たない形式（JWW）で、前の図形を除くか足したとき。ID を使う前に entities で読み直す。ずれないときは持たない。小版 30 から
    pub ids_shifted: Option<bool>,
}

impl ApplyResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let applied = object.required("applied", |value| json::array(value, Applied::from_json))?;
        let hash = object.required("hash", json::string)?;
        let ids_shifted = object.optional("ids_shifted", json::boolean)?;
        Ok(Self {
            applied,
            hash,
            ids_shifted,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("applied");
        out.array(&self.applied, |out, item| item.to_json(out));
        out.key("hash");
        out.string(&self.hash);
        if let Some(value) = &self.ids_shifted {
            out.key("ids_shifted");
            out.bool(*value);
        }
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

/// 開いた文書の図面モデルを、別の形式の新しいファイルに書き、新しい文書として開く（決定 0139・0206）。書けない要素は落とし、種類ごとの件数を診断で返す。書いたファイルは blobs で返す。今は DXF にだけ書ける。小版 41 から
#[derive(Clone, Debug, PartialEq)]
pub struct ConvertRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
    /// 書く元の文書の名前。open で作ったもの
    pub doc: String,
    /// 書く形式の名前（capabilities の formats の id）。今は dxf だけ
    pub format: String,
    /// 作る文書の名前
    pub to: String,
}

impl ConvertRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "convert")?;
        let doc = object.required("doc", json::string)?;
        let format = object.required("format", json::string)?;
        let to = object.required("to", json::string)?;
        Ok(Self {
            request_common,
            doc,
            format,
            to,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("convert");
        out.key("doc");
        out.string(&self.doc);
        out.key("format");
        out.string(&self.format);
        out.key("to");
        out.string(&self.to);
    }
}

impl FromJson for ConvertRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ConvertRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 書いた文書。書いたファイルは blobs の file で返す
#[derive(Clone, Debug, PartialEq)]
pub struct ConvertResult {
    /// 作った文書の名前
    pub doc: String,
    /// 書いた形式の名前
    pub format: String,
    /// 書いたファイルのハッシュ。apply の前提条件に使う
    pub hash: String,
    /// 書いた要素の数。記号から離した属性の文字を含む
    pub elements: u64,
}

impl ConvertResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let doc = object.required("doc", json::string)?;
        let format = object.required("format", json::string)?;
        let hash = object.required("hash", json::string)?;
        let elements =
            object.required("elements", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            doc,
            format,
            hash,
            elements,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("doc");
        out.string(&self.doc);
        out.key("format");
        out.string(&self.format);
        out.key("hash");
        out.string(&self.hash);
        out.key("elements");
        out.uint(self.elements);
    }
}

impl FromJson for ConvertResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ConvertResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 当てた変更 1 つの結果
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Applied {
    /// 変えた要素（レイヤの変更では、そのレイヤ）の、書き戻した後の ID。除いたときと、ID を決められないときは持たない
    pub id: Option<String>,
    /// 変えた要素（レイヤ）の、書き戻した後のハッシュ。id があるときに持つ
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
#[allow(clippy::large_enum_variant)]
pub enum Edit {
    /// 要素のレイヤ・色・線種・線種の倍率・線幅のうち、書いたものだけを変える。形は変えない
    Restyle(RestyleEdit),
    /// 要素を、アフィン変換で動かす。AutoCAD の系統では、図形とともに動く XDATA と記号の属性も同じ変換で直す。変換した形を元の種類で表せなければ当てない
    Transform(TransformEdit),
    /// 要素を除く
    Remove(RemoveEdit),
    /// 要素の形を、渡す値で置き換える。形の種類は変えない。レイヤとスタイルは変えない。小版 10 から
    Replace(ReplaceEdit),
    /// 空間か用紙に要素を足す。書かない色・線種・線幅はレイヤに従う。足せる形は、形の値のどの種類も。小版 10 から
    Add(AddEdit),
    /// レイヤの名前・状態・色・線種・線幅のうち、書いたものだけを変える（決定 0162）。DXF は、名前を変えると、図形と今のレイヤ（$CLAYER）も新しい名前で指す。DWG は、今あるレイヤの線種と真の色を変えられない。小版 22 から
    Layer(LayerEdit),
    /// レイヤを足す（決定 0162）。書かない欄は、状態なら表示・印刷・編集でき凍結せず、色・線種・線幅ならレイヤ 0 と同じ。同じ apply の要素は、足したレイヤを指せない。小版 22 から
    AddLayer(AddLayerEdit),
    /// レイヤを除く（決定 0162）。要素・ほかのオブジェクト・今のレイヤ（$CLAYER）が指すレイヤと、0・Defpoints・外部参照に依るレイヤは除けない。同じ apply の要素の変更が指すレイヤも除けない。後ろにハンドルのないレイヤがあれば、その ID が変わるので除けない。除いたレイヤより後ろのレイヤと、その上の要素のハッシュは変わるので、読み直す。小版 24 から
    RemoveLayer(RemoveLayerEdit),
    /// 文字スタイルの名前・フォント・高さ・幅の倍率・傾き・裏返し・上下反転・縦書きのうち、書いたものだけを変える（決定 0163）。DXF は、名前を変えると、文字と今の文字スタイル（$TEXTSTYLE）も新しい名前で指す。表の書式などの図形でないオブジェクトが名前で指す文字スタイルの名前は変えられない。小版 25 から
    TextStyle(TextStyleEdit),
    /// 文字スタイルを足す（決定 0163）。書かない欄は Standard と同じ。同じ apply の要素は、足した文字スタイルを指せない。小版 25 から
    AddTextStyle(AddTextStyleEdit),
    /// 文字スタイルを除く（決定 0163）。要素・表の書式や寸法スタイルなどのほかのオブジェクト・今の文字スタイル（$TEXTSTYLE）が指すものと、Standard・外部参照に依るものは除けない。同じ apply で足す要素が指すものも除けない。後ろにハンドルのない文字スタイルがあれば、その ID が変わるので除けない。小版 25 から
    RemoveTextStyle(RemoveTextStyleEdit),
    /// 線種の名前・説明・線と間の長さの列のうち、書いたものだけを変える（決定 0164）。DXF は、名前を変えると、要素・レイヤと今の線種（$CELTYPE）も新しい名前で指す。多重線のスタイルなどの図形でないオブジェクトが名前で指す線種の名前は変えられない。形か文字を添える線種と DWG の線種は、列の要素の数を変えられない。小版 26 から
    LineType(LineTypeEdit),
    /// 線種を足す（決定 0164）。書かない欄は Continuous と同じ。同じ apply の要素とレイヤは、足した線種を指せない。小版 26 から
    AddLineType(AddLineTypeEdit),
    /// 線種を除く（決定 0164）。要素・レイヤ・多重線のスタイルなどのほかのオブジェクト・今の線種（$CELTYPE）が指すものと、ByLayer・ByBlock・Continuous・外部参照に依るものは除けない。同じ apply で足す・変える要素とレイヤが指すものも除けない。後ろにハンドルのない線種があれば、その ID が変わるので除けない。小版 26 から
    RemoveLineType(RemoveLineTypeEdit),
    /// 寸法スタイルの名前・端部記号・その大きさ・斜線・倍率のうち、書いたものだけを変える（決定 0165）。DXF は、名前を変えると、寸法・引出線・公差と今の寸法スタイル（$DIMSTYLE）も新しい名前で指す。図形でないオブジェクトが名前で指す寸法スタイルの名前は変えられない。小版 27 から
    DimensionStyle(DimensionStyleEdit),
    /// 寸法スタイルを足す（決定 0165）。書かない欄は Standard と同じ。同じ apply の要素は、足した寸法スタイルを指せない。小版 27 から
    AddDimensionStyle(AddDimensionStyleEdit),
    /// 寸法スタイルを除く（決定 0165）。寸法・引出線・公差などのほかのオブジェクト・今の寸法スタイル（$DIMSTYLE）が指すものと、Standard・子の寸法スタイル・子を持つ寸法スタイル・外部参照に依るものは除けない。後ろにハンドルのない寸法スタイルがあれば、その ID が変わるので除けない。小版 27 から
    RemoveDimensionStyle(RemoveDimensionStyleEdit),
    /// 文書の点の表し方（marker を持たない点の見え方）を変える（決定 0190）。文書全体の設定なので要素を指さない。読んだときの文書は、apply の hash で確かめる。DXF・DWG は見出しの $PDMODE・$PDSIZE に書く。DWG は、変数を書く幅が変わる値（$PDMODE の 0 とほかの値の間の変更、$PDSIZE の 0・1 とほかの値の間の変更）を、小版 39 から書ける（決定 0193）。JWW・SXF は文書の点の表し方を持たないので断る。applied は id と hash を持たない。小版 38 から
    PointStyle(PointStyleEdit),
}

impl Edit {
    /// 目印の欄（`kind`）の値（スキーマの順）
    pub const TAGS: &[&str] = &[
        "restyle",
        "transform",
        "remove",
        "replace",
        "add",
        "layer",
        "add_layer",
        "remove_layer",
        "text_style",
        "add_text_style",
        "remove_text_style",
        "line_type",
        "add_line_type",
        "remove_line_type",
        "dimension_style",
        "add_dimension_style",
        "remove_dimension_style",
        "point_style",
    ];

    /// 目印の欄（`kind`）の値
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Restyle(_) => "restyle",
            Self::Transform(_) => "transform",
            Self::Remove(_) => "remove",
            Self::Replace(_) => "replace",
            Self::Add(_) => "add",
            Self::Layer(_) => "layer",
            Self::AddLayer(_) => "add_layer",
            Self::RemoveLayer(_) => "remove_layer",
            Self::TextStyle(_) => "text_style",
            Self::AddTextStyle(_) => "add_text_style",
            Self::RemoveTextStyle(_) => "remove_text_style",
            Self::LineType(_) => "line_type",
            Self::AddLineType(_) => "add_line_type",
            Self::RemoveLineType(_) => "remove_line_type",
            Self::DimensionStyle(_) => "dimension_style",
            Self::AddDimensionStyle(_) => "add_dimension_style",
            Self::RemoveDimensionStyle(_) => "remove_dimension_style",
            Self::PointStyle(_) => "point_style",
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
            "replace" => Self::Replace(ReplaceEdit::read_fields(&mut object)?),
            "add" => Self::Add(AddEdit::read_fields(&mut object)?),
            "layer" => Self::Layer(LayerEdit::read_fields(&mut object)?),
            "add_layer" => Self::AddLayer(AddLayerEdit::read_fields(&mut object)?),
            "remove_layer" => Self::RemoveLayer(RemoveLayerEdit::read_fields(&mut object)?),
            "text_style" => Self::TextStyle(TextStyleEdit::read_fields(&mut object)?),
            "add_text_style" => Self::AddTextStyle(AddTextStyleEdit::read_fields(&mut object)?),
            "remove_text_style" => {
                Self::RemoveTextStyle(RemoveTextStyleEdit::read_fields(&mut object)?)
            }
            "line_type" => Self::LineType(LineTypeEdit::read_fields(&mut object)?),
            "add_line_type" => Self::AddLineType(AddLineTypeEdit::read_fields(&mut object)?),
            "remove_line_type" => {
                Self::RemoveLineType(RemoveLineTypeEdit::read_fields(&mut object)?)
            }
            "dimension_style" => {
                Self::DimensionStyle(DimensionStyleEdit::read_fields(&mut object)?)
            }
            "add_dimension_style" => {
                Self::AddDimensionStyle(AddDimensionStyleEdit::read_fields(&mut object)?)
            }
            "remove_dimension_style" => {
                Self::RemoveDimensionStyle(RemoveDimensionStyleEdit::read_fields(&mut object)?)
            }
            "point_style" => Self::PointStyle(PointStyleEdit::read_fields(&mut object)?),
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
            Self::Replace(value) => value.to_json(out),
            Self::Add(value) => value.to_json(out),
            Self::Layer(value) => value.to_json(out),
            Self::AddLayer(value) => value.to_json(out),
            Self::RemoveLayer(value) => value.to_json(out),
            Self::TextStyle(value) => value.to_json(out),
            Self::AddTextStyle(value) => value.to_json(out),
            Self::RemoveTextStyle(value) => value.to_json(out),
            Self::LineType(value) => value.to_json(out),
            Self::AddLineType(value) => value.to_json(out),
            Self::RemoveLineType(value) => value.to_json(out),
            Self::DimensionStyle(value) => value.to_json(out),
            Self::AddDimensionStyle(value) => value.to_json(out),
            Self::RemoveDimensionStyle(value) => value.to_json(out),
            Self::PointStyle(value) => value.to_json(out),
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

/// 要素の形を、渡す値で置き換える。形の種類は変えない。レイヤとスタイルは変えない。小版 10 から
#[derive(Clone, Debug, PartialEq)]
pub struct ReplaceEdit {
    /// 置き換える要素の ID。entities が返したもの
    pub id: String,
    /// 読んだときの要素のハッシュ。entities が返したもの
    pub hash: String,
    /// 新しい形。entities の geometry と同じ種類
    pub geometry: Geometry,
}

impl ReplaceEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "replace")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        let geometry = object.required("geometry", Geometry::from_json)?;
        Ok(Self { id, hash, geometry })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("replace");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
        out.key("geometry");
        self.geometry.to_json(out);
    }
}

impl FromJson for ReplaceEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ReplaceEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 空間か用紙に要素を足す。書かない色・線種・線幅はレイヤに従う。足せる形は、形の値のどの種類も。小版 10 から
#[derive(Clone, Debug, PartialEq)]
pub struct AddEdit {
    /// 足す先の空間か用紙の ID（space/0・sheet/h22 など）。entities の ID の先頭の部分。小版 31 から、記号の定義の空間（space/h1F など）も指せる（決定 0177）
    pub place: String,
    /// レイヤの元の名前。図面にあるレイヤで、名前が 1 つに決まるもの。layer_id と一緒には書かない。どちらも書かなければレイヤ 0
    pub layer: Option<String>,
    /// レイヤの ID。tables が返したもの
    pub layer_id: Option<String>,
    /// 色。SXF はレイヤに従う色を書けないので、書く。小版 29 から
    pub color: Option<ColorChange>,
    /// 線種。小版 29 から
    pub line_type: Option<LineTypeChange>,
    /// 線幅。小版 29 から
    pub line_weight: Option<LineWeightChange>,
    /// 足す形
    pub geometry: Geometry,
}

impl AddEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "add")?;
        let place = object.required("place", json::string)?;
        let layer = object.optional("layer", json::string)?;
        let layer_id = object.optional("layer_id", json::string)?;
        let color = object.optional("color", ColorChange::from_json)?;
        let line_type = object.optional("line_type", LineTypeChange::from_json)?;
        let line_weight = object.optional("line_weight", LineWeightChange::from_json)?;
        let geometry = object.required("geometry", Geometry::from_json)?;
        Ok(Self {
            place,
            layer,
            layer_id,
            color,
            line_type,
            line_weight,
            geometry,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("add");
        out.key("place");
        out.string(&self.place);
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
        if let Some(value) = &self.line_weight {
            out.key("line_weight");
            value.to_json(out);
        }
        out.key("geometry");
        self.geometry.to_json(out);
    }
}

impl FromJson for AddEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for AddEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// レイヤの名前・状態・色・線種・線幅のうち、書いたものだけを変える（決定 0162）。DXF は、名前を変えると、図形と今のレイヤ（$CLAYER）も新しい名前で指す。DWG は、今あるレイヤの線種と真の色を変えられない。小版 22 から
#[derive(Clone, Debug, PartialEq)]
pub struct LayerEdit {
    /// 変えるレイヤの ID。tables が返したもの
    pub id: String,
    /// 読んだときのレイヤのハッシュ。tables が返したもの
    pub hash: String,
    /// 新しい名前。ほかのレイヤと重ならず（大文字と小文字を区別しない）、< > / \ " : ; ? * | = ` を含まないもの。0 と Defpoints の名前は変えられない
    pub name: Option<String>,
    /// 表示する。凍結するなら偽
    pub visible: Option<bool>,
    /// 印刷する
    pub printable: Option<bool>,
    /// 編集できる（ロックしない）
    pub editable: Option<bool>,
    /// 凍結する
    pub frozen: Option<bool>,
    /// 色。aci が要る。inherit は書けない
    pub color: Option<ColorChange>,
    /// 線種。name か id で指す。inherit は書けない
    pub line_type: Option<LineTypeChange>,
    /// 線幅。mm か default。inherit は書けない
    pub line_weight: Option<LineWeightChange>,
}

impl LayerEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "layer")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        let name = object.optional("name", json::string)?;
        let visible = object.optional("visible", json::boolean)?;
        let printable = object.optional("printable", json::boolean)?;
        let editable = object.optional("editable", json::boolean)?;
        let frozen = object.optional("frozen", json::boolean)?;
        let color = object.optional("color", ColorChange::from_json)?;
        let line_type = object.optional("line_type", LineTypeChange::from_json)?;
        let line_weight = object.optional("line_weight", LineWeightChange::from_json)?;
        Ok(Self {
            id,
            hash,
            name,
            visible,
            printable,
            editable,
            frozen,
            color,
            line_type,
            line_weight,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("layer");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.visible {
            out.key("visible");
            out.bool(*value);
        }
        if let Some(value) = &self.printable {
            out.key("printable");
            out.bool(*value);
        }
        if let Some(value) = &self.editable {
            out.key("editable");
            out.bool(*value);
        }
        if let Some(value) = &self.frozen {
            out.key("frozen");
            out.bool(*value);
        }
        if let Some(value) = &self.color {
            out.key("color");
            value.to_json(out);
        }
        if let Some(value) = &self.line_type {
            out.key("line_type");
            value.to_json(out);
        }
        if let Some(value) = &self.line_weight {
            out.key("line_weight");
            value.to_json(out);
        }
    }
}

impl FromJson for LayerEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LayerEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// レイヤを除く（決定 0162）。要素・ほかのオブジェクト・今のレイヤ（$CLAYER）が指すレイヤと、0・Defpoints・外部参照に依るレイヤは除けない。同じ apply の要素の変更が指すレイヤも除けない。後ろにハンドルのないレイヤがあれば、その ID が変わるので除けない。除いたレイヤより後ろのレイヤと、その上の要素のハッシュは変わるので、読み直す。小版 24 から
#[derive(Clone, Debug, PartialEq)]
pub struct RemoveLayerEdit {
    /// 除くレイヤの ID。tables が返したもの
    pub id: String,
    /// 読んだときのレイヤのハッシュ。tables が返したもの
    pub hash: String,
}

impl RemoveLayerEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "remove_layer")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        Ok(Self { id, hash })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("remove_layer");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
    }
}

impl FromJson for RemoveLayerEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RemoveLayerEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 文字スタイルの名前・フォント・高さ・幅の倍率・傾き・裏返し・上下反転・縦書きのうち、書いたものだけを変える（決定 0163）。DXF は、名前を変えると、文字と今の文字スタイル（$TEXTSTYLE）も新しい名前で指す。表の書式などの図形でないオブジェクトが名前で指す文字スタイルの名前は変えられない。小版 25 から
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyleEdit {
    /// 変える文字スタイルの ID。tables が返したもの
    pub id: String,
    /// 読んだときの文字スタイルのハッシュ。tables が返したもの
    pub hash: String,
    /// 新しい名前。ほかの文字スタイルと重ならず（大文字と小文字を区別しない）、< > / \ " : ; ? * | = ` を含まないもの。Standard の名前は変えられない
    pub name: Option<String>,
    /// 主のフォント。形のファイル（shape）か、太字・斜体・文字集合・ピッチを持たない TrueType のファイル（named で、名前が .ttf・.ttc・.otf で終わるもの）。名前を空にすると除く。XDATA の書体の名前で指すフォントは変えられない
    pub font: Option<Font>,
    /// 大きいフォント。形のファイル（shape）だけ。名前を空にすると除く
    pub big_font: Option<Font>,
    /// 決まった高さ。文書の単位。0 なら決めない。負の数は書けない
    pub height: Option<f64>,
    /// 幅の倍率。正の数
    pub width: Option<f64>,
    /// 傾き（度）。-85 から 85 まで
    pub oblique: Option<f64>,
    /// 左右を裏返す
    pub backward: Option<bool>,
    /// 上下を反転する
    pub upside_down: Option<bool>,
    /// 縦書き
    pub vertical: Option<bool>,
}

impl TextStyleEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "text_style")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        let name = object.optional("name", json::string)?;
        let font = object.optional("font", Font::from_json)?;
        let big_font = object.optional("big_font", Font::from_json)?;
        let height = object.optional("height", json::number)?;
        let width = object.optional("width", json::number)?;
        let oblique = object.optional("oblique", json::number)?;
        let backward = object.optional("backward", json::boolean)?;
        let upside_down = object.optional("upside_down", json::boolean)?;
        let vertical = object.optional("vertical", json::boolean)?;
        Ok(Self {
            id,
            hash,
            name,
            font,
            big_font,
            height,
            width,
            oblique,
            backward,
            upside_down,
            vertical,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("text_style");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.font {
            out.key("font");
            value.to_json(out);
        }
        if let Some(value) = &self.big_font {
            out.key("big_font");
            value.to_json(out);
        }
        if let Some(value) = &self.height {
            out.key("height");
            out.float(*value);
        }
        if let Some(value) = &self.width {
            out.key("width");
            out.float(*value);
        }
        if let Some(value) = &self.oblique {
            out.key("oblique");
            out.float(*value);
        }
        if let Some(value) = &self.backward {
            out.key("backward");
            out.bool(*value);
        }
        if let Some(value) = &self.upside_down {
            out.key("upside_down");
            out.bool(*value);
        }
        if let Some(value) = &self.vertical {
            out.key("vertical");
            out.bool(*value);
        }
    }
}

impl FromJson for TextStyleEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for TextStyleEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 文字スタイルを足す（決定 0163）。書かない欄は Standard と同じ。同じ apply の要素は、足した文字スタイルを指せない。小版 25 から
#[derive(Clone, Debug, PartialEq)]
pub struct AddTextStyleEdit {
    /// 名前。text_style の name と同じ決まり
    pub name: String,
    /// 主のフォント。形のファイル（shape）か、太字・斜体・文字集合・ピッチを持たない TrueType のファイル（named で、名前が .ttf・.ttc・.otf で終わるもの）。名前を空にすると除く。XDATA の書体の名前で指すフォントは変えられない
    pub font: Option<Font>,
    /// 大きいフォント。形のファイル（shape）だけ。名前を空にすると除く
    pub big_font: Option<Font>,
    /// 決まった高さ。文書の単位。0 なら決めない。負の数は書けない
    pub height: Option<f64>,
    /// 幅の倍率。正の数
    pub width: Option<f64>,
    /// 傾き（度）。-85 から 85 まで
    pub oblique: Option<f64>,
    /// 左右を裏返す
    pub backward: Option<bool>,
    /// 上下を反転する
    pub upside_down: Option<bool>,
    /// 縦書き
    pub vertical: Option<bool>,
}

impl AddTextStyleEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "add_text_style")?;
        let name = object.required("name", json::string)?;
        let font = object.optional("font", Font::from_json)?;
        let big_font = object.optional("big_font", Font::from_json)?;
        let height = object.optional("height", json::number)?;
        let width = object.optional("width", json::number)?;
        let oblique = object.optional("oblique", json::number)?;
        let backward = object.optional("backward", json::boolean)?;
        let upside_down = object.optional("upside_down", json::boolean)?;
        let vertical = object.optional("vertical", json::boolean)?;
        Ok(Self {
            name,
            font,
            big_font,
            height,
            width,
            oblique,
            backward,
            upside_down,
            vertical,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("add_text_style");
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.font {
            out.key("font");
            value.to_json(out);
        }
        if let Some(value) = &self.big_font {
            out.key("big_font");
            value.to_json(out);
        }
        if let Some(value) = &self.height {
            out.key("height");
            out.float(*value);
        }
        if let Some(value) = &self.width {
            out.key("width");
            out.float(*value);
        }
        if let Some(value) = &self.oblique {
            out.key("oblique");
            out.float(*value);
        }
        if let Some(value) = &self.backward {
            out.key("backward");
            out.bool(*value);
        }
        if let Some(value) = &self.upside_down {
            out.key("upside_down");
            out.bool(*value);
        }
        if let Some(value) = &self.vertical {
            out.key("vertical");
            out.bool(*value);
        }
    }
}

impl FromJson for AddTextStyleEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for AddTextStyleEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 文字スタイルを除く（決定 0163）。要素・表の書式や寸法スタイルなどのほかのオブジェクト・今の文字スタイル（$TEXTSTYLE）が指すものと、Standard・外部参照に依るものは除けない。同じ apply で足す要素が指すものも除けない。後ろにハンドルのない文字スタイルがあれば、その ID が変わるので除けない。小版 25 から
#[derive(Clone, Debug, PartialEq)]
pub struct RemoveTextStyleEdit {
    /// 除く文字スタイルの ID。tables が返したもの
    pub id: String,
    /// 読んだときの文字スタイルのハッシュ。tables が返したもの
    pub hash: String,
}

impl RemoveTextStyleEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "remove_text_style")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        Ok(Self { id, hash })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("remove_text_style");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
    }
}

impl FromJson for RemoveTextStyleEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RemoveTextStyleEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線種の名前・説明・線と間の長さの列のうち、書いたものだけを変える（決定 0164）。DXF は、名前を変えると、要素・レイヤと今の線種（$CELTYPE）も新しい名前で指す。多重線のスタイルなどの図形でないオブジェクトが名前で指す線種の名前は変えられない。形か文字を添える線種と DWG の線種は、列の要素の数を変えられない。小版 26 から
#[derive(Clone, Debug, PartialEq)]
pub struct LineTypeEdit {
    /// 変える線種の ID。tables が返したもの
    pub id: String,
    /// 読んだときの線種のハッシュ。tables が返したもの
    pub hash: String,
    /// 新しい名前。ほかの線種と重ならず（大文字と小文字を区別しない）、< > / \ " : ; ? * | = ` を含まないもの。ByLayer・ByBlock・Continuous と外部参照に依る線種の名前は変えられない
    pub name: Option<String>,
    /// 説明
    pub description: Option<String>,
    /// 線と間の長さの列。文書の単位。正は線、負は間、0 は点
    pub pattern: Option<Vec<f64>>,
}

impl LineTypeEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "line_type")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        let name = object.optional("name", json::string)?;
        let description = object.optional("description", json::string)?;
        let pattern = object.optional("pattern", |value| json::array(value, json::number))?;
        Ok(Self {
            id,
            hash,
            name,
            description,
            pattern,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("line_type");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.description {
            out.key("description");
            out.string(value);
        }
        if let Some(value) = &self.pattern {
            out.key("pattern");
            out.array(value, |out, item| out.float(*item));
        }
    }
}

impl FromJson for LineTypeEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LineTypeEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線種を足す（決定 0164）。書かない欄は Continuous と同じ。同じ apply の要素とレイヤは、足した線種を指せない。小版 26 から
#[derive(Clone, Debug, PartialEq)]
pub struct AddLineTypeEdit {
    /// 名前。line_type の name と同じ決まり
    pub name: String,
    /// 説明
    pub description: Option<String>,
    /// 線と間の長さの列。文書の単位。正は線、負は間、0 は点
    pub pattern: Option<Vec<f64>>,
}

impl AddLineTypeEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "add_line_type")?;
        let name = object.required("name", json::string)?;
        let description = object.optional("description", json::string)?;
        let pattern = object.optional("pattern", |value| json::array(value, json::number))?;
        Ok(Self {
            name,
            description,
            pattern,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("add_line_type");
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.description {
            out.key("description");
            out.string(value);
        }
        if let Some(value) = &self.pattern {
            out.key("pattern");
            out.array(value, |out, item| out.float(*item));
        }
    }
}

impl FromJson for AddLineTypeEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for AddLineTypeEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線種を除く（決定 0164）。要素・レイヤ・多重線のスタイルなどのほかのオブジェクト・今の線種（$CELTYPE）が指すものと、ByLayer・ByBlock・Continuous・外部参照に依るものは除けない。同じ apply で足す・変える要素とレイヤが指すものも除けない。後ろにハンドルのない線種があれば、その ID が変わるので除けない。小版 26 から
#[derive(Clone, Debug, PartialEq)]
pub struct RemoveLineTypeEdit {
    /// 除く線種の ID。tables が返したもの
    pub id: String,
    /// 読んだときの線種のハッシュ。tables が返したもの
    pub hash: String,
}

impl RemoveLineTypeEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "remove_line_type")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        Ok(Self { id, hash })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("remove_line_type");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
    }
}

impl FromJson for RemoveLineTypeEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RemoveLineTypeEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 寸法スタイルの名前・端部記号・その大きさ・斜線・倍率のうち、書いたものだけを変える（決定 0165）。DXF は、名前を変えると、寸法・引出線・公差と今の寸法スタイル（$DIMSTYLE）も新しい名前で指す。図形でないオブジェクトが名前で指す寸法スタイルの名前は変えられない。小版 27 から
#[derive(Clone, Debug, PartialEq)]
pub struct DimensionStyleEdit {
    /// 変える寸法スタイルの ID。tables が返したもの
    pub id: String,
    /// 読んだときの寸法スタイルのハッシュ。tables が返したもの
    pub hash: String,
    /// 新しい名前。ほかの寸法スタイルと重ならず（大文字と小文字を区別しない）、$ と < > / \ " : ; ? * | = ` を含まないもの。Standard・子の寸法スタイル（名前に $ を持つもの）・子を持つ寸法スタイル・外部参照に依る寸法スタイルの名前は変えられない
    pub name: Option<String>,
    /// 寸法線の 1 本目と 2 本目の補助線の側の端部記号。2 つのブロックの名前で、空の名前は既定の矢印（塗りつぶした閉じた矢印）。DXF は R2000 から変えられる。DWG の端部記号は変えられない
    pub ends: Option<Vec<String>>,
    /// 引出線の端部記号。ブロックの名前で、空の名前は既定の矢印。ends と同じ決まり
    pub leader: Option<String>,
    /// 端部記号の大きさ（DIMASZ）。文書の単位で、scale を掛ける前の値。負の数は書けない
    pub size: Option<f64>,
    /// 端部記号の代わりに描く斜線の大きさ（DIMTSZ）。0 なら斜線を描かない。負の数は書けない
    pub tick: Option<f64>,
    /// 大きさに掛ける倍率（DIMSCALE）。0 はビューの縮尺から決める。負の数は書けない
    pub scale: Option<f64>,
}

impl DimensionStyleEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "dimension_style")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        let name = object.optional("name", json::string)?;
        let ends = object.optional("ends", |value| json::array(value, json::string))?;
        let leader = object.optional("leader", json::string)?;
        let size = object.optional("size", json::number)?;
        let tick = object.optional("tick", json::number)?;
        let scale = object.optional("scale", json::number)?;
        Ok(Self {
            id,
            hash,
            name,
            ends,
            leader,
            size,
            tick,
            scale,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("dimension_style");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.ends {
            out.key("ends");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.leader {
            out.key("leader");
            out.string(value);
        }
        if let Some(value) = &self.size {
            out.key("size");
            out.float(*value);
        }
        if let Some(value) = &self.tick {
            out.key("tick");
            out.float(*value);
        }
        if let Some(value) = &self.scale {
            out.key("scale");
            out.float(*value);
        }
    }
}

impl FromJson for DimensionStyleEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DimensionStyleEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 寸法スタイルを足す（決定 0165）。書かない欄は Standard と同じ。同じ apply の要素は、足した寸法スタイルを指せない。小版 27 から
#[derive(Clone, Debug, PartialEq)]
pub struct AddDimensionStyleEdit {
    /// 名前。dimension_style の name と同じ決まり
    pub name: String,
    /// 寸法線の 1 本目と 2 本目の補助線の側の端部記号。2 つのブロックの名前で、空の名前は既定の矢印（塗りつぶした閉じた矢印）。DXF は R2000 から変えられる。DWG の端部記号は変えられない
    pub ends: Option<Vec<String>>,
    /// 引出線の端部記号。ブロックの名前で、空の名前は既定の矢印。ends と同じ決まり
    pub leader: Option<String>,
    /// 端部記号の大きさ（DIMASZ）。文書の単位で、scale を掛ける前の値。負の数は書けない
    pub size: Option<f64>,
    /// 端部記号の代わりに描く斜線の大きさ（DIMTSZ）。0 なら斜線を描かない。負の数は書けない
    pub tick: Option<f64>,
    /// 大きさに掛ける倍率（DIMSCALE）。0 はビューの縮尺から決める。負の数は書けない
    pub scale: Option<f64>,
}

impl AddDimensionStyleEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "add_dimension_style")?;
        let name = object.required("name", json::string)?;
        let ends = object.optional("ends", |value| json::array(value, json::string))?;
        let leader = object.optional("leader", json::string)?;
        let size = object.optional("size", json::number)?;
        let tick = object.optional("tick", json::number)?;
        let scale = object.optional("scale", json::number)?;
        Ok(Self {
            name,
            ends,
            leader,
            size,
            tick,
            scale,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("add_dimension_style");
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.ends {
            out.key("ends");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.leader {
            out.key("leader");
            out.string(value);
        }
        if let Some(value) = &self.size {
            out.key("size");
            out.float(*value);
        }
        if let Some(value) = &self.tick {
            out.key("tick");
            out.float(*value);
        }
        if let Some(value) = &self.scale {
            out.key("scale");
            out.float(*value);
        }
    }
}

impl FromJson for AddDimensionStyleEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for AddDimensionStyleEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 寸法スタイルを除く（決定 0165）。寸法・引出線・公差などのほかのオブジェクト・今の寸法スタイル（$DIMSTYLE）が指すものと、Standard・子の寸法スタイル・子を持つ寸法スタイル・外部参照に依るものは除けない。後ろにハンドルのない寸法スタイルがあれば、その ID が変わるので除けない。小版 27 から
#[derive(Clone, Debug, PartialEq)]
pub struct RemoveDimensionStyleEdit {
    /// 除く寸法スタイルの ID。tables が返したもの
    pub id: String,
    /// 読んだときの寸法スタイルのハッシュ。tables が返したもの
    pub hash: String,
}

impl RemoveDimensionStyleEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "remove_dimension_style")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        Ok(Self { id, hash })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("remove_dimension_style");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
    }
}

impl FromJson for RemoveDimensionStyleEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RemoveDimensionStyleEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 文書の点の表し方（marker を持たない点の見え方）を変える（決定 0190）。文書全体の設定なので要素を指さない。読んだときの文書は、apply の hash で確かめる。DXF・DWG は見出しの $PDMODE・$PDSIZE に書く。DWG は、変数を書く幅が変わる値（$PDMODE の 0 とほかの値の間の変更、$PDSIZE の 0・1 とほかの値の間の変更）を、小版 39 から書ける（決定 0193）。JWW・SXF は文書の点の表し方を持たないので断る。applied は id と hash を持たない。小版 38 から
#[derive(Clone, Debug, PartialEq)]
pub struct PointStyleEdit {
    /// 基本の形
    pub mark: PointMark,
    /// 円で囲む。書かなければ偽
    pub circle: Option<bool>,
    /// 四角で囲む。書かなければ偽
    pub square: Option<bool>,
    /// 大きさ。0 より大きい有限の数。意味は size_kind で決まる
    pub size: f64,
    /// 大きさの意味
    pub size_kind: PointSizeKind,
}

impl PointStyleEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "point_style")?;
        let mark = object.required("mark", PointMark::from_json)?;
        let circle = object.optional("circle", json::boolean)?;
        let square = object.optional("square", json::boolean)?;
        let size = object.required("size", json::number)?;
        let size_kind = object.required("size_kind", PointSizeKind::from_json)?;
        Ok(Self {
            mark,
            circle,
            square,
            size,
            size_kind,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("point_style");
        out.key("mark");
        self.mark.to_json(out);
        if let Some(value) = &self.circle {
            out.key("circle");
            out.bool(*value);
        }
        if let Some(value) = &self.square {
            out.key("square");
            out.bool(*value);
        }
        out.key("size");
        out.float(self.size);
        out.key("size_kind");
        self.size_kind.to_json(out);
    }
}

impl FromJson for PointStyleEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PointStyleEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 文書の点の基本の形。dot は小さな点、none はなし、plus は十字、cross はバツ、tick は上向きの短い線
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PointMark {
    /// `dot`
    Dot,
    /// `none`
    None,
    /// `plus`
    Plus,
    /// `cross`
    Cross,
    /// `tick`
    Tick,
}

impl PointMark {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Dot, Self::None, Self::Plus, Self::Cross, Self::Tick];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dot => "dot",
            Self::None => "none",
            Self::Plus => "plus",
            Self::Cross => "cross",
            Self::Tick => "tick",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "dot" => Some(Self::Dot),
            "none" => Some(Self::None),
            "plus" => Some(Self::Plus),
            "cross" => Some(Self::Cross),
            "tick" => Some(Self::Tick),
            _ => None,
        }
    }
}

impl FromJson for PointMark {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for PointMark {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 文書の点の大きさの意味。length は点のある空間の長さ、screen は画面の高さに対する百分率
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PointSizeKind {
    /// `length`
    Length,
    /// `screen`
    Screen,
}

impl PointSizeKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Length, Self::Screen];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Length => "length",
            Self::Screen => "screen",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "length" => Some(Self::Length),
            "screen" => Some(Self::Screen),
            _ => None,
        }
    }
}

impl FromJson for PointSizeKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for PointSizeKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// レイヤを足す（決定 0162）。書かない欄は、状態なら表示・印刷・編集でき凍結せず、色・線種・線幅ならレイヤ 0 と同じ。同じ apply の要素は、足したレイヤを指せない。小版 22 から
#[derive(Clone, Debug, PartialEq)]
pub struct AddLayerEdit {
    /// 名前。layer の name と同じ決まり
    pub name: String,
    /// 表示する。凍結するなら偽
    pub visible: Option<bool>,
    /// 印刷する
    pub printable: Option<bool>,
    /// 編集できる（ロックしない）
    pub editable: Option<bool>,
    /// 凍結する
    pub frozen: Option<bool>,
    /// 色。aci が要る。inherit は書けない
    pub color: Option<ColorChange>,
    /// 線種。name か id で指す。inherit は書けない
    pub line_type: Option<LineTypeChange>,
    /// 線幅。mm か default。inherit は書けない
    pub line_weight: Option<LineWeightChange>,
}

impl AddLayerEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "add_layer")?;
        let name = object.required("name", json::string)?;
        let visible = object.optional("visible", json::boolean)?;
        let printable = object.optional("printable", json::boolean)?;
        let editable = object.optional("editable", json::boolean)?;
        let frozen = object.optional("frozen", json::boolean)?;
        let color = object.optional("color", ColorChange::from_json)?;
        let line_type = object.optional("line_type", LineTypeChange::from_json)?;
        let line_weight = object.optional("line_weight", LineWeightChange::from_json)?;
        Ok(Self {
            name,
            visible,
            printable,
            editable,
            frozen,
            color,
            line_type,
            line_weight,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("add_layer");
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.visible {
            out.key("visible");
            out.bool(*value);
        }
        if let Some(value) = &self.printable {
            out.key("printable");
            out.bool(*value);
        }
        if let Some(value) = &self.editable {
            out.key("editable");
            out.bool(*value);
        }
        if let Some(value) = &self.frozen {
            out.key("frozen");
            out.bool(*value);
        }
        if let Some(value) = &self.color {
            out.key("color");
            value.to_json(out);
        }
        if let Some(value) = &self.line_type {
            out.key("line_type");
            value.to_json(out);
        }
        if let Some(value) = &self.line_weight {
            out.key("line_weight");
            value.to_json(out);
        }
    }
}

impl FromJson for AddLayerEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for AddLayerEdit {
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

/// 新しい色。inherit か、aci・sxf・jww・rgb の少なくとも 1 つのどちらかを書く。aci・sxf・jww は 1 つだけ書く
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColorChange {
    /// 継承する先
    pub inherit: Option<Inherit>,
    /// AutoCAD の色の番号（1〜255）
    pub aci: Option<u64>,
    /// SXF の色のコード。1〜16 は既定義色、17 からは利用者定義色を定義の順に数えたもの。小版 29 から
    pub sxf: Option<u64>,
    /// Jw_cad のペンの番号。0〜9 と、版 420 からの SXF の拡張色 100〜356。小版 30 から
    pub jww: Option<u64>,
    /// [赤, 緑, 青]。それぞれ 0〜255
    pub rgb: Option<Vec<u64>>,
}

impl ColorChange {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let inherit = object.optional("inherit", Inherit::from_json)?;
        let aci = object.optional("aci", |value| json::uint(value, 1, 255))?;
        let sxf = object.optional("sxf", |value| json::uint(value, 1, 9007199254740991))?;
        let jww = object.optional("jww", |value| json::uint(value, 0, 356))?;
        let rgb = object.optional("rgb", |value| {
            json::array(value, |value| json::uint(value, 0, 255))
        })?;
        Ok(Self {
            inherit,
            aci,
            sxf,
            jww,
            rgb,
        })
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
        if let Some(value) = &self.sxf {
            out.key("sxf");
            out.uint(*value);
        }
        if let Some(value) = &self.jww {
            out.key("jww");
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
    pub layers: Vec<LayerItem>,
    /// 線種。小版 26 から、定義を持つ
    pub line_types: Vec<LineTypeItem>,
    /// 寸法スタイル。小版 27 から、端部記号と大きさを持つ
    pub dimension_styles: Vec<DimensionStyleItem>,
    /// 文字スタイル（決定 0158）。要素が名前で指すだけで、ファイルに定義がないものは、定義の欄を持たない項目として後ろに並ぶ。小版 20 から
    pub text_styles: Vec<TextStyleItem>,
}

impl TablesResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let layers = object.required("layers", |value| json::array(value, LayerItem::from_json))?;
        let line_types = object.required("line_types", |value| {
            json::array(value, LineTypeItem::from_json)
        })?;
        let dimension_styles = object.required("dimension_styles", |value| {
            json::array(value, DimensionStyleItem::from_json)
        })?;
        let text_styles = object.required("text_styles", |value| {
            json::array(value, TextStyleItem::from_json)
        })?;
        Ok(Self {
            layers,
            line_types,
            dimension_styles,
            text_styles,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("layers");
        out.array(&self.layers, |out, item| item.to_json(out));
        out.key("line_types");
        out.array(&self.line_types, |out, item| item.to_json(out));
        out.key("dimension_styles");
        out.array(&self.dimension_styles, |out, item| item.to_json(out));
        out.key("text_styles");
        out.array(&self.text_styles, |out, item| item.to_json(out));
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

/// 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、2D のシーンを blob で返す（決定 0172）。シーンの配置は schema/scene.toml にある。小版 33 から
#[derive(Clone, Debug, PartialEq)]
pub struct SceneRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
    /// 文書の名前。open で作ったもの
    pub doc: String,
    /// 起点。実寸の空間・記号の定義の空間・用紙の ID（drawing と entities が返すもの）
    pub from: String,
    /// 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した空間の要素だけを描く。from が用紙のときだけ書ける
    pub view: Option<u64>,
    /// [x の最小, y の最小, x の最大, y の最大]。起点の座標の長方形。範囲の重なる塊だけを返す
    pub region: Option<Vec<f64>>,
    /// 弦の誤差（起点の長さ）。0 より大きい。書かなければ、用紙からは 0.02 mm、空間からは要素の範囲の対角線の 1/50000
    pub tolerance: Option<f64>,
    /// true なら印刷する要素を、書かないか false なら表示する要素を描く
    pub print: Option<bool>,
    /// 色の表の入力の名前（zm_push で渡したもの）。プロファイルの色の語彙と同じ形の JSON で、values に rgb（#rrggbb）と aci・sxf・jww の符号を並べる（決定 0138・0178）。書かなければ、ファイルが RGB を持たない色は未解決の印を立てる
    pub palette: Option<String>,
    /// 文字を置くフォント（決定 0188）。書けば、文字を字形で置き、シーンの字形の区画に形と変換を返す。文字スタイルと書式の区間のフォントの名前（シーンの font_names）で引き、同じ名前、なければ ASCII の大文字と小文字を区別しない名前で合わせる。合うものがなければ名前のないフォントで置く。小版 37 から
    pub fonts: Option<Vec<SceneFont>>,
    /// 形式の宣言値の高さ（キーは dxf.TEXT.40・dxf.MTEXT.40・jww.moji.size_y など）が、フォントのどの寸法に当たるか（決定 0176・0188）。書かないキーは大文字の高さとみなす。fonts を書いたときだけ使う。小版 37 から
    pub heights: Option<BTreeMap<String, HeightReference>>,
}

impl SceneRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "scene")?;
        let doc = object.required("doc", json::string)?;
        let from = object.required("from", json::string)?;
        let view = object.optional("view", |value| json::uint(value, 0, 9007199254740991))?;
        let region = object.optional("region", |value| json::array(value, json::number))?;
        let tolerance = object.optional("tolerance", json::number)?;
        let print = object.optional("print", json::boolean)?;
        let palette = object.optional("palette", json::string)?;
        let fonts = object.optional("fonts", |value| json::array(value, SceneFont::from_json))?;
        let heights = object.optional("heights", |value| {
            json::map(value, HeightReference::from_json)
        })?;
        Ok(Self {
            request_common,
            doc,
            from,
            view,
            region,
            tolerance,
            print,
            palette,
            fonts,
            heights,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("scene");
        out.key("doc");
        out.string(&self.doc);
        out.key("from");
        out.string(&self.from);
        if let Some(value) = &self.view {
            out.key("view");
            out.uint(*value);
        }
        if let Some(value) = &self.region {
            out.key("region");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.tolerance {
            out.key("tolerance");
            out.float(*value);
        }
        if let Some(value) = &self.print {
            out.key("print");
            out.bool(*value);
        }
        if let Some(value) = &self.palette {
            out.key("palette");
            out.string(value);
        }
        if let Some(value) = &self.fonts {
            out.key("fonts");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.heights {
            out.key("heights");
            out.map(value, |out, item| item.to_json(out));
        }
    }
}

impl FromJson for SceneRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SceneRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// シーンの文字を置くフォント（決定 0188）
#[derive(Clone, Debug, PartialEq)]
pub struct SceneFont {
    /// 文字が指すフォントの名前（ファイルに書かれた元の名前）。書かなければ、名前の合わない文字に使う既定のフォント。同じ名前と、名前のないものは 1 つまで
    pub name: Option<String>,
    /// TrueType・OpenType（CFF・CFF2 を含む）のファイルの入力の名前（zm_push で渡したもの）
    pub input: String,
    /// フォントのコレクション（TTC）の中の番号。書かなければ 0
    pub index: Option<u64>,
}

impl SceneFont {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.optional("name", json::string)?;
        let input = object.required("input", json::string)?;
        let index = object.optional("index", |value| json::uint(value, 0, 4294967295))?;
        Ok(Self { name, input, index })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        out.key("input");
        out.string(&self.input);
        if let Some(value) = &self.index {
            out.key("index");
            out.uint(*value);
        }
    }
}

impl FromJson for SceneFont {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SceneFont {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 宣言された高さが当たるフォントの寸法。cap_height は大文字の高さ、em は em、frame はアセンダーとディセンダーを合わせた高さ
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HeightReference {
    /// `cap_height`
    CapHeight,
    /// `em`
    Em,
    /// `frame`
    Frame,
}

impl HeightReference {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::CapHeight, Self::Em, Self::Frame];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::CapHeight => "cap_height",
            Self::Em => "em",
            Self::Frame => "frame",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "cap_height" => Some(Self::CapHeight),
            "em" => Some(Self::Em),
            "frame" => Some(Self::Frame),
            _ => None,
        }
    }
}

impl FromJson for HeightReference {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for HeightReference {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// シーンの要約と、シーンの blob の番号
#[derive(Clone, Debug, PartialEq)]
pub struct SceneResult {
    /// 起点の ID
    pub from: String,
    /// 起点のビューの番号。ビューでなければ持たない
    pub view: Option<u64>,
    /// 起点の座標の単位。用紙とビューからは millimeter
    pub unit: Unit,
    /// 使った弦の誤差（起点の長さ）
    pub tolerance: f64,
    /// 返した描くものの数
    pub counts: SceneCounts,
    /// 返した塊の範囲。起点の座標。塊がなければ持たない
    pub extent: Option<Extent>,
    /// シーンの blob の番号（種類は scene）。zm_take で取り出す
    pub blob: u64,
}

impl SceneResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let from = object.required("from", json::string)?;
        let view = object.optional("view", |value| json::uint(value, 0, 9007199254740991))?;
        let unit = object.required("unit", Unit::from_json)?;
        let tolerance = object.required("tolerance", json::number)?;
        let counts = object.required("counts", SceneCounts::from_json)?;
        let extent = object.optional("extent", Extent::from_json)?;
        let blob = object.required("blob", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            from,
            view,
            unit,
            tolerance,
            counts,
            extent,
            blob,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("from");
        out.string(&self.from);
        if let Some(value) = &self.view {
            out.key("view");
            out.uint(*value);
        }
        out.key("unit");
        self.unit.to_json(out);
        out.key("tolerance");
        out.float(self.tolerance);
        out.key("counts");
        self.counts.to_json(out);
        if let Some(value) = &self.extent {
            out.key("extent");
            value.to_json(out);
        }
        out.key("blob");
        out.uint(self.blob);
    }
}

impl FromJson for SceneResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SceneResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// シーンの描くものの種類ごとの数
#[derive(Clone, Debug, PartialEq)]
pub struct SceneCounts {
    /// 塊
    pub chunks: u64,
    /// 線（折れ線）
    pub lines: u64,
    /// 線と面の頂点
    pub vertices: u64,
    /// 面
    pub faces: u64,
    /// 文字
    pub texts: u64,
    /// 点
    pub points: u64,
    /// 画像
    pub images: u64,
    /// 置いた字形。fonts を書かなければ 0。小版 37 から
    pub glyphs: u64,
}

impl SceneCounts {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let chunks = object.required("chunks", |value| json::uint(value, 0, 9007199254740991))?;
        let lines = object.required("lines", |value| json::uint(value, 0, 9007199254740991))?;
        let vertices =
            object.required("vertices", |value| json::uint(value, 0, 9007199254740991))?;
        let faces = object.required("faces", |value| json::uint(value, 0, 9007199254740991))?;
        let texts = object.required("texts", |value| json::uint(value, 0, 9007199254740991))?;
        let points = object.required("points", |value| json::uint(value, 0, 9007199254740991))?;
        let images = object.required("images", |value| json::uint(value, 0, 9007199254740991))?;
        let glyphs = object.required("glyphs", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            chunks,
            lines,
            vertices,
            faces,
            texts,
            points,
            images,
            glyphs,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("chunks");
        out.uint(self.chunks);
        out.key("lines");
        out.uint(self.lines);
        out.key("vertices");
        out.uint(self.vertices);
        out.key("faces");
        out.uint(self.faces);
        out.key("texts");
        out.uint(self.texts);
        out.key("points");
        out.uint(self.points);
        out.key("images");
        out.uint(self.images);
        out.key("glyphs");
        out.uint(self.glyphs);
    }
}

impl FromJson for SceneCounts {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SceneCounts {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、プロファイルの規則と語彙で照合する（決定 0138・0173・0179）。小版 34 から
#[derive(Clone, Debug, PartialEq)]
pub struct CheckRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
    /// 文書の名前。open で作ったもの
    pub doc: String,
    /// プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す
    pub profiles: Vec<String>,
    /// 照合に使うプロファイル。profiles の中から選ぶ
    pub profile: ProfileRef,
    /// たどる起点。実寸の空間・記号の定義の空間・用紙の ID。書かなければ、すべての用紙から、用紙がなければ実寸の空間からたどる
    pub from: Option<String>,
    /// 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した要素と、そのビューの縮尺だけを照合する。from が用紙のときだけ書ける
    pub view: Option<u64>,
    /// 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。小版 40 から
    pub file_name: Option<String>,
    /// 返す最初の指摘の位置。前の応答の next_offset を渡す。書かなければ 0
    pub offset: Option<u64>,
    /// 返す指摘の数の上限。書かなければ 1000
    pub limit: Option<u64>,
    /// 色の表の入力の名前（zm_push で渡したもの）。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く
    pub palette: Option<String>,
}

impl CheckRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "check")?;
        let doc = object.required("doc", json::string)?;
        let profiles = object.required("profiles", |value| json::array(value, json::string))?;
        let profile = object.required("profile", ProfileRef::from_json)?;
        let from = object.optional("from", json::string)?;
        let view = object.optional("view", |value| json::uint(value, 0, 9007199254740991))?;
        let file_name = object.optional("file_name", json::string)?;
        let offset = object.optional("offset", |value| json::uint(value, 0, 9007199254740991))?;
        let limit = object.optional("limit", |value| json::uint(value, 1, 10000))?;
        let palette = object.optional("palette", json::string)?;
        Ok(Self {
            request_common,
            doc,
            profiles,
            profile,
            from,
            view,
            file_name,
            offset,
            limit,
            palette,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("check");
        out.key("doc");
        out.string(&self.doc);
        out.key("profiles");
        out.array(&self.profiles, |out, item| out.string(item));
        out.key("profile");
        self.profile.to_json(out);
        if let Some(value) = &self.from {
            out.key("from");
            out.string(value);
        }
        if let Some(value) = &self.view {
            out.key("view");
            out.uint(*value);
        }
        if let Some(value) = &self.file_name {
            out.key("file_name");
            out.string(value);
        }
        if let Some(value) = &self.offset {
            out.key("offset");
            out.uint(*value);
        }
        if let Some(value) = &self.limit {
            out.key("limit");
            out.uint(*value);
        }
        if let Some(value) = &self.palette {
            out.key("palette");
            out.string(value);
        }
    }
}

impl FromJson for CheckRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CheckRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// プロファイルの識別（決定 0138）
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileRef {
    /// 基準の系統（mlit-cad など）
    pub id: String,
    /// 基準の版（2025-12 など）
    pub edition: String,
}

impl ProfileRef {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let edition = object.required("edition", json::string)?;
        Ok(Self { id, edition })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("edition");
        out.string(&self.edition);
    }
}

impl FromJson for ProfileRef {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ProfileRef {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 照合の結果。指摘は offset から limit 個まで
#[derive(Clone, Debug, PartialEq)]
pub struct CheckResult {
    /// 照合に使ったプロファイルの鎖。extends の基から順
    pub chain: Vec<ProfileIdentity>,
    /// 照合した規則ごとの数。プロファイルの順。使えない規則は持たず、profile.rule.unsupported の診断で返す
    pub rules: Vec<CheckRule>,
    /// 指摘。規則の順に、規則ごとにたどった順
    pub findings: Vec<CheckFinding>,
    /// 全体の指摘の数
    pub total: u64,
    /// 続きの最初の指摘の位置。続きがなければ持たない
    pub next_offset: Option<u64>,
}

impl CheckResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let chain = object.required("chain", |value| {
            json::array(value, ProfileIdentity::from_json)
        })?;
        let rules = object.required("rules", |value| json::array(value, CheckRule::from_json))?;
        let findings = object.required("findings", |value| {
            json::array(value, CheckFinding::from_json)
        })?;
        let total = object.required("total", |value| json::uint(value, 0, 9007199254740991))?;
        let next_offset = object.optional("next_offset", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            chain,
            rules,
            findings,
            total,
            next_offset,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("chain");
        out.array(&self.chain, |out, item| item.to_json(out));
        out.key("rules");
        out.array(&self.rules, |out, item| item.to_json(out));
        out.key("findings");
        out.array(&self.findings, |out, item| item.to_json(out));
        out.key("total");
        out.uint(self.total);
        if let Some(value) = &self.next_offset {
            out.key("next_offset");
            out.uint(*value);
        }
    }
}

impl FromJson for CheckResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CheckResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 照合に使ったプロファイルの識別と、データの直し
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileIdentity {
    /// 基準の系統
    pub id: String,
    /// 基準の版
    pub edition: String,
    /// このデータの直し。1 から
    pub revision: u64,
}

impl ProfileIdentity {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let edition = object.required("edition", json::string)?;
        let revision =
            object.required("revision", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            id,
            edition,
            revision,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("edition");
        out.string(&self.edition);
        out.key("revision");
        out.uint(self.revision);
    }
}

impl FromJson for ProfileIdentity {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ProfileIdentity {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 規則ごとの数
#[derive(Clone, Debug, PartialEq)]
pub struct CheckRule {
    /// 規則の id
    pub id: String,
    /// 照合の種類（layer-name など）
    pub check: String,
    /// 指摘の数。照合できなかったものを除く
    pub findings: u64,
    /// 照合できなかった数
    pub unsupported: u64,
}

impl CheckRule {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let check = object.required("check", json::string)?;
        let findings =
            object.required("findings", |value| json::uint(value, 0, 9007199254740991))?;
        let unsupported = object.required("unsupported", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            id,
            check,
            findings,
            unsupported,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("check");
        out.string(&self.check);
        out.key("findings");
        out.uint(self.findings);
        out.key("unsupported");
        out.uint(self.unsupported);
    }
}

impl FromJson for CheckRule {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CheckRule {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 指摘。diagnostic は診断と同じ形で、describe で文にできる。severity は規則の level で、照合できない値（profile.<種類>.unsupported）は info。count は 1
#[derive(Clone, Debug, PartialEq)]
pub struct CheckFinding {
    /// 規則の id
    pub rule: String,
    /// code・重要度・場所（要素・実効のレイヤ・用紙の ID）・見つけた値
    pub diagnostic: Diagnostic,
}

impl CheckFinding {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let rule = object.required("rule", json::string)?;
        let diagnostic = object.required("diagnostic", Diagnostic::from_json)?;
        Ok(Self { rule, diagnostic })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("rule");
        out.string(&self.rule);
        out.key("diagnostic");
        self.diagnostic.to_json(out);
    }
}

impl FromJson for CheckFinding {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CheckFinding {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// zm_push で渡した IFC（IFC-SPF か ifcZIP）を、zm_push で渡した EXPRESS のスキーマに照らしてチェックする（決定 0207・0216）。インスタンスの実体の名前・抽象・属性の数・値の型・省略を見る。WHERE・UNIQUE・RULE の規則は評価せず、数を返す。zumen は EXPRESS を同梱しない。小版 42 から
#[derive(Clone, Debug, PartialEq)]
pub struct ValidateRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
    /// IFC の入力の名前（zm_push で渡した .ifc か .ifczip）
    pub input: String,
    /// EXPRESS の入力の名前（zm_push で渡した .exp）
    pub express: String,
    /// 返す最初の指摘の位置。前の応答の next_offset を渡す。書かなければ 0
    pub offset: Option<u64>,
    /// 返す指摘の数の上限。書かなければ 1000
    pub limit: Option<u64>,
}

impl ValidateRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "validate")?;
        let input = object.required("input", json::string)?;
        let express = object.required("express", json::string)?;
        let offset = object.optional("offset", |value| json::uint(value, 0, 9007199254740991))?;
        let limit = object.optional("limit", |value| json::uint(value, 1, 10000))?;
        Ok(Self {
            request_common,
            input,
            express,
            offset,
            limit,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("validate");
        out.key("input");
        out.string(&self.input);
        out.key("express");
        out.string(&self.express);
        if let Some(value) = &self.offset {
            out.key("offset");
            out.uint(*value);
        }
        if let Some(value) = &self.limit {
            out.key("limit");
            out.uint(*value);
        }
    }
}

impl FromJson for ValidateRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ValidateRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// スキーマに照らしたチェックの結果。指摘は offset から limit 個まで。スキーマの名前の違いと評価しない規則は、診断で返す
#[derive(Clone, Debug, PartialEq)]
pub struct ValidateResult {
    /// EXPRESS のスキーマの名前（大文字）
    pub schema: String,
    /// IFC の FILE_SCHEMA の名前
    pub file_schema: String,
    /// EXPRESS のバイト列のハッシュ（v2: と 32 桁の 16 進）。どの EXPRESS でチェックしたかを残す
    pub express_hash: String,
    /// 評価しなかった規則（WHERE・UNIQUE・RULE）の数。確認できなかった項目
    pub rules_not_evaluated: u64,
    /// 指摘。インスタンスの番号の順に、属性の順。診断と同じ形で、describe で文にできる。count は 1
    pub findings: Vec<Diagnostic>,
    /// 全体の指摘の数
    pub total: u64,
    /// 続きの最初の指摘の位置。続きがなければ持たない
    pub next_offset: Option<u64>,
}

impl ValidateResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let schema = object.required("schema", json::string)?;
        let file_schema = object.required("file_schema", json::string)?;
        let express_hash = object.required("express_hash", json::string)?;
        let rules_not_evaluated = object.required("rules_not_evaluated", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        let findings = object.required("findings", |value| {
            json::array(value, Diagnostic::from_json)
        })?;
        let total = object.required("total", |value| json::uint(value, 0, 9007199254740991))?;
        let next_offset = object.optional("next_offset", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            schema,
            file_schema,
            express_hash,
            rules_not_evaluated,
            findings,
            total,
            next_offset,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("schema");
        out.string(&self.schema);
        out.key("file_schema");
        out.string(&self.file_schema);
        out.key("express_hash");
        out.string(&self.express_hash);
        out.key("rules_not_evaluated");
        out.uint(self.rules_not_evaluated);
        out.key("findings");
        out.array(&self.findings, |out, item| item.to_json(out));
        out.key("total");
        out.uint(self.total);
        if let Some(value) = &self.next_offset {
            out.key("next_offset");
            out.uint(*value);
        }
    }
}

impl FromJson for ValidateResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ValidateResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// レビューの文書（決定 0214・0218）の 1 つの対象を、開いた図面の文書の版へ移す。要素の参照を図面の要素の ID で引き、参照ごとに unchanged・changed・removed を返す。場所と図形に元の版のハッシュと状態を書き、対象のハッシュを文書のハッシュにする。2D の点は動かさない。移した文書は blobs の file で返す。小版 43 から
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewCarryRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
    /// 新しい版の文書の名前。open で作ったもの。小版 44 から、IFC の建物モデルの文書も渡せ、要素の参照を建物モデルの要素の ID とハッシュで引く
    pub doc: String,
    /// レビューの文書の入力の名前（zm_push で渡した JSON）
    pub review: String,
    /// 移す対象の ID。レビューの文書の targets の id
    pub target: String,
    /// 返す最初の参照の位置。前の応答の next_offset を渡す。書かなければ 0
    pub offset: Option<u64>,
    /// 返す参照の数の上限。書かなければ 1000
    pub limit: Option<u64>,
}

impl ReviewCarryRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "review_carry")?;
        let doc = object.required("doc", json::string)?;
        let review = object.required("review", json::string)?;
        let target = object.required("target", json::string)?;
        let offset = object.optional("offset", |value| json::uint(value, 0, 9007199254740991))?;
        let limit = object.optional("limit", |value| json::uint(value, 1, 10000))?;
        Ok(Self {
            request_common,
            doc,
            review,
            target,
            offset,
            limit,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("review_carry");
        out.key("doc");
        out.string(&self.doc);
        out.key("review");
        out.string(&self.review);
        out.key("target");
        out.string(&self.target);
        if let Some(value) = &self.offset {
            out.key("offset");
            out.uint(*value);
        }
        if let Some(value) = &self.limit {
            out.key("limit");
            out.uint(*value);
        }
    }
}

impl FromJson for ReviewCarryRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ReviewCarryRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 移した結果。参照は offset から limit 個まで。移した文書は blobs の file で返す
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewCarryResult {
    /// 移す前の対象のハッシュ
    pub from: String,
    /// 移した後の対象のハッシュ。文書のハッシュ
    pub hash: String,
    /// 要素の参照ごとの状態。指摘の順に、場所の順、参照の順
    pub references: Vec<CarriedReference>,
    /// 全体の参照の数
    pub total: u64,
    /// 続きの最初の参照の位置。続きがなければ持たない
    pub next_offset: Option<u64>,
}

impl ReviewCarryResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let from = object.required("from", json::string)?;
        let hash = object.required("hash", json::string)?;
        let references = object.required("references", |value| {
            json::array(value, CarriedReference::from_json)
        })?;
        let total = object.required("total", |value| json::uint(value, 0, 9007199254740991))?;
        let next_offset = object.optional("next_offset", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            from,
            hash,
            references,
            total,
            next_offset,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("from");
        out.string(&self.from);
        out.key("hash");
        out.string(&self.hash);
        out.key("references");
        out.array(&self.references, |out, item| item.to_json(out));
        out.key("total");
        out.uint(self.total);
        if let Some(value) = &self.next_offset {
            out.key("next_offset");
            out.uint(*value);
        }
    }
}

impl FromJson for ReviewCarryResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ReviewCarryResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 移した要素の参照
#[derive(Clone, Debug, PartialEq)]
pub struct CarriedReference {
    /// 指摘の ID
    pub topic: String,
    /// 場所の ID
    pub location: String,
    /// 要素の ID
    pub element: String,
    /// 状態
    pub state: CarriedState,
}

impl CarriedReference {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let topic = object.required("topic", json::string)?;
        let location = object.required("location", json::string)?;
        let element = object.required("element", json::string)?;
        let state = object.required("state", CarriedState::from_json)?;
        Ok(Self {
            topic,
            location,
            element,
            state,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("topic");
        out.string(&self.topic);
        out.key("location");
        out.string(&self.location);
        out.key("element");
        out.string(&self.element);
        out.key("state");
        self.state.to_json(out);
    }
}

impl FromJson for CarriedReference {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CarriedReference {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 移したときの状態（決定 0214）。unchanged は要素のハッシュが同じ、changed は違うか元のハッシュがない、removed は新しい版に要素がない
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CarriedState {
    /// `unchanged`
    Unchanged,
    /// `changed`
    Changed,
    /// `removed`
    Removed,
}

impl CarriedState {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Unchanged, Self::Changed, Self::Removed];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Unchanged => "unchanged",
            Self::Changed => "changed",
            Self::Removed => "removed",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "unchanged" => Some(Self::Unchanged),
            "changed" => Some(Self::Changed),
            "removed" => Some(Self::Removed),
            _ => None,
        }
    }
}

impl FromJson for CarriedState {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for CarriedState {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 開いた文書を check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0214・0218）に足す。指摘の ID は対象のハッシュ・規則・診断から決め、取り込み済みの指摘は足さない。足した文書は blobs の file で返す。小版 43 から
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewAdoptRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
    /// 照合する文書の名前。open で作ったもの
    pub doc: String,
    /// プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す
    pub profiles: Vec<String>,
    /// 照合に使うプロファイル。profiles の中から選ぶ
    pub profile: ProfileRef,
    /// たどる起点。実寸の空間・記号の定義の空間・用紙の ID。書かなければ、すべての用紙から、用紙がなければ実寸の空間からたどる
    pub from: Option<String>,
    /// 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した要素と、そのビューの縮尺だけを照合する。from が用紙のときだけ書ける
    pub view: Option<u64>,
    /// 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。小版 40 から
    pub file_name: Option<String>,
    /// 色の表の入力の名前（zm_push で渡したもの）。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く
    pub palette: Option<String>,
    /// 足す先のレビューの文書の入力の名前（zm_push で渡した JSON）。書かなければ空の文書に足す
    pub review: Option<String>,
    /// 指摘が指す対象の ID。文書になければ、文書の形式とハッシュで足す
    pub target: String,
    /// 取り込む指摘の番号の列。check の指摘の並びの順で、0 から数える
    pub findings: Vec<u64>,
}

impl ReviewAdoptRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "review_adopt")?;
        let doc = object.required("doc", json::string)?;
        let profiles = object.required("profiles", |value| json::array(value, json::string))?;
        let profile = object.required("profile", ProfileRef::from_json)?;
        let from = object.optional("from", json::string)?;
        let view = object.optional("view", |value| json::uint(value, 0, 9007199254740991))?;
        let file_name = object.optional("file_name", json::string)?;
        let palette = object.optional("palette", json::string)?;
        let review = object.optional("review", json::string)?;
        let target = object.required("target", json::string)?;
        let findings = object.required("findings", |value| {
            json::array(value, |value| json::uint(value, 0, 9007199254740991))
        })?;
        Ok(Self {
            request_common,
            doc,
            profiles,
            profile,
            from,
            view,
            file_name,
            palette,
            review,
            target,
            findings,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("review_adopt");
        out.key("doc");
        out.string(&self.doc);
        out.key("profiles");
        out.array(&self.profiles, |out, item| out.string(item));
        out.key("profile");
        self.profile.to_json(out);
        if let Some(value) = &self.from {
            out.key("from");
            out.string(value);
        }
        if let Some(value) = &self.view {
            out.key("view");
            out.uint(*value);
        }
        if let Some(value) = &self.file_name {
            out.key("file_name");
            out.string(value);
        }
        if let Some(value) = &self.palette {
            out.key("palette");
            out.string(value);
        }
        if let Some(value) = &self.review {
            out.key("review");
            out.string(value);
        }
        out.key("target");
        out.string(&self.target);
        out.key("findings");
        out.array(&self.findings, |out, item| out.uint(*item));
    }
}

impl FromJson for ReviewAdoptRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ReviewAdoptRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 取り込んだ結果。足した文書は blobs の file で返す
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewAdoptResult {
    /// 足した指摘の ID。取り込み済みで足さなかった指摘は持たない
    pub added: Vec<String>,
    /// 足した後の文書の指摘の数
    pub topics: u64,
}

impl ReviewAdoptResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let added = object.required("added", |value| json::array(value, json::string))?;
        let topics = object.required("topics", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self { added, topics })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("added");
        out.array(&self.added, |out, item| out.string(item));
        out.key("topics");
        out.uint(self.topics);
    }
}

impl FromJson for ReviewAdoptResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ReviewAdoptResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開いた IFC の建物モデル（決定 0207・0219）の要約を返す。小版 44 から
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
    /// 文書の名前。open で IFC を開いたもの
    pub doc: String,
}

impl BuildingRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "building")?;
        let doc = object.required("doc", json::string)?;
        Ok(Self {
            request_common,
            doc,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("building");
        out.key("doc");
        out.string(&self.doc);
    }
}

impl FromJson for BuildingRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 建物モデルの要約
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingResult {
    /// IFC のスキーマの名前（FILE_SCHEMA の最初）
    pub schema: String,
    /// 入力のバイト列のハッシュ。review_carry の対象の版に使う
    pub hash: String,
    /// プロジェクトの要素の ID。なければ持たない
    pub project: Option<String>,
    /// プロジェクトの名前。なければ持たない
    pub project_name: Option<String>,
    /// プロジェクトの単位
    pub units: Vec<BuildingUnit>,
    /// 要素の数
    pub elements: u64,
    /// クラスごとの要素の数。クラスの名前の順
    pub classes: Vec<ClassCount>,
}

impl BuildingResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let schema = object.required("schema", json::string)?;
        let hash = object.required("hash", json::string)?;
        let project = object.optional("project", json::string)?;
        let project_name = object.optional("project_name", json::string)?;
        let units =
            object.required("units", |value| json::array(value, BuildingUnit::from_json))?;
        let elements =
            object.required("elements", |value| json::uint(value, 0, 9007199254740991))?;
        let classes =
            object.required("classes", |value| json::array(value, ClassCount::from_json))?;
        Ok(Self {
            schema,
            hash,
            project,
            project_name,
            units,
            elements,
            classes,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("schema");
        out.string(&self.schema);
        out.key("hash");
        out.string(&self.hash);
        if let Some(value) = &self.project {
            out.key("project");
            out.string(value);
        }
        if let Some(value) = &self.project_name {
            out.key("project_name");
            out.string(value);
        }
        out.key("units");
        out.array(&self.units, |out, item| item.to_json(out));
        out.key("elements");
        out.uint(self.elements);
        out.key("classes");
        out.array(&self.classes, |out, item| item.to_json(out));
    }
}

impl FromJson for BuildingResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 単位
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingUnit {
    /// 単位の種類（LENGTHUNIT・AREAUNIT など）
    pub kind: String,
    /// SI の接頭辞（MILLI など）
    pub prefix: Option<String>,
    /// 単位の名前（METRE、換算する単位なら inch など）
    pub name: String,
    /// 換算する単位の、SI の単位での大きさ。SI の単位は持たない
    pub scale: Option<f64>,
}

impl BuildingUnit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let kind = object.required("kind", json::string)?;
        let prefix = object.optional("prefix", json::string)?;
        let name = object.required("name", json::string)?;
        let scale = object.optional("scale", json::number)?;
        Ok(Self {
            kind,
            prefix,
            name,
            scale,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string(&self.kind);
        if let Some(value) = &self.prefix {
            out.key("prefix");
            out.string(value);
        }
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.scale {
            out.key("scale");
            out.float(*value);
        }
    }
}

impl FromJson for BuildingUnit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingUnit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// クラスの要素の数
#[derive(Clone, Debug, PartialEq)]
pub struct ClassCount {
    /// IFC のクラスの名前（IfcWall など）
    pub name: String,
    /// 要素の数
    pub count: u64,
}

impl ClassCount {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.required("name", json::string)?;
        let count = object.required("count", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self { name, count })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("name");
        out.string(&self.name);
        out.key("count");
        out.uint(self.count);
    }
}

impl FromJson for ClassCount {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ClassCount {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開いた IFC の建物モデルの要素を、書いた順に返す。成分は components で選ぶ（決定 0207・0219）。小版 44 から
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingElementsRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
    /// 文書の名前。open で IFC を開いたもの
    pub doc: String,
    /// 返す要素の ID。書けば、その要素だけを、書いた順に返す。ない ID は返さない
    pub ids: Option<Vec<String>>,
    /// IFC のクラスの名前。書けば、そのクラスの要素だけを返す。大文字と小文字を区別しない
    pub class: Option<String>,
    /// 返す成分の名前。1 つ以上で、重ねない。書かなければ class と identity を返す
    pub components: Option<Vec<BuildingComponent>>,
    /// 返す最初の位置。前の応答の next_offset を渡す。書かなければ 0
    pub offset: Option<u64>,
    /// 返す数の上限。書かなければ 1000
    pub limit: Option<u64>,
}

impl BuildingElementsRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "building_elements")?;
        let doc = object.required("doc", json::string)?;
        let ids = object.optional("ids", |value| json::array(value, json::string))?;
        let class = object.optional("class", json::string)?;
        let components = object.optional("components", |value| {
            json::array(value, BuildingComponent::from_json)
        })?;
        let offset = object.optional("offset", |value| json::uint(value, 0, 9007199254740991))?;
        let limit = object.optional("limit", |value| json::uint(value, 1, 10000))?;
        Ok(Self {
            request_common,
            doc,
            ids,
            class,
            components,
            offset,
            limit,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("building_elements");
        out.key("doc");
        out.string(&self.doc);
        if let Some(value) = &self.ids {
            out.key("ids");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.class {
            out.key("class");
            out.string(value);
        }
        if let Some(value) = &self.components {
            out.key("components");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.offset {
            out.key("offset");
            out.uint(*value);
        }
        if let Some(value) = &self.limit {
            out.key("limit");
            out.uint(*value);
        }
    }
}

impl FromJson for BuildingElementsRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingElementsRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// building_elements で返す成分（決定 0207）。relations は索引から求める子・参照・型の実例・開口・埋める要素・構成員
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BuildingComponent {
    /// `identity`
    Identity,
    /// `class`
    Class,
    /// `classification`
    Classification,
    /// `spatial`
    Spatial,
    /// `parent`
    Parent,
    /// `placement`
    Placement,
    /// `type_of`
    TypeOf,
    /// `properties`
    Properties,
    /// `quantities`
    Quantities,
    /// `material`
    Material,
    /// `opening`
    Opening,
    /// `group`
    Group,
    /// `connection`
    Connection,
    /// `level`
    Level,
    /// `zone`
    Zone,
    /// `style`
    Style,
    /// `relations`
    Relations,
}

impl BuildingComponent {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Identity,
        Self::Class,
        Self::Classification,
        Self::Spatial,
        Self::Parent,
        Self::Placement,
        Self::TypeOf,
        Self::Properties,
        Self::Quantities,
        Self::Material,
        Self::Opening,
        Self::Group,
        Self::Connection,
        Self::Level,
        Self::Zone,
        Self::Style,
        Self::Relations,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Class => "class",
            Self::Classification => "classification",
            Self::Spatial => "spatial",
            Self::Parent => "parent",
            Self::Placement => "placement",
            Self::TypeOf => "type_of",
            Self::Properties => "properties",
            Self::Quantities => "quantities",
            Self::Material => "material",
            Self::Opening => "opening",
            Self::Group => "group",
            Self::Connection => "connection",
            Self::Level => "level",
            Self::Zone => "zone",
            Self::Style => "style",
            Self::Relations => "relations",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "identity" => Some(Self::Identity),
            "class" => Some(Self::Class),
            "classification" => Some(Self::Classification),
            "spatial" => Some(Self::Spatial),
            "parent" => Some(Self::Parent),
            "placement" => Some(Self::Placement),
            "type_of" => Some(Self::TypeOf),
            "properties" => Some(Self::Properties),
            "quantities" => Some(Self::Quantities),
            "material" => Some(Self::Material),
            "opening" => Some(Self::Opening),
            "group" => Some(Self::Group),
            "connection" => Some(Self::Connection),
            "level" => Some(Self::Level),
            "zone" => Some(Self::Zone),
            "style" => Some(Self::Style),
            "relations" => Some(Self::Relations),
            _ => None,
        }
    }
}

impl FromJson for BuildingComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for BuildingComponent {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 要素の一覧。offset から limit 個まで
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingElementsResult {
    /// 要素
    pub elements: Vec<BuildingElement>,
    /// 選んだ要素の数
    pub total: u64,
    /// 続きの最初の位置。続きがなければ持たない
    pub next_offset: Option<u64>,
}

impl BuildingElementsResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let elements = object.required("elements", |value| {
            json::array(value, BuildingElement::from_json)
        })?;
        let total = object.required("total", |value| json::uint(value, 0, 9007199254740991))?;
        let next_offset = object.optional("next_offset", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            elements,
            total,
            next_offset,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("elements");
        out.array(&self.elements, |out, item| item.to_json(out));
        out.key("total");
        out.uint(self.total);
        if let Some(value) = &self.next_offset {
            out.key("next_offset");
            out.uint(*value);
        }
    }
}

impl FromJson for BuildingElementsResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingElementsResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 建物モデルの要素。成分は、選んだもののうち要素が持つものだけを持つ
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingElement {
    /// 要素の ID（GlobalId か、i と番号）
    pub id: String,
    /// 要素の成分のハッシュ（v1: と 32 桁の 16 進、決定 0207）
    pub hash: String,
    /// 名前・説明・タグ・長い名前
    pub identity: Option<IdentityComponent>,
    /// クラス
    pub class: Option<ClassComponent>,
    /// 分類の参照
    pub classification: Option<Vec<ClassificationReference>>,
    /// 空間の要素の種類と高さ
    pub spatial: Option<SpatialComponent>,
    /// 親と、参照される空間
    pub parent: Option<ParentComponent>,
    /// 配置
    pub placement: Option<PlacementComponent>,
    /// 型の要素の ID
    pub type_of: Option<String>,
    /// プロパティのセット
    pub properties: Option<Vec<PropertySet>>,
    /// 数量のセット
    pub quantities: Option<Vec<QuantitySet>>,
    /// 材料
    pub material: Option<MaterialComponent>,
    /// 開口と、埋める要素
    pub opening: Option<OpeningComponent>,
    /// 属するグループの要素の ID
    pub group: Option<Vec<String>>,
    /// 接続
    pub connection: Option<Vec<ConnectionLink>>,
    /// レベル
    pub level: Option<LevelComponent>,
    /// 区画
    pub zone: Option<ZoneComponent>,
    /// 表示のスタイル
    pub style: Option<StyleComponent>,
    /// 索引から求める関係
    pub relations: Option<RelationsComponent>,
}

impl BuildingElement {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        let identity = object.optional("identity", IdentityComponent::from_json)?;
        let class = object.optional("class", ClassComponent::from_json)?;
        let classification = object.optional("classification", |value| {
            json::array(value, ClassificationReference::from_json)
        })?;
        let spatial = object.optional("spatial", SpatialComponent::from_json)?;
        let parent = object.optional("parent", ParentComponent::from_json)?;
        let placement = object.optional("placement", PlacementComponent::from_json)?;
        let type_of = object.optional("type_of", json::string)?;
        let properties = object.optional("properties", |value| {
            json::array(value, PropertySet::from_json)
        })?;
        let quantities = object.optional("quantities", |value| {
            json::array(value, QuantitySet::from_json)
        })?;
        let material = object.optional("material", MaterialComponent::from_json)?;
        let opening = object.optional("opening", OpeningComponent::from_json)?;
        let group = object.optional("group", |value| json::array(value, json::string))?;
        let connection = object.optional("connection", |value| {
            json::array(value, ConnectionLink::from_json)
        })?;
        let level = object.optional("level", LevelComponent::from_json)?;
        let zone = object.optional("zone", ZoneComponent::from_json)?;
        let style = object.optional("style", StyleComponent::from_json)?;
        let relations = object.optional("relations", RelationsComponent::from_json)?;
        Ok(Self {
            id,
            hash,
            identity,
            class,
            classification,
            spatial,
            parent,
            placement,
            type_of,
            properties,
            quantities,
            material,
            opening,
            group,
            connection,
            level,
            zone,
            style,
            relations,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
        if let Some(value) = &self.identity {
            out.key("identity");
            value.to_json(out);
        }
        if let Some(value) = &self.class {
            out.key("class");
            value.to_json(out);
        }
        if let Some(value) = &self.classification {
            out.key("classification");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.spatial {
            out.key("spatial");
            value.to_json(out);
        }
        if let Some(value) = &self.parent {
            out.key("parent");
            value.to_json(out);
        }
        if let Some(value) = &self.placement {
            out.key("placement");
            value.to_json(out);
        }
        if let Some(value) = &self.type_of {
            out.key("type_of");
            out.string(value);
        }
        if let Some(value) = &self.properties {
            out.key("properties");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.quantities {
            out.key("quantities");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.material {
            out.key("material");
            value.to_json(out);
        }
        if let Some(value) = &self.opening {
            out.key("opening");
            value.to_json(out);
        }
        if let Some(value) = &self.group {
            out.key("group");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.connection {
            out.key("connection");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.level {
            out.key("level");
            value.to_json(out);
        }
        if let Some(value) = &self.zone {
            out.key("zone");
            value.to_json(out);
        }
        if let Some(value) = &self.style {
            out.key("style");
            value.to_json(out);
        }
        if let Some(value) = &self.relations {
            out.key("relations");
            value.to_json(out);
        }
    }
}

impl FromJson for BuildingElement {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingElement {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 名前・説明・タグ・長い名前
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IdentityComponent {
    /// 名前
    pub name: Option<String>,
    /// 説明
    pub description: Option<String>,
    /// タグ
    pub tag: Option<String>,
    /// 長い名前
    pub long_name: Option<String>,
}

impl IdentityComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.optional("name", json::string)?;
        let description = object.optional("description", json::string)?;
        let tag = object.optional("tag", json::string)?;
        let long_name = object.optional("long_name", json::string)?;
        Ok(Self {
            name,
            description,
            tag,
            long_name,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.description {
            out.key("description");
            out.string(value);
        }
        if let Some(value) = &self.tag {
            out.key("tag");
            out.string(value);
        }
        if let Some(value) = &self.long_name {
            out.key("long_name");
            out.string(value);
        }
    }
}

impl FromJson for IdentityComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for IdentityComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// クラス
#[derive(Clone, Debug, PartialEq)]
pub struct ClassComponent {
    /// IFC4 のクラスの名前。IFC2X3 の StandardCase などは、元の名前を original に残して直す
    pub name: String,
    /// PredefinedType
    pub predefined: Option<String>,
    /// ObjectType など、USERDEFINED のときの名前
    pub user_defined: Option<String>,
    /// ファイルに書いた実体の名前
    pub original: String,
}

impl ClassComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.required("name", json::string)?;
        let predefined = object.optional("predefined", json::string)?;
        let user_defined = object.optional("user_defined", json::string)?;
        let original = object.required("original", json::string)?;
        Ok(Self {
            name,
            predefined,
            user_defined,
            original,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.predefined {
            out.key("predefined");
            out.string(value);
        }
        if let Some(value) = &self.user_defined {
            out.key("user_defined");
            out.string(value);
        }
        out.key("original");
        out.string(&self.original);
    }
}

impl FromJson for ClassComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ClassComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 分類の参照
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClassificationReference {
    /// 分類の体系の名前
    pub system: Option<String>,
    /// 体系の版
    pub edition: Option<String>,
    /// 分類の記号
    pub code: Option<String>,
    /// 分類の名前
    pub name: Option<String>,
}

impl ClassificationReference {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let system = object.optional("system", json::string)?;
        let edition = object.optional("edition", json::string)?;
        let code = object.optional("code", json::string)?;
        let name = object.optional("name", json::string)?;
        Ok(Self {
            system,
            edition,
            code,
            name,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.system {
            out.key("system");
            out.string(value);
        }
        if let Some(value) = &self.edition {
            out.key("edition");
            out.string(value);
        }
        if let Some(value) = &self.code {
            out.key("code");
            out.string(value);
        }
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
    }
}

impl FromJson for ClassificationReference {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ClassificationReference {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 単位とともに持つ値
#[derive(Clone, Debug, PartialEq)]
pub struct Measured {
    /// 値
    pub value: f64,
    /// 単位。解けなければ持たない
    pub unit: Option<BuildingUnit>,
}

impl Measured {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let value = object.required("value", json::number)?;
        let unit = object.optional("unit", BuildingUnit::from_json)?;
        Ok(Self { value, unit })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("value");
        out.float(self.value);
        if let Some(value) = &self.unit {
            out.key("unit");
            value.to_json(out);
        }
    }
}

impl FromJson for Measured {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Measured {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 空間の要素
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialComponent {
    /// site・building・storey・space・spatial_zone か、元の実体の名前
    pub kind: String,
    /// 高さ
    pub elevation: Option<Measured>,
    /// 基準の高さ
    pub ref_elevation: Option<Measured>,
}

impl SpatialComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let kind = object.required("kind", json::string)?;
        let elevation = object.optional("elevation", Measured::from_json)?;
        let ref_elevation = object.optional("ref_elevation", Measured::from_json)?;
        Ok(Self {
            kind,
            elevation,
            ref_elevation,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string(&self.kind);
        if let Some(value) = &self.elevation {
            out.key("elevation");
            value.to_json(out);
        }
        if let Some(value) = &self.ref_elevation {
            out.key("ref_elevation");
            value.to_json(out);
        }
    }
}

impl FromJson for SpatialComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SpatialComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 親
#[derive(Clone, Debug, PartialEq)]
pub struct ParentComponent {
    /// 親の要素
    pub of: Option<Membership>,
    /// 参照される空間の要素の ID
    pub referenced_in: Vec<String>,
}

impl ParentComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let of = object.optional("of", Membership::from_json)?;
        let referenced_in =
            object.required("referenced_in", |value| json::array(value, json::string))?;
        Ok(Self { of, referenced_in })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.of {
            out.key("of");
            value.to_json(out);
        }
        out.key("referenced_in");
        out.array(&self.referenced_in, |out, item| out.string(item));
    }
}

impl FromJson for ParentComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ParentComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 親との関係
#[derive(Clone, Debug, PartialEq)]
pub struct Membership {
    /// 親の要素の ID
    pub target: String,
    /// 関係の種類
    pub kind: ParentKind,
    /// 入れ子の順。0 から
    pub order: Option<u64>,
}

impl Membership {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let target = object.required("target", json::string)?;
        let kind = object.required("kind", ParentKind::from_json)?;
        let order = object.optional("order", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            target,
            kind,
            order,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("target");
        out.string(&self.target);
        out.key("kind");
        self.kind.to_json(out);
        if let Some(value) = &self.order {
            out.key("order");
            out.uint(*value);
        }
    }
}

impl FromJson for Membership {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Membership {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 配置
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlacementComponent {
    /// 配置の親の要素の ID
    pub relative_to: Option<String>,
    /// 親をたどった世界の変換。3 行 4 列（回転と移動）。解けなければ持たない
    pub world: Option<Vec<Vec<f64>>>,
    /// 解けなかった配置の実体の名前
    pub unsupported: Option<String>,
}

impl PlacementComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let relative_to = object.optional("relative_to", json::string)?;
        let world = object.optional("world", |value| {
            json::array(value, |value| json::array(value, json::number))
        })?;
        let unsupported = object.optional("unsupported", json::string)?;
        Ok(Self {
            relative_to,
            world,
            unsupported,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.relative_to {
            out.key("relative_to");
            out.string(value);
        }
        if let Some(value) = &self.world {
            out.key("world");
            out.array(value, |out, item| {
                out.array(item, |out, item| out.float(*item))
            });
        }
        if let Some(value) = &self.unsupported {
            out.key("unsupported");
            out.string(value);
        }
    }
}

impl FromJson for PlacementComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PlacementComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 値
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingValue {
    /// 値の型の名前（IfcLengthMeasure・IfcLabel など）
    pub type_name: Option<String>,
    /// 値の種類
    pub kind: BuildingValueKind,
    /// real と、2^53 − 1 までの integer の値
    pub number: Option<f64>,
    /// text と enumeration の値と、2^53 − 1 を超える integer の値
    pub text: Option<String>,
    /// boolean と logical の値。logical の不明は持たない
    pub boolean: Option<bool>,
    /// element の要素の ID
    pub element: Option<String>,
    /// list の値
    pub items: Option<Vec<BuildingValue>>,
}

impl BuildingValue {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let type_name = object.optional("type_name", json::string)?;
        let kind = object.required("kind", BuildingValueKind::from_json)?;
        let number = object.optional("number", json::number)?;
        let text = object.optional("text", json::string)?;
        let boolean = object.optional("boolean", json::boolean)?;
        let element = object.optional("element", json::string)?;
        let items = object.optional("items", |value| {
            json::array(value, BuildingValue::from_json)
        })?;
        Ok(Self {
            type_name,
            kind,
            number,
            text,
            boolean,
            element,
            items,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.type_name {
            out.key("type_name");
            out.string(value);
        }
        out.key("kind");
        self.kind.to_json(out);
        if let Some(value) = &self.number {
            out.key("number");
            out.float(*value);
        }
        if let Some(value) = &self.text {
            out.key("text");
            out.string(value);
        }
        if let Some(value) = &self.boolean {
            out.key("boolean");
            out.bool(*value);
        }
        if let Some(value) = &self.element {
            out.key("element");
            out.string(value);
        }
        if let Some(value) = &self.items {
            out.key("items");
            out.array(value, |out, item| item.to_json(out));
        }
    }
}

impl FromJson for BuildingValue {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingValue {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// プロパティのセット
#[derive(Clone, Debug, PartialEq)]
pub struct PropertySet {
    /// 名前
    pub name: String,
    /// プロパティ
    pub properties: Vec<Property>,
}

impl PropertySet {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.required("name", json::string)?;
        let properties = object.required("properties", |value| {
            json::array(value, Property::from_json)
        })?;
        Ok(Self { name, properties })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("name");
        out.string(&self.name);
        out.key("properties");
        out.array(&self.properties, |out, item| item.to_json(out));
    }
}

impl FromJson for PropertySet {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PropertySet {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// プロパティ
#[derive(Clone, Debug, PartialEq)]
pub struct Property {
    /// 名前
    pub name: String,
    /// 値。値がなければ持たない
    pub value: Option<BuildingValue>,
    /// 単位
    pub unit: Option<BuildingUnit>,
}

impl Property {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.required("name", json::string)?;
        let value = object.optional("value", BuildingValue::from_json)?;
        let unit = object.optional("unit", BuildingUnit::from_json)?;
        Ok(Self { name, value, unit })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.value {
            out.key("value");
            value.to_json(out);
        }
        if let Some(value) = &self.unit {
            out.key("unit");
            value.to_json(out);
        }
    }
}

impl FromJson for Property {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Property {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 数量のセット
#[derive(Clone, Debug, PartialEq)]
pub struct QuantitySet {
    /// 名前
    pub name: String,
    /// 計測の方法
    pub method: Option<String>,
    /// 数量
    pub quantities: Vec<Quantity>,
}

impl QuantitySet {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.required("name", json::string)?;
        let method = object.optional("method", json::string)?;
        let quantities = object.required("quantities", |value| {
            json::array(value, Quantity::from_json)
        })?;
        Ok(Self {
            name,
            method,
            quantities,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.method {
            out.key("method");
            out.string(value);
        }
        out.key("quantities");
        out.array(&self.quantities, |out, item| item.to_json(out));
    }
}

impl FromJson for QuantitySet {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for QuantitySet {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 数量
#[derive(Clone, Debug, PartialEq)]
pub struct Quantity {
    /// 名前
    pub name: String,
    /// 種類
    pub kind: QuantityKind,
    /// 値
    pub value: Measured,
    /// 式
    pub formula: Option<String>,
}

impl Quantity {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.required("name", json::string)?;
        let kind = object.required("kind", QuantityKind::from_json)?;
        let value = object.required("value", Measured::from_json)?;
        let formula = object.optional("formula", json::string)?;
        Ok(Self {
            name,
            kind,
            value,
            formula,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("name");
        out.string(&self.name);
        out.key("kind");
        self.kind.to_json(out);
        out.key("value");
        self.value.to_json(out);
        if let Some(value) = &self.formula {
            out.key("formula");
            out.string(value);
        }
    }
}

impl FromJson for Quantity {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Quantity {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 材料
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialComponent {
    /// 材料の組み方
    pub kind: MaterialKind,
    /// 部分
    pub parts: Vec<MaterialPart>,
    /// 層の向き
    pub direction: Option<String>,
    /// 層の基準からのずれ
    pub offset: Option<Measured>,
}

impl MaterialComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let kind = object.required("kind", MaterialKind::from_json)?;
        let parts =
            object.required("parts", |value| json::array(value, MaterialPart::from_json))?;
        let direction = object.optional("direction", json::string)?;
        let offset = object.optional("offset", Measured::from_json)?;
        Ok(Self {
            kind,
            parts,
            direction,
            offset,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        self.kind.to_json(out);
        out.key("parts");
        out.array(&self.parts, |out, item| item.to_json(out));
        if let Some(value) = &self.direction {
            out.key("direction");
            out.string(value);
        }
        if let Some(value) = &self.offset {
            out.key("offset");
            value.to_json(out);
        }
    }
}

impl FromJson for MaterialComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for MaterialComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 材料の部分
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MaterialPart {
    /// 材料の要素の ID
    pub material: Option<String>,
    /// 名前
    pub name: Option<String>,
    /// 厚さ
    pub thickness: Option<Measured>,
}

impl MaterialPart {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let material = object.optional("material", json::string)?;
        let name = object.optional("name", json::string)?;
        let thickness = object.optional("thickness", Measured::from_json)?;
        Ok(Self {
            material,
            name,
            thickness,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.material {
            out.key("material");
            out.string(value);
        }
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.thickness {
            out.key("thickness");
            value.to_json(out);
        }
    }
}

impl FromJson for MaterialPart {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for MaterialPart {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開口
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpeningComponent {
    /// この開口が穴を開ける要素の ID
    pub voids: Option<String>,
    /// この要素が埋める開口の要素の ID
    pub fills: Option<String>,
}

impl OpeningComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let voids = object.optional("voids", json::string)?;
        let fills = object.optional("fills", json::string)?;
        Ok(Self { voids, fills })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.voids {
            out.key("voids");
            out.string(value);
        }
        if let Some(value) = &self.fills {
            out.key("fills");
            out.string(value);
        }
    }
}

impl FromJson for OpeningComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for OpeningComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 接続
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectionLink {
    /// 関係の実体の名前
    pub kind: String,
    /// 接続する要素の ID
    pub target: String,
}

impl ConnectionLink {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let kind = object.required("kind", json::string)?;
        let target = object.required("target", json::string)?;
        Ok(Self { kind, target })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string(&self.kind);
        out.key("target");
        out.string(&self.target);
    }
}

impl FromJson for ConnectionLink {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ConnectionLink {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// レベル
#[derive(Clone, Debug, PartialEq)]
pub struct LevelComponent {
    /// 名前
    pub name: Option<String>,
    /// 種類
    pub kind: Option<String>,
    /// 高さ
    pub height: Measured,
    /// 基準
    pub datum: Option<String>,
}

impl LevelComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.optional("name", json::string)?;
        let kind = object.optional("kind", json::string)?;
        let height = object.required("height", Measured::from_json)?;
        let datum = object.optional("datum", json::string)?;
        Ok(Self {
            name,
            kind,
            height,
            datum,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.kind {
            out.key("kind");
            out.string(value);
        }
        out.key("height");
        self.height.to_json(out);
        if let Some(value) = &self.datum {
            out.key("datum");
            out.string(value);
        }
    }
}

impl FromJson for LevelComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LevelComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 区画
#[derive(Clone, Debug, PartialEq)]
pub struct ZoneComponent {
    /// 区画の種類
    pub kind: Option<String>,
    /// 構成する要素の ID
    pub members: Vec<String>,
}

impl ZoneComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let kind = object.optional("kind", json::string)?;
        let members = object.required("members", |value| json::array(value, json::string))?;
        Ok(Self { kind, members })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.kind {
            out.key("kind");
            out.string(value);
        }
        out.key("members");
        out.array(&self.members, |out, item| out.string(item));
    }
}

impl FromJson for ZoneComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ZoneComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 表示のスタイル
#[derive(Clone, Debug, PartialEq)]
pub struct StyleComponent {
    /// 面の色の RGB（0 から 1）
    pub surface_color: Option<Vec<f64>>,
    /// 透明度（0 から 1）
    pub transparency: Option<f64>,
    /// 線の色の RGB（0 から 1）
    pub curve_color: Option<Vec<f64>>,
    /// 提示のレイヤの名前
    pub layers: Vec<String>,
}

impl StyleComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let surface_color =
            object.optional("surface_color", |value| json::array(value, json::number))?;
        let transparency = object.optional("transparency", json::number)?;
        let curve_color =
            object.optional("curve_color", |value| json::array(value, json::number))?;
        let layers = object.required("layers", |value| json::array(value, json::string))?;
        Ok(Self {
            surface_color,
            transparency,
            curve_color,
            layers,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.surface_color {
            out.key("surface_color");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.transparency {
            out.key("transparency");
            out.float(*value);
        }
        if let Some(value) = &self.curve_color {
            out.key("curve_color");
            out.array(value, |out, item| out.float(*item));
        }
        out.key("layers");
        out.array(&self.layers, |out, item| out.string(item));
    }
}

impl FromJson for StyleComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for StyleComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 索引から求める関係。要素の ID の列
#[derive(Clone, Debug, PartialEq)]
pub struct RelationsComponent {
    /// 子（集約・入れ子・包含）
    pub children: Vec<String>,
    /// 参照する要素
    pub referenced: Vec<String>,
    /// 型の実例
    pub instances: Vec<String>,
    /// 開口
    pub openings: Vec<String>,
    /// 開口を埋める要素
    pub fillings: Vec<String>,
    /// グループ・システム・区画の構成員
    pub members: Vec<String>,
}

impl RelationsComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let children = object.required("children", |value| json::array(value, json::string))?;
        let referenced = object.required("referenced", |value| json::array(value, json::string))?;
        let instances = object.required("instances", |value| json::array(value, json::string))?;
        let openings = object.required("openings", |value| json::array(value, json::string))?;
        let fillings = object.required("fillings", |value| json::array(value, json::string))?;
        let members = object.required("members", |value| json::array(value, json::string))?;
        Ok(Self {
            children,
            referenced,
            instances,
            openings,
            fillings,
            members,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("children");
        out.array(&self.children, |out, item| out.string(item));
        out.key("referenced");
        out.array(&self.referenced, |out, item| out.string(item));
        out.key("instances");
        out.array(&self.instances, |out, item| out.string(item));
        out.key("openings");
        out.array(&self.openings, |out, item| out.string(item));
        out.key("fillings");
        out.array(&self.fillings, |out, item| out.string(item));
        out.key("members");
        out.array(&self.members, |out, item| out.string(item));
    }
}

impl FromJson for RelationsComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RelationsComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 親との関係の種類。集約・入れ子・包含
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ParentKind {
    /// `aggregates`
    Aggregates,
    /// `nests`
    Nests,
    /// `contained`
    Contained,
}

impl ParentKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Aggregates, Self::Nests, Self::Contained];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Aggregates => "aggregates",
            Self::Nests => "nests",
            Self::Contained => "contained",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "aggregates" => Some(Self::Aggregates),
            "nests" => Some(Self::Nests),
            "contained" => Some(Self::Contained),
            _ => None,
        }
    }
}

impl FromJson for ParentKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for ParentKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 値の種類
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BuildingValueKind {
    /// `real`
    Real,
    /// `integer`
    Integer,
    /// `text`
    Text,
    /// `boolean`
    Boolean,
    /// `logical`
    Logical,
    /// `enumeration`
    Enumeration,
    /// `element`
    Element,
    /// `list`
    List,
}

impl BuildingValueKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Real,
        Self::Integer,
        Self::Text,
        Self::Boolean,
        Self::Logical,
        Self::Enumeration,
        Self::Element,
        Self::List,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Real => "real",
            Self::Integer => "integer",
            Self::Text => "text",
            Self::Boolean => "boolean",
            Self::Logical => "logical",
            Self::Enumeration => "enumeration",
            Self::Element => "element",
            Self::List => "list",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "real" => Some(Self::Real),
            "integer" => Some(Self::Integer),
            "text" => Some(Self::Text),
            "boolean" => Some(Self::Boolean),
            "logical" => Some(Self::Logical),
            "enumeration" => Some(Self::Enumeration),
            "element" => Some(Self::Element),
            "list" => Some(Self::List),
            _ => None,
        }
    }
}

impl FromJson for BuildingValueKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for BuildingValueKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 数量の種類
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuantityKind {
    /// `length`
    Length,
    /// `area`
    Area,
    /// `volume`
    Volume,
    /// `count`
    Count,
    /// `weight`
    Weight,
    /// `time`
    Time,
}

impl QuantityKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Length,
        Self::Area,
        Self::Volume,
        Self::Count,
        Self::Weight,
        Self::Time,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Length => "length",
            Self::Area => "area",
            Self::Volume => "volume",
            Self::Count => "count",
            Self::Weight => "weight",
            Self::Time => "time",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "length" => Some(Self::Length),
            "area" => Some(Self::Area),
            "volume" => Some(Self::Volume),
            "count" => Some(Self::Count),
            "weight" => Some(Self::Weight),
            "time" => Some(Self::Time),
            _ => None,
        }
    }
}

impl FromJson for QuantityKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for QuantityKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 材料の組み方。単一・層・断面・構成・一覧
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MaterialKind {
    /// `single`
    Single,
    /// `layers`
    Layers,
    /// `profiles`
    Profiles,
    /// `constituents`
    Constituents,
    /// `list`
    List,
}

impl MaterialKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Single,
        Self::Layers,
        Self::Profiles,
        Self::Constituents,
        Self::List,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Layers => "layers",
            Self::Profiles => "profiles",
            Self::Constituents => "constituents",
            Self::List => "list",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "single" => Some(Self::Single),
            "layers" => Some(Self::Layers),
            "profiles" => Some(Self::Profiles),
            "constituents" => Some(Self::Constituents),
            "list" => Some(Self::List),
            _ => None,
        }
    }
}

impl FromJson for MaterialKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for MaterialKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 仕様の結果。unknown は確かめられない facet があった
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IdsStatus {
    /// `passed`
    Passed,
    /// `failed`
    Failed,
    /// `unknown`
    Unknown,
}

impl IdsStatus {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Passed, Self::Failed, Self::Unknown];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "passed" => Some(Self::Passed),
            "failed" => Some(Self::Failed),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

impl FromJson for IdsStatus {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for IdsStatus {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 開いた IFC の建物モデルを、zm_push で渡した IDS で照合する（決定 0217・0219）。確かめられない facet は、指摘と分けて info の指摘で返す。小版 44 から
#[derive(Clone, Debug, PartialEq)]
pub struct IdsCheckRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
    /// 文書の名前。open で IFC を開いたもの
    pub doc: String,
    /// IDS の入力の名前（zm_push で渡した .ids）
    pub ids: String,
    /// EXPRESS の入力の名前（zm_push で渡した .exp）。渡せば、IFC の属性の全体を EXPRESS の名前で照らす。渡さなければ、建物モデルの成分にある属性だけを照らす
    pub express: Option<String>,
    /// 返す最初の位置。前の応答の next_offset を渡す。書かなければ 0
    pub offset: Option<u64>,
    /// 返す数の上限。書かなければ 1000
    pub limit: Option<u64>,
}

impl IdsCheckRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "ids_check")?;
        let doc = object.required("doc", json::string)?;
        let ids = object.required("ids", json::string)?;
        let express = object.optional("express", json::string)?;
        let offset = object.optional("offset", |value| json::uint(value, 0, 9007199254740991))?;
        let limit = object.optional("limit", |value| json::uint(value, 1, 10000))?;
        Ok(Self {
            request_common,
            doc,
            ids,
            express,
            offset,
            limit,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("ids_check");
        out.key("doc");
        out.string(&self.doc);
        out.key("ids");
        out.string(&self.ids);
        if let Some(value) = &self.express {
            out.key("express");
            out.string(value);
        }
        if let Some(value) = &self.offset {
            out.key("offset");
            out.uint(*value);
        }
        if let Some(value) = &self.limit {
            out.key("limit");
            out.uint(*value);
        }
    }
}

impl FromJson for IdsCheckRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for IdsCheckRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 照合の結果。指摘は offset から limit 個まで
#[derive(Clone, Debug, PartialEq)]
pub struct IdsCheckResult {
    /// IDS の題
    pub title: Option<String>,
    /// 仕様ごとの結果。IDS の順
    pub specifications: Vec<IdsSpecification>,
    /// 指摘。仕様の順
    pub findings: Vec<IdsFinding>,
    /// 全体の指摘の数
    pub total: u64,
    /// 続きの最初の位置。続きがなければ持たない
    pub next_offset: Option<u64>,
}

impl IdsCheckResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let title = object.optional("title", json::string)?;
        let specifications = object.required("specifications", |value| {
            json::array(value, IdsSpecification::from_json)
        })?;
        let findings = object.required("findings", |value| {
            json::array(value, IdsFinding::from_json)
        })?;
        let total = object.required("total", |value| json::uint(value, 0, 9007199254740991))?;
        let next_offset = object.optional("next_offset", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            title,
            specifications,
            findings,
            total,
            next_offset,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.title {
            out.key("title");
            out.string(value);
        }
        out.key("specifications");
        out.array(&self.specifications, |out, item| item.to_json(out));
        out.key("findings");
        out.array(&self.findings, |out, item| item.to_json(out));
        out.key("total");
        out.uint(self.total);
        if let Some(value) = &self.next_offset {
            out.key("next_offset");
            out.uint(*value);
        }
    }
}

impl FromJson for IdsCheckResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for IdsCheckResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 仕様の結果
#[derive(Clone, Debug, PartialEq)]
pub struct IdsSpecification {
    /// 名前
    pub name: String,
    /// 識別子
    pub identifier: Option<String>,
    /// 結果
    pub status: IdsStatus,
    /// 適用の条件に合う要素の数
    pub applicable: u64,
    /// 要件を満たさない要素の数
    pub failed: u64,
    /// 確かめられなかった要素の数
    pub unsupported: u64,
}

impl IdsSpecification {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.required("name", json::string)?;
        let identifier = object.optional("identifier", json::string)?;
        let status = object.required("status", IdsStatus::from_json)?;
        let applicable =
            object.required("applicable", |value| json::uint(value, 0, 9007199254740991))?;
        let failed = object.required("failed", |value| json::uint(value, 0, 9007199254740991))?;
        let unsupported = object.required("unsupported", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            name,
            identifier,
            status,
            applicable,
            failed,
            unsupported,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.identifier {
            out.key("identifier");
            out.string(value);
        }
        out.key("status");
        self.status.to_json(out);
        out.key("applicable");
        out.uint(self.applicable);
        out.key("failed");
        out.uint(self.failed);
        out.key("unsupported");
        out.uint(self.unsupported);
    }
}

impl FromJson for IdsSpecification {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for IdsSpecification {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 指摘。diagnostic は診断と同じ形で、describe で文にできる
#[derive(Clone, Debug, PartialEq)]
pub struct IdsFinding {
    /// 仕様の名前
    pub specification: String,
    /// code・重要度・場所（要素の ID）・値
    pub diagnostic: Diagnostic,
}

impl IdsFinding {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let specification = object.required("specification", json::string)?;
        let diagnostic = object.required("diagnostic", Diagnostic::from_json)?;
        Ok(Self {
            specification,
            diagnostic,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("specification");
        out.string(&self.specification);
        out.key("diagnostic");
        self.diagnostic.to_json(out);
    }
}

impl FromJson for IdsFinding {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for IdsFinding {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開いた IFC を ids_check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0218・0219）に足す。足した文書は blobs の file で返す。小版 44 から
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewAdoptIdsRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
    /// 文書の名前。open で IFC を開いたもの
    pub doc: String,
    /// IDS の入力の名前（zm_push で渡した .ids）
    pub ids: String,
    /// EXPRESS の入力の名前（zm_push で渡した .exp）。渡せば、IFC の属性の全体を EXPRESS の名前で照らす。渡さなければ、建物モデルの成分にある属性だけを照らす
    pub express: Option<String>,
    /// 足す先のレビューの文書の入力の名前（zm_push で渡した JSON）。書かなければ空の文書に足す
    pub review: Option<String>,
    /// 指摘が指す対象の ID。文書になければ、文書の形式とハッシュで足す
    pub target: String,
    /// 取り込む指摘の番号の列。ids_check の指摘の並びの順で、0 から数える
    pub findings: Vec<u64>,
}

impl ReviewAdoptIdsRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "review_adopt_ids")?;
        let doc = object.required("doc", json::string)?;
        let ids = object.required("ids", json::string)?;
        let express = object.optional("express", json::string)?;
        let review = object.optional("review", json::string)?;
        let target = object.required("target", json::string)?;
        let findings = object.required("findings", |value| {
            json::array(value, |value| json::uint(value, 0, 9007199254740991))
        })?;
        Ok(Self {
            request_common,
            doc,
            ids,
            express,
            review,
            target,
            findings,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("review_adopt_ids");
        out.key("doc");
        out.string(&self.doc);
        out.key("ids");
        out.string(&self.ids);
        if let Some(value) = &self.express {
            out.key("express");
            out.string(value);
        }
        if let Some(value) = &self.review {
            out.key("review");
            out.string(value);
        }
        out.key("target");
        out.string(&self.target);
        out.key("findings");
        out.array(&self.findings, |out, item| out.uint(*item));
    }
}

impl FromJson for ReviewAdoptIdsRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ReviewAdoptIdsRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// zm_push で渡した BCF 3.0 の bcfzip を読み、開いた IFC を対象にしたレビューの文書（決定 0214・0220）にする。topic は指摘に、comment はコメントに、viewpoint は 3D の場所に写し、要素の参照のハッシュは建物モデルの要素のハッシュで埋める。文書は blobs の file で返す。小版 45 から
#[derive(Clone, Debug, PartialEq)]
pub struct BcfReadRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
    /// BCF が指す建物モデルの文書の名前。open で IFC を開いたもの
    pub doc: String,
    /// bcfzip の入力の名前（zm_push で渡した .bcf）
    pub input: String,
    /// レビューの文書の対象の ID
    pub target: String,
    /// 指摘の状態と BCF の TopicStatus の文字列の対応の表。書かなければ Open・Resolved・Closed。読むとき、表にない文字列は open にし、元の文字列を status_label に残す
    pub statuses: Option<Vec<BcfStatus>>,
}

impl BcfReadRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "bcf_read")?;
        let doc = object.required("doc", json::string)?;
        let input = object.required("input", json::string)?;
        let target = object.required("target", json::string)?;
        let statuses =
            object.optional("statuses", |value| json::array(value, BcfStatus::from_json))?;
        Ok(Self {
            request_common,
            doc,
            input,
            target,
            statuses,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("bcf_read");
        out.key("doc");
        out.string(&self.doc);
        out.key("input");
        out.string(&self.input);
        out.key("target");
        out.string(&self.target);
        if let Some(value) = &self.statuses {
            out.key("statuses");
            out.array(value, |out, item| item.to_json(out));
        }
    }
}

impl FromJson for BcfReadRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BcfReadRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 状態の対応
#[derive(Clone, Debug, PartialEq)]
pub struct BcfStatus {
    /// BCF の TopicStatus の文字列
    pub text: String,
    /// 指摘の状態
    pub status: ReviewStatus,
}

impl BcfStatus {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let text = object.required("text", json::string)?;
        let status = object.required("status", ReviewStatus::from_json)?;
        Ok(Self { text, status })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("text");
        out.string(&self.text);
        out.key("status");
        self.status.to_json(out);
    }
}

impl FromJson for BcfStatus {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BcfStatus {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 指摘の状態（決定 0214）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReviewStatus {
    /// `open`
    Open,
    /// `resolved`
    Resolved,
    /// `closed`
    Closed,
}

impl ReviewStatus {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Open, Self::Resolved, Self::Closed];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Resolved => "resolved",
            Self::Closed => "closed",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "open" => Some(Self::Open),
            "resolved" => Some(Self::Resolved),
            "closed" => Some(Self::Closed),
            _ => None,
        }
    }
}

impl FromJson for ReviewStatus {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for ReviewStatus {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 読んだ bcfzip
#[derive(Clone, Debug, PartialEq)]
pub struct BcfReadResult {
    /// BCF の版
    pub version: String,
    /// 指摘の数
    pub topics: u64,
}

impl BcfReadResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let version = object.required("version", json::string)?;
        let topics = object.required("topics", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self { version, topics })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("version");
        out.string(&self.version);
        out.key("topics");
        out.uint(self.topics);
    }
}

impl FromJson for BcfReadResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BcfReadResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// zm_push で渡したレビューの文書を、BCF 3.0 の bcfzip に書く（決定 0214・0220）。target を対象にした 3D の場所を viewpoint にし、2D の場所と図形は書かずに診断する。original を渡せば、変えていない topic を元のバイト列のまま書き、変えた topic でも写さない要素と知らない要素を保つ。bcfzip は blobs の file で返す。小版 45 から
#[derive(Clone, Debug, PartialEq)]
pub struct BcfWriteRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
    /// レビューの文書の入力の名前（zm_push で渡した JSON）
    pub review: String,
    /// viewpoint にする 3D の場所の対象の ID
    pub target: String,
    /// 元の bcfzip の入力の名前（zm_push で渡した .bcf）。書かなければ新しく書く
    pub original: Option<String>,
    /// 指摘の状態と BCF の TopicStatus の文字列の対応の表。書かなければ Open・Resolved・Closed。読むとき、表にない文字列は open にし、元の文字列を status_label に残す
    pub statuses: Option<Vec<BcfStatus>>,
    /// 種類のない指摘に書く TopicType
    pub kind: Option<String>,
    /// 作者のない指摘とコメントに書く作者
    pub author: Option<String>,
    /// 日時のない指摘とコメントに書く日時（RFC 3339）
    pub date: Option<String>,
}

impl BcfWriteRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "bcf_write")?;
        let review = object.required("review", json::string)?;
        let target = object.required("target", json::string)?;
        let original = object.optional("original", json::string)?;
        let statuses =
            object.optional("statuses", |value| json::array(value, BcfStatus::from_json))?;
        let kind = object.optional("kind", json::string)?;
        let author = object.optional("author", json::string)?;
        let date = object.optional("date", json::string)?;
        Ok(Self {
            request_common,
            review,
            target,
            original,
            statuses,
            kind,
            author,
            date,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("bcf_write");
        out.key("review");
        out.string(&self.review);
        out.key("target");
        out.string(&self.target);
        if let Some(value) = &self.original {
            out.key("original");
            out.string(value);
        }
        if let Some(value) = &self.statuses {
            out.key("statuses");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.kind {
            out.key("kind");
            out.string(value);
        }
        if let Some(value) = &self.author {
            out.key("author");
            out.string(value);
        }
        if let Some(value) = &self.date {
            out.key("date");
            out.string(value);
        }
    }
}

impl FromJson for BcfWriteRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BcfWriteRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 書いた bcfzip
#[derive(Clone, Debug, PartialEq)]
pub struct BcfWriteResult {
    /// 書いた指摘の数
    pub topics: u64,
}

impl BcfWriteResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let topics = object.required("topics", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self { topics })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("topics");
        out.uint(self.topics);
    }
}

impl FromJson for BcfWriteResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BcfWriteResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 表の項目
#[derive(Clone, Debug, PartialEq)]
pub struct TableItem {
    /// ID。表のレコードがハンドルを持てば layer/h10・line-type/h14、なければ読んだ順の番号（layer/2）。寸法スタイルは dimension-style/h1A か、寸法スタイルの並びの中の順（dimension-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
    pub id: String,
    /// 元の名前。翻訳しない
    pub name: String,
    /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
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

/// レイヤの表の項目（決定 0162）。小版 22 から、状態とスタイルを持つ
#[derive(Clone, Debug, PartialEq)]
pub struct LayerItem {
    /// ID。表のレコードがハンドルを持てば layer/h10、なければ読んだ順の番号（layer/2）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
    pub id: String,
    /// 元の名前。翻訳しない
    pub name: String,
    /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
    pub hash: String,
    /// ファイルに定義がある。偽なら、要素が名前で指すだけのレイヤで、ほかの欄を持たない。小版 22 から
    pub defined: bool,
    /// 表示する。小版 22 から
    pub visible: Option<bool>,
    /// 印刷する。小版 22 から
    pub printable: Option<bool>,
    /// 編集できる。小版 22 から
    pub editable: Option<bool>,
    /// 凍結する。小版 22 から
    pub frozen: Option<bool>,
    /// 色。inherit を持たない。定義が色を持たなければ書かない。小版 22 から
    pub color: Option<ColorChange>,
    /// 線種の ID。定義が線種を持たなければ書かない。小版 22 から
    pub line_type_id: Option<String>,
    /// 線幅。inherit を持たない。定義が線幅を持たなければ書かない。小版 22 から
    pub line_weight: Option<LineWeightChange>,
}

impl LayerItem {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let name = object.required("name", json::string)?;
        let hash = object.required("hash", json::string)?;
        let defined = object.required("defined", json::boolean)?;
        let visible = object.optional("visible", json::boolean)?;
        let printable = object.optional("printable", json::boolean)?;
        let editable = object.optional("editable", json::boolean)?;
        let frozen = object.optional("frozen", json::boolean)?;
        let color = object.optional("color", ColorChange::from_json)?;
        let line_type_id = object.optional("line_type_id", json::string)?;
        let line_weight = object.optional("line_weight", LineWeightChange::from_json)?;
        Ok(Self {
            id,
            name,
            hash,
            defined,
            visible,
            printable,
            editable,
            frozen,
            color,
            line_type_id,
            line_weight,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("name");
        out.string(&self.name);
        out.key("hash");
        out.string(&self.hash);
        out.key("defined");
        out.bool(self.defined);
        if let Some(value) = &self.visible {
            out.key("visible");
            out.bool(*value);
        }
        if let Some(value) = &self.printable {
            out.key("printable");
            out.bool(*value);
        }
        if let Some(value) = &self.editable {
            out.key("editable");
            out.bool(*value);
        }
        if let Some(value) = &self.frozen {
            out.key("frozen");
            out.bool(*value);
        }
        if let Some(value) = &self.color {
            out.key("color");
            value.to_json(out);
        }
        if let Some(value) = &self.line_type_id {
            out.key("line_type_id");
            out.string(value);
        }
        if let Some(value) = &self.line_weight {
            out.key("line_weight");
            value.to_json(out);
        }
    }
}

impl FromJson for LayerItem {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LayerItem {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線種の表の項目（決定 0164）。小版 26 から、定義を持つ
#[derive(Clone, Debug, PartialEq)]
pub struct LineTypeItem {
    /// ID。表のレコードがハンドルを持てば line-type/h14、なければ読んだ順の番号（line-type/2）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
    pub id: String,
    /// 元の名前。翻訳しない
    pub name: String,
    /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
    pub hash: String,
    /// ファイルに定義がある。偽なら、要素が名前で指すだけの線種で、ほかの欄を持たない。小版 26 から
    pub defined: bool,
    /// 元の説明。小版 26 から
    pub description: Option<String>,
    /// 線と間の長さの列。正は線、負は間、0 は点。空なら実線。長さの単位は paper が決める。小版 26 から
    pub pattern: Option<Vec<f64>>,
    /// 列の長さが用紙の mm なら真、文書の単位なら偽。用紙の mm の列は line_type で変えられない。小版 26 から
    pub paper: Option<bool>,
    /// 列の要素に形か文字を添える。添えれば、line_type で列の要素の数を変えられない。小版 26 から
    pub marked: Option<bool>,
    /// 列の要素に添える形と文字（決定 0150・0166）。添えなければ空。小版 28 から
    pub marks: Option<Vec<LineTypeMark>>,
}

impl LineTypeItem {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let name = object.required("name", json::string)?;
        let hash = object.required("hash", json::string)?;
        let defined = object.required("defined", json::boolean)?;
        let description = object.optional("description", json::string)?;
        let pattern = object.optional("pattern", |value| json::array(value, json::number))?;
        let paper = object.optional("paper", json::boolean)?;
        let marked = object.optional("marked", json::boolean)?;
        let marks =
            object.optional("marks", |value| json::array(value, LineTypeMark::from_json))?;
        Ok(Self {
            id,
            name,
            hash,
            defined,
            description,
            pattern,
            paper,
            marked,
            marks,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("name");
        out.string(&self.name);
        out.key("hash");
        out.string(&self.hash);
        out.key("defined");
        out.bool(self.defined);
        if let Some(value) = &self.description {
            out.key("description");
            out.string(value);
        }
        if let Some(value) = &self.pattern {
            out.key("pattern");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.paper {
            out.key("paper");
            out.bool(*value);
        }
        if let Some(value) = &self.marked {
            out.key("marked");
            out.bool(*value);
        }
        if let Some(value) = &self.marks {
            out.key("marks");
            out.array(value, |out, item| item.to_json(out));
        }
    }
}

impl FromJson for LineTypeItem {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LineTypeItem {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 寸法スタイルの表の項目（決定 0165）。小版 27 から、端部記号と大きさを持つ
#[derive(Clone, Debug, PartialEq)]
pub struct DimensionStyleItem {
    /// ID。寸法スタイルのレコードがハンドルを持てば dimension-style/h1A、なければ寸法スタイルの並びの中の順（dimension-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
    pub id: String,
    /// 元の名前。翻訳しない
    pub name: String,
    /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
    pub hash: String,
    /// 寸法線の 1 本目と 2 本目の補助線の側の端部記号の、ブロックの名前。空の名前は既定の矢印。小版 27 から
    pub ends: Option<Vec<String>>,
    /// 引出線の端部記号の、ブロックの名前。空の名前は既定の矢印。小版 27 から
    pub leader: Option<String>,
    /// 端部記号の大きさ。文書の単位で、scale を掛ける前の値。小版 27 から
    pub size: Option<f64>,
    /// 端部記号の代わりに描く斜線の大きさ。0 なら斜線を描かない。小版 27 から
    pub tick: Option<f64>,
    /// 大きさに掛ける倍率。0 はビューの縮尺から決める。小版 27 から
    pub scale: Option<f64>,
}

impl DimensionStyleItem {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let name = object.required("name", json::string)?;
        let hash = object.required("hash", json::string)?;
        let ends = object.optional("ends", |value| json::array(value, json::string))?;
        let leader = object.optional("leader", json::string)?;
        let size = object.optional("size", json::number)?;
        let tick = object.optional("tick", json::number)?;
        let scale = object.optional("scale", json::number)?;
        Ok(Self {
            id,
            name,
            hash,
            ends,
            leader,
            size,
            tick,
            scale,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("name");
        out.string(&self.name);
        out.key("hash");
        out.string(&self.hash);
        if let Some(value) = &self.ends {
            out.key("ends");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.leader {
            out.key("leader");
            out.string(value);
        }
        if let Some(value) = &self.size {
            out.key("size");
            out.float(*value);
        }
        if let Some(value) = &self.tick {
            out.key("tick");
            out.float(*value);
        }
        if let Some(value) = &self.scale {
            out.key("scale");
            out.float(*value);
        }
    }
}

impl FromJson for DimensionStyleItem {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DimensionStyleItem {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線種の列の要素に添える形か文字（決定 0150・0166）。要素の始まりに、線の向きとずれで置く。小版 28 から
#[derive(Clone, Debug, PartialEq)]
pub struct LineTypeMark {
    /// 添える列の要素の番号（pattern の添字）
    pub index: u64,
    /// 形（shape）か文字（text）
    pub kind: LineTypeMarkKind,
    /// 形の形のファイル。元の形式が指す文字スタイルがなければ書かない
    pub font: Option<Font>,
    /// 形の番号
    pub number: Option<u64>,
    /// 文字の文字列
    pub text: Option<String>,
    /// 文字の文字スタイルの元の名前。元の形式が指す文字スタイルがなければ書かない
    pub style: Option<String>,
    /// 文字の文字スタイルの ID。tables の text_styles の ID で、名前で引いたもの（大文字と小文字を区別せず、同じ名前があれば先の項目）
    pub style_id: Option<String>,
    /// 文字の高さ。文字スタイルが決まった高さを持てば、それに scale を掛けたもの。持たなければ scale。単位は pattern と同じ
    pub height: Option<f64>,
    /// 倍率。文字なら、文字スタイルの高さへの倍率
    pub scale: f64,
    /// 回転（度、反時計回り）
    pub rotation: f64,
    /// 回転の基準
    pub rotation_basis: MarkRotation,
    /// 置く位置のずれ（x・y）。線の向きを x とする。単位は pattern と同じ
    pub offset: Vec<f64>,
}

impl LineTypeMark {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let index = object.required("index", |value| json::uint(value, 0, 9007199254740991))?;
        let kind = object.required("kind", LineTypeMarkKind::from_json)?;
        let font = object.optional("font", Font::from_json)?;
        let number = object.optional("number", |value| json::uint(value, 0, 9007199254740991))?;
        let text = object.optional("text", json::string)?;
        let style = object.optional("style", json::string)?;
        let style_id = object.optional("style_id", json::string)?;
        let height = object.optional("height", json::number)?;
        let scale = object.required("scale", json::number)?;
        let rotation = object.required("rotation", json::number)?;
        let rotation_basis = object.required("rotation_basis", MarkRotation::from_json)?;
        let offset = object.required("offset", |value| json::array(value, json::number))?;
        Ok(Self {
            index,
            kind,
            font,
            number,
            text,
            style,
            style_id,
            height,
            scale,
            rotation,
            rotation_basis,
            offset,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("index");
        out.uint(self.index);
        out.key("kind");
        self.kind.to_json(out);
        if let Some(value) = &self.font {
            out.key("font");
            value.to_json(out);
        }
        if let Some(value) = &self.number {
            out.key("number");
            out.uint(*value);
        }
        if let Some(value) = &self.text {
            out.key("text");
            out.string(value);
        }
        if let Some(value) = &self.style {
            out.key("style");
            out.string(value);
        }
        if let Some(value) = &self.style_id {
            out.key("style_id");
            out.string(value);
        }
        if let Some(value) = &self.height {
            out.key("height");
            out.float(*value);
        }
        out.key("scale");
        out.float(self.scale);
        out.key("rotation");
        out.float(self.rotation);
        out.key("rotation_basis");
        self.rotation_basis.to_json(out);
        out.key("offset");
        out.array(&self.offset, |out, item| out.float(*item));
    }
}

impl FromJson for LineTypeMark {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LineTypeMark {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線種に添えるものの種類。shape は形のファイルの形、text は文字
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LineTypeMarkKind {
    /// `shape`
    Shape,
    /// `text`
    Text,
}

impl LineTypeMarkKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Shape, Self::Text];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Shape => "shape",
            Self::Text => "text",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "shape" => Some(Self::Shape),
            "text" => Some(Self::Text),
            _ => None,
        }
    }
}

impl FromJson for LineTypeMarkKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for LineTypeMarkKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 回転の基準。relative は線の向きから、absolute は文書の x 軸から測る。upright は線の向きから測り、文字が逆さに見えるなら半回転する
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MarkRotation {
    /// `relative`
    Relative,
    /// `absolute`
    Absolute,
    /// `upright`
    Upright,
}

impl MarkRotation {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Relative, Self::Absolute, Self::Upright];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Relative => "relative",
            Self::Absolute => "absolute",
            Self::Upright => "upright",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "relative" => Some(Self::Relative),
            "absolute" => Some(Self::Absolute),
            "upright" => Some(Self::Upright),
            _ => None,
        }
    }
}

impl FromJson for MarkRotation {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for MarkRotation {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 文字スタイルの表の項目（決定 0158）。フォントと縦書きは、この文字スタイルを指す文字に効く。高さ・幅の倍率・傾き・裏返し・上下反転は、新しく作る文字の既定の値で、今の文字はそれぞれの値を持つ。小版 20 から
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyleItem {
    /// ID。表のレコードがハンドルを持てば text-style/h11、なければ文字スタイルの並びの中の順（text-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
    pub id: String,
    /// 元の名前。翻訳しない
    pub name: String,
    /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
    pub hash: String,
    /// ファイルに定義がある。偽なら、ほかの欄を持たない
    pub defined: bool,
    /// 主のフォント。ファイルが決めていなければ書かない
    pub font: Option<Font>,
    /// 大きいフォント（漢字などの形のファイル）。なければ書かない
    pub big_font: Option<Font>,
    /// 決まった高さ。文書の単位。決めなければ書かない
    pub height: Option<f64>,
    /// 幅の倍率
    pub width: Option<f64>,
    /// 傾き（度）
    pub oblique: Option<f64>,
    /// 左右を裏返す
    pub backward: Option<bool>,
    /// 上下を反転する
    pub upside_down: Option<bool>,
    /// 縦書き
    pub vertical: Option<bool>,
}

impl TextStyleItem {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let name = object.required("name", json::string)?;
        let hash = object.required("hash", json::string)?;
        let defined = object.required("defined", json::boolean)?;
        let font = object.optional("font", Font::from_json)?;
        let big_font = object.optional("big_font", Font::from_json)?;
        let height = object.optional("height", json::number)?;
        let width = object.optional("width", json::number)?;
        let oblique = object.optional("oblique", json::number)?;
        let backward = object.optional("backward", json::boolean)?;
        let upside_down = object.optional("upside_down", json::boolean)?;
        let vertical = object.optional("vertical", json::boolean)?;
        Ok(Self {
            id,
            name,
            hash,
            defined,
            font,
            big_font,
            height,
            width,
            oblique,
            backward,
            upside_down,
            vertical,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("name");
        out.string(&self.name);
        out.key("hash");
        out.string(&self.hash);
        out.key("defined");
        out.bool(self.defined);
        if let Some(value) = &self.font {
            out.key("font");
            value.to_json(out);
        }
        if let Some(value) = &self.big_font {
            out.key("big_font");
            value.to_json(out);
        }
        if let Some(value) = &self.height {
            out.key("height");
            out.float(*value);
        }
        if let Some(value) = &self.width {
            out.key("width");
            out.float(*value);
        }
        if let Some(value) = &self.oblique {
            out.key("oblique");
            out.float(*value);
        }
        if let Some(value) = &self.backward {
            out.key("backward");
            out.bool(*value);
        }
        if let Some(value) = &self.upside_down {
            out.key("upside_down");
            out.bool(*value);
        }
        if let Some(value) = &self.vertical {
            out.key("vertical");
            out.bool(*value);
        }
    }
}

impl FromJson for TextStyleItem {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for TextStyleItem {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// フォントの指定。取得と字形の評価は使い手が行う
#[derive(Clone, Debug, PartialEq)]
pub struct Font {
    /// 元の名前。形のファイルならファイルの名前、名前で指すフォントなら書体の名前。翻訳しない
    pub name: String,
    /// 種類
    pub kind: FontKind,
    /// 太字。名前で指すフォントだけ
    pub bold: Option<bool>,
    /// 斜体。名前で指すフォントだけ
    pub italic: Option<bool>,
    /// 文字集合の番号（128 は Shift_JIS）。名前で指すフォントで、元の形式が決めていれば
    pub charset: Option<u64>,
    /// ピッチと系統の番号。名前で指すフォントで、元の形式が決めていれば
    pub pitch: Option<u64>,
}

impl Font {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.required("name", json::string)?;
        let kind = object.required("kind", FontKind::from_json)?;
        let bold = object.optional("bold", json::boolean)?;
        let italic = object.optional("italic", json::boolean)?;
        let charset = object.optional("charset", |value| json::uint(value, 0, 9007199254740991))?;
        let pitch = object.optional("pitch", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            name,
            kind,
            bold,
            italic,
            charset,
            pitch,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("name");
        out.string(&self.name);
        out.key("kind");
        self.kind.to_json(out);
        if let Some(value) = &self.bold {
            out.key("bold");
            out.bool(*value);
        }
        if let Some(value) = &self.italic {
            out.key("italic");
            out.bool(*value);
        }
        if let Some(value) = &self.charset {
            out.key("charset");
            out.uint(*value);
        }
        if let Some(value) = &self.pitch {
            out.key("pitch");
            out.uint(*value);
        }
    }
}

impl FromJson for Font {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Font {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// フォントの種類。shape は形のファイル（SHX など）、named は名前で指すフォント（TrueType など）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FontKind {
    /// `shape`
    Shape,
    /// `named`
    Named,
}

impl FontKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Shape, Self::Named];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Shape => "shape",
            Self::Named => "named",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "shape" => Some(Self::Shape),
            "named" => Some(Self::Named),
            _ => None,
        }
    }
}

impl FromJson for FontKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for FontKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
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

/// バイナリの結果の種類。file は書き出したファイル、image は描いた画像、scene は 2D のシーン（schema/scene.toml の配置、決定 0137・0172）、arrow は表（Arrow の IPC ストリーム。ネイティブでは zm_take_arrow でも取り出せる）
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
    /// engine が `process` のときの、子プロセスにする CLI（zumen）の実行ファイルのパス。書かなければ、ライブラリと同じフォルダーの zumen を使う（決定 0013・0200）
    pub process_path: Option<String>,
}

impl SessionOptions {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let v = object.required("v", |value| json::uint(value, 1, 9007199254740991))?;
        let engine = object.optional("engine", Engine::from_json)?;
        let process_path = object.optional("process_path", json::string)?;
        Ok(Self {
            v,
            engine,
            process_path,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("v");
        out.uint(self.v);
        if let Some(value) = &self.engine {
            out.key("engine");
            value.to_json(out);
        }
        if let Some(value) = &self.process_path {
            out.key("process_path");
            out.string(value);
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

/// 線形（決定 0230）の上の所を、追加距離か測点の表記から求め、点・方向角・高さ・勾配を返す。線形は、要求に書く（alignment）か、open で開いた J-LandXML の文書（doc）の線形を id で指す。distances・stations・interval の所を、この順に並べて返す。求められない所は、理由を診断して除く。doc を渡せば、文書の線形と地表面の要約も返す。図面のモジュールに置く。小版 46 から（doc と id は小版 47 から）
#[derive(Clone, Debug, PartialEq)]
pub struct CivilRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
    /// 線形。doc と一緒には書かない
    pub alignment: Option<CivilAlignment>,
    /// open で開いた J-LandXML の文書の名前。小版 47 から
    pub doc: Option<String>,
    /// doc の線形の ID。文書に線形が 1 つなら書かなくてよい。小版 47 から
    pub id: Option<String>,
    /// 追加距離（メートル）の列
    pub distances: Option<Vec<f64>>,
    /// 測点の表記（No.12+5.000・No.12+5・No.12）の列。測点の不連続のために同じ値が何か所にもあれば、最初の所にして診断する
    pub stations: Option<Vec<String>>,
    /// 線形の初めから終わりまで、この間隔（メートル）ごとの所を足す。終わりの所も足す
    pub interval: Option<f64>,
    /// 測点の間隔（メートル）。書かなければ 20
    pub pitch: Option<f64>,
    /// 測点の表記の端数の小数の桁数。書かなければ 3
    pub digits: Option<u64>,
}

impl CivilRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "civil")?;
        let alignment = object.optional("alignment", CivilAlignment::from_json)?;
        let doc = object.optional("doc", json::string)?;
        let id = object.optional("id", json::string)?;
        let distances = object.optional("distances", |value| json::array(value, json::number))?;
        let stations = object.optional("stations", |value| json::array(value, json::string))?;
        let interval = object.optional("interval", json::number)?;
        let pitch = object.optional("pitch", json::number)?;
        let digits = object.optional("digits", |value| json::uint(value, 0, 9))?;
        Ok(Self {
            request_common,
            alignment,
            doc,
            id,
            distances,
            stations,
            interval,
            pitch,
            digits,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("civil");
        if let Some(value) = &self.alignment {
            out.key("alignment");
            value.to_json(out);
        }
        if let Some(value) = &self.doc {
            out.key("doc");
            out.string(value);
        }
        if let Some(value) = &self.id {
            out.key("id");
            out.string(value);
        }
        if let Some(value) = &self.distances {
            out.key("distances");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.stations {
            out.key("stations");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.interval {
            out.key("interval");
            out.float(*value);
        }
        if let Some(value) = &self.pitch {
            out.key("pitch");
            out.float(*value);
        }
        if let Some(value) = &self.digits {
            out.key("digits");
            out.uint(*value);
        }
    }
}

impl FromJson for CivilRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線形。点は地図の空間（北と東、メートル）に置き、方向角は北から時計回りのラジアンにする。半径は、右へ曲がるものを正にする（決定 0230）
#[derive(Clone, Debug, PartialEq)]
pub struct CivilAlignment {
    /// 平面線形の区間。線形の初めから順に並べる
    pub horizontal: Vec<CivilHorizontal>,
    /// 縦断線形の区間。書かなければ高さを返さない
    pub vertical: Option<Vec<CivilVertical>>,
    /// 線形の初めの測点の値（メートル）。書かなければ 0
    pub start_station: Option<f64>,
    /// 測点の不連続。追加距離の順に並べる
    pub breaks: Option<Vec<CivilStationBreak>>,
}

impl CivilAlignment {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let horizontal = object.required("horizontal", |value| {
            json::array(value, CivilHorizontal::from_json)
        })?;
        let vertical = object.optional("vertical", |value| {
            json::array(value, CivilVertical::from_json)
        })?;
        let start_station = object.optional("start_station", json::number)?;
        let breaks = object.optional("breaks", |value| {
            json::array(value, CivilStationBreak::from_json)
        })?;
        Ok(Self {
            horizontal,
            vertical,
            start_station,
            breaks,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("horizontal");
        out.array(&self.horizontal, |out, item| item.to_json(out));
        if let Some(value) = &self.vertical {
            out.key("vertical");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.start_station {
            out.key("start_station");
            out.float(*value);
        }
        if let Some(value) = &self.breaks {
            out.key("breaks");
            out.array(value, |out, item| item.to_json(out));
        }
    }
}

impl FromJson for CivilAlignment {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilAlignment {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 平面線形の区間
#[derive(Clone, Debug, PartialEq)]
pub struct CivilHorizontal {
    /// 区間の種類
    pub kind: CivilHorizontalKind,
    /// 初めの点の北（メートル）
    pub northing: f64,
    /// 初めの点の東（メートル）
    pub easting: f64,
    /// 初めの方向角（北から時計回りのラジアン）
    pub direction: f64,
    /// 長さ（メートル）
    pub length: f64,
    /// 円弧の半径（メートル）。arc に書く
    pub radius: Option<f64>,
    /// 緩和曲線の初めの半径（メートル）。直線の側は書かない
    pub start_radius: Option<f64>,
    /// 緩和曲線の終わりの半径（メートル）。直線の側は書かない
    pub end_radius: Option<f64>,
    /// 緩和曲線の種類。transition に書き、書かなければ clothoid
    pub curve: Option<CivilTransitionCurve>,
}

impl CivilHorizontal {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let kind = object.required("kind", CivilHorizontalKind::from_json)?;
        let northing = object.required("northing", json::number)?;
        let easting = object.required("easting", json::number)?;
        let direction = object.required("direction", json::number)?;
        let length = object.required("length", json::number)?;
        let radius = object.optional("radius", json::number)?;
        let start_radius = object.optional("start_radius", json::number)?;
        let end_radius = object.optional("end_radius", json::number)?;
        let curve = object.optional("curve", CivilTransitionCurve::from_json)?;
        Ok(Self {
            kind,
            northing,
            easting,
            direction,
            length,
            radius,
            start_radius,
            end_radius,
            curve,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        self.kind.to_json(out);
        out.key("northing");
        out.float(self.northing);
        out.key("easting");
        out.float(self.easting);
        out.key("direction");
        out.float(self.direction);
        out.key("length");
        out.float(self.length);
        if let Some(value) = &self.radius {
            out.key("radius");
            out.float(*value);
        }
        if let Some(value) = &self.start_radius {
            out.key("start_radius");
            out.float(*value);
        }
        if let Some(value) = &self.end_radius {
            out.key("end_radius");
            out.float(*value);
        }
        if let Some(value) = &self.curve {
            out.key("curve");
            value.to_json(out);
        }
    }
}

impl FromJson for CivilHorizontal {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilHorizontal {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 縦断線形の区間
#[derive(Clone, Debug, PartialEq)]
pub struct CivilVertical {
    /// 区間の種類
    pub kind: CivilVerticalKind,
    /// 初めの追加距離（メートル）
    pub start_distance: f64,
    /// 初めの高さ（メートル）
    pub start_height: f64,
    /// 初めの勾配（高さ ÷ 平面の距離）
    pub start_gradient: f64,
    /// 平面の距離での長さ（メートル）
    pub length: f64,
    /// 放物線の終わりの勾配。parabola に書く
    pub end_gradient: Option<f64>,
    /// 円弧の半径（メートル）。上に凸のものを正にする。circular_arc に書く
    pub radius: Option<f64>,
}

impl CivilVertical {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let kind = object.required("kind", CivilVerticalKind::from_json)?;
        let start_distance = object.required("start_distance", json::number)?;
        let start_height = object.required("start_height", json::number)?;
        let start_gradient = object.required("start_gradient", json::number)?;
        let length = object.required("length", json::number)?;
        let end_gradient = object.optional("end_gradient", json::number)?;
        let radius = object.optional("radius", json::number)?;
        Ok(Self {
            kind,
            start_distance,
            start_height,
            start_gradient,
            length,
            end_gradient,
            radius,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        self.kind.to_json(out);
        out.key("start_distance");
        out.float(self.start_distance);
        out.key("start_height");
        out.float(self.start_height);
        out.key("start_gradient");
        out.float(self.start_gradient);
        out.key("length");
        out.float(self.length);
        if let Some(value) = &self.end_gradient {
            out.key("end_gradient");
            out.float(*value);
        }
        if let Some(value) = &self.radius {
            out.key("radius");
            out.float(*value);
        }
    }
}

impl FromJson for CivilVertical {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilVertical {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 平面線形の区間の種類。直線・円弧・緩和曲線
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CivilHorizontalKind {
    /// `line`
    Line,
    /// `arc`
    Arc,
    /// `transition`
    Transition,
}

impl CivilHorizontalKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Line, Self::Arc, Self::Transition];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Line => "line",
            Self::Arc => "arc",
            Self::Transition => "transition",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "line" => Some(Self::Line),
            "arc" => Some(Self::Arc),
            "transition" => Some(Self::Transition),
            _ => None,
        }
    }
}

impl FromJson for CivilHorizontalKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for CivilHorizontalKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 緩和曲線の種類。クロソイド・3 次放物線・サイン半波長逓減曲線・コサイン曲線・ブロス曲線・ヘルマート曲線・ウィーン曲線
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CivilTransitionCurve {
    /// `clothoid`
    Clothoid,
    /// `cubic_parabola`
    CubicParabola,
    /// `sine_half_wave`
    SineHalfWave,
    /// `cosine`
    Cosine,
    /// `bloss`
    Bloss,
    /// `helmert`
    Helmert,
    /// `viennese`
    Viennese,
}

impl CivilTransitionCurve {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Clothoid,
        Self::CubicParabola,
        Self::SineHalfWave,
        Self::Cosine,
        Self::Bloss,
        Self::Helmert,
        Self::Viennese,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Clothoid => "clothoid",
            Self::CubicParabola => "cubic_parabola",
            Self::SineHalfWave => "sine_half_wave",
            Self::Cosine => "cosine",
            Self::Bloss => "bloss",
            Self::Helmert => "helmert",
            Self::Viennese => "viennese",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "clothoid" => Some(Self::Clothoid),
            "cubic_parabola" => Some(Self::CubicParabola),
            "sine_half_wave" => Some(Self::SineHalfWave),
            "cosine" => Some(Self::Cosine),
            "bloss" => Some(Self::Bloss),
            "helmert" => Some(Self::Helmert),
            "viennese" => Some(Self::Viennese),
            _ => None,
        }
    }
}

impl FromJson for CivilTransitionCurve {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for CivilTransitionCurve {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 縦断線形の区間の種類。一定の勾配・2 次放物線・円弧
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CivilVerticalKind {
    /// `constant_gradient`
    ConstantGradient,
    /// `parabola`
    Parabola,
    /// `circular_arc`
    CircularArc,
}

impl CivilVerticalKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::ConstantGradient, Self::Parabola, Self::CircularArc];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::ConstantGradient => "constant_gradient",
            Self::Parabola => "parabola",
            Self::CircularArc => "circular_arc",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "constant_gradient" => Some(Self::ConstantGradient),
            "parabola" => Some(Self::Parabola),
            "circular_arc" => Some(Self::CircularArc),
            _ => None,
        }
    }
}

impl FromJson for CivilVerticalKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for CivilVerticalKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 測点の不連続
#[derive(Clone, Debug, PartialEq)]
pub struct CivilStationBreak {
    /// 追加距離（メートル）
    pub distance: f64,
    /// 戻りの測点の値（不連続の手前から数えた値）
    pub back: f64,
    /// 進みの測点の値（不連続の先で数え直す値）
    pub ahead: f64,
}

impl CivilStationBreak {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let distance = object.required("distance", json::number)?;
        let back = object.required("back", json::number)?;
        let ahead = object.required("ahead", json::number)?;
        Ok(Self {
            distance,
            back,
            ahead,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("distance");
        out.float(self.distance);
        out.key("back");
        out.float(self.back);
        out.key("ahead");
        out.float(self.ahead);
    }
}

impl FromJson for CivilStationBreak {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilStationBreak {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線形の上の所
#[derive(Clone, Debug, PartialEq)]
pub struct CivilResult {
    /// 平面線形の長さ（メートル）
    pub length: f64,
    /// 求めた所
    pub positions: Vec<CivilPosition>,
    /// 所を求めた doc の線形の ID。小版 47 から
    pub alignment: Option<String>,
    /// doc の線形の要約。doc を渡したときに返す。小版 47 から
    pub alignments: Option<Vec<CivilAlignmentSummary>>,
    /// doc の地表面（TIN）の要約。doc を渡したときに返す。小版 47 から
    pub surfaces: Option<Vec<CivilSurfaceSummary>>,
    /// doc の水平の座標系の EPSG のコード。分からなければ書かない。小版 47 から
    pub crs: Option<u64>,
}

impl CivilResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let length = object.required("length", json::number)?;
        let positions = object.required("positions", |value| {
            json::array(value, CivilPosition::from_json)
        })?;
        let alignment = object.optional("alignment", json::string)?;
        let alignments = object.optional("alignments", |value| {
            json::array(value, CivilAlignmentSummary::from_json)
        })?;
        let surfaces = object.optional("surfaces", |value| {
            json::array(value, CivilSurfaceSummary::from_json)
        })?;
        let crs = object.optional("crs", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            length,
            positions,
            alignment,
            alignments,
            surfaces,
            crs,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("length");
        out.float(self.length);
        out.key("positions");
        out.array(&self.positions, |out, item| item.to_json(out));
        if let Some(value) = &self.alignment {
            out.key("alignment");
            out.string(value);
        }
        if let Some(value) = &self.alignments {
            out.key("alignments");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.surfaces {
            out.key("surfaces");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.crs {
            out.key("crs");
            out.uint(*value);
        }
    }
}

impl FromJson for CivilResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線形の要約
#[derive(Clone, Debug, PartialEq)]
pub struct CivilAlignmentSummary {
    /// 線形の ID
    pub id: String,
    /// 名前
    pub name: Option<String>,
    /// 平面線形の長さ（メートル）
    pub length: f64,
    /// 線形の初めの測点の値（メートル）
    pub start_station: f64,
    /// 平面線形の区間の数
    pub horizontal: u64,
    /// 縦断線形の区間の数
    pub vertical: u64,
    /// 測点の不連続の数
    pub breaks: u64,
    /// 横断の数
    pub cross_sections: u64,
}

impl CivilAlignmentSummary {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let name = object.optional("name", json::string)?;
        let length = object.required("length", json::number)?;
        let start_station = object.required("start_station", json::number)?;
        let horizontal =
            object.required("horizontal", |value| json::uint(value, 0, 9007199254740991))?;
        let vertical =
            object.required("vertical", |value| json::uint(value, 0, 9007199254740991))?;
        let breaks = object.required("breaks", |value| json::uint(value, 0, 9007199254740991))?;
        let cross_sections = object.required("cross_sections", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            id,
            name,
            length,
            start_station,
            horizontal,
            vertical,
            breaks,
            cross_sections,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        out.key("length");
        out.float(self.length);
        out.key("start_station");
        out.float(self.start_station);
        out.key("horizontal");
        out.uint(self.horizontal);
        out.key("vertical");
        out.uint(self.vertical);
        out.key("breaks");
        out.uint(self.breaks);
        out.key("cross_sections");
        out.uint(self.cross_sections);
    }
}

impl FromJson for CivilAlignmentSummary {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilAlignmentSummary {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 地表面（TIN）の要約
#[derive(Clone, Debug, PartialEq)]
pub struct CivilSurfaceSummary {
    /// 地表面の ID
    pub id: String,
    /// 名前
    pub name: Option<String>,
    /// 役割。existing（現況）・design（計画）か、元の形式の値
    pub role: String,
    /// 点の数
    pub points: u64,
    /// 三角形の数
    pub triangles: u64,
}

impl CivilSurfaceSummary {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let name = object.optional("name", json::string)?;
        let role = object.required("role", json::string)?;
        let points = object.required("points", |value| json::uint(value, 0, 9007199254740991))?;
        let triangles =
            object.required("triangles", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            id,
            name,
            role,
            points,
            triangles,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        out.key("role");
        out.string(&self.role);
        out.key("points");
        out.uint(self.points);
        out.key("triangles");
        out.uint(self.triangles);
    }
}

impl FromJson for CivilSurfaceSummary {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilSurfaceSummary {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線形の上の 1 つの所
#[derive(Clone, Debug, PartialEq)]
pub struct CivilPosition {
    /// 追加距離（メートル）
    pub distance: f64,
    /// 測点の値（メートル）
    pub station: f64,
    /// 測点の表記。表記にできなければ書かない
    pub label: Option<String>,
    /// 点の北（メートル）
    pub northing: f64,
    /// 点の東（メートル）
    pub easting: f64,
    /// 接線の方向角（北から時計回りのラジアン）
    pub direction: f64,
    /// 縦断線形の高さ（メートル）。求められなければ書かない
    pub height: Option<f64>,
    /// 縦断線形の勾配。高さと同じく、求められなければ書かない
    pub gradient: Option<f64>,
}

impl CivilPosition {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let distance = object.required("distance", json::number)?;
        let station = object.required("station", json::number)?;
        let label = object.optional("label", json::string)?;
        let northing = object.required("northing", json::number)?;
        let easting = object.required("easting", json::number)?;
        let direction = object.required("direction", json::number)?;
        let height = object.optional("height", json::number)?;
        let gradient = object.optional("gradient", json::number)?;
        Ok(Self {
            distance,
            station,
            label,
            northing,
            easting,
            direction,
            height,
            gradient,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("distance");
        out.float(self.distance);
        out.key("station");
        out.float(self.station);
        if let Some(value) = &self.label {
            out.key("label");
            out.string(value);
        }
        out.key("northing");
        out.float(self.northing);
        out.key("easting");
        out.float(self.easting);
        out.key("direction");
        out.float(self.direction);
        if let Some(value) = &self.height {
            out.key("height");
            out.float(*value);
        }
        if let Some(value) = &self.gradient {
            out.key("gradient");
            out.float(*value);
        }
    }
}

impl FromJson for CivilPosition {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilPosition {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// open で開いた点群（LAS・LAZ）の要約と、条件に合う点を返す。点は八分木の浅い段の節から順に選ぶ（決定 0232）。図面のモジュールに置く。小版 48 から
#[derive(Clone, Debug, PartialEq)]
pub struct PointcloudRequest {
    /// `request_common` の欄
    pub request_common: RequestCommon,
    /// open で開いた点群の文書の名前
    pub doc: String,
    /// 点を選ぶ範囲。書かなければ全体
    pub region: Option<PointcloudBox>,
    /// 許容差（点の間隔、メートル）。これより細かい段の点を選ばない。書かなければすべての段
    pub tolerance: Option<f64>,
    /// 返す点の数の上限。書かなければ点を返さず、要約だけを返す
    pub limit: Option<u64>,
}

impl PointcloudRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "pointcloud")?;
        let doc = object.required("doc", json::string)?;
        let region = object.optional("region", PointcloudBox::from_json)?;
        let tolerance = object.optional("tolerance", json::number)?;
        let limit = object.optional("limit", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            request_common,
            doc,
            region,
            tolerance,
            limit,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("pointcloud");
        out.key("doc");
        out.string(&self.doc);
        if let Some(value) = &self.region {
            out.key("region");
            value.to_json(out);
        }
        if let Some(value) = &self.tolerance {
            out.key("tolerance");
            out.float(*value);
        }
        if let Some(value) = &self.limit {
            out.key("limit");
            out.uint(*value);
        }
    }
}

impl FromJson for PointcloudRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PointcloudRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 地図の空間の箱。北・東・標高（メートル）の最小と最大
#[derive(Clone, Debug, PartialEq)]
pub struct PointcloudBox {
    /// 最小の北・東・標高
    pub min: Vec<f64>,
    /// 最大の北・東・標高
    pub max: Vec<f64>,
}

impl PointcloudBox {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let min = object.required("min", |value| json::array(value, json::number))?;
        let max = object.required("max", |value| json::array(value, json::number))?;
        Ok(Self { min, max })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("min");
        out.array(&self.min, |out, item| out.float(*item));
        out.key("max");
        out.array(&self.max, |out, item| out.float(*item));
    }
}

impl FromJson for PointcloudBox {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PointcloudBox {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 点群の要約と、選んだ点
#[derive(Clone, Debug, PartialEq)]
pub struct PointcloudResult {
    /// 点の数
    pub points: u64,
    /// 点の範囲。点がなければ書かない
    pub bounds: Option<PointcloudBox>,
    /// 水平の座標系の EPSG のコード。分からなければ書かない
    pub crs: Option<u64>,
    /// 高さの座標系の EPSG のコード。分からなければ書かない
    pub vertical: Option<u64>,
    /// 点が持つ属性（position・intensity・classification・color・returns・time）
    pub attributes: Vec<String>,
    /// 八分木のいちばん深い段
    pub depth: u64,
    /// 八分木の節の数
    pub nodes: u64,
    /// 選んだ点。limit を書いたときに返す
    pub selected: Option<PointcloudPoints>,
}

impl PointcloudResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let points = object.required("points", |value| json::uint(value, 0, 9007199254740991))?;
        let bounds = object.optional("bounds", PointcloudBox::from_json)?;
        let crs = object.optional("crs", |value| json::uint(value, 0, 9007199254740991))?;
        let vertical =
            object.optional("vertical", |value| json::uint(value, 0, 9007199254740991))?;
        let attributes = object.required("attributes", |value| json::array(value, json::string))?;
        let depth = object.required("depth", |value| json::uint(value, 0, 9007199254740991))?;
        let nodes = object.required("nodes", |value| json::uint(value, 0, 9007199254740991))?;
        let selected = object.optional("selected", PointcloudPoints::from_json)?;
        Ok(Self {
            points,
            bounds,
            crs,
            vertical,
            attributes,
            depth,
            nodes,
            selected,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("points");
        out.uint(self.points);
        if let Some(value) = &self.bounds {
            out.key("bounds");
            value.to_json(out);
        }
        if let Some(value) = &self.crs {
            out.key("crs");
            out.uint(*value);
        }
        if let Some(value) = &self.vertical {
            out.key("vertical");
            out.uint(*value);
        }
        out.key("attributes");
        out.array(&self.attributes, |out, item| out.string(item));
        out.key("depth");
        out.uint(self.depth);
        out.key("nodes");
        out.uint(self.nodes);
        if let Some(value) = &self.selected {
            out.key("selected");
            value.to_json(out);
        }
    }
}

impl FromJson for PointcloudResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PointcloudResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 選んだ点。どの列も、選んだ点の順で同じ長さにする
#[derive(Clone, Debug, PartialEq)]
pub struct PointcloudPoints {
    /// 北・東・標高（メートル）
    pub positions: Vec<Vec<f64>>,
    /// 強度。点群が持つときに返す
    pub intensity: Option<Vec<u64>>,
    /// 分類。点群が持つときに返す
    pub classification: Option<Vec<u64>>,
    /// 色（赤・緑・青。16 ビット）。点群が持つときに返す
    pub color: Option<Vec<Vec<u64>>>,
    /// limit のために選ばなかった点がある
    pub truncated: bool,
    /// 選んだいちばん深い段
    pub level: u64,
}

impl PointcloudPoints {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let positions = object.required("positions", |value| {
            json::array(value, |value| json::array(value, json::number))
        })?;
        let intensity = object.optional("intensity", |value| {
            json::array(value, |value| json::uint(value, 0, 9007199254740991))
        })?;
        let classification = object.optional("classification", |value| {
            json::array(value, |value| json::uint(value, 0, 9007199254740991))
        })?;
        let color = object.optional("color", |value| {
            json::array(value, |value| {
                json::array(value, |value| json::uint(value, 0, 9007199254740991))
            })
        })?;
        let truncated = object.required("truncated", json::boolean)?;
        let level = object.required("level", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            positions,
            intensity,
            classification,
            color,
            truncated,
            level,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("positions");
        out.array(&self.positions, |out, item| {
            out.array(item, |out, item| out.float(*item))
        });
        if let Some(value) = &self.intensity {
            out.key("intensity");
            out.array(value, |out, item| out.uint(*item));
        }
        if let Some(value) = &self.classification {
            out.key("classification");
            out.array(value, |out, item| out.uint(*item));
        }
        if let Some(value) = &self.color {
            out.key("color");
            out.array(value, |out, item| {
                out.array(item, |out, item| out.uint(*item))
            });
        }
        out.key("truncated");
        out.bool(self.truncated);
        out.key("level");
        out.uint(self.level);
    }
}

impl FromJson for PointcloudPoints {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PointcloudPoints {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
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
