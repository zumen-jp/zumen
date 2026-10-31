//! `zumen serve --http`: 標準入出力と同じ JSON-RPC を、HTTP/1.1 で受ける（決定 0013・0200）。
//!
//! 標準ライブラリ（`std::net`）だけで書く。1 つのつながりで 1 つの要求を受け、答えたら閉じる。
//!
//! | 要求 | 中身 |
//! | --- | --- |
//! | `POST /`（`application/json`） | JSON-RPC 2.0 の 1 つのメッセージ。メソッドは `serve --stdio` と同じで、`session_new` 以外は `session` で指す |
//! | `POST /sessions/<id>/inputs/<名前>`（`application/octet-stream`） | 本文のバイト列を、そのまま入力に足す（`push`） |
//! | `GET /sessions/<id>/blobs/<番号>` | バイナリの結果を、そのまま本文で返す（`take`） |
//!
//! - 既定は 127.0.0.1 だけで待ち受ける。TLS と認証は持たない。ループバックで待ち受けるときは、`Host` が
//!   ループバックの名前の要求だけを受ける。ブラウザーが付ける `Origin` のある要求は受けない
//! - 送られたバイト列だけを扱い、サーバーのファイルは読まない（`push` の `path` は受けない）
//! - セッションは推測できない ID で指し、使われない時間が続いたら捨てる。数・入力の合計・本文の大きさ・
//!   同時のつながりに上限を置く

// HTTP のサーバーはコアではなくホストなので、使われない時間を時計で測る（決定 0013）
#![allow(clippy::disallowed_methods)]

use std::collections::BTreeMap;
use std::hash::{BuildHasher, RandomState};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError, TryLockError};
use std::thread;
use std::time::{Duration, Instant};

use zumen_abi1::Status;
use zumen_base::CancelToken;
use zumen_json::{Value, Writer};

use crate::host::{Answer, Host, field, str_field};
use crate::rpc::{
    INVALID_PARAMS, METHOD_NOT_FOUND, Outcome, error_line, parse_message, result_line,
};
use crate::serve::{Form, blob_param, bytes, call_result, status_only, take_result};

/// 既定の待ち受けのアドレス
pub const DEFAULT_LISTEN: &str = "127.0.0.1:24180";

/// 上限が足りないときの JSON-RPC の誤りの番号（実装が決める -32000 から -32099 の範囲）
const LIMIT_EXCEEDED: i64 = -32000;

/// 上限
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// セッションの数
    pub sessions: usize,
    /// 要求の本文のバイト数
    pub body_bytes: u64,
    /// すべてのセッションの入力の合計のバイト数
    pub input_bytes: u64,
    /// 使われないセッションを捨てるまでの時間
    pub idle: Duration,
    /// 同時のつながりの数
    pub connections: usize,
    /// 要求の行と見出しのバイト数
    pub head_bytes: u64,
    /// 要求を読み終えるまでの時間
    pub read_timeout: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            sessions: 16,
            body_bytes: 256 << 20,
            input_bytes: 1 << 30,
            idle: Duration::from_secs(600),
            connections: 32,
            head_bytes: 16 << 10,
            read_timeout: Duration::from_secs(60),
        }
    }
}

/// 1 つのセッション
struct Slot {
    host: Mutex<Host>,
    /// 動いている呼び出しの中断の合図
    running: Mutex<CancelToken>,
    last_used: Mutex<Instant>,
    /// 足した入力のバイト数
    input_bytes: AtomicU64,
}

impl Slot {
    fn touch(&self) {
        *self
            .last_used
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Instant::now();
    }
}

/// サーバーの状態。つながりのスレッドで共有する
struct State {
    limits: Limits,
    /// ループバックで待ち受けているか。そうなら `Host` を確かめる
    loopback: bool,
    sessions: Mutex<BTreeMap<String, Arc<Slot>>>,
    /// ID の種。OS の乱数で鍵を決めた SipHash に、数を通して ID を作る
    seeds: [RandomState; 2],
    next: AtomicU64,
    input_bytes: AtomicU64,
    connections: AtomicUsize,
}

