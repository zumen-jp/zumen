//! 2D のシーン（L3、決定 0137・0172）。図面モデルの配置をたどり（決定 0159）、解いた値を、線・面・文字・点・
//! 画像の区画と、それらが引く表に並べた 1 つのバッファーにする。区画の配置は `schema/scene.toml` にあり、
//! [`layout`] に生成してある（決定 0178）。
//!
//! - 曲線・ふくらみのある折れ線は、起点の長さの許容差で折れ線にする（`zumen-geom2d`）。作った点は予算の
//!   `segments` で数え、使った許容差を `scene.curve.approximated` で返す。
//! - 線種は、線と間に展開する。線種に添える形と文字は、まだ描かない（`scene.line-type-mark.dropped`）。
//! - 寸法・引出線・注記・表と、ハッチングのパターンの線は、まだ描かない（`scene.<種類>.dropped`）。

pub mod layout;
mod write;

use std::collections::BTreeMap;

use zumen_base::{Cx, Diagnostic, Kind, Ref, Stop, codes};
use zumen_drawing::transform::Point as Spot;
use zumen_drawing::{
    Affine, Boundary, Clipping, Color, ColorSystem, Drawing2D, Edge, Fill, Flow, FormattedText,
    Hatch, Horizontal, Islands, Item, Length, LineTypeId, Origin, Piece, Placement, PointLook,
    PointMark, PointSize, Polyline, Rotation, Scaled, Shape, Text, Underlay, UnderlayFrame,
    UnderlayKind, Vertex, Vertical, Visit,
};
use zumen_geom2d::{self as geom, Xy};

use crate::layout::{color_flag, color_system, entity_kind, horizontal, text_flag, vertical};

/// 塊の分け方。塊の大きさは実際の図面で測って決める（決定 0137・0172、#219）。それまでの仮の値を
/// [`Chunking::PROVISIONAL`] に置く
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chunking {
    /// 塊ごとの描くものの数の上限
    pub items: usize,
    /// 塊の範囲の幅と高さの上限の、許容差に対する倍率。f32 の相対座標の刻み（範囲の 2^-24 倍）を、
    /// 許容差より十分に小さく保つ
    pub span: f64,
}

impl Chunking {
    /// 仮の値（決定 0178）。描くもの 4096 個、許容差の 2^16 倍
    pub const PROVISIONAL: Self = Self {
        items: 4096,
        span: 65536.0,
    };
}

/// 色の表（決定 0172）。系統と符号から RGB を引く
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Palette {
    entries: BTreeMap<(u8, u32), [u8; 3]>,
}

impl Palette {
    /// 系統の符号に RGB を足す。同じ符号があれば、先に足したものを使う
    pub fn insert(&mut self, system: ColorSystem, code: u32, rgb: [u8; 3]) {
        self.entries
            .entry((system_code(system), code))
            .or_insert(rgb);
    }

    fn get(&self, system: ColorSystem, code: u32) -> Option<[u8; 3]> {
        self.entries.get(&(system_code(system), code)).copied()
    }
}

fn system_code(system: ColorSystem) -> u8 {
    match system {
        ColorSystem::Aci => color_system::ACI,
        ColorSystem::Sxf => color_system::SXF,
        ColorSystem::Jww => color_system::JWW,
    }
}

/// シーンの作り方
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// 起点
    pub origin: Origin,
    /// 弦の誤差（起点の長さ）。`None` なら、用紙と用紙のビューからは 0.02 mm、空間からは範囲の対角線の
    /// 1/50000（決定 0172）
    pub tolerance: Option<f64>,
    /// 印刷する要素を描く。偽なら表示する要素を描く
    pub print: bool,
    /// 起点の座標の長方形 `[x の最小, y の最小, x の最大, y の最大]`。重なる塊だけを返す
    pub region: Option<[f64; 4]>,
    /// 色の表
    pub palette: Palette,
    /// 塊の分け方
    pub chunking: Chunking,
}

/// 描くものの種類ごとの数
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// 塊
    pub chunks: u64,
    /// 線
    pub lines: u64,
    /// 線と面の頂点
    pub vertices: u64,
    /// 面
    pub faces: u64,
    /// 文字
    pub texts: u64,
    /// 点
    pub points: u64,
    /// 画像
    pub images: u64,
}

/// 作ったシーン
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// シーンのバッファー（`schema/scene.toml` の配置）
    pub bytes: Vec<u8>,
    /// 使った許容差（起点の長さ）
    pub tolerance: f64,
    /// 返した描くものの数
    pub counts: Counts,
    /// 返した塊の範囲 `[x の最小, y の最小, x の最大, y の最大]`。塊がなければ `None`
    pub extent: Option<[f64; 4]>,
}

/// 用紙からの既定の許容差（mm、決定 0018）
pub const PAPER_TOLERANCE: f64 = 0.02;

/// 空間からの既定の許容差の、範囲の対角線に対する割合の逆数（決定 0172）
pub const SPACE_TOLERANCE_DIVISOR: f64 = 50000.0;

/// 起点から配置をたどり、シーンを作る。たどる要素は `work`、作った点は `segments` で数える
pub fn build(
    drawing: &Drawing2D,
    options: &Options,
    cx: &mut Cx<'_>,
) -> Result<Scene, (Stop, Ref)> {
    let visits = drawing.walk(options.origin, cx)?;
    let shown = |visit: &Visit<'_>| {
        if options.print {
            visit.printable
        } else {
            visit.visible
        }
    };
    let tolerance = match options.tolerance {
        Some(tolerance) => tolerance,
        None => match options.origin {
            Origin::Sheet(_) | Origin::View { .. } => PAPER_TOLERANCE,
            Origin::Space(_) => {
                let mut bounds = Bounds::default();
                for visit in visits.iter().filter(|v| shown(v)) {
                    rough(visit, &mut bounds);
                }
                match bounds.get() {
                    Some([x0, y0, x1, y1]) if x1 - x0 + (y1 - y0) > 0.0 => {
                        libm::hypot(x1 - x0, y1 - y0) / SPACE_TOLERANCE_DIVISOR
                    }
                    _ => 1.0,
                }
            }
        },
    };
    let mut builder = Builder::new(drawing, tolerance, options, cx);
    for visit in visits.iter().filter(|v| shown(v)) {
        builder.visit(visit)?;
    }
    if builder.approximated {
        builder
            .cx
            .report(Diagnostic::new(codes::SCENE_CURVE_APPROXIMATED).with("tolerance", tolerance));
    }
    Ok(builder.finish(options.region))
}

