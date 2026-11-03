//! hayro-interpret が塗る内容（経路・字形の並び・画像）を、用紙の要素にする（決定 0255）。
//!
//! hayro-interpret は、ページの内容を塗る順に `Device` に渡す。座標は、渡した変換（[`crate::page::Geometry`] に
//! [`DEVICE_PER_MM`] を掛けたもの）の装置の単位。経路は線分・折れ線・曲線（3 次のベジェ曲線をそのまま持つ
//! スプライン）とハッチングに、字形の並びは文字に、画像は下敷きにする。移せないもの・形を変えたものは、診断にする。

use std::collections::{BTreeMap, BTreeSet};

use hayro_interpret::font::{Glyph, GlyphRun};
use hayro_interpret::hayro_cmap::{BfString, WritingMode};
use hayro_interpret::hayro_syntax::object::{Array, Dict, ObjectIdentifier};
use hayro_interpret::kurbo::{Affine, BezPath, PathEl, Point, Rect};
use hayro_interpret::pattern::Pattern;
use hayro_interpret::{
    BlendMode, ClipPath, Device, DrawMode, DrawProps, FillRule, Image, ImageDrawProps, Paint,
    SoftMask, StrokeProps,
};
use zumen_base::{Code, Diagnostic, Kind, Ref, Stop, codes};
use zumen_drawing::{
    Boundary, Color, Edge, Element, Fill, Font, FontKind, Hatch, HatchLoop, Height, HeightMeaning,
    Horizontal, Inherit, Islands, LayerId, Length, LineType, LineTypeDefinition, LineTypeId,
    LineWeight, PaperLength, PaperPoint, PatternBasis, Placement, Polyline, Resource, Shape,
    SheetId, Spline, Style, Text, TextStyle, TextStyleDefinition, Transparency, Underlay,
    UnderlayFrame, UnderlayKind, Vertex, Vertical,
};
use zumen_sdk::Cx;

use crate::color::{self, Mapped};

/// 用紙の 1 mm の装置の単位の数。2 の累乗なので、装置の座標を mm に戻しても値は変わらない。面積のない塗りを
/// hayro-interpret は 1 装置単位の幅の線にするので、その線は 1/1024 mm の細い線になる
pub(crate) const DEVICE_PER_MM: f64 = 1024.0;

/// 表示グループ（optional content group）の外の内容を置くレイヤ
pub(crate) const PAGE_LAYER: LayerId = LayerId(0);

/// 破線の長さで、hayro-interpret が 0 の代わりに置く値（ユーザー空間）。これ以下は点（0）に戻す
const DASH_ZERO: f32 = 0.01;

/// hayro-interpret が、UTF-8 でない名前の代わりに返す文字列（hayro-syntax の `Name::as_str`）
const NON_UTF8_NAME: &str = "{non-ascii key}";

/// 装置の座標 → 用紙の mm
fn to_mm() -> Affine {
    Affine::scale(1.0 / DEVICE_PER_MM)
}

/// 図面全体で共有する、線種と文字スタイルの表
#[derive(Debug, Default)]
pub(crate) struct Styles {
    /// 破線の列（用紙の mm のビット列）→ 線種
    line_types: BTreeMap<Vec<u64>, LineTypeId>,
    /// 線種。作った順
    pub(crate) line_type_list: Vec<LineType>,
    /// 文字スタイルの名前
    text_style_names: BTreeSet<String>,
    /// 文字スタイル。作った順
    pub(crate) text_style_list: Vec<TextStyle>,
}

impl Styles {
    /// 破線の列（用紙の mm、正は線、負は間、0 は点）の線種。名前は、PDF の破線の配列の書き方で、mm の値を
    /// 小数 3 桁まで並べたもの。実線は `[]`
    fn line_type(&mut self, pattern: Vec<f64>) -> LineTypeId {
        let key: Vec<u64> = pattern.iter().map(|value| value.to_bits()).collect();
        if let Some(id) = self.line_types.get(&key) {
            return *id;
        }
        let id = LineTypeId(u32::try_from(self.line_type_list.len()).unwrap_or(u32::MAX));
        let lengths: Vec<String> = pattern
            .iter()
            .map(|value| millimeters(value.abs()))
            .collect();
        self.line_type_list.push(LineType {
            id,
            name: format!("[{}]", lengths.join(" ")),
            definition: Some(LineTypeDefinition {
                description: String::new(),
                pattern,
                basis: PatternBasis::Paper,
                marks: Vec::new(),
            }),
            source: Ref::default(),
        });
        self.line_types.insert(key, id);
        id
    }

    /// フォントの文字スタイル。同じ名前は 1 度だけ作る。名前が空なら作らない
    fn text_style(&mut self, name: &str, font: Option<Font>, vertical: bool) {
        if name.is_empty() || !self.text_style_names.insert(name.to_owned()) {
            return;
        }
        self.text_style_list.push(TextStyle {
            name: name.to_owned(),
            definition: Some(TextStyleDefinition {
                font,
                big_font: None,
                height: None,
                width: 1.0,
                oblique: 0.0,
                backward: false,
                upside_down: false,
                vertical,
            }),
            source: Ref::default(),
        });
    }
}

