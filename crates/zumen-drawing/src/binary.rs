//! 図面モデル（L2）のバイナリ（決定 0246）。
//!
//! プラグインだけの wasm のモジュール（決定 0243）が読んだ図面を、公式の図面のモジュールに文書として渡すための配置。
//! 見出しと区画の表は、2D のシーン（決定 0137）と同じ形にし、区画の中には、値を型の契約（`schema/drawing.toml`）の
//! 順に並べる。
//!
//! - 符号のない整数は最短の LEB128、符号のある整数は zigzag の LEB128、`u8` は 1 バイト、`f64` はビットをリトル
//!   エンディアンの 8 バイトで置く。`char` は符号位置の整数
//! - `bool` と `Option` の有無は、0 か 1 の 1 バイト。文字列は、UTF-8 のバイト数と中身。一覧は、項目の数と項目。
//!   配列と組は、項目だけ
//! - 名前のある欄の構造体と、列挙の値の欄は、中身のバイト数を前に置く。小版で欄を足すときは後ろに足す。古い読み手は、
//!   知らない欄を読み飛ばして診断を足し、新しい読み手は、古いバイナリにない欄を既定の値にする
//! - 列挙は、値の番号（契約の順）と、値の中身。知らない番号の値は読めない
//! - 欄に名前のない構造体（`LayerId(u32)` など）は、中の値だけを置く
//!
//! 型ごとの読み書きは、`cargo xtask codegen` が図面モデルの型から作る（`binary/generated.rs`）。

use std::borrow::Cow;

use zumen_base::{ByteRange, Cx, Diagnostic, Key, Kind, Ref, Stop, Value, codes};

use crate::{Drawing2D, Hash};

mod generated;
#[cfg(any(test, feature = "random"))]
mod random;

pub use generated::{MAJOR, MINOR};

/// 見出しの始まりの 4 バイト
pub const MAGIC: [u8; 4] = *b"ZMDR";

/// 区画の番号。足すときは後ろに足す
const SOURCE: usize = 0;
const DRAWING: usize = 1;
/// この版が書く区画の数
const SECTIONS: usize = 2;
/// 区画の表の前のバイト数。始まりの 4 バイト・大版・小版・要素の数・区画の数
const HEADER: usize = 16;
/// 区画を置く境界
const ALIGN: usize = 8;

/// 図面を読んだ元の文書
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    /// 形式の台帳のキー
    pub format: String,
    /// 形式の系統（capabilities の family）
    pub family: String,
    /// 元の文書のハッシュ（決定 0156）。読んだバイト列から作る
    pub hash: Hash,
}

/// 読めなかった理由。理由の診断は、文脈に足してある
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// 予算の上限か、中断の合図で止めた
    Stopped(Stop),
    /// 壊れていて読めない
    Invalid,
    /// この版では読めない大版か、知らない値を使っている
    Unsupported,
}

/// 図面モデルのバイナリか。始まりの 4 バイトで見分ける
pub fn matches(bytes: &[u8]) -> bool {
    bytes.starts_with(&MAGIC)
}

/// 図面を、元の文書の情報と一緒にバイナリにする。同じ図面からは、どの環境でも同じバイト列になる
pub fn write(source: &Source, drawing: &Drawing2D) -> Vec<u8> {
    let mut first = Writer::default();
    source.pack(&mut first);
    let mut second = Writer::default();
    drawing.pack(&mut second);
    assemble(MINOR, second.elements, &[first.bytes, second.bytes])
}

/// バイナリを読む。壊れているか、この版で読めないものを使っていれば、診断を足して失敗する。後の小版で足した欄と
/// 区画を読み飛ばしたときは、診断を足す
pub fn read(bytes: &[u8], cx: &mut Cx<'_>) -> Result<(Source, Drawing2D), Error> {
    if let Err(stop) = cx.check(Kind::InputBytes, bytes.len() as u64) {
        cx.report(stop.to_diagnostic());
        return Err(Error::Stopped(stop));
    }
    let mut minor = MINOR;
    parse(bytes, &mut minor, cx).map_err(|bad| report(bad, minor, cx))
}

