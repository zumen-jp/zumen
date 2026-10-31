//! 言語に依存しない適合試験（決定 0012）。
//!
//! 試験の組（`cases/*.json`）を、境界のどの形・どの言語の結合でも同じように流し、状態と、応答の値
//! （JSON Pointer で指す）を確かめる。あわせて、応答とバイナリの結果のハッシュ（SHA-256）の一覧を作る。
//! 一覧は `hashes.txt` と一致しなければならない（決定 0007）。
//!
//! この crate は、Rust の 3 つの形を受け持つ。コアを直接呼ぶ [`Direct`]、C ABI の関数を呼ぶ [`CAbi`]、
//! 利用者向けの `zumen` の crate を通す [`Wrapper`]。
//! wasm のモジュールは `wasm.mjs` が流す。試験の組の書き方は `README.md` にある。

use std::fs;
use std::path::{Path, PathBuf};
use std::ptr;

use zumen_abi1::{Session, Status, ffi};
use zumen_base::CancelToken;
use zumen_json::{Value, Writer};

mod sha256;
mod wrapper;

pub use sha256::hex as sha256_hex;
pub use wrapper::Wrapper;

/// ハッシュの一覧の先頭に書く説明
const HASHES_HEADER: &str = "\
# 適合試験の応答とバイナリの結果のハッシュ（SHA-256）。生成したファイル。手で直さない。
# `cargo run -p zumen-conformance -- --write` で作り直す。どの形・どの言語の結合でも、この一覧と一致する（決定 0007・0012）。
# 行の形: <試験の組> <手順の番号> <response か blob> <ハッシュ>
";

/// 試験の組とハッシュの一覧の置き場所
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// 境界の 1 つの形。状態は C の値（`ZM_OK` など）で返す
pub trait Engine {
    /// セッションを作る。前のセッションは閉じる
    fn session_new(&mut self, options: &[u8]) -> i32;
    /// 入力を足す
    fn push(&mut self, name: &[u8], data: &[u8]) -> i32;
    /// 要求を渡し、応答があれば一緒に返す
    fn call(&mut self, request: &[u8]) -> (i32, Option<Vec<u8>>);
    /// バイナリの結果を取り出す
    fn take(&mut self, blob: u32) -> (i32, Option<Vec<u8>>);
}

/// コアを Rust から直接呼ぶ形
#[derive(Debug, Default)]
pub struct Direct {
    session: Option<Session>,
}

impl Engine for Direct {
    fn session_new(&mut self, options: &[u8]) -> i32 {
        match Session::new(options) {
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
                let reply = session.call(request, &CancelToken::new());
                (reply.status.code(), Some(reply.response))
            }
            None => (Status::Invalid.code(), None),
        }
    }

    fn take(&mut self, blob: u32) -> (i32, Option<Vec<u8>>) {
        match self.session.as_mut().and_then(|session| session.take(blob)) {
            Some(bytes) => (Status::Ok.code(), Some(bytes)),
            None => (Status::Invalid.code(), None),
        }
    }
}

/// C ABI の関数（`zm_session_new` など）を呼ぶ形
#[derive(Debug)]
pub struct CAbi {
    session: *mut ffi::SessionHandle,
}

impl Default for CAbi {
    fn default() -> Self {
        Self {
            session: ptr::null_mut(),
        }
    }
}

impl Drop for CAbi {
    fn drop(&mut self) {
        unsafe { ffi::zm_session_close(self.session) };
    }
}

/// コアが書いたバッファーを写して返す。状態が成功でなければ、何も書かれていない
fn take_buf(status: i32, buf: &mut ffi::Buf) -> Option<Vec<u8>> {
    if buf.ptr.is_null() {
        return (status == Status::Ok.code()).then(Vec::new);
    }
    let bytes = unsafe { std::slice::from_raw_parts(buf.ptr, buf.len) }.to_vec();
    unsafe { ffi::zm_free(buf) };
    Some(bytes)
}

fn empty_buf() -> ffi::Buf {
    ffi::Buf {
        ptr: ptr::null_mut(),
        len: 0,
    }
}