/// 長さの名前。小数 3 桁で丸め、後ろの 0 を除く
fn millimeters(value: f64) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

/// 経路の 1 つの部分経路（8.5.2）
#[derive(Debug)]
struct Subpath {
    start: Point,
    segments: Vec<Segment>,
    closed: bool,
}

#[derive(Clone, Copy, Debug)]
enum Segment {
    Line(Point),
    Cubic(Point, Point, Point),
}

impl Segment {
    fn end(self) -> Point {
        match self {
            Segment::Line(end) | Segment::Cubic(_, _, end) => end,
        }
    }

    /// 制御点（終点を含む）
    fn points(self) -> Vec<Point> {
        match self {
            Segment::Line(end) => vec![end],
            Segment::Cubic(first, second, end) => vec![first, second, end],
        }
    }
}

impl Subpath {
    fn end(&self) -> Point {
        self.segments
            .last()
            .map_or(self.start, |segment| segment.end())
    }

    fn curved(&self) -> bool {
        self.segments
            .iter()
            .any(|segment| matches!(segment, Segment::Cubic(..)))
    }

    /// 始点と制御点の外接矩形
    fn bounds(&self) -> Option<Bounds> {
        Bounds::of(
            std::iter::once(self.start)
                .chain(self.segments.iter().flat_map(|segment| segment.points())),
        )
    }
}

/// 経路を部分経路に分ける。閉じた部分経路の後ろの線は、その始点から続く（8.5.2.1）。2 次の曲線は、同じ形の
/// 3 次の曲線にする
fn subpaths(path: &BezPath) -> Vec<Subpath> {
    let mut out: Vec<Subpath> = Vec::new();
    // 閉じた部分経路の後ろに線が続けば、その始点から新しい部分経路を始める
    let current = |out: &mut Vec<Subpath>| {
        let start = match out.last() {
            Some(last) if !last.closed => return,
            Some(last) => last.start,
            None => Point::ZERO,
        };
        out.push(Subpath {
            start,
            segments: Vec::new(),
            closed: false,
        });
    };
    for element in path.elements() {
        match *element {
            PathEl::MoveTo(point) => out.push(Subpath {
                start: point,
                segments: Vec::new(),
                closed: false,
            }),
            PathEl::LineTo(point) => {
                current(&mut out);
                if let Some(last) = out.last_mut() {
                    last.segments.push(Segment::Line(point));
                }
            }
            PathEl::QuadTo(control, end) => {
                current(&mut out);
                if let Some(last) = out.last_mut() {
                    let from = last.end();
                    let first = from + (control - from) * (2.0 / 3.0);
                    let second = end + (control - end) * (2.0 / 3.0);
                    last.segments.push(Segment::Cubic(first, second, end));
                }
            }
            PathEl::CurveTo(first, second, end) => {
                current(&mut out);
                if let Some(last) = out.last_mut() {
                    last.segments.push(Segment::Cubic(first, second, end));
                }
            }
            PathEl::ClosePath => {
                if let Some(last) = out.last_mut() {
                    last.closed = true;
                }
            }
        }
    }
    out
}

/// 点の外接矩形
#[derive(Clone, Copy, Debug)]
struct Bounds {
    min: Point,
    max: Point,
}

impl Bounds {
    /// 有限の点の外接矩形。有限の点がなければ `None`
    fn of(points: impl IntoIterator<Item = Point>) -> Option<Self> {
        let mut out: Option<Bounds> = None;
        for point in points {
            if !(point.x.is_finite() && point.y.is_finite()) {
                continue;
            }
            out = Some(match out {
                None => Bounds {
                    min: point,
                    max: point,
                },
                Some(bounds) => Bounds {
                    min: Point::new(bounds.min.x.min(point.x), bounds.min.y.min(point.y)),
                    max: Point::new(bounds.max.x.max(point.x), bounds.max.y.max(point.y)),
                },
            });
        }
        out
    }

    fn rect(self) -> Rect {
        Rect::new(self.min.x, self.min.y, self.max.x, self.max.y)
    }
}

/// 経路の制御点の外接矩形。曲線は、制御点の凸包の中にある
fn path_bounds(path: &BezPath) -> Option<Bounds> {
    Bounds::of(path.elements().iter().flat_map(|element| match *element {
        PathEl::MoveTo(point) | PathEl::LineTo(point) => vec![point],
        PathEl::QuadTo(control, end) => vec![control, end],
        PathEl::CurveTo(first, second, end) => vec![first, second, end],
        PathEl::ClosePath => Vec::new(),
    }))
}

/// 切り抜きに対する要素の位置
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Clipped {
    Inside,
    Crossing,
    Outside,
}

