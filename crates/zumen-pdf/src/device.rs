//! hayro-interpret が塗る内容（経路・字形の並び・画像）を、用紙の要素にする（決定 0256）。
//!
//! hayro-interpret は、ページの内容を塗る順に `Device` に渡す。座標は、渡した変換（[`crate::page::Geometry`] に
//! [`DEVICE_PER_MM`] を掛けたもの）の装置の単位。経路は線分・折れ線・曲線（3 次のベジェ曲線をそのまま持つ
//! スプライン）とハッチングに、字形の並びは文字に、画像は下敷きにする。移せないもの・形を変えたものは、診断にする。
//! 要素は、切り抜きと比べてから色を決めるので、切り抜きで捨てる要素には、色・透明の診断を返さない。

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use hayro_cmap::{BfString, WritingMode};
use hayro_interpret::font::{Glyph, GlyphRun, PositionedGlyph};
use hayro_interpret::pattern::Pattern;
use hayro_interpret::{
    BlendMode, ClipPath, Device, DrawMode, DrawProps, FillRule, Image, ImageDrawProps, Paint,
    SoftMask, StrokeProps,
};
use hayro_syntax::object::{Array, Dict, ObjectIdentifier};
use kurbo::{Affine, BezPath, Cap, Join, PathEl, Point, Rect, Vec2};
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

/// 字形の原点が、フォントの送りからこれ（em）より大きくずれる所で、字形の並びを別の文字に分ける（推定）
const SPLIT_EM: f64 = 0.5;

/// 分けた文字の中で、字形の原点がフォントの送りからこれ（em）より大きくずれれば、字形ごとの位置を移さなかった
/// ことを診断する（推定）
const SHIFT_EM: f64 = 0.05;

/// 縦書きの字形の、横の原点から見た縦の原点（字形の空間の単位）。横の幅を 1,000、`DW2` を既定の [880 −1000] と
/// みなす（9.7.4.3、推定）
const VERTICAL_ORIGIN: Point = Point::new(500.0, 880.0);

/// 文字を切り抜きと比べる箱（字形の空間の単位）。em の正方形に、下がりの分を足したもの（推定）
const GLYPH_BOX: [Point; 4] = [
    Point::new(0.0, -200.0),
    Point::new(1000.0, -200.0),
    Point::new(0.0, 1000.0),
    Point::new(1000.0, 1000.0),
];

/// 装置の座標 → 用紙の mm
fn to_mm() -> Affine {
    Affine::scale(1.0 / DEVICE_PER_MM)
}

