//! 図面モデル（L2）のバイナリ（決定 0247）。
//!
//! プラグインだけの wasm のモジュール（決定 0243）が読んだ図面を、公式の図面のモジュールに文書として渡すための配置。
//! ヘッダーとセクションの表は、2D のシーン（決定 0137）と同じ形にし、セクションの中には、値を型の契約（`schema/drawing.toml`）の
//! 順に並べる。
//!
//! - 符号のない整数は最短の LEB128、符号のある整数は zigzag の LEB128、`u8` は 1 バイト、`f64` はビットをリトル
//!   エンディアンの 8 バイトで置く。`char` は符号位置の整数
//! - `bool` と `Option` の有無は、0 か 1 の 1 バイト。文字列は、UTF-8 のバイト数と中身。一覧は、項目の数と項目。
//!   配列と組は、項目だけ
//! - 名前のあるフィールドの構造体と、列挙の値のフィールドは、中身のバイト数を前に置く。マイナーバージョンでフィールドを足すときは後ろに足す。古いリーダーは、
//!   知らないフィールドを読み飛ばして診断を足し、新しいリーダーは、古いバイナリにないフィールドを既定の値にする
//! - 列挙は、値の番号（契約の順）と、値の中身。知らない番号の値は読めない
//! - フィールドに名前のない構造体（`LayerId(u32)` など）は、中の値だけを置く
//!
//! 型ごとの読み書きは、`cargo xtask codegen` が図面モデルの型から作る（`binary/generated.rs`）。

use std::borrow::Cow;
use std::collections::BTreeSet;

use zumen_base::{ByteRange, Cx, Diagnostic, Key, Kind, Ref, Stop, Value, codes};

use crate::{Drawing2D, Hash};

mod generated;
#[cfg(any(test, feature = "random"))]
mod random;

pub use generated::{MAJOR, MINOR};

/// ヘッダーの始まりの 4 バイト
pub const MAGIC: [u8; 4] = *b"ZMDR";

/// セクションの番号。足すときは後ろに足す
const SOURCE: usize = 0;
const DRAWING: usize = 1;
/// このバージョンが書くセクションの数
const SECTIONS: usize = 2;
/// セクションの表の前のバイト数。始まりの 4 バイト・メジャーバージョン・マイナーバージョン・要素の数・セクションの数
const HEADER: usize = 16;
/// セクションを置く境界
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
    /// このバージョンでは読めないメジャーバージョンか、知らない値を使っている
    Unsupported,
}

/// バイナリを書けなかった理由
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteError {
    /// 予算の上限か、中断の合図で止めた
    Stopped(Stop),
    /// セクションの位置・長さか要素の数が形式の上限を超えた
    TooLarge,
}

/// 図面モデルのバイナリか。始まりの 4 バイトで見分ける
pub fn matches(bytes: &[u8]) -> bool {
    bytes.starts_with(&MAGIC)
}

/// 図面を、元の文書の情報と一緒にバイナリにする。同じ図面からは、どの環境でも同じバイト列になる。要素を 1 つ
/// 書くごとに作業量（`work`）を 1 使い、中断の合図を確かめる。止めたときは、書きかけのものを捨てる
pub fn write(source: &Source, drawing: &Drawing2D, cx: &mut Cx<'_>) -> Result<Vec<u8>, WriteError> {
    // 要素のない図面でも、中断の合図は確かめる
    cx.check(Kind::Work, 0).map_err(WriteError::Stopped)?;
    let mut out = Writer {
        bytes: Vec::new(),
        elements: 0,
        cx,
        stop: None,
    };
    source.pack(&mut out);
    let first = std::mem::take(&mut out.bytes);
    drawing.pack(&mut out);
    match out.stop {
        Some(stop) => Err(WriteError::Stopped(stop)),
        None if !fits(out.elements, &[first.len() as u64, out.bytes.len() as u64]) => {
            Err(WriteError::TooLarge)
        }
        None => Ok(assemble(MINOR, out.elements, &[first, out.bytes])),
    }
}