/// 要素の外接矩形を、切り抜きの外接矩形と比べる（推定。切り抜きの形ではなく外接矩形で比べる）
fn clipped(bounds: Bounds, clip: Rect) -> Clipped {
    // 用紙の mm での、比べるときの余裕
    const SLACK: f64 = 1e-6;
    if bounds.max.x < clip.x0 - SLACK
        || bounds.min.x > clip.x1 + SLACK
        || bounds.max.y < clip.y0 - SLACK
        || bounds.min.y > clip.y1 + SLACK
    {
        Clipped::Outside
    } else if bounds.min.x >= clip.x0 - SLACK
        && bounds.max.x <= clip.x1 + SLACK
        && bounds.min.y >= clip.y0 - SLACK
        && bounds.max.y <= clip.y1 + SLACK
    {
        Clipped::Inside
    } else {
        Clipped::Crossing
    }
}

/// 塗りの字形の並びの、比べるための値
#[derive(Clone, Debug, PartialEq)]
struct RunKey {
    content: String,
    first: [f64; 6],
    count: usize,
}

/// ページの内容を集める
pub(crate) struct Collector<'c, 'x> {
    cx: &'c mut Cx<'x>,
    /// 要素の出所（ページの最初の内容のストリームか、ページのオブジェクト）
    source: Ref,
    sheet: SheetId,
    /// 表示グループのオブジェクト → レイヤ
    ocgs: &'c BTreeMap<(u32, i32), LayerId>,
    /// 間接オブジェクト → L1 のレコードの出所
    objects: &'c BTreeMap<(u32, i32), Ref>,
    styles: &'c mut Styles,
    pub(crate) elements: Vec<Element<PaperPoint>>,
    /// 切り抜きの外接矩形（用紙の mm）の重なり。最初はページ
    clips: Vec<Rect>,
    /// 印つきの内容の入れ子。表示グループなら、そのレイヤ
    marks: Vec<Option<LayerId>>,
    /// 透明のグループの不透明度を掛け合わせたもの（0〜1）の重なり
    groups: Vec<f32>,
    /// 直前の塗りの字形の並び。続く同じ並びの線（描画モード 2）は、同じ文字にする
    last_fill_run: Option<RunKey>,
    /// 場所を添えた診断。同じ code と値の 2 つ目からは場所を添えない
    placed: BTreeSet<(&'static str, String)>,
    /// 予算か中断の合図で止めた理由
    pub(crate) stopped: Option<Stop>,
}

impl<'c, 'x> Collector<'c, 'x> {
    pub(crate) fn new(
        cx: &'c mut Cx<'x>,
        source: Ref,
        sheet: SheetId,
        page: Rect,
        ocgs: &'c BTreeMap<(u32, i32), LayerId>,
        objects: &'c BTreeMap<(u32, i32), Ref>,
        styles: &'c mut Styles,
    ) -> Self {
        Self {
            cx,
            source,
            sheet,
            ocgs,
            objects,
            styles,
            elements: Vec::new(),
            clips: vec![page],
            marks: Vec::new(),
            groups: Vec::new(),
            last_fill_run: None,
            placed: BTreeSet::new(),
            stopped: None,
        }
    }

    fn clip(&self) -> Rect {
        self.clips.last().copied().unwrap_or(Rect::ZERO)
    }

    fn layer(&self) -> LayerId {
        self.marks
            .iter()
            .rev()
            .find_map(|mark| *mark)
            .unwrap_or(PAGE_LAYER)
    }

    fn opacity(&self) -> f32 {
        self.groups.last().copied().unwrap_or(1.0)
    }

    fn point(&self, point: Point) -> PaperPoint {
        PaperPoint {
            sheet: self.sheet,
            x: point.x,
            y: point.y,
        }
    }

    /// 予算を使う。止めたら、以後は何もしない
    fn spend(&mut self, kind: Kind, amount: u64) -> bool {
        if self.stopped.is_some() {
            return false;
        }
        match self.cx.check(kind, amount) {
            Ok(()) => true,
            Err(stop) => {
                self.cx.report(stop.to_diagnostic().at(self.source.clone()));
                self.stopped = Some(stop);
                false
            }
        }
    }

    /// 診断を足す。ページの出所は、同じ code と値の最初の 1 件にだけ添える
    fn note(&mut self, diagnostic: Diagnostic, value: &str) {
        let diagnostic = if self
            .placed
            .insert((diagnostic.code().name(), value.to_owned()))
        {
            diagnostic.at(self.source.clone())
        } else {
            diagnostic
        };
        self.cx.report(diagnostic);
    }

    fn note_code(&mut self, code: Code) {
        self.note(Diagnostic::new(code), "");
    }

    /// 外接矩形の要素が、切り抜きの外で捨てられないか。捨てる要素には、ほかの診断を返さない
    fn shown(&self, bounds: Option<Bounds>) -> bool {
        bounds.is_none_or(|bounds| clipped(bounds, self.clip()) != Clipped::Outside)
    }

