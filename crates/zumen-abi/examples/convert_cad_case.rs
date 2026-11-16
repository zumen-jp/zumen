//! DXF → P21/JWW の見積もり・新規書き出し・再読の適合入力（#360）。
#![cfg_attr(test, allow(dead_code))]
#![allow(clippy::unwrap_used, clippy::panic)] // 自作入力と固定の期待値。
use zumen_abi1::{Session, Status};
use zumen_base::CancelToken;
use zumen_json::{Value, Writer};

/// 数値・文字・表はテストのために組み立てたもの。
pub fn source() -> Vec<u8> {
    concat!(
        "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1021\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n",
        "0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLTYPE\n",
        "0\nLTYPE\n5\n10\n2\nCONTINUOUS\n70\n0\n3\n\n72\n65\n73\n0\n40\n0\n",
        "0\nLTYPE\n5\n11\n2\nDASHED\n70\n0\n3\nTest\n72\n65\n73\n2\n40\n3\n49\n2\n74\n0\n49\n-1\n74\n0\n",
        "0\nENDTAB\n0\nTABLE\n2\nLAYER\n",
        "0\nLAYER\n5\n12\n2\n図形\n70\n0\n62\n1\n6\nDASHED\n370\n35\n",
        "0\nENDTAB\n0\nTABLE\n2\nSTYLE\n0\nSTYLE\n5\n13\n2\nStandard\n70\n0\n40\n0\n41\n1\n50\n0\n3\nArial\n",
        "0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n",
        "0\nLINE\n5\nA1\n8\n図形\n10\n1\n20\n2\n11\n11\n21\n12\n",
        "0\nCIRCLE\n5\nA2\n8\n図形\n10\n20\n20\n30\n40\n4\n",
        "0\nARC\n5\nA3\n8\n図形\n10\n40\n20\n50\n40\n5\n50\n10\n51\n130\n",
        "0\nLWPOLYLINE\n5\nA4\n8\n図形\n90\n3\n70\n0\n10\n0\n20\n20\n10\n10\n20\n30\n10\n20\n20\n20\n",
        "0\nTEXT\n5\nA5\n8\n図形\n10\n30\n20\n40\n40\n3\n1\n変換 Test\n7\nStandard\n41\n1\n50\n15\n",
        "0\nPOINT\n5\nA6\n8\n図形\n10\n60\n20\n70\n",
        "0\nSPLINE\n5\nAF\n8\n図形\n70\n0\n71\n1\n72\n4\n73\n2\n74\n0\n",
        "40\n0\n40\n0\n40\n1\n40\n1\n10\n0\n20\n0\n10\n10\n20\n10\n",
        "0\nENDSEC\n0\nEOF\n"
    ).as_bytes().to_vec()
}
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
/// 自作 DXF を開く。
pub fn open() -> Session {
    let mut session = Session::new(b"").unwrap();
    session.push(b"source.dxf", &source()).unwrap();
    call(&mut session, r#"{"v":1,"op":"open","input":"source.dxf"}"#);
    session
}
/// 実変換か、上限を指定した見積もりの要求。
pub fn request(format: &str, estimate: bool, limit: u64) -> String {
    if estimate {
        format!(
            r#"{{"v":1,"op":"convert_estimate","doc":"source.dxf","format":"{format}","to":"out","limit":{limit}}}"#
        )
    } else {
        format!(r#"{{"v":1,"op":"convert","doc":"source.dxf","format":"{format}","to":"out"}}"#)
    }
}
/// 再読して照らす要素。
pub const ENTITIES: &str =
    r#"{"v":1,"op":"entities","doc":"out","fields":["geometry","hash","layer","style"]}"#;
/// 再読して照らす表。
pub const TABLES: &str = r#"{"v":1,"op":"tables","doc":"out"}"#;

/// 同じ合成入力が成功する最小の予算を探す。
pub fn minimum_budget(format: &str, kind: &str) -> u64 {
    let success = |limit| {
        let mut session = open();
        session
            .call(
                budget_request(format, kind, limit).as_bytes(),
                &CancelToken::new(),
            )
            .status
            == Status::Ok
    };
    let (mut low, mut high) = (0, 64_000_000);
    assert!(success(high));
    while low < high {
        let middle = low + (high - low) / 2;
        if success(middle) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    high
}

/// 予算の値だけを変えた実変換の要求。
pub fn budget_request(format: &str, kind: &str, limit: u64) -> String {
    request(format, false, 0).replace(
        "\"to\":\"out\"",
        &format!("\"to\":\"out\",\"budget\":{{\"{kind}\":{limit}}}"),
    )
}
fn json(value: &Value) -> String {
    let mut w = Writer::new();
    w.value(value);
    String::from_utf8(w.into_bytes()).unwrap()
}
fn hex(value: &[u8]) -> String {
    value.iter().map(|v| format!("{v:02x}")).collect()
}

/// 適合入力と外部照合用の合成ファイルを作る。
pub fn steps(output: Option<&std::path::Path>) -> Vec<String> {
    let mut steps = Vec::new();
    for format in ["sxf", "jww"] {
        let mut session = open();
        let short = call(&mut session, &request(format, true, 0));
        let full = call(&mut session, &request(format, true, 100));
        assert_eq!(
            short.pointer("/result/estimate/hash"),
            full.pointer("/result/estimate/hash")
        );
        assert_eq!(
            short.pointer("/result/estimate/counts"),
            full.pointer("/result/estimate/counts")
        );
        let result = call(&mut session, &request(format, false, 0));
        let bytes = session.take(0).unwrap();
        assert_eq!(
            full.pointer("/result/estimate/hash"),
            result.pointer("/result/hash")
        );
        let entities = call(&mut session, ENTITIES);
        let tables = call(&mut session, TABLES);
        let mut reopened = Session::new(b"").unwrap();
        reopened.push(b"out", &bytes).unwrap();
        call(&mut reopened, r#"{"v":1,"op":"open","input":"out"}"#);
        assert_eq!(call(&mut reopened, ENTITIES), entities);
        assert_eq!(call(&mut reopened, TABLES), tables);
        let expected = if format == "sxf" { 6 } else { 7 };
        assert_eq!(
            result.pointer("/result/elements"),
            Some(&zumen_json::parse(expected.to_string().as_bytes()).unwrap()),
            "{format}: {}",
            json(&result)
        );
        if let Some(output) = output {
            std::fs::create_dir_all(output).unwrap();
            std::fs::write(
                output.join(if format == "sxf" {
                    "converted.p21"
                } else {
                    "converted.jww"
                }),
                &bytes,
            )
            .unwrap();
            std::fs::write(
                output.join(format!("{format}-entities.json")),
                json(&entities),
            )
            .unwrap();
            std::fs::write(output.join(format!("{format}-tables.json")), json(&tables)).unwrap();
            std::fs::write(output.join(format!("{format}-estimate.json")), json(&full)).unwrap();
        }
        steps.extend([
            r#"{"session":{}}"#.into(),
            format!(r#"{{"push":{{"name":"source.dxf","hex":"{}"}}}}"#,hex(&source())),
            r#"{"call":{"v":1,"op":"open","input":"source.dxf"}}"#.into(),
            format!(r#"{{"call":{}}}"#,request(format,true,0)),
            format!(r#"{{"call":{}}}"#,request(format,true,100)),
            format!(r#"{{"call":{},"expect":{{"/result/format":"{format}","/result/elements":{expected},"/result/hash":{}}}}}"#,
                request(format,false,0),json(result.pointer("/result/hash").unwrap())),
            r#"{"take":0}"#.into(),
            format!(r#"{{"call":{ENTITIES}}}"#),format!(r#"{{"call":{TABLES}}}"#),
            r#"{"session":{}}"#.into(),
            format!(r#"{{"push":{{"name":"out","hex":"{}"}}}}"#,hex(&bytes)),
            r#"{"call":{"v":1,"op":"open","input":"out"}}"#.into(),
            format!(r#"{{"call":{ENTITIES}}}"#),format!(r#"{{"call":{TABLES}}}"#),
        ]);
    }
    for format in ["sxf", "jww"] {
        for kind in ["work", "decoded_bytes"] {
            let minimum = minimum_budget(format, kind);
            steps.extend([
                r#"{"session":{}}"#.into(),
                format!(
                    r#"{{"push":{{"name":"source.dxf","hex":"{}"}}}}"#,
                    hex(&source())
                ),
                r#"{"call":{"v":1,"op":"open","input":"source.dxf"}}"#.into(),
                format!(
                    r#"{{"call":{},"status":"failed","expect":{{"/blobs":[]}}}}"#,
                    budget_request(format, kind, minimum - 1)
                ),
                format!(r#"{{"call":{}}}"#, budget_request(format, kind, minimum)),
                r#"{"take":0}"#.into(),
            ]);
        }
    }
    steps
}
#[cfg(not(test))]
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let output = if args.first().map(String::as_str) == Some("--output") {
        args.get(1).map(std::path::Path::new)
    } else {
        None
    };
    let result = format!(
        "{{\"description\":\"自作 DXF から P21・JWW の変換と再読、忠実度・予算停止\",\"module\":\"drawing\",\"steps\":[{}]}}\n",
        steps(output).join(",\n")
    );
    if output.is_some() {
        use std::io::Write as _;
        std::io::stdout()
            .lock()
            .write_all(result.as_bytes())
            .unwrap();
    } else {
        std::fs::write("bindings/conformance/cases/convert_cad.json", result).unwrap();
    }
}
