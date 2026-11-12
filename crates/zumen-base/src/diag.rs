//! 診断と、そのまとめ方（決定 0009）。

use core::cmp::Ordering;
use std::collections::BTreeMap;

use crate::budget::{Budget, Kind};
use crate::cancel::CancelToken;
use crate::code::{Code, Fidelity, Severity};
use crate::cx::Stop;

/// 診断の値。人向けに整形した文字列は持たない。
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// 符号のない整数
    UInt(u64),
    /// 符号のある整数
    Int(i64),
    /// 浮動小数
    Float(f64),
    /// 図面に記録された名前などの文字列。翻訳しない
    Text(String),
    /// 列挙の値のキー。名前はカタログで引く
    Key(&'static str),
}

impl Value {
    fn rank(&self) -> u8 {
        match self {
            Value::UInt(_) => 0,
            Value::Int(_) => 1,
            Value::Float(_) => 2,
            Value::Text(_) => 3,
            Value::Key(_) => 4,
        }
    }

    /// 全順序で比べる。浮動小数は `total_cmp` で比べる（決定 0007）。
    fn total_cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Value::UInt(a), Value::UInt(b)) => a.cmp(b),
            (Value::Int(a), Value::Int(b)) => a.cmp(b),
            (Value::Float(a), Value::Float(b)) => a.total_cmp(b),
            (Value::Text(a), Value::Text(b)) => a.cmp(b),
            (Value::Key(a), Value::Key(b)) => a.cmp(b),
            _ => self.rank().cmp(&other.rank()),
        }
    }
}

impl From<u64> for Value {
    fn from(value: u64) -> Self {
        Value::UInt(value)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Int(value)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Float(value)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::Text(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::Text(value.to_owned())
    }
}

/// L1 のバイト範囲（`start..end`）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteRange {
    /// 先頭の位置
    pub start: u64,
    /// 終わりの位置（含まない）
    pub end: u64,
}

/// 形式が持つ、変わらないキー（決定 0156）。書き戻しても、前のレコードが増えても変わらない
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Key {
    /// AutoCAD の系統のハンドル
    Handle(u64),
    /// SXF のインスタンスの番号（`#n`）
    Instance(u64),
}

/// 診断の場所。ID と、L1 のレコード・バイト範囲（決定 0008 の出所）と、形式が持つキー
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ref {
    id: Option<String>,
    record: Option<u64>,
    bytes: Option<ByteRange>,
    key: Option<Key>,
}

impl Ref {
    /// L1 のレコードの番号で指す
    pub fn at_record(index: u64) -> Self {
        Self {
            record: Some(index),
            ..Self::default()
        }
    }

    /// L1 のバイト範囲で指す
    pub fn at_bytes(start: u64, end: u64) -> Self {
        Self {
            bytes: Some(ByteRange { start, end }),
            ..Self::default()
        }
    }

    /// ID を足す
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// レコードの番号を足す
    pub fn with_record(mut self, index: u64) -> Self {
        self.record = Some(index);
        self
    }

    /// バイト範囲を足す
    pub fn with_bytes(mut self, start: u64, end: u64) -> Self {
        self.bytes = Some(ByteRange { start, end });
        self
    }

    /// 形式が持つキーを足す
    pub fn with_key(mut self, key: Key) -> Self {
        self.key = Some(key);
        self
    }

    /// ID
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// L1 のレコードの番号
    pub fn record(&self) -> Option<u64> {
        self.record
    }

    /// L1 のバイト範囲
    pub fn bytes(&self) -> Option<ByteRange> {
        self.bytes
    }

    /// 形式が持つキー
    pub fn key(&self) -> Option<Key> {
        self.key
    }
}

/// 仕様の節。仕様の本文は写さない。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SpecRef {
    /// 仕様の一覧での名前
    pub spec: &'static str,
    /// 節
    pub section: &'static str,
}

/// 診断。`severity` と `fidelity` は code から決まる。
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    code: Code,
    count: u64,
    at: Vec<Ref>,
    cite: Option<SpecRef>,
    params: Vec<(&'static str, Value)>,
}

