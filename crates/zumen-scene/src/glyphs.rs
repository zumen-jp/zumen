//! 呼び出し側が渡したフォントで文字を置き、字形の形と変換をシーンに足す（決定 0188・#875）。
//!
//! - フォントは、文字スタイルと区間のフォントの名前で引く。名前は、同じ名前、ASCII の大文字と小文字を
//!   区別しない一致、フォント拡張子（`.shx`・`.ttf`・`.otf`・`.ttc`）を除いた幹の一致の順で比べる。
//!   合うものがなければ、名前のない既定のフォントで置き（`scene.font.assumed`）、既定もなければ字形を作らない
//!   （`scene.font.missing`）。文字スタイルが大きいフォント（`big_font`）を持てば、主フォントにない文字を
//!   そちらから引く。
//! - OpenType の字形の輪郭は、許容差を字形の単位に直した値を 2 の冪に切り下げた刻みで閉じた折れ線にし、
//!   フォント・字形・刻みの組ごとに 1 つの形として共有する。`.shx` のストロークは開いた折れ線としてシーンの
//!   線に出す。作った点は予算の `segments` で数える。
//! - 字形で置いた文字の高さは、フォントの寸法で大文字の高さに直し、高さの意味のフラグを外す。

use std::collections::BTreeMap;
use std::f64::consts::PI;

use zumen_base::{Diagnostic, Ref, Stop, codes};
use zumen_drawing::transform::Point as Spot;
use zumen_drawing::{
    FormattedText, Height, HeightMeaning, Horizontal, Length, PaperLength, Piece, Placement, Text,
    Vertical, VisitRef,
};
use zumen_geom2d::Xy;
use zumen_text::Typeface;
use zumen_text::font::{Contour, Segment};
use zumen_text::layout::{self, FontMetrics, Glyph, Glyphs, Reference, VerticalGlyph};
use zumen_text::shx::{ShxPath, ShxSegment, ShxStrokeError};

use crate::{Builder, GlyphItem, TextItem, layout::text_flag};

/// 字形の形を折れ線にする刻みの指数の範囲（2^-8〜2^16 の字形の単位）
const LEVELS: [i32; 2] = [-8, 16];
/// 1 つの曲線の区間を分ける数の上限
const PIECES: f64 = 64.0;
/// ストロークで描く字形や空の字形が共有する空の形のキー
const EMPTY_SHAPE_KEY: (usize, u16, i32) = (usize::MAX, u16::MAX, i32::MAX);

fn strip_font_ext(name: &str) -> (&str, bool) {
    if name.len() > 4
        && let Some(ext) = name.as_bytes().get(name.len() - 4..)
        && (ext.eq_ignore_ascii_case(b".shx")
            || ext.eq_ignore_ascii_case(b".ttf")
            || ext.eq_ignore_ascii_case(b".otf")
            || ext.eq_ignore_ascii_case(b".ttc"))
        && let Some((stem, _)) = name.split_at_checked(name.len() - 4)
    {
        return (stem, true);
    }
    (name, false)
}

/// シーンの文字を置くフォント（決定 0188）。名前つきのフォントと、名前の合わない文字に使う既定のフォントと、
/// 形式の宣言値の高さの意味を持つ
#[derive(Default)]
pub struct FontSet<'a> {
    named: Vec<(String, Typeface<'a>)>,
    default: Option<Typeface<'a>>,
    heights: BTreeMap<String, Reference>,
}

impl<'a> FontSet<'a> {
    /// フォントを足す。`name` が `None` なら既定のフォントにする。同じ名前（既定を含む）がすでにあれば
    /// 足さずに偽を返す
    pub fn add(&mut self, name: Option<&str>, face: impl Into<Typeface<'a>>) -> bool {
        let face = face.into();
        match name {
            None if self.default.is_some() => false,
            None => {
                self.default = Some(face);
                true
            }
            Some(name) if self.named.iter().any(|(n, _)| n == name) => false,
            Some(name) => {
                self.named.push((name.to_owned(), face));
                true
            }
        }
    }

    /// 形式の宣言値の高さ（`HeightMeaning::Declared` のキー）が、フォントのどの寸法に当たるかを決める。
    /// 決めないキーは、表示と比べて決めた意味（`dxf.TEXT.40` は TrueType で `A` の字形の上、`.shx` で
    /// 大文字の高さ）にし、それもなければ大文字の高さとみなして `scene.text-height.assumed` を返す
    pub fn declare(&mut self, key: &str, reference: Reference) {
        self.heights.insert(key.to_owned(), reference);
    }

    /// フォントを 1 つも持たない
    pub fn is_empty(&self) -> bool {
        self.named.is_empty() && self.default.is_none()
    }

    /// 番号のフォント
    fn at(&self, index: usize) -> Option<&Typeface<'a>> {
        match self.named.get(index) {
            Some((_, face)) => Some(face),
            None => self.default.as_ref(),
        }
    }

    /// 名前のフォント。番号（名前つきの並び、既定は最後）・フォント・既定で代えたか
    fn find(&self, name: Option<&str>) -> Option<(usize, &Typeface<'a>, bool)> {
        if let Some(name) = name.filter(|n| !n.is_empty()) {
            let (wanted_stem, wanted_has_ext) = strip_font_ext(name);
            let found = self
                .named
                .iter()
                .position(|(n, _)| n == name)
                .or_else(|| {
                    self.named
                        .iter()
                        .position(|(n, _)| n.eq_ignore_ascii_case(name))
                })
                .or_else(|| {
                    self.named.iter().position(|(n, _)| {
                        let (stem, has_ext) = strip_font_ext(n);
                        (has_ext || wanted_has_ext)
                            && !stem.is_empty()
                            && stem.eq_ignore_ascii_case(wanted_stem)
                    })
                });
            if let Some(index) = found {
                return self.named.get(index).map(|(_, face)| (index, face, false));
            }
        }
        self.default
            .as_ref()
            .map(|face| (self.named.len(), face, true))
    }
}

