//! セッション: 名前のついた入力を持ち、要求を受けて応答を返す（決定 0012）。

mod building_ops;
#[cfg(feature = "drawing")]
mod calibration_ops;
mod civil_ops;
mod conversion_ops;
mod delivery_ops;
mod drawing_ops;
mod pointcloud_ops;
mod review_ops;
mod scene_ops;

use std::collections::BTreeMap;

use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, Stop};
use zumen_base::{Key, Ref, codes, name};
use zumen_drawing::{
    Applied, Drawing2D, Element, Encode, Hash, Leader, Placement, PointMark, PointSize, PointStyle,
    Shape, Unit, binary, id,
};
use zumen_json::{FromJson, Invalid, MAX_SAFE_INTEGER, Problem, ToJson, Writer};
use zumen_sdk::{Apply, Codec};

use crate::abi::{ABI_VERSION, SCHEMA_MAJOR, SCHEMA_MINOR, Status};
#[cfg(feature = "pointcloud")]
use crate::asbuilt;
use crate::bcf;
#[cfg(feature = "drawing")]
use crate::boring;
use crate::bounds;
use crate::building;
use crate::changes::changes;
use crate::check;
#[cfg(any(
    feature = "drawing",
    feature = "bim",
    feature = "civil",
    feature = "pointcloud"
))]
use crate::civil;
use crate::convert;
#[cfg(feature = "drawing")]
use crate::delivery;
use crate::describe;
use crate::formats::{self, DrawingFile};
#[cfg(feature = "civil")]
use crate::geo;
use crate::geometry;
use crate::messages::{
    self, ApplyRequest, ApplyResult, BcfReadRequest, BcfReadResult, BcfWriteRequest,
    BcfWriteResult, BlobKind, ByteRange, CapabilitiesResult, CheckRequest, CheckResult,
    CloseRequest, CloseResult, ConvertRequest, ConvertResult, DrawingRequest, DrawingResult,
    ElementCounts, Engine, EntitiesRequest, EntitiesResult, Entity, EntityField, EntityKind,
    Extent, Format, IdsCheckRequest, IdsCheckResult, IdsFinding, LiftRequest, LiftResult,
    OpenRequest, OpenResult, PointStyleSummary, Request, RequestCommon, ReviewAdoptIdsRequest,
    ReviewAdoptRequest, ReviewAdoptResult, ReviewCarryRequest, ReviewCarryResult, SceneRequest,
    SceneResult, SchemaVersion, SessionOptions, SheetSummary, Source, SpaceSummary, TablesRequest,
    TablesResult,
};
#[cfg(feature = "drawing")]
use crate::photo;
use crate::plugin;
#[cfg(feature = "pointcloud")]
use crate::pointcloud;
use crate::review;
use crate::scene;
use crate::tables::{
    StyleIds, dimension_style_item, layer_item, line_type_item, table_id, text_style_item,
};
use crate::validate;

/// entities の op で、limit を書かなかったときに返す要素の数（決定 0117）
const ENTITIES_LIMIT: u64 = 1000;

/// このライブラリの実行の形態（決定 0013）
const BUILT: Engine = if cfg!(target_family = "wasm") {
    Engine::Wasm
} else {
    Engine::Native
};

/// 名前のついた入力を持つセッション。
///
/// 1 つのファイルを読むときも、セッションを作り、[`Session::push`] で入力を渡し、`open` の要求を送る。
/// 入力は、ホストが読んで渡す。コアはファイルシステムを読まない（決定 0007）。
#[derive(Debug)]
pub struct Session {
    engine: Engine,
    inputs: BTreeMap<String, Vec<u8>>,
    /// 一つの名前で元の文書と、そこから作った領域モデルを一緒に管理する（決定 0219・0235）。
    docs: BTreeMap<String, Document>,
    /// 実行中の op の名前。使えない文書の診断に書く
    op: &'static str,
    /// 直前の呼び出しが作ったバイナリの結果。取り出したものは `None` になる
    blobs: Vec<Option<Blob>>,
    /// 実行の形態が `process` なら、子プロセスの CLI。要求をそのまま送る（決定 0013・0200）
    #[cfg(not(target_family = "wasm"))]
    child: Option<crate::process::Child>,
}

/// 一つの元文書。IFC から読んだ線形は、その建物と一緒に登録・更新・破棄する。
#[derive(Debug)]
enum Document {
    Drawing {
        file: Box<DrawingFile>,
        hash: Hash,
    },
    Building {
        model: Box<building::Model>,
        civil: Option<Box<zumen_civil::CivilModel>>,
    },
    Civil(Box<zumen_civil::CivilModel>),
    Geo(Box<zumen_geo::GeoModel>),
    Cloud(Box<zumen_pointcloud::PointCloud>),
}

