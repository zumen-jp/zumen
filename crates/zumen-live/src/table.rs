//! 塊ごとの Copy-on-Write 列で持つ文書エンジンの表と、ロックなしで読める版の写し。
//!
//! 行の本体・生死・囲む箱をそれぞれ 4,096 行単位の [`CowColumn`] に持ち、ハンドル索引 [`HandleIndex`] と
//! 単調増加の [`Revision`]・内容識別子 `state_id` を束ねる。[`VersionedTable::snapshot`] は塊のポインターだけを
//! 複製するため、後ろで動く保存・書き出し・検査のスレッドが前の版を読んでいる間も、対話の変更を止めずに進められる。

use zumen_base::{Cx, Kind, Stop};

use crate::cow::CowColumn;
use crate::handle::{HandleIndex, Revision, RowId};

/// 除かれた行や箱を持たない行に入れる空の箱
pub const EMPTY_BOUNDS: [f64; 4] = [
    f64::INFINITY,
    f64::INFINITY,
    f64::NEG_INFINITY,
    f64::NEG_INFINITY,
];

/// 表の 1 行が満たす最小限の契約
pub trait TableRow: Clone {
    /// 予算で数える行本体のバイト数。可変幅の型を持つ行は環境共通の値で上書きする。
    const INLINE_BYTES: usize = core::mem::size_of::<Self>();

    /// この行が持つハンドル（なければ `None`）
    fn row_handle(&self) -> Option<u64>;

    /// 本体の外に保持するヒープの見積もり（固定長の行なら `0`）。予算にも使うため、環境で同じ値を返す。
    fn extra_heap_bytes(&self) -> usize {
        0
    }
}

/// 特定の時点の表を不変のまま読むための版の写し（`R: Send + Sync` なら別スレッドへそのまま渡せる）
#[derive(Clone, Debug)]
pub struct TableSnapshot<R> {
    revision: Revision,
    state_id: u64,
    rows: CowColumn<R, 12>,
    alive: CowColumn<bool, 12>,
    bounds: CowColumn<[f64; 4], 12>,
    handles: HandleIndex,
    live_count: u32,
}

impl<R: PartialEq> PartialEq for TableSnapshot<R> {
    fn eq(&self, other: &Self) -> bool {
        self.state_id == other.state_id
            && self.live_count == other.live_count
            && self.rows == other.rows
            && self.alive == other.alive
            && self.bounds == other.bounds
            && self.handles == other.handles
    }
}

impl<R: Eq> Eq for TableSnapshot<R> {}

impl<R> TableSnapshot<R> {
    /// この写しが取られた時点の版の番号
    #[inline]
    pub const fn revision(&self) -> Revision {
        self.revision
    }

    /// 内容の状態番号（取り消しで前の内容に戻ると、その時点と同じ番号に戻る）
    #[inline]
    pub const fn state_id(&self) -> u64 {
        self.state_id
    }

    /// 除かれた行を含む全行数
    #[inline]
    pub const fn len(&self) -> usize {
        self.rows.len()
    }

    /// 行が 1 つもないか
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// 生きている行の数
    #[inline]
    pub const fn live_count(&self) -> u32 {
        self.live_count
    }

    /// 索引の中で生きている最大のハンドルを返す
    #[inline]
    pub fn max_handle(&self) -> Option<u64> {
        self.handles.max_handle()
    }

    /// `id` の行が生きているか
    #[inline]
    pub fn is_alive(&self, id: RowId) -> bool {
        self.alive.get(id.index()).copied().unwrap_or(false)
    }

    /// 生きている `id` の行を借りる（除かれた行なら `None`）
    #[inline]
    pub fn row(&self, id: RowId) -> Option<&R> {
        if !self.is_alive(id) {
            return None;
        }
        self.rows.get(id.index())
    }

    /// 生死を問わず `id` のスロットにある行を借りる
    #[inline]
    pub fn raw_row(&self, id: RowId) -> Option<&R> {
        self.rows.get(id.index())
    }

