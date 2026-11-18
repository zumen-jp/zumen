//! 常駐 IPC が表示の定義を引く読み取り。状態・保存・`apply` と表示の作り方は変えない。

use zumen_base::{Cx, Kind, Stop};
use zumen_drawing::Layer;
use zumen_live::RowId;
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

impl AcadLive {
    /// 生きた行を、先に描くものから並べる。図面モデルのモデル空間の `order` と同じ規則で、SORTENTSTABLE の
    /// キー（なければハンドル）の昇順、同じキーは行の順。ハンドルのない行があれば、表を使わず行の順にする。
    /// 行の走査・並べ替えの作業と、並べる組の領域を、作る前に共有の予算で数える
    pub fn display_order(&self, cx: &mut Cx<'_>) -> Result<Vec<RowId>, Error> {
        let table = &self.state.table;
        let total = table.len() as u64;
        cx.check(Kind::Work, total)
            .map_err(|stop| stopped(cx, stop))?;
        let mut alive = 0_u64;
        let mut handles = true;
        for index in 0..table.len() {
            if let Some(row) = RowId::from_index(index).and_then(|id| table.row(id)) {
                alive += 1;
                handles &= row.raw_handle().is_some();
            }
        }
        let sort = self
            .state
            .model_sort_keys
            .as_ref()
            .map(|(_, keys)| keys)
            .filter(|_| handles);
        // 並べる組（キー 8 バイトと行 4 バイト、詰めて 16 バイト）と、比べる回数の上限 n·⌈log2 n⌉
        let compares = sort.map_or(0, |_| {
            alive.saturating_mul(u64::from(u64::BITS - alive.leading_zeros()))
        });
        (|| -> Result<(), Stop> {
            cx.check(Kind::Elements, alive)?;
            cx.check(Kind::DecodedBytes, alive.saturating_mul(16))?;
            cx.check(Kind::Work, compares)
        })()
        .map_err(|stop| stopped(cx, stop))?;
        let mut keyed = Vec::with_capacity(usize::try_from(alive).unwrap_or(0));
        for index in 0..table.len() {
            let Some(id) = RowId::from_index(index) else {
                break;
            };
            if let Some(row) = table.row(id) {
                let handle = row.raw_handle().unwrap_or_default();
                let key = sort.map_or(0, |keys| keys.get(&handle).copied().unwrap_or(handle));
                keyed.push((key, id));
            }
        }
        if sort.is_some() {
            keyed.sort_by_key(|&(key, _)| key);
        }
        Ok(keyed.into_iter().map(|(_, id)| id).collect())
    }
}

fn stopped(cx: &mut Cx<'_>, stop: Stop) -> Error {
    cx.report(stop.to_diagnostic());
    Error::Stopped(stop)
}

#[cfg(test)]
mod tests;
