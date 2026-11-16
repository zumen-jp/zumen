//! 記号定義とビューの詳細。元の順・名前・参照を保ち、複製と計算を予算へ数える。

use super::details::{Error, copied, reserve, source};
use super::*;
use crate::messages as m;
use zumen_drawing::{SheetId, SymbolDefinition, View};

fn failed(error: Error, id: &str, at: &Ref, cx: &mut Cx<'_>) -> Failed {
    let diagnostic = match error {
        Error::Stopped(stop) => stop.to_diagnostic(),
        Error::Invalid(reason) => {
            let bytes = (id.len() as u64).saturating_add(128);
            if let Err(stop) = cx
                .check(Kind::Work, bytes)
                .and_then(|()| cx.check(Kind::DecodedBytes, bytes))
            {
                return fail(cx, stop.to_diagnostic().at(at.clone()));
            }
            Diagnostic::new(codes::DRAWING_METADATA_FAILED)
                .with("id", id)
                .with("reason", reason)
        }
    };
    fail(cx, diagnostic.at(at.clone()))
}

pub(super) fn symbols(
    drawing: &Drawing2D,
    cx: &mut Cx<'_>,
) -> Result<Vec<m::SymbolDefinitionItem>, Failed> {
    let mut result = Vec::new();
    // ID の重複を数える木と、返す値の固定部分も含む。32 bit と 64 bit で同じ量を数える。
    reserve(&mut result, drawing.symbols.len(), 1024, cx)
        .map_err(|error| failed(error, "tables", &Ref::default(), cx))?;
    let mut names = id::DefinitionNamer::default();
    for definition in &drawing.symbols {
        cx.check(Kind::Work, 256)
            .map_err(|stop| stopped(cx, stop))?;
        let id = names.next(definition);
        let item = symbol(&id, definition, cx)
            .map_err(|error| failed(error, &id, &definition.source, cx))?;
        result.push(item);
    }
    Ok(result)
}

fn symbol(
    id: &str,
    definition: &SymbolDefinition,
    cx: &mut Cx<'_>,
) -> Result<m::SymbolDefinitionItem, Error> {
    let base = definition
        .base
        .map(|point| {
            if point.space != definition.space.id || !point.x.is_finite() || !point.y.is_finite() {
                return Err(Error::Invalid("symbol-base"));
            }
            Ok(m::Point2 {
                x: point.x,
                y: point.y,
            })
        })
        .transpose()?;
    let mut attributes = Vec::new();
    reserve(&mut attributes, definition.attributes.len(), 512, cx)?;
    for item in &definition.attributes {
        cx.check(Kind::Work, 1)?;
        let attribute = &item.attribute;
        attributes.push(m::SymbolAttributeDefinition {
            tag: copied(&attribute.tag, cx)?,
            value: copied(&attribute.text.content, cx)?,
            formatted_value: attribute
                .formatted
                .as_ref()
                .map(|text| copied(&text.content, cx))
                .transpose()?,
            prompt: copied(&item.prompt, cx)?,
            constant: item.constant,
            verify: item.verify,
            preset: item.preset,
            hidden: attribute.hidden,
            source: source(&attribute.source),
        });
    }
    Ok(m::SymbolDefinitionItem {
        id: copied(id, cx)?,
        namespace: copied(&definition.key.namespace, cx)?,
        name: copied(&definition.key.name, cx)?,
        base,
        attributes,
        anonymous: definition.anonymous,
        external: definition
            .external
            .as_deref()
            .map(|value| copied(value, cx))
            .transpose()?,
        source: source(&definition.source),
    })
}

