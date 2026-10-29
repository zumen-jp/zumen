//! セッション: 名前のついた入力を持ち、要求を受けて応答を返す（決定 0012）。

use std::borrow::Cow;
use std::collections::BTreeMap;

use zumen_acad::{Dwg, DwgFile, Dxf, DxfFile};
use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, Stop};
use zumen_base::{Key, Ref, codes, name};
use zumen_drawing::{
    Affine, Applied, Changes, Color, ColorCode, ColorSystem, Drawing2D, Edit, Element, Encode,
    Hash, Inherit, Layer, LayerId, LayerState, LayerStyle, LineTypeId, LineWeight, ModelPoint,
    PaperLength, PaperPoint, Placed, Placement, Restyle, Seen, Shape, Style, Unit, id,
};
use zumen_json::{FromJson, Invalid, MAX_SAFE_INTEGER, Problem, ToJson, Writer};
use zumen_jww::{Jww, JwwFile};
use zumen_sdk::{Apply, Codec, Lift};
use zumen_sxf::{Sxf, SxfFile};

use crate::abi::{ABI_VERSION, SCHEMA_MAJOR, SCHEMA_MINOR, Status};
use crate::bounds;
use crate::describe;
use crate::geometry;
use crate::messages::{
    self, ApplyRequest, ApplyResult, BlobKind, ByteRange, CapabilitiesResult, CloseRequest,
    CloseResult, DrawingRequest, DrawingResult, ElementCounts, Engine, EntitiesRequest,
    EntitiesResult, Entity, EntityField, EntityKind, Extent, Font, FontKind, Format, LayerItem,
    OpenRequest, OpenResult, Request, RequestCommon, SchemaVersion, SessionOptions, SheetSummary,
    Source, SpaceSummary, TableItem, TablesRequest, TablesResult, TextStyleItem,
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
            Request::Tables(request) => self.tables(request, &mut cx).map(|r| Box::new(r) as _),
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
            style_ids: None,
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

    /// レイヤと線種の表の項目を、ID・名前・ハッシュで返す（決定 0156）
    fn tables(&self, request: &TablesRequest, cx: &mut Cx<'_>) -> Result<TablesResult, Failed> {
        let drawing = self.lift(&request.doc, cx)?;
        let mut seen = BTreeMap::new();
        let mut line_types = Vec::with_capacity(drawing.line_types.len());
        let mut line_type_ids = BTreeMap::new();
        for line_type in &drawing.line_types {
            let hash = Hash::line_type_checked(line_type, cx).map_err(|stop| stopped(cx, stop))?;
            let id = table_id(id::line_type(line_type), &mut seen);
            line_type_ids
                .entry(line_type.id)
                .or_insert_with(|| id.clone());
            line_types.push(TableItem {
                id,
                name: line_type.name.clone(),
                hash: hash.to_string(),
            });
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
            dimension_styles.push(TableItem {
                id: table_id(id::dimension_style(style, index), &mut seen),
                name: style.name.clone(),
                hash: hash.to_string(),
            });
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

/// 文字スタイルの表の項目。定義がなければ、ID・名前・ハッシュだけを返す
/// レイヤの表の項目（決定 0162）。線種は `line_types` の ID で指す
fn layer_item(
    id: String,
    layer: &Layer,
    hash: Hash,
    line_types: &BTreeMap<LineTypeId, String>,
) -> LayerItem {
    let style = layer.style.as_ref();
    LayerItem {
        id,
        name: layer.name.clone(),
        hash: hash.to_string(),
        defined: layer.state.is_some() || layer.style.is_some(),
        visible: layer.state.map(|state| state.visible),
        printable: layer.state.map(|state| state.printable),
        editable: layer.state.map(|state| state.editable),
        frozen: layer.state.map(|state| state.frozen),
        color: style
            .and_then(|style| style.color)
            .map(|color| messages::ColorChange {
                inherit: None,
                aci: color
                    .code
                    .filter(|code| code.system == ColorSystem::Aci)
                    .map(|code| u64::from(code.value)),
                rgb: color
                    .rgb
                    .map(|rgb| rgb.iter().map(|&value| u64::from(value)).collect()),
            }),
        line_type_id: style
            .and_then(|style| style.line_type)
            .and_then(|line_type| line_types.get(&line_type).cloned()),
        line_weight: style
            .and_then(|style| style.line_weight)
            .map(|weight| match weight {
                LineWeight::Width(width) => messages::LineWeightChange {
                    inherit: None,
                    mm: Some(width.0),
                    default: None,
                },
                LineWeight::Default => messages::LineWeightChange {
                    inherit: None,
                    mm: None,
                    default: Some(true),
                },
            }),
    }
}

fn text_style_item(id: String, style: &zumen_drawing::TextStyle, hash: Hash) -> TextStyleItem {
    let definition = style.definition.as_ref();
    TextStyleItem {
        id,
        name: style.name.clone(),
        hash: hash.to_string(),
        defined: definition.is_some(),
        font: definition.and_then(|d| d.font.as_ref()).map(font_of),
        big_font: definition.and_then(|d| d.big_font.as_ref()).map(font_of),
        height: definition.and_then(|d| d.height),
        width: definition.map(|d| d.width),
        oblique: definition.map(|d| d.oblique),
        backward: definition.map(|d| d.backward),
        upside_down: definition.map(|d| d.upside_down),
        vertical: definition.map(|d| d.vertical),
    }
}

fn font_of(font: &zumen_drawing::Font) -> Font {
    match font.kind {
        zumen_drawing::FontKind::Shape => Font {
            name: font.name.clone(),
            kind: FontKind::Shape,
            bold: None,
            italic: None,
            charset: None,
            pitch: None,
        },
        zumen_drawing::FontKind::Named {
            bold,
            italic,
            charset,
            pitch,
        } => Font {
            name: font.name.clone(),
            kind: FontKind::Named,
            bold: Some(bold),
            italic: Some(italic),
            charset: charset.map(u64::from),
            pitch: pitch.map(u64::from),
        },
    }
}

/// 同じ表の ID が重なるときは、2 つ目から番号を足す
fn table_id(base: String, seen: &mut BTreeMap<String, u64>) -> String {
    let count = seen.entry(base.clone()).or_default();
    let result = if *count == 0 {
        base
    } else {
        format!("{base}/{count}")
    };
    *count = count.saturating_add(1);
    result
}

/// 要素が名前で指す文字スタイルと寸法スタイルの ID（tables が返すもの）。名前の比較は図面モデルと同じにし、
/// 同じ名前があれば先の項目にする（決定 0158）
#[derive(Default)]
struct StyleIds {
    text: BTreeMap<String, String>,
    dimension: BTreeMap<String, String>,
}

impl StyleIds {
    fn new(drawing: &Drawing2D, cx: &mut Cx<'_>) -> Result<Self, Failed> {
        let mut result = Self::default();
        let mut seen = BTreeMap::new();
        for (index, style) in drawing.dimension_styles.iter().enumerate() {
            let work = style.name.len().saturating_add(1);
            cx.check(Kind::Work, u64::try_from(work).unwrap_or(u64::MAX))
                .map_err(|stop| stopped(cx, stop))?;
            let id = table_id(id::dimension_style(style, index), &mut seen);
            result.dimension.entry(style.name.clone()).or_insert(id);
        }
        seen.clear();
        for (index, style) in drawing.text_styles.iter().enumerate() {
            let work = style.name.len().saturating_add(1);
            cx.check(Kind::Work, u64::try_from(work).unwrap_or(u64::MAX))
                .map_err(|stop| stopped(cx, stop))?;
            let id = table_id(id::text_style(style, index), &mut seen);
            result
                .text
                .entry(style.name.to_ascii_lowercase())
                .or_insert(id);
        }
        Ok(result)
    }

    /// 形の値に、名前で引いたスタイルの ID を書く
    fn fill(&self, geometry: &mut messages::Geometry) {
        let find = |items: &BTreeMap<String, String>, name: &str| {
            items.get(&name.to_ascii_lowercase()).cloned()
        };
        match geometry {
            messages::Geometry::Text(text) => {
                text.style_id = text
                    .style
                    .as_deref()
                    .and_then(|name| find(&self.text, name));
            }
            messages::Geometry::FormattedText(text) => {
                text.style_id = text
                    .style
                    .as_deref()
                    .and_then(|name| find(&self.text, name));
            }
            messages::Geometry::Dimension(dimension) => {
                dimension.style_id = self.dimension.get(&dimension.style).cloned();
            }
            _ => {}
        }
    }
}

/// 要求の変更を、図面モデルの変更の一覧にする。レイヤと線種は、ID か、名前が 1 つに決まるものを図面から引く
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
    let mut elements = None;
    let mut places = None;
    let mut definitions: Option<BTreeMap<String, &zumen_drawing::SymbolDefinition>> = None;
    for (index, edit) in request.edits.iter().enumerate() {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        let at = |field: &str| format!("/edits/{index}/{field}");
        let seen = |id: &str, text: &str, cx: &mut Cx<'_>| {
            cx.check(Kind::Work, u64::try_from(id.len()).unwrap_or(u64::MAX))
                .map_err(|stop| stopped(cx, stop))?;
            Ok(Seen {
                id: id.to_string(),
                hash: hash(text, at("hash"), cx)?,
            })
        };
        let value = match edit {
            messages::Edit::Replace(edit) => Some(&edit.geometry),
            messages::Edit::Add(edit) => Some(&edit.geometry),
            _ => None,
        };
        let size = match value {
            Some(messages::Geometry::Polyline(line)) => line.points.len(),
            Some(messages::Geometry::Spline(spline)) => spline
                .control
                .len()
                .saturating_add(spline.knots.len())
                .saturating_add(spline.weights.as_ref().map_or(0, Vec::len))
                .saturating_add(spline.fit.as_ref().map_or(0, Vec::len)),
            Some(messages::Geometry::Text(text)) => text
                .content
                .len()
                .saturating_add(text.style.as_ref().map_or(0, String::len))
                .saturating_add(text.style_id.as_ref().map_or(0, String::len)),
            Some(messages::Geometry::FormattedText(text)) => text
                .style
                .as_ref()
                .map_or(0, String::len)
                .saturating_add(text.style_id.as_ref().map_or(0, String::len)),
            Some(messages::Geometry::Symbol(symbol)) => symbol.name.len(),
            _ => 0,
        };
        cx.check(Kind::Work, u64::try_from(size).unwrap_or(u64::MAX))
            .map_err(|stop| stopped(cx, stop))?;
        if let Some(messages::Geometry::Spline(spline)) = value {
            for point in spline.control.iter().chain(spline.fit.iter().flatten()) {
                cx.check(Kind::Work, u64::try_from(point.len()).unwrap_or(u64::MAX))
                    .map_err(|stop| stopped(cx, stop))?;
            }
        }

        if let Some(messages::Geometry::Symbol(symbol)) = value {
            for item in symbol.attributes.iter().flatten() {
                let size = 1_usize
                    .saturating_add(item.tag.len())
                    .saturating_add(item.value.len());
                cx.check(Kind::Work, u64::try_from(size).unwrap_or(u64::MAX))
                    .map_err(|stop| stopped(cx, stop))?;
            }
        }
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
            messages::Edit::RemoveLayer(edit) => Edit::RemoveLayer {
                target: seen(&edit.id, &edit.hash, cx)?,
            },
            messages::Edit::Replace(edit) => {
                let target = seen(&edit.id, &edit.hash, cx)?;
                count_formatted_geometry(&edit.geometry, cx)?;
                if elements.is_none() {
                    elements = Some(ElementLookup::new(drawing, cx)?);
                }
                if tables.is_none() && has_style_id(&edit.geometry) {
                    tables = Some(TableLookup::new(drawing, cx)?);
                }
                let geometry = named_style(&edit.geometry, tables.as_ref())
                    .map_err(|field| fail(cx, invalid_value(at(field))))?;
                // 指した要素がなければ、除くときと同じく `change.element.missing` にする
                let element = match elements.as_ref().and_then(|index| index.get(&edit.id)) {
                    Some(FoundElement::Model(element)) => {
                        count_old_element(element, cx)?;
                        reshaped(element, &geometry).map(Placed::Model)
                    }
                    Some(FoundElement::Paper(element)) => {
                        count_old_element(element, cx)?;
                        reshaped(element, &geometry).map(Placed::Paper)
                    }
                    None => {
                        let missing = Diagnostic::new(codes::CHANGE_ELEMENT_MISSING)
                            .with("id", edit.id.as_str());
                        return Err(fail(cx, missing));
                    }
                }
                .map_err(|field| fail(cx, invalid_value(at(&field))))?;
                Edit::Replace {
                    target,
                    element: Box::new(element),
                }
            }
            messages::Edit::Add(edit) => {
                cx.check(
                    Kind::Work,
                    u64::try_from(edit.place.len()).unwrap_or(u64::MAX),
                )
                .map_err(|stop| stopped(cx, stop))?;
                count_formatted_geometry(&edit.geometry, cx)?;
                if tables.is_none() {
                    tables = Some(TableLookup::new(drawing, cx)?);
                }
                let Some(tables) = &tables else {
                    return Err(Failed);
                };
                if places.is_none() {
                    places = Some(PlaceLookup::new(drawing, cx)?);
                }
                let Some(places) = &places else {
                    return Err(Failed);
                };
                let definition = if let messages::Geometry::Symbol(symbol) = &edit.geometry {
                    if definitions.is_none() {
                        let mut found = BTreeMap::new();
                        for definition in &drawing.symbols {
                            let work = definition.key.name.len().saturating_add(1);
                            cx.check(Kind::Work, u64::try_from(work).unwrap_or(u64::MAX))
                                .map_err(|stop| stopped(cx, stop))?;
                            if definition.key.namespace == "dxf.block" {
                                found
                                    .entry(definition.key.name.to_ascii_lowercase())
                                    .or_insert(definition);
                            }
                        }
                        definitions = Some(found);
                    }
                    let work = symbol.name.len();
                    cx.check(Kind::Work, u64::try_from(work).unwrap_or(u64::MAX))
                        .map_err(|stop| stopped(cx, stop))?;
                    let found = definitions
                        .as_ref()
                        .and_then(|items| items.get(&symbol.name.to_ascii_lowercase()).copied());
                    if let Some(definition) = found {
                        for item in &definition.attributes {
                            let attr = &item.attribute;
                            let size = 1_usize
                                .saturating_add(attr.tag.len())
                                .saturating_add(attr.text.content.len())
                                .saturating_add(attr.text.style.len());
                            cx.check(Kind::Work, u64::try_from(size).unwrap_or(u64::MAX))
                                .map_err(|stop| stopped(cx, stop))?;
                        }
                    }
                    found
                } else {
                    None
                };
                let geometry = named_style(&edit.geometry, Some(tables))
                    .map_err(|field| fail(cx, invalid_value(at(field))))?;
                let element = added(edit, &geometry, places, tables, definition)
                    .map_err(|field| fail(cx, invalid_value(at(&field))))?;
                Edit::Add {
                    place: edit.place.clone(),
                    element: Box::new(element),
                }
            }
            messages::Edit::Layer(edit) => {
                let target = seen(&edit.id, &edit.hash, cx)?;
                if tables.is_none() {
                    tables = Some(TableLookup::new(drawing, cx)?);
                }
                let Some(tables) = &tables else {
                    return Err(Failed);
                };
                cx.check(
                    Kind::Work,
                    edit.name.as_ref().map_or(0, |name| name.len() as u64),
                )
                .map_err(|stop| stopped(cx, stop))?;
                // 指したレイヤがなければ、要素と同じく `change.element.missing` にする
                let Some(base) = tables
                    .layer_ids
                    .get(&edit.id)
                    .copied()
                    .flatten()
                    .and_then(|(_, layer)| drawing.layer(layer))
                else {
                    let missing =
                        Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", edit.id.as_str());
                    return Err(fail(cx, missing));
                };
                let fields = LayerFields {
                    name: edit.name.as_deref(),
                    visible: edit.visible,
                    printable: edit.printable,
                    editable: edit.editable,
                    frozen: edit.frozen,
                    color: edit.color.as_ref(),
                    line_type: edit.line_type.as_ref(),
                    line_weight: edit.line_weight.as_ref(),
                };
                let layer = layer_change(base, &fields, tables)
                    .map_err(|field| fail(cx, invalid_value(at(field))))?;
                Edit::ReplaceLayer {
                    target,
                    layer: Box::new(layer),
                }
            }
            messages::Edit::AddLayer(edit) => {
                if tables.is_none() {
                    tables = Some(TableLookup::new(drawing, cx)?);
                }
                let Some(tables) = &tables else {
                    return Err(Failed);
                };
                cx.check(Kind::Work, edit.name.len() as u64)
                    .map_err(|stop| stopped(cx, stop))?;
                // 書かない色・線種・線幅は、レイヤ 0 と同じにする
                let zero = tables
                    .layers
                    .get("0")
                    .copied()
                    .flatten()
                    .and_then(|layer| drawing.layer(layer));
                let base = Layer {
                    id: LayerId(u32::MAX),
                    name: String::new(),
                    state: None,
                    style: zero.and_then(|layer| layer.style),
                    follows_placement: false,
                    source: Ref::default(),
                };
                let fields = LayerFields {
                    name: Some(&edit.name),
                    visible: edit.visible,
                    printable: edit.printable,
                    editable: edit.editable,
                    frozen: edit.frozen,
                    color: edit.color.as_ref(),
                    line_type: edit.line_type.as_ref(),
                    line_weight: edit.line_weight.as_ref(),
                };
                let layer = layer_change(&base, &fields, tables)
                    .map_err(|field| fail(cx, invalid_value(at(field))))?;
                Edit::AddLayer {
                    layer: Box::new(layer),
                }
            }
        });
    }
    Ok(Changes { edits, document })
}

