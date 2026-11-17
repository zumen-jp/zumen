//! 建物の文書の `building_apply` を文書エンジンで当てる経路（`docs/live.md` の「セッションの文書エンジン」）。
//!
//! IFC-SPF と ifcZIP の文書は、最初の `building_apply` で文書エンジン（[`IfcLive`]）を作って持ち続け、変更を
//! 差分で当てる。ifcZIP は中の IFC-SPF で作り、アーカイブには今までの経路と同じく入れ直す。保存したバイト列・
//! 結果・診断は今までの経路と同じで、予算の使い方と止まる所は文書エンジンの仕事に従う。正は建物モデルの文書の
//! ままにし、当てたら、文書エンジンが保存したバイト列から文書を読み直す。

use super::*;
use crate::building::{Live, Prepared, Updated};
use zumen_ifc::{IfcFile, IfcLive, TextEncoding};

/// 文書エンジンを作る。作る途中の診断は返さない。予算か中断で止まったら失敗にし、ほかの理由で作れないか、
/// 今までの経路と同じ読み方でセットとプロパティの名前を比べられなければ `Live::Refused` を返す。
///
/// 文字コードの指定が `auto` で、手がかりから CP932 で読むファイルは、今までの経路が書き戻したファイルの手がかりを
/// 読み直すので、手がかりの文字列を書き換えると読み方が変わる。文書エンジンは開いたときの読み方を保つので作らない。
/// 今までの経路で当てた文書は `Live::Absent` から始まるので、手がかりがなくなれば次の `building_apply` で作る
fn open(file: &IfcFile, cx: &mut Cx<'_>) -> Result<Live, Failed> {
    if file.text_encoding() == TextEncoding::Auto && IfcLive::file_reads_shift_jis(file) {
        return Ok(Live::Refused);
    }
    let mut quiet = Diagnostics::new();
    match IfcLive::from_file(
        file.clone(),
        1,
        &mut Cx::new(cx.budget, &mut quiet, cx.cancel),
    ) {
        Ok(engine) => Ok(Live::Ifc(Box::new(engine))),
        Err(zumen_sdk::Error::Stopped(stop)) => Err(stopped(cx, stop)),
        Err(_) => Ok(Live::Refused),
    }
}

/// 止まった理由の診断を移す。文書エンジンが届けた理由の診断（場所を持つことがある）を移し、なければ足す。
/// ほかの診断（止まる前に断った変更・建物モデルを作り直す途中の診断）は、今までの経路が返さないことがあるので移さない
fn stopped_on_engine(cx: &mut Cx<'_>, reported: &Diagnostics, stop: Stop) -> Failed {
    let reason = stop.to_diagnostic();
    let mut moved = false;
    for diagnostic in reported.iter() {
        if diagnostic.code() == reason.code() {
            cx.report(diagnostic.clone());
            moved = true;
        }
    }
    if !moved {
        cx.report(reason);
    }
    Failed
}

/// 確かめ終えた変更を文書エンジンで当て、書き戻した文書と結果と、書き戻したファイルを返す。
///
/// 文書エンジンで当てない文書と、文書エンジンが断った変更は `None` を返し、今までの経路に任せる。文書エンジンは、
/// 断ったときと止まったときに状態を変えないので、文書に戻して持ち続ける。当てたら、書き戻した文書に移す。
/// 当てたあとの保存・読み直し・ハッシュで止まったときは、文書エンジンを捨てる。どの場合も `model` の内容は変えない
pub(super) fn on_engine(
    model: &mut building::Model,
    prepared: &Prepared,
    engines: bool,
    cx: &mut Cx<'_>,
) -> Result<Option<Updated>, Failed> {
    let Some(file) = &model.file else {
        return Ok(None);
    };
    if !engines {
        return Ok(None);
    }
    let mut live = match std::mem::take(&mut model.live) {
        Live::Absent => open(file, cx)?,
        live => live,
    };
    let Live::Ifc(engine) = &mut live else {
        model.live = live;
        return Ok(None);
    };
    // 断る診断は、今までの経路で当て直して返す。建物モデルを作り直す途中の診断は、今までの経路が返さないので捨てる
    let mut reported = Diagnostics::new();
    let outcome = engine.apply(
        &prepared.edits,
        &model.hash.0,
        &mut Cx::new(cx.budget, &mut reported, cx.cancel),
    );
    match outcome {
        Ok(_) => {}
        Err(zumen_sdk::Error::Stopped(stop)) => {
            model.live = live;
            return Err(stopped_on_engine(cx, &reported, stop));
        }
        Err(_) => {
            model.live = live;
            return Ok(None);
        }
    }
    // 取り消しは、前の版のバイト列を開き直す今までの経路のまま。記録は持たない
    engine.clear_history();
    // ここからは、文書エンジンが文書より先に進んでいる。失敗したら文書エンジンを捨てる（`model.live` は空のまま）
    let spf = engine.save_spf(cx).map_err(|_| Failed)?;
    let (mut updated, result, bytes) = building::finish(model, spf, prepared, cx)?;
    updated.live = live;
    Ok(Some((updated, result, bytes)))
}
