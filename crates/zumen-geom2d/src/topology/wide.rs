//! 円と円弧の整数式を比較するための、固定長 256 bit の値と 512 bit の積。
//! 座標の範囲は親のモジュールで制限する。除算・動的な確保・可変長の反復は使わない。

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub(super) struct Wide {
    high: u128,
    low: u128,
}

impl Wide {
    pub(super) const ZERO: Self = Self { high: 0, low: 0 };

    /// 128 bit どうしの積を、64 bit の 4 つの積から求める。
    #[cfg_attr(kani, kani::ensures(|res: &Self| (a == 0 || b == 0) == (*res == Self::ZERO)))]
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

    pub(super) fn checked_add(self, other: Self) -> Option<Self> {
        let (low, carry) = self.low.overflowing_add(other.low);
        Some(Self {
            high: self
                .high
                .checked_add(other.high)?
                .checked_add(u128::from(carry))?,
            low,
        })
    }

    /// 256 bit どうしの積。円弧の向きと二次式の根を比較する。
    pub(super) fn full_product(self, other: Self) -> DoubleWide {
        let p0 = Self::product(self.low, other.low);
        let p1 = Self::product(self.high, other.low);
        let p2 = Self::product(self.low, other.high);
        let p3 = Self::product(self.high, other.high);
        let (middle, c1) = p0.high.overflowing_add(p1.low);
        let (middle, c2) = middle.overflowing_add(p2.low);
        let carry = u128::from(c1) + u128::from(c2);
        let (upper, c3) = p1.high.overflowing_add(p2.high);
        let (upper, c4) = upper.overflowing_add(p3.low);
        let (upper, c5) = upper.overflowing_add(carry);
        DoubleWide {
            high: Self {
                high: p3.high + u128::from(c3) + u128::from(c4) + u128::from(c5),
                low: upper,
            },
            low: Self {
                high: middle,
                low: p0.low,
            },
        }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct DoubleWide {
    high: Wide,
    low: Wide,
}

/// 符号を分けた 256 bit の値。円弧の境界との外積に使う。
pub(super) struct SignedWide {
    pub(super) negative: bool,
    pub(super) magnitude: Wide,
}

impl SignedWide {
    pub(super) fn difference_of_products(a: i128, b: i128, c: i128, d: i128) -> Option<Self> {
        let left = Wide::product(a.unsigned_abs(), b.unsigned_abs());
        let right = Wide::product(c.unsigned_abs(), d.unsigned_abs());
        let left_negative = (a < 0) != (b < 0);
        let right_negative = (c < 0) == (d < 0);
        if left_negative == right_negative {
            Some(Self {
                negative: left_negative,
                magnitude: left.checked_add(right)?,
            })
        } else if left >= right {
            Some(Self {
                negative: left_negative,
                magnitude: left.checked_sub(right)?,
            })
        } else {
            Some(Self {
                negative: right_negative,
                magnitude: right.checked_sub(left)?,
            })
        }
    }

    pub(super) fn sign(&self) -> std::cmp::Ordering {
        if self.magnitude == Wide::ZERO {
            std::cmp::Ordering::Equal
        } else if self.negative {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
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

    #[test]
    fn full_products_and_signed_differences_keep_every_carry() {
        let max = Wide {
            high: u128::MAX,
            low: u128::MAX,
        };
        assert_eq!(max.checked_add(Wide { high: 0, low: 1 }), None);
        assert_eq!(
            max.full_product(max),
            DoubleWide {
                high: Wide {
                    high: u128::MAX,
                    low: u128::MAX - 1
                },
                low: Wide { high: 0, low: 1 },
            }
        );
        for a in [-123_i128, -1, 0, 1, 127] {
            for b in [-127_i128, -1, 0, 1, 123] {
                for c in [-53_i128, 0, 53] {
                    for d in [-97_i128, 0, 97] {
                        let value = a * b - c * d;
                        let signed = SignedWide::difference_of_products(a, b, c, d).unwrap();
                        assert_eq!(signed.sign(), value.cmp(&0));
                        assert_eq!(
                            signed.magnitude,
                            Wide {
                                high: 0,
                                low: value.unsigned_abs()
                            }
                        );
                    }
                }
            }
        }
        let difference =
            SignedWide::difference_of_products(i128::MIN, i128::MIN, i128::MIN, i128::MAX).unwrap();
        assert_eq!(difference.sign(), std::cmp::Ordering::Greater);
        assert_eq!(
            difference.magnitude,
            Wide {
                high: (1 << 127) - 1,
                low: 1 << 127
            }
        );
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof_for_contract(Wide::product)]
    #[kani::solver(kissat)]
    fn verify_wide_product_contract() {
        let a: u128 = kani::any();
        let b: u128 = kani::any();
        let _ = Wide::product(a, b);
    }

    #[kani::proof]
    #[kani::solver(kissat)]
    fn verify_wide_product_commutativity_and_small_exact() {
        // 下位 64 bit からの桁上がり（<< 60）と上位 64 bit を含む 4 肢の積すべてで可換性を検証する
        let a0 = u128::from(kani::any::<u8>() & 0x0F);
        let a1 = u128::from(kani::any::<u8>() & 0x0F);
        let b0 = u128::from(kani::any::<u8>() & 0x0F);
        let b1 = u128::from(kani::any::<u8>() & 0x0F);
        let a = (a1 << 64) | (a0 << 60);
        let b = (b1 << 64) | (b0 << 60);
        assert_eq!(Wide::product(a, b), Wide::product(b, a));

        let x = u128::from(kani::any::<u8>());
        let y = u128::from(kani::any::<u8>());
        let small = Wide::product(x, y);
        assert_eq!(small.high, 0);
        assert_eq!(small.low, x * y);
    }

    #[kani::proof]
    #[kani::solver(kissat)]
    fn verify_wide_add_sub_roundtrip_and_overflow() {
        let a = Wide {
            high: kani::any(),
            low: kani::any(),
        };
        let b = Wide {
            high: kani::any(),
            low: kani::any(),
        };
        if let Some(sum) = a.checked_add(b) {
            assert_eq!(sum.checked_sub(b), Some(a));
            assert_eq!(sum.checked_sub(a), Some(b));
            assert!(sum >= a && sum >= b);
        }
        if let Some(diff) = a.checked_sub(b) {
            assert!(a >= b);
            assert_eq!(diff.checked_add(b), Some(a));
        } else {
            assert!(a < b);
        }
    }

    #[kani::proof]
    #[kani::solver(kissat)]
    fn verify_wide_full_product_no_overflow_and_matches_checked_mul() {
        // 下位・上位の桁上がり境界と 256 bit 上限超過（Some / None の両枝）をすべて通す
        let ah: u8 = kani::any();
        let al: u8 = kani::any();
        let lh: u8 = kani::any();
        let ll: u8 = kani::any();
        let f: u8 = kani::any();
        let a = Wide {
            high: (u128::from(ah) << 120) | u128::from(al),
            low: (u128::from(lh) << 120) | u128::from(ll),
        };
        let factor = u128::from(f);
        let full = a.full_product(Wide {
            high: 0,
            low: factor,
        });
        match a.checked_mul(factor) {
            Some(prod) => {
                assert_eq!(full.high, Wide::ZERO);
                assert_eq!(full.low, prod);
            }
            None => {
                assert!(full.high != Wide::ZERO);
            }
        }
    }

    #[kani::proof]
    #[kani::stub_verified(Wide::product)]
    fn verify_wide_square_zero_iff_input_zero_via_verified_stub() {
        let value: i128 = kani::any();
        let sq = Wide::square(value);
        assert_eq!(sq == Wide::ZERO, value == 0);
    }

    #[kani::proof]
    #[kani::solver(kissat)]
    fn verify_wide_full_product_total_no_overflow() {
        let a = Wide {
            high: kani::any(),
            low: kani::any(),
        };
        let b = Wide {
            high: kani::any(),
            low: kani::any(),
        };
        let p = a.full_product(b);
        if a == Wide::ZERO || b == Wide::ZERO {
            assert_eq!(p.high, Wide::ZERO);
            assert_eq!(p.low, Wide::ZERO);
        }
    }

    #[kani::proof]
    #[kani::solver(kissat)]
    fn verify_signed_wide_difference_of_products_antisymmetry_and_exact() {
        let a: i8 = kani::any();
        let b: i8 = kani::any();
        let c: i8 = kani::any();
        let d: i8 = kani::any();
        let ai = i128::from(a);
        let bi = i128::from(b);
        let ci = i128::from(c);
        let di = i128::from(d);

        // 全符号組合せで反対称性と厳密な a * b - c * d との一致を検証する
        let fwd = SignedWide::difference_of_products(ai, bi, ci, di).unwrap();
        let rev = SignedWide::difference_of_products(ci, di, ai, bi).unwrap();
        assert_eq!(fwd.magnitude, rev.magnitude);
        assert_eq!(fwd.sign(), rev.sign().reverse());

        let exact = ai * bi - ci * di;
        assert_eq!(fwd.sign(), exact.cmp(&0));
        assert_eq!(fwd.magnitude.high, 0);
        assert_eq!(fwd.magnitude.low, exact.unsigned_abs());
    }
}