/// 要素の ID を一度だけ作り、複数の置き換えで共有する。
enum FoundElement<'a> {
    Model(&'a Element<ModelPoint>),
    Paper(&'a Element<PaperPoint>),
}

struct ElementLookup<'a>(BTreeMap<String, FoundElement<'a>>);

impl<'a> ElementLookup<'a> {
    fn new(drawing: &'a Drawing2D, cx: &mut Cx<'_>) -> Result<Self, Failed> {
        let mut found = BTreeMap::new();
        for space in &drawing.spaces {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            let place = id::space(space.id);
            let mut namer = id::Namer::new(&place);
            for (index, element) in space.elements.iter().enumerate() {
                cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
                found.insert(namer.next(index, element), FoundElement::Model(element));
            }
        }
        let mut definitions = id::DefinitionNamer::default();
        for definition in &drawing.symbols {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            let place = definitions.next(definition);
            let mut namer = id::Namer::new(&place);
            for (index, element) in definition.space.elements.iter().enumerate() {
                cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
                found.insert(namer.next(index, element), FoundElement::Model(element));
            }
        }
        let mut sheets = id::SheetNamer::default();
        for sheet in &drawing.sheets {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            let place = sheets.next(sheet);
            let mut namer = id::Namer::new(&place);
            for (index, element) in sheet.elements.iter().enumerate() {
                cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
                found.insert(namer.next(index, element), FoundElement::Paper(element));
            }
        }
        Ok(Self(found))
    }

