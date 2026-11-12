//! `zumen mcp --http`: MCP の Streamable HTTP のサーバー（MCP 2026-07-28 と、初期化のある前のバージョン）。
//!
//! - `POST /` と `POST /mcp`（`application/json`）で JSON-RPC 2.0 の 1 つのメッセージを受ける
//! - 通知（`id` のないメッセージ）には `202 Accepted`（空の本文）を返し、`notifications/cancelled` は `requestId` で
//!   実行中の要求を止める
//! - `GET` と `DELETE` には `405 Method Not Allowed`（`Allow: POST`）を返す
//! - `2026-07-28` の要求では `MCP-Protocol-Version`・`Mcp-Method`・`Mcp-Name` を確かめ、合わなければ `400` と `-32020` を返す
//! - `Accept` に `text/event-stream` があり、要求に `progressToken` があれば、進み具合の通知と結果を SSE で流す

use std::collections::BTreeMap;
use std::io::Write as _;
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use zumen_base::CancelToken;
use zumen_json::Value;

use crate::host::{compact, field, str_field};
use crate::http::{Limits as HttpLimits, Reply, Request, listen_and_serve, loopback_host};
use crate::mcp::{Engine, HEADER_MISMATCH, Limits, VERSION_KEY, is_supported_version, unsupported};
use crate::rpc::{Message, Notify, Outcome, error_line, parse_message, result_line};

/// 既定の待ち受けのアドレス（`serve --http` の 24180 の隣）
pub const DEFAULT_LISTEN: &str = "127.0.0.1:24181";

struct State {
    loopback: bool,
    engine: Engine,
    in_flight: Mutex<BTreeMap<Vec<u8>, CancelToken>>,
}

impl State {
    fn new(root: Option<PathBuf>, limits: HttpLimits, loopback: bool) -> Self {
        let mcp_limits = Limits {
            sessions: limits.sessions,
            ..Limits::default()
        };
        Self {
            loopback,
            engine: Engine::new_with_options(
                root,
                mcp_limits,
                Some(limits.input_bytes),
                Some(limits.idle),
                true,
            ),
            in_flight: Mutex::new(BTreeMap::new()),
        }
    }

    fn cancel_request(&self, params: &Value) {
        let Some(req_id) = field(params, "requestId") else {
            return;
        };
        let key = compact(req_id);
        if let Some(token) = self
            .in_flight
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
        {
            token.cancel();
        }
    }
}

fn outcome_line(id: &Value, outcome: Outcome) -> Vec<u8> {
    match outcome {
        Outcome::Result(result) => result_line(id, &result),
        Outcome::Error(code, text, data) => error_line(id, code, &text, data.as_deref()),
    }
}

/// `MCP-Protocol-Version`・`Mcp-Method`・`Mcp-Name` を確かめる。誤りなら `400` の `Reply` を返す
fn validate_headers(request: &Request, message: &Message) -> Result<(), Reply> {
    let null = Value::Null;
    let id = message.id.as_ref().unwrap_or(&null);
    let mismatch = |detail: &str| Reply::json(400, error_line(id, HEADER_MISMATCH, detail, None));

    let header_ver = request.header("mcp-protocol-version");
    if let Some(ver) = header_ver
        && !is_supported_version(ver)
    {
        return Err(Reply::json(400, outcome_line(id, unsupported(ver))));
    }

    let meta_ver = field(&message.params, "_meta").and_then(|meta| str_field(meta, VERSION_KEY));
    if let Some(ver) = meta_ver
        && !is_supported_version(ver)
    {
        return Err(Reply::json(400, outcome_line(id, unsupported(ver))));
    }

    if let (Some(hv), Some(mv)) = (header_ver, meta_ver)
        && hv != mv
    {
        return Err(mismatch("Header mismatch: MCP-Protocol-Version"));
    }
    if meta_ver == Some("2026-07-28") && header_ver != Some("2026-07-28") {
        return Err(mismatch(
            "Header mismatch: MCP-Protocol-Version is required",
        ));
    }

    let modern = header_ver == Some("2026-07-28");
    let header_method = request.header("mcp-method");
    if modern {
        if header_method != Some(message.method.as_str()) {
            return Err(mismatch("Header mismatch: Mcp-Method"));
        }
    } else if let Some(hm) = header_method
        && hm != message.method
    {
        return Err(mismatch("Header mismatch: Mcp-Method"));
    }

    let (takes_name, expected_name) = match message.method.as_str() {
        "tools/call" | "prompts/get" => (true, str_field(&message.params, "name")),
        "resources/read" => (true, str_field(&message.params, "uri")),
        _ => (false, None),
    };
    let header_name = request.header("mcp-name");
    if modern {
        if takes_name {
            if let Some(expected) = expected_name
                && header_name != Some(expected)
            {
                return Err(mismatch("Header mismatch: Mcp-Name"));
            }
        } else if header_name.is_some() {
            return Err(mismatch("Header mismatch: unexpected Mcp-Name"));
        }
    } else if let Some(hn) = header_name {
        if !takes_name {
            return Err(mismatch("Header mismatch: unexpected Mcp-Name"));
        }
        if let Some(expected) = expected_name
            && hn != expected
        {
            return Err(mismatch("Header mismatch: Mcp-Name"));
        }
    }

    Ok(())
}

