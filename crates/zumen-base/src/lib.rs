//! zumen の横断の部品（決定 0008）。どの層からも使う。
//!
//! - [`code`] と [`codes`]: 診断の code と、台帳から生成した定数（決定 0009）
//! - [`diag`]: 診断と、そのまとめ方（決定 0009）
//! - [`budget`]: 決まった作業量で数える予算（決定 0010）
//! - [`cancel`]: 中断の合図（決定 0010）
//! - [`cx`]: 予算・診断・中断の合図を運ぶ文脈（決定 0008）
//! - [`name`]: 入力の名前の規則（決定 0010・0012）

pub mod budget;
pub mod cancel;
pub mod code;
pub mod codes;
pub mod crc;
pub mod cx;
pub mod diag;
pub mod name;

pub use budget::{Budget, Exceeded, Kind, Limit, Limits};
pub use cancel::CancelToken;
pub use code::{Code, Fidelity, Param, ParamType, Severity};
pub use cx::{Cx, Stop};
pub use diag::{ByteRange, Diagnostic, Diagnostics, Key, Ref, SpecRef, Value};
