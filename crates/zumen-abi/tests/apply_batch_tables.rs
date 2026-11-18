//! 同じ apply の中で作った表への参照と、失敗時の原子性。
#![cfg(feature = "drawing")]
#![allow(clippy::unwrap_used, clippy::panic)] // 自作入力と期待する応答だけを扱う。

use zumen_abi1::{Session, Status};
use zumen_base::CancelToken;
use zumen_json::{Value, Writer};

use cases::DXF;

#[path = "../examples/apply_batch_tables_case.rs"]
mod cases;

fn call(session: &mut Session, request: &str) -> (Status, Value) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    (reply.status, zumen_json::parse(&reply.response).unwrap())
}

fn ok(session: &mut Session, request: &str) -> Value {
    let (status, value) = call(session, request);
    assert_eq!(status, Status::Ok, "{value:?}");
    value
}

fn open(bytes: &[u8]) -> Session {
    let mut session = Session::new(b"").unwrap();
    session.push(b"a", bytes).unwrap();
    ok(&mut session, r#"{"v":1,"op":"open","input":"a"}"#);
    session
}

fn json(value: &Value) -> String {
    let mut writer = Writer::new();
    writer.value(value);
    String::from_utf8(writer.into_bytes()).unwrap()
}

fn named<'a>(tables: &'a Value, kind: &str, name: &str) -> &'a Value {
    let Some(Value::Array(items)) = tables.pointer(&format!("/result/{kind}")) else {
        panic!("{tables:?}")
    };
    items
        .iter()
        .find(|item| item.pointer("/name") == Some(&Value::String(name.into())))
        .unwrap()
}

