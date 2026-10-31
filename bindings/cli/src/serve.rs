//! `zumen serve --stdio`: 境界の関数を、標準入出力の JSON-RPC のメソッドで出す（決定 0013・0197）。
//!
//! メソッドは境界の関数に 1 対 1 で当たる。セッションは 1 つで、既定の設定で作ってから始める。
//!
//! | メソッド | 関数 | 引数 | 結果 |
//! | --- | --- | --- | --- |
//! | `session_new` | `zm_session_new` | `options`（JSON の値）・`options_text`・`options_base64` のどれか。なければ既定 | `status` |
//! | `push` | `zm_push` | 名前は `name`・`name_base64`、中身は `text`・`base64`・`path` のどれか | `status` |
//! | `call` | `zm_call` | `request`（JSON の値）・`request_text`・`request_base64` のどれか | `status` と、応答の JSON を文字列のまま入れた `response` |
//! | `take` | `zm_take` | `blob`（番号） | `status` と、中身の `base64` |
//! | `cancel` | `zm_cancel` | なし。実行中の `call` を止める | `{}` |

use std::fs;

use zumen_abi1::Status;
use zumen_base::CancelToken;
use zumen_json::{Value, Writer};

use crate::host::{Answer, Host, base64_decode, base64_encode, compact, field};
use crate::rpc::{Handler, INVALID_PARAMS, METHOD_NOT_FOUND, Notify, Outcome};

/// バイト列の引数の書き方
#[derive(Clone, Copy)]
pub enum Form {
    /// JSON の値を、空白を入れずに書いたもの
    Json,
    /// UTF-8 の文字列
    Text,
    /// base64 の文字列
    Base64,
    /// ファイルの道筋
    Path,
}

/// 書き方の違う欄のうち、あるものからバイト列を作る。2 つ以上あれば誤り
pub fn bytes(params: &Value, keys: &[(&str, Form)]) -> Result<Option<Vec<u8>>, String> {
    let present: Vec<(&str, Form, &Value)> = keys
        .iter()
        .filter_map(|(key, form)| field(params, key).map(|value| (*key, *form, value)))
        .collect();
    let [(key, form, value)] = present.as_slice() else {
        if present.is_empty() {
            return Ok(None);
        }
        let names: Vec<&str> = keys.iter().map(|(key, _)| *key).collect();
        return Err(format!("{} は、どれか 1 つにする", names.join("・")));
    };
    let text = match value {
        Value::String(text) => Some(text.as_str()),
        _ => None,
    };
    match (form, text) {
        (Form::Json, _) => Ok(Some(compact(value))),
        (Form::Text, Some(text)) => Ok(Some(text.as_bytes().to_vec())),
        (Form::Base64, Some(text)) => base64_decode(text)
            .map(Some)
            .ok_or(format!("{key} が base64 でない")),
        (Form::Path, Some(path)) => fs::read(path)
            .map(Some)
            .map_err(|e| format!("{path} を読めない: {e}")),
        _ => Err(format!("{key} は文字列にする")),
    }
}

pub fn status_only(status: Status) -> Vec<u8> {
    let mut out = Writer::new();
    out.begin_object();
    out.key("status");
    out.string(status.name());
    out.end_object();
    out.into_bytes()
}

/// `call` の結果。応答の JSON は、書き換えないよう文字列のまま入れる
pub fn call_result(answer: &Answer) -> Vec<u8> {
    let mut out = Writer::new();
    out.begin_object();
    out.key("status");
    out.string(answer.status.name());
    if let Some(response) = &answer.response {
        match std::str::from_utf8(response) {
            Ok(text) => {
                out.key("response");
                out.string(text);
            }
            Err(_) => {
                out.key("response_base64");
                out.string(&base64_encode(response));
            }
        }
    }
    out.end_object();
    out.into_bytes()
}

/// `take` の引数の blob の番号
pub fn blob_param(params: &Value) -> Result<u32, String> {
    match field(params, "blob") {
        Some(Value::Number(n)) => n.to_safe_integer().and_then(|n| u32::try_from(n).ok()),
        _ => None,
    }
    .ok_or_else(|| "blob は 0 以上の整数にする".to_string())
}

