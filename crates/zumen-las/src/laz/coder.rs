//! LAZ（OGC 24-070r1 第 7 章）の算術復号器・適応確率モデル・整数復号器。
//!
//! 壊れた入力や細工されたストリームでもパニック・スライス範囲外アクセス・算術あふれを起こさず、
//! 不変条件（`value < length` など）が崩れたら `Err(LazErr)` を返す。

/// LAZ 展開中の内部エラー。呼び出し元で `las.laz.invalid` に写す。
#[derive(Clone, Copy, Debug)]
pub(super) struct LazErr;

pub(super) type LazResult<T> = Result<T, LazErr>;

const AC_MIN_LENGTH: u32 = 0x0100_0000;
const AC_MAX_LENGTH: u32 = 0xFFFF_FFFF;
const BM_LENGTH_SHIFT: u32 = 13;
const BM_MAX_COUNT: u32 = 1 << BM_LENGTH_SHIFT;
const DM_LENGTH_SHIFT: u32 = 15;
const DM_MAX_COUNT: u32 = 1 << DM_LENGTH_SHIFT;

/// スライスを先頭からゼロコピーで読み進めるカーソル。
#[derive(Clone, Debug)]
pub(super) struct SliceCursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> SliceCursor<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    pub(super) fn take(&mut self, len: usize) -> LazResult<&'a [u8]> {
        let end = self.pos.checked_add(len).ok_or(LazErr)?;
        let slice = self.bytes.get(self.pos..end).ok_or(LazErr)?;
        self.pos = end;
        Ok(slice)
    }

    pub(super) fn read_into(&mut self, dst: &mut [u8]) -> LazResult<()> {
        let src = self.take(dst.len())?;
        dst.copy_from_slice(src);
        Ok(())
    }

    pub(super) fn u8(&mut self) -> LazResult<u8> {
        let b = *self.take(1)?.first().ok_or(LazErr)?;
        Ok(b)
    }

    pub(super) fn u16_le(&mut self) -> LazResult<u16> {
        let arr: [u8; 2] = self.take(2)?.try_into().map_err(|_| LazErr)?;
        Ok(u16::from_le_bytes(arr))
    }

    pub(super) fn u32_le(&mut self) -> LazResult<u32> {
        let arr: [u8; 4] = self.take(4)?.try_into().map_err(|_| LazErr)?;
        Ok(u32::from_le_bytes(arr))
    }

    pub(super) fn u32_be(&mut self) -> LazResult<u32> {
        let arr: [u8; 4] = self.take(4)?.try_into().map_err(|_| LazErr)?;
        Ok(u32::from_be_bytes(arr))
    }

    pub(super) fn i64_le(&mut self) -> LazResult<i64> {
        let arr: [u8; 8] = self.take(8)?.try_into().map_err(|_| LazErr)?;
        Ok(i64::from_le_bytes(arr))
    }
}

/// 2 値の適応算術確率モデル。
#[derive(Clone, Copy, Debug)]
pub(super) struct BitModel {
    bit_0_count: u32,
    bit_count: u32,
    bit_0_prob: u32,
    bits_until_update: u32,
    update_cycle: u32,
}

impl Default for BitModel {
    fn default() -> Self {
        Self {
            bit_0_count: 1,
            bit_count: 2,
            bit_0_prob: 1 << (BM_LENGTH_SHIFT - 1),
            bits_until_update: 4,
            update_cycle: 4,
        }
    }
}

impl BitModel {
    fn update(&mut self) {
        self.bit_count = self.bit_count.saturating_add(self.update_cycle);
        if self.bit_count > BM_MAX_COUNT {
            self.bit_count = (self.bit_count + 1) >> 1;
            self.bit_0_count = (self.bit_0_count + 1) >> 1;
            if self.bit_0_count == self.bit_count {
                self.bit_count += 1;
            }
        }
        let denom = self.bit_count.max(1);
        let scale = 0x8000_0000u32 / denom;
        self.bit_0_prob = (self.bit_0_count.wrapping_mul(scale)) >> (31 - BM_LENGTH_SHIFT);
        self.update_cycle = ((5 * self.update_cycle) >> 2).min(64);
        self.bits_until_update = self.update_cycle;
    }
}

/// 多値の適応算術確率モデル。
#[derive(Clone, Debug)]
pub(super) struct SymbolModel {
    distribution: Vec<u32>,
    symbol_count: Vec<u32>,
    decoder_table: Vec<u32>,
    total_count: u32,
    update_cycle: u32,
    symbols_until_update: u32,
    symbols: u32,
    table_size: u32,
    table_shift: u32,
}

