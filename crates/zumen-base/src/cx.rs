//! 予算・診断・中断の合図を運ぶ文脈（決定 0008）。

use crate::budget::{Budget, Exceeded, Kind};
use crate::cancel::CancelToken;
use crate::codes;
use crate::diag::{Diagnostic, DiagnosticEvents, Diagnostics, EventCapture};

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
        if let Some(stop) = self.diag.collection_stop() {
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
        if self.diag.collection_stop().is_some() {
            return;
        }
        if let Err(stop) = self
            .diag
            .preflight_event(&diagnostic, self.budget, self.cancel)
        {
            self.diag.stop_record_order(stop, diagnostic);
            return;
        }
        if let Some(amount) = self.diag.record_order_work(&diagnostic)
            && let Err(stop) = self.check(Kind::Work, amount)
        {
            self.diag.stop_record_order(stop, diagnostic);
            return;
        }
        self.diag.record_event(&diagnostic);
        self.diag.report(diagnostic);
    }

    /// 処理中に報告した診断を、集約前の明細と場所を持たない集計として受け取る。
    ///
    /// [`Cx::report`] の報告を捕捉し、`diag` の `max_places` によらず元の値と場所を保つ。
    /// 明細は入力位置順の先頭 `limit` 件とし、同じ位置は報告順にする。
    /// 集計の代表値はもっとも早い入力位置の報告から採り、発生件数は診断の `count` を足す。
    /// 入れ子の呼び出しでは、内外がそれぞれの処理中の報告を捕捉する。
    /// 深さ・収集・整列・確保を予算で数え、明細の上限に達しても集計と中断の確認を続ける。
    /// 停止は処理の最後に報告された場合も返す。集約済みの診断の転送は重ねて捕捉しない。
    pub fn with_diagnostic_events<T, E: From<Stop>>(
        &mut self,
        limit: u64,
        operation: impl FnOnce(&mut Cx<'_>) -> Result<T, E>,
    ) -> Result<(T, DiagnosticEvents), E> {
        if let Err(stop) = self.enter() {
            if self.diag.collection_stop().is_none() {
                self.diag.stop_record_order(stop, stop.to_diagnostic());
            }
            return Err(stop.into());
        }
        let allocation = self.check(Kind::DecodedBytes, EventCapture::LOGICAL_BYTES);
        if let Err(stop) = allocation {
            self.leave();
            if self.diag.collection_stop().is_none() {
                self.diag.stop_record_order(stop, stop.to_diagnostic());
            }
            return Err(stop.into());
        }
        let parent = self.diag.events.take();
        self.diag.events = Some(Box::new(EventCapture::new(limit, parent)));
        let result = operation(self);
        let stop = self.diag.collection_stop();
        let capture = self.diag.events.take();
        self.leave();
        let Some(mut capture) = capture else {
            // 処理が収集先そのものを置き換えた場合にも、パニックしない。
            return result.map(|value| (value, DiagnosticEvents::default()));
        };
        self.diag.events = capture.parent.take();
        if let Some(stop) = stop {
            return Err(stop.into());
        }
        let value = result?;
        match capture.finish(self.budget, self.cancel) {
            Ok(events) => Ok((value, events)),
            Err(stop) => {
                self.diag.stop_record_order(stop, stop.to_diagnostic());
                Err(stop.into())
            }
        }
    }

    /// 処理中の診断を、レコードの入力順でまとめてから届ける（決定 0009）。
    ///
    /// 診断は [`Cx::report`] で足す。処理中の `diag.iter()` は届いた順になる。
    /// 入れ子は同じ収集先を使い、もっとも外側の呼び出しが返すときに入力順で届ける。
    /// レコード番号のある診断はその番号順、バイト範囲だけの診断はその後に開始位置順、位置のない診断は最後にする。
    /// 同じ位置では届いた順にする。値も先頭の診断から採る。
    /// 場所は収集中も `max_places` 件までにし、0 件でも診断の順を保つ。
    /// 収集・場所の移動・最後の整列と転送を Work に数える。途中で止まっても、収集済みの診断は届ける。
    pub fn with_diagnostics_in_record_order<T, E: From<Stop>>(
        &mut self,
        operation: impl FnOnce(&mut Cx<'_>) -> Result<T, E>,
    ) -> Result<T, E> {
        if let Some(stop) = self.diag.collection_stop() {
            return Err(stop.into());
        }
        if self.diag.is_collecting_in_record_order() {
            let result = operation(self);
            return match self.diag.collection_stop() {
                Some(stop) => Err(stop.into()),
                None => result,
            };
        }
        let mut diagnostics = Diagnostics::in_record_order(self.diag.max_places());
        diagnostics.events = self.diag.events.take();
        let result = operation(&mut Cx::new(self.budget, &mut diagnostics, self.cancel));
        let stop = diagnostics.collection_stop();
        let events = diagnostics.events.take();
        for diagnostic in diagnostics.into_vec() {
            // 収集と転送の予算は支払ってあり、元の報告もすでに捕捉している。
            self.diag.report(diagnostic);
        }
        self.diag.events = events;
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
    use crate::{Ref, Value};

    fn event_at(record: u64) -> Diagnostic {
        Diagnostic::new(codes::ZIP_PATH_UNSAFE)
            .with("name", record.to_string())
            .at(Ref::at_record(record))
    }

    fn capture_events(limit: u64, records: &[u64]) -> DiagnosticEvents {
        let mut budget = Budget::default();
        let mut diag = Diagnostics::with_max_places(0);
        let cancel = CancelToken::new();
        Cx::new(&mut budget, &mut diag, &cancel)
            .with_diagnostic_events(limit, |cx| {
                for &record in records {
                    cx.report(event_at(record));
                }
                Ok::<(), Stop>(())
            })
            .unwrap()
            .1
    }

    #[test]
    fn event_limits_only_cut_details_and_preserve_the_earliest_summary_without_places() {
        let full = capture_events(10, &[9, 3, 1, 3]);
        assert_eq!(full.total, 4);
        assert_eq!(full.counts.len(), 1);
        assert_eq!(full.counts[0].count(), 4);
        assert_eq!(full.counts[0].param("name"), Some(&Value::from("1")));
        assert!(full.counts[0].places().is_empty());
        assert!(!full.truncated);
        for limit in [0, 1, 2] {
            let events = capture_events(limit, &[9, 3, 1, 3]);
            assert_eq!(events.total, full.total);
            assert_eq!(events.counts, full.counts);
            assert_eq!(events.details, full.details[..limit as usize]);
            assert!(events.truncated);
        }
    }

    #[test]
    fn events_preserve_raw_places_and_same_place_reports_in_input_order() {
        let mut budget = Budget::default();
        let mut diag = Diagnostics::with_max_places(0);
        let cancel = CancelToken::new();
        let (_, events) = Cx::new(&mut budget, &mut diag, &cancel)
            .with_diagnostic_events(8, |cx| {
                cx.report(Diagnostic::new(codes::SXF_CODE_MISSING));
                cx.report(Diagnostic::new(codes::ZIP_PATH_UNSAFE).at(Ref::at_bytes(2, 5)));
                cx.report(
                    event_at(5)
                        .at(Ref::at_record(2).with_id("first"))
                        .at(Ref::at_bytes(1, 3)),
                );
                cx.report(
                    Diagnostic::new(codes::SXF_P21_INSTANCE_DROPPED)
                        .with("id", 2_u64)
                        .at(Ref::at_record(2).with_id("first")),
                );
                Ok::<(), Stop>(())
            })
            .unwrap();
        assert_eq!(events.total, 4);
        assert_eq!(events.details.len(), 4);
        assert_eq!(events.details[0].places().len(), 3);
        assert_eq!(events.details[0].places()[0].record(), Some(5));
        assert_eq!(events.details[0].places()[1].id(), Some("first"));
        assert_eq!(events.details[1].code(), codes::SXF_P21_INSTANCE_DROPPED);
        assert_eq!(events.details[2].places()[0].bytes().unwrap().start, 2);
        assert_eq!(events.details[3].code(), codes::SXF_CODE_MISSING);
        assert!(events.details[3].fidelity().is_none());
        assert!(events.counts.iter().all(|entry| entry.places().is_empty()));
        let codes: Vec<_> = events
            .counts
            .iter()
            .map(|entry| entry.code().name())
            .collect();
        assert!(codes.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(diag.iter().all(|entry| entry.places().is_empty()));
    }

    #[test]
    fn nested_event_scopes_capture_each_raw_report_once_through_record_order() {
        for max_places in [0, 1] {
            let mut budget = Budget::default();
            let mut diag = Diagnostics::with_max_places(max_places);
            let cancel = CancelToken::new();
            let (inner, outer) = Cx::new(&mut budget, &mut diag, &cancel)
                .with_diagnostic_events(10, |cx| {
                    cx.report(event_at(9));
                    let inner = cx.with_diagnostics_in_record_order(|cx| {
                        cx.report(event_at(8));
                        let (_, inner) = cx.with_diagnostic_events(10, |cx| {
                            cx.report(event_at(5));
                            cx.with_diagnostics_in_record_order(|cx| {
                                cx.report(event_at(1));
                                Ok::<(), Stop>(())
                            })
                        })?;
                        cx.report(event_at(7));
                        Ok::<_, Stop>(inner)
                    })?;
                    cx.report(event_at(6));
                    Ok::<_, Stop>(inner)
                })
                .unwrap();
            assert_eq!(outer.total, 6);
            assert_eq!(outer.counts[0].count(), 6);
            assert_eq!(outer.details.len(), 6);
            assert_eq!(inner.total, 2);
            assert_eq!(inner.counts[0].count(), 2);
            assert_eq!(inner.details.len(), 2);
            assert_eq!(diag.iter().next().unwrap().count(), 6);
            assert_eq!(budget.used(Kind::Depth), 0);
        }
    }

    #[test]
    fn capture_inside_record_order_is_not_charged_for_reports_outside_the_scope() {
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let events = Cx::new(&mut budget, &mut diag, &cancel)
            .with_diagnostics_in_record_order(|cx| {
                cx.report(event_at(8));
                let (_, events) = cx.with_diagnostic_events(1, |cx| {
                    cx.report(event_at(2));
                    Ok::<(), Stop>(())
                })?;
                cx.report(event_at(1));
                Ok::<_, Stop>(events)
            })
            .unwrap();
        assert_eq!(events.total, 1);
        assert_eq!(events.details[0].places()[0].record(), Some(2));
        assert_eq!(diag.iter().next().unwrap().count(), 3);
    }

    #[test]
    fn event_totals_add_diagnostic_counts_without_inventing_more_detail_rows() {
        let mut grouped = Diagnostics::new();
        for _ in 0..3 {
            grouped.report(event_at(4));
        }
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let (_, events) = Cx::new(&mut budget, &mut diag, &cancel)
            .with_diagnostic_events(1, |cx| {
                for diagnostic in grouped.into_vec() {
                    cx.report(diagnostic);
                }
                Ok::<(), Stop>(())
            })
            .unwrap();
        assert_eq!(events.total, 3);
        assert_eq!(events.details.len(), 1);
        assert_eq!(events.details[0].count(), 3);
        assert!(!events.truncated);
    }

    #[test]
    fn event_scope_entry_stops_before_allocating_or_running_the_operation() {
        for (kind, limit, cancelled) in [
            (Kind::Depth, 0, false),
            (Kind::DecodedBytes, EventCapture::LOGICAL_BYTES - 1, false),
            (Kind::Work, 0, true),
        ] {
            let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
            let mut diag = Diagnostics::new();
            let cancel = CancelToken::new();
            if cancelled {
                cancel.cancel();
            }
            let mut entered = false;
            let stop = Cx::new(&mut budget, &mut diag, &cancel)
                .with_diagnostic_events(1, |_| {
                    entered = true;
                    Ok::<(), Stop>(())
                })
                .unwrap_err();
            assert!(!entered);
            assert!(diag.events.is_none());
            assert_eq!(diag.len(), 1);
            assert_eq!(
                diag.iter().next().unwrap().code(),
                stop.to_diagnostic().code()
            );
            assert_eq!(budget.used(Kind::Depth), 0);
            assert_eq!(budget.used(Kind::DecodedBytes), 0);
        }
    }

    #[test]
    fn the_last_report_keeps_a_sticky_stop_before_copying_the_event() {
        for kind in [Kind::Work, Kind::DecodedBytes] {
            let limit = if kind == Kind::DecodedBytes {
                EventCapture::LOGICAL_BYTES
            } else {
                0
            };
            let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
            let mut diag = Diagnostics::new();
            let cancel = CancelToken::new();
            let stop = Cx::new(&mut budget, &mut diag, &cancel)
                .with_diagnostic_events(1, |cx| {
                    cx.report(event_at(7));
                    let stop = cx.check(Kind::Work, 0).unwrap_err();
                    cx.report(stop.to_diagnostic());
                    Ok::<(), Stop>(())
                })
                .unwrap_err();
            assert!(matches!(stop, Stop::Exceeded(exceeded) if exceeded.kind == kind));
            assert_eq!(diag.len(), 1);
            assert_eq!(diag.iter().next().unwrap().code(), kind.exceeded_code());
            assert_eq!(diag.iter().next().unwrap().count(), 1);
            assert_eq!(diag.iter().next().unwrap().places(), [Ref::at_record(7)]);
            assert_eq!(budget.used(Kind::DecodedBytes), EventCapture::LOGICAL_BYTES);
        }
    }

    #[test]
    fn a_scope_stop_is_returned_even_if_the_operation_ignores_its_inner_result() {
        let mut budget = Budget::new(Limits::default().with(Kind::Depth, Limit::Finite(1)));
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let stop = Cx::new(&mut budget, &mut diag, &cancel)
            .with_diagnostic_events(1, |cx| {
                let inner = cx.with_diagnostic_events(1, |_| Ok::<(), Stop>(()));
                assert!(inner.is_err());
                Ok::<(), Stop>(())
            })
            .unwrap_err();
        assert!(matches!(stop, Stop::Exceeded(exceeded) if exceeded.kind == Kind::Depth));
        assert_eq!(diag.len(), 1);
        assert_eq!(budget.used(Kind::Depth), 0);
    }

    #[test]
    fn capture_checks_cancellation_and_work_after_reaching_the_detail_limit() {
        for limit in [0, 1] {
            let mut budget = Budget::default();
            let mut diag = Diagnostics::with_max_places(0);
            let cancel = CancelToken::new();
            let stop = Cx::new(&mut budget, &mut diag, &cancel)
                .with_diagnostic_events(limit, |cx| {
                    cx.report(event_at(9));
                    let before = cx.budget.used(Kind::Work);
                    cx.report(event_at(8));
                    assert!(cx.budget.used(Kind::Work) > before);
                    cancel.cancel();
                    cx.report(event_at(7));
                    Ok::<(), Stop>(())
                })
                .unwrap_err();
            assert_eq!(stop, Stop::Cancelled);
            assert_eq!(diag.len(), 2);
            assert_eq!(diag.iter().next().unwrap().count(), 2);
        }
    }

    #[test]
    fn final_sorting_checks_the_budget_and_reports_its_stop() {
        let mut before_finish = 0;
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        Cx::new(&mut budget, &mut diag, &cancel)
            .with_diagnostic_events(1, |cx| {
                cx.report(event_at(1));
                before_finish = cx.budget.used(Kind::Work);
                Ok::<(), Stop>(())
            })
            .unwrap();
        let mut budget =
            Budget::new(Limits::default().with(Kind::Work, Limit::Finite(before_finish)));
        let mut diag = Diagnostics::new();
        let stop = Cx::new(&mut budget, &mut diag, &cancel)
            .with_diagnostic_events(1, |cx| {
                cx.report(event_at(1));
                Ok::<(), Stop>(())
            })
            .unwrap_err();
        assert!(matches!(stop, Stop::Exceeded(exceeded) if exceeded.kind == Kind::Work));
        assert_eq!(diag.len(), 2);
        assert_eq!(
            diag.iter().last().unwrap().code(),
            codes::BUDGET_WORK_EXCEEDED
        );
    }

    #[test]
    fn merging_captures_keeps_counts_and_the_earliest_summary_without_recapturing() {
        for limit in [0, 1, 4] {
            let first = capture_events(limit, &[9, 5]);
            let second = capture_events(limit, &[1, 5]);
            let mut budget = Budget::default();
            let mut diag = Diagnostics::new();
            let cancel = CancelToken::new();
            let (merged, surrounding) = Cx::new(&mut budget, &mut diag, &cancel)
                .with_diagnostic_events(10, |cx| first.merge(second, limit, cx))
                .unwrap();
            assert_eq!(merged.total, 4);
            assert_eq!(merged.counts[0].count(), 4);
            assert_eq!(merged.counts[0].param("name"), Some(&Value::from("1")));
            assert!(merged.counts[0].places().is_empty());
            assert_eq!(merged.details.len(), (limit as usize).min(4));
            assert_eq!(merged.truncated, limit < 4);
            assert_eq!(surrounding.total, 0);
            assert!(diag.is_empty());
        }
    }

    #[test]
    fn merging_captures_checks_its_budget_before_allocating_and_does_not_restore_cut_details() {
        let first = capture_events(0, &[9]);
        let second = capture_events(0, &[1]);
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let merged = first
            .merge(second, 10, &mut Cx::new(&mut budget, &mut diag, &cancel))
            .unwrap();
        assert_eq!(merged.total, 2);
        assert!(merged.details.is_empty());
        assert!(merged.truncated);
        let mut budget = Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(0)));
        let first = capture_events(1, &[9]);
        let second = capture_events(1, &[1]);
        let stop = first
            .merge(second, 1, &mut Cx::new(&mut budget, &mut diag, &cancel))
            .unwrap_err();
        assert!(matches!(stop, Stop::Exceeded(exceeded) if exceeded.kind == Kind::DecodedBytes));
        assert_eq!(budget.used(Kind::DecodedBytes), 0);
        assert_eq!(
            diag.iter().next().unwrap().code(),
            codes::BUDGET_DECODED_BYTES_EXCEEDED
        );
    }

    #[test]
    fn event_counts_use_catalog_keys_and_input_position_for_representative_values() {
        let mut budget = Budget::default();
        let mut diag = Diagnostics::with_max_places(0);
        let cancel = CancelToken::new();
        let (_, events) = Cx::new(&mut budget, &mut diag, &cancel)
            .with_diagnostic_events(0, |cx| {
                for (kind, code, record, value) in [
                    ("line", 2_i64, 9, "late"),
                    ("line", 2, 1, "early"),
                    ("line", 1, 4, "one"),
                    ("circle", 4, 5, "circle"),
                ] {
                    cx.report(
                        Diagnostic::new(codes::SXF_CODE_MISSING)
                            .with("kind", kind)
                            .with("code", code)
                            .with("value", value)
                            .at(Ref::at_record(record)),
                    );
                }
                Ok::<(), Stop>(())
            })
            .unwrap();
        assert_eq!(events.total, 4);
        assert_eq!(events.counts.len(), 3);
        assert_eq!(events.counts[0].param("kind"), Some(&Value::from("circle")));
        assert_eq!(events.counts[1].param("code"), Some(&Value::Int(1)));
        assert_eq!(events.counts[2].count(), 2);
        assert_eq!(events.counts[2].param("value"), Some(&Value::from("early")));
        assert!(events.details.is_empty());
    }

    #[test]
    fn merging_same_position_reports_preserves_the_phase_order_and_representative() {
        let mut budget = Budget::default();
        let mut diag = Diagnostics::with_max_places(0);
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut phase = |name: &str| {
            cx.with_diagnostic_events(2, |cx| {
                cx.report(event_at(1).with("name", name));
                Ok::<(), Stop>(())
            })
            .unwrap()
            .1
        };
        let first = phase("first");
        let second = phase("second");
        let merged = first.merge(second, 2, &mut cx).unwrap();
        assert_eq!(merged.total, 2);
        assert_eq!(merged.counts[0].count(), 2);
        assert_eq!(merged.counts[0].param("name"), Some(&Value::from("first")));
        assert_eq!(merged.details[0].param("name"), Some(&Value::from("first")));
        assert_eq!(
            merged.details[1].param("name"),
            Some(&Value::from("second"))
        );
        assert!(!merged.truncated);
    }

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

    #[test]
    fn nested_record_order_keeps_positions_values_and_one_work_charge() {
        for max_places in [0, 1] {
            let work = 5 * (4 + max_places as u64);
            let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(work)));
            let mut diag = Diagnostics::with_max_places(max_places);
            let cancel = CancelToken::new();
            Cx::new(&mut budget, &mut diag, &cancel)
                .with_diagnostics_in_record_order(|cx| {
                    cx.report(
                        Diagnostic::new(codes::ZIP_PATH_UNSAFE)
                            .with("name", "late")
                            .at(crate::Ref::at_record(9)),
                    );
                    cx.with_diagnostics_in_record_order(|cx| {
                        cx.report(
                            Diagnostic::new(codes::SXF_P21_INSTANCE_DROPPED)
                                .with("id", 2_u64)
                                .at(crate::Ref::at_record(4)),
                        );
                        cx.with_diagnostics_in_record_order(|cx| {
                            cx.report(
                                Diagnostic::new(codes::ZIP_PATH_UNSAFE)
                                    .with("name", "first")
                                    .at(crate::Ref::at_record(1)),
                            );
                            Ok::<(), Stop>(())
                        })?;
                        cx.report(
                            Diagnostic::new(codes::SXF_P21_INSTANCE_DROPPED)
                                .with("id", 3_u64)
                                .at(crate::Ref::at_record(4)),
                        );
                        Ok::<(), Stop>(())
                    })?;
                    cx.report(
                        Diagnostic::new(codes::SXF_P21_SHEET_DROPPED)
                            .with("id", 80_u64)
                            .at(crate::Ref::at_record(2)),
                    );
                    Ok::<(), Stop>(())
                })
                .unwrap();
            assert_eq!(budget.used(Kind::Work), work);
            let diagnostics = diag.into_vec();
            assert_eq!(diagnostics.len(), 4);
            assert_eq!(diagnostics[0].code(), codes::ZIP_PATH_UNSAFE);
            assert_eq!(diagnostics[0].count(), 2);
            assert_eq!(
                diagnostics[0].param("name"),
                Some(&crate::Value::from("first"))
            );
            assert_eq!(diagnostics[1].code(), codes::SXF_P21_SHEET_DROPPED);
            assert_eq!(diagnostics[2].param("id"), Some(&crate::Value::UInt(2)));
            assert_eq!(diagnostics[3].param("id"), Some(&crate::Value::UInt(3)));
            for (diagnostic, record) in diagnostics.iter().zip([1, 2, 4, 4]) {
                assert_eq!(
                    diagnostic.places(),
                    &[crate::Ref::at_record(record)][..max_places]
                );
            }
        }
    }

    #[test]
    fn nested_record_order_returns_the_inner_stop_and_keeps_collected_reports() {
        for (max_places, cancelled) in [(0, false), (1, false), (0, true), (1, true)] {
            let work = 2 * (4 + max_places as u64);
            let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(work)));
            let mut diag = Diagnostics::with_max_places(max_places);
            let cancel = CancelToken::new();
            let mut inner_stop = None;
            let result =
                Cx::new(&mut budget, &mut diag, &cancel).with_diagnostics_in_record_order(|cx| {
                    cx.report(Diagnostic::new(codes::ZIP_PATH_UNSAFE).at(crate::Ref::at_record(9)));
                    let result = cx.with_diagnostics_in_record_order(|cx| {
                        cx.report(
                            Diagnostic::new(codes::SXF_P21_INSTANCE_DROPPED)
                                .with("id", 2_u64)
                                .at(crate::Ref::at_record(3)),
                        );
                        if cancelled {
                            cancel.cancel();
                        }
                        cx.report(
                            Diagnostic::new(codes::SXF_P21_INSTANCE_DROPPED)
                                .with("id", 3_u64)
                                .at(crate::Ref::at_record(5)),
                        );
                        Ok::<(), Stop>(())
                    });
                    let stop = result.unwrap_err();
                    inner_stop = Some(stop);
                    assert_eq!(cx.check(Kind::Work, 0), Err(stop));
                    let mut entered = false;
                    let after_stop = cx.with_diagnostics_in_record_order(|_| {
                        entered = true;
                        Ok::<(), Stop>(())
                    });
                    assert_eq!(after_stop, Err(stop));
                    assert!(!entered);
                    // 内側の結果を伝え忘れても、外側も同じ停止を返す。
                    Ok::<(), Stop>(())
                });
            let stop = result.unwrap_err();
            assert_eq!(inner_stop, Some(stop));
            assert_eq!(cancelled, stop == Stop::Cancelled);
            assert_eq!(budget.used(Kind::Work), work);
            let diagnostics = diag.into_vec();
            assert_eq!(diagnostics.len(), 3);
            assert_eq!(diagnostics[0].code(), codes::SXF_P21_INSTANCE_DROPPED);
            assert_eq!(diagnostics[0].param("id"), Some(&crate::Value::UInt(2)));
            assert_eq!(diagnostics[1].code(), stop.to_diagnostic().code());
            assert_eq!(diagnostics[2].code(), codes::ZIP_PATH_UNSAFE);
            for (diagnostic, record) in diagnostics.iter().zip([3, 5, 9]) {
                assert_eq!(diagnostic.count(), 1);
                assert_eq!(
                    diagnostic.places(),
                    &[crate::Ref::at_record(record)][..max_places]
                );
            }
        }
    }
}
