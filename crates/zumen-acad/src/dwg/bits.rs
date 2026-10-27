//! DWG のビット単位の値と、CRC（ODS 5.4.1 の 2「Bit codes and data definitions」）。
//!
//! バイトの中のビットは、上位から読む。複数のバイトにわたる生の値は、下位のバイトから並ぶ。
//! どの読み方も、データが尽きたら `None` を返す。

use core::ops::Range;

use super::Release;

/// 色
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Color {
    /// 色の番号。256 は ByLayer、0 は ByBlock
    pub(super) index: i16,
    /// 真の色（下位の 3 バイトが RGB）。DXF の 420 に置く
    pub(super) rgb: Option<u32>,
    /// 図形の色を DBCOLOR のハンドルで指すか。そのときは RGB の値がデータの側にない
    pub(super) book: bool,
}

impl Color {
    /// R2004 の RGB の値を読む。最上位のバイトが色の種類で、0xC0 は ByLayer、0xC1 は ByBlock、
    /// 0xC2 は真の色、0xC3 は下位のバイトが色の番号（試料の DWG と DXF の比較で確かめた）。
    /// それ以外は `index` のまま
    fn r2004(index: i16, value: u32) -> Self {
        let (index, rgb) = match value >> 24 {
            0xC0 => (256, None),
            0xC1 => (0, None),
            0xC2 => (index, Some(value & 0x00FF_FFFF)),
            0xC3 => (i16::from(value as u8), None),
            _ => (index, None),
        };
        Color {
            index,
            rgb,
            book: false,
        }
    }
}

/// ビット列を先頭から読む
pub(super) struct Bits<'a> {
    src: &'a [u8],
    /// 次に読むビットの位置
    pos: usize,
    /// 色の欄の形を決める版
    release: Release,
    /// R2007 からの、文字列の流れの次に読むビットの位置。`None` なら文字列の流れがない
    strings: Option<usize>,
    /// 見出しの変数の、ハンドルの流れの次に読むビットの位置。`None` ならハンドルを `pos` から読む
    handles: Option<usize>,
}

impl<'a> Bits<'a> {
    pub(super) fn new(src: &'a [u8]) -> Self {
        Bits {
            src,
            pos: 0,
            release: Release::R2000,
            strings: None,
            handles: None,
        }
    }

    /// 版 `release` の欄の形で読む
    pub(super) fn with_release(src: &'a [u8], release: Release) -> Self {
        Bits {
            src,
            pos: 0,
            release,
            strings: None,
            handles: None,
        }
    }

    /// 読む版
    pub(super) fn release(&self) -> Release {
        self.release
    }

    /// CMC（2.11「CmColor value」）。色を返す。R2004 からは、番号（BS）の後に RGB の値（BL）と
    /// 印（RC）が続き、印の 1 は色の名前（TV）、2 は色見本帳の名前（TV）が続くことを表す
    pub(super) fn cmc(&mut self) -> Option<Color> {
        let mut color = Color {
            index: self.bs()?,
            rgb: None,
            book: false,
        };
        if self.release >= Release::R2004 {
            color = Color::r2004(color.index, self.bl()? as u32);
            let flags = self.rc()?;
            if flags & 1 != 0 {
                self.text()?;
            }
            if flags & 2 != 0 {
                self.text()?;
            }
        }
        Some(color)
    }

    /// 図形の色（2.12「Entity color encoding」）。色を返す。R2004 からは BS の下位のビットが
    /// 番号で、0x8000 は RGB の値（BL）、0x2000 は透明度（BL）が続くことを表し、0x4000 は色を
    /// DBCOLOR のハンドルで指すことを表す
    pub(super) fn enc(&mut self) -> Option<Color> {
        let value = self.bs()?;
        let index = value & 0x0FFF;
        if self.release == Release::R2000 {
            return Some(Color {
                index: value,
                rgb: None,
                book: false,
            });
        }
        let flags = value as u16;
        let mut color = Color {
            index,
            rgb: None,
            book: flags & 0x4000 != 0,
        };
        // 0x4000 が立つと、0x8000 が立っていても RGB の値はなく、拡張辞書と画層の間に DBCOLOR を指す
        // ハンドルが来る（試料の DWG で確かめた）
        if flags & 0xC000 == 0x8000 {
            color = Color::r2004(index, self.bl()? as u32);
        }
        if flags & 0x2000 != 0 {
            self.bl()?;
        }
        Some(color)
    }