impl Document {
    #[cfg(feature = "bim")]
    fn from_building(model: building::Model, cx: &mut Cx<'_>) -> Result<Self, Failed> {
        formats::ReadDocument::building(model, cx).map(Self::from)
    }
}

impl From<formats::ReadDocument> for Document {
    fn from(document: formats::ReadDocument) -> Self {
        match document {
            formats::ReadDocument::Drawing { file, hash } => Self::Drawing { file, hash },
            formats::ReadDocument::Building { model, civil } => Self::Building { model, civil },
            formats::ReadDocument::Civil(model) => Self::Civil(model),
            formats::ReadDocument::Geo(model) => Self::Geo(model),
            formats::ReadDocument::Cloud(cloud) => Self::Cloud(cloud),
        }
    }
}

#[derive(Debug)]
struct Blob {
    kind: BlobKind,
    bytes: Vec<u8>,
}

/// 要求への答え
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    /// 状態。結果があれば [`Status::Ok`]、なければ [`Status::Failed`]
    pub status: Status,
    /// 応答の JSON（`schema/messages.json` の `response`）
    pub response: Vec<u8>,
}

/// op が失敗したこと。理由の診断は、文脈に足してある
pub(crate) struct Failed;

/// 変換の各段階の診断を一緒に集める。処理が報告した失敗と、収集自体の予算による停止をどちらも返す。
fn in_input_order<T>(
    cx: &mut Cx<'_>,
    operation: impl FnOnce(&mut Cx<'_>) -> Result<T, Failed>,
) -> Result<T, Failed> {
    cx.with_diagnostics_in_record_order(|cx| Ok::<_, Stop>(operation(cx)))
        .map_err(|_| Failed)?
}

/// 診断を足して、失敗にする
pub(crate) fn fail(cx: &mut Cx<'_>, diagnostic: Diagnostic) -> Failed {
    cx.report(diagnostic);
    Failed
}

/// 止める理由を診断にして、失敗にする
pub(crate) fn stopped(cx: &mut Cx<'_>, stop: Stop) -> Failed {
    fail(cx, stop.to_diagnostic())
}

/// 要求の値の型か範囲が合わないことの診断
/// 文書の種類が op に合わない
fn unsupported(name: &str, op: &str) -> Diagnostic {
    Diagnostic::new(codes::CALL_DOCUMENT_UNSUPPORTED)
        .with("name", name)
        .with("op", op)
}

pub(crate) fn invalid_value(pointer: impl Into<String>) -> Diagnostic {
    Diagnostic::new(codes::CALL_VALUE_INVALID).with("pointer", pointer.into())
}

/// 名指しした入力がないことの診断
pub(crate) fn input_missing(name: &str) -> Diagnostic {
    Diagnostic::new(codes::CALL_INPUT_MISSING).with("name", name)
}

impl Session {
    /// 設定の JSON（`session_options`）からセッションを作る。設定が空なら既定の設定にする。
    ///
    /// 設定が読めなければ [`Status::Invalid`]、このライブラリにない実行の形態なら [`Status::Failed`] を返す。
    pub fn new(options: &[u8]) -> Result<Self, Status> {
        Self::for_engine(BUILT, options)
    }

    pub(crate) fn for_engine(built: Engine, options: &[u8]) -> Result<Self, Status> {
        let (engine, process_path) = if options.is_empty() {
            (built, None)
        } else {
            let value = zumen_json::parse(options).map_err(|_| Status::Invalid)?;
            let options = SessionOptions::from_json(&value).map_err(|_| Status::Invalid)?;
            if options.v != SCHEMA_MAJOR {
                return Err(Status::Invalid);
            }
            (options.engine.unwrap_or(built), options.process_path)
        };
        // process（子プロセスの CLI）は、ネイティブのライブラリだけが起こせる。起こせなければ failed
        #[cfg(not(target_family = "wasm"))]
        let child = if engine == Engine::Process && built == Engine::Native {
            let path = crate::process::executable(process_path.as_deref()).ok_or(Status::Failed)?;
            Some(crate::process::Child::spawn(&path).ok_or(Status::Failed)?)
        } else if engine != built {
            return Err(Status::Failed);
        } else {
            None
        };
        #[cfg(target_family = "wasm")]
        {
            let _ = process_path;
            if engine != built {
                return Err(Status::Failed);
            }
        }
        Ok(Self {
            engine,
            inputs: BTreeMap::new(),
            docs: BTreeMap::new(),
            op: "",
            blobs: Vec::new(),
            #[cfg(not(target_family = "wasm"))]
            child,
        })
    }

    /// 名前のついた入力に、バイト列を足す。同じ名前で何回か呼ぶと、後ろに足していく。
    ///
    /// 名前が UTF-8 でないか、安全な相対の名前（[`zumen_base::name::is_safe`]）でなければ、[`Status::Invalid`] を返す。
    pub fn push(&mut self, name: &[u8], data: &[u8]) -> Result<(), Status> {
        let name = core::str::from_utf8(name).map_err(|_| Status::Invalid)?;
        if !name::is_safe(name) {
            return Err(Status::Invalid);
        }
        #[cfg(not(target_family = "wasm"))]
        if let Some(child) = &mut self.child {
            return child.push(name.as_bytes(), data);
        }
        self.inputs
            .entry(name.to_string())
            .or_default()
            .extend_from_slice(data);
        Ok(())
    }

    /// wasm のホストが確保した入力を引き取る。最初の入力と空の入力には、その領域をそのまま使う（決定 0271）
    #[cfg(any(target_family = "wasm", test))]
    pub(crate) fn push_owned(&mut self, name: &[u8], data: Vec<u8>) -> Result<(), Status> {
        let name = core::str::from_utf8(name).map_err(|_| Status::Invalid)?;
        if !name::is_safe(name) {
            return Err(Status::Invalid);
        }
        #[cfg(not(target_family = "wasm"))]
        if let Some(child) = &mut self.child {
            return child.push(name.as_bytes(), &data);
        }
        let input = self.inputs.entry(name.to_string()).or_default();
        if input.is_empty() {
            *input = data;
        } else {
            input.extend_from_slice(&data);
        }
        Ok(())
    }

    /// 要求の JSON を受けて、応答の JSON を返す。直前の呼び出しのバイナリの結果は、ここで捨てる。
    ///
    /// `cancel` は、この呼び出しの中断の合図。時間の上限があれば、その合図で止める。
    pub fn call(&mut self, request: &[u8], cancel: &CancelToken) -> Reply {
        self.blobs.clear();
        #[cfg(not(target_family = "wasm"))]
        if let Some(child) = &mut self.child {
            return process_reply(child.call(request, cancel));
        }
        let mut diagnostics = Diagnostics::new();
        let result = self.run(request, cancel, &mut diagnostics);
        let status = if result.is_some() {
            Status::Ok
        } else {
            Status::Failed
        };
        Reply {
            status,
            response: respond(result.as_deref(), &diagnostics, &self.blobs),
        }
    }

    /// 直前の呼び出しが作ったバイナリの結果を取り出す。番号がないか、取り出したあとなら `None`
    pub fn take(&mut self, blob: u32) -> Option<Vec<u8>> {
        #[cfg(not(target_family = "wasm"))]
        if let Some(child) = &mut self.child {
            return child.take(blob);
        }
        let slot = self.blobs.get_mut(usize::try_from(blob).ok()?)?;
        slot.take().map(|blob| blob.bytes)
    }

    /// コアの不具合で止まったときの応答。`internal.panic.caught` の診断だけを持つ
    pub fn panic_response() -> Vec<u8> {
        let mut diagnostics = Diagnostics::new();
        diagnostics.report(Diagnostic::new(codes::INTERNAL_PANIC_CAUGHT));
        respond(None, &diagnostics, &[])
    }

    fn run(
        &mut self,
        request: &[u8],
        cancel: &CancelToken,
        diagnostics: &mut Diagnostics,
    ) -> Option<Box<dyn ToJson>> {
        let value = match zumen_json::parse(request) {
            Ok(value) => value,
            Err(error) => {
                diagnostics
                    .report(Diagnostic::new(codes::CALL_JSON_INVALID).with("offset", error.offset));
                return None;
            }
        };
        // バージョンを先に確かめる。バージョンが違えば、ほかのフィールドの形も違いうる
        if let Some(version) = value
            .pointer("/v")
            .and_then(|v| zumen_json::uint(v, 0, MAX_SAFE_INTEGER as u64).ok())
            && version != SCHEMA_MAJOR
        {
            diagnostics
                .report(Diagnostic::new(codes::CALL_VERSION_UNSUPPORTED).with("version", version));
            return None;
        }
        // wasm のモジュールは、持つ op だけを入れて作る。持たない op は、フィールドを読む前に断る（決定 0013・0221）
        if let Some(zumen_json::Value::String(op)) = value.pointer("/op")
            && Request::TAGS.contains(&op.as_str())
            && !crate::abi::has_op(op)
        {
            diagnostics.report(Diagnostic::new(codes::CALL_OP_UNSUPPORTED).with("op", op.as_str()));
            return None;
        }
        let request = match Request::from_json(&value) {
            Ok(request) => request,
            Err(invalid) => {
                diagnostics.report(request_problem(&invalid));
                return None;
            }
        };
        let common = common(&request);
        if common.time_limit_ms.is_some() && self.engine == Engine::Wasm {
            // wasm のモジュールの中には時計がない。ホストの手段で止める（決定 0010）
            diagnostics.report(
                Diagnostic::new(codes::CALL_FIELD_UNSUPPORTED).with("pointer", "/time_limit_ms"),
            );
            return None;
        }
        *diagnostics = Diagnostics::with_max_places(
            common
                .max_places
                .map_or(Diagnostics::DEFAULT_MAX_PLACES, |places| {
                    usize::try_from(places).unwrap_or(usize::MAX)
                }),
        );
        let mut budget = Budget::new(limits(common.budget.as_ref()));
        let _watchdog = common
            .time_limit_ms
            .and_then(|ms| watchdog::Watchdog::start(cancel, ms));
        let mut cx = Cx::new(&mut budget, diagnostics, cancel);
        self.op = request.tag();
        let result: Result<Box<dyn ToJson>, Failed> = match &request {
            Request::Capabilities(_) => Ok(Box::new(capabilities())),
            Request::Open(request) => self.open(request, &mut cx).map(|r| Box::new(r) as _),
            Request::Close(request) => self.close(request, &mut cx).map(|r| Box::new(r) as _),
            Request::Describe(request) => {
                describe::describe(&self.inputs, request, &mut cx).map(|r| Box::new(r) as _)
            }
            #[cfg(feature = "drawing")]
            Request::Drawing(request) => self.drawing(request, &mut cx).map(|r| Box::new(r) as _),
            #[cfg(feature = "drawing")]
            Request::Entities(request) => self.entities(request, &mut cx).map(|r| Box::new(r) as _),
            #[cfg(feature = "drawing")]
            Request::Apply(request) => self.apply(request, &mut cx).map(|r| Box::new(r) as _),
            #[cfg(feature = "drawing")]
            Request::Convert(request) => self.convert(request, &mut cx).map(|r| Box::new(r) as _),
            #[cfg(feature = "drawing")]
            Request::ConvertEstimate(request) => self
                .convert_estimate(request, &mut cx)
                .map(|r| Box::new(r) as _),
            #[cfg(feature = "drawing")]
            Request::Tables(request) => self.tables(request, &mut cx).map(|r| Box::new(r) as _),
            #[cfg(feature = "drawing")]
            Request::Calibrations(request) => self
                .calibrations(request, &mut cx)
                .map(|r| Box::new(r) as _),
            #[cfg(feature = "drawing")]
            Request::MeasureDistance(request) => self
                .measure_distance(request, &mut cx)
                .map(|r| Box::new(r) as _),
            #[cfg(feature = "drawing")]
            Request::Scene(request) => self.scene(request, &mut cx).map(|r| Box::new(r) as _),
            #[cfg(feature = "drawing")]
            Request::Render(request) => self.render(request, &mut cx).map(|r| Box::new(r) as _),
            #[cfg(feature = "drawing")]
            Request::Check(request) => self.check(request, &mut cx).map(|r| Box::new(r) as _),
            #[cfg(feature = "bim")]
            Request::Validate(request) => {
                validate::validate(&self.inputs, request, &mut cx).map(|r| Box::new(r) as _)
            }
            Request::ReviewCarry(request) => self
                .review_carry(request, &mut cx)
                .map(|r| Box::new(r) as _),
            #[cfg(feature = "drawing")]
            Request::ReviewAdopt(request) => self
                .review_adopt(request, &mut cx)
                .map(|r| Box::new(r) as _),
            #[cfg(feature = "bim")]
            Request::Building(request) => self
                .model(&request.doc, "building", &mut cx)
                .map(|model| Box::new(building::summary(model)) as _),
            #[cfg(feature = "bim")]
            Request::BuildingElements(request) => self
                .model(&request.doc, "building_elements", &mut cx)
                .and_then(|model| building::elements(model, request, &mut cx))
                .map(|r| Box::new(r) as _),
            #[cfg(feature = "bim")]
            Request::BuildingApply(request) => self
                .building_apply(request, &mut cx)
                .map(|r| Box::new(r) as _),
            #[cfg(feature = "bim")]
            Request::IdsCheck(request) => {
                self.ids_check(request, &mut cx).map(|r| Box::new(r) as _)
            }
            #[cfg(feature = "bim")]
            Request::ReviewAdoptIds(request) => self
                .review_adopt_ids(request, &mut cx)
                .map(|r| Box::new(r) as _),
            #[cfg(feature = "bim")]
            Request::BcfRead(request) => self.bcf_read(request, &mut cx).map(|r| Box::new(r) as _),
            #[cfg(feature = "bim")]
            Request::BcfWrite(request) => {
                self.bcf_write(request, &mut cx).map(|r| Box::new(r) as _)
            }
            #[cfg(any(
                feature = "drawing",
                feature = "bim",
                feature = "civil",
                feature = "pointcloud"
            ))]
            Request::Civil(request) => self.civil(request, &mut cx).map(|r| Box::new(r) as _),
            #[cfg(feature = "pointcloud")]
            Request::Pointcloud(request) => {
                self.pointcloud(request, &mut cx).map(|r| Box::new(r) as _)
            }
            #[cfg(feature = "pointcloud")]
            Request::Deviation(request) => {
                self.deviation(request, &mut cx).map(|r| Box::new(r) as _)
            }
            #[cfg(feature = "drawing")]
            Request::CivilDrawing(request) => self
                .civil_drawing(request, &mut cx)
                .map(|r| Box::new(r) as _),
            #[cfg(feature = "civil")]
            Request::Geo(request) => self.geo(request, &mut cx).map(|r| Box::new(r) as _),
            #[cfg(feature = "drawing")]
            Request::DeliveryCheck(request) => self
                .delivery_check(request, &mut cx)
                .map(|r| Box::new(r) as _),
            #[cfg(feature = "drawing")]
            Request::Photo(request) => self
                .input(&request.input, "/input", &mut cx)
                .and_then(|bytes| photo::photo(bytes, &mut cx))
                .map(|r| Box::new(r) as _),
            #[cfg(any(feature = "drawing", feature = "plugin"))]
            Request::Lift(request) => self.lift_binary(request, &mut cx).map(|r| Box::new(r) as _),
            #[cfg(feature = "drawing")]
            Request::Boring(request) => self
                .input(&request.input, "/input", &mut cx)
                .and_then(|bytes| boring::boring(bytes, &mut cx))
                .map(|r| Box::new(r) as _),
            // 入れなかったモジュールの op は、上で断った
            #[allow(unreachable_patterns)]
            _ => Err(Failed),
        };
        result.ok()
    }

    fn check_new_document(&self, name: &str, cx: &mut Cx<'_>) -> Result<(), Failed> {
        if self.docs.contains_key(name) {
            return Err(fail(
                cx,
                Diagnostic::new(codes::CALL_DOCUMENT_EXISTS).with("name", name),
            ));
        }
        Ok(())
    }

    fn document(&self, name: &str, cx: &mut Cx<'_>) -> Result<&Document, Failed> {
        self.docs.get(name).ok_or_else(|| {
            fail(
                cx,
                Diagnostic::new(codes::CALL_DOCUMENT_MISSING).with("name", name),
            )
        })
    }

    fn open(&mut self, request: &OpenRequest, cx: &mut Cx<'_>) -> Result<OpenResult, Failed> {
        if !name::is_safe(&request.input) {
            return Err(fail(cx, invalid_value("/input")));
        }
        if request
            .doc
            .as_deref()
            .is_some_and(|doc| !name::is_safe(doc))
        {
            return Err(fail(cx, invalid_value("/doc")));
        }
        let Some(bytes) = self.inputs.get(&request.input) else {
            return Err(fail(cx, input_missing(&request.input)));
        };
        let name = request.doc.as_ref().unwrap_or(&request.input);
        self.check_new_document(name, cx)?;
        let opened = formats::read(request, bytes, &self.inputs, cx)?;
        if let Some(document) = opened.document {
            self.docs.insert(name.clone(), document.into());
        }
        Ok(opened.result)
    }

    fn close(&mut self, request: &CloseRequest, cx: &mut Cx<'_>) -> Result<CloseResult, Failed> {
        let docs = request.docs.as_deref().unwrap_or_default();
        let inputs = request.inputs.as_deref().unwrap_or_default();
        // 名前の形を先に確かめる
        for (field, names) in [("docs", docs), ("inputs", inputs)] {
            for (index, name) in names.iter().enumerate() {
                cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
                if !name::is_safe(name) {
                    return Err(fail(cx, invalid_value(format!("/{field}/{index}"))));
                }
            }
        }
        let mut missing = false;
        for doc in docs {
            if !self.docs.contains_key(doc) {
                cx.report(Diagnostic::new(codes::CALL_DOCUMENT_MISSING).with("name", doc.as_str()));
                missing = true;
            }
        }
        for input in inputs {
            if !self.inputs.contains_key(input) {
                cx.report(input_missing(input));
                missing = true;
            }
        }
        // 1 つでもなければ、何も捨てない
        if missing {
            return Err(Failed);
        }
        for doc in docs {
            self.docs.remove(doc);
        }
        for input in inputs {
            self.inputs.remove(input);
        }
        Ok(CloseResult {})
    }

    /// 入力の名前のバイト列。`pointer` は名前を書いたフィールド
    fn input(&self, name: &str, pointer: &str, cx: &mut Cx<'_>) -> Result<&[u8], Failed> {
        if !name::is_safe(name) {
            return Err(fail(cx, invalid_value(pointer)));
        }
        match self.inputs.get(name) {
            Some(bytes) => Ok(bytes),
            None => Err(fail(cx, input_missing(name))),
        }
    }

    #[cfg(test)]
    fn add_blob(&mut self, kind: BlobKind, bytes: Vec<u8>) {
        self.blobs.push(Some(Blob { kind, bytes }));
    }
}

