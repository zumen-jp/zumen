//! 常駐 IPC が行（[`RowId`]）で引く読み取り。表示の座標は用紙の mm で、[`JwwLive::display`] と同じ。
//!
//! 状態・保存・`apply` は変えない。名前と形は `AcadLive` の同名の関数に合わせる。

use zumen_base::{Cx, Kind, Stop};
use zumen_drawing::{
    Affine, Element, FormattedText, Hash, Layer, LineType, Piece, Placed, Seen, Shape,
};
use zumen_live::RowId;
use zumen_sdk::Error;

use super::super::{SXF_LINE_TYPE, SXF_SINCE, build_layers, make_line_type};
use super::{BaseOutcome, DEFINITION_SPACES, JwwLive};

impl JwwLive {
    /// 表の行の数（削除した行を含む）。足した行は末尾に並ぶ
    pub fn row_count(&self) -> usize {
        self.table.len()
    }

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

    /// 表示のスタイルの画層の値（図面モデルの `LayerId`）と、その画層の組。図面モデルの `layers` と同じ順。
    /// 名前のバイト数と画層の数を、作る前に共有の予算で数える
    pub fn display_layers(&self, cx: &mut Cx<'_>) -> Result<Vec<(u32, Layer)>, Error> {
        // 名前のない画層は `{グループ}-{画層}`（3 バイト）になる
        let bytes = self
            .header
            .layer_names
            .iter()
            .map(|name| name.len().max(3) as u64)
            .fold(0_u64, u64::saturating_add)
            .saturating_add(LAYERS.saturating_sub(self.header.layer_names.len() as u64) * 3);
        (|| -> Result<(), Stop> {
            cx.check(Kind::Work, LAYERS)?;
            cx.check(Kind::DecodedBytes, bytes)
        })()
        .map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        Ok(build_layers(&self.header)
            .into_iter()
            .map(|layer| (layer.id.0, layer))
            .collect())
    }
}

impl JwwLive {
    /// 生きた最上位の行を、先に描くものから並べる。図面モデルと同じく、画層グループ（空間）の順に置いたビューの
    /// 順で、同じグループの中はレコードの順（追加した行は一覧の末尾の前）。記号の定義の中の行は返さない。
    /// レコードの走査・並べ替えの作業と、並べる組の領域を、作る前に共有の予算で数える
    pub fn display_order(&self, cx: &mut Cx<'_>) -> Result<Vec<RowId>, Error> {
        let records = self.base_records.len() as u64 + self.born_records.len() as u64;
        cx.check(Kind::Work, records)
            .map_err(|stop| stopped(cx, stop))?;
        let top = self.top_end.min(self.base_records.len());
        let base = |range: core::ops::Range<usize>| {
            range.filter_map(|index| match self.base_records.get(index)?.outcome {
                BaseOutcome::Element(row) if self.base_alive.get(index) => Some(row),
                _ => None,
            })
        };
        let born = (0..self.born_records.len()).filter_map(|index| {
            self.born_alive
                .get(index)
                .then(|| self.born_records.get(index).map(|born| born.row_id))
                .flatten()
        });
        let rows = base(0..top)
            .chain(born)
            .chain(base(top..self.base_records.len()))
            .filter_map(|row| {
                let found = self.table.row(row)?;
                (found.space.0 < DEFINITION_SPACES).then_some((found.space.0, row))
            });
        let count = rows.clone().count() as u64;
        let compares = count.saturating_mul(u64::from(u64::BITS - count.leading_zeros()));
        (|| -> Result<(), Stop> {
            cx.check(Kind::Elements, count)?;
            cx.check(Kind::DecodedBytes, count.saturating_mul(8))?;
            cx.check(Kind::Work, compares)
        })()
        .map_err(|stop| stopped(cx, stop))?;
        let mut keyed: Vec<(u32, RowId)> = rows.collect();
        keyed.sort_by_key(|&(space, _)| space);
        Ok(keyed.into_iter().map(|(_, row)| row).collect())
    }
}

impl JwwLive {
    /// `apply` の `restyle` が受ける線種の表。ID は線種の番号で、実線と破線（1〜9）、ランダム線と倍長線（11〜19）、
    /// SXF の線種を持つ版では定義のある SXF の線種（30 から 62 まで）の順。図面モデルの `line_types` と同じ規則で作る。
    /// 線種の数と、名前・列のバイト数（ビットの列は 1 つ 32 個まで）を、作る前に共有の予算で数える
    pub fn display_line_types(&self, cx: &mut Cx<'_>) -> Result<Vec<LineType>, Error> {
        let header = &self.header;
        let sxf = if header.version >= SXF_SINCE {
            header
                .sxf_line_types
                .len()
                .min(usize::from(MAX_LINE_TYPE - SXF_LINE_TYPE) + 1)
        } else {
            0
        };
        let fixed = fixed_line_types().count() as u64;
        let mut bytes = fixed.saturating_mul(2 + 32 * 8);
        for (name, pattern) in header.sxf_line_types.iter().take(sxf) {
            bytes = bytes
                .saturating_add(name.len().max(2) as u64)
                .saturating_add((pattern.as_ref().map_or(0, Vec::len) as u64).saturating_mul(8));
        }
        (|| -> Result<(), Stop> {
            cx.check(Kind::Work, fixed.saturating_add(sxf as u64))?;
            cx.check(Kind::DecodedBytes, bytes)
        })()
        .map_err(|stop| stopped(cx, stop))?;
        let sxf = (0..sxf).filter_map(|at| u8::try_from(at).ok()?.checked_add(SXF_LINE_TYPE));
        Ok(fixed_line_types()
            .chain(sxf)
            .map(|code| make_line_type(header, code))
            .collect())
    }
}