/// 塊の中の線
struct LineItem {
    points: Vec<Xy>,
    unit: u32,
}

/// 塊の中の面
struct FaceItem {
    rings: Vec<Vec<Xy>>,
    unit: u32,
}

/// 塊の中の文字
struct TextItem {
    string: [u32; 2],
    at: Xy,
    second: Option<Xy>,
    height: f64,
    width: f64,
    rotation: f64,
    oblique: f64,
    box_width: f64,
    align: [u8; 2],
    flags: u8,
    line: u32,
    font: u32,
    unit: u32,
}

/// 塊の中の点
struct PointItem {
    at: Xy,
    rotation: f64,
    marker: [u32; 4],
    scale: f64,
    unit: u32,
}

/// 塊の中の画像
struct ImageItem {
    name: [u32; 2],
    page: [u32; 2],
    at: Xy,
    edges: [Xy; 2],
    kind: u8,
    frame: u8,
    unit: u32,
}

/// 描く順の続きの塊
#[derive(Default)]
struct Chunk {
    lines: Vec<LineItem>,
    faces: Vec<FaceItem>,
    texts: Vec<TextItem>,
    points: Vec<PointItem>,
    images: Vec<ImageItem>,
    bounds: Bounds,
    items: usize,
}

/// 軸に沿った範囲
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Bounds(Option<[f64; 4]>);

impl Bounds {
    fn add(&mut self, [x, y]: Xy) {
        if !(x.is_finite() && y.is_finite()) {
            return;
        }
        self.0 = Some(match self.0 {
            None => [x, y, x, y],
            Some([x0, y0, x1, y1]) => [x0.min(x), y0.min(y), x1.max(x), y1.max(y)],
        });
    }

    fn union(self, other: Bounds) -> Bounds {
        let mut out = self;
        if let Some([x0, y0, x1, y1]) = other.0 {
            out.add([x0, y0]);
            out.add([x1, y1]);
        }
        out
    }

    fn get(self) -> Option<[f64; 4]> {
        self.0
    }

    fn span(self) -> f64 {
        self.0
            .map_or(0.0, |[x0, y0, x1, y1]| (x1 - x0).max(y1 - y0))
    }
}

/// 描く単位
struct Unit {
    entity: u32,
    color: u32,
    weight: f32,
    clip: u32,
    layer: u32,
}

/// 要素
struct Entity {
    id: [u32; 2],
    kind: u8,
    path: u32,
}

/// 色の表の行
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ColorRow {
    system: u8,
    code: u32,
    file: [u8; 4],
    rgba: [u8; 4],
    flags: u8,
}

/// 文字列の区画
#[derive(Default)]
struct Strings {
    bytes: Vec<u8>,
    seen: BTreeMap<String, [u32; 2]>,
}

impl Strings {
    fn add(&mut self, text: &str) -> [u32; 2] {
        if text.is_empty() {
            return [0, 0];
        }
        if let Some(found) = self.seen.get(text) {
            return *found;
        }
        let at = [
            u32::try_from(self.bytes.len()).unwrap_or(u32::MAX),
            u32::try_from(text.len()).unwrap_or(u32::MAX),
        ];
        self.bytes.extend_from_slice(text.as_bytes());
        self.seen.insert(text.to_owned(), at);
        at
    }
}

/// 表に行を足す。同じ値があれば、その行を返す
fn row<K: Ord + Clone>(seen: &mut BTreeMap<K, u32>, rows: &mut Vec<K>, key: K) -> u32 {
    if let Some(found) = seen.get(&key) {
        return *found;
    }
    let index = u32::try_from(rows.len()).unwrap_or(layout::NONE);
    rows.push(key.clone());
    seen.insert(key, index);
    index
}

struct Builder<'d, 'c, 'x> {
    drawing: &'d Drawing2D,
    tolerance: f64,
    palette: &'d Palette,
    chunking: Chunking,
    cx: &'c mut Cx<'x>,
    approximated: bool,
    chunks: Vec<Chunk>,
    current: Chunk,
    strings: Strings,
    units: Vec<Unit>,
    entities: Vec<Entity>,
    steps: Vec<([u32; 2], [u32; 2])>,
    colors: Vec<ColorRow>,
    color_rows: BTreeMap<ColorRow, u32>,
    clips: Vec<Vec<Clipping>>,
    layers: Vec<[u32; 2]>,
    layer_rows: BTreeMap<u32, u32>,
    fonts: Vec<String>,
    font_rows: BTreeMap<String, u32>,
}

/// 変換の、いちばん大きく伸ばす向きの倍率（特異値の最大）
fn stretch(transform: &Affine) -> f64 {
    let [a, b, _, d, e, _] = transform.m;
    let sum = a * a + b * b + d * d + e * e;
    let det = a * e - b * d;
    let root = (sum * sum - 4.0 * det * det).max(0.0).sqrt();
    ((sum + root) / 2.0).sqrt()
}

/// 変換で写した、向き `degrees` の向き（度）と、その向きの長さの倍率
fn turned(transform: &Affine, degrees: f64) -> (f64, f64) {
    let r = degrees.to_radians();
    let [x, y] = transform.vector([libm::cos(r), libm::sin(r)]);
    (libm::atan2(y, x).to_degrees(), libm::hypot(x, y))
}