impl Engine for CAbi {
    fn session_new(&mut self, options: &[u8]) -> i32 {
        unsafe { ffi::zm_session_close(self.session) };
        self.session = ptr::null_mut();
        unsafe { ffi::zm_session_new(options.as_ptr(), options.len(), &mut self.session) }
    }

    fn push(&mut self, name: &[u8], data: &[u8]) -> i32 {
        unsafe {
            ffi::zm_push(
                self.session,
                name.as_ptr(),
                name.len(),
                data.as_ptr(),
                data.len(),
            )
        }
    }

    fn call(&mut self, request: &[u8]) -> (i32, Option<Vec<u8>>) {
        let mut buf = empty_buf();
        let status =
            unsafe { ffi::zm_call(self.session, request.as_ptr(), request.len(), &mut buf) };
        (status, take_buf(status, &mut buf))
    }

    fn take(&mut self, blob: u32) -> (i32, Option<Vec<u8>>) {
        let mut buf = empty_buf();
        let status = unsafe { ffi::zm_take(self.session, blob, &mut buf) };
        (status, take_buf(status, &mut buf))
    }
}

/// 1 件の試験の組
#[derive(Debug)]
pub struct Case {
    /// 名前（ファイル名から `.json` を除いたもの）
    pub name: String,
    /// 中身
    pub value: Value,
}

/// 試験の組を、名前の順に読む
pub fn cases() -> Result<Vec<Case>, String> {
    let dir = root().join("cases");
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .map_err(|e| format!("{} を読めない: {e}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|path| {
            let name = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            let bytes = fs::read(path).map_err(|e| format!("{name}: 読めない: {e}"))?;
            let value = zumen_json::parse(&bytes)
                .map_err(|e| format!("{name}: JSON として読めない（{} バイト目）", e.offset))?;
            Ok(Case { name, value })
        })
        .collect()
}

/// 1 つの形で、すべての組を流した結果
#[derive(Debug, Default)]
pub struct Report {
    /// ハッシュの一覧の行
    pub hashes: Vec<String>,
    /// 期待と違ったこと
    pub failures: Vec<String>,
}

/// すべての組を流す。組ごとに、`engine` で新しい形を作る
pub fn run(cases: &[Case], mut engine: impl FnMut() -> Box<dyn Engine>) -> Report {
    let mut report = Report::default();
    for case in cases {
        let mut engine = engine();
        run_case(case, engine.as_mut(), &mut report);
    }
    report
}

const ACTIONS: &[&str] = &["session", "push", "call", "call_text", "call_hex", "take"];
const KEYS: &[&str] = &[
    "session",
    "push",
    "call",
    "call_text",
    "call_hex",
    "take",
    "status",
    "expect",
    "absent",
];