    fn get(&self, id: &str) -> Option<&FoundElement<'a>> {
        self.0.get(id)
    }
}

/// 足す先の検索を変更ごとに繰り返さないための索引。
struct PlaceLookup {
    spaces: BTreeMap<String, ModelPoint>,
    sheets: BTreeMap<String, PaperPoint>,
}

impl PlaceLookup {
    fn new(drawing: &Drawing2D, cx: &mut Cx<'_>) -> Result<Self, Failed> {
        let mut spaces = BTreeMap::new();
        for space in &drawing.spaces {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            spaces.insert(
                id::space(space.id),
                ModelPoint {
                    space: space.id,
                    x: 0.0,
                    y: 0.0,
                },
            );
        }
        let mut sheets = BTreeMap::new();
        let mut namer = id::SheetNamer::default();
        for sheet in &drawing.sheets {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            sheets.insert(
                namer.next(sheet),
                PaperPoint {
                    sheet: sheet.id,
                    x: 0.0,
                    y: 0.0,
                },
            );
        }
        Ok(Self { spaces, sheets })
    }
}

/// MTEXT の解析は、入れ子や区間ごとにフォント名を複製する。内容の長さと、複製を起こし得る
/// 符号の数から上限を求め、解析より先に作業量を数える。
fn count_formatted_geometry(value: &messages::Geometry, cx: &mut Cx<'_>) -> Result<(), Failed> {
    if let messages::Geometry::FormattedText(text) = value {
        let bytes = u64::try_from(text.content.len()).unwrap_or(u64::MAX);
        cx.check(Kind::Work, bytes)
            .map_err(|stop| stopped(cx, stop))?;
        if text.content.contains("\\f") || text.content.contains("\\F") {
            let signs = text
                .content
                .bytes()
                .filter(|b| matches!(b, b'{' | b'}' | b'\\'))
                .count();
            let signs = u64::try_from(signs).unwrap_or(u64::MAX);
            cx.check(Kind::Work, bytes.saturating_mul(signs))
                .map_err(|stop| stopped(cx, stop))?;
        }
    }
    Ok(())
}

