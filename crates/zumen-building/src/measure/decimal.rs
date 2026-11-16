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
        } else if let Some(divisor) = 10u128.checked_pow((-shift) as u32) {
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
        let increase = remainder
            && match self.mode {
                RoundingMode::Truncate => false,
                RoundingMode::Floor => value < 0.0,
                RoundingMode::Ceiling => value > 0.0,
                RoundingMode::HalfAway => !comparison.is_lt(),
                RoundingMode::HalfEven => {
                    comparison.is_gt() || (comparison.is_eq() && integer % 2 != 0)
                }
            };
        let magnitude = integer
            .checked_add(u128::from(increase))
            .and_then(|v| i128::try_from(v).ok())
            .ok_or(Failure::Invalid("measure_decimal_range"))?;
        Ok(Decimal {
            coefficient: if value < 0.0 { -magnitude } else { magnitude },
            digits: self.digits,
        })
    }
}