impl JwwLive {
    /// 生きた行の文字。`Shape::Text` は 1、`Shape::Formatted` は 2 と、制御文字や書式の符号を含む元の内容。
    /// ほかの形の行は 0 と空。行が生きていなければ `None`。属性・寸法・表の中の文字は返さない。
    /// 行 1 つと内容のバイト数を共有の予算で数える
    pub fn display_text(
        &self,
        row: RowId,
        cx: &mut Cx<'_>,
    ) -> Result<Option<(u32, String)>, Error> {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        if !self.table.is_alive(row) {
            return Ok(None);
        }
        match self.table.row(row) {
            Some(found) => copied_text(&found.element.shape, cx).map(Some),
            None => Ok(None),
        }
    }

    /// 生きた文字の行（`Shape::Text`・`Shape::Formatted`）の要素を、内容だけ `content` に替えて返す。`Edit::Replace` の
    /// 新しい値に使う。行が生きていないか、文字でなければ `None`。行 1 つと新しい内容のバイト数を共有の予算で数える
    pub fn display_retexted(
        &self,
        row: RowId,
        content: &str,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Placed>, Error> {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        if !self.table.is_alive(row) {
            return Ok(None);
        }
        match self.table.row(row) {
            Some(found) => Ok(retexted(&found.element, content, cx)?.map(Placed::Model)),
            None => Ok(None),
        }
    }
}

/// 文字の形の種類と、元の符号のままの内容。`Shape::Text` は 1、`Shape::Formatted` は 2、ほかの形は 0 と空
fn text_of<P>(shape: &Shape<P>) -> (u32, &str) {
    match shape {
        Shape::Text(text) => (1, text.content.as_str()),
        Shape::Formatted(text) => (2, text.content.as_str()),
        _ => (0, ""),
    }
}

/// 内容だけを替えられる文字の要素なら、内容を `content` に替えた写し。替えられるのは `Shape::Text` と、区間が
/// 文字列 1 つだけの `Shape::Formatted`（区間の中身も替える）。ほかの形は `None`。
/// 新しい内容のバイト数を共有の予算で数えてから写す
fn retexted<P: Clone>(
    element: &Element<P>,
    content: &str,
    cx: &mut Cx<'_>,
) -> Result<Option<Element<P>>, Error> {
    let plain = |text: &FormattedText<P>| match text.spans.as_slice() {
        [span] => span.piece == Piece::Text(text.content.clone()),
        _ => false,
    };
    match &element.shape {
        Shape::Text(_) => {}
        Shape::Formatted(text) if plain(text) => {}
        _ => return Ok(None),
    }
    cx.check(Kind::DecodedBytes, content.len() as u64)
        .map_err(|stop| stopped(cx, stop))?;
    let mut out = element.clone();
    match &mut out.shape {
        Shape::Text(text) => content.clone_into(&mut text.content),
        Shape::Formatted(text) => {
            content.clone_into(&mut text.content);
            if let Some(span) = text.spans.first_mut() {
                span.piece = Piece::Text(content.to_owned());
            }
        }
        _ => {}
    }
    Ok(Some(out))
}

/// [`text_of`] の内容を、バイト数を共有の予算で数えてから写す
fn copied_text<P>(shape: &Shape<P>, cx: &mut Cx<'_>) -> Result<(u32, String), Error> {
    let (kind, content) = text_of(shape);
    cx.check(Kind::DecodedBytes, content.len() as u64)
        .map_err(|stop| stopped(cx, stop))?;
    Ok((kind, content.to_owned()))
}

/// 版によらず使える線種の番号。実線と破線（1〜9）、ランダム線と倍長線（11〜19）
fn fixed_line_types() -> impl Iterator<Item = u8> {
    (1..=9).chain(11..=19)
}

/// SXF の線種の番号の上限
const MAX_LINE_TYPE: u8 = 62;

fn stopped(cx: &mut Cx<'_>, stop: Stop) -> Error {
    cx.report(stop.to_diagnostic());
    Error::Stopped(stop)
}

/// JWW の画層の数（16 グループ × 16 画層）
const LAYERS: u64 = 256;

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
