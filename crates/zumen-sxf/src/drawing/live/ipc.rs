//! 常駐 IPC が行（[`RowId`]）で引く読み取り。表示の座標は用紙の座標で、[`SxfLive::display`] と同じ。
//!
//! 状態・保存・`apply` は変えない。名前と形は `AcadLive` の同名の関数に合わせる。

use zumen_base::{Cx, Kind, Stop};
use zumen_drawing::{Affine, Layer, LineType, LineTypeId, Placed, Seen, Shape};
use zumen_live::RowId;
use zumen_sdk::Error;

use super::super::{LINE_TYPES, USER_LINE_TYPE};
use super::{BaseSheetSlot, SxfContainer, SxfLive, hash_placed};

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

    /// 表示のスタイルの画層の値（図面モデルの `LayerId`）と、その画層の組。図面モデルの `layers` と同じ順。
    /// 要素が名前で指すだけの画層も含める。生きた行の走査と、写した名前・説明のバイト数を共有の予算で数える
    pub fn display_layers(&self, cx: &mut Cx<'_>) -> Result<Vec<(u32, Layer)>, Error> {
        let layers = self.current_layers(cx)?;
        let bytes = layers
            .iter()
            .map(|layer| {
                (layer.name.len() + layer.description.as_ref().map_or(0, String::len)) as u64
            })
            .fold(0_u64, u64::saturating_add);
        cx.check(Kind::DecodedBytes, bytes).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        Ok(layers
            .into_iter()
            .map(|layer| (layer.id.0, layer))
            .collect())
    }
}

impl SxfLive {
    /// `apply` の `restyle` が受ける線種の表。ID は線種のコードで、既定の線種（1〜15）、文書が定義する利用者定義の
    /// 線種の順。既定の線種は図面モデルの線種があればそれを、なければ既定の名前だけを返す。
    /// 線種の数と、名前・説明・列のバイト数を、作る前に共有の予算で数える
    pub fn display_line_types(&self, cx: &mut Cx<'_>) -> Result<Vec<LineType>, Error> {
        let base = &self.lifter_tables.base_line_types;
        let user = |line_type: &&LineType| {
            i64::from(line_type.id.0) > USER_LINE_TYPE && line_type.source != Default::default()
        };
        let mut bytes = LINE_TYPES
            .iter()
            .map(|name| name.len() as u64)
            .fold(0_u64, u64::saturating_add);
        for line_type in base {
            bytes = bytes.saturating_add(definition_bytes(line_type));
        }
        let count = (LINE_TYPES.len() as u64).saturating_add(base.len() as u64);
        (|| -> Result<(), Stop> {
            cx.check(Kind::Work, count)?;
            cx.check(Kind::DecodedBytes, bytes)
        })()
        .map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        let mut out = Vec::with_capacity(LINE_TYPES.len());
        for (at, name) in (1_u32..).zip(LINE_TYPES) {
            let id = LineTypeId(at);
            out.push(
                base.iter()
                    .find(|line_type| line_type.id == id)
                    .cloned()
                    .unwrap_or_else(|| LineType {
                        id,
                        name: (*name).to_owned(),
                        definition: None,
                        source: Default::default(),
                    }),
            );
        }
        out.extend(base.iter().filter(user).cloned());
        Ok(out)
    }
}

