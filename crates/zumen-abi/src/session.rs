//! セッション: 名前のついた入力を持ち、要求を受けて応答を返す（決定 0012）。

use std::collections::BTreeMap;

use zumen_acad::{Dwg, DwgFile, Dxf, DxfFile};
use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, Stop};
use zumen_base::{codes, name};
use zumen_drawing::{Drawing2D, Element, Placement, Shape, Unit};
use zumen_json::{FromJson, Invalid, MAX_SAFE_INTEGER, Problem, ToJson, Writer};
use zumen_sdk::{Codec, Lift};

use crate::abi::{ABI_VERSION, SCHEMA_MAJOR, SCHEMA_MINOR, Status};
use crate::describe;
use crate::messages::{
    self, BlobKind, CapabilitiesResult, CloseRequest, CloseResult, DrawingRequest, DrawingResult,
    ElementCounts, Engine, Extent, Format, OpenRequest, OpenResult, Request, RequestCommon,
    SchemaVersion, SessionOptions, SheetSummary, SpaceSummary,
};

/// このライブラリの実行の形（決定 0013）
const BUILT: Engine = if cfg!(target_family = "wasm") {
    Engine::Wasm
} else {
    Engine::Native
};

/// 名前のついた入力を持つセッション。
///
/// 1 つのファイルを読むときも、セッションを作り、[`Session::push`] で入力を渡し、`open` の要求を送る。
/// 入力は、ホストが読んで渡す。核はファイルシステムを読まない（決定 0007）。
#[derive(Debug)]
pub struct Session {
    engine: Engine,
    inputs: BTreeMap<String, Vec<u8>>,
    /// `open` で作った文書
    docs: BTreeMap<String, Doc>,
    /// 直前の呼び出しが作ったバイナリの結果。取り出したものは `None` になる
    blobs: Vec<Option<Blob>>,
}

/// 開いた文書。形式ごとのファイルモデル（L1）を持つ
#[derive(Debug)]
enum Doc {
    Dxf(DxfFile),
    Dwg(DwgFile),
}

impl Doc {
    /// 形式の台帳のキー
    fn format(&self) -> &'static str {
        match self {
            Doc::Dxf(_) => Dxf::FORMAT,
            Doc::Dwg(_) => Dwg::FORMAT,
        }
    }

    /// 図面モデル（L2）に変換する
    fn lift(&self, cx: &mut Cx<'_>) -> Result<Drawing2D, zumen_sdk::Error> {
        match self {
            Doc::Dxf(file) => file.lift(cx),
            Doc::Dwg(file) => file.lift(cx),
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

    /// 核の不具合で止まったときの応答。`internal.panic.caught` の診断だけを持つ
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
        }
        for input in inputs {
            self.inputs.remove(input);
        }
        Ok(CloseResult {})
    }

    fn drawing(&self, request: &DrawingRequest, cx: &mut Cx<'_>) -> Result<DrawingResult, Failed> {
        if !name::is_safe(&request.doc) {
            return Err(fail(cx, invalid_value("/doc")));
        }
        let Some(doc) = self.docs.get(&request.doc) else {
            return Err(fail(
                cx,
                Diagnostic::new(codes::CALL_DOCUMENT_MISSING).with("name", request.doc.as_str()),
            ));
        };
        // 止めた理由は、変換が診断として足してある
        let drawing = doc.lift(cx).map_err(|_| Failed)?;
        Ok(summary(&drawing))
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
        ops: Request::TAGS.iter().map(|op| op.to_string()).collect(),
        formats: [Dxf::FORMAT, Dwg::FORMAT]
            .into_iter()
            .map(|id| Format {
                id: id.to_string(),
                family: "acad".to_string(),
                read: true,
                write: false,
                id_rules: 1,
            })
            .collect(),
    }
}

/// 図面モデルの要約。空間と用紙は、モデルの順に並べる
fn summary(drawing: &Drawing2D) -> DrawingResult {
    DrawingResult {
        spaces: drawing
            .spaces
            .iter()
            .map(|space| SpaceSummary {
                unit: unit_of(space.unit),
                elements: counts(&space.elements),
                extent: extent(&space.elements, |p| (p.x, p.y)),
            })
            .collect(),
        sheets: drawing
            .sheets
            .iter()
            .map(|sheet| SheetSummary {
                size: sheet.size.map(|size| size.iter().map(|l| l.0).collect()),
                elements: counts(&sheet.elements),
                views: sheet.views.len() as u64,
                extent: extent(&sheet.elements, |p| (p.x, p.y)),
            })
            .collect(),
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
    let lines = elements
        .iter()
        .filter(|e| matches!(e.shape, Shape::Line { .. }))
        .count() as u64;
    ElementCounts {
        lines,
        texts: elements.len() as u64 - lines,
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

/// 核の診断を、メッセージの診断にする
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

    /// 見出しの単位と用紙の範囲、モデル空間の線分と文字、ペーパー空間の線分、移さない円を持つ DXF
    const DRAWING: &[u8] = b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$INSUNITS\n70\n4\n\
        9\n$PLIMMIN\n10\n0.0\n20\n0.0\n9\n$PLIMMAX\n10\n420.0\n20\n297.0\n0\nENDSEC\n\
        0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n1.5\n20\n2.0\n11\n100.0\n21\n50.0\n\
        0\nTEXT\n8\nTEXT\n10\n-3.0\n20\n4.0\n40\n2.5\n1\nABC\n\
        0\nLINE\n8\n0\n67\n1\n10\n10.0\n20\n10.0\n11\n410.0\n21\n287.0\n\
        0\nCIRCLE\n8\n0\n10\n0.0\n20\n0.0\n40\n1.0\n0\nENDSEC\n0\nEOF\n";

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
            r#"{{"v":1,"result":{{"package":"{}","abi":1,"schema":{{"major":1,"minor":1}},"ops":["capabilities","open","close","describe","drawing"],"formats":[{{"id":"dxf","family":"acad","read":true,"write":false,"id_rules":1}},{{"id":"dwg","family":"acad","read":true,"write":false,"id_rules":1}}]}},"diagnostics":[],"blobs":[]}}"#,
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
        assert_eq!(text(&response, "/result/spaces/0/unit"), "millimeter");
        assert_eq!(number(&response, "/result/spaces/0/elements/lines"), "1");
        assert_eq!(number(&response, "/result/spaces/0/elements/texts"), "1");
        assert_eq!(number(&response, "/result/spaces/0/extent/min_x"), "-3");
        assert_eq!(number(&response, "/result/spaces/0/extent/max_y"), "50");
        assert_eq!(number(&response, "/result/sheets/0/size/0"), "420");
        assert_eq!(number(&response, "/result/sheets/0/elements/lines"), "1");
        assert_eq!(number(&response, "/result/sheets/0/extent/max_x"), "410");
        assert_eq!(text(&response, "/diagnostics/0/code"), "dxf.lift.dropped");
        // 表を作るときに 9 つのレコードごと、変換で 4 つの図形ごとに、作業を 1 つ使う
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"drawing","doc":"a.dxf","budget":{"work":12}}"#,
        );
        assert_eq!(status, Status::Failed);
        assert_eq!(
            text(&response, "/diagnostics/0/code"),
            "budget.work.exceeded"
        );
        assert_eq!(number(&response, "/diagnostics/0/params/requested"), "13");
        let (status, _) = call(
            &mut session,
            r#"{"v":1,"op":"drawing","doc":"a.dxf","budget":{"work":13}}"#,
        );
        assert_eq!(status, Status::Ok);
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
