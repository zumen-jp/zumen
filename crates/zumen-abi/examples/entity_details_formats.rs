//! 既存の合成入力から、4 形式の restyle と属性の再読を照らす適合入力を作る。
use zumen_abi1::{Session, Status};
use zumen_base::CancelToken;
use zumen_json::{ToJson, Value, Writer};

fn read(bytes: &[u8]) -> Result<Value, String> {
    zumen_json::parse(bytes).map_err(|e| format!("{e:?}"))
}
fn text<'a>(value: &'a Value, path: &str) -> Result<&'a str, String> {
    match value.pointer(path) {
        Some(Value::String(value)) => Ok(value),
        _ => Err(format!("文字列がない: {path}")),
    }
}
fn json(value: &Value) -> Result<String, String> {
    let mut writer = Writer::new();
    value.to_json(&mut writer);
    String::from_utf8(writer.into_bytes()).map_err(|e| e.to_string())
}
fn call(session: &mut Session, request: &str) -> Result<Vec<u8>, String> {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    if reply.status != Status::Ok {
        return Err(String::from_utf8_lossy(&reply.response).into_owned());
    }
    Ok(reply.response)
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn main() -> Result<(), String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cases = root.join("bindings/conformance/cases");
    let mut steps = Vec::new();
    let query = r#"{"v":1,"op":"entities","doc":"a","fields":["style","extensions","hash"]}"#;
    for (case, system, number) in [
        ("apply", "aci", 3),
        ("open_dwg", "aci", 3),
        ("open_jww", "jww", 3),
        ("open_sxf", "sxf", 2),
    ] {
        let fixture =
            read(&std::fs::read(cases.join(format!("{case}.json"))).map_err(|e| e.to_string())?)?;
        let push = fixture.pointer("/steps/0/push").ok_or("入力がない")?;
        let input = match push.pointer("/text") {
            Some(Value::String(value)) => value.as_bytes().to_vec(),
            _ => text(push, "/hex")?
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| {
                    u8::from_str_radix(std::str::from_utf8(pair).map_err(|e| e.to_string())?, 16)
                        .map_err(|e| e.to_string())
                })
                .collect::<Result<Vec<_>, _>>()?,
        };
        let mut session = Session::new(b"").map_err(|e| format!("{e:?}"))?;
        session.push(b"a", &input).map_err(|e| format!("{e:?}"))?;
        let open = r#"{"v":1,"op":"open","input":"a"}"#;
        call(&mut session, open)?;
        let before = read(&call(&mut session, query)?)?;
        let id = json(&Value::String(text(&before, "/result/items/0/id")?.into()))?;
        let hash = json(&Value::String(
            text(&before, "/result/items/0/hash")?.into(),
        ))?;
        let apply = format!(
            r#"{{"v":1,"op":"apply","doc":"a","edits":[{{"kind":"restyle","id":{id},"hash":{hash},"color":{{"{system}":{number}}}}}]}}"#
        );
        call(&mut session, &apply)?;
        let written = session.take(0).ok_or("書き戻したファイルがない")?;
        let after = call(&mut session, query)?;
        let mut reread = Session::new(b"").map_err(|e| format!("{e:?}"))?;
        reread.push(b"a", &written).map_err(|e| format!("{e:?}"))?;
        call(&mut reread, open)?;
        if call(&mut reread, query)? != after {
            return Err(format!("{case}: 書き戻し後の属性が再読と合わない"));
        }
        let expected = format!(
            r#"{{"/result/items/0/id":{id},"/result/items/0/style/color/{system}":{number}}}"#
        );
        steps.extend([
            r#"{"session":{}}"#.into(),
            format!(r#"{{"push":{{"name":"a","hex":"{}"}}}}"#, hex(&input)),
            format!(r#"{{"call":{open}}}"#),
            format!(r#"{{"call":{query}}}"#),
            format!(r#"{{"call":{apply}}}"#),
            r#"{"take":0}"#.into(),
            format!(r#"{{"call":{query},"expect":{expected}}}"#),
            r#"{"session":{}}"#.into(),
            format!(r#"{{"push":{{"name":"a","hex":"{}"}}}}"#, hex(&written)),
            format!(r#"{{"call":{open}}}"#),
            format!(r#"{{"call":{query},"expect":{expected}}}"#),
        ]);
    }
    let result = format!(
        "{{\n  \"description\": \"DXF・DWG・JWW・SXF の restyle とスタイル・拡張の再読。entity_details_formats.rs から生成する\",\n  \"module\": \"drawing\",\n  \"steps\": [\n    {}\n  ]\n}}\n",
        steps.join(",\n    ")
    );
    let output = cases.join("entity_details_formats.json");
    if std::env::args().nth(1).as_deref() == Some("--check") {
        if std::fs::read_to_string(output)
            .map_err(|e| e.to_string())?
            .replace("\r\n", "\n")
            != result
        {
            return Err("4 形式の属性の適合入力が古い".into());
        }
    } else {
        std::fs::write(output, result).map_err(|e| e.to_string())?;
    }
    Ok(())
}
