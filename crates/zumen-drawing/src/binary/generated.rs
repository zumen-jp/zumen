// 生成したファイル。手で直さない。`cargo xtask codegen` が、図面モデルの型と schema/drawing.toml から作る（決定 0246）。

use super::{Input, Pack, Unpack, Writer};

/// 大版。読み手は、同じ大版のバイナリだけを読む
pub const MAJOR: u16 = 1;

/// 小版。型の契約に型・欄・値を足すたびに上げる
pub const MINOR: u16 = 0;

impl Pack for crate::space::Affine {
    fn pack(&self, out: &mut Writer) {
        let Self { m } = self;
        out.record(|out| {
            m.pack(out);
        });
    }
}

impl Unpack for crate::space::Affine {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Affine", |input| {
            Ok(Self {
                m: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::style::Arrowhead {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Default => out.uint(0),
            Self::Symbol(f0) => {
                out.uint(1);
                f0.pack(out);
            }
        }
    }
}

impl Unpack for crate::style::Arrowhead {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Arrowhead", 2)? {
            0 => Self::Default,
            _ => Self::Symbol(Unpack::unpack(input)?),
        })
    }
}

impl<P: Pack> Pack for crate::symbol::Attribute<P> {
    fn pack(&self, out: &mut Writer) {
        let Self {
            tag,
            text,
            formatted,
            hidden,
            layer,
            style,
            source,
        } = self;
        out.record(|out| {
            tag.pack(out);
            text.pack(out);
            formatted.pack(out);
            hidden.pack(out);
            layer.pack(out);
            style.pack(out);
            source.pack(out);
        });
    }
}

impl<P: Unpack> Unpack for crate::symbol::Attribute<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Attribute", |input| {
            Ok(Self {
                tag: Unpack::unpack(input)?,
                text: Unpack::unpack(input)?,
                formatted: Unpack::unpack(input)?,
                hidden: Unpack::unpack(input)?,
                layer: Unpack::unpack(input)?,
                style: Unpack::unpack(input)?,
                source: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::symbol::AttributeDefinition {
    fn pack(&self, out: &mut Writer) {
        let Self {
            attribute,
            prompt,
            constant,
            verify,
            preset,
        } = self;
        out.record(|out| {
            attribute.pack(out);
            prompt.pack(out);
            constant.pack(out);
            verify.pack(out);
            preset.pack(out);
        });
    }
}

impl Unpack for crate::symbol::AttributeDefinition {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("AttributeDefinition", |input| {
            Ok(Self {
                attribute: Unpack::unpack(input)?,
                prompt: Unpack::unpack(input)?,
                constant: Unpack::unpack(input)?,
                verify: Unpack::unpack(input)?,
                preset: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::callout::AttributeValue {
    fn pack(&self, out: &mut Writer) {
        let Self { definition, text } = self;
        out.record(|out| {
            definition.pack(out);
            text.pack(out);
        });
    }
}

impl Unpack for crate::callout::AttributeValue {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("AttributeValue", |input| {
            Ok(Self {
                definition: Unpack::unpack(input)?,
                text: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for zumen_crs::Axes {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::EastNorth => out.uint(0),
            Self::NorthEast => out.uint(1),
        }
    }
}

impl Unpack for zumen_crs::Axes {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Axes", 2)? {
            0 => Self::EastNorth,
            _ => Self::NorthEast,
        })
    }
}

impl Pack for crate::dimension::Axis {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::X => out.uint(0),
            Self::Y => out.uint(1),
        }
    }
}

impl Unpack for crate::dimension::Axis {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Axis", 2)? {
            0 => Self::X,
            _ => Self::Y,
        })
    }
}

impl Pack for crate::text::Background {
    fn pack(&self, out: &mut Writer) {
        let Self {
            fill,
            frame,
            scale,
            transparency,
        } = self;
        out.record(|out| {
            fill.pack(out);
            frame.pack(out);
            scale.pack(out);
            transparency.pack(out);
        });
    }
}

impl Unpack for crate::text::Background {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Background", |input| {
            Ok(Self {
                fill: Unpack::unpack(input)?,
                frame: Unpack::unpack(input)?,
                scale: Unpack::unpack(input)?,
                transparency: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::text::BackgroundFill {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Color(f0) => {
                out.uint(0);
                f0.pack(out);
            }
            Self::Screen => out.uint(1),
        }
    }
}

impl Unpack for crate::text::BackgroundFill {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("BackgroundFill", 2)? {
            0 => Self::Color(Unpack::unpack(input)?),
            _ => Self::Screen,
        })
    }
}

impl Pack for crate::extension::Basis {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Document => out.uint(0),
            Self::Hash(f0) => {
                out.uint(1);
                f0.pack(out);
            }
        }
    }
}

impl Unpack for crate::extension::Basis {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Basis", 2)? {
            0 => Self::Document,
            _ => Self::Hash(Unpack::unpack(input)?),
        })
    }
}

impl Pack for crate::table::Border {
    fn pack(&self, out: &mut Writer) {
        let Self {
            color,
            weight,
            visible,
        } = self;
        out.record(|out| {
            color.pack(out);
            weight.pack(out);
            visible.pack(out);
        });
    }
}

impl Unpack for crate::table::Border {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Border", |input| {
            Ok(Self {
                color: Unpack::unpack(input)?,
                weight: Unpack::unpack(input)?,
                visible: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::table::Borders {
    fn pack(&self, out: &mut Writer) {
        let Self {
            top,
            right,
            bottom,
            left,
            horizontal,
            vertical,
        } = self;
        out.record(|out| {
            top.pack(out);
            right.pack(out);
            bottom.pack(out);
            left.pack(out);
            horizontal.pack(out);
            vertical.pack(out);
        });
    }
}

impl Unpack for crate::table::Borders {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Borders", |input| {
            Ok(Self {
                top: Unpack::unpack(input)?,
                right: Unpack::unpack(input)?,
                bottom: Unpack::unpack(input)?,
                left: Unpack::unpack(input)?,
                horizontal: Unpack::unpack(input)?,
                vertical: Unpack::unpack(input)?,
            })
        })
    }
}

impl<P: Pack> Pack for crate::hatch::Boundary<P> {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Polyline { vertices, closed } => {
                out.uint(0);
                out.record(|out| {
                    vertices.pack(out);
                    closed.pack(out);
                });
            }
            Self::Edges(f0) => {
                out.uint(1);
                f0.pack(out);
            }
        }
    }
}

impl<P: Unpack> Unpack for crate::hatch::Boundary<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Boundary", 2)? {
            0 => input.record("Boundary", |input| {
                Ok(Self::Polyline {
                    vertices: Unpack::unpack(input)?,
                    closed: Unpack::unpack(input)?,
                })
            })?,
            _ => Self::Edges(Unpack::unpack(input)?),
        })
    }
}

impl<P: Pack> Pack for crate::callout::Branch<P> {
    fn pack(&self, out: &mut Writer) {
        let Self { lines, end, dogleg } = self;
        out.record(|out| {
            lines.pack(out);
            end.pack(out);
            dogleg.pack(out);
        });
    }
}

impl<P: Unpack> Unpack for crate::callout::Branch<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Branch", |input| {
            Ok(Self {
                lines: Unpack::unpack(input)?,
                end: Unpack::unpack(input)?,
                dogleg: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for zumen_base::ByteRange {
    fn pack(&self, out: &mut Writer) {
        let Self { start, end } = self;
        out.record(|out| {
            start.pack(out);
            end.pack(out);
        });
    }
}

impl Unpack for zumen_base::ByteRange {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("ByteRange", |input| {
            Ok(Self {
                start: Unpack::unpack(input)?,
                end: Unpack::unpack(input)?,
            })
        })
    }
}

impl<P: Pack> Pack for crate::callout::Callout<P> {
    fn pack(&self, out: &mut Writer) {
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
        out.record(|out| {
            branches.pack(out);
            content.pack(out);
            path.pack(out);
            arrowhead.pack(out);
            arrow_size.pack(out);
            landing.pack(out);
            scale.pack(out);
            line_style.pack(out);
            content_color.pack(out);
            style.pack(out);
        });
    }
}

impl<P: Unpack> Unpack for crate::callout::Callout<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Callout", |input| {
            Ok(Self {
                branches: Unpack::unpack(input)?,
                content: Unpack::unpack(input)?,
                path: Unpack::unpack(input)?,
                arrowhead: Unpack::unpack(input)?,
                arrow_size: Unpack::unpack(input)?,
                landing: Unpack::unpack(input)?,
                scale: Unpack::unpack(input)?,
                line_style: Unpack::unpack(input)?,
                content_color: Unpack::unpack(input)?,
                style: Unpack::unpack(input)?,
            })
        })
    }
}

impl<P: Pack> Pack for crate::callout::CalloutContent<P> {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::None => out.uint(0),
            Self::Text(f0) => {
                out.uint(1);
                f0.pack(out);
            }
            Self::Symbol { symbol, values } => {
                out.uint(2);
                out.record(|out| {
                    symbol.pack(out);
                    values.pack(out);
                });
            }
        }
    }
}

impl<P: Unpack> Unpack for crate::callout::CalloutContent<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("CalloutContent", 3)? {
            0 => Self::None,
            1 => Self::Text(Unpack::unpack(input)?),
            _ => input.record("CalloutContent", |input| {
                Ok(Self::Symbol {
                    symbol: Unpack::unpack(input)?,
                    values: Unpack::unpack(input)?,
                })
            })?,
        })
    }
}

impl Pack for crate::table::Cell {
    fn pack(&self, out: &mut Writer) {
        let Self {
            content,
            span,
            covered,
            value,
            rotation,
            format,
        } = self;
        out.record(|out| {
            content.pack(out);
            span.pack(out);
            covered.pack(out);
            value.pack(out);
            rotation.pack(out);
            format.pack(out);
        });
    }
}

