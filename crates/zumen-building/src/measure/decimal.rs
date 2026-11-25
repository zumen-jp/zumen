//! 最短の十進表記へ移してから、指定した桁へ丸める。二進の値に 10 の累乗を掛けて丸めない。

use zumen_base::{Cx, Kind};
use zumen_geom3d::Failure;

/// 十進の丸め方。符号がある値も同じ規則で扱う。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundingMode {
    /// 0 に近い側へ切り捨てる。
    Truncate,
    /// 負の無限大に近い側。
    Floor,
    /// 正の無限大に近い側。
    Ceiling,
    /// 最も近い値。同じ距離なら 0 から遠い側。
    HalfAway,
    /// 最も近い値。同じ距離なら末尾が偶数になる側。
    HalfEven,
}

/// 丸める段。要素ごとに丸めた和と、丸める前の和を丸めた値を区別する。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundingStage {
    /// 各要素を丸め、整数の係数で足す。
    Element,
    /// 要素の丸め前の値を足し、最後に丸める。
    Total,
}

/// 明示する丸めの引数。小数点以下の桁は -18 から 18 まで。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rounding {
    /// 丸め方。
    pub mode: RoundingMode,
    /// 小数点以下の桁。負なら小数点より左を丸める。
    pub digits: i8,
    /// 丸める段。
    pub stage: RoundingStage,
}

/// coefficient × 10^(-digits) を表す十進数。桁区切りや単位の表示は持たない。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decimal {
    coefficient: i128,
    digits: i8,
}

impl Decimal {
    /// 十進の整数の係数。
    pub fn coefficient(self) -> i128 {
        self.coefficient
    }

    /// 小数点以下の桁。末尾の 0 も指定どおり保つ。
    pub fn digits(self) -> i8 {
        self.digits
    }

    /// 同じ桁の十進数を、二進の実数へ戻さずに足す。
    pub fn checked_add(self, other: Self) -> Option<Self> {
        (self.digits == other.digits)
            .then(|| self.coefficient.checked_add(other.coefficient))
            .flatten()
            .map(|coefficient| Self {
                coefficient,
                ..self
            })
    }
}

impl core::fmt::Display for Decimal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let magnitude = self.coefficient.unsigned_abs();
        if self.coefficient < 0 {
            f.write_str("-")?;
        }
        if self.digits <= 0 {
            write!(f, "{magnitude}")?;
            if magnitude != 0 {
                for _ in 0..-self.digits {
                    f.write_str("0")?;
                }
            }
            return Ok(());
        }
        let denominator = 10u128.pow(self.digits as u32);
        write!(
            f,
            "{}.{:0width$}",
            magnitude / denominator,
            magnitude % denominator,
            width = self.digits as usize,
        )
    }
}

impl Rounding {
    /// 値と桁を検査し、十進の係数で丸める。表せない大きさは失敗にする。
    pub fn apply(self, value: f64, cx: &mut Cx<'_>) -> Result<Decimal, Failure> {
        cx.check(Kind::Work, 128)?;
        if !value.is_finite() || !(-18..=18).contains(&self.digits) {
            return Err(Failure::Invalid("measure_rounding"));
        }
        // 指数表記は f64 の最短の仮数と指数だけなので、極小値にも長い 0 の列を作らない。
        cx.check(Kind::DecodedBytes, 96)?;
        let text = format!("{:e}", value.abs());
        let (mantissa, exponent) = text
            .split_once('e')
            .ok_or(Failure::Invalid("measure_decimal"))?;
        let exponent: i32 = exponent
            .parse()
            .map_err(|_| Failure::Invalid("measure_decimal"))?;
        let mut coefficient = 0u128;
        let mut fraction = None;
        for byte in mantissa.bytes() {
            match byte {
                b'.' => fraction = Some(0),
                b'0'..=b'9' => {
                    coefficient = coefficient
                        .checked_mul(10)
                        .and_then(|v| v.checked_add(u128::from(byte - b'0')))
                        .ok_or(Failure::Invalid("measure_decimal"))?;
                    if let Some(n) = fraction.as_mut() {
                        *n += 1;
                    }
                }
                _ => return Err(Failure::Invalid("measure_decimal")),
            }
        }
        let shift = exponent - fraction.unwrap_or(0) + i32::from(self.digits);
        round_decimal(coefficient, shift, value < 0.0, self.digits, self.mode)
    }
}

/// 仮数の整数係数と十進の桁移動量から、指定した丸め方で [`Decimal`] を求める。
fn round_decimal(
    coefficient: u128,
    shift: i32,
    negative: bool,
    digits: i8,
    mode: RoundingMode,
) -> Result<Decimal, Failure> {
    if !(-18..=18).contains(&digits) {
        return Err(Failure::Invalid("measure_rounding"));
    }
    let (integer, remainder, comparison) = if coefficient == 0 {
        (0, false, core::cmp::Ordering::Less)
    } else if shift >= 0 {
        let power = 10u128
            .checked_pow(shift as u32)
            .ok_or(Failure::Invalid("measure_decimal_range"))?;
        (
            coefficient
                .checked_mul(power)
                .ok_or(Failure::Invalid("measure_decimal_range"))?,
            false,
            core::cmp::Ordering::Less,
        )
    } else if let Some(divisor) = shift
        .checked_neg()
        .and_then(|exp| 10u128.checked_pow(exp as u32))
    {
        let remainder = coefficient % divisor;
        (
            coefficient / divisor,
            remainder != 0,
            remainder.cmp(&(divisor - remainder)),
        )
    } else {
        // 最短の f64 の係数より十分大きい除数。半分には達しない。
        (0, true, core::cmp::Ordering::Less)
    };
    apply_rounding(integer, remainder, comparison, negative, digits, mode)
}

