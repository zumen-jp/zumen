//! C ABI と wasm で出す関数（決定 0012）。形は `schema/boundary.toml` にあり、`abi.rs` がコンパイルで確かめる。
//!
//! 関数は引数を確かめ、セッションの錠を取ってから [`Session`] に渡す。
//! - ネイティブでは、どの関数もパニックを `catch_unwind` で止める。そのセッションは、閉じるしかない
//! - wasm ではパニックは trap になり、ホストがインスタンスを作り直す（決定 0010）
//! - 32 bit の Windows では `extern "system"` が stdcall になる（VBA の `Declare` のため、決定 0011）

// FFI のために unsafe を使う。ワークスペースで禁じた unsafe を、このモジュールでだけ許す（決定 0012）
#![allow(unsafe_code)]

use core::ptr;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError, TryLockError};

use zumen_base::CancelToken;

use crate::abi::{ABI_VERSION, Status};
use crate::session::Session;

/// コアが返すバッファー（C の `zm_buf`）。`zm_free` で返す
#[repr(C)]
#[derive(Debug)]
pub struct Buf {
    /// 先頭。空なら NULL
    pub ptr: *mut u8,
    /// バイト数
    pub len: usize,
}

/// Arrow の C Stream Interface の `struct ArrowArrayStream`。この版では、中身に触れない
#[repr(C)]
#[derive(Debug)]
pub struct ArrowArrayStream {
    _private: [u8; 0],
}

/// C の `zm_session` の中身。セッションと、動いている呼び出しの中断の合図を持つ
#[derive(Debug)]
pub struct SessionHandle {
    session: Mutex<Session>,
    /// 動いている呼び出しの中断の合図。`zm_cancel` が、どのスレッドからでも使う
    running: Mutex<Option<CancelToken>>,
    /// コアの不具合で止まったか
    panicked: AtomicBool,
}

impl SessionHandle {
    fn new(session: Session) -> Self {
        Self {
            session: Mutex::new(session),
            running: Mutex::new(None),
            panicked: AtomicBool::new(false),
        }
    }

    /// 錠を取ってから `f` を呼ぶ。別のスレッドが使っていれば [`Status::Busy`]、不具合で止まったあとなら [`Status::Panicked`]
    fn with<T>(&self, f: impl FnOnce(&mut Session) -> T) -> Result<T, Status> {
        if self.panicked.load(Ordering::Acquire) {
            return Err(Status::Panicked);
        }
        let mut session = match self.session.try_lock() {
            Ok(session) => session,
            Err(TryLockError::WouldBlock) => return Err(Status::Busy),
            Err(TryLockError::Poisoned(_)) => return Err(Status::Panicked),
        };
        match catch_unwind(AssertUnwindSafe(|| f(&mut session))) {
            Ok(value) => Ok(value),
            Err(_) => {
                self.panicked.store(true, Ordering::Release);
                Err(Status::Panicked)
            }
        }
    }

    fn set_running(&self, token: Option<CancelToken>) {
        *self.running.lock().unwrap_or_else(PoisonError::into_inner) = token;
    }

    fn cancel_running(&self) {
        if let Some(token) = &*self.running.lock().unwrap_or_else(PoisonError::into_inner) {
            token.cancel();
        }
    }
}

/// パニックを止めて、状態コードにする
fn guard(f: impl FnOnce() -> Status) -> i32 {
    catch_unwind(AssertUnwindSafe(f))
        .unwrap_or(Status::Panicked)
        .code()
}

/// ポインターと長さを、借りるバイト列にする。長さが 0 なら、ポインターは見ない
///
/// # Safety
///
/// `len` が 0 でなければ、`ptr` から `len` バイトを、呼び出しの間だけ読めること。
unsafe fn bytes<'a>(ptr: *const u8, len: usize) -> Option<&'a [u8]> {
    if len == 0 {
        return Some(&[]);
    }
    if ptr.is_null() || isize::try_from(len).is_err() {
        return None;
    }
    // SAFETY: 呼ぶ側が、ptr から len バイトを呼び出しの間だけ読めるようにする（schema/boundary.toml の約束）
    Some(unsafe { core::slice::from_raw_parts(ptr, len) })
}