    /// `id` の囲む箱を返す
    #[inline]
    pub fn bounds(&self, id: RowId) -> Option<[f64; 4]> {
        self.bounds.get(id.index()).copied()
    }

    /// 囲む箱の列を借りる（[`crate::spatial::SpatialGrid::query`] にそのまま渡せる）
    #[inline]
    pub const fn bounds_column(&self) -> &CowColumn<[f64; 4], 12> {
        &self.bounds
    }

    /// ハンドルから生きている行の番号を引く
    #[inline]
    pub fn find_handle(&self, handle: u64) -> Option<RowId> {
        self.handles.get(handle)
    }

    /// 生きている行を `(RowId, &R)` の順でたどる
    pub fn iter_live(&self) -> impl Iterator<Item = (RowId, &R)> {
        self.rows
            .iter()
            .zip(self.alive.iter())
            .enumerate()
            .filter_map(|(idx, (row, &alive))| {
                if !alive {
                    return None;
                }
                RowId::from_index(idx).map(|id| (id, row))
            })
    }

    /// 現在の表とポインターを共有している行データの塊（4,096 行単位）の数
    pub fn shared_row_chunks_with(&self, table: &VersionedTable<R>) -> usize {
        self.rows.shared_chunks_with(&table.rows)
    }
}

/// 変更を受け付ける文書エンジンの表
#[derive(Clone, Debug)]
pub struct VersionedTable<R> {
    pub(crate) revision: Revision,
    pub(crate) state_id: u64,
    pub(crate) next_state_id: u64,
    pub(crate) rows: CowColumn<R, 12>,
    pub(crate) alive: CowColumn<bool, 12>,
    pub(crate) bounds: CowColumn<[f64; 4], 12>,
    pub(crate) handles: HandleIndex,
    pub(crate) live_count: u32,
}

impl<R> Default for VersionedTable<R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R> VersionedTable<R> {
    /// 空の表を作る
    pub fn new() -> Self {
        Self {
            revision: Revision::INITIAL,
            state_id: 0,
            next_state_id: 1,
            rows: CowColumn::new(),
            alive: CowColumn::new(),
            bounds: CowColumn::new(),
            handles: HandleIndex::new(),
            live_count: 0,
        }
    }

    /// 現在の版の番号
    #[inline]
    pub const fn revision(&self) -> Revision {
        self.revision
    }

    /// 現在の内容の状態番号（開いた直後は `0`）
    #[inline]
    pub const fn state_id(&self) -> u64 {
        self.state_id
    }

    /// 次に割り当てる内容の状態番号
    #[inline]
    pub const fn next_state_id(&self) -> u64 {
        self.next_state_id
    }

    /// 構造再構築の前後で版・状態番号を引き継ぐ
    #[inline]
    pub fn set_version(&mut self, revision: Revision, state_id: u64, next_state_id: u64) {
        self.revision = revision;
        self.state_id = state_id;
        self.next_state_id = next_state_id;
    }

    /// 除かれた行を含む全行数
    #[inline]
    pub const fn len(&self) -> usize {
        self.rows.len()
    }

    /// 行が 1 つもないか
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// 生きている行の数
    #[inline]
    pub const fn live_count(&self) -> u32 {
        self.live_count
    }

    /// 索引の中で生きている最大のハンドルを返す
    #[inline]
    pub fn max_handle(&self) -> Option<u64> {
        self.handles.max_handle()
    }

    /// 行データの 4,096 行塊の数
    #[inline]
    pub fn row_chunk_count(&self) -> usize {
        self.rows.chunk_count()
    }

    /// `id` の行が生きているか
    #[inline]
    pub fn is_alive(&self, id: RowId) -> bool {
        self.alive.get(id.index()).copied().unwrap_or(false)
    }

    /// 生きている `id` の行を借りる
    #[inline]
    pub fn row(&self, id: RowId) -> Option<&R> {
        if !self.is_alive(id) {
            return None;
        }
        self.rows.get(id.index())
    }

