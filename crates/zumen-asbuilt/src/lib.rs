//! 出来形（決定 0234）。設計の TIN と点群の標高の差を求め、プロファイルの `surface-elevation` の規則の区分ごとに
//! 集計して、規格値と比べる。
//!
//! 点の真下の三角形を格子の索引で探し、三角形の勾配で規則を選ぶ。区分の違う三角形から `exclude` の内にある点は
//! 除く。結果は指摘で返し、合格・適合とは言わない（`docs/style.md`）。

mod surface;

use zumen_base::{Cx, Diagnostic, Kind, Severity, Stop, codes};
use zumen_civil::Tin;
use zumen_json::Value;
use zumen_pointcloud::Points;
use zumen_profile::Finding;

use crate::surface::Surface;

/// 照合の種類の名前
pub const CHECK: &str = "surface-elevation";

/// 値の幅。書かない端は比べない
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Range {
    /// 下限
    pub min: Option<f64>,
    /// 上限
    pub max: Option<f64>,
}

/// 読んだ `surface-elevation` の規則
#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    /// 規則の id
    pub id: String,
    /// 重要度。規則の `level`
    pub level: Severity,
    /// 区分の名前
    pub part: String,
    /// 区分に入る三角形の勾配（割）。`min` より大きく `max` 以下
    pub slope: Range,
    /// 平均値の幅（mm）
    pub mean: Range,
    /// 個々の計測値の幅（mm）
    pub each: Range,
    /// 計測の密度の下限（点／m²）
    pub density: Option<f64>,
    /// 区分の境から除く水平の距離（m）
    pub exclude: f64,
}

/// 規則の区分ごとの集計
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    /// 規則の id
    pub rule: String,
    /// 区分の名前
    pub part: String,
    /// 評価した点の数
    pub points: u64,
    /// 区分の三角形の、平面に投影した面積（m²）
    pub area: f64,
    /// 計測の密度（点／m²）。面積がなければ `None`
    pub density: Option<f64>,
    /// 差の平均（m）。点がなければ `None`
    pub mean: Option<f64>,
    /// 差の最小（m）
    pub min: Option<f64>,
    /// 差の最大（m）
    pub max: Option<f64>,
    /// 指摘の数（確認できなかったものを除く）
    pub findings: u64,
    /// 確認できなかった数
    pub unsupported: u64,
}

/// 評価した点
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Evaluated {
    /// 点群の中の番号
    pub index: usize,
    /// 点の標高から設計面の標高を引いた差（m）
    pub difference: f64,
    /// 評価した規則の番号（規則の並びの位置）
    pub rule: usize,
}

/// 差を求めた結果
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    /// 規則ごとの集計。規則の順
    pub summaries: Vec<Summary>,
    /// 指摘。規則の順
    pub findings: Vec<Finding>,
    /// 評価した点。点群の順
    pub evaluated: Vec<Evaluated>,
    /// TIN の外の点の数
    pub outside: u64,
    /// どの規則の勾配にも入らない三角形の上の点の数
    pub unmatched: u64,
    /// 区分の境の近くなので除いた点の数
    pub excluded: u64,
}

fn number(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(number) => number.to_f64(),
        _ => None,
    }
}

fn text(value: Option<&Value>) -> Option<&str> {
    match value? {
        Value::String(text) => Some(text),
        _ => None,
    }
}

/// `{ min, max }` の欄。書かなければ幅なし。数でない端があれば `None`
fn range(value: Option<&Value>) -> Option<Range> {
    let Some(value) = value else {
        return Some(Range::default());
    };
    let Value::Object(object) = value else {
        return None;
    };
    let end = |key| match object.get(key) {
        None => Some(None),
        some => number(some).map(Some),
    };
    Some(Range {
        min: end("min")?,
        max: end("max")?,
    })
}

fn rule(id: &str, value: &Value) -> Option<Rule> {
    let Value::Object(object) = value else {
        return None;
    };
    let level = match text(object.get("level"))? {
        "error" => Severity::Error,
        "warning" => Severity::Warning,
        "info" => Severity::Info,
        _ => return None,
    };
    let density = match object.get("density") {
        None => None,
        some => Some(number(some).filter(|v| *v >= 0.0)?),
    };
    let exclude = match object.get("exclude") {
        None => 0.0,
        some => number(some).filter(|v| *v >= 0.0)?,
    };
    Some(Rule {
        id: id.to_string(),
        level,
        part: text(object.get("part"))?.to_string(),
        slope: range(object.get("slope"))?,
        mean: range(object.get("mean"))?,
        each: range(object.get("each"))?,
        density,
        exclude,
    })
}

