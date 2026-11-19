// 生成したファイル。手で直さない。`cargo xtask codegen` が、図面モデルの型と schema/drawing.toml から作る（#559）。

use super::{ModelSize, largest};

impl ModelSize for crate::space::Affine {
    const SIZE: u64 = <[f64; 6] as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { m } = self;
        0_u64.saturating_add(m.heap_bytes())
    }
}

impl ModelSize for crate::style::Arrowhead {
    const SIZE: u64 = 8 + largest(&[0, <crate::symbol::SymbolKey as ModelSize>::SIZE]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Default => 0,
            Self::Symbol(f0) => 0_u64.saturating_add(f0.heap_bytes()),
        }
    }
}

impl<P: ModelSize> ModelSize for crate::symbol::Attribute<P> {
    const SIZE: u64 = <String as ModelSize>::SIZE
        + <crate::text::Text<P> as ModelSize>::SIZE
        + <Option<crate::text::FormattedText<P>> as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <crate::style::LayerId as ModelSize>::SIZE
        + <crate::style::Style as ModelSize>::SIZE
        + <zumen_base::Ref as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            tag,
            text,
            formatted,
            hidden,
            layer,
            style,
            source,
        } = self;
        0_u64
            .saturating_add(tag.heap_bytes())
            .saturating_add(text.heap_bytes())
            .saturating_add(formatted.heap_bytes())
            .saturating_add(hidden.heap_bytes())
            .saturating_add(layer.heap_bytes())
            .saturating_add(style.heap_bytes())
            .saturating_add(source.heap_bytes())
    }
}

impl ModelSize for crate::symbol::AttributeDefinition {
    const SIZE: u64 = <crate::symbol::Attribute<crate::space::ModelPoint> as ModelSize>::SIZE
        + <String as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            attribute,
            prompt,
            constant,
            verify,
            preset,
        } = self;
        0_u64
            .saturating_add(attribute.heap_bytes())
            .saturating_add(prompt.heap_bytes())
            .saturating_add(constant.heap_bytes())
            .saturating_add(verify.heap_bytes())
            .saturating_add(preset.heap_bytes())
    }
}

impl ModelSize for crate::callout::AttributeValue {
    const SIZE: u64 = <zumen_base::Ref as ModelSize>::SIZE + <String as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { definition, text } = self;
        0_u64
            .saturating_add(definition.heap_bytes())
            .saturating_add(text.heap_bytes())
    }
}

impl ModelSize for zumen_crs::Axes {
    const SIZE: u64 = 8 + largest(&[0, 0]);
}

impl ModelSize for crate::dimension::Axis {
    const SIZE: u64 = 8 + largest(&[0, 0]);
}

impl ModelSize for crate::text::Background {
    const SIZE: u64 = <Option<crate::text::BackgroundFill> as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <Option<u32> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            fill,
            frame,
            scale,
            transparency,
        } = self;
        0_u64
            .saturating_add(fill.heap_bytes())
            .saturating_add(frame.heap_bytes())
            .saturating_add(scale.heap_bytes())
            .saturating_add(transparency.heap_bytes())
    }
}

impl ModelSize for crate::text::BackgroundFill {
    const SIZE: u64 = 8 + largest(&[<crate::style::Color as ModelSize>::SIZE, 0]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Color(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Screen => 0,
        }
    }
}

impl ModelSize for crate::extension::Basis {
    const SIZE: u64 = 8 + largest(&[0, <crate::hash::Hash as ModelSize>::SIZE]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Document => 0,
            Self::Hash(f0) => 0_u64.saturating_add(f0.heap_bytes()),
        }
    }
}

impl ModelSize for crate::table::Border {
    const SIZE: u64 = <Option<crate::style::Inherit<crate::style::Color>> as ModelSize>::SIZE
        + <Option<crate::style::Inherit<crate::style::LineWeight>> as ModelSize>::SIZE
        + <Option<bool> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            color,
            weight,
            visible,
        } = self;
        0_u64
            .saturating_add(color.heap_bytes())
            .saturating_add(weight.heap_bytes())
            .saturating_add(visible.heap_bytes())
    }
}

impl ModelSize for crate::table::Borders {
    const SIZE: u64 = <crate::table::Border as ModelSize>::SIZE
        + <crate::table::Border as ModelSize>::SIZE
        + <crate::table::Border as ModelSize>::SIZE
        + <crate::table::Border as ModelSize>::SIZE
        + <crate::table::Border as ModelSize>::SIZE
        + <crate::table::Border as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            top,
            right,
            bottom,
            left,
            horizontal,
            vertical,
        } = self;
        0_u64
            .saturating_add(top.heap_bytes())
            .saturating_add(right.heap_bytes())
            .saturating_add(bottom.heap_bytes())
            .saturating_add(left.heap_bytes())
            .saturating_add(horizontal.heap_bytes())
            .saturating_add(vertical.heap_bytes())
    }
}

impl<P: ModelSize> ModelSize for crate::hatch::Boundary<P> {
    const SIZE: u64 = 8 + largest(&[
        <Vec<crate::shape::Vertex<P>> as ModelSize>::SIZE + <bool as ModelSize>::SIZE,
        <Vec<crate::shape::Edge<P>> as ModelSize>::SIZE,
    ]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Polyline { vertices, closed } => 0_u64
                .saturating_add(vertices.heap_bytes())
                .saturating_add(closed.heap_bytes()),
            Self::Edges(f0) => 0_u64.saturating_add(f0.heap_bytes()),
        }
    }
}

impl<P: ModelSize> ModelSize for crate::callout::Branch<P> {
    const SIZE: u64 = <Vec<Vec<P>> as ModelSize>::SIZE
        + <Option<P> as ModelSize>::SIZE
        + <Option<crate::callout::Dogleg> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { lines, end, dogleg } = self;
        0_u64
            .saturating_add(lines.heap_bytes())
            .saturating_add(end.heap_bytes())
            .saturating_add(dogleg.heap_bytes())
    }
}

impl ModelSize for zumen_base::ByteRange {
    const SIZE: u64 = <u64 as ModelSize>::SIZE + <u64 as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { start, end } = self;
        0_u64
            .saturating_add(start.heap_bytes())
            .saturating_add(end.heap_bytes())
    }
}

impl<P: ModelSize> ModelSize for crate::callout::Callout<P> {
    const SIZE: u64 = <Vec<crate::callout::Branch<P>> as ModelSize>::SIZE
        + <crate::callout::CalloutContent<P> as ModelSize>::SIZE
        + <crate::callout::PathKind as ModelSize>::SIZE
        + <crate::style::Arrowhead as ModelSize>::SIZE
        + <crate::space::Length as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <crate::style::Style as ModelSize>::SIZE
        + <crate::style::Inherit<crate::style::Color> as ModelSize>::SIZE
        + <Option<zumen_base::Ref> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            branches,
            content,
            path,
            arrowhead,
            arrow_size,
            landing,
            scale,
            line_style,
            content_color,
            style,
        } = self;
        0_u64
            .saturating_add(branches.heap_bytes())
            .saturating_add(content.heap_bytes())
            .saturating_add(path.heap_bytes())
            .saturating_add(arrowhead.heap_bytes())
            .saturating_add(arrow_size.heap_bytes())
            .saturating_add(landing.heap_bytes())
            .saturating_add(scale.heap_bytes())
            .saturating_add(line_style.heap_bytes())
            .saturating_add(content_color.heap_bytes())
            .saturating_add(style.heap_bytes())
    }
}

impl<P: ModelSize> ModelSize for crate::callout::CalloutContent<P> {
    const SIZE: u64 = 8 + largest(&[
        0,
        <Box<crate::text::FormattedText<P>> as ModelSize>::SIZE,
        <Box<crate::symbol::Symbol<P>> as ModelSize>::SIZE
            + <Vec<crate::callout::AttributeValue> as ModelSize>::SIZE,
    ]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::None => 0,
            Self::Text(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Symbol { symbol, values } => 0_u64
                .saturating_add(symbol.heap_bytes())
                .saturating_add(values.heap_bytes()),
        }
    }
}

impl ModelSize for crate::table::Cell {
    const SIZE: u64 = <crate::table::CellContent as ModelSize>::SIZE
        + <[u32; 2] as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <Option<crate::table::CellValue> as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <crate::table::CellFormat as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            content,
            span,
            covered,
            value,
            rotation,
            format,
        } = self;
        0_u64
            .saturating_add(content.heap_bytes())
            .saturating_add(span.heap_bytes())
            .saturating_add(covered.heap_bytes())
            .saturating_add(value.heap_bytes())
            .saturating_add(rotation.heap_bytes())
            .saturating_add(format.heap_bytes())
    }
}

impl ModelSize for crate::table::CellContent {
    const SIZE: u64 = 8 + largest(&[
        0,
        <String as ModelSize>::SIZE + <Vec<crate::text::Span> as ModelSize>::SIZE,
        <crate::symbol::SymbolKey as ModelSize>::SIZE,
    ]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Empty => 0,
            Self::Text { content, spans } => 0_u64
                .saturating_add(content.heap_bytes())
                .saturating_add(spans.heap_bytes()),
            Self::Symbol(f0) => 0_u64.saturating_add(f0.heap_bytes()),
        }
    }
}