fn map(transform: &Affine, [x, y]: Xy) -> Xy {
    let (x, y) = transform.apply(x, y);
    [x, y]
}

fn length(length: Length) -> f64 {
    match length {
        Length::Model(model) => model.value,
        Length::Paper(paper) => paper.0,
    }
}

impl<'d, 'c, 'x> Builder<'d, 'c, 'x> {
    fn new(
        drawing: &'d Drawing2D,
        tolerance: f64,
        options: &'d Options,
        cx: &'c mut Cx<'x>,
    ) -> Self {
        Self {
            drawing,
            tolerance,
            palette: &options.palette,
            chunking: options.chunking,
            cx,
            approximated: false,
            chunks: Vec::new(),
            current: Chunk::default(),
            strings: Strings::default(),
            units: Vec::new(),
            entities: Vec::new(),
            steps: Vec::new(),
            colors: Vec::new(),
            color_rows: BTreeMap::new(),
            clips: Vec::new(),
            layers: Vec::new(),
            layer_rows: BTreeMap::new(),
            fonts: Vec::new(),
            font_rows: BTreeMap::new(),
        }
    }

    /// 作った点を `segments` で数える
    fn segments(&mut self, count: usize, at: &Ref) -> Result<(), (Stop, Ref)> {
        let count = u64::try_from(count).unwrap_or(u64::MAX);
        self.cx
            .check(Kind::Segments, count)
            .map_err(|stop| (stop, at.clone()))
    }

    fn visit(&mut self, visit: &Visit<'_>) -> Result<(), (Stop, Ref)> {
        let mut unit = None;
        match visit.item {
            Item::Model(element) => {
                self.shape(visit, &element.shape, &element.source, &mut unit)?;
            }
            Item::Paper(element) => {
                self.shape(visit, &element.shape, &element.source, &mut unit)?;
            }
            Item::ModelAttribute { symbol, index } => {
                if let Shape::Symbol(placed) = &symbol.shape
                    && let Some(attribute) = placed.attributes.get(index)
                {
                    self.attribute(visit, &attribute.text, attribute.formatted.as_ref())?;
                }
            }
            Item::PaperAttribute { symbol, index } => {
                if let Shape::Symbol(placed) = &symbol.shape
                    && let Some(attribute) = placed.attributes.get(index)
                {
                    self.attribute(visit, &attribute.text, attribute.formatted.as_ref())?;
                }
            }
        }
        Ok(())
    }

    fn attribute<P: Spot>(
        &mut self,
        visit: &Visit<'_>,
        text: &Text<P>,
        formatted: Option<&FormattedText<P>>,
    ) -> Result<(), (Stop, Ref)> {
        let mut unit = None;
        let unit = self.unit(visit, entity_kind::ATTRIBUTE, &mut unit);
        match formatted {
            Some(formatted) => self.formatted(visit, formatted, unit),
            None => self.text(visit, text, unit),
        }
        Ok(())
    }

    /// 描く単位。最初に描くものを足すときに作る
    fn unit(&mut self, visit: &Visit<'_>, kind: u8, made: &mut Option<u32>) -> u32 {
        if let Some(unit) = made {
            return *unit;
        }
        let id = self.strings.add(&visit.id);
        let path = u32::try_from(self.steps.len()).unwrap_or(layout::NONE);
        for step in &visit.path {
            let id = self.strings.add(&step.id);
            self.steps.push((id, step.cell));
        }
        let entity = u32::try_from(self.entities.len()).unwrap_or(layout::NONE);
        self.entities.push(Entity { id, kind, path });
        let color = self.color(visit.color);
        let clip = if visit.clips.is_empty() {
            layout::NONE
        } else {
            match self.clips.iter().position(|c| *c == visit.clips) {
                Some(found) => u32::try_from(found).unwrap_or(layout::NONE),
                None => {
                    self.clips.push(visit.clips.clone());
                    u32::try_from(self.clips.len() - 1).unwrap_or(layout::NONE)
                }
            }
        };
        let layer = self.layer(visit.layer.0);
        let index = u32::try_from(self.units.len()).unwrap_or(layout::NONE);
        self.units.push(Unit {
            entity,
            color,
            // 線幅は用紙の mm。読まなかった線幅は、文書の既定の値で解いてある。それもなければ、いちばん細い線
            weight: visit.line_weight.map_or(0.0, |w| w.0 as f32),
            clip,
            layer,
        });
        *made = Some(index);
        index
    }

    fn color(&mut self, color: Option<Color>) -> u32 {
        let (system, code) = color
            .and_then(|c| c.code)
            .map_or((color_system::NONE, 0), |c| {
                (system_code(c.system), c.value)
            });
        let file = color.and_then(|c| c.rgb);
        let looked = color
            .and_then(|c| c.code)
            .and_then(|c| self.palette.get(c.system, c.value));
        let shown = file.or(looked);
        let mut flags = 0;
        if shown.is_none() {
            flags |= color_flag::UNRESOLVED;
        }
        if system == color_system::ACI && code == 7 {
            flags |= color_flag::FOREGROUND;
        }
        let key = ColorRow {
            system,
            code,
            file: file.map_or([0; 4], |[r, g, b]| [r, g, b, 255]),
            rgba: shown.map_or([0; 4], |[r, g, b]| [r, g, b, 255]),
            flags,
        };
        row(&mut self.color_rows, &mut self.colors, key)
    }

    fn layer(&mut self, id: u32) -> u32 {
        if let Some(found) = self.layer_rows.get(&id) {
            return *found;
        }
        let name = self
            .drawing
            .layers
            .iter()
            .find(|layer| layer.id.0 == id)
            .map(zumen_drawing::id::layer)
            .unwrap_or_default();
        let string = self.strings.add(&name);
        let index = u32::try_from(self.layers.len()).unwrap_or(layout::NONE);
        self.layers.push(string);
        self.layer_rows.insert(id, index);
        index
    }

