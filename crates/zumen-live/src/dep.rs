//! 要素どうしの依存（寸法と測る相手、ハッチングと境界、挿入と属性、壁と開口など）を持つ有向グラフ。
//!
//! 変更があった行から依存する側（再計算・再描画が要る行）を、環境によらない決まった順で取り出す。
//! 循環があっても無限ループせず、取り消し用に前の辺の状態をそのまま戻せる。

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use zumen_base::{Cx, Kind, Stop};

use crate::handle::RowId;

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
}

/// 行どうしの依存グラフ
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DepGraph {
    /// `dependent -> その行が参照する source 行の集合`
    sources_by_dep: BTreeMap<RowId, BTreeSet<RowId>>,
    /// `source -> その行に依存する dependent 行の集合`
    deps_by_source: BTreeMap<RowId, BTreeSet<RowId>>,
}

impl DepGraph {
    /// 空の依存グラフを作る
    pub fn new() -> Self {
        Self::default()
    }

    /// `row` に関わる現在の辺を、取り消し用に記録する
    pub fn snapshot_row(&self, row: RowId) -> DepBefore {
        let sources = self
            .sources_by_dep
            .get(&row)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default();
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

    /// `dependent` が参照する元の行の集合を `sources` に置き換える（自分自身への辺は除く）
    pub fn set_sources(&mut self, dependent: RowId, sources: impl IntoIterator<Item = RowId>) {
        self.clear_dependent(dependent);
        let mut set = BTreeSet::new();
        for src in sources {
            if src != dependent && !src.is_none() {
                set.insert(src);
            }
        }
        if set.is_empty() {
            return;
        }
        for &src in &set {
            self.deps_by_source
                .entry(src)
                .or_default()
                .insert(dependent);
        }
        self.sources_by_dep.insert(dependent, set);
    }

    /// `dependent` が持つ参照元の辺をすべて外す
    pub fn clear_dependent(&mut self, dependent: RowId) {
        let Some(old_sources) = self.sources_by_dep.remove(&dependent) else {
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
                let empty = if let Some(srcs) = self.sources_by_dep.get_mut(&dep) {
                    srcs.remove(&row);
                    srcs.is_empty()
                } else {
                    false
                };
                if empty {
                    self.sources_by_dep.remove(&dep);
                }
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
                self.sources_by_dep
                    .entry(dep)
                    .or_default()
                    .insert(before.row);
                self.deps_by_source
                    .entry(before.row)
                    .or_default()
                    .insert(dep);
            }
        }
    }

    /// `dependent` が直接参照している行を昇順で返す
    pub fn sources_of(&self, dependent: RowId) -> Vec<RowId> {
        self.sources_by_dep
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
        cx.check(Kind::Work, (changed_roots.len() as u64).max(1))?;
        let mut visited = BTreeSet::new();
        let mut queue: VecDeque<RowId> = VecDeque::new();
        let mut roots_sorted: Vec<RowId> = changed_roots.to_vec();
        roots_sorted.sort_unstable();
        roots_sorted.dedup();
        for root in roots_sorted {
            cx.check(Kind::Work, 1)?;
            if let Some(deps) = self.deps_by_source.get(&root) {
                for &dep in deps {
                    cx.check(Kind::Work, 1)?;
                    if visited.insert(dep) {
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
                    if visited.insert(dep) {
                        queue.push_back(dep);
                    }
                }
            }
        }
        cx.check(Kind::Work, visited.len() as u64)?;
        Ok(self.order_affected(&visited))
    }

    /// 指定したノード集合 `nodes` の誘導部分グラフを、上流から順（同順位・循環部は `RowId` 昇順）に並べて返す
    pub fn topological_order(&self, nodes: &BTreeSet<RowId>) -> Vec<RowId> {
        self.order_affected(nodes)
    }

    fn order_affected(&self, affected: &BTreeSet<RowId>) -> Vec<RowId> {
        if affected.is_empty() {
            return Vec::new();
        }
        // 部分グラフ内の入次数を、`affected` 内の頂点からの出力辺だけで数える（未変更の上流参照元が大量にあっても走査しない）。
        // Kahn 法（BTreeSet で最小の RowId から取り出す）でトポロジカル順に並べ、循環があれば残った頂点を行番号の昇順で後ろに足す。
        let mut in_deg: BTreeMap<RowId, usize> =
            affected.iter().map(|&node| (node, 0usize)).collect();
        for &u in affected {
            if let Some(deps) = self.deps_by_source.get(&u) {
                for &v in deps {
                    if let Some(deg) = in_deg.get_mut(&v) {
                        *deg = deg.saturating_add(1);
                    }
                }
            }
        }
        let mut ready: BTreeSet<RowId> = in_deg
            .iter()
            .filter_map(|(&node, &deg)| (deg == 0).then_some(node))
            .collect();
        let mut ordered = Vec::with_capacity(affected.len());
        while let Some(node) = ready.pop_first() {
            in_deg.remove(&node);
            ordered.push(node);
            if let Some(deps) = self.deps_by_source.get(&node) {
                for &dep in deps {
                    if let Some(deg) = in_deg.get_mut(&dep) {
                        *deg = deg.saturating_sub(1);
                        if *deg == 0 {
                            ready.insert(dep);
                        }
                    }
                }
            }
        }
        for &remaining in in_deg.keys() {
            ordered.push(remaining);
        }
        ordered
    }

    /// 登録されている依存辺の総数
    pub fn edge_count(&self) -> usize {
        self.sources_by_dep.values().map(BTreeSet::len).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
