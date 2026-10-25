//! AutoCAD の系統（DXF・DWG）のファイルモデル（L1）と、そのコーデック（決定 0008）。
//!
//! 今は ASCII の DXF だけを読み書きする（[`dxf`]）。DWG と共有するモデルの分担は、#3 の試作で決める。

pub mod dxf;

pub use dxf::{Dxf, DxfFile, DxfTxn, Line, Record, Tag, ViewError};
