//! 書き手が採用・棄却した項目の集計。出所が同じ別の項目も、それぞれ数える。

use std::collections::BTreeMap;

use zumen_base::{Cx, Kind, Ref};
use zumen_sdk::Error;

/// 書き出した値と参照の忠実度。元のファイルのバイトの保持とは区別する
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConversionFidelity {
    /// 同系統の値と参照を保持した
    Native,
    /// 別の表現で同じ意味を持つ
    Equivalent,
    /// 許容差付きの近似
    Approximated,
    /// 見た目のための形にした
    Drawn,
    /// 書かなかった
    Dropped,
}

/// 種類と忠実度ごとの件数
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversionCount {
    /// 書き手の項目の種類
    pub kind: String,
    /// 項目の忠実度
    pub fidelity: ConversionFidelity,
    /// 入力の項目の数
    pub count: u64,
    /// その項目から書いた図形の数。表・用紙・ビューなどの管理項目は 0
    pub output_elements: u64,
}

/// 書き手の項目 1 つの結果
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversionDetail {
    /// 書き手の項目の種類
    pub kind: String,
    /// 項目の忠実度
    pub fidelity: ConversionFidelity,
    /// 元の項目の出所
    pub source: Ref,
    /// この項目から書いた図形の数
    pub output_elements: u64,
    /// 変えた値や参照の種類。文言を含まないキー
    pub reasons: Vec<String>,
}

/// 全件の集計と、上限までの明細。診断の件数とは別に数える
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConversionReport {
    /// 入力の項目の合計
    pub total: u64,
    /// 種類と忠実度の順の全件集計
    pub counts: Vec<ConversionCount>,
    /// 書き手が結果を確定した順の明細
    pub details: Vec<ConversionDetail>,
    /// 上限のため明細に含めなかった項目がある
    pub truncated: bool,
}

pub(crate) struct Report {
    limit: u64,
    total: u64,
    counts: BTreeMap<(String, ConversionFidelity), (u64, u64)>,
    details: Vec<ConversionDetail>,
}

impl Report {
    pub(crate) fn new(limit: u64) -> Self {
        Self {
            limit,
            total: 0,
            counts: BTreeMap::new(),
            details: Vec::new(),
        }
    }

    pub(crate) fn record(
        &mut self,
        kind: &str,
        fidelity: ConversionFidelity,
        source: &Ref,
        output_elements: u64,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        self.record_with_reasons(kind, fidelity, source, output_elements, &[], cx)
    }

    pub(crate) fn record_with_reasons(
        &mut self,
        kind: &str,
        fidelity: ConversionFidelity,
        source: &Ref,
        output_elements: u64,
        reasons: &[&str],
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        self.total += 1;
        let count = self.counts.entry((kind.to_owned(), fidelity)).or_default();
        count.0 += 1;
        count.1 += output_elements;
        if (self.details.len() as u64) < self.limit {
            // 環境のポインター幅に依存する size_of は使わない。参照と各文字列の管理には固定の論理量を数える。
            let bytes = 256_u64
                .saturating_add(kind.len() as u64)
                .saturating_add(source.id().map_or(0, |id| id.len() as u64))
                .saturating_add(
                    reasons
                        .iter()
                        .map(|reason| reason.len() as u64)
                        .sum::<u64>(),
                );
            for (kind, amount) in [(Kind::DecodedBytes, bytes), (Kind::Work, bytes)] {
                cx.check(kind, amount)?;
            }
            self.details.push(ConversionDetail {
                kind: kind.to_owned(),
                fidelity,
                source: source.clone(),
                output_elements,
                reasons: reasons.iter().map(|reason| (*reason).to_owned()).collect(),
            });
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> ConversionReport {
        ConversionReport {
            total: self.total,
            counts: self
                .counts
                .into_iter()
                .map(
                    |((kind, fidelity), (count, output_elements))| ConversionCount {
                        kind,
                        fidelity,
                        count,
                        output_elements,
                    },
                )
                .collect(),
            truncated: self.total > self.details.len() as u64,
            details: self.details,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};

    #[test]
    fn duplicate_sources_are_distinct_and_the_limit_only_truncates_details() {
        let run = |limit| {
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
            let mut report = Report::new(limit);
            let source = Ref::at_record(7).with_id("same");
            report
                .record("line", ConversionFidelity::Equivalent, &source, 1, &mut cx)
                .unwrap();
            report
                .record("line", ConversionFidelity::Equivalent, &source, 2, &mut cx)
                .unwrap();
            report.finish()
        };
        let full = run(10);
        assert_eq!(full.total, 2);
        assert_eq!(full.counts[0].count, 2);
        assert_eq!(full.counts[0].output_elements, 3);
        assert_eq!(full.details.len(), 2);
        assert_eq!(full.details[0].source, full.details[1].source);
        assert!(!full.truncated);
        for limit in [0, 1] {
            let short = run(limit);
            assert_eq!(short.counts, full.counts);
            assert_eq!(short.total, full.total);
            assert_eq!(short.details.len(), limit as usize);
            assert!(short.truncated);
        }
    }

    #[test]
    fn detail_copy_checks_a_fixed_logical_budget_before_allocating() {
        let mut budget =
            Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(255)));
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let mut report = Report::new(1);
        assert!(matches!(
            report.record(
                "line",
                ConversionFidelity::Equivalent,
                &Ref::default(),
                1,
                &mut cx
            ),
            Err(Error::Stopped(_))
        ));
        assert!(report.details.is_empty());
        assert_eq!(diagnostics.iter().count(), 0);
    }
}