impl Diagnostic {
    /// 件数 1 の診断を作る
    pub fn new(code: Code) -> Self {
        Self {
            code,
            count: 1,
            at: Vec::new(),
            cite: None,
            params: Vec::new(),
        }
    }

    /// 値を入れる。名前は台帳の値の名前にする。同じ名前の値は置き換える
    pub fn with(mut self, name: &'static str, value: impl Into<Value>) -> Self {
        let value = value.into();
        match self.params.iter_mut().find(|(key, _)| *key == name) {
            Some((_, slot)) => *slot = value,
            None => self.params.push((name, value)),
        }
        self
    }

    /// 場所を足す
    pub fn at(mut self, place: Ref) -> Self {
        self.at.push(place);
        self
    }

    /// 仕様の節を入れる
    pub fn cite(mut self, spec: SpecRef) -> Self {
        self.cite = Some(spec);
        self
    }

    /// code
    pub fn code(&self) -> Code {
        self.code
    }

    /// 重さ
    pub fn severity(&self) -> Severity {
        self.code.severity()
    }

    /// 変換の結果の段
    pub fn fidelity(&self) -> Option<Fidelity> {
        self.code.fidelity()
    }

    /// まとめた件数
    pub fn count(&self) -> u64 {
        self.count
    }

    /// 場所。入力の順に、先頭の数件
    pub fn places(&self) -> &[Ref] {
        &self.at
    }

    /// 仕様の節
    pub fn spec(&self) -> Option<SpecRef> {
        self.cite
    }

    /// 値。入れた順
    pub fn params(&self) -> &[(&'static str, Value)] {
        &self.params
    }

    /// 名前で値を引く
    pub fn param(&self, name: &str) -> Option<&Value> {
        self.params
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value)
    }
}

/// まとめるときのキー。code の名前と、台帳で決めた値
#[derive(Clone, Debug)]
struct GroupKey {
    code: &'static str,
    values: Vec<Option<Value>>,
}

impl GroupKey {
    fn of(diagnostic: &Diagnostic) -> Self {
        let code = diagnostic.code();
        Self {
            code: code.name(),
            values: code
                .group_by()
                .iter()
                .map(|name| diagnostic.param(name).cloned())
                .collect(),
        }
    }
}

impl PartialEq for GroupKey {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for GroupKey {}

impl PartialOrd for GroupKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for GroupKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.code.cmp(other.code).then_with(|| {
            let mut pairs = self.values.iter().zip(&other.values);
            pairs
                .find_map(|pair| match pair {
                    (Some(a), Some(b)) => Some(a.total_cmp(b)).filter(|o| o.is_ne()),
                    (None, None) => None,
                    (None, Some(_)) => Some(Ordering::Less),
                    (Some(_), None) => Some(Ordering::Greater),
                })
                .unwrap_or_else(|| self.values.len().cmp(&other.values.len()))
        })
    }
}

/// 診断を、届いた順にまとめて持つ。
///
/// 同じ code で、台帳で決めた値が同じ診断は 1 つにまとめ、件数と先頭の数件の場所を持つ。
/// まとめた診断の値は、最初に届いたものを残す。
#[derive(Clone, Debug)]
pub struct Diagnostics {
    list: Vec<Diagnostic>,
    index: BTreeMap<GroupKey, usize>,
    max_places: usize,
    record_order: Option<RecordOrder>,
    pub(crate) events: Option<Box<EventCapture>>,
}

/// 集約前に報告された診断の発生件数・集計と、上限付きの明細。
///
/// [`crate::Cx::with_diagnostic_events`] で収集する。図形や要素の数ではない。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiagnosticEvents {
    /// 報告された診断の `count` の合計
    pub total: u64,
    /// code とカタログの `group_by` の値でまとめた診断。そのキーの順で、場所は持たない
    pub counts: Vec<Diagnostic>,
    /// 元の値と場所を保った、入力位置順の明細。同じ位置では報告順にする
    pub details: Vec<Diagnostic>,
    /// 明細に収めなかった報告がある
    pub truncated: bool,
    count_positions: Vec<RecordPosition>,
}

