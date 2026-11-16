//! 空の DXF の作成・作図・再読を照らす適合入力。
#![cfg_attr(test, allow(dead_code))]
#![allow(clippy::unwrap_used, clippy::panic)] // 合成入力と適合テストの生成だけに使う。

use zumen_abi1::{Session, Status};
use zumen_base::CancelToken;
use zumen_json::{Value, Writer};

/// 入力なしの新規作成。
pub const CREATE: &str = r#"{"v":1,"op":"create","format":"dxf","to":"a"}"#;
/// 文書の概要。
pub const DRAWING: &str = r#"{"v":1,"op":"drawing","doc":"a"}"#;
/// 既定と追加後の表。
pub const TABLES: &str = r#"{"v":1,"op":"tables","doc":"a"}"#;
/// 再読して比べる全属性。
pub const ENTITIES: &str =
    r#"{"v":1,"op":"entities","doc":"a","fields":["geometry","hash","layer","style"]}"#;
/// 同じ要求でレイヤと線・円・文字を足す。
pub const APPLY: &str = r#"{"v":1,"op":"apply","doc":"a","edits":[{"kind":"add_layer","name":"Draft","color":{"aci":3}},{"kind":"add","place":"space/0","layer":"Draft","geometry":{"shape":"line","start":[1,2],"end":[3,4]}},{"kind":"add","place":"space/0","geometry":{"shape":"circle","center":[8,9],"radius":2}},{"kind":"add","place":"space/0","geometry":{"shape":"text","position":[10,20],"content":"Test","height":2.5,"width_factor":1,"rotation":0,"style":"Standard"}}]}"#;

/// 成功する公開操作を呼ぶ。
pub fn call(session: &mut Session, request: &str) -> Value {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    assert_eq!(
        reply.status,
        Status::Ok,
        "{}",
        String::from_utf8_lossy(&reply.response)
    );
    zumen_json::parse(&reply.response).unwrap()
}

fn json(value: &Value) -> String {
    let mut writer = Writer::new();
    writer.value(value);
    String::from_utf8(writer.into_bytes()).unwrap()
}

