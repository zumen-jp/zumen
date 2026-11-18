//! 常駐 IPC が行（[`RowId`]）で引く読み取り。表示の座標は用紙の mm で、[`JwwLive::display`] と同じ。
//!
//! 状態・保存・`apply` は変えない。名前と形は `AcadLive` の同名の関数に合わせる。

use zumen_base::{Cx, Kind, Stop};
use zumen_drawing::{Affine, Hash, Seen};
use zumen_live::RowId;
use zumen_sdk::Error;

use super::{DEFINITION_SPACES, JwwLive};

impl JwwLive {
    /// 表示の座標（用紙の mm）の矩形に掛かる、生きた最上位の行を `out` の後ろに足す。記号の定義の中の行は返さない。
    /// 索引の探索と候補の数を共有の予算で数え、止まったときは `out` を呼ぶ前の長さに戻す
    pub fn select_rect_with_cx(
        &self,
        rect: [f64; 4],
        out: &mut Vec<RowId>,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        let start = out.len();
        let result = (|| -> Result<(), Stop> {
            self.spatial
                .query_with_cx(self.table.bounds_column(), rect, out, cx)?;
            cx.check(Kind::Work, out.len().saturating_sub(start) as u64)?;
            let mut write = start;
            for read in start..out.len() {
                if let Some(row) = out.get(read).copied()
                    && self.table.is_alive(row)
                    && self
                        .table
                        .row(row)
                        .is_some_and(|found| found.space.0 < DEFINITION_SPACES)
                {
                    if let Some(slot) = out.get_mut(write) {
                        *slot = row;
                    }
                    write += 1;
                }
            }
            out.truncate(write);
            Ok(())
        })();
        result.map_err(|stop| {
            out.truncate(start);
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })
    }

    /// 生きた行の、今の正規 ID と要素のハッシュ。`apply` の対象（[`Seen`]）にそのまま使える。
    /// 行が生きていなければ `None`
    pub fn seen_of_row(&self, row: RowId, cx: &mut Cx<'_>) -> Result<Option<Seen>, Error> {
        if !self.table.is_alive(row) {
            return Ok(None);
        }
        let Some(found) = self.table.row(row) else {
            return Ok(None);
        };
        let result = (|| -> Result<Seen, Stop> {
            cx.check(Kind::Work, 1)?;
            let record = self.effective_record(found.slot);
            // `space/{空間}/r{レコード}` の長さ。文字列を確保する前に検査する
            let length = 8 + decimal_digits(found.space.0 as usize) + decimal_digits(record);
            cx.check(Kind::DecodedBytes, length as u64)?;
            let id = self.canonical_id(found.space, found.slot);
            let hash = Hash::element_checked(&found.element, cx)?;
            Ok(Seen { id, hash })
        })();
        result.map(Some).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })
    }

    /// 行の空間の座標から表示の座標（用紙の mm）への変換。記号の定義の中の行と、縮尺が正の有限でない空間は `None`
    pub fn sheet_of_row(&self, row: RowId) -> Option<Affine> {
        if !self.table.is_alive(row) {
            return None;
        }
        let space = self.table.row(row)?.space.0;
        if space >= DEFINITION_SPACES {
            return None;
        }
        let scale = self.header.scale(space as usize);
        if !(scale.is_finite() && scale > 0.0) {
            return None;
        }
        let k = 1.0 / scale;
        let [ox, oy] = self.paper_origin;
        Some(Affine {
            m: [k, 0.0, ox, 0.0, k, oy],
        })
    }
}

/// 10 進の桁数
fn decimal_digits(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

#[cfg(test)]
mod tests;