pub(super) fn summary(
    drawing: &Drawing2D,
    hash: Option<String>,
    cx: &mut Cx<'_>,
) -> Result<DrawingResult, Failed> {
    let mut spaces = Vec::new();
    let mut sheets = Vec::new();
    reserve(&mut spaces, drawing.spaces.len(), 512, cx)
        .and_then(|()| reserve(&mut sheets, drawing.sheets.len(), 768, cx))
        .map_err(|error| failed(error, "drawing", &Ref::default(), cx))?;
    for space in &drawing.spaces {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        spaces.push(SpaceSummary {
            id: Some(id::space(space.id)),
            unit: unit_of(space.unit),
            elements: counts(&space.elements, cx).map_err(|stop| stopped(cx, stop))?,
            extent: extent(&space.elements, |p| (p.x, p.y)),
        });
    }
    let mut names = id::SheetNamer::default();
    for sheet in &drawing.sheets {
        cx.check(Kind::Work, 256)
            .map_err(|stop| stopped(cx, stop))?;
        let id = names.next(sheet);
        let name = sheet
            .name
            .as_deref()
            .map(|value| copied(value, cx))
            .transpose()
            .map_err(|error| failed(error, &id, &sheet.source, cx))?;
        let mut view_details = Vec::new();
        reserve(&mut view_details, sheet.views.len(), 768, cx)
            .map_err(|error| failed(error, &id, &sheet.source, cx))?;
        for view in &sheet.views {
            let detail = view_detail(drawing, sheet.id, view, cx)
                .map_err(|error| failed(error, &id, &view.source, cx))?;
            view_details.push(detail);
        }
        sheets.push(SheetSummary {
            id: Some(id),
            name,
            size: sheet
                .size
                .map(|size| size.iter().map(|length| length.0).collect()),
            elements: counts(&sheet.elements, cx).map_err(|stop| stopped(cx, stop))?,
            views: sheet.views.len() as u64,
            view_details,
            extent: extent(&sheet.elements, |p| (p.x, p.y)),
        });
    }
    Ok(DrawingResult {
        hash,
        spaces,
        sheets,
        point_style: drawing.point_style.map(point_style_summary),
    })
}

fn counts<P>(elements: &[Element<P>], cx: &mut Cx<'_>) -> Result<ElementCounts, Stop> {
    // この走査と、従来の extent の走査を先に数える。
    cx.check(Kind::Work, (elements.len() as u64).saturating_mul(2))?;
    let mut counts = ElementCounts {
        lines: 0,
        texts: 0,
        circles: 0,
        curves: 0,
        polylines: 0,
        formatted_texts: 0,
        dimensions: 0,
        leaders: 0,
        callouts: 0,
        tables: 0,
        underlays: 0,
        symbols: 0,
        hatches: 0,
        points: 0,
    };
    for element in elements {
        cx.check(Kind::Work, 0)?;
        let count = match &element.shape {
            Shape::Line { .. } => &mut counts.lines,
            Shape::Text(_) => &mut counts.texts,
            Shape::Circle { .. } => &mut counts.circles,
            Shape::Curve(_) => &mut counts.curves,
            Shape::Polyline(_) => &mut counts.polylines,
            Shape::Formatted(_) => &mut counts.formatted_texts,
            Shape::Dimension(_) => &mut counts.dimensions,
            Shape::Leader(_) => &mut counts.leaders,
            Shape::Callout(_) => &mut counts.callouts,
            Shape::Table(_) => &mut counts.tables,
            Shape::Underlay(_) => &mut counts.underlays,
            Shape::Symbol(_) => &mut counts.symbols,
            Shape::Hatch(_) => &mut counts.hatches,
            Shape::Point(_) => &mut counts.points,
        };
        *count += 1;
    }
    Ok(counts)
}

