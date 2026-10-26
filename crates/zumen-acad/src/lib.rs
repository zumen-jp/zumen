//! AutoCAD の系統（DXF・DWG）のファイルモデル（L1）と、そのコーデック（決定 0008）。
//!
//! ASCII の DXF を読み書きする（[`dxf`]）。DWG は R2000 だけを読む試作（[`dwg`]）。DXF と DWG が共有するオブジェクトの表は [`db`]（決定 0023）。

pub mod db;
#[cfg(feature = "drawing")]
mod drawing;
pub mod dwg;
pub mod dxf;

pub use db::{AcadDb, Data, DataType, Line, Object, Origin, Section, Value, ViewError};
pub use dwg::{Dwg, DwgFile, Locator};
pub use dxf::{Dxf, DxfFile, DxfTxn, Record, Tag};
