//! 表示の行から、編集に使う現在の正規 ID とハッシュを得る。

use zumen_base::{Cx, Kind, Stop};
use zumen_drawing::{Hash, Seen};
use zumen_live::{RowId, TableRow};
use zumen_sdk::Error;

use super::open::LiveState;

impl LiveState {
    pub(super) fn seen_of_row(
        &self,
        row_id: RowId,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Seen>, Error> {
        cx.check(Kind::Work, 1).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        let Some(row) = self.table.row(row_id) else {
            return Ok(None);
        };
        let result = (|| -> Result<Option<Seen>, Stop> {
            let id = if let Some(handle) = row.row_handle() {
                // 接頭辞 9 バイトと大文字 16 進の桁数。文字列を確保する前に検査する。
                let digits = (64 - handle.leading_zeros()).div_ceil(4).max(1);
                cx.check(Kind::DecodedBytes, 9 + u64::from(digits))?;
                format!("space/0/h{handle:X}")
            } else {
                // キーなし・重複ハンドルの正規 ID は、現在の状態を作ったときの表を使う。
                // 表を逆引きする走査は予算で制限し、ID の生成規則をここで重ねない。
                let mut found = None;
                for (id, &candidate) in &self.non_unique_model_ids {
                    cx.check(Kind::Work, 1)?;
                    if candidate == row_id {
                        cx.check(Kind::DecodedBytes, id.len() as u64)?;
                        found = Some(id.clone());
                        break;
                    }
                }
                let Some(id) = found else {
                    return Ok(None);
                };
                id
            };
            self.shapes.check_element_copy(row, cx)?;
            let Some(element) = self.shapes.to_element(row, &self.catalog) else {
                return Ok(None);
            };
            // Ref のハッシュはキーだけを使う。保存後のレコード・バイト位置への写し替えは不要。
            let hash = Hash::element_checked(&element, cx)?;
            Ok(Some(Seen { id, hash }))
        })();
        result.map_err(|stop| {
            cx.report(stop.to_diagnostic().at(row.source_ref()));
            Error::Stopped(stop)
        })
    }
}

#[cfg(test)]
mod tests;
