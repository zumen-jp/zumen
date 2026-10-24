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
        self.diag.report(diagnostic);
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
}
