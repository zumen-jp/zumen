//! 実行の形 `process`: 子プロセスの CLI に、同じメッセージを送る（決定 0013・0200・0201）。
//!
//! ライブラリが `zumen serve --stdio` を子プロセスとして起こし、標準入出力の JSON-RPC（1 行に 1 つ）で
//! `push`・`call`・`take`・`cancel` を送る。応答の JSON は、子プロセスが返したものを書き換えずに返す。
//!
//! - 実行ファイルは、設定の `process_path` か、ライブラリと同じフォルダーの `zumen`（Windows は `zumen.exe`）
//! - 子プロセスのメモリに上限（8 GiB）を付ける。Linux は `setrlimit`、Windows はジョブ・オブジェクト
//! - 子プロセスへは別のスレッドで書き、終わった子プロセスへ書いても SIGPIPE でホストを止めない
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
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
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
    /// 書くスレッドへ渡す行と、書けたかどうかの答え
    writes: Sender<Vec<u8>>,
    written: Receiver<bool>,
    lines: Receiver<Vec<u8>>,
    next: u64,
    /// 落ちたか、止めて捨てたか
    broken: bool,
    /// 子プロセスのメモリの上限を付けたジョブ・オブジェクト
    #[cfg(windows)]
    _job: win::Job,
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

/// このライブラリ（静的に結んだときは、実行ファイル）のパス
#[cfg(windows)]
fn library_path() -> Option<PathBuf> {
    win::path_of(library_path as *const ())
}

/// Unix と Windows のほかでは、ライブラリのパスを探さない。設定の `process_path` で渡す
#[cfg(not(any(unix, windows)))]
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

/// 子プロセスのメモリの上限。予算の既定の `input_bytes` と `decoded_bytes`（2 GiB ずつ）の和の 2 倍（決定 0201）。
/// 付けられるのは Linux と Windows。macOS は `RLIMIT_AS` を受け付けず、ほかの上限は mmap に効かない
#[cfg(any(target_os = "linux", target_os = "android", windows))]
const MEMORY_LIMIT: u64 = 8 << 30;

/// Linux の `setrlimit` で、子プロセスのアドレス空間に上限を付ける（決定 0201）
#[cfg(any(target_os = "linux", target_os = "android"))]
#[allow(unsafe_code)]
mod rlimit {
    use std::ffi::c_int;
    use std::io;
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    /// `RLIMIT_AS`（Linux の asm-generic/resource.h。mips だけ 6）
    #[cfg(not(any(
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "mips32r6",
        target_arch = "mips64r6"
    )))]
    const RLIMIT_AS: c_int = 9;
    #[cfg(any(
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "mips32r6",
        target_arch = "mips64r6"
    ))]
    const RLIMIT_AS: c_int = 6;

    /// `rlim_t`。musl は常に 64 bit、glibc と bionic は `unsigned long`
    #[cfg(target_env = "musl")]
    type Rlim = u64;
    #[cfg(not(target_env = "musl"))]
    type Rlim = std::ffi::c_ulong;

    /// POSIX の `struct rlimit`
    #[repr(C)]
    struct Rlimit {
        cur: Rlim,
        max: Rlim,
    }

    unsafe extern "C" {
        fn getrlimit(resource: c_int, limit: *mut Rlimit) -> c_int;
        fn setrlimit(resource: c_int, limit: *const Rlimit) -> c_int;
    }

    /// 子プロセスが exec する前に、アドレス空間の上限を `bytes` に下げる。今の上限が低ければ、そのまま
    pub(super) fn confine(command: &mut Command, bytes: u64) {
        // 32 bit の rlim_t で表せない値は、上限なし（RLIM_INFINITY）と同じ
        let bytes = Rlim::try_from(bytes).unwrap_or(Rlim::MAX);
        let apply = move || {
            let mut limit = Rlimit { cur: 0, max: 0 };
            // SAFETY: limit は書ける struct rlimit
            if unsafe { getrlimit(RLIMIT_AS, &mut limit) } != 0 {
                return Err(io::Error::last_os_error());
            }
            limit.cur = limit.cur.min(bytes);
            limit.max = limit.max.min(bytes);
            // SAFETY: limit は読める struct rlimit
            if unsafe { setrlimit(RLIMIT_AS, &limit) } != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        };
        // SAFETY: fork のあと exec の前に、子プロセスの側で呼ばれる。getrlimit と setrlimit は
        // async-signal-safe で、閉包は確保もロックもしない
        unsafe {
            command.pre_exec(apply);
        }
    }
}