impl SymbolModel {
    pub(super) fn new(symbols: u32) -> Self {
        let (table_size, table_shift, decoder_table) = if symbols > 64 {
            let mut bits = 3u32;
            while symbols > (1u32 << (bits + 2)) && bits < DM_LENGTH_SHIFT {
                bits += 1;
            }
            let size = 1u32 << bits;
            (
                size,
                DM_LENGTH_SHIFT - bits,
                vec![0u32; (size + 2) as usize],
            )
        } else {
            (0, 0, Vec::new())
        };
        let n = symbols as usize;
        let mut model = Self {
            distribution: vec![0u32; n],
            symbol_count: vec![1u32; n],
            decoder_table,
            total_count: 0,
            update_cycle: symbols,
            symbols_until_update: symbols,
            symbols,
            table_size,
            table_shift,
        };
        model.update();
        let initial_cycle = (symbols + 6) >> 1;
        model.symbols_until_update = initial_cycle;
        model.update_cycle = initial_cycle;
        model
    }

    fn update(&mut self) {
        self.total_count = self.total_count.saturating_add(self.update_cycle);
        if self.total_count > DM_MAX_COUNT {
            self.total_count = 0;
            for count in &mut self.symbol_count {
                *count = (*count + 1) >> 1;
                self.total_count = self.total_count.saturating_add(*count);
            }
        }
        let denom = self.total_count.max(1);
        let scale = 0x8000_0000u32 / denom;
        let mut sum = 0u32;

        if self.table_size == 0 {
            for (dist, count) in self.distribution.iter_mut().zip(&self.symbol_count) {
                *dist = (scale.wrapping_mul(sum)) >> (31 - DM_LENGTH_SHIFT);
                sum = sum.wrapping_add(*count);
            }
        } else {
            let mut s = 0usize;
            for (k, (dist, count)) in self
                .distribution
                .iter_mut()
                .zip(&self.symbol_count)
                .enumerate()
            {
                *dist = (scale.wrapping_mul(sum)) >> (31 - DM_LENGTH_SHIFT);
                sum = sum.wrapping_add(*count);
                let w = (*dist >> self.table_shift) as usize;
                while s < w {
                    s += 1;
                    if let Some(slot) = self.decoder_table.get_mut(s) {
                        *slot = k.saturating_sub(1) as u32;
                    }
                }
            }
            if let Some(first) = self.decoder_table.first_mut() {
                *first = 0;
            }
            let max_s = self.table_size as usize;
            while s <= max_s {
                s += 1;
                if let Some(slot) = self.decoder_table.get_mut(s) {
                    *slot = self.symbols.saturating_sub(1);
                }
            }
        }

        let max_cycle = (self.symbols + 6) << 3;
        self.update_cycle = ((5 * self.update_cycle) >> 2).min(max_cycle);
        self.symbols_until_update = self.update_cycle;
    }
}

/// 算術復号器（OGC 24-070r1 7.1）。
#[derive(Clone, Debug)]
pub(super) struct RangeDecoder<'a> {
    cursor: SliceCursor<'a>,
    value: u32,
    length: u32,
}

impl<'a> RangeDecoder<'a> {
    pub(super) fn from_cursor(mut cursor: SliceCursor<'a>) -> LazResult<Self> {
        let value = cursor.u32_be()?;
        // `value == 0xFFFF_FFFF` のときは `value >= length` となり、除算でテーブル範囲を超える（#510）。
        if value == AC_MAX_LENGTH {
            return Err(LazErr);
        }
        Ok(Self {
            cursor,
            value,
            length: AC_MAX_LENGTH,
        })
    }

    pub(super) fn from_slice(bytes: &'a [u8]) -> LazResult<Self> {
        Self::from_cursor(SliceCursor::new(bytes))
    }