impl State {
    fn new(limits: Limits, loopback: bool) -> Self {
        Self {
            limits,
            loopback,
            sessions: Mutex::new(BTreeMap::new()),
            seeds: [RandomState::new(), RandomState::new()],
            next: AtomicU64::new(0),
            input_bytes: AtomicU64::new(0),
            connections: AtomicUsize::new(0),
        }
    }

    fn new_id(&self) -> String {
        let n = self.next.fetch_add(1, Ordering::Relaxed);
        let [a, b] = &self.seeds;
        format!("{:016x}{:016x}", a.hash_one(n), b.hash_one(n))
    }

    fn sessions(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Arc<Slot>>> {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn slot(&self, id: &str) -> Option<Arc<Slot>> {
        self.sweep();
        let slot = self.sessions().get(id).cloned();
        if let Some(slot) = &slot {
            slot.touch();
        }
        slot
    }

    fn remove(&self, id: &str) -> bool {
        let slot = self.sessions().remove(id);
        if let Some(slot) = &slot {
            self.input_bytes
                .fetch_sub(slot.input_bytes.load(Ordering::Relaxed), Ordering::Relaxed);
        }
        slot.is_some()
    }

    /// 使われない時間が続いたセッションを捨てる。使っている最中のものは残す
    fn sweep(&self) {
        let now = Instant::now();
        let idle: Vec<String> = self
            .sessions()
            .iter()
            .filter(|(_, slot)| {
                let last = *slot
                    .last_used
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                now.duration_since(last) > self.limits.idle
                    && !matches!(slot.host.try_lock(), Err(TryLockError::WouldBlock))
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in idle {
            self.remove(&id);
        }
    }

    fn session_new(&self, options: &[u8]) -> Result<Vec<u8>, Outcome> {
        self.sweep();
        if self.sessions().len() >= self.limits.sessions {
            return Err(Outcome::Error(
                LIMIT_EXCEEDED,
                format!(
                    "セッションは {} までにする。使わないものを session_close で閉じる",
                    self.limits.sessions
                ),
                None,
            ));
        }
        let host = match Host::new(options) {
            Ok(host) => host,
            Err(status) => return Ok(status_only(status)),
        };
        let id = self.new_id();
        self.sessions().insert(
            id.clone(),
            Arc::new(Slot {
                host: Mutex::new(host),
                running: Mutex::new(CancelToken::new()),
                last_used: Mutex::new(Instant::now()),
                input_bytes: AtomicU64::new(0),
            }),
        );
        let mut out = Writer::new();
        out.begin_object();
        out.key("status");
        out.string(Status::Ok.name());
        out.key("session");
        out.string(&id);
        out.end_object();
        Ok(out.into_bytes())
    }

    /// 入力を足す。入力の合計の上限を超えるなら誤り
    fn push(&self, slot: &Slot, name: &[u8], data: &[u8]) -> Result<Status, String> {
        let size = data.len() as u64;
        let limit = self.limits.input_bytes;
        let before = self.input_bytes.fetch_add(size, Ordering::Relaxed);
        if before.saturating_add(size) > limit {
            self.input_bytes.fetch_sub(size, Ordering::Relaxed);
            return Err(format!(
                "入力の合計は {limit} バイトまでにする。使わないセッションを閉じる"
            ));
        }
        let status = match slot.host.try_lock() {
            Ok(mut host) => host.push(name, data),
            Err(TryLockError::WouldBlock) => Status::Busy,
            Err(TryLockError::Poisoned(_)) => Status::Panicked,
        };
        if status == Status::Ok {
            slot.input_bytes.fetch_add(size, Ordering::Relaxed);
        } else {
            self.input_bytes.fetch_sub(size, Ordering::Relaxed);
        }
        Ok(status)
    }

    fn take(slot: &Slot, blob: u32) -> Result<Option<Vec<u8>>, Status> {
        match slot.host.try_lock() {
            Ok(mut host) => Ok(host.take(blob)),
            Err(TryLockError::WouldBlock) => Err(Status::Busy),
            Err(TryLockError::Poisoned(_)) => Err(Status::Panicked),
        }
    }

    /// JSON-RPC のメソッドを処理する
    fn run(&self, method: &str, params: &Value) -> Result<Vec<u8>, Outcome> {
        let invalid = |problem: String| Outcome::Error(INVALID_PARAMS, problem, None);
        if method == "session_new" {
            let options = bytes(
                params,
                &[
                    ("options", Form::Json),
                    ("options_text", Form::Text),
                    ("options_base64", Form::Base64),
                ],
            )
            .map_err(invalid)?
            .unwrap_or_default();
            return self.session_new(&options);
        }
        let known = ["session_close", "push", "call", "take", "cancel"];
        if !known.contains(&method) {
            return Err(Outcome::Error(
                METHOD_NOT_FOUND,
                "Method not found".to_string(),
                None,
            ));
        }
        let id = str_field(params, "session")
            .ok_or_else(|| invalid("session がない。session_new で作る".to_string()))?;
        let Some(slot) = self.slot(id) else {
            return Ok(status_only(Status::Invalid));
        };
        match method {
            "session_close" => {
                self.remove(id);
                Ok(status_only(Status::Ok))
            }
            "cancel" => {
                slot.running
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .cancel();
                Ok(b"{}".to_vec())
            }
            "push" => {
                if field(params, "path").is_some() {
                    return Err(invalid(
                        "HTTP では、サーバーのファイルを読まない。text か base64 で送る"
                            .to_string(),
                    ));
                }
                let name = bytes(
                    params,
                    &[("name", Form::Text), ("name_base64", Form::Base64)],
                )
                .map_err(invalid)?
                .ok_or_else(|| invalid("name か name_base64 がない".to_string()))?;
                let data = bytes(params, &[("text", Form::Text), ("base64", Form::Base64)])
                    .map_err(invalid)?
                    .unwrap_or_default();
                let status = self
                    .push(&slot, &name, &data)
                    .map_err(|problem| Outcome::Error(LIMIT_EXCEEDED, problem, None))?;
                Ok(status_only(status))
            }
            "call" => {
                let request = bytes(
                    params,
                    &[
                        ("request", Form::Json),
                        ("request_text", Form::Text),
                        ("request_base64", Form::Base64),
                    ],
                )
                .map_err(invalid)?
                .ok_or_else(|| invalid("request がない".to_string()))?;
                let answer = match slot.host.try_lock() {
                    Ok(mut host) => {
                        let token = CancelToken::new();
                        *slot.running.lock().unwrap_or_else(PoisonError::into_inner) =
                            token.clone();
                        let answer = host.call(&request, &token);
                        slot.touch();
                        answer
                    }
                    Err(TryLockError::WouldBlock) => Answer {
                        status: Status::Busy,
                        response: None,
                    },
                    Err(TryLockError::Poisoned(_)) => Answer {
                        status: Status::Panicked,
                        response: None,
                    },
                };
                Ok(call_result(&answer))
            }
            _ => {
                let blob = blob_param(params).map_err(invalid)?;
                match Self::take(&slot, blob) {
                    Ok(data) => Ok(take_result(data.as_deref())),
                    Err(status) => Ok(status_only(status)),
                }
            }
        }
    }
}

/// 読んだ要求
struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// `Content-Type` の、引数を除いた種類
    fn content_type(&self) -> Option<String> {
        self.header("content-type").map(|value| {
            value
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
        })
    }
}

/// 返す答え
struct Reply {
    code: u16,
    content_type: &'static str,
    body: Vec<u8>,
}

impl Reply {
    fn json(code: u16, body: Vec<u8>) -> Self {
        Self {
            code,
            content_type: "application/json",
            body,
        }
    }

    fn problem(code: u16, text: &str) -> Self {
        let mut out = Writer::new();
        out.begin_object();
        out.key("error");
        out.string(text);
        out.end_object();
        Self::json(code, out.into_bytes())
    }
}

fn reason(code: u16) -> &'static str {
    match code {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        411 => "Length Required",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        431 => "Request Header Fields Too Large",
        501 => "Not Implemented",
        503 => "Service Unavailable",
        _ => "Error",
    }
}

fn write_reply(stream: &mut TcpStream, reply: &Reply) -> io::Result<()> {
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        reply.code,
        reason(reply.code),
        reply.content_type,
        reply.body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(&reply.body)?;
    stream.flush()
}

/// 要求の行と見出しと本文を読む。読めなければ、返す答え
fn read_request(
    stream: &mut TcpStream,
    reader: &mut BufReader<TcpStream>,
    limits: &Limits,
) -> Result<Request, Reply> {
    let broken = || Reply::problem(400, "HTTP/1.1 の要求として読めない");
    let mut head = reader.by_ref().take(limits.head_bytes);
    let mut lines = Vec::new();
    loop {
        let mut line = Vec::new();
        match head.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => {
                return Err(if head.limit() == 0 {
                    Reply::problem(431, "要求の行と見出しが長すぎる")
                } else {
                    broken()
                });
            }
            Ok(_) => {}
        }
        if !line.ends_with(b"\n") {
            return Err(Reply::problem(431, "要求の行と見出しが長すぎる"));
        }
        while line.last().is_some_and(|b| *b == b'\n' || *b == b'\r') {
            line.pop();
        }
        if line.is_empty() {
            if lines.is_empty() {
                continue;
            }
            break;
        }
        lines.push(String::from_utf8(line).map_err(|_| broken())?);
    }
    let (first, rest) = lines.split_first().ok_or_else(broken)?;
    let mut parts = first.split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(broken());
    };
    if !version.starts_with("HTTP/1.") {
        return Err(Reply::problem(501, "HTTP/1.1 だけを受ける"));
    }
    let mut headers = Vec::new();
    for line in rest {
        let (name, value) = line.split_once(':').ok_or_else(broken)?;
        headers.push((name.trim().to_string(), value.trim().to_string()));
    }
    let mut request = Request {
        method: method.to_string(),
        path: target.split('?').next().unwrap_or_default().to_string(),
        headers,
        body: Vec::new(),
    };
    if request.header("transfer-encoding").is_some() {
        return Err(Reply::problem(
            501,
            "Transfer-Encoding は受けない。Content-Length を書く",
        ));
    }
    let length = match request.header("content-length") {
        Some(value) => value.parse::<u64>().map_err(|_| broken())?,
        None if request.method == "POST" => {
            return Err(Reply::problem(411, "Content-Length を書く"));
        }
        None => 0,
    };
    if length > limits.body_bytes {
        return Err(Reply::problem(
            413,
            &format!("本文は {} バイトまでにする", limits.body_bytes),
        ));
    }
    if request
        .header("expect")
        .is_some_and(|value| value.eq_ignore_ascii_case("100-continue"))
    {
        stream
            .write_all(b"HTTP/1.1 100 Continue\r\n\r\n")
            .map_err(|_| broken())?;
    }
    let mut body = Vec::new();
    reader
        .by_ref()
        .take(length)
        .read_to_end(&mut body)
        .map_err(|_| broken())?;
    if body.len() as u64 != length {
        return Err(broken());
    }
    request.body = body;
    Ok(request)
}

