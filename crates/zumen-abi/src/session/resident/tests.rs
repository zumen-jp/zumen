//! 常駐の図面モデルを使っても、毎回変換し直したときと同じ予算・診断・止まる所・応答になることを確かめる。

use super::*;
use crate::abi::Status;
use crate::session::{Document, Session};
use zumen_base::{Budget, CancelToken, Diagnostic, Diagnostics, Kind, Limit, Limits};
use zumen_json::Value;

/// 形式ごとの公開サンプル。R12 の DXF はハンドルを持たず、ID がレコードの番号で決まる
const SAMPLES: [(&str, &[u8]); 5] = [
    (
        "a.dxf",
        include_bytes!("../../../../../samples/dxf/text_alignments.dxf"),
    ),
    (
        "r12.dxf",
        include_bytes!("../../../../../samples/dxf/ezdxf-ascii-r12.dxf"),
    ),
    (
        "a.dwg",
        include_bytes!("../../../../../samples/dwg/zumen-layout-r2000.dwg"),
    ),
    (
        "a.jww",
        include_bytes!("../../../../../samples/jww/ezjww-geometry.jww"),
    ),
    (
        "a.sfc",
        include_bytes!("../../../../../samples/sxf/ezsxf-writer-all-features.sfc"),
    ),
];

fn call(session: &mut Session, request: &str) -> (Status, Vec<u8>) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    (reply.status, reply.response)
}

fn opened(name: &str, bytes: &[u8]) -> Session {
    let mut session = Session::new(b"").unwrap();
    session.push(name.as_bytes(), bytes).unwrap();
    let (status, response) = call(
        &mut session,
        &format!(r#"{{"v":1,"op":"open","input":"{name}"}}"#),
    );
    assert_eq!(status, Status::Ok, "{}", String::from_utf8_lossy(&response));
    session
}

fn resident<'a>(session: &'a Session, name: &str) -> &'a Resident {
    match session.docs.get(name) {
        Some(Document::Drawing { file, .. }) => file,
        _ => panic!("{name}"),
    }
}

/// 変換の結果と、使った予算と、届いた診断
type Outcome = (Result<Drawing2D, String>, Budget, Vec<Diagnostic>);

fn outcome<'d>(
    limits: &Limits,
    max_places: usize,
    cancel: &CancelToken,
    lift: impl FnOnce(&mut Cx<'_>) -> Result<Cow<'d, Drawing2D>, zumen_sdk::Error>,
) -> Outcome {
    let mut budget = Budget::new(*limits);
    let mut diag = Diagnostics::with_max_places(max_places);
    let result = lift(&mut Cx::new(&mut budget, &mut diag, cancel))
        .map(Cow::into_owned)
        .map_err(|error| format!("{error:?}"));
    (result, budget, diag.into_vec())
}

/// 止まる所の前後を含む上限の組。単位ごとに、変換が使う量の前後と途中を試す
fn limit_sets(spent: &[u64; Kind::ALL.len()]) -> Vec<Limits> {
    let mut sets = vec![Limits::default()];
    for (kind, &spent) in Kind::ALL.iter().zip(spent) {
        let values: Vec<u64> = if *kind == Kind::Depth {
            (0..6).collect()
        } else if spent == 0 {
            continue;
        } else {
            [0, 1, spent / 2, spent.saturating_sub(1), spent, spent + 1].to_vec()
        };
        for value in values {
            sets.push(Limits::default().with(*kind, Limit::Finite(value)));
        }
    }
    sets.push(Limits::default().with(Kind::Depth, Limit::Unlimited));
    sets
}

#[test]
fn resident_lift_matches_converting_again_at_every_stop_point() {
    let cancel = CancelToken::new();
    for (name, bytes) in SAMPLES {
        let warm = opened(name, bytes);
        let file = resident(&warm, name);
        // 常駐させる。既定の予算と場所の数で記録する
        let first = outcome(&Limits::default(), 16, &cancel, |cx| file.lift(cx));
        assert!(first.0.is_ok(), "{name}");
        assert!(file.lifted.get().is_some(), "{name}");
        let spent = Kind::ALL.map(|kind| first.1.used(kind));
        for limits in limit_sets(&spent) {
            for max_places in [0, 1, 16] {
                let expected = outcome(&limits, max_places, &cancel, |cx| {
                    file.file(cx).and_then(|file| file.lift(cx))
                });
                let replayed = outcome(&limits, max_places, &cancel, |cx| file.lift(cx));
                assert_eq!(replayed, expected, "{name} {limits:?} {max_places}");
                // 常駐させる前の、最初の変換の記録も同じになる
                let cold = opened(name, bytes);
                let recorded = outcome(&limits, max_places, &cancel, |cx| {
                    resident(&cold, name).lift(cx)
                });
                assert_eq!(recorded, expected, "{name} {limits:?} {max_places}");
            }
        }
        // 中断の合図があれば、記録を当てずに変換し、同じ所で止まる
        let cancelled = CancelToken::new();
        cancelled.cancel();
        let expected = outcome(&Limits::default(), 16, &cancelled, |cx| {
            file.file(cx).and_then(|file| file.lift(cx))
        });
        assert!(expected.0.is_err());
        let replayed = outcome(&Limits::default(), 16, &cancelled, |cx| file.lift(cx));
        assert_eq!(replayed, expected, "{name}");
    }
}