/// 予算だけを変えた要求。
pub fn budget_request(kind: &str, limit: u64) -> String {
    format!(r#"{{"v":1,"op":"create","format":"dxf","to":"a","budget":{{"{kind}":{limit}}}}}"#)
}

/// 同じ要求が成功する最小の予算を探す。
pub fn minimum_budget(kind: &str) -> u64 {
    let mut low = 0;
    let mut high = 16_000_000;
    let success = |limit| {
        Session::new(b"")
            .unwrap()
            .call(budget_request(kind, limit).as_bytes(), &CancelToken::new())
            .status
            == Status::Ok
    };
    assert!(success(high));
    while low < high {
        let mid = low + (high - low) / 2;
        if success(mid) {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    high
}

/// 再現する操作列と、外部検証用の自作 DXF を作る。
pub fn steps(output: Option<&std::path::Path>) -> Vec<String> {
    let mut steps = Vec::new();
    for unit in [
        None,
        Some("millimeter"),
        Some("centimeter"),
        Some("meter"),
        Some("inch"),
        Some("foot"),
        Some("unknown"),
    ] {
        let request = unit.map_or_else(
            || CREATE.to_string(),
            |unit| format!(r#"{{"v":1,"op":"create","format":"dxf","to":"a","unit":"{unit}"}}"#),
        );
        let mut session = Session::new(b"").unwrap();
        let created = call(&mut session, &request);
        let empty = session.take(0).unwrap();
        let drawing = call(&mut session, DRAWING);
        let tables = call(&mut session, TABLES);
        let mut reread = Session::new(b"").unwrap();
        reread.push(b"a", &empty).unwrap();
        call(&mut reread, r#"{"v":1,"op":"open","input":"a"}"#);
        assert_eq!(call(&mut reread, DRAWING), drawing);
        assert_eq!(call(&mut reread, TABLES), tables);
        call(&mut session, APPLY);
        let written = session.take(0).unwrap();
        let entities = call(&mut session, ENTITIES);
        let tables = call(&mut session, TABLES);
        let mut reread = Session::new(b"").unwrap();
        reread.push(b"a", &written).unwrap();
        call(&mut reread, r#"{"v":1,"op":"open","input":"a"}"#);
        assert_eq!(call(&mut reread, ENTITIES), entities);
        assert_eq!(call(&mut reread, TABLES), tables);
        if let Some(output) = output {
            std::fs::create_dir_all(output).unwrap();
            let suffix = unit.unwrap_or("default");
            std::fs::write(output.join(format!("empty-{suffix}.dxf")), &empty).unwrap();
            std::fs::write(output.join(format!("edited-{suffix}.dxf")), &written).unwrap();
        }
        let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        steps.extend([
            r#"{"session":{}}"#.into(),
            format!(r#"{{"call":{request},"expect":{{"/result/elements":0,"/result/format":"dxf","/diagnostics":[]}}}}"#),
            r#"{"take":0}"#.into(),
            format!(r#"{{"call":{DRAWING},"expect":{{"/result/hash":{}}}}}"#,json(created.pointer("/result/hash").unwrap())),
            format!(r#"{{"call":{TABLES}}}"#),
            format!(r#"{{"call":{APPLY}}}"#),
            r#"{"take":0}"#.into(),
            format!(r#"{{"call":{ENTITIES}}}"#),
            format!(r#"{{"call":{TABLES}}}"#),
            r#"{"session":{}}"#.into(),
            format!(r#"{{"push":{{"name":"a","hex":"{}"}}}}"#,hex(&written)),
            r#"{"call":{"v":1,"op":"open","input":"a"}}"#.into(),
            format!(r#"{{"call":{ENTITIES}}}"#),
            format!(r#"{{"call":{TABLES}}}"#),
        ]);
    }
    for kind in ["work", "decoded_bytes", "records"] {
        let limit = minimum_budget(kind);
        let failed = budget_request(kind, limit - 1);
        let success = budget_request(kind, limit);
        steps.extend([
            r#"{"session":{}}"#.into(),
            format!(r#"{{"call":{failed},"status":"failed"}}"#),
            format!(r#"{{"call":{success}}}"#),
            r#"{"take":0}"#.into(),
        ]);
    }
    for (request, code) in [
        (
            r#"{"v":1,"op":"create","format":"dwg","to":"new"}"#,
            "create.format.unsupported",
        ),
        (
            r#"{"v":1,"op":"create","format":"dxf","to":"../new"}"#,
            "call.value.invalid",
        ),
        (CREATE, "call.document.exists"),
    ] {
        steps.push(format!(r#"{{"call":{request},"status":"failed","expect":{{"/diagnostics/0/code":"{code}","/blobs":[]}}}}"#));
    }
    steps
}

#[cfg(not(test))]
fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let first = args.next();
    let output = if first.as_deref() == Some("--output") {
        Some(std::path::PathBuf::from(args.next().ok_or("出力先がない")?))
    } else {
        None
    };
    let result = format!(
        "{{\n  \"description\": \"空の DXF の作成・全単位・作図と再読・予算停止と再利用。create_case.rs から生成する\",\n  \"module\": \"drawing\",\n  \"steps\": [\n    {}\n  ]\n}}\n",
        steps(output.as_deref()).join(",\n    ")
    );
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bindings/conformance/cases/create.json");
    if first.as_deref() == Some("--check") {
        if std::fs::read_to_string(path)
            .map_err(|e| e.to_string())?
            .replace("\r\n", "\n")
            != result
        {
            return Err("新規作成の適合入力が古い".into());
        }
    } else {
        std::fs::write(path, result).map_err(|e| e.to_string())?;
    }
    Ok(())
}
