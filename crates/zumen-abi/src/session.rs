//! セッション: 名前のついた入力を持ち、要求を受けて応答を返す（決定 0012）。

use std::collections::BTreeMap;

use zumen_acad::{Dwg, DwgFile, Dxf, DxfFile};
use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, Stop};
use zumen_base::{Key, Ref, codes, name};
use zumen_drawing::{
    Applied, Drawing2D, Element, Encode, Hash, Leader, Placement, PointMark, PointSize, PointStyle,
    Shape, Unit, id,
};
use zumen_json::{FromJson, Invalid, MAX_SAFE_INTEGER, Problem, ToJson, Writer};
use zumen_jww::{Jww, JwwFile};
use zumen_sdk::{Apply, Codec, Lift, Registration};
use zumen_sxf::{Sxf, SxfFile};

use crate::abi::{ABI_VERSION, SCHEMA_MAJOR, SCHEMA_MINOR, Status};
#[cfg(feature = "drawing")]
use crate::asbuilt;
use crate::bcf;
#[cfg(feature = "drawing")]
use crate::boring;
use crate::bounds;
use crate::building;
use crate::changes::changes;
use crate::check;
#[cfg(feature = "drawing")]
use crate::civil;
use crate::convert;
#[cfg(feature = "drawing")]
use crate::delivery;
use crate::describe;
use crate::formats;
#[cfg(feature = "drawing")]
use crate::geo;
use crate::geometry;
use crate::messages::{
    self, ApplyRequest, ApplyResult, BcfReadRequest, BcfReadResult, BcfWriteRequest,
    BcfWriteResult, BlobKind, ByteRange, CapabilitiesResult, CheckRequest, CheckResult,
    CloseRequest, CloseResult, ConvertRequest, ConvertResult, DrawingRequest, DrawingResult,
    ElementCounts, Engine, EntitiesRequest, EntitiesResult, Entity, EntityField, EntityKind,
    Extent, Format, IdsCheckRequest, IdsCheckResult, IdsFinding, OpenRequest, OpenResult,
    PointStyleSummary, Request, RequestCommon, ReviewAdoptIdsRequest, ReviewAdoptRequest,
    ReviewAdoptResult, ReviewCarryRequest, ReviewCarryResult, SceneRequest, SceneResult,
    SchemaVersion, SessionOptions, SheetSummary, Source, SpaceSummary, TablesRequest, TablesResult,
};
#[cfg(feature = "drawing")]
use crate::packed;
use crate::packed::Packed;
#[cfg(feature = "drawing")]
use crate::photo;
use crate::plugin;
#[cfg(feature = "drawing")]
use crate::pointcloud;
use crate::review;
use crate::scene;
#[cfg(feature = "drawing")]
use crate::sheet;
use crate::tables::{
    StyleIds, dimension_style_item, layer_item, line_type_item, table_id, text_style_item,
};
use crate::validate;

/// entities の op で、limit を書かなかったときに返す要素の数（決定 0117）
const ENTITIES_LIMIT: u64 = 1000;

/// このライブラリの実行の形（決定 0013）
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
    /// `open` で作った文書
    docs: BTreeMap<String, Doc>,
    /// `open` で作った文書の、元のバイト列のハッシュ（決定 0156）
    hashes: BTreeMap<String, Hash>,
    /// `open` で作った建物モデルの文書（決定 0219）。名前は `docs` と分け合う
    models: BTreeMap<String, building::Model>,
    /// `open` で作った土木の領域モデルの文書（J-LandXML、決定 0230）。名前は `docs` と分け合う
    civils: BTreeMap<String, zumen_civil::CivilModel>,
    /// `open` で作った地理の領域モデルの文書（CityGML、決定 0233）。名前は `docs` と分け合う
    geos: BTreeMap<String, zumen_geo::GeoModel>,
    /// 開いた点群（決定 0232）
    clouds: BTreeMap<String, zumen_pointcloud::PointCloud>,
    /// 実行中の op の名前。使えない文書の診断に書く
    op: &'static str,
    /// 直前の呼び出しが作ったバイナリの結果。取り出したものは `None` になる
    blobs: Vec<Option<Blob>>,
    /// 実行の形が `process` なら、子プロセスの CLI。要求をそのまま送る（決定 0013・0200）
    #[cfg(not(target_family = "wasm"))]
    child: Option<crate::process::Child>,
}

/// 開いた文書。形式ごとのファイルモデル（L1）を持つ
#[derive(Debug)]
enum Doc {
    Dxf(DxfFile),
    Dwg(DwgFile),
    /// SXF。P2Z・SFZ なら、書庫の項目も持つ（#385）
    Sxf(SxfFile, Option<Box<Packed>>),
    Jww(JwwFile),
    /// プラグインの形式（決定 0243）。読むだけで、図面モデルを持つ
    Plugin(Registration, Drawing2D),
}