/// 表の検索を変更ごとに繰り返さないための索引。同じ名前は 1 つに決まらない
struct TableLookup<'a> {
    layers: BTreeMap<&'a str, Option<LayerId>>,
    layer_ids: BTreeMap<String, Option<(&'a str, LayerId)>>,
    line_types: BTreeMap<&'a str, Option<LineTypeId>>,
    line_type_ids: BTreeMap<String, Option<(&'a str, LineTypeId)>>,
    /// 文字スタイルの ID から名前。同じ名前（大文字と小文字を区別しない）の項目が複数あれば `None`
    text_style_ids: BTreeMap<String, Option<&'a str>>,
}

impl<'a> TableLookup<'a> {
    fn new(drawing: &'a Drawing2D, cx: &mut Cx<'_>) -> Result<Self, Failed> {
        let mut result = Self {
            layers: BTreeMap::new(),
            layer_ids: BTreeMap::new(),
            line_types: BTreeMap::new(),
            line_type_ids: BTreeMap::new(),
            text_style_ids: BTreeMap::new(),
        };
        let mut seen = BTreeMap::new();
        for layer in &drawing.layers {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            result
                .layers
                .entry(&layer.name)
                .and_modify(|v| *v = None)
                .or_insert(Some(layer.id));
            result
                .layer_ids
                .entry(table_id(id::layer(layer), &mut seen))
                .and_modify(|v| *v = None)
                .or_insert(Some((&layer.name, layer.id)));
        }
        seen.clear();
        for line_type in &drawing.line_types {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            result
                .line_types
                .entry(&line_type.name)
                .and_modify(|v| *v = None)
                .or_insert(Some(line_type.id));
            result
                .line_type_ids
                .entry(table_id(id::line_type(line_type), &mut seen))
                .and_modify(|v| *v = None)
                .or_insert(Some((&line_type.name, line_type.id)));
        }
        seen.clear();
        let mut names: BTreeMap<String, usize> = BTreeMap::new();
        for style in &drawing.text_styles {
            let work = style.name.len().saturating_add(1);
            cx.check(Kind::Work, u64::try_from(work).unwrap_or(u64::MAX))
                .map_err(|stop| stopped(cx, stop))?;
            *names.entry(style.name.to_ascii_lowercase()).or_default() += 1;
        }
        for (index, style) in drawing.text_styles.iter().enumerate() {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            let unique = names.get(&style.name.to_ascii_lowercase()) == Some(&1);
            result.text_style_ids.insert(
                table_id(id::text_style(style, index), &mut seen),
                unique.then_some(style.name.as_str()),
            );
        }
        Ok(result)
    }
}

/// 文字スタイルを ID で指した形の値か
fn has_style_id(geometry: &messages::Geometry) -> bool {
    match geometry {
        messages::Geometry::Text(text) => text.style_id.is_some(),
        messages::Geometry::FormattedText(text) => text.style_id.is_some(),
        _ => false,
    }
}

/// 文字スタイルを ID で指した形の値を、名前で指す形の値にする。合わなければ、その欄の名前を返す
fn named_style<'g>(
    geometry: &'g messages::Geometry,
    tables: Option<&TableLookup<'_>>,
) -> Result<Cow<'g, messages::Geometry>, &'static str> {
    let (style, wanted) = match geometry {
        messages::Geometry::Text(text) => (&text.style, &text.style_id),
        messages::Geometry::FormattedText(text) => (&text.style, &text.style_id),
        _ => return Ok(Cow::Borrowed(geometry)),
    };
    let Some(wanted) = wanted else {
        return Ok(Cow::Borrowed(geometry));
    };
    let name = tables
        .and_then(|tables| {
            tables
                .text_style_ids
                .get(wanted.as_str())
                .copied()
                .flatten()
        })
        .ok_or("geometry/style_id")?
        .to_string();
    // 名前も書けば、同じ文字スタイルを指す（entities が返した形の値を、そのまま戻せる）
    if style
        .as_ref()
        .is_some_and(|style| !style.eq_ignore_ascii_case(&name))
    {
        return Err("geometry/style_id");
    }
    let mut geometry = geometry.clone();
    match &mut geometry {
        messages::Geometry::Text(text) => (text.style, text.style_id) = (Some(name), None),
        messages::Geometry::FormattedText(text) => {
            (text.style, text.style_id) = (Some(name), None);
        }
        _ => {}
    }
    Ok(Cow::Owned(geometry))
}