    fn font(&mut self, name: Option<&str>) -> u32 {
        let Some(name) = name.filter(|n| !n.is_empty()) else {
            return layout::NONE;
        };
        row(&mut self.font_rows, &mut self.fonts, name.to_owned())
    }

    /// 塊に描くものを足す。範囲か数が上限を超えるなら、先に塊を閉じる
    fn place(&mut self, bounds: Bounds) {
        let merged = self.current.bounds.union(bounds);
        let limit = self.tolerance * self.chunking.span;
        if self.current.items > 0
            && (self.current.items >= self.chunking.items || merged.span() > limit)
        {
            let done = std::mem::take(&mut self.current);
            self.chunks.push(done);
            self.current.bounds = bounds;
        } else {
            self.current.bounds = merged;
        }
        self.current.items += 1;
    }

    fn line(&mut self, points: Vec<Xy>, unit: u32) {
        if points.len() < 2 {
            return;
        }
        let mut bounds = Bounds::default();
        points.iter().for_each(|p| bounds.add(*p));
        self.place(bounds);
        self.current.lines.push(LineItem { points, unit });
    }

    fn face(&mut self, rings: Vec<Vec<Xy>>, unit: u32) {
        let rings: Vec<Vec<Xy>> = rings.into_iter().filter(|r| r.len() >= 3).collect();
        if rings.is_empty() {
            return;
        }
        let mut bounds = Bounds::default();
        rings.iter().flatten().for_each(|p| bounds.add(*p));
        self.place(bounds);
        self.current.faces.push(FaceItem { rings, unit });
    }

    fn shape<P: Spot>(
        &mut self,
        visit: &Visit<'_>,
        shape: &Shape<P>,
        source: &Ref,
        unit: &mut Option<u32>,
    ) -> Result<(), (Stop, Ref)> {
        let t = visit.transform;
        let local = self.tolerance / stretch(&t).max(f64::MIN_POSITIVE);
        let xy = |p: &P| map(&t, p.xy());
        match shape {
            Shape::Line { start, end } | Shape::Curve(Edge::Line { start, end }) => {
                let unit = self.unit(visit, entity_kind_of(shape), unit);
                self.stroke(visit, vec![xy(start), xy(end)], unit, source)?;
            }
            Shape::Circle { center, radius } => {
                let mut points = Vec::new();
                geom::arc(center.xy(), length(*radius), 0.0, 360.0, local, &mut points);
                self.approximated = true;
                self.segments(points.len(), source)?;
                let unit = self.unit(visit, entity_kind::CIRCLE, unit);
                let points = points.into_iter().map(|p| map(&t, p)).collect();
                self.stroke(visit, points, unit, source)?;
            }
            Shape::Curve(edge) => {
                let points = self.edge(edge, local, source)?;
                let unit = self.unit(visit, entity_kind::CURVE, unit);
                let points = points.into_iter().map(|p| map(&t, p)).collect();
                self.stroke(visit, points, unit, source)?;
            }
            Shape::Polyline(polyline) => {
                let unit = self.unit(visit, entity_kind::POLYLINE, unit);
                self.polyline(visit, polyline, local, unit, source)?;
            }
            Shape::Text(text) => {
                let unit = self.unit(visit, entity_kind::TEXT, unit);
                self.text(visit, text, unit);
            }
            Shape::Formatted(text) => {
                let unit = self.unit(visit, entity_kind::FORMATTED, unit);
                self.formatted(visit, text, unit);
            }
            Shape::Point(point) => {
                let unit = self.unit(visit, entity_kind::POINT, unit);
                let (rotation, _) = turned(&t, point.rotation);
                let (marker, scale) = match &point.look {
                    PointLook::Document => ([0; 4], 1.0),
                    PointLook::Marker { key, scale } => {
                        let [a, b] = self.strings.add(key.namespace);
                        let [c, d] = self.strings.add(&key.code);
                        ([a, b, c, d], *scale)
                    }
                };
                let at = xy(&point.position);
                let mut bounds = Bounds::default();
                bounds.add(at);
                self.place(bounds);
                self.current.points.push(PointItem {
                    at,
                    rotation,
                    marker,
                    scale,
                    unit,
                });
            }
            Shape::Underlay(underlay) => self.underlay(visit, underlay, source, unit),
            Shape::Hatch(hatch) => self.hatch(visit, hatch, local, source, unit)?,
            Shape::Dimension(_) => self.report(codes::SCENE_DIMENSION_DROPPED, source),
            Shape::Leader(_) => self.report(codes::SCENE_LEADER_DROPPED, source),
            Shape::Callout(_) => self.report(codes::SCENE_CALLOUT_DROPPED, source),
            Shape::Table(_) => self.report(codes::SCENE_TABLE_DROPPED, source),
            // 記号の中身は、たどった要素として描く
            Shape::Symbol(_) => {}
        }
        Ok(())
    }

    fn report(&mut self, code: zumen_base::Code, source: &Ref) {
        self.cx.report(Diagnostic::new(code).at(source.clone()));
    }

