//! 既存の公開型の経路を保ち、各形式で共通の集計を使う。
pub(crate) use zumen_sdk::conversion::Report;
pub use zumen_sdk::conversion::{
    ConversionCount, ConversionDetail, ConversionFidelity, ConversionReport,
};
