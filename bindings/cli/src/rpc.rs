//! 標準入出力の JSON-RPC 2.0（決定 0013）。1 行に 1 つのメッセージを読み書きする。
//!
//! 読む側のスレッドが行を読み、中断のメソッドだけはその場で、実行中の呼び出しの合図にする。
//! ほかのメッセージは、主のスレッドが届いた順に 1 つずつ処理する。

use std::io::{self, BufRead, Read, Write};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use zumen_base::CancelToken;
use zumen_json::{Value, Writer};

use crate::host::{compact, field, str_field};

/// JSON-RPC の誤りの番号（JSON-RPC 2.0 の 5.1 節）
pub const PARSE_ERROR: i64 = -32700;
/// 要求の形が違う
pub const INVALID_REQUEST: i64 = -32600;
/// メソッドがない
pub const METHOD_NOT_FOUND: i64 = -32601;
/// 引数が違う
pub const INVALID_PARAMS: i64 = -32602;

/// 1 行の上限（バイト）。超えた行は読み捨てて、誤りを返す
pub const MAX_LINE: usize = 1 << 30;

/// 処理の結果
pub enum Outcome {
    /// 結果の JSON
    Result(Vec<u8>),
    /// 誤り。番号・文・付け足しのデータ
    Error(i64, String, Option<Vec<u8>>),
}

/// メッセージを処理するもの
pub trait Handler {
    /// 実行中の呼び出しを止めるメソッドの名前。このメソッドは主のスレッドに渡さない
    const CANCEL: &'static str;
    /// メッセージを処理する
    fn handle(&mut self, method: &str, params: &Value, cancel: &CancelToken) -> Outcome;
}

type Output = Arc<Mutex<io::Stdout>>;

fn write_line(output: &Output, line: &[u8]) {
    let mut out = output.lock().unwrap_or_else(PoisonError::into_inner);
    // 書けないなら、読み手がいない。読む側が入力の終わりで止まる
    let _ = out
        .write_all(line)
        .and_then(|()| out.write_all(b"\n"))
        .and_then(|()| out.flush());
}

