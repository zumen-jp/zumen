//! 要素と表の項目のハッシュ（決定 0156）
//!
//! 値を決まった並びのバイト列にし、BLAKE3 の先頭の 128 ビットを取る。出所の位置（レコードの番号・
//! バイト範囲）は入れず、指す先は形式が持つ鍵だけを入れる。実数はビットの並びで書き、-0 は 0 に、
//! NaN は 1 つの値にそろえる。並びを変えるときは、[`Hash::VERSION`] を上げる。

use core::fmt;

use zumen_base::{Cx, Key, Kind, Ref, Stop};

use crate::{
    Affine, Arrowhead, Attribute, AttributeValue, Axis, Background, BackgroundFill, Border,
    Borders, Boundary, Branch, Callout, CalloutContent, Cell, CellContent, CellData, CellFormat,
    CellValue, Clip, Color, ColorCode, ColorSystem, DateTime, Dimension, DimensionKind,
    DimensionText, Dogleg, Edge, Element, Fill, Fit, Flow, Font, FontKind, Format, FormattedText,
    Gradient, Grid, Hatch, HatchLoop, Height, HeightMeaning, Horizontal, Inherit, Islands, Label,
    Layer, LayerId, LayerState, LayerStyle, Leader, Length, LineSpacing, LineType,
    LineTypeDefinition, LineTypeId, LineTypeMark, LineWeight, MarkContent, MarkRotation, Measured,
    ModelLength, ModelPoint, PaperLength, PaperPoint, PathKind, Pattern, PatternBasis, PatternLine,
    PatternOrigin, Piece, Placement, Polyline, Resource, Rotation, Scaled, Shape, SheetId, SpaceId,
    Span, Spline, StackKind, Style, Symbol, SymbolKey, Table, TableFormats, Text, Underlay,
    UnderlayFrame, UnderlayKind, ValueUnit, Vertex, Vertical,
};

/// 要素か表の項目のハッシュ。`v1:` と 32 桁の 16 進で書く
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hash(pub [u8; 16]);

impl Hash {
    /// 並びの版
    pub const VERSION: u32 = 1;

    /// 要素のハッシュ。出所は入れない
    pub fn element<P: Encode>(element: &Element<P>) -> Self {
        let Element {
            source: _,
            layer,
            style,
            shape,
        } = element;
        Self::of(b"element", |out| {
            layer.encode(out);
            style.encode(out);
            shape.encode(out);
        })
    }

    /// 予算と中断の合図を確かめながら要素をハッシュする
    pub fn element_checked<P: Encode>(element: &Element<P>, cx: &mut Cx<'_>) -> Result<Self, Stop> {
        let mut check = || cx.check(Kind::Work, 1);
        let mut out = Canon {
            hasher: blake3::Hasher::new(),
            check: Some(&mut check),
            stop: None,
        };
        Self::VERSION.encode(&mut out);
        b"element".as_slice().encode(&mut out);
        element.layer.encode(&mut out);
        element.style.encode(&mut out);
        element.shape.encode(&mut out);
        if let Some(stop) = out.stop {
            return Err(stop);
        }
        let digest = out.hasher.finalize();
        let mut bytes = [0; 16];
        bytes.copy_from_slice(&digest.as_bytes()[..16]);
        Ok(Self(bytes))
    }

    /// 予算と中断の合図を確かめながら文書をハッシュする
    pub fn document_checked(bytes: &[u8], cx: &mut Cx<'_>) -> Result<Self, Stop> {
        let mut hasher = blake3::Hasher::new();
        for chunk in bytes.chunks(1024) {
            cx.check(Kind::Work, 1)?;
            hasher.update(chunk);
        }
        let digest = hasher.finalize();
        let mut result = [0; 16];
        result.copy_from_slice(&digest.as_bytes()[..16]);
        Ok(Self(result))
    }

    /// 文書のハッシュ。元のバイト列そのものから作る
    pub fn document(bytes: &[u8]) -> Self {
        let digest = blake3::hash(bytes);
        let mut result = [0; 16];
        result.copy_from_slice(&digest.as_bytes()[..16]);
        Self(result)
    }