fn view_detail(
    drawing: &Drawing2D,
    sheet: SheetId,
    view: &View,
    cx: &mut Cx<'_>,
) -> Result<m::ViewDetail, Error> {
    cx.check(Kind::Work, (drawing.spaces.len() as u64).saturating_add(64))?;
    let space = drawing
        .spaces
        .iter()
        .find(|space| space.id == view.space)
        .ok_or(Error::Invalid("view-space"))?;
    let [a, b, c, d, e, f] = view.transform.m;
    if view.sheet != sheet || !view.transform.m.iter().all(|value| value.is_finite()) {
        return Err(Error::Invalid("view-transform"));
    }
    let sx = libm::hypot(a, d);
    let sy = libm::hypot(b, e);
    if ![sx, sy]
        .iter()
        .all(|value| value.is_finite() && *value > 0.0)
    {
        return Err(Error::Invalid("view-scale"));
    }
    // 列ごとに正規化し、行列式の積がオーバーフローするのを避ける。
    let (an, bn, dn, en) = (a / sx, b / sy, d / sx, e / sy);
    let determinant = an * en - bn * dn;
    if determinant == 0.0 {
        return Err(Error::Invalid("view-singular"));
    }
    let ratio = space
        .unit
        .millimeters()
        .map(|unit| vec![sx / unit, sy / unit]);
    if ratio.as_ref().is_some_and(|ratio| {
        ratio
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
    }) {
        return Err(Error::Invalid("view-scale"));
    }
    let mut clip = Vec::new();
    let mut model_clip = Vec::new();
    reserve(&mut clip, view.clip.len(), 32, cx)?;
    reserve(&mut model_clip, view.clip.len(), 32, cx)?;
    let mut bounds: Option<Extent> = None;
    for point in &view.clip {
        cx.check(Kind::Work, 16)?;
        if point.sheet != sheet || !point.x.is_finite() || !point.y.is_finite() {
            return Err(Error::Invalid("view-clip"));
        }
        let (dx, dy) = (point.x - c, point.y - f);
        let x = (en * dx - bn * dy) / determinant / sx;
        let y = (an * dy - dn * dx) / determinant / sy;
        if !x.is_finite() || !y.is_finite() {
            return Err(Error::Invalid("view-range"));
        }
        clip.push(m::Point2 {
            x: point.x,
            y: point.y,
        });
        model_clip.push(m::Point2 { x, y });
        bounds = Some(match bounds {
            None => Extent {
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            },
            Some(bounds) => Extent {
                min_x: bounds.min_x.min(x),
                min_y: bounds.min_y.min(y),
                max_x: bounds.max_x.max(x),
                max_y: bounds.max_y.max(y),
            },
        });
    }
    Ok(m::ViewDetail {
        space: id::space(space.id),
        matrix: view.transform.m.to_vec(),
        scale: vec![sx, sy],
        ratio,
        rotation: libm::atan2(d, a).to_degrees(),
        mirrored: determinant < 0.0,
        clip,
        model_clip,
        extent: bounds,
        source: source(&view.source),
    })
}

#[cfg(test)]
#[path = "../../../examples/drawing_metadata_case.rs"]
mod fixture;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_copy_and_view_calculation_stop_at_each_budget_boundary() {
        let mut drawing = fixture::fixture().1;
        drawing.symbols[0].attributes[0].prompt = "定義".repeat(10_000);
        for table in [false, true] {
            let run = |cx: &mut Cx<'_>| {
                if table {
                    symbols(&drawing, cx).map(|_| ())
                } else {
                    summary(&drawing, None, cx).map(|_| ())
                }
            };
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            assert!(
                run(&mut Cx::new(
                    &mut budget,
                    &mut diagnostics,
                    &CancelToken::new()
                ))
                .is_ok()
            );
            for kind in [Kind::Work, Kind::DecodedBytes] {
                let amount = budget.used(kind);
                assert!(amount > 0);
                for (limit, success) in [(amount - 1, false), (amount, true)] {
                    let mut bounded =
                        Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
                    let mut diagnostics = Diagnostics::new();
                    assert_eq!(
                        run(&mut Cx::new(
                            &mut bounded,
                            &mut diagnostics,
                            &CancelToken::new()
                        ))
                        .is_ok(),
                        success
                    );
                    assert_eq!(diagnostics.is_empty(), success);
                }
            }
            let cancel = CancelToken::new();
            cancel.cancel();
            assert!(
                run(&mut Cx::new(
                    &mut Budget::default(),
                    &mut Diagnostics::new(),
                    &cancel
                ))
                .is_err()
            );
            assert!(
                run(&mut Cx::new(
                    &mut Budget::default(),
                    &mut Diagnostics::new(),
                    &CancelToken::new()
                ))
                .is_ok()
            );
        }
    }
}
