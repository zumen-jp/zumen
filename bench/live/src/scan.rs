//! 行とタグの読み。本体は `zumen-acad` の [`zumen_acad::dxf::live_io`] に置く。

pub use zumen_acad::dxf::live_io::{Next, Span, next_tag, parse_code, split_points};