/// セッションのハンドルを借りる
///
/// # Safety
///
/// `session` は NULL か、`zm_session_new` が作って、まだ閉じていないものであること。
unsafe fn handle<'a>(session: *mut SessionHandle) -> Option<&'a SessionHandle> {
    // SAFETY: 上の約束による。ハンドルは共有して使い、中身は錠で守る
    unsafe { session.as_ref() }
}

/// コアのバイト列を、ホストが `zm_free` で返すバッファーにして書く
///
/// # Safety
///
/// `out` は NULL でなく、`zm_buf` を書けること。揃っていなくてもよい。
unsafe fn write_buf(out: *mut Buf, bytes: Vec<u8>) {
    let buf = if bytes.is_empty() {
        Buf {
            ptr: ptr::null_mut(),
            len: 0,
        }
    } else {
        let boxed = bytes.into_boxed_slice();
        let len = boxed.len();
        Buf {
            ptr: Box::into_raw(boxed).cast::<u8>(),
            len,
        }
    };
    // SAFETY: 上の約束による。wasm のホストが確保した所は揃っていないことがあるので、揃いを求めずに書く
    unsafe { out.write_unaligned(buf) };
}

/// ABI の大版を返す
#[unsafe(no_mangle)]
pub extern "system" fn zm_abi_version() -> u32 {
    ABI_VERSION
}

/// セッションを作る
///
/// # Safety
///
/// `options` から `options_len` バイトを読めること。`session` は NULL か、ポインターを書けること。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn zm_session_new(
    options: *const u8,
    options_len: usize,
    session: *mut *mut SessionHandle,
) -> i32 {
    guard(|| {
        if session.is_null() {
            return Status::Invalid;
        }
        // SAFETY: 関数の約束による
        let Some(options) = (unsafe { bytes(options, options_len) }) else {
            return Status::Invalid;
        };
        match Session::new(options) {
            Ok(created) => {
                let created = Box::into_raw(Box::new(SessionHandle::new(created)));
                // SAFETY: 関数の約束による
                unsafe { session.write_unaligned(created) };
                Status::Ok
            }
            Err(status) => status,
        }
    })
}

/// 名前のついた入力に、バイト列を足す
///
/// # Safety
///
/// `session` は NULL か、開いているセッション。`name`・`data` から、それぞれの長さを読めること。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn zm_push(
    session: *mut SessionHandle,
    name: *const u8,
    name_len: usize,
    data: *const u8,
    data_len: usize,
) -> i32 {
    guard(|| {
        // SAFETY: 関数の約束による
        let (Some(handle), Some(name), Some(data)) = (unsafe {
            (
                handle(session),
                bytes(name, name_len),
                bytes(data, data_len),
            )
        }) else {
            return Status::Invalid;
        };
        match handle.with(|session| session.push(name, data)) {
            Ok(Ok(())) => Status::Ok,
            Ok(Err(status)) | Err(status) => status,
        }
    })
}

/// 要求のメッセージを渡し、応答のメッセージを受け取る
///
/// # Safety
///
/// `session` は NULL か、開いているセッション。`request` から `request_len` バイトを読めること。
/// `response` は NULL か、`zm_buf` を書けること。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn zm_call(
    session: *mut SessionHandle,
    request: *const u8,
    request_len: usize,
    response: *mut Buf,
) -> i32 {
    guard(|| {
        // SAFETY: 関数の約束による
        let (Some(handle), Some(request)) =
            (unsafe { (handle(session), bytes(request, request_len)) })
        else {
            return Status::Invalid;
        };
        if response.is_null() {
            return Status::Invalid;
        }
        let outcome = handle.with(|session| {
            let cancel = CancelToken::new();
            handle.set_running(Some(cancel.clone()));
            let reply = session.call(request, &cancel);
            handle.set_running(None);
            reply
        });
        let (status, bytes) = match outcome {
            Ok(reply) => (reply.status, reply.response),
            Err(Status::Panicked) => (Status::Panicked, Session::panic_response()),
            Err(status) => return status,
        };
        // SAFETY: 関数の約束による
        unsafe { write_buf(response, bytes) };
        status
    })
}

