//! 表示部品のない寸法に使う値。元のスタイルの設定と寸法ごとの上書きを保つ。

use std::cmp::Ordering;
use std::collections::BTreeMap;

use zumen_base::{Cx, Kind, Ref, Stop};

use crate::{Drawing2D, Hash};

/// 寸法の表示に使う設定。値の出所は対象の寸法スタイルか要素からたどる。
#[derive(Clone, Debug, PartialEq)]
pub struct DimensionDisplayRecord {
    /// 設定が付く先。
    pub target: DimensionDisplayTarget,
    /// 設定にある値だけを持つ。
    pub values: DimensionDisplayValues,
}

/// 部品のない寸法に使う表示設定の対象。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DimensionDisplayTarget {
    /// 寸法スタイルの名前。既存の寸法スタイルと同じく ASCII の大文字と小文字を区別しない。
    Style(String),
    /// 要素だけの上書き。読んだ後で同じ ID がほかの値を指せば使わない。
    Element {
        /// 図面の要素の ID。
        id: String,
        /// 読んだときの要素のハッシュ。
        basis: Hash,
    },
}

/// 長さは倍率を掛ける前の図面の単位。値がなければ、上位の設定を継承する。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DimensionDisplayValues {
    /// 端部・補助線・文字に掛ける倍率。要素で上書きしないときは寸法スタイルの倍率。
    pub scale: Option<f64>,
    /// 矢印の大きさ。要素で上書きしないときは寸法スタイルの矢印の大きさ。
    pub arrow_size: Option<f64>,
    /// 斜線目盛りの大きさ。明示した 0 は斜線を使わない。
    pub tick_size: Option<f64>,
    /// 補助線の始まりと定義点の間隔。
    pub extension_offset: Option<f64>,
    /// 補助線が寸法線を越えて延びる長さ。
    pub extension_beyond: Option<f64>,
    /// 最初の補助線を省くか。
    pub suppress_first_extension: Option<bool>,
    /// 2 本目の補助線を省くか。
    pub suppress_second_extension: Option<bool>,
    /// 寸法文字の高さ。
    pub text_height: Option<f64>,
    /// 表示する長さに掛ける倍率。
    pub linear_factor: Option<f64>,
    /// 表示する長さの小数の桁数。
    pub linear_precision: Option<u8>,
    /// 寸法文字のスタイル名。
    pub text_style: Option<String>,
    /// 寸法線に対する文字の垂直配置。DXF の DIMTAD に対応する 0 から 4。
    pub text_vertical_placement: Option<u8>,
    /// 寸法文字と線のすき間。DXF の DIMGAP は負の値で文字枠も指定する。
    pub text_gap: Option<f64>,
}

/// 基本の寸法スタイルと個別の上書きを重ねた、部品のない寸法の表示設定。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ResolvedDimensionDisplay<'a> {
    /// 要素で個別に指定した倍率。
    pub scale: Option<f64>,
    /// 要素で個別に指定した矢印の大きさ。
    pub arrow_size: Option<f64>,
    /// 要素で個別に指定した斜線目盛りの大きさ。
    pub tick_size: Option<f64>,
    /// 補助線の始まりと定義点の間隔。
    pub extension_offset: Option<f64>,
    /// 寸法線を越えて延びる長さ。
    pub extension_beyond: Option<f64>,
    /// 最初の補助線を省くか。
    pub suppress_first_extension: Option<bool>,
    /// 2 本目の補助線を省くか。
    pub suppress_second_extension: Option<bool>,
    /// 寸法文字の高さ。
    pub text_height: Option<f64>,
    /// 表示する長さに掛ける倍率。
    pub linear_factor: Option<f64>,
    /// 表示する長さの小数の桁数。
    pub linear_precision: Option<u8>,
    /// 寸法文字のスタイル名。
    pub text_style: Option<&'a str>,
    /// 寸法線に対する文字の垂直配置。
    pub text_vertical_placement: Option<u8>,
    /// 寸法文字と線のすき間。負の値では文字枠を指定する。
    pub text_gap: Option<f64>,
}

// 照合用の名前を複製せず、元の文字列の ASCII だけを小文字として比較する。
#[derive(Clone, Copy)]
struct StyleName<'a>(&'a str);

impl Ord for StyleName<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0
            .bytes()
            .map(|byte| byte.to_ascii_lowercase())
            .cmp(other.0.bytes().map(|byte| byte.to_ascii_lowercase()))
    }
}
impl PartialOrd for StyleName<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Eq for StyleName<'_> {}
impl PartialEq for StyleName<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_ignore_ascii_case(other.0)
    }
}

/// 名前と要素の ID から表示設定を引く索引。重複する対象は図面にある最初の設定を使う。
#[derive(Default)]
pub struct DimensionDisplayIndex<'a> {
    styles: BTreeMap<StyleName<'a>, &'a DimensionDisplayValues>,
    elements: BTreeMap<&'a str, (Hash, &'a DimensionDisplayValues)>,
}