    /// レイヤのハッシュ。出所は鍵だけを入れる
    pub fn layer(layer: &Layer) -> Self {
        Self::of(b"layer", |out| layer.encode(out))
    }

    /// 線種のハッシュ。出所は鍵だけを入れる
    pub fn line_type(line_type: &LineType) -> Self {
        Self::of(b"line-type", |out| line_type.encode(out))
    }

    fn of(domain: &[u8], body: impl FnOnce(&mut Canon<'_>)) -> Self {
        let mut out = Canon {
            hasher: blake3::Hasher::new(),
            check: None,
            stop: None,
        };
        Self::VERSION.encode(&mut out);
        domain.encode(&mut out);
        body(&mut out);
        let digest = out.hasher.finalize();
        let mut bytes = [0; 16];
        bytes.copy_from_slice(&digest.as_bytes()[..16]);
        Self(bytes)
    }
}

impl core::str::FromStr for Hash {
    type Err = ();

    /// `v1:` と 32 桁の 16 進を読む。版が違うか、桁か文字が合わなければ `Err`
    fn from_str(text: &str) -> Result<Self, ()> {
        let digits = text.strip_prefix("v1:").ok_or(())?;
        if digits.len() != 32 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(());
        }
        let mut bytes = [0; 16];
        for (byte, pair) in bytes.iter_mut().zip(digits.as_bytes().chunks(2)) {
            let pair = core::str::from_utf8(pair).map_err(|_| ())?;
            *byte = u8::from_str_radix(pair, 16).map_err(|_| ())?;
        }
        Ok(Self(bytes))
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}:", Self::VERSION)?;
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

/// 決まった並びのバイト列を受ける所
pub struct Canon<'a> {
    hasher: blake3::Hasher,
    check: Option<&'a mut dyn FnMut() -> Result<(), Stop>>,
    stop: Option<Stop>,
}

impl Canon<'_> {
    fn bytes(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(1024) {
            if self.stop.is_some() {
                break;
            }
            if let Some(check) = &mut self.check
                && let Err(stop) = check()
            {
                self.stop = Some(stop);
                break;
            }
            self.hasher.update(chunk);
        }
    }

    fn tag(&mut self, tag: u8) {
        self.bytes(&[tag]);
    }
}

/// 決まった並びのバイト列にできる値
pub trait Encode {
    /// 値のバイト列を書き足す
    fn encode(&self, out: &mut Canon<'_>);
}

impl Encode for bool {
    fn encode(&self, out: &mut Canon<'_>) {
        out.tag(u8::from(*self));
    }
}

macro_rules! integers {
    ($($ty:ty),*) => {$(
        impl Encode for $ty {
            fn encode(&self, out: &mut Canon<'_>) {
                out.bytes(&self.to_le_bytes());
            }
        }
    )*};
}

integers!(u8, u16, u32, u64, i64);

impl Encode for usize {
    fn encode(&self, out: &mut Canon<'_>) {
        (*self as u64).encode(out);
    }
}

impl Encode for f64 {
    fn encode(&self, out: &mut Canon<'_>) {
        let bits = if self.is_nan() {
            f64::NAN.to_bits()
        } else if *self == 0.0 {
            0
        } else {
            self.to_bits()
        };
        bits.encode(out);
    }
}

impl Encode for [u8] {
    fn encode(&self, out: &mut Canon<'_>) {
        self.len().encode(out);
        out.bytes(self);
    }
}

impl Encode for str {
    fn encode(&self, out: &mut Canon<'_>) {
        self.as_bytes().encode(out);
    }
}

impl Encode for String {
    fn encode(&self, out: &mut Canon<'_>) {
        self.as_str().encode(out);
    }
}

impl Encode for &str {
    fn encode(&self, out: &mut Canon<'_>) {
        (*self).encode(out);
    }
}

impl<T: Encode> Encode for Vec<T> {
    fn encode(&self, out: &mut Canon<'_>) {
        self.len().encode(out);
        for item in self {
            if out.stop.is_some() {
                break;
            }
            item.encode(out);
        }
    }
}

impl<T: Encode, const N: usize> Encode for [T; N] {
    fn encode(&self, out: &mut Canon<'_>) {
        for item in self {
            if out.stop.is_some() {
                break;
            }
            item.encode(out);
        }
    }
}

impl<T: Encode> Encode for Option<T> {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            None => out.tag(0),
            Some(value) => {
                out.tag(1);
                value.encode(out);
            }
        }
    }
}