/// `Host` がループバックの名前か
fn loopback_host(host: &str) -> bool {
    let name = if let Some(rest) = host.strip_prefix('[') {
        rest.split(']').next().unwrap_or_default()
    } else {
        host.rsplit_once(':').map_or(host, |(name, _)| name)
    };
    matches!(
        name.to_ascii_lowercase().as_str(),
        "localhost" | "127.0.0.1" | "::1"
    )
}

/// `%XX` を戻す。壊れていれば `None`
fn percent_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len());
    let mut bytes = text.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let hex = [bytes.next()?, bytes.next()?];
            let hex = std::str::from_utf8(&hex).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
        } else {
            out.push(b);
        }
    }
    Some(out)
}

fn rpc(state: &State, request: &Request) -> Reply {
    if request.content_type().as_deref() != Some("application/json") {
        return Reply::problem(415, "Content-Type は application/json にする");
    }
    let message = match parse_message(&request.body) {
        Ok(message) => message,
        Err((id, code, text)) => return Reply::json(200, error_line(&id, code, text, None)),
    };
    let outcome = match state.run(&message.method, &message.params) {
        Ok(result) => Outcome::Result(result),
        Err(outcome) => outcome,
    };
    let Some(id) = &message.id else {
        return Reply {
            code: 204,
            content_type: "application/json",
            body: Vec::new(),
        };
    };
    Reply::json(
        200,
        match outcome {
            Outcome::Result(result) => result_line(id, &result),
            Outcome::Error(code, text, data) => error_line(id, code, &text, data.as_deref()),
        },
    )
}

