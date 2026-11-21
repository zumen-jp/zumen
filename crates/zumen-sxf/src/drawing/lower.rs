//! L2 の基本図形を、新しい P21 の用紙へ書く（#360）。
//! SXF Ver.3.1 STEP AP202 サブセット編の表記・見え方・レイヤ・用紙を使う。

use crate::{Sxf, SxfFile};
use std::collections::BTreeMap;
use zumen_base::{Cx, Diagnostic, Error, Kind, Ref, codes};
use zumen_drawing::{
    Axes, Changes, Color, Drawing2D, Edit, Element, Hash, HeightMeaning, Inherit, LayerId, Length,
    LineWeight, MarkerKey, ModelPoint, PaperLength, PaperPoint, Placed, PointLook, Seen, Shape,
    SheetId, id, transfer,
};
use zumen_sdk::conversion::{self, ConversionFidelity as Fidelity, ConversionReport, Report};
use zumen_sdk::{Apply, Codec, Lift};

mod dimension;
mod template;

/// 新しい P21 と、書いた要素数・忠実度。
pub struct Lowered {
    /// ファイルモデル。
    pub file: SxfFile,
    /// P21 のバイト列。
    pub bytes: Vec<u8>,
    /// 出力した図形の数。
    pub elements: u64,
    /// 上限付きの見積もり。
    pub report: ConversionReport,
}

struct Item {
    element: Element<PaperPoint>,
    appearance: transfer::Appearance,
    kind: &'static str,
    source: Ref,
    fidelity: Fidelity,
}

struct Lower<'a> {
    drawing: &'a Drawing2D,
    items: Vec<Item>,
    dimensions: Vec<dimension::Linear>,
    layers: BTreeMap<LayerId, u32>,
    report: Report,
}

impl Lower<'_> {
    fn note(
        &mut self,
        kind: &'static str,
        fidelity: Fidelity,
        source: &Ref,
        elements: u64,
        reasons: &[&'static str],
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        conversion::note(
            &mut self.report,
            "P21",
            kind,
            fidelity,
            source,
            elements,
            reasons,
            cx,
        )
    }

    #[allow(clippy::too_many_arguments)] // 点・長さの写し方を同じ借用で渡す。
    fn add<P: Copy>(
        &mut self,
        element: &Element<P>,
        point: &impl Fn(P) -> Option<PaperPoint>,
        length: &impl Fn(Length) -> Option<Length>,
        scale: f64,
        paper: bool,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        let kind = transfer::kind(&element.shape);
        let Some(mut shape) = transfer::basic(&element.shape, point, length, cx)? else {
            return self.note(kind, Fidelity::Dropped, &element.source, 0, &[], cx);
        };
        cx.check(
            Kind::DecodedBytes,
            1024 + element.source.id().map_or(0, |id| id.len() as u64),
        )?;
        let mut appearance = transfer::appearance(self.drawing, element, scale, cx)?;
        if let Shape::Text(text) = &mut shape {
            // 字形の実測値はなく、SXF の枠の高さとして数値を使う。字幅と間隔も apply が見積もる。
            text.height.meaning = HeightMeaning::Frame;
            if let zumen_drawing::Placement::Anchor { vertical, .. } = &mut text.placement
                && *vertical == zumen_drawing::Vertical::Baseline
            {
                *vertical = zumen_drawing::Vertical::Bottom;
            }
            appearance.reasons.push("text_height_and_width");
            cx.check(
                Kind::Work,
                (self.drawing.text_styles.len() as u64).saturating_mul(text.style.len() as u64 + 1),
            )?;
            if let Some(font) = self
                .drawing
                .text_style(&text.style)
                .and_then(|style| style.definition.as_ref().and_then(|def| def.font.as_ref()))
            {
                cx.check(Kind::DecodedBytes, font.name.len() as u64)?;
                text.style.clone_from(&font.name);
            }
        }
        if let Shape::Point(dot) = &mut shape
            && dot.look == PointLook::Document
            && !dot.construction
        {
            dot.look = PointLook::Marker {
                key: MarkerKey {
                    namespace: "sxf.marker".into(),
                    code: "3".into(),
                },
                scale: 1.0,
            };
            appearance.reasons.push("point_appearance");
        }
        if !paper {
            appearance.reasons.push("model_to_paper");
        }
        let layer = self.layers.get(&element.layer).copied().unwrap_or(1);
        if !self.layers.contains_key(&element.layer) {
            appearance.reasons.push("layer_missing");
        }
        let fidelity = if appearance
            .reasons
            .iter()
            .any(|r| *r != "text_height_and_width")
        {
            Fidelity::Drawn
        } else if appearance.reasons.is_empty() {
            Fidelity::Equivalent
        } else {
            Fidelity::Approximated
        };
        self.items.push(Item {
            element: Element {
                source: Ref::default(),
                layer: LayerId(layer),
                style: element.style,
                shape,
            },
            kind,
            source: element.source.clone(),
            appearance,
            fidelity,
        });
        Ok(())
    }
}

