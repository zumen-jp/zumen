//! 予算・診断・中断の合図を運ぶ文脈（決定 0008）。

use crate::budget::{Budget, Exceeded, Kind};
use crate::cancel::CancelToken;
use crate::codes;
use crate::diag::{Diagnostic, Diagnostics};

/// 作業をする関数すべてに、明示的に渡す文脈。thread_local の上限の代わり
#[derive(Debug)]
pub struct Cx<'a> {
    /// 予算
    pub budget: &'a mut Budget,
    /// 診断
    pub diag: &'a mut Diagnostics,
    /// 中断の合図
    pub cancel: &'a CancelToken,
}

/// 処理を止める理由
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// 予算の上限を超えた
    Exceeded(Exceeded),
    /// 中断の合図を受けた
    Cancelled,
    /// 時間の上限に達した。止まる所は環境で変わる（決定 0010）
    TimedOut,
}

impl Stop {
    /// この理由を表す診断
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            Stop::Exceeded(exceeded) => exceeded.to_diagnostic(),
            Stop::Cancelled => Diagnostic::new(codes::CALL_REQUEST_CANCELLED),
            Stop::TimedOut => Diagnostic::new(codes::BUDGET_TIME_EXCEEDED),
        }
    }
}

impl From<Exceeded> for Stop {
    fn from(exceeded: Exceeded) -> Self {
        Stop::Exceeded(exceeded)
    }
}

impl<'a> Cx<'a> {
    /// 文脈を作る
    pub fn new(budget: &'a mut Budget, diag: &'a mut Diagnostics, cancel: &'a CancelToken) -> Self {
        Self {
            budget,
            diag,
            cancel,
        }
    }

    /// コードの決まった所で呼ぶ。中断の合図を確かめてから、予算を使う
    pub fn check(&mut self, kind: Kind, amount: u64) -> Result<(), Stop> {
        if let Some(stop) = self.diag.record_order_stop() {
            return Err(stop);
        }
        if let Some(stop) = self.cancel.stop() {
            return Err(stop);
        }
        self.budget.spend(kind, amount).map_err(Stop::from)
    }

    /// 入れ子に 1 つ入る
    pub fn enter(&mut self) -> Result<(), Stop> {
        self.check(Kind::Depth, 1)
    }

    /// 入れ子を 1 つ出る
    pub fn leave(&mut self) {
        self.budget.leave(1);
    }

    /// 診断を足す
    pub fn report(&mut self, diagnostic: Diagnostic) {
        if self.diag.record_order_stop().is_some() {
            return;
        }
        if let Some(amount) = self.diag.record_order_work(&diagnostic)
            && let Err(stop) = self.check(Kind::Work, amount)
        {
            self.diag.stop_record_order(stop, diagnostic);
            return;
        }
        self.diag.report(diagnostic);
    }

