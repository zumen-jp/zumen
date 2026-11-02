//! 試験のための乱数の値。バイナリを読む代わりに、乱数で値を取り出す

use super::Input;

/// 一覧の項目を作れる数。使い切ったら、一覧を空にする
const ITEMS: u64 = 400;

/// 乱数で値を作る所（splitmix64）
pub(crate) struct Random {
    state: u64,
    /// まだ作れる項目の数
    left: u64,
    /// 浮動小数を有限にするか
    finite: bool,
}

impl Random {
    pub(crate) fn new(seed: u64, finite: bool) -> Self {
        Self {
            state: seed,
            left: ITEMS,
            finite,
        }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// 0 から `n` の手前まで
    fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }
}

impl Input for Random {
    type Error = &'static str;

    fn byte(&mut self) -> Result<u8, Self::Error> {
        Ok(self.next() as u8)
    }

    fn uint(&mut self, max: u64) -> Result<u64, Self::Error> {
        // 端の値と小さな値を、よく出す
        Ok(match self.below(4) {
            0 => 0,
            1 => max,
            2 => self.below(16).min(max),
            _ => self.next() % max.saturating_add(1).max(1),
        })
    }

    fn int(&mut self, min: i64, max: i64) -> Result<i64, Self::Error> {
        Ok(match self.below(4) {
            0 => min,
            1 => max,
            2 => (self.below(16) as i64 - 8).clamp(min, max),
            _ => {
                let span = (i128::from(max) - i128::from(min) + 1) as u128;
                (i128::from(min) + (u128::from(self.next()) % span) as i128) as i64
            }
        })
    }

    fn float(&mut self) -> Result<f64, Self::Error> {
        let value = match self.below(8) {
            0 => 0.0,
            1 => -0.0,
            2 => 1.0,
            3 => -1.5,
            4 => (self.below(2001) as f64 - 1000.0) / 8.0,
            5 if !self.finite => match self.below(3) {
                0 => f64::NAN,
                1 => f64::INFINITY,
                _ => f64::NEG_INFINITY,
            },
            _ => f64::from_bits(self.next()),
        };
        Ok(if self.finite && !value.is_finite() {
            0.5
        } else {
            value
        })
    }

    fn flag(&mut self) -> Result<bool, Self::Error> {
        Ok(self.next() & 1 == 1)
    }

    fn char(&mut self) -> Result<char, Self::Error> {
        Ok(match self.below(3) {
            0 => 'a',
            1 => '図',
            _ => char::from_u32((self.next() % 0x11_0000) as u32).unwrap_or('\u{fffd}'),
        })
    }

    fn count(&mut self, _size: usize) -> Result<usize, Self::Error> {
        let count = self.below(4).min(self.left);
        self.left -= count;
        Ok(count as usize)
    }

    fn text(&mut self) -> Result<String, Self::Error> {
        let len = self.below(4);
        let mut text = String::new();
        for _ in 0..len {
            text.push(self.char()?);
        }
        Ok(text)
    }

    fn variant(&mut self, _ty: &'static str, count: u64) -> Result<u64, Self::Error> {
        Ok(self.below(count))
    }

    fn record<T>(
        &mut self,
        _ty: &'static str,
        read: impl FnOnce(&mut Self) -> Result<T, Self::Error>,
    ) -> Result<T, Self::Error> {
        read(self)
    }

    fn more(&self) -> bool {
        true
    }

    fn element(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn invalid(&self, reason: &'static str) -> Self::Error {
        reason
    }
}