impl ModelSize for crate::table::CellData {
    const SIZE: u64 = 8 + largest(&[
        <String as ModelSize>::SIZE,
        <i64 as ModelSize>::SIZE,
        <f64 as ModelSize>::SIZE,
        <crate::table::DateTime as ModelSize>::SIZE,
        <[f64; 3] as ModelSize>::SIZE,
    ]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Text(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Integer(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Real(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Date(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Point(f0) => 0_u64.saturating_add(f0.heap_bytes()),
        }
    }
}

impl ModelSize for crate::table::CellFormat {
    const SIZE: u64 = <Option<String> as ModelSize>::SIZE
        + <Option<crate::space::Length> as ModelSize>::SIZE
        + <Option<(crate::text::Horizontal, crate::text::Vertical)> as ModelSize>::SIZE
        + <Option<crate::style::Inherit<crate::style::Color>> as ModelSize>::SIZE
        + <Option<bool> as ModelSize>::SIZE
        + <Option<crate::style::Inherit<crate::style::Color>> as ModelSize>::SIZE
        + <crate::table::Borders as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            text_style,
            text_height,
            alignment,
            text_color,
            fill,
            fill_color,
            borders,
        } = self;
        0_u64
            .saturating_add(text_style.heap_bytes())
            .saturating_add(text_height.heap_bytes())
            .saturating_add(alignment.heap_bytes())
            .saturating_add(text_color.heap_bytes())
            .saturating_add(fill.heap_bytes())
            .saturating_add(fill_color.heap_bytes())
            .saturating_add(borders.heap_bytes())
    }
}

impl ModelSize for crate::table::CellValue {
    const SIZE: u64 = <crate::table::CellData as ModelSize>::SIZE
        + <crate::table::ValueUnit as ModelSize>::SIZE
        + <Option<String> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { data, unit, format } = self;
        0_u64
            .saturating_add(data.heap_bytes())
            .saturating_add(unit.heap_bytes())
            .saturating_add(format.heap_bytes())
    }
}

impl ModelSize for crate::underlay::Clip {
    const SIZE: u64 = <Vec<[f64; 2]> as ModelSize>::SIZE + <bool as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { vertices, inverted } = self;
        0_u64
            .saturating_add(vertices.heap_bytes())
            .saturating_add(inverted.heap_bytes())
    }
}

impl ModelSize for crate::style::Color {
    const SIZE: u64 =
        <Option<crate::style::ColorCode> as ModelSize>::SIZE + <Option<[u8; 3]> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { code, rgb } = self;
        0_u64
            .saturating_add(code.heap_bytes())
            .saturating_add(rgb.heap_bytes())
    }
}

impl ModelSize for crate::style::ColorCode {
    const SIZE: u64 = <crate::style::ColorSystem as ModelSize>::SIZE + <u32 as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { system, value } = self;
        0_u64
            .saturating_add(system.heap_bytes())
            .saturating_add(value.heap_bytes())
    }
}

impl ModelSize for crate::style::ColorSystem {
    const SIZE: u64 = 8 + largest(&[0, 0, 0]);
}

impl ModelSize for zumen_crs::Crs {
    const SIZE: u64 = 8 + largest(&[
        0,
        0,
        <zumen_crs::Zone as ModelSize>::SIZE,
        <zumen_crs::Zone as ModelSize>::SIZE,
        <zumen_crs::Zone as ModelSize>::SIZE,
    ]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Geographic => 0,
            Self::Wgs84 => 0,
            Self::Plane(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Jgd2000(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Tokyo(f0) => 0_u64.saturating_add(f0.heap_bytes()),
        }
    }
}

impl ModelSize for crate::table::DateTime {
    const SIZE: u64 = <u16 as ModelSize>::SIZE
        + <u8 as ModelSize>::SIZE
        + <u8 as ModelSize>::SIZE
        + <u8 as ModelSize>::SIZE
        + <u8 as ModelSize>::SIZE
        + <u8 as ModelSize>::SIZE
        + <u16 as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            millisecond,
        } = self;
        0_u64
            .saturating_add(year.heap_bytes())
            .saturating_add(month.heap_bytes())
            .saturating_add(day.heap_bytes())
            .saturating_add(hour.heap_bytes())
            .saturating_add(minute.heap_bytes())
            .saturating_add(second.heap_bytes())
            .saturating_add(millisecond.heap_bytes())
    }
}

impl<P: ModelSize> ModelSize for crate::dimension::Dimension<P> {
    const SIZE: u64 = <crate::dimension::DimensionKind<P> as ModelSize>::SIZE
        + <Option<crate::dimension::Measured> as ModelSize>::SIZE
        + <crate::dimension::Label as ModelSize>::SIZE
        + <crate::dimension::DimensionText<P> as ModelSize>::SIZE
        + <String as ModelSize>::SIZE
        + <Option<String> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            kind,
            measurement,
            label,
            text,
            style,
            block,
        } = self;
        0_u64
            .saturating_add(kind.heap_bytes())
            .saturating_add(measurement.heap_bytes())
            .saturating_add(label.heap_bytes())
            .saturating_add(text.heap_bytes())
            .saturating_add(style.heap_bytes())
            .saturating_add(block.heap_bytes())
    }
}

impl ModelSize for crate::dimension_display::DimensionDisplayRecord {
    const SIZE: u64 = <crate::dimension_display::DimensionDisplayTarget as ModelSize>::SIZE
        + <crate::dimension_display::DimensionDisplayValues as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { target, values } = self;
        0_u64
            .saturating_add(target.heap_bytes())
            .saturating_add(values.heap_bytes())
    }
}

impl ModelSize for crate::dimension_display::DimensionDisplayTarget {
    const SIZE: u64 = 8 + largest(&[
        <String as ModelSize>::SIZE,
        <String as ModelSize>::SIZE + <crate::hash::Hash as ModelSize>::SIZE,
    ]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Style(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Element { id, basis } => 0_u64
                .saturating_add(id.heap_bytes())
                .saturating_add(basis.heap_bytes()),
        }
    }
}

impl ModelSize for crate::dimension_display::DimensionDisplayValues {
    const SIZE: u64 = <Option<f64> as ModelSize>::SIZE
        + <Option<f64> as ModelSize>::SIZE
        + <Option<f64> as ModelSize>::SIZE
        + <Option<f64> as ModelSize>::SIZE
        + <Option<f64> as ModelSize>::SIZE
        + <Option<bool> as ModelSize>::SIZE
        + <Option<bool> as ModelSize>::SIZE
        + <Option<f64> as ModelSize>::SIZE
        + <Option<f64> as ModelSize>::SIZE
        + <Option<u8> as ModelSize>::SIZE
        + <Option<String> as ModelSize>::SIZE
        + <Option<u8> as ModelSize>::SIZE
        + <Option<f64> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            scale,
            arrow_size,
            tick_size,
            extension_offset,
            extension_beyond,
            suppress_first_extension,
            suppress_second_extension,
            text_height,
            linear_factor,
            linear_precision,
            text_style,
            text_vertical_placement,
            text_gap,
        } = self;
        0_u64
            .saturating_add(scale.heap_bytes())
            .saturating_add(arrow_size.heap_bytes())
            .saturating_add(tick_size.heap_bytes())
            .saturating_add(extension_offset.heap_bytes())
            .saturating_add(extension_beyond.heap_bytes())
            .saturating_add(suppress_first_extension.heap_bytes())
            .saturating_add(suppress_second_extension.heap_bytes())
            .saturating_add(text_height.heap_bytes())
            .saturating_add(linear_factor.heap_bytes())
            .saturating_add(linear_precision.heap_bytes())
            .saturating_add(text_style.heap_bytes())
            .saturating_add(text_vertical_placement.heap_bytes())
            .saturating_add(text_gap.heap_bytes())
    }
}