fn route(state: &State, request: &Request) -> Reply {
    if state.loopback && !request.header("host").is_some_and(loopback_host) {
        return Reply::problem(403, "Host はループバックの名前にする");
    }
    if request.header("origin").is_some() {
        return Reply::problem(403, "ブラウザーからの要求は受けない");
    }
    if request.path == "/" {
        if request.method != "POST" {
            return Reply::problem(405, "POST で JSON-RPC を送る");
        }
        return rpc(state, request);
    }
    let Some(rest) = request.path.strip_prefix("/sessions/") else {
        return Reply::problem(404, "ない");
    };
    let Some((id, rest)) = rest.split_once('/') else {
        return Reply::problem(404, "ない");
    };
    let Some(slot) = state.slot(id) else {
        return Reply::json(404, status_only(Status::Invalid));
    };
    if let Some(name) = rest.strip_prefix("inputs/") {
        if request.method != "POST" {
            return Reply::problem(405, "POST で本文を送る");
        }
        if request.content_type().as_deref() != Some("application/octet-stream") {
            return Reply::problem(415, "Content-Type は application/octet-stream にする");
        }
        let Some(name) = percent_decode(name) else {
            return Reply::problem(400, "入力の名前が読めない");
        };
        return match state.push(&slot, &name, &request.body) {
            Ok(status) => Reply::json(200, status_only(status)),
            Err(problem) => Reply::problem(413, &problem),
        };
    }
    if let Some(blob) = rest.strip_prefix("blobs/") {
        if request.method != "GET" {
            return Reply::problem(405, "GET で取り出す");
        }
        let Ok(blob) = blob.parse::<u32>() else {
            return Reply::problem(404, "blob の番号が読めない");
        };
        return match State::take(&slot, blob) {
            Ok(Some(data)) => Reply {
                code: 200,
                content_type: "application/octet-stream",
                body: data,
            },
            Ok(None) => Reply::json(404, status_only(Status::Invalid)),
            Err(status) => Reply::json(200, status_only(status)),
        };
    }
    Reply::problem(404, "ない")
}

