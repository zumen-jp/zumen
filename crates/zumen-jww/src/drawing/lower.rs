//! 新しい JWW 700 に基本図形を書く（#360）。
//! 実寸を mm にそろえ、空間ごとに 16 レイヤ単位のグループを確保する。
//! 文字・レイヤと図形の符号化は既存の apply を通し、仮の線は公開前に除く。

use crate::{Jww, JwwFile};
use std::collections::{BTreeMap, BTreeSet};
use zumen_base::{Cx, Diagnostic, Error, Kind, Ref, codes};
use zumen_drawing::{
    Axes, Changes, Drawing2D, Edit, Element, Hash, HeightMeaning, LayerId, Length, ModelLength,
    ModelPoint, PaperPoint, Placed, Seen, Shape, SpaceId, Vertical, id, transfer,
};
use zumen_sdk::conversion::{self, ConversionFidelity as Fidelity, ConversionReport, Report};
use zumen_sdk::{Apply, Codec, Lift};

mod template;

/// 新しい JWW と、書いた要素数・忠実度。
pub struct Lowered {
    /// ファイルモデル。
    pub file: JwwFile,
    /// JWW 700 のバイト列。
    pub bytes: Vec<u8>,
    /// 出力した図形の数。折れ線は線分ごとに数える。
    pub elements: u64,
    /// 上限付きの見積もり。
    pub report: ConversionReport,
}

struct Item {
    elements: Vec<Element<ModelPoint>>,
    appearance: transfer::Appearance,
    kind: &'static str,
    source: Ref,
    fidelity: Fidelity,
}
struct Slots {
    layers: Vec<Option<LayerId>>,
    map: BTreeMap<(usize, LayerId), u32>,
}