/// 直前の `zm_call` が作ったバイナリの結果を取り出す
///
/// # Safety
///
/// `session` は NULL か、開いているセッション。`out` は NULL か、`zm_buf` を書けること。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn zm_take(
    session: *mut SessionHandle,
    blob: u32,
    out: *mut Buf,
) -> i32 {
    guard(|| {
        // SAFETY: 関数の約束による
        let Some(handle) = (unsafe { handle(session) }) else {
            return Status::Invalid;
        };
        if out.is_null() {
            return Status::Invalid;
        }
        match handle.with(|session| session.take(blob)) {
            Ok(Some(bytes)) => {
                // SAFETY: 関数の約束による
                unsafe { write_buf(out, bytes) };
                Status::Ok
            }
            Ok(None) => Status::Invalid,
            Err(status) => status,
        }
    })
}

/// 表の結果を、Arrow の C Stream Interface で取り出す
///
/// # Safety
///
/// `session` は NULL か、開いているセッション。`out` は NULL か、`struct ArrowArrayStream` を書けること。
#[cfg(not(target_family = "wasm"))]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn zm_take_arrow(
    session: *mut SessionHandle,
    blob: u32,
    out: *mut ArrowArrayStream,
) -> i32 {
    guard(|| {
        // SAFETY: 関数の約束による
        let Some(handle) = (unsafe { handle(session) }) else {
            return Status::Invalid;
        };
        if out.is_null() {
            return Status::Invalid;
        }
        // この版には、表（種類が arrow）の結果を作る op がないので、どの番号も使えない。
        // C Stream Interface で渡す形は、表を返す最初の op と一緒に作る
        let _ = blob;
        handle
            .with(|_| Status::Invalid)
            .unwrap_or_else(|status| status)
    })
}

/// 動いている呼び出しを止める
///
/// # Safety
///
/// `session` は NULL か、開いているセッション。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn zm_cancel(session: *mut SessionHandle) {
    guard(|| {
        // SAFETY: 関数の約束による
        if let Some(handle) = unsafe { handle(session) } {
            handle.cancel_running();
        }
        Status::Ok
    });
}

/// セッションを閉じる
///
/// # Safety
///
/// `session` は NULL か、開いているセッション。閉じたあとは使わないこと。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn zm_session_close(session: *mut SessionHandle) {
    guard(|| {
        // SAFETY: 関数の約束による
        let Some(handle) = (unsafe { handle(session) }) else {
            return Status::Ok;
        };
        // 別のスレッドが使っている途中なら、閉じない
        if matches!(handle.session.try_lock(), Err(TryLockError::WouldBlock)) {
            return Status::Busy;
        }
        // SAFETY: session は zm_session_new が Box から作り、まだ閉じていないもの。ここで返す
        drop(unsafe { Box::from_raw(session) });
        Status::Ok
    });
}

/// コアが返したバッファーを返す
///
/// # Safety
///
/// `buf` は NULL か、`zm_call`・`zm_take` が書いて、まだ返していない `zm_buf`。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn zm_free(buf: *mut Buf) {
    guard(|| {
        if buf.is_null() {
            return Status::Ok;
        }
        // SAFETY: 関数の約束による
        let Buf { ptr, len } = unsafe { buf.read_unaligned() };
        if !ptr.is_null() {
            // SAFETY: ptr と len は、write_buf が Box<[u8]> から作ったもの
            drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(ptr, len)) });
        }
        // SAFETY: 関数の約束による。返したあとは空にして、2 度目の zm_free を何もしないようにする
        unsafe {
            buf.write_unaligned(Buf {
                ptr: ptr::null_mut(),
                len: 0,
            });
        };
        Status::Ok
    });
}

/// ホストが要求や入力を書くためのメモリを確保する。wasm だけ
#[cfg(target_family = "wasm")]
#[unsafe(no_mangle)]
pub extern "system" fn zm_alloc(len: usize) -> *mut u8 {
    if len == 0 {
        return ptr::null_mut();
    }
    Box::into_raw(vec![0_u8; len].into_boxed_slice()).cast::<u8>()
}

