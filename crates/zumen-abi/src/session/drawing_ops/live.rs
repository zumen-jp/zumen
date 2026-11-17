//! 文書エンジンで `apply` を当てる経路（`docs/live.md` の「セッションの文書エンジン」）。
//!
//! DXF（ASCII）・DWG（R2007 を除く）・JWW・SXF の文書は、最初の `apply` で文書エンジン（[`AcadLive`]・
//! [`JwwLive`]・[`SxfLive`]）を作って持ち続け、変更を差分で当てる。保存したバイト列・適用の結果・診断は
//! 今までの経路と同じで、予算の使い方と止まる所は文書エンジンの仕事に従う。正はファイルモデルのままにし、
//! 当てたら、文書エンジンが保存したバイト列からファイルモデルを読み直す。

use super::*;
use crate::packed::Packed;
use crate::session::resident::Live;
use zumen_acad::{AcadLive, LiveFormat};
use zumen_drawing::Changes;
use zumen_jww::JwwLive;
use zumen_sxf::{SxfFile, SxfLive};

/// 読み直しの診断は返さない。止まったときと、読めなかったときだけ、その理由を返す
fn quietly<T>(
    cx: &mut Cx<'_>,
    operation: impl FnOnce(&mut Cx<'_>) -> Result<T, zumen_sdk::Error>,
) -> Result<T, Failed> {
    let mut quiet = Diagnostics::new();
    match operation(&mut Cx::new(cx.budget, &mut quiet, cx.cancel)) {
        Ok(value) => Ok(value),
        Err(zumen_sdk::Error::Stopped(stop)) => Err(stopped(cx, stop)),
        Err(_) => {
            for diagnostic in quiet.iter() {
                cx.report(diagnostic.clone());
            }
            Err(Failed)
        }
    }
}

/// 別に集めた診断を、届いた順に移す
fn moved(cx: &mut Cx<'_>, diagnostics: &Diagnostics) {
    for diagnostic in diagnostics.iter() {
        cx.report(diagnostic.clone());
    }
}

/// 文書エンジンか今までの経路が失敗したとき。止まった理由の診断を足していなければ足す。
/// DWG の差分の経路と、SXF の今までの経路の変換には、止まった理由を足さずに返す所がある
pub(super) fn engine_failed(cx: &mut Cx<'_>, error: zumen_sdk::Error) -> Failed {
    if let zumen_sdk::Error::Stopped(stop) = error {
        let diagnostic = stop.to_diagnostic();
        if !cx
            .diag
            .iter()
            .any(|reported| reported.code() == diagnostic.code())
        {
            cx.report(diagnostic);
        }
    }
    Failed
}

/// 文書エンジンで当てる文書か。バイナリの DXF は差分の経路を持たず、R2007 の DWG は書き戻せないので、今までの経路に任せる
fn served(file: &DrawingFile) -> bool {
    match file {
        DrawingFile::Dxf(file) => !file.is_binary(),
        DrawingFile::Dwg(file) => file.version() != b"AC1021",
        DrawingFile::Jww(_) | DrawingFile::Sxf(..) => true,
        _ => false,
    }
}

/// 変更の一覧が持つ文書のハッシュを、文書エンジンに渡す前に確かめられるか。
///
/// 今と違うハッシュは、今までの経路に任せて同じ診断で断る。SXF のアーカイブ（P2Z・SFZ）の今までの経路は、
/// アーカイブではなく中の SXF のバイト列のハッシュと比べるので、ハッシュを持つ要求は今までの経路に任せる
fn checked(file: &DrawingFile, document: Option<Hash>, current: Hash) -> bool {
    match document {
        None => true,
        Some(hash) => hash == current && !matches!(file, DrawingFile::Sxf(_, Some(_), _)),
    }
}

/// ファイルモデルから文書エンジンを作る。作る途中の診断は返さない。予算か中断で止まったら失敗にし、
/// ほかの理由で作れなければ `Live::Refused` を返す
fn open(file: &DrawingFile, cx: &mut Cx<'_>) -> Result<Live, Failed> {
    let acad = |live: AcadLive| match live.format() {
        LiveFormat::AsciiDxf | LiveFormat::Dwg => Live::Acad(Box::new(live)),
        _ => Live::Refused,
    };
    let mut quiet = Diagnostics::new();
    let mut inner = Cx::new(cx.budget, &mut quiet, cx.cancel);
    let opened = match file {
        DrawingFile::Dxf(file) => {
            // ASCII の DXF は、バイト列から開くと差分の経路を使える
            let mut bytes = Vec::new();
            Dxf::write(file, &mut bytes, &mut inner)
                .and_then(|()| AcadLive::open_dxf(&bytes, &mut inner))
                .map(acad)
        }
        DrawingFile::Dwg(file) => file
            .snapshot(&mut inner)
            .and_then(|file| AcadLive::open_from_dwg_file(file, &mut inner))
            .map(acad),
        DrawingFile::Jww(file) => {
            JwwLive::from_file(file, &mut inner).map(|live| Live::Jww(Box::new(live)))
        }
        // 今までの経路と同じく、属性ファイル（SAF）を結ばない図面モデルで要素のハッシュを比べる
        DrawingFile::Sxf(file, ..) => {
            SxfLive::from_file(file, &mut inner).map(|live| Live::Sxf(Box::new(live)))
        }
        _ => return Ok(Live::Refused),
    };
    match opened {
        Ok(live) => Ok(live),
        Err(zumen_sdk::Error::Stopped(stop)) => Err(stopped(cx, stop)),
        Err(_) => Ok(Live::Refused),
    }
}

/// 文書エンジンで当てられなかったとき
enum Fault {
    /// 断ったか止まった。文書エンジンは状態を変えないので持ち続ける
    Kept,
    /// 当てたあとの保存・ハッシュ・読み直しで失敗した。文書エンジンは当てたあとの状態なので捨てる
    Lost,
    /// 文書エンジンが断った。診断は返さずに、文書エンジンを持ち続けて今までの経路で当て直す
    Retry,
}

impl Fault {
    /// 文書エンジンが断ったか止まった。文書エンジンは状態を変えない
    fn refused(cx: &mut Cx<'_>, error: zumen_sdk::Error) -> Self {
        engine_failed(cx, error);
        Self::Kept
    }

    /// 当てたあとで止まった
    fn lost(cx: &mut Cx<'_>, error: zumen_sdk::Error) -> Self {
        engine_failed(cx, error);
        Self::Lost
    }
}

/// JWW・SXF の文書エンジンの `apply`。診断は別に集め、当てたときと止まったときだけ移す。
///
/// 文書エンジンが断る変更には、今までの経路なら当てられるもの（SXF の文字の要素の移動・置き換え、
/// 表の変更など）と、診断の場所が今までの経路と違うものがある。そこで、断ったときは診断を捨て、
/// 今までの経路で当て直す。文書エンジンは断ったときに状態を変えない
fn tried<T>(
    cx: &mut Cx<'_>,
    apply: impl FnOnce(&mut Cx<'_>) -> Result<T, zumen_sdk::Error>,
) -> Result<T, Fault> {
    let mut reported = Diagnostics::with_max_places(cx.diag.max_places());
    match apply(&mut Cx::new(cx.budget, &mut reported, cx.cancel)) {
        Ok(value) => {
            moved(cx, &reported);
            Ok(value)
        }
        Err(error @ zumen_sdk::Error::Stopped(_)) => {
            moved(cx, &reported);
            Err(Fault::refused(cx, error))
        }
        Err(_) => Err(Fault::Retry),
    }
}

/// 読み直したファイルモデル。SXF は、アーカイブと属性ファイル（SAF）を残して、中のファイルだけを置き換える
enum Reread {
    Whole(Box<DrawingFile>),
    Sxf(SxfFile),
}

impl Reread {
    fn put(self, model: &mut DrawingFile) {
        match (self, model) {
            (Self::Sxf(file), DrawingFile::Sxf(current, ..)) => *current = file,
            (Self::Sxf(file), model) => *model = DrawingFile::Sxf(file, None, None),
            (Self::Whole(file), model) => *model = *file,
        }
    }
}

/// 文書エンジンで当て、保存したバイト列と、そこから読み直したファイルモデルを返す
struct Done {
    applied: Vec<Applied>,
    bytes: Vec<u8>,
    hash: Hash,
    file: Reread,
    lifted: Option<(Drawing2D, zumen_base::Replay)>,
    ids_shifted: bool,
}

impl Session {
    /// 名前の文書の文書エンジンを受け取る。ない文書なら `Live::Absent`
    pub(super) fn take_live(&mut self, doc: &str) -> Live {
        match self.docs.get_mut(doc) {
            Some(Document::Drawing { file, .. }) => file.take_live(),
            _ => Live::Absent,
        }
    }

    /// 文書を変えなかったときに、受け取った文書エンジンを戻す
    pub(super) fn put_live(&mut self, doc: &str, live: Live) {
        if let Some(Document::Drawing { file, .. }) = self.docs.get_mut(doc) {
            file.put_live(live);
        }
    }

    /// 文書エンジンで変更を当てる。当てない文書と、JWW・SXF の文書エンジンが断った変更は、変更をそのまま
    /// `Err` で返し、今までの経路に任せる。
    ///
    /// 変更の一覧が文書のハッシュを持ち、今のハッシュと違うときも、今までの経路に任せる（同じ診断で断る）。
    /// 同じときは、確かめ済みとしてハッシュを外して当てる。文書エンジンが断ったときと止まったときは、文書を変えない
    pub(super) fn apply_on_engine(
        &mut self,
        request: &ApplyRequest,
        drawing: &Drawing2D,
        mut changes: Changes,
        cx: &mut Cx<'_>,
    ) -> Result<Result<ApplyResult, Changes>, Failed> {
        let document = changes.document;
        let Some(Document::Drawing {
            file,
            hash: current,
        }) = self.docs.get_mut(&request.doc)
        else {
            return Ok(Err(changes));
        };
        #[cfg(test)]
        if !self.engines {
            return Ok(Err(changes));
        }
        if !served(file) || !checked(file, changes.document, *current) {
            return Ok(Err(changes));
        }
        let mut live = match file.take_live() {
            Live::Absent => open(file, cx)?,
            live => live,
        };
        changes.document = None;
        let result = match &mut live {
            Live::Acad(engine) => on_acad(engine, file, drawing, &changes, cx),
            Live::Jww(engine) => on_jww(engine, &changes, cx),
            Live::Sxf(engine) => {
                let archive = match &**file {
                    DrawingFile::Sxf(_, archive, _) => archive.as_deref(),
                    _ => None,
                };
                on_sxf(engine, archive, &changes, cx)
            }
            Live::Absent | Live::Refused => {
                file.put_live(live);
                return Ok(Err(changes));
            }
        };
        // ファイルモデルは変わらない。文書エンジンが当てたあとで失敗したときは、文書エンジンを捨てる
        let done = match result {
            Ok(done) => done,
            Err(Fault::Kept) => {
                file.put_live(live);
                return Err(Failed);
            }
            Err(Fault::Lost) => return Err(Failed),
            Err(Fault::Retry) => {
                file.put_live(live);
                changes.document = document;
                return Ok(Err(changes));
            }
        };
        match &mut live {
            Live::Acad(engine) => engine.clear_history(),
            Live::Jww(engine) => engine.clear_history(),
            Live::Sxf(engine) => engine.clear_history(),
            Live::Absent | Live::Refused => {}
        }
        let Done {
            applied,
            bytes,
            hash,
            file: reread,
            lifted,
            ids_shifted,
        } = done;
        file.commit_live(|model| reread.put(model), lifted, live);
        *current = hash;
        #[cfg(test)]
        {
            self.engine_applies += 1;
        }
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes,
        }));
        Ok(Ok(ApplyResult {
            applied: applied
                .into_iter()
                .map(|Applied { id, hash }| messages::Applied {
                    id,
                    hash: hash.map(|hash| hash.to_string()),
                })
                .collect(),
            hash: hash.to_string(),
            ids_shifted: ids_shifted.then_some(true),
        }))
    }
}