impl<T: Encode + ?Sized> Encode for Box<T> {
    fn encode(&self, out: &mut Canon<'_>) {
        (**self).encode(out);
    }
}

impl<A: Encode, B: Encode> Encode for (A, B) {
    fn encode(&self, out: &mut Canon<'_>) {
        self.0.encode(out);
        self.1.encode(out);
    }
}

/// 出所は、形式が持つ鍵だけを入れる。位置は、前のレコードが増えると変わる
impl Encode for Ref {
    fn encode(&self, out: &mut Canon<'_>) {
        match self.key() {
            None => out.tag(0),
            Some(Key::Handle(handle)) => {
                out.tag(1);
                handle.encode(out);
            }
            Some(Key::Instance(number)) => {
                out.tag(2);
                number.encode(out);
            }
        }
    }
}

/// フィールドを分解して並べる。フィールドが増えると、ここで型が合わなくなる
macro_rules! records {
    ($($ty:ident $(<$p:ident>)? { $($field:ident),* $(,)? })*) => {$(
        impl$(<$p: Encode>)? Encode for $ty$(<$p>)? {
            fn encode(&self, out: &mut Canon<'_>) {
                let $ty { $($field),* } = self;
                $($field.encode(out);)*
            }
        }
    )*};
}

/// 中身のない列挙は、宣言の順の番号を並べる
macro_rules! units {
    ($($ty:ident { $($variant:ident),* $(,)? })*) => {$(
        impl Encode for $ty {
            fn encode(&self, out: &mut Canon<'_>) {
                let tags = [$($ty::$variant),*];
                let tag = tags.iter().position(|tag| tag == self);
                match tag {
                    Some(tag) => out.tag(tag as u8),
                    None => panic!("every variant is listed"),
                }
            }
        }
    )*};
}

/// 1 つの値を持つ型
macro_rules! newtypes {
    ($($ty:ident),*) => {$(
        impl Encode for $ty {
            fn encode(&self, out: &mut Canon<'_>) {
                self.0.encode(out);
            }
        }
    )*};
}

newtypes!(SpaceId, SheetId, LayerId, LineTypeId, PaperLength);

units! {
    ColorSystem { Aci, Sxf, Jww }
    MarkRotation { Relative, Absolute, Upright }
    PatternBasis { Element, Paper }
    Axis { X, Y }
    UnderlayKind { Image, Pdf, Dwf, Dgn }
    ValueUnit { None, Distance, Angle, Area, Volume, Currency, Percentage }
    PathKind { Straight, Spline, Hidden }
    Islands { Nested, Outer, Ignore }
    PatternOrigin { UserDefined, Predefined, Custom }
    Horizontal { Left, Center, Right, Middle }
    Vertical { Baseline, Bottom, Middle, Top }
    Fit { Aligned, Fit }
    Flow { LeftToRight, TopToBottom, ByStyle }
    StackKind { Horizontal, Diagonal, Tolerance }
}

