//! 浮動小数の表記。

use core::fmt::Write;

/// 浮動小数を、[`crate::Writer::float`] と同じ表記の文字列にする。有限でなければ `None`
pub fn float_text(value: f64) -> Option<String> {
    value.is_finite().then(|| {
        let mut out = String::new();
        write_finite(&mut out, value);
        out
    })
}

/// 有限の浮動小数を、ECMAScript の `Number::toString`（基数 10）と同じ表記で書く。
///
/// - 桁は、読み直すと同じ値に戻る最短のもの。候補が 2 つあれば値に近いほうを、同じだけ近ければ最後の桁が偶数のほうを選ぶ。
/// - 指数は、小数点の位置が 21 桁より右か、小数点のあとに 0 が 6 つ以上続くときだけ使う。
/// - 負の 0 は `0` と書く。
pub(crate) fn write_finite(out: &mut String, x: f64) {
    if x == 0.0 {
        out.push('0');
        return;
    }
    if x < 0.0 {
        out.push('-');
    }
    let abs = x.abs();
    let Some((mut digits, exponent)) = shortest(abs) else {
        let _ = write!(out, "{abs:e}");
        return;
    };
    break_tie_to_even(abs, &mut digits, exponent);
    let k = digits.len() as i64;
    // 値は 0.d1d2…dk × 10^n
    let n = exponent + 1;
    if k <= n && n <= 21 {
        out.push_str(&digits);
        out.extend(core::iter::repeat_n('0', (n - k) as usize));
    } else if 0 < n && n <= 21 {
        let (whole, fraction) = digits.split_at_checked(n as usize).unwrap_or((&digits, ""));
        out.push_str(whole);
        out.push('.');
        out.push_str(fraction);
    } else if -6 < n && n <= 0 {
        out.push_str("0.");
        out.extend(core::iter::repeat_n('0', (-n) as usize));
        out.push_str(&digits);
    } else {
        let (first, rest) = digits.split_at_checked(1).unwrap_or((&digits, ""));
        out.push_str(first);
        if !rest.is_empty() {
            out.push('.');
            out.push_str(rest);
        }
        let sign = if n > 0 { '+' } else { '-' };
        let _ = write!(out, "e{sign}{}", (n - 1).abs());
    }
}

/// 読み直すと同じ値に戻る最短の桁と、先頭の桁の 10 の指数。Rust の `{:e}` から取る
fn shortest(x: f64) -> Option<(String, i64)> {
    let scientific = format!("{x:e}");
    let (mantissa, exponent) = scientific.split_once('e')?;
    let digits = mantissa.chars().filter(|c| *c != '.').collect();
    Some((digits, exponent.parse().ok()?))
}

/// 同じ桁数の候補が 2 つあり、`x` がちょうど真ん中にあるとき、最後の桁が偶数のほうにする。
///
/// Rust の `{:e}` は、このとき偶数のほうを選ぶとは限らない。2 つの候補の最後の桁は偶数と奇数なので、
/// 選ばれた桁が奇数のときだけ、隣の候補を確かめる。
fn break_tie_to_even(x: f64, digits: &mut String, exponent: i64) {
    let Some(last) = digits.bytes().last() else {
        return;
    };
    if last % 2 == 0 {
        return;
    }
    let lower = with_last_digit(digits, last - 1);
    if reads_as(x, &lower, exponent) && is_exactly(x, &format!("{lower}5"), exponent) {
        *digits = lower;
        return;
    }
    // 9 の次は繰り上がる。繰り上がった候補は桁が減るので、読み直して同じ値になるなら、最初から選ばれている
    if last < b'9' {
        let upper = with_last_digit(digits, last + 1);
        if reads_as(x, &upper, exponent) && is_exactly(x, &format!("{digits}5"), exponent) {
            *digits = upper;
        }
    }
}

fn with_last_digit(digits: &str, last: u8) -> String {
    let mut changed = digits.to_string();
    changed.pop();
    changed.push(char::from(last));
    changed
}