fn run_case(case: &Case, engine: &mut dyn Engine, report: &mut Report) {
    let name = &case.name;
    let Some(Value::Array(steps)) = case.value.pointer("/steps") else {
        report.failures.push(format!("{name}: steps がない"));
        return;
    };
    if !matches!(case.value.pointer("/description"), Some(Value::String(_))) {
        report.failures.push(format!("{name}: description がない"));
    }
    let mut has_session = false;
    for (index, step) in steps.iter().enumerate() {
        let at = format!("{name} の手順 {index}");
        let Value::Object(object) = step else {
            report.failures.push(format!("{at}: オブジェクトでない"));
            continue;
        };
        if let Some((key, _)) = object.iter().find(|(key, _)| !KEYS.contains(key)) {
            report.failures.push(format!("{at}: 知らない欄 `{key}`"));
        }
        let actions: Vec<&str> = object
            .iter()
            .map(|(key, _)| key)
            .filter(|key| ACTIONS.contains(key))
            .collect();
        let [action] = actions.as_slice() else {
            report.failures.push(format!(
                "{at}: 手順は {} のどれか 1 つにする",
                ACTIONS.join("・")
            ));
            continue;
        };
        let expected = match object.get("status") {
            Some(Value::String(status)) => status.as_str(),
            _ => "ok",
        };
        if *action != "session" && !has_session {
            engine.session_new(b"");
            has_session = true;
        }
        let input = object.get(action).unwrap_or(&Value::Null);
        let (status, output) = match *action {
            "session" => {
                has_session = true;
                match bytes_of(input, "options", "options_text") {
                    Ok(options) => (engine.session_new(&options), None),
                    Err(problem) => {
                        report.failures.push(format!("{at}: {problem}"));
                        continue;
                    }
                }
            }
            "push" => {
                let name = match input.pointer("/name_hex") {
                    Some(Value::String(hex)) => decode_hex(hex),
                    _ => match input.pointer("/name") {
                        Some(Value::String(name)) => Ok(name.as_bytes().to_vec()),
                        _ => Err("push に name か name_hex がない".to_string()),
                    },
                };
                let data = match (
                    input.pointer("/text"),
                    input.pointer("/hex"),
                    input.pointer("/file"),
                ) {
                    (Some(Value::String(text)), None, None) => Ok(text.as_bytes().to_vec()),
                    (None, Some(Value::String(hex)), None) => decode_hex(hex),
                    (None, None, Some(Value::String(file))) => read_sample(file),
                    (None, None, None) => Ok(Vec::new()),
                    _ => Err("push の text と hex と file は、どれか 1 つにする".to_string()),
                };
                match (name, data) {
                    (Ok(name), Ok(data)) => (engine.push(&name, &data), None),
                    (Err(problem), _) | (_, Err(problem)) => {
                        report.failures.push(format!("{at}: {problem}"));
                        continue;
                    }
                }
            }
            "take" => {
                let blob = match input {
                    Value::Number(number) => {
                        number.to_safe_integer().and_then(|n| u32::try_from(n).ok())
                    }
                    _ => None,
                };
                let Some(blob) = blob else {
                    report
                        .failures
                        .push(format!("{at}: take は 0 以上の整数にする"));
                    continue;
                };
                let (status, data) = engine.take(blob);
                (status, data.map(|data| ("blob", data)))
            }
            _ => {
                let request = match *action {
                    "call_text" => match input {
                        Value::String(text) => Ok(text.as_bytes().to_vec()),
                        _ => Err("call_text は文字列にする".to_string()),
                    },
                    "call_hex" => match input {
                        Value::String(hex) => decode_hex(hex),
                        _ => Err("call_hex は文字列にする".to_string()),
                    },
                    _ => Ok(compact(input)),
                };
                match request {
                    Ok(request) => {
                        let (status, response) = engine.call(&request);
                        (status, response.map(|response| ("response", response)))
                    }
                    Err(problem) => {
                        report.failures.push(format!("{at}: {problem}"));
                        continue;
                    }
                }
            }
        };
        let actual = status_name(status);
        if actual != expected {
            report
                .failures
                .push(format!("{at}: 状態が {actual}（期待は {expected}）"));
        }
        let Some((kind, data)) = output else {
            if object.get("expect").is_some() || object.get("absent").is_some() {
                report
                    .failures
                    .push(format!("{at}: 応答がないので、確かめられない"));
            }
            continue;
        };
        report
            .hashes
            .push(format!("{name} {index} {kind} {}", sha256::hex(&data)));
        if kind == "response" {
            check_response(&at, object, &data, &mut report.failures);
        }
    }
}

/// 応答を、期待する値（`expect`）と、ないはずの値（`absent`）で確かめる
fn check_response(
    at: &str,
    step: &zumen_json::Object,
    response: &[u8],
    failures: &mut Vec<String>,
) {
    let response = match zumen_json::parse(response) {
        Ok(response) => response,
        Err(error) => {
            failures.push(format!(
                "{at}: 応答が JSON として読めない（{} バイト目）",
                error.offset
            ));
            return;
        }
    };
    if let Some(Value::Object(expect)) = step.get("expect") {
        for (pointer, expected) in expect.iter() {
            match response.pointer(pointer) {
                Some(actual) if same(actual, expected) => {}
                Some(actual) => failures.push(format!(
                    "{at}: {pointer} が {}（期待は {}）",
                    String::from_utf8_lossy(&compact(actual)),
                    String::from_utf8_lossy(&compact(expected))
                )),
                None => failures.push(format!("{at}: {pointer} がない")),
            }
        }
    }
    if let Some(Value::Array(absent)) = step.get("absent") {
        for pointer in absent {
            if let Value::String(pointer) = pointer
                && response.pointer(pointer).is_some()
            {
                failures.push(format!("{at}: {pointer} があってはいけない"));
            }
        }
    }
}

