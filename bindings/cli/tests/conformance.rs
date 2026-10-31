//! 適合試験を、CLI の形で流す（決定 0006・0197・0200）。
//!
//! - `zumen serve --stdio`: すべての組を流し、ハッシュの一覧が `hashes.txt` と一致することを確かめる
//! - `zumen serve --http`: すべての組を、HTTP の JSON-RPC と本文のままの push・take で流す
//! - 実行の形 `process`: すべての組を、子プロセスの CLI に送るセッションで流す
//! - `zumen mcp`: 道具で表せる組（セッションの設定を JSON の値で渡し、要求が知っている op のオブジェクトで、
//!   入力の名前が文字列のもの）だけを流し、その組の行が `hashes.txt` と一致することを確かめる

// 試験の道具の関数の中でも、試験と同じく unwrap・panic・添字を使う
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::rc::Rc;

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

/// `zumen serve --http` を待ち受けのアドレスで起こしたもの
struct HttpServer {
    child: Child,
    address: String,
}

impl HttpServer {
    fn spawn() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_zumen"))
            .args(["serve", "--http", "--listen", "127.0.0.1:0"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let address = line
            .trim()
            .strip_prefix("http://")
            .unwrap()
            .trim_end_matches('/')
            .to_string();
        Self { child, address }
    }

    /// 要求を送り、状態の番号・Content-Type・本文を返す
    fn send(&self, method: &str, path: &str, kind: &str, body: &[u8]) -> (u16, String, Vec<u8>) {
        let mut stream = TcpStream::connect(&self.address).unwrap();
        let head = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\r\n",
            self.address,
            body.len()
        );
        stream.write_all(head.as_bytes()).unwrap();
        stream.write_all(body).unwrap();
        let mut reply = Vec::new();
        stream.read_to_end(&mut reply).unwrap();
        let split = reply.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let head = String::from_utf8(reply[..split].to_vec()).unwrap();
        let code = head.split(' ').nth(1).unwrap().parse().unwrap();
        let kind = head
            .lines()
            .find_map(|line| line.strip_prefix("Content-Type: "))
            .unwrap_or_default()
            .to_string();
        (code, kind, reply[split + 4..].to_vec())
    }

