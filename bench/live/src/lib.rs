//! 文書エンジンの試作（`docs/live.md`）。
//!
//! 文書を開いたままメモリに置き、変更を、変えた要素の分だけの手間で行う。この試作は ASCII の DXF の
//! モデル空間の線・円・円弧・折れ線・文字を扱い、動かす変更・取り消し・やり直し・全体の保存・範囲で選ぶことを持つ。
//! 保存したバイト列は、同じ変更を今の実装の `apply` で 1 つずつ行ったバイト列と同じになるように作る。

pub mod display;
pub mod edit;
pub mod grid;
pub mod grow;
pub mod lift;
pub mod save;
pub mod scan;
pub mod store;
pub mod value;