records! {
    Layer { id, name, state, style, follows_placement, source }
    LayerState { visible, printable, editable, frozen }
    LayerStyle { color, line_type, line_weight }
    Style { color, line_type, line_type_scale, line_weight }
    Color { code, rgb }
    ColorCode { system, value }
    LineType { id, name, definition, source }
    LineTypeDefinition { description, pattern, basis, marks }
    LineTypeMark { index, content, scale, rotation, rotation_basis, offset }
    ModelPoint { space, x, y }
    PaperPoint { sheet, x, y }
    ModelLength { space, value }
    Affine { m }
    Dimension<P> { kind, measurement, label, text, style, block }
    Measured { value, key }
    DimensionText<P> { position, moved, rotation, attachment }
    Leader<P> { path, spline, arrow, annotation, style }
    Underlay<P> { kind, resource, page, frame, clip, visible }
    Resource { path, definition }
    Clip { vertices, inverted }
    Table<P> { position, direction, row_heights, column_widths, cells, style, formats, drawn }
    Cell { content, span, covered, value, rotation, format }
    TableFormats { title, header, title_format, header_format, data_format }
    CellFormat { text_style, text_height, alignment, text_color, fill, fill_color, borders }
    Borders { top, right, bottom, left, horizontal, vertical }
    Border { color, weight, visible }
    CellValue { data, unit, format }
    DateTime { year, month, day, hour, minute, second, millisecond }
    Callout<P> { branches, content, path, arrowhead, arrow_size, landing, scale, line_style, content_color, style }
    Branch<P> { lines, end, dogleg }
    Dogleg { direction, length }
    AttributeValue { definition, text }
    SymbolKey { namespace, name }
    Symbol<P> { key, position, scale, rotation, grid, attributes, clip }
    Grid { columns, rows, column_spacing, row_spacing }
    Attribute<P> { tag, text, formatted, hidden, layer, style, source }
    Hatch<P> { loops, islands, fill, associative, seeds }
    HatchLoop<P> { boundary, external, outermost, derived, text_box, sources }
    Vertex<P> { point, bulge }
    Polyline<P> { vertices, closed, width, vertex_widths }
    Spline<P> { degree, knots, control, weights, fit, start_tangent, end_tangent, closed, periodic }
    Pattern { name, origin, angle, scale, double, lines }
    PatternLine { angle, base, offset, dashes }
    Gradient { name, angle, shift, colors, tint }
    Text<P> { content, height, placement, rotation, width_factor, oblique, backward, upside_down, style }
    Height { value, meaning }
    FormattedText<P> { content, spans, height, point, horizontal, vertical, rotation, width, frame_height, flow, line_spacing, background, style }
    LineSpacing { factor, exact }
    Background { fill, frame, scale, transparency }
    Span { piece, format }
    Format { height, width_factor, tracking, oblique, font, color, align, underline, overline, strike }
    Font { name, kind }
}

impl<T: Encode> Encode for Inherit<T> {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Inherit::Layer => out.tag(0),
            Inherit::Block => out.tag(1),
            Inherit::Own(value) => {
                out.tag(2);
                value.encode(out);
            }
        }
    }
}

impl Encode for MarkContent {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            MarkContent::Shape { font, number } => {
                out.tag(0);
                font.encode(out);
                number.encode(out);
            }
            MarkContent::Text { content, style } => {
                out.tag(1);
                content.encode(out);
                style.encode(out);
            }
        }
    }
}

impl Encode for LineWeight {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            LineWeight::Width(width) => {
                out.tag(0);
                width.encode(out);
            }
            LineWeight::Default => out.tag(1),
        }
    }
}

impl Encode for Length {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Length::Paper(length) => {
                out.tag(0);
                length.encode(out);
            }
            Length::Model(length) => {
                out.tag(1);
                length.encode(out);
            }
        }
    }
}

impl<P: Encode> Encode for Shape<P> {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Shape::Line { start, end } => {
                out.tag(0);
                start.encode(out);
                end.encode(out);
            }
            Shape::Circle { center, radius } => {
                out.tag(1);
                center.encode(out);
                radius.encode(out);
            }
            Shape::Curve(edge) => {
                out.tag(2);
                edge.encode(out);
            }
            Shape::Polyline(polyline) => {
                out.tag(3);
                polyline.encode(out);
            }
            Shape::Text(text) => {
                out.tag(4);
                text.encode(out);
            }
            Shape::Formatted(text) => {
                out.tag(5);
                text.encode(out);
            }
            Shape::Dimension(dimension) => {
                out.tag(6);
                dimension.encode(out);
            }
            Shape::Leader(leader) => {
                out.tag(7);
                leader.encode(out);
            }
            Shape::Callout(callout) => {
                out.tag(8);
                callout.encode(out);
            }
            Shape::Table(table) => {
                out.tag(9);
                table.encode(out);
            }
            Shape::Underlay(underlay) => {
                out.tag(10);
                underlay.encode(out);
            }
            Shape::Symbol(symbol) => {
                out.tag(11);
                symbol.encode(out);
            }
            Shape::Hatch(hatch) => {
                out.tag(12);
                hatch.encode(out);
            }
        }
    }
}