/// 名前か ID で指したレイヤ。どちらもなければ `None`。合わなければ、その欄の名前を返す
fn layer_of(
    name: Option<&str>,
    wanted: Option<&str>,
    tables: &TableLookup<'_>,
) -> Result<Option<LayerId>, &'static str> {
    Ok(match (name, wanted) {
        (None, None) => None,
        (Some(name), None) => Some(tables.layers.get(name).copied().flatten().ok_or("layer")?),
        (None, Some(wanted)) => {
            let (name, id) = tables
                .layer_ids
                .get(wanted)
                .copied()
                .flatten()
                .ok_or("layer_id")?;
            // DXF は名前で書くので、名前が 1 つに決まらないレイヤは ID でも書き戻さない
            if tables.layers.get(name).copied().flatten() != Some(id) {
                return Err("layer_id");
            }
            Some(id)
        }
        (Some(_), Some(_)) => return Err("layer_id"),
    })
}

/// 形を形の値で置き換えた要素。合わない欄があれば、その欄の名前を返す
fn count_old_element<P>(element: &Element<P>, cx: &mut Cx<'_>) -> Result<(), Failed> {
    let source = element.source.id().map_or(0, str::len);
    let size = match &element.shape {
        Shape::Text(text) => text.style.len(),
        Shape::Formatted(text) => text.style.len(),
        Shape::Symbol(symbol) => {
            // 属性と切り抜きは元のまま複製する。内容と区間は、たどる前に予算を確かめる。
            for attribute in &symbol.attributes {
                let size = 1usize
                    .saturating_add(attribute.tag.len())
                    .saturating_add(attribute.text.content.len())
                    .saturating_add(attribute.text.style.len())
                    .saturating_add(attribute.source.id().map_or(0, str::len));
                cx.check(Kind::Work, u64::try_from(size).unwrap_or(u64::MAX))
                    .map_err(|stop| stopped(cx, stop))?;
                if let Some(text) = &attribute.formatted {
                    let size = text.content.len().saturating_add(text.style.len());
                    cx.check(Kind::Work, u64::try_from(size).unwrap_or(u64::MAX))
                        .map_err(|stop| stopped(cx, stop))?;
                    for span in &text.spans {
                        let text = match &span.piece {
                            zumen_drawing::Piece::Text(text)
                            | zumen_drawing::Piece::Unparsed(text) => text.len(),
                            zumen_drawing::Piece::Stack { upper, lower, .. } => {
                                upper.len().saturating_add(lower.len())
                            }
                            _ => 0,
                        };
                        let font = span.format.font.as_ref().map_or(0, |font| font.name.len());
                        let size = 1usize.saturating_add(text).saturating_add(font);
                        cx.check(Kind::Work, u64::try_from(size).unwrap_or(u64::MAX))
                            .map_err(|stop| stopped(cx, stop))?;
                    }
                }
            }
            symbol
                .key
                .name
                .len()
                .saturating_add(symbol.clip.as_ref().map_or(0, |clip| clip.vertices.len()))
        }
        _ => 0,
    };
    cx.check(
        Kind::Work,
        u64::try_from(source.saturating_add(size)).unwrap_or(u64::MAX),
    )
    .map_err(|stop| stopped(cx, stop))?;
    Ok(())
}

