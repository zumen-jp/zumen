//! 2D のシーン（L3、決定 0137・0172）。図面モデルの配置をたどり（決定 0159）、解いた値を、線・面・文字・点・
//! 画像の区画と、それらが引く表に並べた 1 つのバッファーにする。区画の配置は `schema/scene.toml` にあり、
//! [`layout`] に生成してある（決定 0178）。
//!
//! - 曲線・ふくらみのある折れ線は、起点の長さの許容差で折れ線にする（`zumen-geom2d`）。作った点は予算の
//!   `segments` で数え、使った許容差を `scene.curve.approximated` で返す。
//! - 線種は、線と間に展開し、添える文字を置く。添える形は描かない（`scene.line-type-shape.dropped`）。
//! - 寸法・表・注記の記号は、描いた部品の中をたどって描く。部品がなければ、定義点・行と列から線と文字を
//!   作る。引出線と注記は、経路と端部記号と文字を描く（決定 0184）。
//! - ハッチングのパターンの線は、輪の内側（偶奇の規則）に切って描く（決定 0184）。

mod annotate;

mod glyphs;
pub mod layout;
mod write;

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Cx, Diagnostic, Kind, Ref, Stop, codes};
use zumen_drawing::transform::Point as Spot;
use zumen_drawing::{
    Affine, Boundary, Clipping, Color, ColorSystem, Drawing2D, Edge, Fill, Flow, FormattedText,
    Hatch, HeightMeaning, Horizontal, Inherit, Islands, Item, KnotParameterization, Length,
    LineTypeDefinition, LineTypeId, MarkContent, MarkRotation, Origin, Piece, Placement, PointLook,
    PointMark, PointSize, Polyline, Rotation, Scaled, Shape, Text, Underlay, UnderlayFrame,
    UnderlayKind, Vertex, Vertical, Visit,
};
use zumen_geom2d::{self as geom, Xy};

pub use crate::glyphs::FontSet;
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

    /// 系統の符号の RGB。表になければ `None`
    pub fn get(&self, system: ColorSystem, code: u32) -> Option<[u8; 3]> {
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
    /// 字形（決定 0188）
    pub glyphs: u64,
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
    build_with(drawing, options, &FontSet::default(), cx)
}

/// [`build`] と同じく作り、文字を `fonts` の字形で置く（決定 0188）。字形と置いた字形の形は、作った点と
/// 同じく `segments` で数える
pub fn build_with(
    drawing: &Drawing2D,
    options: &Options,
    fonts: &FontSet<'_>,
    cx: &mut Cx<'_>,
) -> Result<Scene, (Stop, Ref)> {
    let visits = drawing.walk_parts(options.origin, cx)?;
    // 描いた部品の中をたどった要素（寸法・表・注記の記号）は、道筋に持つ
    let parted: BTreeSet<&str> = visits
        .iter()
        .flat_map(|visit| visit.path.iter().map(|step| step.id.as_str()))
        .collect();
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
    let mut builder = Builder::new(drawing, tolerance, options, fonts, cx);
    builder.parted = parted;
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
    bounds: Bounds,
}

/// 塊の中の面
struct FaceItem {
    rings: Vec<Vec<Xy>>,
    unit: u32,
    bounds: Bounds,
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
    /// 字数と高さから大きめに見積もった範囲。範囲の絞り込みに使う
    bounds: Bounds,
    /// フォントで置いた字形（決定 0188）。置かなければ空
    glyphs: Vec<GlyphItem>,
}

/// 置いた字形
struct GlyphItem {
    /// 形（`shapes` の行）
    shape: u32,
    /// 字形の単位の座標を起点の座標へ写す変換 `[a, b, c, d, e, f]`（`x = a·u + c·v + e`、`y = b·u + d·v + f`）
    transform: [f64; 6],
}

/// 塊の中の点
struct PointItem {
    at: Xy,
    rotation: f64,
    marker: [u32; 4],
    scale: f64,
    unit: u32,
    bounds: Bounds,
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
    bounds: Bounds,
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

