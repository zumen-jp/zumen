//! 複写する引出線・記号の寸法表示を、編集の前提条件に使う古いハッシュとは別に照合する。

use super::*;
use zumen_base::{Kind, Stop};
use zumen_drawing::{DimensionDisplayIndex, DimensionDisplayValues};

struct Display<'a> {
    drawing: &'a Drawing2D,
    index: DimensionDisplayIndex<'a>,
}

impl<'a> Display<'a> {
    fn new(drawing: &'a Drawing2D, cx: &mut Cx<'_>) -> Result<Self, Stop> {
        Ok(Self {
            drawing,
            index: DimensionDisplayIndex::new(&drawing.dimension_display, cx)?,
        })
    }

    fn font(&self, name: &str, cx: &mut Cx<'_>) -> Result<Option<&'a TextStyle>, Stop> {
        for style in &self.drawing.text_styles {
            cx.check(Kind::Work, 1 + name.len().min(style.name.len()) as u64 / 16)?;
            if style.name.eq_ignore_ascii_case(name) {
                return Ok(Some(style));
            }
        }
        Ok(None)
    }

    fn same_values(
        &self,
        src: Option<&DimensionDisplayValues>,
        dst: &Self,
        other: Option<&DimensionDisplayValues>,
        cx: &mut Cx<'_>,
    ) -> Result<bool, Stop> {
        let (Some(src), Some(other)) = (src, other) else {
            return Ok(src.is_none() && other.is_none());
        };
        let src_font = src.text_style.as_deref();
        let dst_font = other.text_style.as_deref();
        cx.check(
            Kind::Work,
            1 + src_font
                .map_or(0, str::len)
                .min(dst_font.map_or(0, str::len)) as u64
                / 16,
        )?;
        if src_font != dst_font {
            let (Some(a), Some(b)) = (src_font, dst_font) else {
                return Ok(false);
            };
            if !a.eq_ignore_ascii_case(b) {
                return Ok(false);
            }
            // 異なる表記の文字スタイル名だけ合わせ、残るすべての項目を比較する。
            cx.check(
                Kind::DecodedBytes,
                (a.len() as u64).saturating_add(b.len() as u64),
            )?;
            let mut value = other.clone();
            value.text_style.clone_from(&src.text_style);
            if src != &value {
                return Ok(false);
            }
        } else if src != other {
            return Ok(false);
        }
        let Some(font) = src_font else {
            return Ok(true);
        };
        Ok(match (self.font(font, cx)?, dst.font(font, cx)?) {
            (Some(a), Some(b)) => a.definition == b.definition,
            _ => false,
        })
    }

    fn style(&self, name: &str, cx: &mut Cx<'_>) -> Result<Option<&'a DimensionStyle>, Stop> {
        for style in &self.drawing.dimension_styles {
            cx.check(Kind::Work, 1 + name.len().min(style.name.len()) as u64 / 16)?;
            if style.name.eq_ignore_ascii_case(name) {
                return Ok(Some(style));
            }
        }
        Ok(None)
    }

    fn check_style(
        &self,
        dst: &Self,
        name: &str,
        cx: &mut Cx<'_>,
    ) -> Result<Result<(), Refusal>, Stop> {
        match (self.style(name, cx)?, dst.style(name, cx)?) {
            (Some(a), Some(b)) => {
                // 従来の定義の照合を先に行い、表示設定は続けて個別に照合する。
                if Hash::dimension_style_checked(a, cx)? != Hash::dimension_style_checked(b, cx)? {
                    return Ok(Err(("dimension_style", "conflict")));
                }
            }
            (None, None) if name.eq_ignore_ascii_case("Standard") => {}
            _ => return Ok(Err(("dimension_style", "missing"))),
        }
        let a = self.index.style_values(name, cx)?;
        let b = dst.index.style_values(name, cx)?;
        Ok(if self.same_values(a, dst, b, cx)? {
            Ok(())
        } else {
            Err(("dimension_style", "conflict"))
        })
    }
}

pub(super) fn stopped(cx: &mut Cx<'_>, at: &Ref, stop: Stop) -> Error {
    cx.report(stop.to_diagnostic().at(at.clone()));
    Error::Stopped(stop)
}

pub(super) fn check_style(
    a: &Drawing2D,
    b: &Drawing2D,
    name: &str,
    cx: &mut Cx<'_>,
) -> Result<Result<(), Refusal>, Stop> {
    let src = Display::new(a, cx)?;
    let dst = Display::new(b, cx)?;
    src.check_style(&dst, name, cx)
}

/// 公開 ID に使う記号名を、重複ハンドルを含め、図面と同じ順に決める。
fn symbols<'a>(
    drawing: &'a Drawing2D,
    cx: &mut Cx<'_>,
) -> Result<BTreeMap<String, (&'a SymbolDefinition, String)>, Stop> {
    let mut names = id::DefinitionNamer::default();
    let mut items = BTreeMap::new();
    for def in &drawing.symbols {
        cx.check(Kind::Work, 1 + def.key.name.len() as u64 / 16)?;
        cx.check(
            Kind::DecodedBytes,
            192u64.saturating_add(def.key.name.len() as u64),
        )?;
        let place = names.next(def);
        if def.key.namespace == BLOCK {
            items
                .entry(def.key.name.to_ascii_lowercase())
                .or_insert((def, place));
        }
    }
    Ok(items)
}

pub(super) fn same_in_symbol(
    a: &Drawing2D,
    b: &Drawing2D,
    name: &str,
    cx: &mut Cx<'_>,
) -> Result<bool, Stop> {
    if a.dimension_display.is_empty() && b.dimension_display.is_empty() {
        return Ok(true);
    }
    let src = Display::new(a, cx)?;
    let dst = Display::new(b, cx)?;
    let src_symbols = symbols(a, cx)?;
    let dst_symbols = symbols(b, cx)?;
    let mut seen = BTreeSet::new();
    let mut pending = vec![name.to_ascii_lowercase()];
    while let Some(name) = pending.pop() {
        cx.check(Kind::Work, 1 + name.len() as u64 / 16)?;
        let ((left_def, left_place), (right_def, right_place)) =
            match (src_symbols.get(&name), dst_symbols.get(&name)) {
                (Some(a), Some(b)) => (a, b),
                (None, None) => continue,
                _ => return Ok(false),
            };
        if !seen.insert(name) {
            continue;
        }
        let mut left = id::Namer::new(left_place);
        let mut right = id::Namer::new(right_place);
        if left_def.space.elements.len() != right_def.space.elements.len() {
            return Ok(false);
        }
        for (index, (ea, eb)) in left_def
            .space
            .elements
            .iter()
            .zip(&right_def.space.elements)
            .enumerate()
        {
            cx.check(Kind::Work, 1)?;
            cx.check(Kind::DecodedBytes, 192)?;
            let aid = left.next(index, ea);
            let bid = right.next(index, eb);
            match (&ea.shape, &eb.shape) {
                (Shape::Dimension(da), Shape::Dimension(db)) => {
                    if !da.style.eq_ignore_ascii_case(&db.style)
                        || src.check_style(&dst, &da.style, cx)?.is_err()
                    {
                        return Ok(false);
                    }
                    let va =
                        src.index
                            .element_values(&aid, |cx| Hash::element_checked(ea, cx), cx)?;
                    let vb =
                        dst.index
                            .element_values(&bid, |cx| Hash::element_checked(eb, cx), cx)?;
                    if !src.same_values(va, &dst, vb, cx)? {
                        return Ok(false);
                    }
                }
                (Shape::Leader(la), Shape::Leader(lb)) => {
                    if !la.style.eq_ignore_ascii_case(&lb.style)
                        || src.check_style(&dst, &la.style, cx)?.is_err()
                    {
                        return Ok(false);
                    }
                }
                (Shape::Symbol(sa), Shape::Symbol(sb)) => {
                    if sa.key.namespace != BLOCK && sa.key == sb.key {
                        continue;
                    }
                    if sa.key.namespace != BLOCK
                        || sb.key.namespace != BLOCK
                        || !sa.key.name.eq_ignore_ascii_case(&sb.key.name)
                    {
                        return Ok(false);
                    }
                    cx.check(Kind::DecodedBytes, sa.key.name.len() as u64)?;
                    pending.push(sa.key.name.to_ascii_lowercase());
                }
                (Shape::Dimension(_) | Shape::Leader(_) | Shape::Symbol(_), _)
                | (_, Shape::Dimension(_) | Shape::Leader(_) | Shape::Symbol(_)) => {
                    return Ok(false);
                }
                _ => {}
            }
        }
    }
    Ok(true)
}