    /// 次に読むビットの位置
    pub(super) fn pos(&self) -> usize {
        self.pos
    }

    /// 読む位置を `pos` ビット目に移す
    pub(super) fn seek(&mut self, pos: usize) {
        self.pos = pos;
    }

    /// R2007 からの文字列の流れを、ビット `end` の前に置く。`end` の 1 つ前のビットが流れの有無を
    /// 表し、その前の RS（0x8000 が立てば、さらに前の RS を上位とする）が流れのビットの数を表す。
    /// 主のデータが終わるビット（流れがなければ印の位置）を返す。読む位置は変えない
    pub(super) fn split_strings(&mut self, end: usize) -> Option<usize> {
        let pos = self.pos;
        let start = self.find_strings(end);
        self.pos = pos;
        start
    }

    fn find_strings(&mut self, end: usize) -> Option<usize> {
        let mut at = end.checked_sub(1)?;
        self.pos = at;
        if !self.bit()? {
            self.strings = None;
            return Some(at);
        }
        at = at.checked_sub(16)?;
        self.pos = at;
        let low = self.rs()? as u16;
        let mut size = usize::from(low);
        if low & 0x8000 != 0 {
            at = at.checked_sub(16)?;
            self.pos = at;
            let high = self.rs()? as u16;
            size = usize::from(low & 0x7FFF) | (usize::from(high) << 15);
        }
        let start = at.checked_sub(size)?;
        self.strings = Some(start);
        Some(start)
    }

    /// ハンドルを、ビット `at` から始まるハンドルの流れから読む（見出しの変数、R2007 から）
    pub(super) fn split_handles(&mut self, at: usize) {
        self.handles = Some(at);
    }

    /// B
    pub(super) fn bit(&mut self) -> Option<bool> {
        let byte = self.src.get(self.pos / 8)?;
        let bit = (byte >> (7 - self.pos % 8)) & 1;
        self.pos += 1;
        Some(bit == 1)
    }

    /// 上位から `n` ビット（8 以下）
    fn small(&mut self, n: u8) -> Option<u8> {
        let mut value = 0;
        for _ in 0..n {
            value = (value << 1) | u8::from(self.bit()?);
        }
        Some(value)
    }

    /// BOT（R2010 から）。上位の 2 ビットが 0 なら RC、1 なら RC と 0x1F0 の和、それ以外は RS
    pub(super) fn bot(&mut self) -> Option<i16> {
        match self.bb()? {
            0 => self.rc().map(i16::from),
            1 => self.rc().map(|code| i16::from(code) + 0x1F0),
            _ => self.rs(),
        }
    }

    /// BLL（R2010 から）。3 ビットのバイトの数と、下位から並ぶバイト
    pub(super) fn bll(&mut self) -> Option<u64> {
        let len = self.small(3)?;
        let mut value = 0u64;
        for shift in 0..len {
            value |= u64::from(self.rc()?) << (8 * shift);
        }
        Some(value)
    }

    /// BB
    pub(super) fn bb(&mut self) -> Option<u8> {
        self.small(2)
    }

    /// RC
    pub(super) fn rc(&mut self) -> Option<u8> {
        self.small(8)
    }

    fn raw<const N: usize>(&mut self) -> Option<[u8; N]> {
        let mut out = [0; N];
        for byte in &mut out {
            *byte = self.rc()?;
        }
        Some(out)
    }

    /// RS
    pub(super) fn rs(&mut self) -> Option<i16> {
        self.raw().map(i16::from_le_bytes)
    }

    /// RL
    pub(super) fn rl(&mut self) -> Option<i32> {
        self.raw().map(i32::from_le_bytes)
    }

    /// RD
    pub(super) fn rd(&mut self) -> Option<f64> {
        self.raw().map(f64::from_le_bytes)
    }