impl Doc {
    /// 形式の台帳のキー
    fn format(&self) -> &'static str {
        match self {
            Doc::Dxf(_) => Dxf::FORMAT,
            Doc::Dwg(_) => Dwg::FORMAT,
            Doc::Sxf(..) => Sxf::FORMAT,
            Doc::Jww(_) => Jww::FORMAT,
            Doc::Plugin(registration, _) => registration.format,
        }
    }

    /// 形式の系統（capabilities の family）
    fn family(&self) -> &'static str {
        match self {
            Doc::Dxf(_) | Doc::Dwg(_) => "acad",
            Doc::Sxf(..) => "sxf",
            Doc::Jww(_) => "jww",
            Doc::Plugin(registration, _) => registration.family,
        }
    }

    /// 図面モデル（L2）に変換する
    fn lift(&self, cx: &mut Cx<'_>) -> Result<Drawing2D, zumen_sdk::Error> {
        match self {
            Doc::Dxf(file) => file.lift(cx),
            Doc::Dwg(file) => file.lift(cx),
            Doc::Sxf(file, _) => file.lift(cx),
            Doc::Jww(file) => file.lift(cx),
            Doc::Plugin(_, drawing) => Ok(drawing.clone()),
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
/// 図面の形式なら読む。どの図面の形式でもなければ `None`。形式は、コーデックが持つ判定の規則で決める
/// （決定 0007・0008）。入力の大きさは、コーデックが確かめる
#[cfg(feature = "drawing")]
fn read_drawing(bytes: &[u8], cx: &mut Cx<'_>) -> Result<Option<Doc>, Failed> {
    // 失敗の理由は、コーデックが診断として足してある
    let format = zumen_sdk::detect(formats::DRAWING, bytes).map(|registration| registration.format);
    let doc = if format == Some(Dxf::FORMAT) {
        Doc::Dxf(Dxf::read(bytes, cx).map_err(|_| Failed)?)
    } else if format == Some(Dwg::FORMAT) {
        // 読めるのは R2000（AC1015）だけ。ほかの版は、コーデックが診断を足して失敗する
        Doc::Dwg(Dwg::read(bytes, cx).map_err(|_| Failed)?)
    } else if format == Some(Sxf::FORMAT) {
        // 同じ始まりの IFC などは、コーデックがスキーマを確かめて、診断を足して失敗する
        Doc::Sxf(Sxf::read(bytes, cx).map_err(|_| Failed)?, None)
    } else if format == Some(Jww::FORMAT) {
        Doc::Jww(Jww::read(bytes, cx).map_err(|_| Failed)?)
    } else if let Some((sxf, archive)) = packed::read(bytes, cx)? {
        // P2Z・SFZ（#385）。SXF の項目のない ZIP は、ifcZIP などとして後で試す
        let file = Sxf::read(&sxf, cx).map_err(|_| Failed)?;
        Doc::Sxf(file, Some(Box::new(archive)))
    } else if let Some((registration, read)) =
        plugin::installed()
            .iter()
            .find_map(|plugin| match plugin.read {
                plugin::Reader::Drawing(read) if plugin.registration.matches(bytes) => {
                    Some((plugin.registration, read))
                }
                _ => None,
            })
    {
        // コアのどの形式でもなければ、登録した図面の形式のプラグインを順に試す（決定 0243）
        Doc::Plugin(registration, read(bytes, cx).map_err(|_| Failed)?)
    } else {
        return Ok(None);
    };
    Ok(Some(doc))
}

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
    /// 設定が読めなければ [`Status::Invalid`]、このライブラリにない実行の形なら [`Status::Failed`] を返す。
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
            hashes: BTreeMap::new(),
            models: BTreeMap::new(),
            civils: BTreeMap::new(),
            geos: BTreeMap::new(),
            clouds: BTreeMap::new(),
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
        // 版を先に確かめる。版が違えば、ほかの欄の形も違いうる
        if let Some(version) = value
            .pointer("/v")
            .and_then(|v| zumen_json::uint(v, 0, MAX_SAFE_INTEGER as u64).ok())
            && version != SCHEMA_MAJOR
        {
            diagnostics
                .report(Diagnostic::new(codes::CALL_VERSION_UNSUPPORTED).with("version", version));
            return None;
        }
        // wasm のモジュールは、持つ op だけを入れて作る。持たない op は、欄を読む前に断る（決定 0013・0221）
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
            Request::Tables(request) => self.tables(request, &mut cx).map(|r| Box::new(r) as _),
            #[cfg(feature = "drawing")]
            Request::Scene(request) => self.scene(request, &mut cx).map(|r| Box::new(r) as _),
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
            #[cfg(feature = "drawing")]
            Request::Civil(request) => self.civil(request, &mut cx).map(|r| Box::new(r) as _),
            #[cfg(feature = "drawing")]
            Request::Pointcloud(request) => {
                self.pointcloud(request, &mut cx).map(|r| Box::new(r) as _)
            }
            #[cfg(feature = "drawing")]
            Request::Deviation(request) => {
                self.deviation(request, &mut cx).map(|r| Box::new(r) as _)
            }
            #[cfg(feature = "drawing")]
            Request::CivilDrawing(request) => self
                .civil_drawing(request, &mut cx)
                .map(|r| Box::new(r) as _),
            #[cfg(feature = "drawing")]
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
            #[cfg(feature = "drawing")]
            Request::Sheet(request) => self
                .input(&request.input, "/input", &mut cx)
                .and_then(|bytes| sheet::sheet(request, bytes, &mut cx))
                .map(|r| Box::new(r) as _),
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
        if self.docs.contains_key(name)
            || self.models.contains_key(name)
            || self.civils.contains_key(name)
            || self.clouds.contains_key(name)
            || self.geos.contains_key(name)
        {
            return Err(fail(
                cx,
                Diagnostic::new(codes::CALL_DOCUMENT_EXISTS).with("name", name.as_str()),
            ));
        }
        // IFC-SPF は SXF と同じ始まりなので先に、ifcZIP は ZIP なので最後に試す（決定 0219）。モジュールには、
        // 持つ形式の判定だけを入れる（決定 0221）
        #[cfg(feature = "bim")]
        if let Some(model) = building::read_spf(bytes, cx) {
            return self.open_model(name.clone(), model?, cx);
        }
        // 名指せる形式は xyz だけ。ほかの形式は入力から判定する
        if request.format.as_deref().is_some_and(|format| {
            format != zumen_xyz::Xyz::FORMAT || cfg!(not(feature = "drawing"))
        }) {
            return Err(fail(cx, invalid_value("/format")));
        }
        // LAS・LAZ は範囲で読む（決定 0232）
        #[cfg(feature = "drawing")]
        if let Some(opened) = pointcloud::read(&request.input, &self.inputs, request.size, cx) {
            let format = zumen_las::Las::FORMAT.to_string();
            return Ok(match opened? {
                pointcloud::Opened::Cloud(cloud, done) => {
                    self.clouds.insert(name.clone(), *cloud);
                    OpenResult {
                        doc: name.clone(),
                        format,
                        needs: None,
                        done: Some(done).filter(|done| !done.is_empty()),

                        entries: None,
                    }
                }
                pointcloud::Opened::Need(needs) => OpenResult {
                    doc: name.clone(),
                    format,
                    needs: Some(needs),
                    done: None,

                    entries: None,
                },
            });
        }
        // PLY・E57 と、名指した XYZ
        #[cfg(feature = "drawing")]
        if let Some(opened) = pointcloud::read_whole(bytes, request.format.as_deref(), cx) {
            let (format, cloud) = opened?;
            self.clouds.insert(name.clone(), cloud);
            return Ok(OpenResult {
                doc: name.clone(),
                format: format.to_string(),
                needs: None,
                done: None,

                entries: None,
            });
        }
        #[cfg(feature = "drawing")]
        if let Some(doc) = read_drawing(bytes, cx)? {
            let result = OpenResult {
                doc: name.clone(),
                format: doc.format().to_string(),
                needs: None,
                done: None,

                entries: match &doc {
                    Doc::Sxf(_, Some(archive)) => Some(archive.names()),
                    _ => None,
                },
            };
            let hash = Hash::document_checked(bytes, cx).map_err(|stop| stopped(cx, stop))?;
            self.hashes.insert(name.clone(), hash);
            self.docs.insert(name.clone(), doc);
            return Ok(result);
        }
        // J-LandXML は XML の根で見分ける（決定 0230）
        #[cfg(feature = "drawing")]
        if let Some(model) = civil::read(bytes, cx) {
            self.civils.insert(name.clone(), model?);
            return Ok(OpenResult {
                doc: name.clone(),
                format: zumen_landxml::LandXml::FORMAT.to_string(),
                needs: None,
                done: None,

                entries: None,
            });
        }
        // CityGML も XML の根で見分ける（決定 0233）
        #[cfg(feature = "drawing")]
        if let Some(model) = geo::read(bytes, cx) {
            self.geos.insert(name.clone(), model?);
            return Ok(OpenResult {
                doc: name.clone(),
                format: zumen_citygml::CityGml::FORMAT.to_string(),
                needs: None,
                done: None,

                entries: None,
            });
        }
        #[cfg(feature = "bim")]
        if let Some(model) = building::read_zip(bytes, cx) {
            return self.open_model(name.clone(), model?, cx);
        }
        // コアのどの形式でもなければ、登録した建物の形式のプラグインを順に試す
        #[cfg(feature = "bim")]
        if let Some(model) = building::read_plugin(bytes, cx) {
            return self.open_model(name.clone(), model?, cx);
        }
        cx.check(Kind::InputBytes, bytes.len() as u64)
            .map_err(|stop| stopped(cx, stop))?;
        Err(fail(
            cx,
            Diagnostic::new(codes::CALL_FORMAT_UNSUPPORTED).with("name", request.input.as_str()),
        ))
    }

    fn open_model(
        &mut self,
        name: String,
        model: building::Model,
        cx: &mut Cx<'_>,
    ) -> Result<OpenResult, Failed> {
        // IFC4.3 の線形は、土木の領域モデルにもする（決定 0235）
        #[cfg(all(feature = "drawing", feature = "bim"))]
        if let Some(file) = &model.file
            && let Some(civil) = civil::from_ifc(file, cx)?
        {
            self.civils.insert(name.clone(), civil);
        }
        #[cfg(not(all(feature = "drawing", feature = "bim")))]
        let _ = cx;
        let format = model.format.to_string();
        self.models.insert(name.clone(), model);
        Ok(OpenResult {
            doc: name,
            format,
            needs: None,
            done: None,

            entries: None,
        })
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
            if !self.docs.contains_key(doc)
                && !self.models.contains_key(doc)
                && !self.civils.contains_key(doc)
                && !self.clouds.contains_key(doc)
                && !self.geos.contains_key(doc)
            {
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
            self.hashes.remove(doc);
            self.models.remove(doc);
            self.civils.remove(doc);
            self.geos.remove(doc);
            self.clouds.remove(doc);
        }
        for input in inputs {
            self.inputs.remove(input);
        }
        Ok(CloseResult {})
    }

    fn drawing(&self, request: &DrawingRequest, cx: &mut Cx<'_>) -> Result<DrawingResult, Failed> {
        let drawing = self.lift(&request.doc, cx)?;
        let hash = self.hashes.get(&request.doc).map(Hash::to_string);
        summary(&drawing, hash, cx)
    }

    fn entities(
        &self,
        request: &EntitiesRequest,
        cx: &mut Cx<'_>,
    ) -> Result<EntitiesResult, Failed> {
        let fields = match &request.fields {
            None => vec![EntityField::Kind, EntityField::Layer, EntityField::Source],
            Some(fields) => {
                let mut sorted = fields.clone();
                sorted.sort();
                sorted.dedup();
                if fields.is_empty() || sorted.len() != fields.len() {
                    return Err(fail(cx, invalid_value("/fields")));
                }
                sorted
            }
        };
        let region = match request.region.as_deref() {
            None => None,
            Some(&[min_x, min_y, max_x, max_y]) if min_x <= max_x && min_y <= max_y => {
                Some(Extent {
                    min_x,
                    min_y,
                    max_x,
                    max_y,
                })
            }
            Some(_) => return Err(fail(cx, invalid_value("/region"))),
        };
        let drawing = self.lift(&request.doc, cx)?;
        let offset = request.offset.unwrap_or(0);
        let limit = request.limit.unwrap_or(ENTITIES_LIMIT);
        let mut page = Page {
            start: offset,
            end: offset.saturating_add(limit),
            fields,
            region,
            position: 0,
            items: Vec::new(),
            style_ids: None,
        };
        // 空間か用紙を指せば、その要素だけを返す。記号の定義の空間は、指したときだけ返す（決定 0177）
        let wanted = |place: &str| {
            request
                .place
                .as_deref()
                .is_none_or(|wanted| wanted == place)
        };
        for space in &drawing.spaces {
            let place = id::space(space.id);
            if wanted(&place) {
                page.collect(&drawing, &place, &space.elements, cx)?;
            }
        }
        if request.place.is_some() {
            let mut definitions = id::DefinitionNamer::default();
            for definition in &drawing.symbols {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| fail(cx, stop.to_diagnostic()))?;
                let place = definitions.next(definition);
                if wanted(&place) {
                    page.collect(&drawing, &place, &definition.space.elements, cx)?;
                }
            }
        }
        let mut sheets = id::SheetNamer::default();
        for sheet in &drawing.sheets {
            cx.check(Kind::Work, 1)
                .map_err(|stop| fail(cx, stop.to_diagnostic()))?;
            let place = sheets.next(sheet);
            if wanted(&place) {
                page.collect(&drawing, &place, &sheet.elements, cx)?;
            }
        }
        let total = page.position;
        Ok(EntitiesResult {
            items: page.items,
            total,
            next_offset: (page.end < total).then_some(page.end),
        })
    }

    /// レイヤと線種の表の項目を、ID・名前・ハッシュで返す（決定 0156）
    fn tables(&self, request: &TablesRequest, cx: &mut Cx<'_>) -> Result<TablesResult, Failed> {
        let drawing = self.lift(&request.doc, cx)?;
        // 線種に添える文字が指す文字スタイルの ID（決定 0166）
        let marked = drawing.line_types.iter().any(|line_type| {
            line_type
                .definition
                .as_ref()
                .is_some_and(|d| !d.marks.is_empty())
        });
        let style_ids = match marked {
            true => StyleIds::new(&drawing, cx)?,
            false => StyleIds::default(),
        };
        let mut seen = BTreeMap::new();
        let mut line_types = Vec::with_capacity(drawing.line_types.len());
        let mut line_type_ids = BTreeMap::new();
        for line_type in &drawing.line_types {
            let hash = Hash::line_type_checked(line_type, cx).map_err(|stop| stopped(cx, stop))?;
            let id = table_id(id::line_type(line_type), &mut seen);
            line_type_ids
                .entry(line_type.id)
                .or_insert_with(|| id.clone());
            line_types.push(line_type_item(id, line_type, hash, &style_ids, cx)?);
        }
        seen.clear();
        let mut layers = Vec::with_capacity(drawing.layers.len());
        for layer in &drawing.layers {
            let hash = Hash::layer_checked(layer, cx).map_err(|stop| stopped(cx, stop))?;
            layers.push(layer_item(
                table_id(id::layer(layer), &mut seen),
                layer,
                hash,
                &line_type_ids,
            ));
        }
        seen.clear();
        let mut dimension_styles = Vec::with_capacity(drawing.dimension_styles.len());
        for (index, style) in drawing.dimension_styles.iter().enumerate() {
            let hash =
                Hash::dimension_style_checked(style, cx).map_err(|stop| stopped(cx, stop))?;
            dimension_styles.push(dimension_style_item(
                table_id(id::dimension_style(style, index), &mut seen),
                style,
                hash,
            ));
        }
        seen.clear();
        let mut text_styles = Vec::with_capacity(drawing.text_styles.len());
        for (index, style) in drawing.text_styles.iter().enumerate() {
            let hash = Hash::text_style_checked(style, cx).map_err(|stop| stopped(cx, stop))?;
            text_styles.push(text_style_item(
                table_id(id::text_style(style, index), &mut seen),
                style,
                hash,
            ));
        }
        Ok(TablesResult {
            layers,
            line_types,
            dimension_styles,
            text_styles,
        })
    }

    /// 変更の一覧を文書に当てて書き戻し、書き戻したファイルを blob で返す（決定 0139）
    fn apply(&mut self, request: &ApplyRequest, cx: &mut Cx<'_>) -> Result<ApplyResult, Failed> {
        let drawing = self.lift(&request.doc, cx)?;
        // 移せなかった ATTDEF は図面モデルの定義に現れない。どの定義の属性かを境界で判定できないため、
        // 安全に属性を作れない記号の追加は拒否する。
        let mut symbol_add = None;
        for (index, edit) in request.edits.iter().enumerate() {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            if matches!(edit, messages::Edit::Add(edit) if matches!(edit.geometry, messages::Geometry::Symbol(_)))
            {
                symbol_add = Some(index);
                break;
            }
        }
        if let Some(index) = symbol_add {
            let diagnostics = u64::try_from(cx.diag.len()).unwrap_or(u64::MAX);
            cx.check(Kind::Work, diagnostics)
                .map_err(|stop| stopped(cx, stop))?;
            let invalid_definitions = cx.diag.iter().any(|diagnostic| {
                matches!(diagnostic.param("kind"), Some(zumen_base::Value::Text(kind)) if kind == "ATTDEF")
            });
            if invalid_definitions {
                return Err(fail(
                    cx,
                    invalid_value(format!("/edits/{index}/geometry/attributes")),
                ));
            }
        }
        let changes = changes(request, &drawing, cx)?;
        let Some(doc) = self.docs.get_mut(&request.doc) else {
            return Err(fail(
                cx,
                Diagnostic::new(codes::CALL_DOCUMENT_MISSING).with("name", request.doc.as_str()),
            ));
        };
        // 書き出しが予算や中断で止まっても、元の文書は変更しない。
        let mut bytes = Vec::new();
        let mut ids_shifted = false;
        let (applied, hash) = match doc {
            // プラグインの文書は書き戻せない（決定 0243）
            Doc::Plugin(..) => return Err(fail(cx, unsupported(&request.doc, self.op))),
            Doc::Dxf(file) => {
                let mut updated = file.snapshot(cx).map_err(|_| Failed)?;
                let applied = updated.apply(&changes, cx).map_err(|_| Failed)?;
                Dxf::write(&updated, &mut bytes, cx).map_err(|_| Failed)?;
                let hash = Hash::document_checked(&bytes, cx).map_err(|stop| stopped(cx, stop))?;
                *file = updated;
                (applied, hash)
            }
            Doc::Dwg(file) => {
                let mut updated = file.snapshot(cx).map_err(|_| Failed)?;
                let applied = updated.apply(&changes, cx).map_err(|_| Failed)?;
                Dwg::write(&updated, &mut bytes, cx).map_err(|_| Failed)?;
                let hash = Hash::document_checked(&bytes, cx).map_err(|stop| stopped(cx, stop))?;
                *file = updated;
                (applied, hash)
            }
            Doc::Sxf(file, archive) => {
                let mut updated = file.clone();
                let applied = updated.apply(&changes, cx).map_err(|_| Failed)?;
                Sxf::write(&updated, &mut bytes, cx).map_err(|_| Failed)?;
                if let Some(archive) = archive {
                    bytes = archive
                        .write(&bytes)
                        .ok_or_else(|| fail(cx, Diagnostic::new(codes::SXF_ARCHIVE_UNSUPPORTED)))?;
                }
                let hash = Hash::document_checked(&bytes, cx).map_err(|stop| stopped(cx, stop))?;
                *file = updated;
                (applied, hash)
            }
            Doc::Jww(file) => {
                let mut updated = file.clone();
                let (applied, shifted) = updated.apply(&changes, cx).map_err(|_| Failed)?;
                Jww::write(&updated, &mut bytes, cx).map_err(|_| Failed)?;
                let hash = Hash::document_checked(&bytes, cx).map_err(|stop| stopped(cx, stop))?;
                *file = updated;
                ids_shifted = shifted;
                (applied, hash)
            }
        };
        self.hashes.insert(request.doc.clone(), hash);
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes,
        }));
        Ok(ApplyResult {
            applied: applied
                .into_iter()
                .map(|Applied { id, hash }| messages::Applied {
                    id,
                    hash: hash.map(|hash| hash.to_string()),
                })
                .collect(),
            hash: hash.to_string(),
            ids_shifted: ids_shifted.then_some(true),
        })
    }

    /// 図面モデルを別の形式の新しいファイルに書き、新しい文書として開く（決定 0206）
    fn convert(
        &mut self,
        request: &ConvertRequest,
        cx: &mut Cx<'_>,
    ) -> Result<ConvertResult, Failed> {
        if !name::is_safe(&request.to) {
            return Err(fail(cx, invalid_value("/to")));
        }
        if self.docs.contains_key(&request.to) {
            return Err(fail(
                cx,
                Diagnostic::new(codes::CALL_DOCUMENT_EXISTS).with("name", request.to.as_str()),
            ));
        }
        if request.format != Dxf::FORMAT {
            return Err(fail(
                cx,
                Diagnostic::new(codes::CONVERT_FORMAT_UNSUPPORTED)
                    .with("format", request.format.as_str()),
            ));
        }
        let drawing = self.lift(&request.doc, cx)?;
        let converted = convert::to_dxf(&drawing, cx)?;
        let hash =
            Hash::document_checked(&converted.bytes, cx).map_err(|stop| stopped(cx, stop))?;
        self.hashes.insert(request.to.clone(), hash);
        self.docs
            .insert(request.to.clone(), Doc::Dxf(converted.file));
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes: converted.bytes,
        }));
        Ok(ConvertResult {
            doc: request.to.clone(),
            format: Dxf::FORMAT.to_string(),
            hash: hash.to_string(),
            elements: converted.elements,
        })
    }

    /// 起点から配置をたどった 2D のシーンを、blob で返す（決定 0172）
    fn scene(&mut self, request: &SceneRequest, cx: &mut Cx<'_>) -> Result<SceneResult, Failed> {
        scene::check(request, cx)?;
        let palette = self.palette(request.palette.as_deref(), cx)?;
        let (mut result, bytes) = {
            let fonts = self.fonts(request, cx)?;
            let drawing = self.lift(&request.doc, cx)?;
            scene::scene(&drawing, request, palette, &fonts, cx)?
        };
        result.blob = self.blobs.len() as u64;
        self.blobs.push(Some(Blob {
            kind: BlobKind::Scene,
            bytes,
        }));
        Ok(result)
    }

    /// 起点から配置をたどって、プロファイルで照合する（決定 0173・0179）
    fn check(&mut self, request: &CheckRequest, cx: &mut Cx<'_>) -> Result<CheckResult, Failed> {
        let palette = self.palette(request.palette.as_deref(), cx)?;
        let mut profiles = Vec::with_capacity(request.profiles.len());
        for (index, name) in request.profiles.iter().enumerate() {
            if !name::is_safe(name) {
                return Err(fail(cx, invalid_value(format!("/profiles/{index}"))));
            }
            let Some(bytes) = self.inputs.get(name) else {
                return Err(fail(cx, input_missing(name)));
            };
            profiles.push((name.as_str(), bytes.as_slice()));
        }
        let family = match self.docs.get(&request.doc) {
            Some(doc) => doc.family(),
            None => "",
        };
        let drawing = self.lift(&request.doc, cx)?;
        check::check(&drawing, request, &profiles, &palette, family, cx)
    }

    /// 入力の名前のバイト列。`pointer` は名前を書いた欄
    fn input(&self, name: &str, pointer: &str, cx: &mut Cx<'_>) -> Result<&[u8], Failed> {
        if !name::is_safe(name) {
            return Err(fail(cx, invalid_value(pointer)));
        }
        match self.inputs.get(name) {
            Some(bytes) => Ok(bytes),
            None => Err(fail(cx, input_missing(name))),
        }
    }

    /// レビューの文書の対象を、文書の版へ移し、移した文書を blob で返す（決定 0218）
    fn review_carry(
        &mut self,
        request: &ReviewCarryRequest,
        cx: &mut Cx<'_>,
    ) -> Result<ReviewCarryResult, Failed> {
        let plan = match self.models.get(&request.doc) {
            Some(model) => review::Plan {
                hash: model.hash.to_string(),
                elements: model.hashes(cx)?,
            },
            #[cfg(feature = "drawing")]
            None => {
                let drawing = self.lift(&request.doc, cx)?;
                let Some(hash) = self.hashes.get(&request.doc) else {
                    return Err(fail(cx, invalid_value("/doc")));
                };
                review::Plan::drawing(&drawing, hash.to_string(), cx)?
            }
            #[cfg(not(feature = "drawing"))]
            None => {
                return Err(fail(
                    cx,
                    Diagnostic::new(codes::CALL_DOCUMENT_MISSING)
                        .with("name", request.doc.as_str()),
                ));
            }
        };
        let bytes = self.input(&request.review, "/review", cx)?;
        let (result, bytes) = review::carry(plan, bytes, request, cx)?;
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes,
        }));
        Ok(result)
    }

    /// 文書を照合し、選んだ指摘をレビューの文書に取り込み、足した文書を blob で返す（決定 0218）
    fn review_adopt(
        &mut self,
        request: &ReviewAdoptRequest,
        cx: &mut Cx<'_>,
    ) -> Result<ReviewAdoptResult, Failed> {
        let check = CheckRequest {
            request_common: request.request_common.clone(),
            doc: request.doc.clone(),
            profiles: request.profiles.clone(),
            profile: request.profile.clone(),
            from: request.from.clone(),
            view: request.view,
            file_name: request.file_name.clone(),
            offset: None,
            limit: Some(u64::MAX),
            palette: request.palette.clone(),
        };
        let findings = self.check(&check, cx)?.findings;
        let drawing = self.lift(&request.doc, cx)?;
        let (Some(hash), Some(doc)) = (self.hashes.get(&request.doc), self.docs.get(&request.doc))
        else {
            return Err(fail(cx, invalid_value("/doc")));
        };
        let target = zumen_review::Target {
            id: request.target.clone(),
            format: doc.format().to_string(),
            hash: hash.to_string(),
            name: Some(request.doc.clone()),
            extra: Vec::new(),
        };
        let review = match &request.review {
            Some(name) => Some(self.input(name, "/review", cx)?),
            None => None,
        };
        let plan = review::Plan::drawing(&drawing, target.hash.clone(), cx)?;
        let findings: Vec<(String, &messages::Diagnostic)> = findings
            .iter()
            .map(|f| (f.rule.clone(), &f.diagnostic))
            .collect();
        let (result, bytes) =
            review::adopt(&plan, target, review, &findings, &request.findings, cx)?;
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes,
        }));
        Ok(result)
    }

    /// 変更の一覧を IFC に当てて書き戻し、書き戻したファイルを blob で返す（決定 0244）
    #[cfg(feature = "bim")]
    fn building_apply(
        &mut self,
        request: &messages::BuildingApplyRequest,
        cx: &mut Cx<'_>,
    ) -> Result<messages::BuildingApplyResult, Failed> {
        let model = self.model(&request.doc, "building_apply", cx)?;
        // 書き出しが予算や中断で止まっても、元の文書は変更しない。
        let (updated, result, bytes) = building::apply(model, request, cx)?;
        // IFC4.3 の線形の土木の領域モデルも、書き戻したものから作り直す（決定 0235）
        #[cfg(feature = "drawing")]
        {
            self.civils.remove(&request.doc);
            if let Some(file) = &updated.file
                && let Some(civil) = civil::from_ifc(file, cx)?
            {
                self.civils.insert(request.doc.clone(), civil);
            }
        }
        self.models.insert(request.doc.clone(), updated);
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes,
        }));
        Ok(result)
    }

    /// 名前の建物モデル。図面の文書なら、`op` を使えない
    fn model(&self, doc: &str, op: &str, cx: &mut Cx<'_>) -> Result<&building::Model, Failed> {
        if !name::is_safe(doc) {
            return Err(fail(cx, invalid_value("/doc")));
        }
        match self.models.get(doc) {
            Some(model) => Ok(model),
            None if self.docs.contains_key(doc)
                || self.civils.contains_key(doc)
                || self.clouds.contains_key(doc)
                || self.geos.contains_key(doc) =>
            {
                Err(fail(cx, unsupported(doc, op)))
            }
            None => Err(fail(
                cx,
                Diagnostic::new(codes::CALL_DOCUMENT_MISSING).with("name", doc),
            )),
        }
    }

    /// 建物モデルを IDS で照合する（決定 0219）
    fn ids_check(
        &self,
        request: &IdsCheckRequest,
        cx: &mut Cx<'_>,
    ) -> Result<IdsCheckResult, Failed> {
        let model = self.model(&request.doc, "ids_check", cx)?;
        let ids = self.input(&request.ids, "/ids", cx)?;
        let express = match &request.express {
            Some(name) => Some(self.input(name, "/express", cx)?),
            None => None,
        };
        let (ids, report) = building::check(model, ids, express, cx)?;
        Ok(building::ids_check(
            &ids,
            &report,
            request.offset,
            request.limit,
        ))
    }

    /// 建物モデルを IDS で照合し、選んだ指摘をレビューの文書に取り込み、足した文書を blob で返す（決定 0219）
    fn review_adopt_ids(
        &mut self,
        request: &ReviewAdoptIdsRequest,
        cx: &mut Cx<'_>,
    ) -> Result<ReviewAdoptResult, Failed> {
        let model = self.model(&request.doc, "review_adopt_ids", cx)?;
        let ids = self.input(&request.ids, "/ids", cx)?;
        let express = match &request.express {
            Some(name) => Some(self.input(name, "/express", cx)?),
            None => None,
        };
        let (_, report) = building::check(model, ids, express, cx)?;
        let findings: Vec<IdsFinding> = report.findings.iter().map(building::finding).collect();
        let findings: Vec<(String, &messages::Diagnostic)> = findings
            .iter()
            .map(|f| (f.specification.clone(), &f.diagnostic))
            .collect();
        let plan = review::Plan {
            hash: model.hash.to_string(),
            elements: model.hashes(cx)?,
        };
        let target = zumen_review::Target {
            id: request.target.clone(),
            format: "ifc".to_string(),
            hash: plan.hash.clone(),
            name: Some(request.doc.clone()),
            extra: Vec::new(),
        };
        let review = match &request.review {
            Some(name) => Some(self.input(name, "/review", cx)?),
            None => None,
        };
        let (result, bytes) =
            review::adopt(&plan, target, review, &findings, &request.findings, cx)?;
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes,
        }));
        Ok(result)
    }

    /// 線形の上の所を求める。doc を渡せば、開いた土木の領域モデルの線形を使う（決定 0230）
    #[cfg(feature = "drawing")]
    fn civil(
        &self,
        request: &messages::CivilRequest,
        cx: &mut Cx<'_>,
    ) -> Result<messages::CivilResult, Failed> {
        let model = match &request.doc {
            None => None,
            Some(doc) => {
                if !name::is_safe(doc) {
                    return Err(fail(cx, invalid_value("/doc")));
                }
                match self.civils.get(doc) {
                    Some(model) => Some(model),
                    None if self.docs.contains_key(doc)
                        || self.models.contains_key(doc)
                        || self.clouds.contains_key(doc)
                        || self.geos.contains_key(doc) =>
                    {
                        return Err(fail(cx, unsupported(doc, "civil")));
                    }
                    None => {
                        return Err(fail(
                            cx,
                            Diagnostic::new(codes::CALL_DOCUMENT_MISSING).with("name", doc.clone()),
                        ));
                    }
                }
            }
        };
        civil::civil(request, model, cx)
    }

    /// 開いた CityGML の地理の領域モデルの要約と、地物の中身（決定 0233）
    #[cfg(feature = "drawing")]
    fn geo(
        &self,
        request: &messages::GeoRequest,
        cx: &mut Cx<'_>,
    ) -> Result<messages::GeoResult, Failed> {
        let doc = &request.doc;
        if !name::is_safe(doc) {
            return Err(fail(cx, invalid_value("/doc")));
        }
        match self.geos.get(doc) {
            Some(model) => geo::geo(request, model, cx),
            None if self.docs.contains_key(doc)
                || self.models.contains_key(doc)
                || self.civils.contains_key(doc)
                || self.clouds.contains_key(doc) =>
            {
                Err(fail(cx, unsupported(doc, "geo")))
            }
            None => Err(fail(
                cx,
                Diagnostic::new(codes::CALL_DOCUMENT_MISSING).with("name", doc.clone()),
            )),
        }
    }

    /// 電子納品の成果品を読み、管理ファイルの要約を返す（決定 0241）
    #[cfg(feature = "drawing")]
    fn delivery_check(
        &self,
        request: &messages::DeliveryCheckRequest,
        cx: &mut Cx<'_>,
    ) -> Result<messages::DeliveryCheckResult, Failed> {
        let source = match (&request.root, &request.archive) {
            (Some(root), None) => {
                let root = match root.strip_suffix('/') {
                    Some(stem) if !stem.is_empty() && name::is_safe(stem) => root.as_str(),
                    _ => return Err(fail(cx, invalid_value("/root"))),
                };
                let files: Vec<(&str, &[u8])> = self
                    .inputs
                    .iter()
                    .filter_map(|(name, bytes)| {
                        name.strip_prefix(root).map(|rest| (rest, bytes.as_slice()))
                    })
                    .collect();
                if files.is_empty() {
                    return Err(fail(cx, invalid_value("/root")));
                }
                delivery::Source::Files(files)
            }
            (None, Some(archive)) => {
                delivery::Source::Archive(self.input(archive, "/archive", cx)?)
            }
            _ => return Err(fail(cx, invalid_value("/root"))),
        };
        let names = request.profiles.as_deref().unwrap_or_default();
        let mut profiles = Vec::with_capacity(names.len());
        for (index, name) in names.iter().enumerate() {
            let bytes = self.input(name, &format!("/profiles/{index}"), cx)?;
            profiles.push((name.as_str(), bytes));
        }
        delivery::delivery_check(request, source, &profiles, cx)
    }

    /// bcfzip を読み、開いた建物モデルを対象にしたレビューの文書を blob で返す（決定 0220）
    fn bcf_read(
        &mut self,
        request: &BcfReadRequest,
        cx: &mut Cx<'_>,
    ) -> Result<BcfReadResult, Failed> {
        let model = self.model(&request.doc, "bcf_read", cx)?;
        let bytes = self.input(&request.input, "/input", cx)?;
        let (result, bytes) = bcf::read(model, bytes, request, cx)?;
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes,
        }));
        Ok(result)
    }

    /// レビューの文書を bcfzip に書き、blob で返す（決定 0220）
    fn bcf_write(
        &mut self,
        request: &BcfWriteRequest,
        cx: &mut Cx<'_>,
    ) -> Result<BcfWriteResult, Failed> {
        let review = self.input(&request.review, "/review", cx)?;
        let original = match &request.original {
            Some(name) => Some(self.input(name, "/original", cx)?),
            None => None,
        };
        let (result, bytes) = bcf::write(review, original, request, cx)?;
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes,
        }));
        Ok(result)
    }

    /// シーンの文字を置くフォントの入力を読む（決定 0188）
    fn fonts(
        &self,
        request: &SceneRequest,
        cx: &mut Cx<'_>,
    ) -> Result<zumen_scene::FontSet<'_>, Failed> {
        let mut set = zumen_scene::FontSet::default();
        for (index, font) in request.fonts.iter().flatten().enumerate() {
            let pointer = format!("/fonts/{index}/input");
            if !name::is_safe(&font.input) {
                return Err(fail(cx, invalid_value(pointer)));
            }
            let Some(bytes) = self.inputs.get(&font.input) else {
                return Err(fail(cx, input_missing(&font.input)));
            };
            let number = u32::try_from(font.index.unwrap_or(0)).unwrap_or(u32::MAX);
            let Ok(face) = zumen_drawing::font::Font::parse(bytes, number) else {
                return Err(fail(cx, invalid_value(pointer)));
            };
            if !set.add(font.name.as_deref(), face) {
                let pointer = match font.name {
                    Some(_) => format!("/fonts/{index}/name"),
                    None => format!("/fonts/{index}"),
                };
                return Err(fail(cx, invalid_value(pointer)));
            }
        }
        for (key, reference) in request.heights.iter().flatten() {
            set.declare(key, scene::reference(*reference));
        }
        Ok(set)
    }

    /// 色の表の入力を読む。名前がなければ空の表
    fn palette(&self, name: Option<&str>, cx: &mut Cx<'_>) -> Result<zumen_scene::Palette, Failed> {
        let Some(name) = name else {
            return Ok(zumen_scene::Palette::default());
        };
        if !name::is_safe(name) {
            return Err(fail(cx, invalid_value("/palette")));
        }
        let Some(bytes) = self.inputs.get(name) else {
            return Err(fail(cx, input_missing(name)));
        };
        scene::palette(bytes).ok_or_else(|| fail(cx, invalid_value("/palette")))
    }

    /// 点群と TIN の標高の差の、規則の区分ごとの集計（決定 0234）
    #[cfg(feature = "drawing")]
    fn deviation(
        &self,
        request: &messages::DeviationRequest,
        cx: &mut Cx<'_>,
    ) -> Result<messages::DeviationResult, Failed> {
        let cloud = self.cloud(&request.doc, "/doc", cx)?;
        let doc = &request.surface.doc;
        if !name::is_safe(doc) {
            return Err(fail(cx, invalid_value("/surface/doc")));
        }
        let model = match self.civils.get(doc) {
            Some(model) => model,
            None if self.docs.contains_key(doc)
                || self.models.contains_key(doc)
                || self.clouds.contains_key(doc)
                || self.geos.contains_key(doc) =>
            {
                return Err(fail(cx, unsupported(doc, "deviation")));
            }
            None => {
                return Err(fail(
                    cx,
                    Diagnostic::new(codes::CALL_DOCUMENT_MISSING).with("name", doc.clone()),
                ));
            }
        };
        let mut profiles = Vec::with_capacity(request.profiles.len());
        for (index, name) in request.profiles.iter().enumerate() {
            let bytes = self.input(name, &format!("/profiles/{index}"), cx)?;
            profiles.push((name.as_str(), bytes));
        }
        asbuilt::deviation(request, cloud, model, &profiles, cx)
    }

    /// 土木の文書の線形から図を作り、新しい DXF の文書として開く（決定 0239）
    #[cfg(feature = "drawing")]
    fn civil_drawing(
        &mut self,
        request: &messages::CivilDrawingRequest,
        cx: &mut Cx<'_>,
    ) -> Result<messages::CivilDrawingResult, Failed> {
        let doc = &request.doc;
        if !name::is_safe(doc) {
            return Err(fail(cx, invalid_value("/doc")));
        }
        if !name::is_safe(&request.to) {
            return Err(fail(cx, invalid_value("/to")));
        }
        let to = &request.to;
        if self.docs.contains_key(to)
            || self.models.contains_key(to)
            || self.civils.contains_key(to)
            || self.clouds.contains_key(to)
            || self.geos.contains_key(to)
        {
            return Err(fail(
                cx,
                Diagnostic::new(codes::CALL_DOCUMENT_EXISTS).with("name", to.as_str()),
            ));
        }
        let model = match self.civils.get(doc) {
            Some(model) => model,
            None if self.docs.contains_key(doc)
                || self.models.contains_key(doc)
                || self.clouds.contains_key(doc)
                || self.geos.contains_key(doc) =>
            {
                return Err(fail(cx, unsupported(doc, "civil_drawing")));
            }
            None => {
                return Err(fail(
                    cx,
                    Diagnostic::new(codes::CALL_DOCUMENT_MISSING).with("name", doc.clone()),
                ));
            }
        };
        let names = request.profiles.as_deref().unwrap_or_default();
        let mut profiles = Vec::with_capacity(names.len());
        for (index, name) in names.iter().enumerate() {
            let bytes = self.input(name, &format!("/profiles/{index}"), cx)?;
            profiles.push((name.as_str(), bytes));
        }
        let (drawing, mut result) = civil::civil_drawing(request, model, &profiles, cx)?;
        let converted = convert::to_dxf(&drawing, cx)?;
        let hash =
            Hash::document_checked(&converted.bytes, cx).map_err(|stop| stopped(cx, stop))?;
        self.hashes.insert(to.clone(), hash);
        self.docs.insert(to.clone(), Doc::Dxf(converted.file));
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes: converted.bytes,
        }));
        result.format = Dxf::FORMAT.to_string();
        result.hash = hash.to_string();
        result.elements = converted.elements;
        Ok(result)
    }

    /// 開いた点群の文書。`pointer` は名前を書いた欄
    #[cfg(feature = "drawing")]
    fn cloud(
        &self,
        doc: &str,
        pointer: &str,
        cx: &mut Cx<'_>,
    ) -> Result<&zumen_pointcloud::PointCloud, Failed> {
        if !name::is_safe(doc) {
            return Err(fail(cx, invalid_value(pointer)));
        }
        match self.clouds.get(doc) {
            Some(cloud) => Ok(cloud),
            None if self.docs.contains_key(doc)
                || self.models.contains_key(doc)
                || self.civils.contains_key(doc)
                || self.geos.contains_key(doc) =>
            {
                Err(fail(cx, unsupported(doc, "pointcloud")))
            }
            None => Err(fail(
                cx,
                Diagnostic::new(codes::CALL_DOCUMENT_MISSING).with("name", doc.to_string()),
            )),
        }
    }

    /// 開いた点群の要約と、条件に合う点（決定 0232）
    #[cfg(feature = "drawing")]
    fn pointcloud(
        &self,
        request: &messages::PointcloudRequest,
        cx: &mut Cx<'_>,
    ) -> Result<messages::PointcloudResult, Failed> {
        let cloud = self.cloud(&request.doc, "/doc", cx)?;
        pointcloud::pointcloud(request, cloud, cx)
    }

    /// 名前の文書を、図面モデルに変換する
    fn lift(&self, doc: &str, cx: &mut Cx<'_>) -> Result<Drawing2D, Failed> {
        if !name::is_safe(doc) {
            return Err(fail(cx, invalid_value("/doc")));
        }
        let Some(found) = self.docs.get(doc) else {
            if self.models.contains_key(doc)
                || self.civils.contains_key(doc)
                || self.clouds.contains_key(doc)
                || self.geos.contains_key(doc)
            {
                return Err(fail(cx, unsupported(doc, self.op)));
            }
            return Err(fail(
                cx,
                Diagnostic::new(codes::CALL_DOCUMENT_MISSING).with("name", doc),
            ));
        };
        // 止めた理由は、変換が診断として足してある
        found.lift(cx).map_err(|_| Failed)
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
            .filter(|_| cfg!(feature = "drawing"))
            .chain(
                plugin::installed()
                    .iter()
                    .filter(|plugin| match plugin.read {
                        plugin::Reader::Drawing(_) => cfg!(feature = "drawing"),
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

/// 図面モデルの要約。空間と用紙は、モデルの順に並べる
fn summary(
    drawing: &Drawing2D,
    hash: Option<String>,
    cx: &mut Cx<'_>,
) -> Result<DrawingResult, Failed> {
    let mut sheet_names = id::SheetNamer::default();
    Ok(DrawingResult {
        hash,
        spaces: drawing
            .spaces
            .iter()
            .map(|space| SpaceSummary {
                id: Some(id::space(space.id)),
                unit: unit_of(space.unit),
                elements: counts(&space.elements),
                extent: extent(&space.elements, |p| (p.x, p.y)),
            })
            .collect(),
        sheets: drawing
            .sheets
            .iter()
            .map(|sheet| {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| fail(cx, stop.to_diagnostic()))?;
                Ok(SheetSummary {
                    id: Some(sheet_names.next(sheet)),
                    name: sheet.name.clone(),
                    size: sheet.size.map(|size| size.iter().map(|l| l.0).collect()),
                    elements: counts(&sheet.elements),
                    views: sheet.views.len() as u64,
                    extent: extent(&sheet.elements, |p| (p.x, p.y)),
                })
            })
            .collect::<Result<_, Failed>>()?,
        point_style: drawing.point_style.map(point_style_summary),
    })
}

/// 文書の点の表し方の要約（決定 0193）
fn point_style_summary(style: PointStyle) -> PointStyleSummary {
    let (size, size_kind) = match style.size {
        PointSize::Length(size) => (size, messages::PointSizeKind::Length),
        PointSize::Screen(size) => (size, messages::PointSizeKind::Screen),
    };
    PointStyleSummary {
        mark: match style.mark {
            PointMark::Dot => messages::PointMark::Dot,
            PointMark::None => messages::PointMark::None,
            PointMark::Plus => messages::PointMark::Plus,
            PointMark::Cross => messages::PointMark::Cross,
            PointMark::Tick => messages::PointMark::Tick,
        },
        circle: style.circle,
        square: style.square,
        size,
        size_kind,
    }
}

/// entities の op で返す範囲と、そこまでに数えた要素
struct Page {
    /// 返す最初の要素の位置
    start: u64,
    /// 返す範囲の終わり（含まない）
    end: u64,
    /// 返す欄。`id` のほかに返すもの
    fields: Vec<EntityField>,
    /// 範囲がこの長方形と重なる要素だけを数える
    region: Option<Extent>,
    /// 次に数える要素の位置
    position: u64,
    items: Vec<Entity>,
    /// 形の値に書くスタイルの ID。形の値を返すときに作る
    style_ids: Option<StyleIds>,
}

impl Page {
    /// 空間か用紙の要素を数え、返す範囲にあるものを足す
    fn collect<P: geometry::Spot + Encode>(
        &mut self,
        drawing: &Drawing2D,
        place: &str,
        elements: &[Element<P>],
        cx: &mut Cx<'_>,
    ) -> Result<(), Failed> {
        let mut namer = id::Namer::new(place);
        // 引出線の注記の出所から、同じ並びの要素の ID。注記を持つ引出線の形の値を返すときに作る。同じ出所の
        // 要素が複数あれば、どれか決まらないので `None`
        let mut annotations: Option<BTreeMap<&Ref, Option<String>>> = None;
        for (index, element) in elements.iter().enumerate() {
            let id = namer.next(index, element);
            let mut bbox = None;
            // 止めたら、扱っていた要素の出所を場所に入れる（決定 0119）
            let stop_at = |cx: &mut Cx<'_>, stop: Stop| {
                let mut diagnostic = stop.to_diagnostic();
                if element.source != Ref::default() {
                    diagnostic = diagnostic.at(element.source.clone());
                }
                fail(cx, diagnostic)
            };
            if let Some(region) = &self.region {
                cx.check(Kind::Work, 1).map_err(|stop| stop_at(cx, stop))?;
                bbox = bounds::bbox(&element.shape);
                if !bbox
                    .as_ref()
                    .is_some_and(|extent| bounds::overlaps(extent, region))
                {
                    continue;
                }
            }
            let position = self.position;
            self.position += 1;
            if position < self.start || position >= self.end {
                continue;
            }
            cx.check(Kind::Work, 1).map_err(|stop| stop_at(cx, stop))?;
            let has = |field| self.fields.contains(&field);
            if has(EntityField::Bbox) && self.region.is_none() {
                bbox = bounds::bbox(&element.shape);
            }
            let hash = if has(EntityField::Hash) {
                Some(
                    Hash::element_checked(element, cx)
                        .map_err(|stop| stop_at(cx, stop))?
                        .to_string(),
                )
            } else {
                None
            };
            // 形の値に複製する頂点と文字列の大きさを数える
            let geometry = if has(EntityField::Geometry) {
                let size = match &element.shape {
                    Shape::Polyline(line) => line.vertices.len(),
                    Shape::Curve(zumen_drawing::Edge::Spline(spline)) => spline
                        .control
                        .len()
                        .saturating_add(spline.knots.len())
                        .saturating_add(spline.weights.len())
                        .saturating_add(spline.fit.len()),
                    Shape::Text(text) => text.content.len().saturating_add(text.style.len()),
                    Shape::Formatted(text) => text.content.len().saturating_add(text.style.len()),
                    Shape::Symbol(symbol) => symbol.key.name.len(),
                    _ => 0,
                };
                cx.check(Kind::Work, u64::try_from(size).unwrap_or(u64::MAX))
                    .map_err(|stop| stop_at(cx, stop))?;
                if let Shape::Symbol(symbol) = &element.shape {
                    for attribute in &symbol.attributes {
                        let size = 1_usize
                            .saturating_add(attribute.tag.len())
                            .saturating_add(attribute.text.content.len());
                        cx.check(Kind::Work, u64::try_from(size).unwrap_or(u64::MAX))
                            .map_err(|stop| stop_at(cx, stop))?;
                    }
                }
                if self.style_ids.is_none() {
                    self.style_ids = Some(StyleIds::new(drawing, cx)?);
                }
                let annotation = match &element.shape {
                    Shape::Leader(Leader {
                        annotation: Some(annotation),
                        ..
                    }) => {
                        if annotations.is_none() {
                            let mut found = BTreeMap::new();
                            let mut namer = id::Namer::new(place);
                            for (index, element) in elements.iter().enumerate() {
                                cx.check(Kind::Work, 1).map_err(|stop| stop_at(cx, stop))?;
                                let id = namer.next(index, element);
                                found
                                    .entry(&element.source)
                                    .and_modify(|id: &mut Option<String>| *id = None)
                                    .or_insert(Some(id));
                            }
                            annotations = Some(found);
                        }
                        annotations
                            .as_ref()
                            .and_then(|found| found.get(annotation).cloned().flatten())
                    }
                    _ => None,
                };
                geometry::geometry_of(&element.shape).map(|mut geometry| {
                    if let Some(ids) = &self.style_ids {
                        ids.fill(&mut geometry);
                    }
                    if let messages::Geometry::Leader(leader) = &mut geometry {
                        leader.annotation = annotation;
                    }
                    geometry
                })
            } else {
                None
            };
            self.items.push(Entity {
                id,
                kind: has(EntityField::Kind).then(|| kind_of(&element.shape)),
                layer: drawing
                    .layer(element.layer)
                    .filter(|_| has(EntityField::Layer))
                    .map(|layer| layer.name.clone()),
                source: has(EntityField::Source).then(|| Source {
                    record: element.source.record(),
                    bytes: element.source.bytes().map(|bytes| ByteRange {
                        start: bytes.start,
                        end: bytes.end,
                    }),
                    handle: match element.source.key() {
                        Some(Key::Handle(handle)) => Some(format!("{handle:X}")),
                        _ => None,
                    },
                    instance: match element.source.key() {
                        Some(Key::Instance(number)) => Some(number.to_string()),
                        _ => None,
                    },
                }),
                bbox: bbox.filter(|_| has(EntityField::Bbox)),
                hash,
                geometry,
            });
        }
        Ok(())
    }
}

fn kind_of<P>(shape: &Shape<P>) -> EntityKind {
    match shape {
        Shape::Line { .. } => EntityKind::Line,
        Shape::Circle { .. } => EntityKind::Circle,
        Shape::Curve(_) => EntityKind::Curve,
        Shape::Polyline(_) => EntityKind::Polyline,
        Shape::Text(_) => EntityKind::Text,
        Shape::Formatted(_) => EntityKind::FormattedText,
        Shape::Dimension(_) => EntityKind::Dimension,
        Shape::Leader(_) => EntityKind::Leader,
        Shape::Callout(_) => EntityKind::Callout,
        Shape::Table(_) => EntityKind::Table,
        Shape::Underlay(_) => EntityKind::Underlay,
        Shape::Symbol(_) => EntityKind::Symbol,
        Shape::Hatch(_) => EntityKind::Hatch,
        Shape::Point(_) => EntityKind::Point,
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

fn counts<P>(elements: &[Element<P>]) -> ElementCounts {
    // 書式つきの文字・寸法・引出線・引出線つきの注記・記号・ハッチングは、要約の欄がまだないので数えない（#66）
    let count = |is: fn(&Shape<P>) -> bool| elements.iter().filter(|e| is(&e.shape)).count() as u64;
    ElementCounts {
        lines: count(|shape| matches!(shape, Shape::Line { .. })),
        texts: count(|shape| matches!(shape, Shape::Text(_))),
    }
}

/// 線分の端点と文字の基点（2 点の間に置く文字は 2 点とも）を囲む範囲。要素がなければ `None`
fn extent<P: Copy>(elements: &[Element<P>], xy: impl Fn(P) -> (f64, f64)) -> Option<Extent> {
    let points = elements.iter().flat_map(|e| match &e.shape {
        Shape::Line { start, end } => [Some(*start), Some(*end)],
        Shape::Text(text) => match text.placement {
            Placement::Anchor { point, .. } => [Some(point), None],
            Placement::Between { first, second, .. } => [Some(first), Some(second)],
        },
        Shape::Circle { .. }
        | Shape::Curve(_)
        | Shape::Polyline(_)
        | Shape::Formatted(_)
        | Shape::Dimension(_)
        | Shape::Leader(_)
        | Shape::Callout(_)
        | Shape::Table(_)
        | Shape::Underlay(_)
        | Shape::Symbol(_)
        | Shape::Hatch(_)
        | Shape::Point(_) => [None, None],
    });
    points.flatten().map(xy).fold(None, |extent, (x, y)| {
        Some(match extent {
            None => Extent {
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            },
            Some(e) => Extent {
                min_x: e.min_x.min(x),
                min_y: e.min_y.min(y),
                max_x: e.max_x.max(x),
                max_y: e.max_y.max(y),
            },
        })
    })
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
        Request::Tables(request) => &request.request_common,
        Request::Scene(request) => &request.request_common,
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
        Request::Sheet(request) => &request.request_common,
        Request::Boring(request) => &request.request_common,
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

/// 要求の予算の欄
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

/// コアの診断を、メッセージの診断にする
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

/// 時間の上限。ネイティブの境界が時計を見て、中断の合図で伝える（決定 0010）
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

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_drawing::{
        LayerId, LineType, LineTypeDefinition, LineTypeId, MarkContent, ModelPoint, PatternBasis,
        Style, TextStyle, TextStyleDefinition,
    };
    use zumen_json::Value;

    use crate::changes::{ElementLookup, PlaceLookup, TableLookup, count_old_element};

    /// 見出しの単位と用紙の範囲、モデル空間の線分と文字、ペーパー空間の線分、移さない構築線を持つ DXF
    const DRAWING: &[u8] = b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$INSUNITS\n70\n4\n\
        9\n$PLIMMIN\n10\n0.0\n20\n0.0\n9\n$PLIMMAX\n10\n420.0\n20\n297.0\n0\nENDSEC\n\
        0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n1.5\n20\n2.0\n11\n100.0\n21\n50.0\n\
        0\nTEXT\n8\nTEXT\n10\n-3.0\n20\n4.0\n40\n2.5\n1\nABC\n\
        0\nLINE\n8\n0\n67\n1\n10\n10.0\n20\n10.0\n11\n410.0\n21\n287.0\n\
        0\nXLINE\n8\n0\n10\n0.0\n20\n0.0\n11\n1.0\n21\n0.0\n0\nENDSEC\n0\nEOF\n";

    fn call(session: &mut Session, request: &str) -> (Status, Value) {
        let reply = session.call(request.as_bytes(), &CancelToken::new());
        let value = zumen_json::parse(&reply.response).unwrap();
        (reply.status, value)
    }

    fn text(value: &Value, pointer: &str) -> String {
        match value.pointer(pointer) {
            Some(Value::String(text)) => text.clone(),
            other => panic!("{pointer}: {other:?}"),
        }
    }

    fn number(value: &Value, pointer: &str) -> String {
        match value.pointer(pointer) {
            Some(Value::Number(number)) => number.as_str().to_string(),
            other => panic!("{pointer}: {other:?}"),
        }
    }

    #[test]
    fn formatted_geometry_checks_work_before_parsing() {
        // フォント名を入れ子ごとに複製する内容。追加と置き換えのどちらも解析前に止める。
        let content = format!("\\f{};{}", "A".repeat(80), "{".repeat(80));
        let geometry = format!(
            r#"{{"shape":"formatted_text","content":{},"position":[0,0],"height":1,"rotation":0}}"#,
            {
                let mut out = Writer::new();
                out.string(&content);
                String::from_utf8(out.into_bytes()).unwrap()
            }
        );
        for edit in [
            format!(r#"{{"kind":"add","place":"space/0","geometry":{geometry}}}"#),
            format!(
                r#"{{"kind":"replace","id":"space/0/h1","hash":"v2:00000000000000000000000000000000","geometry":{geometry}}}"#
            ),
        ] {
            let json = format!(r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{edit}]}}"#);
            let value = zumen_json::parse(json.as_bytes()).unwrap();
            let Request::Apply(request) = Request::from_json(&value).unwrap() else {
                panic!();
            };
            let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(500)));
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
            assert!(changes(&request, &Drawing2D::default(), &mut cx).is_err());
            assert_eq!(
                diagnostics.iter().next().unwrap().code(),
                codes::BUDGET_WORK_EXCEEDED
            );
        }
    }

    #[test]
    fn geometry_copies_obey_work_budget() {
        use zumen_drawing::{
            Clip, Flow, FormattedText, Height, HeightMeaning, Horizontal, LineSpacing, ModelLength,
            Piece, Rotation, SpaceId, Span, Symbol, SymbolKey, Vertical,
        };
        let point = ModelPoint {
            space: SpaceId(0),
            x: 0.0,
            y: 0.0,
        };
        let formatted = FormattedText {
            content: "x".repeat(100),
            spans: vec![Span {
                piece: Piece::Text("x".repeat(100)),
                format: Default::default(),
            }],
            height: Height {
                value: zumen_drawing::Length::Model(ModelLength {
                    space: SpaceId(0),
                    value: 1.0,
                }),
                meaning: HeightMeaning::Frame,
            },
            point,
            horizontal: Horizontal::Left,
            vertical: Vertical::Top,
            rotation: Rotation::Angle(0.0),
            width: None,
            frame_height: None,
            flow: Flow::LeftToRight,
            line_spacing: LineSpacing {
                factor: 1.0,
                exact: false,
            },
            background: None,
            style: "style".repeat(30),
        };
        let mut nested = formatted.clone();
        nested.content.clear();
        nested.style.clear();
        nested.spans[0].piece = Piece::Text("x".into());
        nested.spans[0].format.font = Some(zumen_drawing::Font {
            name: "x".repeat(100),
            kind: zumen_drawing::FontKind::Shape,
        });
        let attribute = zumen_drawing::Attribute {
            tag: "tag".into(),
            text: zumen_drawing::Text {
                content: "value".into(),
                height: formatted.height.clone(),
                placement: Placement::Anchor {
                    point,
                    horizontal: Horizontal::Left,
                    vertical: Vertical::Baseline,
                },
                rotation: 0.0,
                width_factor: 1.0,
                oblique: 0.0,
                backward: false,
                upside_down: false,
                style: "s".into(),
            },
            formatted: Some(nested),
            hidden: false,
            layer: LayerId(0),
            style: Style::BLOCK,
            source: Ref::default(),
        };
        let shapes = [
            Shape::Formatted(formatted),
            Shape::Symbol(Symbol {
                key: SymbolKey {
                    namespace: "dxf.block".into(),
                    name: "x".repeat(100),
                },
                position: point,
                scale: [1.0, 1.0],
                rotation: 0.0,
                grid: None,
                attributes: vec![],
                clip: Some(Clip {
                    vertices: vec![[0.0, 0.0]; 100],
                    inverted: false,
                }),
            }),
            Shape::Symbol(Symbol {
                key: SymbolKey {
                    namespace: "dxf.block".into(),
                    name: "x".repeat(100),
                },
                position: point,
                scale: [1.0, 1.0],
                rotation: 0.0,
                grid: None,
                attributes: vec![attribute],
                clip: None,
            }),
        ];
        for shape in shapes {
            let element = Element {
                source: Ref::default(),
                layer: LayerId(0),
                style: Style::BLOCK,
                shape,
            };
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(20)));
            let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
            assert!(count_old_element(&element, &mut cx).is_err());
            assert_eq!(
                diagnostics.iter().next().unwrap().code(),
                codes::BUDGET_WORK_EXCEEDED
            );
            let mut page = Page {
                start: 0,
                end: 1,
                fields: vec![EntityField::Geometry],
                region: None,
                position: 0,
                items: vec![],
                style_ids: None,
            };
            let mut diagnostics = Diagnostics::new();
            let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(10)));
            let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
            assert!(
                page.collect(&Drawing2D::default(), "space/0", &[element], &mut cx)
                    .is_err()
            );
            assert_eq!(
                diagnostics.iter().next().unwrap().code(),
                codes::BUDGET_WORK_EXCEEDED
            );
        }
    }

    #[test]
    fn request_geometry_strings_obey_work_budget() {
        let long = "x".repeat(100);
        for geometry in [
            format!(
                r#"{{"shape":"text","content":"x","position":[0,0],"height":1,"rotation":0,"style":"{long}"}}"#
            ),
            format!(
                r#"{{"shape":"formatted_text","content":"x","position":[0,0],"height":1,"rotation":0,"style":"{long}"}}"#
            ),
            format!(
                r#"{{"shape":"symbol","name":"{long}","position":[0,0],"scale":[1,1],"rotation":0}}"#
            ),
        ] {
            let json = format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"space/0","geometry":{geometry}}}]}}"#
            );
            let value = zumen_json::parse(json.as_bytes()).unwrap();
            let Request::Apply(request) = Request::from_json(&value).unwrap() else {
                panic!()
            };
            let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(20)));
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
            assert!(changes(&request, &Drawing2D::default(), &mut cx).is_err());
            assert_eq!(
                diagnostics.iter().next().unwrap().code(),
                codes::BUDGET_WORK_EXCEEDED
            );
        }
    }

    #[test]
    fn spline_geometry_obeys_work_budget() {
        let knots = vec!["0"; 100].join(",");
        let geometry = format!(
            r#"{{"shape":"spline","degree":1,"knots":[{knots}],"control":[[0,0],[1,1]],"closed":false,"periodic":false}}"#
        );
        for edit in [
            format!(r#"{{"kind":"add","place":"space/0","geometry":{geometry}}}"#),
            format!(
                r#"{{"kind":"replace","id":"space/0/h1","hash":"v2:00000000000000000000000000000000","geometry":{geometry}}}"#
            ),
        ] {
            let json = format!(r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{edit}]}}"#);
            let value = zumen_json::parse(json.as_bytes()).unwrap();
            let Request::Apply(request) = Request::from_json(&value).unwrap() else {
                panic!();
            };
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(20)));
            let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
            assert!(changes(&request, &Drawing2D::default(), &mut cx).is_err());
            assert_eq!(
                diagnostics.iter().next().unwrap().code(),
                codes::BUDGET_WORK_EXCEEDED
            );
        }
        let point = ModelPoint {
            space: zumen_drawing::SpaceId(0),
            x: 0.0,
            y: 0.0,
        };
        let element = Element {
            source: Ref::default(),
            layer: LayerId(0),
            style: Style::BLOCK,
            shape: Shape::Curve(zumen_drawing::Edge::Spline(zumen_drawing::Spline {
                degree: 1,
                knots: vec![0.0; 100],
                control: vec![point; 2],
                weights: vec![],
                fit: vec![],
                start_tangent: None,
                end_tangent: None,
                closed: false,
                periodic: false,
                fit_tolerance: None,
                knot_parameterization: None,
            })),
        };
        let mut page = Page {
            start: 0,
            end: 1,
            fields: vec![EntityField::Geometry],
            region: None,
            position: 0,
            items: vec![],
            style_ids: None,
        };
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(20)));
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        assert!(
            page.collect(&Drawing2D::default(), "space/0", &[element], &mut cx)
                .is_err()
        );
        assert_eq!(
            diagnostics.iter().next().unwrap().code(),
            codes::BUDGET_WORK_EXCEEDED
        );
    }

    #[test]
    fn answers_capabilities() {
        let mut session = Session::new(b"").unwrap();
        let reply = session.call(br#"{"v":1,"op":"capabilities"}"#, &CancelToken::new());
        assert_eq!(reply.status, Status::Ok);
        let expected = format!(
            r#"{{"v":1,"result":{{"package":"{}","abi":1,"schema":{{"major":1,"minor":58}},"ops":["capabilities","open","close","describe","drawing","entities","apply","convert","tables","scene","check","validate","review_carry","review_adopt","building","building_elements","ids_check","review_adopt_ids","bcf_read","bcf_write","civil","pointcloud","deviation","civil_drawing","geo","delivery_check","photo","sheet","boring","building_apply"],"formats":[{{"id":"dxf","family":"acad","read":true,"write":true,"id_rules":2}},{{"id":"dwg","family":"acad","read":true,"write":true,"id_rules":2}},{{"id":"sxf","family":"sxf","read":true,"write":true,"id_rules":2}},{{"id":"jww","family":"jww","read":true,"write":true,"id_rules":1}},{{"id":"landxml","family":"landxml","read":true,"write":false,"id_rules":1}},{{"id":"citygml","family":"citygml","read":true,"write":false,"id_rules":1}},{{"id":"las","family":"las","read":true,"write":false,"id_rules":1}},{{"id":"ply","family":"ply","read":true,"write":false,"id_rules":1}},{{"id":"xyz","family":"xyz","read":true,"write":false,"id_rules":1}},{{"id":"e57","family":"e57","read":true,"write":false,"id_rules":1}}]}},"diagnostics":[],"blobs":[]}}"#,
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(String::from_utf8(reply.response).unwrap(), expected);
    }

    #[test]
    fn reports_requests_that_cannot_be_read() {
        let mut session = Session::new(b"").unwrap();
        let cases: &[(&str, &str, &str, &str)] = &[
            (
                "{",
                "call.json.invalid",
                "/diagnostics/0/params/offset",
                "1",
            ),
            (
                r#"{"v":2,"op":"x"}"#,
                "call.version.unsupported",
                "/diagnostics/0/params/version",
                "2",
            ),
            (
                r#"{"v":1,"op":"draw"}"#,
                "call.op.unsupported",
                "/diagnostics/0/params/op",
                "draw",
            ),
            (
                r#"{"v":1,"op":"capabilities","x":1}"#,
                "call.field.unsupported",
                "/diagnostics/0/params/pointer",
                "/x",
            ),
            (
                r#"{"op":"capabilities"}"#,
                "call.field.missing",
                "/diagnostics/0/params/pointer",
                "/v",
            ),
            (
                r#"{"v":"1","op":"capabilities"}"#,
                "call.value.invalid",
                "/diagnostics/0/params/pointer",
                "/v",
            ),
            (
                r#"{"v":1,"op":"open","input":"a","budget":{"records":-1}}"#,
                "call.value.invalid",
                "/diagnostics/0/params/pointer",
                "/budget/records",
            ),
            (
                r#"[]"#,
                "call.value.invalid",
                "/diagnostics/0/params/pointer",
                "",
            ),
        ];
        for (request, code, pointer, value) in cases {
            let (status, response) = call(&mut session, request);
            assert_eq!(status, Status::Failed, "{request}");
            assert_eq!(text(&response, "/diagnostics/0/code"), *code, "{request}");
            assert!(response.pointer("/result").is_none(), "{request}");
            let actual = match response.pointer(pointer) {
                Some(Value::Number(number)) => number.as_str().to_string(),
                Some(Value::String(text)) => text.clone(),
                other => panic!("{request}: {other:?}"),
            };
            assert_eq!(actual, *value, "{request}");
        }
        let reply = session.call(b"\xff", &CancelToken::new());
        let response = zumen_json::parse(&reply.response).unwrap();
        assert_eq!(text(&response, "/diagnostics/0/code"), "call.json.invalid");
        assert_eq!(number(&response, "/diagnostics/0/params/offset"), "0");
    }

    #[test]
    fn checks_names_and_appends_pushed_bytes() {
        let mut session = Session::new(b"").unwrap();
        assert_eq!(session.push(b"a/b.sfc", b"12"), Ok(()));
        assert_eq!(session.push(b"a/b.sfc", b"34"), Ok(()));
        assert_eq!(
            session.inputs.get("a/b.sfc").map(Vec::as_slice),
            Some(&b"1234"[..])
        );
        assert_eq!(session.push(b"../x", b""), Err(Status::Invalid));
        assert_eq!(session.push(b"\xff", b""), Err(Status::Invalid));
    }

    #[test]
    fn summarizes_the_drawing_model_of_an_open_document() {
        let mut session = Session::new(b"").unwrap();
        let (_, response) = call(&mut session, r#"{"v":1,"op":"drawing","doc":"a.dxf"}"#);
        assert_eq!(
            text(&response, "/diagnostics/0/code"),
            "call.document.missing"
        );
        session.push(b"a.dxf", DRAWING).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let (status, response) = call(&mut session, r#"{"v":1,"op":"drawing","doc":"a.dxf"}"#);
        assert_eq!(status, Status::Ok);
        assert_eq!(text(&response, "/result/spaces/0/id"), "space/0");
        assert_eq!(text(&response, "/result/sheets/0/id"), "sheet/0");
        assert!(text(&response, "/result/hash").starts_with("v2:"));
        assert_eq!(text(&response, "/result/spaces/0/unit"), "millimeter");
        assert_eq!(number(&response, "/result/spaces/0/elements/lines"), "1");
        assert_eq!(number(&response, "/result/spaces/0/elements/texts"), "1");
        assert_eq!(number(&response, "/result/spaces/0/extent/min_x"), "-3");
        assert_eq!(number(&response, "/result/spaces/0/extent/max_y"), "50");
        assert_eq!(number(&response, "/result/sheets/0/size/0"), "420");
        assert_eq!(number(&response, "/result/sheets/0/elements/lines"), "1");
        assert_eq!(number(&response, "/result/sheets/0/extent/max_x"), "410");
        assert_eq!(text(&response, "/diagnostics/0/code"), "dxf.lift.dropped");
        // 変換の途中で予算に達すれば失敗し、十分な予算があれば成功する
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"drawing","doc":"a.dxf","budget":{"work":13}}"#,
        );
        assert_eq!(status, Status::Failed);
        assert_eq!(
            text(&response, "/diagnostics/1/code"),
            "budget.work.exceeded"
        );
        assert_eq!(number(&response, "/diagnostics/1/params/requested"), "14");
        let (status, _) = call(
            &mut session,
            r#"{"v":1,"op":"drawing","doc":"a.dxf","budget":{"work":100}}"#,
        );
        assert_eq!(status, Status::Ok);
    }

    #[test]
    fn duplicate_layout_handles_have_consistent_ids_across_ops() {
        let file = |extra: &str| {
            format!(concat!(
            "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n",
            "0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nBLOCK_RECORD\n",
            "0\nBLOCK_RECORD\n5\nB1\n2\n*Paper_Space\n",
            "0\nBLOCK_RECORD\n5\nC1\n2\n*Paper_Space0\n0\nENDTAB\n0\nENDSEC\n",
            "0\nSECTION\n2\nBLOCKS\n",
            "0\nBLOCK\n5\nB2\n330\nB1\n2\n*Paper_Space\n70\n0\n10\n0\n20\n0\n0\nENDBLK\n5\nB3\n330\nB1\n",
            "0\nBLOCK\n5\nC2\n330\nC1\n2\n*Paper_Space0\n70\n0\n10\n0\n20\n0\n",
            "0\nLINE\n5\nC3\n330\nC1\n8\n0\n67\n1\n10\n0\n20\n0\n11\n1\n21\n1\n",
            "0\nENDBLK\n5\nC4\n330\nC1\n0\nENDSEC\n",
            "0\nSECTION\n2\nENTITIES\n{extra}",
            "0\nLINE\n5\nB4\n330\nB1\n8\n0\n67\n1\n10\n0\n20\n0\n11\n1\n21\n1\n0\nENDSEC\n",
            "0\nSECTION\n2\nOBJECTS\n",
            "0\nLAYOUT\n5\nF1\n330\n1A\n100\nAcDbPlotSettings\n1\n\n100\nAcDbLayout\n1\nMain\n71\n1\n10\n0\n20\n0\n11\n10\n21\n10\n330\nB1\n",
            "0\nLAYOUT\n5\nF1\n330\n1A\n100\nAcDbPlotSettings\n1\n\n100\nAcDbLayout\n1\nOther\n71\n2\n10\n0\n20\n0\n11\n10\n21\n10\n330\nC1\n",
            "0\nENDSEC\n0\nEOF\n"
        ), extra = extra).into_bytes()
        };
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", &file("")).unwrap();
        session
            .push(
                b"b.dxf",
                &file("0\nCIRCLE\n5\nD4\n8\n0\n10\n0\n20\n0\n40\n1\n"),
            )
            .unwrap();
        for doc in ["a.dxf", "b.dxf"] {
            call(
                &mut session,
                &format!(r#"{{"v":1,"op":"open","input":"{doc}"}}"#),
            );
            let (status, drawing) = call(
                &mut session,
                &format!(r#"{{"v":1,"op":"drawing","doc":"{doc}"}}"#),
            );
            assert_eq!(status, Status::Ok);
            assert_eq!(text(&drawing, "/result/sheets/0/id"), "sheet/hF1");
            assert_eq!(text(&drawing, "/result/sheets/1/id"), "sheet/hF1/1");
            let (status, entities) = call(
                &mut session,
                &format!(r#"{{"v":1,"op":"entities","doc":"{doc}"}}"#),
            );
            assert_eq!(status, Status::Ok);
            let offset = usize::from(doc == "b.dxf");
            assert_eq!(
                text(&entities, &format!("/result/items/{offset}/id")),
                "sheet/hF1/hB4"
            );
            assert_eq!(
                text(&entities, &format!("/result/items/{}/id", offset + 1)),
                "sheet/hF1/1/hC3"
            );
        }
    }

    #[test]
    fn handles_keep_ids_and_hashes_when_an_element_is_added_before() {
        const LINE: &str = "0\nLINE\n5\n2F\n8\n0\n10\n1.5\n20\n2.0\n11\n100.0\n21\n50.0\n";
        let drawing = |before: &str| {
            format!("0\nSECTION\n2\nENTITIES\n{before}{LINE}0\nENDSEC\n0\nEOF\n").into_bytes()
        };
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", &drawing("")).unwrap();
        let circle = "0\nCIRCLE\n5\n30\n8\n0\n10\n0.0\n20\n0.0\n40\n1.0\n";
        session.push(b"b.dxf", &drawing(circle)).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        call(&mut session, r#"{"v":1,"op":"open","input":"b.dxf"}"#);
        let request = |doc: &str| {
            format!(r#"{{"v":1,"op":"entities","doc":"{doc}","fields":["source","hash"]}}"#)
        };
        let (status, a) = call(&mut session, &request("a.dxf"));
        assert_eq!(status, Status::Ok);
        let (_, b) = call(&mut session, &request("b.dxf"));
        assert_eq!(text(&a, "/result/items/0/id"), "space/0/h2F");
        assert_eq!(text(&a, "/result/items/0/source/handle"), "2F");
        assert_eq!(text(&b, "/result/items/0/id"), "space/0/h30");
        assert_eq!(text(&b, "/result/items/1/id"), "space/0/h2F");
        // 前に要素が増えて、レコードの番号がずれても、ハッシュは変わらない
        assert_ne!(
            number(&a, "/result/items/0/source/record"),
            number(&b, "/result/items/1/source/record")
        );
        let hash = text(&a, "/result/items/0/hash");
        assert!(hash.starts_with("v2:") && hash.len() == 35, "{hash}");
        assert_eq!(hash, text(&b, "/result/items/1/hash"));
        assert_ne!(hash, text(&b, "/result/items/0/hash"));
        assert!(a.pointer("/result/items/0/kind").is_none());
        // 文書のハッシュは、バイト列が違えば違う
        let summary = |session: &mut Session, doc: &str| {
            let (_, response) = call(
                session,
                &format!(r#"{{"v":1,"op":"drawing","doc":"{doc}"}}"#),
            );
            text(&response, "/result/hash")
        };
        let (a, b) = (
            summary(&mut session, "a.dxf"),
            summary(&mut session, "b.dxf"),
        );
        assert!(a.starts_with("v2:") && a != b, "{a} {b}");
    }

    /// 応答の最初の error の診断
    fn error(response: &Value) -> Value {
        let Some(Value::Array(diagnostics)) = response.pointer("/diagnostics") else {
            panic!("{response:?}");
        };
        diagnostics
            .iter()
            .find(|diagnostic| text(diagnostic, "/severity") == "error")
            .cloned()
            .unwrap_or_else(|| panic!("{response:?}"))
    }

    /// ハンドルを持つ線 2 本と円を持つ DXF
    const EDITABLE: &[u8] =
        b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
        0\nSECTION\n2\nENTITIES\n\
        0\nLINE\n5\n2F\n8\n0\n10\n0.0\n20\n0.0\n11\n10.0\n21\n0.0\n\
        0\nCIRCLE\n5\n30\n8\n0\n10\n5.0\n20\n5.0\n40\n2.0\n\
        0\nLINE\n5\n31\n8\n0\n10\n0.0\n20\n1.0\n11\n10.0\n21\n1.0\n0\nENDSEC\n0\nEOF\n";

    /// 開いた文書の要素の ID とハッシュ
    fn seen(session: &mut Session, doc: &str) -> Vec<(String, String)> {
        let (_, response) = call(
            session,
            &format!(r#"{{"v":1,"op":"entities","doc":"{doc}","fields":["hash"]}}"#),
        );
        let Some(Value::Array(items)) = response.pointer("/result/items") else {
            panic!("{response:?}");
        };
        (0..items.len())
            .map(|index| {
                (
                    text(&response, &format!("/result/items/{index}/id")),
                    text(&response, &format!("/result/items/{index}/hash")),
                )
            })
            .collect()
    }

    #[test]
    fn returns_scenes_as_blobs_and_checks_the_origin() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", EDITABLE).unwrap();
        session
            .push(b"p.json", br##"{"values":[{"rgb":"#ff0000","aci":1}]}"##)
            .unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"scene","doc":"a.dxf","from":"space/0","tolerance":0.5,"palette":"p.json"}"#,
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(text(&response, "/result/unit"), "millimeter");
        assert_eq!(number(&response, "/result/tolerance"), "0.5");
        assert_eq!(number(&response, "/result/blob"), "0");
        assert_eq!(text(&response, "/blobs/0/kind"), "scene");
        let scene = session.take(0).unwrap();
        assert_eq!(&scene[..4], b"ZMSC");
        for (request, pointer) in [
            (r#""from":"space/9""#, "/from"),
            (r#""from":"space/0","view":0"#, "/view"),
            (r#""from":"space/0","region":[1,0,0,0]"#, "/region"),
            (r#""from":"space/0","tolerance":0"#, "/tolerance"),
            (r#""from":"space/0","palette":"a.dxf""#, "/palette"),
        ] {
            let (status, response) = call(
                &mut session,
                &format!(r#"{{"v":1,"op":"scene","doc":"a.dxf",{request}}}"#),
            );
            assert_eq!(status, Status::Failed, "{request}");
            assert_eq!(text(&response, "/diagnostics/0/code"), "call.value.invalid");
            assert_eq!(text(&response, "/diagnostics/0/params/pointer"), pointer);
        }
    }

    /// 応答の最初の blob を取り出す
    fn blob_of(session: &mut Session, response: &Value) -> Vec<u8> {
        let id = response
            .pointer("/blobs/0/id")
            .and_then(|v| zumen_json::uint(v, 0, u64::from(u32::MAX)).ok())
            .and_then(|id| u32::try_from(id).ok());
        id.and_then(|id| session.take(id)).unwrap_or_default()
    }

    /// 建物の op の試験に使う、小さな IFC4
    const IFC: &[u8] = b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\n\
FILE_NAME('','',(''),(''),'','','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n\
#1=IFCPROJECT('0000000000000000000001',$,'Project',$,$,$,$,$,#10);\n\
#10=IFCUNITASSIGNMENT((#11));\n\
#11=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n\
#20=IFCLOCALPLACEMENT($,#21);\n\
#21=IFCAXIS2PLACEMENT3D(#22,$,$);\n\
#22=IFCCARTESIANPOINT((0.,0.,3000.));\n\
#34=IFCBUILDINGSTOREY('0000000000000000000034',$,'1F',$,$,#20,$,$,.ELEMENT.,3000.);\n\
#40=IFCRELAGGREGATES('0000000000000000000040',$,$,$,#1,(#34));\n\
#60=IFCRELCONTAINEDINSPATIALSTRUCTURE('0000000000000000000060',$,$,$,(#80),#34);\n\
#61=IFCRELDEFINESBYPROPERTIES('0000000000000000000061',$,$,$,(#80),#62);\n\
#62=IFCPROPERTYSET('0000000000000000000062',$,'Pset_DoorCommon',$,(#63));\n\
#63=IFCPROPERTYSINGLEVALUE('IsExternal',$,IFCBOOLEAN(.T.),$);\n\
#80=IFCDOOR('0000000000000000000080',$,'Door',$,$,$,$,'D-1',2100.,$,.DOOR.,$,$);\n\
ENDSEC;\nEND-ISO-10303-21;\n";

    /// IFC4 の一部を、属性の並びだけ写した小さなスキーマ
    const EXPRESS: &[u8] = b"SCHEMA IFC4;
TYPE IfcGloballyUniqueId = STRING (22) FIXED; END_TYPE;
TYPE IfcLabel = STRING; END_TYPE;
TYPE IfcText = STRING; END_TYPE;
TYPE IfcIdentifier = STRING; END_TYPE;
TYPE IfcPositiveLengthMeasure = IfcLengthMeasure; END_TYPE;
TYPE IfcLengthMeasure = REAL; END_TYPE;
TYPE IfcDoorTypeEnum = ENUMERATION OF (DOOR, GATE, USERDEFINED, NOTDEFINED); END_TYPE;
ENTITY IfcRoot ABSTRACT;
  GlobalId : IfcGloballyUniqueId;
  OwnerHistory : OPTIONAL IfcRoot;
  Name : OPTIONAL IfcLabel;
  Description : OPTIONAL IfcText;
END_ENTITY;
ENTITY IfcDoor SUBTYPE OF (IfcRoot);
  ObjectType : OPTIONAL IfcLabel;
  ObjectPlacement : OPTIONAL IfcRoot;
  Representation : OPTIONAL IfcRoot;
  Tag : OPTIONAL IfcIdentifier;
  OverallHeight : OPTIONAL IfcPositiveLengthMeasure;
  OverallWidth : OPTIONAL IfcPositiveLengthMeasure;
  PredefinedType : OPTIONAL IfcDoorTypeEnum;
  OperationType : OPTIONAL IfcLabel;
  UserDefinedOperationType : OPTIONAL IfcLabel;
END_ENTITY;
END_SCHEMA;
";

    /// 扉の属性を求める IDS。`Tag` は合い、`OverallWidth` は値がない
    const IDS: &[u8] = b"<ids xmlns=\"http://standards.buildingsmart.org/IDS\"><info><title>t</title></info><specifications>\
<specification name=\"tag\" ifcVersion=\"IFC4\"><applicability maxOccurs=\"unbounded\"><entity><name><simpleValue>IFCDOOR</simpleValue></name></entity></applicability>\
<requirements><attribute><name><simpleValue>Tag</simpleValue></name><value><simpleValue>D-1</simpleValue></value></attribute></requirements></specification>\
<specification name=\"width\" ifcVersion=\"IFC4\"><applicability maxOccurs=\"unbounded\"><entity><name><simpleValue>IFCDOOR</simpleValue></name></entity></applicability>\
<requirements><attribute><name><simpleValue>OverallWidth</simpleValue></name></attribute></requirements></specification>\
</specifications></ids>";

    /// 扉の ID とハッシュ
    #[cfg(feature = "bim")]
    fn door(session: &mut Session) -> String {
        let (_, response) = call(
            session,
            r#"{"v":1,"op":"building_elements","doc":"a.ifc","class":"ifcdoor"}"#,
        );
        text(&response, "/result/elements/0/hash")
    }

    #[cfg(feature = "bim")]
    #[test]
    fn applies_edits_to_ifc_and_keeps_other_bytes() {
        for zipped in [false, true] {
            let mut session = Session::new(b"").unwrap();
            let input = if zipped {
                zumen_zip::write(&[("model/a.ifc", IFC)]).unwrap()
            } else {
                IFC.to_vec()
            };
            session.push(b"a.ifc", &input).unwrap();
            call(&mut session, r#"{"v":1,"op":"open","input":"a.ifc"}"#);
            let (_, response) = call(&mut session, r#"{"v":1,"op":"building","doc":"a.ifc"}"#);
            let document = text(&response, "/result/hash");
            let hash = door(&mut session);
            let request = format!(
                r#"{{"v":1,"op":"building_apply","doc":"a.ifc","hash":"{document}","edits":[{{"kind":"property","id":"0000000000000000000080","hash":"{hash}","set":"Pset_DoorCommon","name":"IsExternal","value":{{"kind":"boolean","type_name":"IfcBoolean","boolean":false}}}},{{"kind":"attribute","id":"0000000000000000000080","hash":"{hash}","attribute":"name","value":"扉"}}]}}"#
            );
            let (status, response) = call(&mut session, &request);
            assert_eq!(status, Status::Ok, "{response:?}");
            // 次の呼び出しの前に取り出す
            let blob = blob_of(&mut session, &response);
            assert_eq!(
                text(&response, "/result/applied/0/id"),
                "0000000000000000000080"
            );
            let after = door(&mut session);
            assert_eq!(text(&response, "/result/applied/1/hash"), after);
            assert_ne!(after, hash);
            let written = if zipped {
                assert!(blob.starts_with(b"PK\x03\x04"));
                let mut budget = Budget::default();
                let mut diag = Diagnostics::new();
                let cancel = CancelToken::new();
                let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
                let archive = zumen_zip::read(&blob, &mut cx).unwrap();
                let entry = archive.entries().first().unwrap();
                assert_eq!(entry.name(), "model/a.ifc");
                archive.extract(entry, &mut cx).unwrap()
            } else {
                blob
            };
            let written = String::from_utf8(written).unwrap();
            let original = String::from_utf8(IFC.to_vec()).unwrap();
            // 書き直したのは、扉とプロパティの 2 つのインスタンスだけ
            assert_eq!(
                written
                    .lines()
                    .filter(|line| !original.lines().any(|o| o == *line))
                    .collect::<Vec<_>>(),
                [
                    "#63=IFCPROPERTYSINGLEVALUE('IsExternal',$,IFCBOOLEAN(.F.),$);",
                    "#80=IFCDOOR('0000000000000000000080',$,'\\X2\\6249\\X0\\',$,$,$,$,'D-1',2100.,$,.DOOR.,$,$);",
                ]
            );
            let (_, response) = call(
                &mut session,
                r#"{"v":1,"op":"building_elements","doc":"a.ifc","class":"ifcdoor","components":["identity","properties"]}"#,
            );
            assert_eq!(text(&response, "/result/elements/0/identity/name"), "扉");
            // 前のハッシュでは当てない
            let (status, response) = call(&mut session, &request);
            assert_eq!(status, Status::Failed);
            assert_eq!(
                text(&response, "/diagnostics/0/code"),
                "change.document.stale"
            );
            let stale = request.replace(&format!(r#""hash":"{document}","#), "");
            let (status, response) = call(&mut session, &stale);
            assert_eq!(status, Status::Failed);
            assert_eq!(
                text(&response, "/diagnostics/0/code"),
                "change.element.stale"
            );
            let missing = stale.replace("0000000000000000000080", "0000000000000000000099");
            let (_, response) = call(&mut session, &missing);
            assert_eq!(
                text(&response, "/diagnostics/0/code"),
                "change.element.missing"
            );
        }
    }

    #[test]
    fn opens_ifc_and_answers_the_building_ops() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.ifc", IFC).unwrap();
        session.push(b"door.ids", IDS).unwrap();
        session.push(b"a.dxf", EDITABLE).unwrap();
        let (status, response) = call(&mut session, r#"{"v":1,"op":"open","input":"a.ifc"}"#);
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(text(&response, "/result/format"), "ifc");
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        // 名前は図面の文書と分け合う
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"open","input":"a.dxf","doc":"a.ifc"}"#,
        );
        assert_eq!(status, Status::Failed);
        assert_eq!(
            text(&response, "/diagnostics/0/code"),
            "call.document.exists"
        );

        let (_, response) = call(&mut session, r#"{"v":1,"op":"building","doc":"a.ifc"}"#);
        assert_eq!(text(&response, "/result/schema"), "IFC4");
        assert_eq!(text(&response, "/result/project"), "0000000000000000000001");
        assert_eq!(number(&response, "/result/elements"), "3");
        assert_eq!(text(&response, "/result/classes/1/name"), "IfcDoor");
        assert_eq!(text(&response, "/result/units/0/prefix"), "MILLI");
        let hash = text(&response, "/result/hash");

        let (_, response) = call(
            &mut session,
            r#"{"v":1,"op":"building_elements","doc":"a.ifc","class":"ifcdoor","components":["identity","properties","parent","relations"]}"#,
        );
        assert_eq!(number(&response, "/result/total"), "1");
        assert_eq!(
            text(&response, "/result/elements/0/id"),
            "0000000000000000000080"
        );
        assert_eq!(text(&response, "/result/elements/0/identity/tag"), "D-1");
        assert!(response.pointer("/result/elements/0/class").is_none());
        assert_eq!(
            text(&response, "/result/elements/0/parent/of/target"),
            "0000000000000000000034"
        );
        assert_eq!(
            text(&response, "/result/elements/0/parent/of/kind"),
            "contained"
        );
        assert_eq!(
            response.pointer("/result/elements/0/properties/0/properties/0/value/boolean"),
            Some(&Value::Bool(true))
        );
        assert!(
            response
                .pointer("/result/elements/0/relations/children")
                .is_some()
        );

        let (_, response) = call(
            &mut session,
            r#"{"v":1,"op":"building_elements","doc":"a.ifc","limit":1,"offset":1,"components":["placement","class"]}"#,
        );
        assert_eq!(number(&response, "/result/total"), "3");
        assert_eq!(number(&response, "/result/next_offset"), "2");
        assert_eq!(
            text(&response, "/result/elements/0/class/name"),
            "IfcBuildingStorey"
        );
        assert_eq!(
            number(&response, "/result/elements/0/placement/world/2/3"),
            "3000"
        );

        // 図面の op と建物の op は、文書の種類が合わなければ使えない
        for (request, name, op) in [
            (
                r#"{"v":1,"op":"drawing","doc":"a.ifc"}"#,
                "a.ifc",
                "drawing",
            ),
            (
                r#"{"v":1,"op":"building","doc":"a.dxf"}"#,
                "a.dxf",
                "building",
            ),
        ] {
            let (status, response) = call(&mut session, request);
            assert_eq!(status, Status::Failed);
            assert_eq!(
                text(&response, "/diagnostics/0/code"),
                "call.document.unsupported"
            );
            assert_eq!(text(&response, "/diagnostics/0/params/name"), name);
            assert_eq!(text(&response, "/diagnostics/0/params/op"), op);
        }
        let (_, response) = call(
            &mut session,
            r#"{"v":1,"op":"building_elements","doc":"a.ifc","components":["class","class"]}"#,
        );
        assert_eq!(
            text(&response, "/diagnostics/0/params/pointer"),
            "/components/1"
        );

        // EXPRESS がなければ、属性の要件は確かめられない
        let (_, response) = call(
            &mut session,
            r#"{"v":1,"op":"ids_check","doc":"a.ifc","ids":"door.ids"}"#,
        );
        assert_eq!(text(&response, "/result/title"), "t");
        assert_eq!(text(&response, "/result/specifications/0/status"), "passed");
        assert_eq!(
            text(&response, "/result/specifications/1/status"),
            "unknown"
        );
        assert_eq!(
            text(&response, "/result/findings/0/diagnostic/severity"),
            "info"
        );
        session.push(b"ifc4.exp", EXPRESS).unwrap();
        let (_, response) = call(
            &mut session,
            r#"{"v":1,"op":"ids_check","doc":"a.ifc","ids":"door.ids","express":"ifc4.exp"}"#,
        );
        assert_eq!(text(&response, "/result/specifications/1/status"), "failed");
        assert_eq!(text(&response, "/result/findings/0/specification"), "width");
        assert_eq!(
            text(&response, "/result/findings/0/diagnostic/severity"),
            "error"
        );
        assert_eq!(
            text(&response, "/result/findings/0/diagnostic/at/0/id"),
            "0000000000000000000080"
        );

        // 指摘をレビューの文書に取り込み、同じ版へ移す
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"review_adopt_ids","doc":"a.ifc","ids":"door.ids","express":"ifc4.exp","target":"model","findings":[0]}"#,
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(number(&response, "/result/topics"), "1");
        let review = blob_of(&mut session, &response);
        let document = zumen_json::parse(&review).unwrap();
        assert_eq!(
            document.pointer("/targets/0/format"),
            Some(&Value::String("ifc".to_string()))
        );
        assert_eq!(
            document.pointer("/targets/0/hash"),
            Some(&Value::String(hash.clone()))
        );
        assert_eq!(
            document.pointer("/topics/0/origin/rule"),
            Some(&Value::String("width".to_string()))
        );
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"review_adopt_ids","doc":"a.ifc","ids":"door.ids","target":"model","findings":[1]}"#,
        );
        assert_eq!(status, Status::Failed);
        assert_eq!(
            text(&response, "/diagnostics/0/params/pointer"),
            "/findings/0"
        );
        session.push(b"review.json", &review).unwrap();
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"review_carry","doc":"a.ifc","review":"review.json","target":"model"}"#,
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(text(&response, "/result/hash"), hash);
        assert_eq!(text(&response, "/result/references/0/state"), "unchanged");

        // BCF の topic をレビューの文書にし、要素のハッシュを建物モデルで埋めて、bcfzip に書き戻す
        let topic = "6e4a7c1c-1111-4c3b-9a3e-0f6c3a2b1d01";
        let view = "0b2c3d4e-2222-4f5a-8b6c-7d8e9f0a1b02";
        let markup = format!(
            "<Markup><Topic Guid=\"{topic}\" TopicType=\"Issue\" TopicStatus=\"Done\">\
<Title>t</Title><CreationDate>2026-10-01T09:00:00Z</CreationDate><CreationAuthor>a</CreationAuthor>\
<Viewpoints><ViewPoint Guid=\"{view}\"><Viewpoint>v.bcfv</Viewpoint></ViewPoint></Viewpoints></Topic></Markup>"
        );
        let visinfo = format!(
            "<VisualizationInfo Guid=\"{view}\"><Components><Selection>\
<Component IfcGuid=\"0000000000000000000080\"/></Selection></Components><PerspectiveCamera>\
<CameraViewPoint><X>0</X><Y>0</Y><Z>0</Z></CameraViewPoint><CameraDirection><X>1</X><Y>0</Y><Z>0</Z></CameraDirection>\
<CameraUpVector><X>0</X><Y>0</Y><Z>1</Z></CameraUpVector><FieldOfView>60</FieldOfView><AspectRatio>1</AspectRatio>\
</PerspectiveCamera></VisualizationInfo>"
        );
        let markup_name = format!("{topic}/markup.bcf");
        let visinfo_name = format!("{topic}/v.bcfv");
        let bcfzip = zumen_zip::write(&[
            ("bcf.version", br#"<Version VersionId="3.0"/>"#.as_slice()),
            (&markup_name, markup.as_bytes()),
            (&visinfo_name, visinfo.as_bytes()),
        ])
        .unwrap();
        session.push(b"a.bcf", &bcfzip).unwrap();
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"bcf_read","doc":"a.ifc","input":"a.bcf","target":"model","statuses":[{"text":"Done","status":"resolved"}]}"#,
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(text(&response, "/result/version"), "3.0");
        assert_eq!(number(&response, "/result/topics"), "1");
        let review = blob_of(&mut session, &response);
        let document = zumen_json::parse(&review).unwrap();
        assert_eq!(
            document.pointer("/topics/0/status"),
            Some(&Value::String("resolved".to_string()))
        );
        assert_eq!(
            document.pointer("/targets/0/hash"),
            Some(&Value::String(hash.clone()))
        );
        assert!(matches!(
            document.pointer("/topics/0/locations/0/selected/0/hash"),
            Some(Value::String(_))
        ));
        session.push(b"bcf.json", &review).unwrap();
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"bcf_write","review":"bcf.json","target":"model","original":"a.bcf","statuses":[{"text":"Done","status":"resolved"}]}"#,
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(number(&response, "/result/topics"), "1");
        let written = blob_of(&mut session, &response);
        let mut budget = Budget::new(Limits::default());
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let archive = zumen_zip::read(&written, &mut cx).unwrap();
        let entry = archive.find(&markup_name).unwrap();
        assert_eq!(archive.extract(entry, &mut cx).unwrap(), markup.as_bytes());
        // 建物モデルでない文書は読めない
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"bcf_read","doc":"missing.ifc","input":"a.bcf","target":"model"}"#,
        );
        assert_eq!(status, Status::Failed);
        assert_eq!(
            text(&response, "/diagnostics/0/code"),
            "call.document.missing"
        );

        // 閉じれば、名前が空く
        call(&mut session, r#"{"v":1,"op":"close","docs":["a.ifc"]}"#);
        let (_, response) = call(&mut session, r#"{"v":1,"op":"building","doc":"a.ifc"}"#);
        assert_eq!(
            text(&response, "/diagnostics/0/code"),
            "call.document.missing"
        );
    }

    #[test]
    fn adopts_check_findings_and_carries_them_to_the_document() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", EDITABLE).unwrap();
        session
            .push(
                b"base.json",
                br#"{"format":"0.1","id":"base","edition":"1","revision":1,
                "vocabularies":{"widths":{"values":[0.13]}},
                "rules":[{"id":"widths","check":"line-width","level":"warning","vocabulary":"widths"}]}"#,
            )
            .unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let adopt = |rest: &str| {
            format!(
                r#"{{"v":1,"op":"review_adopt","doc":"a.dxf","profiles":["base.json"],"profile":{{"id":"base","edition":"1"}},"target":"plan"{rest}}}"#
            )
        };
        let (status, response) = call(&mut session, &adopt(r#","findings":[0,1]"#));
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(number(&response, "/result/topics"), "2");
        let topic = text(&response, "/result/added/0");
        assert_eq!(text(&response, "/blobs/0/kind"), "file");
        let blob = u32::try_from(
            response
                .pointer("/blobs/0/id")
                .and_then(|v| zumen_json::uint(v, 0, u64::from(u32::MAX)).ok())
                .unwrap(),
        )
        .unwrap();
        let review = session.take(blob).unwrap();
        let document = zumen_json::parse(&review).unwrap();
        assert_eq!(
            document.pointer("/topics/0/origin/rule"),
            Some(&Value::String("widths".to_string()))
        );
        assert_eq!(
            document.pointer("/topics/0/origin/diagnostic/code"),
            Some(&Value::String("profile.line-width.unsupported".to_string()))
        );
        session.push(b"review.json", &review).unwrap();
        // 取り込み済みの指摘は足さない
        let (_, again) = call(
            &mut session,
            &adopt(r#","review":"review.json","findings":[0]"#),
        );
        assert!(again.pointer("/result/added/0").is_none(), "{again:?}");
        assert_eq!(number(&again, "/result/topics"), "2");

        let carry = |review: &str, target: &str| {
            format!(
                r#"{{"v":1,"op":"review_carry","doc":"a.dxf","review":"{review}","target":"{target}"}}"#
            )
        };
        let (status, response) = call(&mut session, &carry("review.json", "plan"));
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(
            text(&response, "/result/from"),
            text(&response, "/result/hash")
        );
        let total: u64 = number(&response, "/result/total").parse().unwrap();
        assert_eq!(total, 2);
        assert_eq!(text(&response, "/result/references/0/topic"), topic);
        assert_eq!(text(&response, "/result/references/0/state"), "unchanged");
        assert_eq!(text(&response, "/blobs/0/kind"), "file");
        // 参照のハッシュが違えば changed、要素がなければ removed
        let mut changed = String::from_utf8(review).unwrap();
        // 対象・1 つ目の要素・2 つ目の要素の順にハッシュがある。2 つ目の要素のハッシュを変える
        let at = changed.match_indices(r#""hash":"v2:"#).nth(2).unwrap().0 + 11;
        changed.insert(at, '0');
        let changed = changed.replacen(r#"{"id":"space/0/h"#, r#"{"id":"space/0/gone"#, 1);
        session.push(b"changed.json", changed.as_bytes()).unwrap();
        let (_, response) = call(&mut session, &carry("changed.json", "plan"));
        assert_eq!(
            text(&response, "/result/references/0/state"),
            "removed",
            "{response:?}"
        );
        assert_eq!(text(&response, "/result/references/1/state"), "changed");
        let carried = blob_of(&mut session, &response);
        let carried = zumen_json::parse(&carried).unwrap();
        assert_eq!(
            carried.pointer("/topics/0/locations/0/carried/state"),
            Some(&Value::String("removed".to_string()))
        );
        assert_eq!(
            carried.pointer("/topics/1/locations/0/carried/state"),
            Some(&Value::String("changed".to_string()))
        );
        for (request, code) in [
            (carry("review.json", "model"), "review.target.missing"),
            (carry("a.dxf", "plan"), "review.file.invalid"),
            (carry("none.json", "plan"), "call.input.missing"),
            (adopt(r#","findings":[9]"#), "call.value.invalid"),
        ] {
            let (status, response) = call(&mut session, &request);
            assert_eq!(status, Status::Failed, "{request}");
            assert_eq!(text(&response, "/diagnostics/0/code"), code, "{request}");
        }
    }

    #[test]
    fn checks_documents_against_profiles_and_pages_the_findings() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", EDITABLE).unwrap();
        session
            .push(
                b"base.json",
                br#"{"format":"0.1","id":"base","edition":"1","revision":2,
                "vocabularies":{"who":{"values":[{"code":"C"}]},"what":{"values":[{"code":"STR"}]}},
                "rules":[{"id":"layers","check":"layer-name","level":"error","parts":["who","what"],"separator":"-"}]}"#,
            )
            .unwrap();
        session
            .push(
                b"child.json",
                br#"{"format":"0.1","id":"child","edition":"1","revision":1,"extends":{"id":"base","edition":"1"},
                "vocabularies":{"widths":{"values":[0.35]}},
                "rules":[{"id":"widths","check":"line-width","level":"warning","vocabulary":"widths"}]}"#,
            )
            .unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let request = |rest: &str| {
            format!(
                r#"{{"v":1,"op":"check","doc":"a.dxf","profiles":["base.json","child.json"],"profile":{{"id":"child","edition":"1"}}{rest}}}"#
            )
        };
        let (status, response) = call(&mut session, &request(r#","limit":1"#));
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(text(&response, "/result/chain/0/id"), "base");
        assert_eq!(number(&response, "/result/chain/0/revision"), "2");
        assert_eq!(text(&response, "/result/chain/1/id"), "child");
        assert_eq!(text(&response, "/result/rules/0/id"), "layers");
        assert_eq!(number(&response, "/result/rules/0/findings"), "1");
        assert_eq!(text(&response, "/result/findings/0/rule"), "layers");
        assert_eq!(
            text(&response, "/result/findings/0/diagnostic/code"),
            "profile.layer-name.invalid"
        );
        assert_eq!(
            text(&response, "/result/findings/0/diagnostic/severity"),
            "error"
        );
        assert_eq!(
            text(&response, "/result/findings/0/diagnostic/params/layer"),
            "0"
        );
        assert!(response.pointer("/result/findings/1").is_none());
        let total = number(&response, "/result/total");
        assert_eq!(number(&response, "/result/next_offset"), "1");
        let (_, rest) = call(&mut session, &request(r#","offset":1"#));
        assert_eq!(number(&rest, "/result/total"), total);
        assert!(rest.pointer("/result/next_offset").is_none());
        for (rest, code) in [
            (r#","from":"space/9""#, "call.value.invalid"),
            (r#","palette":"a.dxf""#, "call.value.invalid"),
        ] {
            let (status, response) = call(&mut session, &request(rest));
            assert_eq!(status, Status::Failed, "{rest}");
            assert_eq!(text(&response, "/diagnostics/0/code"), code);
        }
        for (profiles, code) in [
            (r#"["child.json"]"#, "profile.extends.missing"),
            (r#"["a.dxf"]"#, "profile.format.unsupported"),
            (r#"["none.json"]"#, "call.input.missing"),
            (r#"["base.json"]"#, "call.value.invalid"),
        ] {
            let (status, response) = call(
                &mut session,
                &format!(
                    r#"{{"v":1,"op":"check","doc":"a.dxf","profiles":{profiles},"profile":{{"id":"child","edition":"1"}}}}"#
                ),
            );
            assert_eq!(status, Status::Failed, "{profiles}");
            assert_eq!(text(&response, "/diagnostics/0/code"), code, "{profiles}");
        }
    }

    #[test]
    fn validates_ifc_against_pushed_express_and_pages_the_findings() {
        let mut session = Session::new(b"").unwrap();
        session
            .push(
                b"tiny.exp",
                b"SCHEMA IFCTINY;\nTYPE LabelT = STRING; WHERE WR1 : TRUE; END_TYPE;\n\
                ENTITY Wall;\n  Name : LabelT;\n  Tag : OPTIONAL LabelT;\nEND_ENTITY;\nEND_SCHEMA;\n",
            )
            .unwrap();
        session
            .push(
                b"a.ifc",
                b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\n\
                FILE_NAME('','',(''),(''),'','','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n\
                #1=WALL('a',$);\n#2=WALL(1,$);\n#3=WALL($,$);\n#4=DOOR();\nENDSEC;\nEND-ISO-10303-21;\n",
            )
            .unwrap();
        let request = |rest: &str| {
            format!(r#"{{"v":1,"op":"validate","input":"a.ifc","express":"tiny.exp"{rest}}}"#)
        };
        let (status, response) = call(&mut session, &request(r#","limit":2"#));
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(text(&response, "/result/schema"), "IFCTINY");
        assert_eq!(text(&response, "/result/file_schema"), "IFC4");
        assert!(text(&response, "/result/express_hash").starts_with("v2:"));
        assert_eq!(number(&response, "/result/rules_not_evaluated"), "1");
        assert_eq!(number(&response, "/result/total"), "3");
        assert_eq!(number(&response, "/result/next_offset"), "2");
        assert_eq!(
            text(&response, "/result/findings/0/code"),
            "express.value.invalid"
        );
        assert_eq!(
            text(&response, "/result/findings/0/params/attribute"),
            "NAME"
        );
        assert_eq!(
            text(&response, "/result/findings/1/code"),
            "express.value.missing"
        );
        let diagnostics: Vec<_> = (0..2)
            .map(|i| text(&response, &format!("/diagnostics/{i}/code")))
            .collect();
        assert_eq!(
            diagnostics,
            ["express.schema.mixed", "express.rule.unsupported"]
        );
        let (_, rest) = call(&mut session, &request(r#","offset":2"#));
        assert_eq!(
            text(&rest, "/result/findings/0/code"),
            "express.entity.missing"
        );
        assert!(rest.pointer("/result/next_offset").is_none());

        session.push(b"bad.exp", b"SCHEMA X;").unwrap();
        for (input, express, code) in [
            ("a.ifc", "none.exp", "call.input.missing"),
            ("../a.ifc", "tiny.exp", "call.value.invalid"),
            ("a.ifc", "bad.exp", "express.file.invalid"),
            ("tiny.exp", "tiny.exp", "ifc.file.invalid"),
        ] {
            let (status, response) = call(
                &mut session,
                &format!(r#"{{"v":1,"op":"validate","input":"{input}","express":"{express}"}}"#),
            );
            assert_eq!(status, Status::Failed, "{input} {express}");
            assert_eq!(text(&response, "/diagnostics/0/code"), code, "{response:?}");
        }
    }

    #[test]
    fn applies_changes_by_id_and_returns_the_written_file() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", EDITABLE).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let (_, summary) = call(&mut session, r#"{"v":1,"op":"drawing","doc":"a.dxf"}"#);
        let document = text(&summary, "/result/hash");
        let before = seen(&mut session, "a.dxf");
        let ids: Vec<&str> = before.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["space/0/h2F", "space/0/h30", "space/0/h31"]);
        let request = format!(
            r#"{{"v":1,"op":"apply","doc":"a.dxf","hash":"{document}","edits":[{{"kind":"transform","id":"{}","hash":"{}","matrix":[1,0,5,0,1,7]}},{{"kind":"restyle","id":"{}","hash":"{}","color":{{"aci":1}},"line_weight":{{"mm":0.35}}}},{{"kind":"remove","id":"{}","hash":"{}"}}]}}"#,
            before[0].0, before[0].1, before[1].0, before[1].1, before[2].0, before[2].1
        );
        let (status, response) = call(&mut session, &request);
        assert_eq!(status, Status::Ok, "{response:?}");
        // バイナリの結果は、次の呼び出しで捨てるので、先に取り出す
        let file = session.take(0).unwrap();
        // 書き戻した文書を読み直した ID とハッシュが、apply の返すものと同じになる
        let after = seen(&mut session, "a.dxf");
        assert_eq!(after.len(), 2);
        for (index, (id, hash)) in after.iter().enumerate() {
            assert_eq!(&text(&response, &format!("/result/applied/{index}/id")), id);
            assert_eq!(
                &text(&response, &format!("/result/applied/{index}/hash")),
                hash
            );
            assert_ne!(hash, &before[index].1);
        }
        assert_eq!(
            response.pointer("/result/applied/2"),
            Some(&Value::Object(Default::default()))
        );
        assert_eq!(text(&response, "/blobs/0/kind"), "file");
        assert_eq!(number(&response, "/blobs/0/size"), file.len().to_string());
        assert_eq!(
            text(&response, "/result/hash"),
            Hash::document(&file).to_string()
        );
        let (_, summary) = call(&mut session, r#"{"v":1,"op":"drawing","doc":"a.dxf"}"#);
        assert_eq!(
            text(&summary, "/result/hash"),
            text(&response, "/result/hash")
        );
        // 書き戻したファイルを開き直しても、同じ要素になる
        session.push(b"b.dxf", &file).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"b.dxf"}"#);
        assert_eq!(seen(&mut session, "b.dxf"), after);
        // 読んだときのハッシュが古い変更は、当てない
        let (status, response) = call(&mut session, &request);
        assert_eq!(status, Status::Failed);
        assert_eq!(text(&error(&response), "/code"), "change.document.stale");
        assert_eq!(response.pointer("/blobs"), Some(&Value::Array(Vec::new())));
        assert_eq!(seen(&mut session, "a.dxf"), after);
    }

    /// 2 つのレイヤと 2 つの線種の表と、線と円を持つ R2004 の DXF
    const TABLED: &str = "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1018\n9\n$INSUNITS\n70\n4\n\
        9\n$HANDSEED\n5\n40\n0\nENDSEC\n0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLTYPE\n\
        0\nLTYPE\n5\n11\n2\nCONTINUOUS\n70\n0\n0\nLTYPE\n5\n12\n2\nDashed\n70\n0\n0\nENDTAB\n\
        0\nTABLE\n2\nLAYER\n0\nLAYER\n5\n10\n2\n0\n70\n0\n62\n7\n6\nCONTINUOUS\n\
        0\nLAYER\n5\n13\n2\nWall\n70\n0\n62\n1\n6\nCONTINUOUS\n0\nENDTAB\n\
        0\nTABLE\n2\nDIMSTYLE\n0\nDIMSTYLE\n105\n1D\n2\nStandard\n70\n0\n41\n2.5\n0\nENDTAB\n0\nENDSEC\n\
        0\nSECTION\n2\nENTITIES\n0\nLINE\n5\n1A\n8\n0\n10\n0.0\n20\n0.0\n11\n1.0\n21\n1.0\n\
        0\nCIRCLE\n5\n1B\n8\n0\n10\n5.0\n20\n5.0\n40\n2.0\n0\nENDSEC\n0\nEOF\n";

    /// 3 つの文字スタイル（形のファイルと大きいフォント・TrueType の書体・形のファイルのレコード）と、表にない
    /// 文字スタイルを指す文字を持つ R2004 の DXF
    const STYLED: &str = "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1018\n9\n$HANDSEED\n5\n40\n\
        0\nENDSEC\n0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nSTYLE\n\
        0\nSTYLE\n5\n11\n2\nStandard\n70\n0\n40\n0.0\n41\n0.8\n50\n15.0\n71\n2\n42\n2.5\n3\ntxt\n4\nextfont2.shx\n\
        0\nSTYLE\n5\n12\n2\nGothic\n70\n4\n40\n3.5\n41\n1.0\n50\n0.0\n71\n0\n42\n3.5\n3\nmsgothic.ttc\n4\n\n\
        1001\nACAD\n1000\nMS Gothic\n1071\n33587249\n\
        0\nSTYLE\n5\n13\n2\n\n70\n1\n40\n0.0\n41\n1.0\n50\n0.0\n71\n0\n42\n1.0\n3\nltypeshp.shx\n4\n\n\
        0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n\
        0\nTEXT\n5\n1A\n8\n0\n10\n0.0\n20\n0.0\n40\n2.5\n1\nA\n7\nMissing\n\
        0\nTEXT\n5\n1B\n8\n0\n10\n0.0\n20\n5.0\n40\n2.5\n1\nB\n7\nGOTHIC\n0\nENDSEC\n0\nEOF\n";

    #[test]
    fn lists_text_styles_with_their_fonts() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", STYLED.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let (status, tables) = call(&mut session, r#"{"v":1,"op":"tables","doc":"a.dxf"}"#);
        assert_eq!(status, Status::Ok, "{tables:?}");
        let mut out = zumen_json::Writer::new();
        out.value(tables.pointer("/result/text_styles").unwrap());
        let styles = String::from_utf8(out.into_bytes()).unwrap();
        let hashes: Vec<String> = (0..3)
            .map(|index| text(&tables, &format!("/result/text_styles/{index}/hash")))
            .collect();
        // 形のファイルのレコードは文字スタイルにしない。表にない名前は、定義のない項目として後ろに足す。
        // 1071 の 33587249 は 0x02008031（太字、文字集合 128、ピッチと系統 0x31）
        assert_eq!(
            styles,
            format!(
                concat!(
                    r#"[{{"id":"text-style/h11","name":"Standard","hash":"{}","defined":true,"#,
                    r#""font":{{"name":"txt","kind":"shape"}},"big_font":{{"name":"extfont2.shx","kind":"shape"}},"#,
                    r#""width":0.8,"oblique":15,"backward":true,"upside_down":false,"vertical":false}},"#,
                    r#"{{"id":"text-style/h12","name":"Gothic","hash":"{}","defined":true,"#,
                    r#""font":{{"name":"MS Gothic","kind":"named","bold":true,"italic":false,"charset":128,"pitch":49}},"#,
                    r#""height":3.5,"width":1,"oblique":0,"backward":false,"upside_down":false,"vertical":true}},"#,
                    r#"{{"id":"text-style/2","name":"Missing","hash":"{}","defined":false}}]"#
                ),
                hashes[0], hashes[1], hashes[2]
            )
        );
    }

    #[test]
    fn changes_adds_and_removes_text_styles() {
        let mut session = Session::new(b"").unwrap();
        // 表にない文字スタイルは並びの番号が ID なので、その前の文字スタイルを除けない。文字は Standard を指す
        let styled = STYLED.replace("7\nMissing", "7\nStandard");
        session.push(b"a.dxf", styled.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let (_, tables) = call(&mut session, r#"{"v":1,"op":"tables","doc":"a.dxf"}"#);
        let standard = text(&tables, "/result/text_styles/0/hash");
        let gothic = text(&tables, "/result/text_styles/1/hash");
        let apply =
            |edits: &str| format!(r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{edits}]}}"#);
        let style = |fields: &str| {
            apply(&format!(
                r#"{{"kind":"text_style","id":"text-style/h11","hash":"{standard}",{fields}}}"#
            ))
        };
        for (request, code, pointer) in [
            (
                style(r#""font":{"name":"txt","kind":"shape","bold":true}"#),
                "call.value.invalid",
                Some("/edits/0/font"),
            ),
            (
                style(r#""height":-1"#),
                "call.value.invalid",
                Some("/edits/0/height"),
            ),
            (
                style(r#""width":0"#),
                "call.value.invalid",
                Some("/edits/0/width"),
            ),
            // Standard の名前は変えない。文字が指す文字スタイルは除かない
            (style(r#""name":"Main""#), "change.edit.unsupported", None),
            (
                apply(&format!(
                    r#"{{"kind":"remove_text_style","id":"text-style/h12","hash":"{gothic}"}}"#
                )),
                "change.edit.unsupported",
                None,
            ),
            (
                apply(&format!(
                    r#"{{"kind":"text_style","id":"text-style/h99","hash":"{standard}"}}"#
                )),
                "change.element.missing",
                None,
            ),
        ] {
            let (status, response) = call(&mut session, &request);
            assert_eq!(status, Status::Failed, "{request}");
            let error = error(&response);
            assert_eq!(text(&error, "/code"), code, "{request}");
            if let Some(pointer) = pointer {
                assert_eq!(text(&error, "/params/pointer"), pointer, "{request}");
            }
        }
        // 足す文字スタイルの書かない欄は、Standard と同じ
        let edits = [
            format!(
                r#"{{"kind":"text_style","id":"text-style/h11","hash":"{standard}","width":0.9,"backward":false}}"#
            ),
            r#"{"kind":"add_text_style","name":"Notes","font":{"name":"simplex.shx","kind":"shape"},"height":2.5}"#
                .to_owned(),
        ];
        let (status, response) = call(&mut session, &apply(&edits.join(",")));
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(text(&response, "/result/applied/0/id"), "text-style/h11");
        assert_eq!(text(&response, "/result/applied/1/id"), "text-style/h40");
        let (_, after) = call(&mut session, r#"{"v":1,"op":"tables","doc":"a.dxf"}"#);
        let mut out = zumen_json::Writer::new();
        out.value(after.pointer("/result/text_styles").unwrap());
        assert_eq!(
            String::from_utf8(out.into_bytes()).unwrap(),
            format!(
                concat!(
                    r#"[{{"id":"text-style/h11","name":"Standard","hash":"{}","defined":true,"#,
                    r#""font":{{"name":"txt","kind":"shape"}},"big_font":{{"name":"extfont2.shx","kind":"shape"}},"#,
                    r#""width":0.9,"oblique":15,"backward":false,"upside_down":false,"vertical":false}},"#,
                    r#"{{"id":"text-style/h12","name":"Gothic","hash":"{}","defined":true,"#,
                    r#""font":{{"name":"MS Gothic","kind":"named","bold":true,"italic":false,"charset":128,"pitch":49}},"#,
                    r#""height":3.5,"width":1,"oblique":0,"backward":false,"upside_down":false,"vertical":true}},"#,
                    r#"{{"id":"text-style/h40","name":"Notes","hash":"{}","defined":true,"#,
                    r#""font":{{"name":"simplex.shx","kind":"shape"}},"big_font":{{"name":"extfont2.shx","kind":"shape"}},"#,
                    r#""height":2.5,"width":0.8,"oblique":15,"backward":true,"upside_down":false,"vertical":false}}]"#
                ),
                text(&response, "/result/applied/0/hash"),
                gothic,
                text(&response, "/result/applied/1/hash"),
            )
        );

        // 足した文字スタイルは、次の apply で除ける
        let notes = text(&response, "/result/applied/1/hash");
        let (status, removed) = call(
            &mut session,
            &apply(&format!(
                r#"{{"kind":"remove_text_style","id":"text-style/h40","hash":"{notes}"}}"#
            )),
        );
        assert_eq!(status, Status::Ok, "{removed:?}");
        assert_eq!(removed.pointer("/result/applied/0/id"), None);
        let (_, last) = call(&mut session, r#"{"v":1,"op":"tables","doc":"a.dxf"}"#);
        assert_eq!(last.pointer("/result/text_styles/2"), None);
    }

    /// レイヤ A・B、利用者定義色（10, 20, 30）、利用者定義線幅 0.3 と線分を持つ SFC
    const SFC: &str = concat!(
        "ISO-10303-21;\nHEADER;\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n",
        "/*SXF\n#10 = layer_feature(\\'A\\','1')\nSXF*/\n",
        "/*SXF\n#20 = layer_feature(\\'B\\','1')\nSXF*/\n",
        "/*SXF\n#30 = user_defined_colour_feature('10','20','30')\nSXF*/\n",
        "/*SXF\n#40 = width_feature('0.3')\nSXF*/\n",
        "/*SXF\n#50 = line_feature('1','1','1','1','0','0','10','0')\nSXF*/\n",
        "/*SXF\n#60 = drawing_sheet_feature(\\'S\\','3','1','0','0')\nSXF*/\n",
        "ENDSEC;\nEND-ISO-10303-21;\n",
    );

    #[test]
    fn applies_changes_to_sfc() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.sfc", SFC.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.sfc"}"#);
        let before = seen(&mut session, "a.sfc");
        let (id, hash) = &before[0];
        assert_eq!(id, "sheet/0/i50");
        let apply =
            |edits: &str| format!(r#"{{"v":1,"op":"apply","doc":"a.sfc","edits":[{edits}]}}"#);
        // aci と sxf は一緒に書かない。SXF はレイヤに従う色を書けない
        for (request, code) in [
            (
                apply(&format!(
                    r#"{{"kind":"restyle","id":"{id}","hash":"{hash}","color":{{"aci":1,"sxf":1}}}}"#
                )),
                "call.value.invalid",
            ),
            (
                apply(&format!(
                    r#"{{"kind":"restyle","id":"{id}","hash":"{hash}","color":{{"aci":1}}}}"#
                )),
                "change.edit.unsupported",
            ),
            (
                apply(
                    r#"{"kind":"add","place":"sheet/0","layer":"A","geometry":{"shape":"line","start":[0,1],"end":[10,1]}}"#,
                ),
                "change.edit.unsupported",
            ),
        ] {
            let (status, response) = call(&mut session, &request);
            assert_eq!(status, Status::Failed, "{request}");
            assert_eq!(text(&error(&response), "/code"), code, "{request}");
        }
        let edits = [
            format!(
                r#"{{"kind":"restyle","id":"{id}","hash":"{hash}","layer":"B","color":{{"sxf":17}},"line_weight":{{"mm":0.3}}}}"#
            ),
            concat!(
                r#"{"kind":"add","place":"sheet/0","layer":"A","color":{"rgb":[10,20,30]},"#,
                r#""line_type":{"name":"continuous"},"line_weight":{"mm":0.25},"#,
                r#""geometry":{"shape":"circle","center":[5,5],"radius":2}}"#
            )
            .to_owned(),
        ];
        let (status, response) = call(&mut session, &apply(&edits.join(",")));
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(text(&response, "/result/applied/0/id"), "sheet/0/i50");
        assert_eq!(text(&response, "/result/applied/1/id"), "sheet/0/i61");
        let file = String::from_utf8(session.take(0).unwrap()).unwrap();
        assert!(file.contains("#50 = line_feature('2','17','1','11','0','0','10','0')"));
        assert!(file.contains(
            "/*SXF\n#61 = circle_feature('1','17','1','3','5','5','2')\nSXF*/\n/*SXF\n#60 ="
        ));
        let after = seen(&mut session, "a.sfc");
        assert_eq!(after[0].1, text(&response, "/result/applied/0/hash"));
        assert_eq!(after[1].1, text(&response, "/result/applied/1/hash"));
    }

    #[test]
    fn reads_and_writes_back_sxf_in_a_zip() {
        // P2Z・SFZ（#385）。SXF の項目のほかの項目は、名前で返し、書き戻す書庫にそのまま写す
        let packed = zumen_zip::write(&[
            ("ATTR/", b"".as_slice()),
            ("ATTR/A.SAF", b"saf".as_slice()),
            ("D0PL001Z.SFC", SFC.as_bytes()),
        ])
        .unwrap();
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.sfz", &packed).unwrap();
        let (status, response) = call(&mut session, r#"{"v":1,"op":"open","input":"a.sfz"}"#);
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(text(&response, "/result/format"), "sxf");
        assert_eq!(text(&response, "/result/entries/0"), "ATTR/");
        assert_eq!(text(&response, "/result/entries/1"), "ATTR/A.SAF");
        let before = seen(&mut session, "a.sfz");
        let (id, hash) = &before[0];
        let request = format!(
            r#"{{"v":1,"op":"apply","doc":"a.sfz","edits":[{{"kind":"restyle","id":"{id}","hash":"{hash}","layer":"B"}}]}}"#
        );
        let (status, response) = call(&mut session, &request);
        assert_eq!(status, Status::Ok, "{response:?}");
        let written = session.take(0).unwrap();
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let archive = zumen_zip::read(&written, &mut cx).unwrap();
        let names: Vec<&str> = archive.entries().iter().map(|entry| entry.name()).collect();
        assert_eq!(names, ["ATTR/", "ATTR/A.SAF", "D0PL001Z.SFC"]);
        let saf = archive.find("ATTR/A.SAF").unwrap();
        assert_eq!(archive.extract(saf, &mut cx).unwrap(), b"saf");
        let sfc = archive.find("D0PL001Z.SFC").unwrap();
        let sfc = String::from_utf8(archive.extract(sfc, &mut cx).unwrap()).unwrap();
        assert!(sfc.contains("#50 = line_feature('2',"), "{sfc}");
        let mut again = Session::new(b"").unwrap();
        again.push(b"b.sfz", &written).unwrap();
        let (status, _) = call(&mut again, r#"{"v":1,"op":"open","input":"b.sfz"}"#);
        assert_eq!(status, Status::Ok);
        // SXF の項目のない ZIP は、SXF として読まない
        let other = zumen_zip::write(&[("A.TXT", b"text".as_slice())]).unwrap();
        again.push(b"c.zip", &other).unwrap();
        let (status, _) = call(&mut again, r#"{"v":1,"op":"open","input":"c.zip"}"#);
        assert_eq!(status, Status::Failed);
    }

    #[test]
    fn changes_adds_and_removes_dimension_styles() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", TABLED.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let (_, tables) = call(&mut session, r#"{"v":1,"op":"tables","doc":"a.dxf"}"#);
        let standard = text(&tables, "/result/dimension_styles/0/hash");
        let apply =
            |edits: &str| format!(r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{edits}]}}"#);
        // Standard の名前は変えず、除かない。ブロックのない端部記号・2 つでない端部記号・負の大きさは書けない
        for (request, code) in [
            (
                apply(&format!(
                    r#"{{"kind":"dimension_style","id":"dimension-style/h1D","hash":"{standard}","name":"Main"}}"#
                )),
                "change.edit.unsupported",
            ),
            (
                apply(&format!(
                    r#"{{"kind":"remove_dimension_style","id":"dimension-style/h1D","hash":"{standard}"}}"#
                )),
                "change.edit.unsupported",
            ),
            (
                apply(r#"{"kind":"add_dimension_style","name":"Dot","leader":"_DOT"}"#),
                "change.edit.unsupported",
            ),
            (
                apply(r#"{"kind":"add_dimension_style","name":"Dot","ends":["_DOT"]}"#),
                "call.value.invalid",
            ),
            (
                apply(&format!(
                    r#"{{"kind":"dimension_style","id":"dimension-style/h1D","hash":"{standard}","size":-1}}"#
                )),
                "call.value.invalid",
            ),
            (
                apply(&format!(
                    r#"{{"kind":"dimension_style","id":"dimension-style/h99","hash":"{standard}"}}"#
                )),
                "change.element.missing",
            ),
        ] {
            let (status, response) = call(&mut session, &request);
            assert_eq!(status, Status::Failed, "{request}");
            assert_eq!(text(&error(&response), "/code"), code, "{request}");
        }
        // 足す寸法スタイルの書かない欄は、Standard と同じ
        let edits = [
            format!(
                r#"{{"kind":"dimension_style","id":"dimension-style/h1D","hash":"{standard}","size":3.5,"scale":100}}"#
            ),
            r#"{"kind":"add_dimension_style","name":"Plan","tick":1.5}"#.to_owned(),
        ];
        let (status, response) = call(&mut session, &apply(&edits.join(",")));
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(
            text(&response, "/result/applied/0/id"),
            "dimension-style/h1D"
        );
        assert_eq!(
            text(&response, "/result/applied/1/id"),
            "dimension-style/h40"
        );
        let (_, after) = call(&mut session, r#"{"v":1,"op":"tables","doc":"a.dxf"}"#);
        let mut out = zumen_json::Writer::new();
        out.value(after.pointer("/result/dimension_styles").unwrap());
        assert_eq!(
            String::from_utf8(out.into_bytes()).unwrap(),
            format!(
                concat!(
                    r#"[{{"id":"dimension-style/h1D","name":"Standard","hash":"{}","ends":["",""],"#,
                    r#""leader":"","size":3.5,"tick":0,"scale":100}},"#,
                    r#"{{"id":"dimension-style/h40","name":"Plan","hash":"{}","ends":["",""],"#,
                    r#""leader":"","size":2.5,"tick":1.5,"scale":1}}]"#
                ),
                text(&response, "/result/applied/0/hash"),
                text(&response, "/result/applied/1/hash"),
            )
        );
        // 足した寸法スタイルは、次の apply で除ける
        let plan = text(&response, "/result/applied/1/hash");
        let (status, removed) = call(
            &mut session,
            &apply(&format!(
                r#"{{"kind":"remove_dimension_style","id":"dimension-style/h40","hash":"{plan}"}}"#
            )),
        );
        assert_eq!(status, Status::Ok, "{removed:?}");
        assert_eq!(removed.pointer("/result/applied/0/id"), None);
        let (_, last) = call(&mut session, r#"{"v":1,"op":"tables","doc":"a.dxf"}"#);
        assert_eq!(last.pointer("/result/dimension_styles/1"), None);
    }

    #[test]
    fn line_type_marks_are_listed_with_their_text_heights() {
        // 文字を添える GAS と形を添える ZIGZAG。文字スタイル Gas は決まった高さ 2 を持つ
        let src = "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1018\n9\n$HANDSEED\n5\n40\n0\nENDSEC\n\
            0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLTYPE\n\
            0\nLTYPE\n5\n14\n2\nGAS\n70\n0\n3\nGas ---- GAS ----\n72\n65\n73\n2\n40\n1.5\n\
            49\n1.0\n74\n2\n75\n0\n340\n11\n46\n0.25\n50\n0.0\n44\n-0.1\n45\n-0.05\n9\nGAS\n49\n-0.5\n74\n0\n\
            0\nLTYPE\n5\n15\n2\nZIGZAG\n70\n0\n3\nZig zag\n72\n65\n73\n2\n40\n0.4\n\
            49\n0.2\n74\n5\n75\n2\n340\n12\n46\n0.2\n50\n0.0\n44\n0.0\n45\n0.0\n49\n-0.2\n74\n0\n\
            0\nENDTAB\n0\nTABLE\n2\nSTYLE\n\
            0\nSTYLE\n5\n11\n2\nGas\n70\n0\n40\n2.0\n41\n1.0\n50\n0.0\n71\n0\n42\n2.0\n3\ntxt\n4\n\n\
            0\nSTYLE\n5\n12\n2\n\n70\n1\n40\n0.0\n41\n1.0\n50\n0.0\n71\n0\n42\n1.0\n3\nltypeshp.shx\n4\n\n\
            0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n";
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", src.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let (status, tables) = call(&mut session, r#"{"v":1,"op":"tables","doc":"a.dxf"}"#);
        assert_eq!(status, Status::Ok, "{tables:?}");
        let mut out = zumen_json::Writer::new();
        out.value(tables.pointer("/result/line_types/0/marks").unwrap());
        out.value(tables.pointer("/result/line_types/1/marks").unwrap());
        assert_eq!(
            String::from_utf8(out.into_bytes()).unwrap(),
            concat!(
                r#"[{"index":0,"kind":"text","text":"GAS","style":"Gas","style_id":"text-style/h11","#,
                r#""height":0.5,"scale":0.25,"rotation":0,"rotation_basis":"relative","offset":[-0.1,-0.05]}]"#,
                r#"[{"index":0,"kind":"shape","font":{"name":"ltypeshp.shx","kind":"shape"},"number":2,"#,
                r#""scale":0.2,"rotation":0,"rotation_basis":"absolute","offset":[0,0]}]"#
            )
        );
        // どちらも有限でも、積が大きすぎる高さは JSON に書けない。
        let overflow = src
            .replace("46\n0.25\n", "46\n1e200\n")
            .replace("2\nGas\n70\n0\n40\n2.0\n", "2\nGas\n70\n0\n40\n1e200\n");
        let mut session = Session::new(b"").unwrap();
        session.push(b"overflow.dxf", overflow.as_bytes()).unwrap();
        call(
            &mut session,
            r#"{"v":1,"op":"open","input":"overflow.dxf"}"#,
        );
        let (status, result) = call(
            &mut session,
            r#"{"v":1,"op":"tables","doc":"overflow.dxf"}"#,
        );
        assert_eq!(status, Status::Ok);
        assert_eq!(result.pointer("/result/line_types/0/marks/0/height"), None);
        let mut diagnostics = zumen_json::Writer::new();
        diagnostics.value(result.pointer("/diagnostics").unwrap());
        let diagnostics = String::from_utf8(diagnostics.into_bytes()).unwrap();
        assert!(
            diagnostics.contains("drawing.mark-height.dropped"),
            "{diagnostics}"
        );
        // 入力に有限でない倍率・回転・ずれがあれば、添える文字は診断して移さない。
        for (field, value) in [
            ("46\n0.25\n", "46\nNaN\n"),
            ("50\n0.0\n", "50\ninf\n"),
            ("44\n-0.1\n", "44\n-inf\n"),
            ("45\n-0.05\n", "45\nNaN\n"),
        ] {
            let invalid = src.replacen(field, value, 1);
            let mut session = Session::new(b"").unwrap();
            session.push(b"invalid.dxf", invalid.as_bytes()).unwrap();
            call(&mut session, r#"{"v":1,"op":"open","input":"invalid.dxf"}"#);
            let (status, result) =
                call(&mut session, r#"{"v":1,"op":"tables","doc":"invalid.dxf"}"#);
            assert_eq!(status, Status::Ok, "{field}");
            let mut diagnostics = zumen_json::Writer::new();
            diagnostics.value(result.pointer("/result/line_types").unwrap());
            diagnostics.value(result.pointer("/diagnostics").unwrap());
            let diagnostics = String::from_utf8(diagnostics.into_bytes()).unwrap();
            assert!(
                !diagnostics.contains("\"kind\":\"text\""),
                "{field}: {diagnostics}"
            );
            assert!(
                diagnostics.contains("dxf.lift.invalid")
                    || diagnostics.contains("dxf.line-type-element.dropped"),
                "{field}: {diagnostics}"
            );
        }
    }

    #[test]
    fn line_type_mark_heights_use_a_bounded_style_lookup() {
        let style = |name: String, height: f64| TextStyle {
            name,
            definition: Some(TextStyleDefinition {
                font: None,
                big_font: None,
                height: Some(height),
                width: 1.0,
                oblique: 0.0,
                backward: false,
                upside_down: false,
                vertical: false,
            }),
            source: Ref::default(),
        };
        let mut drawing = Drawing2D {
            text_styles: (0..256)
                .map(|i| style(format!("style{i:03}"), 2.0))
                .collect(),
            ..Drawing2D::default()
        };
        // 同名の後ろの項目より先の項目を使う。
        drawing.text_styles.push(style("STYLE255".to_owned(), 8.0));
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut budget = Budget::default();
        let ids = StyleIds::new(
            &drawing,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        )
        .ok()
        .unwrap();
        let line_type = LineType {
            id: LineTypeId(0),
            name: "marked".to_owned(),
            definition: Some(LineTypeDefinition {
                description: String::new(),
                pattern: vec![1.0],
                basis: PatternBasis::Element,
                marks: (0..64)
                    .map(|_| zumen_drawing::LineTypeMark {
                        index: 0,
                        content: MarkContent::Text {
                            content: "GAS".to_owned(),
                            style: Some("STYLE255".to_owned()),
                        },
                        scale: 0.5,
                        rotation: 0.0,
                        rotation_basis: zumen_drawing::MarkRotation::Relative,
                        offset: [0.0, 0.0],
                    })
                    .collect(),
            }),
            source: Ref::default(),
        };
        let hash = Hash::line_type(&line_type);
        let mut budget = Budget::default();
        let result = line_type_item(
            "line-type/0".to_owned(),
            &line_type,
            hash,
            &ids,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        )
        .ok()
        .unwrap();
        let work = budget.used(Kind::Work);
        assert!(work >= 64);
        assert!(
            result
                .marks
                .unwrap()
                .iter()
                .all(|mark| mark.height == Some(1.0))
        );
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(work - 1)));
        assert!(
            line_type_item(
                "line-type/0".to_owned(),
                &line_type,
                hash,
                &ids,
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel)
            )
            .is_err()
        );
        cancel.cancel();
        let mut budget = Budget::default();
        assert!(
            line_type_item(
                "line-type/0".to_owned(),
                &line_type,
                hash,
                &ids,
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel)
            )
            .is_err()
        );
    }

    #[test]
    fn line_type_marks_without_style_count_construction_and_text_copies() {
        let line_type = LineType {
            id: LineTypeId(0),
            name: "marked".to_owned(),
            definition: Some(LineTypeDefinition {
                description: String::new(),
                pattern: vec![1.0],
                basis: PatternBasis::Element,
                marks: (0..64)
                    .map(|_| zumen_drawing::LineTypeMark {
                        index: 0,
                        content: MarkContent::Text {
                            content: "A".repeat(2048),
                            style: None,
                        },
                        scale: 1.0,
                        rotation: 0.0,
                        rotation_basis: zumen_drawing::MarkRotation::Relative,
                        offset: [0.0, 0.0],
                    })
                    .collect(),
            }),
            source: Ref::default(),
        };
        let hash = Hash::line_type(&line_type);
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut budget = Budget::default();
        let ids = StyleIds::default();
        assert!(
            line_type_item(
                "line-type/0".to_owned(),
                &line_type,
                hash,
                &ids,
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel)
            )
            .is_ok()
        );
        let work = budget.used(Kind::Work);
        assert!(work >= 64 * 3, "{work}");
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(work - 1)));
        assert!(
            line_type_item(
                "line-type/0".to_owned(),
                &line_type,
                hash,
                &ids,
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel)
            )
            .is_err()
        );
    }

    #[test]
    fn changes_adds_and_removes_line_types() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", TABLED.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let (_, tables) = call(&mut session, r#"{"v":1,"op":"tables","doc":"a.dxf"}"#);
        let continuous = text(&tables, "/result/line_types/0/hash");
        let dashed = text(&tables, "/result/line_types/1/hash");
        let apply =
            |edits: &str| format!(r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{edits}]}}"#);
        // Continuous の名前は変えず、除かない。ない線種は指せない
        for (request, code) in [
            (
                apply(&format!(
                    r#"{{"kind":"line_type","id":"line-type/h11","hash":"{continuous}","name":"Solid"}}"#
                )),
                "change.edit.unsupported",
            ),
            (
                apply(&format!(
                    r#"{{"kind":"remove_line_type","id":"line-type/h11","hash":"{continuous}"}}"#
                )),
                "change.edit.unsupported",
            ),
            (
                apply(&format!(
                    r#"{{"kind":"line_type","id":"line-type/h99","hash":"{dashed}"}}"#
                )),
                "change.element.missing",
            ),
        ] {
            let (status, response) = call(&mut session, &request);
            assert_eq!(status, Status::Failed, "{request}");
            assert_eq!(text(&error(&response), "/code"), code, "{request}");
        }
        // 足す線種の書かない欄は、Continuous と同じ
        let edits = [
            format!(
                r#"{{"kind":"line_type","id":"line-type/h12","hash":"{dashed}","name":"Hidden","pattern":[0.5,-0.25]}}"#
            ),
            r#"{"kind":"add_line_type","name":"Center","description":"Center ____ _ ____","pattern":[1.25,-0.25,0.25,-0.25]}"#
                .to_owned(),
        ];
        let (status, response) = call(&mut session, &apply(&edits.join(",")));
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(text(&response, "/result/applied/0/id"), "line-type/h12");
        assert_eq!(text(&response, "/result/applied/1/id"), "line-type/h40");
        let (_, after) = call(&mut session, r#"{"v":1,"op":"tables","doc":"a.dxf"}"#);
        let mut out = zumen_json::Writer::new();
        out.value(after.pointer("/result/line_types").unwrap());
        assert_eq!(
            String::from_utf8(out.into_bytes()).unwrap(),
            format!(
                concat!(
                    r#"[{{"id":"line-type/h11","name":"CONTINUOUS","hash":"{}","defined":true,"#,
                    r#""description":"","pattern":[],"paper":false,"marked":false,"marks":[]}},"#,
                    r#"{{"id":"line-type/h12","name":"Hidden","hash":"{}","defined":true,"#,
                    r#""description":"","pattern":[0.5,-0.25],"paper":false,"marked":false,"marks":[]}},"#,
                    r#"{{"id":"line-type/h40","name":"Center","hash":"{}","defined":true,"#,
                    r#""description":"Center ____ _ ____","pattern":[1.25,-0.25,0.25,-0.25],"paper":false,"marked":false,"marks":[]}}]"#
                ),
                continuous,
                text(&response, "/result/applied/0/hash"),
                text(&response, "/result/applied/1/hash"),
            )
        );
        // 足した線種は、次の apply で除ける
        let center = text(&response, "/result/applied/1/hash");
        let (status, removed) = call(
            &mut session,
            &apply(&format!(
                r#"{{"kind":"remove_line_type","id":"line-type/h40","hash":"{center}"}}"#
            )),
        );
        assert_eq!(status, Status::Ok, "{removed:?}");
        assert_eq!(removed.pointer("/result/applied/0/id"), None);
        let (_, last) = call(&mut session, r#"{"v":1,"op":"tables","doc":"a.dxf"}"#);
        assert_eq!(last.pointer("/result/line_types/2"), None);
    }

    #[test]
    fn lists_table_items_and_restyles_with_their_ids() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", TABLED.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let (status, tables) = call(&mut session, r#"{"v":1,"op":"tables","doc":"a.dxf"}"#);
        assert_eq!(status, Status::Ok, "{tables:?}");
        let item = |table: &str, name: &str| {
            let Some(Value::Array(items)) = tables.pointer(&format!("/result/{table}")) else {
                panic!("{tables:?}");
            };
            let item = items
                .iter()
                .find(|item| text(item, "/name") == name)
                .unwrap_or_else(|| panic!("{name}: {tables:?}"));
            assert!(text(item, "/hash").starts_with("v2:"));
            text(item, "/id")
        };
        assert_eq!(item("layers", "0"), "layer/h10");
        assert_eq!(item("layers", "Wall"), "layer/h13");
        assert_eq!(item("line_types", "Dashed"), "line-type/h12");
        // DIMSTYLE のハンドルは 105 に書く
        assert_eq!(item("dimension_styles", "Standard"), "dimension-style/h1D");
        let before = seen(&mut session, "a.dxf");
        let (id, hash) = &before[0];
        let restyle = |fields: &str| {
            format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"restyle","id":"{id}","hash":"{hash}",{fields}}}]}}"#
            )
        };
        for (fields, pointer) in [
            (r#""layer_id":"layer/h99""#, "/edits/0/layer_id"),
            (
                r#""layer":"Wall","layer_id":"layer/h13""#,
                "/edits/0/layer_id",
            ),
            (
                r#""line_type":{"id":"line-type/h99"}"#,
                "/edits/0/line_type",
            ),
            (
                r#""line_type":{"id":"line-type/h12","name":"Dashed"}"#,
                "/edits/0/line_type",
            ),
        ] {
            let (status, response) = call(&mut session, &restyle(fields));
            assert_eq!(status, Status::Failed, "{fields}");
            let error = error(&response);
            assert_eq!(text(&error, "/code"), "call.value.invalid", "{fields}");
            assert_eq!(text(&error, "/params/pointer"), pointer, "{fields}");
        }
        let (status, response) = call(
            &mut session,
            &restyle(r#""layer_id":"layer/h13","line_type":{"id":"line-type/h12"}"#),
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        let (_, listed) = call(
            &mut session,
            r#"{"v":1,"op":"entities","doc":"a.dxf","fields":["layer","hash"]}"#,
        );
        assert_eq!(text(&listed, "/result/items/0/layer"), "Wall");
        assert_eq!(
            text(&listed, "/result/items/0/hash"),
            text(&response, "/result/applied/0/hash")
        );
        // 表の項目の ID は、書き戻しても変わらない
        let (_, again) = call(&mut session, r#"{"v":1,"op":"tables","doc":"a.dxf"}"#);
        assert_eq!(again.pointer("/result"), tables.pointer("/result"));
    }

    #[test]
    fn changes_and_adds_layers_with_their_definitions() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", TABLED.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let (_, tables) = call(&mut session, r#"{"v":1,"op":"tables","doc":"a.dxf"}"#);
        let hash = text(&tables, "/result/layers/1/hash");
        let mut out = zumen_json::Writer::new();
        out.value(tables.pointer("/result/layers/1").unwrap());
        assert_eq!(
            String::from_utf8(out.into_bytes()).unwrap(),
            format!(
                concat!(
                    r#"{{"id":"layer/h13","name":"Wall","hash":"{}","defined":true,"visible":true,"#,
                    r#""printable":true,"editable":true,"frozen":false,"color":{{"aci":1}},"#,
                    r#""line_type_id":"line-type/h11"}}"#
                ),
                hash
            )
        );
        let apply =
            |edits: &str| format!(r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{edits}]}}"#);
        let layer = |fields: &str| {
            apply(&format!(
                r#"{{"kind":"layer","id":"layer/h13","hash":"{hash}",{fields}}}"#
            ))
        };
        for (request, code, pointer) in [
            (
                layer(r#""color":{"inherit":"layer"}"#),
                "call.value.invalid",
                Some("/edits/0/color"),
            ),
            (
                layer(r#""line_type":{"name":"Missing"}"#),
                "call.value.invalid",
                Some("/edits/0/line_type"),
            ),
            (
                layer(r#""line_weight":{"inherit":"block"}"#),
                "call.value.invalid",
                Some("/edits/0/line_weight"),
            ),
            (layer(r#""name":"0""#), "change.edit.unsupported", None),
            (
                apply(&format!(
                    r#"{{"kind":"layer","id":"layer/h99","hash":"{hash}"}}"#
                )),
                "change.element.missing",
                None,
            ),
            (
                apply(r#"{"kind":"add_layer","name":"wall"}"#),
                "change.edit.unsupported",
                None,
            ),
        ] {
            let (status, response) = call(&mut session, &request);
            assert_eq!(status, Status::Failed, "{request}");
            let error = error(&response);
            assert_eq!(text(&error, "/code"), code, "{request}");
            if let Some(pointer) = pointer {
                assert_eq!(text(&error, "/params/pointer"), pointer, "{request}");
            }
        }
        // 凍結するだけなら表示もしない。足すレイヤの書かない色と線種は、レイヤ 0 と同じ
        let edits = [
            format!(
                r#"{{"kind":"layer","id":"layer/h13","hash":"{hash}","name":"Walls","frozen":true,"color":{{"aci":3}},"line_type":{{"name":"Dashed"}},"line_weight":{{"mm":0.5}}}}"#
            ),
            r#"{"kind":"add_layer","name":"Grid","printable":false}"#.to_owned(),
        ];
        let (status, response) = call(&mut session, &apply(&edits.join(",")));
        assert_eq!(status, Status::Ok, "{response:?}");
        let (_, after) = call(&mut session, r#"{"v":1,"op":"tables","doc":"a.dxf"}"#);
        let mut out = zumen_json::Writer::new();
        out.value(after.pointer("/result/layers").unwrap());
        assert_eq!(
            String::from_utf8(out.into_bytes()).unwrap(),
            format!(
                concat!(
                    r#"[{{"id":"layer/h10","name":"0","hash":"{}","defined":true,"visible":true,"#,
                    r#""printable":true,"editable":true,"frozen":false,"color":{{"aci":7}},"#,
                    r#""line_type_id":"line-type/h11"}},"#,
                    r#"{{"id":"layer/h13","name":"Walls","hash":"{}","defined":true,"visible":false,"#,
                    r#""printable":true,"editable":true,"frozen":true,"color":{{"aci":3}},"#,
                    r#""line_type_id":"line-type/h12","line_weight":{{"mm":0.5}}}},"#,
                    r#"{{"id":"layer/h40","name":"Grid","hash":"{}","defined":true,"visible":true,"#,
                    r#""printable":false,"editable":true,"frozen":false,"color":{{"aci":7}},"#,
                    r#""line_type_id":"line-type/h11"}}]"#
                ),
                text(&after, "/result/layers/0/hash"),
                text(&response, "/result/applied/0/hash"),
                text(&response, "/result/applied/1/hash"),
            )
        );
        assert_eq!(text(&response, "/result/applied/0/id"), "layer/h13");
        assert_eq!(text(&response, "/result/applied/1/id"), "layer/h40");
        // レイヤ 0 は除けない。足したレイヤは、次の apply で除ける
        let remove = |id: &str, hash: &str| {
            apply(&format!(
                r#"{{"kind":"remove_layer","id":"{id}","hash":"{hash}"}}"#
            ))
        };
        let zero = text(&after, "/result/layers/0/hash");
        let (status, refused) = call(&mut session, &remove("layer/h10", &zero));
        assert_eq!(status, Status::Failed, "{refused:?}");
        assert_eq!(text(&error(&refused), "/code"), "change.edit.unsupported");
        let grid = text(&response, "/result/applied/1/hash");
        let (status, removed) = call(&mut session, &remove("layer/h40", &grid));
        assert_eq!(status, Status::Ok, "{removed:?}");
        assert_eq!(removed.pointer("/result/applied/0/id"), None);
        let (_, last) = call(&mut session, r#"{"v":1,"op":"tables","doc":"a.dxf"}"#);
        assert_eq!(text(&last, "/result/layers/0/id"), "layer/h10");
        assert_eq!(text(&last, "/result/layers/1/id"), "layer/h13");
        assert_eq!(last.pointer("/result/layers/2"), None);
    }

    #[test]
    fn repeated_apply_accepts_latest_document_hash() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", EDITABLE).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let before = seen(&mut session, "a.dxf");
        let (_, drawing) = call(&mut session, r#"{"v":1,"op":"drawing","doc":"a.dxf"}"#);
        let document = text(&drawing, "/result/hash");
        let request = |id: &str, hash: &str, document: &str| {
            format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","hash":"{document}","edits":[{{"kind":"transform","id":"{id}","hash":"{hash}","matrix":[1,0,5,0,1,7]}}]}}"#
            )
        };
        let (status, response) = call(
            &mut session,
            &request(&before[0].0, &before[0].1, &document),
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        let document = text(&response, "/result/hash");
        let id = text(&response, "/result/applied/0/id");
        let hash = text(&response, "/result/applied/0/hash");
        let bytes = session.take(0).unwrap();
        assert_eq!(document, Hash::document(&bytes).to_string());
        let (status, rejected) = call(&mut session, &request(&id, &before[0].1, &document));
        assert_eq!(status, Status::Failed, "{rejected:?}");
        assert_eq!(text(&error(&rejected), "/code"), "change.element.stale");
        assert_eq!(rejected.pointer("/blobs"), Some(&Value::Array(Vec::new())));
        assert!(session.take(0).is_none());
        let (status, response) = call(&mut session, &request(&id, &hash, &document));
        assert_eq!(status, Status::Ok, "{response:?}");
        let second = text(&response, "/result/applied/0/hash");
        assert_ne!(second, hash);
        assert_eq!(second, seen(&mut session, "a.dxf")[0].1);
    }

    #[test]
    fn failed_apply_keeps_document_and_hash_at_every_work_limit() {
        let mut failed = 0;
        let mut succeeded = false;
        for limit in 0..300 {
            let mut session = Session::new(b"").unwrap();
            session.push(b"a.dxf", EDITABLE).unwrap();
            call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
            let before = seen(&mut session, "a.dxf");
            let (_, summary) = call(&mut session, r#"{"v":1,"op":"drawing","doc":"a.dxf"}"#);
            let document = text(&summary, "/result/hash");
            let request = format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"transform","id":"{}","hash":"{}","matrix":[1,0,5,0,1,7]}}],"budget":{{"work":{limit}}}}}"#,
                before[0].0, before[0].1
            );
            let (status, _) = call(&mut session, &request);
            if status == Status::Ok {
                succeeded = true;
                break;
            }
            failed += 1;
            assert_eq!(seen(&mut session, "a.dxf"), before, "work={limit}");
            let (_, summary) = call(&mut session, r#"{"v":1,"op":"drawing","doc":"a.dxf"}"#);
            assert_eq!(text(&summary, "/result/hash"), document, "work={limit}");
        }
        assert!(failed > 0 && succeeded);
    }

    #[test]
    fn tables_keep_ids_unique_when_handles_repeat() {
        let source = TABLED
            .replace("5\n13\n2\nWall\n", "5\n10\n2\nWall\n")
            .replace("5\n12\n2\nDashed\n", "5\n11\n2\nDashed\n")
            .replace("105\n1D\n2\nStandard\n70\n0\n41\n2.5\n", "105\n1D\n2\nStandard\n70\n0\n41\n2.5\n0\nDIMSTYLE\n105\n1D\n2\nOther\n70\n0\n41\n3.0\n");
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", source.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let (status, tables) = call(&mut session, r#"{"v":1,"op":"tables","doc":"a.dxf"}"#);
        assert_eq!(status, Status::Ok, "{tables:?}");
        assert_eq!(text(&tables, "/result/layers/0/id"), "layer/h10");
        assert_eq!(text(&tables, "/result/layers/1/id"), "layer/h10/1");
        assert_eq!(text(&tables, "/result/line_types/0/id"), "line-type/h11");
        assert_eq!(text(&tables, "/result/line_types/1/id"), "line-type/h11/1");
        assert_eq!(
            text(&tables, "/result/dimension_styles/0/id"),
            "dimension-style/h1D"
        );
        assert_eq!(
            text(&tables, "/result/dimension_styles/1/id"),
            "dimension-style/h1D/1"
        );
    }

    #[test]
    fn rejects_table_ids_when_names_cannot_be_written_unambiguously() {
        let source = TABLED
            .replace("5\n13\n2\nWall\n", "5\n13\n2\n0\n")
            .replace("5\n12\n2\nDashed\n", "5\n12\n2\nCONTINUOUS\n");
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", source.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let before = seen(&mut session, "a.dxf");
        let (id, hash) = &before[0];
        for fields in [
            r#""layer_id":"layer/h13""#,
            r#""line_type":{"id":"line-type/h12"}"#,
        ] {
            let request = format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"restyle","id":"{id}","hash":"{hash}",{fields}}}]}}"#
            );
            let (status, response) = call(&mut session, &request);
            assert_eq!(status, Status::Failed, "{response:?}");
            assert_eq!(text(&error(&response), "/code"), "call.value.invalid");
            assert_eq!(seen(&mut session, "a.dxf"), before);
        }
    }

    /// 開いた文書の要素の形の値
    fn shapes(session: &mut Session, doc: &str) -> Vec<String> {
        let (_, response) = call(
            session,
            &format!(r#"{{"v":1,"op":"entities","doc":"{doc}","fields":["geometry"]}}"#),
        );
        let Some(Value::Array(items)) = response.pointer("/result/items") else {
            panic!("{response:?}");
        };
        items
            .iter()
            .map(|item| match item.pointer("/geometry") {
                Some(geometry) => {
                    let mut out = Writer::new();
                    out.value(geometry);
                    String::from_utf8(out.into_bytes()).unwrap()
                }
                None => String::new(),
            })
            .collect()
    }

    #[test]
    fn replaces_and_adds_shapes_by_their_geometry() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", EDITABLE).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        assert_eq!(
            shapes(&mut session, "a.dxf"),
            [
                r#"{"shape":"line","start":[0,0],"end":[10,0]}"#,
                r#"{"shape":"circle","center":[5,5],"radius":2}"#,
                r#"{"shape":"line","start":[0,1],"end":[10,1]}"#,
            ]
        );
        let before = seen(&mut session, "a.dxf");
        let request = format!(
            r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{}","hash":"{}","geometry":{{"shape":"line","start":[1,2],"end":[3,4]}}}},{{"kind":"replace","id":"{}","hash":"{}","geometry":{{"shape":"circle","center":[6,6],"radius":3}}}},{{"kind":"add","place":"space/0","geometry":{{"shape":"polyline","points":[[0,0],[5,0],[5,5]],"bulges":[0,0.5,0],"closed":true}}}},{{"kind":"add","place":"space/0","layer":"0","geometry":{{"shape":"arc","center":[1,1],"radius":2,"start_angle":0,"end_angle":90}}}}]}}"#,
            before[0].0, before[0].1, before[1].0, before[1].1
        );
        let (status, response) = call(&mut session, &request);
        assert_eq!(status, Status::Ok, "{response:?}");
        let file = session.take(0).unwrap();
        let after = seen(&mut session, "a.dxf");
        assert_eq!(after.len(), 5);
        // 置き換えた要素の ID は変わらず、足した要素の ID とハッシュは読み直したものと同じ
        for (index, at) in [(0, 0), (1, 1), (2, 3), (3, 4)] {
            assert_eq!(
                text(&response, &format!("/result/applied/{index}/id")),
                after[at].0
            );
            assert_eq!(
                text(&response, &format!("/result/applied/{index}/hash")),
                after[at].1
            );
        }
        assert_eq!(after[0].0, before[0].0);
        let expected = [
            r#"{"shape":"line","start":[1,2],"end":[3,4]}"#,
            r#"{"shape":"circle","center":[6,6],"radius":3}"#,
            r#"{"shape":"line","start":[0,1],"end":[10,1]}"#,
            r#"{"shape":"polyline","points":[[0,0],[5,0],[5,5]],"bulges":[0,0.5,0],"closed":true}"#,
            r#"{"shape":"arc","center":[1,1],"radius":2,"start_angle":0,"end_angle":90}"#,
        ];
        assert_eq!(shapes(&mut session, "a.dxf"), expected);
        session.push(b"b.dxf", &file).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"b.dxf"}"#);
        assert_eq!(shapes(&mut session, "b.dxf"), expected);
        assert_eq!(seen(&mut session, "b.dxf"), after);
    }

    #[test]
    fn adds_and_replaces_ellipses_by_their_geometry() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", EDITABLE).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let added = r#"{"shape":"ellipse","center":[2,3],"major":[6,3],"ratio":0.5,"start_angle":0,"end_angle":360}"#;
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"space/0","geometry":{added}}}]}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        let id = text(&response, "/result/applied/0/id");
        let hash = text(&response, "/result/applied/0/hash");
        assert_eq!(shapes(&mut session, "a.dxf")[3], added);
        let replaced = r#"{"shape":"ellipse","center":[2,3],"major":[2,7],"ratio":0.25,"start_angle":30,"end_angle":120}"#;
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{replaced}}}]}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(text(&response, "/result/applied/0/id"), id);
        assert_eq!(
            seen(&mut session, "a.dxf")[3].1,
            text(&response, "/result/applied/0/hash")
        );
        let found = shapes(&mut session, "a.dxf")[3].clone();
        let found = zumen_json::parse(found.as_bytes()).unwrap();
        for (pointer, expected) in [
            ("/center/1", 3.0),
            ("/major/1", 7.0),
            ("/ratio", 0.25),
            ("/start_angle", 30.0),
            ("/end_angle", 120.0),
        ] {
            let value: f64 = number(&found, pointer).parse().unwrap();
            assert!((value - expected).abs() < 1e-9, "{pointer}: {value}");
        }
        // 中心と同じ長軸の端と、1 を超える比は受けない
        for (geometry, pointer) in [
            (
                r#""center":[2,3],"major":[2,3],"ratio":0.5"#,
                "/edits/0/geometry/major",
            ),
            (
                r#""center":[2,3],"major":[4,3],"ratio":1.5"#,
                "/edits/0/geometry/ratio",
            ),
        ] {
            let (status, response) = call(
                &mut session,
                &format!(
                    r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"space/0","geometry":{{"shape":"ellipse",{geometry},"start_angle":0,"end_angle":360}}}}]}}"#
                ),
            );
            assert_eq!(status, Status::Failed);
            assert_eq!(text(&error(&response), "/params/pointer"), pointer);
        }
    }

    #[test]
    fn rejects_missing_text_style_without_a_style_table() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", EDITABLE).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let before = seen(&mut session, "a.dxf");
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"apply","doc":"a.dxf","edits":[{"kind":"add","place":"space/0","geometry":{"shape":"text","content":"A","position":[0,0],"height":2,"rotation":0,"style":"Missing"}}]}"#,
        );
        assert_eq!(status, Status::Failed, "{response:?}");
        assert_eq!(text(&error(&response), "/code"), "change.edit.unsupported");
        assert_eq!(seen(&mut session, "a.dxf"), before);
    }

    #[test]
    fn texts_refer_to_text_styles_by_their_ids() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", STYLED.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        // 名前は大文字と小文字を区別せずに引く。表にない名前も、定義のない項目の ID で指す
        let shapes_before = shapes(&mut session, "a.dxf");
        assert!(shapes_before[0].ends_with(r#""style":"Missing","style_id":"text-style/2"}"#));
        assert!(shapes_before[1].ends_with(r#""style":"GOTHIC","style_id":"text-style/h12"}"#));
        let add = |style: &str| {
            format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"space/0","geometry":{{"shape":"text","content":"C","position":[0,9],"height":2,"rotation":0{style}}}}}]}}"#
            )
        };
        // ID で指せば、名前で書く。名前も書くなら、同じ文字スタイルを指す
        for style in [
            r#","style_id":"text-style/h12""#,
            r#","style":"gothic","style_id":"text-style/h12""#,
        ] {
            let mut session = Session::new(b"").unwrap();
            session.push(b"a.dxf", STYLED.as_bytes()).unwrap();
            call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
            let (status, response) = call(&mut session, &add(style));
            assert_eq!(status, Status::Ok, "{response:?}");
            assert!(
                shapes(&mut session, "a.dxf")[2]
                    .ends_with(r#""style":"Gothic","style_id":"text-style/h12"}"#)
            );
        }
        for style in [
            r#","style_id":"text-style/h99""#,
            r#","style":"Standard","style_id":"text-style/h12""#,
        ] {
            let (status, response) = call(&mut session, &add(style));
            assert_eq!(status, Status::Failed);
            assert_eq!(
                text(&error(&response), "/params/pointer"),
                "/edits/0/geometry/style_id"
            );
        }
        // entities が返した形の値は、そのまま置き換えに戻せる。ほかの文字スタイルの ID には置き換えない
        let (id, hash) = seen(&mut session, "a.dxf")[1].clone();
        let replace = |geometry: &str| {
            format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{geometry}}}]}}"#
            )
        };
        let other = shapes_before[1].replace("text-style/h12", "text-style/h11");
        let other = other.replace(r#""style":"GOTHIC","#, "");
        let (status, response) = call(&mut session, &replace(&other));
        assert_eq!(status, Status::Failed);
        assert_eq!(
            text(&error(&response), "/params/pointer"),
            "/edits/0/geometry/style"
        );
        let (status, response) = call(&mut session, &replace(&shapes_before[1]));
        assert_eq!(status, Status::Ok, "{response:?}");
    }

    #[test]
    fn duplicated_text_style_names_cannot_be_referred_to_by_id() {
        let dxf = STYLED.replace("2\nGothic\n", "2\nstandard\n");
        let dxf = dxf.replace("7\nGOTHIC\n", "7\nSTANDARD\n");
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", dxf.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        // entities は、図面モデルと同じく先の項目の ID を返す
        assert!(shapes(&mut session, "a.dxf")[1].ends_with(r#""style_id":"text-style/h11"}"#));
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"apply","doc":"a.dxf","edits":[{"kind":"add","place":"space/0","geometry":{"shape":"text","content":"C","position":[0,9],"height":2,"rotation":0,"style_id":"text-style/h11"}}]}"#,
        );
        assert_eq!(status, Status::Failed, "{response:?}");
        assert_eq!(
            text(&error(&response), "/params/pointer"),
            "/edits/0/geometry/style_id"
        );
    }

    #[test]
    fn non_ascii_text_style_names_remain_distinct() {
        let dxf = STYLED
            .replace("AC1018", "AC1021")
            .replace("2\nStandard\n", "2\nÄ\n")
            .replace("2\nGothic\n", "2\nä\n")
            .replace("7\nGOTHIC\n", "7\nä\n");
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", dxf.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        assert!(
            shapes(&mut session, "a.dxf")[1]
                .ends_with(r#""style":"ä","style_id":"text-style/h12"}"#)
        );
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"apply","doc":"a.dxf","edits":[{"kind":"add","place":"space/0","geometry":{"shape":"text","content":"C","position":[0,9],"height":2,"rotation":0,"style_id":"text-style/h12"}}]}"#,
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        assert!(
            shapes(&mut session, "a.dxf")[2]
                .ends_with(r#""style":"ä","style_id":"text-style/h12"}"#)
        );
    }

    #[test]
    fn dimension_style_ids_follow_the_models_case_sensitive_lookup() {
        let dxf =
            b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
            0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nDIMSTYLE\n0\nDIMSTYLE\n105\n2A\n2\nSTYLE\n70\n0\n\
            0\nDIMSTYLE\n105\n2B\n2\nstyle\n70\n0\n0\nENDTAB\n0\nENDSEC\n\
            0\nSECTION\n2\nENTITIES\n0\nDIMENSION\n5\n40\n8\n0\n3\nstyle\n10\n0.0\n20\n100.0\n\
            70\n32\n13\n0.0\n23\n0.0\n14\n1000.0\n24\n0.0\n0\nENDSEC\n0\nEOF\n";
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", dxf).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let geometry = &shapes(&mut session, "a.dxf")[0];
        assert!(
            geometry.contains(r#""style":"style","style_id":"dimension-style/h2B""#),
            "{geometry}"
        );
    }

    /// 足した文字の形の値に、entities が書く文字スタイルの ID（表にない Standard）を足したもの
    fn with_style_id(geometry: &str) -> String {
        let open = geometry.strip_suffix('}').unwrap();
        format!(r#"{open},"style_id":"text-style/0"}}"#)
    }

    #[test]
    fn adds_text_with_a_defined_style() {
        let dxf = String::from_utf8(EDITABLE.to_vec()).unwrap().replace(
            "0\nSECTION\n2\nENTITIES\n",
            "0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nSTYLE\n0\nSTYLE\n2\nCustom\n0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n",
        );
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", dxf.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"apply","doc":"a.dxf","edits":[{"kind":"add","place":"space/0","geometry":{"shape":"text","content":"A","position":[0,0],"height":2,"rotation":0,"style":"custom"}}]}"#,
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        assert!(shapes(&mut session, "a.dxf")[3].contains(r#""style":"Custom""#));
    }

    #[test]
    fn adds_texts_and_keeps_their_style_when_replacing() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", EDITABLE).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let added = r#"{"shape":"text","content":"GRID X1","position":[2,3],"height":2.5,"rotation":30,"style":"Standard"}"#;
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"space/0","geometry":{added}}}]}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        let id = text(&response, "/result/applied/0/id");
        let hash = text(&response, "/result/applied/0/hash");
        assert_eq!(seen(&mut session, "a.dxf")[3], (id.clone(), hash.clone()));
        assert_eq!(shapes(&mut session, "a.dxf")[3], with_style_id(added));
        // 置き換えは文字スタイルを変えない
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{{"shape":"text","content":"X2","position":[2,3],"height":2.5,"rotation":0,"style":"Other"}}}}]}}"#
            ),
        );
        assert_eq!(status, Status::Failed);
        assert_eq!(
            text(&error(&response), "/params/pointer"),
            "/edits/0/geometry/style"
        );
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{{"shape":"text","content":"X2","position":[4,5],"height":3,"rotation":0}}}}]}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(
            shapes(&mut session, "a.dxf")[3],
            r#"{"shape":"text","content":"X2","position":[4,5],"height":3,"rotation":0,"style":"Standard","style_id":"text-style/0"}"#
        );
    }

    #[test]
    fn adds_and_replaces_texts_fitted_between_two_points() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", EDITABLE).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        // 2 点目と合わせ方は一緒に書き、回転は 1 点目から 2 点目への向きと同じにする
        for (geometry, pointer) in [
            (
                r#"{"shape":"text","content":"X1","position":[0,0],"end":[0,5],"height":1,"rotation":90}"#,
                "/edits/0/geometry/fit",
            ),
            (
                r#"{"shape":"text","content":"X1","position":[0,0],"fit":"fit","height":1,"rotation":90}"#,
                "/edits/0/geometry/end",
            ),
            (
                r#"{"shape":"text","content":"X1","position":[0,0],"end":[0,0],"fit":"fit","height":1,"rotation":0}"#,
                "/edits/0/geometry/end",
            ),
            (
                r#"{"shape":"text","content":"X1","position":[0,0],"end":[0,5],"fit":"fit","height":1,"rotation":0}"#,
                "/edits/0/geometry/rotation",
            ),
        ] {
            let (status, response) = call(
                &mut session,
                &format!(
                    r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"space/0","geometry":{geometry}}}]}}"#
                ),
            );
            assert_eq!(status, Status::Failed);
            assert_eq!(text(&error(&response), "/params/pointer"), pointer);
        }
        let added = r#"{"shape":"text","content":"X1","position":[0,0],"end":[0,5],"fit":"aligned","height":1,"rotation":90,"style":"Standard"}"#;
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"space/0","geometry":{added}}}]}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        let id = text(&response, "/result/applied/0/id");
        let hash = text(&response, "/result/applied/0/hash");
        assert_eq!(seen(&mut session, "a.dxf")[3], (id.clone(), hash.clone()));
        assert_eq!(shapes(&mut session, "a.dxf")[3], with_style_id(added));
        // 合わせ方は変えず、1 点に置く文字にも替えない
        for (geometry, pointer) in [
            (
                r#"{"shape":"text","content":"X2","position":[1,1],"end":[1,6],"fit":"fit","height":1,"rotation":90}"#,
                "/edits/0/geometry/fit",
            ),
            (
                r#"{"shape":"text","content":"X2","position":[1,1],"height":1,"rotation":90}"#,
                "/edits/0/geometry/end",
            ),
        ] {
            let (status, response) = call(
                &mut session,
                &format!(
                    r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{geometry}}}]}}"#
                ),
            );
            assert_eq!(status, Status::Failed);
            assert_eq!(text(&error(&response), "/params/pointer"), pointer);
        }
        let replaced = r#"{"shape":"text","content":"X2","position":[1,1],"end":[-5,1],"fit":"aligned","height":2,"rotation":180,"style":"Standard","style_id":"text-style/0"}"#;
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{replaced}}}]}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(shapes(&mut session, "a.dxf")[3], replaced);
        let (id, hash) = seen(&mut session, "a.dxf")[3].clone();
        assert_eq!(id, text(&response, "/result/applied/0/id"));
        assert_eq!(hash, text(&response, "/result/applied/0/hash"));
        // 1 点に置く文字は、2 点の間に合わせる文字に替えない
        let plain = r#"{"shape":"text","content":"X3","position":[0,0],"height":1,"rotation":0}"#;
        let (_, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"space/0","geometry":{plain}}}]}}"#
            ),
        );
        let (id, hash) = (
            text(&response, "/result/applied/0/id"),
            text(&response, "/result/applied/0/hash"),
        );
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{added}}}]}}"#
            ),
        );
        assert_eq!(status, Status::Failed);
        assert_eq!(
            text(&error(&response), "/params/pointer"),
            "/edits/0/geometry/end"
        );
    }

    #[test]
    fn adds_and_replaces_formatted_texts_by_their_geometry() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", EDITABLE).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let added = r#"{"shape":"formatted_text","content":"{\\H2x;GRID}\\PX1","position":[2,3],"height":2.5,"rotation":0,"attachment":"top_left","style":"Standard"}"#;
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"space/0","geometry":{added}}}]}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        let id = text(&response, "/result/applied/0/id");
        let hash = text(&response, "/result/applied/0/hash");
        assert_eq!(seen(&mut session, "a.dxf")[3], (id.clone(), hash.clone()));
        assert_eq!(shapes(&mut session, "a.dxf")[3], with_style_id(added));
        // 生の改行と、違う文字スタイルは受けない
        for (geometry, pointer) in [
            (
                r#"{"shape":"formatted_text","content":"A\nB","position":[2,3],"height":2.5,"rotation":0}"#,
                "/edits/0/geometry/content",
            ),
            (
                r#"{"shape":"formatted_text","content":"A","position":[2,3],"height":2.5,"rotation":0,"style":"Other"}"#,
                "/edits/0/geometry/style",
            ),
            (
                r#"{"shape":"formatted_text","content":"A","position":[2,3],"height":2.5,"rotation":0,"width":0}"#,
                "/edits/0/geometry/width",
            ),
        ] {
            let (status, response) = call(
                &mut session,
                &format!(
                    r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{geometry}}}]}}"#
                ),
            );
            assert_eq!(status, Status::Failed);
            assert_eq!(text(&error(&response), "/params/pointer"), pointer);
        }
        // 位置合わせを書かなければ元のまま。枠の幅を足す
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{{"shape":"formatted_text","content":"X2","position":[4,5],"height":3,"rotation":0,"width":20}}}}]}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(
            shapes(&mut session, "a.dxf")[3],
            r#"{"shape":"formatted_text","content":"X2","position":[4,5],"height":3,"rotation":0,"attachment":"top_left","width":20,"style":"Standard","style_id":"text-style/0"}"#
        );
        let hash = text(&response, "/result/applied/0/hash");
        let id = text(&response, "/result/applied/0/id");
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{{"shape":"formatted_text","content":"X2","position":[4,5],"height":3,"rotation":0,"attachment":"middle_center"}}}}]}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(
            shapes(&mut session, "a.dxf")[3],
            r#"{"shape":"formatted_text","content":"X2","position":[4,5],"height":3,"rotation":0,"attachment":"middle_center","style":"Standard","style_id":"text-style/0"}"#
        );
    }

    #[test]
    fn symbol_replace_does_not_move_unreadable_attributes() {
        // タグのない ATTRIB はモデルへ移せないが、元のファイルには残る。
        // 続く印と所有者のどちらで結び付いた場合も INSERT だけを動かさない。
        for (flag, owner) in [("66\n1\n", ""), ("", "330\n40\n")] {
            let file = format!(
                "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
                 0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n5\n20\n8\n0\n2\nDOOR\n70\n0\n10\n0\n20\n0\n3\nDOOR\n\
                 0\nENDBLK\n5\n22\n8\n0\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n\
                 0\nINSERT\n5\n40\n8\n0\n2\nDOOR\n{flag}10\n5\n20\n5\n\
                 0\nATTRIB\n5\n41\n{owner}8\n0\n10\n5\n20\n5\n40\n1\n1\nvalue\n\
                 0\nSEQEND\n5\n42\n0\nENDSEC\n0\nEOF\n"
            );
            let mut session = Session::new(b"").unwrap();
            session.push(b"a.dxf", file.as_bytes()).unwrap();
            call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
            let (id, hash) = seen(&mut session, "a.dxf")
                .into_iter()
                .find(|(id, _)| id == "space/0/h40")
                .unwrap();
            let before = shapes(&mut session, "a.dxf");
            for edit in [
                format!(
                    r#"{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{{"shape":"symbol","name":"DOOR","position":[7,8],"scale":[1,1],"rotation":0}}}}"#
                ),
                format!(
                    r#"{{"kind":"transform","id":"{id}","hash":"{hash}","matrix":[1,0,0,1,2,3]}}"#
                ),
            ] {
                let (status, response) = call(
                    &mut session,
                    &format!(r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{edit}]}}"#),
                );
                assert_eq!(status, Status::Failed, "{response:?}");
                assert_eq!(text(&error(&response), "/code"), "change.edit.unsupported");
                assert_eq!(shapes(&mut session, "a.dxf"), before);
            }
        }
    }

    #[test]
    fn adds_and_replaces_symbols_by_their_geometry() {
        let file =
            b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
            0\nSECTION\n2\nBLOCKS\n\
            0\nBLOCK\n5\n20\n8\n0\n2\nDOOR\n70\n0\n10\n0\n20\n0\n3\nDOOR\n\
            0\nLINE\n5\n21\n8\n0\n10\n0\n20\n0\n11\n1\n21\n0\n\
            0\nENDBLK\n5\n22\n8\n0\n0\nENDSEC\n\
            0\nSECTION\n2\nENTITIES\n\
            0\nINSERT\n5\n40\n8\n0\n2\nDOOR\n10\n5.0\n20\n5.0\n0\nENDSEC\n0\nEOF\n";
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", file).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let symbols = |session: &mut Session| {
            shapes(session, "a.dxf")
                .into_iter()
                .filter(|shape| shape.contains(r#""shape":"symbol""#))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            symbols(&mut session),
            [r#"{"shape":"symbol","name":"DOOR","position":[5,5],"scale":[1,1],"rotation":0}"#]
        );
        let (id, hash) = seen(&mut session, "a.dxf")
            .into_iter()
            .find(|(id, _)| id == "space/0/h40")
            .unwrap();
        let apply = |session: &mut Session, edit: &str| {
            call(
                session,
                &format!(r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{edit}]}}"#),
            )
        };
        // 定義と格子の列と行の数は変えない。尺度は 0 でない
        for (geometry, pointer) in [
            (
                r#"{"shape":"symbol","name":"WINDOW","position":[5,5],"scale":[1,1],"rotation":0}"#,
                "/edits/0/geometry/name",
            ),
            (
                r#"{"shape":"symbol","name":"DOOR","position":[5,5],"scale":[1,1],"rotation":0,"grid":{"columns":2,"rows":1,"column_spacing":3,"row_spacing":0}}"#,
                "/edits/0/geometry/grid",
            ),
            (
                r#"{"shape":"symbol","name":"DOOR","position":[5,5],"scale":[0,1],"rotation":0}"#,
                "/edits/0/geometry/scale",
            ),
        ] {
            let (status, response) = apply(
                &mut session,
                &format!(
                    r#"{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{geometry}}}"#
                ),
            );
            assert_eq!(status, Status::Failed);
            assert_eq!(text(&error(&response), "/params/pointer"), pointer);
        }
        let replaced =
            r#"{"shape":"symbol","name":"DOOR","position":[7,8],"scale":[2,-1],"rotation":30}"#;
        let (status, response) = apply(
            &mut session,
            &format!(r#"{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{replaced}}}"#),
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(symbols(&mut session), [replaced]);
        // 格子に並べた記号を足す。1×1 の格子は持たない
        let (status, response) = apply(
            &mut session,
            r#"{"kind":"add","place":"space/0","geometry":{"shape":"symbol","name":"DOOR","position":[0,0],"scale":[1,1],"rotation":0,"grid":{"columns":1,"rows":1,"column_spacing":0,"row_spacing":0}}}"#,
        );
        assert_eq!(status, Status::Failed);
        assert_eq!(
            text(&error(&response), "/params/pointer"),
            "/edits/0/geometry/grid"
        );
        let added = r#"{"shape":"symbol","name":"DOOR","position":[1,2],"scale":[1,1],"rotation":90,"grid":{"columns":2,"rows":3,"column_spacing":4,"row_spacing":-5}}"#;
        let (status, response) = apply(
            &mut session,
            &format!(r#"{{"kind":"add","place":"space/0","geometry":{added}}}"#),
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        let id = text(&response, "/result/applied/0/id");
        let hash = text(&response, "/result/applied/0/hash");
        assert!(seen(&mut session, "a.dxf").contains(&(id, hash)));
        assert_eq!(symbols(&mut session), [replaced, added]);
        // 図面にない定義は足せない
        let (status, response) = apply(
            &mut session,
            r#"{"kind":"add","place":"space/0","geometry":{"shape":"symbol","name":"WINDOW","position":[0,0],"scale":[1,1],"rotation":0}}"#,
        );
        assert_eq!(status, Status::Failed);
        assert_eq!(text(&error(&response), "/code"), "change.edit.unsupported");
    }

    #[test]
    fn adds_and_replaces_splines_by_their_geometry() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", EDITABLE).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let added = r#"{"shape":"spline","degree":3,"knots":[0,0,0,0,1,2,2,2,2],"control":[[0,0],[1,2],[3,3],[5,1],[6,0]],"closed":false,"periodic":false}"#;
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"space/0","geometry":{added}}}]}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        let id = text(&response, "/result/applied/0/id");
        let hash = text(&response, "/result/applied/0/hash");
        assert_eq!(seen(&mut session, "a.dxf")[3], (id.clone(), hash.clone()));
        assert_eq!(shapes(&mut session, "a.dxf")[3], added);
        // 次数・制御点・節点・重みの数が合わない値は受けない
        for (geometry, pointer) in [
            (
                r#"{"shape":"spline","degree":0,"knots":[0,1],"control":[[0,0],[1,1]],"closed":false,"periodic":false}"#,
                "/edits/0/geometry/degree",
            ),
            (
                r#"{"shape":"spline","degree":3,"knots":[0,0,0,1,1,1],"control":[[0,0],[1,1],[2,0]],"closed":false,"periodic":false}"#,
                "/edits/0/geometry/control",
            ),
            (
                r#"{"shape":"spline","degree":2,"knots":[0,0,1,1,1],"control":[[0,0],[1,1],[2,0]],"closed":false,"periodic":false}"#,
                "/edits/0/geometry/knots",
            ),
            (
                r#"{"shape":"spline","degree":2,"knots":[0,0,0,1,1,1],"control":[[0,0],[1,1],[2,0]],"weights":[1,0],"closed":false,"periodic":false}"#,
                "/edits/0/geometry/weights",
            ),
        ] {
            let (status, response) = call(
                &mut session,
                &format!(
                    r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{geometry}}}]}}"#
                ),
            );
            assert_eq!(status, Status::Failed);
            assert_eq!(text(&error(&response), "/params/pointer"), pointer);
        }
        // 有理の 2 次のスプライン（円弧）に置き換える
        let replaced = r#"{"shape":"spline","degree":2,"knots":[0,0,0,1,1,1],"control":[[10,0],[10,10],[0,10]],"weights":[1,0.5,1],"closed":false,"periodic":false}"#;
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{replaced}}}]}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(shapes(&mut session, "a.dxf")[3], replaced);
        let (id, hash) = seen(&mut session, "a.dxf")[3].clone();
        assert_eq!(id, text(&response, "/result/applied/0/id"));
        assert_eq!(hash, text(&response, "/result/applied/0/hash"));
    }

    #[test]
    fn counts_symbol_attribute_values_before_building_changes() {
        let value = "A".repeat(1000);
        let json = format!(
            r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"space/0","geometry":{{"shape":"symbol","name":"DOOR","position":[0,0],"scale":[1,1],"rotation":0,"attributes":[{{"tag":"NO","value":"{value}"}}]}}}}]}}"#
        );
        let value = zumen_json::parse(json.as_bytes()).unwrap();
        let Request::Apply(request) = Request::from_json(&value).unwrap() else {
            panic!()
        };
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(100)));
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        assert!(changes(&request, &Drawing2D::default(), &mut cx).is_err());
        assert_eq!(
            diagnostics.iter().next().unwrap().code(),
            codes::BUDGET_WORK_EXCEEDED
        );
    }

    /// 寸法スタイル Standard と、注記の MTEXT と、それを指す引出線を持つ R2000 の DXF。ブロック B と名前のない
    /// ブロック *U1 は、ブロックのレコードを持つ
    const LEADERS: &[u8] = b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$INSUNITS\n70\n4\n\
        9\n$HANDSEED\n5\n100\n0\nENDSEC\n\
        0\nSECTION\n2\nTABLES\n\
        0\nTABLE\n2\nLAYER\n0\nLAYER\n5\n10\n2\n0\n70\n0\n62\n7\n6\nCONTINUOUS\n0\nENDTAB\n\
        0\nTABLE\n2\nDIMSTYLE\n0\nDIMSTYLE\n105\n27\n2\nStandard\n70\n0\n0\nENDTAB\n\
        0\nTABLE\n2\nBLOCK_RECORD\n0\nBLOCK_RECORD\n5\n1F\n2\n*Model_Space\n\
        0\nBLOCK_RECORD\n5\n20\n2\nB\n0\nBLOCK_RECORD\n5\n21\n2\n*U1\n0\nENDTAB\n0\nENDSEC\n\
        0\nSECTION\n2\nBLOCKS\n\
        0\nBLOCK\n5\n30\n330\n20\n8\n0\n2\nB\n70\n0\n10\n0.0\n20\n0.0\n30\n0.0\n\
        0\nLINE\n5\n31\n330\n20\n8\n0\n10\n0.0\n20\n0.0\n30\n0.0\n11\n1.0\n21\n0.0\n31\n0.0\n\
        0\nENDBLK\n5\n32\n330\n20\n8\n0\n\
        0\nBLOCK\n5\n33\n330\n21\n8\n0\n2\n*U1\n70\n1\n10\n0.0\n20\n0.0\n30\n0.0\n\
        0\nENDBLK\n5\n34\n330\n21\n8\n0\n0\nENDSEC\n\
        0\nSECTION\n2\nENTITIES\n\
        0\nMTEXT\n5\n40\n330\n1F\n8\n0\n10\n10.0\n20\n5.0\n30\n0.0\n40\n2.5\n1\nNOTE\n\
        0\nLEADER\n5\n41\n330\n1F\n8\n0\n3\nStandard\n71\n1\n72\n0\n73\n0\n74\n1\n75\n0\n\
        40\n2.5\n41\n10.0\n76\n2\n10\n0.0\n20\n0.0\n30\n0.0\n10\n10.0\n20\n5.0\n30\n0.0\n340\n40\n\
        0\nENDSEC\n0\nEOF\n";

    #[test]
    fn replaces_and_adds_leaders_by_their_geometry() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", LEADERS).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let leader = r#"{"shape":"leader","path":[[0,0],[10,5]],"spline":false,"arrow":true,"annotation":"space/0/h40","style":"Standard","style_id":"dimension-style/h27"}"#;
        let before = shapes(&mut session, "a.dxf");
        assert_eq!(before.len(), 2);
        assert_eq!(before[1], leader);
        let (id, hash) = seen(&mut session, "a.dxf").remove(1);
        let replace = |geometry: &str| {
            format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{geometry}}}]}}"#
            )
        };
        let add = |geometry: &str| {
            format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"space/0","geometry":{geometry}}}]}}"#
            )
        };
        // 注記と寸法スタイルは変えず、足す引出線の注記は図面にある要素を指す。合わない欄を指して失敗し、何も
        // 変えない
        for (request, pointer) in [
            (
                replace(
                    r#"{"shape":"leader","path":[[0,0],[10,5]],"spline":false,"style":"Standard"}"#,
                ),
                "/edits/0/geometry/annotation",
            ),
            (
                replace(
                    r#"{"shape":"leader","path":[[0,0],[10,5]],"spline":false,"annotation":"space/0/h41","style":"Standard"}"#,
                ),
                "/edits/0/geometry/annotation",
            ),
            (
                replace(
                    r#"{"shape":"leader","path":[[0,0],[10,5]],"spline":false,"annotation":"space/0/h40","style":"Other"}"#,
                ),
                "/edits/0/geometry/style",
            ),
            (
                replace(
                    r#"{"shape":"leader","path":[[0,0]],"spline":false,"annotation":"space/0/h40","style":"Standard"}"#,
                ),
                "/edits/0/geometry/path",
            ),
            (
                add(
                    r#"{"shape":"leader","path":[[0,0],[1,1]],"spline":false,"annotation":"space/0/h99","style":"Standard"}"#,
                ),
                "/edits/0/geometry/annotation",
            ),
            (
                add(
                    r#"{"shape":"leader","path":[[0,0],[1,1]],"spline":false,"style":"Other","style_id":"dimension-style/h27"}"#,
                ),
                "/edits/0/geometry/style_id",
            ),
        ] {
            let (status, response) = call(&mut session, &request);
            assert_eq!(status, Status::Failed, "{request}");
            assert_eq!(text(&error(&response), "/params/pointer"), pointer);
        }
        // 図面にない寸法スタイルの引出線は足さない
        let (status, response) = call(
            &mut session,
            &add(r#"{"shape":"leader","path":[[0,0],[1,1]],"spline":false,"style":"Other"}"#),
        );
        assert_eq!(status, Status::Failed);
        assert_eq!(text(&error(&response), "/code"), "change.edit.unsupported");
        assert_eq!(shapes(&mut session, "a.dxf"), before);

        // 経路・曲線か・矢印を置き換え、注記のない引出線を足す
        let replaced = r#"{"shape":"leader","path":[[1,1],[10,5],[20,5]],"spline":true,"arrow":false,"annotation":"space/0/h40","style":"Standard","style_id":"dimension-style/h27"}"#;
        let added = r#"{"shape":"leader","path":[[0,10],[5,15]],"spline":false,"style":"Standard","style_id":"dimension-style/h27"}"#;
        let request = format!(
            r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{replaced}}},{{"kind":"add","place":"space/0","geometry":{added}}}]}}"#
        );
        let (status, response) = call(&mut session, &request);
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(shapes(&mut session, "a.dxf"), [&before[0], replaced, added]);
        let after = seen(&mut session, "a.dxf");
        assert_eq!(text(&response, "/result/applied/0/id"), id);
        assert_eq!(text(&response, "/result/applied/1/id"), after[2].0);
        assert_eq!(text(&response, "/result/applied/1/hash"), after[2].1);
        // 矢印を寸法スタイルに従わせると、71 を除く
        let (id, hash) = after[1].clone();
        let request = format!(
            r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{}}}]}}"#,
            replaced.replace(r#""arrow":false,"#, "")
        );
        let (status, response) = call(&mut session, &request);
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(
            shapes(&mut session, "a.dxf")[1],
            replaced.replace(r#""arrow":false,"#, "")
        );
    }

    /// 注記を持つ引出線を足し、引出線を変換で動かす（決定 0185）。注記にできるのは同じ空間の MTEXT だけで、
    /// ほかの要素を指す追加は断る
    #[test]
    fn adds_annotated_leaders_and_transforms_leaders() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", LEADERS).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let before = shapes(&mut session, "a.dxf");
        let add = |annotation: &str| {
            format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"space/0","geometry":{{"shape":"leader","path":[[0,10],[10,5]],"spline":false,"arrow":true,"annotation":"{annotation}","style":"Standard"}}}}]}}"#
            )
        };
        // 引出線を注記にはできない
        let (status, response) = call(&mut session, &add("space/0/h41"));
        assert_eq!(status, Status::Failed);
        assert_eq!(text(&error(&response), "/code"), "change.edit.unsupported");
        assert_eq!(shapes(&mut session, "a.dxf"), before);

        let (status, response) = call(&mut session, &add("space/0/h40"));
        assert_eq!(status, Status::Ok, "{response:?}");
        let after = shapes(&mut session, "a.dxf");
        assert_eq!(after.len(), 3);
        assert_eq!(
            after[2],
            r#"{"shape":"leader","path":[[0,10],[10,5]],"spline":false,"arrow":true,"annotation":"space/0/h40","style":"Standard","style_id":"dimension-style/h27"}"#
        );

        // 変換は経路の頂点を写し、注記を保つ
        let (id, hash) = seen(&mut session, "a.dxf")[1].clone();
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"transform","id":"{id}","hash":"{hash}","matrix":[0,-1,1,1,0,2]}}]}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(
            shapes(&mut session, "a.dxf")[1],
            r#"{"shape":"leader","path":[[1,2],[-4,12]],"spline":false,"arrow":true,"annotation":"space/0/h40","style":"Standard","style_id":"dimension-style/h27"}"#
        );
    }

    /// 注記を持たない MULTILEADER を 1 つ持つ R2013 の DXF。枝は 1 つで、集まる点と右への折れ曲がりと線 1 本を持つ
    const CALLOUTS: &[u8] = b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1027\n9\n$INSUNITS\n70\n4\n\
        9\n$HANDSEED\n5\n100\n0\nENDSEC\n\
        0\nSECTION\n2\nENTITIES\n\
        0\nMULTILEADER\n5\n40\n8\n0\n300\nCONTEXT_DATA{\n40\n1.0\n140\n2.0\n290\n0\n296\n0\n\
        302\nLEADER{\n290\n1\n291\n1\n10\n30.0\n20\n20.0\n30\n0.0\n11\n1.0\n21\n0.0\n31\n0.0\n90\n0\n40\n5.0\n\
        304\nLEADER_LINE{\n10\n0.0\n20\n0.0\n30\n0.0\n91\n0\n305\n}\n303\n}\n301\n}\n\
        170\n1\n42\n2.5\n290\n1\n\
        0\nENDSEC\n0\nEOF\n";

    /// 引出線つきの注記は、枝の線・集まる点・折れ曲がりだけを置き換え、足すことは断る（決定 0185）
    #[test]
    fn replaces_callout_branches_by_their_geometry() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", CALLOUTS).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let callout = |branches: &str, landing: bool| {
            format!(
                r#"{{"shape":"callout","branches":{branches},"path":"straight","landing":{landing},"arrow_size":2.5,"scale":1,"content":"none"}}"#
            )
        };
        let before = shapes(&mut session, "a.dxf");
        assert_eq!(
            before,
            [callout(
                r#"[{"lines":[[[0,0]]],"end":[30,20],"dogleg":{"direction":[1,0],"length":5}}]"#,
                true
            )]
        );
        let (id, hash) = seen(&mut session, "a.dxf").remove(0);
        let replace = |geometry: &str| {
            format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{geometry}}}]}}"#
            )
        };
        let add = |geometry: &str| {
            format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"space/0","geometry":{geometry}}}]}}"#
            )
        };
        let one =
            r#"{"lines":[[[0,0],[5,5]]],"end":[30,20],"dogleg":{"direction":[-1,0],"length":4}}"#;
        let branch = &format!("[{one}]");
        // 枝のほかの欄を変えたり、合わない枝を書いたりすると、その欄を指して失敗する
        for (request, pointer) in [
            (
                replace(&callout(branch, false)),
                "/edits/0/geometry/landing",
            ),
            (
                replace(&callout(
                    r#"[{"lines":[[[0,0]]],"end":[30,20],"dogleg":{"direction":[0,0],"length":4}}]"#,
                    true,
                )),
                "/edits/0/geometry/branches/0/dogleg/direction",
            ),
            (
                replace(&callout(r#"[{"lines":[[]]}]"#, true)),
                "/edits/0/geometry/branches/0/lines/0",
            ),
            (
                add(&callout(branch, true).replace(r#""none""#, r#""text","text":"A""#)),
                "/edits/0/geometry/content",
            ),
        ] {
            let (status, response) = call(&mut session, &request);
            assert_eq!(status, Status::Failed, "{request}");
            assert_eq!(
                text(&error(&response), "/params/pointer"),
                pointer,
                "{request}"
            );
        }
        // 枝の数と、集まる点の有無は変えられず、MULTILEADER はまだ足さない
        for request in [
            replace(&callout(&format!("[{one},{one}]"), true)),
            replace(&callout(
                r#"[{"lines":[[[0,0]]],"dogleg":{"direction":[1,0],"length":5}}]"#,
                true,
            )),
            add(&callout(branch, true)),
        ] {
            let (status, response) = call(&mut session, &request);
            assert_eq!(status, Status::Failed, "{request}");
            assert_eq!(text(&error(&response), "/code"), "change.edit.unsupported");
        }
        assert_eq!(shapes(&mut session, "a.dxf"), before);

        let (status, response) = call(&mut session, &replace(&callout(branch, true)));
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(shapes(&mut session, "a.dxf"), [callout(branch, true)]);
    }

    #[test]
    fn leaders_are_not_added_to_r12_dxf() {
        let mut session = Session::new(b"").unwrap();
        let r12 = std::str::from_utf8(EDITABLE)
            .unwrap()
            .replace("AC1015", "AC1009");
        session.push(b"a.dxf", r12.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let before = shapes(&mut session, "a.dxf");
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"apply","doc":"a.dxf","edits":[{"kind":"add","place":"space/0","geometry":{"shape":"leader","path":[[0,0],[1,1]],"spline":false,"style":"Standard"}}]}"#,
        );
        assert_eq!(status, Status::Failed);
        assert_eq!(text(&error(&response), "/code"), "change.edit.unsupported");
        assert_eq!(shapes(&mut session, "a.dxf"), before);
    }

    #[test]
    fn adds_elements_to_symbol_definitions() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", LEADERS).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let defined = |session: &mut Session| {
            let (_, response) = call(
                session,
                r#"{"v":1,"op":"entities","doc":"a.dxf","fields":["hash","geometry"],"place":"space/h30"}"#,
            );
            let Some(Value::Array(items)) = response.pointer("/result/items") else {
                panic!("{response:?}");
            };
            items
                .iter()
                .map(|item| {
                    let mut out = Writer::new();
                    out.value(item);
                    String::from_utf8(out.into_bytes()).unwrap()
                })
                .collect::<Vec<_>>()
        };
        let line = r#"{"shape":"line","start":[0,0],"end":[1,0]}"#;
        let before = defined(&mut session);
        assert_eq!(before.len(), 1);
        assert!(
            before[0].starts_with(r#"{"id":"space/h30/h31""#),
            "{before:?}"
        );
        // 場所を指さなければ、定義の中の要素は返さない
        assert_eq!(shapes(&mut session, "a.dxf").len(), 2);
        let add = |place: &str, geometry: &str| {
            format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"{place}","geometry":{geometry}}}]}}"#
            )
        };
        // 名前のない定義に足すことと、定義に自分自身を足すこと（#333）は断り、何も変えない
        for (request, code) in [
            (add("space/h33", line), "change.edit.unsupported"),
            (
                add(
                    "space/h30",
                    r#"{"shape":"symbol","name":"B","position":[0,0],"scale":[1,1],"rotation":0}"#,
                ),
                "change.symbol-cycle.invalid",
            ),
        ] {
            let (status, response) = call(&mut session, &request);
            assert_eq!(status, Status::Failed, "{request}");
            assert_eq!(text(&error(&response), "/code"), code);
        }
        assert_eq!(defined(&mut session), before);
        // 線分と引出線を定義に足す。読み直すと定義の要素の終わりに並び、apply が返す ID とハッシュで引ける
        let leader = r#"{"shape":"leader","path":[[0,0],[2,2]],"spline":false,"style":"Standard"}"#;
        let request = format!(
            r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"space/h30","geometry":{{"shape":"circle","center":[0,0],"radius":2}}}},{{"kind":"add","place":"space/h30","geometry":{leader}}}]}}"#
        );
        let (status, response) = call(&mut session, &request);
        assert_eq!(status, Status::Ok, "{response:?}");
        let after = defined(&mut session);
        assert_eq!(after.len(), 3);
        assert_eq!(after[0], before[0]);
        for (index, item) in [(0, &after[1]), (1, &after[2])] {
            let id = text(&response, &format!("/result/applied/{index}/id"));
            let hash = text(&response, &format!("/result/applied/{index}/hash"));
            assert!(id.starts_with("space/h30/h"), "{id}");
            assert!(
                item.starts_with(&format!(r#"{{"id":"{id}","hash":"{hash}","#)),
                "{item}"
            );
        }
        assert!(after[1].ends_with(r#""geometry":{"shape":"circle","center":[0,0],"radius":2}}"#));
        assert!(after[2].contains(r#""path":[[0,0],[2,2]]"#), "{}", after[2]);
        // 実寸の空間の要素は変わらない
        assert_eq!(shapes(&mut session, "a.dxf").len(), 2);
    }

    #[test]
    fn returns_dimensions_but_does_not_write_them() {
        // 1000 離れた 2 点の線形寸法。測定値は 1000 で、文字を「995」に上書きし、利用者が動かした（128）
        const DIMENSIONED: &[u8] =
            b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
            0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nDIMSTYLE\n0\nDIMSTYLE\n105\n2A\n2\nISO-25\n70\n0\n0\nENDTAB\n0\nENDSEC\n\
            0\nSECTION\n2\nENTITIES\n\
            0\nDIMENSION\n5\n40\n8\n0\n3\nISO-25\n10\n0.0\n20\n100.0\n11\n520.0\n21\n110.0\n\
            70\n160\n71\n5\n42\n1000.0\n1\n995\n13\n0.0\n23\n0.0\n14\n1000.0\n24\n0.0\n\
            0\nENDSEC\n0\nEOF\n";
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", DIMENSIONED).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let geometry = r#"{"shape":"dimension","kind":"linear","first":[0,0],"second":[1000,0],"line":[0,100],"rotation":0,"oblique":0,"label":"override","content":"995","measurement":1000,"measurement_key":"dxf.DIMENSION.42","text_position":[520,110],"style":"ISO-25","style_id":"dimension-style/h2A"}"#;
        // 寸法スタイルも、名前で引いた ID を返す
        assert_eq!(shapes(&mut session, "a.dxf"), [geometry]);
        let (id, hash) = seen(&mut session, "a.dxf").remove(0);
        // 種類が持たない欄・欠けた欄・合わない文字と測定値は、その欄を指す
        for (geometry, pointer) in [
            (
                r#"{"shape":"dimension","kind":"linear","first":[0,0],"second":[1,0],"line":[0,1],"rotation":0,"oblique":0,"vertex":[0,0],"label":"measurement","style":"ISO-25"}"#,
                "/edits/0/geometry/vertex",
            ),
            (
                r#"{"shape":"dimension","kind":"radius","center":[0,0],"chord":[1,0],"label":"measurement","style":"ISO-25"}"#,
                "/edits/0/geometry/leader",
            ),
            (
                r#"{"shape":"dimension","kind":"aligned","first":[0,0],"second":[1,0],"line":[0,1],"oblique":0,"label":"override","style":"ISO-25"}"#,
                "/edits/0/geometry/content",
            ),
            (
                r#"{"shape":"dimension","kind":"ordinate","origin":[0,0],"feature":[1,0],"leader_end":[1,1],"axis":"x","label":"hidden","measurement":1,"measurement_key":"dxf.DIMENSION.42","style":"ISO-25"}"#,
                "/edits/0/geometry/measurement",
            ),
        ] {
            let (status, response) = call(
                &mut session,
                &format!(
                    r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"space/0","geometry":{geometry}}}]}}"#
                ),
            );
            assert_eq!(status, Status::Failed, "{geometry}");
            assert_eq!(text(&error(&response), "/params/pointer"), pointer);
        }
        // 合う値でも、寸法を描いた部品を作り直せないので、置き換えも追加も書き戻さない
        let moved = geometry.replace(r#""line":[0,100]"#, r#""line":[0,150]"#);
        for edit in [
            format!(r#"{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{moved}}}"#),
            format!(
                r#"{{"kind":"add","place":"space/0","geometry":{}}}"#,
                moved
                    .replace(
                        r#","measurement":1000,"measurement_key":"dxf.DIMENSION.42""#,
                        ""
                    )
                    .replace(
                        r#""label":"override","content":"995""#,
                        r#""label":"measurement""#
                    )
            ),
        ] {
            let (status, response) = call(
                &mut session,
                &format!(r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{edit}]}}"#),
            );
            assert_eq!(status, Status::Failed, "{edit}");
            assert_eq!(
                text(&error(&response), "/code"),
                "change.edit.unsupported",
                "{response:?}"
            );
        }
        assert_eq!(shapes(&mut session, "a.dxf"), [geometry]);
    }

    #[test]
    fn adds_and_replaces_splines_fitted_through_points() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", EDITABLE).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        // 当てはめる点で決まるものは、節点・制御点・重みを持たず、当てはめる点を 2 つ以上と、0 でない接線を持つ
        for (geometry, pointer) in [
            (
                r#"{"shape":"spline","degree":3,"knots":[0,1],"control":[],"fit":[[0,0],[1,1]],"closed":false,"periodic":false}"#,
                "/edits/0/geometry/knots",
            ),
            (
                r#"{"shape":"spline","degree":3,"knots":[],"control":[[0,0]],"fit":[[0,0],[1,1]],"closed":false,"periodic":false}"#,
                "/edits/0/geometry/control",
            ),
            (
                r#"{"shape":"spline","degree":3,"knots":[],"control":[],"weights":[],"fit":[[0,0],[1,1]],"closed":false,"periodic":false}"#,
                "/edits/0/geometry/weights",
            ),
            (
                r#"{"shape":"spline","degree":3,"knots":[],"control":[],"fit":[[0,0]],"closed":false,"periodic":false}"#,
                "/edits/0/geometry/fit",
            ),
            (
                r#"{"shape":"spline","degree":3,"knots":[],"control":[],"fit":[[0,0],[1,1]],"start_tangent":[0,0],"closed":false,"periodic":false}"#,
                "/edits/0/geometry/start_tangent",
            ),
            (
                r#"{"shape":"spline","degree":1,"knots":[0,0,1,1],"control":[[0,0],[1,1]],"end_tangent":[1,0],"closed":false,"periodic":false}"#,
                "/edits/0/geometry/end_tangent",
            ),
        ] {
            let (status, response) = call(
                &mut session,
                &format!(
                    r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"space/0","geometry":{geometry}}}]}}"#
                ),
            );
            assert_eq!(status, Status::Failed, "{geometry}");
            assert_eq!(text(&error(&response), "/params/pointer"), pointer);
        }
        let added = r#"{"shape":"spline","degree":3,"knots":[],"control":[],"fit":[[0,0],[2,3],[5,1],[8,4]],"start_tangent":[1,1],"end_tangent":[1,-0.5],"closed":false,"periodic":false}"#;
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"space/0","geometry":{added}}}]}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        let id = text(&response, "/result/applied/0/id");
        let hash = text(&response, "/result/applied/0/hash");
        assert_eq!(seen(&mut session, "a.dxf")[3], (id.clone(), hash.clone()));
        assert_eq!(shapes(&mut session, "a.dxf")[3], added);
        // 制御点で決まるものには替えない
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{{"shape":"spline","degree":1,"knots":[0,0,1,1],"control":[[0,0],[1,1]],"closed":false,"periodic":false}}}}]}}"#
            ),
        );
        assert_eq!(status, Status::Failed);
        assert_eq!(
            text(&error(&response), "/params/pointer"),
            "/edits/0/geometry/control"
        );
        // 接線をなくし、当てはめる点を替える
        let replaced = r#"{"shape":"spline","degree":3,"knots":[],"control":[],"fit":[[1,0],[3,2],[6,0]],"closed":false,"periodic":false}"#;
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{replaced}}}]}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        let written = String::from_utf8(session.take(0).unwrap()).unwrap();
        assert_eq!(shapes(&mut session, "a.dxf")[3], replaced);
        let (id, hash) = seen(&mut session, "a.dxf")[3].clone();
        assert_eq!(id, text(&response, "/result/applied/0/id"));
        assert_eq!(hash, text(&response, "/result/applied/0/hash"));
        let (_, spline) = written.split_once("SPLINE").unwrap();
        let (spline, _) = spline.split_once("ENDSEC").unwrap();
        for field in [" 72\n0\n", " 73\n0\n", " 74\n3\n", " 11\n6.0\n"] {
            assert!(spline.contains(field), "{field:?}: {spline}");
        }
        for code in [" 10\n", " 12\n", " 40\n"] {
            assert!(!spline.contains(code), "{code:?}: {spline}");
        }
        // 制御点で決まるものは、当てはめる点で決まるものに替えない
        let controlled = r#"{"shape":"spline","degree":1,"knots":[0,0,1,1],"control":[[0,0],[1,1]],"closed":false,"periodic":false}"#;
        let (_, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"space/0","geometry":{controlled}}}]}}"#
            ),
        );
        let (id, hash) = (
            text(&response, "/result/applied/0/id"),
            text(&response, "/result/applied/0/hash"),
        );
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{replaced}}}]}}"#
            ),
        );
        assert_eq!(status, Status::Failed);
        assert_eq!(
            text(&error(&response), "/params/pointer"),
            "/edits/0/geometry/fit"
        );
    }

    #[test]
    fn adds_symbols_with_attributes_made_from_their_definitions() {
        let file =
            b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
            0\nSECTION\n2\nBLOCKS\n\
            0\nBLOCK\n5\n20\n8\n0\n2\nDOOR\n70\n2\n10\n0\n20\n0\n3\nDOOR\n\
            0\nLINE\n5\n21\n8\n0\n10\n0\n20\n0\n11\n1\n21\n0\n\
            0\nATTDEF\n5\n23\n8\n0\n10\n0.5\n20\n1.0\n40\n0.25\n1\nD1\n3\nNumber\n2\nNO\n70\n0\n\
            0\nATTDEF\n5\n24\n8\n0\n10\n0\n20\n-1\n40\n0.25\n1\nACME\n3\nMaker\n2\nMAKER\n70\n2\n\
            0\nENDBLK\n5\n22\n8\n0\n0\nENDSEC\n\
            0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n";
        // 読めない ATTDEF があれば、属性を省いた記号を足さない。
        let broken = String::from_utf8(file.to_vec())
            .unwrap()
            .replace("2\nNO\n70\n0", "70\n0");
        let mut rejected = Session::new(b"").unwrap();
        rejected.push(b"broken.dxf", broken.as_bytes()).unwrap();
        call(&mut rejected, r#"{"v":1,"op":"open","input":"broken.dxf"}"#);
        let (status, response) = call(
            &mut rejected,
            r#"{"v":1,"op":"apply","doc":"broken.dxf","edits":[{"kind":"add","place":"space/0","geometry":{"shape":"symbol","name":"DOOR","position":[0,0],"scale":[1,1],"rotation":0}}]}"#,
        );
        assert_eq!(status, Status::Failed, "{response:?}");
        assert_eq!(
            text(&error(&response), "/params/pointer"),
            "/edits/0/geometry/attributes"
        );

        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", file).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let add = |session: &mut Session, geometry: &str| {
            call(
                session,
                &format!(
                    r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"space/0","geometry":{geometry}}}]}}"#
                ),
            )
        };
        // 値が決まった属性の定義（MAKER）は、属性を作らない。尺度の大きさが違えば、文字を写せない
        for (geometry, pointer) in [
            (
                r#"{"shape":"symbol","name":"DOOR","position":[0,0],"scale":[1,1],"rotation":0,"attributes":[{"tag":"MAKER","value":"X"}]}"#,
                "/edits/0/geometry/attributes/0/tag",
            ),
            (
                r#"{"shape":"symbol","name":"DOOR","position":[0,0],"scale":[1,2],"rotation":0,"attributes":[{"tag":"NO","value":"X"}]}"#,
                "/edits/0/geometry/attributes",
            ),
        ] {
            let (status, response) = add(&mut session, geometry);
            assert_eq!(status, Status::Failed);
            assert_eq!(text(&error(&response), "/params/pointer"), pointer);
        }
        let (status, response) = add(
            &mut session,
            r#"{"shape":"symbol","name":"DOOR","position":[10,20],"scale":[2,2],"rotation":90,"attributes":[{"tag":"no","value":"D7"}]}"#,
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        let written = String::from_utf8(session.take(0).unwrap()).unwrap();
        let id = text(&response, "/result/applied/0/id");
        let hash = text(&response, "/result/applied/0/hash");
        assert!(seen(&mut session, "a.dxf").contains(&(id.clone(), hash.clone())));
        assert_eq!(
            shapes(&mut session, "a.dxf"),
            [
                r#"{"shape":"symbol","name":"DOOR","position":[10,20],"scale":[2,2],"rotation":90,"attributes":[{"tag":"NO","value":"D7"}]}"#
            ]
        );
        // 属性の基点は、定義の (0.5, 1) を 2 倍して 90° 回し、(10, 20) へ動かした (8, 21)。高さは 2 倍の 0.5
        let (_, attribute) = written.split_once("ATTRIB").unwrap();
        let (attribute, _) = attribute.split_once("SEQEND").unwrap();
        for field in [
            " 10\n8.0\n",
            " 20\n21.0\n",
            " 40\n0.5\n",
            " 50\n90.0\n",
            "  1\nD7\n",
        ] {
            assert!(attribute.contains(field), "{field:?}: {attribute}");
        }
        // 置き換えは、属性のタグと 1 行の値だけを受け、値を変えれば ATTRIB の値を書き直す
        let replace = |session: &mut Session, attributes: &str| {
            call(
                session,
                &format!(
                    r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{{"shape":"symbol","name":"DOOR","position":[10,20],"scale":[2,2],"rotation":90,"attributes":{attributes}}}}}]}}"#
                ),
            )
        };
        for (attributes, pointer) in [
            (
                r#"[{"tag":"NUMBER","value":"D8"}]"#,
                "/edits/0/geometry/attributes/0/tag",
            ),
            (
                r#"[{"tag":"NO","value":"D\n8"}]"#,
                "/edits/0/geometry/attributes/0/value",
            ),
            (
                r#"[{"tag":"NO","value":"D8"},{"tag":"NO","value":"D9"}]"#,
                "/edits/0/geometry/attributes",
            ),
        ] {
            let (status, response) = replace(&mut session, attributes);
            assert_eq!(status, Status::Failed, "{attributes}");
            assert_eq!(text(&error(&response), "/params/pointer"), pointer);
        }
        let (status, response) = replace(&mut session, r#"[{"tag":"NO","value":"D8"}]"#);
        assert_eq!(status, Status::Ok, "{response:?}");
        let rewritten = String::from_utf8(session.take(0).unwrap()).unwrap();
        let (_, attribute) = rewritten.split_once("ATTRIB").unwrap();
        let (attribute, _) = attribute.split_once("SEQEND").unwrap();
        assert!(attribute.contains("  1\nD8\n"), "{attribute}");
        assert!(attribute.contains(" 10\n8.0\n"), "{attribute}");
        assert_eq!(
            seen(&mut session, "a.dxf"),
            [(
                text(&response, "/result/applied/0/id"),
                text(&response, "/result/applied/0/hash")
            )]
        );
        assert_eq!(
            shapes(&mut session, "a.dxf"),
            [
                r#"{"shape":"symbol","name":"DOOR","position":[10,20],"scale":[2,2],"rotation":90,"attributes":[{"tag":"NO","value":"D8"}]}"#
            ]
        );
        // 値を渡さなくても、定義の属性を作れなければ拒否する
        let (status, response) = add(
            &mut session,
            r#"{"shape":"symbol","name":"DOOR","position":[0,0],"scale":[1,2],"rotation":0}"#,
        );
        assert_eq!(status, Status::Failed);
        assert_eq!(
            text(&error(&response), "/params/pointer"),
            "/edits/0/geometry/attributes"
        );
        // 同じタグを大小文字を変えて渡しても、後の値を黙って捨てない
        let (status, response) = add(
            &mut session,
            r#"{"shape":"symbol","name":"DOOR","position":[0,0],"scale":[1,1],"rotation":0,"attributes":[{"tag":"NO","value":"A"},{"tag":"no","value":"B"}]}"#,
        );
        assert_eq!(status, Status::Failed);
        assert_eq!(
            text(&error(&response), "/params/pointer"),
            "/edits/0/geometry/attributes/1/tag"
        );
        // 値を渡さなければ、既定の値で作る
        let (status, response) = add(
            &mut session,
            r#"{"shape":"symbol","name":"DOOR","position":[5,5],"scale":[1,1],"rotation":0}"#,
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(
            shapes(&mut session, "a.dxf")[1],
            r#"{"shape":"symbol","name":"DOOR","position":[5,5],"scale":[1,1],"rotation":0,"attributes":[{"tag":"NO","value":"D1"}]}"#
        );
    }

    #[test]
    fn checks_the_changes_before_applying_them() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", EDITABLE).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let before = seen(&mut session, "a.dxf");
        let (id, hash) = &before[0];
        let cases = [
            (
                format!(r#""hash":"{hash}","kind":"remove","id":"{id}""#),
                r#""hash":"v2:00""#,
                "call.value.invalid",
                "/hash",
            ),
            (
                format!(r#""kind":"remove","id":"{id}","hash":"v2:00""#),
                "",
                "call.value.invalid",
                "/edits/0/hash",
            ),
            (
                format!(r#""kind":"move","id":"{id}","hash":"{hash}""#),
                "",
                "call.value.invalid",
                "/edits/0/kind",
            ),
            (
                format!(r#""kind":"transform","id":"{id}","hash":"{hash}","matrix":[1,0,0,0,1]"#),
                "",
                "call.value.invalid",
                "/edits/0/matrix",
            ),
            (
                format!(
                    r#""kind":"restyle","id":"{id}","hash":"{hash}","color":{{"inherit":"layer","aci":1}}"#
                ),
                "",
                "call.value.invalid",
                "/edits/0/color",
            ),
            (
                format!(r#""kind":"restyle","id":"{id}","hash":"{hash}","color":{{"rgb":[1,2]}}"#),
                "",
                "call.value.invalid",
                "/edits/0/color",
            ),
            (
                format!(r#""kind":"restyle","id":"{id}","hash":"{hash}","layer":"WALL""#),
                "",
                "call.value.invalid",
                "/edits/0/layer",
            ),
            (
                format!(
                    r#""kind":"restyle","id":"{id}","hash":"{hash}","line_type":{{"name":"DASHED"}}"#
                ),
                "",
                "call.value.invalid",
                "/edits/0/line_type",
            ),
            (
                format!(r#""kind":"restyle","id":"{id}","hash":"{hash}","line_type_scale":0"#),
                "",
                "call.value.invalid",
                "/edits/0/line_type_scale",
            ),
            (
                format!(
                    r#""kind":"restyle","id":"{id}","hash":"{hash}","line_weight":{{"default":false}}"#
                ),
                "",
                "call.value.invalid",
                "/edits/0/line_weight",
            ),
            (
                format!(r#""kind":"remove","id":"space/0/h99","hash":"{hash}""#),
                "",
                "change.element.missing",
                "",
            ),
            (
                format!(
                    r#""kind":"replace","id":"space/0/h99","hash":"{hash}","geometry":{{"shape":"line","start":[0,0],"end":[1,1]}}"#
                ),
                "",
                "change.element.missing",
                "",
            ),
            (
                format!(
                    r#""kind":"replace","id":"{id}","hash":"{hash}","geometry":{{"shape":"circle","center":[0,0],"radius":1}}"#
                ),
                "",
                "call.value.invalid",
                "/edits/0/geometry",
            ),
            (
                format!(
                    r#""kind":"replace","id":"{id}","hash":"{hash}","geometry":{{"shape":"line","start":[0],"end":[1,1]}}"#
                ),
                "",
                "call.value.invalid",
                "/edits/0/geometry/start",
            ),
            (
                r#""kind":"add","place":"space/9","geometry":{"shape":"line","start":[0,0],"end":[1,1]}"#.to_string(),
                "",
                "call.value.invalid",
                "/edits/0/place",
            ),
            (
                r#""kind":"add","place":"space/0","layer":"WALL","geometry":{"shape":"line","start":[0,0],"end":[1,1]}"#.to_string(),
                "",
                "call.value.invalid",
                "/edits/0/layer",
            ),
            (
                r#""kind":"add","place":"space/0","geometry":{"shape":"circle","center":[0,0],"radius":0}"#.to_string(),
                "",
                "call.value.invalid",
                "/edits/0/geometry/radius",
            ),
            (
                r#""kind":"add","place":"space/0","geometry":{"shape":"polyline","points":[[0,0],[1,1]],"bulges":[0],"closed":false}"#.to_string(),
                "",
                "call.value.invalid",
                "/edits/0/geometry/bulges",
            ),
            (
                r#""kind":"add","place":"space/0","geometry":{"shape":"text","content":"A\nB","position":[0,0],"height":2.5,"rotation":0}"#.to_string(),
                "",
                "call.value.invalid",
                "/edits/0/geometry/content",
            ),
        ];
        for (edit, document, code, pointer) in cases {
            let (edit, document) = if document.is_empty() {
                (edit, String::new())
            } else {
                // 文書のハッシュの例は、変更をそのままにして文書のハッシュだけを壊す
                let edit = edit.replacen(&format!(r#""hash":"{hash}","#), "", 1)
                    + &format!(r#","hash":"{hash}""#);
                (edit, format!(",{document}"))
            };
            let request =
                format!(r#"{{"v":1,"op":"apply","doc":"a.dxf"{document},"edits":[{{{edit}}}]}}"#);
            let (status, response) = call(&mut session, &request);
            assert_eq!(status, Status::Failed, "{request}");
            let error = error(&response);
            assert_eq!(text(&error, "/code"), code, "{request}");
            if !pointer.is_empty() {
                assert_eq!(text(&error, "/params/pointer"), pointer, "{request}");
            }
        }
        assert_eq!(seen(&mut session, "a.dxf"), before);
    }

    #[test]
    fn lists_points_as_points() {
        // 点は、記号ではなく点の種類で返す（決定 0160）
        let mut session = Session::new(b"").unwrap();
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nPOINT\n8\n0\n10\n1.0\n20\n2.0\n0\nENDSEC\n0\nEOF\n";
        session.push(b"a.dxf", dxf.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let (status, response) = call(&mut session, r#"{"v":1,"op":"entities","doc":"a.dxf"}"#);
        assert_eq!(status, Status::Ok);
        assert_eq!(text(&response, "/result/items/0/kind"), "point");
    }

    #[test]
    fn replaces_and_adds_points_by_their_geometry() {
        // 点の形の値を返し、置き換えて足す。DXF の POINT は既定義の印を持たない（決定 0183）
        let mut session = Session::new(b"").unwrap();
        let dxf = concat!(
            "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$HANDSEED\n5\n20\n0\nENDSEC\n",
            "0\nSECTION\n2\nENTITIES\n0\nPOINT\n5\n1A\n100\nAcDbEntity\n8\n0\n100\nAcDbPoint\n",
            "10\n1.0\n20\n2.0\n30\n0.0\n0\nENDSEC\n0\nEOF\n",
        );
        session.push(b"a.dxf", dxf.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let before = shapes(&mut session, "a.dxf");
        assert_eq!(
            before,
            [r#"{"shape":"point","position":[1,2],"rotation":0}"#]
        );
        let (id, hash) = seen(&mut session, "a.dxf").remove(0);
        let replace = |geometry: &str| {
            format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{geometry}}}]}}"#
            )
        };
        let add = |geometry: &str| {
            format!(
                r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"add","place":"space/0","geometry":{geometry}}}]}}"#
            )
        };
        // 置き換えは描き方を変えず、印の名前空間は既知のものだけ
        for (request, pointer) in [
            (
                replace(
                    r#"{"shape":"point","position":[1,2],"rotation":0,"marker":{"namespace":"sxf.marker","code":"2","scale":1}}"#,
                ),
                "/edits/0/geometry/marker",
            ),
            (
                add(
                    r#"{"shape":"point","position":[1,2],"rotation":0,"marker":{"namespace":"other","code":"2","scale":1}}"#,
                ),
                "/edits/0/geometry/marker/namespace",
            ),
        ] {
            let (status, response) = call(&mut session, &request);
            assert_eq!(status, Status::Failed, "{request}");
            assert_eq!(text(&error(&response), "/params/pointer"), pointer);
        }
        let (status, response) = call(
            &mut session,
            &add(
                r#"{"shape":"point","position":[1,2],"rotation":0,"marker":{"namespace":"sxf.marker","code":"2","scale":1}}"#,
            ),
        );
        assert_eq!(status, Status::Failed);
        assert_eq!(text(&error(&response), "/code"), "change.edit.unsupported");
        assert_eq!(shapes(&mut session, "a.dxf"), before);

        let replaced = r#"{"shape":"point","position":[5,6],"rotation":30}"#;
        let added = r#"{"shape":"point","position":[3,4],"rotation":0}"#;
        let request = format!(
            r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{replaced}}},{{"kind":"add","place":"space/0","geometry":{added}}}]}}"#
        );
        let (status, response) = call(&mut session, &request);
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(shapes(&mut session, "a.dxf"), [replaced, added]);
    }

    #[test]
    fn changes_the_point_style_of_a_document() {
        // 文書の点の表し方を、DXF の見出しの $PDMODE・$PDSIZE に書く。applied は ID を持たない（決定 0190）
        let mut session = Session::new(b"").unwrap();
        let dxf = concat!(
            "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$HANDSEED\n5\n20\n0\nENDSEC\n",
            "0\nSECTION\n2\nENTITIES\n0\nPOINT\n5\n1A\n100\nAcDbEntity\n8\n0\n100\nAcDbPoint\n",
            "10\n1.0\n20\n2.0\n30\n0.0\n0\nENDSEC\n0\nEOF\n",
        );
        session.push(b"a.dxf", dxf.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let before = shapes(&mut session, "a.dxf");
        // 大きさは 0 より大きい有限の数
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"apply","doc":"a.dxf","edits":[{"kind":"point_style","mark":"cross","size":0,"size_kind":"length"}]}"#,
        );
        assert_eq!(status, Status::Failed);
        assert_eq!(text(&error(&response), "/params/pointer"), "/edits/0/size");
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"apply","doc":"a.dxf","edits":[{"kind":"point_style","mark":"cross","circle":true,"size":2.5,"size_kind":"length"}]}"#,
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        assert!(response.pointer("/result/applied/0/id").is_none());
        assert!(response.pointer("/result/applied/0/hash").is_none());
        let file = String::from_utf8(session.take(0).unwrap()).unwrap();
        let (header, _) = file.split_once("ENTITIES").unwrap();
        assert!(header.contains("$PDMODE\n 70\n35\n"), "{header}");
        assert!(header.contains("$PDSIZE\n 40\n2.5\n"), "{header}");
        assert_eq!(shapes(&mut session, "a.dxf"), before);
        // drawing の point_style で、今の点の表し方を読める（決定 0193）
        let (status, response) = call(&mut session, r#"{"v":1,"op":"drawing","doc":"a.dxf"}"#);
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(text(&response, "/result/point_style/mark"), "cross");
        assert_eq!(
            response.pointer("/result/point_style/circle"),
            Some(&Value::Bool(true))
        );
        assert_eq!(number(&response, "/result/point_style/size"), "2.5");
        assert_eq!(text(&response, "/result/point_style/size_kind"), "length");
    }

    #[test]
    fn lists_the_elements_of_an_open_document_page_by_page() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", DRAWING).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let (status, response) = call(&mut session, r#"{"v":1,"op":"entities","doc":"a.dxf"}"#);
        assert_eq!(status, Status::Ok);
        assert_eq!(number(&response, "/result/total"), "3");
        assert_eq!(text(&response, "/result/items/0/id"), "space/0/r3");
        assert_eq!(text(&response, "/result/items/0/kind"), "line");
        assert_eq!(text(&response, "/result/items/0/layer"), "0");
        assert_eq!(number(&response, "/result/items/0/source/record"), "3");
        assert_eq!(text(&response, "/result/items/1/kind"), "text");
        assert_eq!(text(&response, "/result/items/1/layer"), "TEXT");
        assert_eq!(text(&response, "/result/items/2/id"), "sheet/0/r5");
        assert!(response.pointer("/result/next_offset").is_none());
        // 続きは next_offset から読む
        let (_, response) = call(
            &mut session,
            r#"{"v":1,"op":"entities","doc":"a.dxf","limit":2}"#,
        );
        assert_eq!(number(&response, "/result/next_offset"), "2");
        assert!(response.pointer("/result/items/2").is_none());
        let (_, response) = call(
            &mut session,
            r#"{"v":1,"op":"entities","doc":"a.dxf","offset":2,"limit":2}"#,
        );
        assert_eq!(text(&response, "/result/items/0/kind"), "line");
        assert!(response.pointer("/result/items/1").is_none());
        assert!(response.pointer("/result/next_offset").is_none());
        let (_, response) = call(
            &mut session,
            r#"{"v":1,"op":"entities","doc":"a.dxf","offset":9}"#,
        );
        assert!(response.pointer("/result/items/0").is_none());
        assert_eq!(number(&response, "/result/total"), "3");
        // 返す要素 1 つごとに、作業を 1 つ使う
        let request =
            |work| format!(r#"{{"v":1,"op":"entities","doc":"a.dxf","budget":{{"work":{work}}}}}"#);
        let enough = (1..200)
            .find(|&work| call(&mut session, &request(work)).0 == Status::Ok)
            .unwrap();
        let (status, response) = call(&mut session, &request(enough - 1));
        assert_eq!(status, Status::Failed);
        assert_eq!(
            number(&response, "/diagnostics/1/params/requested"),
            enough.to_string()
        );
        // 止めた診断は、返そうとしていた要素の出所を場所に持つ（決定 0119）
        assert_eq!(number(&response, "/diagnostics/1/at/0/record"), "5");
        for request in [
            r#"{"v":1,"op":"entities","doc":"a.dxf","limit":0}"#,
            r#"{"v":1,"op":"entities","doc":"a.dxf","limit":10001}"#,
        ] {
            let (status, _) = call(&mut session, request);
            assert_eq!(status, Status::Failed, "{request}");
        }
    }

    #[test]
    fn selects_fields_and_narrows_to_a_region() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", DRAWING).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"entities","doc":"a.dxf","fields":["bbox"]}"#,
        );
        assert_eq!(status, Status::Ok);
        assert_eq!(text(&response, "/result/items/0/id"), "space/0/r3");
        assert_eq!(number(&response, "/result/items/0/bbox/min_x"), "1.5");
        assert_eq!(number(&response, "/result/items/0/bbox/max_y"), "50");
        assert!(response.pointer("/result/items/0/kind").is_none());
        assert!(response.pointer("/result/items/0/source").is_none());
        // 文字は基点だけを囲む。用紙の要素は用紙の座標で比べる
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"entities","doc":"a.dxf","region":[0,0,50,50],"fields":["kind","bbox"]}"#,
        );
        assert_eq!(status, Status::Ok);
        assert_eq!(number(&response, "/result/total"), "2");
        assert_eq!(text(&response, "/result/items/0/id"), "space/0/r3");
        assert_eq!(text(&response, "/result/items/1/id"), "sheet/0/r5");
        assert_eq!(number(&response, "/result/items/1/bbox/max_x"), "410");
        for (request, pointer) in [
            (
                r#"{"v":1,"op":"entities","doc":"a.dxf","fields":[]}"#,
                "/fields",
            ),
            (
                r#"{"v":1,"op":"entities","doc":"a.dxf","fields":["id","id"]}"#,
                "/fields",
            ),
            (
                r#"{"v":1,"op":"entities","doc":"a.dxf","region":[0,0,1]}"#,
                "/region",
            ),
            (
                r#"{"v":1,"op":"entities","doc":"a.dxf","region":[1,0,0,1]}"#,
                "/region",
            ),
        ] {
            let (status, response) = call(&mut session, request);
            assert_eq!(status, Status::Failed, "{request}");
            assert_eq!(text(&response, "/diagnostics/0/params/pointer"), pointer);
        }
    }

    #[test]
    fn keeps_large_sxf_instance_numbers_exact() {
        let src = "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('SCADEC level2 feature_mode'),'2;1');\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n/*SXF3\n#10 = layer_feature(\\'L\\','1')\nSXF3*/\n/*SXF3\n#18446744073709551615 = line_feature('1','1','1','1','0.0','0.0','100.0','50.0')\nSXF3*/\n/*SXF3\n#30 = drawing_sheet_feature(\\'S\\','3','1','0','0')\nSXF3*/\nENDSEC;\nEND-ISO-10303-21;\n";
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.sfc", src.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.sfc"}"#);
        let (status, result) = call(&mut session, r#"{"v":1,"op":"entities","doc":"a.sfc"}"#);
        assert_eq!(status, Status::Ok);
        assert_eq!(
            result.pointer("/result/items/0/id"),
            Some(&Value::String("sheet/0/i18446744073709551615".into()))
        );
        assert_eq!(
            result.pointer("/result/items/0/source/instance"),
            Some(&Value::String("18446744073709551615".into()))
        );
    }

    #[test]
    fn numbers_elements_that_share_a_source() {
        let mut page = Page {
            start: 0,
            end: 10,
            fields: vec![EntityField::Layer, EntityField::Source],
            region: Some(Extent {
                min_x: 0.5,
                min_y: 0.5,
                max_x: 9.0,
                max_y: 9.0,
            }),
            position: 0,
            items: Vec::new(),
            style_ids: None,
        };
        let at = |x: f64| zumen_drawing::PaperPoint {
            sheet: zumen_drawing::SheetId(3),
            x,
            y: x,
        };
        let line = |source: Ref, from: f64| Element {
            source,
            layer: zumen_drawing::LayerId(9),
            style: zumen_drawing::Style {
                color: zumen_drawing::Inherit::Layer,
                line_type: zumen_drawing::Inherit::Layer,
                line_type_scale: 1.0,
                line_weight: None,
                transparency: zumen_drawing::Inherit::Layer,
            },
            shape: Shape::Line {
                start: at(from),
                end: at(from + 1.0),
            },
        };
        // 2 つ目は region の外にあるが、ID の番号は絞り込む前の要素で振る
        let elements = [
            line(Ref::at_bytes(40, 80), 0.0),
            line(Ref::at_bytes(40, 80), 20.0),
            line(Ref::at_bytes(40, 80), 1.0),
            line(Ref::default(), 2.0),
        ];
        let mut budget = Budget::new(Limits::default());
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        assert!(
            page.collect(&Drawing2D::default(), "sheet/3", &elements, &mut cx)
                .is_ok()
        );
        let ids: Vec<_> = page.items.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, ["sheet/3/b40", "sheet/3/b40/2", "sheet/3/e3"]);
        assert_eq!(page.position, 3);
        assert_eq!(page.items[0].layer, None);
        assert_eq!(page.items[0].kind, None);
        assert_eq!(page.items[0].bbox, None);
        let source = page.items[0].source.as_ref();
        assert_eq!(
            source.and_then(|s| s.bytes.as_ref()).map(|b| b.end),
            Some(80)
        );
    }

    #[test]
    fn opens_dxf_after_checking_the_input_and_the_budget() {
        let mut session = Session::new(b"").unwrap();
        let (status, response) = call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        assert_eq!(status, Status::Failed);
        assert_eq!(text(&response, "/diagnostics/0/code"), "call.input.missing");
        session.push(b"a.dxf", b"0\nSECTION\n").unwrap();
        let (_, response) = call(
            &mut session,
            r#"{"v":1,"op":"open","input":"a.dxf","budget":{"input_bytes":4}}"#,
        );
        assert_eq!(
            text(&response, "/diagnostics/0/code"),
            "budget.input-bytes.exceeded"
        );
        assert_eq!(number(&response, "/diagnostics/0/params/limit"), "4");
        assert_eq!(number(&response, "/diagnostics/0/params/requested"), "10");
        let (_, response) = call(
            &mut session,
            r#"{"v":1,"op":"open","input":"a.dxf","budget":{"records":0}}"#,
        );
        assert_eq!(
            text(&response, "/diagnostics/0/code"),
            "budget.records.exceeded"
        );
        assert!(session.docs.is_empty());
        let (status, response) = call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        assert_eq!(status, Status::Ok);
        assert_eq!(text(&response, "/result/doc"), "a.dxf");
        assert_eq!(text(&response, "/result/format"), "dxf");
        let (_, response) = call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        assert_eq!(
            text(&response, "/diagnostics/0/code"),
            "call.document.exists"
        );
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"open","input":"a.dxf","doc":"b"}"#,
        );
        assert_eq!(status, Status::Ok);
        assert_eq!(text(&response, "/result/doc"), "b");
        session.push(b"c.txt", b"0\n").unwrap();
        let (_, response) = call(&mut session, r#"{"v":1,"op":"open","input":"c.txt"}"#);
        assert_eq!(
            text(&response, "/diagnostics/0/code"),
            "call.format.unsupported"
        );
        let (_, response) = call(
            &mut session,
            r#"{"v":1,"op":"open","input":"a.dxf","doc":"/x"}"#,
        );
        assert_eq!(text(&response, "/diagnostics/0/params/pointer"), "/doc");
    }

    #[test]
    fn closes_inputs_only_when_every_name_exists() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a", b"1").unwrap();
        session.push(b"b", b"2").unwrap();
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"close","inputs":["a","c","d"]}"#,
        );
        assert_eq!(status, Status::Failed);
        assert_eq!(text(&response, "/diagnostics/0/params/name"), "c");
        assert_eq!(text(&response, "/diagnostics/1/params/name"), "d");
        assert_eq!(session.inputs.len(), 2);
        let (status, response) = call(&mut session, r#"{"v":1,"op":"close","inputs":["a","b"]}"#);
        assert_eq!(status, Status::Ok);
        assert_eq!(
            response.pointer("/result"),
            Some(&zumen_json::parse(b"{}").unwrap())
        );
        assert!(session.inputs.is_empty());
        let (_, response) = call(&mut session, r#"{"v":1,"op":"close","docs":["a.dxf"]}"#);
        assert_eq!(
            text(&response, "/diagnostics/0/code"),
            "call.document.missing"
        );
        session.push(b"a.dxf", b"0\nSECTION\n").unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let (status, _) = call(
            &mut session,
            r#"{"v":1,"op":"close","docs":["a.dxf","b.dxf"]}"#,
        );
        assert_eq!(status, Status::Failed);
        assert_eq!(session.docs.len(), 1);
        let (status, _) = call(&mut session, r#"{"v":1,"op":"close","docs":["a.dxf"]}"#);
        assert_eq!(status, Status::Ok);
        assert!(session.docs.is_empty());
    }

    #[test]
    fn rejects_the_time_limit_in_wasm_and_unknown_engines() {
        let mut wasm = Session::for_engine(Engine::Wasm, b"").unwrap();
        let (_, response) = call(
            &mut wasm,
            r#"{"v":1,"op":"capabilities","time_limit_ms":10}"#,
        );
        assert_eq!(
            text(&response, "/diagnostics/0/code"),
            "call.field.unsupported"
        );
        assert_eq!(
            text(&response, "/diagnostics/0/params/pointer"),
            "/time_limit_ms"
        );
        let mut native = Session::new(b"").unwrap();
        let (status, _) = call(
            &mut native,
            r#"{"v":1,"op":"capabilities","time_limit_ms":10000}"#,
        );
        assert_eq!(status, Status::Ok);
        assert!(Session::new(br#"{"v":1}"#).is_ok());
        assert!(Session::new(br#"{"v":1,"engine":"native"}"#).is_ok());
        assert_eq!(
            Session::new(br#"{"v":1,"engine":"process","process_path":"no-such-folder/zumen"}"#)
                .err(),
            Some(Status::Failed)
        );
        assert_eq!(Session::new(br#"{"v":2}"#).err(), Some(Status::Invalid));
        assert_eq!(
            Session::new(br#"{"engine":"native"}"#).err(),
            Some(Status::Invalid)
        );
        assert_eq!(Session::new(b"{").err(), Some(Status::Invalid));
    }

    #[test]
    fn stops_when_cancelled() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a", b"1").unwrap();
        let cancel = CancelToken::new();
        cancel.cancel();
        let reply = session.call(br#"{"v":1,"op":"close","inputs":["a"]}"#, &cancel);
        assert_eq!(reply.status, Status::Failed);
        let response = zumen_json::parse(&reply.response).unwrap();
        assert_eq!(
            text(&response, "/diagnostics/0/code"),
            "call.request.cancelled"
        );
        assert_eq!(session.inputs.len(), 1);
    }

    #[test]
    fn hands_out_blobs_once_and_drops_them_on_the_next_call() {
        let mut session = Session::new(b"").unwrap();
        session.add_blob(BlobKind::File, b"abc".to_vec());
        let blobs = respond(None, &Diagnostics::new(), &session.blobs);
        assert_eq!(
            String::from_utf8(blobs).unwrap(),
            r#"{"v":1,"diagnostics":[],"blobs":[{"id":0,"kind":"file","size":3}]}"#
        );
        assert_eq!(session.take(0), Some(b"abc".to_vec()));
        assert_eq!(session.take(0), None);
        assert_eq!(session.take(1), None);
        session.add_blob(BlobKind::File, b"x".to_vec());
        call(&mut session, r#"{"v":1,"op":"capabilities"}"#);
        assert_eq!(session.take(0), None);
    }

    #[test]
    fn writes_large_and_non_finite_values_as_strings() {
        assert_eq!(
            param_of(&zumen_base::Value::UInt(u64::MAX)),
            messages::ParamValue::String("18446744073709551615".to_string())
        );
        assert_eq!(
            param_of(&zumen_base::Value::UInt(9_007_199_254_740_991)),
            messages::ParamValue::Number(9_007_199_254_740_991.0)
        );
        assert_eq!(
            param_of(&zumen_base::Value::Int(i64::MIN)),
            messages::ParamValue::String("-9223372036854775808".to_string())
        );
        assert_eq!(
            param_of(&zumen_base::Value::Float(f64::NAN)),
            messages::ParamValue::String("NaN".to_string())
        );
        assert_eq!(
            param_of(&zumen_base::Value::Float(f64::NEG_INFINITY)),
            messages::ParamValue::String("-Infinity".to_string())
        );
        assert_eq!(
            param_of(&zumen_base::Value::Key("hatch")),
            messages::ParamValue::String("hatch".to_string())
        );
    }

    #[test]
    fn writes_every_part_of_a_diagnostic() {
        let mut diagnostics = Diagnostics::new();
        diagnostics.report(
            Diagnostic::new(codes::ZIP_PATH_UNSAFE)
                .with("name", "../a")
                .at(zumen_base::Ref::at_record(3)
                    .with_id("7")
                    .with_bytes(10, 20))
                .cite(zumen_base::SpecRef {
                    spec: "zip-appnote-6.3.10",
                    section: "4.4.17",
                }),
        );
        let response = String::from_utf8(respond(None, &diagnostics, &[])).unwrap();
        assert_eq!(
            response,
            r#"{"v":1,"diagnostics":[{"code":"zip.path.unsafe","severity":"warning","count":1,"at":[{"id":"7","record":3,"bytes":{"start":10,"end":20}}],"cite":{"spec":"zip-appnote-6.3.10","section":"4.4.17"},"params":{"name":"../a"}}],"blobs":[]}"#
        );
    }

    #[test]
    fn element_and_place_lookup_count_work() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", EDITABLE).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let cancel = CancelToken::new();
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let drawing = session
            .lift(
                "a.dxf",
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
            )
            .ok()
            .unwrap();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(3)));
        let mut diagnostics = Diagnostics::new();
        assert!(
            ElementLookup::new(
                &drawing,
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel)
            )
            .is_err()
        );
        assert_eq!(
            diagnostics.iter().next().unwrap().code(),
            codes::BUDGET_WORK_EXCEEDED
        );
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let lookup = ElementLookup::new(
            &drawing,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        )
        .ok()
        .unwrap();
        assert!(lookup.get("space/0/h2F").is_some());
        assert_eq!(budget.used(Kind::Work), 4);
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(0)));
        let mut diagnostics = Diagnostics::new();
        assert!(
            PlaceLookup::new(
                &drawing,
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel)
            )
            .is_err()
        );
    }

    #[test]
    fn table_lookup_counts_work_and_rejects_ambiguous_names() {
        let mut drawing = Drawing2D::default();
        for id in 0..2 {
            drawing.layers.push(zumen_drawing::Layer {
                id: LayerId(id),
                name: "same".to_string(),
                state: None,
                style: None,
                follows_placement: false,
                source: Ref::default(),
                description: None,
            });
        }
        let cancel = CancelToken::new();
        let mut diagnostics = Diagnostics::new();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(1)));
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        assert!(TableLookup::new(&drawing, &mut cx).is_err());
        assert_eq!(
            diagnostics.iter().next().unwrap().code(),
            codes::BUDGET_WORK_EXCEEDED
        );

        let mut budget = Budget::new(Limits::default());
        let mut diagnostics = Diagnostics::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let lookup = TableLookup::new(&drawing, &mut cx).ok().unwrap();
        assert_eq!(lookup.layers.get("same"), Some(&None));
        assert_eq!(budget.used(Kind::Work), 2);
    }

    #[test]
    fn maps_every_budget_unit_to_its_field() {
        let budget = messages::Budget {
            input_bytes: Some(messages::Limit::Integer(1)),
            decoded_bytes: Some(messages::Limit::Integer(2)),
            records: Some(messages::Limit::Integer(3)),
            elements: Some(messages::Limit::Integer(4)),
            depth: Some(messages::Limit::Integer(5)),
            segments: Some(messages::Limit::Integer(6)),
            work: Some(messages::Limit::Unlimited),
        };
        let limits = limits(Some(&budget));
        for (index, kind) in Kind::ALL.into_iter().enumerate() {
            let expected = if kind == Kind::Work {
                Limit::Unlimited
            } else {
                Limit::Finite(index as u64 + 1)
            };
            assert_eq!(limits.get(kind), expected, "{kind:?}");
        }
        // JSON の欄の名前は、単位の名前の - を _ にしたもの
        let mut out = Writer::new();
        budget.to_json(&mut out);
        let written = zumen_json::parse(&out.into_bytes()).unwrap();
        let Value::Object(object) = written else {
            panic!()
        };
        let keys: Vec<String> = object.iter().map(|(key, _)| key.to_string()).collect();
        let names: Vec<String> = Kind::ALL
            .iter()
            .map(|kind| kind.name().replace('-', "_"))
            .collect();
        assert_eq!(keys, names);
    }

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn the_watchdog_expires_the_token_after_the_limit() {
        let cancel = CancelToken::new();
        let watchdog = watchdog::Watchdog::start(&cancel, 1).unwrap();
        let mut waited = 0;
        while cancel.stop().is_none() && waited < 5000 {
            std::thread::sleep(std::time::Duration::from_millis(1));
            waited += 1;
        }
        assert_eq!(cancel.stop(), Some(Stop::TimedOut));
        drop(watchdog);
        let untouched = CancelToken::new();
        drop(watchdog::Watchdog::start(&untouched, 60_000).unwrap());
        assert_eq!(untouched.stop(), None);
    }

    #[test]
    fn fitted_text_geometry_uses_the_direction_of_its_points() {
        let source = b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nTEXT\n5\n20\n8\n0\n10\n0\n20\n0\n11\n0\n21\n5\n40\n1\n1\nX1\n72\n5\n50\n0\n0\nENDSEC\n0\nEOF\n";
        let mut session = Session::new(b"").unwrap();
        session.push(b"fit.dxf", source).unwrap();
        assert_eq!(
            call(&mut session, r#"{"v":1,"op":"open","input":"fit.dxf"}"#).0,
            Status::Ok
        );
        let shape = shapes(&mut session, "fit.dxf")[0].clone();
        let value = zumen_json::parse(shape.as_bytes()).unwrap();
        assert_eq!(number(&value, "/rotation"), "90");
        let (id, hash) = seen(&mut session, "fit.dxf")[0].clone();
        let request = format!(
            r#"{{"v":1,"op":"apply","doc":"fit.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{shape}}}]}}"#
        );
        let (status, response) = call(&mut session, &request);
        assert_eq!(status, Status::Ok, "{response:?}");
    }

    #[test]
    fn spline_with_fit_and_control_is_not_silently_replaced() {
        let source = b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nSPLINE\n5\n20\n8\n0\n70\n8\n71\n1\n72\n4\n73\n2\n74\n2\n40\n0\n40\n0\n40\n1\n40\n1\n10\n0\n20\n0\n10\n1\n20\n0\n11\n0\n21\n0\n11\n1\n21\n2\n0\nENDSEC\n0\nEOF\n";
        let mut session = Session::new(b"").unwrap();
        session.push(b"fit.dxf", source).unwrap();
        assert_eq!(
            call(&mut session, r#"{"v":1,"op":"open","input":"fit.dxf"}"#).0,
            Status::Ok
        );
        assert_eq!(shapes(&mut session, "fit.dxf"), [""]);
        let (id, hash) = seen(&mut session, "fit.dxf")[0].clone();
        let request = format!(
            r#"{{"v":1,"op":"apply","doc":"fit.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{{"shape":"spline","degree":1,"knots":[],"control":[],"fit":[[0,0],[1,2]],"closed":false,"periodic":false}}}}]}}"#
        );
        assert_eq!(call(&mut session, &request).0, Status::Failed);
        assert_eq!(seen(&mut session, "fit.dxf")[0], (id, hash));
    }

    #[test]
    fn fit_points_obey_the_work_budget_before_conversion() {
        let points = std::iter::repeat_n("[0,0]", 80)
            .collect::<Vec<_>>()
            .join(",");
        let geometry = format!(
            r#"{{"shape":"spline","degree":3,"knots":[],"control":[],"fit":[{points}],"closed":false,"periodic":false}}"#
        );
        for edit in [
            format!(r#"{{"kind":"add","place":"space/0","geometry":{geometry}}}"#),
            format!(
                r#"{{"kind":"replace","id":"space/0/h1","hash":"v2:00000000000000000000000000000000","geometry":{geometry}}}"#
            ),
        ] {
            let json = format!(r#"{{"v":1,"op":"apply","doc":"a.dxf","edits":[{edit}]}}"#);
            let value = zumen_json::parse(json.as_bytes()).unwrap();
            let Request::Apply(request) = Request::from_json(&value).unwrap() else {
                panic!()
            };
            let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(50)));
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            assert!(
                changes(
                    &request,
                    &Drawing2D::default(),
                    &mut Cx::new(&mut budget, &mut diagnostics, &cancel)
                )
                .is_err()
            );
            assert_eq!(
                diagnostics.iter().next().unwrap().code(),
                codes::BUDGET_WORK_EXCEEDED
            );
        }
    }

    #[test]
    fn returning_fit_points_obeys_the_work_budget() {
        let source = b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nSPLINE\n5\n20\n8\n0\n70\n8\n71\n1\n72\n0\n73\n0\n74\n2\n11\n0\n21\n0\n11\n1\n21\n2\n0\nENDSEC\n0\nEOF\n";
        let mut session = Session::new(b"").unwrap();
        session.push(b"fit.dxf", source).unwrap();
        assert_eq!(
            call(&mut session, r#"{"v":1,"op":"open","input":"fit.dxf"}"#).0,
            Status::Ok
        );
        let cancel = CancelToken::new();
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let drawing = session
            .lift(
                "fit.dxf",
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
            )
            .ok()
            .unwrap();
        let mut page = Page {
            start: 0,
            end: 1,
            fields: vec![EntityField::Geometry],
            region: None,
            position: 0,
            items: Vec::new(),
            style_ids: None,
        };
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(1)));
        let mut diagnostics = Diagnostics::new();
        assert!(
            page.collect(
                &drawing,
                "space/0",
                &drawing.spaces[0].elements,
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel)
            )
            .is_err()
        );
        assert_eq!(
            diagnostics.iter().next().unwrap().code(),
            codes::BUDGET_WORK_EXCEEDED
        );
    }

    #[test]
    fn fit_spline_with_zero_tangent_is_not_returned_or_replaced() {
        let src = b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nSPLINE\n5\n20\n8\n0\n70\n8\n71\n1\n72\n0\n73\n0\n74\n2\n12\n0\n22\n0\n11\n0\n21\n0\n11\n1\n21\n2\n0\nENDSEC\n0\nEOF\n";
        let mut session = Session::new(b"").unwrap();
        session.push(b"fit.dxf", src).unwrap();
        assert_eq!(
            call(&mut session, r#"{"v":1,"op":"open","input":"fit.dxf"}"#).0,
            Status::Ok
        );
        assert_eq!(shapes(&mut session, "fit.dxf"), [""]);
        let (id, hash) = seen(&mut session, "fit.dxf")[0].clone();
        let request = format!(
            r#"{{"v":1,"op":"apply","doc":"fit.dxf","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{{"shape":"spline","degree":1,"knots":[],"control":[],"fit":[[0,0],[1,2]],"closed":false,"periodic":false}}}}]}}"#
        );
        assert_eq!(call(&mut session, &request).0, Status::Failed);
        assert_eq!(seen(&mut session, "fit.dxf")[0], (id, hash));
    }

    /// レイヤ・線種・文字スタイル・属性を持つ記号・ハッチングを持つ R2000 の DXF
    const CONVERTIBLE: &str = "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$INSUNITS\n70\n4\n\
        9\n$HANDSEED\n5\n90\n0\nENDSEC\n0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLTYPE\n\
        0\nLTYPE\n5\n11\n2\nContinuous\n70\n0\n3\nSolid line\n72\n65\n73\n0\n40\n0.0\n\
        0\nLTYPE\n5\n12\n2\nDashed\n70\n0\n3\n__ __\n72\n65\n73\n2\n40\n15.0\n49\n10.0\n74\n0\n49\n-5.0\n74\n0\n\
        0\nENDTAB\n0\nTABLE\n2\nLAYER\n0\nLAYER\n5\n10\n2\n0\n70\n0\n62\n7\n6\nContinuous\n\
        0\nLAYER\n5\n13\n2\nWall\n70\n0\n62\n1\n6\nDashed\n0\nENDTAB\n\
        0\nTABLE\n2\nSTYLE\n0\nSTYLE\n5\n14\n2\nStandard\n70\n0\n40\n0.0\n41\n1.0\n50\n0.0\n71\n0\n42\n2.5\n3\ntxt\n4\n\n\
        0\nSTYLE\n5\n15\n2\nNote\n70\n0\n40\n0.0\n41\n0.8\n50\n0.0\n71\n0\n42\n3.5\n3\nromans.shx\n4\n\n\
        0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nBLOCKS\n\
        0\nBLOCK\n5\n20\n8\n0\n2\nDOOR\n70\n2\n10\n1.0\n20\n2.0\n3\nDOOR\n\
        0\nLINE\n5\n21\n8\n0\n10\n0.0\n20\n0.0\n11\n1.0\n21\n0.0\n\
        0\nATTDEF\n5\n23\n8\n0\n10\n0.0\n20\n1.0\n40\n1.0\n1\n\n3\nNo\n2\nNO\n70\n0\n\
        0\nENDBLK\n5\n22\n8\n0\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n\
        0\nLINE\n5\n30\n8\nWall\n10\n0.0\n20\n0.0\n11\n100.0\n21\n0.0\n\
        0\nCIRCLE\n5\n31\n8\n0\n6\nDashed\n10\n5.0\n20\n5.0\n40\n2.0\n\
        0\nTEXT\n5\n32\n8\n0\n10\n0.0\n20\n10.0\n40\n2.5\n1\nABC\n7\nNote\n\
        0\nINSERT\n5\n40\n8\n0\n66\n1\n2\nDOOR\n10\n50.0\n20\n5.0\n\
        0\nATTRIB\n5\n41\n8\n0\n10\n50.0\n20\n6.0\n40\n1.0\n1\nD-1\n2\nNO\n70\n0\n\
        0\nSEQEND\n5\n42\n8\n0\n\
        0\nHATCH\n5\n50\n8\n0\n10\n0.0\n20\n0.0\n30\n0.0\n2\nSOLID\n70\n1\n71\n0\n91\n1\n\
        92\n1\n93\n3\n72\n1\n10\n0.0\n20\n0.0\n11\n1.0\n21\n0.0\n72\n1\n10\n1.0\n20\n0.0\n11\n0.0\n21\n1.0\n\
        72\n1\n10\n0.0\n20\n1.0\n11\n0.0\n21\n0.0\n97\n0\n75\n0\n76\n1\n98\n0\n\
        0\nENDSEC\n0\nEOF\n";

    fn codes_of(response: &Value) -> Vec<String> {
        let Some(Value::Array(diagnostics)) = response.pointer("/diagnostics") else {
            return Vec::new();
        };
        (0..diagnostics.len())
            .map(|index| text(response, &format!("/diagnostics/{index}/code")))
            .collect()
    }

    #[test]
    fn converts_a_drawing_to_a_new_dxf() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", CONVERTIBLE.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"convert","doc":"a.dxf","format":"dxf","to":"b.dxf"}"#,
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(text(&response, "/result/doc"), "b.dxf");
        // 線分・円・文字・記号と、記号から離した属性の文字。ハッチングは落とす
        assert_eq!(number(&response, "/result/elements"), "5", "{response:?}");
        let codes = codes_of(&response);
        assert!(
            codes.contains(&"convert.attribute.drawn".to_string()),
            "{codes:?}"
        );
        assert!(
            codes.contains(&"convert.element.unsupported".to_string()),
            "{codes:?}"
        );
        let file = String::from_utf8(session.take(0).unwrap()).unwrap();
        assert!(file.contains("\nDOOR\n"), "{file}");
        // 書いた文書は、開いた文書と同じに使える
        let (status, tables) = call(&mut session, r#"{"v":1,"op":"tables","doc":"b.dxf"}"#);
        assert_eq!(status, Status::Ok, "{tables:?}");
        let mut out = zumen_json::Writer::new();
        out.value(&tables);
        let tables = String::from_utf8(out.into_bytes()).unwrap();
        for name in ["\"Wall\"", "\"Dashed\"", "\"Note\""] {
            assert!(tables.contains(name), "{name}: {tables}");
        }
        let (status, entities) = call(
            &mut session,
            r#"{"v":1,"op":"entities","doc":"b.dxf","fields":["geometry"]}"#,
        );
        assert_eq!(status, Status::Ok, "{entities:?}");
        let mut out = zumen_json::Writer::new();
        out.value(&entities);
        let entities = String::from_utf8(out.into_bytes()).unwrap();
        for value in ["\"ABC\"", "\"D-1\"", "\"DOOR\""] {
            assert!(entities.contains(value), "{value}: {entities}");
        }
    }

    #[test]
    fn converts_sfc_to_dxf() {
        // SXF の要素は用紙にあるので、DXF の用紙に書く
        let sfc = concat!(
            "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('SCADEC level2 feature_mode'),'2;1');\n",
            "FILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n",
            "/*SXF3\n#10 = layer_feature(\\'L\\','1')\nSXF3*/\n",
            "/*SXF3\n#20 = line_feature('1','1','1','1','0.0','0.0','100.0','50.0')\nSXF3*/\n",
            "/*SXF3\n#30 = drawing_sheet_feature(\\'S\\','3','1','0','0')\nSXF3*/\n",
            "ENDSEC;\nEND-ISO-10303-21;\n",
        );
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.sfc", sfc.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.sfc"}"#);
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"convert","doc":"a.sfc","format":"dxf","to":"a.dxf"}"#,
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(text(&response, "/result/format"), "dxf");
        assert_eq!(number(&response, "/result/elements"), "1", "{response:?}");
        let (status, drawing) = call(&mut session, r#"{"v":1,"op":"drawing","doc":"a.dxf"}"#);
        assert_eq!(status, Status::Ok, "{drawing:?}");
        let mut out = zumen_json::Writer::new();
        out.value(&drawing);
        let drawing = String::from_utf8(out.into_bytes()).unwrap();
        assert!(drawing.contains(r#""size":[420,297]"#), "{drawing}");
        assert!(drawing.contains(r#""max_x":100"#), "{drawing}");
    }

    #[test]
    fn refuses_conversions_it_cannot_write() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", CONVERTIBLE.as_bytes()).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        for (request, code) in [
            (
                r#"{"v":1,"op":"convert","doc":"a.dxf","format":"sxf","to":"b.sfc"}"#,
                "convert.format.unsupported",
            ),
            (
                r#"{"v":1,"op":"convert","doc":"a.dxf","format":"dxf","to":"a.dxf"}"#,
                "call.document.exists",
            ),
            (
                r#"{"v":1,"op":"convert","doc":"c.dxf","format":"dxf","to":"b.dxf"}"#,
                "call.document.missing",
            ),
        ] {
            let (status, response) = call(&mut session, request);
            assert_eq!(status, Status::Failed, "{response:?}");
            assert_eq!(codes_of(&response), vec![code.to_string()], "{request}");
        }
    }
}