fn capabilities() -> CapabilitiesResult {
    CapabilitiesResult {
        package: env!("CARGO_PKG_VERSION").to_string(),
        abi: u64::from(ABI_VERSION),
        schema: SchemaVersion {
            major: SCHEMA_MAJOR,
            minor: SCHEMA_MINOR,
        },
        // wasm のモジュールは、持つ op と形式だけを返す（決定 0221）
        ops: Request::TAGS
            .iter()
            .filter(|op| crate::abi::has_op(op))
            .map(|op| op.to_string())
            .collect(),
        // 登録した形式（#398）
        formats: formats::DRAWING
            .iter()
            .chain(formats::READ_ONLY)
            .filter(|registration| crate::abi::has_format(registration.format))
            .chain(
                plugin::installed()
                    .iter()
                    .filter(|plugin| match plugin.read {
                        plugin::Reader::Drawing(_) => {
                            cfg!(any(feature = "drawing", feature = "plugin"))
                        }
                        #[cfg(feature = "bim")]
                        plugin::Reader::Building(_) => true,
                    })
                    .map(|plugin| &plugin.registration),
            )
            .map(|registration| Format {
                id: registration.format.to_string(),
                family: registration.family.to_string(),
                read: true,
                write: registration.write,
                id_rules: registration.id_rules,
            })
            .collect(),
    }
}