    /// BS
    pub(super) fn bs(&mut self) -> Option<i16> {
        match self.bb()? {
            0 => self.rs(),
            1 => self.rc().map(i16::from),
            2 => Some(0),
            _ => Some(256),
        }
    }

    /// BL
    pub(super) fn bl(&mut self) -> Option<i32> {
        match self.bb()? {
            0 => self.rl(),
            1 => self.rc().map(i32::from),
            2 => Some(0),
            _ => None,
        }
    }

    /// BD
    pub(super) fn bd(&mut self) -> Option<f64> {
        match self.bb()? {
            0 => self.rd(),
            1 => Some(1.0),
            2 => Some(0.0),
            _ => None,
        }
    }

    /// DD。`default` の一部のバイトを、続くバイトで置き換える
    pub(super) fn dd(&mut self, default: f64) -> Option<f64> {
        let [_, _, _, _, d4, d5, d6, d7] = default.to_le_bytes();
        let bytes = match self.bb()? {
            0 => return Some(default),
            1 => {
                let [l0, l1, l2, l3] = self.raw()?;
                [l0, l1, l2, l3, d4, d5, d6, d7]
            }
            2 => {
                let [m4, m5] = self.raw()?;
                let [l0, l1, l2, l3] = self.raw()?;
                [l0, l1, l2, l3, m4, m5, d6, d7]
            }
            _ => return self.rd(),
        };
        Some(f64::from_le_bytes(bytes))
    }

    /// H。ハンドルの流れが分かれていれば、そこから読む
    pub(super) fn handle(&mut self) -> Option<HandleRef> {
        let Some(at) = self.handles else {
            return self.main_handle();
        };
        let pos = core::mem::replace(&mut self.pos, at);
        let handle = self.main_handle();
        self.handles = Some(self.pos);
        self.pos = pos;
        handle
    }

    /// 主のデータの H。符号（4 ビット）と、上位のバイトから並ぶ値
    pub(super) fn main_handle(&mut self) -> Option<HandleRef> {
        let code = self.small(4)?;
        let counter = self.small(4)?;
        if counter > 8 {
            return None;
        }
        let mut value = 0u64;
        for _ in 0..counter {
            value = (value << 8) | u64::from(self.rc()?);
        }
        Some(HandleRef { code, value })
    }

    /// T。R2007 からは、文字列の流れの TU（BS の文字の数と UTF-16LE）を UTF-8 に直して返す。
    /// 流れがなければ空
    pub(super) fn text(&mut self) -> Option<Vec<u8>> {
        if self.release < Release::R2010 {
            return self.code_page_text();
        }
        let Some(at) = self.strings else {
            return Some(Vec::new());
        };
        let pos = core::mem::replace(&mut self.pos, at);
        let text = self.unicode_text();
        self.strings = Some(self.pos);
        self.pos = pos;
        text
    }

    fn unicode_text(&mut self) -> Option<Vec<u8>> {
        let len = usize::try_from(self.bs()?).ok()?;
        let mut units: Vec<u16> = (0..len)
            .map(|_| self.rs().map(|unit| unit as u16))
            .collect::<Option<_>>()?;
        if units.last() == Some(&0) {
            units.pop();
        }
        Some(String::from_utf16_lossy(&units).into_bytes())
    }

    /// T（R2004 まで）。BS の長さと、コード・ページのバイト。長さは終わりの NUL を含むので、
    /// 末尾の NUL を 1 つ除く
    fn code_page_text(&mut self) -> Option<Vec<u8>> {
        let len = usize::try_from(self.bs()?).ok()?;
        let mut text: Vec<u8> = (0..len).map(|_| self.rc()).collect::<Option<_>>()?;
        if text.last() == Some(&0) {
            text.pop();
        }
        Some(text)
    }

    /// `n` バイトを読み飛ばす
    pub(super) fn skip_bytes(&mut self, n: usize) -> Option<()> {
        let end = self.pos.checked_add(n.checked_mul(8)?)?;
        if end > self.src.len().checked_mul(8)? {
            return None;
        }
        self.pos = end;
        Some(())
    }
}

