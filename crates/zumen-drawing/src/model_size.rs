//! 図面モデル（L2）が保持する要素・表・拡張の論理バイト数（#559）。
//!
//! `std::mem::size_of` は 64 ビット環境と 32 ビットの wasm で値が変わるため使わない。図面バイナリの [`crate::binary`] と
//! 同じく、文字列と一覧は 24、`Box` は 8、`Option` は中身に 8 を足し、構造体はフィールドの和、列挙はいちばん大きい値に
//! 8 を足した 64 ビット相当の論理バイト数を `cargo xtask codegen` で生成する（`model_size/generated.rs`）。
//! 動的な領域は、一覧の項目数と項目の動的領域・文字列の UTF-8 バイト数・`Box` の中身を飽和加算する。

use std::borrow::Cow;

use zumen_base::{ByteRange, Cx, Key, Kind, Ref, Stop};

use crate::{Draft, Drawing2D, Zone};

mod generated;

/// いちばん大きい値。列挙の大きさ（[`ModelSize::SIZE`]）に使う
#[allow(dead_code)]
pub(crate) const fn largest(sizes: &[u64]) -> u64 {
    match sizes {
        [] => 0,
        [first, rest @ ..] => {
            let rest = largest(rest);
            if *first > rest { *first } else { rest }
        }
    }
}

/// 図面モデルの値の、ポインター幅によらない論理バイト数
pub trait ModelSize {
    /// 値そのものの論理バイト数
    const SIZE: u64;

    /// 値が所有する動的領域（一覧・文字列・`Box`）の論理バイト数
    fn heap_bytes(&self) -> u64 {
        0
    }

    /// 値そのものと動的領域を合わせた論理バイト数
    fn model_bytes(&self) -> u64 {
        Self::SIZE.saturating_add(self.heap_bytes())
    }
}

/// 値 `value` の論理バイト数を `Kind::ModelBytes` に計上する
pub fn check_model_bytes<T: ModelSize>(value: &T, cx: &mut Cx<'_>) -> Result<u64, Stop> {
    let bytes = value.model_bytes();
    cx.check(Kind::ModelBytes, bytes)?;
    Ok(bytes)
}

/// 未確定の拡張 `draft` が持つフィールド列の動的領域を `Kind::ModelBytes` に計上する
pub fn check_draft_bytes(draft: &Draft, cx: &mut Cx<'_>) -> Result<u64, Stop> {
    let bytes = draft.fields.heap_bytes();
    cx.check(Kind::ModelBytes, bytes)?;
    Ok(bytes)
}

macro_rules! fixed_model_size {
    ($($ty:ty => $size:expr),* $(,)?) => {
        $(
            impl ModelSize for $ty {
                const SIZE: u64 = $size;
            }
        )*
    };
}

fixed_model_size! {
    bool => 1,
    u8 => 1,
    u16 => 2,
    i16 => 2,
    u32 => 4,
    i32 => 4,
    char => 4,
    u64 => 8,
    usize => 8,
    i64 => 8,
    f64 => 8,
    Zone => 1,
}

impl ModelSize for String {
    const SIZE: u64 = 24;

    fn heap_bytes(&self) -> u64 {
        self.len() as u64
    }
}

impl ModelSize for Cow<'static, str> {
    const SIZE: u64 = 24;

    fn heap_bytes(&self) -> u64 {
        self.len() as u64
    }
}

impl<T: ModelSize> ModelSize for Option<T> {
    const SIZE: u64 = 8 + T::SIZE;

    fn heap_bytes(&self) -> u64 {
        self.as_ref().map_or(0, T::heap_bytes)
    }
}

impl<T: ModelSize> ModelSize for Vec<T> {
    const SIZE: u64 = 24;

    fn heap_bytes(&self) -> u64 {
        let base = (self.len() as u64).saturating_mul(T::SIZE);
        self.iter()
            .fold(base, |acc, item| acc.saturating_add(item.heap_bytes()))
    }
}

impl<T: ModelSize> ModelSize for Box<T> {
    const SIZE: u64 = 8;

    fn heap_bytes(&self) -> u64 {
        T::SIZE.saturating_add(T::heap_bytes(self))
    }
}

impl<T: ModelSize, const N: usize> ModelSize for [T; N] {
    const SIZE: u64 = N as u64 * T::SIZE;

    fn heap_bytes(&self) -> u64 {
        self.iter()
            .fold(0, |acc, item| acc.saturating_add(item.heap_bytes()))
    }
}

impl<A: ModelSize, B: ModelSize> ModelSize for (A, B) {
    const SIZE: u64 = A::SIZE + B::SIZE;

    fn heap_bytes(&self) -> u64 {
        self.0.heap_bytes().saturating_add(self.1.heap_bytes())
    }
}

impl ModelSize for Ref {
    const SIZE: u64 = Option::<String>::SIZE
        + Option::<u64>::SIZE
        + Option::<ByteRange>::SIZE
        + Option::<Key>::SIZE;

    fn heap_bytes(&self) -> u64 {
        self.id().map_or(0, |id| id.len() as u64)
    }
}

impl Drawing2D {
    /// 完成した図面モデル全体の論理バイト数
    pub fn total_model_bytes(&self) -> u64 {
        <Self as ModelSize>::model_bytes(self)
    }

    /// 途中で計上済みの `accounted` バイトとの差分だけを `Kind::ModelBytes` に計上し、合計を `total_model_bytes` にそろえる
    pub fn finish_model_bytes(&self, accounted: u64, cx: &mut Cx<'_>) -> Result<(), Stop> {
        cx.check(
            Kind::ModelBytes,
            self.total_model_bytes().saturating_sub(accounted),
        )
    }
}
