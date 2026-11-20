//! 要素どうしの依存（寸法と測る相手、ハッチングと境界、挿入と属性、壁と開口など）を持つ有向グラフ。
//!
//! 変更があった行から依存する側（再計算・再描画が要る行）を、環境によらない決まった順で取り出す。
//! 循環があっても無限ループせず、取り消し用に前の辺の状態をそのまま戻せる。

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use zumen_base::{Cx, Kind, Stop};

use crate::handle::RowId;
use crate::paged::Paged;

/// 取り消し用に記録する、1 つの行に関わる変更前の依存辺
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DepBefore {
    row: RowId,
    sources: Vec<RowId>,
    dependents: Vec<RowId>,
}

impl DepBefore {
    /// 保持するヒープバイト数の目安
    pub fn heap_bytes(&self) -> usize {
        (self.sources.capacity() + self.dependents.capacity()) * core::mem::size_of::<RowId>()
    }

    fn edge_count(&self) -> u64 {
        (self.sources.len() as u64).saturating_add(self.dependents.len() as u64)
    }
}

/// 行どうしの依存グラフ
#[derive(Clone, Debug, Default)]
pub struct DepGraph {
    /// 実在行がちょうど 1 つの参照元を持つときの高速列（0 個または 2 個以上なら `RowId::NONE`）
    single_source: Paged<RowId>,
    /// インライン配列に載せる行番号の上限目安
    dense_limit: usize,
    /// 仮想ノード、疎な行番号、または 2 個以上の参照元を持つ行の集合
    many_sources: BTreeMap<RowId, BTreeSet<RowId>>,
    /// `source -> その行に依存する dependent 行の集合`
    deps_by_source: BTreeMap<RowId, BTreeSet<RowId>>,
}

impl PartialEq for DepGraph {
    fn eq(&self, other: &Self) -> bool {
        self.deps_by_source == other.deps_by_source
    }
}

impl Eq for DepGraph {}

impl DepGraph {
    /// 空の依存グラフを作る
    pub fn new() -> Self {
        Self::default()
    }

    /// 初期構築用に `rows` 行までの単一参照元スロット利用を許可する（ページは辺があるときだけ確保する）
    pub fn with_row_capacity(rows: usize) -> Self {
        Self {
            single_source: Paged::with_capacity(rows),
            dense_limit: rows,
            many_sources: BTreeMap::new(),
            deps_by_source: BTreeMap::new(),
        }
    }

    #[inline]
    fn can_inline_single(&self, row: RowId) -> bool {
        let cap = self.dense_limit.max(self.single_source.len());
        row.0 < RowId::VIRTUAL_TAG && row.index() <= cap.saturating_add(4096)
    }

    #[inline]
    fn inline_single(&self, row: RowId) -> Option<RowId> {
        if row.0 >= RowId::VIRTUAL_TAG {
            return None;
        }
        self.single_source
            .get(row.index())
            .copied()
            .filter(|s| !s.is_none())
    }

    #[inline]
    fn put_single(&mut self, row: RowId, src: RowId) {
        let idx = row.index();
        if idx >= self.single_source.len() {
            self.single_source.resize(idx + 1, RowId::NONE);
        }
        if let Some(slot) = self.single_source.get_mut(idx) {
            *slot = src;
        }
    }

    fn insert_forward_only(&mut self, dependent: RowId, src: RowId) {
        if let Some(set) = self.many_sources.get_mut(&dependent) {
            set.insert(src);
            return;
        }
        if let Some(existing) = self.inline_single(dependent) {
            if existing == src {
                return;
            }
            if let Some(slot) = self.single_source.get_mut(dependent.index()) {
                *slot = RowId::NONE;
            }
            let mut set = BTreeSet::new();
            set.insert(existing);
            set.insert(src);
            self.many_sources.insert(dependent, set);
            return;
        }
        if self.can_inline_single(dependent) {
            self.put_single(dependent, src);
        } else {
            self.many_sources.entry(dependent).or_default().insert(src);
        }
    }

