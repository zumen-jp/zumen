//! 円の交点の整数式を比較するための、符号なし 256 bit の積と差。
//! 座標の範囲は親のモジュールで制限する。除算・動的な確保・可変長の反復は使わない。

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Wide {
    high: u128,
    low: u128,
}

impl Wide {
    pub(super) const ZERO: Self = Self { high: 0, low: 0 };

    /// 128 bit どうしの積を、64 bit の 4 つの積から求める。
    pub(super) fn product(a: u128, b: u128) -> Self {
        let (a0, a1) = (u128::from(a as u64), a >> 64);
        let (b0, b1) = (u128::from(b as u64), b >> 64);
        let first = a0 * b0;
        let second = a1 * b0 + (first >> 64);
        let third = a0 * b1 + u128::from(second as u64);
        Self {
            high: a1 * b1 + (second >> 64) + (third >> 64),
            low: (third << 64) | u128::from(first as u64),
        }
    }

    pub(super) fn square(value: i128) -> Self {
        Self::product(value.unsigned_abs(), value.unsigned_abs())
    }

    /// 負の差は返さない。
    pub(super) fn checked_sub(self, other: Self) -> Option<Self> {
        let (low, borrow) = self.low.overflowing_sub(other.low);
        Some(Self {
            high: self
                .high
                .checked_sub(other.high)?
                .checked_sub(u128::from(borrow))?,
            low,
        })
    }

    /// 円の式を比較するときの係数の二乗を掛ける。256 bit を超える場合は返さない。
    pub(super) fn checked_mul(self, factor: u128) -> Option<Self> {
        let lower = Self::product(self.low, factor);
        let upper = Self::product(self.high, factor);
        if upper.high != 0 {
            return None;
        }
        Some(Self {
            high: lower.high.checked_add(upper.low)?,
            low: lower.low,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn products_carry_across_all_four_limbs() {
        let cases = [
            (0, u128::MAX, Wide::ZERO),
            (
                1,
                u128::MAX,
                Wide {
                    high: 0,
                    low: u128::MAX,
                },
            ),
            (
                u128::MAX,
                u128::MAX,
                Wide {
                    high: u128::MAX - 1,
                    low: 1,
                },
            ),
            (
                1 << 127,
                1 << 127,
                Wide {
                    high: 1 << 126,
                    low: 0,
                },
            ),
            (
                (1 << 64) + 1,
                (1 << 64) + 1,
                Wide {
                    high: 1,
                    low: (2 << 64) + 1,
                },
            ),
            (
                (1 << 127) + 3,
                (1 << 126) + 5,
                Wide {
                    high: (1 << 125) + 3,
                    low: (1 << 126) + 15,
                },
            ),
        ];
        for (a, b, expected) in cases {
            assert_eq!(Wide::product(a, b), expected);
            assert_eq!(Wide::product(b, a), expected);
        }
        assert_eq!(
            Wide::square(i128::MIN),
            Wide {
                high: 1 << 126,
                low: 0
            }
        );
    }

    #[test]
    fn differences_and_products_reject_overflow() {
        let one = Wide { high: 0, low: 1 };
        let max = Wide {
            high: u128::MAX,
            low: u128::MAX,
        };
        assert_eq!(Wide::ZERO.checked_sub(one), None);
        assert_eq!(max.checked_sub(max), Some(Wide::ZERO));
        assert_eq!(
            Wide { high: 1, low: 0 }.checked_sub(one),
            Some(Wide {
                high: 0,
                low: u128::MAX
            })
        );
        assert_eq!(max.checked_mul(0), Some(Wide::ZERO));
        assert_eq!(max.checked_mul(1), Some(max));
        assert_eq!(max.checked_mul(2), None);
        assert_eq!(
            Wide { high: 1, low: 1 }.checked_mul(1 << 127),
            Some(Wide {
                high: 1 << 127,
                low: 1 << 127
            })
        );
    }
}
