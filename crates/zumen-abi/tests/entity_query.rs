//! 絞り込みとページ、属性の複製、予算停止の契約を公開 API で照らす。
#![cfg(feature = "drawing")]
#![allow(clippy::unwrap_used, clippy::panic)] // 自作の有限な入力と期待値だけを扱う。

use zumen_abi1::{Session, Status};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::{Drawing2D, binary};
use zumen_json::Value;

#[path = "../examples/entity_query_case.rs"]
mod fixture;

fn session(drawing: Drawing2D) -> Session {
    let (source, _) = fixture::fixture();
    let input = binary::write(
        &source,
        &drawing,
        &mut Cx::new(
            &mut Budget::default(),
            &mut Diagnostics::new(),
            &CancelToken::new(),
        ),
    )
    .unwrap();
    let mut session = Session::new(b"").unwrap();
    session.push(b"a", &input).unwrap();
    assert_eq!(
        session
            .call(br#"{"v":1,"op":"open","input":"a"}"#, &CancelToken::new())
            .status,
        Status::Ok
    );
    session
}
fn request(extra: &str) -> String {
    format!(r#"{{"v":1,"op":"entities","doc":"a","fields":["id"]{extra}}}"#)
}
fn call(session: &mut Session, request: &str) -> (Status, Value) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    (reply.status, zumen_json::parse(&reply.response).unwrap())
}
fn ids(value: &Value) -> Vec<&str> {
    let Some(Value::Array(items)) = value.pointer("/result/items") else {
        panic!("{value:?}")
    };
    items
        .iter()
        .map(|item| {
            let Some(Value::String(id)) = item.pointer("/id") else {
                panic!("{item:?}")
            };
            id.as_str()
        })
        .collect()
}

#[test]
fn conditions_intersect_before_pagination_without_changing_ids_or_order() {
    let mut session = session(fixture::fixture().1);
    let all = [
        "space/0/hA",
        "space/0/hA/1",
        "space/0/i20",
        "space/0/e3",
        "sheet/hF0/hA",
    ];
    // 各集合は合成入力から手で定める。互いに条件を重ね、元の順と選択後のページ位置を比べる。
    for (id_query, id_set) in [
        ("", vec![0, 1, 2, 3, 4]),
        (
            r#", "ids":["sheet/hF0/hA","space/0/hA/1","space/0/hA/1","missing"]"#,
            vec![1, 4],
        ),
    ] {
        for (layer_query, layer_set) in [
            ("", vec![0, 1, 2, 3, 4]),
            (r#", "layers":["layer/h10/1"]"#, vec![1, 2]),
        ] {
            for (kind_query, kind_set) in [
                ("", vec![0, 1, 2, 3, 4]),
                (r#", "kinds":["line"]"#, vec![0, 1, 3, 4]),
            ] {
                for (source_query, source_set) in [
                    ("", vec![0, 1, 2, 3, 4]),
                    (r#", "source":{"bytes":{"start":10,"end":20}}"#, vec![0, 4]),
                ] {
                    let expected: Vec<_> = (0..all.len())
                        .filter(|i| {
                            id_set.contains(i)
                                && layer_set.contains(i)
                                && kind_set.contains(i)
                                && source_set.contains(i)
                        })
                        .map(|i| all[i])
                        .collect();
                    for offset in 0..=expected.len() {
                        let query = request(&format!(
                            r#"{id_query}{layer_query}{kind_query}{source_query},"offset":{offset},"limit":1"#
                        ));
                        let (status, response) = call(&mut session, &query);
                        assert_eq!(status, Status::Ok, "{query}: {response:?}");
                        assert_eq!(
                            ids(&response),
                            expected
                                .iter()
                                .copied()
                                .skip(offset)
                                .take(1)
                                .collect::<Vec<_>>(),
                            "{query}"
                        );
                        assert_eq!(
                            response.pointer("/result/total"),
                            Some(
                                &zumen_json::parse(expected.len().to_string().as_bytes()).unwrap()
                            )
                        );
                        if offset + 1 < expected.len() {
                            assert_eq!(
                                response.pointer("/result/next_offset"),
                                Some(
                                    &zumen_json::parse((offset + 1).to_string().as_bytes())
                                        .unwrap()
                                )
                            );
                        } else {
                            assert!(response.pointer("/result/next_offset").is_none());
                        }
                    }
                }
            }
        }
    }
    assert_eq!(ids(&call(&mut session, &request("")).1), all);
}

#[test]
fn filtering_happens_before_selected_attributes_are_copied() {
    let (_, mut drawing) = fixture::fixture();
    drawing.spaces[0].elements[0].style.line_type_scale = f64::NAN;
    let mut session = session(drawing);
    let selected = r#"{"v":1,"op":"entities","doc":"a","ids":["space/0/hA/1"],"fields":["style"]}"#;
    let (status, result) = call(&mut session, selected);
    assert_eq!(status, Status::Ok, "{result:?}");
    assert_eq!(ids(&result), ["space/0/hA/1"]);
    let invalid = selected.replace("space/0/hA/1", "space/0/hA");
    assert_eq!(call(&mut session, &invalid).0, Status::Failed);
    assert_eq!(call(&mut session, selected), (status, result));
}

#[test]
fn layer_reference_matches_the_first_internal_definition_like_drawing_layer() {
    let (_, mut drawing) = fixture::fixture();
    drawing.layers.push(drawing.layers[0].clone());
    let mut session = session(drawing);
    assert!(ids(&call(&mut session, &request(r#", "layers":["layer/h10/2"]"#)).1).is_empty());
    assert_eq!(
        ids(&call(&mut session, &request(r#", "layers":["layer/h10"]"#)).1),
        ["space/0/hA", "sheet/hF0/hA"]
    );
}

fn minimum_budget(session: &mut Session, extra: &str, kind: &str) -> u64 {
    let mut low = 0;
    let mut high = 1_u64 << 30;
    let run = |session: &mut Session, limit| {
        call(
            session,
            &request(&format!(r#"{extra},"budget":{{"{kind}":{limit}}}"#)),
        )
        .0
    };
    assert_eq!(run(session, high), Status::Ok);
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if run(session, middle) == Status::Ok {
            high = middle;
        } else {
            low = middle;
        }
    }
    high
}

#[test]
fn selector_copy_sort_and_lookup_are_bounded_and_do_not_poison_the_document() {
    let mut session = session(fixture::fixture().1);
    let extra = format!(
        r#", "ids":["space/0/hA",{}],"layers":["layer/h10"],"kinds":["line","line"]"#,
        (0..1024)
            .map(|i| format!("\"missing/{i:04}\""))
            .collect::<Vec<_>>()
            .join(",")
    );
    let query = request(&extra);
    let expected = call(&mut session, &query);
    assert_eq!(expected.0, Status::Ok);
    assert_eq!(ids(&expected.1), ["space/0/hA"]);
    for (kind, code) in [
        ("work", "budget.work.exceeded"),
        ("decoded_bytes", "budget.decoded-bytes.exceeded"),
    ] {
        let base = minimum_budget(&mut session, "", kind);
        let required = minimum_budget(&mut session, &extra, kind);
        assert!(required > base);
        for limit in [base, required - 1] {
            let (status, response) = call(
                &mut session,
                &request(&format!(r#"{extra},"budget":{{"{kind}":{limit}}}"#)),
            );
            assert_eq!(status, Status::Failed);
            assert_eq!(
                response.pointer("/diagnostics/0/code"),
                Some(&Value::String(code.into()))
            );
            assert!(response.pointer("/result").is_none());
        }
        assert_eq!(
            call(
                &mut session,
                &request(&format!(r#"{extra},"budget":{{"{kind}":{required}}}"#))
            ),
            expected
        );
    }
    let token = CancelToken::new();
    token.cancel();
    assert_eq!(
        session.call(query.as_bytes(), &token).status,
        Status::Failed
    );
    assert_eq!(call(&mut session, &query), expected);
    assert_eq!(
        call(&mut session, &request("")),
        call(&mut session, &request(r#", "source":{}"#))
    );
}