impl<P: ModelSize> ModelSize for crate::dimension::DimensionKind<P> {
    const SIZE: u64 = 8 + largest(&[
        <P as ModelSize>::SIZE
            + <P as ModelSize>::SIZE
            + <P as ModelSize>::SIZE
            + <f64 as ModelSize>::SIZE
            + <f64 as ModelSize>::SIZE,
        <P as ModelSize>::SIZE
            + <P as ModelSize>::SIZE
            + <P as ModelSize>::SIZE
            + <f64 as ModelSize>::SIZE,
        <P as ModelSize>::SIZE
            + <P as ModelSize>::SIZE
            + <P as ModelSize>::SIZE
            + <P as ModelSize>::SIZE,
        <[P; 2] as ModelSize>::SIZE + <[P; 2] as ModelSize>::SIZE + <P as ModelSize>::SIZE,
        <P as ModelSize>::SIZE
            + <P as ModelSize>::SIZE
            + <P as ModelSize>::SIZE
            + <P as ModelSize>::SIZE,
        <P as ModelSize>::SIZE + <P as ModelSize>::SIZE + <crate::space::Length as ModelSize>::SIZE,
        <P as ModelSize>::SIZE + <P as ModelSize>::SIZE + <crate::space::Length as ModelSize>::SIZE,
        <P as ModelSize>::SIZE
            + <P as ModelSize>::SIZE
            + <P as ModelSize>::SIZE
            + <crate::dimension::Axis as ModelSize>::SIZE,
    ]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Linear {
                first,
                second,
                line,
                rotation,
                oblique,
            } => 0_u64
                .saturating_add(first.heap_bytes())
                .saturating_add(second.heap_bytes())
                .saturating_add(line.heap_bytes())
                .saturating_add(rotation.heap_bytes())
                .saturating_add(oblique.heap_bytes()),
            Self::Aligned {
                first,
                second,
                line,
                oblique,
            } => 0_u64
                .saturating_add(first.heap_bytes())
                .saturating_add(second.heap_bytes())
                .saturating_add(line.heap_bytes())
                .saturating_add(oblique.heap_bytes()),
            Self::Angular {
                vertex,
                first,
                second,
                arc,
            } => 0_u64
                .saturating_add(vertex.heap_bytes())
                .saturating_add(first.heap_bytes())
                .saturating_add(second.heap_bytes())
                .saturating_add(arc.heap_bytes()),
            Self::AngularLines { first, second, arc } => 0_u64
                .saturating_add(first.heap_bytes())
                .saturating_add(second.heap_bytes())
                .saturating_add(arc.heap_bytes()),
            Self::ArcLength {
                center,
                first,
                second,
                arc,
            } => 0_u64
                .saturating_add(center.heap_bytes())
                .saturating_add(first.heap_bytes())
                .saturating_add(second.heap_bytes())
                .saturating_add(arc.heap_bytes()),
            Self::Radius {
                center,
                chord,
                leader,
            } => 0_u64
                .saturating_add(center.heap_bytes())
                .saturating_add(chord.heap_bytes())
                .saturating_add(leader.heap_bytes()),
            Self::Diameter {
                chord,
                opposite,
                leader,
            } => 0_u64
                .saturating_add(chord.heap_bytes())
                .saturating_add(opposite.heap_bytes())
                .saturating_add(leader.heap_bytes()),
            Self::Ordinate {
                origin,
                feature,
                leader_end,
                axis,
            } => 0_u64
                .saturating_add(origin.heap_bytes())
                .saturating_add(feature.heap_bytes())
                .saturating_add(leader_end.heap_bytes())
                .saturating_add(axis.heap_bytes()),
        }
    }
}

impl ModelSize for crate::style::DimensionStyle {
    const SIZE: u64 = <String as ModelSize>::SIZE
        + <[crate::style::Arrowhead; 2] as ModelSize>::SIZE
        + <crate::style::Arrowhead as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <Option<f64> as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <zumen_base::Ref as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            name,
            ends,
            leader,
            size,
            tick,
            scale,
            source,
        } = self;
        0_u64
            .saturating_add(name.heap_bytes())
            .saturating_add(ends.heap_bytes())
            .saturating_add(leader.heap_bytes())
            .saturating_add(size.heap_bytes())
            .saturating_add(tick.heap_bytes())
            .saturating_add(scale.heap_bytes())
            .saturating_add(source.heap_bytes())
    }
}

impl<P: ModelSize> ModelSize for crate::dimension::DimensionText<P> {
    const SIZE: u64 = <Option<P> as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <Option<f64> as ModelSize>::SIZE
        + <Option<(crate::text::Horizontal, crate::text::Vertical)> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            position,
            moved,
            rotation,
            attachment,
        } = self;
        0_u64
            .saturating_add(position.heap_bytes())
            .saturating_add(moved.heap_bytes())
            .saturating_add(rotation.heap_bytes())
            .saturating_add(attachment.heap_bytes())
    }
}

impl ModelSize for crate::callout::Dogleg {
    const SIZE: u64 = <[f64; 2] as ModelSize>::SIZE + <crate::space::Length as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { direction, length } = self;
        0_u64
            .saturating_add(direction.heap_bytes())
            .saturating_add(length.heap_bytes())
    }
}

impl ModelSize for crate::Drawing2D {
    const SIZE: u64 = <Vec<crate::space::ModelSpace> as ModelSize>::SIZE
        + <Vec<crate::space::Sheet> as ModelSize>::SIZE
        + <Vec<crate::style::Layer> as ModelSize>::SIZE
        + <Vec<crate::style::LineType> as ModelSize>::SIZE
        + <Option<f64> as ModelSize>::SIZE
        + <Option<crate::space::PaperLength> as ModelSize>::SIZE
        + <Option<crate::style::Color> as ModelSize>::SIZE
        + <Option<crate::style::LineTypeId> as ModelSize>::SIZE
        + <Vec<crate::style::DimensionStyle> as ModelSize>::SIZE
        + <Vec<crate::style::TextStyle> as ModelSize>::SIZE
        + <Vec<crate::symbol::SymbolDefinition> as ModelSize>::SIZE
        + <Vec<crate::extension::Extension> as ModelSize>::SIZE
        + <Vec<crate::TitleBlock> as ModelSize>::SIZE
        + <Option<crate::point::PointStyle> as ModelSize>::SIZE
        + <Option<zumen_crs::Georeference> as ModelSize>::SIZE
        + <Vec<crate::dimension_display::DimensionDisplayRecord> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            spaces,
            sheets,
            layers,
            line_types,
            line_type_scale,
            default_line_weight,
            default_color,
            default_line_type,
            dimension_styles,
            text_styles,
            symbols,
            extensions,
            title_blocks,
            point_style,
            georeference,
            dimension_display,
        } = self;
        0_u64
            .saturating_add(spaces.heap_bytes())
            .saturating_add(sheets.heap_bytes())
            .saturating_add(layers.heap_bytes())
            .saturating_add(line_types.heap_bytes())
            .saturating_add(line_type_scale.heap_bytes())
            .saturating_add(default_line_weight.heap_bytes())
            .saturating_add(default_color.heap_bytes())
            .saturating_add(default_line_type.heap_bytes())
            .saturating_add(dimension_styles.heap_bytes())
            .saturating_add(text_styles.heap_bytes())
            .saturating_add(symbols.heap_bytes())
            .saturating_add(extensions.heap_bytes())
            .saturating_add(title_blocks.heap_bytes())
            .saturating_add(point_style.heap_bytes())
            .saturating_add(georeference.heap_bytes())
            .saturating_add(dimension_display.heap_bytes())
    }
}

impl<P: ModelSize> ModelSize for crate::shape::Edge<P> {
    const SIZE: u64 = 8 + largest(&[
        <P as ModelSize>::SIZE + <P as ModelSize>::SIZE,
        <P as ModelSize>::SIZE
            + <crate::space::Length as ModelSize>::SIZE
            + <f64 as ModelSize>::SIZE
            + <f64 as ModelSize>::SIZE
            + <bool as ModelSize>::SIZE,
        <P as ModelSize>::SIZE
            + <P as ModelSize>::SIZE
            + <f64 as ModelSize>::SIZE
            + <f64 as ModelSize>::SIZE
            + <f64 as ModelSize>::SIZE
            + <bool as ModelSize>::SIZE,
        <crate::shape::Spline<P> as ModelSize>::SIZE,
        <P as ModelSize>::SIZE
            + <crate::space::Length as ModelSize>::SIZE
            + <f64 as ModelSize>::SIZE
            + <bool as ModelSize>::SIZE
            + <crate::space::Length as ModelSize>::SIZE
            + <crate::space::Length as ModelSize>::SIZE,
    ]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Line { start, end } => 0_u64
                .saturating_add(start.heap_bytes())
                .saturating_add(end.heap_bytes()),
            Self::Arc {
                center,
                radius,
                start,
                end,
                counterclockwise,
            } => 0_u64
                .saturating_add(center.heap_bytes())
                .saturating_add(radius.heap_bytes())
                .saturating_add(start.heap_bytes())
                .saturating_add(end.heap_bytes())
                .saturating_add(counterclockwise.heap_bytes()),
            Self::Elliptic {
                center,
                major,
                ratio,
                start,
                end,
                counterclockwise,
            } => 0_u64
                .saturating_add(center.heap_bytes())
                .saturating_add(major.heap_bytes())
                .saturating_add(ratio.heap_bytes())
                .saturating_add(start.heap_bytes())
                .saturating_add(end.heap_bytes())
                .saturating_add(counterclockwise.heap_bytes()),
            Self::Spline(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Clothoid {
                origin,
                parameter,
                angle,
                counterclockwise,
                start,
                end,
            } => 0_u64
                .saturating_add(origin.heap_bytes())
                .saturating_add(parameter.heap_bytes())
                .saturating_add(angle.heap_bytes())
                .saturating_add(counterclockwise.heap_bytes())
                .saturating_add(start.heap_bytes())
                .saturating_add(end.heap_bytes()),
        }
    }
}

