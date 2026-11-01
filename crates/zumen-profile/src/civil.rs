//! 土木の図（決定 0231）の作図の値を、プロファイルの語彙から読む（決定 0239）。
//!
//! 語彙 `civil-drawing` の 1 つ目の値に、平面図・縦断図の横と縦・横断図の尺度の分母と、測点の間隔（メートル）
//! を書く。文字の高さ（mm）は、語彙 `text-height` の値から選ぶ。書いていない値は、呼ぶ側の既定のままにする。

use zumen_base::{Cx, Diagnostic, codes};
use zumen_json::Value;

use crate::profile::{self, GetKey};
use crate::{Error, Identity, number};

/// 作図の値の語彙の名前
pub const CIVIL_DRAWING: &str = "civil-drawing";

/// 土木の図の作図の値。書いていないものは `None`
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CivilDrawing {
    /// 平面図の尺度の分母
    pub plan: Option<f64>,
    /// 縦断図の横の尺度の分母
    pub horizontal: Option<f64>,
    /// 縦断図の縦の尺度の分母
    pub vertical: Option<f64>,
    /// 横断図の尺度の分母
    pub cross_section: Option<f64>,
    /// 測点の間隔（メートル）
    pub station_pitch: Option<f64>,
    /// 語彙 `text-height` の値（mm）。小さい順
    pub text_heights: Vec<f64>,
}

/// `profiles`（入力の名前とバイト列）から `id` と `edition` のプロファイルを選び、`extends` を解いて、土木の
/// 図の作図の値を読む。値が正の有限の数でなければ、`profile.vocabulary.invalid` で失敗にする
pub fn civil_drawing(
    profiles: &[(&str, &[u8])],
    id: &str,
    edition: &str,
    cx: &mut Cx<'_>,
) -> Result<(Vec<Identity>, CivilDrawing), Error> {
    let resolved = profile::resolve(profiles, id, edition, cx)?;
    let invalid = |vocabulary: &str, field: &str| {
        Error::Profile(Box::new(
            Diagnostic::new(codes::PROFILE_VOCABULARY_INVALID)
                .with("vocabulary", vocabulary)
                .with("field", field),
        ))
    };
    let positive = |value: &Value| number(value).filter(|v| v.is_finite() && *v > 0.0);
    let mut out = CivilDrawing::default();
    if let Some(vocabulary) = resolved.vocabularies.get(CIVIL_DRAWING) {
        let first = match vocabulary.get_key("values") {
            Some(Value::Array(values)) => values.first(),
            _ => None,
        }
        .ok_or_else(|| invalid(CIVIL_DRAWING, "values"))?;
        let field = |name: &str| match first.get_key(name) {
            None => Ok(None),
            Some(value) => positive(value)
                .map(Some)
                .ok_or_else(|| invalid(CIVIL_DRAWING, name)),
        };
        out.plan = field("plan")?;
        out.horizontal = field("horizontal")?;
        out.vertical = field("vertical")?;
        out.cross_section = field("cross-section")?;
        out.station_pitch = field("station-pitch")?;
    }
    if let Some(vocabulary) = resolved.vocabularies.get("text-height") {
        let Some(Value::Array(values)) = vocabulary.get_key("values") else {
            return Err(invalid("text-height", "values"));
        };
        out.text_heights = values
            .iter()
            .map(positive)
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| invalid("text-height", "values"))?;
        out.text_heights.sort_by(f64::total_cmp);
    }
    Ok((resolved.chain, out))
}
