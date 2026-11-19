//! 常駐 IPC が表示の定義を引く読み取り。状態・保存・`apply` と表示の作り方は変えない。

use zumen_base::{Cx, Kind, Ref, Stop};
use zumen_drawing::{
    Element, Hash, Layer, Length, LineType, MarkContent, ModelLength, ModelPoint, Placed, Seen,
    Shape, SpaceId, SymbolKey, id,
};
use zumen_live::display::Display;
use zumen_live::hit::SymbolHitCache;
use zumen_live::{BoundsSource, RowId};
use zumen_sdk::Error;

use super::super::mtext;
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

    /// 行を雛形にして要素を足すときの、足す先の場所の ID（`Edit::Add` の `place`）と、雛形の画層・スタイルを持つ要素。
    /// 要素の形は、足す先の原点の長さ 0 の線分で、雛形の形は写さない。生きた最上位の要素の行だけが雛形になり、
    /// 生きていない行と記号の定義の中の要素は `None`。行 1 つを共有の予算で数える
    pub fn display_template(
        &self,
        row: RowId,
        cx: &mut Cx<'_>,
    ) -> Result<Option<(String, Placed)>, Error> {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        // DXF・DWG の行は、どれもモデル空間（空間 0）の要素
        let space = SpaceId(0);
        Ok(self.element_with_source(row, cx)?.map(|element| {
            let template = Placed::Model(Element {
                source: Ref::default(),
                layer: element.layer,
                style: element.style,
                shape: Shape::Line {
                    start: ModelPoint {
                        space,
                        x: 0.0,
                        y: 0.0,
                    },
                    end: ModelPoint {
                        space,
                        x: 0.0,
                        y: 0.0,
                    },
                },
            });
            (id::space(space), template)
        }))
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

/// 文字の要素なら、内容を `content` に替えた写し。替えられるのは `Shape::Text` と `Shape::Formatted`（書式・段落の
/// 符号を含む内容から区間を作り直す）。ほかの形は `None`。新しい内容と区間のバイト数を共有の予算で数えてから写す
fn retexted(
    element: &Element<ModelPoint>,
    content: &str,
    cx: &mut Cx<'_>,
) -> Result<Option<Element<ModelPoint>>, Error> {
    match &element.shape {
        Shape::Text(_) => {
            cx.check(Kind::DecodedBytes, content.len() as u64)
                .map_err(|stop| stopped(cx, stop))?;
            let mut out = element.clone();
            if let Shape::Text(text) = &mut out.shape {
                content.clone_into(&mut text.content);
            }
            Ok(Some(out))
        }
        Shape::Formatted(current) => {
            let space = current.point.space;
            cx.check(Kind::Work, 1 + content.len() as u64 / 64)
                .and_then(|()| cx.check(Kind::DecodedBytes, content.len() as u64))
                .map_err(|stop| stopped(cx, stop))?;
            let spans = mtext::spans(content, &|value| {
                Length::Model(ModelLength { space, value })
            });
            cx.check(Kind::DecodedBytes, (spans.len() as u64).saturating_mul(128))
                .map_err(|stop| stopped(cx, stop))?;
            let mut out = element.clone();
            if let Shape::Formatted(text) = &mut out.shape {
                content.clone_into(&mut text.content);
                text.spans = spans;
            }
            Ok(Some(out))
        }
        _ => Ok(None),
    }
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
    /// 表示の座標の点 `point` から許容距離 `tolerance` 以内にある生きた最上位の行を、昇順で `out` の後ろに足す。
    /// 負や非有限の値では何も足さない。停止時は `out` を呼ぶ前の長さに戻し、診断を 1 件報告する
    pub fn select_point_with_cx(
        &self,
        point: [f64; 2],
        tolerance: f64,
        out: &mut Vec<RowId>,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        if !(point[0].is_finite()
            && point[1].is_finite()
            && tolerance.is_finite()
            && tolerance >= 0.0)
        {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            return Ok(());
        }
        let start = out.len();
        let query = [
            point[0] - tolerance,
            point[1] - tolerance,
            point[0] + tolerance,
            point[1] + tolerance,
        ];
        if let Err(err) = self.select_rect_with_cx(query, out, cx) {
            out.truncate(start);
            return Err(err);
        }
        let bounds_col = self.state.table.bounds_column();
        let mut cache = SymbolHitCache::new();
        let result = (|| -> Result<(), Stop> {
            let mut write = start;
            for read in start..out.len() {
                let Some(row) = out.get(read).copied() else {
                    continue;
                };
                let bounds = bounds_col.bounds_at(row.index());
                let hits = cache.row_hits_with_cx(
                    &self.state.display,
                    row.0,
                    bounds,
                    point,
                    tolerance,
                    &mut |def_idx, sym_display, inner_cx| {
                        self.display_symbol_raw(def_idx, sym_display, inner_cx)
                    },
                    cx,
                )?;
                if hits {
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
            stopped(cx, stop)
        })
    }

    /// 表示の挿入が指す記号の定義の数。挿入の定義の値は `0..` この数か、定義が見つからない印
    pub fn display_symbol_count(&self) -> u32 {
        u32::try_from(self.state.symbols.len()).unwrap_or(u32::MAX)
    }

    /// 記号の定義 `index` のキー。範囲外なら `None`
    pub fn display_symbol_key(&self, index: u32) -> Option<SymbolKey> {
        self.state
            .symbols
            .get(index as usize)
            .map(|sym| sym.key.clone())
    }

    /// 記号の定義 `symbol` の中の `element` 番目の要素の正規 ID とハッシュを共有予算で返す。範囲外なら `None`
    pub fn seen_of_symbol_element(
        &self,
        symbol: u32,
        element: u32,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Seen>, Error> {
        let sym_idx = symbol as usize;
        let elem_idx = element as usize;
        let Some(def) = self.state.symbols.get(sym_idx) else {
            return Ok(None);
        };
        let Some(target_el) = def.space.elements.get(elem_idx) else {
            return Ok(None);
        };
        let work_steps = (sym_idx as u64 + 1).saturating_add(elem_idx as u64 + 1);
        cx.check(Kind::Work, work_steps)
            .map_err(|stop| stopped(cx, stop))?;
        let mut def_namer = id::DefinitionNamer::default();
        let mut place = String::new();
        for item in self.state.symbols.iter().take(sym_idx + 1) {
            place = def_namer.next(item);
        }
        let any_keyless = def
            .space
            .elements
            .iter()
            .take(elem_idx + 1)
            .any(|item| item.source.key().is_none());
        let dxf_mapper = match &self.state.backing {
            super::backing::LiveBacking::AsciiDxf(dxf) if any_keyless => {
                dxf.record_mapper_with_cx(cx)?
            }
            _ => None,
        };
        let dwg_mapper = match &self.state.backing {
            super::backing::LiveBacking::Dwg(dwg) if any_keyless => dwg.source_mapper(cx)?,
            _ => None,
        };
        let mut seen = std::collections::BTreeMap::<String, u64>::new();
        let mut canonical = String::new();
        for (idx, item) in def.space.elements.iter().enumerate().take(elem_idx + 1) {
            let mut source = item.source.clone();
            if source.key().is_none() {
                if let Some(mapper) = &dxf_mapper {
                    mapper.remap_ref(&mut source);
                }
                if let Some(mapper) = &dwg_mapper {
                    mapper.remap_ref(&mut source);
                }
            }
            let origin = id::origin(&source, idx);
            let repeat = if source.key().is_none()
                && source.record().is_none()
                && source.bytes().is_none()
            {
                0
            } else {
                let count = seen.entry(origin.clone()).or_insert(0);
                let current = *count;
                *count += 1;
                current
            };
            if idx == elem_idx {
                canonical = match repeat {
                    0 => format!("{place}/{origin}"),
                    n => format!("{place}/{origin}/{n}"),
                };
            }
        }
        cx.check(Kind::DecodedBytes, canonical.len() as u64)
            .map_err(|stop| stopped(cx, stop))?;
        let hash = Hash::element_checked(target_el, cx).map_err(|stop| stopped(cx, stop))?;
        Ok(Some(Seen {
            id: canonical,
            hash,
        }))
    }

    /// 記号の定義 `index` の要素を、定義の座標のまま 1 要素 1 行で `display` の末尾に足し、足した行の数を返す。
    /// 行のスタイルは文書の表示と同じ画層の値と色。入れ子の挿入は、挿入のまま書く。範囲外なら `None`
    pub fn display_symbol(
        &self,
        index: u32,
        display: &mut Display,
        cx: &mut Cx<'_>,
    ) -> Result<Option<u32>, Error> {
        let written = self
            .display_symbol_raw(index, display, cx)
            .map_err(|stop| stopped(cx, stop))?;
        match written {
            None => Ok(None),
            Some(u32::MAX) => Err(Error::Invalid),
            Some(rows) => Ok(Some(rows)),
        }
    }

    fn display_symbol_raw(
        &self,
        index: u32,
        display: &mut Display,
        cx: &mut Cx<'_>,
    ) -> Result<Option<u32>, Stop> {
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
            display.push_with_cx(cx, counts, style, |writer| {
                write_shape_display(
                    &element.shape,
                    symbols,
                    layer_ids,
                    &Xform::IDENTITY,
                    0,
                    writer,
                );
            })?;
        }
        Ok(Some(
            u32::try_from(definition.space.elements.len()).unwrap_or(u32::MAX),
        ))
    }
}

fn stopped(cx: &mut Cx<'_>, stop: Stop) -> Error {
    cx.report(stop.to_diagnostic());
    Error::Stopped(stop)
}

#[cfg(test)]
mod tests;