fn lookup_work(entries: usize, key: &str) -> u64 {
    // 木の高さ、節にあるキーとの比較、調べる名前の長さの分を数える。
    let height = u64::from(entries.max(1).ilog2()) + 1;
    height
        .saturating_mul(12)
        .saturating_mul(1 + key.len() as u64 / 16)
}

impl Drawing2D {
    /// 寸法の表示設定を記録順に訪問し、変換の明細で使う種類と出所を渡す。
    /// スタイルは名前が一致する最初の定義を指す。個別の設定は記録された要素 ID を指す。
    pub fn each_dimension_display_source<E: From<Stop>>(
        &self,
        cx: &mut Cx<'_>,
        mut visit: impl FnMut(&'static str, &Ref, &mut Cx<'_>) -> Result<(), E>,
    ) -> Result<(), E> {
        if self.dimension_display.is_empty() {
            return Ok(());
        }
        let mut styles = BTreeMap::new();
        cx.check(Kind::Work, self.dimension_display.len() as u64)?;
        if self
            .dimension_display
            .iter()
            .any(|r| matches!(r.target, DimensionDisplayTarget::Style(_)))
        {
            for style in &self.dimension_styles {
                cx.check(Kind::Work, lookup_work(styles.len(), &style.name))?;
                cx.check(Kind::DecodedBytes, 64)?;
                styles
                    .entry(StyleName(&style.name))
                    .or_insert(&style.source);
            }
        }
        let empty = Ref::default();
        for record in &self.dimension_display {
            match &record.target {
                DimensionDisplayTarget::Style(name) => {
                    cx.check(Kind::Work, lookup_work(styles.len(), name))?;
                    let source = styles.get(&StyleName(name)).copied().unwrap_or(&empty);
                    visit("dimension_display_style", source, cx)?;
                }
                DimensionDisplayTarget::Element { id, .. } => {
                    cx.check(Kind::Work, 1 + id.len() as u64 / 16)?;
                    cx.check(Kind::DecodedBytes, id.len() as u64)?;
                    visit("dimension_display_target", &Ref::default().with_id(id), cx)?;
                }
            }
        }
        Ok(())
    }
}

impl<'a> DimensionDisplayIndex<'a> {
    /// 1 回の走査で両方の索引を作る。図面と同じ文字列を借りる。
    pub fn new(records: &'a [DimensionDisplayRecord], cx: &mut Cx<'_>) -> Result<Self, Stop> {
        let mut index = Self::default();
        for record in records {
            // 木の節・キーの参照・値への参照と、要素の場合のハッシュを含む論理バイト数。
            cx.check(Kind::DecodedBytes, 64)?;
            match &record.target {
                DimensionDisplayTarget::Style(name) => {
                    cx.check(Kind::Work, lookup_work(index.styles.len(), name))?;
                    index
                        .styles
                        .entry(StyleName(name))
                        .or_insert(&record.values);
                }
                DimensionDisplayTarget::Element { id, basis } => {
                    cx.check(Kind::Work, lookup_work(index.elements.len(), id))?;
                    index.elements.entry(id).or_insert((*basis, &record.values));
                }
            }
        }
        Ok(index)
    }

    /// スタイルに記録された値を返す。文書をまたいで定義を照合するときにも使う。
    pub fn style_values(
        &self,
        name: &str,
        cx: &mut Cx<'_>,
    ) -> Result<Option<&'a DimensionDisplayValues>, Stop> {
        if self.styles.is_empty() {
            return Ok(None);
        }
        cx.check(Kind::Work, lookup_work(self.styles.len(), name))?;
        Ok(self.styles.get(&StyleName(name)).copied())
    }

    /// 現在の要素と基準ハッシュが一致する個別の上書きを返す。
    pub fn element_values(
        &self,
        id: &str,
        element_hash: impl FnOnce(&mut Cx<'_>) -> Result<Hash, Stop>,
        cx: &mut Cx<'_>,
    ) -> Result<Option<&'a DimensionDisplayValues>, Stop> {
        if self.elements.is_empty() {
            return Ok(None);
        }
        cx.check(Kind::Work, lookup_work(self.elements.len(), id))?;
        Ok(match self.elements.get(id) {
            Some((basis, values)) if *basis == element_hash(cx)? => Some(*values),
            _ => None,
        })
    }

    /// 個別の指定がある要素だけハッシュを調べる。設定がなければ従来の近似に使う `None` を返す。
    /// 不正な実数や桁数は、その項目だけ上書きを使わず、基本の設定から探す。
    pub fn resolve(
        &self,
        style: &str,
        id: &str,
        element_hash: impl FnOnce(&mut Cx<'_>) -> Result<Hash, Stop>,
        cx: &mut Cx<'_>,
    ) -> Result<Option<ResolvedDimensionDisplay<'a>>, Stop> {
        if self.styles.is_empty() && self.elements.is_empty() {
            return Ok(None);
        }
        let base = self.style_values(style, cx)?;
        let local = self.element_values(id, element_hash, cx)?;
        if base.is_none() && local.is_none() {
            return Ok(None);
        }
        Ok(Some(ResolvedDimensionDisplay::from_records(base, local)))
    }
}