    /// 辺を、要素の空間の許容差 `local` で折れ線にする
    fn edge<P: Spot>(
        &mut self,
        edge: &Edge<P>,
        local: f64,
        source: &Ref,
    ) -> Result<Vec<Xy>, (Stop, Ref)> {
        let mut points = Vec::new();
        match edge {
            Edge::Line { start, end } => {
                points.push(start.xy());
                points.push(end.xy());
            }
            Edge::Arc {
                center,
                radius,
                start,
                end,
                counterclockwise,
            } => {
                let sweep = geom::sweep(*start, *end, *counterclockwise);
                geom::arc(
                    center.xy(),
                    length(*radius),
                    *start,
                    sweep,
                    local,
                    &mut points,
                );
                self.approximated = true;
            }
            Edge::Elliptic {
                center,
                major,
                ratio,
                start,
                end,
                counterclockwise,
            } => {
                let sweep = geom::sweep(*start, *end, *counterclockwise);
                geom::ellipse(
                    center.xy(),
                    major.xy(),
                    *ratio,
                    *start,
                    sweep,
                    local,
                    &mut points,
                );
                self.approximated = true;
            }
            Edge::Spline(spline) => {
                let control: Vec<Xy> = spline.control.iter().map(|p| p.xy()).collect();
                match geom::spline(
                    spline.degree,
                    &spline.knots,
                    &control,
                    &spline.weights,
                    local,
                ) {
                    Some(found) => {
                        points = found;
                        self.approximated = true;
                    }
                    None => {
                        // 制御点で決まらないものは、当てはめる点か制御点を結ぶ
                        points = if spline.fit.len() >= 2 {
                            spline.fit.iter().map(|p| p.xy()).collect()
                        } else {
                            control
                        };
                        self.report(codes::SCENE_SPLINE_APPROXIMATED, source);
                    }
                }
            }
            Edge::Clothoid {
                origin,
                parameter,
                angle,
                counterclockwise,
                start,
                end,
            } => {
                points = geom::clothoid(
                    origin.xy(),
                    length(*parameter),
                    *angle,
                    *counterclockwise,
                    length(*start),
                    length(*end),
                    local,
                );
                self.approximated = true;
            }
        }
        self.segments(points.len(), source)?;
        Ok(points)
    }

    /// 起点の座標の折れ線を、解いた線種で線と間に分けて描く
    fn stroke(
        &mut self,
        visit: &Visit<'_>,
        points: Vec<Xy>,
        unit: u32,
        source: &Ref,
    ) -> Result<(), (Stop, Ref)> {
        let Some((pattern, marks)) = self.pattern(visit) else {
            self.segments(points.len(), source)?;
            self.line(points, unit);
            return Ok(());
        };
        if marks {
            self.report(codes::SCENE_LINE_TYPE_MARK_DROPPED, source);
        }
        let period = geom::period(&pattern);
        // 列の 1 周が許容差より短ければ、間は見分けられないので、続けて描く
        if period.is_nan() || period <= self.tolerance {
            self.segments(points.len(), source)?;
            self.line(points, unit);
            return Ok(());
        }
        // 作る前に、作る線の数を見積もって数える
        let estimate = geom::length(&points) / period * pattern.len() as f64 * 2.0;
        let estimate = if estimate.is_finite() {
            estimate.min(u64::MAX as f64) as usize
        } else {
            usize::MAX
        };
        self.segments(estimate.saturating_add(points.len()), source)?;
        for dash in geom::dashes(&points, &pattern) {
            self.line(dash, unit);
        }
        Ok(())
    }

    /// 解いた線種の、起点の長さの列と、添える形と文字を持つか。続く線なら `None`
    fn pattern(&self, visit: &Visit<'_>) -> Option<(Vec<f64>, bool)> {
        let id: LineTypeId = visit.line_type?;
        let definition = self.drawing.line_type(id)?.definition.as_ref()?;
        if definition.pattern.is_empty() {
            return None;
        }
        let own = match visit.item {
            Item::Model(element) => element.style.line_type_scale,
            Item::Paper(element) => element.style.line_type_scale,
            Item::ModelAttribute { .. } | Item::PaperAttribute { .. } => 1.0,
        };
        let document = self.drawing.line_type_scale.unwrap_or(1.0);
        // 要素の空間の長さは、要素から起点への変換で伸ばす。用紙の mm は、用紙からならそのまま使う。
        // 空間からは用紙への尺度が決まらないので、起点の長さとみなす（決定 0178）
        let basis = match definition.basis {
            zumen_drawing::PatternBasis::Element => {
                let [a, b, _, d, e, _] = visit.transform.m;
                (a * e - b * d).abs().sqrt()
            }
            zumen_drawing::PatternBasis::Paper => 1.0,
        };
        let scale = own * document * basis;
        if !(scale.is_finite() && scale > 0.0) {
            return None;
        }
        Some((
            definition.pattern.iter().map(|v| v * scale).collect(),
            !definition.marks.is_empty(),
        ))
    }

    fn polyline<P: Spot>(
        &mut self,
        visit: &Visit<'_>,
        polyline: &Polyline<P>,
        local: f64,
        unit: u32,
        source: &Ref,
    ) -> Result<(), (Stop, Ref)> {
        let t = visit.transform;
        let vertices: Vec<(Xy, f64)> = polyline
            .vertices
            .iter()
            .map(|v| (v.point.xy(), v.bulge))
            .collect();
        if vertices.iter().any(|(_, b)| *b != 0.0) {
            self.approximated = true;
        }
        let widths = widths(polyline);
        if widths.iter().any(|[a, b]| *a > 0.0 || *b > 0.0) {
            // 幅のある折れ線は、区間ごとに面にする
            let count = vertices.len();
            let segments = if polyline.closed {
                count
            } else {
                count.saturating_sub(1)
            };
            for i in 0..segments {
                let (Some(from), Some(to)) = (vertices.get(i), vertices.get((i + 1) % count))
                else {
                    continue;
                };
                let mut points = vec![from.0];
                geom::bulge(from.0, to.0, from.1, local, &mut points);
                self.segments(points.len() * 2, source)?;
                let [w0, w1] = widths.get(i).copied().unwrap_or([0.0, 0.0]);
                let ring = band(&points, w0, w1);
                let ring = ring.into_iter().map(|p| map(&t, p)).collect();
                self.face(vec![ring], unit);
            }
            return Ok(());
        }
        let points = geom::polyline(&vertices, polyline.closed, local);
        self.segments(points.len(), source)?;
        let points = points.into_iter().map(|p| map(&t, p)).collect();
        self.stroke(visit, points, unit, source)
    }