/// 子プロセスへ書くときに、SIGPIPE でホストを止めない（決定 0201）。
///
/// 終わった子プロセスへ書くと、書き込みは EPIPE で失敗し、SIGPIPE も届く。ホストの SIGPIPE の扱い
/// （既定では止まる）と、ほかのスレッドは変えない。
///
/// - Linux と FreeBSD は、書いたスレッドに SIGPIPE を送る。書くスレッドで止めておけば、保留になるだけ
/// - macOS はプロセスに送るので、スレッドで止めても別のスレッドに届く。書き口に `F_SETNOSIGPIPE` を付ける
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd"
))]
#[allow(unsafe_code)]
mod sigpipe {
    use std::ffi::c_int;
    use std::process::ChildStdin;

    #[cfg(any(not(target_vendor = "apple"), test))]
    const SIGPIPE: c_int = 13;

    /// 書き口に SIGPIPE を送らせない
    #[cfg(target_vendor = "apple")]
    pub(super) fn guard(input: &ChildStdin) {
        use std::os::fd::AsRawFd;
        /// `F_SETNOSIGPIPE`（sys/fcntl.h）
        const F_SETNOSIGPIPE: c_int = 73;
        unsafe extern "C" {
            fn fcntl(fd: c_int, command: c_int, ...) -> c_int;
        }
        // SAFETY: input は開いている書き口で、F_SETNOSIGPIPE は int を 1 つ取る
        unsafe {
            fcntl(input.as_raw_fd(), F_SETNOSIGPIPE, 1 as c_int);
        }
    }

    /// 呼んだスレッドで SIGPIPE を止める
    #[cfg(not(target_vendor = "apple"))]
    pub(super) fn guard(_input: &ChildStdin) {
        use std::ffi::c_void;
        /// `SIG_BLOCK`。Linux は 0（mips・sparc は 1）、FreeBSD は 1
        #[cfg(all(
            any(target_os = "linux", target_os = "android"),
            not(any(
                target_arch = "mips",
                target_arch = "mips64",
                target_arch = "mips32r6",
                target_arch = "mips64r6",
                target_arch = "sparc",
                target_arch = "sparc64"
            ))
        ))]
        const SIG_BLOCK: c_int = 0;
        #[cfg(not(all(
            any(target_os = "linux", target_os = "android"),
            not(any(
                target_arch = "mips",
                target_arch = "mips64",
                target_arch = "mips32r6",
                target_arch = "mips64r6",
                target_arch = "sparc",
                target_arch = "sparc64"
            ))
        )))]
        const SIG_BLOCK: c_int = 1;
        unsafe extern "C" {
            fn sigemptyset(set: *mut c_void) -> c_int;
            fn sigaddset(set: *mut c_void, signal: c_int) -> c_int;
            fn pthread_sigmask(how: c_int, set: *const c_void, old: *mut c_void) -> c_int;
        }
        // sigset_t の大きさは OS で違う（glibc は 128 バイト、FreeBSD は 16 バイト）。大きい方に合わせる
        let mut set = [0u64; 16];
        let set = set.as_mut_ptr().cast::<c_void>();
        // SAFETY: set は、どの OS の sigset_t よりも大きく、揃った書ける領域
        unsafe {
            sigemptyset(set);
            sigaddset(set, SIGPIPE);
            pthread_sigmask(SIG_BLOCK, set, std::ptr::null_mut());
        }
    }

    /// 試験のために、プロセスの SIGPIPE の扱いを既定（止まる）か無視に変え、前の扱いを返す
    #[cfg(test)]
    pub(super) fn set_default(default: bool) -> bool {
        unsafe extern "C" {
            fn signal(signal: c_int, handler: usize) -> usize;
        }
        // SAFETY: SIG_DFL（0）と SIG_IGN（1）は、関数を指さない決まった値
        unsafe { signal(SIGPIPE, usize::from(!default)) == 0 }
    }
}

/// Windows の関数（kernel32）。依存を足さずに宣言する（決定 0201）
#[cfg(windows)]
#[allow(unsafe_code, non_snake_case)]
mod win {
    use std::ffi::{OsString, c_void};
    use std::os::windows::ffi::OsStringExt;
    use std::os::windows::io::AsRawHandle;
    use std::path::PathBuf;
    use std::process::Child;

    type Handle = *mut c_void;

    /// `GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT` と `GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS`
    const FROM_ADDRESS: u32 = 0x2 | 0x4;
    /// `JobObjectExtendedLimitInformation`
    const EXTENDED_LIMIT_INFORMATION: i32 = 9;
    /// `JOB_OBJECT_LIMIT_PROCESS_MEMORY`
    const LIMIT_PROCESS_MEMORY: u32 = 0x100;
    /// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`。ホストが落ちても、子プロセスを残さない
    const LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;
    /// パスの長さの上限（UNICODE_STRING で表せる文字の数）
    const PATH_LIMIT: usize = 32_768;

