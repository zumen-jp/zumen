//! AutoCAD の系統（DXF・DWG）のファイルモデル（L1）と、そのコーデック（決定 0008）。
//!
//! ASCII の DXF を読み書きする（[`dxf`]）。DWG は R2000・R2004・R2010・R2013・R2018 を読み、R2000 のオブジェクトのデータを置き換えられる（[`dwg`]）。DXF と DWG が共有するオブジェクトの表は [`db`]（決定 0023）。

pub mod charset;
mod conversion_report;
pub mod db;
#[cfg(feature = "drawing")]
mod drawing;
pub mod dwg;
pub mod dxf;
mod snapshot;

pub use charset::{Charset, charset};
pub use conversion_report::{
    ConversionCount, ConversionDetail, ConversionFidelity, ConversionReport,
};
pub use db::{AcadDb, Data, DataType, Line, Object, Origin, Section, Value, ViewError};
#[cfg(feature = "drawing")]
pub use drawing::{
    Lowered, NewDxfTemplate, Relocate, Reseat, dimension_spans, lift as lift_drawing, lower_to_dxf,
    lower_to_dxf_with, lower_to_dxf_with_report, mtext_spans, single,
};
pub use dwg::{Dwg, DwgFile, DwgTxn, Locator};
pub use dxf::{Dxf, DxfFile, DxfTxn, Record, Tag};