/// 参照のハンドル（ODS 5.4.1 の 2.13）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HandleRef {
    pub(super) code: u8,
    pub(super) value: u64,
}

impl HandleRef {
    /// 指すハンドル。符号 6・8・A・C は、`base` からの相対
    pub(super) fn resolve(self, base: u64) -> Option<u64> {
        match self.code {
            0x6 => base.checked_add(1),
            0x8 => base.checked_sub(1),
            0xA => base.checked_add(self.value),
            0xC => base.checked_sub(self.value),
            _ => Some(self.value),
        }
    }
}

/// 剰余の文字（MC）。`signed` なら、最後のバイトの 0x40 が負の印
pub(super) fn modular_char(src: &[u8], pos: &mut usize, signed: bool) -> Option<i64> {
    let mut value: i64 = 0;
    let mut shift = 0u32;
    loop {
        let byte = *src.get(*pos)?;
        *pos += 1;
        if byte & 0x80 != 0 {
            value |= i64::from(byte & 0x7F).checked_shl(shift)?;
            shift += 7;
            if shift > 56 {
                return None;
            }
            continue;
        }
        if signed && byte & 0x40 != 0 {
            value |= i64::from(byte & 0x3F).checked_shl(shift)?;
            return Some(-value);
        }
        value |= i64::from(byte).checked_shl(shift)?;
        return Some(value);
    }
}

/// 剰余の short（MS）。オブジェクトの大きさに使う
pub(super) fn modular_short(src: &[u8], pos: &mut usize) -> Option<u64> {
    let mut value: u64 = 0;
    let mut shift = 0u32;
    loop {
        let word = u16::from_le_bytes([*src.get(*pos)?, *src.get(*pos + 1)?]);
        *pos += 2;
        value |= u64::from(word & 0x7FFF).checked_shl(shift)?;
        if word & 0x8000 == 0 {
            return Some(value);
        }
        shift += 15;
        if shift > 45 {
            return None;
        }
    }
}

/// 剰余の文字（MC）を書く。[`modular_char`] の逆
pub(super) fn push_modular_char(out: &mut Vec<u8>, value: i64, signed: bool) {
    let sign = if signed && value < 0 { 0x40 } else { 0 };
    let last = if signed { 0x40 } else { 0x80 };
    let mut rest = value.unsigned_abs();
    while rest >= last {
        out.push((rest & 0x7F) as u8 | 0x80);
        rest >>= 7;
    }
    out.push(rest as u8 | sign);
}

/// 剰余の short（MS）を書く。[`modular_short`] の逆
pub(super) fn push_modular_short(out: &mut Vec<u8>, value: u64) {
    let mut rest = value;
    while rest > 0x7FFF {
        out.extend_from_slice(&((rest & 0x7FFF) as u16 | 0x8000).to_le_bytes());
        rest >>= 15;
    }
    out.extend_from_slice(&(rest as u16).to_le_bytes());
}

/// ビットの並びを、上位のビットから書く。[`Bits`] の逆
#[derive(Debug, Default)]
pub(super) struct BitWriter {
    out: Vec<u8>,
    /// 書いたビットの数
    len: usize,
    /// R2007 からの文字列の流れ。あれば、T をここに TU で書く
    strings: Option<Box<BitWriter>>,
}

impl BitWriter {
    /// T を文字列の流れに TU で書く（R2007 から）
    pub(super) fn with_strings() -> Self {
        Self {
            strings: Some(Box::default()),
            ..Self::default()
        }
    }

    /// `src` の `bits` のビットを、文字列の流れに写す。流れがないか、`src` の外を指せば `None`
    pub(super) fn copy_strings(&mut self, src: &[u8], bits: Range<usize>) -> Option<()> {
        self.strings.as_mut()?.copy(src, bits)
    }

    /// 文字列の流れを取り出す。なければ空
    pub(super) fn take_strings(&mut self) -> BitWriter {
        self.strings
            .take()
            .map(|strings| *strings)
            .unwrap_or_default()
    }

    /// 書いたビットの数
    pub(super) fn len(&self) -> usize {
        self.len
    }

    /// 書いたバイト列。最後のバイトの残りのビットは 0
    pub(super) fn into_bytes(self) -> Vec<u8> {
        self.out
    }

