//! 建物の文書の `building_apply` を文書エンジンで当てても、今までの経路と同じ応答・blob・読み取りの応答になり、
//! どの予算で止まっても文書を変えないことを確かめる（`docs/live.md` の「セッションの文書エンジン」）。

use crate::abi::Status;
use crate::building::Live;
use crate::session::{Document, Session};
use zumen_base::CancelToken;
use zumen_json::Value;

/// 公開サンプル。小さいものから、共有するプロパティのセットを持つもの・日本語の名前を持つものを選ぶ
fn samples() -> Vec<(&'static str, Vec<u8>)> {
    let mut samples = vec![
        (
            "outside.ifc",
            include_bytes!("../../../../../samples/ifc/nakano/nakano_Outside.ifc").to_vec(),
        ),
        (
            "text2bim.ifc",
            include_bytes!("../../../../../samples/ifc/text2bim/sample_model.ifc").to_vec(),
        ),
        (
            "conformance.ifc",
            conformance_text("building_apply", "/steps/0/push/text"),
        ),
    ];
    // ifcZIP は、中の IFC-SPF で文書エンジンを作り、今までの経路と同じくアーカイブに入れ直す
    let zipped: Vec<_> = samples
        .iter()
        .filter(|(name, _)| *name != "outside.ifc")
        .map(|(name, bytes)| {
            let archive = zumen_zip::write(&[("model/a.ifc", bytes)]).unwrap();
            (
                if *name == "conformance.ifc" {
                    "conformance.ifczip"
                } else {
                    "text2bim.ifczip"
                },
                archive,
            )
        })
        .collect();
    samples.extend(zipped);
    samples
}

/// 適合テストの入力の文字列
fn conformance_text(case: &str, pointer: &str) -> Vec<u8> {
    let text = std::fs::read_to_string(format!(
        "{}/../../bindings/conformance/cases/{case}.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    match zumen_json::parse(text.as_bytes()).unwrap().pointer(pointer) {
        Some(Value::String(text)) => text.clone().into_bytes(),
        _ => panic!("適合テストの {case} に {pointer} がない"),
    }
}

fn call(session: &mut Session, request: &str) -> (Status, Vec<u8>) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    (reply.status, reply.response)
}

/// 応答と、取り出したバイナリの結果
fn answer(session: &mut Session, request: &str) -> (Status, Vec<u8>, Vec<Vec<u8>>) {
    let (status, response) = call(session, request);
    let blobs = (0..4).filter_map(|blob| session.take(blob)).collect();
    (status, response, blobs)
}

/// 文字コードの指定（`encoding`、空なら書かない）で開いたセッション
fn opened(name: &str, bytes: &[u8], encoding: &str) -> Session {
    let mut session = Session::new(b"").unwrap();
    session.push(name.as_bytes(), bytes).unwrap();
    let encoding = if encoding.is_empty() {
        String::new()
    } else {
        format!(r#","ifc_text_encoding":"{encoding}""#)
    };
    let (status, response) = call(
        &mut session,
        &format!(r#"{{"v":1,"op":"open","input":"{name}"{encoding}}}"#),
    );
    assert_eq!(status, Status::Ok, "{}", String::from_utf8_lossy(&response));
    session
}

/// 文書エンジンを使うセッションと、今までの経路だけを使うセッション
fn twins(name: &str, bytes: &[u8], encoding: &str) -> (Session, Session) {
    let engine = opened(name, bytes, encoding);
    let mut old = opened(name, bytes, encoding);
    old.engines = false;
    (engine, old)
}

fn live<'a>(session: &'a Session, name: &str) -> &'a Live {
    match session.docs.get(name) {
        Some(Document::Building { model, .. }) => &model.live,
        _ => panic!("{name}"),
    }
}

/// 文書エンジンを持ち続けているか
fn engine_kept(session: &Session, name: &str) -> bool {
    matches!(live(session, name), Live::Ifc(_))
}