/// 2 つの値が等しいか。数は値で、オブジェクトはキーの順によらずに比べる
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.to_f64().is_some() && x.to_f64() == y.to_f64(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same(x, y))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(key, x)| y.get(key).is_some_and(|y| same(x, y)))
        }
        _ => a == b,
    }
}

/// 値を、空白を入れない JSON にする
fn compact(value: &Value) -> Vec<u8> {
    let mut out = Writer::new();
    out.value(value);
    out.into_bytes()
}

/// `value` か `text` の欄からバイト列を作る。どちらもなければ空
fn bytes_of(input: &Value, value_key: &str, text_key: &str) -> Result<Vec<u8>, String> {
    match input {
        Value::Null => Ok(Vec::new()),
        Value::Object(object) => match (object.get(value_key), object.get(text_key)) {
            (Some(value), None) => Ok(compact(value)),
            (None, Some(Value::String(text))) => Ok(text.as_bytes().to_vec()),
            (None, None) => Ok(Vec::new()),
            _ => Err(format!("{value_key} と {text_key} は、どちらか 1 つにする")),
        },
        _ => Err("オブジェクトにする".to_string()),
    }
}

/// 公開サンプル（決定 0171）を読む。道筋はリポジトリの根から書き、`samples/` の中に限る
fn read_sample(file: &str) -> Result<Vec<u8>, String> {
    let inside = file.strip_prefix("samples/").is_some_and(|rest| {
        rest.split('/')
            .all(|part| !part.is_empty() && part != "." && part != ".." && !part.contains('\\'))
    });
    if !inside {
        return Err(format!(
            "push の file は samples/ の中のファイルにする: {file}"
        ));
    }
    fs::read(root().join("../..").join(file)).map_err(|e| format!("{file} を読めない: {e}"))
}

fn decode_hex(hex: &str) -> Result<Vec<u8>, String> {
    let digits: Vec<u32> = hex
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| {
            c.to_digit(16)
                .ok_or_else(|| format!("16 進でない文字 `{c}`"))
        })
        .collect::<Result<_, _>>()?;
    let (pairs, rest) = digits.as_chunks::<2>();
    if !rest.is_empty() {
        return Err("16 進の桁の数が奇数".to_string());
    }
    Ok(pairs
        .iter()
        .map(|[high, low]| (high * 16 + low) as u8)
        .collect())
}

fn status_name(code: i32) -> String {
    Status::ALL
        .iter()
        .find(|status| status.code() == code)
        .map_or_else(
            || format!("不明な状態 {code}"),
            |status| status.name().to_string(),
        )
}

/// 置いてあるハッシュの一覧（説明の行を除く）
pub fn read_hashes() -> Result<Vec<String>, String> {
    let path = root().join("hashes.txt");
    let text =
        fs::read_to_string(&path).map_err(|e| format!("{} を読めない: {e}", path.display()))?;
    Ok(text
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect())
}

/// ハッシュの一覧を書く
pub fn write_hashes(lines: &[String]) -> Result<(), String> {
    let path = root().join("hashes.txt");
    let mut text = HASHES_HEADER.to_string();
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    fs::write(&path, text).map_err(|e| format!("{} に書けない: {e}", path.display()))
}

/// 2 つの一覧の違いを、読める形にする
pub fn differences(expected: &[String], actual: &[String]) -> String {
    let mut out = String::new();
    for line in expected.iter().filter(|line| !actual.contains(line)) {
        out.push_str(&format!("- {line}\n"));
    }
    for line in actual.iter().filter(|line| !expected.contains(line)) {
        out.push_str(&format!("+ {line}\n"));
    }
    out
}