/// 主フォントと大きいフォント（`big_font`）の組。主フォントにない文字を大きいフォントから引き、大文字の高さの
/// 比で主フォントの単位にそろえる
struct PairedFace<'s, 'a> {
    primary_index: usize,
    primary: &'s Typeface<'a>,
    big: Option<(usize, &'s Typeface<'a>, f64)>,
}

impl<'s, 'a> PairedFace<'s, 'a> {
    fn new(fonts: &'s FontSet<'a>, primary_index: usize, big_index: Option<usize>) -> Option<Self> {
        let primary = fonts.at(primary_index)?;
        let big = big_index
            .filter(|&idx| idx != primary_index)
            .and_then(|idx| {
                let bf = fonts.at(idx)?;
                let p_cap = primary.metrics().cap_height;
                let b_cap = bf.metrics().cap_height;
                let ratio = if p_cap > 0.0 && b_cap > 0.0 && (p_cap / b_cap).is_finite() {
                    p_cap / b_cap
                } else {
                    1.0
                };
                Some((idx, bf, ratio))
            });
        Some(Self {
            primary_index,
            primary,
            big,
        })
    }

    fn resolve(&self, ch: char) -> (usize, f64) {
        if self.primary.glyph(ch).is_some() {
            return (self.primary_index, 1.0);
        }
        if let Some((idx, bf, ratio)) = self.big
            && bf.glyph(ch).is_some()
        {
            return (idx, ratio);
        }
        (self.primary_index, 1.0)
    }
}

impl Glyphs for PairedFace<'_, '_> {
    fn metrics(&self) -> FontMetrics {
        self.primary.metrics()
    }

    fn glyph(&self, ch: char) -> Option<Glyph> {
        if let Some(g) = self.primary.glyph(ch) {
            return Some(g);
        }
        let (_, bf, ratio) = self.big?;
        let g = bf.glyph(ch)?;
        Some(Glyph {
            advance: g.advance * ratio,
            bounds: g
                .bounds
                .map(|[x0, y0, x1, y1]| [x0 * ratio, y0 * ratio, x1 * ratio, y1 * ratio]),
        })
    }

    fn fallback(&self) -> Glyph {
        self.primary.fallback()
    }

    fn kerning(&self, left: char, right: char) -> f64 {
        if self.primary.glyph(left).is_some() && self.primary.glyph(right).is_some() {
            return self.primary.kerning(left, right);
        }
        if self.primary.glyph(left).is_none()
            && self.primary.glyph(right).is_none()
            && let Some((_, bf, ratio)) = self.big
        {
            return bf.kerning(left, right) * ratio;
        }
        0.0
    }

    fn vertical_kerning(&self, left: char, right: char) -> f64 {
        if self.primary.glyph(left).is_some() && self.primary.glyph(right).is_some() {
            return self.primary.vertical_kerning(left, right);
        }
        if self.primary.glyph(left).is_none()
            && self.primary.glyph(right).is_none()
            && let Some((_, bf, ratio)) = self.big
        {
            return bf.vertical_kerning(left, right) * ratio;
        }
        0.0
    }

    fn vertical(&self, ch: char) -> Option<VerticalGlyph> {
        if self.primary.glyph(ch).is_some() {
            return self.primary.vertical(ch);
        }
        let (_, bf, ratio) = self.big?;
        bf.glyph(ch)?;
        let v = bf.vertical(ch)?;
        Some(VerticalGlyph {
            glyph: Glyph {
                advance: v.glyph.advance * ratio,
                bounds: v
                    .glyph
                    .bounds
                    .map(|[x0, y0, x1, y1]| [x0 * ratio, y0 * ratio, x1 * ratio, y1 * ratio]),
            },
            advance: v.advance * ratio,
            top: v.top * ratio,
            substituted: v.substituted,
        })
    }

    fn vertical_fallback(&self) -> VerticalGlyph {
        self.primary.vertical_fallback()
    }
}

/// 書式つきの文字を置くのに渡すフォント
struct SpanFonts<'s, 'a> {
    set: &'s FontSet<'a>,
    base: &'s PairedFace<'s, 'a>,
    vertical: bool,
}

impl layout::Fonts for SpanFonts<'_, '_> {
    fn base(&self) -> &dyn Glyphs {
        self.base
    }

    fn font(&self, font: &zumen_drawing::Font) -> Option<&dyn Glyphs> {
        self.set
            .find(Some(&font.name))
            .map(|(_, face, _)| face as &dyn Glyphs)
    }

    fn vertical(&self) -> bool {
        self.vertical
    }
}

/// 書式つきの文字を置いた結果。区間の位置ごとの字形・ストロークと、上下に重ねた文字の線
pub(crate) struct LaidSpans {
    pub glyphs: BTreeMap<usize, Vec<GlyphItem>>,
    pub strokes: BTreeMap<usize, Vec<Vec<Xy>>>,
    pub lines: BTreeMap<usize, [Xy; 2]>,
    /// 上下に重ねた文字の区間の、上の段の字数
    pub uppers: BTreeMap<usize, usize>,
    /// 宣言された高さを大文字の高さに直す倍率
    pub ratio: f64,
}