/// `zm_alloc` で確保したメモリを返す。wasm だけ
///
/// # Safety
///
/// `ptr` は NULL か、`zm_alloc(len)` が返して、まだ返していないもの。
#[cfg(target_family = "wasm")]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn zm_dealloc(ptr: *mut u8, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    // SAFETY: 関数の約束による
    drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(ptr, len)) });
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAPABILITIES: &[u8] = br#"{"v":1,"op":"capabilities"}"#;

    fn empty() -> Buf {
        Buf {
            ptr: ptr::null_mut(),
            len: 0,
        }
    }

    fn open() -> *mut SessionHandle {
        let mut session = ptr::null_mut();
        assert_eq!(
            unsafe { zm_session_new(ptr::null(), 0, &mut session) },
            Status::Ok.code()
        );
        assert!(!session.is_null());
        session
    }

    /// 応答を取り出して、バッファーを返す
    fn take_text(buf: &mut Buf) -> String {
        let text =
            String::from_utf8(unsafe { core::slice::from_raw_parts(buf.ptr, buf.len) }.to_vec())
                .unwrap();
        unsafe { zm_free(buf) };
        assert!(buf.ptr.is_null());
        assert_eq!(buf.len, 0);
        text
    }

    #[test]
    fn creates_calls_and_closes_a_session() {
        assert_eq!(zm_abi_version(), 1);
        let session = open();
        let mut response = empty();
        let status = unsafe {
            zm_call(
                session,
                CAPABILITIES.as_ptr(),
                CAPABILITIES.len(),
                &mut response,
            )
        };
        assert_eq!(status, Status::Ok.code());
        assert!(take_text(&mut response).contains(r#""abi":1"#));
        // 2 度目の zm_free と NULL は、何もしない
        unsafe { zm_free(&mut response) };
        unsafe { zm_free(ptr::null_mut()) };
        unsafe { zm_session_close(session) };
        unsafe { zm_session_close(ptr::null_mut()) };
    }

    #[test]
    fn rejects_null_and_unusable_arguments_without_a_response() {
        let session = open();
        let mut response = empty();
        let mut out = ptr::null_mut();
        assert_eq!(
            unsafe { zm_session_new(ptr::null(), 0, ptr::null_mut()) },
            Status::Invalid.code()
        );
        assert_eq!(
            unsafe { zm_session_new(ptr::null(), 3, &mut out) },
            Status::Invalid.code()
        );
        assert_eq!(
            unsafe { zm_session_new(b"{".as_ptr(), 1, &mut out) },
            Status::Invalid.code()
        );
        let process = br#"{"v":1,"engine":"process","process_path":"no-such-folder/zumen"}"#;
        assert_eq!(
            unsafe { zm_session_new(process.as_ptr(), process.len(), &mut out) },
            Status::Failed.code()
        );
        assert!(out.is_null());
        assert_eq!(
            unsafe {
                zm_call(
                    ptr::null_mut(),
                    CAPABILITIES.as_ptr(),
                    CAPABILITIES.len(),
                    &mut response,
                )
            },
            Status::Invalid.code()
        );
        assert_eq!(
            unsafe {
                zm_call(
                    session,
                    CAPABILITIES.as_ptr(),
                    CAPABILITIES.len(),
                    ptr::null_mut(),
                )
            },
            Status::Invalid.code()
        );
        assert_eq!(
            unsafe { zm_call(session, ptr::null(), 5, &mut response) },
            Status::Invalid.code()
        );
        assert!(response.ptr.is_null());
        let name = b"../a";
        assert_eq!(
            unsafe { zm_push(session, name.as_ptr(), name.len(), ptr::null(), 0) },
            Status::Invalid.code()
        );
        assert_eq!(
            unsafe { zm_push(session, b"a".as_ptr(), 1, ptr::null(), 1) },
            Status::Invalid.code()
        );
        assert_eq!(
            unsafe { zm_push(session, b"a".as_ptr(), 1, ptr::null(), 0) },
            Status::Ok.code()
        );
        assert_eq!(
            unsafe { zm_take(session, 0, &mut response) },
            Status::Invalid.code()
        );
        assert_eq!(
            unsafe { zm_take(session, 0, ptr::null_mut()) },
            Status::Invalid.code()
        );
        let mut stream = ArrowArrayStream { _private: [] };
        assert_eq!(
            unsafe { zm_take_arrow(session, 0, &mut stream) },
            Status::Invalid.code()
        );
        assert_eq!(
            unsafe { zm_take_arrow(session, 0, ptr::null_mut()) },
            Status::Invalid.code()
        );
        unsafe { zm_session_close(session) };
    }

    #[test]
    fn answers_busy_while_another_thread_holds_the_session() {
        let session = open();
        let handle = unsafe { &*session };
        let held = handle.session.lock().unwrap();
        let mut response = empty();
        assert_eq!(
            unsafe {
                zm_call(
                    session,
                    CAPABILITIES.as_ptr(),
                    CAPABILITIES.len(),
                    &mut response,
                )
            },
            Status::Busy.code()
        );
        assert!(response.ptr.is_null());
        assert_eq!(
            unsafe { zm_push(session, b"a".as_ptr(), 1, b"1".as_ptr(), 1) },
            Status::Busy.code()
        );
        // 使っている途中のセッションは閉じない
        unsafe { zm_session_close(session) };
        drop(held);
        assert_eq!(
            unsafe {
                zm_call(
                    session,
                    CAPABILITIES.as_ptr(),
                    CAPABILITIES.len(),
                    &mut response,
                )
            },
            Status::Ok.code()
        );
        take_text(&mut response);
        unsafe { zm_session_close(session) };
    }

    #[test]
    fn a_panicked_session_answers_panicked_with_the_diagnostic() {
        let session = open();
        let handle = unsafe { &*session };
        let caught = handle.with(|_| -> () { panic!("試験のための panic") });
        assert_eq!(caught, Err(Status::Panicked));
        let mut response = empty();
        assert_eq!(
            unsafe {
                zm_call(
                    session,
                    CAPABILITIES.as_ptr(),
                    CAPABILITIES.len(),
                    &mut response,
                )
            },
            Status::Panicked.code()
        );
        assert!(take_text(&mut response).contains(r#""code":"internal.panic.caught""#));
        assert_eq!(
            unsafe { zm_push(session, b"a".as_ptr(), 1, ptr::null(), 0) },
            Status::Panicked.code()
        );
        unsafe { zm_session_close(session) };
        assert_eq!(
            guard(|| panic!("試験のための panic")),
            Status::Panicked.code()
        );
    }

    #[test]
    fn cancels_only_the_running_call() {
        let session = open();
        let handle = unsafe { &*session };
        // 動いている呼び出しがなければ、何もしない
        unsafe { zm_cancel(session) };
        unsafe { zm_cancel(ptr::null_mut()) };
        let token = CancelToken::new();
        handle.set_running(Some(token.clone()));
        unsafe { zm_cancel(session) };
        assert!(token.stop().is_some());
        handle.set_running(None);
        let mut response = empty();
        assert_eq!(
            unsafe {
                zm_call(
                    session,
                    CAPABILITIES.as_ptr(),
                    CAPABILITIES.len(),
                    &mut response,
                )
            },
            Status::Ok.code()
        );
        take_text(&mut response);
        unsafe { zm_session_close(session) };
    }

    #[test]
    fn writes_buffers_that_need_no_alignment() {
        let mut storage = [0_u8; 1 + core::mem::size_of::<Buf>()];
        let out = unsafe { storage.as_mut_ptr().add(1) }.cast::<Buf>();
        unsafe { write_buf(out, b"abc".to_vec()) };
        let buf = unsafe { out.read_unaligned() };
        assert_eq!(buf.len, 3);
        unsafe { zm_free(out) };
        assert!(unsafe { out.read_unaligned() }.ptr.is_null());
        unsafe { write_buf(out, Vec::new()) };
        assert!(unsafe { out.read_unaligned() }.ptr.is_null());
    }
}
