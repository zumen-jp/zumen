//! 変更前の値を記録する原子トランザクションと、取り消し・やり直しの履歴。
//!
//! 変更は一時的な [`Transaction`] の上で組み立て、すべての検証と予算・中断（[`Cx`]）の検査に通ってから
//! [`VersionedTable`]・[`SpatialGrid`]・[`DirtyColumn`]・[`DepGraph`] へ一括で反映する。
//! 途中で断られるか予算超過・中断が起きた場合は、元の状態を 1 ビットも書き換えない。
//! 取り消し（[`History::undo`]）は逆変換の計算ではなく、記録した変更前の行・箱・ハンドル索引・依存辺・
//! 形式別の差分をそのまま戻すため、浮動小数の丸め誤差やハンドルのずれが起きない。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Cx, Kind, Stop};

use crate::dep::{DepBefore, DepGraph};
use crate::dirty::DirtyColumn;
use crate::handle::{HandleBefore, Revision, RowId};
use crate::spatial::SpatialGrid;
use crate::table::{EMPTY_BOUNDS, TableRow, VersionedTable};

/// 形式別のレコード差分（DXF のタグ差分、DWG のレコードパッチ、ヘッダー変更前後の値など）が満たす契約
pub trait PatchPayload: Clone {
    /// `size_of::<Self>()` の外に保持するヒープバイト数
    fn heap_bytes(&self) -> usize {
        0
    }
}

impl PatchPayload for () {}

impl PatchPayload for Vec<u8> {
    fn heap_bytes(&self) -> usize {
        self.capacity()
    }
}

/// トランザクションへの変更の積み上げで起きる誤り
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageError {
    /// 指定した行が存在しないか、すでに除かれている
    NotAlive(RowId),
    /// 同じトランザクションの中で新設した行を除こうとした
    BornInSameTransaction(RowId),
    /// 指定したハンドルがすでに別の生きている行で使われている
    DuplicateHandle(u64),
    /// 行数が `u32::MAX - 1` の上限に達した
    RowOverflow,
}

/// [`run_transaction`] が返す失敗理由
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TxnError<E> {
    /// 呼び出し側の検証で変更を断った（表や履歴は何も変わらない）
    Rejected(E),
    /// トランザクションへの積み上げ規則に反した（表や履歴は何も変わらない）
    Stage(StageError),
    /// 予算の上限または中断の合図で止まった（表や履歴は何も変わらない）
    Stop(Stop),
}

impl<E> From<Stop> for TxnError<E> {
    fn from(stop: Stop) -> Self {
        Self::Stop(stop)
    }
}

impl<E> From<StageError> for TxnError<E> {
    fn from(err: StageError) -> Self {
        Self::Stage(err)
    }
}

/// 既存の 1 行に対する確定済みの変更記録（変更前と変更後の両方の値を持つ）
#[derive(Clone, Debug, PartialEq)]
pub enum ExistingRowDiff<R> {
    /// 行の値または囲む箱を書き換えた
    Updated {
        /// 対象の行
        row: RowId,
        /// 変更前の行
        before_row: R,
        /// 変更前の囲む箱
        before_bounds: [f64; 4],
        /// 変更後の行
        after_row: R,
        /// 変更後の囲む箱
        after_bounds: [f64; 4],
    },
    /// 生きている行を除いた
    Removed {
        /// 対象の行
        row: RowId,
        /// 除く前の行
        before_row: R,
        /// 除く前の囲む箱
        before_bounds: [f64; 4],
    },
}

impl<R: TableRow> ExistingRowDiff<R> {
    /// 対象の行番号
    #[inline]
    pub const fn row_id(&self) -> RowId {
        match self {
            Self::Updated { row, .. } | Self::Removed { row, .. } => *row,
        }
    }

    fn heap_bytes(&self) -> usize {
        match self {
            Self::Updated {
                before_row,
                after_row,
                ..
            } => before_row.extra_heap_bytes() + after_row.extra_heap_bytes(),
            Self::Removed { before_row, .. } => before_row.extra_heap_bytes(),
        }
    }
}

/// トランザクションで末尾に追加された 1 行の記録
#[derive(Clone, Debug, PartialEq)]
pub struct BornRowDiff<R> {
    /// 割り当てられた行番号
    pub row: RowId,
    /// 追加された行の値
    pub after_row: R,
    /// 追加された行の囲む箱
    pub after_bounds: [f64; 4],
}

/// 確定した 1 トランザクションの完全な前後記録
#[derive(Clone, Debug)]
pub struct CommittedTxn<R, P = ()> {
    before_state_id: u64,
    after_state_id: u64,
    before_live_count: u32,
    after_live_count: u32,
    existing: Vec<ExistingRowDiff<R>>,
    born: Vec<BornRowDiff<R>>,
    handle_before: Vec<(u64, HandleBefore)>,
    handle_after: Vec<(u64, Option<RowId>)>,
    dep_before: Vec<DepBefore>,
    dep_after: Vec<(RowId, Vec<RowId>)>,
    patches: Vec<P>,
}

impl<R: TableRow, P: PatchPayload> CommittedTxn<R, P> {
    /// 確定前の内容状態番号
    #[inline]
    pub const fn before_state_id(&self) -> u64 {
        self.before_state_id
    }

    /// 確定後の内容状態番号
    #[inline]
    pub const fn after_state_id(&self) -> u64 {
        self.after_state_id
    }

    /// 既存行への変更一覧（行番号の昇順）
    #[inline]
    pub fn existing(&self) -> &[ExistingRowDiff<R>] {
        &self.existing
    }