impl<P: ModelSize> ModelSize for crate::shape::Element<P> {
    const SIZE: u64 = <zumen_base::Ref as ModelSize>::SIZE
        + <crate::style::LayerId as ModelSize>::SIZE
        + <crate::style::Style as ModelSize>::SIZE
        + <crate::shape::Shape<P> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            source,
            layer,
            style,
            shape,
        } = self;
        0_u64
            .saturating_add(source.heap_bytes())
            .saturating_add(layer.heap_bytes())
            .saturating_add(style.heap_bytes())
            .saturating_add(shape.heap_bytes())
    }
}

impl ModelSize for crate::extension::Extension {
    const SIZE: u64 = <std::borrow::Cow<'static, str> as ModelSize>::SIZE
        + <String as ModelSize>::SIZE
        + <Option<String> as ModelSize>::SIZE
        + <u32 as ModelSize>::SIZE
        + <crate::extension::Target as ModelSize>::SIZE
        + <crate::extension::Basis as ModelSize>::SIZE
        + <Vec<crate::extension::Field> as ModelSize>::SIZE
        + <zumen_base::Ref as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            namespace,
            name,
            version,
            schema,
            target,
            basis,
            fields,
            source,
        } = self;
        0_u64
            .saturating_add(namespace.heap_bytes())
            .saturating_add(name.heap_bytes())
            .saturating_add(version.heap_bytes())
            .saturating_add(schema.heap_bytes())
            .saturating_add(target.heap_bytes())
            .saturating_add(basis.heap_bytes())
            .saturating_add(fields.heap_bytes())
            .saturating_add(source.heap_bytes())
    }
}

impl ModelSize for crate::extension::Field {
    const SIZE: u64 = <std::borrow::Cow<'static, str> as ModelSize>::SIZE
        + <Option<i16> as ModelSize>::SIZE
        + <crate::extension::FieldValue as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { key, code, value } = self;
        0_u64
            .saturating_add(key.heap_bytes())
            .saturating_add(code.heap_bytes())
            .saturating_add(value.heap_bytes())
    }
}

impl ModelSize for crate::extension::FieldValue {
    const SIZE: u64 = 8 + largest(&[
        <String as ModelSize>::SIZE,
        <i64 as ModelSize>::SIZE,
        <f64 as ModelSize>::SIZE,
        <[f64; 3] as ModelSize>::SIZE,
        <u64 as ModelSize>::SIZE,
        <Vec<u8> as ModelSize>::SIZE,
    ]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Text(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Integer(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Real(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Point(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Handle(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Bytes(f0) => 0_u64.saturating_add(f0.heap_bytes()),
        }
    }
}

impl ModelSize for crate::hatch::Fill {
    const SIZE: u64 = 8 + largest(&[
        0,
        <crate::hatch::Pattern as ModelSize>::SIZE,
        <crate::hatch::Gradient as ModelSize>::SIZE,
    ]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Solid => 0,
            Self::Pattern(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Gradient(f0) => 0_u64.saturating_add(f0.heap_bytes()),
        }
    }
}

impl ModelSize for crate::text::Fit {
    const SIZE: u64 = 8 + largest(&[0, 0]);
}

impl ModelSize for crate::text::Flow {
    const SIZE: u64 = 8 + largest(&[0, 0, 0]);
}

impl ModelSize for crate::text::Font {
    const SIZE: u64 = <String as ModelSize>::SIZE + <crate::text::FontKind as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { name, kind } = self;
        0_u64
            .saturating_add(name.heap_bytes())
            .saturating_add(kind.heap_bytes())
    }
}

impl ModelSize for crate::text::FontKind {
    const SIZE: u64 = 8 + largest(&[
        0,
        <bool as ModelSize>::SIZE
            + <bool as ModelSize>::SIZE
            + <Option<u32> as ModelSize>::SIZE
            + <Option<u32> as ModelSize>::SIZE,
    ]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Shape => 0,
            Self::Named {
                bold,
                italic,
                charset,
                pitch,
            } => 0_u64
                .saturating_add(bold.heap_bytes())
                .saturating_add(italic.heap_bytes())
                .saturating_add(charset.heap_bytes())
                .saturating_add(pitch.heap_bytes()),
        }
    }
}

impl ModelSize for crate::text::Format {
    const SIZE: u64 = <Option<crate::text::Scaled<crate::space::Length>> as ModelSize>::SIZE
        + <Option<crate::text::Scaled<f64>> as ModelSize>::SIZE
        + <Option<crate::text::Scaled<f64>> as ModelSize>::SIZE
        + <Option<f64> as ModelSize>::SIZE
        + <Option<crate::text::Font> as ModelSize>::SIZE
        + <Option<crate::style::Inherit<crate::style::Color>> as ModelSize>::SIZE
        + <Option<crate::text::Vertical> as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            height,
            width_factor,
            tracking,
            oblique,
            font,
            color,
            align,
            underline,
            overline,
            strike,
        } = self;
        0_u64
            .saturating_add(height.heap_bytes())
            .saturating_add(width_factor.heap_bytes())
            .saturating_add(tracking.heap_bytes())
            .saturating_add(oblique.heap_bytes())
            .saturating_add(font.heap_bytes())
            .saturating_add(color.heap_bytes())
            .saturating_add(align.heap_bytes())
            .saturating_add(underline.heap_bytes())
            .saturating_add(overline.heap_bytes())
            .saturating_add(strike.heap_bytes())
    }
}

impl<P: ModelSize> ModelSize for crate::text::FormattedText<P> {
    const SIZE: u64 = <String as ModelSize>::SIZE
        + <Vec<crate::text::Span> as ModelSize>::SIZE
        + <crate::text::Height as ModelSize>::SIZE
        + <P as ModelSize>::SIZE
        + <crate::text::Horizontal as ModelSize>::SIZE
        + <crate::text::Vertical as ModelSize>::SIZE
        + <crate::text::Rotation as ModelSize>::SIZE
        + <Option<crate::space::Length> as ModelSize>::SIZE
        + <Option<crate::space::Length> as ModelSize>::SIZE
        + <crate::text::Flow as ModelSize>::SIZE
        + <crate::text::LineSpacing as ModelSize>::SIZE
        + <Option<crate::text::Background> as ModelSize>::SIZE
        + <String as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            content,
            spans,
            height,
            point,
            horizontal,
            vertical,
            rotation,
            width,
            frame_height,
            flow,
            line_spacing,
            background,
            style,
        } = self;
        0_u64
            .saturating_add(content.heap_bytes())
            .saturating_add(spans.heap_bytes())
            .saturating_add(height.heap_bytes())
            .saturating_add(point.heap_bytes())
            .saturating_add(horizontal.heap_bytes())
            .saturating_add(vertical.heap_bytes())
            .saturating_add(rotation.heap_bytes())
            .saturating_add(width.heap_bytes())
            .saturating_add(frame_height.heap_bytes())
            .saturating_add(flow.heap_bytes())
            .saturating_add(line_spacing.heap_bytes())
            .saturating_add(background.heap_bytes())
            .saturating_add(style.heap_bytes())
    }
}

impl ModelSize for zumen_crs::Georeference {
    const SIZE: u64 =
        <zumen_crs::Crs as ModelSize>::SIZE + <zumen_crs::MapConversion as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { crs, conversion } = self;
        0_u64
            .saturating_add(crs.heap_bytes())
            .saturating_add(conversion.heap_bytes())
    }
}

impl ModelSize for crate::hatch::Gradient {
    const SIZE: u64 = <String as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <Vec<[u8; 3]> as ModelSize>::SIZE
        + <Option<f64> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            name,
            angle,
            shift,
            colors,
            tint,
        } = self;
        0_u64
            .saturating_add(name.heap_bytes())
            .saturating_add(angle.heap_bytes())
            .saturating_add(shift.heap_bytes())
            .saturating_add(colors.heap_bytes())
            .saturating_add(tint.heap_bytes())
    }
}

impl ModelSize for crate::symbol::Grid {
    const SIZE: u64 = <u32 as ModelSize>::SIZE
        + <u32 as ModelSize>::SIZE
        + <crate::space::Length as ModelSize>::SIZE
        + <crate::space::Length as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            columns,
            rows,
            column_spacing,
            row_spacing,
        } = self;
        0_u64
            .saturating_add(columns.heap_bytes())
            .saturating_add(rows.heap_bytes())
            .saturating_add(column_spacing.heap_bytes())
            .saturating_add(row_spacing.heap_bytes())
    }
}

impl ModelSize for crate::hash::Hash {
    const SIZE: u64 = <[u8; 16] as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self(f0) = self;
        0_u64.saturating_add(f0.heap_bytes())
    }
}

impl<P: ModelSize> ModelSize for crate::hatch::Hatch<P> {
    const SIZE: u64 = <Vec<crate::hatch::HatchLoop<P>> as ModelSize>::SIZE
        + <crate::hatch::Islands as ModelSize>::SIZE
        + <crate::hatch::Fill as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <Vec<P> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            loops,
            islands,
            fill,
            associative,
            seeds,
        } = self;
        0_u64
            .saturating_add(loops.heap_bytes())
            .saturating_add(islands.heap_bytes())
            .saturating_add(fill.heap_bytes())
            .saturating_add(associative.heap_bytes())
            .saturating_add(seeds.heap_bytes())
    }
}