    /// 要素を足す。切り抜きの外なら足さない
    fn push(&mut self, shape: Shape<PaperPoint>, style: Style, bounds: Option<Bounds>) {
        if let Some(bounds) = bounds {
            match clipped(bounds, self.clip()) {
                Clipped::Outside => {
                    self.note_code(codes::PDF_CLIP_DROPPED);
                    return;
                }
                Clipped::Crossing => self.note_code(codes::PDF_CLIP_APPROXIMATED),
                Clipped::Inside => {}
            }
        }
        if !self.spend(Kind::Elements, 1) {
            return;
        }
        self.elements.push(Element {
            source: self.source.clone(),
            layer: self.layer(),
            style,
            shape,
        });
    }

    /// 塗りの色と不透明度。シェーディングとパターンは移さない
    fn paint(&mut self, paint: &Paint<'_>) -> Option<(Inherit<Color>, Inherit<Transparency>)> {
        match paint {
            Paint::Color(color) => Some(self.color(color)),
            Paint::Pattern(pattern) => {
                let code = match **pattern {
                    Pattern::Shading(_) => codes::PDF_SHADING_DROPPED,
                    Pattern::Tiling(_) => codes::PDF_PATTERN_DROPPED,
                };
                self.note_code(code);
                None
            }
        }
    }

    fn color(
        &mut self,
        color: &hayro_interpret::color::Color,
    ) -> (Inherit<Color>, Inherit<Transparency>) {
        let (mapped, alpha) = color::map(color);
        let rgb = |rgb| {
            Inherit::Own(Color {
                code: None,
                rgb: Some(rgb),
            })
        };
        let color = match mapped {
            Mapped::Exact(value) => rgb(value),
            Mapped::Approximated(value, space) => {
                self.note(
                    Diagnostic::new(codes::PDF_COLOR_APPROXIMATED).with("space", space),
                    space,
                );
                rgb(value)
            }
            Mapped::Dropped(space) => {
                self.note(
                    Diagnostic::new(codes::PDF_COLOR_DROPPED).with("space", space),
                    space,
                );
                Inherit::Layer
            }
        };
        (color, self.transparency(alpha))
    }

    /// 不透明度（0〜255）に透明のグループの不透明度を掛けた指定。不透明ならレイヤから継承する（PDF のレイヤは
    /// 透過性を持たないので、不透明になる）
    fn transparency(&self, alpha: u8) -> Inherit<Transparency> {
        let alpha = (f32::from(alpha) * self.opacity() + 0.5) as u8;
        if alpha == u8::MAX {
            Inherit::Layer
        } else {
            Inherit::Own(Transparency { alpha })
        }
    }

    fn fill(&mut self, path: &BezPath, paint: &Paint<'_>, rule: FillRule) {
        let Some((color, transparency)) = self.paint(paint) else {
            return;
        };
        let loops: Vec<HatchLoop<PaperPoint>> = subpaths(path)
            .iter()
            .filter(|subpath| !subpath.segments.is_empty())
            .map(|subpath| HatchLoop {
                boundary: self.boundary(subpath),
                external: false,
                outermost: false,
                derived: false,
                text_box: false,
                sources: Vec::new(),
            })
            .collect();
        if loops.is_empty() {
            return;
        }
        // 偶奇の規則は、輪の中の輪を交互に埋めるのと同じ。非ゼロ回転数の規則は、輪が 1 つなら同じになる（8.5.3.3）
        if rule == FillRule::NonZero && loops.len() > 1 {
            self.note_code(codes::PDF_FILL_RULE_APPROXIMATED);
        }
        let style = Style {
            color,
            line_type: Inherit::Layer,
            line_type_scale: 1.0,
            line_weight: None,
            transparency,
        };
        let hatch = Hatch {
            loops,
            islands: Islands::Nested,
            fill: Fill::Solid,
            associative: false,
            seeds: Vec::new(),
        };
        self.push(Shape::Hatch(hatch), style, path_bounds(path));
    }

    fn boundary(&self, subpath: &Subpath) -> Boundary<PaperPoint> {
        if !subpath.curved() {
            let mut vertices = vec![self.vertex(subpath.start)];
            vertices.extend(
                subpath
                    .segments
                    .iter()
                    .map(|segment| self.vertex(segment.end())),
            );
            // 始点に戻ってから閉じた輪は、重なる最後の点を除く
            if vertices.len() > 2 && subpath.end() == subpath.start {
                vertices.pop();
            }
            return Boundary::Polyline {
                vertices,
                closed: true,
            };
        }
        let mut edges = Vec::new();
        let mut from = subpath.start;
        for segment in &subpath.segments {
            edges.push(self.edge(from, *segment));
            from = segment.end();
        }
        if from != subpath.start {
            edges.push(self.edge(from, Segment::Line(subpath.start)));
        }
        Boundary::Edges(edges)
    }

    fn vertex(&self, point: Point) -> Vertex<PaperPoint> {
        Vertex {
            point: self.point(point),
            bulge: 0.0,
        }
    }

    fn edge(&self, from: Point, segment: Segment) -> Edge<PaperPoint> {
        match segment {
            Segment::Line(end) => Edge::Line {
                start: self.point(from),
                end: self.point(end),
            },
            Segment::Cubic(first, second, end) => {
                Edge::Spline(self.bezier(&[from, first, second, end], 1))
            }
        }
    }