fn reshaped<P: geometry::Spot>(
    element: &Element<P>,
    value: &messages::Geometry,
) -> Result<Element<P>, String> {
    let origin = geometry::origin_of(&element.shape).ok_or("geometry")?;
    let shape = geometry::shape_of(value, Some(&element.shape), origin)?;
    Ok(Element {
        source: element.source.clone(),
        layer: element.layer,
        style: element.style,
        shape,
    })
}

/// 足す要素。レイヤを指さなければ「0」に置き、色・線種・線幅はレイヤに従う。合わない欄があれば、その欄の名前を返す
fn added(
    edit: &messages::AddEdit,
    geometry: &messages::Geometry,
    places: &PlaceLookup,
    tables: &TableLookup<'_>,
    definition: Option<&zumen_drawing::SymbolDefinition>,
) -> Result<Placed, String> {
    let layer = match layer_of(edit.layer.as_deref(), edit.layer_id.as_deref(), tables)? {
        Some(layer) => layer,
        None => layer_of(Some("0"), None, tables)?.ok_or("layer")?,
    };
    let style = Style {
        color: Inherit::Layer,
        line_type: Inherit::Layer,
        line_type_scale: 1.0,
        line_weight: Some(Inherit::Layer),
    };
    if let Some(&origin) = places.spaces.get(&edit.place) {
        return Ok(Placed::Model(Element {
            source: Ref::default(),
            layer,
            style,
            shape: attributed(
                geometry::shape_of(geometry, None, origin)?,
                edit,
                definition,
            )?,
        }));
    }
    if let Some(&origin) = places.sheets.get(&edit.place) {
        return Ok(Placed::Paper(Element {
            source: Ref::default(),
            layer,
            style,
            shape: attributed(
                geometry::shape_of(geometry, None, origin)?,
                edit,
                definition,
            )?,
        }));
    }
    Err("place".to_string())
}