    /// `JOBOBJECT_BASIC_LIMIT_INFORMATION`
    #[repr(C)]
    #[derive(Default)]
    struct BasicLimit {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: u32,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    /// `JOBOBJECT_EXTENDED_LIMIT_INFORMATION`（`IO_COUNTERS` は u64 が 6 つ）
    #[repr(C)]
    #[derive(Default)]
    struct ExtendedLimit {
        basic: BasicLimit,
        io: [u64; 6],
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    #[cfg(target_pointer_width = "64")]
    const _: () = assert!(size_of::<ExtendedLimit>() == 144);
    #[cfg(target_pointer_width = "32")]
    const _: () = assert!(size_of::<ExtendedLimit>() == 112);

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleExW(flags: u32, name: *const u16, module: *mut Handle) -> i32;
        fn GetModuleFileNameW(module: Handle, name: *mut u16, size: u32) -> u32;
        fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> Handle;
        fn SetInformationJobObject(job: Handle, class: i32, info: *const c_void, size: u32) -> i32;
        fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
    }

    /// 番地を含むモジュール（DLL か実行ファイル）のパス
    pub(super) fn path_of(address: *const ()) -> Option<PathBuf> {
        let mut module: Handle = std::ptr::null_mut();
        // SAFETY: address はこのライブラリの関数の番地で、`module` は書けるハンドル。参照の数は変えない
        if unsafe { GetModuleHandleExW(FROM_ADDRESS, address.cast(), &mut module) } == 0 {
            return None;
        }
        let mut name = vec![0u16; 260];
        while name.len() <= PATH_LIMIT {
            let size = u32::try_from(name.len()).ok()?;
            // SAFETY: name は size 個の u16 を書ける
            let written = unsafe { GetModuleFileNameW(module, name.as_mut_ptr(), size) };
            if written == 0 {
                return None;
            }
            // 足りなければ、size で切って size を返す
            if written < size {
                name.truncate(usize::try_from(written).ok()?);
                return Some(PathBuf::from(OsString::from_wide(&name)));
            }
            name.resize(name.len() * 2, 0);
        }
        None
    }

    /// 子プロセスを入れたジョブ・オブジェクト。閉じると、子プロセスも止まる
    #[derive(Debug)]
    pub(super) struct Job(Handle);

    // SAFETY: カーネルのオブジェクトのハンドルは、どのスレッドからも使える
    unsafe impl Send for Job {}
    // SAFETY: Job はハンドルを閉じるときのほかに使わない
    unsafe impl Sync for Job {}

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: self.0 は CreateJobObjectW が返し、ほかで閉じないハンドル
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    /// 子プロセスをジョブ・オブジェクトに入れ、確定したメモリ（commit）の上限を `bytes` にする
    pub(super) fn confine(process: &Child, bytes: u64) -> Option<Job> {
        // SAFETY: 名前と属性のないジョブ・オブジェクトを作る
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return None;
        }
        let job = Job(handle);
        let mut info = ExtendedLimit::default();
        info.basic.limit_flags = LIMIT_PROCESS_MEMORY | LIMIT_KILL_ON_JOB_CLOSE;
        // 32 bit で表せない値は、上限なしと同じ
        info.process_memory_limit = usize::try_from(bytes).unwrap_or(usize::MAX);
        let size = u32::try_from(size_of::<ExtendedLimit>()).ok()?;
        // SAFETY: info は size バイトの JOBOBJECT_EXTENDED_LIMIT_INFORMATION
        let set = unsafe {
            SetInformationJobObject(
                job.0,
                EXTENDED_LIMIT_INFORMATION,
                (&raw const info).cast(),
                size,
            )
        };
        // SAFETY: job と、生きている子プロセスのハンドル
        if set == 0 || unsafe { AssignProcessToJobObject(job.0, process.as_raw_handle()) } == 0 {
            return None;
        }
        Some(job)
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

/// 子プロセスの標準入力へ、受けた行を書く。書けなければやめる。
///
/// 終わった子プロセスへ書くと SIGPIPE が届き、無視していないホストは止まる。書く前に防ぐ（決定 0201）
fn write_lines(mut input: ChildStdin, lines: &Receiver<Vec<u8>>, written: &Sender<bool>) {
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "freebsd"
    ))]
    sigpipe::guard(&input);
    for line in lines {
        let ok = input.write_all(&line).and_then(|()| input.flush()).is_ok();
        if written.send(ok).is_err() || !ok {
            break;
        }
    }
}