fn connection(state: &State, mut stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(state.limits.read_timeout));
    let reply = match stream.try_clone() {
        Ok(read) => {
            let mut reader = BufReader::new(read);
            match read_request(&mut stream, &mut reader, &state.limits) {
                Ok(request) => route(state, &request),
                Err(reply) => reply,
            }
        }
        Err(_) => return,
    };
    let _ = write_reply(&mut stream, &reply);
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

/// 待ち受けを始める。待ち受けたアドレスを標準出力の 1 行目に書き、止められるまで答える
pub fn serve(listen: &str, limits: Limits) -> Result<(), String> {
    let listener =
        TcpListener::bind(listen).map_err(|e| format!("{listen} で待ち受けられない: {e}"))?;
    let address: SocketAddr = listener
        .local_addr()
        .map_err(|e| format!("待ち受けたアドレスが分からない: {e}"))?;
    let loopback = address.ip().is_loopback();
    {
        let mut out = io::stdout().lock();
        writeln!(out, "http://{address}/")
            .and_then(|()| out.flush())
            .map_err(|e| format!("標準出力に書けない: {e}"))?;
    }
    if !loopback {
        let _ = writeln!(
            io::stderr(),
            "{address} で待ち受ける。TLS と認証を持たないので、認証のある前段の後ろに置く"
        );
    }
    let state = Arc::new(State::new(limits, loopback));
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        if state.connections.fetch_add(1, Ordering::AcqRel) >= state.limits.connections {
            state.connections.fetch_sub(1, Ordering::AcqRel);
            let _ = write_reply(
                &mut stream,
                &Reply::problem(503, "同時のつながりが多すぎる"),
            );
            continue;
        }
        let shared = Arc::clone(&state);
        let spawned = thread::Builder::new()
            .name("zumen-http".to_string())
            .spawn(move || {
                connection(&shared, stream);
                shared.connections.fetch_sub(1, Ordering::AcqRel);
            });
        if spawned.is_err() {
            // スレッドを作れなければ、つながりは閉じる（閉じたものは数えない）
            state.connections.fetch_sub(1, Ordering::AcqRel);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(state: &State, method: &str, params: &str) -> String {
        let params = zumen_json::parse(params.as_bytes()).unwrap();
        match state.run(method, &params) {
            Ok(result) => String::from_utf8(result).unwrap(),
            Err(Outcome::Error(code, text, _)) => format!("error {code} {text}"),
            Err(Outcome::Result(_)) => panic!("誤りでない結果"),
        }
    }

    fn session(reply: &str) -> String {
        let reply = zumen_json::parse(reply.as_bytes()).unwrap();
        str_field(&reply, "session").unwrap().to_string()
    }

    #[test]
    fn makes_unguessable_ids_and_keeps_limits() {
        let limits = Limits {
            sessions: 2,
            input_bytes: 4,
            ..Limits::default()
        };
        let state = State::new(limits, true);
        let a = session(&run(&state, "session_new", "{}"));
        let b = session(&run(&state, "session_new", "{}"));
        assert_eq!(a.len(), 32);
        assert_ne!(a, b);
        assert!(run(&state, "session_new", "{}").starts_with("error -32000"));
        let push = format!(r#"{{"session":"{a}","name":"a.dxf","text":"0123"}}"#);
        assert_eq!(run(&state, "push", &push), r#"{"status":"ok"}"#);
        assert!(run(&state, "push", &push).starts_with("error -32000"));
        let path = format!(r#"{{"session":"{a}","name":"a","path":"/etc/hosts"}}"#);
        assert!(run(&state, "push", &path).starts_with("error -32602"));
        let close = format!(r#"{{"session":"{a}"}}"#);
        assert_eq!(run(&state, "session_close", &close), r#"{"status":"ok"}"#);
        assert_eq!(state.input_bytes.load(Ordering::Relaxed), 0);
        assert_eq!(
            run(
                &state,
                "call",
                &format!(r#"{{"session":"{a}","request_text":"{{}}"}}"#)
            ),
            r#"{"status":"invalid"}"#
        );
        assert!(run(&state, "nothing", "{}").starts_with("error -32601"));
    }

    #[test]
    fn drops_idle_sessions() {
        let limits = Limits {
            idle: Duration::ZERO,
            ..Limits::default()
        };
        let state = State::new(limits, true);
        let a = session(&run(&state, "session_new", "{}"));
        thread::sleep(Duration::from_millis(2));
        assert!(state.slot(&a).is_none());
    }

    #[test]
    fn checks_hosts_and_names() {
        for host in ["localhost:24180", "127.0.0.1", "[::1]:80", "LOCALHOST"] {
            assert!(loopback_host(host), "{host}");
        }
        for host in ["example.com", "127.0.0.1.example.com:80", "[::2]"] {
            assert!(!loopback_host(host), "{host}");
        }
        assert_eq!(percent_decode("a%2Fb.dxf").unwrap(), b"a/b.dxf");
        assert!(percent_decode("a%2").is_none());
    }
}