    pub(super) fn into_cursor(self) -> SliceCursor<'a> {
        self.cursor
    }

    fn renorm(&mut self) -> LazResult<()> {
        while self.length < AC_MIN_LENGTH {
            let b = u32::from(self.cursor.u8()?);
            self.value = (self.value << 8) | b;
            self.length <<= 8;
        }
        if self.value >= self.length {
            return Err(LazErr);
        }
        Ok(())
    }

    pub(super) fn decode_bit(&mut self, model: &mut BitModel) -> LazResult<u32> {
        let x = model
            .bit_0_prob
            .checked_mul(self.length >> BM_LENGTH_SHIFT)
            .ok_or(LazErr)?;
        let sym = if self.value >= x {
            self.value -= x;
            self.length = self.length.checked_sub(x).ok_or(LazErr)?;
            if self.length == 0 {
                return Err(LazErr);
            }
            1
        } else {
            self.length = x;
            if self.length == 0 {
                return Err(LazErr);
            }
            model.bit_0_count += 1;
            0
        };
        if self.length < AC_MIN_LENGTH {
            self.renorm()?;
        } else if self.value >= self.length {
            return Err(LazErr);
        }
        model.bits_until_update = model.bits_until_update.saturating_sub(1);
        if model.bits_until_update == 0 {
            model.update();
        }
        Ok(sym)
    }

    pub(super) fn decode_symbol(&mut self, model: &mut SymbolModel) -> LazResult<u32> {
        let mut sym: u32;
        let mut x: u32;
        let mut y = self.length;

        if model.table_size > 0 {
            self.length >>= DM_LENGTH_SHIFT;
            if self.length == 0 {
                return Err(LazErr);
            }
            let dv = self.value / self.length;
            let t = (dv >> model.table_shift) as usize;
            sym = *model.decoder_table.get(t).ok_or(LazErr)?;
            let mut n = model
                .decoder_table
                .get(t + 1)
                .copied()
                .ok_or(LazErr)?
                .saturating_add(1);
            while n > sym + 1 {
                let m = (sym + n) >> 1;
                let dist_m = *model.distribution.get(m as usize).ok_or(LazErr)?;
                if dist_m > dv {
                    n = m;
                } else {
                    sym = m;
                }
            }
            let dist_sym = *model.distribution.get(sym as usize).ok_or(LazErr)?;
            x = dist_sym.checked_mul(self.length).ok_or(LazErr)?;
            if sym + 1 < model.symbols {
                let dist_next = *model.distribution.get((sym + 1) as usize).ok_or(LazErr)?;
                y = dist_next.checked_mul(self.length).ok_or(LazErr)?;
            }
        } else {
            sym = 0;
            x = 0;
            self.length >>= DM_LENGTH_SHIFT;
            if self.length == 0 {
                return Err(LazErr);
            }
            let mut n = model.symbols;
            let mut m = n >> 1;
            while m != sym {
                let dist_m = *model.distribution.get(m as usize).ok_or(LazErr)?;
                let z = self.length.checked_mul(dist_m).ok_or(LazErr)?;
                if self.value >= z {
                    sym = m;
                    x = z;
                } else {
                    n = m;
                    y = z;
                }
                m = (sym + n) >> 1;
            }
        }

        self.value = self.value.checked_sub(x).ok_or(LazErr)?;
        self.length = y.checked_sub(x).ok_or(LazErr)?;
        if self.length == 0 {
            return Err(LazErr);
        }
        if self.length < AC_MIN_LENGTH {
            self.renorm()?;
        } else if self.value >= self.length {
            return Err(LazErr);
        }

        let slot = model.symbol_count.get_mut(sym as usize).ok_or(LazErr)?;
        *slot = slot.saturating_add(1);
        model.symbols_until_update = model.symbols_until_update.saturating_sub(1);
        if model.symbols_until_update == 0 {
            model.update();
        }
        Ok(sym)
    }

    pub(super) fn read_bits(&mut self, mut bits: u32) -> LazResult<u32> {
        if bits == 0 {
            return Ok(0);
        }
        if bits > 32 {
            return Err(LazErr);
        }
        if bits > 19 {
            let low = u32::from(self.read_u16()?);
            bits -= 16;
            let high = self.read_bits(bits)?;
            return Ok((high << 16) | low);
        }
        self.length >>= bits;
        if self.length == 0 {
            return Err(LazErr);
        }
        let sym = self.value / self.length;
        if bits < 32 && sym >= (1u32 << bits) {
            return Err(LazErr);
        }
        self.value = self
            .value
            .checked_sub(self.length.checked_mul(sym).ok_or(LazErr)?)
            .ok_or(LazErr)?;
        if self.length < AC_MIN_LENGTH {
            self.renorm()?;
        }
        Ok(sym)
    }

    pub(super) fn read_u16(&mut self) -> LazResult<u16> {
        self.length >>= 16;
        if self.length == 0 {
            return Err(LazErr);
        }
        let sym = self.value / self.length;
        let out = u16::try_from(sym).map_err(|_| LazErr)?;
        self.value = self
            .value
            .checked_sub(self.length.checked_mul(sym).ok_or(LazErr)?)
            .ok_or(LazErr)?;
        if self.length < AC_MIN_LENGTH {
            self.renorm()?;
        }
        Ok(out)
    }

    pub(super) fn read_u32(&mut self) -> LazResult<u32> {
        let low = u32::from(self.read_u16()?);
        let high = u32::from(self.read_u16()?);
        Ok((high << 16) | low)
    }

    pub(super) fn read_u64(&mut self) -> LazResult<u64> {
        let low = u64::from(self.read_u32()?);
        let high = u64::from(self.read_u32()?);
        Ok((high << 32) | low)
    }
}

