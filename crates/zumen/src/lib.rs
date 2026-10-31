//! zumen を Rust から使う入口（決定 0017・0203）。
//!
//! [`Session`] が境界の関数（決定 0012）に 1 つずつ当たる。要求と応答の型は [`messages`] にあり、
//! `schema/messages.json` から生成している。シーンの blob は [`scene::read_scene`] で読む。
//!
//! ```
//! use zumen::json::{FromJson, parse};
//! use zumen::messages::{CapabilitiesResult, Request};
//!
//! let mut session = zumen::Session::open().unwrap();
//! let request = Request::from_json(&parse(br#"{"v": 1, "op": "capabilities"}"#).unwrap()).unwrap();
//! let response = session.call(&request).unwrap();
//! let result: CapabilitiesResult = response.result_as().unwrap().unwrap();
//! assert!(!result.package.is_empty());
//! ```

use std::fmt;

use zumen_json::{FromJson, Invalid, ObjectReader, ToJson, Value, Writer};

pub mod scene;

pub use zumen_abi1::Status;
pub use zumen_abi1::messages;
pub use zumen_base::CancelToken;
/// JSON の値と、メッセージの型が実装する読み書きの trait
pub use zumen_json as json;

use messages::{Blob, Diagnostic, Request, SessionOptions};

/// 境界の呼び出しが成功しなかったこと
#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    /// 状態。[`Status::Ok`] にはならない
    pub status: Status,
    /// 応答の JSON。[`Status::Failed`] のときにあり、診断を持つ
    pub data: Option<Vec<u8>>,
}

impl Error {
    fn status(status: Status) -> Self {
        Self { status, data: None }
    }

    /// 応答を読む。応答がないか読めなければ `None`
    pub fn response(&self) -> Option<Response> {
        Response::parse(self.data.as_deref()?).ok()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "zumen の呼び出しが {} で終わった", self.status.name())
    }
}

impl std::error::Error for Error {}

/// 応答（`schema/messages.json` の `response`）。知らない欄は読み飛ばす
#[derive(Clone, Debug, PartialEq)]
pub struct Response {
    /// メッセージのスキーマの大版
    pub v: u64,
    /// op ごとの結果。失敗したときは持たない。型は [`Response::result_as`] で決める
    pub result: Option<Value>,
    /// 診断。入力の順
    pub diagnostics: Vec<Diagnostic>,
    /// バイナリの結果。[`Session::take`] で取り出す。次の呼び出しで捨てる
    pub blobs: Vec<Blob>,
}

impl Response {
    /// 応答の JSON を読む
    pub fn parse(data: &[u8]) -> Result<Self, Invalid> {
        let value = json::parse(data).map_err(|_| Invalid::new(json::Problem::Type))?;
        Self::from_json(&value)
    }

    /// 結果を op の結果の型（`messages::CapabilitiesResult` など）で読む。結果がなければ `None`
    pub fn result_as<T: FromJson>(&self) -> Option<Result<T, Invalid>> {
        self.result.as_ref().map(T::from_json)
    }
}

impl FromJson for Response {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let v = object.required("v", |value| {
            json::uint(value, 1, json::MAX_SAFE_INTEGER as u64)
        })?;
        let result = object.optional("result", |value| Ok(value.clone()))?;
        let diagnostics = object
            .optional("diagnostics", |value| {
                json::array(value, Diagnostic::from_json)
            })?
            .unwrap_or_default();
        let blobs = object
            .optional("blobs", |value| json::array(value, Blob::from_json))?
            .unwrap_or_default();
        Ok(Self {
            v,
            result,
            diagnostics,
            blobs,
        })
    }
}

/// 型を JSON のバイト列にする
fn to_bytes(value: &impl ToJson) -> Vec<u8> {
    let mut writer = Writer::new();
    value.to_json(&mut writer);
    writer.into_bytes()
}

/// コアのセッション。境界の関数に 1 つずつ当たる（決定 0012）。落とすと閉じる
#[derive(Debug)]
pub struct Session {
    inner: zumen_abi1::Session,
}

impl Session {
    /// 既定の設定でセッションを作る
    pub fn open() -> Result<Self, Error> {
        Self::open_bytes(b"")
    }

    /// 設定（`session_options`）からセッションを作る
    pub fn with_options(options: &SessionOptions) -> Result<Self, Error> {
        Self::open_bytes(&to_bytes(options))
    }

    /// 設定の JSON のバイト列からセッションを作る。空なら既定の設定にする
    pub fn open_bytes(options: &[u8]) -> Result<Self, Error> {
        zumen_abi1::Session::new(options)
            .map(|inner| Self { inner })
            .map_err(Error::status)
    }

    /// 入力を名前で足す。同じ名前があれば置き換える
    pub fn push(&mut self, name: &str, data: &[u8]) -> Result<(), Error> {
        self.push_bytes(name.as_bytes(), data)
    }

    /// 入力を、名前のバイト列で足す。名前が UTF-8 でなければ [`Status::Invalid`]
    pub fn push_bytes(&mut self, name: &[u8], data: &[u8]) -> Result<(), Error> {
        self.inner.push(name, data).map_err(Error::status)
    }

    /// 要求を渡し、応答を読む
    pub fn call(&mut self, request: &Request) -> Result<Response, Error> {
        self.call_with(request, &CancelToken::new())
    }

    /// 要求を渡し、応答を読む。`cancel` で、別のスレッドから止められる
    pub fn call_with(
        &mut self,
        request: &Request,
        cancel: &CancelToken,
    ) -> Result<Response, Error> {
        let data = self.call_bytes(&to_bytes(request), cancel)?;
        Response::parse(&data).map_err(|_| Error {
            status: Status::Panicked,
            data: Some(data),
        })
    }

    /// 要求の JSON のバイト列を渡し、応答の JSON のバイト列を返す
    pub fn call_bytes(&mut self, request: &[u8], cancel: &CancelToken) -> Result<Vec<u8>, Error> {
        let reply = self.inner.call(request, cancel);
        match reply.status {
            Status::Ok => Ok(reply.response),
            status => Err(Error {
                status,
                data: (!reply.response.is_empty()).then_some(reply.response),
            }),
        }
    }

    /// 直前の応答のバイナリの結果を取り出す。番号がなければ [`Status::Invalid`]
    pub fn take(&mut self, blob: u32) -> Result<Vec<u8>, Error> {
        self.inner
            .take(blob)
            .ok_or_else(|| Error::status(Status::Invalid))
    }
}