    /// 長方形 `[x の最小, y の最小, x の最大, y の最大]` と重なる
    fn overlaps(self, [rx0, ry0, rx1, ry1]: [f64; 4]) -> bool {
        self.0
            .is_some_and(|[x0, y0, x1, y1]| x0 <= rx1 && rx0 <= x1 && y0 <= ry1 && ry0 <= y1)
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
    fonts: &'d FontSet<'d>,
    font_names: Vec<String>,
    font_rows: BTreeMap<String, u32>,
    /// 字形の形（輪の列、字形の単位）と、フォント・字形・刻みの組からの行（決定 0188）
    shapes: Vec<Vec<Vec<Xy>>>,
    shape_rows: BTreeMap<(usize, u16, i32), u32>,
    /// 描いた部品の中をたどった要素の ID
    parted: BTreeSet<&'d str>,
    /// 単位と色の行ごとの、色だけを変えた単位
    recolored: BTreeMap<(u32, u32), u32>,
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
        fonts: &'d FontSet<'d>,
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
            fonts,
            font_names: Vec::new(),
            font_rows: BTreeMap::new(),
            shapes: Vec::new(),
            shape_rows: BTreeMap::new(),
            parted: BTreeSet::new(),
            recolored: BTreeMap::new(),
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
    }

    /// 描く単位。最初に描くものを足すときに作る
    fn unit(&mut self, visit: &Visit<'_>, kind: u8, made: &mut Option<u32>) -> u32 {
        self.unit_clipped(visit, kind, made, None)
    }

