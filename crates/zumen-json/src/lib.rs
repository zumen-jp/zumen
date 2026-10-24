//! zumen の境界のメッセージの JSON（決定 0012）。
//!
//! - [`parse`]: UTF-8 の JSON（RFC 8259）を読む。キーが重なるもの、深すぎるもの、値のあとに続きがあるものは読まない。
//! - [`Writer`]: キーを書いた順に、空白を入れずに書く。浮動小数は、ECMAScript の `Number::toString` と同じ最短の表記にする。
//! - [`ObjectReader`] と、[`string`]・[`uint`] などの関数: 値を型に直し、合わない所を JSON Pointer で返す。
//!   スキーマから生成した型（`schema/messages.json`）が使う。
//!
//! どの環境でも、同じバイト列から同じ値を読み、同じ値から同じバイト列を書く（決定 0007）。

mod number;
mod parse;
mod read;
mod value;
mod write;

pub use number::float_text;
pub use parse::{MAX_DEPTH, SyntaxError, parse};
pub use read::{
    FromJson, Invalid, MAX_SAFE_INTEGER, ObjectReader, Problem, array, boolean, int, map, number,
    str, string, uint,
};
pub use value::{Number, Object, Value};
pub use write::{ToJson, Writer};
