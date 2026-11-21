//! IFC-SPF の壁の説明を編集し、保持する属性・診断・依存先と保存を照合する。
//! 各実行は 1 ファイル・1 スレッド設定。時間の単位はナノ秒。

use std::{env, fs, process::ExitCode, time::Instant};
use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind};
use zumen_building::Building;
use zumen_ifc::{Edit, Ifc, IfcLive, RootAttribute, apply};
use zumen_sdk::{Codec, Error, Lift};

struct Measured<T> {
    value: T,
    diagnostics: Vec<Diagnostic>,
    nanos: u128,
    decoded_bytes: u64,
}

fn measure<T>(
    operation: &str,
    f: impl FnOnce(&mut Cx) -> Result<T, Error>,
) -> Result<Measured<T>, String> {
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    #[allow(
        clippy::disallowed_methods,
        reason = "計測用の実行入口で経過時間を測る"
    )]
    let start = Instant::now();
    let result = f(&mut cx);
    let nanos = start.elapsed().as_nanos();
    let value = result.map_err(|error| format!("{operation}: {error:?}"))?;
    Ok(Measured {
        value,
        diagnostics: diagnostics.into_vec(),
        nanos,
        decoded_bytes: budget.used(Kind::DecodedBytes),
    })
}

fn check_model(live: &IfcLive, expected: &[u8], geometry: bool) -> Result<(), String> {
    let file = measure("read", |cx| Ifc::read(expected, cx))?.value;
    let model: Measured<Building> = measure("lift", |cx| {
        if geometry {
            file.lift_geometry(cx)
        } else {
            file.lift(cx)
        }
    })?;
    let reported = measure("live.report", |cx| live.report_lift_diagnostics(cx))?;
    if live.building() != &model.value || reported.diagnostics != model.diagnostics {
        return Err("保持する建物モデルか変換の診断が逐次実装と違う".into());
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args().skip(1).collect();
    let (path, threads, geometry) = match args.as_slice() {
        [path, threads] => (path, threads, false),
        [path, threads, flag] if flag == "--geometry" => (path, threads, true),
        _ => return Err("使い方: ifc-live <IFC-SPF> <1 または 4> [--geometry]".into()),
    };
    let threads = match threads.as_str() {
        "1" => 1,
        "4" => 4,
        _ => return Err("スレッド数は 1 または 4 を指定する".into()),
    };
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let base = measure("read", |cx| Ifc::read(&bytes, cx))?.value;
    let open = |input: &[u8], cx: &mut Cx| {
        if geometry {
            IfcLive::open_with_geometry(input, threads, cx)
        } else {
            IfcLive::open_with_threads(input, threads, cx)
        }
    };
    let opened = measure("live.open", |cx| open(&bytes, cx))?;
    let mut live = opened.value;
    let building = live.building();
    let count = building.elements().len();
    let classes = &building.components.class;
    let root = classes
        .iter()
        .find(|(_, class)| class.name == "IfcBuilding")
        .map(|(element, _)| element)
        .ok_or("対象の建物がない")?;
    let wall = classes
        .iter()
        .find(|(_, class)| {
            matches!(
                class.name.as_str(),
                "IfcWall" | "IfcWallStandardCase" | "IfcWallElementedCase"
            )
        })
        .map(|(element, _)| element)
        .ok_or("対象の壁がない")?;
    // どちらのモードも同じ壁を編集して比べる。
    if geometry && building.geometry(wall).is_none() {
        return Err("比較に使う最初の壁に形状がない".into());
    }
    let instance = building
        .element(wall)
        .and_then(|element| element.source().instance)
        .ok_or("壁のインスタンス番号がない")?;
    let edits = [Edit::Attribute {
        element: instance,
        attribute: RootAttribute::Description,
        value: Some("Live benchmark".into()),
    }];
    let expected = measure("apply", |cx| apply(&base, &edits, b"live-bench", cx))?;
    let affected =
        |current: &IfcLive| measure("affected", |cx| current.affected_elements(&[root], cx));
    let before = affected(&live)?;
    let edited = measure("live.apply", |cx| live.apply(&edits, b"live-bench", cx))?;
    let saved = measure("live.save", |cx| live.save_spf(cx))?;
    if edited.value != 1
        || expected.value.changed != 1
        || edited.diagnostics != expected.diagnostics
        || saved.value != expected.value.bytes
    {
        return Err("変更件数・診断・保存バイト列のいずれかが期待値と違う".into());
    }
    check_model(&live, &expected.value.bytes, geometry)?;
    let reopened = measure("live.reopen", |cx| open(&expected.value.bytes, cx))?;
    let after = affected(&reopened.value)?.value;
    if affected(&live)?.value != after {
        return Err("編集後の依存先が再読と違う".into());
    }
    let undone = measure("live.undo", |cx| live.undo(cx))?;
    if !undone.value
        || measure("live.save", |cx| live.save_spf(cx))?.value != bytes
        || affected(&live)?.value != before.value
    {
        return Err("取り消し後の保存または依存先が初期状態と違う".into());
    }
    check_model(&live, &bytes, geometry)?;
    let redone = measure("live.redo", |cx| live.redo(cx))?;
    if !redone.value
        || measure("live.save", |cx| live.save_spf(cx))?.value != expected.value.bytes
        || affected(&live)?.value != after
    {
        return Err("やり直し後の保存または依存先が編集後と違う".into());
    }
    check_model(&live, &expected.value.bytes, geometry)?;
    println!(
        "bytes\trecords\telements\tthreads\taffected\topen_ns\tbatch_ns\tapply_ns\tsave_ns\tundo_ns\tredo_ns\tdeps_ns\topen_decoded_bytes\tapply_decoded_bytes\tsave_decoded_bytes\tundo_decoded_bytes\tredo_decoded_bytes"
    );
    println!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        bytes.len(),
        base.records().len(),
        count,
        threads,
        before.value.len(),
        opened.nanos,
        expected.nanos,
        edited.nanos,
        saved.nanos,
        undone.nanos,
        redone.nanos,
        before.nanos,
        opened.decoded_bytes,
        edited.decoded_bytes,
        saved.decoded_bytes,
        undone.decoded_bytes,
        redone.decoded_bytes
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
