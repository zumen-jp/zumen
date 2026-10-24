//! 決まった作業量で数える予算（決定 0010）。
//!
//! 数える所はコードの中で決めておく。時計は読まないので、どの環境でも同じ所で止まる。

use crate::code::Code;
use crate::codes;
use crate::diag::Diagnostic;

const GIB: u64 = 1 << 30;

/// 作業量の単位
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// 受け取ったバイト数
    InputBytes,
    /// 展開・復号で作ったバイト数。入れ子の分も足す
    DecodedBytes,
    /// L0・L1 で読んだレコードの数
    Records,
    /// L2・L3 で作った要素の数
    Elements,
    /// 入れ子と再帰の深さ。足していくのではなく、今の深さを数える
    Depth,
    /// 幾何の分割で作った点・線・面の数
    Segments,
    /// ほかの単位に入らない繰り返しの回数
    Work,
}

impl Kind {
    /// すべての単位
    pub const ALL: [Kind; 7] = [
        Kind::InputBytes,
        Kind::DecodedBytes,
        Kind::Records,
        Kind::Elements,
        Kind::Depth,
        Kind::Segments,
        Kind::Work,
    ];

    /// code に使う名前（`decoded-bytes` など）
    pub const fn name(self) -> &'static str {
        match self {
            Kind::InputBytes => "input-bytes",
            Kind::DecodedBytes => "decoded-bytes",
            Kind::Records => "records",
            Kind::Elements => "elements",
            Kind::Depth => "depth",
            Kind::Segments => "segments",
            Kind::Work => "work",
        }
    }

    /// 上限を超えたことを表す code
    pub const fn exceeded_code(self) -> Code {
        match self {
            Kind::InputBytes => codes::BUDGET_INPUT_BYTES_EXCEEDED,
            Kind::DecodedBytes => codes::BUDGET_DECODED_BYTES_EXCEEDED,
            Kind::Records => codes::BUDGET_RECORDS_EXCEEDED,
            Kind::Elements => codes::BUDGET_ELEMENTS_EXCEEDED,
            Kind::Depth => codes::BUDGET_DEPTH_EXCEEDED,
            Kind::Segments => codes::BUDGET_SEGMENTS_EXCEEDED,
            Kind::Work => codes::BUDGET_WORK_EXCEEDED,
        }
    }
}

/// 単位ごとの値
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
struct PerKind<T> {
    input_bytes: T,
    decoded_bytes: T,
    records: T,
    elements: T,
    depth: T,
    segments: T,
    work: T,
}

impl<T> PerKind<T> {
    fn get(&self, kind: Kind) -> &T {
        match kind {
            Kind::InputBytes => &self.input_bytes,
            Kind::DecodedBytes => &self.decoded_bytes,
            Kind::Records => &self.records,
            Kind::Elements => &self.elements,
            Kind::Depth => &self.depth,
            Kind::Segments => &self.segments,
            Kind::Work => &self.work,
        }
    }

    fn get_mut(&mut self, kind: Kind) -> &mut T {
        match kind {
            Kind::InputBytes => &mut self.input_bytes,
            Kind::DecodedBytes => &mut self.decoded_bytes,
            Kind::Records => &mut self.records,
            Kind::Elements => &mut self.elements,
            Kind::Depth => &mut self.depth,
            Kind::Segments => &mut self.segments,
            Kind::Work => &mut self.work,
        }
    }
}

/// 1 つの単位の上限
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limit {
    /// この量までは使える
    Finite(u64),
    /// 上限なし。明示的に指定したときだけ使う
    Unlimited,
}

/// 単位ごとの上限
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits(PerKind<Limit>);

impl Limits {
    /// 仮の既定値。実図面で測って決め直す（決定 0010）
    const DEFAULT: PerKind<Limit> = PerKind {
        input_bytes: Limit::Finite(2 * GIB),
        decoded_bytes: Limit::Finite(2 * GIB),
        records: Limit::Finite(100_000_000),
        elements: Limit::Finite(100_000_000),
        depth: Limit::Finite(256),
        segments: Limit::Finite(100_000_000),
        work: Limit::Finite(10_000_000_000),
    };

    /// 単位の上限
    pub fn get(&self, kind: Kind) -> Limit {
        *self.0.get(kind)
    }

    /// 単位の上限を決める
    pub fn set(&mut self, kind: Kind, limit: Limit) {
        *self.0.get_mut(kind) = limit;
    }

    /// 単位の上限を決めたものを返す
    pub fn with(mut self, kind: Kind, limit: Limit) -> Self {
        self.set(kind, limit);
        self
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self(Self::DEFAULT)
    }
}

/// 上限を超えたこと
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exceeded {
    /// 単位
    pub kind: Kind,
    /// 上限
    pub limit: u64,
    /// 使おうとした量。`depth` では、入ろうとした深さ
    pub requested: u64,
}

