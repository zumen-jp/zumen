// 生成したファイル。手で直さない。
// 元は schema/boundary.toml で、`cargo xtask codegen` で作り直す。

//! 境界の版・状態コード・関数の形（決定 0012・0015）。

/// ABI の大版。`zm_abi_version` が返す
pub const ABI_VERSION: u32 = 1;
/// メッセージのスキーマの大版。要求と応答の `v`
pub const SCHEMA_MAJOR: u64 = 1;
/// メッセージのスキーマの小版。`capabilities` が返す
pub const SCHEMA_MINOR: u64 = 40;

/// 状態コード。理由は応答の診断で返す
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// 成功した
    Ok,
    /// 操作が失敗した。応答に error の診断がある
    Failed,
    /// 引数が使えない（NULL、長さが合わない、使えない名前など）。応答は書かない
    Invalid,
    /// 別のスレッドが、同じセッションを使っている
    Busy,
    /// コアの不具合で止まった。セッションは閉じるしかない
    Panicked,
}

impl Status {
    /// すべての状態
    pub const ALL: &[Self] = &[
        Self::Ok,
        Self::Failed,
        Self::Invalid,
        Self::Busy,
        Self::Panicked,
    ];

    /// C の値（`ZM_OK` など）
    pub const fn code(self) -> i32 {
        match self {
            Self::Ok => 0,
            Self::Failed => 1,
            Self::Invalid => 2,
            Self::Busy => 3,
            Self::Panicked => 4,
        }
    }

    /// 名前（`ok` など）
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Failed => "failed",
            Self::Invalid => "invalid",
            Self::Busy => "busy",
            Self::Panicked => "panicked",
        }
    }
}

/// ネイティブのライブラリが出す関数の名前
pub const NATIVE_FUNCTIONS: &[&str] = &[
    "zm_abi_version",
    "zm_session_new",
    "zm_push",
    "zm_call",
    "zm_take",
    "zm_take_arrow",
    "zm_cancel",
    "zm_session_close",
    "zm_free",
];

/// wasm のモジュールが出す関数の名前
pub const WASM_FUNCTIONS: &[&str] = &[
    "zm_abi_version",
    "zm_session_new",
    "zm_push",
    "zm_call",
    "zm_take",
    "zm_cancel",
    "zm_session_close",
    "zm_free",
    "zm_alloc",
    "zm_dealloc",
];

// 関数の形が schema/boundary.toml と合うことを、コンパイルで確かめる
const _: unsafe extern "system" fn() -> u32 = crate::ffi::zm_abi_version;
const _: unsafe extern "system" fn(*const u8, usize, *mut *mut crate::ffi::SessionHandle) -> i32 =
    crate::ffi::zm_session_new;
const _: unsafe extern "system" fn(
    *mut crate::ffi::SessionHandle,
    *const u8,
    usize,
    *const u8,
    usize,
) -> i32 = crate::ffi::zm_push;
const _: unsafe extern "system" fn(
    *mut crate::ffi::SessionHandle,
    *const u8,
    usize,
    *mut crate::ffi::Buf,
) -> i32 = crate::ffi::zm_call;
const _: unsafe extern "system" fn(
    *mut crate::ffi::SessionHandle,
    u32,
    *mut crate::ffi::Buf,
) -> i32 = crate::ffi::zm_take;
#[cfg(not(target_family = "wasm"))]
const _: unsafe extern "system" fn(
    *mut crate::ffi::SessionHandle,
    u32,
    *mut crate::ffi::ArrowArrayStream,
) -> i32 = crate::ffi::zm_take_arrow;
const _: unsafe extern "system" fn(*mut crate::ffi::SessionHandle) = crate::ffi::zm_cancel;
const _: unsafe extern "system" fn(*mut crate::ffi::SessionHandle) = crate::ffi::zm_session_close;
const _: unsafe extern "system" fn(*mut crate::ffi::Buf) = crate::ffi::zm_free;
#[cfg(target_family = "wasm")]
const _: unsafe extern "system" fn(usize) -> *mut u8 = crate::ffi::zm_alloc;
#[cfg(target_family = "wasm")]
const _: unsafe extern "system" fn(*mut u8, usize) = crate::ffi::zm_dealloc;