/// 見出しと区画の表を付けて、区画を並べる
fn assemble(minor: u16, elements: u64, sections: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&MAJOR.to_le_bytes());
    out.extend_from_slice(&minor.to_le_bytes());
    out.extend_from_slice(&word(elements).to_le_bytes());
    out.extend_from_slice(&word(sections.len() as u64).to_le_bytes());
    let mut start = aligned(HEADER + sections.len() * 8);
    for section in sections {
        out.extend_from_slice(&word(start as u64).to_le_bytes());
        out.extend_from_slice(&word(section.len() as u64).to_le_bytes());
        start = aligned(start + section.len());
    }
    for section in sections {
        out.resize(aligned(out.len()), 0);
        out.extend_from_slice(section);
    }
    out
}

/// u32 に収める。収まらない数は u32::MAX にする。読み手は、区画が入力の外に出るので壊れているとみなす
fn word(value: u64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn aligned(at: usize) -> usize {
    at.div_ceil(ALIGN) * ALIGN
}

/// 読めなかったこと。診断は、まだ足していない
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bad {
    /// 予算の上限か、中断の合図で止めた
    Stopped(Stop),
    /// `at` バイト目が壊れている
    Invalid { at: usize, reason: &'static str },
    /// 大版が違う
    Version { major: u16 },
    /// 列挙 `ty` の、知らない値
    Variant { ty: &'static str, value: u64 },
}

fn report(bad: Bad, minor: u16, cx: &mut Cx<'_>) -> Error {
    match bad {
        Bad::Stopped(stop) => {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        }
        Bad::Invalid { at, reason } => {
            cx.report(
                Diagnostic::new(codes::DRAWING_BINARY_INVALID)
                    .with("position", at as u64)
                    .with("reason", Value::Key(reason)),
            );
            Error::Invalid
        }
        Bad::Version { major } => {
            cx.report(
                Diagnostic::new(codes::DRAWING_BINARY_VERSION_UNSUPPORTED)
                    .with("major", u64::from(major))
                    .with("supported", u64::from(MAJOR)),
            );
            Error::Unsupported
        }
        Bad::Variant { ty, value } => {
            cx.report(
                Diagnostic::new(codes::DRAWING_BINARY_VALUE_UNSUPPORTED)
                    .with("type", Value::Key(ty))
                    .with("value", value)
                    .with("minor", u64::from(minor)),
            );
            Error::Unsupported
        }
    }
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<usize> {
    let value = u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?);
    usize::try_from(value).ok()
}

fn parse(bytes: &[u8], minor: &mut u16, cx: &mut Cx<'_>) -> Result<(Source, Drawing2D), Bad> {
    let invalid = |at, reason| Bad::Invalid { at, reason };
    if !matches(bytes) {
        return Err(invalid(0, "magic"));
    }
    let (Some(major), Some(written), Some(elements), Some(sections)) = (
        u16_at(bytes, 4),
        u16_at(bytes, 6),
        u32_at(bytes, 8),
        u32_at(bytes, 12),
    ) else {
        return Err(invalid(bytes.len(), "header"));
    };
    *minor = written;
    if major != MAJOR {
        return Err(Bad::Version { major });
    }
    let table = sections
        .checked_mul(8)
        .and_then(|len| len.checked_add(HEADER))
        .filter(|end| *end <= bytes.len())
        .ok_or(invalid(12, "sections"))?;
    if sections < SECTIONS {
        return Err(invalid(12, "sections"));
    }
    let mut ranges = Vec::with_capacity(SECTIONS);
    for index in 0..sections {
        let at = HEADER + index * 8;
        let (Some(start), Some(len)) = (u32_at(bytes, at), u32_at(bytes, at + 4)) else {
            return Err(invalid(at, "section"));
        };
        let end = start.checked_add(len).ok_or(invalid(at, "section"))?;
        if start < table || start % ALIGN != 0 || end > bytes.len() {
            return Err(invalid(at, "section"));
        }
        if index < SECTIONS {
            ranges.push(start..end);
        } else if written > MINOR {
            // 後の小版で足した区画は、読み飛ばす
            cx.report(
                Diagnostic::new(codes::DRAWING_BINARY_SECTION_DROPPED)
                    .with("section", index as u64)
                    .with("minor", u64::from(written)),
            );
        } else {
            // 知らない区画を持てるのは、この版より後の小版で書いたものだけ
            return Err(invalid(at, "section"));
        }
    }
    let (Some(first), Some(second)) = (ranges.get(SOURCE), ranges.get(DRAWING)) else {
        return Err(invalid(12, "sections"));
    };
    let mut reader = Reader {
        bytes,
        at: first.start,
        end: first.end,
        minor: written,
        elements: 0,
        limit: elements as u64,
        cx,
    };
    let source = Source::unpack(&mut reader)?;
    reader.finish()?;
    reader.at = second.start;
    reader.end = second.end;
    let drawing = Drawing2D::unpack(&mut reader)?;
    reader.finish()?;
    if reader.elements != reader.limit {
        return Err(invalid(8, "elements"));
    }
    Ok((source, drawing))
}

/// 書く所
#[derive(Default)]
pub(crate) struct Writer {
    bytes: Vec<u8>,
    /// 書いた要素（[`crate::Element`]）の数
    elements: u64,
}

/// 最短の LEB128 にする。返すのは、バイト列と使った長さ
fn leb128(mut value: u64) -> ([u8; 10], usize) {
    let mut out = [0; 10];
    let mut len = 0;
    for slot in &mut out {
        let low = (value & 0x7f) as u8;
        value >>= 7;
        len += 1;
        if value == 0 {
            *slot = low;
            break;
        }
        *slot = low | 0x80;
    }
    (out, len)
}

impl Writer {
    pub(crate) fn byte(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub(crate) fn uint(&mut self, value: u64) {
        let (bytes, len) = leb128(value);
        self.bytes.extend(bytes.iter().take(len));
    }

    pub(crate) fn int(&mut self, value: i64) {
        self.uint(((value << 1) ^ (value >> 63)) as u64);
    }

    pub(crate) fn float(&mut self, value: f64) {
        self.bytes.extend_from_slice(&value.to_bits().to_le_bytes());
    }

    pub(crate) fn text(&mut self, value: &str) {
        self.uint(value.len() as u64);
        self.bytes.extend_from_slice(value.as_bytes());
    }

    /// 中身のバイト数を前に置いて書く
    pub(crate) fn record(&mut self, write: impl FnOnce(&mut Self)) {
        // たいていの中身は 128 バイトより短いので、1 バイトを空けておき、長ければ後ろをずらす
        let start = self.bytes.len();
        self.bytes.push(0);
        write(self);
        let (prefix, len) = leb128((self.bytes.len() - start - 1) as u64);
        self.bytes
            .splice(start..=start, prefix.iter().take(len).copied());
    }

    /// 要素を 1 つ書く
    pub(crate) fn element(&mut self) {
        self.elements += 1;
    }
}

/// バイナリに書ける型
pub(crate) trait Pack {
    fn pack(&self, out: &mut Writer);
}

/// 値を取り出す所。バイナリを読む [`Reader`] と、試験のために乱数で値を作る所がある
pub(crate) trait Input {
    /// 取り出せなかったこと
    type Error;
    /// 1 バイト
    fn byte(&mut self) -> Result<u8, Self::Error>;
    /// 符号のない整数。`max` を超えれば壊れている
    fn uint(&mut self, max: u64) -> Result<u64, Self::Error>;
    /// 符号のある整数。`min` から `max` の外なら壊れている
    fn int(&mut self, min: i64, max: i64) -> Result<i64, Self::Error>;
    /// 浮動小数
    fn float(&mut self) -> Result<f64, Self::Error>;
    /// `bool` と、`Option` の有無
    fn flag(&mut self) -> Result<bool, Self::Error>;
    /// 文字
    fn char(&mut self) -> Result<char, Self::Error>;
    /// 一覧の項目の数。項目ごとに `size` バイトのメモリを使う
    fn count(&mut self, size: usize) -> Result<usize, Self::Error>;
    /// 文字列
    fn text(&mut self) -> Result<String, Self::Error>;
    /// 列挙 `ty` の値の番号。`count` 以上は、この版の知らない値
    fn variant(&mut self, ty: &'static str, count: u64) -> Result<u64, Self::Error>;
    /// 中身のバイト数を前に置いたもの。`read` が読み残した欄は、後の小版で足した欄として読み飛ばす
    fn record<T>(
        &mut self,
        ty: &'static str,
        read: impl FnOnce(&mut Self) -> Result<T, Self::Error>,
    ) -> Result<T, Self::Error>;
    /// 中身に、まだ欄があるか。後の小版で足した欄を、古いバイナリが持たないときに既定の値にする
    fn more(&self) -> bool;
    /// 要素を 1 つ読む
    fn element(&mut self) -> Result<(), Self::Error>;
    /// 壊れていること
    fn invalid(&self, reason: &'static str) -> Self::Error;
}

/// バイナリから読める型
pub(crate) trait Unpack: Sized {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error>;
}

/// バイナリを読む所
pub(crate) struct Reader<'a, 'b, 'c> {
    bytes: &'a [u8],
    /// 次に読む位置
    at: usize,
    /// 今の中身の終わり
    end: usize,
    /// 書いた側の小版
    minor: u16,
    /// 読んだ要素の数と、見出しにある数
    elements: u64,
    limit: u64,
    cx: &'b mut Cx<'c>,
}

impl Reader<'_, '_, '_> {
    fn bad(&self, reason: &'static str) -> Bad {
        Bad::Invalid {
            at: self.at,
            reason,
        }
    }

    fn take(&mut self, len: usize) -> Result<&[u8], Bad> {
        let end = self
            .at
            .checked_add(len)
            .filter(|end| *end <= self.end)
            .ok_or(self.bad("truncated"))?;
        let bytes = self.bytes.get(self.at..end).ok_or(self.bad("truncated"))?;
        self.at = end;
        Ok(bytes)
    }

    /// 区画の中身を読み終えた
    fn finish(&self) -> Result<(), Bad> {
        if self.at == self.end {
            Ok(())
        } else {
            Err(self.bad("trailing"))
        }
    }

    /// 項目の数。どの項目も 1 バイトより短くならないので、残りのバイト数より多い項目は持てない
    fn items(&mut self) -> Result<usize, Bad> {
        let start = self.at;
        let count = self.uint(usize::MAX as u64)? as usize;
        if count > self.end - self.at {
            self.at = start;
            return Err(self.bad("count"));
        }
        Ok(count)
    }
}

impl Input for Reader<'_, '_, '_> {
    type Error = Bad;

    fn byte(&mut self) -> Result<u8, Bad> {
        let [byte] = self.take(1)? else {
            return Err(self.bad("truncated"));
        };
        Ok(*byte)
    }

    fn uint(&mut self, max: u64) -> Result<u64, Bad> {
        let start = self.at;
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = self.byte()?;
            let bits = u64::from(byte & 0x7f);
            // 64 ビットを超えるか、最短でない形は壊れている
            if (shift == 63 && bits > 1) || (byte == 0 && shift > 0) {
                self.at = start;
                return Err(self.bad("integer"));
            }
            value |= bits << shift;
            if byte & 0x80 == 0 {
                if value > max {
                    self.at = start;
                    return Err(self.bad("range"));
                }
                return Ok(value);
            }
        }
        self.at = start;
        Err(self.bad("integer"))
    }

    fn int(&mut self, min: i64, max: i64) -> Result<i64, Bad> {
        let start = self.at;
        let raw = self.uint(u64::MAX)?;
        let value = ((raw >> 1) as i64) ^ -((raw & 1) as i64);
        if value < min || value > max {
            self.at = start;
            return Err(self.bad("range"));
        }
        Ok(value)
    }

    fn float(&mut self) -> Result<f64, Bad> {
        let start = self.at;
        let bytes: [u8; 8] = self.take(8)?.try_into().map_err(|_| Bad::Invalid {
            at: start,
            reason: "truncated",
        })?;
        Ok(f64::from_bits(u64::from_le_bytes(bytes)))
    }

    fn flag(&mut self) -> Result<bool, Bad> {
        let start = self.at;
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Bad::Invalid {
                at: start,
                reason: "flag",
            }),
        }
    }

    fn char(&mut self) -> Result<char, Bad> {
        let start = self.at;
        let value = self.uint(u64::from(u32::MAX))?;
        char::from_u32(value as u32).ok_or(Bad::Invalid {
            at: start,
            reason: "char",
        })
    }

    fn count(&mut self, size: usize) -> Result<usize, Bad> {
        let count = self.items()?;
        // 1 バイトで書ける項目が、メモリでは大きいことがある。作る前に、復号で作ったバイト数として数える
        let bytes = (count as u64).saturating_mul(size as u64);
        self.cx
            .check(Kind::DecodedBytes, bytes)
            .map_err(Bad::Stopped)?;
        Ok(count)
    }

    fn text(&mut self) -> Result<String, Bad> {
        let len = self.items()?;
        let start = self.at;
        let bytes = self.take(len)?;
        match std::str::from_utf8(bytes) {
            Ok(text) => Ok(text.to_owned()),
            Err(_) => Err(Bad::Invalid {
                at: start,
                reason: "utf-8",
            }),
        }
    }

    fn variant(&mut self, ty: &'static str, count: u64) -> Result<u64, Bad> {
        let value = self.uint(u64::MAX)?;
        if value >= count {
            return Err(Bad::Variant { ty, value });
        }
        Ok(value)
    }

    fn record<T>(
        &mut self,
        ty: &'static str,
        read: impl FnOnce(&mut Self) -> Result<T, Bad>,
    ) -> Result<T, Bad> {
        let len = self.items()?;
        let outer = self.end;
        self.end = self.at + len;
        let value = read(self)?;
        if self.more() {
            // 知らない欄を持てるのは、この版より後の小版で書いたものだけ
            if self.minor > MINOR {
                self.cx.report(
                    Diagnostic::new(codes::DRAWING_BINARY_FIELD_DROPPED)
                        .with("type", Value::Key(ty))
                        .with("minor", u64::from(self.minor)),
                );
                self.at = self.end;
            } else {
                return Err(self.bad("trailing"));
            }
        }
        self.end = outer;
        Ok(value)
    }

    fn more(&self) -> bool {
        self.at < self.end
    }

    fn element(&mut self) -> Result<(), Bad> {
        self.elements += 1;
        if self.elements > self.limit {
            return Err(self.bad("elements"));
        }
        self.cx.check(Kind::Elements, 1).map_err(Bad::Stopped)
    }

    fn invalid(&self, reason: &'static str) -> Bad {
        self.bad(reason)
    }
}