impl<P: ModelSize> ModelSize for crate::hatch::HatchLoop<P> {
    const SIZE: u64 = <crate::hatch::Boundary<P> as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <Vec<zumen_base::Ref> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            boundary,
            external,
            outermost,
            derived,
            text_box,
            sources,
        } = self;
        0_u64
            .saturating_add(boundary.heap_bytes())
            .saturating_add(external.heap_bytes())
            .saturating_add(outermost.heap_bytes())
            .saturating_add(derived.heap_bytes())
            .saturating_add(text_box.heap_bytes())
            .saturating_add(sources.heap_bytes())
    }
}

impl ModelSize for crate::text::Height {
    const SIZE: u64 =
        <crate::space::Length as ModelSize>::SIZE + <crate::text::HeightMeaning as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { value, meaning } = self;
        0_u64
            .saturating_add(value.heap_bytes())
            .saturating_add(meaning.heap_bytes())
    }
}

impl ModelSize for crate::text::HeightMeaning {
    const SIZE: u64 = 8 + largest(&[0, 0, 0, <std::borrow::Cow<'static, str> as ModelSize>::SIZE]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Frame => 0,
            Self::CapHeight => 0,
            Self::Em => 0,
            Self::Declared(f0) => 0_u64.saturating_add(f0.heap_bytes()),
        }
    }
}

impl ModelSize for crate::text::Horizontal {
    const SIZE: u64 = 8 + largest(&[0, 0, 0, 0]);
}

impl<T: ModelSize> ModelSize for crate::style::Inherit<T> {
    const SIZE: u64 = 8 + largest(&[0, 0, <T as ModelSize>::SIZE]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Layer => 0,
            Self::Block => 0,
            Self::Own(f0) => 0_u64.saturating_add(f0.heap_bytes()),
        }
    }
}

impl ModelSize for crate::hatch::Islands {
    const SIZE: u64 = 8 + largest(&[0, 0, 0]);
}

impl ModelSize for zumen_base::Key {
    const SIZE: u64 = 8 + largest(&[<u64 as ModelSize>::SIZE, <u64 as ModelSize>::SIZE]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Handle(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Instance(f0) => 0_u64.saturating_add(f0.heap_bytes()),
        }
    }
}

impl ModelSize for crate::shape::KnotParameterization {
    const SIZE: u64 = 8 + largest(&[0, 0, 0, 0]);
}

impl ModelSize for crate::dimension::Label {
    const SIZE: u64 = 8 + largest(&[
        0,
        <String as ModelSize>::SIZE + <Vec<crate::text::Span> as ModelSize>::SIZE,
        0,
    ]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Measurement => 0,
            Self::Override { content, spans } => 0_u64
                .saturating_add(content.heap_bytes())
                .saturating_add(spans.heap_bytes()),
            Self::Hidden => 0,
        }
    }
}

impl ModelSize for crate::style::Layer {
    const SIZE: u64 = <crate::style::LayerId as ModelSize>::SIZE
        + <String as ModelSize>::SIZE
        + <Option<crate::style::LayerState> as ModelSize>::SIZE
        + <Option<crate::style::LayerStyle> as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <Option<String> as ModelSize>::SIZE
        + <zumen_base::Ref as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            id,
            name,
            state,
            style,
            follows_placement,
            description,
            source,
        } = self;
        0_u64
            .saturating_add(id.heap_bytes())
            .saturating_add(name.heap_bytes())
            .saturating_add(state.heap_bytes())
            .saturating_add(style.heap_bytes())
            .saturating_add(follows_placement.heap_bytes())
            .saturating_add(description.heap_bytes())
            .saturating_add(source.heap_bytes())
    }
}

impl ModelSize for crate::style::LayerId {
    const SIZE: u64 = <u32 as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self(f0) = self;
        0_u64.saturating_add(f0.heap_bytes())
    }
}

impl ModelSize for crate::style::LayerState {
    const SIZE: u64 = <bool as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            visible,
            printable,
            editable,
            frozen,
        } = self;
        0_u64
            .saturating_add(visible.heap_bytes())
            .saturating_add(printable.heap_bytes())
            .saturating_add(editable.heap_bytes())
            .saturating_add(frozen.heap_bytes())
    }
}

impl ModelSize for crate::style::LayerStyle {
    const SIZE: u64 = <Option<crate::style::Color> as ModelSize>::SIZE
        + <Option<crate::style::LineTypeId> as ModelSize>::SIZE
        + <Option<crate::style::LineWeight> as ModelSize>::SIZE
        + <Option<crate::style::Transparency> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            color,
            line_type,
            line_weight,
            transparency,
        } = self;
        0_u64
            .saturating_add(color.heap_bytes())
            .saturating_add(line_type.heap_bytes())
            .saturating_add(line_weight.heap_bytes())
            .saturating_add(transparency.heap_bytes())
    }
}

impl<P: ModelSize> ModelSize for crate::dimension::Leader<P> {
    const SIZE: u64 = <Vec<P> as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <Option<bool> as ModelSize>::SIZE
        + <Option<zumen_base::Ref> as ModelSize>::SIZE
        + <String as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            path,
            spline,
            arrow,
            annotation,
            style,
        } = self;
        0_u64
            .saturating_add(path.heap_bytes())
            .saturating_add(spline.heap_bytes())
            .saturating_add(arrow.heap_bytes())
            .saturating_add(annotation.heap_bytes())
            .saturating_add(style.heap_bytes())
    }
}

impl ModelSize for crate::space::Length {
    const SIZE: u64 = 8 + largest(&[
        <crate::space::PaperLength as ModelSize>::SIZE,
        <crate::space::ModelLength as ModelSize>::SIZE,
    ]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Paper(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Model(f0) => 0_u64.saturating_add(f0.heap_bytes()),
        }
    }
}

impl ModelSize for crate::text::LineSpacing {
    const SIZE: u64 = <f64 as ModelSize>::SIZE + <bool as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { factor, exact } = self;
        0_u64
            .saturating_add(factor.heap_bytes())
            .saturating_add(exact.heap_bytes())
    }
}

impl ModelSize for crate::style::LineType {
    const SIZE: u64 = <crate::style::LineTypeId as ModelSize>::SIZE
        + <String as ModelSize>::SIZE
        + <Option<crate::style::LineTypeDefinition> as ModelSize>::SIZE
        + <zumen_base::Ref as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            id,
            name,
            definition,
            source,
        } = self;
        0_u64
            .saturating_add(id.heap_bytes())
            .saturating_add(name.heap_bytes())
            .saturating_add(definition.heap_bytes())
            .saturating_add(source.heap_bytes())
    }
}

impl ModelSize for crate::style::LineTypeDefinition {
    const SIZE: u64 = <String as ModelSize>::SIZE
        + <Vec<f64> as ModelSize>::SIZE
        + <crate::style::PatternBasis as ModelSize>::SIZE
        + <Vec<crate::style::LineTypeMark> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            description,
            pattern,
            basis,
            marks,
        } = self;
        0_u64
            .saturating_add(description.heap_bytes())
            .saturating_add(pattern.heap_bytes())
            .saturating_add(basis.heap_bytes())
            .saturating_add(marks.heap_bytes())
    }
}

impl ModelSize for crate::style::LineTypeId {
    const SIZE: u64 = <u32 as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self(f0) = self;
        0_u64.saturating_add(f0.heap_bytes())
    }
}

impl ModelSize for crate::style::LineTypeMark {
    const SIZE: u64 = <usize as ModelSize>::SIZE
        + <crate::style::MarkContent as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <crate::style::MarkRotation as ModelSize>::SIZE
        + <[f64; 2] as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            index,
            content,
            scale,
            rotation,
            rotation_basis,
            offset,
        } = self;
        0_u64
            .saturating_add(index.heap_bytes())
            .saturating_add(content.heap_bytes())
            .saturating_add(scale.heap_bytes())
            .saturating_add(rotation.heap_bytes())
            .saturating_add(rotation_basis.heap_bytes())
            .saturating_add(offset.heap_bytes())
    }
}

impl ModelSize for crate::style::LineWeight {
    const SIZE: u64 = 8 + largest(&[<crate::space::PaperLength as ModelSize>::SIZE, 0]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Width(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Default => 0,
        }
    }
}

impl ModelSize for zumen_crs::MapConversion {
    const SIZE: u64 = <f64 as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            eastings,
            northings,
            orthogonal_height,
            x_axis_abscissa,
            x_axis_ordinate,
            scale,
        } = self;
        0_u64
            .saturating_add(eastings.heap_bytes())
            .saturating_add(northings.heap_bytes())
            .saturating_add(orthogonal_height.heap_bytes())
            .saturating_add(x_axis_abscissa.heap_bytes())
            .saturating_add(x_axis_ordinate.heap_bytes())
            .saturating_add(scale.heap_bytes())
    }
}