pub(crate) fn unit_of(unit: Unit) -> messages::Unit {
    match unit {
        Unit::Millimeter => messages::Unit::Millimeter,
        Unit::Centimeter => messages::Unit::Centimeter,
        Unit::Meter => messages::Unit::Meter,
        Unit::Inch => messages::Unit::Inch,
        Unit::Foot => messages::Unit::Foot,
        Unit::Unknown => messages::Unit::Unknown,
    }
}

fn common(request: &Request) -> &RequestCommon {
    match request {
        Request::Capabilities(request) => &request.request_common,
        Request::Open(request) => &request.request_common,
        Request::Close(request) => &request.request_common,
        Request::Describe(request) => &request.request_common,
        Request::Drawing(request) => &request.request_common,
        Request::Entities(request) => &request.request_common,
        Request::Apply(request) => &request.request_common,
        Request::Convert(request) => &request.request_common,
        Request::ConvertEstimate(request) => &request.request_common,
        Request::Tables(request) => &request.request_common,
        Request::Calibrations(request) => &request.request_common,
        Request::MeasureDistance(request) => &request.request_common,
        Request::Scene(request) => &request.request_common,
        Request::Render(request) => &request.request_common,
        Request::Check(request) => &request.request_common,
        Request::Validate(request) => &request.request_common,
        Request::ReviewCarry(request) => &request.request_common,
        Request::ReviewAdopt(request) => &request.request_common,
        Request::Building(request) => &request.request_common,
        Request::BuildingElements(request) => &request.request_common,
        Request::BuildingApply(request) => &request.request_common,
        Request::IdsCheck(request) => &request.request_common,
        Request::ReviewAdoptIds(request) => &request.request_common,
        Request::BcfRead(request) => &request.request_common,
        Request::BcfWrite(request) => &request.request_common,
        Request::Civil(request) => &request.request_common,
        Request::Pointcloud(request) => &request.request_common,
        Request::Deviation(request) => &request.request_common,
        Request::CivilDrawing(request) => &request.request_common,
        Request::Geo(request) => &request.request_common,
        Request::DeliveryCheck(request) => &request.request_common,
        Request::Photo(request) => &request.request_common,
        Request::Boring(request) => &request.request_common,
        Request::Lift(request) => &request.request_common,
    }
}