/// 足す記号に、定義の属性の定義から属性を付ける
fn attributed<P: geometry::Spot>(
    shape: Shape<P>,
    edit: &messages::AddEdit,
    definition: Option<&zumen_drawing::SymbolDefinition>,
) -> Result<Shape<P>, String> {
    match (shape, &edit.geometry) {
        (Shape::Symbol(mut symbol), messages::Geometry::Symbol(given)) => {
            symbol.attributes =
                geometry::attributes_of(definition, &symbol, given.attributes.as_deref())?;
            Ok(Shape::Symbol(symbol))
        }
        (shape, _) => Ok(shape),
    }
}

/// 要求のスタイルの変更を、図面モデルの変更にする。合わない欄があれば、その欄の名前を返す
fn restyle(
    edit: &messages::RestyleEdit,
    tables: &TableLookup<'_>,
) -> Result<Restyle, &'static str> {
    let layer = layer_of(edit.layer.as_deref(), edit.layer_id.as_deref(), tables)?;
    let color = edit
        .color
        .as_ref()
        .map(color_change)
        .transpose()
        .map_err(|()| "color")?;
    let line_type = edit
        .line_type
        .as_ref()
        .map(|change| line_type_change(change, tables))
        .transpose()
        .map_err(|()| "line_type")?;
    let line_type_scale = match edit.line_type_scale {
        Some(scale) if !(scale.is_finite() && scale > 0.0) => return Err("line_type_scale"),
        scale => scale,
    };
    let line_weight = edit
        .line_weight
        .as_ref()
        .map(line_weight_change)
        .transpose()
        .map_err(|()| "line_weight")?;
    Ok(Restyle {
        layer,
        color,
        line_type,
        line_type_scale,
        line_weight,
    })
}

/// 新しい色。合わなければ `Err`
fn color_change(change: &messages::ColorChange) -> Result<Inherit<Color>, ()> {
    Ok(match (change.inherit, change.aci, change.rgb.as_deref()) {
        (Some(to), None, None) => inherit(to),
        (None, None, None) | (Some(_), _, _) => return Err(()),
        (None, aci, rgb) => {
            let rgb = match rgb {
                None => None,
                Some(&[r, g, b]) => {
                    Some([r, g, b].map(|value| u8::try_from(value).unwrap_or(u8::MAX)))
                }
                Some(_) => return Err(()),
            };
            let code = match aci {
                None => None,
                Some(value) => Some(ColorCode {
                    system: ColorSystem::Aci,
                    value: u32::try_from(value).map_err(|_| ())?,
                }),
            };
            Inherit::Own(Color { code, rgb })
        }
    })
}