    fn remove_forward_only(&mut self, dependent: RowId, src: RowId) {
        if let Some(existing) = self.inline_single(dependent) {
            if existing == src
                && let Some(slot) = self.single_source.get_mut(dependent.index())
            {
                *slot = RowId::NONE;
            }
            return;
        }
        let empty = if let Some(srcs) = self.many_sources.get_mut(&dependent) {
            srcs.remove(&src);
            srcs.is_empty()
        } else {
            false
        };
        if empty {
            self.many_sources.remove(&dependent);
        }
    }

    /// `row` に関わる現在の辺を、取り消し用に記録する
    pub fn snapshot_row(&self, row: RowId) -> DepBefore {
        let sources = self.sources_of(row);
        let dependents = self
            .deps_by_source
            .get(&row)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default();
        DepBefore {
            row,
            sources,
            dependents,
        }
    }

    pub(crate) fn snapshot_row_with_cx(
        &self,
        row: RowId,
        cx: &mut Cx<'_>,
    ) -> Result<DepBefore, Stop> {
        let edges = self.row_edge_count(row);
        cx.check(Kind::Work, edges.saturating_add(1))?;
        cx.check(Kind::DecodedBytes, edges.saturating_mul(4))?;
        Ok(self.snapshot_row(row))
    }

    fn source_count(&self, row: RowId) -> u64 {
        if self.inline_single(row).is_some() {
            1
        } else {
            self.many_sources
                .get(&row)
                .map_or(0, |sources| sources.len() as u64)
        }
    }

    fn row_edge_count(&self, row: RowId) -> u64 {
        self.source_count(row).saturating_add(
            self.deps_by_source
                .get(&row)
                .map_or(0, |deps| deps.len() as u64),
        )
    }

    pub(crate) fn check_clear_dependent(&self, row: RowId, cx: &mut Cx<'_>) -> Result<(), Stop> {
        cx.check(
            Kind::Work,
            self.source_count(row).saturating_mul(2).saturating_add(1),
        )
    }

    pub(crate) fn check_remove_row(&self, row: RowId, cx: &mut Cx<'_>) -> Result<(), Stop> {
        cx.check(
            Kind::Work,
            self.row_edge_count(row).saturating_mul(2).saturating_add(1),
        )
    }

    pub(crate) fn check_set_sources(
        &self,
        row: RowId,
        sources: usize,
        cx: &mut Cx<'_>,
    ) -> Result<(), Stop> {
        self.check_clear_dependent(row, cx)?;
        let edges = sources as u64;
        cx.check(Kind::Work, edges.saturating_mul(3))?;
        Self::check_new_edges(edges, u64::from(edges != 0), cx)
    }

    fn check_new_edges(edges: u64, rows: u64, cx: &mut Cx<'_>) -> Result<(), Stop> {
        // 両向きの木と集合の節点を、ポインター幅に依らない余裕を含む量で数える。
        cx.check(
            Kind::DecodedBytes,
            edges
                .saturating_mul(768)
                .saturating_add(rows.saturating_mul(512)),
        )
    }

    pub(crate) fn check_restore_rows(
        &self,
        before: &[DepBefore],
        cx: &mut Cx<'_>,
    ) -> Result<(), Stop> {
        let mut current = 0u64;
        let mut restored = 0u64;
        let mut rows = 0u64;
        for row in before {
            cx.check(Kind::Work, 1)?;
            current = current.saturating_add(self.row_edge_count(row.row));
            restored = restored.saturating_add(row.edge_count());
            rows = rows.saturating_add(u64::from(row.edge_count() != 0));
        }
        // 先の復元で足した辺を、次の行の復元で一度除いて戻す場合も数える。
        cx.check(
            Kind::Work,
            current
                .saturating_mul(2)
                .saturating_add(restored.saturating_mul(6)),
        )?;
        Self::check_new_edges(restored, rows, cx)
    }