    pub(super) fn bit(&mut self, bit: bool) {
        let shift = self.len % 8;
        if shift == 0 {
            self.out.push(0);
        }
        if bit && let Some(last) = self.out.last_mut() {
            *last |= 0x80 >> shift;
        }
        self.len += 1;
    }

    /// `value` の下位の `n` ビットを、上位から
    fn small(&mut self, value: u8, n: u8) {
        for shift in (0..n).rev() {
            self.bit((value >> shift) & 1 == 1);
        }
    }

    /// バイト列をそのまま
    pub(super) fn raw(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.small(byte, 8);
        }
    }

    /// `src` の `bits` のビットを写す。`src` の外を指せば `None`
    pub(super) fn copy(&mut self, src: &[u8], bits: Range<usize>) -> Option<()> {
        for pos in bits {
            let byte = src.get(pos / 8)?;
            self.bit((byte >> (7 - pos % 8)) & 1 == 1);
        }
        Some(())
    }

    /// H。符号（4 ビット）と、値のバイトの数（4 ビット）と、上位のバイトから並ぶ値
    pub(super) fn handle(&mut self, code: u8, value: u64) {
        let bytes = value.to_be_bytes();
        let skip = bytes.iter().take_while(|&&byte| byte == 0).count();
        let bytes = bytes.get(skip..).unwrap_or_default();
        self.small(code, 4);
        self.small(u8::try_from(bytes.len()).unwrap_or_default(), 4);
        self.raw(bytes);
    }

    /// BB
    pub(super) fn bb(&mut self, value: u8) {
        self.small(value, 2);
    }

    /// RC
    pub(super) fn rc(&mut self, value: u8) {
        self.small(value, 8);
    }

    /// RS
    pub(super) fn rs(&mut self, value: i16) {
        self.raw(&value.to_le_bytes());
    }

    /// RL
    pub(super) fn rl(&mut self, value: i32) {
        self.raw(&value.to_le_bytes());
    }

    /// BS。0 と 256 は 2 ビットで、0〜255 は RC で書く
    pub(super) fn bs(&mut self, value: i16) {
        match value {
            0 => self.small(2, 2),
            256 => self.small(3, 2),
            _ => match u8::try_from(value) {
                Ok(byte) => {
                    self.small(1, 2);
                    self.small(byte, 8);
                }
                Err(_) => {
                    self.small(0, 2);
                    self.raw(&value.to_le_bytes());
                }
            },
        }
    }

    /// BL。0 は 2 ビットで、0〜255 は RC で書く
    pub(super) fn bl(&mut self, value: i32) {
        match u8::try_from(value) {
            Ok(0) => self.bb(2),
            Ok(byte) => {
                self.bb(1);
                self.rc(byte);
            }
            Err(_) => {
                self.bb(0);
                self.rl(value);
            }
        }
    }

    /// RD
    pub(super) fn rd(&mut self, value: f64) {
        self.raw(&value.to_le_bytes());
    }

    /// BD。1 と 0 は 2 ビットで書く。-0 は RD で書く
    pub(super) fn bd(&mut self, value: f64) {
        if value.to_bits() == 1.0f64.to_bits() {
            self.small(1, 2);
        } else if value.to_bits() == 0 {
            self.small(2, 2);
        } else {
            self.small(0, 2);
            self.rd(value);
        }
    }

    /// T。R2000 は BS のバイトの数とバイト列。文字列の流れがあれば、UTF-8 の `text` を、そこに TU（BS の
    /// 文字の数と UTF-16LE）で書く。BS に収まらない長さなら `None`
    pub(super) fn text(&mut self, text: &[u8]) -> Option<()> {
        if let Some(strings) = &mut self.strings {
            // 流れの TU は、終わりの NUL を持たない（図面で確かめた）
            let text = text.strip_suffix(&[0]).unwrap_or(text);
            let units: Vec<u16> = core::str::from_utf8(text).ok()?.encode_utf16().collect();
            strings.bs(i16::try_from(units.len()).ok()?);
            for unit in units {
                strings.rs(unit as i16);
            }
            return Some(());
        }
        self.bs(i16::try_from(text.len()).ok()?);
        self.raw(text);
        Some(())
    }

    /// DD。`default` と同じ上位のバイトは書かない
    pub(super) fn dd(&mut self, value: f64, default: f64) {
        let [v0, v1, v2, v3, v4, v5, v6, v7] = value.to_le_bytes();
        let [_, _, _, _, d4, d5, d6, d7] = default.to_le_bytes();
        if value.to_bits() == default.to_bits() {
            self.small(0, 2);
        } else if [v4, v5, v6, v7] == [d4, d5, d6, d7] {
            self.small(1, 2);
            self.raw(&[v0, v1, v2, v3]);
        } else if [v6, v7] == [d6, d7] {
            self.small(2, 2);
            self.raw(&[v4, v5, v0, v1, v2, v3]);
        } else {
            self.small(3, 2);
            self.rd(value);
        }
    }
}