impl Pack for bool {
    fn pack(&self, out: &mut Writer) {
        out.byte(u8::from(*self));
    }
}

impl Unpack for bool {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.flag()
    }
}

impl Pack for u8 {
    fn pack(&self, out: &mut Writer) {
        out.byte(*self);
    }
}

impl Unpack for u8 {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.byte()
    }
}

macro_rules! unsigned {
    ($($ty:ty),*) => {$(
        impl Pack for $ty {
            fn pack(&self, out: &mut Writer) {
                out.uint(*self as u64);
            }
        }

        impl Unpack for $ty {
            fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
                Ok(input.uint(<$ty>::MAX as u64)? as $ty)
            }
        }
    )*};
}

macro_rules! signed {
    ($($ty:ty),*) => {$(
        impl Pack for $ty {
            fn pack(&self, out: &mut Writer) {
                out.int(i64::from(*self));
            }
        }

        impl Unpack for $ty {
            fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
                Ok(input.int(i64::from(<$ty>::MIN), i64::from(<$ty>::MAX))? as $ty)
            }
        }
    )*};
}

unsigned!(u16, u32, u64, usize);
signed!(i16, i32, i64);

impl Pack for f64 {
    fn pack(&self, out: &mut Writer) {
        out.float(*self);
    }
}

