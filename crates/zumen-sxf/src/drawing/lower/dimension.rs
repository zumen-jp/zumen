//! SXF Ver.3.1 STEP AP202 サブセット編「直線寸法」「寸法補助線」「部分図」に従って、
//! 水平な寸法を実寸の定義点と用紙上の配置に分けて書く。
use super::template::{Writer, quoted, real};
use super::{Fidelity, Lower};
use std::collections::BTreeMap;
use zumen_base::{Cx, Error, Kind};
use zumen_drawing::{
    Dimension, DimensionDisplayIndex, DimensionKind, Drawing2D, Element, Label, Position,
    ResolvedDimensionDisplay, Shape, SpaceId, View, id, transfer,
};

// 要素ごとの表示倍率と精度は、変更していない要素にだけ元の個別指定を当てる。
pub(super) fn formats<'a>(
    drawing: &'a Drawing2D,
    cx: &mut Cx<'_>,
) -> Result<BTreeMap<Position, ResolvedDimensionDisplay<'a>>, Error> {
    let mut result = BTreeMap::new();
    if drawing.dimension_display.is_empty() {
        return Ok(result);
    }
    // 寸法スタイルを持つだけの大きな DXF で全要素の ID を作らない。
    cx.check(
        Kind::Work,
        drawing.spaces.len() as u64 + drawing.sheets.len() as u64,
    )?;
    let elements = drawing
        .spaces
        .iter()
        .map(|s| s.elements.len())
        .chain(drawing.sheets.iter().map(|s| s.elements.len()))
        .fold(0u64, |n, count| n.saturating_add(count as u64));
    cx.check(Kind::Work, elements)?;
    let uses_measurement = |dim: &Label| matches!(dim, Label::Measurement);
    if !drawing
        .spaces
        .iter()
        .flat_map(|s| &s.elements)
        .any(|e| matches!(&e.shape, Shape::Dimension(d) if uses_measurement(&d.label)))
        && !drawing
            .sheets
            .iter()
            .flat_map(|s| &s.elements)
            .any(|e| matches!(&e.shape, Shape::Dimension(d) if uses_measurement(&d.label)))
    {
        return Ok(result);
    }
    let index = DimensionDisplayIndex::new(&drawing.dimension_display, cx)?;
    id::each_element(drawing, |at, name, item| {
        cx.check(Kind::Work, 1)?;
        cx.check(Kind::DecodedBytes, 96 + name.len() as u64)?;
        let style = match item {
            id::Found::Model(e) => match &e.shape {
                Shape::Dimension(d) => Some(&d.style),
                _ => None,
            },
            id::Found::Paper(e) => match &e.shape {
                Shape::Dimension(d) => Some(&d.style),
                _ => None,
            },
        };
        if let Some(style) = style
            && let Some(settings) = index.resolve(style, &name, |cx| item.hash_checked(cx), cx)?
        {
            result.insert(at, settings);
        }
        Ok::<_, Error>(())
    })?;
    Ok(result)
}

#[derive(Clone, Copy)]
pub(super) struct Projection {
    space: Option<SpaceId>,
    scale: f64,
    offset: [f64; 2],
    clip: Option<[f64; 4]>,
    mm: f64,
}

impl Projection {
    pub(super) fn paper() -> Self {
        Self {
            space: None,
            scale: 1.0,
            offset: [0.0; 2],
            clip: None,
            mm: 1.0,
        }
    }

    /// ビューがある場合、軸平行の矩形に収まる寸法だけを、元の縮尺で部分図に置く。
    pub(super) fn model(space: SpaceId, mm: f64, view: Option<&View>) -> Option<Self> {
        let mut result = Self {
            space: Some(space),
            mm,
            ..Self::paper()
        };
        if let Some(view) = view {
            let [a, b, dx, d, e, dy] = view.transform.m;
            if ![a, b, dx, d, e, dy].into_iter().all(f64::is_finite)
                || !view.hidden_layers.is_empty()
                || a <= 0.0
                || a != e
                || b != 0.0
                || d != 0.0
            {
                return None;
            }
            result.scale = a / mm;
            result.offset = [dx, dy];
            result.clip = match &view.clip[..] {
                [] => None,
                [p, q, r, s]
                    if [p, q, r, s].iter().all(|p| p.sheet == view.sheet)
                        && p.y == q.y
                        && q.x == r.x
                        && r.y == s.y
                        && s.x == p.x
                        && p.x < r.x
                        && p.y < r.y =>
                {
                    Some([p.x, p.y, r.x, r.y])
                }
                _ => return None,
            };
        }
        Some(result)
    }