    /// 末尾に追加された行の一覧（行番号の昇順）
    #[inline]
    pub fn born(&self) -> &[BornRowDiff<R>] {
        &self.born
    }

    /// 形式別の差分一覧
    #[inline]
    pub fn patches(&self) -> &[P] {
        &self.patches
    }

    /// この記録が保持する合計バイト数の見積もり
    pub fn bytes(&self) -> usize {
        core::mem::size_of::<Self>()
            + self.existing.capacity() * core::mem::size_of::<ExistingRowDiff<R>>()
            + self
                .existing
                .iter()
                .map(ExistingRowDiff::heap_bytes)
                .sum::<usize>()
            + self.born.capacity() * core::mem::size_of::<BornRowDiff<R>>()
            + self
                .born
                .iter()
                .map(|b| b.after_row.extra_heap_bytes())
                .sum::<usize>()
            + self.handle_before.capacity() * core::mem::size_of::<(u64, HandleBefore)>()
            + self.handle_after.capacity() * core::mem::size_of::<(u64, Option<RowId>)>()
            + self.dep_before.capacity() * core::mem::size_of::<DepBefore>()
            + self
                .dep_before
                .iter()
                .map(DepBefore::heap_bytes)
                .sum::<usize>()
            + self.dep_after.capacity() * core::mem::size_of::<(RowId, Vec<RowId>)>()
            + self
                .dep_after
                .iter()
                .map(|(_, v)| v.capacity() * core::mem::size_of::<RowId>())
                .sum::<usize>()
            + self.patches.capacity() * core::mem::size_of::<P>()
            + self
                .patches
                .iter()
                .map(PatchPayload::heap_bytes)
                .sum::<usize>()
    }
}

/// 表と一緒に差分で更新・巻き戻しする補助索引の組
#[derive(Default)]
pub struct SideIndexes<'a> {
    /// 範囲選択のための空間索引
    pub grid: Option<&'a mut SpatialGrid>,
    /// 4,096 行ページの列に対する 64 行塊の変更追跡
    pub dirty_rows: Option<&'a mut DirtyColumn<12>>,
    /// 要素間の依存グラフ
    pub dep: Option<&'a mut DepGraph>,
}

impl core::fmt::Debug for SideIndexes<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SideIndexes")
            .field("has_grid", &self.grid.is_some())
            .field("has_dirty_rows", &self.dirty_rows.is_some())
            .field("has_dep", &self.dep.is_some())
            .finish()
    }
}

/// 確定前の変更を保持するステージング用トランザクション。破棄すれば元の表は何も変わらない
#[derive(Debug)]
pub struct Transaction<'a, R, P = ()> {
    base: &'a VersionedTable<R>,
    existing: BTreeMap<RowId, ExistingRowDiff<R>>,
    born: Vec<BornRowDiff<R>>,
    staged_handles: BTreeMap<u64, Option<RowId>>,
    staged_deps: BTreeMap<RowId, Vec<RowId>>,
    patches: Vec<P>,
    live_delta: i64,
}

impl<'a, R: TableRow, P: PatchPayload> Transaction<'a, R, P> {
    /// `base` の上に空のトランザクションを開く
    pub fn new(base: &'a VersionedTable<R>) -> Self {
        Self {
            base,
            existing: BTreeMap::new(),
            born: Vec::new(),
            staged_handles: BTreeMap::new(),
            staged_deps: BTreeMap::new(),
            patches: Vec::new(),
            live_delta: 0,
        }
    }

    /// ステージング中の変更を含めた全行数
    #[inline]
    pub fn len(&self) -> usize {
        self.base.len() + self.born.len()
    }

    /// ステージング中の変更を含めた全行数が 0 か
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// ステージング中の変更を含めて `id` が生きているか
    pub fn is_alive(&self, id: RowId) -> bool {
        if id.index() >= self.base.len() {
            let born_idx = id.index() - self.base.len();
            return born_idx < self.born.len();
        }
        match self.existing.get(&id) {
            Some(ExistingRowDiff::Updated { .. }) => true,
            Some(ExistingRowDiff::Removed { .. }) => false,
            None => self.base.is_alive(id),
        }
    }

    /// ステージング中の変更を含めて `id` の行を借りる
    pub fn row(&self, id: RowId) -> Option<&R> {
        if id.index() >= self.base.len() {
            let born_idx = id.index() - self.base.len();
            return self.born.get(born_idx).map(|b| &b.after_row);
        }
        match self.existing.get(&id) {
            Some(ExistingRowDiff::Updated { after_row, .. }) => Some(after_row),
            Some(ExistingRowDiff::Removed { .. }) => None,
            None => self.base.row(id),
        }
    }

    /// ステージング中の変更を含めて `id` の囲む箱を返す
    pub fn bounds(&self, id: RowId) -> Option<[f64; 4]> {
        if id.index() >= self.base.len() {
            let born_idx = id.index() - self.base.len();
            return self.born.get(born_idx).map(|b| b.after_bounds);
        }
        match self.existing.get(&id) {
            Some(ExistingRowDiff::Updated { after_bounds, .. }) => Some(*after_bounds),
            Some(ExistingRowDiff::Removed { .. }) => Some(EMPTY_BOUNDS),
            None => self.base.bounds(id),
        }
    }

    /// ステージング中の変更を含めてハンドルから生きている行を引く
    pub fn find_handle(&self, handle: u64) -> Option<RowId> {
        if let Some(&ov) = self.staged_handles.get(&handle) {
            return ov;
        }
        self.base.find_handle(handle)
    }