/// 桁 d1d2…dk と、先頭の桁の指数で表した数を読み直すと、`x` になるか
fn reads_as(x: f64, digits: &str, exponent: i64) -> bool {
    let (first, rest) = digits.split_at_checked(1).unwrap_or((digits, ""));
    let text = format!("{first}.{rest}0e{exponent}");
    text.parse::<f64>()
        .is_ok_and(|y| y.to_bits() == x.to_bits())
}

/// `x` が、桁 d1d2…dk と先頭の桁の指数で表した数に、ちょうど等しいか。
///
/// 先に `x` を k 桁に丸めて比べ、違えば書き切らずに済ませる。浮動小数を 10 進で書き切ると、
/// 有効数字は多くても 767 桁なので、801 桁で書けば正確な値になる。
fn is_exactly(x: f64, digits: &str, exponent: i64) -> bool {
    let rounded = format!(
        "{x:.precision$e}",
        precision = digits.len().saturating_sub(1)
    );
    if !has_digits(&rounded, digits, exponent, false) {
        return false;
    }
    has_digits(&format!("{x:.800e}"), digits, exponent, true)
}

/// `{:e}` で書いたものが、桁 `digits` と指数 `exponent` で始まるか。`exact` なら、残りの桁がすべて 0 か
fn has_digits(scientific: &str, digits: &str, exponent: i64, exact: bool) -> bool {
    let Some((mantissa, written_exponent)) = scientific.split_once('e') else {
        return false;
    };
    if written_exponent.parse::<i64>().ok() != Some(exponent) {
        return false;
    }
    let written: String = mantissa.chars().filter(|c| *c != '.').collect();
    written
        .strip_prefix(digits)
        .is_some_and(|rest| !exact || rest.bytes().all(|b| b == b'0'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(x: f64) -> String {
        let mut out = String::new();
        write_finite(&mut out, x);
        out
    }

    #[test]
    fn matches_ecmascript_number_to_string() {
        // ECMAScript の (x).toString() と同じ表記
        let cases: &[(f64, &str)] = &[
            (0.0, "0"),
            (-0.0, "0"),
            (1.0, "1"),
            (-1.5, "-1.5"),
            (0.1, "0.1"),
            (100.0, "100"),
            (123.456, "123.456"),
            (1e20, "100000000000000000000"),
            (1e21, "1e+21"),
            (1.2345678901234568e20, "123456789012345680000"),
            (1.5e21, "1.5e+21"),
            (1e-6, "0.000001"),
            (1.5e-6, "0.0000015"),
            (1e-7, "1e-7"),
            (1.5e-7, "1.5e-7"),
            (1.23e-18, "1.23e-18"),
            (5e-324, "5e-324"),
            (f64::MAX, "1.7976931348623157e+308"),
            (9_007_199_254_740_992.0, "9007199254740992"),
            (0.000_123, "0.000123"),
            (2.0 / 3.0, "0.6666666666666666"),
        ];
        for &(x, expected) in cases {
            assert_eq!(text(x), expected, "{x:e}");
        }
    }

    #[test]
    fn breaks_exact_ties_to_the_even_digit() {
        // どちらも 2 つの候補のちょうど真ん中にある。ECMAScript は偶数の桁を選ぶ
        assert_eq!(
            text(f64::from_bits(0x4302_a972_30b5_50ea)),
            "656607186168349.2"
        );
        assert_eq!(
            text(f64::from_bits(0x42d5_df70_4592_fa48)),
            "96197621992425.12"
        );
        assert_eq!(
            text(-f64::from_bits(0x430c_9bbd_e3da_0f6a)),
            "-1006567402717677.2"
        );
    }

    #[test]
    fn reads_back_to_the_same_value() {
        for x in [0.1, 1.0 / 3.0, 1e300, 4.9e-324, 123_456.789, -2.5e-12] {
            assert_eq!(text(x).parse::<f64>().unwrap().to_bits(), x.to_bits());
        }
    }
}