impl ModelSize for crate::style::MarkContent {
    const SIZE: u64 = 8 + largest(&[
        <Option<crate::text::Font> as ModelSize>::SIZE + <u16 as ModelSize>::SIZE,
        <String as ModelSize>::SIZE + <Option<String> as ModelSize>::SIZE,
    ]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Shape { font, number } => 0_u64
                .saturating_add(font.heap_bytes())
                .saturating_add(number.heap_bytes()),
            Self::Text { content, style } => 0_u64
                .saturating_add(content.heap_bytes())
                .saturating_add(style.heap_bytes()),
        }
    }
}

impl ModelSize for crate::style::MarkRotation {
    const SIZE: u64 = 8 + largest(&[0, 0, 0]);
}

impl ModelSize for crate::point::MarkerKey {
    const SIZE: u64 =
        <std::borrow::Cow<'static, str> as ModelSize>::SIZE + <String as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { namespace, code } = self;
        0_u64
            .saturating_add(namespace.heap_bytes())
            .saturating_add(code.heap_bytes())
    }
}

impl ModelSize for crate::dimension::Measured {
    const SIZE: u64 =
        <f64 as ModelSize>::SIZE + <std::borrow::Cow<'static, str> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { value, key } = self;
        0_u64
            .saturating_add(value.heap_bytes())
            .saturating_add(key.heap_bytes())
    }
}

impl ModelSize for crate::space::ModelLength {
    const SIZE: u64 = <crate::SpaceId as ModelSize>::SIZE + <f64 as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { space, value } = self;
        0_u64
            .saturating_add(space.heap_bytes())
            .saturating_add(value.heap_bytes())
    }
}

impl ModelSize for crate::space::ModelPoint {
    const SIZE: u64 =
        <crate::SpaceId as ModelSize>::SIZE + <f64 as ModelSize>::SIZE + <f64 as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { space, x, y } = self;
        0_u64
            .saturating_add(space.heap_bytes())
            .saturating_add(x.heap_bytes())
            .saturating_add(y.heap_bytes())
    }
}

impl ModelSize for crate::space::ModelSpace {
    const SIZE: u64 = <crate::SpaceId as ModelSize>::SIZE
        + <crate::space::Unit as ModelSize>::SIZE
        + <zumen_crs::Axes as ModelSize>::SIZE
        + <Vec<crate::shape::Element<crate::space::ModelPoint>> as ModelSize>::SIZE
        + <Vec<usize> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            id,
            unit,
            axes,
            elements,
            order,
        } = self;
        0_u64
            .saturating_add(id.heap_bytes())
            .saturating_add(unit.heap_bytes())
            .saturating_add(axes.heap_bytes())
            .saturating_add(elements.heap_bytes())
            .saturating_add(order.heap_bytes())
    }
}

impl ModelSize for crate::space::PaperLength {
    const SIZE: u64 = <f64 as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self(f0) = self;
        0_u64.saturating_add(f0.heap_bytes())
    }
}

impl ModelSize for crate::space::PaperPoint {
    const SIZE: u64 =
        <crate::SheetId as ModelSize>::SIZE + <f64 as ModelSize>::SIZE + <f64 as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { sheet, x, y } = self;
        0_u64
            .saturating_add(sheet.heap_bytes())
            .saturating_add(x.heap_bytes())
            .saturating_add(y.heap_bytes())
    }
}

impl ModelSize for crate::callout::PathKind {
    const SIZE: u64 = 8 + largest(&[0, 0, 0]);
}

impl ModelSize for crate::hatch::Pattern {
    const SIZE: u64 = <String as ModelSize>::SIZE
        + <crate::hatch::PatternOrigin as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <Vec<crate::hatch::PatternLine> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            name,
            origin,
            angle,
            scale,
            double,
            lines,
        } = self;
        0_u64
            .saturating_add(name.heap_bytes())
            .saturating_add(origin.heap_bytes())
            .saturating_add(angle.heap_bytes())
            .saturating_add(scale.heap_bytes())
            .saturating_add(double.heap_bytes())
            .saturating_add(lines.heap_bytes())
    }
}

impl ModelSize for crate::style::PatternBasis {
    const SIZE: u64 = 8 + largest(&[0, 0]);
}

impl ModelSize for crate::hatch::PatternLine {
    const SIZE: u64 = <f64 as ModelSize>::SIZE
        + <[f64; 2] as ModelSize>::SIZE
        + <[f64; 2] as ModelSize>::SIZE
        + <Vec<f64> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            angle,
            base,
            offset,
            dashes,
        } = self;
        0_u64
            .saturating_add(angle.heap_bytes())
            .saturating_add(base.heap_bytes())
            .saturating_add(offset.heap_bytes())
            .saturating_add(dashes.heap_bytes())
    }
}

impl ModelSize for crate::hatch::PatternOrigin {
    const SIZE: u64 = 8 + largest(&[0, 0, 0]);
}

impl ModelSize for crate::text::Piece {
    const SIZE: u64 = 8 + largest(&[
        <String as ModelSize>::SIZE,
        0,
        0,
        0,
        0,
        <String as ModelSize>::SIZE
            + <String as ModelSize>::SIZE
            + <crate::text::StackKind as ModelSize>::SIZE,
        <String as ModelSize>::SIZE,
    ]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Text(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Paragraph => 0,
            Self::Column => 0,
            Self::Measurement => 0,
            Self::Below => 0,
            Self::Stack { upper, lower, kind } => 0_u64
                .saturating_add(upper.heap_bytes())
                .saturating_add(lower.heap_bytes())
                .saturating_add(kind.heap_bytes()),
            Self::Unparsed(f0) => 0_u64.saturating_add(f0.heap_bytes()),
        }
    }
}

impl<P: ModelSize> ModelSize for crate::text::Placement<P> {
    const SIZE: u64 = 8 + largest(&[
        <P as ModelSize>::SIZE
            + <crate::text::Horizontal as ModelSize>::SIZE
            + <crate::text::Vertical as ModelSize>::SIZE,
        <P as ModelSize>::SIZE + <P as ModelSize>::SIZE + <crate::text::Fit as ModelSize>::SIZE,
    ]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Anchor {
                point,
                horizontal,
                vertical,
            } => 0_u64
                .saturating_add(point.heap_bytes())
                .saturating_add(horizontal.heap_bytes())
                .saturating_add(vertical.heap_bytes()),
            Self::Between { first, second, fit } => 0_u64
                .saturating_add(first.heap_bytes())
                .saturating_add(second.heap_bytes())
                .saturating_add(fit.heap_bytes()),
        }
    }
}

impl<P: ModelSize> ModelSize for crate::point::Point<P> {
    const SIZE: u64 = <P as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <crate::point::PointLook as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            position,
            rotation,
            look,
            construction,
        } = self;
        0_u64
            .saturating_add(position.heap_bytes())
            .saturating_add(rotation.heap_bytes())
            .saturating_add(look.heap_bytes())
            .saturating_add(construction.heap_bytes())
    }
}

impl ModelSize for crate::point::PointLook {
    const SIZE: u64 = 8 + largest(&[
        0,
        <crate::point::MarkerKey as ModelSize>::SIZE + <f64 as ModelSize>::SIZE,
    ]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Document => 0,
            Self::Marker { key, scale } => 0_u64
                .saturating_add(key.heap_bytes())
                .saturating_add(scale.heap_bytes()),
        }
    }
}

impl ModelSize for crate::point::PointMark {
    const SIZE: u64 = 8 + largest(&[0, 0, 0, 0, 0]);
}

impl ModelSize for crate::point::PointSize {
    const SIZE: u64 = 8 + largest(&[<f64 as ModelSize>::SIZE, <f64 as ModelSize>::SIZE]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Length(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Screen(f0) => 0_u64.saturating_add(f0.heap_bytes()),
        }
    }
}

impl ModelSize for crate::point::PointStyle {
    const SIZE: u64 = <crate::point::PointMark as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <crate::point::PointSize as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            mark,
            circle,
            square,
            size,
        } = self;
        0_u64
            .saturating_add(mark.heap_bytes())
            .saturating_add(circle.heap_bytes())
            .saturating_add(square.heap_bytes())
            .saturating_add(size.heap_bytes())
    }
}

impl<P: ModelSize> ModelSize for crate::shape::Polyline<P> {
    const SIZE: u64 = <Vec<crate::shape::Vertex<P>> as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <Option<crate::space::Length> as ModelSize>::SIZE
        + <Vec<[crate::space::Length; 2]> as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            vertices,
            closed,
            width,
            vertex_widths,
            line_type_generation,
        } = self;
        0_u64
            .saturating_add(vertices.heap_bytes())
            .saturating_add(closed.heap_bytes())
            .saturating_add(width.heap_bytes())
            .saturating_add(vertex_widths.heap_bytes())
            .saturating_add(line_type_generation.heap_bytes())
    }
}

impl ModelSize for crate::underlay::Resource {
    const SIZE: u64 = <String as ModelSize>::SIZE + <zumen_base::Ref as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { path, definition } = self;
        0_u64
            .saturating_add(path.heap_bytes())
            .saturating_add(definition.heap_bytes())
    }
}

