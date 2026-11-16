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
fn limit_sets(spent: &[u64; 7]) -> Vec<Limits> {
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
                let expected = outcome(&limits, max_places, &cancel, |cx| file.file.lift(cx));
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
        let expected = outcome(&Limits::default(), 16, &cancelled, |cx| file.file.lift(cx));
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
            resident(&probe, name).file.lift(cx)
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

/// 常駐の図面モデルを捨て、次の読み取りで変換し直させる
fn forget(session: &mut Session, name: &str) {
    if let Some(Document::Drawing { file, .. }) = session.docs.get_mut(name) {
        file.file_mut();
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