#[derive(Clone, Debug)]
pub(crate) struct EventCapture {
    pub(crate) parent: Option<Box<EventCapture>>,
    counts: Diagnostics,
    count_positions: Vec<(RecordPosition, u64)>,
    details: Vec<Diagnostic>,
    positions: Vec<(RecordPosition, u64)>,
    limit: u64,
    total: u64,
    reports: u64,
    pub(crate) stop: Option<Stop>,
}

#[derive(Clone, Copy)]
struct EventPlan {
    group: Option<usize>,
    replace_count: bool,
    detail: Option<usize>,
    position: RecordPosition,
}

fn event_check(
    budget: &mut Budget,
    cancel: &CancelToken,
    kind: Kind,
    amount: u64,
) -> Result<(), Stop> {
    if let Some(stop) = cancel.stop() {
        return Err(stop);
    }
    budget.spend(kind, amount).map_err(Stop::from)
}

fn value_bytes(value: &Value) -> u64 {
    match value {
        Value::Text(text) => text.len() as u64,
        _ => 0,
    }
}

fn diagnostic_bytes(diagnostic: &Diagnostic, places: bool) -> u64 {
    let params = (diagnostic.params.len() as u64)
        .saturating_mul(32)
        .saturating_add(diagnostic.params.iter().fold(0_u64, |sum, (_, value)| {
            sum.saturating_add(value_bytes(value))
        }));
    let places = if places {
        (diagnostic.at.len() as u64)
            .saturating_mul(64)
            .saturating_add(diagnostic.at.iter().fold(0_u64, |sum, place| {
                sum.saturating_add(place.id().map_or(0, |id| id.len() as u64))
            }))
    } else {
        0
    };
    128_u64.saturating_add(params).saturating_add(places)
}

fn diagnostic_group_cmp(left: &Diagnostic, right: &Diagnostic) -> Ordering {
    left.code.name().cmp(right.code.name()).then_with(|| {
        left.code
            .group_by()
            .iter()
            .find_map(|name| {
                let order = match (left.param(name), right.param(name)) {
                    (Some(left), Some(right)) => left.total_cmp(right),
                    (None, None) => Ordering::Equal,
                    (None, Some(_)) => Ordering::Less,
                    (Some(_), None) => Ordering::Greater,
                };
                order.is_ne().then_some(order)
            })
            .unwrap_or(Ordering::Equal)
    })
}

fn diagnostic_position(diagnostic: &Diagnostic) -> RecordPosition {
    diagnostic
        .at
        .iter()
        .map(record_position)
        .min()
        .unwrap_or(RecordPosition::Unknown)
}

impl EventCapture {
    pub(crate) const LOGICAL_BYTES: u64 = 256;

    pub(crate) fn new(limit: u64, parent: Option<Box<Self>>) -> Self {
        Self {
            parent,
            counts: Diagnostics::with_max_places(0),
            count_positions: Vec::new(),
            details: Vec::new(),
            positions: Vec::new(),
            limit,
            total: 0,
            reports: 0,
            stop: None,
        }
    }

    fn group(&self, diagnostic: &Diagnostic) -> Option<usize> {
        self.counts
            .list
            .iter()
            .position(|known| diagnostic_group_cmp(known, diagnostic).is_eq())
    }

    fn plan(&self, diagnostic: &Diagnostic) -> EventPlan {
        let position = diagnostic_position(diagnostic);
        let detail = self
            .positions
            .partition_point(|known| *known <= (position, self.reports));
        let group = self.group(diagnostic);
        EventPlan {
            group,
            replace_count: group
                .and_then(|index| self.count_positions.get(index))
                .is_none_or(|known| (position, self.reports) < *known),
            detail: ((detail as u64) < self.limit).then_some(detail),
            position,
        }
    }