    /// 生死を問わず `id` のスロットにある行を借りる
    #[inline]
    pub fn raw_row(&self, id: RowId) -> Option<&R> {
        self.rows.get(id.index())
    }

    /// `id` の囲む箱を返す
    #[inline]
    pub fn bounds(&self, id: RowId) -> Option<[f64; 4]> {
        self.bounds.get(id.index()).copied()
    }

    /// 囲む箱の列を借りる
    #[inline]
    pub const fn bounds_column(&self) -> &CowColumn<[f64; 4], 12> {
        &self.bounds
    }

    /// ハンドル索引を借りる
    #[inline]
    pub const fn handles(&self) -> &HandleIndex {
        &self.handles
    }

    /// ハンドルから生きている行の番号を引く
    #[inline]
    pub fn find_handle(&self, handle: u64) -> Option<RowId> {
        self.handles.get(handle)
    }

    /// 生きている行を `(RowId, &R)` の順でたどる
    pub fn iter_live(&self) -> impl Iterator<Item = (RowId, &R)> {
        self.rows
            .iter()
            .zip(self.alive.iter())
            .enumerate()
            .filter_map(|(idx, (row, &alive))| {
                if !alive {
                    return None;
                }
                RowId::from_index(idx).map(|id| (id, row))
            })
    }
}

impl<R: TableRow> VersionedTable<R> {
    /// 開いた直後の行と箱の列から表を組む（予算と中断を確かめる）
    pub fn from_rows_with_cx(
        initial_rows: Vec<(R, [f64; 4], bool)>,
        cx: &mut Cx<'_>,
    ) -> Result<Self, Stop> {
        let count = initial_rows.len();
        cx.check(Kind::Elements, count as u64)?;
        if count >= u32::MAX as usize {
            // RowId::NONE と重なる行数は要素予算が Unlimited でも受け付けない
            return Err(Stop::Exceeded(zumen_base::Exceeded {
                kind: Kind::Elements,
                limit: u64::from(u32::MAX - 1),
                requested: count as u64,
            }));
        }
        let per_row =
            R::INLINE_BYTES + core::mem::size_of::<bool>() + core::mem::size_of::<[f64; 4]>();
        let extra_bytes = initial_rows.iter().fold(0u64, |acc, (r, _, _)| {
            acc.saturating_add(r.extra_heap_bytes() as u64)
        });
        cx.check(
            Kind::DecodedBytes,
            (count as u64)
                .saturating_mul(per_row as u64)
                .saturating_add(extra_bytes),
        )?;

        let mut rows = CowColumn::with_capacity(count);
        let mut alive_col = CowColumn::with_capacity(count);
        let mut bounds_col = CowColumn::with_capacity(count);
        let mut handle_pairs = Vec::with_capacity(count);
        let mut live_count = 0u32;

        for (idx, (row, bbox, is_alive)) in initial_rows.into_iter().enumerate() {
            if (idx & 4095) == 0 {
                cx.check(Kind::Work, 1)?;
            }
            if let Some(id) = RowId::from_index(idx)
                && is_alive
            {
                live_count = live_count.saturating_add(1);
                if let Some(h) = row.row_handle() {
                    handle_pairs.push((h, id));
                }
            }
            rows.push(row);
            alive_col.push(is_alive);
            bounds_col.push(if is_alive { bbox } else { EMPTY_BOUNDS });
        }
        let handles = HandleIndex::from_pairs_with_cx(handle_pairs, cx)?;
        Ok(Self {
            revision: Revision::INITIAL,
            state_id: 0,
            next_state_id: 1,
            rows,
            alive: alive_col,
            bounds: bounds_col,
            handles,
            live_count,
        })
    }

    /// 4,096 行の塊を共有したまま不変の版の写しを取る（`O(塊の数)`）
    pub fn snapshot(&self) -> TableSnapshot<R> {
        TableSnapshot {
            revision: self.revision,
            state_id: self.state_id,
            rows: self.rows.clone(),
            alive: self.alive.clone(),
            bounds: self.bounds.clone(),
            handles: self.handles.clone(),
            live_count: self.live_count,
        }
    }
}