    /// 描く単位。`extra` は、要素が自分で持つ切り抜き（下敷き）で、配置の切り抜きの内側に足す
    fn unit_clipped(
        &mut self,
        visit: &Visit<'_>,
        kind: u8,
        made: &mut Option<u32>,
        extra: Option<Clipping>,
    ) -> u32 {
        if let Some(unit) = made {
            return *unit;
        }
        let mut clips = visit.clips.clone();
        clips.extend(extra);
        let id = self.strings.add(&visit.id);
        let path = u32::try_from(self.steps.len()).unwrap_or(layout::NONE);
        for step in &visit.path {
            let id = self.strings.add(&step.id);
            self.steps.push((id, step.cell));
        }
        let entity = u32::try_from(self.entities.len()).unwrap_or(layout::NONE);
        self.entities.push(Entity { id, kind, path });
        let color = self.color(visit.color);
        let clip = if clips.is_empty() {
            layout::NONE
        } else {
            match self.clips.iter().position(|c| *c == clips) {
                Some(found) => u32::try_from(found).unwrap_or(layout::NONE),
                None => {
                    self.clips.push(clips);
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

    /// 色だけを変えた単位。同じ色なら元の単位
    fn recolor(&mut self, unit: u32, color: Option<Color>) -> u32 {
        let row = self.color(color);
        let Some(base) = self.units.get(unit as usize) else {
            return unit;
        };
        if base.color == row {
            return unit;
        }
        if let Some(found) = self.recolored.get(&(unit, row)) {
            return *found;
        }
        let made = Unit {
            entity: base.entity,
            color: row,
            weight: base.weight,
            clip: base.clip,
            layer: base.layer,
        };
        let index = u32::try_from(self.units.len()).unwrap_or(layout::NONE);
        self.units.push(made);
        self.recolored.insert((unit, row), index);
        index
    }

    /// 書式の色を解く。`Block` は要素の色、`Layer` は要素のレイヤの色
    fn inherited(&self, visit: &Visit<'_>, color: Inherit<Color>) -> Option<Color> {
        match color {
            Inherit::Own(color) => Some(color),
            Inherit::Block => visit.color,
            Inherit::Layer => self
                .drawing
                .layer(visit.layer)
                .and_then(|layer| layer.style.as_ref())
                .and_then(|style| style.color),
        }
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
        row(&mut self.font_rows, &mut self.font_names, name.to_owned())
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
        self.current.lines.push(LineItem {
            points,
            unit,
            bounds,
        });
    }

    fn face(&mut self, rings: Vec<Vec<Xy>>, unit: u32) {
        let rings: Vec<Vec<Xy>> = rings.into_iter().filter(|r| r.len() >= 3).collect();
        if rings.is_empty() {
            return;
        }
        let mut bounds = Bounds::default();
        rings.iter().flatten().for_each(|p| bounds.add(*p));
        self.place(bounds);
        self.current.faces.push(FaceItem {
            rings,
            unit,
            bounds,
        });
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
                self.text(visit, text, unit)?;
            }
            Shape::Formatted(text) => {
                let unit = self.unit(visit, entity_kind::FORMATTED, unit);
                self.formatted(visit, text, unit)?;
            }
            Shape::Point(point) => {
                let unit = self.unit(visit, entity_kind::POINT, unit);
                let (rotation, _) = turned(&t, point.rotation);
                let (marker, scale) = match &point.look {
                    PointLook::Document => ([0; 4], 1.0),
                    PointLook::Marker { key, scale } => {
                        let [a, b] = self.strings.add(&key.namespace);
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
                    bounds,
                });
            }
            Shape::Underlay(underlay) => self.underlay(visit, underlay, source, unit),
            Shape::Hatch(hatch) => self.hatch(visit, hatch, local, source, unit)?,
            Shape::Dimension(dimension) => {
                if !self.parted.contains(visit.id.as_str()) {
                    self.dimension(visit, dimension, local, source, unit)?;
                }
            }
            Shape::Leader(leader) => self.leader(visit, leader, local, source, unit)?,
            Shape::Callout(callout) => self.callout(visit, callout, local, source, unit)?,
            Shape::Table(table) => {
                if !self.parted.contains(visit.id.as_str()) {
                    self.table(visit, table, source, unit)?;
                }
            }
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
                        // 制御点で決まらないものは、当てはめる点を通る 3 次のスプラインにする（決定 0184）。
                        // それもできなければ、当てはめる点か制御点を結ぶ
                        let fit: Vec<Xy> = spline.fit.iter().map(|p| p.xy()).collect();
                        let spacing = match spline.knot_parameterization {
                            Some(KnotParameterization::SquareRoot) => geom::Spacing::SquareRoot,
                            Some(KnotParameterization::Uniform) => geom::Spacing::Uniform,
                            Some(KnotParameterization::Chord | KnotParameterization::Custom)
                            | None => geom::Spacing::Chord,
                        };
                        let fitted = geom::interpolate(
                            &fit,
                            spline.start_tangent,
                            spline.end_tangent,
                            spacing,
                            spline.closed || spline.periodic,
                        )
                        .and_then(|(knots, control)| geom::spline(3, &knots, &control, &[], local));
                        match fitted {
                            Some(found) => {
                                points = found;
                                self.approximated = true;
                                if spline.fit_tolerance.is_some_and(|t| t > 0.0) {
                                    self.report(codes::SCENE_FIT_TOLERANCE_APPROXIMATED, source);
                                }
                            }
                            None => {
                                points = if fit.len() >= 2 { fit } else { control };
                                self.report(codes::SCENE_SPLINE_APPROXIMATED, source);
                            }
                        }
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
        let Some((pattern, scale, definition)) = self.pattern(visit) else {
            self.segments(points.len(), source)?;
            self.line(points, unit);
            return Ok(());
        };
        let period = geom::period(&pattern);
        if !definition.marks.is_empty() && period > self.tolerance {
            self.marks(visit, &points, &pattern, scale, definition, unit, source)?;
        }
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

    /// 線種に添える文字を、列の要素の始まりごとに置く（決定 0184）。形は描かない
    #[allow(clippy::too_many_arguments)]
    fn marks(
        &mut self,
        visit: &Visit<'_>,
        points: &[Xy],
        pattern: &[f64],
        scale: f64,
        definition: &LineTypeDefinition,
        unit: u32,
        source: &Ref,
    ) -> Result<(), (Stop, Ref)> {
        let t = visit.transform;
        let mut shapes = false;
        for mark in &definition.marks {
            let MarkContent::Text { content, style } = &mark.content else {
                shapes = true;
                continue;
            };
            let Some(height) = self.drawing.mark_text_height(mark) else {
                continue;
            };
            let places = geom::starts(points, pattern, mark.index);
            self.segments(places.len(), source)?;
            let drawing = self.drawing;
            let font_name = style
                .as_deref()
                .and_then(|name| drawing.text_style(name))
                .and_then(|s| s.definition.as_ref())
                .and_then(|s| s.font.as_ref())
                .map(|f| f.name.as_str());
            let font = self.font(font_name);
            let string = self.strings.add(content);
            for (at, direction) in places {
                let r = direction.to_radians();
                let (c, s) = (libm::cos(r), libm::sin(r));
                let [dx, dy] = [mark.offset[0] * scale, mark.offset[1] * scale];
                let at = [at[0] + dx * c - dy * s, at[1] + dx * s + dy * c];
                let mut rotation = match mark.rotation_basis {
                    MarkRotation::Relative | MarkRotation::Upright => direction + mark.rotation,
                    MarkRotation::Absolute => turned(&t, mark.rotation).0,
                };
                if mark.rotation_basis == MarkRotation::Upright {
                    let turn = rotation.rem_euclid(360.0);
                    if turn > 90.0 && turn <= 270.0 {
                        rotation += 180.0;
                    }
                }
                let mut item = TextItem {
                    string,
                    at,
                    second: None,
                    height: height * scale,
                    width: 1.0,
                    rotation,
                    oblique: 0.0,
                    box_width: 0.0,
                    align: [horizontal::LEFT, vertical::BASELINE],
                    flags: 0,
                    line: 0,
                    font,
                    unit,
                    bounds: Bounds::default(),
                    glyphs: Vec::new(),
                };
                self.lay_mark(
                    content,
                    at,
                    height * scale,
                    rotation,
                    font_name,
                    &mut item,
                    source,
                )?;
                self.put_text(item);
            }
        }
        if shapes {
            self.report(codes::SCENE_LINE_TYPE_SHAPE_DROPPED, source);
        }
        Ok(())
    }

    /// 解いた線種の、起点の長さの列と、列に掛けた倍率と、定義。続く線なら `None`
    fn pattern(&self, visit: &Visit<'_>) -> Option<(Vec<f64>, f64, &'d LineTypeDefinition)> {
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
            scale,
            definition,
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

    fn text<P: Spot>(
        &mut self,
        visit: &Visit<'_>,
        text: &Text<P>,
        unit: u32,
    ) -> Result<(), (Stop, Ref)> {
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
        let font_name = style.and_then(|s| s.font.as_ref()).map(|f| f.name.as_str());
        let font = self.font(font_name);
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
        flags |= meaning_flag(&text.height.meaning);
        let string = self.strings.add(&text.content);
        let mut item = TextItem {
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
            bounds: Bounds::default(),
            glyphs: Vec::new(),
        };
        let vertical = style.is_some_and(|s| s.vertical);
        self.lay_text(visit, text, font_name, vertical, &mut item)?;
        self.put_text(item);
        Ok(())
    }

    fn put_text(&mut self, mut item: TextItem) {
        let mut bounds = Bounds::default();
        bounds.add(item.at);
        if let Some(second) = item.second {
            bounds.add(second);
        }
        self.place(bounds);
        // 範囲の絞り込みには、字数 × 高さ × 幅の倍率（枠の幅があれば、その幅）の円で大きめに囲む
        let start = item.string[0] as usize;
        let count = self
            .strings
            .bytes
            .get(start..start.saturating_add(item.string[1] as usize))
            .map_or(1, |b| String::from_utf8_lossy(b).chars().count().max(1));
        let reach = (item.height.abs() * count as f64 * item.width.abs().max(1.0))
            .max(item.box_width.abs())
            + item.height.abs();
        if reach.is_finite() {
            let [x, y] = item.at;
            bounds.add([x - reach, y - reach]);
            bounds.add([x + reach, y + reach]);
        }
        item.bounds = bounds;
        self.current.texts.push(item);
    }

    /// 書式つきの文字を、書式の区間ごとの文字にする。行の中の位置は、表示する側が送る（決定 0172）
    fn formatted<P: Spot>(
        &mut self,
        visit: &Visit<'_>,
        text: &FormattedText<P>,
        unit: u32,
    ) -> Result<(), (Stop, Ref)> {
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
        flags |= meaning_flag(&text.height.meaning);
        let align = [
            horizontal_code(text.horizontal),
            vertical_code(text.vertical),
        ];
        let box_width = text.width.map_or(0.0, |w| length(w) * along);
        let mut laid = self.lay_formatted(visit, text, base_font.as_deref(), vertical)?;
        if laid.is_some() {
            flags &= !(text_flag::HEIGHT_FRAME | text_flag::HEIGHT_EM);
        }
        let ratio = laid.as_ref().map_or(1.0, |laid| laid.ratio);
        let mut line = 0u32;
        for (index, span) in text.spans.iter().enumerate() {
            let height = ratio
                * match span.format.height {
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
            // 区間の色は、色だけを変えた単位にする（決定 0184）
            let unit = match span.format.color {
                Some(color) => {
                    let color = self.inherited(visit, color);
                    self.recolor(unit, color)
                }
                None => unit,
            };
            let put = |builder: &mut Self, content: &str, extra: u8, glyphs: Vec<GlyphItem>| {
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
                    bounds: Bounds::default(),
                    glyphs,
                });
            };
            let mut glyphs = laid
                .as_mut()
                .and_then(|laid| laid.glyphs.remove(&index))
                .unwrap_or_default();
            match &span.piece {
                Piece::Text(content) => put(self, content, 0, glyphs),
                Piece::Stack { upper, lower, .. } => {
                    let count = laid
                        .as_ref()
                        .and_then(|laid| laid.uppers.get(&index))
                        .copied()
                        .unwrap_or(0)
                        .min(glyphs.len());
                    let below = glyphs.split_off(count);
                    put(self, upper, text_flag::STACK_UPPER, glyphs);
                    put(self, lower, text_flag::STACK_LOWER, below);
                    // 字形で置いた重ねた文字の線は、線にする（決定 0188）
                    if let Some(ends) = laid.as_ref().and_then(|laid| laid.lines.get(&index)) {
                        self.line(ends.to_vec(), unit);
                    }
                }
                Piece::Paragraph => line = line.saturating_add(1),
                _ => {}
            }
        }
        Ok(())
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
            UnderlayKind::Dwf => layout::image_kind::DWF,
            UnderlayKind::Dgn => layout::image_kind::DGN,
        };
        let t = visit.transform;
        let clip = match &underlay.clip {
            None => None,
            Some(clip) => {
                let found = underlay_clip(&underlay.frame, clip, &t);
                if found.is_none() {
                    self.report(codes::SCENE_UNDERLAY_CLIP_DROPPED, source);
                }
                found
            }
        };
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
        let unit = self.unit_clipped(visit, entity_kind::UNDERLAY, unit, clip);
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
            bounds,
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
        if let Fill::Gradient(_) = &hatch.fill {
            self.report(codes::SCENE_GRADIENT_APPROXIMATED, source);
        }
        let t = visit.transform;
        // 輪は要素の空間で作り、パターンの線を切ってから起点へ写す
        let outermost = hatch.loops.iter().any(|l| l.outermost);
        let mut rings: Vec<Vec<Xy>> = Vec::new();
        for (index, ring) in hatch.loops.iter().enumerate() {
            // 内側の輪を無視するなら、いちばん外の輪だけを塗る
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
            if points.len() >= 3 {
                rings.push(points);
            }
        }
        if hatch.islands == Islands::Outer {
            rings = outer_islands(rings);
        }
        let unit = self.unit(visit, entity_kind::HATCH, unit);
        let mapped = |rings: &[Vec<Xy>]| -> Vec<Vec<Xy>> {
            rings
                .iter()
                .map(|r| r.iter().map(|p| map(&t, *p)).collect())
                .collect()
        };
        let Fill::Pattern(pattern) = &hatch.fill else {
            self.face(mapped(&rings), unit);
            return Ok(());
        };
        // パターンの線の長さは要素の空間なので、起点の長さへは変換の面積の倍率の平方根で伸ばす
        let [a, b, _, d, e, _] = t.m;
        let grow = (a * e - b * d).abs().sqrt();
        let mut extent = Bounds::default();
        rings.iter().flatten().for_each(|p| extent.add(*p));
        let span = extent
            .get()
            .map_or(0.0, |[x0, y0, x1, y1]| libm::hypot(x1 - x0, y1 - y0));
        let mut filled = false;
        for family in &pattern.lines {
            let r = family.angle.to_radians();
            let spacing = (libm::cos(r) * family.offset[1] - libm::sin(r) * family.offset[0]).abs();
            if spacing * grow <= self.tolerance {
                // 線の間隔が見分けられないなら塗る
                if !filled {
                    self.report(codes::SCENE_HATCH_PATTERN_APPROXIMATED, source);
                    self.face(mapped(&rings), unit);
                    filled = true;
                }
                continue;
            }
            let Some((first, last)) =
                geom::hatch_rows(&rings, family.angle, family.base, family.offset)
            else {
                continue;
            };
            if last < first || last.is_nan() || first.is_nan() {
                continue;
            }
            let period = geom::period(&family.dashes);
            // 列の 1 周が許容差より短ければ、続けて描く
            let dashes: &[f64] = if period * grow <= self.tolerance {
                &[]
            } else {
                &family.dashes
            };
            let per_row = if dashes.is_empty() {
                2.0
            } else {
                2.0 + span / period * dashes.len() as f64 * 2.0
            };
            let estimate = (last - first + 1.0) * per_row;
            let estimate = if estimate.is_finite() {
                estimate.min(u64::MAX as f64) as usize
            } else {
                usize::MAX
            };
            self.segments(estimate, source)?;
            let mut k = first;
            while k <= last {
                for piece in
                    geom::hatch_row(&rings, family.angle, family.base, family.offset, k, dashes)
                {
                    self.line(piece.into_iter().map(|p| map(&t, p)).collect(), unit);
                }
                k += 1.0;
            }
        }
        Ok(())
    }

    /// 範囲で絞ったとき、残る描くものが引く行だけに表（単位・要素・道筋・色・切り抜き・レイヤ・フォント・
    /// 文字列）を絞る（決定 0184）。行の番号は、残る描くものが初めて引く順に振り直す
    fn compact(&mut self, chunks: &mut [Chunk]) {
        fn moved(old: &Strings, new: &mut Strings, [start, len]: [u32; 2]) -> [u32; 2] {
            let start = start as usize;
            match old.bytes.get(start..start.saturating_add(len as usize)) {
                Some(bytes) if !bytes.is_empty() => new.add(&String::from_utf8_lossy(bytes)),
                _ => [0, 0],
            }
        }
        fn renumber(map: &mut BTreeMap<u32, u32>, old: u32) -> (u32, bool) {
            if old == layout::NONE {
                return (old, false);
            }
            let next = u32::try_from(map.len()).unwrap_or(layout::NONE);
            match map.get(&old) {
                Some(found) => (*found, false),
                None => {
                    map.insert(old, next);
                    (next, true)
                }
            }
        }
        let old = std::mem::take(&mut self.strings);
        let mut strings = Strings::default();
        let (mut shape_map, mut shapes) = (BTreeMap::new(), Vec::new());
        let (mut unit_map, mut entity_map, mut color_map) =
            (BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
        let (mut clip_map, mut layer_map, mut font_map) =
            (BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
        let (mut units, mut entities, mut steps) = (Vec::new(), Vec::new(), Vec::new());
        let (mut colors, mut clips, mut layers, mut fonts) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut unit_of = |unit: u32| -> u32 {
            let (index, fresh) = renumber(&mut unit_map, unit);
            if !fresh {
                return index;
            }
            let Some(base) = self.units.get(unit as usize) else {
                units.push(Unit {
                    entity: layout::NONE,
                    color: layout::NONE,
                    weight: 0.0,
                    clip: layout::NONE,
                    layer: layout::NONE,
                });
                return index;
            };
            let (entity, fresh) = renumber(&mut entity_map, base.entity);
            if fresh && let Some(e) = self.entities.get(base.entity as usize) {
                let start = e.path as usize;
                let end = self
                    .entities
                    .get(base.entity as usize + 1)
                    .map_or(self.steps.len(), |n| n.path as usize);
                let path = u32::try_from(steps.len()).unwrap_or(layout::NONE);
                for (id, cell) in self.steps.get(start..end).unwrap_or_default() {
                    steps.push((moved(&old, &mut strings, *id), *cell));
                }
                entities.push(Entity {
                    id: moved(&old, &mut strings, e.id),
                    kind: e.kind,
                    path,
                });
            }
            let (color, fresh) = renumber(&mut color_map, base.color);
            if fresh && let Some(row) = self.colors.get(base.color as usize) {
                colors.push(*row);
            }
            let (clip, fresh) = renumber(&mut clip_map, base.clip);
            if fresh && let Some(set) = self.clips.get(base.clip as usize) {
                clips.push(set.clone());
            }
            let (layer, fresh) = renumber(&mut layer_map, base.layer);
            if fresh && let Some(name) = self.layers.get(base.layer as usize) {
                layers.push(moved(&old, &mut strings, *name));
            }
            units.push(Unit {
                entity,
                color,
                weight: base.weight,
                clip,
                layer,
            });
            index
        };
        for chunk in chunks.iter_mut() {
            for item in &mut chunk.lines {
                item.unit = unit_of(item.unit);
            }
            for item in &mut chunk.faces {
                item.unit = unit_of(item.unit);
            }
            for item in &mut chunk.texts {
                item.unit = unit_of(item.unit);
            }
            for item in &mut chunk.points {
                item.unit = unit_of(item.unit);
            }
            for item in &mut chunk.images {
                item.unit = unit_of(item.unit);
            }
        }
        for chunk in chunks.iter_mut() {
            for item in &mut chunk.texts {
                item.string = moved(&old, &mut strings, item.string);
                let (font, fresh) = renumber(&mut font_map, item.font);
                if fresh && let Some(name) = self.font_names.get(item.font as usize) {
                    fonts.push(name.clone());
                }
                item.font = font;
                for glyph in &mut item.glyphs {
                    let (shape, fresh) = renumber(&mut shape_map, glyph.shape);
                    if fresh && let Some(rings) = self.shapes.get(glyph.shape as usize) {
                        shapes.push(rings.clone());
                    }
                    glyph.shape = shape;
                }
            }
            for item in &mut chunk.points {
                let [a, b, c, d] = item.marker;
                let [a, b] = moved(&old, &mut strings, [a, b]);
                let [c, d] = moved(&old, &mut strings, [c, d]);
                item.marker = [a, b, c, d];
            }
            for item in &mut chunk.images {
                item.name = moved(&old, &mut strings, item.name);
                item.page = moved(&old, &mut strings, item.page);
            }
        }
        self.units = units;
        self.entities = entities;
        self.steps = steps;
        self.colors = colors;
        self.clips = clips;
        self.layers = layers;
        self.font_names = fonts;
        self.shapes = shapes;
        self.strings = strings;
    }

    fn finish(mut self, region: Option<[f64; 4]>) -> Scene {
        if self.current.items > 0 {
            let done = std::mem::take(&mut self.current);
            self.chunks.push(done);
        }
        let mut chunks = std::mem::take(&mut self.chunks);
        if let Some(region) = region {
            // 重なる塊の中で、重なる描くものだけを残す。内容の大きさの分からない画像（辺が内容の 1 単位）は残す
            chunks.retain(|chunk| chunk.bounds.overlaps(region));
            for chunk in &mut chunks {
                chunk.lines.retain(|i| i.bounds.overlaps(region));
                chunk.faces.retain(|i| i.bounds.overlaps(region));
                chunk.texts.retain(|i| i.bounds.overlaps(region));
                chunk.points.retain(|i| i.bounds.overlaps(region));
                chunk
                    .images
                    .retain(|i| i.frame == layout::image_frame::UNIT || i.bounds.overlaps(region));
            }
            chunks.retain(|c| {
                !(c.lines.is_empty()
                    && c.faces.is_empty()
                    && c.texts.is_empty()
                    && c.points.is_empty()
                    && c.images.is_empty())
            });
            self.compact(&mut chunks);
        }
        let chunks: Vec<&Chunk> = chunks.iter().collect();
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
            fonts: &self.font_names,
            shapes: &self.shapes,
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

/// 外側の島だけを残す（決定 0184）。ほかの輪に囲まれる数が 1 以下の輪だけを、偶奇の規則で塗る。囲まれるかは、
/// 輪の最初の頂点で決める
fn outer_islands(rings: Vec<Vec<Xy>>) -> Vec<Vec<Xy>> {
    let depths: Vec<usize> = rings
        .iter()
        .enumerate()
        .map(|(i, ring)| {
            let Some(p) = ring.first() else { return 0 };
            rings
                .iter()
                .enumerate()
                .filter(|(j, other)| *j != i && geom::inside(other, *p))
                .count()
        })
        .collect();
    rings
        .into_iter()
        .zip(depths)
        .filter(|(_, depth)| *depth <= 1)
        .map(|(ring, _)| ring)
        .collect()
}

/// 高さの意味の文字の印（決定 0184）
fn meaning_flag(meaning: &HeightMeaning) -> u8 {
    match meaning {
        HeightMeaning::Frame => text_flag::HEIGHT_FRAME,
        HeightMeaning::Em => text_flag::HEIGHT_EM,
        HeightMeaning::CapHeight | HeightMeaning::Declared(_) => 0,
    }
}

/// 下敷きの切り抜きを、起点の座標の多角形にする（決定 0184）。画素の格子は、DXF の IMAGE と同じく左上の
/// 画素の中心を原点とし、下へ進む画素の座標とする。尺度で写すものは、内容の座標とする（推定）。内容の
/// 大きさが分からない置き方なら `None`
fn underlay_clip<P: Spot>(
    frame: &UnderlayFrame<P>,
    clip: &zumen_drawing::Clip,
    t: &Affine,
) -> Option<Clipping> {
    let corners = match clip.vertices.as_slice() {
        &[[x0, y0], [x1, y1]] => vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
        vertices => vertices.to_vec(),
    };
    if corners.len() < 3 {
        return None;
    }
    let place: Box<dyn Fn(Xy) -> Xy> = match frame {
        UnderlayFrame::Pixels {
            size,
            origin,
            right,
            top,
        } => {
            let (o, r, u) = (origin.xy(), right.xy(), top.xy());
            if !(size[0] > 0.0 && size[1] > 0.0) {
                return None;
            }
            let du = [(r[0] - o[0]) / size[0], (r[1] - o[1]) / size[0]];
            let dv = [(u[0] - o[0]) / size[1], (u[1] - o[1]) / size[1]];
            let height = size[1];
            Box::new(move |[x, y]: Xy| {
                let (a, b) = (x + 0.5, height - y - 0.5);
                [o[0] + du[0] * a + dv[0] * b, o[1] + du[1] * a + dv[1] * b]
            })
        }
        UnderlayFrame::Scaled {
            origin,
            scale,
            rotation,
        } => {
            let o = origin.xy();
            let r = rotation.to_radians();
            let (c, s) = (libm::cos(r), libm::sin(r));
            let [sx, sy] = [length(scale[0]), length(scale[1])];
            Box::new(move |[x, y]: Xy| {
                let (x, y) = (x * sx, y * sy);
                [o[0] + x * c - y * s, o[1] + x * s + y * c]
            })
        }
        UnderlayFrame::Corners { .. } => return None,
    };
    let polygon: Vec<Xy> = corners.into_iter().map(|p| map(t, place(p))).collect();
    polygon
        .iter()
        .all(|p| p[0].is_finite() && p[1].is_finite())
        .then_some(Clipping {
            polygon,
            inverted: clip.inverted,
        })
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