    /// 生きている行の値と囲む箱を更新する（同じ行を複数回更新した場合は最初の変更前の値を保つ）
    pub fn update_row(
        &mut self,
        id: RowId,
        after_row: R,
        after_bounds: [f64; 4],
    ) -> Result<(), StageError> {
        if id.index() >= self.base.len() {
            let born_idx = id.index() - self.base.len();
            let Some(cur) = self.born.get(born_idx) else {
                return Err(StageError::NotAlive(id));
            };
            let old_handle = cur.after_row.row_handle();
            let new_handle = after_row.row_handle();
            if old_handle != new_handle {
                if let Some(nh) = new_handle
                    && let Some(owner) = self.find_handle(nh)
                    && owner != id
                {
                    return Err(StageError::DuplicateHandle(nh));
                }
                if let Some(oh) = old_handle
                    && self.find_handle(oh) == Some(id)
                {
                    self.staged_handles.insert(oh, None);
                }
                if let Some(nh) = new_handle {
                    self.staged_handles.insert(nh, Some(id));
                }
            }
            if let Some(slot) = self.born.get_mut(born_idx) {
                slot.after_row = after_row;
                slot.after_bounds = after_bounds;
            }
            return Ok(());
        }

        let (before_row, before_bounds, cur_handle) = match self.existing.get(&id) {
            Some(ExistingRowDiff::Removed { .. }) => return Err(StageError::NotAlive(id)),
            Some(ExistingRowDiff::Updated {
                before_row,
                before_bounds,
                after_row: cur_after,
                ..
            }) => (before_row.clone(), *before_bounds, cur_after.row_handle()),
            None => {
                let Some(orig_row) = self.base.row(id) else {
                    return Err(StageError::NotAlive(id));
                };
                let orig_bounds = self.base.bounds(id).unwrap_or(EMPTY_BOUNDS);
                (orig_row.clone(), orig_bounds, orig_row.row_handle())
            }
        };

        let new_handle = after_row.row_handle();
        if cur_handle != new_handle {
            if let Some(nh) = new_handle
                && let Some(owner) = self.find_handle(nh)
                && owner != id
            {
                return Err(StageError::DuplicateHandle(nh));
            }
            if let Some(oh) = cur_handle
                && self.find_handle(oh) == Some(id)
            {
                self.staged_handles.insert(oh, None);
            }
            if let Some(nh) = new_handle {
                self.staged_handles.insert(nh, Some(id));
            }
        }

        self.existing.insert(
            id,
            ExistingRowDiff::Updated {
                row: id,
                before_row,
                before_bounds,
                after_row,
                after_bounds,
            },
        );
        Ok(())
    }

    /// 末尾に新しい行を足し、割り当てられた [`RowId`] を返す
    pub fn insert_row(&mut self, row: R, bounds: [f64; 4]) -> Result<RowId, StageError> {
        let Some(id) = RowId::from_index(self.len()) else {
            return Err(StageError::RowOverflow);
        };
        if let Some(h) = row.row_handle() {
            if self.find_handle(h).is_some() {
                return Err(StageError::DuplicateHandle(h));
            }
            self.staged_handles.insert(h, Some(id));
        }
        self.born.push(BornRowDiff {
            row: id,
            after_row: row,
            after_bounds: bounds,
        });
        self.live_delta += 1;
        Ok(id)
    }

    /// 生きている既存の行を除き、除く前の行の値を返す
    pub fn remove_row(&mut self, id: RowId) -> Result<R, StageError> {
        if id.index() >= self.base.len() {
            let born_idx = id.index() - self.base.len();
            if born_idx < self.born.len() {
                return Err(StageError::BornInSameTransaction(id));
            }
            return Err(StageError::NotAlive(id));
        }
        let (before_row, before_bounds, current_row) = match self.existing.get(&id) {
            Some(ExistingRowDiff::Removed { .. }) => return Err(StageError::NotAlive(id)),
            Some(ExistingRowDiff::Updated {
                before_row,
                before_bounds,
                after_row,
                ..
            }) => (before_row.clone(), *before_bounds, after_row.clone()),
            None => {
                let Some(orig_row) = self.base.row(id) else {
                    return Err(StageError::NotAlive(id));
                };
                let orig_bounds = self.base.bounds(id).unwrap_or(EMPTY_BOUNDS);
                (orig_row.clone(), orig_bounds, orig_row.clone())
            }
        };
        if let Some(h) = current_row.row_handle()
            && self.find_handle(h) == Some(id)
        {
            self.staged_handles.insert(h, None);
        }
        self.staged_deps.remove(&id);
        for srcs in self.staged_deps.values_mut() {
            srcs.retain(|&s| s != id);
        }
        self.existing.insert(
            id,
            ExistingRowDiff::Removed {
                row: id,
                before_row,
                before_bounds,
            },
        );
        self.live_delta -= 1;
        Ok(current_row)
    }

    /// 生きている `dependent` が参照する生きている元の行の集合を更新するよう予約する
    pub fn set_dep_sources(
        &mut self,
        dependent: RowId,
        sources: impl IntoIterator<Item = RowId>,
    ) -> Result<(), StageError> {
        if !self.is_alive(dependent) {
            return Err(StageError::NotAlive(dependent));
        }
        let mut srcs: Vec<RowId> = Vec::new();
        for src in sources {
            if !self.is_alive(src) {
                return Err(StageError::NotAlive(src));
            }
            if src != dependent {
                srcs.push(src);
            }
        }
        srcs.sort_unstable();
        srcs.dedup();
        self.staged_deps.insert(dependent, srcs);
        Ok(())
    }