/// CRC の表の `index` 番目の値。多項式 0xA001 の、下位のビットから送る CRC
fn crc_entry(index: u8) -> u16 {
    (0..8).fold(u16::from(index), |crc, _| {
        if crc & 1 != 0 {
            (crc >> 1) ^ 0xA001
        } else {
            crc >> 1
        }
    })
}

/// 16 ビットの CRC（ODS 5.4.1 の 2.14.1）。`seed` から始めて、`bytes` を足す
pub(super) fn crc16(seed: u16, bytes: &[u8]) -> u16 {
    bytes.iter().fold(seed, |crc, &byte| {
        let [low, _] = crc.to_le_bytes();
        (crc >> 8) ^ crc_entry(byte ^ low)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(text: &str) -> Vec<u8> {
        let digits: Vec<u8> = text.bytes().filter(|b| *b != b' ').collect();
        digits
            .chunks(8)
            .map(|chunk| {
                chunk
                    .iter()
                    .enumerate()
                    .fold(0, |byte, (i, d)| byte | (u8::from(*d == b'1') << (7 - i)))
            })
            .collect()
    }

    #[test]
    fn bit_shorts_and_longs_use_their_two_bit_codes() {
        // 2.2・2.3 の例
        let src = bits("00 00000001 00000001 10 11 01 00001111 10");
        let mut b = Bits::new(&src);
        let found: Vec<_> = (0..5).map(|_| b.bs().unwrap()).collect();
        assert_eq!(found, [257, 0, 256, 15, 0]);
        let src = bits("00 00000001 00000001 00000000 00000000 10 01 00001111 10");
        let mut b = Bits::new(&src);
        let found: Vec<_> = (0..4).map(|_| b.bl().unwrap()).collect();
        assert_eq!(found, [257, 0, 15, 0]);
        assert_eq!(Bits::new(&bits("11")).bl(), None);
    }

    #[test]
    fn bit_doubles_and_defaults() {
        assert_eq!(Bits::new(&bits("01")).bd(), Some(1.0));
        assert_eq!(Bits::new(&bits("10")).bd(), Some(0.0));
        assert_eq!(Bits::new(&bits("11")).bd(), None);
        assert_eq!(Bits::new(&bits("00")).dd(2.5), Some(2.5));
        // 01 の後の 4 バイトが、既定の値の下位の 4 バイトを置き換える
        let default = f64::from_le_bytes([0, 0, 0, 0, 0, 0, 0xF0, 0x3F]);
        let src = bits("01 00000001 00000000 00000000 00000000");
        let found = Bits::new(&src).dd(default).unwrap();
        assert_eq!(found.to_le_bytes(), [1, 0, 0, 0, 0, 0, 0xF0, 0x3F]);
        // 10 の後の 2 バイトが 5・6 番目を、続く 4 バイトが下位の 4 バイトを置き換える
        let src = bits("10 00000010 00000011 00000001 00000000 00000000 00000000");
        let found = Bits::new(&src).dd(default).unwrap();
        assert_eq!(found.to_le_bytes(), [1, 0, 0, 0, 2, 3, 0xF0, 0x3F]);
    }

    #[test]
    fn handles_are_read_most_significant_byte_first() {
        // 2.13 の例: 5.2.05E7
        let src = bits("0101 0010 00000101 11100111");
        let handle = Bits::new(&src).handle().unwrap();
        assert_eq!(
            handle,
            HandleRef {
                code: 5,
                value: 0x5E7
            }
        );
        assert_eq!(
            HandleRef {
                code: 0x6,
                value: 0
            }
            .resolve(0x10),
            Some(0x11)
        );
        assert_eq!(
            HandleRef {
                code: 0xC,
                value: 2
            }
            .resolve(0x10),
            Some(0x0E)
        );
    }

    #[test]
    fn modular_values_follow_the_examples() {
        // 2.6・2.7 の例
        let mut pos = 0;
        assert_eq!(
            modular_char(&[0b1000_0010, 0b0010_0100], &mut pos, false),
            Some(4610)
        );
        assert_eq!(pos, 2);
        let src = [0b1110_1001, 0b1001_0111, 0b1110_0110, 0b0011_0101];
        assert_eq!(modular_char(&src, &mut 0, false), Some(112_823_273));
        assert_eq!(
            modular_char(&[0b1000_0101, 0b0100_1011], &mut 0, true),
            Some(-1413)
        );
        let src = [0b0011_0001, 0b1111_0100, 0b1000_1101, 0];
        assert_eq!(modular_short(&src, &mut 0), Some(4_650_033));
        assert_eq!(modular_char(&[0x80], &mut 0, false), None);
    }

    #[test]
    fn bit_values_are_written_back() {
        let mut out = BitWriter::default();
        out.bit(true);
        out.rl(-2);
        let shorts = [0, 1, 255, 256, -1, 0x7FFF];
        for value in shorts {
            out.bs(value);
        }
        let reals = [0.0, -0.0, 1.0, 2.5, 1.0e-300, f64::MAX];
        for value in reals {
            out.rd(value);
            out.bd(value);
            for default in [0.0, 2.5, 2.5000001, 1.0e-300] {
                out.dd(value, default);
            }
        }
        let mut copied = BitWriter::default();
        let len = out.len();
        let bytes = out.into_bytes();
        copied.copy(&bytes, 0..len);
        assert_eq!(copied.len(), len);
        let copied = copied.into_bytes();
        assert_eq!(copied, bytes);
        let mut bits = Bits::new(&copied);
        assert_eq!(bits.bit(), Some(true));
        assert_eq!(bits.rl(), Some(-2));
        for value in shorts {
            assert_eq!(bits.bs(), Some(value));
        }
        for value in reals {
            assert_eq!(bits.rd().map(f64::to_bits), Some(value.to_bits()));
            assert_eq!(bits.bd().map(f64::to_bits), Some(value.to_bits()));
            for default in [0.0, 2.5, 2.5000001, 1.0e-300] {
                assert_eq!(bits.dd(default).map(f64::to_bits), Some(value.to_bits()));
            }
        }
        assert_eq!(bits.pos(), len);
    }

    #[test]
    fn modular_values_are_written_back() {
        for value in [0, 1, 63, 64, 127, 128, 4610, 112_823_273, -1, -64, -1413] {
            for signed in [false, true] {
                if !signed && value < 0 {
                    continue;
                }
                let mut out = Vec::new();
                push_modular_char(&mut out, value, signed);
                let mut pos = 0;
                assert_eq!(modular_char(&out, &mut pos, signed), Some(value));
                assert_eq!(pos, out.len());
            }
        }
        let mut out = Vec::new();
        push_modular_char(&mut out, -1413, true);
        assert_eq!(out, [0b1000_0101, 0b0100_1011]);
        for value in [0, 0x7FFF, 0x8000, 4_650_033] {
            let mut out = Vec::new();
            push_modular_short(&mut out, value);
            assert_eq!(modular_short(&out, &mut 0), Some(value));
        }
    }

    #[test]
    fn the_crc_table_starts_like_the_spec() {
        let start: Vec<_> = (0..4).map(crc_entry).collect();
        assert_eq!(start, [0x0000, 0xC0C1, 0xC181, 0x0140]);
        assert_eq!(crc16(0, b""), 0);
    }
}