impl Child {
    /// 子プロセスを起こす
    pub(crate) fn spawn(path: &Path) -> Option<Self> {
        let mut command = Command::new(path);
        command
            .args(["serve", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(any(target_os = "linux", target_os = "android"))]
        rlimit::confine(&mut command, MEMORY_LIMIT);
        let mut process = command.spawn().ok()?;
        // 子プロセスは、標準入力から要求を読むまで何もしない。起こしてすぐ入れれば、上限の前に使うことはない
        #[cfg(windows)]
        let Some(job) = win::confine(&process, MEMORY_LIMIT) else {
            let _ = process.kill();
            let _ = process.wait();
            return None;
        };
        let started = Self::start(&mut process);
        let Some((writes, written, lines)) = started else {
            let _ = process.kill();
            let _ = process.wait();
            return None;
        };
        Some(Self {
            process,
            writes,
            written,
            lines,
            next: 1,
            broken: false,
            #[cfg(windows)]
            _job: job,
        })
    }

    /// 子プロセスの標準入力へ書くスレッドと、標準出力を読むスレッドを起こす
    #[allow(clippy::type_complexity)]
    fn start(
        process: &mut Process,
    ) -> Option<(Sender<Vec<u8>>, Receiver<bool>, Receiver<Vec<u8>>)> {
        let input = process.stdin.take()?;
        let output = process.stdout.take()?;
        let (send, lines) = channel();
        thread::Builder::new()
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
            })
            .ok()?;
        let (writes, receive) = channel::<Vec<u8>>();
        let (answer, written) = channel();
        thread::Builder::new()
            .name("zumen-process-input".to_string())
            .spawn(move || write_lines(input, &receive, &answer))
            .ok()?;
        Some((writes, written, lines))
    }

    fn write(&mut self, line: &[u8]) -> bool {
        // 終わった子プロセスには書かない
        if !matches!(self.process.try_wait(), Ok(None)) {
            return false;
        }
        self.send(line)
    }

    /// 書くスレッドに 1 行を渡し、書き終わるのを待つ
    fn send(&self, line: &[u8]) -> bool {
        let mut line = line.to_vec();
        line.push(b'\n');
        self.writes.send(line).is_ok() && self.written.recv() == Ok(true)
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
        if cfg!(any(unix, windows)) {
            let path = path.unwrap();
            let name = format!("zumen{}", std::env::consts::EXE_SUFFIX);
            assert!(path.ends_with(&name), "{}", path.display());
            assert!(path.parent().unwrap().is_absolute(), "{}", path.display());
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

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "freebsd"
    ))]
    #[test]
    fn survives_writing_to_a_child_that_has_ended() {
        let mut child = Child::spawn(Path::new("/usr/bin/true")).unwrap();
        child.process.wait().unwrap();
        // Rust の試験は SIGPIPE を無視して始まる。C のホストと同じ既定の扱い（止まる）にして書く。
        // ほかの試験の子プロセスが、exec までのあいだ読み口を持つことがあるので、失敗するまで書く
        let ignored = !sigpipe::set_default(true);
        let mut written = true;
        for _ in 0..100 {
            written = child.send(b"{}");
            if !written {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        sigpipe::set_default(!ignored);
        assert!(!written);
        assert!(!child.send(b"{}"));
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn limits_the_address_space_of_the_child() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "ulimit -v"]);
        rlimit::confine(&mut command, MEMORY_LIMIT);
        let output = command.output().unwrap();
        let kib = String::from_utf8(output.stdout).unwrap();
        assert_eq!(kib.trim(), (MEMORY_LIMIT / 1024).to_string());
    }

    /// 答えない子プロセス。標準出力を開いたまま、何も書かない
    #[cfg(unix)]
    fn silent() -> (PathBuf, Child) {
        use std::os::unix::fs::PermissionsExt;
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        // 呼ぶたびに別のフォルダーにする。動いている子のファイルを書き直さない
        let dir = std::env::temp_dir().join(format!(
            "zumen-abi-process-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("zumen");
        std::fs::write(&path, "#!/bin/sh\nexec sleep 60\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        // Linux では、ほかの試験のスレッドが fork した子が書き込みの記述子を exec まで
        // 持つので、書いたばかりのファイルの実行が ETXTBSY で一時的に失敗しうる
        for _ in 0..100 {
            if let Some(child) = Child::spawn(&path) {
                return (dir, child);
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("{} を動かせない", path.display());
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
        let (other, mut child) = silent();
        let cancel = CancelToken::new();
        cancel.cancel();
        let answer = child.call(br#"{"v":1,"op":"capabilities"}"#, &cancel);
        assert_eq!(answer, Err(Failure::Cancelled));
        for dir in [dir, other] {
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
