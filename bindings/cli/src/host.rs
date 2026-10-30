//! コアのセッションを持つホストの部品。CLI・標準入出力の JSON-RPC・MCP で共有する（決定 0013・0197）。

use std::panic::{AssertUnwindSafe, catch_unwind};

use zumen_abi1::{Session, Status};
use zumen_base::CancelToken;
use zumen_json::{Value, Writer};

/// 1 つのセッション。コアがパニックしたら、ネイティブの境界と同じく閉じるしかない（`zm_call` と同じ）
#[derive(Debug)]
pub struct Host {
    session: Option<Session>,
}

/// 呼び出しの答え
#[derive(Debug)]
pub struct Answer {
    /// 状態
    pub status: Status,
    /// 応答の JSON。invalid と busy のときはない
    pub response: Option<Vec<u8>>,
}

impl Host {
    /// 設定からセッションを作る
    pub fn new(options: &[u8]) -> Result<Self, Status> {
        Session::new(options).map(|session| Self {
            session: Some(session),
        })
    }

    /// 入力を足す
    pub fn push(&mut self, name: &[u8], data: &[u8]) -> Status {
        match &mut self.session {
            Some(session) => match session.push(name, data) {
                Ok(()) => Status::Ok,
                Err(status) => status,
            },
            None => Status::Invalid,
        }
    }

    /// 要求を渡す
    pub fn call(&mut self, request: &[u8], cancel: &CancelToken) -> Answer {
        let Some(session) = &mut self.session else {
            return Answer {
                status: Status::Invalid,
                response: None,
            };
        };
        match catch_unwind(AssertUnwindSafe(|| session.call(request, cancel))) {
            Ok(reply) => Answer {
                status: reply.status,
                response: Some(reply.response),
            },
            Err(_) => {
                self.session = None;
                Answer {
                    status: Status::Panicked,
                    response: Some(Session::panic_response()),
                }
            }
        }
    }

    /// バイナリの結果を取り出す
    pub fn take(&mut self, blob: u32) -> Option<Vec<u8>> {
        self.session.as_mut().and_then(|session| session.take(blob))
    }
}

/// 値を、空白を入れない JSON にする
pub fn compact(value: &Value) -> Vec<u8> {
    let mut out = Writer::new();
    out.value(value);
    out.into_bytes()
}

/// オブジェクトの欄の文字列
pub fn str_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    match value {
        Value::Object(object) => match object.get(key) {
            Some(Value::String(text)) => Some(text),
            _ => None,
        },
        _ => None,
    }
}

/// オブジェクトの欄
pub fn field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(object) => object.get(key),
        _ => None,
    }
}

/// 応答の blobs の番号と種類
pub fn blobs(response: &[u8]) -> Vec<(u32, String)> {
    let Ok(response) = zumen_json::parse(response) else {
        return Vec::new();
    };
    let Some(Value::Array(blobs)) = response.pointer("/blobs") else {
        return Vec::new();
    };
    blobs
        .iter()
        .filter_map(|blob| {
            let id = match field(blob, "id") {
                Some(Value::Number(n)) => n.to_safe_integer().and_then(|n| u32::try_from(n).ok()),
                _ => None,
            }?;
            Some((id, str_field(blob, "kind").unwrap_or("blob").to_string()))
        })
        .collect()
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// base64（RFC 4648 の 4 節、`=` で埋める）で書く
pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let (chunks, rest) = bytes.as_chunks::<3>();
    let mut put = |index: u32| {
        if let Some(&c) = ALPHABET.get((index & 63) as usize) {
            out.push(char::from(c));
        }
    };
    for [a, b, c] in chunks {
        let n = u32::from(*a) << 16 | u32::from(*b) << 8 | u32::from(*c);
        put(n >> 18);
        put(n >> 12);
        put(n >> 6);
        put(n);
    }
    let tail = match rest {
        [a] => Some((u32::from(*a) << 16, 2)),
        [a, b] => Some((u32::from(*a) << 16 | u32::from(*b) << 8, 3)),
        _ => None,
    };
    if let Some((n, digits)) = tail {
        for shift in [18, 12, 6].into_iter().take(digits) {
            put(n >> shift);
        }
        for _ in digits..4 {
            out.push('=');
        }
    }
    out
}

/// base64（RFC 4648 の 4 節）を読む。`=` の埋めは省いてもよい。ほかの文字があれば読まない
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let text = text.trim_end_matches('=');
    let mut out = Vec::with_capacity(text.len() / 4 * 3 + 2);
    let mut acc = 0u32;
    let mut bits = 0;
    for c in text.bytes() {
        let digit = ALPHABET.iter().position(|&a| a == c)?;
        acc = acc << 6 | digit as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    // 残りのビットが 6 以上なら、桁の数が合わない
    (bits < 6).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips() {
        for (bytes, text) in [
            (&b""[..], ""),
            (b"f", "Zg=="),
            (b"fo", "Zm8="),
            (b"foo", "Zm9v"),
            (b"foob", "Zm9vYg=="),
            (b"fooba", "Zm9vYmE="),
            (b"foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64_encode(bytes), text);
            assert_eq!(base64_decode(text).unwrap(), bytes);
        }
        assert_eq!(base64_decode("Zg").unwrap(), b"f");
        assert!(base64_decode("Z").is_none());
        assert!(base64_decode("Zm9v!").is_none());
    }

    #[test]
    fn closes_after_a_panic_like_the_native_boundary() {
        let mut host = Host::new(b"").unwrap();
        let answer = host.call(br#"{"v":1,"op":"capabilities"}"#, &CancelToken::new());
        assert_eq!(answer.status, Status::Ok);
        assert_eq!(host.push(b"a", b""), Status::Ok);
        host.session = None;
        assert_eq!(host.push(b"a", b""), Status::Invalid);
    }
}