impl<P: Encode> Encode for DimensionKind<P> {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            DimensionKind::Linear {
                first,
                second,
                line,
                rotation,
                oblique,
            } => {
                out.tag(0);
                first.encode(out);
                second.encode(out);
                line.encode(out);
                rotation.encode(out);
                oblique.encode(out);
            }
            DimensionKind::Aligned {
                first,
                second,
                line,
                oblique,
            } => {
                out.tag(1);
                first.encode(out);
                second.encode(out);
                line.encode(out);
                oblique.encode(out);
            }
            DimensionKind::Angular {
                vertex,
                first,
                second,
                arc,
            } => {
                out.tag(2);
                vertex.encode(out);
                first.encode(out);
                second.encode(out);
                arc.encode(out);
            }
            DimensionKind::AngularLines { first, second, arc } => {
                out.tag(3);
                first.encode(out);
                second.encode(out);
                arc.encode(out);
            }
            DimensionKind::ArcLength {
                center,
                first,
                second,
                arc,
            } => {
                out.tag(4);
                center.encode(out);
                first.encode(out);
                second.encode(out);
                arc.encode(out);
            }
            DimensionKind::Radius {
                center,
                chord,
                leader,
            } => {
                out.tag(5);
                center.encode(out);
                chord.encode(out);
                leader.encode(out);
            }
            DimensionKind::Diameter {
                chord,
                opposite,
                leader,
            } => {
                out.tag(6);
                chord.encode(out);
                opposite.encode(out);
                leader.encode(out);
            }
            DimensionKind::Ordinate {
                origin,
                feature,
                leader_end,
                axis,
            } => {
                out.tag(7);
                origin.encode(out);
                feature.encode(out);
                leader_end.encode(out);
                axis.encode(out);
            }
        }
    }
}

impl Encode for Label {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Label::Measurement => out.tag(0),
            Label::Override { content, spans } => {
                out.tag(1);
                content.encode(out);
                spans.encode(out);
            }
            Label::Hidden => out.tag(2),
        }
    }
}

impl<P: Encode> Encode for UnderlayFrame<P> {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            UnderlayFrame::Pixels {
                size,
                origin,
                right,
                top,
            } => {
                out.tag(0);
                size.encode(out);
                origin.encode(out);
                right.encode(out);
                top.encode(out);
            }
            UnderlayFrame::Corners { origin, right, top } => {
                out.tag(1);
                origin.encode(out);
                right.encode(out);
                top.encode(out);
            }
            UnderlayFrame::Scaled {
                origin,
                scale,
                rotation,
            } => {
                out.tag(2);
                origin.encode(out);
                scale.encode(out);
                rotation.encode(out);
            }
        }
    }
}

impl Encode for CellData {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            CellData::Text(text) => {
                out.tag(0);
                text.encode(out);
            }
            CellData::Integer(value) => {
                out.tag(1);
                value.encode(out);
            }
            CellData::Real(value) => {
                out.tag(2);
                value.encode(out);
            }
            CellData::Date(date) => {
                out.tag(3);
                date.encode(out);
            }
            CellData::Point(point) => {
                out.tag(4);
                point.encode(out);
            }
        }
    }
}

impl Encode for CellContent {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            CellContent::Empty => out.tag(0),
            CellContent::Text { content, spans } => {
                out.tag(1);
                content.encode(out);
                spans.encode(out);
            }
            CellContent::Symbol(key) => {
                out.tag(2);
                key.encode(out);
            }
        }
    }
}

impl<P: Encode> Encode for CalloutContent<P> {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            CalloutContent::None => out.tag(0),
            CalloutContent::Text(text) => {
                out.tag(1);
                text.encode(out);
            }
            CalloutContent::Symbol { symbol, values } => {
                out.tag(2);
                symbol.encode(out);
                values.encode(out);
            }
        }
    }
}

impl Encode for Arrowhead {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Arrowhead::Default => out.tag(0),
            Arrowhead::Symbol(key) => {
                out.tag(1);
                key.encode(out);
            }
        }
    }
}