fn write_sse_event(stream: &mut TcpStream, data: &[u8]) {
    let _ = stream
        .write_all(b"data: ")
        .and_then(|()| stream.write_all(data))
        .and_then(|()| stream.write_all(b"\n\n"))
        .and_then(|()| stream.flush());
}

fn handle(state: &State, request: &Request, stream: &mut TcpStream) -> Option<Reply> {
    if state.loopback && !request.header("host").is_some_and(loopback_host) {
        return Some(Reply::problem(403, "Host はループバックの名前にする"));
    }
    if request.header("origin").is_some() {
        return Some(Reply::problem(403, "ブラウザーからの要求は受けない"));
    }
    if request.path != "/" && request.path != "/mcp" {
        return Some(Reply::problem(404, "ない"));
    }
    if request.method != "POST" {
        return Some(Reply::method_not_allowed("POST", "POST で JSON-RPC を送る"));
    }
    if request.content_type().as_deref() != Some("application/json") {
        return Some(Reply::problem(
            415,
            "Content-Type は application/json にする",
        ));
    }
    let message = match parse_message(&request.body) {
        Ok(message) => message,
        Err((id, code, text)) => return Some(Reply::json(400, error_line(&id, code, text, None))),
    };
    if let Err(reply) = validate_headers(request, &message) {
        return Some(reply);
    }
    if message.method == "notifications/cancelled" {
        state.cancel_request(&message.params);
        return Some(match &message.id {
            Some(id) => Reply::json(200, result_line(id, b"{}")),
            None => Reply::empty(202),
        });
    }
    let Some(id) = &message.id else {
        // notifications/initialized などの通知は 202 Accepted で受ける
        return Some(Reply::empty(202));
    };

    let key = compact(id);
    let cancel = CancelToken::new();
    state
        .in_flight
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(key.clone(), cancel.clone());

    let accepts_sse = request
        .header("accept")
        .is_some_and(|a| a.to_ascii_lowercase().contains("text/event-stream"));
    let has_progress = field(&message.params, "_meta")
        .and_then(|meta| field(meta, "progressToken"))
        .is_some_and(|token| matches!(token, Value::String(_) | Value::Number(_)));

    if accepts_sse
        && has_progress
        && let Ok(sse_stream) = stream.try_clone()
        && stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
            )
            .and_then(|()| stream.flush())
            .is_ok()
    {
        let writer = Arc::new(Mutex::new(sse_stream));
        let sink = Arc::clone(&writer);
        let notify: Notify = Arc::new(move |line: &[u8]| {
            let mut guard = sink.lock().unwrap_or_else(PoisonError::into_inner);
            write_sse_event(&mut guard, line);
        });
        let outcome = state
            .engine
            .dispatch(&message.method, &message.params, &cancel, &notify);
        state
            .in_flight
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&key);
        let final_line = outcome_line(id, outcome);
        let mut guard = writer.lock().unwrap_or_else(PoisonError::into_inner);
        write_sse_event(&mut guard, &final_line);
        return None;
    }

    let noop: Notify = Arc::new(|_: &[u8]| {});
    let outcome = state
        .engine
        .dispatch(&message.method, &message.params, &cancel, &noop);
    state
        .in_flight
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&key);
    Some(Reply::json(200, outcome_line(id, outcome)))
}

/// Streamable HTTP の待ち受けを始める。待ち受けたアドレスを標準出力の 1 行目に書き、止められるまで答える
pub fn serve(listen: &str, root: Option<PathBuf>, limits: HttpLimits) -> Result<(), String> {
    listen_and_serve(
        listen,
        limits,
        "zumen-mcp-http",
        move |loopback| State::new(root, limits, loopback),
        handle,
    )
}