impl Slots {
    fn add<P>(
        &mut self,
        source: usize,
        elements: &[Element<P>],
        drawing: &Drawing2D,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        cx.check(
            Kind::Work,
            (elements.len() as u64)
                .saturating_mul(64)
                .saturating_add(drawing.layers.len() as u64 * 64),
        )?;
        cx.check(
            Kind::DecodedBytes,
            (elements.len() as u64).saturating_mul(128),
        )?;
        let mut used: BTreeSet<_> = elements.iter().map(|e| e.layer).collect();
        let mut ordered = Vec::new();
        for layer in &drawing.layers {
            if used.remove(&layer.id) {
                ordered.push(layer.id);
            }
        }
        ordered.extend(used);
        if ordered.is_empty() {
            return Ok(());
        }
        let start = self.layers.len().div_ceil(16) * 16;
        self.layers.resize(start, None);
        for layer in ordered {
            if self.layers.len() >= 256 {
                break;
            }
            let n = self.layers.len() as u32;
            self.map.insert((source, layer), n + 1);
            self.layers.push(Some(layer));
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)] // 点・長さの写し方と所属を同時に検証する。
fn add<P: Copy>(
    items: &mut Vec<Item>,
    drawing: &Drawing2D,
    element: &Element<P>,
    slot: Option<u32>,
    point: &impl Fn(P, SpaceId) -> Option<ModelPoint>,
    length: &impl Fn(Length, SpaceId) -> Option<Length>,
    scale: f64,
    paper: bool,
    cx: &mut Cx<'_>,
) -> Result<bool, Error> {
    let Some(slot) = slot else {
        return Ok(false);
    };
    let space = SpaceId((slot - 1) / 16);
    let Some(shape) = transfer::basic(
        &element.shape,
        &|p| point(p, space),
        &|l| length(l, space),
        cx,
    )?
    else {
        return Ok(false);
    };
    cx.check(
        Kind::DecodedBytes,
        2048 + element.source.id().map_or(0, |id| id.len() as u64),
    )?;
    let mut appearance = transfer::appearance(drawing, element, scale, cx)?;
    let width = libm::round(appearance.width * 100.0).max(1.0) / 100.0;
    if !width.is_finite() || width > 655.35 {
        return Ok(false);
    }
    if width != appearance.width {
        appearance.reasons.push("line_weight_grid");
    }
    appearance.width = width;
    let mut shapes = Vec::new();
    match shape {
        Shape::Polyline(poly) => {
            if poly.width.is_some()
                || !poly.vertex_widths.is_empty()
                || poly.vertices.len() < 2
                || poly.vertices.iter().any(|v| v.bulge != 0.0)
            {
                return Ok(false);
            }
            cx.check(Kind::Work, poly.vertices.len() as u64)?;
            cx.check(Kind::DecodedBytes, poly.vertices.len() as u64 * 512)?;
            for pair in poly.vertices.windows(2) {
                shapes.push(Shape::Line {
                    start: pair.first().ok_or(Error::Invalid)?.point,
                    end: pair.get(1).ok_or(Error::Invalid)?.point,
                });
            }
            if poly.closed {
                shapes.push(Shape::Line {
                    start: poly.vertices.last().ok_or(Error::Invalid)?.point,
                    end: poly.vertices.first().ok_or(Error::Invalid)?.point,
                });
            }
            appearance.reasons.push("polyline_segments");
        }
        Shape::Text(mut text) => {
            text.height.meaning = HeightMeaning::Declared("jww.CDataMoji.height".into());
            if let zumen_drawing::Placement::Anchor { vertical, .. } = &mut text.placement
                && *vertical == Vertical::Baseline
            {
                *vertical = Vertical::Bottom;
            }
            appearance.reasons.push("text_height_and_width");
            cx.check(
                Kind::Work,
                (drawing.text_styles.len() as u64).saturating_mul(text.style.len() as u64 + 1),
            )?;
            if let Some(font) = drawing
                .text_style(&text.style)
                .and_then(|style| style.definition.as_ref().and_then(|def| def.font.as_ref()))
            {
                cx.check(Kind::DecodedBytes, font.name.len() as u64)?;
                text.style.clone_from(&font.name);
            }
            shapes.push(Shape::Text(text));
        }
        shape => shapes.push(shape),
    }
    if paper {
        appearance.reasons.push("paper_to_model");
    }
    let fidelity = if appearance
        .reasons
        .iter()
        .any(|r| !matches!(*r, "text_height_and_width" | "line_weight_grid"))
    {
        Fidelity::Drawn
    } else if appearance.reasons.is_empty() {
        Fidelity::Equivalent
    } else {
        Fidelity::Approximated
    };
    items.push(Item {
        elements: shapes
            .into_iter()
            .map(|shape| Element {
                source: Ref::default(),
                layer: LayerId(slot),
                style: element.style,
                shape,
            })
            .collect(),
        appearance,
        kind: transfer::kind(&element.shape),
        source: element.source.clone(),
        fidelity,
    });
    Ok(true)
}

/// L2 の基本図形を JWW 700 に書く。limit は見積もりの明細だけを制限する。
/// 空間の単位を mm に直し、グループの縮尺は 1:1。単位不明は数値を使って診断する。
/// 固定の 256 レイヤに入らない要素と、追加に対応しない形は出所付きで落とす。
pub fn lower_to_jww(drawing: &Drawing2D, limit: u64, cx: &mut Cx<'_>) -> Result<Lowered, Error> {
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
    let mut report = Report::new(limit);
    let mut slots = Slots {
        layers: Vec::new(),
        map: BTreeMap::new(),
    };
    let mut items = Vec::new();
    for (i, space) in drawing.spaces.iter().enumerate() {
        slots.add(i, &space.elements, drawing, cx)?;
        let scale = space.unit.millimeters().unwrap_or(1.0);
        if space.unit.millimeters().is_none() {
            conversion::note(
                &mut report,
                "JWW",
                "space",
                Fidelity::Drawn,
                &Ref::default(),
                0,
                &["unit_unspecified"],
                cx,
            )?;
        }
        let (order, changed) = transfer::order(space.elements.len(), &space.order, cx)?;
        if changed {
            conversion::note(
                &mut report,
                "JWW",
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
            let accepted = space.axes == Axes::EastNorth
                && add(
                    &mut items,
                    drawing,
                    element,
                    slots.map.get(&(i, element.layer)).copied(),
                    &|p: ModelPoint, dest| {
                        (p.space == space.id).then_some(ModelPoint {
                            space: dest,
                            x: p.x * scale,
                            y: p.y * scale,
                        })
                    },
                    &|v, dest| match v {
                        Length::Model(v) if v.space == space.id => {
                            Some(Length::Model(ModelLength {
                                space: dest,
                                value: v.value * scale,
                            }))
                        }
                        Length::Paper(v) => Some(Length::Model(ModelLength {
                            space: dest,
                            value: v.0,
                        })),
                        _ => None,
                    },
                    scale,
                    false,
                    cx,
                )?;
            if !accepted {
                conversion::note(
                    &mut report,
                    "JWW",
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
    let paper = template::paper(drawing.sheets.first());
    let paper_size = super::PAPERS
        .iter()
        .find(|(code, _)| *code == paper)
        .map(|(_, size)| *size)
        .ok_or(Error::Invalid)?;
    for (i, sheet) in drawing.sheets.iter().enumerate() {
        conversion::note(
            &mut report,
            "JWW",
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
            conversion::note(
                &mut report,
                "JWW",
                "view",
                Fidelity::Dropped,
                &view.source,
                0,
                &[],
                cx,
            )?;
        }
        if i == 0 {
            slots.add(drawing.spaces.len(), &sheet.elements, drawing, cx)?;
        }
        for element in &sheet.elements {
            let accepted = i == 0
                && add(
                    &mut items,
                    drawing,
                    element,
                    slots
                        .map
                        .get(&(drawing.spaces.len(), element.layer))
                        .copied(),
                    &|p: PaperPoint, dest| {
                        (p.sheet == sheet.id).then_some(ModelPoint {
                            space: dest,
                            x: p.x - paper_size[0] / 2.0,
                            y: p.y - paper_size[1] / 2.0,
                        })
                    },
                    &|v, dest| match v {
                        Length::Paper(v) => Some(Length::Model(ModelLength {
                            space: dest,
                            value: v.0,
                        })),
                        _ => None,
                    },
                    1.0,
                    true,
                    cx,
                )?;
            if !accepted {
                conversion::note(
                    &mut report,
                    "JWW",
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
    for layer in &drawing.layers {
        let kept = slots.layers.contains(&Some(layer.id));
        conversion::note(
            &mut report,
            "JWW",
            "layer",
            if kept {
                Fidelity::Drawn
            } else {
                Fidelity::Dropped
            },
            &layer.source,
            0,
            if kept { &["layer_style_resolved"] } else { &[] },
            cx,
        )?;
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
        conversion::note(
            &mut report,
            "JWW",
            source.0,
            Fidelity::Dropped,
            source.1,
            0,
            &[],
            cx,
        )?;
    }
    if drawing.point_style.is_some() {
        conversion::note(
            &mut report,
            "JWW",
            "point_style",
            Fidelity::Dropped,
            &Ref::default(),
            0,
            &[],
            cx,
        )?;
    }
    let mut file = template::make(&mut items, &slots, drawing, paper, cx)?;
    let seed = quiet(cx, |cx| file.lift(cx))?;
    let mut removals = Vec::new();
    for space in &seed.spaces {
        for (i, element) in space.elements.iter().enumerate() {
            cx.check(Kind::DecodedBytes, 512)?;
            removals.push(Edit::Remove {
                target: Seen {
                    id: format!("{}/{}", id::space(space.id), id::origin(&element.source, i)),
                    hash: Hash::element_checked(element, cx)?,
                },
            });
        }
    }
    let accepted = conversion::batches(
        items.len(),
        |range, cx| {
            let mut edits = Vec::new();
            for item in items.get(range).ok_or(Error::Invalid)? {
                for element in &item.elements {
                    let shape =
                        transfer::basic(&element.shape, &Some, &Some, cx)?.ok_or(Error::Invalid)?;
                    cx.check(Kind::DecodedBytes, 512)?;
                    let space = SpaceId((element.layer.0 - 1) / 16);
                    edits.push(Edit::Add {
                        place: id::space(space),
                        element: Box::new(Placed::Model(Element {
                            source: Ref::default(),
                            layer: element.layer,
                            style: element.style,
                            shape,
                        })),
                    });
                }
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
    let mut elements = 0;
    for (item, accepted) in items.iter().zip(accepted) {
        let count = if accepted {
            item.elements.len() as u64
        } else {
            0
        };
        elements += count;
        conversion::note(
            &mut report,
            "JWW",
            item.kind,
            if accepted {
                item.fidelity
            } else {
                Fidelity::Dropped
            },
            &item.source,
            count,
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
    quiet(cx, |cx| Jww::write(&file, &mut bytes, cx))?;
    Ok(Lowered {
        file,
        bytes,
        elements,
        report: report.finish(cx)?,
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
