//! 管理 XML の更新・再読・停止を照らす自作の入力。要領の作成例は使わない。
#![cfg_attr(test, allow(dead_code))]
#![allow(clippy::unwrap_used, clippy::panic)] // 固定した合成入力と検査だけに使う。

use zumen_abi1::{Session, Status};
use zumen_base::CancelToken;
use zumen_drawing::Hash;
use zumen_json::{Value, Writer};

/// 元の入力の登録名。
pub const NAME: &str = "out/INDEX_C.XML";
/// 再読で保つ特殊文字と改行。
pub const CONTENT: &str = "更新<&>\n\r値";
/// 値の更新と 2 つの記録の追加。
pub const APPLY: &str = r#"{"v":1,"op":"delivery_apply","input":"out/INDEX_C.XML","edits":[{"kind":"set_text","path":[{"name":"title"}],"value":"更新<&>\n\r値"},{"kind":"set_text","path":[{"name":"entry","index":1},{"name":"file"}],"value":"TWO.P21"},{"kind":"set_text","path":[{"name":"empty"}],"value":"filled"},{"kind":"append","path":[],"child":{"name":"p:extra","value":"新","children":[{"name":"child","value":"value"}]}},{"kind":"append","path":[],"child":{"name":"entry","children":[{"name":"file","value":"THREE.P21"}]}}]}"#;
/// 元の入力とハッシュを得る変更なしの操作。
pub const UNCHANGED: &str = r#"{"v":1,"op":"delivery_apply","input":"out/INDEX_C.XML","edits":[]}"#;
/// 元の入力の登録を捨て、書き戻したバイト列を新しく登録するための操作。
pub const RESET: &str = r#"{"v":1,"op":"close","inputs":["out/INDEX_C.XML"]}"#;
/// 書き戻した成果品を自作の規則と照らす操作。
pub const CHECK: &str = r#"{"v":1,"op":"delivery_check","root":"out/","profiles":["profile"],"profile":{"id":"synthetic","edition":"1"}}"#;
/// DTD の名前・値・参照を照らす自作の規則。
pub const PROFILE: &str = r#"{"format":"0.1","id":"synthetic","edition":"1","revision":1,"license":"MIT","sources":{},"rules":[{"id":"links","check":"delivery-link","level":"error","elements":["file"],"source":{"basis":"chosen"}},{"id":"doctype","check":"delivery-doctype","level":"error","values":["synthetic.dtd"],"source":{"basis":"chosen"}},{"id":"value","check":"delivery-standard","level":"error","element":"empty","values":["filled"],"source":{"basis":"chosen"}}]}"#;
/// 文字コードと BOM の組。
pub const ENCODINGS: &[&str] = &[
    "utf8",
    "utf8-bom",
    "sjis",
    "utf16le",
    "utf16be",
    "utf16le-no-bom",
    "utf16be-no-bom",
];

/// 元の管理 XML を指定の符号化で作る。
pub fn source(kind: &str) -> Vec<u8> {
    let encoding = match kind {
        "sjis" => "Shift_JIS",
        "utf16le" | "utf16be" => "UTF-16",
        "utf16le-no-bom" => "UTF-16LE",
        "utf16be-no-bom" => "UTF-16BE",
        _ => "UTF-8",
    };
    let text = format!(
        "<?xml version=\"1.0\" encoding=\"{encoding}\"?>\n<!DOCTYPE package SYSTEM \"synthetic.dtd\">\n<package xmlns:p=\"urn:test\" version='1'>\n  <!-- keep -->\n  <title code='x'>old<![CDATA[ tail]]><?keep x?></title>\n  <entry><file>ONE.P21</file><note>A</note></entry>\n  <entry><file>MISSING.P21</file><note>B</note></entry>\n  <empty />\n</package>\n"
    );
    let newline = match kind {
        "utf8-bom" => "\n",
        "sjis" => "\r",
        _ => "\r\n",
    };
    let text = text.replace('\n', newline);
    match kind {
        "utf16le" => [
            vec![0xff, 0xfe],
            text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        ]
        .concat(),
        "utf16be" => [
            vec![0xfe, 0xff],
            text.encode_utf16().flat_map(u16::to_be_bytes).collect(),
        ]
        .concat(),
        "utf16le-no-bom" => text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        "utf16be-no-bom" => text.encode_utf16().flat_map(u16::to_be_bytes).collect(),
        "utf8-bom" => [vec![0xef, 0xbb, 0xbf], text.into_bytes()].concat(),
        // 元の合成入力は ASCII だけなので Shift_JIS と同じバイト列。
        _ => text.into_bytes(),
    }
}