/// 除算または桁移動で得た整数部と端数の情報から、丸め規則に従って [`Decimal`] を組み立てる。
fn apply_rounding(
    integer: u128,
    remainder: bool,
    comparison: core::cmp::Ordering,
    negative: bool,
    digits: i8,
    mode: RoundingMode,
) -> Result<Decimal, Failure> {
    let increase = remainder
        && match mode {
            RoundingMode::Truncate => false,
            RoundingMode::Floor => negative,
            RoundingMode::Ceiling => !negative,
            RoundingMode::HalfAway => !comparison.is_lt(),
            RoundingMode::HalfEven => {
                comparison.is_gt() || (comparison.is_eq() && !integer.is_multiple_of(2))
            }
        };
    let magnitude = integer
        .checked_add(u128::from(increase))
        .and_then(|v| i128::try_from(v).ok())
        .ok_or(Failure::Invalid("measure_decimal_range"))?;
    Ok(Decimal {
        coefficient: if negative { -magnitude } else { magnitude },
        digits,
    })
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    fn any_mode() -> RoundingMode {
        match kani::any::<u8>() % 5 {
            0 => RoundingMode::Truncate,
            1 => RoundingMode::Floor,
            2 => RoundingMode::Ceiling,
            3 => RoundingMode::HalfAway,
            _ => RoundingMode::HalfEven,
        }
    }

    #[kani::proof]
    fn verify_round_decimal_sign_bounds_and_order() {
        let coefficient: u128 = kani::any();
        let shift: i32 = kani::any();
        let negative: bool = kani::any();
        let digits: i8 = kani::any();
        let mode = any_mode();

        // f64 の最短十進表記と桁 -18..=18 から生じる範囲を十分に覆う
        kani::assume((-400..=400).contains(&shift));

        let result = round_decimal(coefficient, shift, negative, digits, mode);
        if !(-18..=18).contains(&digits) {
            assert!(result.is_err());
            return;
        }
        if let Ok(dec) = result {
            assert_eq!(dec.digits(), digits);
            if coefficient == 0 {
                assert_eq!(dec.coefficient(), 0);
            } else if negative {
                assert!(dec.coefficient() <= 0);
            } else {
                assert!(dec.coefficient() >= 0);
            }
        }
    }

    #[kani::proof]
    fn verify_round_decimal_floor_le_truncate_and_ceiling() {
        // 1) 丸め規則単体を u128 全域で検証し、Floor <= 任意の丸め方 <= Ceiling と差 <= 1 を示す
        let integer: u128 = kani::any();
        let remainder: bool = kani::any();
        let comparison = match kani::any::<u8>() % 3 {
            0 => core::cmp::Ordering::Less,
            1 => core::cmp::Ordering::Equal,
            _ => core::cmp::Ordering::Greater,
        };
        let negative: bool = kani::any();
        let digits: i8 = kani::any();
        let mode = any_mode();
        kani::assume((-18..=18).contains(&digits));

        let floor = apply_rounding(
            integer,
            remainder,
            comparison,
            negative,
            digits,
            RoundingMode::Floor,
        );
        let mid = apply_rounding(integer, remainder, comparison, negative, digits, mode);
        let ceil = apply_rounding(
            integer,
            remainder,
            comparison,
            negative,
            digits,
            RoundingMode::Ceiling,
        );

        if let (Ok(f), Ok(m), Ok(ce)) = (&floor, &mid, &ceil) {
            assert!(f.coefficient() <= m.coefficient());
            assert!(m.coefficient() <= ce.coefficient());
            assert!(ce.coefficient() - f.coefficient() <= 1);
            if !remainder {
                assert_eq!(f.coefficient(), ce.coefficient());
            }
        }

        // 2) 除算と丸めの接続も定数除数 (shift = -1, divisor = 10) の全端数パターンで確かめる
        let c = u128::from(kani::any::<u16>());
        let f_e2e = round_decimal(c, -1, negative, digits, RoundingMode::Floor).unwrap();
        let m_e2e = round_decimal(c, -1, negative, digits, mode).unwrap();
        let c_e2e = round_decimal(c, -1, negative, digits, RoundingMode::Ceiling).unwrap();
        assert!(f_e2e.coefficient() <= m_e2e.coefficient());
        assert!(m_e2e.coefficient() <= c_e2e.coefficient());
        assert!(c_e2e.coefficient() - f_e2e.coefficient() <= 1);
    }

    #[kani::proof]
    fn verify_decimal_checked_add_and_denominator_safe() {
        let a_coeff: i128 = kani::any();
        let b_coeff: i128 = kani::any();
        let a_digits: i8 = kani::any();
        let b_digits: i8 = kani::any();
        kani::assume((-18..=18).contains(&a_digits));
        kani::assume((-18..=18).contains(&b_digits));

        let a = Decimal {
            coefficient: a_coeff,
            digits: a_digits,
        };
        let b = Decimal {
            coefficient: b_coeff,
            digits: b_digits,
        };
        let sum = a.checked_add(b);
        if a_digits != b_digits {
            assert!(sum.is_none());
        } else {
            assert_eq!(sum.map(Decimal::coefficient), a_coeff.checked_add(b_coeff));
        }
        // Display の 10u128.pow(digits as u32) が正の桁で決してあふれないことを検証
        if a_digits > 0 {
            assert!(10u128.checked_pow(a_digits as u32).is_some());
        }
    }
}
