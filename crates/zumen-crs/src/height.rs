//! 標高の基準と成果（決定 0019 の「標高」）。
//!
//! 成果の違う標高は、同じ地点でも数十 cm 違うことがある。黙って比べたり足したりせず、診断を返す。形式が成果を
//! 書いていなければ、仮定して `assumed` の診断にする（決定 0009）。

use zumen_base::{Cx, Diagnostic, Ref, codes};

/// 高さの基準
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HeightDatum {
    /// 東京湾平均海面（T.P.）
    TokyoBay,
    /// 離島の独自の平均海面
    Island,
    /// 楕円体（GRS80）。楕円体高
    Ellipsoid,
}

impl HeightDatum {
    /// 診断の値に入れる名前
    pub fn key(self) -> &'static str {
        match self {
            HeightDatum::TokyoBay => "tokyo-bay",
            HeightDatum::Island => "island",
            HeightDatum::Ellipsoid => "ellipsoid",
        }
    }
}

/// 標高の成果
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Realization {
    /// 測地成果2011（2025-03-31 まで）
    Survey2011,
    /// 測地成果2024（2025-04-01 から）
    Survey2024,
}

impl Realization {
    /// 診断の値に入れる名前
    pub fn key(self) -> &'static str {
        match self {
            Realization::Survey2011 => "2011",
            Realization::Survey2024 => "2024",
        }
    }

    /// 形式が書いた成果。書かれていなければ `assumed` を仮定し、`crs.height-realization.assumed` を返す
    pub fn or_assume(
        written: Option<Realization>,
        assumed: Realization,
        at: Ref,
        cx: &mut Cx,
    ) -> Realization {
        written.unwrap_or_else(|| {
            cx.report(
                Diagnostic::new(codes::CRS_HEIGHT_REALIZATION_ASSUMED)
                    .with("realization", assumed.key())
                    .at(at),
            );
            assumed
        })
    }
}

/// 高さ（m）と、その基準と成果
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Height {
    /// 高さ（m）。上を正にする
    pub value: f64,
    /// 基準
    pub datum: HeightDatum,
    /// 成果
    pub realization: Realization,
}

impl Height {
    /// `self` から `other` を引いた差（m）。
    ///
    /// 基準が違えば差を求めず、`crs.height-datum.unsupported` を返して `None` にする（楕円体高と標高の差は
    /// ジオイド高で、引き算では決まらない）。成果が違えば差を求め、`crs.height-realization.mixed` を返す
    pub fn difference(&self, other: &Height, at: Ref, cx: &mut Cx) -> Option<f64> {
        if self.datum != other.datum {
            cx.report(
                Diagnostic::new(codes::CRS_HEIGHT_DATUM_UNSUPPORTED)
                    .with("first", self.datum.key())
                    .with("second", other.datum.key())
                    .at(at),
            );
            return None;
        }
        if self.realization != other.realization {
            cx.report(
                Diagnostic::new(codes::CRS_HEIGHT_REALIZATION_MIXED)
                    .with("first", self.realization.key())
                    .with("second", other.realization.key())
                    .at(at),
            );
        }
        Some(self.value - other.value)
    }
}