impl Unpack for crate::table::Cell {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Cell", |input| {
            Ok(Self {
                content: Unpack::unpack(input)?,
                span: Unpack::unpack(input)?,
                covered: Unpack::unpack(input)?,
                value: Unpack::unpack(input)?,
                rotation: Unpack::unpack(input)?,
                format: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::table::CellContent {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Empty => out.uint(0),
            Self::Text { content, spans } => {
                out.uint(1);
                out.record(|out| {
                    content.pack(out);
                    spans.pack(out);
                });
            }
            Self::Symbol(f0) => {
                out.uint(2);
                f0.pack(out);
            }
        }
    }
}

impl Unpack for crate::table::CellContent {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("CellContent", 3)? {
            0 => Self::Empty,
            1 => input.record("CellContent", |input| {
                Ok(Self::Text {
                    content: Unpack::unpack(input)?,
                    spans: Unpack::unpack(input)?,
                })
            })?,
            _ => Self::Symbol(Unpack::unpack(input)?),
        })
    }
}

impl Pack for crate::table::CellData {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Text(f0) => {
                out.uint(0);
                f0.pack(out);
            }
            Self::Integer(f0) => {
                out.uint(1);
                f0.pack(out);
            }
            Self::Real(f0) => {
                out.uint(2);
                f0.pack(out);
            }
            Self::Date(f0) => {
                out.uint(3);
                f0.pack(out);
            }
            Self::Point(f0) => {
                out.uint(4);
                f0.pack(out);
            }
        }
    }
}

impl Unpack for crate::table::CellData {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("CellData", 5)? {
            0 => Self::Text(Unpack::unpack(input)?),
            1 => Self::Integer(Unpack::unpack(input)?),
            2 => Self::Real(Unpack::unpack(input)?),
            3 => Self::Date(Unpack::unpack(input)?),
            _ => Self::Point(Unpack::unpack(input)?),
        })
    }
}

impl Pack for crate::table::CellFormat {
    fn pack(&self, out: &mut Writer) {
        let Self {
            text_style,
            text_height,
            alignment,
            text_color,
            fill,
            fill_color,
            borders,
        } = self;
        out.record(|out| {
            text_style.pack(out);
            text_height.pack(out);
            alignment.pack(out);
            text_color.pack(out);
            fill.pack(out);
            fill_color.pack(out);
            borders.pack(out);
        });
    }
}

