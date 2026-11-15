//! 行の番号、版の番号、ハンドルから行を引く索引。
//!
//! 開いたときに整列済みの配列を `Arc` で 1 つだけ作り、編集で変わった・増えた・消えたハンドルだけを
//! 小さい `BTreeMap` の上書きとして持つ。版の写しを取るときも元の配列と上書き表を `Arc` で共有する。

use std::collections::BTreeMap;
use std::sync::Arc;

use zumen_base::{Cx, Kind, Stop};

/// 表の中の詰めた 32 bit の行番号
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RowId(pub u32);

impl RowId {
    /// どの行も指さない番兵
    pub const NONE: Self = Self(u32::MAX);

    /// `usize` の添字から作る（`u32::MAX` 以上は `None`）
    #[inline]
    pub fn from_index(index: usize) -> Option<Self> {
        let raw = u32::try_from(index).ok()?;
        (raw != u32::MAX).then_some(Self(raw))
    }

    /// `usize` の添字に変換する
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// [`Self::NONE`] か
    #[inline]
    pub const fn is_none(self) -> bool {
        self.0 == u32::MAX
    }
}

/// 単調に増える版の番号。変更の確定・取り消し・やり直しのたびに 1 つ進めるため、ABA の取り違えが起きない
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(pub u64);

impl Revision {
    /// 開いた直後の最初の版
    pub const INITIAL: Self = Self(0);

    /// 番号を取り出す
    #[inline]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// 次の版の番号を返す
    #[inline]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// 取り消し用に記録する、1 つのハンドルの上書き表における変更前の状態
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandleBefore {
    /// 上書き表にまだ項目がなかった（開いたときの配列のまま）
    Base,
    /// 上書き表に `Some(row)` または `None`（削除済み）が入っていた
    Overridden(Option<RowId>),
}

/// ハンドル（`u64`）から [`RowId`] を引く索引
#[derive(Clone, Debug, Default)]
pub struct HandleIndex {
    base: Arc<Vec<(u64, RowId)>>,
    overrides: Arc<BTreeMap<u64, Option<RowId>>>,
}

impl PartialEq for HandleIndex {
    fn eq(&self, other: &Self) -> bool {
        // 意味として同じ対応かではなく、保持する状態の一致を見る
        (Arc::ptr_eq(&self.base, &other.base) || self.base == other.base)
            && (Arc::ptr_eq(&self.overrides, &other.overrides) || self.overrides == other.overrides)
    }
}

impl Eq for HandleIndex {}

impl HandleIndex {
    /// 空の索引を作る
    pub fn new() -> Self {
        Self::default()
    }

    /// `(ハンドル, 行)` の列から整列済みの索引を作る（同じハンドルがあれば先の行を残す）
    pub fn from_pairs(mut pairs: Vec<(u64, RowId)>) -> Self {
        pairs.sort_by_key(|&(h, row)| (h, row));
        pairs.dedup_by_key(|pair| pair.0);
        Self {
            base: Arc::new(pairs),
            overrides: Arc::new(BTreeMap::new()),
        }
    }

    /// 予算と中断を確かめてから、`(ハンドル, 行)` の列から索引を作る
    pub fn from_pairs_with_cx(pairs: Vec<(u64, RowId)>, cx: &mut Cx<'_>) -> Result<Self, Stop> {
        let count = pairs.len() as u64;
        cx.check(Kind::Work, count)?;
        let bytes = count.saturating_mul(core::mem::size_of::<(u64, RowId)>() as u64);
        cx.check(Kind::DecodedBytes, bytes)?;
        Ok(Self::from_pairs(pairs))
    }

    #[inline]
    fn base_get(&self, handle: u64) -> Option<RowId> {
        let idx = self.base.binary_search_by_key(&handle, |&(h, _)| h).ok()?;
        self.base.get(idx).map(|&(_, row)| row)
    }