/// DXF・DWG を [`AcadLive`] で当てる
fn on_acad(
    engine: &mut AcadLive,
    file: &DrawingFile,
    drawing: &Drawing2D,
    changes: &Changes,
    cx: &mut Cx<'_>,
) -> Result<Done, Fault> {
    // DWG の今までの経路は、当てる前にオブジェクトの表を作り、型どおりに読めないオブジェクトの診断を返す。
    // 文書エンジンは、読み直して参照の経路で当てたときだけ同じ診断を返す。そこで、同じ表を先に作って診断を
    // 別に集め、文書エンジンの診断も別に集める。参照の経路に入らなかったときだけ、表の診断を前に足す。
    // apply の診断は届いた順にまとめるので、まとめたものを移しても件数・場所・順は 1 件ずつ足したときと同じ
    let max_places = cx.diag.max_places();
    let mut objects = Diagnostics::with_max_places(max_places);
    if let DrawingFile::Dwg(dwg) = file
        && dwg
            .objects(&mut Cx::new(cx.budget, &mut objects, cx.cancel))
            .is_err()
    {
        moved(cx, &objects);
        return Err(Fault::Kept);
    }
    let mut reported = Diagnostics::with_max_places(max_places);
    let (applied, rereads) =
        engine.apply_rereading(changes, &mut Cx::new(cx.budget, &mut reported, cx.cancel));
    if !rereads {
        moved(cx, &objects);
    }
    moved(cx, &reported);
    // 文書エンジンは、断ったときと止まったときに状態を変えない
    let applied = applied.map_err(|error| Fault::refused(cx, error))?;
    let keyed = matches!(file, DrawingFile::Dwg(_));
    finish(engine, applied, keyed, drawing, changes, cx).map_err(|Failed| Fault::Lost)
}