    /// 3 次のベジェ曲線をつないだスプライン。`count` 個の曲線の制御点を、つなぎ目を共有して並べたもの
    /// （`3 count + 1` 個）。節点はつなぎ目で 3 重にし、形を変えない
    fn bezier(&self, control: &[Point], count: usize) -> Spline<PaperPoint> {
        let mut knots = vec![0.0; 4];
        for index in 1..count {
            knots.extend([index as f64; 3]);
        }
        knots.extend([count as f64; 4]);
        Spline {
            degree: 3,
            knots,
            control: control.iter().map(|point| self.point(*point)).collect(),
            weights: Vec::new(),
            fit: Vec::new(),
            start_tangent: None,
            end_tangent: None,
            closed: false,
            periodic: false,
            fit_tolerance: None,
            knot_parameterization: None,
        }
    }

    fn stroke(
        &mut self,
        path: &BezPath,
        transform: Affine,
        paint: &Paint<'_>,
        stroke: &StrokeProps,
    ) {
        let Some((color, transparency)) = self.paint(paint) else {
            return;
        };
        // 線の太さと破線はユーザー空間の長さ。用紙の mm には、変換の平均の倍率を掛ける（推定。斜めに伸ばした
        // 変換では、向きで太さが変わる）
        let scale = libm::sqrt(transform.determinant().abs());
        let width = f64::from(stroke.line_width) * scale;
        let width = if width.is_finite() {
            width.max(0.0)
        } else {
            0.0
        };
        let line_type = self.dash(stroke, scale);
        let style = Style {
            color,
            line_type: Inherit::Own(line_type),
            line_type_scale: 1.0,
            line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(width)))),
            transparency,
        };
        for subpath in subpaths(path) {
            if subpath.segments.is_empty() {
                continue;
            }
            let shape = self.stroke_shape(&subpath);
            self.push(shape, style, subpath.bounds());
            if self.stopped.is_some() {
                return;
            }
        }
    }

    fn stroke_shape(&self, subpath: &Subpath) -> Shape<PaperPoint> {
        if subpath.curved() {
            let mut control = vec![subpath.start];
            let mut from = subpath.start;
            let mut segments = subpath.segments.clone();
            if subpath.closed && subpath.end() != subpath.start {
                segments.push(Segment::Line(subpath.start));
            }
            for segment in &segments {
                match *segment {
                    // 線分は、同じ形の 3 次の曲線にする
                    Segment::Line(end) => control.extend([
                        from + (end - from) * (1.0 / 3.0),
                        from + (end - from) * (2.0 / 3.0),
                        end,
                    ]),
                    Segment::Cubic(first, second, end) => control.extend([first, second, end]),
                }
                from = segment.end();
            }
            let mut spline = self.bezier(&control, segments.len());
            spline.closed = subpath.closed;
            return Shape::Curve(Edge::Spline(spline));
        }
        if let [Segment::Line(end)] = subpath.segments.as_slice()
            && !subpath.closed
        {
            return Shape::Line {
                start: self.point(subpath.start),
                end: self.point(*end),
            };
        }
        let mut vertices = vec![self.vertex(subpath.start)];
        vertices.extend(
            subpath
                .segments
                .iter()
                .map(|segment| self.vertex(segment.end())),
        );
        if subpath.closed && vertices.len() > 2 && subpath.end() == subpath.start {
            vertices.pop();
        }
        Shape::Polyline(Polyline {
            vertices,
            closed: subpath.closed,
            width: None,
            vertex_widths: Vec::new(),
            line_type_generation: false,
        })
    }

    /// 破線（8.4.3.6）の線種。数が奇数の列は、2 回並べて線と間をそろえる。位相は持てないので、0 でなければ診断する
    fn dash(&mut self, stroke: &StrokeProps, scale: f64) -> LineTypeId {
        let lengths: &[f32] = &stroke.dash_array;
        let dash = |length: f32| length.is_finite() && length > DASH_ZERO;
        // 長さがすべて 0 の列は、実線とみなす（推定。仕様は、すべてが 0 の列を認めない）
        if !lengths.iter().any(|length| dash(*length)) {
            return self.styles.line_type(Vec::new());
        }
        if stroke.dash_offset != 0.0 {
            self.note_code(codes::PDF_DASH_APPROXIMATED);
        }
        let repeat = if lengths.len() % 2 == 1 { 2 } else { 1 };
        let pattern: Vec<f64> = lengths
            .iter()
            .cycle()
            .take(lengths.len() * repeat)
            .enumerate()
            .map(|(index, length)| {
                let mm = if dash(*length) {
                    f64::from(*length) * scale
                } else {
                    0.0
                };
                if index % 2 == 0 { mm } else { -mm }
            })
            .collect();
        self.styles.line_type(pattern)
    }

    /// 表示グループの辞書のレイヤ。表示の規則の辞書（OCMD、8.11.2.2）は、最初の表示グループのレイヤにする
    fn layer_of(&mut self, properties: &Dict<'_>) -> Option<LayerId> {
        let find = |id: Option<ObjectIdentifier>| {
            id.and_then(|id| self.ocgs.get(&(id.obj_number, id.gen_number)).copied())
        };
        if let Some(layer) = find(properties.obj_id()) {
            return Some(layer);
        }
        let first = properties
            .get_ref(b"OCGs")
            .map(ObjectIdentifier::from)
            .or_else(|| {
                properties
                    .get::<Array<'_>>(b"OCGs")
                    .and_then(|groups| groups.raw_iter().find_map(|item| item.as_obj_ref()))
                    .map(ObjectIdentifier::from)
            });
        let layer = find(first);
        if layer.is_some() {
            self.note_code(codes::PDF_OPTIONAL_CONTENT_APPROXIMATED);
        }
        layer
    }

    /// 字形の並び（9.4）を 1 つの文字にする。並びの最初の字形の基準線が文字の基点と向きで、em の大きさが高さ
    fn glyph_run(&mut self, run: &GlyphRun<'_, '_>, props: &DrawProps<'_>, mode: &DrawMode) {
        let glyphs = run.glyphs();
        let Some(first) = glyphs.first() else {
            return;
        };
        let mut missing = 0u64;
        let mut content = String::new();
        for glyph in glyphs {
            match glyph.as_unicode() {
                Some(BfString::Char(character)) => content.push(character),
                Some(BfString::String(text)) => content.push_str(&text),
                None => {
                    missing += 1;
                    content.push(char::REPLACEMENT_CHARACTER);
                }
            }
        }
        let transform = to_mm() * props.transform * first.transform();
        let key = RunKey {
            content: content.clone(),
            first: transform.as_coeffs(),
            count: glyphs.len(),
        };
        // 塗りと線の両方で描く文字（描画モード 2）は、塗りの後に同じ並びの線が来る。1 つの文字にする
        match mode {
            DrawMode::Stroke(_) if self.last_fill_run.as_ref() == Some(&key) => {
                self.last_fill_run = None;
                return;
            }
            DrawMode::Fill(_) => self.last_fill_run = Some(key),
            _ => self.last_fill_run = None,
        }
        // 空白だけの並びは、何も描かない
        if content.chars().all(char::is_whitespace) {
            return;
        }
        // 切り抜きは、文字の基点で比べる
        let origin = transform * Point::ZERO;
        let bounds = Bounds::of([origin]);
        if self.shown(bounds) {
            if missing > 0 {
                self.note_code(codes::PDF_GLYPH_DROPPED);
                for _ in 1..missing {
                    self.cx.report(Diagnostic::new(codes::PDF_GLYPH_DROPPED));
                }
            }
            // Type 3 の字形は、内容のストリームで描く形を持つ（9.6.4）。形は移さず、書体のない文字にする
            if glyphs
                .iter()
                .any(|glyph| matches!(&**glyph, Glyph::Type3(_)))
            {
                self.note_code(codes::PDF_TYPE3_APPROXIMATED);
            }
        }
        let Some((color, transparency)) = self.paint(&props.paint) else {
            return;
        };
        // 見えない文字（描画モード 3。スキャンした画像に重ねた文字など）は、透明な文字にする
        let transparency = match mode {
            DrawMode::Invisible => Inherit::Own(Transparency { alpha: 0 }),
            _ => transparency,
        };
        // 文字の座標系: 字形の x 軸が基準線の向き、y 軸が em の上向き（字形の座標は em の 1/1000）
        let ex = transform * Point::new(1000.0, 0.0) - origin;
        let ey = transform * Point::new(0.0, 1000.0) - origin;
        let width = libm::hypot(ex.x, ex.y);
        let cross = ex.x * ey.y - ex.y * ey.x;
        if !(width > 0.0 && cross.is_finite() && cross != 0.0) {
            return;
        }
        // em の高さ（基準線に直交する向き）と、幅の倍率（水平の伸縮 Tz と、文字の行列の歪み）
        let height = cross.abs() / width;
        let (ux, uy) = (ex.x / width, ex.y / width);
        // em の上向きを、基準線の向きと、それに直交する向きに分けた値。斜体の角度になる
        let along = ey.x * ux + ey.y * uy;
        let across = (ey.y * ux - ey.x * uy) * cross.signum();
        let vertical = run.writing_mode() == WritingMode::Vertical;
        let (font, name) = self.font_of(first);
        // 縦書きのフォントの文字スタイルは、Windows の縦書きのフォントの名前と同じく `@` を前に付ける
        let name = if vertical && !name.is_empty() {
            format!("@{name}")
        } else {
            name
        };
        self.styles.text_style(&name, font, vertical);
        let (horizontal, vertical_alignment) = if vertical {
            (Horizontal::Center, Vertical::Top)
        } else {
            (Horizontal::Left, Vertical::Baseline)
        };
        let text = Text {
            content,
            height: Height {
                value: Length::Paper(PaperLength(height)),
                meaning: HeightMeaning::Em,
            },
            placement: Placement::Anchor {
                point: self.point(origin),
                horizontal,
                vertical: vertical_alignment,
            },
            rotation: libm::atan2(uy, ux).to_degrees(),
            width_factor: width / height,
            oblique: libm::atan2(along, across).to_degrees(),
            backward: cross < 0.0,
            upside_down: false,
            style: name,
        };
        let style = Style {
            color,
            line_type: Inherit::Layer,
            line_type_scale: 1.0,
            line_weight: None,
            transparency,
        };
        self.push(Shape::Text(text), style, bounds);
    }

    /// 字形のフォントと、その名前（埋め込んだ部分集合の前置きを除いたもの）。名前の分からないフォント（Type 1・
    /// 埋め込みのない TrueType・Type 3）は空。UTF-8 でない名前は空にして診断する
    fn font_of(&mut self, glyph: &Glyph<'_>) -> (Option<Font>, String) {
        let Glyph::Outline(outline) = glyph else {
            return (None, String::new());
        };
        let Some(data) = outline.font_data() else {
            return (None, String::new());
        };
        let Some(name) = data.postscript_name.filter(|name| !name.is_empty()) else {
            return (None, String::new());
        };
        if name == NON_UTF8_NAME {
            self.note_code(codes::PDF_FONT_NAME_DROPPED);
            return (None, String::new());
        }
        let font = Font {
            name: name.clone(),
            kind: FontKind::Named {
                bold: data.weight.is_some_and(|weight| weight >= 700),
                italic: data.is_italic,
                charset: None,
                pitch: None,
            },
        };
        (Some(font), name)
    }

    fn image(&mut self, image: &Image<'_, '_>, props: &ImageDrawProps<'_>) {
        let (width, height) = (f64::from(image.width()), f64::from(image.height()));
        // 画像の変換は、画素の座標（左上が原点、Y が下）から装置へ（8.9.5、hayro-interpret の約束）
        let transform = to_mm() * props.transform;
        let origin = transform * Point::new(0.0, height);
        let right = transform * Point::new(width, height);
        let top = transform * Point::new(0.0, 0.0);
        let opposite = transform * Point::new(width, 0.0);
        let stream = match image {
            Image::Raster(raster) => raster.stream(),
            Image::Stencil(stencil) => stencil.stream(),
        };
        // 画像の XObject は、間接オブジェクトの参照（`n g R`）を名前にする。内容の中に書いた画像（BI … EI）は、
        // 名前を持たない
        let (path, definition) = match stream.dict().obj_id() {
            Some(id) => match self.objects.get(&(id.obj_number, id.gen_number)) {
                Some(record) => (
                    format!("{} {} R", id.obj_number, id.gen_number),
                    record.clone(),
                ),
                None => (String::new(), self.source.clone()),
            },
            None => (String::new(), self.source.clone()),
        };
        let bounds = Bounds::of([origin, right, top, opposite]);
        if self.shown(bounds) {
            // 色を塗る型の画像（ステンシル、8.9.6.2）の色は移さない。色は、マスクを展開しないと得られない
            if matches!(image, Image::Stencil(_)) {
                self.note_code(codes::PDF_IMAGE_MASK_APPROXIMATED);
            }
            // 柔らかいマスクと、通常でない合成の方法は移さない（11.3・11.6.5）
            if props.soft_mask.is_some() || props.blend_mode != BlendMode::Normal {
                self.note_code(codes::PDF_TRANSPARENCY_APPROXIMATED);
            }
        }
        let style = Style {
            color: Inherit::Layer,
            line_type: Inherit::Layer,
            line_type_scale: 1.0,
            line_weight: None,
            transparency: self.transparency(u8::MAX),
        };
        let underlay = Underlay {
            kind: UnderlayKind::Image,
            resource: Resource { path, definition },
            page: None,
            frame: UnderlayFrame::Pixels {
                size: [width, height],
                origin: self.point(origin),
                right: self.point(right),
                top: self.point(top),
            },
            clip: None,
            visible: true,
        };
        self.push(Shape::Underlay(underlay), style, bounds);
    }
}