    fn preflight(
        &self,
        diagnostic: &Diagnostic,
        budget: &mut Budget,
        cancel: &CancelToken,
    ) -> Result<(), Stop> {
        // 借用で読む項目の走査を先に数え、キー・値・場所を写す前に論理量を検査する。
        event_check(
            budget,
            cancel,
            Kind::Work,
            1_u64
                .saturating_add(diagnostic.params.len() as u64)
                .saturating_add(diagnostic.at.len() as u64),
        )?;
        let bytes = diagnostic_bytes(diagnostic, true);
        for known in &self.counts.list {
            let work = bytes
                .saturating_add(diagnostic_bytes(known, false))
                .saturating_mul(
                    (diagnostic.code.group_by().len() as u64)
                        .saturating_add(1)
                        .saturating_mul(2),
                );
            event_check(budget, cancel, Kind::Work, work)?;
        }
        // 計画と反映で 2 回探す分、明細の位置の探索と挿入で動かす分を含める。
        event_check(
            budget,
            cancel,
            Kind::Work,
            bytes.saturating_add((self.details.len() as u64).saturating_mul(4)),
        )?;
        let plan = self.plan(diagnostic);
        let mut allocation = 0_u64;
        if plan.group.is_none() {
            let key = diagnostic.code.group_by().iter().fold(0_u64, |sum, name| {
                sum.saturating_add(32)
                    .saturating_add(diagnostic.param(name).map_or(0, value_bytes))
            });
            // BTreeMap のノードと Vec の領域は、ポインター幅によらない固定量で数える。
            allocation = 544_u64.saturating_add(key);
        }
        if plan.replace_count {
            allocation = allocation.saturating_add(diagnostic_bytes(diagnostic, false));
        }
        if plan.detail.is_some() {
            allocation = allocation.saturating_add(bytes).saturating_add(32);
        }
        event_check(budget, cancel, Kind::Work, allocation)?;
        event_check(budget, cancel, Kind::DecodedBytes, allocation)
    }

    fn record(&mut self, diagnostic: &Diagnostic) {
        let plan = self.plan(diagnostic);
        self.total = self.total.saturating_add(diagnostic.count);
        let summary = || Diagnostic {
            code: diagnostic.code,
            count: diagnostic.count,
            at: Vec::new(),
            cite: diagnostic.cite,
            params: diagnostic.params.clone(),
        };
        if let Some(index) = plan.group {
            if let Some(existing) = self.counts.list.get_mut(index) {
                let count = existing.count.saturating_add(diagnostic.count);
                if plan.replace_count {
                    *existing = summary();
                    if let Some(position) = self.count_positions.get_mut(index) {
                        *position = (plan.position, self.reports);
                    }
                }
                existing.count = count;
            }
        } else {
            self.counts.report(summary());
            self.count_positions.push((plan.position, self.reports));
        }
        if let Some(index) = plan.detail {
            if self.details.len() as u64 == self.limit {
                self.details.pop();
                self.positions.pop();
            }
            self.details.insert(index, diagnostic.clone());
            self.positions.insert(index, (plan.position, self.reports));
        }
        self.reports = self.reports.saturating_add(1);
    }

    pub(crate) fn finish(
        self,
        budget: &mut Budget,
        cancel: &CancelToken,
    ) -> Result<DiagnosticEvents, Stop> {
        let count = self.counts.list.len() as u64;
        let comparisons = count.saturating_mul(u64::from(count.max(1).ilog2()).saturating_add(1));
        let mut key_work = 1_u64;
        event_check(budget, cancel, Kind::Work, count)?;
        for diagnostic in &self.counts.list {
            key_work = key_work.max(
                diagnostic_bytes(diagnostic, false)
                    .saturating_mul((diagnostic.code.group_by().len() as u64).saturating_add(1)),
            );
        }
        event_check(
            budget,
            cancel,
            Kind::Work,
            comparisons.saturating_mul(key_work),
        )?;
        event_check(
            budget,
            cancel,
            Kind::DecodedBytes,
            count.saturating_mul(320),
        )?;
        let mut ordered: Vec<_> = self
            .counts
            .list
            .into_iter()
            .zip(self.count_positions)
            .collect();
        // 不安定ソートは領域を確保しない。集計のキーは重複していない。
        ordered.sort_unstable_by(|(left, _), (right, _)| diagnostic_group_cmp(left, right));
        let mut counts = Vec::with_capacity(ordered.len());
        let mut count_positions = Vec::with_capacity(ordered.len());
        for (diagnostic, (position, _)) in ordered {
            counts.push(diagnostic);
            count_positions.push(position);
        }
        Ok(DiagnosticEvents {
            total: self.total,
            counts,
            truncated: self.reports > self.details.len() as u64,
            details: self.details,
            count_positions,
        })
    }
}