/// ヘッダーとセクションを含む全体の大きさと、要素の数が u32 に収まるか
fn fits(elements: u64, sections: &[u64]) -> bool {
    let max = u64::from(u32::MAX);
    if elements > max || sections.len() as u64 > max {
        return false;
    }
    let mut end = HEADER as u64 + sections.len() as u64 * 8;
    for &len in sections {
        let Some(start) = end
            .checked_add(ALIGN as u64 - 1)
            .map(|at| at & !(ALIGN as u64 - 1))
        else {
            return false;
        };
        let Some(next) = start.checked_add(len) else {
            return false;
        };
        if start > max || len > max || next > max {
            return false;
        }
        end = next;
    }
    true
}

/// バイナリを読む。壊れているか、このバージョンで読めないものを使っていれば、診断を足して失敗する。後のマイナーバージョンで足したフィールドと
/// セクションを読み飛ばしたときは、診断を足す
pub fn read(bytes: &[u8], cx: &mut Cx<'_>) -> Result<(Source, Drawing2D), Error> {
    if let Err(stop) = cx.check(Kind::InputBytes, bytes.len() as u64) {
        cx.report(stop.to_diagnostic());
        return Err(Error::Stopped(stop));
    }
    let mut minor = MINOR;
    parse(bytes, &mut minor, cx).map_err(|bad| report(bad, minor, cx))
}

/// ヘッダーとセクションの表を付けて、セクションを並べる
fn assemble(minor: u16, elements: u64, sections: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&MAJOR.to_le_bytes());
    out.extend_from_slice(&minor.to_le_bytes());
    out.extend_from_slice(&word(elements).to_le_bytes());
    out.extend_from_slice(&word(sections.len() as u64).to_le_bytes());
    let mut start = aligned(HEADER + sections.len() * 8);
    for (index, section) in sections.iter().enumerate() {
        out.extend_from_slice(&word(start as u64).to_le_bytes());
        out.extend_from_slice(&word(section.len() as u64).to_le_bytes());
        if index + 1 < sections.len() {
            start = aligned(start + section.len());
        }
    }
    for section in sections {
        out.resize(aligned(out.len()), 0);
        out.extend_from_slice(section);
    }
    out
}

/// u32 に収める。書き手は `fits` で範囲を確かめる。テストで壊れた配置を組むときは u32::MAX にする
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
    /// メジャーバージョンが違う
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
    let mut previous_end = table;
    for index in 0..sections {
        let at = HEADER + index * 8;
        let (Some(start), Some(len)) = (u32_at(bytes, at), u32_at(bytes, at + 4)) else {
            return Err(invalid(at, "section"));
        };
        let end = start.checked_add(len).ok_or(invalid(at, "section"))?;
        let expected = previous_end
            .checked_add(ALIGN - 1)
            .map(|at| at & !(ALIGN - 1))
            .ok_or(invalid(at, "section"))?;
        if start != expected
            || end > bytes.len()
            || bytes
                .get(previous_end..start)
                .is_none_or(|padding| padding.iter().any(|byte| *byte != 0))
        {
            return Err(invalid(at, "section"));
        }
        previous_end = end;
        if index < SECTIONS {
            ranges.push(start..end);
        } else if written > MINOR {
            // 後のマイナーバージョンで足したセクションは、読み飛ばす
            cx.report(
                Diagnostic::new(codes::DRAWING_BINARY_SECTION_DROPPED)
                    .with("section", index as u64)
                    .with("minor", u64::from(written)),
            );
        } else {
            // 知らないセクションを持てるのは、このバージョンより後のマイナーバージョンで書いたものだけ
            return Err(invalid(at, "section"));
        }
    }
    if previous_end != bytes.len() {
        return Err(invalid(previous_end, "trailing"));
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
    // 後のマイナーバージョンの書き手は、このバージョンが読み飛ばすフィールドの要素も数える。そのときのヘッダーの数は、上限として確かめる
    if reader.elements == reader.limit || written > MINOR {
        // SpaceId は実寸の空間と定義の空間を通して重ならない。重なりを直して読むと ID の契約を壊すので断る（0264）。
        let mut spaces = BTreeSet::new();
        for space in drawing
            .spaces
            .iter()
            .chain(drawing.symbols.iter().map(|s| &s.space))
        {
            reader.cx.check(Kind::Work, 1).map_err(Bad::Stopped)?;
            if !spaces.insert(space.id) {
                return Err(invalid(second.start, "duplicate-space-id"));
            }
        }
        Ok((source, drawing))
    } else {
        Err(invalid(8, "elements"))
    }
}