/// 要求がスキーマに合わない理由を、診断にする
fn request_problem(invalid: &Invalid) -> Diagnostic {
    let pointer = invalid.pointer();
    match invalid.problem() {
        Problem::Tag(op) if pointer == "/op" => {
            Diagnostic::new(codes::CALL_OP_UNSUPPORTED).with("op", op.as_str())
        }
        Problem::Missing => Diagnostic::new(codes::CALL_FIELD_MISSING).with("pointer", pointer),
        Problem::Unknown => Diagnostic::new(codes::CALL_FIELD_UNSUPPORTED).with("pointer", pointer),
        Problem::Tag(_) | Problem::Type | Problem::Value => invalid_value(pointer),
    }
}

/// 要求の予算のフィールド
fn budget_field(budget: &messages::Budget, kind: Kind) -> Option<&messages::Limit> {
    match kind {
        Kind::InputBytes => budget.input_bytes.as_ref(),
        Kind::DecodedBytes => budget.decoded_bytes.as_ref(),
        Kind::Records => budget.records.as_ref(),
        Kind::Elements => budget.elements.as_ref(),
        Kind::Depth => budget.depth.as_ref(),
        Kind::Segments => budget.segments.as_ref(),
        Kind::Work => budget.work.as_ref(),
    }
}

