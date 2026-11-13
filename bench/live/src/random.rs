//! 決まった列を返す乱数。同じ種なら、どの環境でも同じ変更の列になる。

/// 決まった列を返す乱数（SplitMix64）
pub struct Random(pub u64);

impl Random {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// 0 以上 1 未満
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// 0 以上 `count` 未満。`count` は 1 以上
    pub fn below(&mut self, count: usize) -> usize {
        (self.next() % count.max(1) as u64) as usize
    }

    pub fn between(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * self.unit()
    }

    pub fn pick(&mut self, items: &[f64]) -> f64 {
        items.get(self.below(items.len())).copied().unwrap_or(1.0)
    }

    /// 桁の多い値・1/8 の倍数・整数のどれかにする
    pub fn rounded(&mut self, value: f64) -> f64 {
        match self.below(3) {
            0 => value,
            1 => (value * 8.0).round() / 8.0,
            _ => value.round(),
        }
    }
}