/// 要素の出所
pub(crate) fn source(visit: &VisitRef<'_, '_>) -> Ref {
    match visit.item {
        zumen_drawing::Item::Model(element) => element.source.clone(),
        zumen_drawing::Item::Paper(element) => element.source.clone(),
        zumen_drawing::Item::ModelAttribute { symbol, .. } => symbol.source.clone(),
        zumen_drawing::Item::PaperAttribute { symbol, .. } => symbol.source.clone(),
    }
}

/// 字形の座標を図面の座標へ写す変換 `g`（[`layout::Laid::transform`] の形）に、要素の空間から起点への
/// 変換 `m`（`zumen_drawing::Affine` の形）を重ねる
fn compose(m: [f64; 6], g: [f64; 6]) -> [f64; 6] {
    let [p, q, r, s, t, u] = m;
    let [a, b, c, d, e, f] = g;
    [
        p * a + q * b,
        s * a + t * b,
        p * c + q * d,
        s * c + t * d,
        p * e + q * f + r,
        s * e + t * f + u,
    ]
}

/// 送りの位置 `pen`（字形の単位）だけ横に動かした変換
fn advanced([a, b, c, d, e, f]: [f64; 6], pen: f64) -> [f64; 6] {
    [a, b, c, d, a * pen + e, b * pen + f]
}

/// 字形の座標軸を `ratio` 倍に縮尺した変換
fn scaled_glyph([a, b, c, d, e, f]: [f64; 6], ratio: f64) -> [f64; 6] {
    if ratio == 1.0 {
        [a, b, c, d, e, f]
    } else {
        [a * ratio, b * ratio, c * ratio, d * ratio, e, f]
    }
}

/// 表示と比べて決めた、形式の宣言値の高さの意味。呼び出し側が決めなければこれを使う
fn known(key: &str, is_shx: bool) -> Option<Reference> {
    match key {
        // AutoCAD 2027 で、TrueType の TEXT の高さは `A` の字形の上、SHX は `H` の上（above）に当たる（決定 0246）
        "dxf.TEXT.40" if is_shx => Some(Reference::CapHeight),
        "dxf.TEXT.40" => Some(Reference::CapitalA),
        _ => None,
    }
}

/// DXF・DWG の文字（高さの意味のキーが `dxf.` で始まる）なら、特殊な符号を解く（決定 0181）
fn decoded(content: &str, height: &Height, toggles: bool) -> String {
    match &height.meaning {
        HeightMeaning::Declared(key) if key.starts_with("dxf.") => {
            layout::decode(content, toggles).text
        }
        _ => content.to_owned(),
    }
}

/// 刻み `2^level` の指数。許容差を字形の単位に直した値を 2 の冪に切り下げる。ビットから決めるので、
/// どの環境でも同じ値になる
fn level(tolerance: f64) -> i32 {
    if !(tolerance.is_finite() && tolerance > 0.0) {
        return if tolerance > 0.0 {
            LEVELS[1]
        } else {
            LEVELS[0]
        };
    }
    let exponent = ((tolerance.to_bits() >> 52) & 0x7ff) as i32 - 1023;
    exponent.clamp(LEVELS[0], LEVELS[1])
}

fn power(level: i32) -> f64 {
    f64::from_bits(((level + 1023) as u64) << 52)
}

/// 曲線の区間を分ける数。弦の誤差が刻み以下になる数（`deviation` は 1 区間で描いたときの誤差の上限）
fn pieces(deviation: f64, tolerance: f64) -> usize {
    if deviation.partial_cmp(&tolerance) != Some(core::cmp::Ordering::Greater) {
        return 1;
    }
    let n = libm::ceil(libm::sqrt(deviation / tolerance)).min(PIECES);
    if n.is_finite() && n >= 1.0 {
        n as usize
    } else {
        1
    }
}

/// 輪郭を、刻み `tolerance`（字形の単位）の折れ線の輪にする。2 次・3 次の Bézier 曲線は媒介変数を等分し、
/// 分ける数は 2 階の差分から決める
pub(crate) fn flatten(contours: &[Contour], tolerance: f64) -> Vec<Vec<Xy>> {
    let norm = |[x, y]: Xy| libm::hypot(x, y);
    let mut rings = Vec::new();
    for contour in contours {
        let mut ring = vec![contour.start];
        let mut p = contour.start;
        for segment in &contour.segments {
            match *segment {
                Segment::Line(q) => {
                    ring.push(q);
                    p = q;
                }
                Segment::Quad(c, q) => {
                    let n = pieces(
                        norm([p[0] - 2.0 * c[0] + q[0], p[1] - 2.0 * c[1] + q[1]]) / 4.0,
                        tolerance,
                    );
                    for i in 1..=n {
                        let t = i as f64 / n as f64;
                        let s = 1.0 - t;
                        ring.push([
                            s * s * p[0] + 2.0 * s * t * c[0] + t * t * q[0],
                            s * s * p[1] + 2.0 * s * t * c[1] + t * t * q[1],
                        ]);
                    }
                    p = q;
                }
                Segment::Cubic(c1, c2, q) => {
                    let m =
                        norm([p[0] - 2.0 * c1[0] + c2[0], p[1] - 2.0 * c1[1] + c2[1]]).max(norm([
                            c1[0] - 2.0 * c2[0] + q[0],
                            c1[1] - 2.0 * c2[1] + q[1],
                        ]));
                    let n = pieces(m * 0.75, tolerance);
                    for i in 1..=n {
                        let t = i as f64 / n as f64;
                        let s = 1.0 - t;
                        let [w0, w1, w2, w3] =
                            [s * s * s, 3.0 * s * s * t, 3.0 * s * t * t, t * t * t];
                        ring.push([
                            w0 * p[0] + w1 * c1[0] + w2 * c2[0] + w3 * q[0],
                            w0 * p[1] + w1 * c1[1] + w2 * c2[1] + w3 * q[1],
                        ]);
                    }
                    p = q;
                }
            }
        }
        // 輪は閉じているものとし、始点を繰り返さない
        while ring.len() > 1 && ring.last() == ring.first() {
            ring.pop();
        }
        if ring.len() >= 3 {
            rings.push(ring);
        }
    }
    rings
}