/// 要求の予算から、上限を作る。書かない単位は既定値にする（決定 0010）
fn limits(budget: Option<&messages::Budget>) -> Limits {
    let mut limits = Limits::default();
    if let Some(budget) = budget {
        for kind in Kind::ALL {
            match budget_field(budget, kind) {
                Some(messages::Limit::Integer(limit)) => limits.set(kind, Limit::Finite(*limit)),
                Some(messages::Limit::Unlimited) => limits.set(kind, Limit::Unlimited),
                None => {}
            }
        }
    }
    limits
}

/// 子プロセスの答えを、呼び出しの答えにする。子プロセスを止めたときは、その理由の診断で失敗する
#[cfg(not(target_family = "wasm"))]
fn process_reply(answer: Result<(Status, Option<Vec<u8>>), crate::process::Failure>) -> Reply {
    use crate::process::Failure;
    let code = match answer {
        Ok((status, Some(response))) => return Reply { status, response },
        // 応答のない状態は、子プロセスの中のセッションが使えないときだけ起きる
        Ok((_, None)) | Err(Failure::Caught) => codes::CALL_PROCESS_CAUGHT,
        Err(Failure::Cancelled) => codes::CALL_REQUEST_CANCELLED,
        Err(Failure::TimedOut) => codes::BUDGET_TIME_EXCEEDED,
    };
    let mut diagnostics = Diagnostics::new();
    diagnostics.report(Diagnostic::new(code));
    Reply {
        status: Status::Failed,
        response: respond(None, &diagnostics, &[]),
    }
}

