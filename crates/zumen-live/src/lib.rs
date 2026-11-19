//! zumen の文書エンジンの形式非依存の基盤（`docs/live.md`、#878）。
//!
//! 開いた図面をメモリに持ち続け、変更を差分で当てるための共通部品を置く。形式別の 1 レコード読み書きは
//! 各 L1 crate 側に置き、この crate は特定の形式や領域モデルに依存しない。
//!
//! - [`paged`]: 固定長ページの伸びる配列 [`Paged`] と、64 KiB 塊を共有する文字列置き場 [`BytePool`]
//! - [`source`]: 16 KiB のページごとに LZ4 圧縮して元のファイルバイト列を持つ [`SourceBytes`] と [`PageReader`]
//! - [`cow`]: 4,096 行などの塊ごとに `Arc` で共有し、触れた塊だけを写す列 [`CowColumn`]
//! - [`handle`]: 行番号 [`RowId`]、単調増加の版番号 [`Revision`]、ハンドル索引 [`HandleIndex`]
//! - [`table`]: 塊ごとの版を持つ表 [`VersionedTable`] と、別スレッドからロックなしで読める写し [`TableSnapshot`]
//! - [`txn`]: 変更前の値を残す原子トランザクション [`Transaction`] と、取り消し・やり直し履歴 [`History`]
//! - [`spatial`]: 要素数によらない時間で更新・範囲検索する階層格子 [`SpatialGrid`]
//! - [`dirty`]: 64 要素の塊ごとに変更範囲を追跡する [`DirtyColumn`]
//! - [`display`]: 差分で更新する 2D の表示のバッファー [`Display`] と 13 列の変更追跡 [`DisplayDirty`]
//! - [`display_wire`]: 64 要素の表示差分を、予算を共有してバイト列へ写す IPC の部品
//! - [`dep`]: 寸法・ハッチング・属性・開口などの要素間の依存辺をたどる [`DepGraph`]
//! - [`rank`]: レコード番号ベースの形式向けに `rank` と `select` を高速に引く [`RankBitset`]

pub mod cow;
pub mod dep;
pub mod dirty;
pub mod display;
pub mod display_wire;
pub mod handle;
pub mod hit;
pub mod paged;
pub mod rank;
pub mod source;
pub mod spatial;
pub mod table;
pub mod txn;

pub use cow::{CowColumn, CowIter};
pub use dep::{DepBefore, DepGraph};
pub use dirty::{DIRTY_BLOCK, DirtyColumn};
pub use display::{Display, DisplayDirty, DisplayWriter, NO_OWNER, RowBuffers, Slot};
pub use handle::{HandleBefore, HandleIndex, Revision, RowId};
pub use paged::{BytePool, Iter, Paged, TextSpan};
pub use rank::RankBitset;
pub use source::{
    CompressedPart, PageReader, RecordSlice, SourceBytes, compress_pages_slice,
    compress_pages_slice_with_cx,
};
pub use spatial::{BoundsSource, Grid, SpatialGrid};
pub use table::{EMPTY_BOUNDS, TableRow, TableSnapshot, VersionedTable};
pub use txn::{
    BornRowDiff, CommittedTxn, ExistingRowDiff, History, PatchPayload, SideIndexes, StageError,
    Transaction, TxnError, run_transaction,
};