impl DiagnosticEvents {
    /// 同じ入力の別の処理で捕捉した診断を合わせ、入力位置順の明細を `limit` 件まで残す。
    ///
    /// 集計は全件を合わせる。同じ位置では `self` の報告を先にする。
    /// それぞれの収集時に省いた明細は復元できないため、`truncated` を引き継ぐ。
    /// 明細の上限を収集時より増やした場合、元の全報告の先頭件数を復元できるとは限らない。
    /// 合流は診断の再報告をせず、周囲の収集に重ねて捕捉されない。
    pub fn merge(self, other: Self, limit: u64, cx: &mut crate::Cx<'_>) -> Result<Self, Stop> {
        let result = self.merge_inner(other, limit, cx);
        if let Err(stop) = result
            && cx.diag.collection_stop().is_none()
        {
            cx.diag.stop_record_order(stop, stop.to_diagnostic());
        }
        result
    }

    fn merge_inner(self, other: Self, limit: u64, cx: &mut crate::Cx<'_>) -> Result<Self, Stop> {
        let Self {
            total,
            counts,
            details,
            truncated,
            count_positions,
        } = self;
        let total = total.saturating_add(other.total);
        let truncated = truncated || other.truncated;
        let count_len = counts.len().saturating_add(other.counts.len());
        cx.check(Kind::Work, count_len as u64)?;
        cx.check(Kind::DecodedBytes, (count_len as u64).saturating_mul(160))?;
        let mut merged: Vec<(Diagnostic, RecordPosition)> = Vec::with_capacity(count_len);
        let mut left = counts.into_iter().zip(count_positions).peekable();
        let mut right = other
            .counts
            .into_iter()
            .zip(other.count_positions)
            .peekable();
        while left.peek().is_some() || right.peek().is_some() {
            cx.check(Kind::Work, 1)?;
            let order = match (left.peek(), right.peek()) {
                (Some((left, _)), Some((right, _))) => {
                    let work = diagnostic_bytes(left, false)
                        .saturating_add(diagnostic_bytes(right, false))
                        .saturating_mul((left.code.group_by().len() as u64).saturating_add(1));
                    cx.check(Kind::Work, work)?;
                    diagnostic_group_cmp(left, right)
                }
                (Some(_), None) => Ordering::Less,
                _ => Ordering::Greater,
            };
            if order.is_eq() {
                if let (Some((mut left, left_position)), Some((mut right, right_position))) =
                    (left.next(), right.next())
                {
                    let count = left.count.saturating_add(right.count);
                    if right_position < left_position {
                        right.count = count;
                        merged.push((right, right_position));
                    } else {
                        left.count = count;
                        merged.push((left, left_position));
                    }
                }
            } else if order.is_lt() {
                if let Some(item) = left.next() {
                    merged.push(item);
                }
            } else if let Some(item) = right.next() {
                merged.push(item);
            }
        }
        let detail_len = details.len().saturating_add(other.details.len());
        let retained = (detail_len as u64).min(limit);
        cx.check(Kind::Work, detail_len as u64)?;
        cx.check(Kind::DecodedBytes, retained.saturating_mul(128))?;
        let mut merged_details =
            Vec::with_capacity(usize::try_from(retained).unwrap_or(detail_len));
        let mut left = details.into_iter().peekable();
        let mut right = other.details.into_iter().peekable();
        while (merged_details.len() as u64) < retained {
            cx.check(Kind::Work, 1)?;
            let take_left = match (left.peek(), right.peek()) {
                (Some(left), Some(right)) => {
                    cx.check(
                        Kind::Work,
                        (left.at.len() as u64).saturating_add(right.at.len() as u64),
                    )?;
                    diagnostic_position(left) <= diagnostic_position(right)
                }
                (Some(_), None) => true,
                _ => false,
            };
            let item = if take_left { left.next() } else { right.next() };
            if let Some(item) = item {
                merged_details.push(item);
            } else {
                break;
            }
        }
        cx.check(Kind::Work, merged.len() as u64)?;
        cx.check(
            Kind::DecodedBytes,
            (merged.len() as u64).saturating_mul(160),
        )?;
        let mut counts = Vec::with_capacity(merged.len());
        let mut count_positions = Vec::with_capacity(merged.len());
        for (diagnostic, position) in merged {
            counts.push(diagnostic);
            count_positions.push(position);
        }
        Ok(Self {
            total,
            counts,
            details: merged_details,
            truncated: truncated || detail_len as u64 > retained,
            count_positions,
        })
    }
}