/// 書く所
pub(crate) struct Writer<'a, 'b> {
    bytes: Vec<u8>,
    /// 書いた要素（[`crate::Element`]）の数
    elements: u64,
    cx: &'a mut Cx<'b>,
    /// 止めた理由。止めたあとは、中身を書かない
    stop: Option<Stop>,
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

impl Writer<'_, '_> {
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

    /// 中身のバイト数を前に置いて書く。止めたあとは書かない
    pub(crate) fn record(&mut self, write: impl FnOnce(&mut Self)) {
        if self.stop.is_some() {
            return;
        }
        // たいていの中身は 128 バイトより短いので、1 バイトを空けておき、長ければ後ろをずらす
        let start = self.bytes.len();
        self.bytes.push(0);
        write(self);
        let (prefix, len) = leb128((self.bytes.len() - start - 1) as u64);
        self.bytes
            .splice(start..=start, prefix.iter().take(len).copied());
    }

    /// 要素を 1 つ書く。作業量を 1 使い、中断の合図を確かめる
    pub(crate) fn element(&mut self) {
        self.elements += 1;
        if self.stop.is_none() {
            self.stop = self.cx.check(Kind::Work, 1).err();
        }
    }
}

/// バイナリに書ける型
pub(crate) trait Pack {
    fn pack(&self, out: &mut Writer);
}

/// 値を取り出す所。バイナリを読む [`Reader`] と、テストのために乱数で値を作る所がある
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
    /// 一覧の項目の数。項目ごとに `size` バイト（[`Unpack::SIZE`]）のメモリを使う
    fn count(&mut self, size: u64) -> Result<usize, Self::Error>;
    /// 作る値の大きさを、復号で作ったバイト数として数える
    fn decoded(&mut self, bytes: u64) -> Result<(), Self::Error>;
    /// 文字列
    fn text(&mut self) -> Result<String, Self::Error>;
    /// 列挙 `ty` の値の番号。`count` 以上は、このバージョンの知らない値
    fn variant(&mut self, ty: &'static str, count: u64) -> Result<u64, Self::Error>;
    /// 中身のバイト数を前に置いたもの。`read` が読み残したフィールドは、後のマイナーバージョンで足したフィールドとして読み飛ばす
    fn record<T>(
        &mut self,
        ty: &'static str,
        read: impl FnOnce(&mut Self) -> Result<T, Self::Error>,
    ) -> Result<T, Self::Error>;
    /// 書き手のマイナーバージョンが、フィールドを足したマイナーバージョン以上か
    #[allow(dead_code)] // マイナーバージョン 0 には、後から足したフィールドがまだない
    fn has_field(&self, since: u16) -> bool;
    /// 中身に、まだフィールドがあるか。後のマイナーバージョンで足したフィールドを、古いバイナリが持たないときに既定の値にする
    fn more(&self) -> bool;
    /// 要素を 1 つ読む
    fn element(&mut self) -> Result<(), Self::Error>;
    /// 壊れていること
    fn invalid(&self, reason: &'static str) -> Self::Error;
}

/// バイナリから読める型
pub(crate) trait Unpack: Sized {
    /// 値 1 つの大きさ。復号で作ったバイト数（`decoded-bytes`）に数える。64 ビットのメモリでの大きさに近い値を
    /// 型から決めるので、どの環境でも同じ所で止まる。文字列と一覧は 24、`Box` は 8、`Option` は中身に 8 を足し、
    /// 構造体はフィールドの和、列挙はいちばん大きい値に 8 を足す。文字列の中身と、一覧の項目と、`Box` の中身は、作るときに
    /// 数える
    const SIZE: u64;
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error>;
}

/// いちばん大きい値。列挙の大きさ（[`Unpack::SIZE`]）に使う
pub(crate) const fn largest(sizes: &[u64]) -> u64 {
    match sizes {
        [] => 0,
        [first, rest @ ..] => {
            let rest = largest(rest);
            if *first > rest { *first } else { rest }
        }
    }
}

/// バイナリを読む所
pub(crate) struct Reader<'a, 'b, 'c> {
    bytes: &'a [u8],
    /// 次に読む位置
    at: usize,
    /// 今の中身の終わり
    end: usize,
    /// 書いた側のマイナーバージョン
    minor: u16,
    /// 読んだ要素の数と、ヘッダーにある数
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

    /// セクションの中身を読み終えた
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

    fn count(&mut self, size: u64) -> Result<usize, Bad> {
        let count = self.items()?;
        // 1 バイトで書ける項目が、メモリでは大きいことがある。作る前に、復号で作ったバイト数として数える
        self.decoded((count as u64).saturating_mul(size))?;
        Ok(count)
    }

    fn decoded(&mut self, bytes: u64) -> Result<(), Bad> {
        self.cx
            .check(Kind::DecodedBytes, bytes)
            .map_err(Bad::Stopped)
    }

    fn text(&mut self) -> Result<String, Bad> {
        let len = self.items()?;
        // 複製する前に数える
        self.decoded(len as u64)?;
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
            // 知らないフィールドを持てるのは、このバージョンより後のマイナーバージョンで書いたものだけ
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

    fn has_field(&self, since: u16) -> bool {
        self.minor >= since
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
    const SIZE: u64 = 1;

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
    const SIZE: u64 = 1;

    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.byte()
    }
}

macro_rules! unsigned {
    ($($ty:ty = $size:literal),*) => {$(
        impl Pack for $ty {
            fn pack(&self, out: &mut Writer) {
                out.uint(*self as u64);
            }
        }

        impl Unpack for $ty {
            const SIZE: u64 = $size;

            fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
                Ok(input.uint(<$ty>::MAX as u64)? as $ty)
            }
        }
    )*};
}

macro_rules! signed {
    ($($ty:ty = $size:literal),*) => {$(
        impl Pack for $ty {
            fn pack(&self, out: &mut Writer) {
                out.int(i64::from(*self));
            }
        }

        impl Unpack for $ty {
            const SIZE: u64 = $size;

            fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
                Ok(input.int(i64::from(<$ty>::MIN), i64::from(<$ty>::MAX))? as $ty)
            }
        }
    )*};
}

