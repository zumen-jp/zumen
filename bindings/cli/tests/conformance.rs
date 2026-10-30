//! 適合試験を、CLI の 2 つの形で流す（決定 0006・0197）。
//!
//! - `zumen serve --stdio`: すべての組を流し、ハッシュの一覧が `hashes.txt` と一致することを確かめる
//! - `zumen mcp`: 道具で表せる組（セッションの設定を JSON の値で渡し、要求が知っている op のオブジェクトで、
//!   入力の名前が文字列のもの）だけを流し、その組の行が `hashes.txt` と一致することを確かめる

// 試験の道具の関数の中でも、試験と同じく unwrap・panic・添字を使う
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use zumen_abi1::Status;
use zumen_conformance::{Case, Engine, Report, cases, differences, read_hashes, run};
use zumen_json::{Value, Writer};

/// MCP の道具の結果の `_meta` の、境界の状態のキー
const STATUS_KEY: &str = "io.github.zumen-jp~1status";

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(bytes: &[u8]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[(n >> (18 - 6 * i) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn unbase64(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);
    for c in text.bytes().filter(|c| *c != b'=') {
        acc = acc << 6 | ALPHABET.iter().position(|a| *a == c).unwrap() as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    out
}

fn code(name: &str) -> i32 {
    Status::ALL
        .iter()
        .find(|status| status.name() == name)
        .map_or(-1, |status| status.code())
}

fn text(value: &Value, pointer: &str) -> Option<String> {
    match value.pointer(pointer) {
        Some(Value::String(text)) => Some(text.clone()),
        _ => None,
    }
}

/// 子プロセスの zumen と、1 行に 1 つの JSON-RPC でやり取りする
struct Child2 {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    next: u64,
}

impl Child2 {
    fn spawn(args: &[&str]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_zumen"))
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            input,
            output,
            next: 1,
        }
    }

    /// メソッドを呼び、result を返す。params は JSON の文
    fn request(&mut self, method: &str, params: &[u8]) -> Value {
        let id = self.next;
        self.next += 1;
        let mut line =
            format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":"#).into_bytes();
        line.extend_from_slice(params);
        line.extend_from_slice(b"}\n");
        self.input.write_all(&line).unwrap();
        self.input.flush().unwrap();
        let mut reply = String::new();
        self.output.read_line(&mut reply).unwrap();
        let reply = zumen_json::parse(reply.as_bytes()).unwrap();
        assert_eq!(
            reply.pointer("/id").map(compact),
            Some(id.to_string().into_bytes()),
            "id が合わない"
        );
        reply
            .pointer("/result")
            .unwrap_or_else(|| panic!("{method} が誤りを返した: {:?}", reply))
            .clone()
    }
}

impl Drop for Child2 {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn compact(value: &Value) -> Vec<u8> {
    let mut out = Writer::new();
    out.value(value);
    out.into_bytes()
}

fn params(fields: &[(&str, &str)]) -> Vec<u8> {
    let mut out = Writer::new();
    out.begin_object();
    for (key, value) in fields {
        out.key(key);
        out.string(value);
    }
    out.end_object();
    out.into_bytes()
}

/// `zumen serve --stdio` を、境界の 1 つの形として使う
struct Serve(Child2);

impl Engine for Serve {
    fn session_new(&mut self, options: &[u8]) -> i32 {
        let result = self.0.request(
            "session_new",
            &params(&[("options_base64", &base64(options))]),
        );
        code(&text(&result, "/status").unwrap())
    }

    fn push(&mut self, name: &[u8], data: &[u8]) -> i32 {
        let result = self.0.request(
            "push",
            &params(&[("name_base64", &base64(name)), ("base64", &base64(data))]),
        );
        code(&text(&result, "/status").unwrap())
    }

    fn call(&mut self, request: &[u8]) -> (i32, Option<Vec<u8>>) {
        let result = self
            .0
            .request("call", &params(&[("request_base64", &base64(request))]));
        (
            code(&text(&result, "/status").unwrap()),
            text(&result, "/response").map(String::into_bytes),
        )
    }

    fn take(&mut self, blob: u32) -> (i32, Option<Vec<u8>>) {
        let result = self
            .0
            .request("take", format!(r#"{{"blob":{blob}}}"#).as_bytes());
        (
            code(&text(&result, "/status").unwrap()),
            text(&result, "/base64").map(|data| unbase64(&data)),
        )
    }
}

/// `zumen mcp` の道具を、境界の 1 つの形として使う
struct Mcp {
    child: Child2,
    session: Option<String>,
}

impl Mcp {
    fn new() -> Self {
        let mut mcp = Self {
            child: Child2::spawn(&["mcp"]),
            session: None,
        };
        mcp.session_new(b"");
        mcp
    }

    fn tool(&mut self, name: &str, arguments: &[u8]) -> Value {
        let mut params = format!(r#"{{"name":"{name}","arguments":"#).into_bytes();
        params.extend_from_slice(arguments);
        params.push(b'}');
        self.child.request("tools/call", &params)
    }

    /// セッションの名前を足した道具の引数
    fn arguments(&self, fields: &[(&str, Value)]) -> Option<Vec<u8>> {
        let session = self.session.as_ref()?;
        let mut out = Writer::new();
        out.begin_object();
        out.key("session");
        out.string(session);
        for (key, value) in fields {
            out.key(key);
            out.value(value);
        }
        out.end_object();
        Some(out.into_bytes())
    }
}

impl Engine for Mcp {
    fn session_new(&mut self, options: &[u8]) -> i32 {
        if let Some(session) = self.session.take() {
            self.tool("session_close", &params(&[("session", &session)]));
        }
        let arguments = if options.is_empty() {
            b"{}".to_vec()
        } else {
            let mut arguments = br#"{"options":"#.to_vec();
            arguments.extend_from_slice(options);
            arguments.push(b'}');
            arguments
        };
        let result = self.tool("session_new", &arguments);
        let status = text(&result, &format!("/_meta/{STATUS_KEY}")).unwrap();
        if status == "ok" {
            let reply = zumen_json::parse(text(&result, "/content/0/text").unwrap().as_bytes());
            self.session = text(&reply.unwrap(), "/session");
        }
        code(&status)
    }

    fn push(&mut self, name: &[u8], data: &[u8]) -> i32 {
        let name = Value::String(String::from_utf8(name.to_vec()).unwrap());
        let data = Value::String(base64(data));
        let Some(arguments) = self.arguments(&[("name", name), ("base64", data)]) else {
            return Status::Invalid.code();
        };
        let result = self.tool("push", &arguments);
        code(&text(&result, &format!("/_meta/{STATUS_KEY}")).unwrap())
    }

    fn call(&mut self, request: &[u8]) -> (i32, Option<Vec<u8>>) {
        let Ok(Value::Object(request)) = zumen_json::parse(request) else {
            panic!("道具で表せない要求");
        };
        let Some(Value::String(op)) = request.get("op") else {
            panic!("op のない要求");
        };
        let fields: Vec<(&str, Value)> = request
            .iter()
            .filter(|(key, _)| *key != "op")
            .map(|(key, value)| (key, value.clone()))
            .collect();
        let Some(arguments) = self.arguments(&fields) else {
            return (Status::Invalid.code(), None);
        };
        let result = self.tool(op, &arguments);
        let status = text(&result, &format!("/_meta/{STATUS_KEY}")).unwrap();
        let response = (status != "invalid" && status != "busy")
            .then(|| text(&result, "/content/0/text").unwrap().into_bytes());
        (code(&status), response)
    }

    fn take(&mut self, blob: u32) -> (i32, Option<Vec<u8>>) {
        let Some(arguments) = self.arguments(&[(
            "blob",
            zumen_json::parse(blob.to_string().as_bytes()).unwrap(),
        )]) else {
            return (Status::Invalid.code(), None);
        };
        let result = self.tool("take", &arguments);
        let status = text(&result, &format!("/_meta/{STATUS_KEY}")).unwrap();
        (
            code(&status),
            text(&result, "/content/1/resource/blob").map(|data| unbase64(&data)),
        )
    }
}

/// 組が MCP の道具で表せるか
fn fits_mcp(case: &Case, ops: &[String]) -> bool {
    let Some(Value::Array(steps)) = case.value.pointer("/steps") else {
        return false;
    };
    steps.iter().all(|step| {
        if let Some(session) = step.pointer("/session") {
            return session.pointer("/options_text").is_none()
                && matches!(session.pointer("/options"), None | Some(Value::Object(_)));
        }
        if let Some(push) = step.pointer("/push") {
            return matches!(push.pointer("/name"), Some(Value::String(_)));
        }
        if step.pointer("/take").is_some() {
            return true;
        }
        match step.pointer("/call/op") {
            Some(Value::String(op)) => ops.contains(op),
            _ => false,
        }
    })
}

fn check(report: &Report, expected: &[String]) {
    assert!(
        report.failures.is_empty(),
        "期待と違った:\n{}",
        report.failures.join("\n")
    );
    assert!(
        report.hashes == expected,
        "ハッシュの一覧が hashes.txt と合わない:\n{}",
        differences(expected, &report.hashes)
    );
}

#[test]
fn through_serve_stdio() {
    let report = run(&cases().unwrap(), || {
        Box::new(Serve(Child2::spawn(&["serve", "--stdio"]))) as Box<dyn Engine>
    });
    check(&report, &read_hashes().unwrap());
}

#[test]
fn through_mcp_tools() {
    let tools = zumen_json::parse(include_bytes!("../src/tools.json")).unwrap();
    let Value::Array(tools) = tools else {
        panic!("tools.json が配列でない");
    };
    let ops: Vec<String> = tools
        .iter()
        .filter_map(|tool| text(tool, "/name"))
        .collect();
    let cases: Vec<Case> = cases()
        .unwrap()
        .into_iter()
        .filter(|case| fits_mcp(case, &ops))
        .collect();
    assert!(cases.len() >= 8, "MCP で流せる組が少ない: {}", cases.len());
    let names: Vec<&str> = cases.iter().map(|case| case.name.as_str()).collect();
    let expected: Vec<String> = read_hashes()
        .unwrap()
        .into_iter()
        .filter(|line| {
            line.split(' ')
                .next()
                .is_some_and(|name| names.contains(&name))
        })
        .collect();
    let report = run(&cases, || Box::new(Mcp::new()) as Box<dyn Engine>);
    check(&report, &expected);
}