/// 応答の JSON（`schema/messages.json` の `response`）を書く
fn respond(
    result: Option<&dyn ToJson>,
    diagnostics: &Diagnostics,
    blobs: &[Option<Blob>],
) -> Vec<u8> {
    let mut out = Writer::new();
    out.begin_object();
    out.key("v");
    out.uint(SCHEMA_MAJOR);
    if let Some(result) = result {
        out.key("result");
        result.to_json(&mut out);
    }
    out.key("diagnostics");
    out.begin_array();
    for diagnostic in diagnostics {
        message_of(diagnostic).to_json(&mut out);
    }
    out.end_array();
    out.key("blobs");
    out.begin_array();
    for (id, blob) in blobs.iter().enumerate() {
        if let Some(blob) = blob {
            messages::Blob {
                id: id as u64,
                kind: blob.kind,
                size: blob.bytes.len() as u64,
            }
            .to_json(&mut out);
        }
    }
    out.end_array();
    out.end_object();
    out.into_bytes()
}

pub(crate) fn message_of(diagnostic: &Diagnostic) -> messages::Diagnostic {
    messages::Diagnostic {
        code: diagnostic.code().name().to_string(),
        severity: match diagnostic.severity() {
            zumen_base::Severity::Info => messages::Severity::Info,
            zumen_base::Severity::Warning => messages::Severity::Warning,
            zumen_base::Severity::Error => messages::Severity::Error,
        },
        fidelity: diagnostic.fidelity().map(|fidelity| match fidelity {
            zumen_base::Fidelity::Equivalent => messages::Fidelity::Equivalent,
            zumen_base::Fidelity::Approximated => messages::Fidelity::Approximated,
            zumen_base::Fidelity::Drawn => messages::Fidelity::Drawn,
            zumen_base::Fidelity::Dropped => messages::Fidelity::Dropped,
        }),
        count: diagnostic.count(),
        at: diagnostic
            .places()
            .iter()
            .map(|place| messages::Place {
                id: place.id().map(str::to_string),
                record: place.record(),
                bytes: place.bytes().map(|range| messages::ByteRange {
                    start: range.start,
                    end: range.end,
                }),
            })
            .collect(),
        cite: diagnostic.spec().map(|spec| messages::Cite {
            spec: spec.spec.to_string(),
            section: spec.section.to_string(),
        }),
        params: diagnostic
            .params()
            .iter()
            .map(|(name, value)| (name.to_string(), param_of(value)))
            .collect(),
    }
}