    fn contains(self, [x, y]: [f64; 2]) -> bool {
        let x = x * self.scale + self.offset[0];
        let y = y * self.scale + self.offset[1];
        x.is_finite()
            && y.is_finite()
            && self.clip.is_none_or(|[left, bottom, right, top]| {
                x >= left && x <= right && y >= bottom && y <= top
            })
    }
}

pub(super) struct Linear {
    projection: Projection,
    layer: usize,
    pub rgb: [u8; 3],
    pub width: f64,
    first: [f64; 2],
    second: [f64; 2],
    line_y: f64,
    label: Option<String>,
    text_at: [f64; 2],
    height: f64,
}

impl Lower<'_> {
    pub(super) fn dimension<P: Copy>(
        &mut self,
        element: &Element<P>,
        dimension: &Dimension<P>,
        point: impl Fn(P) -> Option<[f64; 2]>,
        projection: Projection,
        format: ResolvedDimensionDisplay<'_>,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        let source = &element.source;
        let lost = |this: &mut Self, cx: &mut Cx<'_>| {
            this.note("dimension", Fidelity::Dropped, source, 0, &[], cx)
        };
        let DimensionKind::Linear {
            first,
            second,
            line,
            rotation,
            oblique,
        } = dimension.kind
        else {
            return lost(self, cx);
        };
        let (Some(first), Some(second), Some(line)) = (point(first), point(second), point(line))
        else {
            return lost(self, cx);
        };
        let distance = (second[0] - first[0]).abs();
        if !projection.scale.is_finite()
            || projection.scale <= 0.0
            || rotation.rem_euclid(360.0) != 0.0
            || oblique != 0.0
            || first[1] != second[1]
            || first[1] == line[1]
            || !(line[1] - first[1]).is_finite()
            || !distance.is_finite()
            || distance == 0.0
            || !first
                .into_iter()
                .chain(second)
                .chain(line)
                .all(f64::is_finite)
            || dimension
                .text
                .rotation
                .is_some_and(|r| r.rem_euclid(360.0) != 0.0)
        {
            return lost(self, cx);
        }
        let mut appearance =
            transfer::appearance(self.drawing, element, projection.scale * projection.mm, cx)?;
        appearance.reasons.push("dimension_appearance");
        appearance.reasons.push("dimension_terminators");
        if !appearance.dashes.is_empty() {
            appearance.reasons.push("line_type");
        }
        if dimension.measurement.is_some() {
            appearance.reasons.push("stored_measurement");
        }
        let label = match &dimension.label {
            Label::Hidden => None,
            Label::Measurement => {
                appearance.reasons.push("dimension_label_fixed");
                let factor = zumen_drawing::parts::dimension_linear_factor(
                    format.linear_factor.unwrap_or(1.0),
                    projection.space.is_none(),
                );
                let value = dimension
                    .measurement
                    .as_ref()
                    .map_or_else(|| distance / projection.mm * factor, |m| m.value);
                if !value.is_finite() {
                    return lost(self, cx);
                }
                Some(zumen_drawing::parts::format_measurement(
                    value,
                    usize::from(format.linear_precision.unwrap_or(8)),
                ))
            }
            Label::Override { content, spans } => {
                cx.check(Kind::Work, content.len() as u64 + spans.len() as u64)?;
                if !matches!(&spans[..], [span] if matches!(&span.piece, zumen_drawing::Piece::Text(s) if s==content))
                    || content.is_empty()
                    || content.chars().any(char::is_control)
                {
                    return lost(self, cx);
                }
                cx.check(Kind::DecodedBytes, content.len() as u64)?;
                Some(content.clone())
            }
        };
        let height = 3.5 / projection.scale;
        let text_at = match dimension.text.position {
            Some(p) => {
                let Some(p) = point(p) else {
                    return lost(self, cx);
                };
                p
            }
            None => [first[0] * 0.5 + second[0] * 0.5, line[1] + height],
        };
        let text_inside = label.as_ref().is_none_or(|text| {
            let half_width = text.chars().count() as f64 * height * 0.4;
            let half_height = height * 0.5;
            projection.contains([text_at[0] - half_width, text_at[1] - half_height])
                && projection.contains([text_at[0] + half_width, text_at[1] + half_height])
        });
        if ![
            first,
            second,
            [first[0], line[1]],
            [second[0], line[1]],
            text_at,
        ]
        .into_iter()
        .all(|p| projection.contains(p))
            || !(height.is_finite() && height > 0.0)
            || !text_inside
        {
            return lost(self, cx);
        }
        let layer = self.layers.get(&element.layer).copied().unwrap_or(1) as usize;
        if !self.layers.contains_key(&element.layer) {
            appearance.reasons.push("layer_missing");
        }
        self.note(
            "dimension",
            Fidelity::Approximated,
            source,
            1,
            &appearance.reasons,
            cx,
        )?;
        for end in ["first", "second"] {
            self.note(
                "dimension_terminator",
                Fidelity::Dropped,
                source,
                0,
                &[end],
                cx,
            )?;
        }
        cx.check(Kind::DecodedBytes, 256)?;
        self.dimensions.push(Linear {
            projection,
            layer,
            rgb: appearance.rgb,
            width: appearance.width,
            first,
            second,
            line_y: line[1],
            label,
            text_at,
            height,
        });
        Ok(())
    }
}