/// 形式がレコード番号を持てばその番号を、バイト範囲だけなら開始位置を使う。
/// 比較できる位置が同じときは届いた順にし、位置のない診断は最後にする。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum RecordPosition {
    Record(u64),
    Byte(u64),
    Unknown,
}

fn record_position(place: &Ref) -> RecordPosition {
    if let Some(record) = place.record() {
        RecordPosition::Record(record)
    } else if let Some(bytes) = place.bytes() {
        RecordPosition::Byte(bytes.start)
    } else {
        RecordPosition::Unknown
    }
}

#[derive(Clone, Debug, Default)]
struct RecordOrder {
    first: Vec<(RecordPosition, u64)>,
    reports: u64,
    stop: Option<Stop>,
}

impl Diagnostics {
    /// 診断ごとに持つ場所の数の既定値
    pub const DEFAULT_MAX_PLACES: usize = 16;

    /// 空のものを作る
    pub fn new() -> Self {
        Self::with_max_places(Self::DEFAULT_MAX_PLACES)
    }

    /// 診断ごとに持つ場所の数を決めて作る
    pub fn with_max_places(max_places: usize) -> Self {
        Self {
            list: Vec::new(),
            index: BTreeMap::new(),
            max_places,
            record_order: None,
            events: None,
        }
    }

    pub(crate) fn in_record_order(max_places: usize) -> Self {
        Self {
            record_order: Some(RecordOrder::default()),
            ..Self::with_max_places(max_places)
        }
    }

    pub(crate) fn is_collecting_in_record_order(&self) -> bool {
        self.record_order.is_some()
    }

    /// 収集と、最後の整列・転送の分を先に数える。場所の挿入で動かす要素も上限に含める。
    pub(crate) fn record_order_work(&self, diagnostic: &Diagnostic) -> Option<u64> {
        self.record_order.as_ref()?;
        let existing = self
            .index
            .get(&GroupKey::of(diagnostic))
            .and_then(|&index| self.list.get(index))
            .map_or(0, |diagnostic| diagnostic.at.len());
        let places = diagnostic.at.len() as u64;
        let retained = self
            .max_places
            .min(existing.saturating_add(diagnostic.at.len())) as u64;
        // 1 報告と 1 転送、場所の走査・転送と挿入時の移動。木と整列の比較は各操作に含める。
        Some(2_u64.saturating_add(places.saturating_mul(2_u64.saturating_add(retained))))
    }

    pub(crate) fn record_order_stop(&self) -> Option<Stop> {
        self.record_order.as_ref().and_then(|order| order.stop)
    }