impl<'a> Device<'a> for Collector<'_, '_> {
    fn draw_path(&mut self, path: &BezPath, props: DrawProps<'a>, draw_mode: &DrawMode) {
        let count = u64::try_from(path.elements().len()).unwrap_or(u64::MAX);
        if !self.spend(Kind::Work, count.saturating_add(1)) {
            return;
        }
        self.last_fill_run = None;
        let transform = to_mm() * props.transform;
        let path = transform * path.clone();
        match draw_mode {
            DrawMode::Fill(rule) => self.fill(&path, &props.paint, *rule),
            DrawMode::Stroke(stroke) => self.stroke(&path, transform, &props.paint, stroke),
            DrawMode::FillAndStroke(rule, stroke) => {
                self.fill(&path, &props.paint, *rule);
                self.stroke(&path, transform, &props.paint, stroke);
            }
            DrawMode::Invisible => {}
        }
    }

    fn draw_rect(&mut self, rect: &Rect, props: DrawProps<'a>, draw_mode: &DrawMode) {
        // 長方形（re、8.5.2.1）は、左下の角から始まる閉じた経路にする。hayro-interpret は、長方形の経路を角の
        // 座標にして渡す
        let mut path = BezPath::new();
        path.move_to((rect.x0, rect.y0));
        path.line_to((rect.x1, rect.y0));
        path.line_to((rect.x1, rect.y1));
        path.line_to((rect.x0, rect.y1));
        path.close_path();
        self.draw_path(&path, props, draw_mode);
    }