impl Unpack for f64 {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.float()
    }
}

impl Pack for char {
    fn pack(&self, out: &mut Writer) {
        out.uint(u64::from(u32::from(*self)));
    }
}

impl Unpack for char {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.char()
    }
}

impl Pack for String {
    fn pack(&self, out: &mut Writer) {
        out.text(self);
    }
}

impl Unpack for String {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.text()
    }
}

impl Pack for Cow<'static, str> {
    fn pack(&self, out: &mut Writer) {
        out.text(self);
    }
}

impl Unpack for Cow<'static, str> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.text().map(Cow::Owned)
    }
}

impl<T: Pack> Pack for Option<T> {
    fn pack(&self, out: &mut Writer) {
        match self {
            Some(value) => {
                out.byte(1);
                value.pack(out);
            }
            None => out.byte(0),
        }
    }
}

impl<T: Unpack> Unpack for Option<T> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        if input.flag()? {
            T::unpack(input).map(Some)
        } else {
            Ok(None)
        }
    }
}

impl<T: Pack> Pack for Vec<T> {
    fn pack(&self, out: &mut Writer) {
        out.uint(self.len() as u64);
        for item in self {
            item.pack(out);
        }
    }
}

impl<T: Unpack> Unpack for Vec<T> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        let count = input.count(size_of::<T>())?;
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            items.push(T::unpack(input)?);
        }
        Ok(items)
    }
}