    /// 初期構築時に単一参照元の辺 `(source, dependent)` の列を一括で登録する
    pub fn insert_initial_single_edges(&mut self, mut edges: Vec<(RowId, RowId)>) {
        if edges.is_empty() {
            return;
        }
        edges.retain(|&(src, dep)| src != dep && !src.is_none() && !dep.is_none());
        edges.sort_unstable();
        edges.dedup();
        for &(src, dep) in &edges {
            self.insert_forward_only(dep, src);
        }
        for slice in edges.chunk_by(|a, b| a.0 == b.0) {
            let Some(&(src, _)) = slice.first() else {
                continue;
            };
            if let Some(existing) = self.deps_by_source.get_mut(&src) {
                existing.extend(slice.iter().map(|&(_, dep)| dep));
            } else {
                let set: BTreeSet<RowId> = slice.iter().map(|&(_, dep)| dep).collect();
                self.deps_by_source.insert(src, set);
            }
        }
    }

    /// `dependent` がただ 1 つの参照元 `source` を持つように置き換える
    pub fn set_single_source(&mut self, dependent: RowId, source: RowId) {
        self.clear_dependent(dependent);
        if source == dependent || source.is_none() || dependent.is_none() {
            return;
        }
        self.deps_by_source
            .entry(source)
            .or_default()
            .insert(dependent);
        if self.can_inline_single(dependent) {
            self.put_single(dependent, source);
        } else {
            let mut set = BTreeSet::new();
            set.insert(source);
            self.many_sources.insert(dependent, set);
        }
    }

    /// `dependent` が参照する元の行の集合を `sources` に置き換える（自分自身への辺は除く）
    pub fn set_sources(&mut self, dependent: RowId, sources: impl IntoIterator<Item = RowId>) {
        self.clear_dependent(dependent);
        if dependent.is_none() {
            return;
        }
        let mut first: Option<RowId> = None;
        let mut many: Option<BTreeSet<RowId>> = None;
        for src in sources {
            if src == dependent || src.is_none() {
                continue;
            }
            if let Some(set) = &mut many {
                set.insert(src);
            } else if let Some(prev) = first {
                if prev != src {
                    let mut set = BTreeSet::new();
                    set.insert(prev);
                    set.insert(src);
                    many = Some(set);
                }
            } else {
                first = Some(src);
            }
        }
        if let Some(set) = many {
            for &src in &set {
                self.deps_by_source
                    .entry(src)
                    .or_default()
                    .insert(dependent);
            }
            self.many_sources.insert(dependent, set);
        } else if let Some(src) = first {
            self.deps_by_source
                .entry(src)
                .or_default()
                .insert(dependent);
            if self.can_inline_single(dependent) {
                self.put_single(dependent, src);
            } else {
                let mut set = BTreeSet::new();
                set.insert(src);
                self.many_sources.insert(dependent, set);
            }
        }
    }

    /// `dependent` が持つ参照元の辺をすべて外す
    pub fn clear_dependent(&mut self, dependent: RowId) {
        if let Some(src) = self.inline_single(dependent) {
            if let Some(slot) = self.single_source.get_mut(dependent.index()) {
                *slot = RowId::NONE;
            }
            let remove_key = if let Some(deps) = self.deps_by_source.get_mut(&src) {
                deps.remove(&dependent);
                deps.is_empty()
            } else {
                false
            };
            if remove_key {
                self.deps_by_source.remove(&src);
            }
            return;
        }
        let Some(old_sources) = self.many_sources.remove(&dependent) else {
            return;
        };
        for src in old_sources {
            let remove_key = if let Some(deps) = self.deps_by_source.get_mut(&src) {
                deps.remove(&dependent);
                deps.is_empty()
            } else {
                false
            };
            if remove_key {
                self.deps_by_source.remove(&src);
            }
        }
    }

    /// 除かれた `row` について、参照元としての辺と依存側としての辺の両方を外す
    pub fn remove_row(&mut self, row: RowId) {
        self.clear_dependent(row);
        if let Some(deps) = self.deps_by_source.remove(&row) {
            for dep in deps {
                self.remove_forward_only(dep, row);
            }
        }
    }

    /// [`Self::snapshot_row`] で記録した状態に戻す
    pub fn restore_row(&mut self, before: &DepBefore) {
        self.remove_row(before.row);
        if !before.sources.is_empty() {
            self.set_sources(before.row, before.sources.iter().copied());
        }
        for &dep in &before.dependents {
            if dep != before.row && !dep.is_none() {
                self.insert_forward_only(dep, before.row);
                self.deps_by_source
                    .entry(before.row)
                    .or_default()
                    .insert(dep);
            }
        }
    }

