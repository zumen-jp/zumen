//! 予算・診断・中断の合図を運ぶ文脈（決定 0008）。

use crate::budget::{Budget, Exceeded, Kind, Limit};
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

/// 一度最後まで通した処理が、予算と診断に及ぼした作用の記録（`docs/live.md`）。
///
/// [`Cx::record`] で作り、[`Cx::replay`] で、同じ処理をもう一度通したときと同じ作用を当てる。
/// 常駐させた結果を使い回しても、使った予算と届く診断を、毎回作り直したときとそろえるために使う。
/// 持つのは、単位ごとに使った量・深さの余裕・集約した診断で、処理の大きさではなく診断の種類の数に比例する。
#[derive(Clone, Debug)]
pub struct Replay {
    /// 単位ごとに使った量（[`Kind::ALL`] の順）。深さは出入りが釣り合ったときだけ記録するので 0 になる
    spent: [u64; Kind::ALL.len()],
    /// 記録したときの深さの余裕。上限がなかったときは `None`。途中の最も深い所は数えないので、
    /// 当てるときにこれ以上の余裕があれば、同じ処理が深さの上限で止まらないと分かる
    depth_room: Option<u64>,
    /// 処理が届けた診断。届いた順にまとめ、場所は記録したときの `max_places` 件まで持つ
    diagnostics: Diagnostics,
}

/// 単位の残り。上限がなければ `None`
fn room(budget: &Budget, kind: Kind) -> Option<u64> {
    match budget.limits().get(kind) {
        Limit::Finite(limit) => Some(limit.saturating_sub(budget.used(kind))),
        Limit::Unlimited => None,
    }
}