impl<T: Pack> Pack for Box<T> {
    fn pack(&self, out: &mut Writer) {
        T::pack(self, out);
    }
}

impl<T: Unpack> Unpack for Box<T> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        T::unpack(input).map(Box::new)
    }
}

impl<T: Pack, const N: usize> Pack for [T; N] {
    fn pack(&self, out: &mut Writer) {
        for item in self {
            item.pack(out);
        }
    }
}

impl<T: Unpack, const N: usize> Unpack for [T; N] {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        let mut items = Vec::with_capacity(N);
        for _ in 0..N {
            items.push(T::unpack(input)?);
        }
        items.try_into().map_err(|_| input.invalid("array"))
    }
}

impl<A: Pack, B: Pack> Pack for (A, B) {
    fn pack(&self, out: &mut Writer) {
        self.0.pack(out);
        self.1.pack(out);
    }
}

impl<A: Unpack, B: Unpack> Unpack for (A, B) {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok((A::unpack(input)?, B::unpack(input)?))
    }
}

// 出所は欄を外に見せないので、手で書く。欄は契約の Ref と同じ順にする（xtask の HANDWRITTEN が確かめる）
impl Pack for Ref {
    fn pack(&self, out: &mut Writer) {
        out.record(|out| {
            self.id().map(str::to_owned).pack(out);
            self.record().pack(out);
            self.bytes().pack(out);
            self.key().pack(out);
        });
    }
}