/// 元の入力を登録した新しいセッション。
pub fn session(kind: &str) -> Session {
    let mut session = Session::new(b"").unwrap();
    session.push(NAME.as_bytes(), &source(kind)).unwrap();
    session
}

/// 成功する操作の応答を読む。
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
fn quote(value: &str) -> String {
    json(&Value::String(value.to_string()))
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 予算の上限だけを変えた操作。
pub fn budget_request(kind: &str, limit: u64) -> String {
    APPLY.replacen(
        "\"v\":1",
        &format!(r#""v":1,"budget":{{"{kind}":{limit}}}"#),
        1,
    )
}
/// 操作全体が成功する最小の予算を探す。
pub fn minimum_budget(kind: &str) -> u64 {
    let mut low = 0;
    let mut high = 16_000_000;
    let success = |limit| {
        session("utf8")
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
    low
}

/// 変更を残さずに拒否する要求と診断。
pub fn invalid_requests() -> Vec<(String, &'static str)> {
    let request = |edits: &str| {
        format!(r#"{{"v":1,"op":"delivery_apply","input":"{NAME}","edits":{edits}}}"#)
    };
    vec![
        (
            request(r#"[{"kind":"set_text","path":[{"name":"entry","index":2}],"value":"x"}]"#),
            "delivery.edit-path.missing",
        ),
        (
            request(r#"[{"kind":"set_text","path":[],"value":"x"}]"#),
            "xml.edit-children.unsupported",
        ),
        (
            request(r#"[{"kind":"set_text","path":[{"name":"title"}],"value":"\u0000"}]"#),
            "xml.edit-character.invalid",
        ),
        (
            request(r#"[{"kind":"append","path":[],"child":{"name":"1bad"}}]"#),
            "xml.edit-name.invalid",
        ),
        (
            request(r#"[{"kind":"append","path":[],"child":{"name":"q:bad"}}]"#),
            "xml.edit-namespace.missing",
        ),
        (
            request(
                r#"[{"kind":"set_text","path":[{"name":"empty"}],"value":"a"},{"kind":"append","path":[{"name":"empty"}],"child":{"name":"b"}}]"#,
            ),
            "xml.edit-range.invalid",
        ),
        (
            APPLY.replacen(
                "\"v\":1",
                &format!(r#""v":1,"hash":"{}""#, Hash::document(b"stale")),
                1,
            ),
            "delivery.edit-hash.stale",
        ),
        (
            APPLY.replacen("\"v\":1", "\"v\":1,\"hash\":\"invalid\"", 1),
            "call.value.invalid",
        ),
        (
            request(
                r#"[{"kind":"append","path":[],"child":{"name":"new"}},{"kind":"set_text","path":[{"name":"new"}],"value":"x"}]"#,
            ),
            "delivery.edit-path.missing",
        ),
    ]
}

/// 適合入力と外部検証用の自作ファイルを作る。
pub fn steps(output: Option<&std::path::Path>) -> Vec<String> {
    let mut steps = Vec::new();
    for kind in ENCODINGS {
        let input = source(kind);
        let mut session = session(kind);
        let condition = APPLY.replacen(
            "\"v\":1",
            &format!(r#""v":1,"hash":"{}""#, Hash::document(&input)),
            1,
        );
        let result = call(&mut session, &condition);
        let written = session.take(0).unwrap();
        call(&mut session, UNCHANGED);
        assert_eq!(session.take(0).unwrap(), input);
        call(&mut session, RESET);
        session.push(NAME.as_bytes(), &written).unwrap();
        session.push(b"profile", PROFILE.as_bytes()).unwrap();
        for file in ["ONE.P21", "TWO.P21", "THREE.P21"] {
            session
                .push(format!("out/{file}").as_bytes(), b"synthetic")
                .unwrap();
        }
        let checked = call(&mut session, CHECK);
        assert_eq!(
            checked.pointer("/diagnostics"),
            Some(&Value::Array(vec![])),
            "{checked:?}"
        );
        assert_eq!(
            checked.pointer("/result/managed/0/records"),
            Some(&zumen_json::parse(b"13").unwrap()),
            "{checked:?}"
        );
        assert_eq!(
            checked.pointer("/result/total"),
            Some(&zumen_json::parse(b"0").unwrap()),
            "{checked:?}"
        );
        if let Some(output) = output {
            std::fs::create_dir_all(output).unwrap();
            std::fs::write(output.join(format!("{kind}-before.xml")), &input).unwrap();
            std::fs::write(output.join(format!("{kind}-after.xml")), &written).unwrap();
        }
        steps.extend([
            r#"{"session":{}}"#.into(),
            format!(r#"{{"push":{{"name":"{NAME}","hex":"{}"}}}}"#,hex(&input)),
            format!(r#"{{"call":{condition},"expect":{{"/result/applied":5,"/diagnostics":[],"/result/hash":{}}}}}"#,json(result.pointer("/result/hash").unwrap())),
            r#"{"take":0}"#.into(),
            format!(r#"{{"call":{UNCHANGED},"expect":{{"/result/hash":"{}"}}}}"#, Hash::document(&input)),
            r#"{"take":0}"#.into(),
            format!(r#"{{"call":{RESET}}}"#),
            format!(r#"{{"push":{{"name":"{NAME}","hex":"{}"}}}}"#,hex(&written)),
            format!(r#"{{"push":{{"name":"profile","text":{}}}}}"#,quote(PROFILE)),
        ]);
        for file in ["ONE.P21", "TWO.P21", "THREE.P21"] {
            steps.push(format!(
                r#"{{"push":{{"name":"out/{file}","text":"synthetic"}}}}"#
            ));
        }
        steps.push(format!(r#"{{"call":{CHECK},"expect":{{"/result/total":0,"/result/managed/0/records":13,"/result/managed/0/links":3,"/result/managed/0/missing":[],"/diagnostics":[]}}}}"#));
    }
    for kind in ["input_bytes", "decoded_bytes", "records", "depth", "work"] {
        let limit = minimum_budget(kind);
        steps.extend([
            r#"{"session":{}}"#.into(),
            format!(
                r#"{{"push":{{"name":"{NAME}","hex":"{}"}}}}"#,
                hex(&source("utf8"))
            ),
            format!(
                r#"{{"call":{},"status":"failed","expect":{{"/blobs":[]}},"absent":["/result"]}}"#,
                budget_request(kind, limit - 1)
            ),
            format!(r#"{{"call":{}}}"#, budget_request(kind, limit)),
            r#"{"take":0}"#.into(),
        ]);
    }
    for (request, code) in invalid_requests() {
        steps.push(format!(r#"{{"call":{request},"status":"failed","expect":{{"/diagnostics/0/code":"{code}","/blobs":[]}},"absent":["/result"]}}"#));
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
        "{{\n  \"description\": \"管理 XML の項目の更新と追加・7 種の符号化・宣言と参照の再読・予算停止と再利用。delivery_apply_case.rs から生成する\",\n  \"module\": \"drawing\",\n  \"steps\": [\n    {}\n  ]\n}}\n",
        steps(output.as_deref()).join(",\n    ")
    );
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bindings/conformance/cases/delivery_apply.json");
    if first.as_deref() == Some("--check") {
        if std::fs::read_to_string(path)
            .map_err(|e| e.to_string())?
            .replace("\r\n", "\n")
            != result
        {
            return Err("管理 XML の適合入力が古い".into());
        }
    } else {
        std::fs::write(path, result).map_err(|e| e.to_string())?;
    }
    Ok(())
}