impl Writer {
    fn xy(&mut self, [x, y]: [f64; 2]) -> Result<u64, Error> {
        Ok(self.add(format!("CARTESIAN_POINT('',({},{}))", real(x)?, real(y)?)))
    }

    fn dimension_curve(
        &mut self,
        projection: Option<&str>,
        from: [f64; 2],
        to: [f64; 2],
        style: u64,
    ) -> Result<u64, Error> {
        let end = self.xy(to)?;
        let start = self.xy(from)?;
        let direction = self.add(format!(
            "DIRECTION(' ',({},{}))",
            real(to[0] - from[0])?,
            real(to[1] - from[1])?
        ));
        let vector = self.add(format!("VECTOR(' ',#{direction},1.)"));
        // 補助線の定義点と、実際に描く線の始点を独立して参照できるようにする。
        let base = if projection.is_some() {
            self.xy(from)?
        } else {
            start
        };
        let line = self.add(format!("LINE(' ',#{base},#{vector})"));
        let trimmed = self.add(format!(
            "TRIMMED_CURVE('',#{line},(#{start}),(#{end}),.T.,.CARTESIAN.)"
        ));
        let types = if projection.is_none() {
            "DIMENSION_CURVE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM()"
        } else {
            "DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() PROJECTION_CURVE()"
        };
        let name = projection.unwrap_or(" ");
        Ok(self.add(format!("(ANNOTATION_CURVE_OCCURRENCE() ANNOTATION_OCCURRENCE() {types} REPRESENTATION_ITEM('{name}') STYLED_ITEM((#{style}),#{trimmed}))")))
    }

    fn dimension_text(
        &mut self,
        dim: &Linear,
        label: &str,
        color: u64,
        font: u64,
        direction: u64,
    ) -> Result<u64, Error> {
        // SXF の middleline は文字の中央より高さの半分だけ下に配置軸を置く。
        let center = self.xy([dim.text_at[0], dim.text_at[1] - dim.height * 0.5])?;
        let placement = self.add(format!("AXIS2_PLACEMENT_2D(' ',#{center},#{direction})"));
        let width = dim.height * label.chars().count() as f64 * 0.8;
        let extent = self.add(format!(
            "PLANAR_EXTENT('',{},{})",
            real(width)?,
            real(dim.height)?
        ));
        let content = quoted(label);
        let value = self.add(format!(
            "TEXT_LITERAL_WITH_EXTENT('$$SXF_middleline centre',{content},#{placement},'baseline centre',.RIGHT.,#{font},#{extent})"
        ));
        let appearance = self.add(format!("TEXT_STYLE_FOR_DEFINED_FONT(#{color})"));
        let style = self.add(format!("(TEXT_STYLE(' ',#{appearance}) TEXT_STYLE_WITH_BOX_CHARACTERISTICS((BOX_HEIGHT({}),BOX_WIDTH({}),BOX_SLANT_ANGLE(0.),BOX_ROTATE_ANGLE(0.))) \
            TEXT_STYLE_WITH_SPACING(LENGTH_MEASURE(0.)))", real(dim.height)?, real(dim.height * 0.8)?));
        let assignment = self.add(format!("PRESENTATION_STYLE_ASSIGNMENT((#{style}))"));
        Ok(self.add(format!(
            "(ANNOTATION_OCCURRENCE() ANNOTATION_TEXT_OCCURRENCE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM('dimension_value') STYLED_ITEM((#{assignment}),#{value}))"
        )))
    }
}

