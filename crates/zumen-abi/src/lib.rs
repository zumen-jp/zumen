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

// モジュールを分けて作るときは、入れなかった op からしか呼ばない関数が残る（決定 0221）
#![cfg_attr(
    not(all(feature = "drawing", feature = "bim")),
    allow(dead_code, unused_imports, unused_variables)
)]

pub mod abi;
#[cfg(feature = "drawing")]
mod asbuilt;
mod bcf;
mod bounds;
mod building;
mod changes;
mod check;
#[cfg(feature = "drawing")]
mod civil;
mod convert;
mod describe;
pub mod ffi;
mod geometry;
pub mod messages;
#[cfg(feature = "drawing")]
mod pointcloud;
#[cfg(not(target_family = "wasm"))]
mod process;
mod review;
mod scene;
mod session;
mod tables;
mod validate;

pub use abi::Status;
pub use session::{Reply, Session};