impl<P: Encode> Encode for Boundary<P> {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Boundary::Polyline { vertices, closed } => {
                out.tag(0);
                vertices.encode(out);
                closed.encode(out);
            }
            Boundary::Edges(edges) => {
                out.tag(1);
                edges.encode(out);
            }
        }
    }
}

impl<P: Encode> Encode for Edge<P> {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Edge::Line { start, end } => {
                out.tag(0);
                start.encode(out);
                end.encode(out);
            }
            Edge::Arc {
                center,
                radius,
                start,
                end,
                counterclockwise,
            } => {
                out.tag(1);
                center.encode(out);
                radius.encode(out);
                start.encode(out);
                end.encode(out);
                counterclockwise.encode(out);
            }
            Edge::Elliptic {
                center,
                major,
                ratio,
                start,
                end,
                counterclockwise,
            } => {
                out.tag(2);
                center.encode(out);
                major.encode(out);
                ratio.encode(out);
                start.encode(out);
                end.encode(out);
                counterclockwise.encode(out);
            }
            Edge::Spline(spline) => {
                out.tag(3);
                spline.encode(out);
            }
            Edge::Clothoid {
                origin,
                parameter,
                angle,
                counterclockwise,
                start,
                end,
            } => {
                out.tag(4);
                origin.encode(out);
                parameter.encode(out);
                angle.encode(out);
                counterclockwise.encode(out);
                start.encode(out);
                end.encode(out);
            }
        }
    }
}

impl Encode for Fill {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Fill::Solid => out.tag(0),
            Fill::Pattern(pattern) => {
                out.tag(1);
                pattern.encode(out);
            }
            Fill::Gradient(gradient) => {
                out.tag(2);
                gradient.encode(out);
            }
        }
    }
}

impl Encode for HeightMeaning {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            HeightMeaning::Frame => out.tag(0),
            HeightMeaning::CapHeight => out.tag(1),
            HeightMeaning::Em => out.tag(2),
            HeightMeaning::Declared(key) => {
                out.tag(3);
                key.encode(out);
            }
        }
    }
}

impl<P: Encode> Encode for Placement<P> {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Placement::Anchor {
                point,
                horizontal,
                vertical,
            } => {
                out.tag(0);
                point.encode(out);
                horizontal.encode(out);
                vertical.encode(out);
            }
            Placement::Between { first, second, fit } => {
                out.tag(1);
                first.encode(out);
                second.encode(out);
                fit.encode(out);
            }
        }
    }
}

impl Encode for Rotation {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Rotation::Angle(angle) => {
                out.tag(0);
                angle.encode(out);
            }
            Rotation::Direction(direction) => {
                out.tag(1);
                direction.encode(out);
            }
        }
    }
}

impl Encode for BackgroundFill {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            BackgroundFill::Color(color) => {
                out.tag(0);
                color.encode(out);
            }
            BackgroundFill::Screen => out.tag(1),
        }
    }
}

impl Encode for Piece {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Piece::Text(text) => {
                out.tag(0);
                text.encode(out);
            }
            Piece::Paragraph => out.tag(1),
            Piece::Column => out.tag(2),
            Piece::Measurement => out.tag(3),
            Piece::Below => out.tag(4),
            Piece::Stack { upper, lower, kind } => {
                out.tag(5);
                upper.encode(out);
                lower.encode(out);
                kind.encode(out);
            }
            Piece::Unparsed(text) => {
                out.tag(6);
                text.encode(out);
            }
        }
    }
}

impl<T: Encode> Encode for Scaled<T> {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Scaled::Value(value) => {
                out.tag(0);
                value.encode(out);
            }
            Scaled::Factor(factor) => {
                out.tag(1);
                factor.encode(out);
            }
        }
    }
}