#[allow(clippy::too_many_arguments)] // 用紙と、テンプレートの長さ・色の参照を共有する。
pub(super) fn write(
    w: &mut Writer,
    dims: &[Linear],
    colors: &BTreeMap<[u8; 3], u64>,
    widths: &BTreeMap<u64, u64>,
    continuous: u64,
    origin: u64,
    unit: u64,
    sheet: &mut Vec<String>,
    cx: &mut Cx<'_>,
) -> Result<BTreeMap<usize, Vec<u64>>, Error> {
    let mut layers = BTreeMap::<usize, Vec<u64>>::new();
    if dims.is_empty() {
        return Ok(layers);
    }
    let external = w.add("EXTERNAL_SOURCE(IDENTIFIER('scadec'))".into());
    let font = w.add(format!(
        "EXTERNALLY_DEFINED_TEXT_FONT(IDENTIFIER('MS Gothic'),#{external})"
    ));
    let null_style = w.add("PRESENTATION_STYLE_ASSIGNMENT((NULL_STYLE(.NULL.)))".into());
    let context = w.context(unit);
    let direction = w.add("DIRECTION(' ',(1.,0.))".into());
    let mut groups = BTreeMap::<(SpaceId, usize), (Projection, Vec<u64>)>::new();
    for dim in dims {
        let n = dim.label.as_ref().map_or(0, |s| s.len() as u64);
        cx.check(Kind::Work, 128 + n)?;
        cx.check(Kind::DecodedBytes, 8192 + n.saturating_mul(10))?;
        let color = colors.get(&dim.rgb).ok_or(Error::Invalid)?;
        let width = widths.get(&dim.width.to_bits()).ok_or(Error::Invalid)?;
        let curve_style = w.add(format!("CURVE_STYLE('',#{continuous},#{width},#{color})"));
        let style = w.add(format!("PRESENTATION_STYLE_ASSIGNMENT((#{curve_style}))"));
        let start = [dim.first[0], dim.line_y];
        let end = [dim.second[0], dim.line_y];
        let line = w.dimension_curve(None, start, end, style)?;
        let a = w.dimension_curve(Some("$$SXF_prj_1"), dim.first, start, style)?;
        let b = w.dimension_curve(Some("$$SXF_prj_2"), dim.second, end, style)?;
        let text = dim
            .label
            .as_deref()
            .map(|label| w.dimension_text(dim, label, *color, font, direction))
            .transpose()?;
        let mut pieces = format!("#{line},#{a},#{b}");
        if let Some(t) = text {
            pieces.push_str(&format!(",#{t}"));
        }
        let structured = text.map(|t| w.add(format!("(DRAUGHTING_CALLOUT((#{t})) DRAUGHTING_ELEMENTS() \
            GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM(' ') STRUCTURED_DIMENSION_CALLOUT())")));
        let callout = w.add(format!("(DIMENSION_CURVE_DIRECTED_CALLOUT() DRAUGHTING_CALLOUT(({pieces})) \
            DRAUGHTING_ELEMENTS() GEOMETRIC_REPRESENTATION_ITEM() LINEAR_DIMENSION() REPRESENTATION_ITEM(' '))"));
        if let Some(structured) = structured {
            w.add(format!(
                "DIMENSION_CALLOUT_RELATIONSHIP('primary',' ',#{callout},#{structured})"
            ));
        }
        let members = layers.entry(dim.layer).or_default();
        members.push(line);
        members.extend(text);
        if let Some(space) = dim.projection.space {
            let group = &mut groups
                .entry((space, dim.layer))
                .or_insert_with(|| (dim.projection, Vec::new()))
                .1;
            group.extend(structured);
            group.push(callout);
        } else {
            sheet.extend(structured.map(|id| format!("#{id}")));
            sheet.push(format!("#{callout}"));
        }
    }
    for ((space, layer), (projection, items)) in groups {
        let name = format!("$$SXF_FM_zumen_dim_{}_{layer}", space.0);
        let members = items
            .iter()
            .map(|id| format!("#{id}"))
            .collect::<Vec<_>>()
            .join(",");
        let definition = w.add(format!(
            "DRAUGHTING_SUBFIGURE_REPRESENTATION('{name}',({members},#{origin}),#{context})"
        ));
        let map = w.add(format!(
            "SYMBOL_REPRESENTATION_MAP(#{origin},#{definition})"
        ));
        let offset = w.xy(projection.offset)?;
        let placement = w.add(format!("AXIS2_PLACEMENT_2D(' ',#{offset},#{direction})"));
        let scale = real(projection.scale)?;
        let target = w.add(format!("SYMBOL_TARGET('',#{placement},{scale},{scale})"));
        let mapped=w.add(format!("(ANNOTATION_SYMBOL() GEOMETRIC_REPRESENTATION_ITEM() MAPPED_ITEM(#{map},#{target}) REPRESENTATION_ITEM(' '))"));
        let instance = w.add(format!("(ANNOTATION_OCCURRENCE() ANNOTATION_SUBFIGURE_OCCURRENCE() \
            ANNOTATION_SYMBOL_OCCURRENCE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() \
            REPRESENTATION_ITEM('{name}') STYLED_ITEM((#{null_style}),#{mapped}))"));
        // 部分図の配置は専用レイヤ、寸法の線と文字は実際のレイヤに属する。
        layers.entry(0).or_default().push(instance);
        sheet.push(format!("#{instance}"));
    }
    Ok(layers)
}