/// 文書エンジンを捨て、次の `building_apply` で作り直させる
fn forget_engine(session: &mut Session, name: &str) {
    if let Some(Document::Building { model, .. }) = session.docs.get_mut(name) {
        model.live = Live::Absent;
    }
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

/// 当てたあとに比べる読み取り
fn reads(doc: &str) -> Vec<String> {
    vec![
        format!(r#"{{"v":1,"op":"building","doc":"{doc}"}}"#),
        format!(
            r#"{{"v":1,"op":"building_elements","doc":"{doc}","components":["identity","properties","quantities","relations"],"limit":60}}"#
        ),
    ]
}

/// 文書のハッシュ
fn document_hash(session: &mut Session, doc: &str) -> String {
    let (_, response) = call(
        session,
        &format!(r#"{{"v":1,"op":"building","doc":"{doc}"}}"#),
    );
    match zumen_json::parse(&response)
        .unwrap()
        .pointer("/result/hash")
    {
        Some(Value::String(hash)) => hash.clone(),
        _ => panic!("{}", String::from_utf8_lossy(&response)),
    }
}

/// 要素の ID とハッシュと、最初のプロパティのセットの名前とその最初のプロパティの名前
type Found = (String, String, Option<(String, String)>);

/// 要素の ID とハッシュと、最初のプロパティのセットの名前とその最初のプロパティの名前。初めから 60 件まで
fn elements(session: &mut Session, doc: &str) -> Vec<Found> {
    let (_, response) = call(
        session,
        &format!(
            r#"{{"v":1,"op":"building_elements","doc":"{doc}","components":["properties"],"limit":60}}"#
        ),
    );
    let value = zumen_json::parse(&response).unwrap();
    (0..60)
        .map_while(|index| {
            let at = |field: &str| match value.pointer(&format!("/result/elements/{index}/{field}"))
            {
                Some(Value::String(text)) => Some(text.clone()),
                _ => None,
            };
            let property = at("properties/0/name").zip(at("properties/0/properties/0/name"));
            Some((at("id")?, at("hash")?, property))
        })
        .collect()
}

fn request(doc: &str, extra: &str, edits: &[String]) -> String {
    format!(
        r#"{{"v":1,"op":"building_apply","doc":"{doc}"{extra},"edits":[{}]}}"#,
        edits.join(",")
    )
}

fn property(id: &str, hash: &str, set: &str, name: &str, value: &str) -> String {
    format!(
        r#"{{"kind":"property","id":"{id}","hash":"{hash}","set":"{set}","name":"{name}","value":{value}}}"#
    )
}

fn attribute(id: &str, hash: &str, attribute: &str, value: Option<&str>) -> String {
    let value = value.map_or(String::new(), |value| format!(r#","value":"{value}""#));
    format!(
        r#"{{"kind":"attribute","id":"{id}","hash":"{hash}","attribute":"{attribute}"{value}}}"#
    )
}

/// 要素に当てる変更の一覧の並び。種類ごとに 1 件ずつと、複数をまとめたもの・断られるものを混ぜたもの
fn edit_lists(
    id: &str,
    hash: &str,
    existing: Option<&(String, String)>,
    round: usize,
) -> Vec<(String, Vec<String>)> {
    let text = format!(r#"{{"kind":"text","type_name":"IfcLabel","text":"値 {round} 'x' \\ 🧱"}}"#);
    let mut lists = vec![
        // 新しいセットとプロパティ（実数・整数・真偽・論理・文字）
        (
            String::new(),
            vec![
                property(
                    id,
                    hash,
                    "Zumen_Check",
                    "Width",
                    r#"{"kind":"real","type_name":"IfcPositiveLengthMeasure","number":900}"#,
                ),
                property(
                    id,
                    hash,
                    "Zumen_Check",
                    "Count",
                    r#"{"kind":"integer","type_name":"IfcInteger","text":"12345678901234"}"#,
                ),
                property(
                    id,
                    hash,
                    "Zumen_Check",
                    "Flag",
                    r#"{"kind":"boolean","type_name":"IfcBoolean","boolean":true}"#,
                ),
                property(
                    id,
                    hash,
                    "Zumen_Check",
                    "Maybe",
                    r#"{"kind":"logical","type_name":"IfcLogical"}"#,
                ),
                property(id, hash, "Zumen_Check", "Label", &text),
            ],
        ),
        // 名前と説明。説明は値をなくす
        (
            String::new(),
            vec![
                attribute(id, hash, "name", Some(&format!("名前 {round}"))),
                attribute(id, hash, "description", None),
            ],
        ),
        // 空の変更
        (String::new(), Vec::new()),
        // 断られる変更を混ぜた一覧（要素のハッシュが違う・ない要素）
        (
            String::new(),
            vec![
                attribute(id, hash, "name", Some("通らない")),
                attribute(id, "v1:00000000000000000000000000000000", "name", Some("x")),
                attribute("0000000000000000000zzz", hash, "name", Some("x")),
            ],
        ),
        // 文書の L1 が断る変更（書けない型の名前）。今までの経路も同じ診断で断る
        (
            String::new(),
            vec![property(
                id,
                hash,
                "Zumen_Check",
                "Bad",
                r#"{"kind":"text","type_name":"1bad","text":"x"}"#,
            )],
        ),
        // 文書のハッシュが今と違う
        (
            r#","hash":"v1:00000000000000000000000000000000""#.to_string(),
            vec![attribute(id, hash, "name", Some("x"))],
        ),
    ];
    // 今あるセットのプロパティを書き換える。ほかの要素と共有するセットとプロパティは写しを作る
    if let Some((set, name)) = existing {
        lists.push((String::new(), vec![property(id, hash, set, name, &text)]));
    }
    lists
}

#[test]
fn building_apply_on_the_engine_answers_like_the_old_path_for_each_edit_kind() {
    for (name, bytes) in samples() {
        let (mut engine, mut old) = twins(name, &bytes, "");
        let mut applied = 0;
        // 最初の 3 件の要素に、順に変更を当てる。ID・要素のハッシュは、当てるたびに読み直す
        for round in 0..3 {
            for step in 0.. {
                let found = elements(&mut engine, name);
                assert_eq!(found, elements(&mut old, name), "{name}");
                let Some((id, hash, existing)) = found.get(round * 7 % found.len().max(1)) else {
                    break;
                };
                let Some((extra, edits)) = edit_lists(id, hash, existing.as_ref(), round)
                    .into_iter()
                    .nth(step)
                else {
                    break;
                };
                // 文書のハッシュを付けたものと、付けないもの
                let extra = if extra.is_empty() && step % 2 == 0 {
                    format!(r#","hash":"{}""#, document_hash(&mut engine, name))
                } else {
                    extra
                };
                let (status, _) = same(&mut engine, &mut old, &request(name, &extra, &edits));
                if status == Status::Ok {
                    applied += 1;
                }
                for read in reads(name) {
                    same(&mut engine, &mut old, &read);
                }
            }
        }
        // 通った変更（空の変更・今あるセットの書き換えを含む）は、どれも今までの経路に任せずに文書エンジンで当て、
        // 当てたあとも持ち続ける
        assert!(applied >= 9, "{name} {applied}");
        assert_eq!(engine.engine_applies, applied, "{name}");
        assert!(engine_kept(&engine, name), "{name}");
        assert_eq!(old.engine_applies, 0, "{name}");
    }
}

/// 文字コードの指定ごとに、日本語の名前のセットとプロパティを書き換える。`auto` で手がかりから CP932 で読む
/// ファイルは文書エンジンを作らず、今までの経路で当てる。ほかは文書エンジンで当て、今までの経路と同じになる
#[test]
fn building_apply_on_the_engine_follows_the_text_encoding() {
    // 「テスト」の CP932 のエスケープ。`\X\83` が手がかりになる
    let ifc = concat!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\nFILE_NAME('','',(''),(''),'','','');\n",
        "FILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n",
        "#1=IFCPROJECT('0000000000000000000001',#2,'P',$,$,$,$,$,$);\n",
        "#2=IFCOWNERHISTORY($,$,$,.ADDED.,$,$,$,0);\n",
        "#30=IFCWALL('0000000000000000000030',#2,'\\X\\83\\X\\65',$,$,$,$,$,.NOTDEFINED.);\n",
        "#31=IFCWALL('0000000000000000000031',#2,'W',$,$,$,$,$,.NOTDEFINED.);\n",
        "#40=IFCPROPERTYSINGLEVALUE('\\X\\83\\X\\65\\X\\83\\X\\58\\X\\83\\X\\67',$,IFCLABEL('A'),$);\n",
        "#41=IFCPROPERTYSET('0000000000000000000041',#2,'\\X\\83\\X\\65\\X\\83\\X\\58\\X\\83\\X\\67',$,(#40));\n",
        "#42=IFCRELDEFINESBYPROPERTIES('0000000000000000000042',#2,$,$,(#30,#31),#41);\n",
        "ENDSEC;\nEND-ISO-10303-21;\n"
    );
    let mut inputs = vec![("hint.ifc", ifc.as_bytes().to_vec())];
    for (step, pointer) in [
        ("hint", "/steps/0/push/text"),
        ("plain", "/steps/1/push/text"),
    ] {
        inputs.push((
            if step == "hint" {
                "case_hint.ifc"
            } else {
                "case_plain.ifc"
            },
            conformance_text("ifc_text_encoding", pointer),
        ));
    }
    for (name, bytes) in &inputs {
        for encoding in ["", "auto", "iso_8859_1", "shift_jis"] {
            let (mut engine, mut old) = twins(name, bytes, encoding);
            let hinted = *name != "case_plain.ifc";
            let refused = hinted && (encoding.is_empty() || encoding == "auto");
            for round in 0..3 {
                let found = elements(&mut engine, name);
                assert_eq!(found, elements(&mut old, name), "{name} {encoding}");
                // 2 回目は最初の要素（適合テストの入力では、手がかりの文字列を持つ要素）を書き換える
                let target = if round == 1 {
                    found.first()
                } else {
                    found.last()
                };
                if let Some((id, hash, existing)) = target {
                    // 読んだ名前で書き換える。CP932 で読むなら元のセットを、ほかは新しいセットを書き換える
                    let mut edits = vec![property(
                        id,
                        hash,
                        "テスト",
                        "テスト",
                        &format!(r#"{{"kind":"text","type_name":"IfcLabel","text":"{round}"}}"#),
                    )];
                    if let Some((set, property_name)) = existing {
                        edits.push(property(
                            id,
                            hash,
                            set,
                            property_name,
                            r#"{"kind":"integer","type_name":"IfcInteger","number":3}"#,
                        ));
                    }
                    // 手がかりの文字列だけを持つ要素の名前を書き換えると、`auto` の今までの経路は読み方を変える
                    edits.push(attribute(id, hash, "name", Some("ascii")));
                    same(&mut engine, &mut old, &request(name, "", &edits));
                    for read in reads(name) {
                        same(&mut engine, &mut old, &read);
                    }
                }
                if refused && round == 0 {
                    assert_eq!(engine.engine_applies, 0, "{name} {encoding}");
                }
            }
            // 適合テストの入力は、2 回目で手がかりの文字列をなくすので、3 回目は `auto` でも文書エンジンで当てる
            if refused && *name == "hint.ifc" {
                assert_eq!(engine.engine_applies, 0, "{name} {encoding}");
            } else {
                assert!(engine.engine_applies > 0, "{name} {encoding}");
                assert!(engine_kept(&engine, name), "{name} {encoding}");
            }
        }
    }
}

/// 文書エンジンで当てたあとに、今までの経路に任せる要求（L1 が断る変更）を挟んでも、文書エンジンを持ち続け、
/// 次からも同じ結果になる。開き直した文書は、文書エンジンを持たずに始まる
#[test]
fn building_apply_keeps_the_engine_across_rejections_and_drops_it_on_reopen() {
    let (name, bytes) = (
        "conformance.ifc",
        conformance_text("building_apply", "/steps/0/push/text"),
    );
    let (mut engine, mut old) = twins(name, &bytes, "");
    let rename = |session: &mut Session, value: &str| {
        let found = elements(session, name);
        let (id, hash, _) = found.first().cloned().unwrap();
        request(name, "", &[attribute(&id, &hash, "name", Some(value))])
    };
    let first = rename(&mut engine, "一");
    same(&mut engine, &mut old, &first);
    assert!(engine_kept(&engine, name));
    let found = elements(&mut engine, name);
    let (id, hash, _) = found.first().cloned().unwrap();
    let rejected = request(
        name,
        "",
        &[property(
            &id,
            &hash,
            "S",
            "P",
            r#"{"kind":"text","type_name":"1bad","text":"x"}"#,
        )],
    );
    let (status, response) = same(&mut engine, &mut old, &rejected);
    assert_eq!(status, Status::Failed);
    assert!(String::from_utf8_lossy(&response).contains("change.edit.unsupported"));
    assert!(engine_kept(&engine, name));
    let second = rename(&mut engine, "二");
    same(&mut engine, &mut old, &second);
    assert_eq!(engine.engine_applies, 2);
    // 閉じて同じ名前で開き直すと、文書エンジンを持たずに始まる
    let (status, _) = call(
        &mut engine,
        &format!(r#"{{"v":1,"op":"close","docs":["{name}"]}}"#),
    );
    assert_eq!(status, Status::Ok);
    let (status, _) = call(
        &mut engine,
        &format!(r#"{{"v":1,"op":"open","input":"{name}"}}"#),
    );
    assert_eq!(status, Status::Ok);
    assert!(matches!(live(&engine, name), Live::Absent));
}

/// 止まったときの診断が、予算か中断のものだけを含むか
fn stopped_on_budget(response: &[u8]) -> bool {
    let value = zumen_json::parse(response).unwrap();
    let Some(Value::Array(diagnostics)) = value.pointer("/diagnostics") else {
        return false;
    };
    !diagnostics.is_empty()
        && diagnostics.iter().all(|diagnostic| {
            matches!(diagnostic.pointer("/code"), Some(Value::String(code)) if code.starts_with("budget."))
        })
}

#[test]
fn building_apply_on_the_engine_stops_without_changing_the_document() {
    for (name, bytes) in samples() {
        if name == "text2bim.ifc" || name == "text2bim.ifczip" {
            // 大きいサンプルは、上の試験で比べる。ここでは止まる所を細かく変える
            continue;
        }
        let (mut engine, mut old) = twins(name, &bytes, "");
        let found = elements(&mut engine, name);
        let (id, hash, _) = found.first().cloned().unwrap();
        let edits = [
            property(
                &id,
                &hash,
                "Zumen_Check",
                "Width",
                r#"{"kind":"real","type_name":"IfcReal","number":1.5}"#,
            ),
            attribute(&id, &hash, "name", Some("止める")),
        ];
        let limited = |budget: &str| request(name, budget, &edits);
        // 文書エンジンを作ってから、L1 が断る変更（文書を変えない）
        let warm = request(
            name,
            "",
            &[property(
                &id,
                &hash,
                "S",
                "P",
                r#"{"kind":"text","type_name":"1bad","text":"x"}"#,
            )],
        );
        let before: Vec<_> = reads(name)
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
            for limit in [
                0_u64, 1, 10, 100, 1_000, 3_000, 10_000, 30_000, 100_000, 1_000_000,
            ] {
                for warmed in [false, true] {
                    if warmed {
                        assert_eq!(answer(&mut engine, &warm).0, Status::Failed, "{name}");
                        assert!(engine_kept(&engine, name), "{name}");
                    } else {
                        forget_engine(&mut engine, name);
                    }
                    let (status, response, blobs) = answer(
                        &mut engine,
                        &limited(&format!(r#","budget":{{"{kind}":{limit}}}"#)),
                    );
                    if status == Status::Ok {
                        // 通ったら、今までの経路と同じ結果になる。文書を戻すために開き直す
                        let expected = answer(&mut old, &limited(""));
                        assert_eq!(
                            (status, &response, &blobs),
                            (expected.0, &expected.1, &expected.2),
                            "{name} {kind} {limit}"
                        );
                        (engine, old) = twins(name, &bytes, "");
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
                    let after: Vec<_> = reads(name)
                        .iter()
                        .map(|read| answer(&mut engine, read))
                        .collect();
                    assert_eq!(after, before, "{name} {kind} {limit}");
                }
            }
        }
        assert!(stops > 0, "{name}");
        // 止まったあとも、今までの経路と同じ結果で当てられる
        same(&mut engine, &mut old, &limited(""));
        assert!(engine_kept(&engine, name), "{name}");
        for read in reads(name) {
            same(&mut engine, &mut old, &read);
        }
    }
}