impl SxfLive {
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
        match self.table.row(row).map(|found| &found.placed) {
            Some(Placed::Model(element)) => copied_text(&element.shape, cx).map(Some),
            Some(Placed::Paper(element)) => copied_text(&element.shape, cx).map(Some),
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

/// [`text_of`] の内容を、バイト数を共有の予算で数えてから写す
fn copied_text<P>(shape: &Shape<P>, cx: &mut Cx<'_>) -> Result<(u32, String), Error> {
    let (kind, content) = text_of(shape);
    cx.check(Kind::DecodedBytes, content.len() as u64)
        .map_err(|stop| stopped(cx, stop))?;
    Ok((kind, content.to_owned()))
}

/// 線種を写すときのバイト数の見積もり。名前・説明・列（1 つ 8 バイト）・添え物（1 つ 64 バイト）
fn definition_bytes(line_type: &LineType) -> u64 {
    let definition = line_type.definition.as_ref().map_or(0, |definition| {
        (definition.description.len() as u64)
            .saturating_add((definition.pattern.len() as u64).saturating_mul(8))
            .saturating_add((definition.marks.len() as u64).saturating_mul(64))
    });
    (line_type.name.len() as u64).saturating_add(definition)
}

impl SxfLive {
    /// 表示に出る生きた行を、先に描くものから並べる。図面モデルの用紙の `order` をたどり、用紙の要素はその位置に、
    /// ビューは置く空間の行を空間の順に置く。2 つ以上のビューが置く空間の行は、最初のビューの位置に 1 回だけ並べる。
    /// ビューのない空間と記号の定義の行は、表示に出ないので返さない。走査の作業と、並べる行の領域を、作る前に
    /// 共有の予算で数える
    pub fn display_order(&self, cx: &mut Cx<'_>) -> Result<Vec<RowId>, Error> {
        let rows: u64 = self.space_rows.iter().map(|rows| rows.len() as u64).sum();
        let work = (self.base_sheet_slots.len() as u64)
            .saturating_add(self.table.len() as u64)
            .saturating_add(rows);
        cx.check(Kind::Work, work)
            .map_err(|stop| stopped(cx, stop))?;
        let mut count = 0_u64;
        self.walk_order(|_| count += 1);
        (|| -> Result<(), Stop> {
            cx.check(Kind::Elements, count)?;
            cx.check(Kind::DecodedBytes, count.saturating_mul(4))
        })()
        .map_err(|stop| stopped(cx, stop))?;
        let mut out = Vec::with_capacity(usize::try_from(count).unwrap_or(0));
        self.walk_order(|row| out.push(row));
        Ok(out)
    }

    /// 用紙の並び（`drawing` の `sheet_order` と同じ）で、表示に出る行を `visit` に渡す
    fn walk_order(&self, mut visit: impl FnMut(RowId)) {
        let insertion = self
            .diagnostics
            .sheet_insertion()
            .min(self.base_sheet_slots.len());
        let (before, after) = self.base_sheet_slots.split_at(insertion);
        let born = (self.base_row_count..self.table.len() as u32)
            .map(|row| BaseSheetSlot::Row(RowId(row)));
        let mut placed = vec![false; self.space_templates.len()];
        for slot in before
            .iter()
            .copied()
            .chain(born)
            .chain(after.iter().copied())
        {
            match slot {
                BaseSheetSlot::View(view) => {
                    let Some(space) = self.sheet_template.views.get(view).map(|view| view.space)
                    else {
                        continue;
                    };
                    let Some(index) = self
                        .space_templates
                        .iter()
                        .position(|found| found.id == space)
                    else {
                        continue;
                    };
                    if placed.get(index).copied().unwrap_or(true) {
                        continue;
                    }
                    if let Some(done) = placed.get_mut(index) {
                        *done = true;
                    }
                    for &row in self.space_rows.get(index).into_iter().flatten() {
                        if self
                            .table
                            .row(row)
                            .is_some_and(|found| matches!(found.placed, Placed::Model(_)))
                        {
                            visit(row);
                        }
                    }
                }
                BaseSheetSlot::Row(row) => {
                    if self
                        .table
                        .row(row)
                        .is_some_and(|found| matches!(found.placed, Placed::Paper(_)))
                    {
                        visit(row);
                    }
                }
            }
        }
    }
}

fn stopped(cx: &mut Cx<'_>, stop: Stop) -> Error {
    cx.report(stop.to_diagnostic());
    Error::Stopped(stop)
}

#[cfg(test)]
mod tests;