/// 図面モデルを使う読み取りの要求。予算を `budget` にする
fn reads(doc: &str, budget: &str) -> Vec<String> {
    [
        r#""op":"drawing""#,
        r#""op":"entities","fields":["kind","layer","source","bbox","hash","geometry","style"],"limit":7"#,
        r#""op":"entities","region":[0,0,100,100],"offset":1"#,
        r#""op":"tables""#,
        r#""op":"scene","from":"space/0","tolerance":0.5"#,
        r#""op":"lift""#,
    ]
    .iter()
    .map(|op| format!(r#"{{"v":1,{op},"doc":"{doc}"{budget}}}"#))
    .collect()
}

/// 応答と、取り出せるバイナリの結果
fn answer(session: &mut Session, request: &str) -> (Status, Vec<u8>, Vec<Vec<u8>>) {
    let (status, response) = call(session, request);
    let blobs = (0..4).filter_map(|blob| session.take(blob)).collect();
    (status, response, blobs)
}

#[test]
fn read_ops_on_the_resident_model_answer_like_a_fresh_session() {
    let cancel = CancelToken::new();
    for (name, bytes) in SAMPLES {
        let probe = opened(name, bytes);
        let work = outcome(&Limits::default(), 16, &cancel, |cx| {
            resident(&probe, name)
                .file(cx)
                .and_then(|file| file.lift(cx))
        })
        .1
        .used(Kind::Work);
        let mut budgets = vec![String::new(), r#","max_places":0"#.to_string()];
        for work in [work.saturating_sub(1), work, work + 1, work + 200] {
            budgets.push(format!(r#","budget":{{"work":{work}}}"#));
        }
        let mut warm = opened(name, bytes);
        for budget in &budgets {
            for request in reads(name, budget) {
                // 新しいセッションの最初の要求と、常駐させたあとの要求を比べる
                let mut cold = opened(name, bytes);
                let expected = answer(&mut cold, &request);
                answer(
                    &mut warm,
                    &format!(r#"{{"v":1,"op":"drawing","doc":"{name}"}}"#),
                );
                assert!(resident(&warm, name).lifted.get().is_some(), "{name}");
                let actual = answer(&mut warm, &request);
                assert_eq!(
                    actual,
                    expected,
                    "{request}\n{}\n{}",
                    String::from_utf8_lossy(&actual.1),
                    String::from_utf8_lossy(&expected.1)
                );
            }
        }
    }
}

/// 常駐の図面モデルを捨て、次の読み取りで変換し直させる。文書エンジンは持ち続ける
fn forget(session: &mut Session, name: &str) {
    if let Some(Document::Drawing { file, .. }) = session.docs.get_mut(name) {
        file.lifted = OnceCell::new();
    }
}

/// 常駐の図面モデルと文書エンジンを捨てる
fn forget_engine(session: &mut Session, name: &str) {
    if let Some(Document::Drawing { file, .. }) = session.docs.get_mut(name) {
        let mut budget = Budget::new(Limits::default());
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let _ = file.file_mut(&mut Cx::new(&mut budget, &mut diag, &cancel));
    }
}

/// 常駐の図面モデルを持たない状態で答える
fn answer_cold(
    session: &mut Session,
    request: &str,
    name: &str,
) -> (Status, Vec<u8>, Vec<Vec<u8>>) {
    forget(session, name);
    answer(session, request)
}

/// 文書の要素の ID とハッシュ。初めから 50 件まで
fn elements(session: &mut Session, doc: &str) -> Vec<(String, String)> {
    let (_, response) = call(
        session,
        &format!(r#"{{"v":1,"op":"entities","doc":"{doc}","fields":["hash"],"limit":50}}"#),
    );
    let value = zumen_json::parse(&response).unwrap();
    (0..50)
        .map_while(|index| {
            match (
                value.pointer(&format!("/result/items/{index}/id")),
                value.pointer(&format!("/result/items/{index}/hash")),
            ) {
                (Some(Value::String(id)), Some(Value::String(hash))) => {
                    Some((id.clone(), hash.clone()))
                }
                _ => None,
            }
        })
        .collect()
}

/// 要素を除く変更
fn remove(doc: &str, budget: &str, id: &str, hash: &str) -> String {
    format!(
        r#"{{"v":1,"op":"apply","doc":"{doc}"{budget},"edits":[{{"kind":"remove","id":"{id}","hash":"{hash}"}}]}}"#
    )
}

#[test]
fn apply_moves_the_resident_model_and_keeps_it_only_when_nothing_changed() {
    let mut edited = Vec::new();
    for (name, bytes) in SAMPLES {
        let mut warm = opened(name, bytes);
        let mut cold = opened(name, bytes);
        // 除ける最初の要素を、別に開いたセッションで探す。除ける要素がないサンプル（R12 の DXF）は除く
        let Some((id, hash)) = elements(&mut warm, name).into_iter().find(|(id, hash)| {
            call(&mut opened(name, bytes), &remove(name, "", id, hash)).0 == Status::Ok
        }) else {
            continue;
        };
        edited.push(name);
        assert!(resident(&warm, name).lifted.get().is_some(), "{name}");
        // 断られる変更（違うハッシュ）は、文書を変えない。常駐の図面モデルは戻る
        let rejected = remove(name, "", &id, "v2:00000000000000000000000000000000");
        let expected = answer_cold(&mut cold, &rejected, name);
        assert_eq!(expected.0, Status::Failed, "{name}");
        assert_eq!(answer(&mut warm, &rejected), expected, "{name}");
        assert!(resident(&warm, name).lifted.get().is_some(), "{name}");
        // 当てた変更は、常駐の図面モデルを捨て、次の読み取りで作り直す
        let removed = remove(name, "", &id, &hash);
        let expected = answer_cold(&mut cold, &removed, name);
        assert_eq!(
            expected.0,
            Status::Ok,
            "{name} {}",
            String::from_utf8_lossy(&expected.1)
        );
        assert_eq!(answer(&mut warm, &removed), expected, "{name}");
        assert!(resident(&warm, name).lifted.get().is_none(), "{name}");
        for request in reads(name, "") {
            assert_eq!(
                answer(&mut warm, &request),
                answer_cold(&mut cold, &request, name),
                "{name} {request}"
            );
        }
        assert!(resident(&warm, name).lifted.get().is_some(), "{name}");
        // 予算で止まる apply も、変換し直す場合と同じ所で止まる。断られる要素も混ぜる
        elements(&mut cold, name);
        for (id, hash) in elements(&mut warm, name).into_iter().take(3) {
            for work in [1, 10, 100, 1000] {
                let limited = remove(name, &format!(r#","budget":{{"work":{work}}}"#), &id, &hash);
                assert_eq!(
                    answer(&mut warm, &limited),
                    answer_cold(&mut cold, &limited, name),
                    "{name} {work}"
                );
            }
        }
    }
    assert_eq!(edited, ["a.dxf", "a.dwg", "a.jww", "a.sfc"]);
}

/// 文書エンジンで当てる形式の公開サンプル。ハンドルのない R12、R2007 の DXF、R2000〜R2018 の DWG を含む
const ACAD_SAMPLES: [(&str, &[u8]); 8] = [
    (
        "a.dxf",
        include_bytes!("../../../../../samples/dxf/text_alignments.dxf"),
    ),
    (
        "r12.dxf",
        include_bytes!("../../../../../samples/dxf/ezdxf-ascii-r12.dxf"),
    ),
    (
        "mtext.dxf",
        include_bytes!("../../../../../samples/dxf/ezdxf-mtext-columns-r2007.dxf"),
    ),
    (
        "layout.dxf",
        include_bytes!("../../../../../samples/dwg/zumen-layout-r2018_ascii.dxf"),
    ),
    (
        "a.dwg",
        include_bytes!("../../../../../samples/dwg/zumen-layout-r2000.dwg"),
    ),
    (
        "r2004.dwg",
        include_bytes!("../../../../../samples/dwg/zumen-minsert-r2004.dwg"),
    ),
    (
        "r2013.dwg",
        include_bytes!("../../../../../samples/dwg/zumen-xclip-r2013.dwg"),
    ),
    (
        "r2018.dwg",
        include_bytes!("../../../../../samples/dwg/zumen-arcdim-r2018.dwg"),
    ),
];

/// 適合テストの `push` の 16 進のバイト列
fn conformance_hex(case: &str, pointer: &str) -> Vec<u8> {
    let text = std::fs::read_to_string(format!(
        "{}/../../bindings/conformance/cases/{case}.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let value = zumen_json::parse(text.as_bytes()).unwrap();
    let Some(Value::String(hex)) = value.pointer(pointer) else {
        panic!("適合テストの {case} に {pointer} がない");
    };
    hex.as_bytes()
        .chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

/// P2Z・SFZ の中の SXF のバイト列
fn unpacked(archive: &[u8]) -> Vec<u8> {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    crate::packed::read(archive, &mut Cx::new(&mut budget, &mut diag, &cancel))
        .ok()
        .flatten()
        .unwrap()
        .0
}

/// 文書エンジンで当てる形式の公開サンプル。DXF・DWG に、JWW、SFC、P21（適合テストの P2Z の中身）、P2Z・SFZ、
/// 属性ファイル（SAF）を持つ P2Z を足す
fn engine_samples() -> Vec<(&'static str, Vec<u8>)> {
    let p2z = conformance_hex("apply_text_p2z", "/steps/0/push/hex");
    let sfz = conformance_hex("apply_text_sfz", "/steps/0/push/hex");
    let mut samples: Vec<_> = ACAD_SAMPLES
        .iter()
        .map(|(name, bytes)| (*name, bytes.to_vec()))
        .collect();
    samples.extend([
        (
            "a.jww",
            include_bytes!("../../../../../samples/jww/ezjww-geometry.jww").to_vec(),
        ),
        (
            "a.sfc",
            include_bytes!("../../../../../samples/sxf/ezsxf-writer-all-features.sfc").to_vec(),
        ),
        ("text.p21", unpacked(&p2z)),
        ("text.sfc", unpacked(&sfz)),
        ("text.p2z", p2z),
        ("text.sfz", sfz),
        (
            "saf.p2z",
            conformance_hex("open_sxf_saf", "/steps/5/push/hex"),
        ),
    ]);
    samples
}

/// 文書エンジンを持ち続けているか
fn engine_kept(session: &Session, name: &str) -> bool {
    matches!(
        resident(session, name).live,
        Live::Acad(_) | Live::Jww(_) | Live::Sxf(_)
    )
}

/// 文書エンジンを使うセッションと、今までの経路だけを使うセッション
fn twins(name: &str, bytes: &[u8]) -> (Session, Session) {
    let engine = opened(name, bytes);
    let mut old = opened(name, bytes);
    old.engines = false;
    (engine, old)
}

/// 両方のセッションに同じ要求を送り、応答とバイナリの結果が同じことを確かめる
fn same(engine: &mut Session, old: &mut Session, request: &str) -> (Status, Vec<u8>) {
    let actual = answer(engine, request);
    let expected = answer(old, request);
    assert_eq!(
        actual,
        expected,
        "{request}\n{}\n{}",
        String::from_utf8_lossy(&actual.1),
        String::from_utf8_lossy(&expected.1)
    );
    (actual.0, actual.1)
}

/// 応答の文書のハッシュ
fn result_hash(response: &[u8]) -> Option<String> {
    match zumen_json::parse(response).ok()?.pointer("/result/hash") {
        Some(Value::String(hash)) => Some(hash.clone()),
        _ => None,
    }
}

/// 止まったときの診断が、予算か中断のものだけか
fn stopped_on_budget(response: &[u8]) -> bool {
    let value = zumen_json::parse(response).unwrap();
    let Some(Value::Array(diagnostics)) = value.pointer("/diagnostics") else {
        return false;
    };
    diagnostics.iter().any(|diagnostic| {
        matches!(diagnostic.pointer("/code"), Some(Value::String(code)) if code.starts_with("budget."))
    })
}

/// 要素ごとに当てる変更の並び。動かす・文書のハッシュを付けて足す・除く・古い文書のハッシュで断られる・違う要素のハッシュで断られる
fn edits(doc: &str, id: &str, hash: &str, document: &str) -> Vec<String> {
    [
        format!(
            r#""edits":[{{"kind":"transform","id":"{id}","hash":"{hash}","matrix":[1,0,5,0,1,7]}}]"#
        ),
        format!(
            r#""hash":"{document}","edits":[{{"kind":"add","place":"space/0","geometry":{{"shape":"line","start":[0,1],"end":[10,1]}}}}]"#
        ),
        format!(r#""edits":[{{"kind":"remove","id":"{id}","hash":"{hash}"}}]"#),
        r#""hash":"v2:00000000000000000000000000000000","edits":[]"#.to_string(),
        format!(r#""edits":[{{"kind":"remove","id":"{id}","hash":"v2:00000000000000000000000000000000"}}]"#),
    ]
    .into_iter()
    .map(|edits| format!(r#"{{"v":1,"op":"apply","doc":"{doc}",{edits}}}"#))
    .collect()
}

#[test]
fn apply_on_the_engine_answers_like_the_file_model() {
    for (name, bytes) in engine_samples() {
        let (mut engine, mut old) = twins(name, &bytes);
        let empty = format!(r#"{{"v":1,"op":"apply","doc":"{name}","edits":[]}}"#);
        let mut document = result_hash(&same(&mut engine, &mut old, &empty).1);
        // 最初の 4 件の要素に、順に変更を当てる。ID・要素のハッシュ・文書のハッシュは、当てるたびに読み直す
        let mut applied = 0;
        for round in 0..4 {
            for step in 0..5 {
                let found = elements(&mut engine, name);
                assert_eq!(found, elements(&mut old, name), "{name}");
                // 要素のない図面（モデル空間が空の R12）は、足すことから始める
                let (id, hash) = match found.get(round) {
                    Some(element) => element.clone(),
                    None if step == 1 => Default::default(),
                    None => continue,
                };
                let request = edits(name, &id, &hash, document.as_deref().unwrap_or_default())
                    .swap_remove(step);
                let (status, response) = same(&mut engine, &mut old, &request);
                if status == Status::Ok {
                    applied += 1;
                    document = result_hash(&response);
                }
                for read in reads(name, "") {
                    same(&mut engine, &mut old, &read);
                }
            }
        }
        assert!(applied > 0, "{name}");
        // 今までの経路に任せずに、文書エンジンで当てた変更がある
        assert!(engine.engine_applies > 0, "{name}");
        // 文書エンジンで当てたあとも、持ち続ける
        assert!(engine_kept(&engine, name), "{name}");
    }
}

#[test]
fn apply_on_the_engine_stops_without_changing_the_document() {
    for (name, bytes) in engine_samples() {
        let (mut engine, mut old) = twins(name, &bytes);
        let transform = |id: &str, hash: &str, budget: &str| {
            format!(
                r#"{{"v":1,"op":"apply","doc":"{name}"{budget},"edits":[{{"kind":"transform","id":"{id}","hash":"{hash}","matrix":[1,0,5,0,1,7]}}]}}"#
            )
        };
        // 予算がなければ今までの経路で動かせる最初の要素を動かす（SXF の文字の要素などは動かせない）
        let Some((id, hash)) = elements(&mut engine, name).into_iter().find(|(id, hash)| {
            let (_, mut fresh) = twins(name, &bytes);
            elements(&mut fresh, name);
            answer(&mut fresh, &transform(id, hash, "")).0 == Status::Ok
        }) else {
            continue;
        };
        elements(&mut old, name);
        let moved = |budget: &str| transform(&id, &hash, budget);
        let before: Vec<_> = reads(name, "")
            .iter()
            .map(|read| answer(&mut engine, read))
            .collect();
        let mut stops = 0;
        for kind in [
            "work",
            "decoded_bytes",
            "records",
            "elements",
            "input_bytes",
        ] {
            for limit in [0_u64, 1, 10, 100, 1_000, 10_000, 100_000, 1_000_000] {
                // 文書エンジンを作る前と、作ったあとの両方で止める。要素のハッシュが違う変更は、文書エンジンを作って断る
                for warm in [false, true] {
                    if warm {
                        let rejected = format!(
                            r#"{{"v":1,"op":"apply","doc":"{name}","edits":[{{"kind":"remove","id":"{id}","hash":"v2:00000000000000000000000000000000"}}]}}"#
                        );
                        assert_eq!(answer(&mut engine, &rejected).0, Status::Failed, "{name}");
                        assert!(engine_kept(&engine, name), "{name}");
                    } else {
                        forget_engine(&mut engine, name);
                    }
                    let limited = moved(&format!(r#","budget":{{"{kind}":{limit}}}"#));
                    let (status, response, blobs) = answer(&mut engine, &limited);
                    if status == Status::Ok {
                        // 通ったら、今までの経路と同じ結果になり、文書を戻すために開き直す
                        let expected = answer(&mut old, &moved(""));
                        assert_eq!(
                            (status, &response, &blobs),
                            (expected.0, &expected.1, &expected.2),
                            "{name} {kind} {limit}"
                        );
                        let (fresh_engine, fresh_old) = twins(name, &bytes);
                        engine = fresh_engine;
                        old = fresh_old;
                        elements(&mut engine, name);
                        elements(&mut old, name);
                        continue;
                    }
                    stops += 1;
                    assert!(
                        stopped_on_budget(&response),
                        "{name} {kind} {limit} {}",
                        String::from_utf8_lossy(&response)
                    );
                    assert!(blobs.is_empty(), "{name}");
                    // 止まった apply は文書を変えない
                    let after: Vec<_> = reads(name, "")
                        .iter()
                        .map(|read| answer(&mut engine, read))
                        .collect();
                    assert_eq!(after, before, "{name} {kind} {limit}");
                }
            }
        }
        assert!(stops > 0, "{name}");
        // 止まったあとも、今までの経路と同じ結果で当てられる
        same(&mut engine, &mut old, &moved(""));
        for read in reads(name, "") {
            same(&mut engine, &mut old, &read);
        }
    }
}

/// 型どおりに読めないオブジェクトを持つ DWG（適合テストの `entity_details_formats` の R2000）。今までの経路は、
/// 当てる前に作るオブジェクトの表でも、その診断を返す。文書エンジンで当てても、同じ診断を同じ順で返す
#[test]
fn apply_on_the_engine_reports_unreadable_dwg_objects_like_the_file_model() {
    let (mut engine, mut old) = twins("a", &unreadable_dwg());
    for aci in [3, 4, 5] {
        let found = elements(&mut engine, "a");
        assert_eq!(found, elements(&mut old, "a"));
        let (id, hash) = found[0].clone();
        let request = format!(
            r#"{{"v":1,"op":"apply","doc":"a","edits":[{{"kind":"restyle","id":"{id}","hash":"{hash}","color":{{"aci":{aci}}}}}]}}"#
        );
        let (status, response) = same(&mut engine, &mut old, &request);
        assert_eq!(status, Status::Ok);
        assert!(
            String::from_utf8_lossy(&response)
                .contains(r#""code":"dwg.object.invalid","severity":"warning","count":2"#)
        );
        for read in reads("a", "") {
            same(&mut engine, &mut old, &read);
        }
        // DWG もファイルモデルを読み直さず常駐文書エンジンから直接答える
        assert!(resident(&engine, "a").held().is_none());
    }
    assert!(engine_kept(&engine, "a"));
}

/// 変更の種類ごとの要求。`{id0}`・`{hash0}`・`{id1}`・`{hash1}` は初めから 1・2 番目の要素、`{layer0}`・`{lhash0}` は
/// 最初のレイヤ、`{doc}` は文書のハッシュ、`{hash:ID}` はその ID の要素のハッシュで埋める
const EDIT_KINDS: [&str; 23] = [
    r#""edits":[]"#,
    r#""hash":"{doc}","edits":[]"#,
    r#""edits":[{"kind":"remove","id":"{id0}","hash":"{hash0}"}]"#,
    // 色が変わらない変更と、変わる変更
    r#""edits":[{"kind":"restyle","id":"{id0}","hash":"{hash0}","color":{"aci":2}}]"#,
    r#""edits":[{"kind":"restyle","id":"{id0}","hash":"{hash0}","color":{"aci":5}}]"#,
    r#""edits":[{"kind":"replace","id":"{id0}","hash":"{hash0}","geometry":{"shape":"circle","center":[100,50],"radius":30}}]"#,
    r#""edits":[{"kind":"replace","id":"{id1}","hash":"{hash1}","geometry":{"shape":"circle","center":[100,50],"radius":30}}]"#,
    r#""edits":[{"kind":"transform","id":"{id0}","hash":"{hash0}","matrix":[1,0,5,0,1,7]}]"#,
    r#""edits":[{"kind":"layer","id":"{layer0}","hash":"{lhash0}","color":{"aci":2}}]"#,
    r#""edits":[{"kind":"point_style","mark":"plus","size":1,"size_kind":"length"}]"#,
    r#""edits":[{"kind":"add_layer","name":"N","color":{"aci":3}}]"#,
    // 当てられる変更と、古いハッシュの変更を一緒に送る
    r#""max_places":1,"edits":[{"kind":"restyle","id":"{id0}","hash":"{hash0}","color":{"aci":5}},{"kind":"restyle","id":"{id1}","hash":"v2:00000000000000000000000000000000","color":{"aci":6}}]"#,
    r#""max_places":0,"edits":[{"kind":"remove","id":"{id0}","hash":"{hash0}"},{"kind":"remove","id":"{id1}","hash":"{hash1}"}]"#,
    r#""hash":"{doc}","edits":[{"kind":"add","place":"space/0","geometry":{"shape":"circle","center":[-3,4],"radius":2}}]"#,
    r#""edits":[{"kind":"add","place":"space/9","geometry":{"shape":"line","start":[0,0],"end":[1,1]}}]"#,
    r#""edits":[{"kind":"add","place":"space/0","geometry":{"shape":"line","start":[0,0],"end":[1e9,-2e9]}}]"#,
    // JWW・SXF の色の番号で、色が変わらない変更と変わる変更
    r#""edits":[{"kind":"restyle","id":"{id0}","hash":"{hash0}","color":{"jww":2}}]"#,
    r#""edits":[{"kind":"restyle","id":"{id1}","hash":"{hash1}","color":{"jww":5}}]"#,
    r#""edits":[{"kind":"restyle","id":"{id0}","hash":"{hash0}","color":{"sxf":2}}]"#,
    r#""edits":[{"kind":"restyle","id":"{id1}","hash":"{hash1}","color":{"sxf":5}}]"#,
    // SXF の要素は用紙に置く。足す・除く・動かすを 1 つの一覧で当てる
    r#""edits":[{"kind":"add","place":"sheet/0","geometry":{"shape":"line","start":[0,0],"end":[10,20]}}]"#,
    r#""edits":[{"kind":"transform","id":"{id1}","hash":"{hash1}","matrix":[1,0,-2,0,1,3]},{"kind":"add","place":"sheet/0","geometry":{"shape":"circle","center":[5,5],"radius":2}},{"kind":"add","place":"space/0","geometry":{"shape":"circle","center":[5,5],"radius":2}}]"#,
    r#""edits":[{"kind":"remove","id":"{id0}","hash":"{hash0}"},{"kind":"transform","id":"{id1}","hash":"{hash1}","matrix":[0,-1,0,1,0,0]}]"#,
];

/// 文書のレイヤの ID とハッシュ
fn layers(session: &mut Session, doc: &str) -> Vec<(String, String)> {
    let (_, response) = call(
        session,
        &format!(r#"{{"v":1,"op":"tables","doc":"{doc}"}}"#),
    );
    let value = zumen_json::parse(&response).unwrap();
    (0..)
        .map_while(|index| {
            match (
                value.pointer(&format!("/result/layers/{index}/id")),
                value.pointer(&format!("/result/layers/{index}/hash")),
            ) {
                (Some(Value::String(id)), Some(Value::String(hash))) => {
                    Some((id.clone(), hash.clone()))
                }
                _ => None,
            }
        })
        .collect()
}

/// 要求の雛形を、今の文書の値で埋める。ない要素は、どの要素とも合わない ID とハッシュにする
fn filled(
    found: &[(String, String)],
    layers: &[(String, String)],
    document: &str,
    template: &str,
) -> String {
    const NONE: &str = "v2:00000000000000000000000000000000";
    let pick = |list: &[(String, String)], index: usize| {
        list.get(index)
            .cloned()
            .unwrap_or_else(|| ("space/0/h0".to_owned(), NONE.to_owned()))
    };
    let (id0, hash0) = pick(found, 0);
    let (id1, hash1) = pick(found, 1);
    let (layer0, lhash0) = pick(layers, 0);
    let mut text = template
        .replace("{id0}", &id0)
        .replace("{hash0}", &hash0)
        .replace("{id1}", &id1)
        .replace("{hash1}", &hash1)
        .replace("{layer0}", &layer0)
        .replace("{lhash0}", &lhash0)
        .replace("{doc}", document);
    while let Some((before, rest)) = text.split_once("{hash:") {
        let (id, after) = rest.split_once('}').unwrap();
        let hash = found
            .iter()
            .find(|(found, _)| found == id)
            .map_or(NONE, |(_, hash)| hash.as_str());
        text = format!("{before}{hash}{after}");
    }
    text
}

/// 両方のセッションに雛形の要求を順に送り、応答・バイナリの結果・読み取りの答えが違えば、その説明を足して止める
fn run_both(
    engine: &mut Session,
    old: &mut Session,
    doc: &str,
    document: &mut String,
    templates: &[&str],
    mismatches: &mut Vec<String>,
) -> usize {
    let mut accepted = 0;
    for template in templates {
        let found = elements(engine, doc);
        if found != elements(old, doc) {
            mismatches.push(format!("{doc} の要素が違う（{template} の前）"));
            return accepted;
        }
        let tables = layers(engine, doc);
        let request = format!(
            r#"{{"v":1,"op":"apply","doc":"{doc}",{}}}"#,
            filled(&found, &tables, document, template)
        );
        let actual = answer(engine, &request);
        let expected = answer(old, &request);
        if actual != expected {
            let blobs = |blobs: &[Vec<u8>]| blobs.iter().map(Vec::len).collect::<Vec<_>>();
            mismatches.push(format!(
                "{request}\n  文書エンジン {:?} {} {:?}\n  今までの経路 {:?} {} {:?}",
                actual.0,
                String::from_utf8_lossy(&actual.1),
                blobs(&actual.2),
                expected.0,
                String::from_utf8_lossy(&expected.1),
                blobs(&expected.2),
            ));
            return accepted;
        }
        if expected.0 == Status::Ok
            && let Some(hash) = result_hash(&expected.1)
        {
            accepted += 1;
            *document = hash;
        }
        for read in reads(doc, "") {
            if answer(engine, &read) != answer(old, &read) {
                mismatches.push(format!("{request} のあとの {read} が違う"));
                return accepted;
            }
        }
    }
    accepted
}

/// 適合テストの `entity_details_formats` の、型どおりに読めないオブジェクトを持つ R2000 の DWG
fn unreadable_dwg() -> Vec<u8> {
    conformance_hex("entity_details_formats", "/steps/12/push/hex")
}

/// 同じハンドルの LINE を 2 つ持つ R2000 の DXF。`garbage` なら EOF の後ろに読まないバイトを足す
fn duplicate_handles(garbage: bool) -> Vec<u8> {
    let line = |handle: &str, x: u32| {
        format!(
            "0\nLINE\n5\n{handle}\n8\n0\n10\n{x}.0\n20\n0.0\n30\n0.0\n11\n{x}.5\n21\n1.0\n31\n0.0\n"
        )
    };
    let mut text = format!(
        "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$HANDSEED\n5\n40\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n{}{}{}0\nENDSEC\n0\nEOF\n",
        line("2A", 0),
        line("2A", 2),
        line("2B", 4),
    );
    if garbage {
        text.push_str("not a group code\nxyz\n");
    }
    text.into_bytes()
}

/// ハンドルを持たない R12 の DXF。モデル空間と紙空間（67 が 1）の LINE・CIRCLE を交互に持ち、図面の範囲は空
/// （$EXTMIN が大きく $EXTMAX が小さい）。ID はレコードの番号で、`space/0/r5` はモデル空間の最初の CIRCLE
fn r12_paper() -> Vec<u8> {
    let mut text = String::from(
        "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n9\n$EXTMIN\n10\n1e+20\n20\n1e+20\n30\n1e+20\n9\n$EXTMAX\n10\n-1e+20\n20\n-1e+20\n30\n-1e+20\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n",
    );
    for index in 0..5 {
        let x = index * 10;
        for paper in ["", "67\n1\n"] {
            text.push_str(&format!(
                "0\nLINE\n8\n0\n{paper}10\n{x}.0\n20\n0.0\n30\n0.0\n11\n{x}.5\n21\n2.0\n31\n0.0\n"
            ));
        }
        for paper in ["", "67\n1\n"] {
            text.push_str(&format!(
                "0\nCIRCLE\n8\n0\n{paper}10\n{x}.0\n20\n5.0\n30\n0.0\n40\n1.5\n"
            ));
        }
    }
    text.push_str("0\nENDSEC\n0\nEOF\n");
    text.into_bytes()
}

/// 文書エンジンで当てても、変更の種類ごとに、今までの経路と同じ応答・保存したバイト列・診断（件数・場所・順）を返す。
/// 公開サンプルに加え、型どおりに読めないオブジェクトを持つ DWG、ハンドルが重複する DXF、ハンドルを持たず
/// 紙空間の図形と空の図面の範囲を持つ R12 の DXF を使う。雛形ごとに開き直す場合と、続けて当てる場合の両方を見る
#[test]
fn apply_on_the_engine_answers_like_the_file_model_for_each_edit_kind() {
    let mut documents = engine_samples();
    let samples = documents.len();
    documents.extend([
        (
            "r2000.dxf",
            include_bytes!("../../../../../samples/dwg/zumen-layout-r2000_ascii.dxf").to_vec(),
        ),
        ("unreadable.dwg", unreadable_dwg()),
        ("duplicate.dxf", duplicate_handles(false)),
        ("garbage.dxf", duplicate_handles(true)),
        ("paper.dxf", r12_paper()),
    ]);
    let mut mismatches = Vec::new();
    for (index, (name, bytes)) in documents.iter().enumerate() {
        let start = zumen_drawing::Hash::document(bytes).to_string();
        let (mut accepted, mut served) = (0, 0);
        for template in EDIT_KINDS {
            let (mut engine, mut old) = twins(name, bytes);
            let mut document = start.clone();
            accepted += run_both(
                &mut engine,
                &mut old,
                name,
                &mut document,
                &[template],
                &mut mismatches,
            );
            served += engine.engine_applies;
        }
        let (mut engine, mut old) = twins(name, bytes);
        let mut document = start.clone();
        accepted += run_both(
            &mut engine,
            &mut old,
            name,
            &mut document,
            &EDIT_KINDS,
            &mut mismatches,
        );
        served += engine.engine_applies;
        // どの公開サンプルでも、当てた変更があり、その一部は文書エンジンで当てた
        assert!(index >= samples || (accepted > 0 && served > 0), "{name}");
    }
    // ハンドルを持たない R12 の DXF で、動かす・古いハッシュで断られる・2 つ除く・断られる足す・置き換える、を続ける。
    // 除くと後ろの要素の ID がずれるので、ずれたあとの `space/0/r5` も置き換える
    let paper = r12_paper();
    let (mut engine, mut old) = twins("paper.dxf", &paper);
    let mut document = zumen_drawing::Hash::document(&paper).to_string();
    run_both(
        &mut engine,
        &mut old,
        "paper.dxf",
        &mut document,
        &[
            r#""edits":[{"kind":"transform","id":"space/0/r3","hash":"{hash:space/0/r3}","matrix":[1,0,5,0,1,7]}]"#,
            r#""edits":[{"kind":"remove","id":"space/0/r3","hash":"v2:00000000000000000000000000000000"}]"#,
            r#""max_places":0,"edits":[{"kind":"remove","id":"space/0/r7","hash":"{hash:space/0/r7}"},{"kind":"remove","id":"space/0/r9","hash":"{hash:space/0/r9}"}]"#,
            r#""edits":[{"kind":"add","place":"space/9","geometry":{"shape":"line","start":[0,0],"end":[1,1]}}]"#,
            r#""edits":[{"kind":"add","place":"space/0","geometry":{"shape":"text","content":"漢字","position":[0,0],"height":1,"rotation":0}}]"#,
            r#""edits":[{"kind":"add","place":"space/0","layer":"nope","geometry":{"shape":"line","start":[0,0],"end":[1,1]}}]"#,
            r#""edits":[{"kind":"add","place":"space/0","geometry":{"shape":"circle","center":[0,0],"radius":-1}}]"#,
            r#""edits":[{"kind":"add","place":"space/0","geometry":{"shape":"line","start":[0,0],"end":[1,1]}},{"kind":"remove","id":"{id0}","hash":"v2:00000000000000000000000000000000"}]"#,
            r#""edits":[{"kind":"replace","id":"space/0/r5","hash":"{hash:space/0/r5}","geometry":{"shape":"circle","center":[100,50],"radius":30}}]"#,
            r#""max_places":0,"edits":[{"kind":"remove","id":"space/0/r3","hash":"{hash:space/0/r3}"},{"kind":"remove","id":"space/0/r5","hash":"{hash:space/0/r5}"}]"#,
            r#""edits":[{"kind":"replace","id":"space/0/r5","hash":"{hash:space/0/r5}","geometry":{"shape":"line","start":[1,2],"end":[3,4]}}]"#,
            r#""edits":[{"kind":"transform","id":"space/0/r3","hash":"{hash:space/0/r3}","matrix":[1,0,5,0,1,7]}]"#,
            r#""edits":[{"kind":"remove","id":"space/0/r5","hash":"{hash:space/0/r5}"}]"#,
        ],
        &mut mismatches,
    );
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

/// 属性ファイル（SAF）を結んで開いた SFC（適合テストの `open_sxf_saf`）。文書エンジンは属性ファイルを結ばずに作り、
/// 今までの経路と同じく、結ばない図面モデルで要素のハッシュを比べる。当てたあとも属性ファイルを持ち、読み取りに使う
#[test]
fn apply_on_the_engine_keeps_the_attribute_file() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../bindings/conformance/cases/open_sxf_saf.json"
    ))
    .unwrap();
    let value = zumen_json::parse(text.as_bytes()).unwrap();
    let pushed = |step: usize| match value.pointer(&format!("/steps/{step}/push/text")) {
        Some(Value::String(text)) => text.clone(),
        _ => panic!("適合テストの open_sxf_saf に {step} の push がない"),
    };
    let (sfc, saf) = (pushed(0), pushed(1));
    let open = |engines: bool| {
        let mut session = Session::new(b"").unwrap();
        session.engines = engines;
        session.push(b"D0PL001Z.SFC", sfc.as_bytes()).unwrap();
        session.push(b"D0PL001Z.SAF", saf.as_bytes()).unwrap();
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"open","input":"D0PL001Z.SFC","saf":"D0PL001Z.SAF"}"#,
        );
        assert_eq!(status, Status::Ok, "{}", String::from_utf8_lossy(&response));
        session
    };
    let name = "D0PL001Z.SFC";
    let start = zumen_drawing::Hash::document(sfc.as_bytes()).to_string();
    let mut mismatches = Vec::new();
    let mut accepted = 0;
    for template in EDIT_KINDS {
        let (mut engine, mut old) = (open(true), open(false));
        let mut document = start.clone();
        accepted += run_both(
            &mut engine,
            &mut old,
            name,
            &mut document,
            &[template],
            &mut mismatches,
        );
    }
    let (mut engine, mut old) = (open(true), open(false));
    let mut document = start;
    accepted += run_both(
        &mut engine,
        &mut old,
        name,
        &mut document,
        &EDIT_KINDS,
        &mut mismatches,
    );
    assert!(
        mismatches.is_empty(),
        "{}",
        mismatches.join(
            "
"
        )
    );
    assert!(accepted > 0);
    assert!(engine_kept(&engine, name));
    // 新しく開いた文書に受け付ける変更だけを当てると、属性ファイル（SAF）を結んだ SXF もファイルモデルを読み直さず常駐文書エンジンから直接答える
    let (mut engine, mut old) = (open(true), open(false));
    let (id0, hash0) = elements(&mut engine, name)[0].clone();
    let move_one = format!(
        r#"{{"v":1,"op":"apply","doc":"{name}","edits":[{{"kind":"transform","id":"{id0}","hash":"{hash0}","matrix":[1,0,5,0,1,7]}}]}}"#
    );
    assert_eq!(same(&mut engine, &mut old, &move_one).0, Status::Ok);
    assert!(resident(&engine, name).held().is_none());
    for read in reads(name, "") {
        same(&mut engine, &mut old, &read);
    }
    assert!(resident(&engine, name).held().is_none());
    // 属性ファイルの診断は、当てたあとの読み取りでも返す
    let (_, response) = call(
        &mut engine,
        &format!(r#"{{"v":1,"op":"drawing","doc":"{name}"}}"#),
    );
    assert!(String::from_utf8_lossy(&response).contains("sxf.saf-figure"));
}