impl Unpack for crate::table::CellFormat {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("CellFormat", |input| {
            Ok(Self {
                text_style: Unpack::unpack(input)?,
                text_height: Unpack::unpack(input)?,
                alignment: Unpack::unpack(input)?,
                text_color: Unpack::unpack(input)?,
                fill: Unpack::unpack(input)?,
                fill_color: Unpack::unpack(input)?,
                borders: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::table::CellValue {
    fn pack(&self, out: &mut Writer) {
        let Self { data, unit, format } = self;
        out.record(|out| {
            data.pack(out);
            unit.pack(out);
            format.pack(out);
        });
    }
}

impl Unpack for crate::table::CellValue {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("CellValue", |input| {
            Ok(Self {
                data: Unpack::unpack(input)?,
                unit: Unpack::unpack(input)?,
                format: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::underlay::Clip {
    fn pack(&self, out: &mut Writer) {
        let Self { vertices, inverted } = self;
        out.record(|out| {
            vertices.pack(out);
            inverted.pack(out);
        });
    }
}

impl Unpack for crate::underlay::Clip {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Clip", |input| {
            Ok(Self {
                vertices: Unpack::unpack(input)?,
                inverted: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::style::Color {
    fn pack(&self, out: &mut Writer) {
        let Self { code, rgb } = self;
        out.record(|out| {
            code.pack(out);
            rgb.pack(out);
        });
    }
}

impl Unpack for crate::style::Color {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Color", |input| {
            Ok(Self {
                code: Unpack::unpack(input)?,
                rgb: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::style::ColorCode {
    fn pack(&self, out: &mut Writer) {
        let Self { system, value } = self;
        out.record(|out| {
            system.pack(out);
            value.pack(out);
        });
    }
}

impl Unpack for crate::style::ColorCode {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("ColorCode", |input| {
            Ok(Self {
                system: Unpack::unpack(input)?,
                value: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::style::ColorSystem {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Aci => out.uint(0),
            Self::Sxf => out.uint(1),
            Self::Jww => out.uint(2),
        }
    }
}

impl Unpack for crate::style::ColorSystem {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("ColorSystem", 3)? {
            0 => Self::Aci,
            1 => Self::Sxf,
            _ => Self::Jww,
        })
    }
}

impl Pack for crate::table::DateTime {
    fn pack(&self, out: &mut Writer) {
        let Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            millisecond,
        } = self;
        out.record(|out| {
            year.pack(out);
            month.pack(out);
            day.pack(out);
            hour.pack(out);
            minute.pack(out);
            second.pack(out);
            millisecond.pack(out);
        });
    }
}

impl Unpack for crate::table::DateTime {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("DateTime", |input| {
            Ok(Self {
                year: Unpack::unpack(input)?,
                month: Unpack::unpack(input)?,
                day: Unpack::unpack(input)?,
                hour: Unpack::unpack(input)?,
                minute: Unpack::unpack(input)?,
                second: Unpack::unpack(input)?,
                millisecond: Unpack::unpack(input)?,
            })
        })
    }
}

impl<P: Pack> Pack for crate::dimension::Dimension<P> {
    fn pack(&self, out: &mut Writer) {
        let Self {
            kind,
            measurement,
            label,
            text,
            style,
            block,
        } = self;
        out.record(|out| {
            kind.pack(out);
            measurement.pack(out);
            label.pack(out);
            text.pack(out);
            style.pack(out);
            block.pack(out);
        });
    }
}

impl<P: Unpack> Unpack for crate::dimension::Dimension<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Dimension", |input| {
            Ok(Self {
                kind: Unpack::unpack(input)?,
                measurement: Unpack::unpack(input)?,
                label: Unpack::unpack(input)?,
                text: Unpack::unpack(input)?,
                style: Unpack::unpack(input)?,
                block: Unpack::unpack(input)?,
            })
        })
    }
}

impl<P: Pack> Pack for crate::dimension::DimensionKind<P> {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Linear {
                first,
                second,
                line,
                rotation,
                oblique,
            } => {
                out.uint(0);
                out.record(|out| {
                    first.pack(out);
                    second.pack(out);
                    line.pack(out);
                    rotation.pack(out);
                    oblique.pack(out);
                });
            }
            Self::Aligned {
                first,
                second,
                line,
                oblique,
            } => {
                out.uint(1);
                out.record(|out| {
                    first.pack(out);
                    second.pack(out);
                    line.pack(out);
                    oblique.pack(out);
                });
            }
            Self::Angular {
                vertex,
                first,
                second,
                arc,
            } => {
                out.uint(2);
                out.record(|out| {
                    vertex.pack(out);
                    first.pack(out);
                    second.pack(out);
                    arc.pack(out);
                });
            }
            Self::AngularLines { first, second, arc } => {
                out.uint(3);
                out.record(|out| {
                    first.pack(out);
                    second.pack(out);
                    arc.pack(out);
                });
            }
            Self::ArcLength {
                center,
                first,
                second,
                arc,
            } => {
                out.uint(4);
                out.record(|out| {
                    center.pack(out);
                    first.pack(out);
                    second.pack(out);
                    arc.pack(out);
                });
            }
            Self::Radius {
                center,
                chord,
                leader,
            } => {
                out.uint(5);
                out.record(|out| {
                    center.pack(out);
                    chord.pack(out);
                    leader.pack(out);
                });
            }
            Self::Diameter {
                chord,
                opposite,
                leader,
            } => {
                out.uint(6);
                out.record(|out| {
                    chord.pack(out);
                    opposite.pack(out);
                    leader.pack(out);
                });
            }
            Self::Ordinate {
                origin,
                feature,
                leader_end,
                axis,
            } => {
                out.uint(7);
                out.record(|out| {
                    origin.pack(out);
                    feature.pack(out);
                    leader_end.pack(out);
                    axis.pack(out);
                });
            }
        }
    }
}

impl<P: Unpack> Unpack for crate::dimension::DimensionKind<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("DimensionKind", 8)? {
            0 => input.record("DimensionKind", |input| {
                Ok(Self::Linear {
                    first: Unpack::unpack(input)?,
                    second: Unpack::unpack(input)?,
                    line: Unpack::unpack(input)?,
                    rotation: Unpack::unpack(input)?,
                    oblique: Unpack::unpack(input)?,
                })
            })?,
            1 => input.record("DimensionKind", |input| {
                Ok(Self::Aligned {
                    first: Unpack::unpack(input)?,
                    second: Unpack::unpack(input)?,
                    line: Unpack::unpack(input)?,
                    oblique: Unpack::unpack(input)?,
                })
            })?,
            2 => input.record("DimensionKind", |input| {
                Ok(Self::Angular {
                    vertex: Unpack::unpack(input)?,
                    first: Unpack::unpack(input)?,
                    second: Unpack::unpack(input)?,
                    arc: Unpack::unpack(input)?,
                })
            })?,
            3 => input.record("DimensionKind", |input| {
                Ok(Self::AngularLines {
                    first: Unpack::unpack(input)?,
                    second: Unpack::unpack(input)?,
                    arc: Unpack::unpack(input)?,
                })
            })?,
            4 => input.record("DimensionKind", |input| {
                Ok(Self::ArcLength {
                    center: Unpack::unpack(input)?,
                    first: Unpack::unpack(input)?,
                    second: Unpack::unpack(input)?,
                    arc: Unpack::unpack(input)?,
                })
            })?,
            5 => input.record("DimensionKind", |input| {
                Ok(Self::Radius {
                    center: Unpack::unpack(input)?,
                    chord: Unpack::unpack(input)?,
                    leader: Unpack::unpack(input)?,
                })
            })?,
            6 => input.record("DimensionKind", |input| {
                Ok(Self::Diameter {
                    chord: Unpack::unpack(input)?,
                    opposite: Unpack::unpack(input)?,
                    leader: Unpack::unpack(input)?,
                })
            })?,
            _ => input.record("DimensionKind", |input| {
                Ok(Self::Ordinate {
                    origin: Unpack::unpack(input)?,
                    feature: Unpack::unpack(input)?,
                    leader_end: Unpack::unpack(input)?,
                    axis: Unpack::unpack(input)?,
                })
            })?,
        })
    }
}

impl Pack for crate::style::DimensionStyle {
    fn pack(&self, out: &mut Writer) {
        let Self {
            name,
            ends,
            leader,
            size,
            tick,
            scale,
            source,
        } = self;
        out.record(|out| {
            name.pack(out);
            ends.pack(out);
            leader.pack(out);
            size.pack(out);
            tick.pack(out);
            scale.pack(out);
            source.pack(out);
        });
    }
}

impl Unpack for crate::style::DimensionStyle {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("DimensionStyle", |input| {
            Ok(Self {
                name: Unpack::unpack(input)?,
                ends: Unpack::unpack(input)?,
                leader: Unpack::unpack(input)?,
                size: Unpack::unpack(input)?,
                tick: Unpack::unpack(input)?,
                scale: Unpack::unpack(input)?,
                source: Unpack::unpack(input)?,
            })
        })
    }
}

impl<P: Pack> Pack for crate::dimension::DimensionText<P> {
    fn pack(&self, out: &mut Writer) {
        let Self {
            position,
            moved,
            rotation,
            attachment,
        } = self;
        out.record(|out| {
            position.pack(out);
            moved.pack(out);
            rotation.pack(out);
            attachment.pack(out);
        });
    }
}

impl<P: Unpack> Unpack for crate::dimension::DimensionText<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("DimensionText", |input| {
            Ok(Self {
                position: Unpack::unpack(input)?,
                moved: Unpack::unpack(input)?,
                rotation: Unpack::unpack(input)?,
                attachment: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::callout::Dogleg {
    fn pack(&self, out: &mut Writer) {
        let Self { direction, length } = self;
        out.record(|out| {
            direction.pack(out);
            length.pack(out);
        });
    }
}

impl Unpack for crate::callout::Dogleg {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Dogleg", |input| {
            Ok(Self {
                direction: Unpack::unpack(input)?,
                length: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::Drawing2D {
    fn pack(&self, out: &mut Writer) {
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
        } = self;
        out.record(|out| {
            spaces.pack(out);
            sheets.pack(out);
            layers.pack(out);
            line_types.pack(out);
            line_type_scale.pack(out);
            default_line_weight.pack(out);
            default_color.pack(out);
            default_line_type.pack(out);
            dimension_styles.pack(out);
            text_styles.pack(out);
            symbols.pack(out);
            extensions.pack(out);
            title_blocks.pack(out);
            point_style.pack(out);
        });
    }
}

impl Unpack for crate::Drawing2D {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Drawing2D", |input| {
            Ok(Self {
                spaces: Unpack::unpack(input)?,
                sheets: Unpack::unpack(input)?,
                layers: Unpack::unpack(input)?,
                line_types: Unpack::unpack(input)?,
                line_type_scale: Unpack::unpack(input)?,
                default_line_weight: Unpack::unpack(input)?,
                default_color: Unpack::unpack(input)?,
                default_line_type: Unpack::unpack(input)?,
                dimension_styles: Unpack::unpack(input)?,
                text_styles: Unpack::unpack(input)?,
                symbols: Unpack::unpack(input)?,
                extensions: Unpack::unpack(input)?,
                title_blocks: Unpack::unpack(input)?,
                point_style: Unpack::unpack(input)?,
            })
        })
    }
}

impl<P: Pack> Pack for crate::shape::Edge<P> {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Line { start, end } => {
                out.uint(0);
                out.record(|out| {
                    start.pack(out);
                    end.pack(out);
                });
            }
            Self::Arc {
                center,
                radius,
                start,
                end,
                counterclockwise,
            } => {
                out.uint(1);
                out.record(|out| {
                    center.pack(out);
                    radius.pack(out);
                    start.pack(out);
                    end.pack(out);
                    counterclockwise.pack(out);
                });
            }
            Self::Elliptic {
                center,
                major,
                ratio,
                start,
                end,
                counterclockwise,
            } => {
                out.uint(2);
                out.record(|out| {
                    center.pack(out);
                    major.pack(out);
                    ratio.pack(out);
                    start.pack(out);
                    end.pack(out);
                    counterclockwise.pack(out);
                });
            }
            Self::Spline(f0) => {
                out.uint(3);
                f0.pack(out);
            }
            Self::Clothoid {
                origin,
                parameter,
                angle,
                counterclockwise,
                start,
                end,
            } => {
                out.uint(4);
                out.record(|out| {
                    origin.pack(out);
                    parameter.pack(out);
                    angle.pack(out);
                    counterclockwise.pack(out);
                    start.pack(out);
                    end.pack(out);
                });
            }
        }
    }
}

impl<P: Unpack> Unpack for crate::shape::Edge<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Edge", 5)? {
            0 => input.record("Edge", |input| {
                Ok(Self::Line {
                    start: Unpack::unpack(input)?,
                    end: Unpack::unpack(input)?,
                })
            })?,
            1 => input.record("Edge", |input| {
                Ok(Self::Arc {
                    center: Unpack::unpack(input)?,
                    radius: Unpack::unpack(input)?,
                    start: Unpack::unpack(input)?,
                    end: Unpack::unpack(input)?,
                    counterclockwise: Unpack::unpack(input)?,
                })
            })?,
            2 => input.record("Edge", |input| {
                Ok(Self::Elliptic {
                    center: Unpack::unpack(input)?,
                    major: Unpack::unpack(input)?,
                    ratio: Unpack::unpack(input)?,
                    start: Unpack::unpack(input)?,
                    end: Unpack::unpack(input)?,
                    counterclockwise: Unpack::unpack(input)?,
                })
            })?,
            3 => Self::Spline(Unpack::unpack(input)?),
            _ => input.record("Edge", |input| {
                Ok(Self::Clothoid {
                    origin: Unpack::unpack(input)?,
                    parameter: Unpack::unpack(input)?,
                    angle: Unpack::unpack(input)?,
                    counterclockwise: Unpack::unpack(input)?,
                    start: Unpack::unpack(input)?,
                    end: Unpack::unpack(input)?,
                })
            })?,
        })
    }
}

impl<P: Pack> Pack for crate::shape::Element<P> {
    fn pack(&self, out: &mut Writer) {
        out.element();
        let Self {
            source,
            layer,
            style,
            shape,
        } = self;
        out.record(|out| {
            source.pack(out);
            layer.pack(out);
            style.pack(out);
            shape.pack(out);
        });
    }
}

impl<P: Unpack> Unpack for crate::shape::Element<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.element()?;
        input.record("Element", |input| {
            Ok(Self {
                source: Unpack::unpack(input)?,
                layer: Unpack::unpack(input)?,
                style: Unpack::unpack(input)?,
                shape: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::extension::Extension {
    fn pack(&self, out: &mut Writer) {
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
        out.record(|out| {
            namespace.pack(out);
            name.pack(out);
            version.pack(out);
            schema.pack(out);
            target.pack(out);
            basis.pack(out);
            fields.pack(out);
            source.pack(out);
        });
    }
}

impl Unpack for crate::extension::Extension {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Extension", |input| {
            Ok(Self {
                namespace: Unpack::unpack(input)?,
                name: Unpack::unpack(input)?,
                version: Unpack::unpack(input)?,
                schema: Unpack::unpack(input)?,
                target: Unpack::unpack(input)?,
                basis: Unpack::unpack(input)?,
                fields: Unpack::unpack(input)?,
                source: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::extension::Field {
    fn pack(&self, out: &mut Writer) {
        let Self { key, code, value } = self;
        out.record(|out| {
            key.pack(out);
            code.pack(out);
            value.pack(out);
        });
    }
}

impl Unpack for crate::extension::Field {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Field", |input| {
            Ok(Self {
                key: Unpack::unpack(input)?,
                code: Unpack::unpack(input)?,
                value: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::extension::FieldValue {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Text(f0) => {
                out.uint(0);
                f0.pack(out);
            }
            Self::Integer(f0) => {
                out.uint(1);
                f0.pack(out);
            }
            Self::Real(f0) => {
                out.uint(2);
                f0.pack(out);
            }
            Self::Point(f0) => {
                out.uint(3);
                f0.pack(out);
            }
            Self::Handle(f0) => {
                out.uint(4);
                f0.pack(out);
            }
            Self::Bytes(f0) => {
                out.uint(5);
                f0.pack(out);
            }
        }
    }
}

impl Unpack for crate::extension::FieldValue {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("FieldValue", 6)? {
            0 => Self::Text(Unpack::unpack(input)?),
            1 => Self::Integer(Unpack::unpack(input)?),
            2 => Self::Real(Unpack::unpack(input)?),
            3 => Self::Point(Unpack::unpack(input)?),
            4 => Self::Handle(Unpack::unpack(input)?),
            _ => Self::Bytes(Unpack::unpack(input)?),
        })
    }
}

impl Pack for crate::hatch::Fill {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Solid => out.uint(0),
            Self::Pattern(f0) => {
                out.uint(1);
                f0.pack(out);
            }
            Self::Gradient(f0) => {
                out.uint(2);
                f0.pack(out);
            }
        }
    }
}

impl Unpack for crate::hatch::Fill {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Fill", 3)? {
            0 => Self::Solid,
            1 => Self::Pattern(Unpack::unpack(input)?),
            _ => Self::Gradient(Unpack::unpack(input)?),
        })
    }
}

impl Pack for crate::text::Fit {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Aligned => out.uint(0),
            Self::Fit => out.uint(1),
        }
    }
}

impl Unpack for crate::text::Fit {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Fit", 2)? {
            0 => Self::Aligned,
            _ => Self::Fit,
        })
    }
}

impl Pack for crate::text::Flow {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::LeftToRight => out.uint(0),
            Self::TopToBottom => out.uint(1),
            Self::ByStyle => out.uint(2),
        }
    }
}