    /// 形式別の差分（タグ編集やレコードパッチ）を積む
    pub fn push_patch(&mut self, patch: P) {
        self.patches.push(patch);
    }

    /// 空のトランザクションか（変更が 1 つもないか）
    pub fn has_changes(&self) -> bool {
        !self.existing.is_empty()
            || !self.born.is_empty()
            || !self.staged_deps.is_empty()
            || !self.patches.is_empty()
    }
}

/// 取り消し・やり直しの履歴
#[derive(Clone, Debug)]
pub struct History<R, P = ()> {
    undo_stack: Vec<CommittedTxn<R, P>>,
    redo_stack: Vec<CommittedTxn<R, P>>,
}

impl<R, P> Default for History<R, P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R, P> History<R, P> {
    /// 空の履歴を作る
    pub const fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// 取り消せる段数
    #[inline]
    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    /// やり直せる段数
    #[inline]
    pub fn redo_len(&self) -> usize {
        self.redo_stack.len()
    }

    /// 履歴を空にする
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

impl<R: TableRow, P: PatchPayload> History<R, P> {
    /// 履歴全体が保持しているバイト数の見積もり
    pub fn bytes(&self) -> usize {
        self.undo_stack
            .iter()
            .map(CommittedTxn::bytes)
            .sum::<usize>()
            + self
                .redo_stack
                .iter()
                .map(CommittedTxn::bytes)
                .sum::<usize>()
    }

    /// 直前の確定済みトランザクションを取り消し、状態を変更前に戻す。
    /// 取り消す項目がなければ `Ok(None)`、予算超過・中断なら何も変えずに `Err(Stop)` を返す。
    pub fn undo(
        &mut self,
        table: &mut VersionedTable<R>,
        mut side: SideIndexes<'_>,
        cx: &mut Cx<'_>,
    ) -> Result<Option<&CommittedTxn<R, P>>, Stop> {
        let Some(top) = self.undo_stack.last() else {
            return Ok(None);
        };
        let work = (top.existing.len()
            + top.born.len()
            + top.handle_before.len()
            + top.dep_before.len()
            + top.patches.len())
        .max(1) as u64;
        // 反映を始める前に中断と予算を検査し、止まるときは何も書き換えない
        cx.check(Kind::Work, work)?;

        let Some(entry) = self.undo_stack.pop() else {
            return Ok(None);
        };

        // 1. 末尾に追加された行を逆順で取り除く
        for _ in 0..entry.born.len() {
            table.rows.pop();
            table.alive.pop();
            table.bounds.pop();
            if let Some(grid) = side.grid.as_deref_mut() {
                grid.pop();
            }
        }
        if let Some(dirty) = side.dirty_rows.as_deref_mut() {
            dirty.truncate(table.len());
        }

        // 2. 既存行への変更を前の値へ戻す
        for diff in &entry.existing {
            match diff {
                ExistingRowDiff::Updated {
                    row,
                    before_row,
                    before_bounds,
                    ..
                }
                | ExistingRowDiff::Removed {
                    row,
                    before_row,
                    before_bounds,
                } => {
                    let idx = row.index();
                    table.rows.set(idx, before_row.clone());
                    table.alive.set(idx, true);
                    table.bounds.set(idx, *before_bounds);
                    if let Some(grid) = side.grid.as_deref_mut() {
                        grid.update_row(*row, *before_bounds);
                    }
                    if let Some(dirty) = side.dirty_rows.as_deref_mut() {
                        dirty.mark_one(idx);
                    }
                }
            }
        }

        // 3. ハンドル索引の上書き表を元の状態へ正確に戻す
        for &(handle, before) in entry.handle_before.iter().rev() {
            table.handles.restore_entry(handle, before);
        }

        // 4. 依存グラフの辺を元の状態へ戻す
        if let Some(dep) = side.dep.as_deref_mut() {
            for &(row, _) in entry.dep_after.iter().rev() {
                dep.clear_dependent(row);
            }
            for before in entry.dep_before.iter().rev() {
                dep.restore_row(before);
            }
        }

        table.live_count = entry.before_live_count;
        table.state_id = entry.before_state_id;
        table.revision = table.revision.next();

        self.redo_stack.push(entry);
        Ok(self.redo_stack.last())
    }

