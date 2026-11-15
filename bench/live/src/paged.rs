//! `crates/zumen-live` のページ配列・文字列置き場・16 KiB ページ圧縮バイト列を再公開する。

pub use zumen_live::paged::{BytePool, Iter, Paged, TextSpan};
pub use zumen_live::source::{
    CompressedPart, PageReader, RecordSlice, SourceBytes, compress_pages_slice,
};
