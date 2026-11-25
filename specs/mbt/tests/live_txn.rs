//! `specs/live_txn.qnt` のトレースを `zumen-live` の実データ構造群へリプレイする MBT。

use std::collections::{BTreeMap, BTreeSet};

use quint_connect::*;
use serde::Deserialize;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_live::{
    BoundsSource, DepGraph, DirtyColumn, EMPTY_BOUNDS, History, Revision, RowId, SideIndexes,
    SpatialGrid, TableRow, TxnError, VersionedTable, run_transaction,
};

struct TableBounds<'a>(&'a VersionedTable<TestRow>);

impl BoundsSource for TableBounds<'_> {
    fn bounds_len(&self) -> usize {
        self.0.len()
    }

    fn bounds_at(&self, index: usize) -> Option<[f64; 4]> {
        self.0.bounds(RowId(index as u32))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TestRow {
    val_num: i64,
    handle: u64,
}

impl TableRow for TestRow {
    const INLINE_BYTES: usize = 32;

    fn row_handle(&self) -> Option<u64> {
        (self.handle != 0).then_some(self.handle)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct RowSnap {
    alive: bool,
    val_num: i64,
    handle: i64,
    bx: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct DocSnap {
    state_id: i64,
    rows: Vec<RowSnap>,
    deps: BTreeMap<i64, BTreeSet<i64>>,
}

fn bounds_of(bx: i64) -> [f64; 4] {
    let x = bx as f64;
    [x, 0.0, x + 0.5, 0.5]
}

struct LiveTxnDriver {
    table: VersionedTable<TestRow>,
    grid: SpatialGrid,
    dirty_rows: DirtyColumn<12>,
    dep: DepGraph,
    history: History<TestRow>,
    last_ok: bool,
    /// 各行の削除前のスナップショット値を保持（削除行でも `RowSnap` 比較を厳密に行うため）
    dead_records: BTreeMap<usize, (i64, i64, i64)>,
    /// 仕様と同じく `undo_snaps` / `redo_snaps` の長さと内容を追跡するための補助鏡像
    undo_mirror: Vec<DocSnap>,
    redo_mirror: Vec<DocSnap>,
}

impl Default for LiveTxnDriver {
    fn default() -> Self {
        Self {
            table: VersionedTable::new(),
            grid: SpatialGrid::build(&[][..]),
            dirty_rows: DirtyColumn::new(),
            dep: DepGraph::new(),
            history: History::new(),
            last_ok: true,
            dead_records: BTreeMap::new(),
            undo_mirror: Vec::new(),
            redo_mirror: Vec::new(),
        }
    }
}

impl LiveTxnDriver {
    fn current_rows(&self) -> Vec<RowSnap> {
        let mut out = Vec::with_capacity(self.table.len());
        for i in 0..self.table.len() {
            let id = RowId(i as u32);
            if self.table.is_alive(id) {
                let row = self.table.row(id).expect("生きた行は存在する");
                let b = self.table.bounds(id).expect("生きた行の箱は存在する");
                out.push(RowSnap {
                    alive: true,
                    val_num: row.val_num,
                    handle: row.handle as i64,
                    bx: b[0].round() as i64,
                });
            } else {
                let &(val_num, handle, bx) =
                    self.dead_records.get(&i).expect("削除行の記録が存在する");
                out.push(RowSnap {
                    alive: false,
                    val_num,
                    handle,
                    bx,
                });
            }
        }
        out
    }

    fn current_deps(&self) -> BTreeMap<i64, BTreeSet<i64>> {
        let mut map = BTreeMap::new();
        for i in 0..self.table.len() {
            let id = RowId(i as u32);
            let set: BTreeSet<i64> = self
                .dep
                .sources_of(id)
                .iter()
                .map(|r| i64::from(r.0))
                .collect();
            map.insert(i as i64, set);
        }
        map
    }

    fn current_doc_snap(&self) -> DocSnap {
        DocSnap {
            state_id: self.table.state_id() as i64,
            rows: self.current_rows(),
            deps: self.current_deps(),
        }
    }

    /// 6 構造体間の内部不変条件（HandleIndex・SpatialGrid・DepGraph 双対性・History 長さ・live_count）を検証する。
    fn assert_internal_invariants(&self) {
        let rows = self.current_rows();

        // 1. live_count の一致
        let expected_live = rows.iter().filter(|r| r.alive).count() as u32;
        assert_eq!(self.table.live_count(), expected_live);

        // 2. HandleIndex が全ハンドル 1..=4 について生きた行と完全一致
        for h in 1u64..=4 {
            let expected = rows
                .iter()
                .enumerate()
                .find(|(_, r)| r.alive && r.handle as u64 == h)
                .map(|(i, _)| RowId(i as u32));
            assert_eq!(
                self.table.find_handle(h),
                expected,
                "HandleIndex がハンドル {h} で不一致"
            );
        }

        // 3. SpatialGrid の範囲検索が全座標 0..=3 で生きた行と完全一致
        let bounds_src = TableBounds(&self.table);
        for bx in 0i64..=3 {
            let mut raw_hits = Vec::new();
            let rect = [bx as f64 + 0.1, 0.1, bx as f64 + 0.4, 0.4];
            self.grid.query(&bounds_src, rect, &mut raw_hits);
            let mut found: Vec<RowId> = raw_hits
                .into_iter()
                .map(RowId)
                .filter(|&id| self.table.is_alive(id))
                .collect();
            found.sort_unstable();
            found.dedup();
            let expected: Vec<RowId> = rows
                .iter()
                .enumerate()
                .filter(|(_, r)| r.alive && r.bx == bx)
                .map(|(i, _)| RowId(i as u32))
                .collect();
            assert_eq!(found, expected, "SpatialGrid が座標 {bx} で不一致");
        }

        // 4. DepGraph の順方向（sources_of）と逆方向（dependents_of）の双対一致
        for src_i in 0..rows.len() {
            let src = RowId(src_i as u32);
            let actual_deps: BTreeSet<RowId> =
                self.dep.dependents_of(src).iter().copied().collect();
            let expected_deps: BTreeSet<RowId> = (0..rows.len())
                .map(|i| RowId(i as u32))
                .filter(|&d| self.dep.sources_of(d).contains(&src))
                .collect();
            assert_eq!(
                actual_deps, expected_deps,
                "DepGraph の逆引きが {src:?} で不一致"
            );
        }

        // 5. History スタック長の一致
        assert_eq!(self.history.undo_len(), self.undo_mirror.len());
        assert_eq!(self.history.redo_len(), self.redo_mirror.len());
    }

    fn init(&mut self) {
        *self = Self::default();
    }

    fn txn_insert(&mut self, val_num: i64, handle: i64, bx: i64) {
        let before_snap = self.current_doc_snap();
        let before_rev = self.table.revision();
        let before_dirty = self.dirty_rows.clone();
        let before_dep = self.dep.clone();

        let mut budget = Budget::new(Limits::default());
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let side = SideIndexes {
            grid: Some(&mut self.grid),
            dirty_rows: Some(&mut self.dirty_rows),
            dep: Some(&mut self.dep),
        };
        let res: std::result::Result<((), Revision), TxnError<()>> = run_transaction(
            &mut self.table,
            side,
            &mut self.history,
            &mut cx,
            |txn, _cx| {
                let row = TestRow {
                    val_num,
                    handle: handle as u64,
                };
                txn.insert_row(row, bounds_of(bx))?;
                Ok(())
            },
        );
        match res {
            Ok(_) => {
                self.undo_mirror.push(before_snap);
                self.redo_mirror.clear();
                self.last_ok = true;
            }
            Err(_) => {
                assert_eq!(self.table.revision(), before_rev);
                assert_eq!(self.current_doc_snap(), before_snap);
                assert_eq!(self.dirty_rows, before_dirty);
                assert_eq!(self.dep, before_dep);
                self.last_ok = false;
            }
        }
    }

    fn txn_update(&mut self, row: i64, val_num: i64, handle: i64, bx: i64) {
        let before_snap = self.current_doc_snap();
        let before_rev = self.table.revision();
        let before_dirty = self.dirty_rows.clone();
        let before_dep = self.dep.clone();

        let mut budget = Budget::new(Limits::default());
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let side = SideIndexes {
            grid: Some(&mut self.grid),
            dirty_rows: Some(&mut self.dirty_rows),
            dep: Some(&mut self.dep),
        };
        let res: std::result::Result<((), Revision), TxnError<()>> = run_transaction(
            &mut self.table,
            side,
            &mut self.history,
            &mut cx,
            |txn, _cx| {
                let new_row = TestRow {
                    val_num,
                    handle: handle as u64,
                };
                txn.update_row(RowId(row as u32), new_row, bounds_of(bx))?;
                Ok(())
            },
        );
        match res {
            Ok(_) => {
                self.undo_mirror.push(before_snap);
                self.redo_mirror.clear();
                self.last_ok = true;
            }
            Err(_) => {
                assert_eq!(self.table.revision(), before_rev);
                assert_eq!(self.current_doc_snap(), before_snap);
                assert_eq!(self.dirty_rows, before_dirty);
                assert_eq!(self.dep, before_dep);
                self.last_ok = false;
            }
        }
    }

    fn txn_remove(&mut self, row: i64) {
        let before_snap = self.current_doc_snap();
        let before_rev = self.table.revision();
        let before_dirty = self.dirty_rows.clone();
        let before_dep = self.dep.clone();
        let row_idx = row as usize;
        let pre_info = self.table.row(RowId(row as u32)).map(|r| {
            let bx = self.table.bounds(RowId(row as u32)).unwrap_or(EMPTY_BOUNDS)[0].round() as i64;
            (r.val_num, r.handle as i64, bx)
        });

        let mut budget = Budget::new(Limits::default());
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let side = SideIndexes {
            grid: Some(&mut self.grid),
            dirty_rows: Some(&mut self.dirty_rows),
            dep: Some(&mut self.dep),
        };
        let res: std::result::Result<((), Revision), TxnError<()>> = run_transaction(
            &mut self.table,
            side,
            &mut self.history,
            &mut cx,
            |txn, _cx| {
                txn.remove_row(RowId(row as u32))?;
                Ok(())
            },
        );
        match res {
            Ok(_) => {
                if let Some(info) = pre_info {
                    self.dead_records.insert(row_idx, info);
                }
                self.undo_mirror.push(before_snap);
                self.redo_mirror.clear();
                self.last_ok = true;
            }
            Err(_) => {
                assert_eq!(self.table.revision(), before_rev);
                assert_eq!(self.current_doc_snap(), before_snap);
                assert_eq!(self.dirty_rows, before_dirty);
                assert_eq!(self.dep, before_dep);
                self.last_ok = false;
            }
        }
    }

    fn txn_set_dep(&mut self, dep_row: i64, src_row: i64) {
        let before_snap = self.current_doc_snap();
        let before_rev = self.table.revision();
        let before_dirty = self.dirty_rows.clone();
        let before_dep = self.dep.clone();

        let mut budget = Budget::new(Limits::default());
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let side = SideIndexes {
            grid: Some(&mut self.grid),
            dirty_rows: Some(&mut self.dirty_rows),
            dep: Some(&mut self.dep),
        };
        let res: std::result::Result<((), Revision), TxnError<()>> = run_transaction(
            &mut self.table,
            side,
            &mut self.history,
            &mut cx,
            |txn, _cx| {
                txn.set_dep_sources(RowId(dep_row as u32), [RowId(src_row as u32)])?;
                Ok(())
            },
        );
        match res {
            Ok(_) => {
                self.undo_mirror.push(before_snap);
                self.redo_mirror.clear();
                self.last_ok = true;
            }
            Err(_) => {
                assert_eq!(self.table.revision(), before_rev);
                assert_eq!(self.current_doc_snap(), before_snap);
                assert_eq!(self.dirty_rows, before_dirty);
                assert_eq!(self.dep, before_dep);
                self.last_ok = false;
            }
        }
    }

    fn txn_aborted(&mut self, row: i64, fail_budget: bool) {
        let before_snap = self.current_doc_snap();
        let before_rev = self.table.revision();
        let before_dirty = self.dirty_rows.clone();
        let before_dep = self.dep.clone();
        let before_undo = self.history.undo_len();
        let before_redo = self.history.redo_len();

        let limits = if fail_budget {
            Limits::default().with(Kind::Work, Limit::Finite(0))
        } else {
            Limits::default()
        };
        let mut budget = Budget::new(limits);
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let side = SideIndexes {
            grid: Some(&mut self.grid),
            dirty_rows: Some(&mut self.dirty_rows),
            dep: Some(&mut self.dep),
        };
        let res: std::result::Result<((), Revision), TxnError<()>> = run_transaction(
            &mut self.table,
            side,
            &mut self.history,
            &mut cx,
            |txn, _cx| {
                let id = RowId(row as u32);
                if txn.is_alive(id) {
                    let _ = txn.update_row(
                        id,
                        TestRow {
                            val_num: 99,
                            handle: 0,
                        },
                        bounds_of(0),
                    );
                } else {
                    let _ = txn.insert_row(
                        TestRow {
                            val_num: 99,
                            handle: 0,
                        },
                        bounds_of(0),
                    );
                }
                if fail_budget {
                    Ok(())
                } else {
                    Err(TxnError::Rejected(()))
                }
            },
        );
        assert!(res.is_err());
        assert_eq!(self.table.revision(), before_rev);
        assert_eq!(self.current_doc_snap(), before_snap);
        assert_eq!(self.dirty_rows, before_dirty);
        assert_eq!(self.dep, before_dep);
        assert_eq!(self.history.undo_len(), before_undo);
        assert_eq!(self.history.redo_len(), before_redo);
        self.last_ok = false;
    }

    fn do_undo(&mut self) {
        let before_snap = self.current_doc_snap();
        let mut budget = Budget::new(Limits::default());
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let side = SideIndexes {
            grid: Some(&mut self.grid),
            dirty_rows: Some(&mut self.dirty_rows),
            dep: Some(&mut self.dep),
        };
        let undone = self
            .history
            .undo(&mut self.table, side, &mut cx)
            .expect("無制限予算での undo は成功する");
        let ok = undone.is_some();
        if ok {
            self.undo_mirror.pop();
            self.redo_mirror.push(before_snap);
        }
        self.last_ok = ok;
    }

    fn do_redo(&mut self) {
        let before_snap = self.current_doc_snap();
        let mut budget = Budget::new(Limits::default());
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let side = SideIndexes {
            grid: Some(&mut self.grid),
            dirty_rows: Some(&mut self.dirty_rows),
            dep: Some(&mut self.dep),
        };
        let redone = self
            .history
            .redo(&mut self.table, side, &mut cx)
            .expect("無制限予算での redo は成功する");
        let ok = redone.is_some();
        if ok {
            self.redo_mirror.pop();
            self.undo_mirror.push(before_snap);
        }
        self.last_ok = ok;
    }

    fn do_trim_undo(&mut self) {
        if self.history.undo_len() > 1 {
            // 最古の 1 段だけを落とすように残り合計バイト数を狙う
            let total = self.history.bytes();
            let target = total.saturating_sub(1);
            // 1 段だけ削る仕様に合わせるため、現在の全段数からちょうど 1 段分だけ減る上限を確認
            let before = self.history.undo_len();
            self.history.trim_to_bytes(target);
            let dropped = before - self.history.undo_len();
            assert!(dropped >= 1);
            for _ in 0..dropped {
                self.undo_mirror.remove(0);
            }
        }
        self.last_ok = true;
    }

    fn do_clear_dirty(&mut self) {
        self.dirty_rows.clear(self.table.len());
        self.last_ok = true;
    }
}

/// ブロック粒度（64 行単位）の `DirtyColumn` と整合する射影状態。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct LiveTxnProjectedState {
    rev: i64,
    state_id: i64,
    rows: Vec<RowSnap>,
    deps: BTreeMap<i64, BTreeSet<i64>>,
    undo_snaps: Vec<DocSnap>,
    redo_snaps: Vec<DocSnap>,
    last_ok: bool,
}

impl State<LiveTxnDriver> for LiveTxnProjectedState {
    fn from_driver(driver: &LiveTxnDriver) -> Result<Self> {
        driver.assert_internal_invariants();
        Ok(Self {
            rev: driver.table.revision().get() as i64,
            state_id: driver.table.state_id() as i64,
            rows: driver.current_rows(),
            deps: driver.current_deps(),
            undo_snaps: driver.undo_mirror.clone(),
            redo_snaps: driver.redo_mirror.clone(),
            last_ok: driver.last_ok,
        })
    }
}

impl Driver for LiveTxnDriver {
    type State = LiveTxnProjectedState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => self.init(),
            txn_insert(val_num, handle, bx) => self.txn_insert(val_num, handle, bx),
            txn_update(row, val_num, handle, bx) => self.txn_update(row, val_num, handle, bx),
            txn_remove(row) => self.txn_remove(row),
            txn_set_dep(dep_row, src_row) => self.txn_set_dep(dep_row, src_row),
            txn_aborted(row, fail_budget) => self.txn_aborted(row, fail_budget),
            do_undo => self.do_undo(),
            do_redo => self.do_redo(),
            do_trim_undo => self.do_trim_undo(),
            do_clear_dirty => self.do_clear_dirty(),
        })
    }
}

#[quint_run(
    spec = "../live_txn.qnt",
    max_samples = 40,
    max_steps = 30,
    seed = "20261124"
)]
fn mbt_live_txn_random_traces() -> impl Driver {
    LiveTxnDriver::default()
}

#[quint_run(
    spec = "../live_txn.qnt",
    max_samples = 25,
    max_steps = 45,
    seed = "20261125"
)]
fn mbt_live_txn_deep_traces() -> impl Driver {
    LiveTxnDriver::default()
}