/// `take` の結果
pub fn take_result(data: Option<&[u8]>) -> Vec<u8> {
    let mut out = Writer::new();
    out.begin_object();
    out.key("status");
    match data {
        Some(data) => {
            out.string(Status::Ok.name());
            out.key("base64");
            out.string(&base64_encode(data));
        }
        None => out.string(Status::Invalid.name()),
    }
    out.end_object();
    out.into_bytes()
}

/// 標準入出力の JSON-RPC の処理
#[derive(Debug)]
pub struct Serve {
    host: Option<Host>,
}

impl Serve {
    /// 既定の設定のセッションで始める
    pub fn new() -> Self {
        Self {
            host: Host::new(b"").ok(),
        }
    }

    fn run(
        &mut self,
        method: &str,
        params: &Value,
        cancel: &CancelToken,
    ) -> Result<Vec<u8>, Outcome> {
        let invalid = |problem: String| Outcome::Error(INVALID_PARAMS, problem, None);
        match method {
            "session_new" => {
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
                let (host, status) = match Host::new(&options) {
                    Ok(host) => (Some(host), Status::Ok),
                    Err(status) => (None, status),
                };
                self.host = host;
                Ok(status_only(status))
            }
            "push" => {
                let name = bytes(
                    params,
                    &[("name", Form::Text), ("name_base64", Form::Base64)],
                )
                .map_err(invalid)?
                .ok_or_else(|| invalid("name か name_base64 がない".to_string()))?;
                let data = bytes(
                    params,
                    &[
                        ("text", Form::Text),
                        ("base64", Form::Base64),
                        ("path", Form::Path),
                    ],
                )
                .map_err(invalid)?
                .unwrap_or_default();
                let status = match &mut self.host {
                    Some(host) => host.push(&name, &data),
                    None => Status::Invalid,
                };
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
                let answer = match &mut self.host {
                    Some(host) => host.call(&request, cancel),
                    None => Answer {
                        status: Status::Invalid,
                        response: None,
                    },
                };
                Ok(call_result(&answer))
            }
            "take" => {
                let blob = blob_param(params).map_err(invalid)?;
                let data = self.host.as_mut().and_then(|host| host.take(blob));
                Ok(take_result(data.as_deref()))
            }
            _ => Err(Outcome::Error(
                METHOD_NOT_FOUND,
                "Method not found".to_string(),
                None,
            )),
        }
    }
}

impl Handler for Serve {
    const CANCEL: &'static str = "cancel";

    fn handle(
        &mut self,
        method: &str,
        params: &Value,
        cancel: &CancelToken,
        _notify: &Notify,
    ) -> Outcome {
        match self.run(method, params, cancel) {
            Ok(result) => Outcome::Result(result),
            Err(outcome) => outcome,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handle(serve: &mut Serve, method: &str, params: &str) -> String {
        let params = zumen_json::parse(params.as_bytes()).unwrap();
        let notify: Notify = std::sync::Arc::new(|_: &[u8]| {});
        match serve.handle(method, &params, &CancelToken::new(), &notify) {
            Outcome::Result(result) => String::from_utf8(result).unwrap(),
            Outcome::Error(code, text, _) => format!("error {code} {text}"),
        }
    }

    #[test]
    fn answers_like_the_boundary_functions() {
        let mut serve = Serve::new();
        let reply = handle(
            &mut serve,
            "call",
            r#"{"request":{"v":1,"op":"capabilities"}}"#,
        );
        assert!(
            reply.starts_with(r#"{"status":"ok","response":"{\"v\":1"#),
            "{reply}"
        );
        assert_eq!(
            handle(&mut serve, "push", r#"{"name":"a.dxf","base64":"MA=="}"#),
            r#"{"status":"ok"}"#
        );
        assert_eq!(
            handle(&mut serve, "push", r#"{"name":"../a","text":""}"#),
            r#"{"status":"invalid"}"#
        );
        assert_eq!(
            handle(&mut serve, "take", r#"{"blob":0}"#),
            r#"{"status":"invalid"}"#
        );
        assert_eq!(
            handle(&mut serve, "session_new", r#"{"options_text":"{"}"#),
            r#"{"status":"invalid"}"#
        );
        assert_eq!(
            handle(&mut serve, "call", r#"{"request_text":"{}"}"#),
            r#"{"status":"invalid"}"#
        );
        assert!(
            handle(&mut serve, "call", r#"{"request":1,"request_text":"1"}"#)
                .starts_with("error -32602")
        );
        assert!(handle(&mut serve, "nothing", "{}").starts_with("error -32601"));
    }
}