#[test]
fn added_line_type_and_layer_are_available_to_later_add_and_restyle() {
    for bytes in [
        DXF,
        include_bytes!("../../../samples/dwg/zumen-layout-r2000.dwg").as_slice(),
        include_bytes!("../../../samples/dwg/zumen-layout-r2004.dwg").as_slice(),
        include_bytes!("../../../samples/dwg/zumen-layout-r2018.dwg").as_slice(),
    ] {
        let mut session = open(bytes);
        let original = ok(
            &mut session,
            r#"{"v":1,"op":"entities","doc":"a","fields":["hash"]}"#,
        );
        let first = original.pointer("/result/items/0").unwrap();
        let request = format!(
            concat!(
                r#"{{"v":1,"op":"apply","doc":"a","edits":["#,
                r#"{{"kind":"add_line_type","name":"BatchType","pattern":[3,-1]}},"#,
                r#"{{"kind":"add_layer","name":"BatchLayer","line_type":{{"name":"BatchType"}}}},"#,
                r#"{{"kind":"add","place":"space/0","layer":"BatchLayer","line_type":{{"name":"BatchType"}},"geometry":{{"shape":"line","start":[5,6],"end":[7,8]}}}},"#,
                r#"{{"kind":"restyle","id":{},"hash":{},"layer":"BatchLayer","line_type":{{"name":"BatchType"}}}}]}}"#
            ),
            json(first.pointer("/id").unwrap()),
            json(first.pointer("/hash").unwrap())
        );
        let applied = ok(&mut session, &request);
        let written = session.take(0).unwrap();
        let tables = ok(&mut session, r#"{"v":1,"op":"tables","doc":"a"}"#);
        let layer = named(&tables, "layers", "BatchLayer");
        let line_type = named(&tables, "line_types", "BatchType");
        let new_id = applied.pointer("/result/applied/2/id").unwrap();
        let query = format!(
            r#"{{"v":1,"op":"entities","doc":"a","ids":[{}],"fields":["layer","geometry","style","hash"]}}"#,
            json(new_id)
        );
        let entity = ok(&mut session, &query);
        assert_eq!(
            entity.pointer("/result/items/0/layer"),
            layer.pointer("/name")
        );
        assert_eq!(
            entity.pointer("/result/items/0/style/line_type/id"),
            line_type.pointer("/id")
        );
        let mut reopened = open(&written);
        assert_eq!(ok(&mut reopened, &query), entity);
        assert_eq!(
            ok(&mut reopened, r#"{"v":1,"op":"tables","doc":"a"}"#),
            tables
        );
    }
}

#[test]
fn added_text_and_dimension_styles_are_available_to_texts_and_leaders() {
    for bytes in [
        DXF,
        include_bytes!("../../../samples/dwg/zumen-layout-r2000.dwg").as_slice(),
        include_bytes!("../../../samples/dwg/zumen-layout-r2004.dwg").as_slice(),
        include_bytes!("../../../samples/dwg/zumen-layout-r2018.dwg").as_slice(),
    ] {
        let mut session = open(bytes);
        let applied = ok(
            &mut session,
            r#"{"v":1,"op":"apply","doc":"a","edits":[
            {"kind":"add_text_style","name":"BatchText","font":{"name":"synthetic","kind":"shape"}},
            {"kind":"add_dimension_style","name":"BatchDim","size":3,"scale":2},
            {"kind":"add","place":"space/0","geometry":{"shape":"text","position":[10,20],"content":"New","height":2.5,"width_factor":1,"rotation":0,"style":"BatchText"}},
            {"kind":"add","place":"space/0","geometry":{"shape":"leader","path":[[0,0],[4,5]],"spline":false,"arrow":true,"style":"BatchDim"}}
        ]}"#,
        );
        let written = session.take(0).unwrap();
        let tables = ok(&mut session, r#"{"v":1,"op":"tables","doc":"a"}"#);
        for (index, kind, name) in [
            (2, "text_styles", "BatchText"),
            (3, "dimension_styles", "BatchDim"),
        ] {
            let id = applied
                .pointer(&format!("/result/applied/{index}/id"))
                .unwrap();
            let query = format!(
                r#"{{"v":1,"op":"entities","doc":"a","ids":[{}],"fields":["geometry","hash"]}}"#,
                json(id)
            );
            let entity = ok(&mut session, &query);
            let style = named(&tables, kind, name);
            assert_eq!(
                entity.pointer("/result/items/0/geometry/style"),
                style.pointer("/name")
            );
            assert_eq!(
                entity.pointer("/result/items/0/geometry/style_id"),
                style.pointer("/id")
            );
            let mut reopened = open(&written);
            assert_eq!(ok(&mut reopened, &query), entity);
        }
    }
}

#[test]
fn replacement_can_use_an_earlier_text_style() {
    let mut session = open(DXF);
    let original = ok(
        &mut session,
        r#"{"v":1,"op":"entities","doc":"a","ids":["space/0/h21"],"fields":["hash"]}"#,
    );
    let hash = original.pointer("/result/items/0/hash").unwrap();
    let request = format!(
        r#"{{"v":1,"op":"apply","doc":"a","edits":[
        {{"kind":"add_text_style","name":"BatchText","font":{{"name":"synthetic","kind":"shape"}}}},
        {{"kind":"replace","id":"space/0/h21","hash":{},"geometry":{{"shape":"text","position":[3,4],"content":"Changed","height":2.5,"width_factor":1,"rotation":0,"style":"BatchText"}}}}
    ]}}"#,
        json(hash)
    );
    ok(&mut session, &request);
    let written = session.take(0).unwrap();
    let query =
        r#"{"v":1,"op":"entities","doc":"a","ids":["space/0/h21"],"fields":["geometry","hash"]}"#;
    let entity = ok(&mut session, query);
    assert_eq!(
        entity.pointer("/result/items/0/geometry/content"),
        Some(&Value::String("Changed".into()))
    );
    assert_eq!(
        entity.pointer("/result/items/0/geometry/style"),
        Some(&Value::String("BatchText".into()))
    );
    assert_eq!(ok(&mut open(&written), query), entity);
}