impl Exceeded {
    /// この出来事を表す診断。場所は呼び出し側が足す
    pub fn to_diagnostic(&self) -> Diagnostic {
        Diagnostic::new(self.kind.exceeded_code())
            .with("limit", self.limit)
            .with("requested", self.requested)
    }
}

/// 呼び出しごとに 1 つ持つ予算。入れ子の読み取りも同じ予算から使う。
///
/// 一度上限を超えると、そのあとはどの単位も使えない。どのループも同じ所で止まる。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Budget {
    limits: Limits,
    used: PerKind<u64>,
    exceeded: Option<Exceeded>,
}

impl Budget {
    /// 上限を決めて作る
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            used: PerKind::default(),
            exceeded: None,
        }
    }

    /// 上限
    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// 使った量。`depth` では、今の深さ
    pub fn used(&self, kind: Kind) -> u64 {
        *self.used.get(kind)
    }

    /// 超えた上限。まだ超えていなければ `None`
    pub fn exceeded(&self) -> Option<Exceeded> {
        self.exceeded
    }

    /// 予算を使う。`depth` では、`amount` だけ深く入る
    pub fn spend(&mut self, kind: Kind, amount: u64) -> Result<(), Exceeded> {
        if let Some(exceeded) = self.exceeded {
            return Err(exceeded);
        }
        let used = self.used.get_mut(kind);
        let requested = used.saturating_add(amount);
        if let Limit::Finite(limit) = self.limits.get(kind)
            && requested > limit
        {
            let exceeded = Exceeded {
                kind,
                limit,
                requested,
            };
            self.exceeded = Some(exceeded);
            return Err(exceeded);
        }
        *used = requested;
        Ok(())
    }

    /// 入れ子を `levels` だけ出る
    pub fn leave(&mut self, levels: u64) {
        let depth = self.used.get_mut(Kind::Depth);
        *depth = depth.saturating_sub(levels);
    }
}

impl Default for Budget {
    fn default() -> Self {
        Self::new(Limits::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stops_at_the_limit_and_stays_stopped() {
        let mut budget = Budget::new(Limits::default().with(Kind::Records, Limit::Finite(3)));
        assert_eq!(budget.spend(Kind::Records, 2), Ok(()));
        assert_eq!(budget.spend(Kind::Records, 1), Ok(()));
        let exceeded = Exceeded {
            kind: Kind::Records,
            limit: 3,
            requested: 4,
        };
        assert_eq!(budget.spend(Kind::Records, 1), Err(exceeded));
        assert_eq!(budget.used(Kind::Records), 3);
        assert_eq!(budget.spend(Kind::Work, 1), Err(exceeded));
        assert_eq!(budget.exceeded(), Some(exceeded));
    }

    #[test]
    fn unlimited_never_stops() {
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Unlimited));
        assert_eq!(budget.spend(Kind::Work, u64::MAX), Ok(()));
        assert_eq!(budget.spend(Kind::Work, 1), Ok(()));
        assert_eq!(budget.used(Kind::Work), u64::MAX);
    }

    #[test]
    fn depth_counts_the_current_nesting() {
        let mut budget = Budget::new(Limits::default().with(Kind::Depth, Limit::Finite(2)));
        assert_eq!(budget.spend(Kind::Depth, 1), Ok(()));
        assert_eq!(budget.spend(Kind::Depth, 1), Ok(()));
        budget.leave(1);
        assert_eq!(budget.used(Kind::Depth), 1);
        assert_eq!(budget.spend(Kind::Depth, 1), Ok(()));
        assert!(budget.spend(Kind::Depth, 1).is_err());
    }

    #[test]
    fn every_default_is_finite() {
        let limits = Limits::default();
        for kind in Kind::ALL {
            assert!(matches!(limits.get(kind), Limit::Finite(_)), "{kind:?}");
        }
    }

    #[test]
    fn each_kind_has_its_exceeded_code() {
        for kind in Kind::ALL {
            let code = kind.exceeded_code();
            assert_eq!(code.name(), format!("budget.{}.exceeded", kind.name()));
            assert!(codes::ALL.contains(&code));
        }
    }

    #[test]
    fn exceeded_becomes_a_diagnostic_with_its_values() {
        let exceeded = Exceeded {
            kind: Kind::DecodedBytes,
            limit: 10,
            requested: 12,
        };
        let diagnostic = exceeded.to_diagnostic();
        assert_eq!(diagnostic.code(), codes::BUDGET_DECODED_BYTES_EXCEEDED);
        assert_eq!(diagnostic.param("limit"), Some(&10_u64.into()));
        assert_eq!(diagnostic.param("requested"), Some(&12_u64.into()));
    }
}