/// 適応 `k` ビット整数復号器（OGC 24-070r1 7.2）。
#[derive(Clone, Debug)]
pub(super) struct IntDecoder {
    k: u32,
    bits_high: u32,
    corr_range: u32,
    corr_min: i32,
    m_bits: Vec<SymbolModel>,
    m_corrector0: BitModel,
    m_corrector: Vec<SymbolModel>,
}

impl IntDecoder {
    pub(super) fn new(bits: u32, contexts: u32) -> Self {
        let bits_high = 8u32;
        let (corr_bits, corr_range, corr_min) = if bits != 0 && bits < 32 {
            let r = 1u32 << bits;
            (bits, r, -((r as i32) / 2))
        } else {
            (32, 0, i32::MIN)
        };
        let m_bits = (0..contexts)
            .map(|_| SymbolModel::new(corr_bits + 1))
            .collect();
        let m_corrector = (1..=corr_bits)
            .map(|i| {
                let syms = if i <= bits_high {
                    1u32 << i
                } else {
                    1u32 << bits_high
                };
                SymbolModel::new(syms)
            })
            .collect();
        Self {
            k: 0,
            bits_high,
            corr_range,
            corr_min,
            m_bits,
            m_corrector0: BitModel::default(),
            m_corrector,
        }
    }

    pub(super) fn k(&self) -> u32 {
        self.k
    }

    pub(super) fn decode(
        &mut self,
        dec: &mut RangeDecoder<'_>,
        pred: i32,
        context: u32,
    ) -> LazResult<i32> {
        let m_bit = self.m_bits.get_mut(context as usize).ok_or(LazErr)?;
        self.k = dec.decode_symbol(m_bit)?;
        let corr = if self.k == 0 {
            dec.decode_bit(&mut self.m_corrector0)? as i32
        } else if self.k < 32 {
            let model = self
                .m_corrector
                .get_mut((self.k - 1) as usize)
                .ok_or(LazErr)?;
            let mut c = if self.k <= self.bits_high {
                dec.decode_symbol(model)? as i32
            } else {
                let k1 = self.k - self.bits_high;
                let high = dec.decode_symbol(model)?;
                let low = dec.read_bits(k1)?;
                ((high << k1) | low) as i32
            };
            let half = (1u32 << (self.k - 1)) as i32;
            if c >= half {
                c = c.wrapping_add(1);
            } else {
                let span = ((1u32 << self.k) - 1) as i32;
                c = c.wrapping_sub(span);
            }
            c
        } else {
            self.corr_min
        };

        let mut real = pred.wrapping_add(corr);
        if self.corr_range != 0 {
            let range_i32 = self.corr_range as i32;
            if real < 0 {
                real = real.wrapping_add(range_i32);
            } else if real >= range_i32 {
                real = real.wrapping_sub(range_i32);
            }
        }
        Ok(real)
    }
}

/// 5 要素の移動中央値（OGC 24-070r1 付録）。
#[derive(Clone, Copy, Debug)]
pub(super) struct Median5 {
    values: [i32; 5],
    high: bool,
}

impl Default for Median5 {
    fn default() -> Self {
        Self {
            values: [0; 5],
            high: true,
        }
    }
}

impl Median5 {
    pub(super) fn get(&self) -> i32 {
        let [_, _, mid, _, _] = self.values;
        mid
    }

    pub(super) fn add(&mut self, v: i32) {
        let [mut a, mut b, mut c, mut d, mut e] = self.values;
        if self.high {
            if v < c {
                e = d;
                d = c;
                if v < a {
                    c = b;
                    b = a;
                    a = v;
                } else if v < b {
                    c = b;
                    b = v;
                } else {
                    c = v;
                }
            } else {
                if v < d {
                    e = d;
                    d = v;
                } else {
                    e = v;
                }
                self.high = false;
            }
        } else if c < v {
            a = b;
            b = c;
            if e < v {
                c = d;
                d = e;
                e = v;
            } else if d < v {
                c = d;
                d = v;
            } else {
                c = v;
            }
        } else {
            if b < v {
                a = b;
                b = v;
            } else {
                a = v;
            }
            self.high = true;
        }
        self.values = [a, b, c, d, e];
    }
}

/// `[0, 255]` に丸める。
pub(super) fn clamp_u8(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}