/// 結果の行を作る
pub fn result_line(id: &Value, result: &[u8]) -> Vec<u8> {
    let mut line = br#"{"jsonrpc":"2.0","id":"#.to_vec();
    line.extend_from_slice(&compact(id));
    line.extend_from_slice(br#","result":"#);
    line.extend_from_slice(result);
    line.push(b'}');
    line
}

/// 誤りの行を作る
pub fn error_line(id: &Value, code: i64, message: &str, data: Option<&[u8]>) -> Vec<u8> {
    let mut error = Writer::new();
    error.begin_object();
    error.key("code");
    error.int(code);
    error.key("message");
    error.string(message);
    error.end_object();
    let mut error = error.into_bytes();
    if let Some(data) = data {
        error.pop();
        error.extend_from_slice(br#","data":"#);
        error.extend_from_slice(data);
        error.push(b'}');
    }
    let mut line = br#"{"jsonrpc":"2.0","id":"#.to_vec();
    line.extend_from_slice(&compact(id));
    line.extend_from_slice(br#","error":"#);
    line.extend_from_slice(&error);
    line.push(b'}');
    line
}

enum Line {
    Text(Vec<u8>),
    TooLong,
}

fn read_line(input: &mut impl BufRead) -> io::Result<Option<Line>> {
    let mut buf = Vec::new();
    let n = input
        .by_ref()
        .take(MAX_LINE as u64 + 1)
        .read_until(b'\n', &mut buf)?;
    if n == 0 {
        return Ok(None);
    }
    if buf.len() > MAX_LINE {
        // 行の残りを読み捨てる
        let mut rest = Vec::new();
        while !buf.ends_with(b"\n") && !rest.ends_with(b"\n") {
            rest.clear();
            if input.by_ref().take(1 << 20).read_until(b'\n', &mut rest)? == 0 {
                break;
            }
        }
        return Ok(Some(Line::TooLong));
    }
    while buf.last().is_some_and(|b| *b == b'\n' || *b == b'\r') {
        buf.pop();
    }
    Ok(Some(Line::Text(buf)))
}

/// 読んだメッセージ。id がなければ通知
struct Message {
    id: Option<Value>,
    method: String,
    params: Value,
}

fn parse_message(line: &[u8]) -> Result<Message, (Value, i64, &'static str)> {
    let value = zumen_json::parse(line).map_err(|_| (Value::Null, PARSE_ERROR, "Parse error"))?;
    let id = field(&value, "id").cloned();
    let invalid = || {
        (
            id.clone().unwrap_or(Value::Null),
            INVALID_REQUEST,
            "Invalid Request",
        )
    };
    if str_field(&value, "jsonrpc") != Some("2.0") {
        return Err(invalid());
    }
    if !matches!(
        id,
        None | Some(Value::String(_) | Value::Number(_) | Value::Null)
    ) {
        return Err(invalid());
    }
    let method = str_field(&value, "method").ok_or_else(invalid)?.to_string();
    let params = field(&value, "params").cloned().unwrap_or(Value::Null);
    Ok(Message { id, method, params })
}

/// 標準入出力で、入力が終わるまで処理する
pub fn serve<T: Handler>(mut handler: T) {
    let output: Output = Arc::new(Mutex::new(io::stdout()));
    let current = Arc::new(Mutex::new(CancelToken::new()));
    let (sender, receiver) = mpsc::channel::<Message>();
    let reader_output = Arc::clone(&output);
    let reader_current = Arc::clone(&current);
    let cancel_method = T::CANCEL;
    thread::spawn(move || {
        let stdin = io::stdin();
        let mut input = stdin.lock();
        loop {
            let line = match read_line(&mut input) {
                Ok(Some(Line::Text(line))) => line,
                Ok(Some(Line::TooLong)) => {
                    write_line(
                        &reader_output,
                        &error_line(&Value::Null, INVALID_REQUEST, "Line too long", None),
                    );
                    continue;
                }
                Ok(None) | Err(_) => break,
            };
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            match parse_message(&line) {
                Ok(message) if message.method == cancel_method => {
                    reader_current
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .cancel();
                    if let Some(id) = &message.id {
                        write_line(&reader_output, &result_line(id, b"{}"));
                    }
                }
                Ok(message) => {
                    if sender.send(message).is_err() {
                        break;
                    }
                }
                Err((id, code, text)) => {
                    write_line(&reader_output, &error_line(&id, code, text, None));
                }
            }
        }
    });
    for message in receiver {
        let token = CancelToken::new();
        *current.lock().unwrap_or_else(PoisonError::into_inner) = token.clone();
        let outcome = handler.handle(&message.method, &message.params, &token);
        let Some(id) = &message.id else {
            continue;
        };
        let line = match outcome {
            Outcome::Result(result) => result_line(id, &result),
            Outcome::Error(code, text, data) => error_line(id, code, &text, data.as_deref()),
        };
        write_line(&output, &line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_results_and_errors() {
        let id = Value::String("a".into());
        assert_eq!(
            result_line(&id, b"{}"),
            br#"{"jsonrpc":"2.0","id":"a","result":{}}"#
        );
        assert_eq!(
            error_line(&Value::Null, -1, "x", Some(b"[1]")),
            br#"{"jsonrpc":"2.0","id":null,"error":{"code":-1,"message":"x","data":[1]}}"#
        );
    }

    #[test]
    fn reads_lines_and_rejects_bad_messages() {
        let mut input = io::Cursor::new(b"a\r\nb".to_vec());
        assert!(matches!(read_line(&mut input), Ok(Some(Line::Text(t))) if t == b"a"));
        assert!(matches!(read_line(&mut input), Ok(Some(Line::Text(t))) if t == b"b"));
        assert!(matches!(read_line(&mut input), Ok(None)));
        assert!(parse_message(b"{").is_err());
        assert!(parse_message(br#"{"jsonrpc":"1.0","method":"a"}"#).is_err());
        assert!(parse_message(br#"{"jsonrpc":"2.0","id":[1],"method":"a"}"#).is_err());
        let message = parse_message(br#"{"jsonrpc":"2.0","id":1,"method":"a"}"#).unwrap();
        assert_eq!(message.method, "a");
    }
}
