//! AutoCAD の系統（DXF・DWG）のファイルモデル（L1）と、そのコーデック（決定 0008）。
//!
//! 今は ASCII の DXF だけを読み書きする（[`dxf`]）。DXF と DWG が共有するオブジェクトの表は [`db`]（決定 0023）。

pub mod db;
pub mod dxf;

pub use db::{AcadDb, Data, DataType, Object, Origin, Section, Value};
pub use dxf::{Dxf, DxfFile, DxfTxn, Line, Record, Tag, ViewError};
