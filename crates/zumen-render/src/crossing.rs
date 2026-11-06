//! 走査線上のサンプルが辺との交点以上にあるかを、丸めた交点を作らずに調べる。

use zumen_base::{Cx, Kind};

use crate::{Error, raster::Point};

/// low.y <= p.y < high.y。サンプル p が辺上か右にあれば true。
pub(crate) fn on_or_right(
    low: Point,
    high: Point,
    p: Point,
    cx: &mut Cx<'_>,
) -> Result<bool, Error> {
    let (left, right, height_at_sample) = if low[0] <= high[0] {
        (low[0], high[0], p[1] - low[1])
    } else {
        (high[0], low[0], high[1] - p[1])
    };
    if p[0] < left {
        return Ok(false);
    }
    if p[0] >= right {
        return Ok(true);
    }
    // 左端からの横の距離 × 辺の高さと、辺の幅 × 左端からの縦の距離を比べる。
    // 4 つの差はすべて非負。各演算を外側の隣接値まで広げ、積の範囲が離れれば符号が決まる。
    let lhs = product_bounds(p[0] - left, high[1] - low[1]);
    let rhs = product_bounds(right - left, height_at_sample);
    if lhs[0] >= rhs[1] {
        return Ok(true);
    }
    if lhs[1] < rhs[0] {
        return Ok(false);
    }
    // 6 個の積と各 3 語への加算、固定長の桁上がりを一度に数える。
    cx.check(Kind::Work, 6 * 4 + WORDS as u64)?;
    exact_on_or_right(low, high, p)
}

fn product_bounds(a: f64, b: f64) -> [f64; 2] {
    let lo = a.next_down().max(0.0) * b.next_down().max(0.0);
    let hi = a.next_up() * b.next_up();
    [lo.next_down(), hi.next_up()]
}

// binary64 の最小単位は 2^-1074、積の単位は 2^-2148。入力の絶対値は 2^31 以下なので
// 積は 2^62 以下。35 語（2240 bit）で最下位の積から 6 個の積の和まで収まる。
const WORDS: usize = 35;

fn exact_on_or_right(a: Point, b: Point, p: Point) -> Result<bool, Error> {
    // (px-ax)(by-ay) - (py-ay)(bx-ax) を展開する。差を先に binary64 に丸めない。
    let mut words = [0_i128; WORDS];
    for (x, y, sign) in [
        (p[0], b[1], 1),
        (a[0], p[1], 1),
        (b[0], a[1], 1),
        (p[0], a[1], -1),
        (a[0], b[1], -1),
        (b[0], p[1], -1),
    ] {
        let (xm, xe) = parts(x);
        let (ym, ye) = parts(y);
        let product = sign * i128::from(xm) * i128::from(ym);
        if product == 0 {
            continue;
        }
        let shift = xe + ye;
        let start = shift / 64;
        let offset = shift % 64;
        // 最大 106 bit の積は、64 bit 語の途中から置いても 3 語に収まる。
        let magnitude = product.unsigned_abs();
        let lower = magnitude << offset;
        let upper = if offset == 0 {
            0
        } else {
            magnitude >> (128 - offset)
        };
        let limbs = [lower as u64, (lower >> 64) as u64, upper as u64];
        let slots = words.get_mut(start..start + 3).ok_or(Error::Options)?;
        for (slot, limb) in slots.iter_mut().zip(limbs) {
            *slot += product.signum() * i128::from(limb);
        }
    }
    // 各語には高々 6 個の u64 の符号付きの和がある。算術右シフトは負の繰り下がりも運ぶ。
    let carry = words
        .into_iter()
        .fold(0_i128, |carry, word| (word + carry) >> 64);
    Ok(carry >= 0)
}

/// v = 仮数 × 2^shift × 2^-1074。非正規化数には先頭の 1 を足さない。
fn parts(v: f64) -> (i64, usize) {
    let bits = v.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as usize;
    let fraction = (bits & ((1_u64 << 52) - 1)) as i64;
    let magnitude = fraction | (i64::from(exponent != 0) << 52);
    let signed = if v.is_sign_negative() {
        -magnitude
    } else {
        magnitude
    };
    (signed, exponent.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Diagnostics};

    #[test]
    fn subnormal_endpoint_changes_the_side_of_a_sample() {
        let (mut budget, mut diag, cancel) =
            (Budget::default(), Diagnostics::new(), CancelToken::new());
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let tiny = f64::from_bits(1);
        for (low, expected) in [
            ([tiny, 0.0], false),
            ([0.0, tiny], true),
            ([tiny, tiny], true),
        ] {
            assert_eq!(
                on_or_right(low, [1.0; 2], [0.125; 2], &mut cx),
                Ok(expected)
            );
        }
    }

    #[test]
    fn exact_products_match_integer_determinants_across_exponents() {
        let (mut budget, mut diag, cancel) =
            (Budget::default(), Diagnostics::new(), CancelToken::new());
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        for (xe, ye) in [
            (-1074, -1074),
            (-1060, 20),
            (-320, 13),
            (20, -1020),
            (20, 20),
        ] {
            for ax in [-37_i64, 0, 91] {
                for bx in [-109_i64, 1, 87] {
                    for px in [-45_i64, 12, 87] {
                        let scale = |x, y| [libm::scalbn(x as f64, xe), libm::scalbn(y as f64, ye)];
                        let expected = (px - ax) * 61 >= (bx - ax) * 37;
                        let (a, b, p) = (scale(ax, -30), scale(bx, 31), scale(px, 7));
                        assert_eq!(exact_on_or_right(a, b, p), Ok(expected));
                        assert_eq!(on_or_right(a, b, p, &mut cx), Ok(expected));
                    }
                }
            }
        }
        let limit = 2_147_483_648.0;
        assert_eq!(
            exact_on_or_right([-limit; 2], [limit; 2], [0.125; 2]),
            Ok(true)
        );
        assert_eq!(
            exact_on_or_right([-limit; 2], [limit; 2], [0.125_f64.next_down(), 0.125]),
            Ok(false)
        );
    }
}