/// 新しい線種。名前か ID が 1 つに決まらなければ `Err`
fn line_type_change(
    change: &messages::LineTypeChange,
    tables: &TableLookup<'_>,
) -> Result<Inherit<LineTypeId>, ()> {
    Ok(match (change.inherit, &change.name, &change.id) {
        (Some(to), None, None) => inherit(to),
        (None, None, Some(wanted)) => {
            let (name, id) = tables
                .line_type_ids
                .get(wanted)
                .copied()
                .flatten()
                .ok_or(())?;
            if tables.line_types.get(name).copied().flatten() != Some(id) {
                return Err(());
            }
            Inherit::Own(id)
        }
        (None, Some(name), None) => Inherit::Own(
            tables
                .line_types
                .get(name.as_str())
                .copied()
                .flatten()
                .ok_or(())?,
        ),
        _ => return Err(()),
    })
}

/// 新しい線幅
fn line_weight_change(change: &messages::LineWeightChange) -> Result<Inherit<LineWeight>, ()> {
    Ok(match (change.inherit, change.mm, change.default) {
        (Some(to), None, None) => inherit(to),
        (None, Some(mm), None) if mm.is_finite() && mm >= 0.0 => {
            Inherit::Own(LineWeight::Width(PaperLength(mm)))
        }
        (None, None, Some(true)) => Inherit::Own(LineWeight::Default),
        _ => return Err(()),
    })
}

/// レイヤの変更と追加の、書いた欄（決定 0162）
struct LayerFields<'e> {
    name: Option<&'e str>,
    visible: Option<bool>,
    printable: Option<bool>,
    editable: Option<bool>,
    frozen: Option<bool>,
    color: Option<&'e messages::ColorChange>,
    line_type: Option<&'e messages::LineTypeChange>,
    line_weight: Option<&'e messages::LineWeightChange>,
}

/// `base` に、書いた欄を重ねたレイヤ。凍結するだけなら表示もしない。表示するだけか、凍結を解くだけなら、
/// 凍結を解いて表示する。継承は書けない。合わなければ、その欄の名前を返す
fn layer_change(
    base: &Layer,
    fields: &LayerFields<'_>,
    tables: &TableLookup<'_>,
) -> Result<Layer, &'static str> {
    let was = base.state.unwrap_or(LayerState {
        visible: true,
        printable: true,
        editable: true,
        frozen: false,
    });
    let frozen = fields
        .frozen
        .unwrap_or(was.frozen && fields.visible != Some(true));
    // 凍結を解いた画層は、表示する
    let visible = fields
        .visible
        .unwrap_or(!frozen && (was.visible || was.frozen));
    let state = LayerState {
        visible,
        printable: fields.printable.unwrap_or(was.printable),
        editable: fields.editable.unwrap_or(was.editable),
        frozen,
    };
    let mut style = base.style.unwrap_or(LayerStyle {
        color: None,
        line_type: None,
        line_weight: None,
    });
    if let Some(change) = fields.color {
        match color_change(change) {
            Ok(Inherit::Own(color)) => style.color = Some(color),
            _ => return Err("color"),
        }
    }
    if let Some(change) = fields.line_type {
        match line_type_change(change, tables) {
            Ok(Inherit::Own(line_type)) => style.line_type = Some(line_type),
            _ => return Err("line_type"),
        }
    }
    if let Some(change) = fields.line_weight {
        match line_weight_change(change) {
            Ok(Inherit::Own(weight)) => style.line_weight = Some(weight),
            _ => return Err("line_weight"),
        }
    }
    let mut layer = base.clone();
    if let Some(name) = fields.name {
        layer.name = name.to_owned();
    }
    layer.state = Some(state);
    layer.style = Some(style);
    Ok(layer)
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
                geometry::geometry_of(&element.shape).map(|mut geometry| {
                    if let Some(ids) = &self.style_ids {
                        ids.fill(&mut geometry);
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
        Request::Tables(request) => &request.request_common,
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
                r#"{{"kind":"replace","id":"space/0/h1","hash":"v1:00000000000000000000000000000000","geometry":{geometry}}}"#
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
                    namespace: "dxf.block",
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
                    namespace: "dxf.block",
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
                r#"{{"kind":"replace","id":"space/0/h1","hash":"v1:00000000000000000000000000000000","geometry":{geometry}}}"#
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
            r#"{{"v":1,"result":{{"package":"{}","abi":1,"schema":{{"major":1,"minor":24}},"ops":["capabilities","open","close","describe","drawing","entities","apply","tables"],"formats":[{{"id":"dxf","family":"acad","read":true,"write":true,"id_rules":2}},{{"id":"dwg","family":"acad","read":true,"write":true,"id_rules":2}},{{"id":"sxf","family":"sxf","read":true,"write":false,"id_rules":2}},{{"id":"jww","family":"jww","read":true,"write":false,"id_rules":1}}]}},"diagnostics":[],"blobs":[]}}"#,
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
            assert!(text(item, "/hash").starts_with("v1:"));
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
                r#"{{"kind":"replace","id":"space/0/h1","hash":"v1:00000000000000000000000000000000","geometry":{geometry}}}"#
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
}
