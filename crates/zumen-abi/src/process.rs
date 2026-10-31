//! 実行の形 `process`: 子プロセスの CLI に、同じメッセージを送る（決定 0013・0200）。
//!
//! ライブラリが `zumen serve --stdio` を子プロセスとして起こし、標準入出力の JSON-RPC（1 行に 1 つ）で
//! `push`・`call`・`take`・`cancel` を送る。応答の JSON は、子プロセスが返したものを書き換えずに返す。
//!
//! - 実行ファイルは、設定の `process_path` か、ライブラリと同じフォルダーの `zumen`
//! - 中断の合図を受けたら `cancel` を送る。時間の上限（要求の `time_limit_ms`）を過ぎても答えがなければ、
//!   猶予のあとで子プロセスを止めて捨てる。依存の中の、止めきれない処理も止まる
//! - 子プロセスが落ちたか、答えが読めなければ、その呼び出しは `call.process.caught` で失敗する。
//!   セッションは作り直すしかない
//!
//! この形はコアではなくホストの仕事なので、時計（`Instant`）を読む。結果の中身には効かない。
#![allow(clippy::disallowed_methods)]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child as Process, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::thread;
use std::time::{Duration, Instant};

use zumen_base::{CancelToken, Stop};
use zumen_json::{Value, Writer};

use crate::abi::Status;

/// 中断の合図や時間の上限のあと、子プロセスが自分で答えるのを待つ時間
const GRACE: Duration = Duration::from_secs(1);

/// 答えを待つあいだ、合図を確かめる間隔
const TICK: Duration = Duration::from_millis(10);

/// 呼び出しが失敗した理由
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Failure {
    /// 子プロセスが落ちたか、答えが読めない
    Caught,
    /// 中断の合図のあと、答えがなかったので止めた
    Cancelled,
    /// 時間の上限を過ぎても、答えがなかったので止めた
    TimedOut,
}

/// 子プロセスの CLI
#[derive(Debug)]
pub(crate) struct Child {
    process: Process,
    input: ChildStdin,
    lines: Receiver<Vec<u8>>,
    next: u64,
    /// 落ちたか、止めて捨てたか
    broken: bool,
}

/// 子プロセスの実行ファイルを決める。設定になければ、ライブラリと同じフォルダーの `zumen`
pub(crate) fn executable(path: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = path {
        return Some(PathBuf::from(path));
    }
    let library = library_path()?;
    let name = format!("zumen{}", std::env::consts::EXE_SUFFIX);
    Some(library.parent()?.join(name))
}

/// このライブラリ（静的に結んだときは、実行ファイル）のパス
#[cfg(unix)]
fn library_path() -> Option<PathBuf> {
    dl::path_of(library_path as *const ())
}

/// Windows では、まだライブラリのパスを探さない。設定の `process_path` で渡す
#[cfg(not(unix))]
fn library_path() -> Option<PathBuf> {
    None
}

#[cfg(unix)]
#[allow(unsafe_code)]
mod dl {
    use std::ffi::{CStr, c_char, c_int, c_void};
    use std::os::unix::ffi::OsStrExt;
    use std::path::PathBuf;

    /// POSIX の `Dl_info`（dladdr）
    #[repr(C)]
    struct DlInfo {
        dli_fname: *const c_char,
        dli_fbase: *mut c_void,
        dli_sname: *const c_char,
        dli_saddr: *mut c_void,
    }

    unsafe extern "C" {
        fn dladdr(address: *const c_void, info: *mut DlInfo) -> c_int;
    }

    /// 番地を含む共有オブジェクトのパス
    pub(super) fn path_of(address: *const ()) -> Option<PathBuf> {
        let mut info = DlInfo {
            dli_fname: std::ptr::null(),
            dli_fbase: std::ptr::null_mut(),
            dli_sname: std::ptr::null(),
            dli_saddr: std::ptr::null_mut(),
        };
        // SAFETY: info は書ける Dl_info で、address はこのライブラリの関数の番地
        let found = unsafe { dladdr(address.cast(), &mut info) };
        if found == 0 || info.dli_fname.is_null() {
            return None;
        }
        // SAFETY: dladdr が返した dli_fname は、NUL で終わる文字列
        let name = unsafe { CStr::from_ptr(info.dli_fname) };
        let path = PathBuf::from(std::ffi::OsStr::from_bytes(name.to_bytes()));
        // 実行ファイルのパスは、相対で返ることがある
        std::fs::canonicalize(&path).ok().or(Some(path))
    }
}

/// 状態の名前を値にする
fn status_of(name: &str) -> Option<Status> {
    Status::ALL.iter().copied().find(|s| s.name() == name)
}