impl<'a> ResolvedDimensionDisplay<'a> {
    /// 対象を確認したスタイルと個別の記録を重ね、その項目で使える値を返す。
    /// 片方だけ渡せば、読み書きと描画で同じ条件を使って個々の記録を解釈できる。
    pub fn from_records(
        base: Option<&'a DimensionDisplayValues>,
        local: Option<&'a DimensionDisplayValues>,
    ) -> Self {
        let empty = DimensionDisplayValues::default();
        let style_name = local
            .and_then(|v| v.text_style.as_deref())
            .or_else(|| base.and_then(|v| v.text_style.as_deref()));
        let base = base.unwrap_or(&empty);
        let local = local.unwrap_or(&empty);
        let nonnegative = |v: &f64| v.is_finite() && *v >= 0.0;
        let positive = |v: &f64| v.is_finite() && *v > 0.0;
        let nonzero = |v: &f64| v.is_finite() && *v != 0.0;
        let precision = |v: &u8| *v <= 8;
        Self {
            // これらの共通値は既存の DimensionStyle にあるため、個別の上書きだけを返す。
            scale: local.scale.filter(nonnegative),
            arrow_size: local.arrow_size.filter(nonnegative),
            tick_size: local.tick_size.filter(nonnegative),
            extension_offset: local
                .extension_offset
                .filter(nonnegative)
                .or_else(|| base.extension_offset.filter(nonnegative)),
            extension_beyond: local
                .extension_beyond
                .filter(nonnegative)
                .or_else(|| base.extension_beyond.filter(nonnegative)),
            suppress_first_extension: local
                .suppress_first_extension
                .or(base.suppress_first_extension),
            suppress_second_extension: local
                .suppress_second_extension
                .or(base.suppress_second_extension),
            text_height: local
                .text_height
                .filter(positive)
                .or_else(|| base.text_height.filter(positive)),
            linear_factor: local
                .linear_factor
                .filter(nonzero)
                .or_else(|| base.linear_factor.filter(nonzero)),
            linear_precision: local
                .linear_precision
                .filter(precision)
                .or_else(|| base.linear_precision.filter(precision)),
            text_style: style_name,
            text_vertical_placement: local
                .text_vertical_placement
                .filter(|v| *v <= 4)
                .or_else(|| base.text_vertical_placement.filter(|v| *v <= 4)),
            text_gap: local
                .text_gap
                .filter(|v| v.is_finite())
                .or_else(|| base.text_gap.filter(|v| v.is_finite())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};

    #[test]
    fn resolves_only_current_overrides_and_keeps_explicit_zero_and_false() {
        let basis = Hash([4; 16]);
        let records = [
            DimensionDisplayRecord {
                target: DimensionDisplayTarget::Style("測点A".to_owned()),
                values: DimensionDisplayValues {
                    text_height: Some(0.25),
                    linear_precision: Some(2),
                    suppress_first_extension: Some(true),
                    text_style: Some("Labels".to_owned()),
                    text_vertical_placement: Some(1),
                    text_gap: Some(0.1),
                    ..Default::default()
                },
            },
            DimensionDisplayRecord {
                target: DimensionDisplayTarget::Element {
                    id: "space/0/h8A".to_owned(),
                    basis,
                },
                values: DimensionDisplayValues {
                    tick_size: Some(0.0),
                    linear_precision: Some(15),
                    text_height: Some(f64::NAN),
                    text_vertical_placement: Some(8),
                    text_gap: Some(-0.25),
                    suppress_first_extension: Some(false),
                    ..Default::default()
                },
            },
        ];
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let index = DimensionDisplayIndex::new(&records, &mut cx).unwrap();
        let base = ResolvedDimensionDisplay {
            text_height: Some(0.25),
            linear_precision: Some(2),
            suppress_first_extension: Some(true),
            text_style: Some("Labels"),
            text_vertical_placement: Some(1),
            text_gap: Some(0.1),
            ..Default::default()
        };
        assert_eq!(
            index.resolve("測点a", "space/0/h8A", |_| Ok(basis), &mut cx),
            Ok(Some(ResolvedDimensionDisplay {
                tick_size: Some(0.0),
                text_gap: Some(-0.25),
                suppress_first_extension: Some(false),
                ..base
            }))
        );
        assert_eq!(
            index.resolve("測点a", "space/0/h8A", |_| Ok(Hash([5; 16])), &mut cx),
            Ok(Some(base))
        );
        assert_eq!(
            index.resolve("測点a", "space/0/h8B", |_| Err(Stop::Cancelled), &mut cx),
            Ok(Some(base))
        );
        assert_eq!(
            index.resolve("Absent", "", |_| Err(Stop::Cancelled), &mut cx),
            Ok(None)
        );

        let mut limited = Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(0)));
        let mut cx = Cx::new(&mut limited, &mut diagnostics, &cancel);
        assert!(DimensionDisplayIndex::new(&records, &mut cx).is_err());
        let mut limited = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(0)));
        let mut cx = Cx::new(&mut limited, &mut diagnostics, &cancel);
        assert!(
            index
                .resolve("測点a", "space/0/h8A", |_| Ok(basis), &mut cx)
                .is_err()
        );
    }
}