impl Cx<'_> {
    /// 診断を、届いた順にそのまままとめる収集先か。入力順の収集と明細の捕捉は、予算の使い方が収集先で変わる
    fn plain(&self) -> bool {
        !self.diag.is_collecting_in_record_order() && self.diag.events.is_none()
    }

    /// 処理を通し、最後まで通せたら、その作用を [`Replay`] に記録する。
    ///
    /// 処理は同じ予算と中断の合図で、空の別の収集先（`max_places` は同じ）に診断を集め、終わったら成否によらず
    /// 届いた順に `diag` へ移す。届いた順にまとめる収集先では、まとめたものを移しても、元の報告を 1 件ずつ
    /// 足したのと同じ件数・場所・値・順になる。移すのに予算は使わない（そのような収集先への報告は予算を使わない）。
    ///
    /// 記録は、処理が成功し、予算が止まっておらず、深さの出入りが釣り合い、使った量が上限の値で飽和して
    /// いないときだけ返す。入力順の収集や明細の捕捉の中では記録せず、処理をそのまま通す。
    /// 記録する処理は、予算の残りや収集先の中身を読んで動きを変えないものに限る。
    pub fn record<T, E>(
        &mut self,
        operation: impl FnOnce(&mut Cx<'_>) -> Result<T, E>,
    ) -> (Result<T, E>, Option<Replay>) {
        if !self.plain() {
            return (operation(self), None);
        }
        let before = Kind::ALL.map(|kind| self.budget.used(kind));
        let depth_room = room(self.budget, Kind::Depth);
        let mut diagnostics = Diagnostics::with_max_places(self.diag.max_places());
        let result = operation(&mut Cx::new(self.budget, &mut diagnostics, self.cancel));
        for diagnostic in diagnostics.iter() {
            self.diag.report(diagnostic.clone());
        }
        let after = Kind::ALL.map(|kind| self.budget.used(kind));
        let mut spent = [0; Kind::ALL.len()];
        let mut recordable = result.is_ok() && self.budget.exceeded().is_none();
        for (index, kind) in Kind::ALL.into_iter().enumerate() {
            let (Some(&before), Some(&after), Some(slot)) =
                (before.get(index), after.get(index), spent.get_mut(index))
            else {
                recordable = false;
                break;
            };
            if after < before || after == u64::MAX || (kind == Kind::Depth && after != before) {
                recordable = false;
                break;
            }
            *slot = after - before;
        }
        let replay = recordable.then_some(Replay {
            spent,
            depth_room,
            diagnostics,
        });
        (result, replay)
    }

    /// 記録した作用を当てる。当てたら `true`、当てられなければ何も変えずに `false` を返す。
    ///
    /// 同じ処理がこの予算で最後まで通ると分かるときだけ当てる。単位ごとの残りが使った量以上で、深さの余裕が
    /// 記録したとき以上で、予算がまだ止まっておらず、中断の合図がなく、収集先が届いた順にまとめる同じ
    /// `max_places` のものであることが条件。当てると、単位ごとに使った量を一度に使い、記録した診断を届いた順に足す。
    /// 処理を通したときと同じ量を使い、同じ診断が届く。
    ///
    /// `false` のときは、呼び出す側が処理をそのまま通す。予算が足りないときの止まる所と、止まる前の診断は、
    /// そうして処理を通したときのものになる。中断の合図は当てる前に一度だけ確かめる。
    pub fn replay(&mut self, replay: &Replay) -> bool {
        if !self.plain()
            || self.diag.max_places() != replay.diagnostics.max_places()
            || self.cancel.stop().is_some()
            || self.budget.exceeded().is_some()
        {
            return false;
        }
        let depth = match (replay.depth_room, room(self.budget, Kind::Depth)) {
            (Some(recorded), Some(now)) => now >= recorded,
            (_, None) => true,
            (None, Some(_)) => false,
        };
        if !depth {
            return false;
        }
        let fits = Kind::ALL
            .into_iter()
            .zip(replay.spent)
            .all(|(kind, spent)| room(self.budget, kind).is_none_or(|room| spent <= room));
        if !fits {
            return false;
        }
        for (kind, spent) in Kind::ALL.into_iter().zip(replay.spent) {
            if spent > 0 && self.budget.spend(kind, spent).is_err() {
                // 上で残りを確かめたので、ここには来ない。来ても、使った量は処理を通した場合を超えない
                return false;
            }
        }
        for diagnostic in replay.diagnostics.iter() {
            self.diag.report(diagnostic.clone());
        }
        true
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

    /// 記録と当て直しを試す処理。入れ子・入力順の収集・別の収集先・場所の多い診断を混ぜる
    fn mixed(cx: &mut Cx<'_>) -> Result<u32, Stop> {
        cx.report(event_at(7));
        for record in 0..5 {
            cx.check(Kind::Records, 1)?;
            cx.report(Diagnostic::new(codes::SXF_CODE_MISSING).at(Ref::at_record(record)));
        }
        cx.enter()?;
        cx.enter()?;
        cx.check(Kind::Work, 3)?;
        cx.leave();
        cx.leave();
        cx.with_diagnostics_in_record_order(|cx| {
            cx.report(event_at(9));
            cx.check(Kind::Work, 2)?;
            cx.report(event_at(1));
            Ok::<(), Stop>(())
        })?;
        let mut quiet = Diagnostics::new();
        let mut inner = Cx::new(cx.budget, &mut quiet, cx.cancel);
        inner.check(Kind::Elements, 4)?;
        inner.report(event_at(2));
        cx.check(Kind::DecodedBytes, 10)?;
        Ok(42)
    }

    /// 前の報告と予算の使用がある、呼び出しの初めの状態
    fn started(limits: Limits, max_places: usize) -> (Budget, Diagnostics) {
        let mut budget = Budget::new(limits);
        budget.spend(Kind::Work, 5).unwrap();
        let mut diag = Diagnostics::with_max_places(max_places);
        diag.report(Diagnostic::new(codes::SXF_CODE_MISSING).at(Ref::at_record(100)));
        diag.report(event_at(3));
        (budget, diag)
    }

    fn recorded(max_places: usize) -> Replay {
        let (mut budget, mut diag) = started(Limits::default(), max_places);
        let cancel = CancelToken::new();
        let (result, replay) = Cx::new(&mut budget, &mut diag, &cancel).record(mixed);
        assert_eq!(result, Ok(42));
        replay.unwrap()
    }

    #[test]
    fn recording_and_replaying_match_running_the_operation_again() {
        for max_places in [0, 2, Diagnostics::DEFAULT_MAX_PLACES] {
            let cancel = CancelToken::new();
            let (mut budget, mut diag) = started(Limits::default(), max_places);
            assert_eq!(mixed(&mut Cx::new(&mut budget, &mut diag, &cancel)), Ok(42));
            let expected = (budget, diag.into_vec());

            let (mut budget, mut diag) = started(Limits::default(), max_places);
            let (result, replay) = Cx::new(&mut budget, &mut diag, &cancel).record(mixed);
            assert_eq!(result, Ok(42));
            assert_eq!((budget, diag.into_vec()), expected);

            let replay = replay.unwrap();
            let (mut budget, mut diag) = started(Limits::default(), max_places);
            assert!(Cx::new(&mut budget, &mut diag, &cancel).replay(&replay));
            assert_eq!((budget.clone(), diag.clone().into_vec()), expected);
            // 続けて当てても、続けて通したのと同じになる
            let (mut twice_budget, mut twice_diag) = started(Limits::default(), max_places);
            for _ in 0..2 {
                let mut cx = Cx::new(&mut twice_budget, &mut twice_diag, &cancel);
                assert_eq!(mixed(&mut cx), Ok(42));
            }
            assert!(Cx::new(&mut budget, &mut diag, &cancel).replay(&replay));
            assert_eq!(
                (budget, diag.into_vec()),
                (twice_budget, twice_diag.into_vec())
            );
        }
    }

    #[test]
    fn replay_refuses_budgets_that_would_stop_the_operation_and_changes_nothing() {
        let replay = recorded(Diagnostics::DEFAULT_MAX_PLACES);
        let cancel = CancelToken::new();
        // 処理を通して使う量を測る。入力順の収集の分も work に入る。深さは 2 まで
        let (mut budget, mut diag) = started(Limits::default(), 16);
        let used = Kind::ALL.map(|kind| budget.used(kind));
        mixed(&mut Cx::new(&mut budget, &mut diag, &cancel)).unwrap();
        assert!(budget.used(Kind::Work) > 5 + 5);
        for (kind, used) in Kind::ALL.into_iter().zip(used) {
            let spent = budget.used(kind) - used;
            if spent == 0 {
                continue;
            }
            for (limit, accepted) in [(used + spent - 1, false), (used + spent, true)] {
                let limits = Limits::default().with(kind, Limit::Finite(limit));
                let (mut budget, mut diag) = started(limits, 16);
                let before = (budget.clone(), diag.clone().into_vec());
                let replayed = Cx::new(&mut budget, &mut diag, &cancel).replay(&replay);
                assert_eq!(replayed, accepted, "{kind:?} {limit}");
                if !accepted {
                    assert_eq!((budget, diag.into_vec()), before);
                    // 処理を通すと、その単位で止まる
                    let (mut budget, mut diag) = started(limits, 16);
                    let stop = mixed(&mut Cx::new(&mut budget, &mut diag, &cancel));
                    assert!(matches!(stop, Err(Stop::Exceeded(e)) if e.kind == kind));
                }
            }
        }
        // 深さは、記録したとき（既定の上限）より余裕が少なければ当てない
        for (limit, accepted) in [
            (Limit::Finite(255), false),
            (Limit::Finite(256), true),
            (Limit::Unlimited, true),
        ] {
            let (mut budget, mut diag) = started(Limits::default().with(Kind::Depth, limit), 16);
            let replayed = Cx::new(&mut budget, &mut diag, &cancel).replay(&replay);
            assert_eq!(replayed, accepted, "{limit:?}");
        }
        // 中断の合図・止まった予算・違う max_places では当てない
        let cancelled = CancelToken::new();
        cancelled.cancel();
        let (mut budget, mut diag) = started(Limits::default(), 16);
        assert!(!Cx::new(&mut budget, &mut diag, &cancelled).replay(&replay));
        assert_eq!(budget.used(Kind::Records), 0);
        let (mut budget, mut diag) = started(Limits::default(), 16);
        assert!(budget.spend(Kind::Work, u64::MAX).is_err());
        assert!(!Cx::new(&mut budget, &mut diag, &cancel).replay(&replay));
        let (mut budget, mut diag) = started(Limits::default(), 15);
        assert!(!Cx::new(&mut budget, &mut diag, &cancel).replay(&replay));
        assert_eq!(diag.len(), 2);
    }

    #[test]
    fn replay_and_record_step_aside_inside_ordered_or_captured_collection() {
        let replay = recorded(Diagnostics::DEFAULT_MAX_PLACES);
        let cancel = CancelToken::new();
        let (mut budget, mut diag) = started(Limits::default(), 16);
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        cx.with_diagnostics_in_record_order(|cx| {
            assert!(!cx.replay(&replay));
            let (result, recorded) = cx.record(mixed);
            assert_eq!(result, Ok(42));
            assert!(recorded.is_none());
            Ok::<(), Stop>(())
        })
        .unwrap();
        cx.with_diagnostic_events(4, |cx| {
            assert!(!cx.replay(&replay));
            assert!(cx.record(mixed).1.is_none());
            Ok::<(), Stop>(())
        })
        .unwrap();
    }

    #[test]
    fn record_keeps_only_operations_that_finish_with_balanced_depth() {
        let cancel = CancelToken::new();
        // 止まった処理は記録せず、診断と使った量は通したときと同じになる
        for work in 5..12 {
            let limits = Limits::default().with(Kind::Work, Limit::Finite(work));
            let (mut budget, mut diag) = started(limits, 2);
            let result = mixed(&mut Cx::new(&mut budget, &mut diag, &cancel));
            let expected = (result, budget, diag.into_vec());
            let (mut budget, mut diag) = started(limits, 2);
            let (result, replay) = Cx::new(&mut budget, &mut diag, &cancel).record(mixed);
            assert_eq!(replay.is_some(), result.is_ok());
            assert_eq!((result, budget, diag.into_vec()), expected);
        }
        let (mut budget, mut diag) = started(Limits::default(), 2);
        let (result, replay) = Cx::new(&mut budget, &mut diag, &cancel).record(|cx| cx.enter());
        assert_eq!(result, Ok(()));
        assert!(replay.is_none());
    }
}