impl Unpack for crate::text::Flow {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Flow", 3)? {
            0 => Self::LeftToRight,
            1 => Self::TopToBottom,
            _ => Self::ByStyle,
        })
    }
}

impl Pack for crate::text::Font {
    fn pack(&self, out: &mut Writer) {
        let Self { name, kind } = self;
        out.record(|out| {
            name.pack(out);
            kind.pack(out);
        });
    }
}

impl Unpack for crate::text::Font {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Font", |input| {
            Ok(Self {
                name: Unpack::unpack(input)?,
                kind: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::text::FontKind {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Shape => out.uint(0),
            Self::Named {
                bold,
                italic,
                charset,
                pitch,
            } => {
                out.uint(1);
                out.record(|out| {
                    bold.pack(out);
                    italic.pack(out);
                    charset.pack(out);
                    pitch.pack(out);
                });
            }
        }
    }
}

impl Unpack for crate::text::FontKind {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("FontKind", 2)? {
            0 => Self::Shape,
            _ => input.record("FontKind", |input| {
                Ok(Self::Named {
                    bold: Unpack::unpack(input)?,
                    italic: Unpack::unpack(input)?,
                    charset: Unpack::unpack(input)?,
                    pitch: Unpack::unpack(input)?,
                })
            })?,
        })
    }
}

impl Pack for crate::text::Format {
    fn pack(&self, out: &mut Writer) {
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
        out.record(|out| {
            height.pack(out);
            width_factor.pack(out);
            tracking.pack(out);
            oblique.pack(out);
            font.pack(out);
            color.pack(out);
            align.pack(out);
            underline.pack(out);
            overline.pack(out);
            strike.pack(out);
        });
    }
}

impl Unpack for crate::text::Format {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Format", |input| {
            Ok(Self {
                height: Unpack::unpack(input)?,
                width_factor: Unpack::unpack(input)?,
                tracking: Unpack::unpack(input)?,
                oblique: Unpack::unpack(input)?,
                font: Unpack::unpack(input)?,
                color: Unpack::unpack(input)?,
                align: Unpack::unpack(input)?,
                underline: Unpack::unpack(input)?,
                overline: Unpack::unpack(input)?,
                strike: Unpack::unpack(input)?,
            })
        })
    }
}

impl<P: Pack> Pack for crate::text::FormattedText<P> {
    fn pack(&self, out: &mut Writer) {
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
        out.record(|out| {
            content.pack(out);
            spans.pack(out);
            height.pack(out);
            point.pack(out);
            horizontal.pack(out);
            vertical.pack(out);
            rotation.pack(out);
            width.pack(out);
            frame_height.pack(out);
            flow.pack(out);
            line_spacing.pack(out);
            background.pack(out);
            style.pack(out);
        });
    }
}

impl<P: Unpack> Unpack for crate::text::FormattedText<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("FormattedText", |input| {
            Ok(Self {
                content: Unpack::unpack(input)?,
                spans: Unpack::unpack(input)?,
                height: Unpack::unpack(input)?,
                point: Unpack::unpack(input)?,
                horizontal: Unpack::unpack(input)?,
                vertical: Unpack::unpack(input)?,
                rotation: Unpack::unpack(input)?,
                width: Unpack::unpack(input)?,
                frame_height: Unpack::unpack(input)?,
                flow: Unpack::unpack(input)?,
                line_spacing: Unpack::unpack(input)?,
                background: Unpack::unpack(input)?,
                style: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::hatch::Gradient {
    fn pack(&self, out: &mut Writer) {
        let Self {
            name,
            angle,
            shift,
            colors,
            tint,
        } = self;
        out.record(|out| {
            name.pack(out);
            angle.pack(out);
            shift.pack(out);
            colors.pack(out);
            tint.pack(out);
        });
    }
}

impl Unpack for crate::hatch::Gradient {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Gradient", |input| {
            Ok(Self {
                name: Unpack::unpack(input)?,
                angle: Unpack::unpack(input)?,
                shift: Unpack::unpack(input)?,
                colors: Unpack::unpack(input)?,
                tint: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::symbol::Grid {
    fn pack(&self, out: &mut Writer) {
        let Self {
            columns,
            rows,
            column_spacing,
            row_spacing,
        } = self;
        out.record(|out| {
            columns.pack(out);
            rows.pack(out);
            column_spacing.pack(out);
            row_spacing.pack(out);
        });
    }
}

impl Unpack for crate::symbol::Grid {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Grid", |input| {
            Ok(Self {
                columns: Unpack::unpack(input)?,
                rows: Unpack::unpack(input)?,
                column_spacing: Unpack::unpack(input)?,
                row_spacing: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::hash::Hash {
    fn pack(&self, out: &mut Writer) {
        let Self(f0) = self;
        f0.pack(out);
    }
}

impl Unpack for crate::hash::Hash {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(Self(Unpack::unpack(input)?))
    }
}

impl<P: Pack> Pack for crate::hatch::Hatch<P> {
    fn pack(&self, out: &mut Writer) {
        let Self {
            loops,
            islands,
            fill,
            associative,
            seeds,
        } = self;
        out.record(|out| {
            loops.pack(out);
            islands.pack(out);
            fill.pack(out);
            associative.pack(out);
            seeds.pack(out);
        });
    }
}

impl<P: Unpack> Unpack for crate::hatch::Hatch<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Hatch", |input| {
            Ok(Self {
                loops: Unpack::unpack(input)?,
                islands: Unpack::unpack(input)?,
                fill: Unpack::unpack(input)?,
                associative: Unpack::unpack(input)?,
                seeds: Unpack::unpack(input)?,
            })
        })
    }
}

impl<P: Pack> Pack for crate::hatch::HatchLoop<P> {
    fn pack(&self, out: &mut Writer) {
        let Self {
            boundary,
            external,
            outermost,
            derived,
            text_box,
            sources,
        } = self;
        out.record(|out| {
            boundary.pack(out);
            external.pack(out);
            outermost.pack(out);
            derived.pack(out);
            text_box.pack(out);
            sources.pack(out);
        });
    }
}

impl<P: Unpack> Unpack for crate::hatch::HatchLoop<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("HatchLoop", |input| {
            Ok(Self {
                boundary: Unpack::unpack(input)?,
                external: Unpack::unpack(input)?,
                outermost: Unpack::unpack(input)?,
                derived: Unpack::unpack(input)?,
                text_box: Unpack::unpack(input)?,
                sources: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::text::Height {
    fn pack(&self, out: &mut Writer) {
        let Self { value, meaning } = self;
        out.record(|out| {
            value.pack(out);
            meaning.pack(out);
        });
    }
}

impl Unpack for crate::text::Height {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Height", |input| {
            Ok(Self {
                value: Unpack::unpack(input)?,
                meaning: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::text::HeightMeaning {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Frame => out.uint(0),
            Self::CapHeight => out.uint(1),
            Self::Em => out.uint(2),
            Self::Declared(f0) => {
                out.uint(3);
                f0.pack(out);
            }
        }
    }
}

impl Unpack for crate::text::HeightMeaning {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("HeightMeaning", 4)? {
            0 => Self::Frame,
            1 => Self::CapHeight,
            2 => Self::Em,
            _ => Self::Declared(Unpack::unpack(input)?),
        })
    }
}

impl Pack for crate::text::Horizontal {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Left => out.uint(0),
            Self::Center => out.uint(1),
            Self::Right => out.uint(2),
            Self::Middle => out.uint(3),
        }
    }
}

impl Unpack for crate::text::Horizontal {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Horizontal", 4)? {
            0 => Self::Left,
            1 => Self::Center,
            2 => Self::Right,
            _ => Self::Middle,
        })
    }
}

impl<T: Pack> Pack for crate::style::Inherit<T> {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Layer => out.uint(0),
            Self::Block => out.uint(1),
            Self::Own(f0) => {
                out.uint(2);
                f0.pack(out);
            }
        }
    }
}

impl<T: Unpack> Unpack for crate::style::Inherit<T> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Inherit", 3)? {
            0 => Self::Layer,
            1 => Self::Block,
            _ => Self::Own(Unpack::unpack(input)?),
        })
    }
}

impl Pack for crate::hatch::Islands {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Nested => out.uint(0),
            Self::Outer => out.uint(1),
            Self::Ignore => out.uint(2),
        }
    }
}

impl Unpack for crate::hatch::Islands {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Islands", 3)? {
            0 => Self::Nested,
            1 => Self::Outer,
            _ => Self::Ignore,
        })
    }
}

impl Pack for zumen_base::Key {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Handle(f0) => {
                out.uint(0);
                f0.pack(out);
            }
            Self::Instance(f0) => {
                out.uint(1);
                f0.pack(out);
            }
        }
    }
}

impl Unpack for zumen_base::Key {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Key", 2)? {
            0 => Self::Handle(Unpack::unpack(input)?),
            _ => Self::Instance(Unpack::unpack(input)?),
        })
    }
}

impl Pack for crate::shape::KnotParameterization {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Chord => out.uint(0),
            Self::SquareRoot => out.uint(1),
            Self::Uniform => out.uint(2),
            Self::Custom => out.uint(3),
        }
    }
}

impl Unpack for crate::shape::KnotParameterization {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("KnotParameterization", 4)? {
            0 => Self::Chord,
            1 => Self::SquareRoot,
            2 => Self::Uniform,
            _ => Self::Custom,
        })
    }
}

impl Pack for crate::dimension::Label {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Measurement => out.uint(0),
            Self::Override { content, spans } => {
                out.uint(1);
                out.record(|out| {
                    content.pack(out);
                    spans.pack(out);
                });
            }
            Self::Hidden => out.uint(2),
        }
    }
}

impl Unpack for crate::dimension::Label {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Label", 3)? {
            0 => Self::Measurement,
            1 => input.record("Label", |input| {
                Ok(Self::Override {
                    content: Unpack::unpack(input)?,
                    spans: Unpack::unpack(input)?,
                })
            })?,
            _ => Self::Hidden,
        })
    }
}