fn text_of<'a>(value: &'a Value, pointer: &str) -> Option<&'a str> {
    match value.pointer(pointer) {
        Some(Value::String(text)) => Some(text),
        _ => None,
    }
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// base64（RFC 4648 の 4 節）で書く
fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                let digit = ALPHABET.get((n >> (18 - 6 * i) & 63) as usize).copied();
                out.push(char::from(digit.unwrap_or(b'A')));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// base64 を読む。読めなければ `None`
fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let text = text.trim_end_matches('=');
    let mut out = Vec::with_capacity(text.len() / 4 * 3 + 2);
    let (mut acc, mut bits) = (0u32, 0);
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
    (bits < 6).then_some(out)
}

/// 要求の `time_limit_ms`
fn time_limit(request: &[u8]) -> Option<Duration> {
    let value = zumen_json::parse(request).ok()?;
    match value.pointer("/time_limit_ms") {
        Some(Value::Number(n)) => n
            .to_safe_integer()
            .and_then(|ms| u64::try_from(ms).ok())
            .map(Duration::from_millis),
        _ => None,
    }
}

impl Child {
    /// 子プロセスを起こす
    pub(crate) fn spawn(path: &Path) -> Option<Self> {
        let mut process = Command::new(path)
            .args(["serve", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let input = process.stdin.take()?;
        let output = process.stdout.take()?;
        let (send, lines) = channel();
        let reader = thread::Builder::new()
            .name("zumen-process".to_string())
            .spawn(move || {
                let mut output = BufReader::new(output);
                loop {
                    let mut line = Vec::new();
                    match output.read_until(b'\n', &mut line) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {
                            if send.send(line).is_err() {
                                break;
                            }
                        }
                    }
                }
            });
        if reader.is_err() {
            let _ = process.kill();
            let _ = process.wait();
            return None;
        }
        Some(Self {
            process,
            input,
            lines,
            next: 1,
            broken: false,
        })
    }

    fn write(&mut self, line: &[u8]) -> bool {
        // 終わった子プロセスに書くと、SIGPIPE を無視しないホストは止まる。先に確かめる
        if !matches!(self.process.try_wait(), Ok(None)) {
            return false;
        }
        self.input
            .write_all(line)
            .and_then(|()| self.input.write_all(b"\n"))
            .and_then(|()| self.input.flush())
            .is_ok()
    }

    fn discard(&mut self) {
        self.broken = true;
        let _ = self.process.kill();
        let _ = self.process.wait();
    }

    /// メソッドを呼び、`result` を返す。`watch` があれば、中断の合図と時間の上限を見張る
    fn request(
        &mut self,
        method: &str,
        params: &[u8],
        watch: Option<(&CancelToken, Option<Duration>)>,
    ) -> Result<Value, Failure> {
        if self.broken {
            return Err(Failure::Caught);
        }
        let id = self.next;
        self.next += 1;
        let mut line =
            format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":"#).into_bytes();
        line.extend_from_slice(params);
        line.push(b'}');
        if !self.write(&line) {
            self.discard();
            return Err(Failure::Caught);
        }
        let started = Instant::now();
        let deadline = watch
            .and_then(|(_, limit)| limit)
            .map(|limit| limit + GRACE);
        let mut cancelled_at: Option<Instant> = None;
        loop {
            match self.lines.recv_timeout(TICK) {
                Ok(line) => {
                    let Ok(reply) = zumen_json::parse(&line) else {
                        self.discard();
                        return Err(Failure::Caught);
                    };
                    let same = matches!(
                        reply.pointer("/id"),
                        Some(Value::Number(n)) if n.to_safe_integer() == i64::try_from(id).ok()
                    );
                    if !same {
                        // 通知と、中断の答えは読み捨てる
                        continue;
                    }
                    return match reply.pointer("/result") {
                        Some(result) => Ok(result.clone()),
                        None => {
                            self.discard();
                            Err(Failure::Caught)
                        }
                    };
                }
                Err(RecvTimeoutError::Disconnected) => {
                    self.discard();
                    return Err(Failure::Caught);
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
            let Some((cancel, _)) = watch else { continue };
            if deadline.is_some_and(|deadline| started.elapsed() > deadline) {
                self.discard();
                return Err(Failure::TimedOut);
            }
            match (cancel.stop(), cancelled_at) {
                (Some(Stop::Cancelled), None) => {
                    cancelled_at = Some(Instant::now());
                    if !self.write(br#"{"jsonrpc":"2.0","method":"cancel"}"#) {
                        self.discard();
                        return Err(Failure::Caught);
                    }
                }
                (Some(_), Some(at)) if at.elapsed() > GRACE => {
                    self.discard();
                    return Err(Failure::Cancelled);
                }
                _ => {}
            }
        }
    }

    /// 入力を足す
    pub(crate) fn push(&mut self, name: &[u8], data: &[u8]) -> Result<(), Status> {
        let mut params = Writer::new();
        params.begin_object();
        params.key("name_base64");
        params.string(&base64_encode(name));
        params.key("base64");
        params.string(&base64_encode(data));
        params.end_object();
        let result = self
            .request("push", &params.into_bytes(), None)
            .map_err(|_| Status::Failed)?;
        match text_of(&result, "/status").and_then(status_of) {
            Some(Status::Ok) => Ok(()),
            Some(status) => Err(status),
            None => Err(Status::Failed),
        }
    }

    /// 要求を渡し、状態と応答を返す。応答のない状態（invalid と busy）なら、応答は `None`
    pub(crate) fn call(
        &mut self,
        request: &[u8],
        cancel: &CancelToken,
    ) -> Result<(Status, Option<Vec<u8>>), Failure> {
        let mut params = Writer::new();
        params.begin_object();
        params.key("request_base64");
        params.string(&base64_encode(request));
        params.end_object();
        let watch = Some((cancel, time_limit(request)));
        let result = self.request("call", &params.into_bytes(), watch)?;
        let status = text_of(&result, "/status")
            .and_then(status_of)
            .ok_or(Failure::Caught)?;
        let response = match (
            text_of(&result, "/response"),
            text_of(&result, "/response_base64"),
        ) {
            (Some(text), _) => Some(text.as_bytes().to_vec()),
            (None, Some(text)) => Some(base64_decode(text).ok_or(Failure::Caught)?),
            (None, None) => None,
        };
        Ok((status, response))
    }

    /// バイナリの結果を取り出す
    pub(crate) fn take(&mut self, blob: u32) -> Option<Vec<u8>> {
        let params = format!(r#"{{"blob":{blob}}}"#);
        let result = self.request("take", params.as_bytes(), None).ok()?;
        if text_of(&result, "/status") != Some(Status::Ok.name()) {
            return None;
        }
        base64_decode(text_of(&result, "/base64")?)
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_base64_and_reads_time_limits() {
        for bytes in [&b""[..], b"f", b"fo", b"foo", b"foob\xff"] {
            assert_eq!(base64_decode(&base64_encode(bytes)).unwrap(), bytes);
        }
        assert_eq!(
            time_limit(br#"{"v":1,"op":"capabilities","time_limit_ms":5}"#),
            Some(Duration::from_millis(5))
        );
        assert_eq!(time_limit(b"{"), None);
    }

    #[test]
    fn finds_the_cli_next_to_the_library() {
        let path = executable(None);
        if cfg!(unix) {
            let path = path.unwrap();
            assert!(path.ends_with("zumen"), "{}", path.display());
        }
        assert_eq!(executable(Some("a/zumen")), Some(PathBuf::from("a/zumen")));
        assert!(Child::spawn(Path::new("no-such-folder/zumen")).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn fails_the_call_when_the_child_is_not_the_cli() {
        let mut child = Child::spawn(Path::new("/usr/bin/true")).unwrap();
        let answer = child.call(br#"{"v":1,"op":"capabilities"}"#, &CancelToken::new());
        assert_eq!(answer, Err(Failure::Caught));
        assert_eq!(child.push(b"a", b""), Err(Status::Failed));
        assert_eq!(child.take(0), None);
    }

    /// 答えない子プロセス。標準出力を開いたまま、何も書かない
    #[cfg(unix)]
    fn silent() -> (PathBuf, Child) {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!(
            "zumen-abi-process-{}-{:?}",
            std::process::id(),
            thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("zumen");
        std::fs::write(&path, "#!/bin/sh\nexec sleep 60\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let child = Child::spawn(&path).unwrap();
        (dir, child)
    }

    #[cfg(unix)]
    #[test]
    fn discards_a_child_that_does_not_answer_in_time_or_after_a_cancel() {
        let (dir, mut child) = silent();
        let answer = child.call(
            br#"{"v":1,"op":"capabilities","time_limit_ms":10}"#,
            &CancelToken::new(),
        );
        assert_eq!(answer, Err(Failure::TimedOut));
        assert_eq!(
            child.call(br#"{"v":1,"op":"capabilities"}"#, &CancelToken::new()),
            Err(Failure::Caught)
        );
        let (_, mut child) = silent();
        let cancel = CancelToken::new();
        cancel.cancel();
        let answer = child.call(br#"{"v":1,"op":"capabilities"}"#, &cancel);
        assert_eq!(answer, Err(Failure::Cancelled));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