impl ModelSize for crate::text::Rotation {
    const SIZE: u64 = 8 + largest(&[<f64 as ModelSize>::SIZE, <[f64; 2] as ModelSize>::SIZE]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Angle(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Direction(f0) => 0_u64.saturating_add(f0.heap_bytes()),
        }
    }
}

impl<T: ModelSize> ModelSize for crate::text::Scaled<T> {
    const SIZE: u64 = 8 + largest(&[<T as ModelSize>::SIZE, <f64 as ModelSize>::SIZE]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Value(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Factor(f0) => 0_u64.saturating_add(f0.heap_bytes()),
        }
    }
}

impl<P: ModelSize> ModelSize for crate::shape::Shape<P> {
    const SIZE: u64 = 8 + largest(&[
        <P as ModelSize>::SIZE + <P as ModelSize>::SIZE,
        <P as ModelSize>::SIZE + <crate::space::Length as ModelSize>::SIZE,
        <crate::shape::Edge<P> as ModelSize>::SIZE,
        <crate::shape::Polyline<P> as ModelSize>::SIZE,
        <crate::text::Text<P> as ModelSize>::SIZE,
        <crate::text::FormattedText<P> as ModelSize>::SIZE,
        <crate::dimension::Dimension<P> as ModelSize>::SIZE,
        <crate::dimension::Leader<P> as ModelSize>::SIZE,
        <crate::callout::Callout<P> as ModelSize>::SIZE,
        <crate::table::Table<P> as ModelSize>::SIZE,
        <crate::underlay::Underlay<P> as ModelSize>::SIZE,
        <crate::symbol::Symbol<P> as ModelSize>::SIZE,
        <crate::hatch::Hatch<P> as ModelSize>::SIZE,
        <crate::point::Point<P> as ModelSize>::SIZE,
    ]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Line { start, end } => 0_u64
                .saturating_add(start.heap_bytes())
                .saturating_add(end.heap_bytes()),
            Self::Circle { center, radius } => 0_u64
                .saturating_add(center.heap_bytes())
                .saturating_add(radius.heap_bytes()),
            Self::Curve(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Polyline(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Text(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Formatted(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Dimension(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Leader(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Callout(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Table(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Underlay(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Symbol(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Hatch(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Point(f0) => 0_u64.saturating_add(f0.heap_bytes()),
        }
    }
}

impl ModelSize for crate::space::Sheet {
    const SIZE: u64 = <crate::SheetId as ModelSize>::SIZE
        + <Option<String> as ModelSize>::SIZE
        + <Option<[crate::space::PaperLength; 2]> as ModelSize>::SIZE
        + <Vec<crate::shape::Element<crate::space::PaperPoint>> as ModelSize>::SIZE
        + <Vec<crate::space::View> as ModelSize>::SIZE
        + <Vec<crate::space::SheetItem> as ModelSize>::SIZE
        + <zumen_base::Ref as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            id,
            name,
            size,
            elements,
            views,
            order,
            source,
        } = self;
        0_u64
            .saturating_add(id.heap_bytes())
            .saturating_add(name.heap_bytes())
            .saturating_add(size.heap_bytes())
            .saturating_add(elements.heap_bytes())
            .saturating_add(views.heap_bytes())
            .saturating_add(order.heap_bytes())
            .saturating_add(source.heap_bytes())
    }
}

impl ModelSize for crate::SheetId {
    const SIZE: u64 = <u32 as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self(f0) = self;
        0_u64.saturating_add(f0.heap_bytes())
    }
}

impl ModelSize for crate::space::SheetItem {
    const SIZE: u64 = 8 + largest(&[<usize as ModelSize>::SIZE, <usize as ModelSize>::SIZE]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Element(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::View(f0) => 0_u64.saturating_add(f0.heap_bytes()),
        }
    }
}

impl ModelSize for crate::SpaceId {
    const SIZE: u64 = <u32 as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self(f0) = self;
        0_u64.saturating_add(f0.heap_bytes())
    }
}

impl ModelSize for crate::text::Span {
    const SIZE: u64 =
        <crate::text::Piece as ModelSize>::SIZE + <crate::text::Format as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { piece, format } = self;
        0_u64
            .saturating_add(piece.heap_bytes())
            .saturating_add(format.heap_bytes())
    }
}

impl<P: ModelSize> ModelSize for crate::shape::Spline<P> {
    const SIZE: u64 = <u32 as ModelSize>::SIZE
        + <Vec<f64> as ModelSize>::SIZE
        + <Vec<P> as ModelSize>::SIZE
        + <Vec<f64> as ModelSize>::SIZE
        + <Vec<P> as ModelSize>::SIZE
        + <Option<[f64; 2]> as ModelSize>::SIZE
        + <Option<[f64; 2]> as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <Option<f64> as ModelSize>::SIZE
        + <Option<crate::shape::KnotParameterization> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            degree,
            knots,
            control,
            weights,
            fit,
            start_tangent,
            end_tangent,
            closed,
            periodic,
            fit_tolerance,
            knot_parameterization,
        } = self;
        0_u64
            .saturating_add(degree.heap_bytes())
            .saturating_add(knots.heap_bytes())
            .saturating_add(control.heap_bytes())
            .saturating_add(weights.heap_bytes())
            .saturating_add(fit.heap_bytes())
            .saturating_add(start_tangent.heap_bytes())
            .saturating_add(end_tangent.heap_bytes())
            .saturating_add(closed.heap_bytes())
            .saturating_add(periodic.heap_bytes())
            .saturating_add(fit_tolerance.heap_bytes())
            .saturating_add(knot_parameterization.heap_bytes())
    }
}

impl ModelSize for crate::text::StackKind {
    const SIZE: u64 = 8 + largest(&[0, 0, 0]);
}

impl ModelSize for crate::style::Style {
    const SIZE: u64 = <crate::style::Inherit<crate::style::Color> as ModelSize>::SIZE
        + <crate::style::Inherit<crate::style::LineTypeId> as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <Option<crate::style::Inherit<crate::style::LineWeight>> as ModelSize>::SIZE
        + <crate::style::Inherit<crate::style::Transparency> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            color,
            line_type,
            line_type_scale,
            line_weight,
            transparency,
        } = self;
        0_u64
            .saturating_add(color.heap_bytes())
            .saturating_add(line_type.heap_bytes())
            .saturating_add(line_type_scale.heap_bytes())
            .saturating_add(line_weight.heap_bytes())
            .saturating_add(transparency.heap_bytes())
    }
}

impl<P: ModelSize> ModelSize for crate::symbol::Symbol<P> {
    const SIZE: u64 = <crate::symbol::SymbolKey as ModelSize>::SIZE
        + <P as ModelSize>::SIZE
        + <[f64; 2] as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <Option<crate::symbol::Grid> as ModelSize>::SIZE
        + <Vec<crate::symbol::Attribute<P>> as ModelSize>::SIZE
        + <Option<crate::underlay::Clip> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            key,
            position,
            scale,
            rotation,
            grid,
            attributes,
            clip,
        } = self;
        0_u64
            .saturating_add(key.heap_bytes())
            .saturating_add(position.heap_bytes())
            .saturating_add(scale.heap_bytes())
            .saturating_add(rotation.heap_bytes())
            .saturating_add(grid.heap_bytes())
            .saturating_add(attributes.heap_bytes())
            .saturating_add(clip.heap_bytes())
    }
}

impl ModelSize for crate::symbol::SymbolDefinition {
    const SIZE: u64 = <crate::symbol::SymbolKey as ModelSize>::SIZE
        + <crate::space::ModelSpace as ModelSize>::SIZE
        + <Option<crate::space::ModelPoint> as ModelSize>::SIZE
        + <Vec<crate::symbol::AttributeDefinition> as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <Option<String> as ModelSize>::SIZE
        + <zumen_base::Ref as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            key,
            space,
            base,
            attributes,
            anonymous,
            external,
            source,
        } = self;
        0_u64
            .saturating_add(key.heap_bytes())
            .saturating_add(space.heap_bytes())
            .saturating_add(base.heap_bytes())
            .saturating_add(attributes.heap_bytes())
            .saturating_add(anonymous.heap_bytes())
            .saturating_add(external.heap_bytes())
            .saturating_add(source.heap_bytes())
    }
}

impl ModelSize for crate::symbol::SymbolKey {
    const SIZE: u64 =
        <std::borrow::Cow<'static, str> as ModelSize>::SIZE + <String as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { namespace, name } = self;
        0_u64
            .saturating_add(namespace.heap_bytes())
            .saturating_add(name.heap_bytes())
    }
}