/// 診断の値を、JSON の数か文字列にする。2^53 − 1 を超える整数と、有限でない浮動小数は、文字列にする（決定 0012）
fn param_of(value: &zumen_base::Value) -> messages::ParamValue {
    use messages::ParamValue;
    use zumen_base::Value;
    let safe = MAX_SAFE_INTEGER as u64;
    match value {
        Value::UInt(n) if *n <= safe => ParamValue::Number(*n as f64),
        Value::Int(n) if n.unsigned_abs() <= safe => ParamValue::Number(*n as f64),
        Value::UInt(n) => ParamValue::String(n.to_string()),
        Value::Int(n) => ParamValue::String(n.to_string()),
        Value::Float(x) if x.is_finite() => ParamValue::Number(*x),
        Value::Float(x) if x.is_nan() => ParamValue::String("NaN".to_string()),
        Value::Float(x) if *x > 0.0 => ParamValue::String("Infinity".to_string()),
        Value::Float(_) => ParamValue::String("-Infinity".to_string()),
        Value::Text(text) => ParamValue::String(text.clone()),
        Value::Key(key) => ParamValue::String((*key).to_string()),
    }
}

/// 時間の上限。ネイティブの API が時計を見て、中断の合図で伝える（決定 0010）
mod watchdog {
    use zumen_base::CancelToken;

    /// 上限の時間がたったら、中断の合図で伝える。落とすと止まる
    #[cfg(not(target_family = "wasm"))]
    pub(super) struct Watchdog {
        done: Option<std::sync::mpsc::Sender<()>>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    #[cfg(not(target_family = "wasm"))]
    impl Watchdog {
        /// 見張りを始める。スレッドを作れなければ `None`（時間の上限は補助なので、なしで続ける）
        pub(super) fn start(cancel: &CancelToken, limit_ms: u64) -> Option<Self> {
            use std::sync::mpsc::{RecvTimeoutError, channel};
            use std::time::Duration;
            let (done, wait) = channel::<()>();
            let cancel = cancel.clone();
            let thread = std::thread::Builder::new()
                .name("zumen-time-limit".to_string())
                .spawn(move || {
                    if let Err(RecvTimeoutError::Timeout) =
                        wait.recv_timeout(Duration::from_millis(limit_ms))
                    {
                        cancel.expire();
                    }
                })
                .ok()?;
            Some(Self {
                done: Some(done),
                thread: Some(thread),
            })
        }
    }

    #[cfg(not(target_family = "wasm"))]
    impl Drop for Watchdog {
        fn drop(&mut self) {
            // 送り手を落とすと、待っているスレッドがすぐに起きる。呼び出しが終わる前に、見張りも終える
            drop(self.done.take());
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    /// wasm では、要求の time_limit_ms を受け付けないので、見張りもない
    #[cfg(target_family = "wasm")]
    pub(super) struct Watchdog;

    #[cfg(target_family = "wasm")]
    impl Watchdog {
        pub(super) fn start(_cancel: &CancelToken, _limit_ms: u64) -> Option<Self> {
            None
        }
    }
}

#[cfg(all(test, feature = "drawing"))]
mod calibration_tests;
#[cfg(test)]
mod tests;