    pub(crate) fn collection_stop(&self) -> Option<Stop> {
        self.record_order_stop()
            .or_else(|| self.events.as_ref().and_then(|events| events.stop))
    }

    pub(crate) fn preflight_event(
        &self,
        diagnostic: &Diagnostic,
        budget: &mut Budget,
        cancel: &CancelToken,
    ) -> Result<(), Stop> {
        let mut capture = self.events.as_deref();
        while let Some(events) = capture {
            events.preflight(diagnostic, budget, cancel)?;
            capture = events.parent.as_deref();
        }
        Ok(())
    }

    pub(crate) fn record_event(&mut self, diagnostic: &Diagnostic) {
        let mut capture = self.events.as_deref_mut();
        while let Some(events) = capture {
            events.record(diagnostic);
            capture = events.parent.as_deref_mut();
        }
    }

    pub(crate) fn stop_record_order(&mut self, stop: Stop, diagnostic: Diagnostic) {
        let mut stopped = stop.to_diagnostic();
        if let Some(place) = diagnostic.at.into_iter().min_by_key(record_position) {
            stopped = stopped.at(place);
        }
        // 停止の報告は 1 件だけにし、予算を使い切った後にも残す。
        self.report(stopped);
        if let Some(order) = &mut self.record_order {
            order.stop = Some(stop);
        }
        let mut capture = self.events.as_deref_mut();
        while let Some(events) = capture {
            events.stop = Some(stop);
            capture = events.parent.as_deref_mut();
        }
    }

    /// 診断を足す。まとめられるものはまとめる
    pub fn report(&mut self, diagnostic: Diagnostic) {
        if self.record_order.is_some() {
            self.report_in_record_order(diagnostic);
            return;
        }
        let key = GroupKey::of(&diagnostic);
        if let Some(existing) = self
            .index
            .get(&key)
            .and_then(|&position| self.list.get_mut(position))
        {
            existing.count = existing.count.saturating_add(diagnostic.count);
            let room = self.max_places.saturating_sub(existing.at.len());
            existing.at.extend(diagnostic.at.into_iter().take(room));
            return;
        }
        let mut diagnostic = diagnostic;
        diagnostic.at.truncate(self.max_places);
        self.index.insert(key, self.list.len());
        self.list.push(diagnostic);
    }

    fn report_in_record_order(&mut self, mut diagnostic: Diagnostic) {
        let Some(order) = &mut self.record_order else {
            return;
        };
        if order.stop.is_some() {
            return;
        }
        let first = (
            diagnostic
                .at
                .iter()
                .map(record_position)
                .min()
                .unwrap_or(RecordPosition::Unknown),
            order.reports,
        );
        order.reports = order.reports.saturating_add(1);
        let key = GroupKey::of(&diagnostic);
        let places = core::mem::take(&mut diagnostic.at);
        let position = if let Some(&position) = self.index.get(&key) {
            let (Some(existing), Some(earliest)) =
                (self.list.get_mut(position), order.first.get_mut(position))
            else {
                return;
            };
            let count = existing.count.saturating_add(diagnostic.count);
            if first < *earliest {
                diagnostic.at = core::mem::take(&mut existing.at);
                *existing = diagnostic;
                *earliest = first;
            }
            existing.count = count;
            position
        } else {
            let position = self.list.len();
            self.index.insert(key, position);
            self.list.push(diagnostic);
            order.first.push(first);
            position
        };
        let Some(existing) = self.list.get_mut(position) else {
            return;
        };
        let retained = &mut existing.at;
        for place in places {
            let position =
                retained.partition_point(|known| record_position(known) <= record_position(&place));
            if position < self.max_places {
                // 挿入する前に末尾を外し、max_places を一時的にも超えない。
                if retained.len() == self.max_places {
                    retained.pop();
                }
                retained.insert(position, place);
            }
        }
    }

    /// 診断ごとに持つ場所の数
    pub fn max_places(&self) -> usize {
        self.max_places
    }