impl<P: ModelSize> ModelSize for crate::table::Table<P> {
    const SIZE: u64 = <P as ModelSize>::SIZE
        + <[f64; 2] as ModelSize>::SIZE
        + <Vec<crate::space::Length> as ModelSize>::SIZE
        + <Vec<crate::space::Length> as ModelSize>::SIZE
        + <Vec<crate::table::Cell> as ModelSize>::SIZE
        + <Option<zumen_base::Ref> as ModelSize>::SIZE
        + <Option<Box<crate::table::TableFormats>> as ModelSize>::SIZE
        + <Option<crate::symbol::SymbolKey> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            position,
            direction,
            row_heights,
            column_widths,
            cells,
            style,
            formats,
            drawn,
        } = self;
        0_u64
            .saturating_add(position.heap_bytes())
            .saturating_add(direction.heap_bytes())
            .saturating_add(row_heights.heap_bytes())
            .saturating_add(column_widths.heap_bytes())
            .saturating_add(cells.heap_bytes())
            .saturating_add(style.heap_bytes())
            .saturating_add(formats.heap_bytes())
            .saturating_add(drawn.heap_bytes())
    }
}

impl ModelSize for crate::table::TableFormats {
    const SIZE: u64 = <bool as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <crate::table::CellFormat as ModelSize>::SIZE
        + <crate::table::CellFormat as ModelSize>::SIZE
        + <crate::table::CellFormat as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            title,
            header,
            title_format,
            header_format,
            data_format,
        } = self;
        0_u64
            .saturating_add(title.heap_bytes())
            .saturating_add(header.heap_bytes())
            .saturating_add(title_format.heap_bytes())
            .saturating_add(header_format.heap_bytes())
            .saturating_add(data_format.heap_bytes())
    }
}

impl ModelSize for crate::extension::Target {
    const SIZE: u64 = 8 + largest(&[
        0,
        <String as ModelSize>::SIZE,
        <crate::style::LayerId as ModelSize>::SIZE,
        <crate::style::LineTypeId as ModelSize>::SIZE,
    ]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Document => 0,
            Self::Element(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::Layer(f0) => 0_u64.saturating_add(f0.heap_bytes()),
            Self::LineType(f0) => 0_u64.saturating_add(f0.heap_bytes()),
        }
    }
}

impl<P: ModelSize> ModelSize for crate::text::Text<P> {
    const SIZE: u64 = <String as ModelSize>::SIZE
        + <crate::text::Height as ModelSize>::SIZE
        + <crate::text::Placement<P> as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <String as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            content,
            height,
            placement,
            rotation,
            width_factor,
            oblique,
            backward,
            upside_down,
            style,
        } = self;
        0_u64
            .saturating_add(content.heap_bytes())
            .saturating_add(height.heap_bytes())
            .saturating_add(placement.heap_bytes())
            .saturating_add(rotation.heap_bytes())
            .saturating_add(width_factor.heap_bytes())
            .saturating_add(oblique.heap_bytes())
            .saturating_add(backward.heap_bytes())
            .saturating_add(upside_down.heap_bytes())
            .saturating_add(style.heap_bytes())
    }
}

impl ModelSize for crate::style::TextStyle {
    const SIZE: u64 = <String as ModelSize>::SIZE
        + <Option<crate::style::TextStyleDefinition> as ModelSize>::SIZE
        + <zumen_base::Ref as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            name,
            definition,
            source,
        } = self;
        0_u64
            .saturating_add(name.heap_bytes())
            .saturating_add(definition.heap_bytes())
            .saturating_add(source.heap_bytes())
    }
}

impl ModelSize for crate::style::TextStyleDefinition {
    const SIZE: u64 = <Option<crate::text::Font> as ModelSize>::SIZE
        + <Option<crate::text::Font> as ModelSize>::SIZE
        + <Option<f64> as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <f64 as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            font,
            big_font,
            height,
            width,
            oblique,
            backward,
            upside_down,
            vertical,
        } = self;
        0_u64
            .saturating_add(font.heap_bytes())
            .saturating_add(big_font.heap_bytes())
            .saturating_add(height.heap_bytes())
            .saturating_add(width.heap_bytes())
            .saturating_add(oblique.heap_bytes())
            .saturating_add(backward.heap_bytes())
            .saturating_add(upside_down.heap_bytes())
            .saturating_add(vertical.heap_bytes())
    }
}

impl ModelSize for crate::TitleBlock {
    const SIZE: u64 =
        <Vec<crate::TitleItem> as ModelSize>::SIZE + <zumen_base::Ref as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { items, source } = self;
        0_u64
            .saturating_add(items.heap_bytes())
            .saturating_add(source.heap_bytes())
    }
}

impl ModelSize for crate::TitleItem {
    const SIZE: u64 = <Option<std::borrow::Cow<'static, str>> as ModelSize>::SIZE
        + <String as ModelSize>::SIZE
        + <String as ModelSize>::SIZE
        + <Vec<crate::extension::Target> as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            key,
            name,
            value,
            display,
        } = self;
        0_u64
            .saturating_add(key.heap_bytes())
            .saturating_add(name.heap_bytes())
            .saturating_add(value.heap_bytes())
            .saturating_add(display.heap_bytes())
    }
}

impl ModelSize for crate::style::Transparency {
    const SIZE: u64 = <u8 as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { alpha } = self;
        0_u64.saturating_add(alpha.heap_bytes())
    }
}

impl<P: ModelSize> ModelSize for crate::underlay::Underlay<P> {
    const SIZE: u64 = <crate::underlay::UnderlayKind as ModelSize>::SIZE
        + <crate::underlay::Resource as ModelSize>::SIZE
        + <Option<String> as ModelSize>::SIZE
        + <crate::underlay::UnderlayFrame<P> as ModelSize>::SIZE
        + <Option<crate::underlay::Clip> as ModelSize>::SIZE
        + <bool as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            kind,
            resource,
            page,
            frame,
            clip,
            visible,
        } = self;
        0_u64
            .saturating_add(kind.heap_bytes())
            .saturating_add(resource.heap_bytes())
            .saturating_add(page.heap_bytes())
            .saturating_add(frame.heap_bytes())
            .saturating_add(clip.heap_bytes())
            .saturating_add(visible.heap_bytes())
    }
}

impl<P: ModelSize> ModelSize for crate::underlay::UnderlayFrame<P> {
    const SIZE: u64 = 8 + largest(&[
        <[f64; 2] as ModelSize>::SIZE
            + <P as ModelSize>::SIZE
            + <P as ModelSize>::SIZE
            + <P as ModelSize>::SIZE,
        <P as ModelSize>::SIZE + <P as ModelSize>::SIZE + <P as ModelSize>::SIZE,
        <P as ModelSize>::SIZE
            + <[crate::space::Length; 2] as ModelSize>::SIZE
            + <f64 as ModelSize>::SIZE,
    ]);

    fn heap_bytes(&self) -> u64 {
        match self {
            Self::Pixels {
                size,
                origin,
                right,
                top,
            } => 0_u64
                .saturating_add(size.heap_bytes())
                .saturating_add(origin.heap_bytes())
                .saturating_add(right.heap_bytes())
                .saturating_add(top.heap_bytes()),
            Self::Corners { origin, right, top } => 0_u64
                .saturating_add(origin.heap_bytes())
                .saturating_add(right.heap_bytes())
                .saturating_add(top.heap_bytes()),
            Self::Scaled {
                origin,
                scale,
                rotation,
            } => 0_u64
                .saturating_add(origin.heap_bytes())
                .saturating_add(scale.heap_bytes())
                .saturating_add(rotation.heap_bytes()),
        }
    }
}

impl ModelSize for crate::underlay::UnderlayKind {
    const SIZE: u64 = 8 + largest(&[0, 0, 0, 0]);
}

impl ModelSize for crate::space::Unit {
    const SIZE: u64 = 8 + largest(&[0, 0, 0, 0, 0, 0]);
}

impl ModelSize for crate::table::ValueUnit {
    const SIZE: u64 = 8 + largest(&[0, 0, 0, 0, 0, 0, 0]);
}

impl<P: ModelSize> ModelSize for crate::shape::Vertex<P> {
    const SIZE: u64 = <P as ModelSize>::SIZE + <f64 as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self { point, bulge } = self;
        0_u64
            .saturating_add(point.heap_bytes())
            .saturating_add(bulge.heap_bytes())
    }
}

impl ModelSize for crate::text::Vertical {
    const SIZE: u64 = 8 + largest(&[0, 0, 0, 0]);
}

impl ModelSize for crate::space::View {
    const SIZE: u64 = <crate::SpaceId as ModelSize>::SIZE
        + <crate::SheetId as ModelSize>::SIZE
        + <crate::space::Affine as ModelSize>::SIZE
        + <Vec<crate::space::PaperPoint> as ModelSize>::SIZE
        + <Vec<crate::style::LayerId> as ModelSize>::SIZE
        + <zumen_base::Ref as ModelSize>::SIZE;

    fn heap_bytes(&self) -> u64 {
        let Self {
            space,
            sheet,
            transform,
            clip,
            hidden_layers,
            source,
        } = self;
        0_u64
            .saturating_add(space.heap_bytes())
            .saturating_add(sheet.heap_bytes())
            .saturating_add(transform.heap_bytes())
            .saturating_add(clip.heap_bytes())
            .saturating_add(hidden_layers.heap_bytes())
            .saturating_add(source.heap_bytes())
    }
}