    /// `dependent` が直接参照している行を昇順で返す
    pub fn sources_of(&self, dependent: RowId) -> Vec<RowId> {
        if let Some(src) = self.inline_single(dependent) {
            return vec![src];
        }
        self.many_sources
            .get(&dependent)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    /// `source` に直接依存している行を昇順で返す
    pub fn dependents_of(&self, source: RowId) -> Vec<RowId> {
        self.deps_by_source
            .get(&source)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    /// `changed_roots` が変わったときに再評価が要る依存行を、上流から順（同順位は行番号の昇順）に返す。
    /// `changed_roots` 自体は、ほかの変更行から依存されていない限り結果に含めない。
    pub fn affected_rows(&self, changed_roots: &[RowId]) -> Vec<RowId> {
        let mut visited = BTreeSet::new();
        let mut queue: VecDeque<RowId> = VecDeque::new();
        let mut roots_sorted: Vec<RowId> = changed_roots.to_vec();
        roots_sorted.sort_unstable();
        roots_sorted.dedup();
        for root in roots_sorted {
            if let Some(deps) = self.deps_by_source.get(&root) {
                for &dep in deps {
                    if visited.insert(dep) {
                        queue.push_back(dep);
                    }
                }
            }
        }
        while let Some(cur) = queue.pop_front() {
            if let Some(deps) = self.deps_by_source.get(&cur) {
                for &dep in deps {
                    if visited.insert(dep) {
                        queue.push_back(dep);
                    }
                }
            }
        }
        self.order_affected(&visited)
    }

    /// 予算と中断を確かめながら、再評価が要る依存行を上流から順に返す
    pub fn affected_rows_with_cx(
        &self,
        changed_roots: &[RowId],
        cx: &mut Cx<'_>,
    ) -> Result<Vec<RowId>, Stop> {
        let roots = changed_roots.len() as u64;
        cx.check(Kind::Work, roots.max(1))?;
        cx.check(Kind::DecodedBytes, roots.saturating_mul(4))?;
        let mut visited = BTreeSet::new();
        let mut queue: VecDeque<RowId> = VecDeque::new();
        let mut roots_sorted: Vec<RowId> = changed_roots.to_vec();
        cx.check(
            Kind::Work,
            roots.saturating_mul(u64::from(roots.max(1).ilog2()) + 1),
        )?;
        roots_sorted.sort_unstable();
        roots_sorted.dedup();
        for root in roots_sorted {
            cx.check(Kind::Work, 1)?;
            if let Some(deps) = self.deps_by_source.get(&root) {
                for &dep in deps {
                    cx.check(Kind::Work, 1)?;
                    if !visited.contains(&dep) {
                        cx.check(
                            Kind::DecodedBytes,
                            if visited.is_empty() { 256 } else { 128 },
                        )?;
                        visited.insert(dep);
                        queue.push_back(dep);
                    }
                }
            }
        }
        while let Some(cur) = queue.pop_front() {
            cx.check(Kind::Work, 1)?;
            if let Some(deps) = self.deps_by_source.get(&cur) {
                for &dep in deps {
                    cx.check(Kind::Work, 1)?;
                    if !visited.contains(&dep) {
                        cx.check(Kind::DecodedBytes, 128)?;
                        visited.insert(dep);
                        queue.push_back(dep);
                    }
                }
            }
        }
        self.order_affected_checked(&visited, |kind, amount| cx.check(kind, amount))
    }

    /// 指定したノード集合 `nodes` の誘導部分グラフを、上流から順（同順位・循環部は `RowId` 昇順）に並べて返す
    pub fn topological_order(&self, nodes: &BTreeSet<RowId>) -> Vec<RowId> {
        self.order_affected(nodes)
    }

    /// 予算と中断を確かめながら、指定した集合を上流から順に並べる
    pub fn topological_order_with_cx(
        &self,
        nodes: &BTreeSet<RowId>,
        cx: &mut Cx<'_>,
    ) -> Result<Vec<RowId>, Stop> {
        cx.check(Kind::Work, 1)?;
        self.order_affected_checked(nodes, |kind, amount| cx.check(kind, amount))
    }

    fn order_affected(&self, affected: &BTreeSet<RowId>) -> Vec<RowId> {
        let result: Result<_, core::convert::Infallible> =
            self.order_affected_checked(affected, |_, _| Ok(()));
        match result {
            Ok(ordered) => ordered,
            Err(never) => match never {},
        }
    }

    fn order_affected_checked<E>(
        &self,
        affected: &BTreeSet<RowId>,
        mut check: impl FnMut(Kind, u64) -> Result<(), E>,
    ) -> Result<Vec<RowId>, E> {
        if affected.is_empty() {
            return Ok(Vec::new());
        }
        // 部分グラフ内の入次数を、`affected` 内の頂点からの出力辺だけで数える（未変更の上流参照元が大量にあっても走査しない）。
        // Kahn 法（BTreeSet で最小の RowId から取り出す）でトポロジカル順に並べ、循環があれば残った頂点を行番号の昇順で後ろに足す。
        // 木の節点と待ち行列の余裕を含む、ポインター幅に依らない見積もり。
        check(Kind::DecodedBytes, 128)?;
        let mut in_deg: BTreeMap<RowId, usize> = BTreeMap::new();
        for &node in affected {
            check(Kind::Work, 1)?;
            check(Kind::DecodedBytes, 128)?;
            in_deg.insert(node, 0);
        }
        for &u in affected {
            check(Kind::Work, 1)?;
            if let Some(deps) = self.deps_by_source.get(&u) {
                for &v in deps {
                    check(Kind::Work, 1)?;
                    if let Some(deg) = in_deg.get_mut(&v) {
                        *deg = deg.saturating_add(1);
                    }
                }
            }
        }
        check(Kind::DecodedBytes, 128)?;
        let mut ready = BTreeSet::new();
        for (&node, &degree) in &in_deg {
            check(Kind::Work, 1)?;
            if degree == 0 {
                check(Kind::DecodedBytes, 128)?;
                ready.insert(node);
            }
        }
        check(
            Kind::DecodedBytes,
            (affected.len() as u64).saturating_mul(4),
        )?;
        let mut ordered = Vec::with_capacity(affected.len());
        while let Some(node) = ready.pop_first() {
            check(Kind::Work, 1)?;
            in_deg.remove(&node);
            ordered.push(node);
            if let Some(deps) = self.deps_by_source.get(&node) {
                for &dep in deps {
                    check(Kind::Work, 1)?;
                    if let Some(deg) = in_deg.get_mut(&dep) {
                        *deg = deg.saturating_sub(1);
                        if *deg == 0 {
                            check(Kind::DecodedBytes, 128)?;
                            ready.insert(dep);
                        }
                    }
                }
            }
        }
        for &remaining in in_deg.keys() {
            check(Kind::Work, 1)?;
            ordered.push(remaining);
        }
        Ok(ordered)
    }

    /// 登録されている依存辺の総数
    pub fn edge_count(&self) -> usize {
        self.deps_by_source.values().map(BTreeSet::len).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};

    #[test]
    fn checked_snapshot_charges_all_retained_edge_capacity() {
        let cancel = CancelToken::new();
        for count in [0, 1, 2, 3, 4, 11, 12, 1000] {
            let mut graph = DepGraph::new();
            graph.set_sources(RowId(0), (1..=count).map(RowId));
            for row in 1..=count {
                graph.set_sources(RowId(row), [RowId(0)]);
            }
            let mut budget = Budget::default();
            let snapshot = graph
                .snapshot_row_with_cx(
                    RowId(0),
                    &mut Cx::new(&mut budget, &mut Diagnostics::new(), &cancel),
                )
                .unwrap();
            assert_eq!(
                budget.used(Kind::DecodedBytes),
                snapshot.heap_bytes() as u64
            );
            assert_eq!(snapshot, graph.snapshot_row(RowId(0)));
        }
    }

    #[test]
    fn checked_propagation_counts_allocations_and_ordering_without_changing_the_graph() {
        let mut graph = DepGraph::new();
        for dependent in 1..12 {
            graph.set_sources(RowId(dependent), (0..dependent).map(RowId));
        }
        graph.set_sources(RowId(20), [RowId(0), RowId(21)]);
        graph.set_sources(RowId(21), [RowId(20)]);
        let roots = [RowId(0), RowId(0), RowId(3)];
        let expected = graph.affected_rows(&roots);
        let before = graph.clone();
        let cancel = CancelToken::new();
        let mut measured = Budget::default();
        let actual = graph.affected_rows_with_cx(
            &roots,
            &mut Cx::new(&mut measured, &mut Diagnostics::new(), &cancel),
        );
        assert_eq!(actual, Ok(expected.clone()));
        for kind in [Kind::Work, Kind::DecodedBytes] {
            let used = measured.used(kind);
            assert!(used > 0, "{kind:?}");
            let limits: Vec<u64> = if kind == Kind::Work {
                (0..=used).collect()
            } else {
                vec![0, 1, used / 2, used - 1, used]
            };
            for limit in limits {
                let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
                let result = graph.affected_rows_with_cx(
                    &roots,
                    &mut Cx::new(&mut budget, &mut Diagnostics::new(), &cancel),
                );
                if limit == used {
                    assert_eq!(result, Ok(expected.clone()));
                } else {
                    assert!(matches!(result, Err(Stop::Exceeded(_))), "{kind:?}={limit}");
                }
                assert_eq!(graph, before);
            }
            let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(used)));
            budget.spend(kind, 1).unwrap();
            assert!(matches!(
                graph.affected_rows_with_cx(
                    &roots,
                    &mut Cx::new(&mut budget, &mut Diagnostics::new(), &cancel)
                ),
                Err(Stop::Exceeded(_))
            ));
        }
        cancel.cancel();
        let mut budget = Budget::default();
        assert_eq!(
            graph.affected_rows_with_cx(
                &roots,
                &mut Cx::new(&mut budget, &mut Diagnostics::new(), &cancel)
            ),
            Err(Stop::Cancelled)
        );
        assert_eq!(graph, before);
    }