/// ソフトマスクか、通常でない合成の方法がかかるか（11.3.5・11.6.5）
fn blended(soft_mask: bool, blend_mode: BlendMode) -> bool {
    soft_mask || blend_mode != BlendMode::Normal
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
    /// 点の外接矩形。点がないか、有限でない点があれば `None`
    fn of(points: impl IntoIterator<Item = Point>) -> Option<Self> {
        let mut out: Option<Bounds> = None;
        for point in points {
            if !(point.x.is_finite() && point.y.is_finite()) {
                return None;
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

/// 字形の変換が作る、文字の座標系。字形の x 軸が基準線の向き、y 軸が em の上向き（字形の空間は em の 1/1000）
#[derive(Clone, Copy, Debug)]
struct Frame {
    /// 字形の原点
    origin: Point,
    /// 字形の空間の 1,000 単位の x 軸と y 軸
    ex: Vec2,
    ey: Vec2,
    /// x 軸の長さ
    width: f64,
    /// x 軸と y 軸の外積。負なら裏返しの変換
    cross: f64,
}

impl Frame {
    /// 有限で、大きさのある座標系。そうでなければ `None`（`0 Tf`・`0 Tz` と、特異な行列）
    fn of(transform: Affine) -> Option<Self> {
        let origin = transform * Point::ZERO;
        let ex = transform * Point::new(1000.0, 0.0) - origin;
        let ey = transform * Point::new(0.0, 1000.0) - origin;
        let width = libm::hypot(ex.x, ex.y);
        let cross = ex.x * ey.y - ex.y * ey.x;
        let frame = Self {
            origin,
            ex,
            ey,
            width,
            cross,
        };
        let finite = origin.x.is_finite()
            && origin.y.is_finite()
            && ey.x.is_finite()
            && ey.y.is_finite()
            && cross.is_finite()
            && (width / frame.height()).is_finite();
        (finite && width > 0.0 && cross != 0.0).then_some(frame)
    }

    /// em の高さ（基準線に直交する向き）。hayro-interpret が字形の幅を合わせる伸縮（x 軸だけ）に依らない
    fn height(&self) -> f64 {
        self.cross.abs() / self.width
    }

    /// 基準線の向きの単位ベクトル
    fn along(&self) -> Vec2 {
        self.ex / self.width
    }
}

/// 字形の送り（em）。PDF のフォントの幅（`Widths`・`W`、9.2.4）。Type 3 の字形は数えない
fn advance(glyph: &Glyph<'_>) -> Option<f64> {
    match glyph {
        Glyph::Outline(outline) => outline
            .advance_width()
            .map(|width| f64::from(width) / 1000.0)
            .filter(|width| width.is_finite()),
        Glyph::Type3(_) => None,
    }
}

/// 並べた値の中央の値（数が偶数なら、小さいほう）
fn median(mut values: Vec<f64>) -> Option<f64> {
    values.sort_by(f64::total_cmp);
    let middle = values.len().checked_sub(1)? / 2;
    values.get(middle).copied()
}

/// 字形の並びを、字形の原点がフォントの送りから [`SPLIT_EM`] より大きくずれる所で分ける（`TJ` の数・`Tc`・`Tw`、
/// 9.4.3・9.4.4）。分けた範囲ごとに、[`SHIFT_EM`] より小さくない、ずれがあるかを返す（推定）。
///
/// 横書きの送りの向きの長さは、hayro-interpret が字形の幅を合わせる伸縮に依らないよう、em の高さを単位にして
/// 比べる。フォントの幅の 1 em あたりの長さ（水平の伸縮 `Tz` と、文字の行列の縦横の比）は、分けるときは、最初の
/// 字形の x 軸と、いちばん狭い送りの小さいほうとし、ずれを見るときは、分けた範囲の送りの中央の値（字形の間が 1 つ
/// なら、最初の字形の x 軸）とする。縦書きは、縦の送りを 1 em（`DW2` の既定）とみなし、送りの向きだけを比べる
fn split(
    glyphs: &[PositionedGlyph<'_>],
    device: Affine,
    vertical: bool,
) -> Vec<(Range<usize>, bool)> {
    let whole = vec![(0..glyphs.len(), false)];
    let Some(frame) = glyphs
        .first()
        .and_then(|first| Frame::of(device * first.transform()))
    else {
        return whole;
    };
    let origins: Vec<Point> = glyphs
        .iter()
        .map(|glyph| device * glyph.transform() * Point::ZERO)
        .collect();
    if vertical {
        let unit = libm::hypot(frame.ey.x, frame.ey.y);
        let down = -frame.ey / unit;
        let deviations: Vec<f64> = origins
            .windows(2)
            .map(|pair| match pair {
                [from, to] => ((*to - *from) - (-frame.ey)).dot(down) / unit,
                _ => 0.0,
            })
            .collect();
        return segments(&deviations, |range| {
            deviations
                .get(range)
                .is_some_and(|part| part.iter().any(|shift| shift.abs() > SHIFT_EM))
        });
    }
    let height = frame.height();
    let along = frame.along();
    let normal = Vec2::new(-along.y, along.x);
    // 送りの向きと、それに直交する向きの長さ（em）と、フォントの幅（em）
    let mut steps = Vec::with_capacity(origins.len().saturating_sub(1));
    for (pair, glyph) in origins.windows(2).zip(glyphs) {
        let (Some(advance), [from, to]) = (advance(glyph), pair) else {
            return whole;
        };
        let step = *to - *from;
        steps.push((step.dot(along) / height, step.dot(normal) / height, advance));
    }
    let ratio = |&(length, _, advance): &(f64, f64, f64)| {
        (advance > 0.0 && length > 0.0).then(|| length / advance)
    };
    let narrowest = steps.iter().filter_map(ratio).fold(f64::INFINITY, f64::min);
    let scale = (frame.width / height).min(narrowest);
    let splits: Vec<f64> = steps
        .iter()
        .map(|&(length, across, advance)| (length - advance * scale).abs().max(across.abs()))
        .collect();
    segments(&splits, |range| {
        let Some(part) = steps.get(range) else {
            return false;
        };
        // 字形の間が 1 つだけなら、最初の字形の x 軸で比べる（字形の幅を合わせる伸縮が 1 から遠いフォントでは、
        // ずれを見誤りうる）
        let scale = median(part.iter().filter_map(ratio).collect())
            .filter(|_| part.len() > 1)
            .unwrap_or(frame.width / height);
        part.iter().any(|&(length, across, advance)| {
            (length - advance * scale).abs() > SHIFT_EM || across.abs() > SHIFT_EM
        })
    })
}

/// 字形の間のずれ（em）が [`SPLIT_EM`] より大きい所で、字形の範囲を分ける。`shifted` は、範囲の中の字形の間
/// （添字は `deviations` と同じ）に、小さいずれがあるか
fn segments(
    deviations: &[f64],
    shifted: impl Fn(Range<usize>) -> bool,
) -> Vec<(Range<usize>, bool)> {
    let mut out = Vec::new();
    let mut start = 0;
    for (index, deviation) in deviations.iter().enumerate() {
        if deviation.abs() > SPLIT_EM {
            out.push((start..index + 1, shifted(start..index)));
            start = index + 1;
        }
    }
    let end = deviations.len() + 1;
    out.push((start..end, shifted(start..end - 1)));
    out
}

/// 塗りの字形の並びの、比べるための値
#[derive(Clone, Debug, PartialEq)]
struct RunKey {
    content: String,
    first: [f64; 6],
    count: usize,
}

/// 透明のグループ（11.4）
#[derive(Clone, Copy, Debug)]
struct Group {
    /// 不透明度を掛け合わせたもの（0〜1）
    opacity: f32,
    /// ソフトマスクか、通常でない合成の方法を持つ
    blended: bool,
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
    /// 透明のグループの重なり
    groups: Vec<Group>,
    /// いま描く要素に、ソフトマスクか通常でない合成の方法がかかる
    blended: bool,
    /// ページが転送関数を使う（[`crate::walk::Survey`]）
    transfer: bool,
    /// 直前の塗りの字形の並び。続く同じ並びの線（描画モード 2）は、同じ文字にする
    last_fill_run: Option<RunKey>,
    /// 場所を添えた診断。同じ code と値の 2 つ目からは場所を添えない
    placed: BTreeSet<(&'static str, String)>,
    /// 予算か中断の合図で止めた理由
    pub(crate) stopped: Option<Stop>,
}

impl<'c, 'x> Collector<'c, 'x> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        cx: &'c mut Cx<'x>,
        source: Ref,
        sheet: SheetId,
        page: Rect,
        ocgs: &'c BTreeMap<(u32, i32), LayerId>,
        objects: &'c BTreeMap<(u32, i32), Ref>,
        styles: &'c mut Styles,
        transfer: bool,
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
            blended: false,
            transfer,
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
        self.groups.last().map_or(1.0, |group| group.opacity)
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

    /// 要素の外接矩形を、切り抜きと比べる。外なら移さないことを、かかるなら切らずに移すことを診断する
    fn visible(&mut self, bounds: Bounds) -> bool {
        match clipped(bounds, self.clip()) {
            Clipped::Outside => {
                self.note_code(codes::PDF_CLIP_DROPPED);
                false
            }
            Clipped::Crossing => {
                self.note_code(codes::PDF_CLIP_APPROXIMATED);
                true
            }
            Clipped::Inside => true,
        }
    }

    /// 要素を足す。ソフトマスクと合成の方法は移さないので、かかっていれば診断する
    fn add(&mut self, shape: Shape<PaperPoint>, style: Style) {
        if self.blended || self.groups.iter().any(|group| group.blended) {
            self.note_code(codes::PDF_TRANSPARENCY_APPROXIMATED);
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
        // 転送関数（10.4）を使うページでは、hayro-interpret が関数を当てた RGB を渡す。その値は、版と環境で
        // 変わりうるので使わない（決定 0256）。どの要素に効くかは渡されないので、ページの色をすべてレイヤに従わせる
        if self.transfer {
            self.note_code(codes::PDF_TRANSFER_DROPPED);
            return (Inherit::Layer, self.transparency(alpha));
        }
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
        let parts: Vec<Subpath> = subpaths(path)
            .into_iter()
            .filter(|subpath| !subpath.segments.is_empty())
            .collect();
        if parts.is_empty() {
            return;
        }
        // 有限でない座標の形は移さない
        let Some(bounds) = path_bounds(path) else {
            self.note_code(codes::PDF_GEOMETRY_DROPPED);
            return;
        };
        if !self.visible(bounds) {
            return;
        }
        let Some((color, transparency)) = self.paint(paint) else {
            return;
        };
        // 偶奇の規則は、輪の中の輪を交互に埋めるのと同じ。非ゼロ回転数の規則は、自分と交わらない輪が 1 つなら
        // 同じになる（8.5.3.3。自分と交わる輪は見ない、推定）
        if rule == FillRule::NonZero && parts.len() > 1 {
            self.note_code(codes::PDF_FILL_RULE_APPROXIMATED);
        }
        let loops = parts
            .iter()
            .map(|subpath| HatchLoop {
                boundary: self.boundary(subpath),
                external: false,
                outermost: false,
                derived: false,
                text_box: false,
                sources: Vec::new(),
            })
            .collect();
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
        self.add(Shape::Hatch(hatch), style);
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
        // 線の太さと破線はユーザー空間の長さ。用紙の mm には、変換の平均の倍率を掛ける（推定。斜めに伸ばした
        // 変換では、向きで太さが変わる）
        let scale = libm::sqrt(transform.determinant().abs());
        let width = f64::from(stroke.line_width) * scale;
        for subpath in subpaths(path) {
            if subpath.segments.is_empty() {
                continue;
            }
            // 有限でない座標・太さ・倍率の線は移さない
            let Some(bounds) = subpath
                .bounds()
                .filter(|_| scale.is_finite() && width.is_finite())
            else {
                self.note_code(codes::PDF_GEOMETRY_DROPPED);
                continue;
            };
            if !self.visible(bounds) {
                continue;
            }
            let Some((color, transparency)) = self.paint(paint) else {
                continue;
            };
            // 線の端（開いた部分経路）とつなぎ（線分が 2 つ以上か、閉じた部分経路）の形は、図面モデルが持たない
            // （8.4.3.3・8.4.3.4）。突き合わせの端と、マイターのつなぎのほかは診断する
            let capped = !subpath.closed && stroke.line_cap != Cap::Butt;
            let joined =
                (subpath.closed || subpath.segments.len() > 1) && stroke.line_join != Join::Miter;
            if capped || joined {
                self.note_code(codes::PDF_STROKE_APPROXIMATED);
            }
            let line_type = self.dash(stroke, scale);
            let style = Style {
                color,
                line_type: Inherit::Own(line_type),
                line_type_scale: 1.0,
                line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(width.max(0.0))))),
                transparency,
            };
            let shape = self.stroke_shape(&subpath);
            self.add(shape, style);
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
    /// （その診断は、見えるかを決める解釈の前の走査で返す）
    fn layer_of(&self, properties: &Dict<'_>) -> Option<LayerId> {
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
        find(first)
    }

    /// 字形の並び（9.4）を文字にする。字形の原点がフォントの送りから大きくずれる所で、別の文字に分ける（[`split`]）
    fn glyph_run(&mut self, run: &GlyphRun<'_, '_>, props: &DrawProps<'_>, mode: &DrawMode) {
        let glyphs = run.glyphs();
        let Some(first) = glyphs.first() else {
            return;
        };
        let device = to_mm() * props.transform;
        let unicode: Vec<Option<BfString>> =
            glyphs.iter().map(|glyph| glyph.as_unicode()).collect();
        let key = RunKey {
            content: text_of(&unicode).0,
            first: (device * first.transform()).as_coeffs(),
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
        let vertical = run.writing_mode() == WritingMode::Vertical;
        for (range, shifted) in split(glyphs, device, vertical) {
            let (Some(part), Some(unicode)) = (glyphs.get(range.clone()), unicode.get(range))
            else {
                continue;
            };
            self.text(part, unicode, device, vertical, props, mode, shifted);
            if self.stopped.is_some() {
                return;
            }
        }
    }

    /// 字形の並びの一部を、1 つの文字にする。最初の字形の基準線が文字の基点と向きで、em の大きさが高さ
    #[allow(clippy::too_many_arguments)]
    fn text(
        &mut self,
        glyphs: &[PositionedGlyph<'_>],
        unicode: &[Option<BfString>],
        device: Affine,
        vertical: bool,
        props: &DrawProps<'_>,
        mode: &DrawMode,
        shifted: bool,
    ) {
        let Some(first) = glyphs.first() else {
            return;
        };
        let (content, missing) = text_of(unicode);
        // 空白だけの並びは、何も描かない
        if content.chars().all(char::is_whitespace) {
            return;
        }
        let transform = device * first.transform();
        // 有限でない座標と、大きさのない文字（`0 Tf`・`0 Tz` と、特異な行列）は移さない
        let bounds = Bounds::of(glyphs.iter().flat_map(|glyph| {
            let transform = device * glyph.transform();
            GLYPH_BOX.map(|corner| transform * corner)
        }));
        let (Some(frame), Some(bounds)) = (Frame::of(transform), bounds) else {
            self.note_code(codes::PDF_GEOMETRY_DROPPED);
            return;
        };
        if !self.visible(bounds) {
            return;
        }
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
        if shifted {
            self.note_code(codes::PDF_GLYPH_POSITION_APPROXIMATED);
        }
        let Some((color, transparency)) = self.paint(&props.paint) else {
            return;
        };
        // 見えない文字（描画モード 3。スキャンした画像に重ねた文字など）は、透明な文字にする
        let transparency = match mode {
            DrawMode::Invisible => Inherit::Own(Transparency { alpha: 0 }),
            _ => transparency,
        };
        // em の高さ（基準線に直交する向き）と、幅の倍率（水平の伸縮 Tz と、文字の行列の歪み）
        let height = frame.height();
        let along = frame.along();
        // em の上向きを、基準線の向きと、それに直交する向きに分けた値。斜体の角度になる
        let slant = frame.ey.dot(along);
        let rise = (frame.ey.y * along.x - frame.ey.x * along.y) * frame.cross.signum();
        let (font, name) = self.font_of(first);
        // 縦書きのフォントの文字スタイルは、Windows の縦書きのフォントの名前と同じく `@` を前に付ける
        let name = if vertical && !name.is_empty() {
            format!("@{name}")
        } else {
            name
        };
        self.styles.text_style(&name, font, vertical);
        // 横書きは字形の原点（左・基準線）に、縦書きは縦の原点（中央・上）に置く
        let (point, horizontal, vertical_alignment) = if vertical {
            (
                transform * VERTICAL_ORIGIN,
                Horizontal::Center,
                Vertical::Top,
            )
        } else {
            (frame.origin, Horizontal::Left, Vertical::Baseline)
        };
        let text = Text {
            content,
            height: Height {
                value: Length::Paper(PaperLength(height)),
                meaning: HeightMeaning::Em,
            },
            placement: Placement::Anchor {
                point: self.point(point),
                horizontal,
                vertical: vertical_alignment,
            },
            rotation: libm::atan2(along.y, along.x).to_degrees(),
            width_factor: frame.width / height,
            oblique: libm::atan2(slant, rise).to_degrees(),
            // 裏返しの変換（行列式が負）は、基準線の向きはそのままに、em の上向きを裏返す
            backward: false,
            upside_down: frame.cross < 0.0,
            style: name,
        };
        let style = Style {
            color,
            line_type: Inherit::Layer,
            line_type_scale: 1.0,
            line_weight: None,
            transparency,
        };
        self.add(Shape::Text(text), style);
    }

    /// 字形のフォントと、その名前（埋め込んだ部分集合の前置きを除いたもの）。hayro-interpret が名前を渡さない
    /// フォント（Type 1・埋め込みのない TrueType）と、UTF-8 でない名前は空にして診断する。Type 3 のフォントは
    /// 名前を持たない
    fn font_of(&mut self, glyph: &Glyph<'_>) -> (Option<Font>, String) {
        let Glyph::Outline(outline) = glyph else {
            return (None, String::new());
        };
        let data = outline.font_data();
        let name = data
            .as_ref()
            .and_then(|data| data.postscript_name.clone())
            .filter(|name| !name.is_empty() && name != NON_UTF8_NAME);
        let (Some(data), Some(name)) = (data, name) else {
            self.note_code(codes::PDF_FONT_NAME_DROPPED);
            return (None, String::new());
        };
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

    fn image(&mut self, image: &Image<'_, '_>, transform: Affine) {
        let stream = match image {
            Image::Raster(raster) => raster.stream(),
            Image::Stencil(stencil) => stencil.stream(),
        };
        let dict = stream.dict();
        // 画素の数（8.9.5.1 の Width・Height、内容の中の画像は W・H）。hayro-interpret は、どちらも持つ画像だけを渡す
        // （hayro-interpret の `Image::width` は公開の API ではないので、辞書から同じ順に読む）
        let size = |short: &[u8], long: &[u8]| {
            dict.get::<u32>(short)
                .or_else(|| dict.get::<u32>(long))
                .map(f64::from)
        };
        let (Some(width), Some(height)) = (size(b"W", b"Width"), size(b"H", b"Height")) else {
            return;
        };
        // 画像の変換は、画素の座標（左上が原点、Y が下）から装置へ（8.9.5、hayro-interpret の約束）
        let transform = to_mm() * transform;
        let origin = transform * Point::new(0.0, height);
        let right = transform * Point::new(width, height);
        let top = transform * Point::new(0.0, 0.0);
        let opposite = transform * Point::new(width, 0.0);
        let Some(bounds) = Bounds::of([origin, right, top, opposite]) else {
            self.note_code(codes::PDF_GEOMETRY_DROPPED);
            return;
        };
        if !self.visible(bounds) {
            return;
        }
        // 色を塗る型の画像（ステンシル、8.9.6.2）の色は移さない。色は、マスクを展開しないと得られない
        if matches!(image, Image::Stencil(_)) {
            self.note_code(codes::PDF_IMAGE_MASK_APPROXIMATED);
        }
        // 画像のソフトマスクとマスク（11.6.5.3・8.9.6.3・8.9.6.4）は移さない
        self.blended = dict.contains_key(b"SMask")
            || dict.contains_key(b"Mask")
            || dict
                .get::<i64>(b"SMaskInData")
                .is_some_and(|value| value != 0);
        // 画像の XObject は、間接オブジェクトの参照（`n g R`）を名前にする。内容の中に書いた画像（BI … EI）は、
        // 名前を持たない
        let (path, definition) = match dict.obj_id() {
            Some(id) => match self.objects.get(&(id.obj_number, id.gen_number)) {
                Some(record) => (
                    format!("{} {} R", id.obj_number, id.gen_number),
                    record.clone(),
                ),
                None => (String::new(), self.source.clone()),
            },
            None => (String::new(), self.source.clone()),
        };
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
        self.add(Shape::Underlay(underlay), style);
    }
}

/// 字形の Unicode をつないだ文字列と、Unicode に戻せない字形の数。戻せない字形は U+FFFD にする
fn text_of(unicode: &[Option<BfString>]) -> (String, u64) {
    let mut content = String::new();
    let mut missing = 0u64;
    for piece in unicode {
        match piece {
            Some(BfString::Char(character)) => content.push(*character),
            Some(BfString::String(text)) => content.push_str(text),
            None => {
                missing += 1;
                content.push(char::REPLACEMENT_CHARACTER);
            }
        }
    }
    (content, missing)
}

impl<'a> Device<'a> for Collector<'_, '_> {
    fn draw_path(&mut self, path: &BezPath, props: DrawProps<'a>, draw_mode: &DrawMode) {
        let count = u64::try_from(path.elements().len()).unwrap_or(u64::MAX);
        if !self.spend(Kind::Work, count.saturating_add(1)) {
            return;
        }
        self.last_fill_run = None;
        self.blended = blended(props.soft_mask.is_some(), props.blend_mode);
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
        self.blended = false;
    }

    fn draw_rect(&mut self, rect: &Rect, props: DrawProps<'a>, draw_mode: &DrawMode) {
        // hayro-interpret は、長方形の形の経路（`re` と、`m`・`l` で描いた軸に平行な長方形、8.5.2.1）を、角の座標に
        // して渡す。左下の角から反時計回りの閉じた経路にする。元の始点と向きは渡されない（推定）
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
        let opacity = if opacity.is_finite() {
            opacity.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.groups.push(Group {
            opacity: self.opacity() * opacity,
            blended: blended(mask.is_some(), blend_mode),
        });
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
        self.blended = blended(props.soft_mask.is_some(), props.blend_mode);
        self.glyph_run(glyph_run, &props, draw_mode);
        self.blended = false;
    }

    fn draw_image(&mut self, image: Image<'a, '_>, props: ImageDrawProps<'a>) {
        if !self.spend(Kind::Work, 1) {
            return;
        }
        self.last_fill_run = None;
        // 画像の状態のソフトマスクと合成の方法は、hayro-interpret が透明のグループにして渡す。`props` には残らない
        self.image(&image, props.transform);
        self.blended = false;
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
        // 有限でない点の外接矩形はない
        assert!(Bounds::of([Point::ZERO, Point::new(f64::NAN, 0.0)]).is_none());
    }

    #[test]
    fn splits_glyph_ranges_at_large_gaps() {
        // 字形の間のずれ（em）が 0.5 より大きい所で分ける
        let found = segments(&[0.0, 2.0, 0.0], |range| !range.is_empty());
        assert_eq!(found, [(0..2, true), (2..4, true)]);
        let found = segments(&[], |_| false);
        assert_eq!(found, [(0..1, false)]);
        assert_eq!(median(vec![3.0, 1.0, 2.0, 4.0]), Some(2.0));
        assert_eq!(median(Vec::new()), None);
    }
}