    /// 届いた順に返す
    pub fn iter(&self) -> core::slice::Iter<'_, Diagnostic> {
        self.list.iter()
    }

    /// まとめたあとの診断の数
    pub fn len(&self) -> usize {
        self.list.len()
    }

    /// 診断がないか
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// `error` の診断があるか
    pub fn has_errors(&self) -> bool {
        self.list
            .iter()
            .any(|diagnostic| diagnostic.severity() == Severity::Error)
    }

    /// 診断を取り出す
    pub fn into_vec(self) -> Vec<Diagnostic> {
        let Some(order) = self.record_order else {
            return self.list;
        };
        let mut ordered: Vec<_> = order.first.into_iter().zip(self.list).collect();
        ordered.sort_by_key(|(position, _)| *position);
        ordered
            .into_iter()
            .map(|(_, diagnostic)| diagnostic)
            .collect()
    }
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> IntoIterator for &'a Diagnostics {
    type Item = &'a Diagnostic;
    type IntoIter = core::slice::Iter<'a, Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::{Param, ParamType};
    use crate::codes;

    /// 値でまとめる code。テストのためだけに作る
    const BY_KIND: Code = Code::new(
        "test.kind.dropped",
        Severity::Warning,
        Some(Fidelity::Dropped),
        &[Param::new("kind", ParamType::Key)],
        &["kind"],
    );

    #[test]
    fn merges_the_same_code_and_keeps_the_first_places() {
        let mut diag = Diagnostics::with_max_places(2);
        for index in 0..3 {
            diag.report(
                Diagnostic::new(codes::ZIP_PATH_UNSAFE)
                    .with("name", format!("../{index}"))
                    .at(Ref::at_record(index)),
            );
        }
        assert_eq!(diag.len(), 1);
        let merged = diag.iter().next().unwrap();
        assert_eq!(merged.count(), 3);
        assert_eq!(merged.places(), [Ref::at_record(0), Ref::at_record(1)]);
        assert_eq!(merged.param("name"), Some(&Value::from("../0")));
    }

    #[test]
    fn separates_by_the_values_in_group_by_and_keeps_arrival_order() {
        let mut diag = Diagnostics::new();
        diag.report(Diagnostic::new(BY_KIND).with("kind", Value::Key("spline")));
        diag.report(Diagnostic::new(BY_KIND).with("kind", Value::Key("hatch")));
        diag.report(Diagnostic::new(BY_KIND).with("kind", Value::Key("spline")));
        let kinds: Vec<(Option<&Value>, u64)> =
            diag.iter().map(|d| (d.param("kind"), d.count())).collect();
        assert_eq!(
            kinds,
            [
                (Some(&Value::Key("spline")), 2),
                (Some(&Value::Key("hatch")), 1)
            ]
        );
    }

    #[test]
    fn severity_and_fidelity_come_from_the_code() {
        let diagnostic = Diagnostic::new(BY_KIND);
        assert_eq!(diagnostic.severity(), Severity::Warning);
        assert_eq!(diagnostic.fidelity(), Some(Fidelity::Dropped));
        let mut diag = Diagnostics::new();
        diag.report(diagnostic);
        assert!(!diag.has_errors());
        diag.report(Diagnostic::new(codes::CALL_REQUEST_CANCELLED));
        assert!(diag.has_errors());
    }

    #[test]
    fn replaces_a_value_with_the_same_name() {
        let diagnostic = Diagnostic::new(codes::BUDGET_RECORDS_EXCEEDED)
            .with("limit", 1_u64)
            .with("limit", 2_u64);
        assert_eq!(diagnostic.params(), [("limit", Value::UInt(2))]);
    }

    #[test]
    fn orders_floats_totally() {
        let nan = Value::Float(f64::NAN);
        assert_eq!(nan.total_cmp(&nan), Ordering::Equal);
        assert_eq!(
            Value::Float(-0.0).total_cmp(&Value::Float(0.0)),
            Ordering::Less
        );
        assert_eq!(
            Value::UInt(1).total_cmp(&Value::Text("a".into())),
            Ordering::Less
        );
    }
}