#[test]
fn failed_batches_keep_the_document_and_tables_and_do_not_publish_a_blob() {
    let base = r#"{"kind":"add_layer","name":"BatchLayer"},{"kind":"add","place":"space/0","layer":"BatchLayer","geometry":{"shape":"line","start":[1,2],"end":[3,4]}}"#;
    for suffix in [
        r#",{"kind":"add","place":"space/0","layer":"Missing","geometry":{"shape":"line","start":[0,0],"end":[1,1]}}"#,
        r#",{"kind":"add_layer","name":"BatchLayer"}"#,
        r#",{"kind":"add","place":"space/0","geometry":{"shape":"text","position":[0,0],"height":2,"width_factor":1,"rotation":0,"content":"Forward","style":"Future"}},{"kind":"add_text_style","name":"Future","font":{"name":"synthetic","kind":"shape"}}"#,
    ] {
        let mut session = open(DXF);
        let drawing = ok(&mut session, r#"{"v":1,"op":"drawing","doc":"a"}"#);
        let tables = ok(&mut session, r#"{"v":1,"op":"tables","doc":"a"}"#);
        let request = format!(r#"{{"v":1,"op":"apply","doc":"a","edits":[{base}{suffix}]}}"#);
        let (status, response) = call(&mut session, &request);
        assert_eq!(status, Status::Failed, "{response:?}");
        assert!(session.take(0).is_none());
        assert_eq!(
            ok(&mut session, r#"{"v":1,"op":"drawing","doc":"a"}"#),
            drawing
        );
        assert_eq!(
            ok(&mut session, r#"{"v":1,"op":"tables","doc":"a"}"#),
            tables
        );
    }
}

#[test]
fn layer_references_work_in_jww_sfc_and_p21() {
    for (format, bytes, place) in cases::other_inputs() {
        let mut session = open(&bytes);
        let original = ok(
            &mut session,
            r#"{"v":1,"op":"entities","doc":"a","fields":["hash"]}"#,
        );
        let first = original.pointer("/result/items/0").unwrap();
        let request = format!(
            r#"{{"v":1,"op":"apply","doc":"a","edits":[
            {{"kind":"add_layer","name":"BatchLayer"}},
            {{"kind":"restyle","id":{},"hash":{},"layer":"BatchLayer"}},
            {{"kind":"add","place":"{place}","layer":"BatchLayer","color":{{"{}":1}},"line_type":{{"name":"{}"}},{}"geometry":{{"shape":"line","start":[1,2],"end":[3,4]}}}}
        ]}}"#,
            json(first.pointer("/id").unwrap()),
            json(first.pointer("/hash").unwrap()),
            if format == "JWW" { "jww" } else { "sxf" },
            if format == "JWW" { "1" } else { "continuous" },
            if format == "JWW" {
                ""
            } else {
                r#""line_weight":{"mm":0.13},"#
            }
        );
        let (status, response) = call(&mut session, &request);
        assert_eq!(status, Status::Ok, "{format}: {response:?}");
        let written = session.take(0).unwrap();
        let query =
            r#"{"v":1,"op":"entities","doc":"a","fields":["hash","geometry","layer","style"]}"#;
        let entities = ok(&mut session, query);
        assert_eq!(
            entities.pointer("/result/items/0/layer"),
            Some(&Value::String("BatchLayer".into()))
        );
        assert_eq!(ok(&mut open(&written), query), entities);
    }
}

#[test]
fn keyless_entities_keep_their_targets_across_table_insertions() {
    let input = String::from_utf8_lossy(DXF)
        .replace("5\n20\n", "")
        .replace("5\n21\n", "");
    let mut session = open(input.as_bytes());
    let before = ok(
        &mut session,
        r#"{"v":1,"op":"entities","doc":"a","fields":["hash","geometry"]}"#,
    );
    let first = before.pointer("/result/items/0").unwrap();
    let request = format!(
        r#"{{"v":1,"op":"apply","doc":"a","edits":[{{"kind":"add_layer","name":"BatchLayer"}},{{"kind":"restyle","id":{},"hash":{},"layer":"BatchLayer"}}]}}"#,
        json(first.pointer("/id").unwrap()),
        json(first.pointer("/hash").unwrap())
    );
    let result = ok(&mut session, &request);
    assert_eq!(
        result.pointer("/result/ids_shifted"),
        Some(&Value::Bool(true))
    );
    let after = ok(
        &mut session,
        r#"{"v":1,"op":"entities","doc":"a","fields":["hash","geometry","layer"]}"#,
    );
    assert_eq!(
        after.pointer("/result/items/0/geometry"),
        first.pointer("/geometry")
    );
    assert_eq!(
        after.pointer("/result/items/0/layer"),
        Some(&Value::String("BatchLayer".into()))
    );
}

#[test]
fn duplicate_targets_are_rejected_even_across_an_added_table() {
    let mut session = open(DXF);
    let before = ok(
        &mut session,
        r#"{"v":1,"op":"entities","doc":"a","fields":["hash"]}"#,
    );
    let first = before.pointer("/result/items/0").unwrap();
    let edit = format!(
        r#"{{"kind":"restyle","id":{},"hash":{},"layer":"BatchLayer"}}"#,
        json(first.pointer("/id").unwrap()),
        json(first.pointer("/hash").unwrap())
    );
    let request = format!(
        r#"{{"v":1,"op":"apply","doc":"a","edits":[{{"kind":"add_layer","name":"BatchLayer"}},{edit},{edit}]}}"#
    );
    let (status, response) = call(&mut session, &request);
    assert_eq!(status, Status::Failed, "{response:?}");
    assert!(session.take(0).is_none());
    assert_eq!(
        ok(
            &mut session,
            r#"{"v":1,"op":"entities","doc":"a","fields":["hash"]}"#
        ),
        before
    );
}

#[test]
fn text_and_mtext_replacements_write_the_new_style_reference() {
    for bytes in [
        DXF,
        include_bytes!("../../../samples/dwg/zumen-layout-r2000.dwg").as_slice(),
        include_bytes!("../../../samples/dwg/zumen-layout-r2004.dwg").as_slice(),
        include_bytes!("../../../samples/dwg/zumen-layout-r2018.dwg").as_slice(),
    ] {
        for shape in ["text", "formatted_text"] {
            let mut session = open(bytes);
            let add = format!(
                r#"{{"v":1,"op":"apply","doc":"a","edits":[{{"kind":"add","place":"space/0","geometry":{{"shape":"{shape}","position":[1,2],"content":"Original","height":2,"rotation":0}}}}]}}"#
            );
            let applied = ok(&mut session, &add);
            session.take(0).unwrap();
            let id = json(applied.pointer("/result/applied/0/id").unwrap());
            let query = format!(
                r#"{{"v":1,"op":"entities","doc":"a","ids":[{id}],"fields":["geometry","hash"]}}"#
            );
            let entity = ok(&mut session, &query);
            let item = entity.pointer("/result/items/0").unwrap();
            let geometry = item.pointer("/geometry").unwrap();
            let mut geometry_json = json(geometry).replace(
                &format!(r#""style":{}"#, json(geometry.pointer("/style").unwrap())),
                r#""style":"BatchText""#,
            );
            if let Some(style_id) = geometry.pointer("/style_id") {
                geometry_json =
                    geometry_json.replace(&format!(r#","style_id":{}"#, json(style_id)), "");
            }
            let request = format!(
                r#"{{"v":1,"op":"apply","doc":"a","edits":[{{"kind":"add_text_style","name":"BatchText","font":{{"name":"synthetic","kind":"shape"}}}},{{"kind":"replace","id":{id},"hash":{},"geometry":{geometry_json}}}]}}"#,
                json(item.pointer("/hash").unwrap())
            );
            ok(&mut session, &request);
            let written = session.take(0).unwrap();
            let after = ok(&mut session, &query);
            assert_eq!(
                after.pointer("/result/items/0/geometry/style"),
                Some(&Value::String("BatchText".into()))
            );
            assert_eq!(ok(&mut open(&written), &query), after);
        }
    }
}

#[test]
fn stopped_batches_and_stale_document_leave_no_table_or_blob() {
    let edits = r#"[{"kind":"add_layer","name":"BatchLayer"},{"kind":"add","place":"space/0","layer":"BatchLayer","geometry":{"shape":"line","start":[1,2],"end":[3,4]}}]"#;
    let query = r#"{"v":1,"op":"drawing","doc":"a"}"#;
    for kind in ["work", "decoded_bytes"] {
        let mut low = 0u64;
        let mut high = 16_000_000u64;
        let mut saw_failure = false;
        while low < high {
            let limit = low + (high - low) / 2;
            let mut session = open(DXF);
            let before = ok(&mut session, query);
            let tables = ok(&mut session, r#"{"v":1,"op":"tables","doc":"a"}"#);
            let request = format!(
                r#"{{"v":1,"op":"apply","doc":"a","edits":{edits},"budget":{{"{kind}":{limit}}}}}"#
            );
            let (status, response) = call(&mut session, &request);
            if status == Status::Ok {
                high = limit;
            } else {
                assert_eq!(status, Status::Failed, "{response:?}");
                assert!(json(&response).contains("budget."), "{response:?}");
                assert!(session.take(0).is_none());
                assert_eq!(ok(&mut session, query), before);
                assert_eq!(
                    ok(&mut session, r#"{"v":1,"op":"tables","doc":"a"}"#),
                    tables
                );
                low = limit + 1;
                saw_failure = true;
            }
        }
        assert!(saw_failure && high < 16_000_000);
    }
    let mut session = open(DXF);
    let before = ok(&mut session, query);
    let request = format!(
        r#"{{"v":1,"op":"apply","doc":"a","hash":"00000000000000000000000000000000","edits":{edits}}}"#
    );
    assert_eq!(call(&mut session, &request).0, Status::Failed);
    assert_eq!(ok(&mut session, query), before);
    let token = CancelToken::new();
    token.cancel();
    let request = format!(r#"{{"v":1,"op":"apply","doc":"a","edits":{edits}}}"#);
    let reply = session.call(request.as_bytes(), &token);
    assert_eq!(reply.status, Status::Failed);
    assert_eq!(ok(&mut session, query), before);
    assert!(session.take(0).is_none());
    ok(&mut session, &request);
}

#[test]
fn annotations_keep_their_target_when_another_table_follows_the_addition() {
    let source = String::from_utf8_lossy(DXF).replace(
        "0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n",
        "0\nENDTAB\n0\nTABLE\n2\nBLOCK_RECORD\n0\nBLOCK_RECORD\n5\n1F\n2\n*Model_Space\n0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n",
    );
    let input = source.replace(
        "0\nENDSEC\n0\nEOF\n",
        "0\nMTEXT\n5\n22\n330\n1F\n8\n0\n10\n3\n20\n4\n40\n2.5\n1\nNote\n7\nStandard\n0\nENDSEC\n0\nEOF\n",
    );
    let mut session = open(input.as_bytes());
    ok(
        &mut session,
        r#"{"v":1,"op":"apply","doc":"a","edits":[{"kind":"add_layer","name":"Ready"},{"kind":"add","place":"space/0","layer":"Ready","geometry":{"shape":"leader","path":[[0,0],[3,4]],"spline":false,"arrow":true,"style":"Standard","annotation":"space/0/h22"}},{"kind":"add_text_style","name":"Later","font":{"name":"synthetic","kind":"shape"}},{"kind":"add","place":"space/0","geometry":{"shape":"text","position":[6,7],"content":"Later","height":2,"rotation":0,"style":"Later"}}]}"#,
    );
    let written = session.take(0).unwrap();
    let query =
        r#"{"v":1,"op":"entities","doc":"a","kinds":["leader"],"fields":["geometry","hash"]}"#;
    let after = ok(&mut session, query);
    assert_eq!(
        after.pointer("/result/items/0/geometry/annotation"),
        Some(&Value::String("space/0/h22".into()))
    );
    assert_eq!(ok(&mut open(&written), query), after);
}

#[test]
fn added_symbol_definition_is_available_to_a_later_symbol_in_the_same_batch() {
    for bytes in [
        include_bytes!("../../../samples/dwg/zumen-layout-r2000_ascii.dxf").as_slice(),
        include_bytes!("../../../samples/dwg/zumen-layout-r2000.dwg").as_slice(),
        include_bytes!("../../../samples/dwg/zumen-layout-r2004.dwg").as_slice(),
        include_bytes!("../../../samples/dwg/zumen-layout-r2018.dwg").as_slice(),
    ] {
        let mut session = open(bytes);
        let original = ok(
            &mut session,
            r#"{"v":1,"op":"entities","doc":"a","fields":["hash"]}"#,
        );
        let applied = ok(
            &mut session,
            r#"{"v":1,"op":"apply","doc":"a","edits":[
            {"kind":"add_layer","name":"KitLayer"},
            {"kind":"add_symbol","name":"Kit","base":[1,2],"elements":[
                {"layer":"KitLayer","geometry":{"shape":"line","start":[0,0],"end":[4,3]}},
                {"geometry":{"shape":"circle","center":[1,2],"radius":1.5}}
            ]},
            {"kind":"add","place":"space/0","geometry":{"shape":"symbol","name":"Kit","position":[10,20],"scale":[1,1],"rotation":0}}
        ]}"#,
        );
        // 定義の追加は、要素の ID とハッシュを返さない
        assert_eq!(applied.pointer("/result/applied/1/id"), None, "{applied:?}");
        let written = session.take(0).unwrap();
        let tables = ok(&mut session, r#"{"v":1,"op":"tables","doc":"a"}"#);
        let symbol = named(&tables, "symbols", "Kit");
        assert_eq!(json(symbol.pointer("/base").unwrap()), r#"{"x":1,"y":2}"#);
        let place = json(symbol.pointer("/id").unwrap());
        let inside = ok(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"entities","doc":"a","place":{place},"fields":["layer","geometry"]}}"#
            ),
        );
        let Some(Value::Array(items)) = inside.pointer("/result/items") else {
            panic!("{inside:?}")
        };
        assert_eq!(items.len(), 2, "{inside:?}");
        assert_eq!(
            items[0].pointer("/layer"),
            Some(&Value::String("KitLayer".into()))
        );
        let new_id = applied.pointer("/result/applied/2/id").unwrap();
        let query = format!(
            r#"{{"v":1,"op":"entities","doc":"a","ids":[{}],"fields":["geometry","hash"]}}"#,
            json(new_id)
        );
        let entity = ok(&mut session, &query);
        assert_eq!(
            entity.pointer("/result/items/0/geometry/name"),
            Some(&Value::String("Kit".into()))
        );
        let mut reopened = open(&written);
        assert_eq!(ok(&mut reopened, &query), entity);
        assert_eq!(
            ok(&mut reopened, r#"{"v":1,"op":"tables","doc":"a"}"#),
            tables
        );
        // 元の要素は ID とハッシュを保つ
        let after = ok(
            &mut reopened,
            r#"{"v":1,"op":"entities","doc":"a","fields":["hash"]}"#,
        );
        let Some(Value::Array(before)) = original.pointer("/result/items") else {
            panic!("{original:?}")
        };
        let Some(Value::Array(after)) = after.pointer("/result/items") else {
            panic!("{after:?}")
        };
        for item in before {
            assert!(after.contains(item), "{item:?}");
        }
        // 同じ名前は、大文字と小文字を区別せずに拒み、何も変えない
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"apply","doc":"a","edits":[{"kind":"add_symbol","name":"KIT","elements":[]}]}"#,
        );
        assert_eq!(status, Status::Failed, "{response:?}");
        assert!(
            json(&response).contains("change.symbol.unsupported"),
            "{response:?}"
        );
        assert!(session.take(0).is_none());
        assert_eq!(
            ok(&mut session, r#"{"v":1,"op":"tables","doc":"a"}"#),
            tables
        );
    }
}
