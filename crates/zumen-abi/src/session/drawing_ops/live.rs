//! 文書エンジンで `apply` を当てる経路（`docs/live.md` の「セッションの文書エンジン」）。
//!
//! DXF（ASCII）と DWG（R2007 を除く）の文書は、最初の `apply` で [`AcadLive`] を作って持ち続け、変更を差分で当てる。
//! 保存したバイト列・適用の結果・診断は今までの経路と同じで、予算の使い方と止まる所は文書エンジンの仕事に従う。
//! 正はファイルモデルのままにし、当てたら、文書エンジンが保存したバイト列からファイルモデルを読み直す。

use super::*;
use crate::session::resident::Live;
use zumen_acad::{AcadLive, LiveFormat};
use zumen_drawing::Changes;

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

/// 文書エンジンが失敗したとき。止まった理由の診断を文書エンジンが足していなければ足す。
/// DWG の差分の経路には、止まった理由を足さずに返す所がある（今までの経路は必ず 1 件足す）
fn engine_failed(cx: &mut Cx<'_>, error: zumen_sdk::Error) -> Failed {
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
        _ => false,
    }
}

/// ファイルモデルから文書エンジンを作る。作る途中の診断は返さない。予算か中断で止まったら失敗にし、
/// ほかの理由で作れなければ `Live::Refused` を返す
fn open(file: &DrawingFile, cx: &mut Cx<'_>) -> Result<Live, Failed> {
    let mut quiet = Diagnostics::new();
    let mut inner = Cx::new(cx.budget, &mut quiet, cx.cancel);
    let opened = match file {
        DrawingFile::Dxf(file) => {
            // ASCII の DXF は、バイト列から開くと差分の経路を使える
            let mut bytes = Vec::new();
            Dxf::write(file, &mut bytes, &mut inner)
                .and_then(|()| AcadLive::open_dxf(&bytes, &mut inner))
        }
        DrawingFile::Dwg(file) => file
            .snapshot(&mut inner)
            .and_then(|file| AcadLive::open_from_dwg_file(file, &mut inner)),
        _ => return Ok(Live::Refused),
    };
    match opened {
        Ok(live) if matches!(live.format(), LiveFormat::AsciiDxf | LiveFormat::Dwg) => {
            Ok(Live::Acad(Box::new(live)))
        }
        Ok(_) => Ok(Live::Refused),
        Err(zumen_sdk::Error::Stopped(stop)) => Err(stopped(cx, stop)),
        Err(_) => Ok(Live::Refused),
    }
}

/// 文書エンジンで当て、保存したバイト列と、そこから読み直したファイルモデルを返す
struct Done {
    applied: Vec<Applied>,
    bytes: Vec<u8>,
    hash: Hash,
    file: DrawingFile,
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

    /// 文書エンジンで変更を当てる。当てない文書なら、変更をそのまま `Err` で返し、今までの経路に任せる。
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
        if !served(file) || changes.document.is_some_and(|hash| hash != *current) {
            return Ok(Err(changes));
        }
        let mut live = match file.take_live() {
            Live::Absent => open(file, cx)?,
            live => live,
        };
        let Live::Acad(engine) = &mut live else {
            file.put_live(live);
            return Ok(Err(changes));
        };
        changes.document = None;
        // DWG の今までの経路は、当てる前にオブジェクトの表を作り、型どおりに読めないオブジェクトの診断を返す。
        // 文書エンジンは、読み直して参照の経路で当てたときだけ同じ診断を返す。そこで、同じ表を先に作って診断を
        // 別に集め、文書エンジンの診断も別に集める。参照の経路に入らなかったときだけ、表の診断を前に足す。
        // apply の診断は届いた順にまとめるので、まとめたものを移しても件数・場所・順は 1 件ずつ足したときと同じ
        let max_places = cx.diag.max_places();
        let mut objects = Diagnostics::with_max_places(max_places);
        if let DrawingFile::Dwg(dwg) = &**file
            && dwg
                .objects(&mut Cx::new(cx.budget, &mut objects, cx.cancel))
                .is_err()
        {
            moved(cx, &objects);
            file.put_live(live);
            return Err(Failed);
        }
        let mut reported = Diagnostics::with_max_places(max_places);
        let (applied, rereads) =
            engine.apply_rereading(&changes, &mut Cx::new(cx.budget, &mut reported, cx.cancel));
        if !rereads {
            moved(cx, &objects);
        }
        moved(cx, &reported);
        let applied = match applied {
            Ok(applied) => applied,
            Err(error) => {
                // 文書エンジンは、断ったときと止まったときに状態を変えない
                file.put_live(live);
                return Err(engine_failed(cx, error));
            }
        };
        // ここから先で失敗したら、文書エンジンを捨てる。ファイルモデルは変わらない
        let keyed = matches!(**file, DrawingFile::Dwg(_));
        let done = finish(engine, applied, keyed, drawing, &changes, cx)?;
        engine.clear_history();
        file.commit_live(done.file, done.lifted, live);
        *current = done.hash;
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes: done.bytes,
        }));
        Ok(Ok(ApplyResult {
            applied: done
                .applied
                .into_iter()
                .map(|Applied { id, hash }| messages::Applied {
                    id,
                    hash: hash.map(|hash| hash.to_string()),
                })
                .collect(),
            hash: done.hash.to_string(),
            ids_shifted: done.ids_shifted.then_some(true),
        }))
    }
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
        file,
        lifted,
        ids_shifted,
    })
}