impl Unpack for Ref {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Ref", |input| {
            let id = Option::<String>::unpack(input)?;
            let record = Option::<u64>::unpack(input)?;
            let bytes = Option::<ByteRange>::unpack(input)?;
            let key = Option::<Key>::unpack(input)?;
            let mut place = Ref::default();
            if let Some(id) = id {
                place = place.with_id(id);
            }
            if let Some(record) = record {
                place = place.with_record(record);
            }
            if let Some(ByteRange { start, end }) = bytes {
                place = place.with_bytes(start, end);
            }
            if let Some(key) = key {
                place = place.with_key(key);
            }
            Ok(place)
        })
    }
}

/// 試験のための乱数の図面と、元の文書の情報。契約にあるどの型の値も作るので、読み書きと、図面を受け取る処理の
/// 頑丈さを試せる。同じ `seed` からは同じ図面を作る。`finite` が false なら、浮動小数に NaN と無限大も入れる
#[cfg(any(test, feature = "random"))]
pub fn random(seed: u64, finite: bool) -> (Source, Drawing2D) {
    let mut input = random::Random::new(seed, finite);
    match (Source::unpack(&mut input), Drawing2D::unpack(&mut input)) {
        (Ok(source), Ok(drawing)) => (source, drawing),
        _ => (
            Source {
                format: String::new(),
                family: String::new(),
                hash: Hash([0; 16]),
            },
            Drawing2D::default(),
        ),
    }
}

#[cfg(test)]
mod tests;