    fn rpc(&self, method: &str, params: &[u8]) -> Value {
        let mut body =
            format!(r#"{{"jsonrpc":"2.0","id":1,"method":"{method}","params":"#).into_bytes();
        body.extend_from_slice(params);
        body.push(b'}');
        let (code, _, reply) = self.send("POST", "/", "application/json", &body);
        assert_eq!(code, 200);
        let reply = zumen_json::parse(&reply).unwrap();
        reply
            .pointer("/result")
            .unwrap_or_else(|| panic!("{method} が誤りを返した: {reply:?}"))
            .clone()
    }
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// `%XX` にする。`/` と英数字などはそのまま
fn percent(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~/".contains(b) {
                char::from(*b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// `zumen serve --http` を、境界の 1 つの形として使う
struct Http {
    server: Rc<HttpServer>,
    session: Option<String>,
}

impl Http {
    fn close(&mut self) {
        if let Some(session) = self.session.take() {
            self.server
                .rpc("session_close", &params(&[("session", &session)]));
        }
    }
}

impl Drop for Http {
    fn drop(&mut self) {
        self.close();
    }
}

impl Engine for Http {
    fn session_new(&mut self, options: &[u8]) -> i32 {
        self.close();
        let result = self.server.rpc(
            "session_new",
            &params(&[("options_base64", &base64(options))]),
        );
        self.session = text(&result, "/session");
        code(&text(&result, "/status").unwrap())
    }

    fn push(&mut self, name: &[u8], data: &[u8]) -> i32 {
        let Some(session) = &self.session else {
            return Status::Invalid.code();
        };
        let path = format!("/sessions/{session}/inputs/{}", percent(name));
        let (_, _, reply) = self
            .server
            .send("POST", &path, "application/octet-stream", data);
        code(&text(&zumen_json::parse(&reply).unwrap(), "/status").unwrap())
    }

    fn call(&mut self, request: &[u8]) -> (i32, Option<Vec<u8>>) {
        let Some(session) = &self.session else {
            return (Status::Invalid.code(), None);
        };
        let result = self.server.rpc(
            "call",
            &params(&[("session", session), ("request_base64", &base64(request))]),
        );
        (
            code(&text(&result, "/status").unwrap()),
            text(&result, "/response").map(String::into_bytes),
        )
    }

    fn take(&mut self, blob: u32) -> (i32, Option<Vec<u8>>) {
        let Some(session) = &self.session else {
            return (Status::Invalid.code(), None);
        };
        let path = format!("/sessions/{session}/blobs/{blob}");
        let (code_, kind, reply) = self.server.send("GET", &path, "application/json", b"");
        if code_ == 200 && kind == "application/octet-stream" {
            return (Status::Ok.code(), Some(reply));
        }
        (
            code(&text(&zumen_json::parse(&reply).unwrap(), "/status").unwrap()),
            None,
        )
    }
}

/// 実行の形を `process` にしたセッション。実行の形を書かない設定に、`process` とこの CLI のパスを足す
#[derive(Default)]
struct Process {
    session: Option<zumen_abi1::Session>,
}

fn with_process(options: &[u8]) -> Vec<u8> {
    let object = if options.is_empty() {
        Some(zumen_json::parse(br#"{"v":1}"#).unwrap())
    } else {
        zumen_json::parse(options).ok()
    };
    let Some(Value::Object(object)) = object else {
        return options.to_vec();
    };
    if object.get("engine").is_some() {
        return options.to_vec();
    }
    let mut out = Writer::new();
    out.begin_object();
    for (key, value) in object.iter() {
        out.key(key);
        out.value(value);
    }
    out.key("engine");
    out.string("process");
    out.key("process_path");
    out.string(env!("CARGO_BIN_EXE_zumen"));
    out.end_object();
    out.into_bytes()
}

impl Engine for Process {
    fn session_new(&mut self, options: &[u8]) -> i32 {
        match zumen_abi1::Session::new(&with_process(options)) {
            Ok(session) => {
                self.session = Some(session);
                Status::Ok.code()
            }
            Err(status) => {
                self.session = None;
                status.code()
            }
        }
    }

    fn push(&mut self, name: &[u8], data: &[u8]) -> i32 {
        match &mut self.session {
            Some(session) => match session.push(name, data) {
                Ok(()) => Status::Ok.code(),
                Err(status) => status.code(),
            },
            None => Status::Invalid.code(),
        }
    }

    fn call(&mut self, request: &[u8]) -> (i32, Option<Vec<u8>>) {
        match &mut self.session {
            Some(session) => {
                let reply = session.call(request, &zumen_base::CancelToken::new());
                (reply.status.code(), Some(reply.response))
            }
            None => (Status::Invalid.code(), None),
        }
    }

    fn take(&mut self, blob: u32) -> (i32, Option<Vec<u8>>) {
        match self.session.as_mut().and_then(|s| s.take(blob)) {
            Some(data) => (Status::Ok.code(), Some(data)),
            None => (Status::Invalid.code(), None),
        }
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
fn through_the_process_engine() {
    let report = run(&cases().unwrap(), || {
        Box::new(Process::default()) as Box<dyn Engine>
    });
    check(&report, &read_hashes().unwrap());
}

#[test]
fn through_serve_http() {
    let server = Rc::new(HttpServer::spawn());
    let report = run(&cases().unwrap(), || {
        Box::new(Http {
            server: Rc::clone(&server),
            session: None,
        }) as Box<dyn Engine>
    });
    check(&report, &read_hashes().unwrap());
}

#[test]
fn http_refuses_browsers_and_other_hosts() {
    let server = HttpServer::spawn();
    let mut stream = TcpStream::connect(&server.address).unwrap();
    stream
        .write_all(b"POST / HTTP/1.1\r\nHost: example.com\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}")
        .unwrap();
    let mut reply = String::new();
    stream.read_to_string(&mut reply).unwrap();
    assert!(reply.starts_with("HTTP/1.1 403"), "{reply}");
    let (code, _, _) = server.send("POST", "/", "text/plain", b"{}");
    assert_eq!(code, 415);
    let (code, _, _) = server.send("GET", "/sessions/0/blobs/0", "application/json", b"");
    assert_eq!(code, 404);
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