/// `.shx` のストロークの列を、刻み `tolerance`（字形の単位）の開いた折れ線にする。円弧があれば真を返す
fn flatten_shx(paths: &[ShxPath], tolerance: f64) -> (Vec<Vec<Xy>>, bool) {
    let mut out = Vec::with_capacity(paths.len());
    let mut has_arc = false;
    for poly in paths {
        let mut pts = Vec::with_capacity(poly.segments.len() + 1);
        pts.push(poly.start);
        for seg in &poly.segments {
            match *seg {
                ShxSegment::Line(q) => pts.push(q),
                ShxSegment::Arc {
                    center,
                    radius,
                    start,
                    sweep,
                    end,
                } => {
                    let abs_sweep = sweep.abs();
                    if radius > 0.0
                        && abs_sweep > 0.0
                        && radius.is_finite()
                        && abs_sweep.is_finite()
                    {
                        has_arc = true;
                        let step =
                            if tolerance.is_finite() && tolerance > 0.0 && tolerance < radius {
                                2.0 * libm::acos(1.0 - tolerance / radius)
                            } else {
                                PI / 2.0
                            }
                            .clamp(f64::MIN_POSITIVE, PI / 2.0);
                        let n = libm::ceil(abs_sweep / step).clamp(1.0, PIECES) as usize;
                        for i in 1..n {
                            let a = start + sweep * (i as f64 / n as f64);
                            pts.push([
                                center[0] + radius * libm::cos(a),
                                center[1] + radius * libm::sin(a),
                            ]);
                        }
                        pts.push(end);
                    }
                }
            }
        }
        if pts.len() >= 2 {
            out.push(pts);
        }
    }
    (out, has_arc)
}

fn transform_polyline(points: &[Xy], [a, b, c, d, e, f]: [f64; 6]) -> Vec<Xy> {
    points
        .iter()
        .map(|&[x, y]| [a * x + c * y + e, b * x + d * y + f])
        .collect()
}

struct EmittedShx {
    row: u32,
    paths: Vec<Vec<Xy>>,
}