impl Pack for crate::style::Layer {
    fn pack(&self, out: &mut Writer) {
        let Self {
            id,
            name,
            state,
            style,
            follows_placement,
            description,
            source,
        } = self;
        out.record(|out| {
            id.pack(out);
            name.pack(out);
            state.pack(out);
            style.pack(out);
            follows_placement.pack(out);
            description.pack(out);
            source.pack(out);
        });
    }
}

impl Unpack for crate::style::Layer {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Layer", |input| {
            Ok(Self {
                id: Unpack::unpack(input)?,
                name: Unpack::unpack(input)?,
                state: Unpack::unpack(input)?,
                style: Unpack::unpack(input)?,
                follows_placement: Unpack::unpack(input)?,
                description: Unpack::unpack(input)?,
                source: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::style::LayerId {
    fn pack(&self, out: &mut Writer) {
        let Self(f0) = self;
        f0.pack(out);
    }
}

impl Unpack for crate::style::LayerId {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(Self(Unpack::unpack(input)?))
    }
}

impl Pack for crate::style::LayerState {
    fn pack(&self, out: &mut Writer) {
        let Self {
            visible,
            printable,
            editable,
            frozen,
        } = self;
        out.record(|out| {
            visible.pack(out);
            printable.pack(out);
            editable.pack(out);
            frozen.pack(out);
        });
    }
}

impl Unpack for crate::style::LayerState {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("LayerState", |input| {
            Ok(Self {
                visible: Unpack::unpack(input)?,
                printable: Unpack::unpack(input)?,
                editable: Unpack::unpack(input)?,
                frozen: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::style::LayerStyle {
    fn pack(&self, out: &mut Writer) {
        let Self {
            color,
            line_type,
            line_weight,
            transparency,
        } = self;
        out.record(|out| {
            color.pack(out);
            line_type.pack(out);
            line_weight.pack(out);
            transparency.pack(out);
        });
    }
}

impl Unpack for crate::style::LayerStyle {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("LayerStyle", |input| {
            Ok(Self {
                color: Unpack::unpack(input)?,
                line_type: Unpack::unpack(input)?,
                line_weight: Unpack::unpack(input)?,
                transparency: Unpack::unpack(input)?,
            })
        })
    }
}

impl<P: Pack> Pack for crate::dimension::Leader<P> {
    fn pack(&self, out: &mut Writer) {
        let Self {
            path,
            spline,
            arrow,
            annotation,
            style,
        } = self;
        out.record(|out| {
            path.pack(out);
            spline.pack(out);
            arrow.pack(out);
            annotation.pack(out);
            style.pack(out);
        });
    }
}

impl<P: Unpack> Unpack for crate::dimension::Leader<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Leader", |input| {
            Ok(Self {
                path: Unpack::unpack(input)?,
                spline: Unpack::unpack(input)?,
                arrow: Unpack::unpack(input)?,
                annotation: Unpack::unpack(input)?,
                style: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::space::Length {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Paper(f0) => {
                out.uint(0);
                f0.pack(out);
            }
            Self::Model(f0) => {
                out.uint(1);
                f0.pack(out);
            }
        }
    }
}

impl Unpack for crate::space::Length {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Length", 2)? {
            0 => Self::Paper(Unpack::unpack(input)?),
            _ => Self::Model(Unpack::unpack(input)?),
        })
    }
}

impl Pack for crate::text::LineSpacing {
    fn pack(&self, out: &mut Writer) {
        let Self { factor, exact } = self;
        out.record(|out| {
            factor.pack(out);
            exact.pack(out);
        });
    }
}

impl Unpack for crate::text::LineSpacing {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("LineSpacing", |input| {
            Ok(Self {
                factor: Unpack::unpack(input)?,
                exact: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::style::LineType {
    fn pack(&self, out: &mut Writer) {
        let Self {
            id,
            name,
            definition,
            source,
        } = self;
        out.record(|out| {
            id.pack(out);
            name.pack(out);
            definition.pack(out);
            source.pack(out);
        });
    }
}

impl Unpack for crate::style::LineType {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("LineType", |input| {
            Ok(Self {
                id: Unpack::unpack(input)?,
                name: Unpack::unpack(input)?,
                definition: Unpack::unpack(input)?,
                source: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::style::LineTypeDefinition {
    fn pack(&self, out: &mut Writer) {
        let Self {
            description,
            pattern,
            basis,
            marks,
        } = self;
        out.record(|out| {
            description.pack(out);
            pattern.pack(out);
            basis.pack(out);
            marks.pack(out);
        });
    }
}

impl Unpack for crate::style::LineTypeDefinition {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("LineTypeDefinition", |input| {
            Ok(Self {
                description: Unpack::unpack(input)?,
                pattern: Unpack::unpack(input)?,
                basis: Unpack::unpack(input)?,
                marks: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::style::LineTypeId {
    fn pack(&self, out: &mut Writer) {
        let Self(f0) = self;
        f0.pack(out);
    }
}

impl Unpack for crate::style::LineTypeId {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(Self(Unpack::unpack(input)?))
    }
}

impl Pack for crate::style::LineTypeMark {
    fn pack(&self, out: &mut Writer) {
        let Self {
            index,
            content,
            scale,
            rotation,
            rotation_basis,
            offset,
        } = self;
        out.record(|out| {
            index.pack(out);
            content.pack(out);
            scale.pack(out);
            rotation.pack(out);
            rotation_basis.pack(out);
            offset.pack(out);
        });
    }
}

impl Unpack for crate::style::LineTypeMark {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("LineTypeMark", |input| {
            Ok(Self {
                index: Unpack::unpack(input)?,
                content: Unpack::unpack(input)?,
                scale: Unpack::unpack(input)?,
                rotation: Unpack::unpack(input)?,
                rotation_basis: Unpack::unpack(input)?,
                offset: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::style::LineWeight {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Width(f0) => {
                out.uint(0);
                f0.pack(out);
            }
            Self::Default => out.uint(1),
        }
    }
}

impl Unpack for crate::style::LineWeight {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("LineWeight", 2)? {
            0 => Self::Width(Unpack::unpack(input)?),
            _ => Self::Default,
        })
    }
}

impl Pack for crate::style::MarkContent {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Shape { font, number } => {
                out.uint(0);
                out.record(|out| {
                    font.pack(out);
                    number.pack(out);
                });
            }
            Self::Text { content, style } => {
                out.uint(1);
                out.record(|out| {
                    content.pack(out);
                    style.pack(out);
                });
            }
        }
    }
}

impl Unpack for crate::style::MarkContent {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("MarkContent", 2)? {
            0 => input.record("MarkContent", |input| {
                Ok(Self::Shape {
                    font: Unpack::unpack(input)?,
                    number: Unpack::unpack(input)?,
                })
            })?,
            _ => input.record("MarkContent", |input| {
                Ok(Self::Text {
                    content: Unpack::unpack(input)?,
                    style: Unpack::unpack(input)?,
                })
            })?,
        })
    }
}

impl Pack for crate::style::MarkRotation {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Relative => out.uint(0),
            Self::Absolute => out.uint(1),
            Self::Upright => out.uint(2),
        }
    }
}

impl Unpack for crate::style::MarkRotation {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("MarkRotation", 3)? {
            0 => Self::Relative,
            1 => Self::Absolute,
            _ => Self::Upright,
        })
    }
}

impl Pack for crate::point::MarkerKey {
    fn pack(&self, out: &mut Writer) {
        let Self { namespace, code } = self;
        out.record(|out| {
            namespace.pack(out);
            code.pack(out);
        });
    }
}

impl Unpack for crate::point::MarkerKey {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("MarkerKey", |input| {
            Ok(Self {
                namespace: Unpack::unpack(input)?,
                code: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::dimension::Measured {
    fn pack(&self, out: &mut Writer) {
        let Self { value, key } = self;
        out.record(|out| {
            value.pack(out);
            key.pack(out);
        });
    }
}

impl Unpack for crate::dimension::Measured {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Measured", |input| {
            Ok(Self {
                value: Unpack::unpack(input)?,
                key: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::space::ModelLength {
    fn pack(&self, out: &mut Writer) {
        let Self { space, value } = self;
        out.record(|out| {
            space.pack(out);
            value.pack(out);
        });
    }
}

impl Unpack for crate::space::ModelLength {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("ModelLength", |input| {
            Ok(Self {
                space: Unpack::unpack(input)?,
                value: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::space::ModelPoint {
    fn pack(&self, out: &mut Writer) {
        let Self { space, x, y } = self;
        out.record(|out| {
            space.pack(out);
            x.pack(out);
            y.pack(out);
        });
    }
}

impl Unpack for crate::space::ModelPoint {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("ModelPoint", |input| {
            Ok(Self {
                space: Unpack::unpack(input)?,
                x: Unpack::unpack(input)?,
                y: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::space::ModelSpace {
    fn pack(&self, out: &mut Writer) {
        let Self {
            id,
            unit,
            axes,
            elements,
            order,
        } = self;
        out.record(|out| {
            id.pack(out);
            unit.pack(out);
            axes.pack(out);
            elements.pack(out);
            order.pack(out);
        });
    }
}

impl Unpack for crate::space::ModelSpace {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("ModelSpace", |input| {
            Ok(Self {
                id: Unpack::unpack(input)?,
                unit: Unpack::unpack(input)?,
                axes: Unpack::unpack(input)?,
                elements: Unpack::unpack(input)?,
                order: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::space::PaperLength {
    fn pack(&self, out: &mut Writer) {
        let Self(f0) = self;
        f0.pack(out);
    }
}

impl Unpack for crate::space::PaperLength {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(Self(Unpack::unpack(input)?))
    }
}

impl Pack for crate::space::PaperPoint {
    fn pack(&self, out: &mut Writer) {
        let Self { sheet, x, y } = self;
        out.record(|out| {
            sheet.pack(out);
            x.pack(out);
            y.pack(out);
        });
    }
}

impl Unpack for crate::space::PaperPoint {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("PaperPoint", |input| {
            Ok(Self {
                sheet: Unpack::unpack(input)?,
                x: Unpack::unpack(input)?,
                y: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::callout::PathKind {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Straight => out.uint(0),
            Self::Spline => out.uint(1),
            Self::Hidden => out.uint(2),
        }
    }
}

impl Unpack for crate::callout::PathKind {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("PathKind", 3)? {
            0 => Self::Straight,
            1 => Self::Spline,
            _ => Self::Hidden,
        })
    }
}

impl Pack for crate::hatch::Pattern {
    fn pack(&self, out: &mut Writer) {
        let Self {
            name,
            origin,
            angle,
            scale,
            double,
            lines,
        } = self;
        out.record(|out| {
            name.pack(out);
            origin.pack(out);
            angle.pack(out);
            scale.pack(out);
            double.pack(out);
            lines.pack(out);
        });
    }
}

impl Unpack for crate::hatch::Pattern {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Pattern", |input| {
            Ok(Self {
                name: Unpack::unpack(input)?,
                origin: Unpack::unpack(input)?,
                angle: Unpack::unpack(input)?,
                scale: Unpack::unpack(input)?,
                double: Unpack::unpack(input)?,
                lines: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::style::PatternBasis {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Element => out.uint(0),
            Self::Paper => out.uint(1),
        }
    }
}

impl Unpack for crate::style::PatternBasis {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("PatternBasis", 2)? {
            0 => Self::Element,
            _ => Self::Paper,
        })
    }
}

impl Pack for crate::hatch::PatternLine {
    fn pack(&self, out: &mut Writer) {
        let Self {
            angle,
            base,
            offset,
            dashes,
        } = self;
        out.record(|out| {
            angle.pack(out);
            base.pack(out);
            offset.pack(out);
            dashes.pack(out);
        });
    }
}

impl Unpack for crate::hatch::PatternLine {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("PatternLine", |input| {
            Ok(Self {
                angle: Unpack::unpack(input)?,
                base: Unpack::unpack(input)?,
                offset: Unpack::unpack(input)?,
                dashes: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::hatch::PatternOrigin {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::UserDefined => out.uint(0),
            Self::Predefined => out.uint(1),
            Self::Custom => out.uint(2),
        }
    }
}

impl Unpack for crate::hatch::PatternOrigin {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("PatternOrigin", 3)? {
            0 => Self::UserDefined,
            1 => Self::Predefined,
            _ => Self::Custom,
        })
    }
}

impl Pack for crate::text::Piece {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Text(f0) => {
                out.uint(0);
                f0.pack(out);
            }
            Self::Paragraph => out.uint(1),
            Self::Column => out.uint(2),
            Self::Measurement => out.uint(3),
            Self::Below => out.uint(4),
            Self::Stack { upper, lower, kind } => {
                out.uint(5);
                out.record(|out| {
                    upper.pack(out);
                    lower.pack(out);
                    kind.pack(out);
                });
            }
            Self::Unparsed(f0) => {
                out.uint(6);
                f0.pack(out);
            }
        }
    }
}

impl Unpack for crate::text::Piece {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Piece", 7)? {
            0 => Self::Text(Unpack::unpack(input)?),
            1 => Self::Paragraph,
            2 => Self::Column,
            3 => Self::Measurement,
            4 => Self::Below,
            5 => input.record("Piece", |input| {
                Ok(Self::Stack {
                    upper: Unpack::unpack(input)?,
                    lower: Unpack::unpack(input)?,
                    kind: Unpack::unpack(input)?,
                })
            })?,
            _ => Self::Unparsed(Unpack::unpack(input)?),
        })
    }
}

impl<P: Pack> Pack for crate::text::Placement<P> {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Anchor {
                point,
                horizontal,
                vertical,
            } => {
                out.uint(0);
                out.record(|out| {
                    point.pack(out);
                    horizontal.pack(out);
                    vertical.pack(out);
                });
            }
            Self::Between { first, second, fit } => {
                out.uint(1);
                out.record(|out| {
                    first.pack(out);
                    second.pack(out);
                    fit.pack(out);
                });
            }
        }
    }
}

impl<P: Unpack> Unpack for crate::text::Placement<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Placement", 2)? {
            0 => input.record("Placement", |input| {
                Ok(Self::Anchor {
                    point: Unpack::unpack(input)?,
                    horizontal: Unpack::unpack(input)?,
                    vertical: Unpack::unpack(input)?,
                })
            })?,
            _ => input.record("Placement", |input| {
                Ok(Self::Between {
                    first: Unpack::unpack(input)?,
                    second: Unpack::unpack(input)?,
                    fit: Unpack::unpack(input)?,
                })
            })?,
        })
    }
}

impl<P: Pack> Pack for crate::point::Point<P> {
    fn pack(&self, out: &mut Writer) {
        let Self {
            position,
            rotation,
            look,
            construction,
        } = self;
        out.record(|out| {
            position.pack(out);
            rotation.pack(out);
            look.pack(out);
            construction.pack(out);
        });
    }
}

impl<P: Unpack> Unpack for crate::point::Point<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Point", |input| {
            Ok(Self {
                position: Unpack::unpack(input)?,
                rotation: Unpack::unpack(input)?,
                look: Unpack::unpack(input)?,
                construction: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::point::PointLook {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Document => out.uint(0),
            Self::Marker { key, scale } => {
                out.uint(1);
                out.record(|out| {
                    key.pack(out);
                    scale.pack(out);
                });
            }
        }
    }
}

impl Unpack for crate::point::PointLook {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("PointLook", 2)? {
            0 => Self::Document,
            _ => input.record("PointLook", |input| {
                Ok(Self::Marker {
                    key: Unpack::unpack(input)?,
                    scale: Unpack::unpack(input)?,
                })
            })?,
        })
    }
}

impl Pack for crate::point::PointMark {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Dot => out.uint(0),
            Self::None => out.uint(1),
            Self::Plus => out.uint(2),
            Self::Cross => out.uint(3),
            Self::Tick => out.uint(4),
        }
    }
}

impl Unpack for crate::point::PointMark {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("PointMark", 5)? {
            0 => Self::Dot,
            1 => Self::None,
            2 => Self::Plus,
            3 => Self::Cross,
            _ => Self::Tick,
        })
    }
}

impl Pack for crate::point::PointSize {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Length(f0) => {
                out.uint(0);
                f0.pack(out);
            }
            Self::Screen(f0) => {
                out.uint(1);
                f0.pack(out);
            }
        }
    }
}

impl Unpack for crate::point::PointSize {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("PointSize", 2)? {
            0 => Self::Length(Unpack::unpack(input)?),
            _ => Self::Screen(Unpack::unpack(input)?),
        })
    }
}

impl Pack for crate::point::PointStyle {
    fn pack(&self, out: &mut Writer) {
        let Self {
            mark,
            circle,
            square,
            size,
        } = self;
        out.record(|out| {
            mark.pack(out);
            circle.pack(out);
            square.pack(out);
            size.pack(out);
        });
    }
}

impl Unpack for crate::point::PointStyle {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("PointStyle", |input| {
            Ok(Self {
                mark: Unpack::unpack(input)?,
                circle: Unpack::unpack(input)?,
                square: Unpack::unpack(input)?,
                size: Unpack::unpack(input)?,
            })
        })
    }
}

impl<P: Pack> Pack for crate::shape::Polyline<P> {
    fn pack(&self, out: &mut Writer) {
        let Self {
            vertices,
            closed,
            width,
            vertex_widths,
            line_type_generation,
        } = self;
        out.record(|out| {
            vertices.pack(out);
            closed.pack(out);
            width.pack(out);
            vertex_widths.pack(out);
            line_type_generation.pack(out);
        });
    }
}

impl<P: Unpack> Unpack for crate::shape::Polyline<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Polyline", |input| {
            Ok(Self {
                vertices: Unpack::unpack(input)?,
                closed: Unpack::unpack(input)?,
                width: Unpack::unpack(input)?,
                vertex_widths: Unpack::unpack(input)?,
                line_type_generation: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::underlay::Resource {
    fn pack(&self, out: &mut Writer) {
        let Self { path, definition } = self;
        out.record(|out| {
            path.pack(out);
            definition.pack(out);
        });
    }
}

impl Unpack for crate::underlay::Resource {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Resource", |input| {
            Ok(Self {
                path: Unpack::unpack(input)?,
                definition: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::text::Rotation {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Angle(f0) => {
                out.uint(0);
                f0.pack(out);
            }
            Self::Direction(f0) => {
                out.uint(1);
                f0.pack(out);
            }
        }
    }
}

impl Unpack for crate::text::Rotation {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Rotation", 2)? {
            0 => Self::Angle(Unpack::unpack(input)?),
            _ => Self::Direction(Unpack::unpack(input)?),
        })
    }
}

impl<T: Pack> Pack for crate::text::Scaled<T> {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Value(f0) => {
                out.uint(0);
                f0.pack(out);
            }
            Self::Factor(f0) => {
                out.uint(1);
                f0.pack(out);
            }
        }
    }
}

impl<T: Unpack> Unpack for crate::text::Scaled<T> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Scaled", 2)? {
            0 => Self::Value(Unpack::unpack(input)?),
            _ => Self::Factor(Unpack::unpack(input)?),
        })
    }
}

impl<P: Pack> Pack for crate::shape::Shape<P> {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Line { start, end } => {
                out.uint(0);
                out.record(|out| {
                    start.pack(out);
                    end.pack(out);
                });
            }
            Self::Circle { center, radius } => {
                out.uint(1);
                out.record(|out| {
                    center.pack(out);
                    radius.pack(out);
                });
            }
            Self::Curve(f0) => {
                out.uint(2);
                f0.pack(out);
            }
            Self::Polyline(f0) => {
                out.uint(3);
                f0.pack(out);
            }
            Self::Text(f0) => {
                out.uint(4);
                f0.pack(out);
            }
            Self::Formatted(f0) => {
                out.uint(5);
                f0.pack(out);
            }
            Self::Dimension(f0) => {
                out.uint(6);
                f0.pack(out);
            }
            Self::Leader(f0) => {
                out.uint(7);
                f0.pack(out);
            }
            Self::Callout(f0) => {
                out.uint(8);
                f0.pack(out);
            }
            Self::Table(f0) => {
                out.uint(9);
                f0.pack(out);
            }
            Self::Underlay(f0) => {
                out.uint(10);
                f0.pack(out);
            }
            Self::Symbol(f0) => {
                out.uint(11);
                f0.pack(out);
            }
            Self::Hatch(f0) => {
                out.uint(12);
                f0.pack(out);
            }
            Self::Point(f0) => {
                out.uint(13);
                f0.pack(out);
            }
        }
    }
}

impl<P: Unpack> Unpack for crate::shape::Shape<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Shape", 14)? {
            0 => input.record("Shape", |input| {
                Ok(Self::Line {
                    start: Unpack::unpack(input)?,
                    end: Unpack::unpack(input)?,
                })
            })?,
            1 => input.record("Shape", |input| {
                Ok(Self::Circle {
                    center: Unpack::unpack(input)?,
                    radius: Unpack::unpack(input)?,
                })
            })?,
            2 => Self::Curve(Unpack::unpack(input)?),
            3 => Self::Polyline(Unpack::unpack(input)?),
            4 => Self::Text(Unpack::unpack(input)?),
            5 => Self::Formatted(Unpack::unpack(input)?),
            6 => Self::Dimension(Unpack::unpack(input)?),
            7 => Self::Leader(Unpack::unpack(input)?),
            8 => Self::Callout(Unpack::unpack(input)?),
            9 => Self::Table(Unpack::unpack(input)?),
            10 => Self::Underlay(Unpack::unpack(input)?),
            11 => Self::Symbol(Unpack::unpack(input)?),
            12 => Self::Hatch(Unpack::unpack(input)?),
            _ => Self::Point(Unpack::unpack(input)?),
        })
    }
}

