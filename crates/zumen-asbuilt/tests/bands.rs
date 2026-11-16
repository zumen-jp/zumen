//! 差の色の段（#476）。段の表と、点ごとの段の決め方が、規格値の判定と同じ端の扱いになるかを確かめる。
use zumen_asbuilt::bands::{Step, bands, classify, color};
use zumen_asbuilt::{Range, Rule};
use zumen_base::Severity;

fn rule(min: Option<f64>, max: Option<f64>) -> Rule {
    Rule {
        id: "r".into(),
        level: Severity::Error,
        part: "flat".into(),
        slope: Range::default(),
        mean: Range::default(),
        each: Range { min, max },
        density: None,
        exclude: 0.0,
    }
}

/// 段の表の中で差（mm）を含む段。境は 0 に近い段、規格値ちょうどは内側
fn table(rule: &Rule, mm: f64) -> Vec<Step> {
    bands(rule)
        .into_iter()
        .filter(|band| {
            let (l, u) = (band.lower, band.upper);
            match band.kind {
                Step::Below => u.is_some_and(|u| mm < u),
                Step::Above => l.is_some_and(|l| mm > l),
                Step::LowOuter | Step::LowInner => {
                    l.is_none_or(|l| mm >= l) && u.is_some_and(|u| mm < u)
                }
                Step::HighInner => l.is_some_and(|l| mm >= l) && u.is_none_or(|u| mm <= u),
                Step::HighOuter => l.is_some_and(|l| mm > l) && u.is_some_and(|u| mm <= u),
                Step::NotEvaluated => false,
            }
        })
        .map(|band| band.kind)
        .collect()
}

#[test]
fn boundaries_go_to_the_band_nearer_zero() {
    let r = rule(Some(-150.0), Some(150.0));
    let cases = [
        (-0.2, Step::Below),
        (-0.15, Step::LowOuter),
        (-0.1, Step::LowOuter),
        (-0.075, Step::LowInner),
        (-0.01, Step::LowInner),
        (0.0, Step::HighInner),
        (0.075, Step::HighInner),
        (0.1, Step::HighOuter),
        (0.15, Step::HighOuter),
        (0.2, Step::Above),
    ];
    for (difference, step) in cases {
        assert_eq!(classify(&r, difference), step, "{difference}");
    }
    let kinds: Vec<Step> = bands(&r).iter().map(|band| band.kind).collect();
    assert_eq!(
        kinds,
        [
            Step::Below,
            Step::LowOuter,
            Step::LowInner,
            Step::HighInner,
            Step::HighOuter,
            Step::Above
        ]
    );
}

#[test]
fn every_difference_falls_in_exactly_the_classified_band() {
    // 片側だけの規格値（上限のない天端など）、0 の規格値、規格値のない規則も含める
    let rules = [
        rule(Some(-150.0), Some(150.0)),
        rule(Some(-150.0), None),
        rule(None, Some(70.0)),
        rule(None, None),
        rule(Some(0.0), Some(0.0)),
        rule(Some(-160.0), Some(0.0)),
    ];
    for r in &rules {
        for step in -400..=400 {
            let difference = f64::from(step) / 2000.0;
            let found = table(r, difference * 1000.0);
            assert_eq!(found, [classify(r, difference)], "{r:?} {difference}");
        }
    }
}

#[test]
fn colors_are_distinct_and_opaque() {
    let steps = [
        Step::Below,
        Step::LowOuter,
        Step::LowInner,
        Step::HighInner,
        Step::HighOuter,
        Step::Above,
        Step::NotEvaluated,
    ];
    for (i, a) in steps.iter().enumerate() {
        assert_eq!(color(*a)[3], u16::MAX);
        for b in &steps[i + 1..] {
            assert_ne!(color(*a), color(*b));
        }
    }
}