    fn text<P: Spot>(&mut self, visit: &Visit<'_>, text: &Text<P>, unit: u32) {
        let t = visit.transform;
        let (rotation, along) = turned(&t, text.rotation);
        let (_, up) = turned(&t, text.rotation + 90.0);
        let [a, b, _, d, e, _] = t.m;
        let mirrored = a * e - b * d < 0.0;
        let (at, second, align) = match text.placement {
            Placement::Anchor {
                point,
                horizontal,
                vertical,
            } => (
                map(&t, point.xy()),
                None,
                [horizontal_code(horizontal), vertical_code(vertical)],
            ),
            Placement::Between { first, second, fit } => (
                map(&t, first.xy()),
                Some(map(&t, second.xy())),
                [
                    match fit {
                        zumen_drawing::Fit::Aligned => horizontal::ALIGNED,
                        zumen_drawing::Fit::Fit => horizontal::FIT,
                    },
                    vertical::BASELINE,
                ],
            ),
        };
        let style = self
            .drawing
            .text_style(&text.style)
            .and_then(|s| s.definition.as_ref());
        let font = self.font(style.and_then(|s| s.font.as_ref()).map(|f| f.name.as_str()));
        let mut flags = 0;
        if text.backward {
            flags |= text_flag::BACKWARD;
        }
        if text.upside_down != mirrored {
            flags |= text_flag::UPSIDE_DOWN;
        }
        if style.is_some_and(|s| s.vertical) {
            flags |= text_flag::VERTICAL;
        }
        let string = self.strings.add(&text.content);
        self.put_text(TextItem {
            string,
            at,
            second,
            height: length(text.height.value) * up,
            width: if up > 0.0 {
                text.width_factor * along / up
            } else {
                text.width_factor
            },
            rotation,
            oblique: text.oblique,
            box_width: 0.0,
            align,
            flags,
            line: 0,
            font,
            unit,
        });
    }

    fn put_text(&mut self, item: TextItem) {
        let mut bounds = Bounds::default();
        bounds.add(item.at);
        if let Some(second) = item.second {
            bounds.add(second);
        }
        self.place(bounds);
        self.current.texts.push(item);
    }

    /// 書式つきの文字を、書式の区間ごとの文字にする。行の中の位置は、表示する側が送る（決定 0172）
    fn formatted<P: Spot>(&mut self, visit: &Visit<'_>, text: &FormattedText<P>, unit: u32) {
        let t = visit.transform;
        let angle = match text.rotation {
            Rotation::Angle(angle) => angle,
            Rotation::Direction([x, y]) => libm::atan2(y, x).to_degrees(),
        };
        let (rotation, along) = turned(&t, angle);
        let (_, up) = turned(&t, angle + 90.0);
        let [a, b, _, d, e, _] = t.m;
        let mirrored = a * e - b * d < 0.0;
        let at = map(&t, text.point.xy());
        let style = self
            .drawing
            .text_style(&text.style)
            .and_then(|s| s.definition.as_ref());
        let base_font = style.and_then(|s| s.font.as_ref()).map(|f| f.name.clone());
        let base_height = length(text.height.value) * up;
        let mut flags = text_flag::FORMATTED;
        let vertical = match text.flow {
            Flow::TopToBottom => true,
            Flow::LeftToRight => false,
            Flow::ByStyle => style.is_some_and(|s| s.vertical),
        };
        if vertical {
            flags |= text_flag::VERTICAL;
        }
        if mirrored {
            flags |= text_flag::UPSIDE_DOWN;
        }
        let align = [
            horizontal_code(text.horizontal),
            vertical_code(text.vertical),
        ];
        let box_width = text.width.map_or(0.0, |w| length(w) * along);
        let mut line = 0u32;
        for span in &text.spans {
            let height = match span.format.height {
                Some(Scaled::Value(value)) => length(value) * up,
                Some(Scaled::Factor(factor)) => base_height * factor,
                None => base_height,
            };
            let width = match span.format.width_factor {
                Some(Scaled::Value(value) | Scaled::Factor(value)) => value,
                None => 1.0,
            };
            let font_name = span
                .format
                .font
                .as_ref()
                .map(|f| f.name.clone())
                .or_else(|| base_font.clone());
            let font = self.font(font_name.as_deref());
            let oblique = span.format.oblique.unwrap_or(0.0);
            let put = |builder: &mut Self, content: &str, extra: u8| {
                let string = builder.strings.add(content);
                builder.put_text(TextItem {
                    string,
                    at,
                    second: None,
                    height,
                    width,
                    rotation,
                    oblique,
                    box_width,
                    align,
                    flags: flags | extra,
                    line,
                    font,
                    unit,
                });
            };
            match &span.piece {
                Piece::Text(content) => put(self, content, 0),
                Piece::Stack { upper, lower, .. } => {
                    put(self, upper, text_flag::STACK_UPPER);
                    put(self, lower, text_flag::STACK_LOWER);
                }
                Piece::Paragraph => line = line.saturating_add(1),
                _ => {}
            }
        }
    }