impl Pack for crate::space::Sheet {
    fn pack(&self, out: &mut Writer) {
        let Self {
            id,
            name,
            size,
            elements,
            views,
            order,
            source,
        } = self;
        out.record(|out| {
            id.pack(out);
            name.pack(out);
            size.pack(out);
            elements.pack(out);
            views.pack(out);
            order.pack(out);
            source.pack(out);
        });
    }
}

impl Unpack for crate::space::Sheet {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Sheet", |input| {
            Ok(Self {
                id: Unpack::unpack(input)?,
                name: Unpack::unpack(input)?,
                size: Unpack::unpack(input)?,
                elements: Unpack::unpack(input)?,
                views: Unpack::unpack(input)?,
                order: Unpack::unpack(input)?,
                source: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::SheetId {
    fn pack(&self, out: &mut Writer) {
        let Self(f0) = self;
        f0.pack(out);
    }
}

impl Unpack for crate::SheetId {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(Self(Unpack::unpack(input)?))
    }
}

impl Pack for crate::space::SheetItem {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Element(f0) => {
                out.uint(0);
                f0.pack(out);
            }
            Self::View(f0) => {
                out.uint(1);
                f0.pack(out);
            }
        }
    }
}

impl Unpack for crate::space::SheetItem {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("SheetItem", 2)? {
            0 => Self::Element(Unpack::unpack(input)?),
            _ => Self::View(Unpack::unpack(input)?),
        })
    }
}