    fn push_clip_path(&mut self, clip_path: &ClipPath) {
        // 切り抜きの経路は装置の座標
        let clip = match path_bounds(&(to_mm() * clip_path.path.clone())) {
            Some(bounds) => self.clip().intersect(bounds.rect()),
            None => Rect::ZERO,
        };
        self.clips.push(clip);
    }

    fn push_clip_rect(&mut self, rect: &Rect) {
        let clip = self.clip().intersect(to_mm().transform_rect_bbox(*rect));
        self.clips.push(clip);
    }

    fn push_transparency_group(
        &mut self,
        opacity: f32,
        mask: Option<SoftMask<'a>>,
        blend_mode: BlendMode,
    ) {
        // 柔らかいマスクと、通常でない合成の方法は移さない（11.3・11.6.5）
        if mask.is_some() || blend_mode != BlendMode::Normal {
            self.note_code(codes::PDF_TRANSPARENCY_APPROXIMATED);
        }
        let opacity = if opacity.is_finite() {
            opacity.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.groups.push(self.opacity() * opacity);
    }

    fn draw_glyph_run(
        &mut self,
        glyph_run: &GlyphRun<'_, 'a>,
        props: DrawProps<'a>,
        draw_mode: &DrawMode,
    ) {
        let count = u64::try_from(glyph_run.glyphs().len()).unwrap_or(u64::MAX);
        if !self.spend(Kind::Work, count.saturating_add(1)) {
            return;
        }
        self.glyph_run(glyph_run, &props, draw_mode);
    }

    fn draw_image(&mut self, image: Image<'a, '_>, props: ImageDrawProps<'a>) {
        if !self.spend(Kind::Work, 1) {
            return;
        }
        self.last_fill_run = None;
        self.image(&image, &props);
    }

    fn pop_clip(&mut self) {
        // ページの切り抜きは残す
        if self.clips.len() > 1 {
            self.clips.pop();
        }
    }

    fn pop_transparency_group(&mut self) {
        self.groups.pop();
    }

    fn begin_marked_content(&mut self, tag: &[u8], properties: Option<&Dict<'_>>) {
        let layer = match (tag, properties) {
            (b"OC", Some(properties)) => self.layer_of(properties),
            _ => None,
        };
        self.marks.push(layer);
    }

    fn end_marked_content(&mut self) {
        self.marks.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_lengths_in_millimeters() {
        assert_eq!(millimeters(3.527_777_777_777_778), "3.528");
        assert_eq!(millimeters(1.0), "1");
        assert_eq!(millimeters(0.0), "0");
        assert_eq!(millimeters(0.25), "0.25");
    }

    #[test]
    fn splits_paths_into_subpaths() {
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.line_to((1.0, 0.0));
        path.quad_to((2.0, 1.0), (3.0, 0.0));
        path.close_path();
        // 閉じた後の線は、閉じた部分経路の始点から始まる
        path.line_to((5.0, 5.0));
        let parts = subpaths(&path);
        assert_eq!(parts.len(), 2);
        assert!(parts[0].closed && parts[0].curved());
        assert_eq!(parts[1].start, Point::ZERO);
        assert_eq!(parts[1].end(), Point::new(5.0, 5.0));
        // 2 次の曲線の制御点は、3 次の曲線の 1/3・2/3 の所に移す
        let Segment::Cubic(first, second, _) = parts[0].segments[1] else {
            panic!("曲線でない");
        };
        assert!((first.x - 5.0 / 3.0).abs() < 1e-12 && (first.y - 2.0 / 3.0).abs() < 1e-12);
        assert!((second.x - 7.0 / 3.0).abs() < 1e-12 && (second.y - 2.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn compares_with_the_clip_bounds() {
        let clip = Rect::new(0.0, 0.0, 10.0, 10.0);
        let bounds = |x0, y0, x1, y1| Bounds {
            min: Point::new(x0, y0),
            max: Point::new(x1, y1),
        };
        assert_eq!(clipped(bounds(1.0, 1.0, 2.0, 2.0), clip), Clipped::Inside);
        assert_eq!(
            clipped(bounds(9.0, 9.0, 11.0, 11.0), clip),
            Clipped::Crossing
        );
        assert_eq!(
            clipped(bounds(11.0, 0.0, 12.0, 1.0), clip),
            Clipped::Outside
        );
    }
}
