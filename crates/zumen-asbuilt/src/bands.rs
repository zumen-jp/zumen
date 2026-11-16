//! 標高の差の色の段（#476）。段は規則の `each`（個々の計測値の規格値、mm）から決め、規格値の判定（`outside`）と同じ
//! 比べ方にする。規格値ちょうどは内側、内側の境の値は 0 に近い段に入れる。

use zumen_base::{Cx, Kind, Stop};

use crate::{Report, Rule};

/// 段の種類。差の小さい順
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Step {
    /// 下限より小さい
    Below,
    /// 下限から下限の半分まで
    LowOuter,
    /// 下限の半分から 0 まで（0 を含まない）
    LowInner,
    /// 0 から上限の半分まで
    HighInner,
    /// 上限の半分から上限まで
    HighOuter,
    /// 上限より大きい
    Above,
    /// 評価しなかった点
    NotEvaluated,
}

/// 段の色（16 ビットの赤・緑・青・不透明度）。8 ビットの値を 257 倍する
pub const fn color(kind: Step) -> [u16; 4] {
    let [r, g, b] = match kind {
        Step::Below => [128_u16, 0, 160],
        Step::LowOuter => [0, 90, 255],
        Step::LowInner => [0, 190, 200],
        Step::HighInner => [120, 200, 0],
        Step::HighOuter => [255, 150, 0],
        Step::Above => [220, 0, 0],
        Step::NotEvaluated => [160, 160, 160],
    };
    [r * 257, g * 257, b * 257, u16::MAX]
}

/// 規則の段
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    /// 種類
    pub kind: Step,
    /// 下の端の差（mm）。端がなければ `None`
    pub lower: Option<f64>,
    /// 上の端の差（mm）。端がなければ `None`
    pub upper: Option<f64>,
}

/// 0 より下の規格値（下限）。なければ下の側は内側の 1 段だけ
fn low(rule: &Rule) -> Option<f64> {
    rule.each.min.filter(|min| *min < 0.0)
}

/// 0 より上の規格値（上限）
fn high(rule: &Rule) -> Option<f64> {
    rule.each.max.filter(|max| *max > 0.0)
}

/// 差（m）の段。規格値の判定と同じく mm にして比べる
pub fn classify(rule: &Rule, difference: f64) -> Step {
    let mm = difference * 1000.0;
    if rule.each.min.is_some_and(|min| mm < min) {
        return Step::Below;
    }
    if rule.each.max.is_some_and(|max| mm > max) {
        return Step::Above;
    }
    if mm < 0.0 {
        match low(rule) {
            Some(min) if mm < min / 2.0 => Step::LowOuter,
            _ => Step::LowInner,
        }
    } else {
        match high(rule) {
            Some(max) if mm > max / 2.0 => Step::HighOuter,
            _ => Step::HighInner,
        }
    }
}

/// 規則で起こりうる段。差の小さい順
pub fn bands(rule: &Rule) -> Vec<Band> {
    let band = |kind, lower, upper| Band { kind, lower, upper };
    let mut out = Vec::new();
    if let Some(min) = rule.each.min {
        out.push(band(Step::Below, None, Some(min)));
    }
    match low(rule) {
        Some(min) => {
            out.push(band(Step::LowOuter, Some(min), Some(min / 2.0)));
            out.push(band(Step::LowInner, Some(min / 2.0), Some(0.0)));
        }
        // 下限が 0 以上なら、0 より下はすべて below
        None if rule.each.min.is_none() => out.push(band(Step::LowInner, None, Some(0.0))),
        None => {}
    }
    match high(rule) {
        Some(max) => {
            out.push(band(Step::HighInner, Some(0.0), Some(max / 2.0)));
            out.push(band(Step::HighOuter, Some(max / 2.0), Some(max)));
        }
        // 上限がちょうど 0 なら、0 だけがこの段に入る
        None if rule.each.max.is_none_or(|max| max >= 0.0) => {
            out.push(band(Step::HighInner, Some(0.0), rule.each.max));
        }
        None => {}
    }
    if let Some(max) = rule.each.max {
        out.push(band(Step::Above, Some(max), None));
    }
    out
}

/// 点群のすべての点の色。評価した点は規則の段の色、ほかは評価しなかった点の色
pub fn colors(
    points: usize,
    report: &Report,
    rules: &[Rule],
    cx: &mut Cx<'_>,
) -> Result<Vec<[u16; 4]>, Stop> {
    cx.check(Kind::DecodedBytes, (points as u64).saturating_mul(8))?;
    cx.check(Kind::Work, points as u64 + report.evaluated.len() as u64)?;
    let mut out = vec![color(Step::NotEvaluated); points];
    for point in &report.evaluated {
        if let (Some(slot), Some(rule)) = (out.get_mut(point.index), rules.get(point.rule)) {
            *slot = color(classify(rule, point.difference));
        }
    }
    Ok(out)
}