    fn underlay<P: Spot>(
        &mut self,
        visit: &Visit<'_>,
        underlay: &Underlay<P>,
        source: &Ref,
        unit: &mut Option<u32>,
    ) {
        if !underlay.visible {
            return;
        }
        let kind = match underlay.kind {
            UnderlayKind::Image => layout::image_kind::IMAGE,
            UnderlayKind::Pdf => layout::image_kind::PDF,
            UnderlayKind::Dwf | UnderlayKind::Dgn => {
                self.report(codes::SCENE_UNDERLAY_DROPPED, source);
                return;
            }
        };
        if underlay.clip.is_some() {
            self.report(codes::SCENE_UNDERLAY_CLIP_DROPPED, source);
        }
        let t = visit.transform;
        let (at, edges, frame) = match &underlay.frame {
            UnderlayFrame::Pixels {
                origin, right, top, ..
            }
            | UnderlayFrame::Corners { origin, right, top } => {
                let o = map(&t, origin.xy());
                let r = map(&t, right.xy());
                let u = map(&t, top.xy());
                (
                    o,
                    [[r[0] - o[0], r[1] - o[1]], [u[0] - o[0], u[1] - o[1]]],
                    layout::image_frame::CORNERS,
                )
            }
            UnderlayFrame::Scaled {
                origin,
                scale,
                rotation,
            } => {
                let r = rotation.to_radians();
                let (c, s) = (libm::cos(r), libm::sin(r));
                let [sx, sy] = [length(scale[0]), length(scale[1])];
                (
                    map(&t, origin.xy()),
                    [t.vector([sx * c, sx * s]), t.vector([-sy * s, sy * c])],
                    layout::image_frame::UNIT,
                )
            }
        };
        let unit = self.unit(visit, entity_kind::UNDERLAY, unit);
        let name = self.strings.add(&underlay.resource.path);
        let page = self.strings.add(underlay.page.as_deref().unwrap_or(""));
        let mut bounds = Bounds::default();
        bounds.add(at);
        if frame == layout::image_frame::CORNERS {
            let [u, v] = edges;
            bounds.add([at[0] + u[0], at[1] + u[1]]);
            bounds.add([at[0] + v[0], at[1] + v[1]]);
            bounds.add([at[0] + u[0] + v[0], at[1] + u[1] + v[1]]);
        }
        self.place(bounds);
        self.current.images.push(ImageItem {
            name,
            page,
            at,
            edges,
            kind,
            frame,
            unit,
        });
    }

    fn hatch<P: Spot>(
        &mut self,
        visit: &Visit<'_>,
        hatch: &Hatch<P>,
        local: f64,
        source: &Ref,
        unit: &mut Option<u32>,
    ) -> Result<(), (Stop, Ref)> {
        match &hatch.fill {
            Fill::Solid => {}
            Fill::Gradient(_) => self.report(codes::SCENE_GRADIENT_APPROXIMATED, source),
            Fill::Pattern(_) => {
                self.report(codes::SCENE_HATCH_PATTERN_DROPPED, source);
                return Ok(());
            }
        }
        let t = visit.transform;
        // 内側の輪を無視するなら、いちばん外の輪だけを塗る。ほかは偶奇の規則で塗る
        let outermost = hatch.loops.iter().any(|l| l.outermost);
        let mut rings = Vec::new();
        for (index, ring) in hatch.loops.iter().enumerate() {
            if hatch.islands == Islands::Ignore && !(ring.outermost || (!outermost && index == 0)) {
                continue;
            }
            let mut points = match &ring.boundary {
                Boundary::Polyline { vertices, .. } => {
                    let vertices: Vec<(Xy, f64)> = vertices
                        .iter()
                        .map(|v: &Vertex<P>| (v.point.xy(), v.bulge))
                        .collect();
                    if vertices.iter().any(|(_, b)| *b != 0.0) {
                        self.approximated = true;
                    }
                    let points = geom::polyline(&vertices, true, local);
                    self.segments(points.len(), source)?;
                    points
                }
                Boundary::Edges(edges) => {
                    let mut points: Vec<Xy> = Vec::new();
                    for edge in edges {
                        let part = self.edge(edge, local, source)?;
                        // 前の辺の終わりと同じ点は繰り返さない
                        let skip = usize::from(points.last() == part.first() && !points.is_empty());
                        points.extend(part.into_iter().skip(skip));
                    }
                    points
                }
            };
            if points.len() > 1 && points.first() == points.last() {
                points.pop();
            }
            rings.push(points.into_iter().map(|p| map(&t, p)).collect());
        }
        let unit = self.unit(visit, entity_kind::HATCH, unit);
        self.face(rings, unit);
        Ok(())
    }

    fn finish(mut self, region: Option<[f64; 4]>) -> Scene {
        if self.current.items > 0 {
            let done = std::mem::take(&mut self.current);
            self.chunks.push(done);
        }
        let overlaps = |chunk: &Chunk| match (region, chunk.bounds.get()) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some([rx0, ry0, rx1, ry1]), Some([x0, y0, x1, y1])) => {
                x0 <= rx1 && rx0 <= x1 && y0 <= ry1 && ry0 <= y1
            }
        };
        let chunks: Vec<&Chunk> = self.chunks.iter().filter(|c| overlaps(c)).collect();
        let point_style = self.drawing.point_style.map(|style| {
            let mark = match style.mark {
                PointMark::Dot => layout::point_mark::DOT,
                PointMark::None => layout::point_mark::NONE,
                PointMark::Plus => layout::point_mark::PLUS,
                PointMark::Cross => layout::point_mark::CROSS,
                PointMark::Tick => layout::point_mark::TICK,
            };
            let mut frame = 0;
            if style.circle {
                frame |= layout::point_frame::CIRCLE;
            }
            if style.square {
                frame |= layout::point_frame::SQUARE;
            }
            let (kind, size) = match style.size {
                PointSize::Length(size) => (layout::point_size::LENGTH, size),
                PointSize::Screen(size) => (layout::point_size::SCREEN, size),
            };
            [
                f32::from(mark),
                f32::from(frame),
                f32::from(kind),
                size as f32,
            ]
        });
        let tables = write::Tables {
            units: &self.units,
            entities: &self.entities,
            steps: &self.steps,
            colors: &self.colors,
            clips: &self.clips,
            layers: &self.layers,
            fonts: &self.fonts,
            point_style,
        };
        let (bytes, counts, extent) = write::write(&chunks, &tables, &mut self.strings);
        Scene {
            bytes,
            tolerance: self.tolerance,
            counts,
            extent,
        }
    }
}

/// 折れ線の区間ごとの、始まりと終わりの幅
fn widths<P>(polyline: &Polyline<P>) -> Vec<[f64; 2]> {
    let constant = polyline.width.map(length).unwrap_or(0.0);
    (0..polyline.vertices.len())
        .map(|i| match polyline.vertex_widths.get(i) {
            Some([a, b]) => [length(*a), length(*b)],
            None => [constant, constant],
        })
        .collect()
}