    /// 直前に取り消したトランザクションをやり直す。
    /// やり直す項目がなければ `Ok(None)`、予算超過・中断なら何も変えずに `Err(Stop)` を返す。
    pub fn redo(
        &mut self,
        table: &mut VersionedTable<R>,
        mut side: SideIndexes<'_>,
        cx: &mut Cx<'_>,
    ) -> Result<Option<&CommittedTxn<R, P>>, Stop> {
        let Some(top) = self.redo_stack.last() else {
            return Ok(None);
        };
        let work = (top.existing.len()
            + top.born.len()
            + top.handle_after.len()
            + top.dep_after.len()
            + top.patches.len())
        .max(1) as u64;
        cx.check(Kind::Work, work)?;

        let Some(entry) = self.redo_stack.pop() else {
            return Ok(None);
        };

        // 1. 既存行への変更を再度当てる
        for diff in &entry.existing {
            match diff {
                ExistingRowDiff::Updated {
                    row,
                    after_row,
                    after_bounds,
                    ..
                } => {
                    let idx = row.index();
                    table.rows.set(idx, after_row.clone());
                    table.alive.set(idx, true);
                    table.bounds.set(idx, *after_bounds);
                    if let Some(grid) = side.grid.as_deref_mut() {
                        grid.update_row(*row, *after_bounds);
                    }
                    if let Some(dirty) = side.dirty_rows.as_deref_mut() {
                        dirty.mark_one(idx);
                    }
                }
                ExistingRowDiff::Removed { row, .. } => {
                    let idx = row.index();
                    table.alive.set(idx, false);
                    table.bounds.set(idx, EMPTY_BOUNDS);
                    if let Some(grid) = side.grid.as_deref_mut() {
                        grid.update_row(*row, EMPTY_BOUNDS);
                    }
                    if let Some(dirty) = side.dirty_rows.as_deref_mut() {
                        dirty.mark_one(idx);
                    }
                    if let Some(dep) = side.dep.as_deref_mut() {
                        dep.remove_row(*row);
                    }
                }
            }
        }

        // 2. 末尾に追加された行を再度足す
        for born in &entry.born {
            let idx = table.len();
            table.rows.push(born.after_row.clone());
            table.alive.push(true);
            table.bounds.push(born.after_bounds);
            if let Some(grid) = side.grid.as_deref_mut() {
                grid.push(born.after_bounds);
            }
            if let Some(dirty) = side.dirty_rows.as_deref_mut() {
                dirty.mark_one(idx);
            }
        }

        // 3. ハンドル索引の更新を再度当てる
        for &(handle, target) in &entry.handle_after {
            match target {
                Some(row) => {
                    table.handles.insert(handle, row);
                }
                None => {
                    table.handles.remove(handle);
                }
            }
        }

        // 4. 依存辺の更新を再度当てる
        if let Some(dep) = side.dep.as_deref_mut() {
            for (row, srcs) in &entry.dep_after {
                dep.set_sources(*row, srcs.iter().copied());
            }
        }

        table.live_count = entry.after_live_count;
        table.state_id = entry.after_state_id;
        table.revision = table.revision.next();

        self.undo_stack.push(entry);
        Ok(self.undo_stack.last())
    }
}

/// [`VersionedTable`] の上でクロージャを実行し、成功かつ予算・中断の検査に通ったときだけ原子的に反映する
pub fn run_transaction<R, P, Out, E>(
    table: &mut VersionedTable<R>,
    mut side: SideIndexes<'_>,
    history: &mut History<R, P>,
    cx: &mut Cx<'_>,
    build: impl FnOnce(&mut Transaction<'_, R, P>, &mut Cx<'_>) -> Result<Out, TxnError<E>>,
) -> Result<(Out, Revision), TxnError<E>>
where
    R: TableRow,
    P: PatchPayload,
{
    let (out, existing, born, staged_handles, staged_deps, patches, live_delta, has_changes) = {
        let mut txn = Transaction::new(table);
        let out = build(&mut txn, cx)?;
        let has_changes = txn.has_changes();
        (
            out,
            txn.existing,
            txn.born,
            txn.staged_handles,
            txn.staged_deps,
            txn.patches,
            txn.live_delta,
            has_changes,
        )
    };

    if !has_changes {
        cx.check(Kind::Work, 1)?;
        return Ok((out, table.revision));
    }

    // 反映前に必要な作業量・要素数・履歴の確保バイト数をすべて検査する（ここで止まれば表も履歴も無傷）
    let ops =
        (existing.len() + born.len() + staged_handles.len() + staged_deps.len() + patches.len())
            .max(1) as u64;
    cx.check(Kind::Work, ops)?;
    if !born.is_empty() {
        cx.check(Kind::Elements, born.len() as u64)?;
    }

    let existing_vec: Vec<ExistingRowDiff<R>> = existing.into_values().collect();
    let handle_after: Vec<(u64, Option<RowId>)> = staged_handles.into_iter().collect();
    let dep_after: Vec<(RowId, Vec<RowId>)> = staged_deps.into_iter().collect();

    let mut dep_snapshot_targets = BTreeSet::new();
    if side.dep.is_some() {
        for diff in &existing_vec {
            if let ExistingRowDiff::Removed { row, .. } = diff {
                dep_snapshot_targets.insert(*row);
            }
        }
        for &(row, _) in &dep_after {
            dep_snapshot_targets.insert(row);
        }
    }
    let dep_before: Vec<DepBefore> = if let Some(dep) = side.dep.as_deref() {
        dep_snapshot_targets
            .into_iter()
            .map(|r| dep.snapshot_row(r))
            .collect()
    } else {
        Vec::new()
    };

    let handle_before: Vec<(u64, HandleBefore)> = handle_after
        .iter()
        .map(|&(h, _)| (h, table.handles.snapshot_entry(h)))
        .collect();

    let before_state_id = table.state_id;
    let after_state_id = table.next_state_id;
    let before_live_count = table.live_count;
    let after_live_count = if live_delta >= 0 {
        before_live_count.saturating_add(u32::try_from(live_delta).unwrap_or(u32::MAX))
    } else {
        before_live_count.saturating_sub(u32::try_from(-live_delta).unwrap_or(u32::MAX))
    };

    let committed = CommittedTxn {
        before_state_id,
        after_state_id,
        before_live_count,
        after_live_count,
        existing: existing_vec,
        born,
        handle_before,
        handle_after,
        dep_before,
        dep_after,
        patches,
    };

    let born_row_bytes = committed.born.len()
        * (core::mem::size_of::<R>()
            + core::mem::size_of::<bool>()
            + core::mem::size_of::<[f64; 4]>());
    cx.check(
        Kind::DecodedBytes,
        (committed.bytes().saturating_add(born_row_bytes)) as u64,
    )?;

    // すべての検査に通ったので、ここから一括で反映する
    for diff in &committed.existing {
        match diff {
            ExistingRowDiff::Updated {
                row,
                after_row,
                after_bounds,
                ..
            } => {
                let idx = row.index();
                table.rows.set(idx, after_row.clone());
                table.alive.set(idx, true);
                table.bounds.set(idx, *after_bounds);
                if let Some(grid) = side.grid.as_deref_mut() {
                    grid.update_row(*row, *after_bounds);
                }
                if let Some(dirty) = side.dirty_rows.as_deref_mut() {
                    dirty.mark_one(idx);
                }
            }
            ExistingRowDiff::Removed { row, .. } => {
                let idx = row.index();
                table.alive.set(idx, false);
                table.bounds.set(idx, EMPTY_BOUNDS);
                if let Some(grid) = side.grid.as_deref_mut() {
                    grid.update_row(*row, EMPTY_BOUNDS);
                }
                if let Some(dirty) = side.dirty_rows.as_deref_mut() {
                    dirty.mark_one(idx);
                }
                if let Some(dep) = side.dep.as_deref_mut() {
                    dep.remove_row(*row);
                }
            }
        }
    }

    for born_row in &committed.born {
        let idx = table.len();
        table.rows.push(born_row.after_row.clone());
        table.alive.push(true);
        table.bounds.push(born_row.after_bounds);
        if let Some(grid) = side.grid.as_deref_mut() {
            grid.push(born_row.after_bounds);
        }
        if let Some(dirty) = side.dirty_rows.as_deref_mut() {
            dirty.mark_one(idx);
        }
    }

    for &(handle, target) in &committed.handle_after {
        match target {
            Some(row) => {
                table.handles.insert(handle, row);
            }
            None => {
                table.handles.remove(handle);
            }
        }
    }

    if let Some(dep) = side.dep.as_deref_mut() {
        for (row, srcs) in &committed.dep_after {
            dep.set_sources(*row, srcs.iter().copied());
        }
    }

    table.live_count = after_live_count;
    table.state_id = after_state_id;
    table.next_state_id = table.next_state_id.saturating_add(1);
    table.revision = table.revision.next();

    history.redo_stack.clear();
    history.undo_stack.push(committed);

    Ok((out, table.revision))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};