impl Encode for FontKind {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            FontKind::Shape => out.tag(0),
            FontKind::Named {
                bold,
                italic,
                charset,
                pitch,
            } => {
                out.tag(1);
                bold.encode(out);
                italic.encode(out);
                charset.encode(out);
                pitch.encode(out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(x: f64, source: Ref) -> Element<ModelPoint> {
        let at = |x, y| ModelPoint {
            space: SpaceId(0),
            x,
            y,
        };
        Element {
            source,
            layer: LayerId(0),
            style: Style::BLOCK,
            shape: Shape::Line {
                start: at(x, 0.0),
                end: at(1.0, f64::NAN),
            },
        }
    }

    #[test]
    fn positions_do_not_change_the_hash_but_values_and_keys_do() {
        let hash = Hash::element(&line(0.0, Ref::at_record(3)));
        assert_eq!(hash, Hash::element(&line(0.0, Ref::at_record(9))));
        assert_eq!(hash, Hash::element(&line(-0.0, Ref::at_bytes(1, 2))));
        let nan = f64::from_bits(f64::NAN.to_bits() | 1);
        let mut other = line(0.0, Ref::default());
        if let Shape::Line { end, .. } = &mut other.shape {
            end.y = nan;
        }
        assert_eq!(hash, Hash::element(&other));
        assert_ne!(hash, Hash::element(&line(0.5, Ref::default())));
        let mut moved = line(0.0, Ref::default());
        moved.layer = LayerId(1);
        assert_ne!(hash, Hash::element(&moved));
    }

    #[test]
    fn checked_hashes_match_and_respect_work_and_cancellation() {
        use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};

        let bytes = vec![42; 4096];
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        assert_eq!(
            Hash::document_checked(&bytes, &mut cx).unwrap(),
            Hash::document(&bytes)
        );
        assert_eq!(
            Hash::document(b"").to_string(),
            "v1:af1349b9f5f9a1a6a0404dea36dcc949"
        );
        let element = line(1.0, Ref::default());
        assert_eq!(
            Hash::element_checked(&element, &mut cx).unwrap(),
            Hash::element(&element)
        );

        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(2)));
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        assert!(matches!(
            Hash::document_checked(&bytes, &mut cx),
            Err(Stop::Exceeded(_))
        ));
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(2)));
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        assert!(matches!(
            Hash::element_checked(&element, &mut cx),
            Err(Stop::Exceeded(_))
        ));
        cancel.cancel();
        let mut budget = Budget::default();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        assert_eq!(
            Hash::element_checked(&element, &mut cx),
            Err(Stop::Cancelled)
        );
    }

    #[test]
    fn encoding_stops_visiting_items_after_the_budget() {
        use std::cell::Cell;

        struct Count<'a>(&'a Cell<usize>);
        impl Encode for Count<'_> {
            fn encode(&self, out: &mut Canon<'_>) {
                self.0.set(self.0.get() + 1);
                1_u8.encode(out);
            }
        }
        let visits = Cell::new(0);
        let items: Vec<_> = (0..1000).map(|_| Count(&visits)).collect();
        let mut budget = zumen_base::Budget::new(
            zumen_base::Limits::default().with(Kind::Work, zumen_base::Limit::Finite(2)),
        );
        let mut diagnostics = zumen_base::Diagnostics::new();
        let cancel = zumen_base::CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let mut check = || cx.check(Kind::Work, 1);
        let mut out = Canon {
            hasher: blake3::Hasher::new(),
            check: Some(&mut check),
            stop: None,
        };
        items.encode(&mut out);
        assert!(matches!(out.stop, Some(Stop::Exceeded(_))));
        assert_eq!(visits.get(), 2);
    }

    #[test]
    fn hashes_are_written_with_the_version() {
        let text = Hash([0xab; 16]).to_string();
        assert_eq!(text, format!("v1:{}", "ab".repeat(16)));
        let hash = Hash([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 0xff]);
        assert_eq!(hash.to_string().parse(), Ok(hash));
        assert_eq!(
            format!("v1:{}", "AB".repeat(16)).parse(),
            Ok(Hash([0xab; 16]))
        );
        for wrong in [
            "",
            "v1:",
            "v2:00000000000000000000000000000000",
            "v1:0",
            "v1:+1000000000000000000000000000000",
            "v1:é0000000000000000000000000000000",
        ] {
            assert_eq!(wrong.parse::<Hash>(), Err(()), "{wrong}");
        }
    }
}