// `usize` も、64 ビットの大きさで数える
unsigned!(u16 = 2, u32 = 4, u64 = 8, usize = 8);
signed!(i16 = 2, i32 = 4, i64 = 8);

impl Pack for f64 {
    fn pack(&self, out: &mut Writer) {
        out.float(*self);
    }
}

impl Unpack for f64 {
    const SIZE: u64 = 8;

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
    const SIZE: u64 = 4;

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
    const SIZE: u64 = 24;

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
    const SIZE: u64 = 24;

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
    const SIZE: u64 = 8 + T::SIZE;

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
            // 止めたあとは、残りの項目をたどらない
            if out.stop.is_some() {
                break;
            }
            item.pack(out);
        }
    }
}

/// 実際のメモリ配置が `isize::MAX` を超える場合も、パニックせずに失敗する
fn vec_with_capacity<T>(count: usize) -> Result<Vec<T>, std::collections::TryReserveError> {
    let mut items = Vec::new();
    items.try_reserve_exact(count)?;
    Ok(items)
}

impl<T: Unpack> Unpack for Vec<T> {
    const SIZE: u64 = 24;

    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        let count = input.count(T::SIZE)?;
        let mut items = vec_with_capacity(count).map_err(|_| input.invalid("count"))?;
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
    const SIZE: u64 = 8;

    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.decoded(T::SIZE)?;
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
    const SIZE: u64 = N as u64 * T::SIZE;

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
    const SIZE: u64 = A::SIZE + B::SIZE;

    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok((A::unpack(input)?, B::unpack(input)?))
    }
}

// 出所はフィールドを外に見せないので、手で書く。フィールドは契約の Ref と同じ順にする（xtask の HANDWRITTEN が確かめる）
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
    const SIZE: u64 = Option::<String>::SIZE
        + Option::<u64>::SIZE
        + Option::<ByteRange>::SIZE
        + Option::<Key>::SIZE;

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

/// テストのための乱数の図面と、元の文書の情報。契約にあるどの型の値も作るので、読み書きと、図面を受け取る処理の
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
