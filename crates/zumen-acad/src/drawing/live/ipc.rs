//! 常駐 IPC が表示の定義を引く読み取り。状態・保存・`apply` と表示の作り方は変えない。

use zumen_base::{Cx, Kind, Stop};
use zumen_drawing::{Element, FormattedText, Layer, LineType, MarkContent, Piece, Placed, Shape};
use zumen_live::RowId;
use zumen_live::display::Display;
use zumen_sdk::Error;

use super::AcadLive;
use super::shape::{Xform, display_layer, pack_color, shape_display_counts, write_shape_display};

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

impl AcadLive {
    /// 線種の表。図面モデルの `line_types` と同じ順・同じ ID で、`Restyle` の線種にこの ID を使う。
    /// 名前・説明・列・添え物のバイト数と線種の数を、作る前に共有の予算で数える
    pub fn display_line_types(&self, cx: &mut Cx<'_>) -> Result<Vec<LineType>, Error> {
        let catalog = &self.state.catalog;
        let mut bytes = 0_u64;
        for &slot in &catalog.active_line_type_slots {
            if let Some(entry) = catalog.line_types.get(slot as usize) {
                let defined = entry.defined.as_ref().map_or(0, line_type_bytes);
                bytes = bytes
                    .saturating_add(entry.name.len() as u64)
                    .saturating_add(defined);
            }
        }
        let count = catalog.active_line_type_slots.len() as u64;
        (|| -> Result<(), Stop> {
            cx.check(Kind::Work, count)?;
            cx.check(Kind::DecodedBytes, bytes)
        })()
        .map_err(|stop| stopped(cx, stop))?;
        let line_types = catalog.build_line_types();
        // 生きたスロットには必ず項目がある。そろわなければ表を作らない
        if line_types.len() != catalog.active_line_type_slots.len() {
            return Err(Error::Invalid);
        }
        Ok(line_types)
    }
}

impl AcadLive {
    /// 生きた行の文字。`Shape::Text` は 1、`Shape::Formatted` は 2 と、制御文字や書式の符号を含む元の内容。
    /// ほかの形の行は 0 と空。行が生きていなければ `None`。属性・寸法・表の中の文字は返さない。
    /// 行 1 つと内容のバイト数を共有の予算で数える
    pub fn display_text(
        &self,
        row: RowId,
        cx: &mut Cx<'_>,
    ) -> Result<Option<(u32, String)>, Error> {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        match self.element_with_source(row, cx)? {
            Some(element) => copied_text(&element.shape, cx).map(Some),
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
        match self.element_with_source(row, cx)? {
            Some(element) => Ok(retexted(&element, content, cx)?.map(Placed::Model)),
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

/// 線種を写すときのバイト数の見積もり。名前・説明・列（1 つ 8 バイト）・添え物（1 つ 64 バイトと文字）
pub(crate) fn line_type_bytes(line_type: &LineType) -> u64 {
    let mut bytes = line_type.name.len() as u64;
    if let Some(definition) = &line_type.definition {
        bytes = bytes
            .saturating_add(definition.description.len() as u64)
            .saturating_add((definition.pattern.len() as u64).saturating_mul(8));
        for mark in &definition.marks {
            let text = match &mark.content {
                MarkContent::Text { content, style } => {
                    content.len() + style.as_ref().map_or(0, String::len)
                }
                MarkContent::Shape { .. } => 0,
            };
            bytes = bytes.saturating_add(64).saturating_add(text as u64);
        }
    }
    bytes
}

impl AcadLive {
    /// 表示の挿入が指す記号の定義の数。挿入の定義の値は `0..` この数か、定義が見つからない印
    pub fn display_symbol_count(&self) -> u32 {
        u32::try_from(self.state.symbols.len()).unwrap_or(u32::MAX)
    }

    /// 記号の定義 `index` の要素を、定義の座標のまま 1 要素 1 行で `display` の末尾に足し、足した行の数を返す。
    /// 行のスタイルは文書の表示と同じ画層の値と色。入れ子の挿入は、挿入のまま書く。範囲外なら `None`
    pub fn display_symbol(
        &self,
        index: u32,
        display: &mut Display,
        cx: &mut Cx<'_>,
    ) -> Result<Option<u32>, Error> {
        let symbols = &self.state.symbols;
        let Some(definition) = symbols.get(index as usize) else {
            return Ok(None);
        };
        let layer_ids = &self.state.catalog.layer_remap;
        for element in &definition.space.elements {
            let counts = shape_display_counts(&element.shape, symbols, 0);
            let style = [
                display_layer(layer_ids, element.layer.0),
                pack_color(element.style.color),
            ];
            display
                .push_with_cx(cx, counts, style, |writer| {
                    write_shape_display(
                        &element.shape,
                        symbols,
                        layer_ids,
                        &Xform::IDENTITY,
                        0,
                        writer,
                    );
                })
                .map_err(|stop| stopped(cx, stop))?;
        }
        Ok(Some(
            u32::try_from(definition.space.elements.len()).map_err(|_| Error::Invalid)?,
        ))
    }
}

fn stopped(cx: &mut Cx<'_>, stop: Stop) -> Error {
    cx.report(stop.to_diagnostic());
    Error::Stopped(stop)
}

#[cfg(test)]
mod tests;
