//! 書き手が採用・棄却した項目の集計。出所が同じ別の項目も、それぞれ数える。

use std::collections::BTreeMap;

use crate::Error;
use zumen_base::{Cx, Kind, Ref};
/// 全件をまとめて試し、断られた範囲だけ二分して、書ける項目を受け入れる。
/// run は範囲全体を検証し、Rejected のときは一切反映しないこと。
/// 再試行の走査・複製は run 側でも予算を数える。中断・内部の失敗は部分結果を返さない。
/// 試行の診断は捨てる。呼び出し元が棄却と停止の理由を一度だけ報告する。
pub fn batches(
    count: usize,
    mut run: impl FnMut(core::ops::Range<usize>, &mut Cx<'_>) -> Result<(), Error>,
    cx: &mut Cx<'_>,
) -> Result<Vec<bool>, Error> {
    cx.check(Kind::DecodedBytes, (count as u64).saturating_add(2048))?;
    let mut accepted = vec![false; count];
    let mut pending = vec![(0, count)];
    while let Some((start, end)) = pending.pop() {
        cx.check(Kind::Work, 1)?;
        if start == end {
            continue;
        }
        let result = {
            let mut diagnostics = zumen_base::Diagnostics::new();
            run(
                start..end,
                &mut Cx::new(cx.budget, &mut diagnostics, cx.cancel),
            )
        };
        match result {
            Ok(()) => {
                cx.check(Kind::Work, (end - start) as u64)?;
                accepted
                    .get_mut(start..end)
                    .ok_or(Error::Invalid)?
                    .fill(true);
            }
            Err(Error::Rejected) if end - start > 1 => {
                let middle = start + (end - start) / 2;
                pending.push((middle, end));
                pending.push((start, middle));
            }
            Err(Error::Rejected) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(accepted)
}

/// 変換で保持しなかった項目と値を、集計とカタログ診断の双方へ記録する。
#[allow(clippy::too_many_arguments)] // 集計行と診断の同じ値を一度だけ渡す。
pub fn note(
    report: &mut Report,
    format: &str,
    kind: &'static str,
    fidelity: ConversionFidelity,
    source: &Ref,
    elements: u64,
    reasons: &[&'static str],
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    use zumen_base::{Diagnostic, Value, codes};
    report.record_with_reasons(kind, fidelity, source, elements, reasons, cx)?;
    let count = if fidelity == ConversionFidelity::Dropped {
        1
    } else {
        reasons.len() as u64
    };
    let bytes = (512_u64
        .saturating_add(format.len() as u64)
        .saturating_add(source.id().map_or(0, |id| id.len() as u64)))
    .saturating_mul(count);
    cx.check(Kind::Work, bytes)?;
    cx.check(Kind::DecodedBytes, bytes)?;
    if fidelity == ConversionFidelity::Dropped {
        cx.report(
            Diagnostic::new(codes::CONVERT_ELEMENT_UNSUPPORTED)
                .with("format", format)
                .with("kind", Value::Key(kind))
                .at(source.clone()),
        );
    } else {
        for reason in reasons {
            cx.report(
                Diagnostic::new(if fidelity == ConversionFidelity::Approximated {
                    codes::CONVERT_ITEM_APPROXIMATED
                } else {
                    codes::CONVERT_ITEM_DRAWN
                })
                .with("format", format)
                .with("kind", Value::Key(kind))
                .with("reason", Value::Key(reason))
                .at(source.clone()),
            );
        }
    }
    Ok(())
}

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

/// 忠実度の件数と上限付きの明細を、予算内で集める。
pub struct Report {
    limit: u64,
    total: u64,
    counts: BTreeMap<String, BTreeMap<ConversionFidelity, (u64, u64)>>,
    details: Vec<ConversionDetail>,
}

impl Report {
    /// 明細の上限を指定する。集計は上限後も続ける。
    pub fn new(limit: u64) -> Self {
        Self {
            limit,
            total: 0,
            counts: BTreeMap::new(),
            details: Vec::new(),
        }
    }

    /// 項目を 1 件数える。
    pub fn record(
        &mut self,
        kind: &str,
        fidelity: ConversionFidelity,
        source: &Ref,
        output_elements: u64,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        self.record_with_reasons(kind, fidelity, source, output_elements, &[], cx)
    }

    /// 変えた値のキーを付けて、項目を 1 件数える。
    pub fn record_with_reasons(
        &mut self,
        kind: &str,
        fidelity: ConversionFidelity,
        source: &Ref,
        output_elements: u64,
        reasons: &[&str],
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        // 上限後も集計の探索と中断の確認を行う。借用で探し、文字列や木の領域は予算を確かめてから作る。
        let search = u64::from(self.counts.len().saturating_add(1).ilog2())
            .saturating_mul(2)
            .saturating_add(kind.len() as u64)
            .saturating_add(1);
        cx.check(Kind::Work, search)?;
        let known = self.counts.get(kind);
        let new_kind = known.is_none();
        let new_fidelity = known.is_none_or(|counts| !counts.contains_key(&fidelity));
        let allocation = if new_kind {
            256_u64.saturating_add(kind.len() as u64)
        } else {
            0
        }
        .saturating_add(if new_fidelity { 256 } else { 0 });
        cx.check(Kind::DecodedBytes, allocation)?;
        if new_kind {
            self.counts.insert(kind.to_owned(), BTreeMap::new());
        }
        self.total += 1;
        let count = self
            .counts
            .get_mut(kind)
            .ok_or(Error::Invalid)?
            .entry(fidelity)
            .or_default();
        count.0 += 1;
        count.1 += output_elements;
        if (self.details.len() as u64) < self.limit {
            // 環境のポインター幅に依存する size_of は使わない。参照と各文字列の管理には固定の論理量を数える。
            let bytes = 256_u64
                .saturating_add(kind.len() as u64)
                .saturating_add(source.id().map_or(0, |id| id.len() as u64))
                .saturating_add((reasons.len() as u64).saturating_mul(32))
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

    /// 種類・忠実度の順で集計を返す。
    pub fn finish(self, cx: &mut Cx<'_>) -> Result<ConversionReport, Error> {
        cx.check(Kind::Work, self.counts.len() as u64)?;
        let mut count = 0_u64;
        let mut names = 0_u64;
        for (kind, fidelities) in &self.counts {
            count = count.saturating_add(fidelities.len() as u64);
            names =
                names.saturating_add((kind.len() as u64).saturating_mul(fidelities.len() as u64));
        }
        // Vec の固定の論理量と、各行に写す種類名のバイト数を確保前に数える。
        cx.check(Kind::Work, count.saturating_add(names))?;
        cx.check(
            Kind::DecodedBytes,
            count.saturating_mul(64).saturating_add(names),
        )?;
        let mut counts = Vec::with_capacity(usize::try_from(count).map_err(|_| Error::Invalid)?);
        for (kind, fidelities) in self.counts {
            for (fidelity, (count, output_elements)) in fidelities {
                counts.push(ConversionCount {
                    kind: kind.clone(),
                    fidelity,
                    count,
                    output_elements,
                });
            }
        }
        Ok(ConversionReport {
            total: self.total,
            counts,
            truncated: self.total > self.details.len() as u64,
            details: self.details,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};

    #[test]
    fn rejected_items_are_isolated_without_duplicates_or_reordering() {
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let mut written = Vec::new();
        let accepted = batches(
            8,
            |range, _| {
                if range.clone().any(|i| i == 2 || i == 5) {
                    return Err(Error::Rejected);
                }
                written.extend(range);
                Ok(())
            },
            &mut cx,
        )
        .unwrap();
        assert_eq!(written, [0, 1, 3, 4, 6, 7]);
        assert_eq!(accepted, [true, true, false, true, true, false, true, true]);
    }

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
            report.finish(&mut cx).unwrap()
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

    #[test]
    fn aggregate_keys_check_the_budget_before_allocating_without_details() {
        let mut budget = Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(0)));
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let mut report = Report::new(0);
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
        assert!(report.counts.is_empty());
        assert_eq!(report.total, 0);
    }

    #[test]
    fn aggregate_work_and_cancellation_are_checked_after_the_detail_limit() {
        for limit in [0, 1] {
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
            let mut report = Report::new(limit);
            report
                .record(
                    "line",
                    ConversionFidelity::Equivalent,
                    &Ref::default(),
                    1,
                    &mut cx,
                )
                .unwrap();
            let before = cx.budget.used(Kind::Work);
            report
                .record(
                    "line",
                    ConversionFidelity::Equivalent,
                    &Ref::default(),
                    1,
                    &mut cx,
                )
                .unwrap();
            assert!(cx.budget.used(Kind::Work) > before);
            cancel.cancel();
            assert!(matches!(
                report.record(
                    "line",
                    ConversionFidelity::Equivalent,
                    &Ref::default(),
                    1,
                    &mut cx
                ),
                Err(Error::Stopped(zumen_base::Stop::Cancelled))
            ));
            assert_eq!(report.total, 2);
        }
    }

    #[test]
    fn the_aggregate_vector_checks_its_budget_before_allocating() {
        let mut budget =
            Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(516)));
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let mut report = Report::new(0);
        report
            .record(
                "line",
                ConversionFidelity::Equivalent,
                &Ref::default(),
                1,
                &mut cx,
            )
            .unwrap();
        assert!(matches!(report.finish(&mut cx), Err(Error::Stopped(_))));
    }
}
