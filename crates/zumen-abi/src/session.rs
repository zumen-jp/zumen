//! セッション: 名前のついた入力を持ち、要求を受けて応答を返す（決定 0012）。

use std::collections::BTreeMap;

use zumen_acad::{Dwg, DwgFile, Dxf, DxfFile};
use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, Stop};
use zumen_base::{Key, Ref, codes, name};
use zumen_drawing::{
    Affine, Applied, Changes, Color, ColorCode, ColorSystem, Drawing2D, Edit, Element, Encode,
    Hash, Inherit, LayerId, LineTypeId, LineWeight, PaperLength, Placement, Restyle, Seen, Shape,
    Unit, id,
};
use zumen_json::{FromJson, Invalid, MAX_SAFE_INTEGER, Problem, ToJson, Writer};
use zumen_jww::{Jww, JwwFile};
use zumen_sdk::{Apply, Codec, Lift};
use zumen_sxf::{Sxf, SxfFile};

use crate::abi::{ABI_VERSION, SCHEMA_MAJOR, SCHEMA_MINOR, Status};
use crate::bounds::{self, Planar};
use crate::describe;
use crate::messages::{
    self, ApplyRequest, ApplyResult, BlobKind, ByteRange, CapabilitiesResult, CloseRequest,
    CloseResult, DrawingRequest, DrawingResult, ElementCounts, Engine, EntitiesRequest,
    EntitiesResult, Entity, EntityField, EntityKind, Extent, Format, OpenRequest, OpenResult,
    Request, RequestCommon, SchemaVersion, SessionOptions, SheetSummary, Source, SpaceSummary,
};

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
    /// 直前の呼び出しが作ったバイナリの結果。取り出したものは `None` になる
    blobs: Vec<Option<Blob>>,
}

/// 開いた文書。形式ごとのファイルモデル（L1）を持つ
#[derive(Debug)]
enum Doc {
    Dxf(DxfFile),
    Dwg(DwgFile),
    Sxf(SxfFile),
    Jww(JwwFile),
}

