//! 測点の値と表記（決定 0230）。
//!
//! 測点の値は、線形の初めの測点の値に追加距離を足したもの。測点の不連続（ブレーキ）の先は、そこでの進みの
//! 測点の値から数え直す。表記（`No.12+5.000`）は、測点の間隔と小数の桁数で決める。

use crate::Alignment;
use zumen_base::{Cx, Diagnostic, codes};

/// 測点の定め
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stationing {
    /// 線形の初めの測点の値（メートル）
    pub start: f64,
    /// 測点の不連続。追加距離の順に並べる
    pub breaks: Vec<StationBreak>,
}

/// 測点の不連続
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StationBreak {
    /// 追加距離（メートル）
    pub distance: f64,
    /// 戻りの測点の値（不連続の手前から数えた値）
    pub back: f64,
    /// 進みの測点の値（不連続の先で数え直す値）
    pub ahead: f64,
}

impl Stationing {
    /// 追加距離での測点の値
    pub fn value(&self, distance: f64) -> f64 {
        let base = self
            .breaks
            .iter()
            .rfind(|at| at.distance <= distance)
            .map_or((0.0, self.start), |at| (at.distance, at.ahead));
        base.1 + (distance - base.0)
    }

    /// 測点の値がある所の追加距離を、線形の初めから順に返す。線形の長さ `length` の外の所は除く
    pub fn distances(&self, value: f64, length: f64) -> Vec<f64> {
        let mut found = Vec::new();
        let mut from = (0.0, self.start);
        for at in &self.breaks {
            let to = at.distance.min(length);
            let distance = from.0 + (value - from.1);
            if distance >= from.0 && distance < to {
                found.push(distance);
            }
            from = (at.distance, at.ahead);
        }
        let distance = from.0 + (value - from.1);
        if distance >= from.0 && distance <= length {
            found.push(distance);
        }
        found
    }
}

/// 測点の表記の決め方。間隔と桁数はプロファイルから取り、なければ既定（20 m と 3 桁）にする
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Notation {
    /// 測点の間隔（メートル）
    pub pitch: f64,
    /// 端数の小数の桁数
    pub digits: u8,
}

impl Default for Notation {
    fn default() -> Self {
        Self {
            pitch: 20.0,
            digits: 3,
        }
    }
}

impl Notation {
    /// 測点の値を `No.12+5.000` の形に書く。間隔か値が使えなければ `None`
    pub fn format(&self, value: f64) -> Option<String> {
        let scale = 10_i64.checked_pow(u32::from(self.digits.min(9)))?;
        let total = libm::round(value * scale as f64);
        let pitch = libm::round(self.pitch * scale as f64);
        if !(total.is_finite() && pitch.is_finite() && pitch >= 1.0 && total.abs() < 9.0e15) {
            return None;
        }
        let (total, pitch) = (total as i64, pitch as i64);
        let (number, rest) = (total.div_euclid(pitch), total.rem_euclid(pitch));
        let (whole, fraction) = (rest / scale, rest % scale);
        Some(if self.digits == 0 {
            format!("No.{number}+{whole}")
        } else {
            let width = usize::from(self.digits.min(9));
            format!("No.{number}+{whole}.{fraction:0width$}")
        })
    }

    /// `No.12+5.000`・`No.12+5`・`No.12` の形を、測点の値にする。`No.` は省いてもよい。読めなければ `None`
    pub fn parse(&self, text: &str) -> Option<f64> {
        let text = text.trim();
        let text = ["No.", "NO.", "no.", "No", "NO", "no"]
            .iter()
            .find_map(|prefix| text.strip_prefix(prefix))
            .unwrap_or(text)
            .trim_start();
        let (number, rest) = text.split_once('+').unwrap_or((text, "0"));
        let number: i64 = number.trim().parse().ok()?;
        let rest = rest.trim();
        if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
            return None;
        }
        let rest: f64 = rest.parse().ok()?;
        let value = number as f64 * self.pitch + rest;
        (self.pitch.is_finite() && self.pitch > 0.0 && value.is_finite()).then_some(value)
    }
}

/// 測点の表記から追加距離を求める。不連続のために同じ値が何か所にもあれば、`occurrence`（0 から）の所に
/// する。指定がなければ最初の所にして知らせる
pub fn distance(
    alignment: &Alignment,
    station: &str,
    notation: &Notation,
    occurrence: Option<usize>,
    cx: &mut Cx<'_>,
) -> Result<f64, Box<Diagnostic>> {
    let value = notation.parse(station).ok_or_else(|| {
        Box::new(Diagnostic::new(codes::CIVIL_STATION_INVALID).with("station", station.to_owned()))
    })?;
    let found = alignment
        .stations
        .distances(value, crate::position::length(alignment));
    let missing = || {
        Box::new(Diagnostic::new(codes::CIVIL_STATION_MISSING).with("station", station.to_owned()))
    };
    match occurrence {
        Some(at) => found.get(at).copied().ok_or_else(missing),
        None => {
            if found.len() > 1 {
                cx.report(
                    Diagnostic::new(codes::CIVIL_STATION_ASSUMED)
                        .with("station", station.to_owned())
                        .with("places", found.len() as u64),
                );
            }
            found.first().copied().ok_or_else(missing)
        }
    }
}