    /// ハンドルが指す生きている行を返す
    #[inline]
    pub fn get(&self, handle: u64) -> Option<RowId> {
        if let Some(&ov) = self.overrides.get(&handle) {
            return ov;
        }
        self.base_get(handle)
    }

    /// ハンドルが生きている行を指しているか
    #[inline]
    pub fn contains(&self, handle: u64) -> bool {
        self.get(handle).is_some()
    }

    /// 生きている最大のハンドルを返す（なければ `None`）
    pub fn max_handle(&self) -> Option<u64> {
        let max_base = self
            .base
            .iter()
            .rev()
            .find_map(|&(h, _)| match self.overrides.get(&h) {
                Some(None) => None,
                Some(Some(_)) | None => Some(h),
            });
        let max_ov = self
            .overrides
            .iter()
            .rev()
            .find_map(|(&h, &slot)| slot.map(|_| h));
        match (max_base, max_ov) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (Some(a), None) => Some(a),
            (None, b) => b,
        }
    }

    /// 取り消しのために、指定ハンドルの現在の上書き状態を記録する
    #[inline]
    pub fn snapshot_entry(&self, handle: u64) -> HandleBefore {
        match self.overrides.get(&handle) {
            Some(&ov) => HandleBefore::Overridden(ov),
            None => HandleBefore::Base,
        }
    }

    /// ハンドルを `row` に結び付け、変更前の状態を返す
    pub fn insert(&mut self, handle: u64, row: RowId) -> HandleBefore {
        let before = self.snapshot_entry(handle);
        if self.base_get(handle) == Some(row) {
            if self.overrides.contains_key(&handle) {
                Arc::make_mut(&mut self.overrides).remove(&handle);
            }
        } else {
            Arc::make_mut(&mut self.overrides).insert(handle, Some(row));
        }
        before
    }

    /// ハンドルを索引から外し、変更前の状態を返す
    pub fn remove(&mut self, handle: u64) -> HandleBefore {
        let before = self.snapshot_entry(handle);
        if self.base_get(handle).is_some() {
            Arc::make_mut(&mut self.overrides).insert(handle, None);
        } else if self.overrides.contains_key(&handle) {
            Arc::make_mut(&mut self.overrides).remove(&handle);
        }
        before
    }

    /// [`Self::snapshot_entry`] で記録した状態に戻す
    pub fn restore_entry(&mut self, handle: u64, before: HandleBefore) {
        match before {
            HandleBefore::Base => {
                if self.overrides.contains_key(&handle) {
                    Arc::make_mut(&mut self.overrides).remove(&handle);
                }
            }
            HandleBefore::Overridden(ov) => {
                Arc::make_mut(&mut self.overrides).insert(handle, ov);
            }
        }
    }

    /// 上書き表に入っている件数
    #[inline]
    pub fn override_len(&self) -> usize {
        self.overrides.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_index_supports_base_lookup_override_and_exact_restore() {
        let mut idx = HandleIndex::from_pairs(vec![
            (0x30, RowId(2)),
            (0x10, RowId(0)),
            (0x20, RowId(1)),
            (0x10, RowId(99)), // 重複は先の RowId(0) を保つ
        ]);
        assert_eq!(idx.get(0x10), Some(RowId(0)));
        assert_eq!(idx.get(0x20), Some(RowId(1)));
        assert_eq!(idx.get(0x30), Some(RowId(2)));
        assert_eq!(idx.get(0x40), None);

        let snap = idx.clone();
        let b_rem = idx.remove(0x20);
        let b_add = idx.insert(0x40, RowId(3));
        assert_eq!(idx.get(0x20), None);
        assert_eq!(idx.get(0x40), Some(RowId(3)));
        // 写しを取った側は変わらない
        assert_eq!(snap.get(0x20), Some(RowId(1)));
        assert_eq!(snap.get(0x40), None);

        idx.restore_entry(0x40, b_add);
        idx.restore_entry(0x20, b_rem);
        assert_eq!(idx, snap);
        assert_eq!(idx.override_len(), 0);
    }
}
