//! zumen の境界（決定 0012・0013）。
//!
//! 各言語へは、数個の関数と、スキーマ（`schema/messages.json`）で定めたメッセージで届ける。
//!
//! - [`Session`]: 名前のついた入力と文書を持つセッション。要求を受けて、応答を返す。Rust からは、これを直接使う
//! - [`ffi`]: C ABI と wasm で出す関数（`zm_session_new`・`zm_call` など）。形は `schema/boundary.toml` にある
//! - [`messages`]: 要求と応答の型。`schema/messages.json` から生成する
//! - [`abi`]: 境界の版と状態コード。`schema/boundary.toml` から生成する
//!
//! ライブラリのファイル名には、ABI の大版を入れる（`zumen_abi1.dll` など、決定 0015）。

pub mod abi;
mod bounds;
mod changes;
mod check;
mod convert;
mod describe;
pub mod ffi;
mod geometry;
pub mod messages;
#[cfg(not(target_family = "wasm"))]
mod process;
mod scene;
mod session;
mod tables;

pub use abi::Status;
pub use session::{Reply, Session};