/// L2 の基本図形を P21 に書く。実変換と見積もりは同じ処理で、limit は明細だけを制限する。
/// 最初の用紙を使い、各空間はその用紙の最初の等方・軸平行で切り抜きのないビューで配置する。
/// ビューがなければ既知の単位を mm に直して 1:1、単位不明は数値を使い、その変更を診断する。
pub fn lower_to_p21(drawing: &Drawing2D, limit: u64, cx: &mut Cx<'_>) -> Result<Lowered, Error> {
    let result = lower(drawing, limit, cx);
    if let Err(Error::Stopped(stop)) = &result {
        cx.report(stop.to_diagnostic());
    }
    if matches!(result, Err(Error::Invalid)) {
        cx.report(Diagnostic::new(codes::INTERNAL_PANIC_CAUGHT));
    }
    result
}

fn lower(drawing: &Drawing2D, limit: u64, cx: &mut Cx<'_>) -> Result<Lowered, Error> {
    let mut lower = Lower {
        drawing,
        items: Vec::new(),
        dimensions: Vec::new(),
        layers: BTreeMap::new(),
        report: Report::new(limit),
    };
    for (i, layer) in drawing.layers.iter().enumerate() {
        cx.check(Kind::Work, 64 + layer.name.len() as u64)?;
        cx.check(
            Kind::DecodedBytes,
            256 + (layer.name.len() as u64).saturating_mul(8),
        )?;
        lower
            .layers
            .entry(layer.id)
            .or_insert(u32::try_from(i + 1).map_err(|_| Error::Invalid)?);
        lower.note(
            "layer",
            Fidelity::Drawn,
            &layer.source,
            0,
            &["layer_style_resolved"],
            cx,
        )?;
    }
    let sheet = drawing.sheets.first();
    let dimension_formats = dimension::formats(drawing, cx)?;
    for space in &drawing.spaces {
        cx.check(Kind::Work, 1 + sheet.map_or(0, |s| s.views.len() as u64))?;
        let view = sheet.and_then(|s| {
            s.views.iter().find(|view| {
                let [a, b, c, d, e, f] = view.transform.m;
                view.space == space.id
                    && view.clip.is_empty()
                    && view.hidden_layers.is_empty()
                    && [a, b, c, d, e, f].iter().all(|v| v.is_finite())
                    && a > 0.0
                    && a == e
                    && b == 0.0
                    && d == 0.0
            })
        });
        let (scale, dx, dy) = view.map_or(
            (space.unit.millimeters().unwrap_or(1.0), 0.0, 0.0),
            |view| {
                (
                    view.transform.m[0],
                    view.transform.m[2],
                    view.transform.m[5],
                )
            },
        );
        if view.is_none() && space.unit.millimeters().is_none() {
            lower.note(
                "space",
                Fidelity::Drawn,
                &Ref::default(),
                0,
                &["unit_unspecified"],
                cx,
            )?;
        }
        let (order, changed) = transfer::order(space.elements.len(), &space.order, cx)?;
        let dimension_view = sheet.and_then(|s| s.views.iter().find(|v| v.space == space.id));
        let dimension_projection = space.unit.millimeters().and_then(|mm| {
            dimension::Projection::model(space.id, mm, dimension_view)
                .map(|projection| (mm, projection))
        });
        if changed {
            lower.note(
                "draw_order",
                Fidelity::Drawn,
                &Ref::default(),
                0,
                &["invalid_order"],
                cx,
            )?;
        }
        for index in order {
            let element = space.elements.get(index).ok_or(Error::Invalid)?;
            if space.axes != Axes::EastNorth {
                lower.note(
                    transfer::kind(&element.shape),
                    Fidelity::Dropped,
                    &element.source,
                    0,
                    &[],
                    cx,
                )?;
                continue;
            }
            if let Shape::Dimension(dim) = &element.shape {
                if let Some((mm, projection)) = dimension_projection {
                    let at = zumen_drawing::Position::Model {
                        space: space.id,
                        index,
                    };
                    lower.dimension(
                        element,
                        dim,
                        |p| (p.space == space.id).then_some([p.x * mm, p.y * mm]),
                        projection,
                        dimension_formats.get(&at).copied().unwrap_or_default(),
                        cx,
                    )?;
                } else {
                    lower.note("dimension", Fidelity::Dropped, &element.source, 0, &[], cx)?;
                }
                continue;
            }
            lower.add(
                element,
                &|p: ModelPoint| {
                    (p.space == space.id).then_some(PaperPoint {
                        sheet: SheetId(0),
                        x: p.x * scale + dx,
                        y: p.y * scale + dy,
                    })
                },
                &|v| match v {
                    Length::Model(v) if v.space == space.id => {
                        Some(Length::Paper(PaperLength(v.value * scale)))
                    }
                    Length::Paper(v) => Some(Length::Paper(v)),
                    _ => None,
                },
                scale,
                false,
                cx,
            )?;
        }
    }
    for (i, sheet) in drawing.sheets.iter().enumerate() {
        lower.note(
            "sheet",
            if i == 0 {
                Fidelity::Drawn
            } else {
                Fidelity::Dropped
            },
            &sheet.source,
            0,
            if i == 0 {
                &["single_paper", "draw_order"]
            } else {
                &[]
            },
            cx,
        )?;
        for view in &sheet.views {
            lower.note("view", Fidelity::Dropped, &view.source, 0, &[], cx)?;
        }
        for (index, element) in sheet.elements.iter().enumerate() {
            if i == 0 {
                if let Shape::Dimension(dim) = &element.shape {
                    let projection = dimension::Projection::paper();
                    let at = zumen_drawing::Position::Paper {
                        sheet: sheet.id,
                        index,
                    };
                    lower.dimension(
                        element,
                        dim,
                        |p| (p.sheet == sheet.id).then_some([p.x, p.y]),
                        projection,
                        dimension_formats.get(&at).copied().unwrap_or_default(),
                        cx,
                    )?;
                    continue;
                }
                lower.add(
                    element,
                    &|p: PaperPoint| {
                        (p.sheet == sheet.id).then_some(PaperPoint {
                            sheet: SheetId(0),
                            ..p
                        })
                    },
                    &|v| matches!(v, Length::Paper(_)).then_some(v),
                    1.0,
                    true,
                    cx,
                )?;
            } else {
                lower.note(
                    transfer::kind(&element.shape),
                    Fidelity::Dropped,
                    &element.source,
                    0,
                    &[],
                    cx,
                )?;
            }
        }
    }
    for source in drawing
        .symbols
        .iter()
        .map(|s| ("symbol_definition", &s.source))
        .chain(
            drawing
                .title_blocks
                .iter()
                .map(|s| ("title_block", &s.source)),
        )
        .chain(drawing.extensions.iter().map(|s| ("extension", &s.source)))
        .chain(
            drawing
                .text_styles
                .iter()
                .map(|s| ("text_style", &s.source)),
        )
        .chain(
            drawing
                .dimension_styles
                .iter()
                .map(|s| ("dimension_style", &s.source)),
        )
        .chain(
            drawing
                .line_types
                .iter()
                .map(|s| ("line_type_definition", &s.source)),
        )
    {
        lower.note(source.0, Fidelity::Dropped, source.1, 0, &[], cx)?;
    }
    drawing.each_dimension_display_source(cx, |kind, source, cx| {
        conversion::note(
            &mut lower.report,
            "P21",
            kind,
            Fidelity::Dropped,
            source,
            0,
            &[],
            cx,
        )
    })?;
    if drawing.point_style.is_some() {
        lower.note(
            "point_style",
            Fidelity::Dropped,
            &Ref::default(),
            0,
            &[],
            cx,
        )?;
    }
    if drawing.georeference.is_some() {
        lower.note(
            "georeference",
            Fidelity::Dropped,
            &Ref::default(),
            0,
            &[],
            cx,
        )?;
    }
    let mut file = template::make(
        &mut lower.items,
        &lower.dimensions,
        &drawing.layers,
        sheet,
        cx,
    )?;
    let seed = quiet(cx, |cx| file.lift(cx))?;
    let seed_sheet = seed.sheets.first().ok_or(Error::Invalid)?;
    cx.check(
        Kind::DecodedBytes,
        (seed_sheet.elements.len() as u64).saturating_mul(512),
    )?;
    let removals = seed_sheet
        .elements
        .iter()
        .enumerate()
        .filter(|(_, element)| matches!(element.shape, Shape::Circle { .. }))
        .map(|(i, element)| {
            Ok(Edit::Remove {
                target: Seen {
                    id: format!("sheet/0/{}", id::origin(&element.source, i)),
                    hash: Hash::element_checked(element, cx)?,
                },
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    for item in &mut lower.items {
        let point_or_text = matches!(item.element.shape, Shape::Text(_) | Shape::Point(_));
        item.element.style.color = Inherit::Own(Color {
            code: None,
            rgb: Some(item.appearance.rgb),
        });
        item.element.style.line_weight = (!point_or_text).then_some(Inherit::Own(
            LineWeight::Width(PaperLength(item.appearance.width)),
        ));
        if point_or_text {
            item.element.style.line_type = Inherit::Layer;
        }
        item.element.style.line_type_scale = 1.0;
        item.element.style.transparency = Inherit::Layer;
    }
    let accepted = conversion::batches(
        lower.items.len(),
        |range, cx| {
            let mut edits = Vec::new();
            for item in lower.items.get(range).ok_or(Error::Invalid)? {
                let shape = transfer::basic(&item.element.shape, &Some, &Some, cx)?
                    .ok_or(Error::Invalid)?;
                cx.check(Kind::DecodedBytes, 512)?;
                edits.push(Edit::Add {
                    place: "sheet/0".into(),
                    element: Box::new(Placed::Paper(Element {
                        source: Ref::default(),
                        layer: item.element.layer,
                        style: item.element.style,
                        shape,
                    })),
                });
            }
            file.apply(
                &Changes {
                    edits,
                    document: None,
                },
                cx,
            )
            .map(|_| ())
        },
        cx,
    )?;
    let mut elements = lower.dimensions.len() as u64;
    for (item, accepted) in lower.items.iter().zip(accepted) {
        if accepted {
            elements += 1;
        }
        conversion::note(
            &mut lower.report,
            "P21",
            item.kind,
            if accepted {
                item.fidelity
            } else {
                Fidelity::Dropped
            },
            &item.source,
            u64::from(accepted),
            if accepted {
                &item.appearance.reasons
            } else {
                &[]
            },
            cx,
        )?;
    }
    quiet(cx, |cx| {
        file.apply(
            &Changes {
                edits: removals,
                document: None,
            },
            cx,
        )
    })
    .map_err(|error| {
        if matches!(error, Error::Rejected) {
            Error::Invalid
        } else {
            error
        }
    })?;
    let mut bytes = Vec::new();
    quiet(cx, |cx| Sxf::write(&file, &mut bytes, cx))?;
    Ok(Lowered {
        file,
        bytes,
        elements,
        report: lower.report.finish(cx)?,
    })
}

// 内部の再読・試行の診断を捨て、停止理由は公開入口で一度だけ返す。
fn quiet<T>(
    cx: &mut Cx<'_>,
    run: impl FnOnce(&mut Cx<'_>) -> Result<T, Error>,
) -> Result<T, Error> {
    let mut diagnostics = zumen_base::Diagnostics::new();
    run(&mut Cx::new(cx.budget, &mut diagnostics, cx.cancel))
}