/// JWW を [`JwwLive`] で当てる。保存・ハッシュは、今までの経路の書き出し・ハッシュと同じ順にする
fn on_jww(engine: &mut JwwLive, changes: &Changes, cx: &mut Cx<'_>) -> Result<Done, Fault> {
    let (applied, ids_shifted) = tried(cx, |cx| engine.apply(changes, cx))?;
    let bytes = engine.save(cx).map_err(|error| Fault::lost(cx, error))?;
    let hash = Hash::document_checked(&bytes, cx).map_err(|stop| {
        stopped(cx, stop);
        Fault::Lost
    })?;
    let file = quietly(cx, |cx| Jww::read(&bytes, cx)).map_err(|Failed| Fault::Lost)?;
    Ok(Done {
        applied,
        bytes,
        hash,
        file: Reread::Whole(Box::new(DrawingFile::Jww(file))),
        lifted: None,
        ids_shifted,
    })
}

/// SXF を [`SxfLive`] で当てる。P2Z・SFZ は、今までの経路と同じく、保存したバイト列をアーカイブに入れる
fn on_sxf(
    engine: &mut SxfLive,
    archive: Option<&Packed>,
    changes: &Changes,
    cx: &mut Cx<'_>,
) -> Result<Done, Fault> {
    let applied = tried(cx, |cx| engine.apply(changes, cx))?;
    let saved = engine.save(cx).map_err(|error| Fault::lost(cx, error))?;
    let packed = match archive {
        Some(archive) => Some(archive.write(&saved).ok_or_else(|| {
            fail(cx, Diagnostic::new(codes::SXF_ARCHIVE_UNSUPPORTED));
            Fault::Lost
        })?),
        None => None,
    };
    let hash = Hash::document_checked(packed.as_deref().unwrap_or(&saved), cx).map_err(|stop| {
        stopped(cx, stop);
        Fault::Lost
    })?;
    let file = quietly(cx, |cx| Sxf::read(&saved, cx)).map_err(|Failed| Fault::Lost)?;
    Ok(Done {
        applied,
        bytes: packed.unwrap_or(saved),
        hash,
        file: Reread::Sxf(file),
        lifted: None,
        ids_shifted: false,
    })
}