    /// 処理中の診断を、レコードの入力順でまとめてから届ける（決定 0009）。
    ///
    /// 診断は [`Cx::report`] で足す。処理中の `diag.iter()` は届いた順、返した後は入力順になる。
    /// レコードのない診断は最後に、同じレコードでは届いた順にする。値も先頭の診断から採る。
    /// 場所は収集中も `max_places` 件までにし、0 件でも診断の順を保つ。
    /// 収集・場所の移動・最後の整列と転送を Work に数える。途中で止まっても、収集済みの診断は届ける。
    pub fn with_diagnostics_in_record_order<T, E: From<Stop>>(
        &mut self,
        operation: impl FnOnce(&mut Cx<'_>) -> Result<T, E>,
    ) -> Result<T, E> {
        let mut diagnostics = Diagnostics::in_record_order(self.diag.max_places());
        let result = operation(&mut Cx::new(self.budget, &mut diagnostics, self.cancel));
        let stop = diagnostics.record_order_stop();
        for diagnostic in diagnostics.into_vec() {
            self.report(diagnostic);
        }
        match stop {
            Some(stop) => Err(stop.into()),
            None => result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::{Limit, Limits};

    #[test]
    fn checks_the_signal_before_the_budget() {
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        assert_eq!(cx.check(Kind::Records, 1), Ok(()));
        cancel.cancel();
        assert_eq!(cx.check(Kind::Records, 1), Err(Stop::Cancelled));
        assert_eq!(budget.used(Kind::Records), 1);
    }

    #[test]
    fn nesting_stops_at_the_depth_limit() {
        let mut budget = Budget::new(Limits::default().with(Kind::Depth, Limit::Finite(1)));
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        assert_eq!(cx.enter(), Ok(()));
        let stop = cx.enter().unwrap_err();
        cx.report(stop.to_diagnostic());
        cx.leave();
        assert_eq!(
            diag.iter().next().map(Diagnostic::code),
            Some(codes::BUDGET_DEPTH_EXCEEDED)
        );
    }

    #[test]
    fn each_stop_has_its_code() {
        assert_eq!(
            Stop::Cancelled.to_diagnostic().code(),
            codes::CALL_REQUEST_CANCELLED
        );
        assert_eq!(
            Stop::TimedOut.to_diagnostic().code(),
            codes::BUDGET_TIME_EXCEEDED
        );
    }

    #[test]
    fn record_order_keeps_first_places_values_and_stable_ties_even_without_places() {
        for max_places in [0, 1, 2, 16] {
            let mut budget = Budget::default();
            let mut diag = Diagnostics::with_max_places(max_places);
            let cancel = CancelToken::new();
            Cx::new(&mut budget, &mut diag, &cancel)
                .with_diagnostics_in_record_order(|cx| {
                    cx.report(
                        Diagnostic::new(codes::ZIP_PATH_UNSAFE)
                            .with("name", "late")
                            .at(crate::Ref::at_record(9)),
                    );
                    cx.report(
                        Diagnostic::new(codes::SXF_P21_INSTANCE_DROPPED)
                            .with("id", 2_u64)
                            .at(crate::Ref::at_record(4)),
                    );
                    cx.report(
                        Diagnostic::new(codes::ZIP_PATH_UNSAFE)
                            .with("name", "first")
                            .at(crate::Ref::at_record(4)),
                    );
                    cx.report(
                        Diagnostic::new(codes::SXF_P21_INSTANCE_DROPPED)
                            .with("id", 1_u64)
                            .at(crate::Ref::at_record(4)),
                    );
                    cx.report(
                        Diagnostic::new(codes::ZIP_PATH_UNSAFE)
                            .with("name", "middle")
                            .at(crate::Ref::at_record(7)),
                    );
                    // 収集中も場所の上限を超えない。
                    assert!(cx.diag.iter().all(|d| d.places().len() <= max_places));
                    Ok::<(), Stop>(())
                })
                .unwrap();
            let diagnostics = diag.into_vec();
            assert_eq!(diagnostics.len(), 3);
            assert_eq!(diagnostics[0].param("id"), Some(&crate::Value::UInt(2)));
            assert_eq!(diagnostics[2].param("id"), Some(&crate::Value::UInt(1)));
            let merged = &diagnostics[1];
            assert_eq!(merged.code(), codes::ZIP_PATH_UNSAFE);
            assert_eq!(merged.count(), 3);
            assert_eq!(merged.param("name"), Some(&crate::Value::from("first")));
            assert_eq!(
                merged.places(),
                &[
                    crate::Ref::at_record(4),
                    crate::Ref::at_record(7),
                    crate::Ref::at_record(9)
                ][..max_places.min(3)]
            );
        }
    }

    #[test]
    fn record_order_stops_before_collecting_and_flushes_already_paid_diagnostics() {
        for cancelled in [false, true] {
            let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(5)));
            let mut diag = Diagnostics::with_max_places(1);
            let cancel = CancelToken::new();
            let result =
                Cx::new(&mut budget, &mut diag, &cancel).with_diagnostics_in_record_order(|cx| {
                    cx.report(Diagnostic::new(codes::ZIP_PATH_UNSAFE).at(crate::Ref::at_record(9)));
                    if cancelled {
                        cancel.cancel();
                    }
                    cx.report(Diagnostic::new(codes::ZIP_PATH_UNSAFE).at(crate::Ref::at_record(3)));
                    // report で見つけた停止は次の check にも返し、停止の二重報告をしない。
                    let stop = cx.check(Kind::Work, 0).unwrap_err();
                    cx.report(stop.to_diagnostic());
                    Ok::<(), Stop>(())
                });
            let stop = result.unwrap_err();
            assert_eq!(cancelled, stop == Stop::Cancelled);
            assert_eq!(budget.used(Kind::Work), 5);
            let diagnostics = diag.into_vec();
            assert_eq!(diagnostics.len(), 2);
            assert_eq!(diagnostics[0].code(), stop.to_diagnostic().code());
            assert_eq!(diagnostics[0].places(), [crate::Ref::at_record(3)]);
            assert_eq!(diagnostics[1].count(), 1);
            assert_eq!(diagnostics[1].places(), [crate::Ref::at_record(9)]);
        }
    }

    #[test]
    fn ordinary_reporting_neither_reorders_nor_spends_work() {
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(0)));
        let mut diag = Diagnostics::with_max_places(1);
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        for record in [9, 3] {
            cx.report(Diagnostic::new(codes::ZIP_PATH_UNSAFE).at(crate::Ref::at_record(record)));
        }
        assert_eq!(budget.used(Kind::Work), 0);
        assert_eq!(
            diag.iter().next().unwrap().places(),
            [crate::Ref::at_record(9)]
        );
        assert_eq!(diag.iter().next().unwrap().count(), 2);
    }

    #[test]
    fn repeated_reports_keep_bounded_storage() {
        for max_places in [0, 1] {
            let mut budget = Budget::default();
            let mut diag = Diagnostics::with_max_places(max_places);
            let cancel = CancelToken::new();
            Cx::new(&mut budget, &mut diag, &cancel)
                .with_diagnostics_in_record_order(|cx| {
                    for record in (0..1000).rev() {
                        cx.report(
                            Diagnostic::new(codes::ZIP_PATH_UNSAFE)
                                .at(crate::Ref::at_record(record)),
                        );
                        assert_eq!(cx.diag.len(), 1);
                        assert_eq!(cx.diag.iter().next().unwrap().places().len(), max_places);
                    }
                    Ok::<(), Stop>(())
                })
                .unwrap();
            let diagnostics = diag.into_vec();
            assert_eq!(diagnostics[0].count(), 1000);
            assert_eq!(
                diagnostics[0].places(),
                &[crate::Ref::at_record(0)][..max_places]
            );
            assert_eq!(budget.used(Kind::Work), 1000 * (4 + max_places as u64));
        }
    }

    #[test]
    fn record_order_preserves_tied_places_and_puts_unlocated_reports_last() {
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(22)));
        let mut diag = Diagnostics::with_max_places(2);
        let cancel = CancelToken::new();
        Cx::new(&mut budget, &mut diag, &cancel)
            .with_diagnostics_in_record_order(|cx| {
                cx.report(Diagnostic::new(codes::SXF_P21_SHEET_MISSING));
                cx.report(
                    Diagnostic::new(codes::ZIP_PATH_UNSAFE)
                        .at(crate::Ref::at_record(5))
                        .at(crate::Ref::at_record(2).with_id("first"))
                        .at(crate::Ref::at_record(2).with_id("second")),
                );
                cx.report(
                    Diagnostic::new(codes::ZIP_PATH_UNSAFE)
                        .at(crate::Ref::at_record(2).with_id("third")),
                );
                Ok::<(), Stop>(())
            })
            .unwrap();
        assert_eq!(budget.used(Kind::Work), 22);
        let diagnostics = diag.into_vec();
        assert_eq!(diagnostics[0].count(), 2);
        assert_eq!(
            diagnostics[0].places(),
            [
                crate::Ref::at_record(2).with_id("first"),
                crate::Ref::at_record(2).with_id("second")
            ]
        );
        assert_eq!(diagnostics[1].code(), codes::SXF_P21_SHEET_MISSING);
        assert!(diagnostics[1].places().is_empty());
    }
}
