//! 常駐 IPC が行（[`RowId`]）で引く読み取り。表示の座標は用紙の座標で、[`SxfLive::display`] と同じ。
//!
//! 状態・保存・`apply` は変えない。名前と形は `AcadLive` の同名の関数に合わせる。

use zumen_base::{Cx, Kind, Stop};
use zumen_drawing::{Affine, Placed, Seen};
use zumen_live::RowId;
use zumen_sdk::Error;

use super::{SxfContainer, SxfLive, hash_placed};

impl SxfLive {
    /// 表示の座標の矩形に掛かる生きた行を `out` の後ろに足す。[`Self::select_sheet`] と同じ行を返す。
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
        cx.check(Kind::Work, 1)
            .and_then(|()| cx.check(Kind::DecodedBytes, found.canonical_id.len() as u64))
            .map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Error::Stopped(stop)
            })?;
        let hash = hash_placed(&found.placed, cx)?;
        Ok(Some(Seen {
            id: found.canonical_id.to_string(),
            hash,
        }))
    }

    /// 行の座標から表示の座標への変換。用紙の行は恒等の変換、実寸の空間の行は、その空間を置いた 1 つのビューの変換。
    /// 空間を置いたビューがないか 2 つ以上あるとき（表示が 1 つに決まらない）と、記号の定義の中の行は `None`
    pub fn sheet_of_row(&self, row: RowId) -> Option<Affine> {
        if !self.table.is_alive(row) {
            return None;
        }
        let found = self.table.row(row)?;
        match (found.container, &found.placed) {
            (SxfContainer::Sheet(_), Placed::Paper(_)) => Some(Affine {
                m: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            }),
            (SxfContainer::Space(space), Placed::Model(_)) => match self.space_views.get(&space) {
                Some(views) if views.len() == 1 => views.first().copied(),
                _ => None,
            },
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