/// 当てたあとの保存・ハッシュ・読み直しと、ハンドルを持たない DXF の ID のずれ（決定 0156、#722）
fn finish(
    engine: &AcadLive,
    applied: Vec<Applied>,
    dwg: bool,
    drawing: &Drawing2D,
    changes: &Changes,
    cx: &mut Cx<'_>,
) -> Result<Done, Failed> {
    let bytes = engine.save(cx).map_err(|error| engine_failed(cx, error))?;
    let hash = Hash::document_checked(&bytes, cx).map_err(|stop| stopped(cx, stop))?;
    let file = if dwg {
        DrawingFile::Dwg(quietly(cx, |cx| Dwg::read(&bytes, cx))?)
    } else {
        DrawingFile::Dxf(quietly(cx, |cx| Dxf::read(&bytes, cx))?)
    };
    let mut lifted = None;
    let mut ids_shifted = false;
    if !dwg && keyless(drawing, cx)? {
        // 作り直しの診断は、書き戻しの診断と重なるので返さない。記録して、次の読み取りで当てる
        let mut quiet = Diagnostics::with_max_places(cx.diag.max_places());
        let (after, replay) = Cx::new(cx.budget, &mut quiet, cx.cancel)
            .record(|cx| file.lift(cx).map(Cow::into_owned));
        let after = after.map_err(|error| match error {
            zumen_sdk::Error::Stopped(stop) => stopped(cx, stop),
            _ => {
                for diagnostic in quiet.iter() {
                    cx.report(diagnostic.clone());
                }
                Failed
            }
        })?;
        ids_shifted = record_ids_shifted(drawing, &after, changes, cx)?;
        lifted = replay.map(|replay| (after, replay));
    }
    Ok(Done {
        applied,
        bytes,
        hash,
        file: Reread::Whole(Box::new(file)),
        lifted,
        ids_shifted,
    })
}