    #[derive(Clone, Debug, PartialEq)]
    struct TestRow {
        handle: Option<u64>,
        x: f64,
        y: f64,
        color: u32,
    }

    impl TableRow for TestRow {
        fn row_handle(&self) -> Option<u64> {
            self.handle
        }
    }

    fn make_cx<'a>(
        budget: &'a mut Budget,
        diag: &'a mut Diagnostics,
        cancel: &'a CancelToken,
    ) -> Cx<'a> {
        Cx::new(budget, diag, cancel)
    }

    #[test]
    fn snapshot_stays_isolated_across_concurrent_read_commit_undo_and_redo() {
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();

        // 10,000 行（4,096 行塊 × 3）を作る
        let init: Vec<(TestRow, [f64; 4], bool)> = (0..10_000u64)
            .map(|i| {
                let x = i as f64;
                (
                    TestRow {
                        handle: Some(0x1000 + i),
                        x,
                        y: 0.0,
                        color: 7,
                    },
                    [x, 0.0, x + 1.0, 1.0],
                    true,
                )
            })
            .collect();

        let mut table = {
            let mut cx = make_cx(&mut budget, &mut diag, &cancel);
            VersionedTable::from_rows_with_cx(init, &mut cx).unwrap()
        };
        let mut grid = SpatialGrid::build(table.bounds_column());
        let mut dirty: DirtyColumn<12> = DirtyColumn::new();
        dirty.clear(table.len());
        let mut dep = DepGraph::new();
        dep.set_sources(RowId(5000), [RowId(10)]);
        let mut history: History<TestRow, Vec<u8>> = History::new();

        let snap0 = table.snapshot();
        assert_eq!(snap0.revision(), Revision(0));
        assert_eq!(snap0.state_id(), 0);
        assert_eq!(snap0.shared_row_chunks_with(&table), 3);

        // 別スレッドで snap0 を読みながら、メインスレッドで変更・取り消し・やり直しを回す
        std::thread::scope(|scope| {
            let reader = scope.spawn({
                let snap = snap0.clone();
                move || {
                    assert_eq!(snap.live_count(), 10_000);
                    assert_eq!(snap.find_handle(0x1000 + 10), Some(RowId(10)));
                    assert_eq!(snap.row(RowId(10)).unwrap().x, 10.0);
                    assert_eq!(snap.find_handle(0x9999_9999), None);
                }
            });

            let mut cx = make_cx(&mut budget, &mut diag, &cancel);
            // トランザクション 1: 行 10 を動かし、行 20 を除き、新しい行を 1 つ足す
            let (born_id, rev1) = run_transaction(
                &mut table,
                SideIndexes {
                    grid: Some(&mut grid),
                    dirty_rows: Some(&mut dirty),
                    dep: Some(&mut dep),
                },
                &mut history,
                &mut cx,
                |txn, _| {
                    let mut r10 = txn.row(RowId(10)).unwrap().clone();
                    r10.x = 500.25;
                    txn.update_row(RowId(10), r10, [500.25, 0.0, 501.25, 1.0])?;
                    txn.remove_row(RowId(20))?;
                    let added = txn.insert_row(
                        TestRow {
                            handle: Some(0x9999_9999),
                            x: -50.0,
                            y: -50.0,
                            color: 1,
                        },
                        [-50.0, -50.0, -49.0, -49.0],
                    )?;
                    txn.set_dep_sources(added, [RowId(10)])?;
                    txn.push_patch(b"patch-1".to_vec());
                    Ok::<_, TxnError<()>>(added)
                },
            )
            .unwrap();

            assert_eq!(rev1, Revision(1));
            assert_eq!(table.state_id(), 1);
            assert_eq!(table.live_count(), 10_000);
            assert_eq!(born_id, RowId(10_000));
            // 塊 0（行 10・20）と塊 2（末尾の追加行 10,000）だけが写され、真ん中の塊 1（4,096..8,192）は共有のまま
            assert_eq!(snap0.shared_row_chunks_with(&table), 1);

            let mut hits = Vec::new();
            grid.query(
                table.bounds_column(),
                [-55.0, -55.0, -45.0, -45.0],
                &mut hits,
            );
            assert_eq!(hits, vec![10_000]);
            assert_eq!(
                dep.affected_rows(&[RowId(10)]),
                vec![RowId(5000), RowId(10_000)]
            );

            reader.join().unwrap();
        });

        // 取り消しで開いた直後の状態（state_id = 0、ハンドル上書き 0 件、依存辺、空間索引）に完全一致する
        {
            let mut cx = make_cx(&mut budget, &mut diag, &cancel);
            let undone = history
                .undo(
                    &mut table,
                    SideIndexes {
                        grid: Some(&mut grid),
                        dirty_rows: Some(&mut dirty),
                        dep: Some(&mut dep),
                    },
                    &mut cx,
                )
                .unwrap()
                .unwrap();
            assert_eq!(undone.patches(), &[b"patch-1".to_vec()]);
        }
        assert_eq!(table.revision(), Revision(2));
        assert_eq!(table.state_id(), 0);
        assert_eq!(table.handles().override_len(), 0);
        assert_eq!(table.snapshot(), snap0);
        assert_eq!(dep.affected_rows(&[RowId(10)]), vec![RowId(5000)]);

        let mut hits = Vec::new();
        grid.query(table.bounds_column(), [10.1, 0.1, 10.9, 0.9], &mut hits);
        assert_eq!(hits, vec![10]);

        // やり直しで変更後の状態へ戻る
        {
            let mut cx = make_cx(&mut budget, &mut diag, &cancel);
            let redone = history
                .redo(
                    &mut table,
                    SideIndexes {
                        grid: Some(&mut grid),
                        dirty_rows: Some(&mut dirty),
                        dep: Some(&mut dep),
                    },
                    &mut cx,
                )
                .unwrap()
                .unwrap();
            assert_eq!(redone.patches(), &[b"patch-1".to_vec()]);
        }
        assert_eq!(table.revision(), Revision(3));
        assert_eq!(table.state_id(), 1);
        assert_eq!(table.find_handle(0x9999_9999), Some(RowId(10_000)));
        assert_eq!(table.row(RowId(20)), None);
        assert_eq!(table.row(RowId(10)).unwrap().x, 500.25);
    }

    #[test]
    fn rejected_cancelled_or_over_budget_transactions_leave_everything_untouched() {
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();

        let init = vec![
            (
                TestRow {
                    handle: Some(1),
                    x: 1.0,
                    y: 2.0,
                    color: 7,
                },
                [1.0, 2.0, 3.0, 4.0],
                true,
            ),
            (
                TestRow {
                    handle: Some(2),
                    x: 5.0,
                    y: 6.0,
                    color: 3,
                },
                [5.0, 6.0, 7.0, 8.0],
                true,
            ),
        ];
        let mut table = {
            let mut cx = make_cx(&mut budget, &mut diag, &cancel);
            VersionedTable::from_rows_with_cx(init, &mut cx).unwrap()
        };
        let mut grid = SpatialGrid::build(table.bounds_column());
        let mut history: History<TestRow, ()> = History::new();
        let baseline = table.snapshot();

        // 1. 重複ハンドルの段階エラー: 先に行った更新も含めて一切反映されない
        {
            let mut cx = make_cx(&mut budget, &mut diag, &cancel);
            let res: Result<((), Revision), TxnError<&str>> = run_transaction(
                &mut table,
                SideIndexes {
                    grid: Some(&mut grid),
                    ..SideIndexes::default()
                },
                &mut history,
                &mut cx,
                |txn, _| {
                    txn.update_row(
                        RowId(0),
                        TestRow {
                            handle: Some(1),
                            x: 99.0,
                            y: 99.0,
                            color: 1,
                        },
                        [99.0, 99.0, 100.0, 100.0],
                    )?;
                    // 既存のハンドル 2 と衝突
                    txn.insert_row(
                        TestRow {
                            handle: Some(2),
                            x: 0.0,
                            y: 0.0,
                            color: 1,
                        },
                        [0.0, 0.0, 1.0, 1.0],
                    )?;
                    Ok(())
                },
            );
            assert_eq!(res, Err(TxnError::Stage(StageError::DuplicateHandle(2))));
        }
        assert_eq!(table.revision(), Revision(0));
        assert_eq!(table.snapshot(), baseline);
        assert_eq!(history.undo_len(), 0);

        // 2. 要素数の予算上限超過: 反映前に止まり、表も履歴も変わらない
        {
            let mut tight = Budget::new(Limits::default().with(Kind::Elements, Limit::Finite(0)));
            let mut cx = make_cx(&mut tight, &mut diag, &cancel);
            let res: Result<((), Revision), TxnError<()>> = run_transaction(
                &mut table,
                SideIndexes {
                    grid: Some(&mut grid),
                    ..SideIndexes::default()
                },
                &mut history,
                &mut cx,
                |txn, _| {
                    txn.insert_row(
                        TestRow {
                            handle: Some(99),
                            x: 0.0,
                            y: 0.0,
                            color: 1,
                        },
                        [0.0, 0.0, 1.0, 1.0],
                    )?;
                    Ok(())
                },
            );
            assert!(
                matches!(res, Err(TxnError::Stop(Stop::Exceeded(e))) if e.kind == Kind::Elements)
            );
        }
        assert_eq!(table.revision(), Revision(0));
        assert_eq!(table.snapshot(), baseline);
        assert_eq!(history.undo_len(), 0);

        // 3. 中断トークン: 反映せず止まる
        cancel.cancel();
        {
            let mut cx = make_cx(&mut budget, &mut diag, &cancel);
            let res: Result<((), Revision), TxnError<()>> = run_transaction(
                &mut table,
                SideIndexes::default(),
                &mut history,
                &mut cx,
                |txn, _| {
                    txn.remove_row(RowId(0))?;
                    Ok(())
                },
            );
            assert_eq!(res, Err(TxnError::Stop(Stop::Cancelled)));
        }
        assert_eq!(table.revision(), Revision(0));
        assert_eq!(table.snapshot(), baseline);
    }

    #[test]
    fn dep_edges_and_duplicate_handles_stay_consistent_across_removal_and_undo() {
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();

        // 行 0 と行 1 がどちらも重複ハンドル 0x50 を持つ（先勝ちで行 0 が索引に乗る）
        let init = vec![
            (
                TestRow {
                    handle: Some(0x50),
                    x: 0.0,
                    y: 0.0,
                    color: 1,
                },
                [0.0, 0.0, 1.0, 1.0],
                true,
            ),
            (
                TestRow {
                    handle: Some(0x50),
                    x: 2.0,
                    y: 0.0,
                    color: 2,
                },
                [2.0, 0.0, 3.0, 1.0],
                true,
            ),
            (
                TestRow {
                    handle: Some(0x60),
                    x: 4.0,
                    y: 0.0,
                    color: 3,
                },
                [4.0, 0.0, 5.0, 1.0],
                true,
            ),
        ];
        let mut table = {
            let mut cx = make_cx(&mut budget, &mut diag, &cancel);
            VersionedTable::from_rows_with_cx(init, &mut cx).unwrap()
        };
        assert_eq!(table.find_handle(0x50), Some(RowId(0)));

        let mut dep = DepGraph::new();
        dep.set_sources(RowId(2), [RowId(0), RowId(1)]);
        let mut history: History<TestRow, ()> = History::new();

        // 1. 除いた行を参照する set_dep_sources は NotAlive で弾かれる
        {
            let mut cx = make_cx(&mut budget, &mut diag, &cancel);
            let err: Result<((), Revision), TxnError<()>> = run_transaction(
                &mut table,
                SideIndexes {
                    dep: Some(&mut dep),
                    ..SideIndexes::default()
                },
                &mut history,
                &mut cx,
                |txn, _| {
                    txn.remove_row(RowId(1))?;
                    txn.set_dep_sources(RowId(2), [RowId(1)])?;
                    Ok(())
                },
            );
            assert_eq!(err, Err(TxnError::Stage(StageError::NotAlive(RowId(1)))));
        }

        // 2. 先に set_dep_sources(RowId(2), [RowId(0), RowId(1)]) してから remove_row(RowId(1)) した場合、
        //    確定後に死んだ行 1 の辺は残らず、先勝ちしていた行 0 のハンドル 0x50 も消えない
        {
            let mut cx = make_cx(&mut budget, &mut diag, &cancel);
            run_transaction(
                &mut table,
                SideIndexes {
                    dep: Some(&mut dep),
                    ..SideIndexes::default()
                },
                &mut history,
                &mut cx,
                |txn, _| {
                    txn.set_dep_sources(RowId(2), [RowId(0), RowId(1)])?;
                    txn.remove_row(RowId(1))?;
                    Ok::<_, TxnError<()>>(())
                },
            )
            .unwrap();
        }
        assert_eq!(table.find_handle(0x50), Some(RowId(0)));
        assert_eq!(dep.sources_of(RowId(2)), vec![RowId(0)]);
        assert!(dep.dependents_of(RowId(1)).is_empty());

        // 取り消しで元の依存辺 [RowId(0), RowId(1)] とハンドル索引が完全に戻る
        {
            let mut cx = make_cx(&mut budget, &mut diag, &cancel);
            history
                .undo(
                    &mut table,
                    SideIndexes {
                        dep: Some(&mut dep),
                        ..SideIndexes::default()
                    },
                    &mut cx,
                )
                .unwrap();
        }
        assert_eq!(table.find_handle(0x50), Some(RowId(0)));
        assert_eq!(dep.sources_of(RowId(2)), vec![RowId(0), RowId(1)]);
        assert_eq!(dep.dependents_of(RowId(1)), vec![RowId(2)]);
    }
}