impl Pack for crate::binary::Source {
    fn pack(&self, out: &mut Writer) {
        let Self {
            format,
            family,
            hash,
        } = self;
        out.record(|out| {
            format.pack(out);
            family.pack(out);
            hash.pack(out);
        });
    }
}

impl Unpack for crate::binary::Source {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Source", |input| {
            Ok(Self {
                format: Unpack::unpack(input)?,
                family: Unpack::unpack(input)?,
                hash: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::SpaceId {
    fn pack(&self, out: &mut Writer) {
        let Self(f0) = self;
        f0.pack(out);
    }
}

impl Unpack for crate::SpaceId {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(Self(Unpack::unpack(input)?))
    }
}

impl Pack for crate::text::Span {
    fn pack(&self, out: &mut Writer) {
        let Self { piece, format } = self;
        out.record(|out| {
            piece.pack(out);
            format.pack(out);
        });
    }
}

impl Unpack for crate::text::Span {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Span", |input| {
            Ok(Self {
                piece: Unpack::unpack(input)?,
                format: Unpack::unpack(input)?,
            })
        })
    }
}

impl<P: Pack> Pack for crate::shape::Spline<P> {
    fn pack(&self, out: &mut Writer) {
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
        out.record(|out| {
            degree.pack(out);
            knots.pack(out);
            control.pack(out);
            weights.pack(out);
            fit.pack(out);
            start_tangent.pack(out);
            end_tangent.pack(out);
            closed.pack(out);
            periodic.pack(out);
            fit_tolerance.pack(out);
            knot_parameterization.pack(out);
        });
    }
}

impl<P: Unpack> Unpack for crate::shape::Spline<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Spline", |input| {
            Ok(Self {
                degree: Unpack::unpack(input)?,
                knots: Unpack::unpack(input)?,
                control: Unpack::unpack(input)?,
                weights: Unpack::unpack(input)?,
                fit: Unpack::unpack(input)?,
                start_tangent: Unpack::unpack(input)?,
                end_tangent: Unpack::unpack(input)?,
                closed: Unpack::unpack(input)?,
                periodic: Unpack::unpack(input)?,
                fit_tolerance: Unpack::unpack(input)?,
                knot_parameterization: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::text::StackKind {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Horizontal => out.uint(0),
            Self::Diagonal => out.uint(1),
            Self::Tolerance => out.uint(2),
        }
    }
}

impl Unpack for crate::text::StackKind {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("StackKind", 3)? {
            0 => Self::Horizontal,
            1 => Self::Diagonal,
            _ => Self::Tolerance,
        })
    }
}

impl Pack for crate::style::Style {
    fn pack(&self, out: &mut Writer) {
        let Self {
            color,
            line_type,
            line_type_scale,
            line_weight,
            transparency,
        } = self;
        out.record(|out| {
            color.pack(out);
            line_type.pack(out);
            line_type_scale.pack(out);
            line_weight.pack(out);
            transparency.pack(out);
        });
    }
}

impl Unpack for crate::style::Style {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Style", |input| {
            Ok(Self {
                color: Unpack::unpack(input)?,
                line_type: Unpack::unpack(input)?,
                line_type_scale: Unpack::unpack(input)?,
                line_weight: Unpack::unpack(input)?,
                transparency: Unpack::unpack(input)?,
            })
        })
    }
}

impl<P: Pack> Pack for crate::symbol::Symbol<P> {
    fn pack(&self, out: &mut Writer) {
        let Self {
            key,
            position,
            scale,
            rotation,
            grid,
            attributes,
            clip,
        } = self;
        out.record(|out| {
            key.pack(out);
            position.pack(out);
            scale.pack(out);
            rotation.pack(out);
            grid.pack(out);
            attributes.pack(out);
            clip.pack(out);
        });
    }
}