/// 解いた規則から、`work` の工種の `surface-elevation` の規則を読む。使えない規則は `profile.rule.unsupported` を
/// 返して使わない
pub fn rules(values: &[(String, Value)], work: &str, cx: &mut Cx<'_>) -> Vec<Rule> {
    let mut found = Vec::new();
    for (id, value) in values {
        let field = |key| match value {
            Value::Object(object) => text(object.get(key)),
            _ => None,
        };
        let check = field("check").unwrap_or_default();
        if check == CHECK && field("work") != Some(work) {
            continue;
        }
        match rule(id, value).filter(|_| check == CHECK) {
            Some(rule) => found.push(rule),
            None => cx.report(
                Diagnostic::new(codes::PROFILE_RULE_UNSUPPORTED)
                    .with("rule", id.as_str())
                    .with("check", check),
            ),
        }
    }
    found
}

/// 区分ごとの差の集計
#[derive(Clone, Copy, Default)]
struct Tally {
    points: u64,
    sum: f64,
    min: Option<f64>,
    max: Option<f64>,
}

/// 幅を外れた値の指摘
fn outside(rule: &Rule, statistic: &str, found: f64, limits: Range) -> Option<Finding> {
    let below = limits.min.filter(|min| found < *min);
    let above = limits.max.filter(|max| found > *max);
    let limit = below.or(above)?;
    Some(Finding {
        rule: rule.id.clone(),
        level: rule.level,
        diagnostic: Diagnostic::new(codes::PROFILE_SURFACE_ELEVATION_INVALID)
            .with("rule", rule.id.as_str())
            .with("statistic", statistic)
            .with("found", found)
            .with("limit", limit),
    })
}

/// 点群の点と TIN の標高の差を求め、規則の区分ごとに集計して規格値と比べる
pub fn deviation(
    points: &Points,
    tin: &Tin,
    rules: &[Rule],
    cx: &mut Cx<'_>,
) -> Result<Report, Stop> {
    let surface = Surface::new(tin, rules, cx)?;
    let mut report = Report {
        summaries: Vec::new(),
        findings: Vec::new(),
        evaluated: Vec::new(),
        outside: 0,
        unmatched: 0,
        excluded: 0,
    };
    let mut tallies = vec![Tally::default(); rules.len()];
    let coordinates = points
        .easting
        .iter()
        .zip(&points.northing)
        .zip(&points.height);
    for (index, ((easting, northing), height)) in coordinates.enumerate() {
        cx.check(Kind::Work, 1)?;
        let Some((face, ground)) = surface.locate(*easting, *northing) else {
            report.outside += 1;
            continue;
        };
        let Some(at) = face.rule else {
            report.unmatched += 1;
            continue;
        };
        let (Some(rule), Some(tally)) = (rules.get(at), tallies.get_mut(at)) else {
            report.unmatched += 1;
            continue;
        };
        if rule.exclude > 0.0 && surface.near_other_part(*easting, *northing, at, rule.exclude) {
            report.excluded += 1;
            continue;
        }
        let difference = height - ground;
        tally.points += 1;
        tally.sum += difference;
        tally.min = Some(tally.min.map_or(difference, |min| min.min(difference)));
        tally.max = Some(tally.max.map_or(difference, |max| max.max(difference)));
        report.evaluated.push(Evaluated {
            index,
            difference,
            rule: at,
        });
    }
    for ((rule, tally), area) in rules.iter().zip(&tallies).zip(surface.areas()) {
        let mean = (tally.points > 0).then(|| tally.sum / tally.points as f64);
        let density = (area > 0.0).then(|| tally.points as f64 / area);
        let mut findings = Vec::new();
        let required = rule.density.unwrap_or(0.0);
        let unsupported = tally.points == 0
            || rule
                .density
                .is_some_and(|_| density.unwrap_or(0.0) < required);
        if unsupported {
            findings.push(Finding {
                rule: rule.id.clone(),
                level: Severity::Info,
                diagnostic: Diagnostic::new(codes::PROFILE_SURFACE_ELEVATION_UNSUPPORTED)
                    .with("rule", rule.id.as_str())
                    .with("density", density.unwrap_or(0.0))
                    .with("required", required),
            });
        }
        let invalid_before = findings.len();
        // 規格値は mm で書くので、差を mm にして比べる
        if let Some(mean) = mean {
            findings.extend(outside(rule, "mean", mean * 1000.0, rule.mean));
        }
        if let Some(min) = tally.min {
            let low = Range {
                min: rule.each.min,
                max: None,
            };
            findings.extend(outside(rule, "minimum", min * 1000.0, low));
        }
        if let Some(max) = tally.max {
            let high = Range {
                min: None,
                max: rule.each.max,
            };
            findings.extend(outside(rule, "maximum", max * 1000.0, high));
        }
        report.summaries.push(Summary {
            rule: rule.id.clone(),
            part: rule.part.clone(),
            points: tally.points,
            area,
            density,
            mean,
            min: tally.min,
            max: tally.max,
            findings: (findings.len() - invalid_before) as u64,
            unsupported: u64::from(unsupported),
        });
        report.findings.append(&mut findings);
    }
    Ok(report)
}