/// 折れ線の両側へ、幅の半分ずつ広げた輪。幅は始まりから終わりへ、長さに沿って変える
fn band(points: &[Xy], start: f64, end: f64) -> Vec<Xy> {
    let total = geom::length(points);
    let mut left = Vec::with_capacity(points.len());
    let mut right = Vec::with_capacity(points.len());
    let mut walked = 0.0;
    for (i, p) in points.iter().enumerate() {
        if i > 0
            && let Some(prev) = points.get(i - 1)
        {
            walked += libm::hypot(p[0] - prev[0], p[1] - prev[1]);
        }
        let (a, b) = match (points.get(i), points.get(i + 1)) {
            (Some(a), Some(b)) => (*a, *b),
            _ => match (points.get(i.wrapping_sub(1)), points.get(i)) {
                (Some(a), Some(b)) => (*a, *b),
                _ => continue,
            },
        };
        let d = libm::hypot(b[0] - a[0], b[1] - a[1]);
        let normal = if d > 0.0 {
            [-(b[1] - a[1]) / d, (b[0] - a[0]) / d]
        } else {
            [0.0, 0.0]
        };
        let f = if total > 0.0 { walked / total } else { 0.0 };
        let half = (start + (end - start) * f) / 2.0;
        left.push([p[0] + normal[0] * half, p[1] + normal[1] * half]);
        right.push([p[0] - normal[0] * half, p[1] - normal[1] * half]);
    }
    left.extend(right.into_iter().rev());
    left
}

fn horizontal_code(value: Horizontal) -> u8 {
    match value {
        Horizontal::Left => horizontal::LEFT,
        Horizontal::Center => horizontal::CENTER,
        Horizontal::Right => horizontal::RIGHT,
        Horizontal::Middle => horizontal::MIDDLE,
    }
}

fn vertical_code(value: Vertical) -> u8 {
    match value {
        Vertical::Baseline => vertical::BASELINE,
        Vertical::Bottom => vertical::BOTTOM,
        Vertical::Middle => vertical::MIDDLE,
        Vertical::Top => vertical::TOP,
    }
}

fn entity_kind_of<P>(shape: &Shape<P>) -> u8 {
    match shape {
        Shape::Line { .. } => entity_kind::LINE,
        Shape::Circle { .. } => entity_kind::CIRCLE,
        Shape::Curve(_) => entity_kind::CURVE,
        Shape::Polyline(_) => entity_kind::POLYLINE,
        Shape::Text(_) => entity_kind::TEXT,
        Shape::Formatted(_) => entity_kind::FORMATTED,
        Shape::Dimension(_) => entity_kind::DIMENSION,
        Shape::Leader(_) => entity_kind::LEADER,
        Shape::Callout(_) => entity_kind::CALLOUT,
        Shape::Table(_) => entity_kind::TABLE,
        Shape::Underlay(_) => entity_kind::UNDERLAY,
        Shape::Symbol(_) => entity_kind::SYMBOL,
        Shape::Hatch(_) => entity_kind::HATCH,
        Shape::Point(_) => entity_kind::POINT,
    }
}

/// 既定の許容差を決めるための、形の定義の点を囲む範囲。曲線は中心と半径で囲む
fn rough(visit: &Visit<'_>, bounds: &mut Bounds) {
    fn points<P: Spot>(shape: &Shape<P>) -> Vec<Xy> {
        let around = |c: Xy, r: f64| vec![[c[0] - r, c[1] - r], [c[0] + r, c[1] + r]];
        match shape {
            Shape::Line { start, end } | Shape::Curve(Edge::Line { start, end }) => {
                vec![start.xy(), end.xy()]
            }
            Shape::Circle { center, radius } | Shape::Curve(Edge::Arc { center, radius, .. }) => {
                around(center.xy(), length(*radius))
            }
            Shape::Curve(Edge::Elliptic { center, major, .. }) => {
                let (c, m) = (center.xy(), major.xy());
                around(c, libm::hypot(m[0] - c[0], m[1] - c[1]))
            }
            Shape::Curve(Edge::Spline(spline)) => spline
                .control
                .iter()
                .chain(&spline.fit)
                .map(|p| p.xy())
                .collect(),
            Shape::Curve(Edge::Clothoid { origin, .. }) => vec![origin.xy()],
            Shape::Polyline(polyline) => polyline.vertices.iter().map(|v| v.point.xy()).collect(),
            Shape::Text(text) => match text.placement {
                Placement::Anchor { point, .. } => vec![point.xy()],
                Placement::Between { first, second, .. } => vec![first.xy(), second.xy()],
            },
            Shape::Formatted(text) => vec![text.point.xy()],
            Shape::Point(point) => vec![point.position.xy()],
            Shape::Hatch(hatch) => hatch
                .loops
                .iter()
                .flat_map(|l| match &l.boundary {
                    Boundary::Polyline { vertices, .. } => {
                        vertices.iter().map(|v| v.point.xy()).collect::<Vec<_>>()
                    }
                    Boundary::Edges(edges) => edges
                        .iter()
                        .flat_map(|e| points(&Shape::Curve(e.clone())))
                        .collect(),
                })
                .collect(),
            Shape::Dimension(_)
            | Shape::Leader(_)
            | Shape::Callout(_)
            | Shape::Table(_)
            | Shape::Underlay(_)
            | Shape::Symbol(_) => Vec::new(),
        }
    }
    let found = match visit.item {
        Item::Model(element) => points(&element.shape),
        Item::Paper(element) => points(&element.shape),
        Item::ModelAttribute { .. } | Item::PaperAttribute { .. } => Vec::new(),
    };
    for p in found {
        bounds.add(map(&visit.transform, p));
    }
}