impl Doc {
    /// 形式の台帳のキー
    fn format(&self) -> &'static str {
        match self {
            Doc::Dxf(_) => Dxf::FORMAT,
            Doc::Dwg(_) => Dwg::FORMAT,
            Doc::Sxf(_) => Sxf::FORMAT,
            Doc::Jww(_) => Jww::FORMAT,
        }
    }

    /// 図面モデル（L2）に変換する
    fn lift(&self, cx: &mut Cx<'_>) -> Result<Drawing2D, zumen_sdk::Error> {
        match self {
            Doc::Dxf(file) => file.lift(cx),
            Doc::Dwg(file) => file.lift(cx),
            Doc::Sxf(file) => file.lift(cx),
            Doc::Jww(file) => file.lift(cx),
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
        let engine = if options.is_empty() {
            built
        } else {
            let value = zumen_json::parse(options).map_err(|_| Status::Invalid)?;
            let options = SessionOptions::from_json(&value).map_err(|_| Status::Invalid)?;
            if options.v != SCHEMA_MAJOR {
                return Err(Status::Invalid);
            }
            options.engine.unwrap_or(built)
        };
        // process（子プロセスの CLI）は、CLI と一緒に作る
        if engine != built {
            return Err(Status::Failed);
        }
        Ok(Self {
            engine,
            inputs: BTreeMap::new(),
            docs: BTreeMap::new(),
            hashes: BTreeMap::new(),
            blobs: Vec::new(),
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
        let result: Result<Box<dyn ToJson>, Failed> = match &request {
            Request::Capabilities(_) => Ok(Box::new(capabilities())),
            Request::Open(request) => self.open(request, &mut cx).map(|r| Box::new(r) as _),
            Request::Close(request) => self.close(request, &mut cx).map(|r| Box::new(r) as _),
            Request::Describe(request) => {
                describe::describe(&self.inputs, request, &mut cx).map(|r| Box::new(r) as _)
            }
            Request::Drawing(request) => self.drawing(request, &mut cx).map(|r| Box::new(r) as _),
            Request::Entities(request) => self.entities(request, &mut cx).map(|r| Box::new(r) as _),
            Request::Apply(request) => self.apply(request, &mut cx).map(|r| Box::new(r) as _),
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
        if self.docs.contains_key(name) {
            return Err(fail(
                cx,
                Diagnostic::new(codes::CALL_DOCUMENT_EXISTS).with("name", name.as_str()),
            ));
        }
        // 形式は、コーデックが持つ判定の規則で決める（決定 0007・0008）。入力の大きさは、コーデックが確かめる
        let doc = if Dxf::SIGNATURES.iter().any(|rule| rule.matches(bytes)) {
            // 失敗の理由は、コーデックが診断として足してある
            Doc::Dxf(Dxf::read(bytes, cx).map_err(|_| Failed)?)
        } else if Dwg::SIGNATURES.iter().any(|rule| rule.matches(bytes)) {
            // 読めるのは R2000（AC1015）だけ。ほかの版は、コーデックが診断を足して失敗する
            Doc::Dwg(Dwg::read(bytes, cx).map_err(|_| Failed)?)
        } else if Sxf::SIGNATURES.iter().any(|rule| rule.matches(bytes)) {
            // 同じ始まりの IFC などは、コーデックがスキーマを確かめて、診断を足して失敗する
            Doc::Sxf(Sxf::read(bytes, cx).map_err(|_| Failed)?)
        } else if Jww::SIGNATURES.iter().any(|rule| rule.matches(bytes)) {
            Doc::Jww(Jww::read(bytes, cx).map_err(|_| Failed)?)
        } else {
            cx.check(Kind::InputBytes, bytes.len() as u64)
                .map_err(|stop| stopped(cx, stop))?;
            return Err(fail(
                cx,
                Diagnostic::new(codes::CALL_FORMAT_UNSUPPORTED)
                    .with("name", request.input.as_str()),
            ));
        };
        let result = OpenResult {
            doc: name.clone(),
            format: doc.format().to_string(),
        };
        let hash = Hash::document_checked(bytes, cx).map_err(|stop| stopped(cx, stop))?;
        self.hashes.insert(name.clone(), hash);
        self.docs.insert(name.clone(), doc);
        Ok(result)
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
            self.hashes.remove(doc);
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
        };
        for space in &drawing.spaces {
            page.collect(&drawing, &id::space(space.id), &space.elements, cx)?;
        }
        let mut sheets = id::SheetNamer::default();
        for sheet in &drawing.sheets {
            cx.check(Kind::Work, 1)
                .map_err(|stop| fail(cx, stop.to_diagnostic()))?;
            page.collect(&drawing, &sheets.next(sheet), &sheet.elements, cx)?;
        }
        let total = page.position;
        Ok(EntitiesResult {
            items: page.items,
            total,
            next_offset: (page.end < total).then_some(page.end),
        })
    }

    /// 変更の一覧を文書に当てて書き戻し、書き戻したファイルを blob で返す（決定 0139）
    fn apply(&mut self, request: &ApplyRequest, cx: &mut Cx<'_>) -> Result<ApplyResult, Failed> {
        let drawing = self.lift(&request.doc, cx)?;
        if let Some(doc @ (Doc::Sxf(_) | Doc::Jww(_))) = self.docs.get(&request.doc) {
            return Err(fail(
                cx,
                Diagnostic::new(codes::CHANGE_FORMAT_UNSUPPORTED).with("format", doc.format()),
            ));
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
        let (applied, hash) = match doc {
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
            Doc::Sxf(_) | Doc::Jww(_) => return Err(Failed),
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
        })
    }

    /// 名前の文書を、図面モデルに変換する
    fn lift(&self, doc: &str, cx: &mut Cx<'_>) -> Result<Drawing2D, Failed> {
        if !name::is_safe(doc) {
            return Err(fail(cx, invalid_value("/doc")));
        }
        let Some(found) = self.docs.get(doc) else {
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

/// 要求の変更を、図面モデルの変更の一覧にする。レイヤと線種は、名前が 1 つに決まるものを図面から引く
fn changes(
    request: &ApplyRequest,
    drawing: &Drawing2D,
    cx: &mut Cx<'_>,
) -> Result<Changes, Failed> {
    let hash = |text: &str, pointer: String, cx: &mut Cx<'_>| {
        text.parse::<Hash>()
            .map_err(|()| fail(cx, invalid_value(pointer)))
    };
    let document = match &request.hash {
        Some(text) => Some(hash(text, "/hash".to_string(), cx)?),
        None => None,
    };
    let mut edits = Vec::with_capacity(request.edits.len());
    let mut tables = None;
    for (index, edit) in request.edits.iter().enumerate() {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        let at = |field: &str| format!("/edits/{index}/{field}");
        let seen = |id: &str, text: &str, cx: &mut Cx<'_>| {
            Ok(Seen {
                id: id.to_string(),
                hash: hash(text, at("hash"), cx)?,
            })
        };
        edits.push(match edit {
            messages::Edit::Restyle(edit) => {
                let target = seen(&edit.id, &edit.hash, cx)?;
                if tables.is_none() {
                    tables = Some(TableLookup::new(drawing, cx)?);
                }
                let Some(tables) = &tables else {
                    return Err(Failed);
                };
                Edit::Restyle {
                    target,
                    style: restyle(edit, tables)
                        .map_err(|field| fail(cx, invalid_value(at(field))))?,
                }
            }
            messages::Edit::Transform(edit) => {
                let target = seen(&edit.id, &edit.hash, cx)?;
                let m = match edit.matrix.as_slice() {
                    &[a, b, c, d, e, f] if edit.matrix.iter().all(|v| v.is_finite()) => {
                        [a, b, c, d, e, f]
                    }
                    _ => return Err(fail(cx, invalid_value(at("matrix")))),
                };
                Edit::Transform {
                    target,
                    transform: Affine { m },
                }
            }
            messages::Edit::Remove(edit) => Edit::Remove {
                target: seen(&edit.id, &edit.hash, cx)?,
            },
        });
    }
    Ok(Changes { edits, document })
}

/// 表の検索を変更ごとに繰り返さないための索引。同じ名前は 1 つに決まらない
struct TableLookup<'a> {
    layers: BTreeMap<&'a str, Option<LayerId>>,
    line_types: BTreeMap<&'a str, Option<LineTypeId>>,
}

impl<'a> TableLookup<'a> {
    fn new(drawing: &'a Drawing2D, cx: &mut Cx<'_>) -> Result<Self, Failed> {
        let mut result = Self {
            layers: BTreeMap::new(),
            line_types: BTreeMap::new(),
        };
        for layer in &drawing.layers {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            result
                .layers
                .entry(&layer.name)
                .and_modify(|v| *v = None)
                .or_insert(Some(layer.id));
        }
        for line_type in &drawing.line_types {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            result
                .line_types
                .entry(&line_type.name)
                .and_modify(|v| *v = None)
                .or_insert(Some(line_type.id));
        }
        Ok(result)
    }
}

/// 要求のスタイルの変更を、図面モデルの変更にする。合わない欄があれば、その欄の名前を返す
fn restyle(
    edit: &messages::RestyleEdit,
    tables: &TableLookup<'_>,
) -> Result<Restyle, &'static str> {
    let layer = match &edit.layer {
        None => None,
        Some(name) => Some(
            tables
                .layers
                .get(name.as_str())
                .copied()
                .flatten()
                .ok_or("layer")?,
        ),
    };
    let color = match &edit.color {
        None => None,
        Some(change) => Some(match (change.inherit, change.aci, change.rgb.as_deref()) {
            (Some(to), None, None) => inherit(to),
            (None, None, None) | (Some(_), _, _) => return Err("color"),
            (None, aci, rgb) => {
                let rgb = match rgb {
                    None => None,
                    Some(&[r, g, b]) => {
                        Some([r, g, b].map(|value| u8::try_from(value).unwrap_or(u8::MAX)))
                    }
                    Some(_) => return Err("color"),
                };
                let code = match aci {
                    None => None,
                    Some(value) => Some(ColorCode {
                        system: ColorSystem::Aci,
                        value: u32::try_from(value).map_err(|_| "color")?,
                    }),
                };
                Inherit::Own(Color { code, rgb })
            }
        }),
    };
    let line_type = match &edit.line_type {
        None => None,
        Some(change) => Some(match (change.inherit, &change.name) {
            (Some(to), None) => inherit(to),
            (None, Some(name)) => Inherit::Own(
                tables
                    .line_types
                    .get(name.as_str())
                    .copied()
                    .flatten()
                    .ok_or("line_type")?,
            ),
            _ => return Err("line_type"),
        }),
    };
    let line_type_scale = match edit.line_type_scale {
        Some(scale) if !(scale.is_finite() && scale > 0.0) => return Err("line_type_scale"),
        scale => scale,
    };
    let line_weight = match &edit.line_weight {
        None => None,
        Some(change) => Some(match (change.inherit, change.mm, change.default) {
            (Some(to), None, None) => inherit(to),
            (None, Some(mm), None) if mm.is_finite() && mm >= 0.0 => {
                Inherit::Own(LineWeight::Width(PaperLength(mm)))
            }
            (None, None, Some(true)) => Inherit::Own(LineWeight::Default),
            _ => return Err("line_weight"),
        }),
    };
    Ok(Restyle {
        layer,
        color,
        line_type,
        line_type_scale,
        line_weight,
    })
}

/// 継承する先
fn inherit<T>(inherit: messages::Inherit) -> Inherit<T> {
    match inherit {
        messages::Inherit::Layer => Inherit::Layer,
        messages::Inherit::Block => Inherit::Block,
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
        ops: Request::TAGS.iter().map(|op| op.to_string()).collect(),
        // ID の規則の版。AutoCAD の系統はハンドルで、SXF はインスタンスの番号で ID を作る 2（決定 0156）
        // 書けるのは、apply で書き戻せる形式（決定 0139）
        formats: [
            (Dxf::FORMAT, "acad", true, 2),
            (Dwg::FORMAT, "acad", true, 2),
            (Sxf::FORMAT, "sxf", false, 2),
            (Jww::FORMAT, "jww", false, 1),
        ]
        .into_iter()
        .map(|(id, family, write, id_rules)| Format {
            id: id.to_string(),
            family: family.to_string(),
            read: true,
            write,
            id_rules,
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
    })
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
}

impl Page {
    /// 空間か用紙の要素を数え、返す範囲にあるものを足す
    fn collect<P: Planar + Encode>(
        &mut self,
        drawing: &Drawing2D,
        place: &str,
        elements: &[Element<P>],
        cx: &mut Cx<'_>,
    ) -> Result<(), Failed> {
        let mut namer = id::Namer::new(place);
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
    }
}

fn unit_of(unit: Unit) -> messages::Unit {
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
        | Shape::Hatch(_) => [None, None],
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
fn message_of(diagnostic: &Diagnostic) -> messages::Diagnostic {
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
    use zumen_json::Value;

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
    fn answers_capabilities() {
        let mut session = Session::new(b"").unwrap();
        let reply = session.call(br#"{"v":1,"op":"capabilities"}"#, &CancelToken::new());
        assert_eq!(reply.status, Status::Ok);
        let expected = format!(
            r#"{{"v":1,"result":{{"package":"{}","abi":1,"schema":{{"major":1,"minor":7}},"ops":["capabilities","open","close","describe","drawing","entities","apply"],"formats":[{{"id":"dxf","family":"acad","read":true,"write":true,"id_rules":2}},{{"id":"dwg","family":"acad","read":true,"write":true,"id_rules":2}},{{"id":"sxf","family":"sxf","read":true,"write":false,"id_rules":2}},{{"id":"jww","family":"jww","read":true,"write":false,"id_rules":1}}]}},"diagnostics":[],"blobs":[]}}"#,
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
        assert!(text(&response, "/result/hash").starts_with("v1:"));
        assert_eq!(text(&response, "/result/spaces/0/unit"), "millimeter");
        assert_eq!(number(&response, "/result/spaces/0/elements/lines"), "1");
        assert_eq!(number(&response, "/result/spaces/0/elements/texts"), "1");
        assert_eq!(number(&response, "/result/spaces/0/extent/min_x"), "-3");
        assert_eq!(number(&response, "/result/spaces/0/extent/max_y"), "50");
        assert_eq!(number(&response, "/result/sheets/0/size/0"), "420");
        assert_eq!(number(&response, "/result/sheets/0/elements/lines"), "1");
        assert_eq!(number(&response, "/result/sheets/0/extent/max_x"), "410");
        assert_eq!(text(&response, "/diagnostics/0/code"), "dxf.lift.dropped");
        // 表の 9 レコード、変換の 4 図形、要約の 1 用紙に、それぞれ作業を 1 つ使う
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
            r#"{"v":1,"op":"drawing","doc":"a.dxf","budget":{"work":14}}"#,
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
        assert!(hash.starts_with("v1:") && hash.len() == 35, "{hash}");
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
        assert!(a.starts_with("v1:") && a != b, "{a} {b}");
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
    fn checks_the_changes_before_applying_them() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", EDITABLE).unwrap();
        call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
        let before = seen(&mut session, "a.dxf");
        let (id, hash) = &before[0];
        let cases = [
            (
                format!(r#""hash":"{hash}","kind":"remove","id":"{id}""#),
                r#""hash":"v1:00""#,
                "call.value.invalid",
                "/hash",
            ),
            (
                format!(r#""kind":"remove","id":"{id}","hash":"v1:00""#),
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
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"entities","doc":"a.dxf","budget":{"work":16}}"#,
        );
        assert_eq!(status, Status::Failed);
        assert_eq!(number(&response, "/diagnostics/1/params/requested"), "17");
        // 止めた診断は、返そうとしていた要素の出所を場所に持つ（決定 0119）
        assert_eq!(number(&response, "/diagnostics/1/at/0/record"), "5");
        let (status, _) = call(
            &mut session,
            r#"{"v":1,"op":"entities","doc":"a.dxf","budget":{"work":17}}"#,
        );
        assert_eq!(status, Status::Ok);
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
            Session::new(br#"{"v":1,"engine":"process"}"#).err(),
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
}
