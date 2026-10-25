//! DWG のビット単位の値と、CRC（ODS 5.4.1 の 2「Bit codes and data definitions」）。
//!
//! バイトの中のビットは、上位から読む。複数のバイトにわたる生の値は、下位のバイトから並ぶ。
//! どの読み方も、データが尽きたら `None` を返す。

/// ビット列を先頭から読む
pub(super) struct Bits<'a> {
    src: &'a [u8],
    /// 次に読むビットの位置
    pos: usize,
}

impl<'a> Bits<'a> {
    pub(super) fn new(src: &'a [u8]) -> Self {
        Bits { src, pos: 0 }
    }

    /// 次に読むビットの位置
    pub(super) fn pos(&self) -> usize {
        self.pos
    }

    /// 読む位置を `pos` ビット目に移す
    pub(super) fn seek(&mut self, pos: usize) {
        self.pos = pos;
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

    /// H。符号（4 ビット）と、上位のバイトから並ぶ値
    pub(super) fn handle(&mut self) -> Option<HandleRef> {
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

    /// T（R2000 まで）。BS の長さと、コード・ページのバイト。長さは終わりの NUL を含むので、
    /// 末尾の NUL を 1 つ除く
    pub(super) fn text(&mut self) -> Option<Vec<u8>> {
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
    fn the_crc_table_starts_like_the_spec() {
        let start: Vec<_> = (0..4).map(crc_entry).collect();
        assert_eq!(start, [0x0000, 0xC0C1, 0xC181, 0x0140]);
        assert_eq!(crc16(0, b""), 0);
    }
}