    #[test]
    fn checked_ordering_does_not_scan_unaffected_upstream_rows() {
        let mut costs = Vec::new();
        for upstream in [2, 10_000] {
            let mut graph = DepGraph::new();
            graph.set_sources(
                RowId(1),
                [RowId(0)].into_iter().chain((10..10 + upstream).map(RowId)),
            );
            let mut budget = Budget::default();
            let cancel = CancelToken::new();
            let affected = graph
                .affected_rows_with_cx(
                    &[RowId(0)],
                    &mut Cx::new(&mut budget, &mut Diagnostics::new(), &cancel),
                )
                .unwrap();
            assert_eq!(affected, [RowId(1)]);
            costs.push(Kind::ALL.map(|kind| budget.used(kind)));
        }
        assert_eq!(costs[0], costs[1]);
    }

    #[test]
    fn propagates_topologically_handles_cycles_and_restores_snapshot() {
        let mut g = DepGraph::new();
        // 線 0 -> 寸法 2 -> 注記 4、線 0 -> ハッチ 1 -> 注記 4
        g.set_sources(RowId(2), [RowId(0)]);
        g.set_sources(RowId(1), [RowId(0)]);
        g.set_sources(RowId(4), [RowId(1), RowId(2)]);

        assert_eq!(
            g.affected_rows(&[RowId(0)]),
            vec![RowId(1), RowId(2), RowId(4)]
        );

        let snap2 = g.snapshot_row(RowId(2));
        g.remove_row(RowId(2));
        assert_eq!(g.affected_rows(&[RowId(0)]), vec![RowId(1), RowId(4)]);

        g.restore_row(&snap2);
        assert_eq!(
            g.affected_rows(&[RowId(0)]),
            vec![RowId(1), RowId(2), RowId(4)]
        );

        // 循環 10 <-> 11 があっても止まらず昇順で返す
        g.set_sources(RowId(10), [RowId(0), RowId(11)]);
        g.set_sources(RowId(11), [RowId(10)]);
        assert_eq!(
            g.affected_rows(&[RowId(0)]),
            vec![RowId(1), RowId(2), RowId(4), RowId(10), RowId(11)]
        );
    }
}