impl<P: Unpack> Unpack for crate::symbol::Symbol<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Symbol", |input| {
            Ok(Self {
                key: Unpack::unpack(input)?,
                position: Unpack::unpack(input)?,
                scale: Unpack::unpack(input)?,
                rotation: Unpack::unpack(input)?,
                grid: Unpack::unpack(input)?,
                attributes: Unpack::unpack(input)?,
                clip: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::symbol::SymbolDefinition {
    fn pack(&self, out: &mut Writer) {
        let Self {
            key,
            space,
            base,
            attributes,
            anonymous,
            external,
            source,
        } = self;
        out.record(|out| {
            key.pack(out);
            space.pack(out);
            base.pack(out);
            attributes.pack(out);
            anonymous.pack(out);
            external.pack(out);
            source.pack(out);
        });
    }
}

impl Unpack for crate::symbol::SymbolDefinition {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("SymbolDefinition", |input| {
            Ok(Self {
                key: Unpack::unpack(input)?,
                space: Unpack::unpack(input)?,
                base: Unpack::unpack(input)?,
                attributes: Unpack::unpack(input)?,
                anonymous: Unpack::unpack(input)?,
                external: Unpack::unpack(input)?,
                source: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::symbol::SymbolKey {
    fn pack(&self, out: &mut Writer) {
        let Self { namespace, name } = self;
        out.record(|out| {
            namespace.pack(out);
            name.pack(out);
        });
    }
}

impl Unpack for crate::symbol::SymbolKey {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("SymbolKey", |input| {
            Ok(Self {
                namespace: Unpack::unpack(input)?,
                name: Unpack::unpack(input)?,
            })
        })
    }
}

impl<P: Pack> Pack for crate::table::Table<P> {
    fn pack(&self, out: &mut Writer) {
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
        out.record(|out| {
            position.pack(out);
            direction.pack(out);
            row_heights.pack(out);
            column_widths.pack(out);
            cells.pack(out);
            style.pack(out);
            formats.pack(out);
            drawn.pack(out);
        });
    }
}

impl<P: Unpack> Unpack for crate::table::Table<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Table", |input| {
            Ok(Self {
                position: Unpack::unpack(input)?,
                direction: Unpack::unpack(input)?,
                row_heights: Unpack::unpack(input)?,
                column_widths: Unpack::unpack(input)?,
                cells: Unpack::unpack(input)?,
                style: Unpack::unpack(input)?,
                formats: Unpack::unpack(input)?,
                drawn: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::table::TableFormats {
    fn pack(&self, out: &mut Writer) {
        let Self {
            title,
            header,
            title_format,
            header_format,
            data_format,
        } = self;
        out.record(|out| {
            title.pack(out);
            header.pack(out);
            title_format.pack(out);
            header_format.pack(out);
            data_format.pack(out);
        });
    }
}

impl Unpack for crate::table::TableFormats {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("TableFormats", |input| {
            Ok(Self {
                title: Unpack::unpack(input)?,
                header: Unpack::unpack(input)?,
                title_format: Unpack::unpack(input)?,
                header_format: Unpack::unpack(input)?,
                data_format: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::extension::Target {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Document => out.uint(0),
            Self::Element(f0) => {
                out.uint(1);
                f0.pack(out);
            }
            Self::Layer(f0) => {
                out.uint(2);
                f0.pack(out);
            }
            Self::LineType(f0) => {
                out.uint(3);
                f0.pack(out);
            }
        }
    }
}

impl Unpack for crate::extension::Target {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Target", 4)? {
            0 => Self::Document,
            1 => Self::Element(Unpack::unpack(input)?),
            2 => Self::Layer(Unpack::unpack(input)?),
            _ => Self::LineType(Unpack::unpack(input)?),
        })
    }
}

impl<P: Pack> Pack for crate::text::Text<P> {
    fn pack(&self, out: &mut Writer) {
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
        out.record(|out| {
            content.pack(out);
            height.pack(out);
            placement.pack(out);
            rotation.pack(out);
            width_factor.pack(out);
            oblique.pack(out);
            backward.pack(out);
            upside_down.pack(out);
            style.pack(out);
        });
    }
}

impl<P: Unpack> Unpack for crate::text::Text<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Text", |input| {
            Ok(Self {
                content: Unpack::unpack(input)?,
                height: Unpack::unpack(input)?,
                placement: Unpack::unpack(input)?,
                rotation: Unpack::unpack(input)?,
                width_factor: Unpack::unpack(input)?,
                oblique: Unpack::unpack(input)?,
                backward: Unpack::unpack(input)?,
                upside_down: Unpack::unpack(input)?,
                style: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::style::TextStyle {
    fn pack(&self, out: &mut Writer) {
        let Self {
            name,
            definition,
            source,
        } = self;
        out.record(|out| {
            name.pack(out);
            definition.pack(out);
            source.pack(out);
        });
    }
}

impl Unpack for crate::style::TextStyle {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("TextStyle", |input| {
            Ok(Self {
                name: Unpack::unpack(input)?,
                definition: Unpack::unpack(input)?,
                source: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::style::TextStyleDefinition {
    fn pack(&self, out: &mut Writer) {
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
        out.record(|out| {
            font.pack(out);
            big_font.pack(out);
            height.pack(out);
            width.pack(out);
            oblique.pack(out);
            backward.pack(out);
            upside_down.pack(out);
            vertical.pack(out);
        });
    }
}

impl Unpack for crate::style::TextStyleDefinition {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("TextStyleDefinition", |input| {
            Ok(Self {
                font: Unpack::unpack(input)?,
                big_font: Unpack::unpack(input)?,
                height: Unpack::unpack(input)?,
                width: Unpack::unpack(input)?,
                oblique: Unpack::unpack(input)?,
                backward: Unpack::unpack(input)?,
                upside_down: Unpack::unpack(input)?,
                vertical: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::TitleBlock {
    fn pack(&self, out: &mut Writer) {
        let Self { items, source } = self;
        out.record(|out| {
            items.pack(out);
            source.pack(out);
        });
    }
}

impl Unpack for crate::TitleBlock {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("TitleBlock", |input| {
            Ok(Self {
                items: Unpack::unpack(input)?,
                source: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::TitleItem {
    fn pack(&self, out: &mut Writer) {
        let Self {
            key,
            name,
            value,
            display,
        } = self;
        out.record(|out| {
            key.pack(out);
            name.pack(out);
            value.pack(out);
            display.pack(out);
        });
    }
}

impl Unpack for crate::TitleItem {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("TitleItem", |input| {
            Ok(Self {
                key: Unpack::unpack(input)?,
                name: Unpack::unpack(input)?,
                value: Unpack::unpack(input)?,
                display: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::style::Transparency {
    fn pack(&self, out: &mut Writer) {
        let Self { alpha } = self;
        out.record(|out| {
            alpha.pack(out);
        });
    }
}

impl Unpack for crate::style::Transparency {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Transparency", |input| {
            Ok(Self {
                alpha: Unpack::unpack(input)?,
            })
        })
    }
}

impl<P: Pack> Pack for crate::underlay::Underlay<P> {
    fn pack(&self, out: &mut Writer) {
        let Self {
            kind,
            resource,
            page,
            frame,
            clip,
            visible,
        } = self;
        out.record(|out| {
            kind.pack(out);
            resource.pack(out);
            page.pack(out);
            frame.pack(out);
            clip.pack(out);
            visible.pack(out);
        });
    }
}

impl<P: Unpack> Unpack for crate::underlay::Underlay<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Underlay", |input| {
            Ok(Self {
                kind: Unpack::unpack(input)?,
                resource: Unpack::unpack(input)?,
                page: Unpack::unpack(input)?,
                frame: Unpack::unpack(input)?,
                clip: Unpack::unpack(input)?,
                visible: Unpack::unpack(input)?,
            })
        })
    }
}

impl<P: Pack> Pack for crate::underlay::UnderlayFrame<P> {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Pixels {
                size,
                origin,
                right,
                top,
            } => {
                out.uint(0);
                out.record(|out| {
                    size.pack(out);
                    origin.pack(out);
                    right.pack(out);
                    top.pack(out);
                });
            }
            Self::Corners { origin, right, top } => {
                out.uint(1);
                out.record(|out| {
                    origin.pack(out);
                    right.pack(out);
                    top.pack(out);
                });
            }
            Self::Scaled {
                origin,
                scale,
                rotation,
            } => {
                out.uint(2);
                out.record(|out| {
                    origin.pack(out);
                    scale.pack(out);
                    rotation.pack(out);
                });
            }
        }
    }
}

impl<P: Unpack> Unpack for crate::underlay::UnderlayFrame<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("UnderlayFrame", 3)? {
            0 => input.record("UnderlayFrame", |input| {
                Ok(Self::Pixels {
                    size: Unpack::unpack(input)?,
                    origin: Unpack::unpack(input)?,
                    right: Unpack::unpack(input)?,
                    top: Unpack::unpack(input)?,
                })
            })?,
            1 => input.record("UnderlayFrame", |input| {
                Ok(Self::Corners {
                    origin: Unpack::unpack(input)?,
                    right: Unpack::unpack(input)?,
                    top: Unpack::unpack(input)?,
                })
            })?,
            _ => input.record("UnderlayFrame", |input| {
                Ok(Self::Scaled {
                    origin: Unpack::unpack(input)?,
                    scale: Unpack::unpack(input)?,
                    rotation: Unpack::unpack(input)?,
                })
            })?,
        })
    }
}

impl Pack for crate::underlay::UnderlayKind {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Image => out.uint(0),
            Self::Pdf => out.uint(1),
            Self::Dwf => out.uint(2),
            Self::Dgn => out.uint(3),
        }
    }
}

impl Unpack for crate::underlay::UnderlayKind {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("UnderlayKind", 4)? {
            0 => Self::Image,
            1 => Self::Pdf,
            2 => Self::Dwf,
            _ => Self::Dgn,
        })
    }
}

impl Pack for crate::space::Unit {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Millimeter => out.uint(0),
            Self::Centimeter => out.uint(1),
            Self::Meter => out.uint(2),
            Self::Inch => out.uint(3),
            Self::Foot => out.uint(4),
            Self::Unknown => out.uint(5),
        }
    }
}

impl Unpack for crate::space::Unit {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Unit", 6)? {
            0 => Self::Millimeter,
            1 => Self::Centimeter,
            2 => Self::Meter,
            3 => Self::Inch,
            4 => Self::Foot,
            _ => Self::Unknown,
        })
    }
}

impl Pack for crate::table::ValueUnit {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::None => out.uint(0),
            Self::Distance => out.uint(1),
            Self::Angle => out.uint(2),
            Self::Area => out.uint(3),
            Self::Volume => out.uint(4),
            Self::Currency => out.uint(5),
            Self::Percentage => out.uint(6),
        }
    }
}

impl Unpack for crate::table::ValueUnit {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("ValueUnit", 7)? {
            0 => Self::None,
            1 => Self::Distance,
            2 => Self::Angle,
            3 => Self::Area,
            4 => Self::Volume,
            5 => Self::Currency,
            _ => Self::Percentage,
        })
    }
}

impl<P: Pack> Pack for crate::shape::Vertex<P> {
    fn pack(&self, out: &mut Writer) {
        let Self { point, bulge } = self;
        out.record(|out| {
            point.pack(out);
            bulge.pack(out);
        });
    }
}

impl<P: Unpack> Unpack for crate::shape::Vertex<P> {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("Vertex", |input| {
            Ok(Self {
                point: Unpack::unpack(input)?,
                bulge: Unpack::unpack(input)?,
            })
        })
    }
}

impl Pack for crate::text::Vertical {
    fn pack(&self, out: &mut Writer) {
        match self {
            Self::Baseline => out.uint(0),
            Self::Bottom => out.uint(1),
            Self::Middle => out.uint(2),
            Self::Top => out.uint(3),
        }
    }
}

impl Unpack for crate::text::Vertical {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        Ok(match input.variant("Vertical", 4)? {
            0 => Self::Baseline,
            1 => Self::Bottom,
            2 => Self::Middle,
            _ => Self::Top,
        })
    }
}

impl Pack for crate::space::View {
    fn pack(&self, out: &mut Writer) {
        let Self {
            space,
            sheet,
            transform,
            clip,
            hidden_layers,
            source,
        } = self;
        out.record(|out| {
            space.pack(out);
            sheet.pack(out);
            transform.pack(out);
            clip.pack(out);
            hidden_layers.pack(out);
            source.pack(out);
        });
    }
}

impl Unpack for crate::space::View {
    fn unpack<I: Input>(input: &mut I) -> Result<Self, I::Error> {
        input.record("View", |input| {
            Ok(Self {
                space: Unpack::unpack(input)?,
                sheet: Unpack::unpack(input)?,
                transform: Unpack::unpack(input)?,
                clip: Unpack::unpack(input)?,
                hidden_layers: Unpack::unpack(input)?,
                source: Unpack::unpack(input)?,
            })
        })
    }
}