impl Builder<'_, '_, '_> {
    fn empty_shape_row(&mut self) -> u32 {
        if let Some(&row) = self.shape_rows.get(&EMPTY_SHAPE_KEY) {
            return row;
        }
        let row = u32::try_from(self.shapes.len()).unwrap_or(crate::layout::NONE);
        self.shapes.push(Vec::new());
        self.shape_rows.insert(EMPTY_SHAPE_KEY, row);
        row
    }

    fn emit_shx_glyph(
        &mut self,
        face: usize,
        ch: char,
        vertical: bool,
        transform: [f64; 6],
        review_budget: bool,
        source: &Ref,
    ) -> Result<EmittedShx, (Stop, Ref)> {
        use zumen_base::Kind;

        if review_budget {
            self.cx
                .check(Kind::Work, 1 << 20)
                .map_err(|s| (s, source.clone()))?;
            self.cx
                .check(Kind::DecodedBytes, 1 << 20)
                .map_err(|s| (s, source.clone()))?;
        }
        let Some(shx) = self.fonts.at(face).and_then(Typeface::as_shx) else {
            return Ok(EmittedShx {
                row: self.empty_shape_row(),
                paths: Vec::new(),
            });
        };
        let strokes = match shx.strokes(ch, vertical) {
            Ok(Some(strokes)) => strokes,
            Ok(None) => {
                return Ok(EmittedShx {
                    row: self.empty_shape_row(),
                    paths: Vec::new(),
                });
            }
            Err(ShxStrokeError::UnsupportedOpcode(op)) => {
                self.cx.report(
                    Diagnostic::new(codes::SCENE_SHX_OPCODE_UNSUPPORTED)
                        .with("opcode", u64::from(op))
                        .at(source.clone()),
                );
                return Ok(EmittedShx {
                    row: self.empty_shape_row(),
                    paths: Vec::new(),
                });
            }
            Err(ShxStrokeError::Malformed) => {
                self.report(codes::SCENE_GLYPH_OUTLINE_INVALID, source);
                return Ok(EmittedShx {
                    row: self.empty_shape_row(),
                    paths: Vec::new(),
                });
            }
        };

        let [a, b, c, d, ..] = transform;
        let scale = libm::hypot(a, b).max(libm::hypot(c, d));
        let tol = power(level(self.tolerance / scale));
        if review_budget {
            let mut upper = 0u64;
            for poly in &strokes.paths {
                self.cx
                    .check(Kind::Work, 1 + poly.segments.len() as u64)
                    .map_err(|s| (s, source.clone()))?;
                upper = upper.saturating_add(1 + (poly.segments.len() as u64).saturating_mul(64));
            }
            self.cx
                .check(Kind::Work, upper.saturating_mul(64))
                .map_err(|s| (s, source.clone()))?;
            self.cx
                .check(Kind::Segments, upper)
                .map_err(|s| (s, source.clone()))?;
            self.cx
                .check(
                    Kind::DecodedBytes,
                    upper.saturating_mul(128).saturating_add(1024),
                )
                .map_err(|s| (s, source.clone()))?;
        }
        let (flat, has_arc) = flatten_shx(&strokes.paths, tol);
        if has_arc {
            self.approximated = true;
        }
        if !review_budget {
            let total_points: usize = flat.iter().map(Vec::len).sum();
            self.segments(total_points, source)?;
        }
        let world: Vec<Vec<Xy>> = flat
            .iter()
            .map(|pts| transform_polyline(pts, transform))
            .collect();
        Ok(EmittedShx {
            row: self.empty_shape_row(),
            paths: world,
        })
    }

    /// レビュー専用の事前予約。通常のシーンの予算と字形キャッシュの規則は変えない（決定 0295）。
    pub(crate) fn lay_review_text<P: Spot>(
        &mut self,
        visit: &VisitRef<'_, '_>,
        text: &Text<P>,
        font: Option<&str>,
        item: &mut TextItem,
    ) -> Result<(), (Stop, Ref)> {
        use zumen_base::Kind;

        if self.fonts.is_empty() {
            return Ok(());
        }
        let source = source(visit);
        let lookup = (self.fonts.named.len() as u64)
            .saturating_mul(font.map_or(1, |s| s.len() as u64 + 1))
            .saturating_mul(4);
        self.cx
            .check(Kind::Work, lookup)
            .map_err(|s| (s, source.clone()))?;
        let Some(index) = self.typeface(font, &source) else {
            return Ok(());
        };
        // glyf の点と部品、CFF/CFF2 の数と命令、SHX の命令数は 2^16 以下。字形の探索と寸法の読み直しも含め、
        // 1 文字につきその 256 倍の作業を、配置する前に予約する。キャッシュした輪郭の量はここに重ねない。
        self.cx
            .check(
                Kind::Work,
                (text.content.len() as u64).saturating_mul(1 << 24),
            )
            .map_err(|s| (s, source.clone()))?;
        self.cx
            .check(
                Kind::DecodedBytes,
                (text.content.len() as u64).saturating_mul(256),
            )
            .map_err(|s| (s, source.clone()))?;
        let fonts = self.fonts;
        let Some(face) = fonts.at(index) else {
            return Ok(());
        };
        let reference = self.reference(&text.height, face.is_shx(), &source);
        let Ok(laid) = layout::place(text, |p| p.xy(), face, |_| Some(reference)) else {
            self.unplaced(&source);
            return Ok(());
        };
        if laid.missing > 0 {
            self.report(codes::SCENE_GLYPH_MISSING, &source);
        }
        self.segments(laid.pens.len(), &source)?;
        let mut glyphs = Vec::with_capacity(laid.pens.len());
        for (ch, pen) in text.content.chars().zip(&laid.pens) {
            let transform = compose(visit.transform.m, advanced(laid.transform, *pen));
            let shape = match fonts.at(index) {
                Some(Typeface::OpenType(ot)) => {
                    let id = ot.glyph_id(ch).unwrap_or(0);
                    self.review_glyph_shape(index, id, transform, &source)?
                }
                Some(Typeface::Shx(_)) => {
                    let emitted =
                        self.emit_shx_glyph(index, ch, false, transform, true, &source)?;
                    for path in emitted.paths {
                        for &pt in &path {
                            item.bounds.add(pt);
                        }
                        self.line(path, item.unit);
                    }
                    emitted.row
                }
                None => self.empty_shape_row(),
            };
            glyphs.push(GlyphItem { shape, transform });
        }
        item.glyphs = glyphs;
        item.height *= face.metrics().cap_height / layout::unit(reference, face);
        item.flags &= !(text_flag::HEIGHT_FRAME | text_flag::HEIGHT_EM);
        Ok(())
    }

    fn review_glyph_shape(
        &mut self,
        face: usize,
        id: u16,
        transform: [f64; 6],
        source: &Ref,
    ) -> Result<u32, (Stop, Ref)> {
        use zumen_base::Kind;

        let [a, b, c, d, ..] = transform;
        let scale = libm::hypot(a, b).max(libm::hypot(c, d));
        let level = level(self.tolerance / scale);
        let key = (face, id, level);
        self.cx
            .check(
                Kind::Work,
                (u64::from(self.shape_rows.len().max(1).ilog2()) + 1) * 4,
            )
            .map_err(|s| (s, source.clone()))?;
        if let Some(row) = self.shape_rows.get(&key) {
            return Ok(*row);
        }
        // zumen-text の glyf/CFF/CFF2 は、1 字形の点・部品または数・命令を 2^16 に制限する。
        // 輪郭を読む前に、その 256 バイト/項目を予約する。glyf の端点・旗・座標・点・輪郭の Vec、
        // CFF のスタックと輪郭、深さ 8 の glyf と深さ 10 の CFF の一時の列を含める。
        self.cx
            .check(Kind::Work, 1 << 24)
            .map_err(|s| (s, source.clone()))?;
        self.cx
            .check(Kind::DecodedBytes, (1 << 24) + (1 << 20))
            .map_err(|s| (s, source.clone()))?;
        let outline = self
            .fonts
            .at(face)
            .and_then(Typeface::as_opentype)
            .and_then(|font| font.outline(id));
        let rings = if let Some(contours) = outline {
            let mut upper = 0u64;
            for contour in &contours {
                self.cx
                    .check(Kind::Work, 1 + contour.segments.len() as u64)
                    .map_err(|s| (s, source.clone()))?;
                // flatten は 1 区間を最大 64 分割し、輪の始点を 1 つ足す。
                upper =
                    upper.saturating_add(1 + (contour.segments.len() as u64).saturating_mul(64));
                if contour
                    .segments
                    .iter()
                    .any(|s| !matches!(s, Segment::Line(_)))
                {
                    self.approximated = true;
                }
            }
            self.cx
                .check(Kind::Work, upper.saturating_mul(64))
                .map_err(|s| (s, source.clone()))?;
            self.cx
                .check(Kind::Segments, upper)
                .map_err(|s| (s, source.clone()))?;
            // 輪郭の列の容量の増加と、列・最終のバッファーへ写す分も含める。
            self.cx
                .check(
                    Kind::DecodedBytes,
                    upper.saturating_mul(128).saturating_add(1024),
                )
                .map_err(|s| (s, source.clone()))?;
            flatten(&contours, power(level))
        } else {
            self.report(codes::SCENE_GLYPH_OUTLINE_INVALID, source);
            Vec::new()
        };
        let row = u32::try_from(self.shapes.len()).unwrap_or(crate::layout::NONE);
        self.shapes.push(rings);
        self.shape_rows.insert(key, row);
        Ok(row)
    }

    /// 高さの意味に当たる寸法。決めていない宣言値は、表示と比べて決めた意味か、大文字の高さとみなす
    fn reference(&mut self, height: &Height, is_shx: bool, source: &Ref) -> Reference {
        match &height.meaning {
            HeightMeaning::CapHeight => Reference::CapHeight,
            HeightMeaning::Em => Reference::Em,
            HeightMeaning::Frame => Reference::Frame,
            HeightMeaning::Declared(key) => match self
                .fonts
                .heights
                .get(key.as_ref())
                .copied()
                .or_else(|| known(key, is_shx))
            {
                Some(reference) => reference,
                None => {
                    self.cx.report(
                        Diagnostic::new(codes::SCENE_TEXT_HEIGHT_ASSUMED)
                            .with("key", key.as_ref())
                            .at(source.clone()),
                    );
                    Reference::CapHeight
                }
            },
        }
    }

    /// 名前のフォントを引く。代えたか、なければ診断を返す
    fn typeface(&mut self, name: Option<&str>, source: &Ref) -> Option<usize> {
        let found = self
            .fonts
            .find(name)
            .map(|(index, _, assumed)| (index, assumed));
        let code = match found {
            Some((_, false)) => None,
            Some((_, true)) => Some(codes::SCENE_FONT_ASSUMED),
            None => Some(codes::SCENE_FONT_MISSING),
        };
        if let Some(code) = code {
            self.cx.report(
                Diagnostic::new(code)
                    .with("name", name.unwrap_or_default())
                    .at(source.clone()),
            );
        }
        found.map(|(index, _)| index)
    }

    /// 字形の形の行。フォント・字形・刻みの組ごとに 1 度だけ輪郭を読んで折れ線にする
    fn glyph_shape(
        &mut self,
        face: usize,
        id: u16,
        transform: [f64; 6],
        source: &Ref,
    ) -> Result<u32, (Stop, Ref)> {
        let [a, b, c, d, ..] = transform;
        let scale = libm::hypot(a, b).max(libm::hypot(c, d));
        let level = level(self.tolerance / scale);
        let key = (face, id, level);
        if let Some(row) = self.shape_rows.get(&key) {
            return Ok(*row);
        }
        let outline = self
            .fonts
            .at(face)
            .and_then(Typeface::as_opentype)
            .and_then(|f| f.outline(id));
        let rings = match outline {
            Some(contours) => {
                if contours
                    .iter()
                    .flat_map(|c| &c.segments)
                    .any(|s| !matches!(s, Segment::Line(_)))
                {
                    self.approximated = true;
                }
                flatten(&contours, power(level))
            }
            None => {
                self.report(codes::SCENE_GLYPH_OUTLINE_INVALID, source);
                Vec::new()
            }
        };
        self.segments(rings.iter().map(Vec::len).sum(), source)?;
        let row = u32::try_from(self.shapes.len()).unwrap_or(crate::layout::NONE);
        self.shapes.push(rings);
        self.shape_rows.insert(key, row);
        Ok(row)
    }

    /// 字形を置けなかった文字を報告する
    fn unplaced(&mut self, source: &Ref) {
        self.report(codes::SCENE_TEXT_LAYOUT_UNSUPPORTED, source);
    }

    /// 1 行の文字を、フォントで字形に置く。置けなければ文字列だけにする
    pub(crate) fn lay_text<P: Spot>(
        &mut self,
        visit: &VisitRef<'_, '_>,
        text: &Text<P>,
        font: Option<&str>,
        big_font: Option<&str>,
        vertical: bool,
        item: &mut TextItem,
    ) -> Result<(), (Stop, Ref)> {
        if self.fonts.is_empty() {
            return Ok(());
        }
        let source = source(visit);
        let Some(index) = self.typeface(font, &source) else {
            return Ok(());
        };
        let big_index = big_font
            .filter(|n| !n.is_empty())
            .and_then(|n| self.typeface(Some(n), &source));
        if vertical {
            // 縦書きの 1 行の文字は、置き方（決定 0176）が扱わない
            self.unplaced(&source);
            return Ok(());
        }
        let fonts = self.fonts;
        let Some(pair) = PairedFace::new(fonts, index, big_index) else {
            return Ok(());
        };
        let reference = self.reference(&text.height, pair.primary.is_shx(), &source);
        let mut laid_text = text.clone();
        laid_text.content = decoded(&text.content, &text.height, true);
        let Ok(laid) = layout::place(&laid_text, |p| p.xy(), &pair, |_| Some(reference)) else {
            self.unplaced(&source);
            return Ok(());
        };
        let unit_value = layout::unit(reference, &pair);
        let metrics = pair.metrics();
        if laid.missing > 0 {
            self.report(codes::SCENE_GLYPH_MISSING, &source);
        }
        self.segments(laid.pens.len(), &source)?;
        let mut glyphs = Vec::with_capacity(laid.pens.len());
        for (ch, pen) in laid_text.content.chars().zip(&laid.pens) {
            let (face_idx, ratio) = pair.resolve(ch);
            let local = scaled_glyph(advanced(laid.transform, *pen), ratio);
            let transform = compose(visit.transform.m, local);
            let shape = match fonts.at(face_idx) {
                Some(Typeface::OpenType(ot)) => {
                    let id = ot.glyph_id(ch).unwrap_or(0);
                    self.glyph_shape(face_idx, id, transform, &source)?
                }
                Some(Typeface::Shx(_)) => {
                    let emitted =
                        self.emit_shx_glyph(face_idx, ch, false, transform, false, &source)?;
                    for path in emitted.paths {
                        self.line(path, item.unit);
                    }
                    emitted.row
                }
                None => self.empty_shape_row(),
            };
            glyphs.push(GlyphItem { shape, transform });
        }
        item.glyphs = glyphs;
        item.height *= metrics.cap_height / unit_value;
        item.flags &= !(text_flag::HEIGHT_FRAME | text_flag::HEIGHT_EM);
        Ok(())
    }

    /// 書式つきの文字を、フォントで字形に置く。置けなければ `None`
    pub(crate) fn lay_formatted<P: Spot>(
        &mut self,
        visit: &VisitRef<'_, '_>,
        text: &FormattedText<P>,
        font: Option<&str>,
        big_font: Option<&str>,
        vertical: bool,
    ) -> Result<Option<LaidSpans>, (Stop, Ref)> {
        if self.fonts.is_empty() {
            return Ok(None);
        }
        let source = source(visit);
        let Some(base) = self.typeface(font, &source) else {
            return Ok(None);
        };
        let big_index = big_font
            .filter(|n| !n.is_empty())
            .and_then(|n| self.typeface(Some(n), &source));
        // 区間で指定されたフォント。合うものがなければ既定で、既定もなければ文字スタイルのフォントで置く
        let mut faces: Vec<Option<usize>> = Vec::with_capacity(text.spans.len());
        for span in &text.spans {
            let face = match &span.format.font {
                Some(wanted) if matches!(span.piece, Piece::Text(_) | Piece::Stack { .. }) => {
                    match self.fonts.find(Some(&wanted.name)) {
                        Some((index, _, false)) => Some(index),
                        found => {
                            self.cx.report(
                                Diagnostic::new(codes::SCENE_FONT_ASSUMED)
                                    .with("name", wanted.name.as_str())
                                    .at(source.clone()),
                            );
                            found.map(|(index, _, _)| index)
                        }
                    }
                }
                _ => None,
            };
            faces.push(face);
        }
        let fonts = self.fonts;
        let Some(base_pair) = PairedFace::new(fonts, base, big_index) else {
            return Ok(None);
        };
        let reference = self.reference(&text.height, base_pair.primary.is_shx(), &source);
        let mut laid_text = text.clone();
        for span in &mut laid_text.spans {
            match &mut span.piece {
                Piece::Text(content) => *content = decoded(content, &text.height, false),
                Piece::Stack { upper, lower, .. } => {
                    *upper = decoded(upper, &text.height, false);
                    *lower = decoded(lower, &text.height, false);
                }
                _ => {}
            }
        }
        let metrics = base_pair.metrics();
        let span_fonts = SpanFonts {
            set: fonts,
            base: &base_pair,
            vertical,
        };
        let Ok(laid) =
            layout::place_formatted(&laid_text, |p| p.xy(), &span_fonts, |_| Some(reference))
        else {
            self.unplaced(&source);
            return Ok(None);
        };
        if laid.missing > 0 {
            self.report(codes::SCENE_GLYPH_MISSING, &source);
        }
        if !laid.skipped.is_empty() {
            self.unplaced(&source);
        }
        self.segments(laid.glyphs.len(), &source)?;
        let m = visit.transform.m;
        let mut out = LaidSpans {
            glyphs: BTreeMap::new(),
            strokes: BTreeMap::new(),
            lines: BTreeMap::new(),
            uppers: laid_text
                .spans
                .iter()
                .enumerate()
                .filter_map(|(index, span)| match &span.piece {
                    Piece::Stack { upper, .. } => Some((index, upper.chars().count())),
                    _ => None,
                })
                .collect(),
            ratio: metrics.cap_height / layout::unit(reference, &base_pair),
        };
        for glyph in &laid.glyphs {
            let (face_idx, ratio) = match faces.get(glyph.span).copied().flatten() {
                Some(explicit) => (explicit, 1.0),
                None => base_pair.resolve(glyph.ch),
            };
            let transform = compose(m, scaled_glyph(glyph.transform, ratio));
            let shape = match fonts.at(face_idx) {
                Some(Typeface::OpenType(ot)) => {
                    let mut id = ot.glyph_id(glyph.ch).unwrap_or(0);
                    if glyph.substituted {
                        id = ot.vertical_id(id);
                    }
                    self.glyph_shape(face_idx, id, transform, &source)?
                }
                Some(Typeface::Shx(_)) => {
                    let emitted = self
                        .emit_shx_glyph(face_idx, glyph.ch, vertical, transform, false, &source)?;
                    if !emitted.paths.is_empty() {
                        out.strokes
                            .entry(glyph.span)
                            .or_default()
                            .extend(emitted.paths);
                    }
                    emitted.row
                }
                None => self.empty_shape_row(),
            };
            out.glyphs
                .entry(glyph.span)
                .or_default()
                .push(GlyphItem { shape, transform });
        }
        for stack in &laid.stacks {
            if let Some([p, q]) = stack.line {
                let map = |[x, y]: Xy| {
                    let (x, y) = visit.transform.apply(x, y);
                    [x, y]
                };
                out.lines.insert(stack.span, [map(p), map(q)]);
            }
        }
        Ok(Some(out))
    }

    /// 線種に添える文字を、フォントで字形に置く。`at` と `rotation` は起点の座標
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lay_mark(
        &mut self,
        content: &str,
        at: Xy,
        height: f64,
        rotation: f64,
        font: Option<&str>,
        big_font: Option<&str>,
        item: &mut TextItem,
        source: &Ref,
    ) -> Result<(), (Stop, Ref)> {
        if self.fonts.is_empty() {
            return Ok(());
        }
        let Some(index) = self.typeface(font, source) else {
            return Ok(());
        };
        let big_index = big_font
            .filter(|n| !n.is_empty())
            .and_then(|n| self.typeface(Some(n), source));
        let fonts = self.fonts;
        let Some(pair) = PairedFace::new(fonts, index, big_index) else {
            return Ok(());
        };
        // 線種の文字の高さは、文字スタイルの高さに倍率を掛けた値で、TEXT の高さと同じ意味とみなす（推定）
        let declared = Height {
            value: Length::Paper(PaperLength(height)),
            meaning: HeightMeaning::Declared("dxf.TEXT.40".into()),
        };
        let reference = self.reference(&declared, pair.primary.is_shx(), source);
        let text = Text {
            content: content.to_owned(),
            height: declared,
            placement: Placement::Anchor {
                point: at,
                horizontal: Horizontal::Left,
                vertical: Vertical::Baseline,
            },
            rotation,
            width_factor: 1.0,
            oblique: 0.0,
            backward: false,
            upside_down: false,
            style: String::new(),
        };
        let Ok(laid) = layout::place(&text, |p| *p, &pair, |_| Some(reference)) else {
            self.report(codes::SCENE_TEXT_LAYOUT_UNSUPPORTED, source);
            return Ok(());
        };
        let unit_value = layout::unit(reference, &pair);
        let metrics = pair.metrics();
        if laid.missing > 0 {
            self.report(codes::SCENE_GLYPH_MISSING, source);
        }
        self.segments(laid.pens.len(), source)?;
        let mut glyphs = Vec::with_capacity(laid.pens.len());
        for (ch, pen) in content.chars().zip(&laid.pens) {
            let (face_idx, ratio) = pair.resolve(ch);
            let transform = scaled_glyph(advanced(laid.transform, *pen), ratio);
            let shape = match fonts.at(face_idx) {
                Some(Typeface::OpenType(ot)) => {
                    let id = ot.glyph_id(ch).unwrap_or(0);
                    self.glyph_shape(face_idx, id, transform, source)?
                }
                Some(Typeface::Shx(_)) => {
                    let emitted =
                        self.emit_shx_glyph(face_idx, ch, false, transform, false, source)?;
                    for path in emitted.paths {
                        self.line(path, item.unit);
                    }
                    emitted.row
                }
                None => self.empty_shape_row(),
            };
            glyphs.push(GlyphItem { shape, transform });
        }
        item.glyphs = glyphs;
        item.height *= metrics.cap_height / unit_value;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_round_down_to_powers_of_two() {
        assert_eq!(level(1.0), 0);
        assert_eq!(level(1.5), 0);
        assert_eq!(level(0.3), -2);
        assert_eq!(level(1e-30), LEVELS[0]);
        assert_eq!(level(f64::INFINITY), LEVELS[1]);
        assert_eq!(level(0.0), LEVELS[0]);
        assert_eq!(power(-2), 0.25);
    }

    #[test]
    fn flattens_curves_within_the_step() {
        let contour = Contour {
            start: [0.0, 0.0],
            segments: vec![
                Segment::Line([100.0, 0.0]),
                Segment::Quad([100.0, 100.0], [0.0, 100.0]),
                Segment::Line([0.0, 0.0]),
            ],
        };
        let rings = flatten(std::slice::from_ref(&contour), 1.0);
        assert_eq!(rings.len(), 1);
        // 2 階の差分の大きさ 141.4 の 1/4 を 1 以下にする数は 6
        assert_eq!(rings[0].len(), 2 + 6);
        assert_eq!(rings[0][0], [0.0, 0.0]);
        // 2 点だけの輪は描かない
        let thin = Contour {
            start: [0.0, 0.0],
            segments: vec![Segment::Line([1.0, 0.0])],
        };
        assert!(flatten(&[thin], 1.0).is_empty());
    }
}
