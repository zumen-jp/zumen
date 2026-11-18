//! 常駐 IPC が表示の定義を引く読み取り。状態・保存・`apply` と表示の作り方は変えない。

use zumen_base::{Cx, Kind, Stop};
use zumen_drawing::Layer;
use zumen_sdk::Error;

use super::AcadLive;

impl AcadLive {
    /// 表示のスタイルの画層の値（画層のスロット番号）と、その画層の組。図面モデルの `layers` と同じ順。
    /// 名前と説明のバイト数と画層の数を、作る前に共有の予算で数える
    pub fn display_layers(&self, cx: &mut Cx<'_>) -> Result<Vec<(u32, Layer)>, Error> {
        let catalog = &self.state.catalog;
        let mut bytes = 0_u64;
        for &slot in &catalog.active_layer_slots {
            if let Some(entry) = catalog.layers.get(slot as usize) {
                let defined = entry.defined.as_ref().map_or(0, |layer| {
                    layer.name.len() + layer.description.as_ref().map_or(0, String::len)
                });
                bytes = bytes.saturating_add((entry.name.len() + defined) as u64);
            }
        }
        let count = catalog.active_layer_slots.len() as u64;
        (|| -> Result<(), Stop> {
            cx.check(Kind::Work, count)?;
            cx.check(Kind::DecodedBytes, bytes)
        })()
        .map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        let layers = catalog.build_layers();
        // 生きたスロットには必ず項目がある。そろわなければ組を作らない
        if layers.len() != catalog.active_layer_slots.len() {
            return Err(Error::Invalid);
        }
        Ok(catalog
            .active_layer_slots
            .iter()
            .copied()
            .zip(layers)
            .collect())
    }
}

#[cfg(test)]
mod tests;
