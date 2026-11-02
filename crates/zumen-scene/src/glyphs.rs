//! 呼び出し側が渡したフォントで文字を置き、字形の形と変換をシーンに足す（決定 0188）。
//!
//! - フォントは、文字スタイルと区間のフォントの名前で引く。名前は、まず同じ名前、なければ ASCII の大文字と
//!   小文字を区別せずに比べる。合うものがなければ、名前のない既定のフォントで置き
//!   （`scene.font.assumed`）、既定もなければ字形を作らない（`scene.font.missing`）。
//! - 字形の輪郭は、許容差を字形の単位に直した値を 2 の冪に切り下げた刻みで折れ線にし、フォント・字形・
//!   刻みの組ごとに 1 つの形として共有する。作った点は予算の `segments` で数える。
//! - 字形で置いた文字の高さは、フォントの寸法で大文字の高さに直し、高さの意味の印を外す。

use std::collections::BTreeMap;

use zumen_base::{Diagnostic, Ref, Stop, codes};
use zumen_drawing::font::{Contour, Font as Face, Segment};
use zumen_drawing::layout::{self, Glyphs, Reference};
use zumen_drawing::transform::Point as Spot;
use zumen_drawing::{
    FormattedText, Height, HeightMeaning, Horizontal, Length, PaperLength, Piece, Placement, Text,
    Vertical, Visit,
};
use zumen_geom2d::Xy;

use crate::{Builder, GlyphItem, TextItem, layout::text_flag};

/// 字形の形を折れ線にする刻みの指数の範囲（2^-8〜2^16 の字形の単位）
const LEVELS: [i32; 2] = [-8, 16];
/// 1 つの曲線の区間を分ける数の上限
const PIECES: f64 = 64.0;

/// シーンの文字を置くフォント（決定 0188）。名前つきのフォントと、名前の合わない文字に使う既定のフォントと、
/// 形式の宣言値の高さの意味を持つ
#[derive(Default)]
pub struct FontSet<'a> {
    named: Vec<(String, Face<'a>)>,
    default: Option<Face<'a>>,
    heights: BTreeMap<String, Reference>,
}

impl<'a> FontSet<'a> {
    /// フォントを足す。`name` が `None` なら既定のフォントにする。同じ名前（既定を含む）がすでにあれば
    /// 足さずに偽を返す
    pub fn add(&mut self, name: Option<&str>, face: Face<'a>) -> bool {
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
    /// 決めないキーは、表示と比べて決めた意味（`dxf.TEXT.40` は `A` の字形の上）にし、それもなければ
    /// 大文字の高さとみなして `scene.text-height.assumed` を返す
    pub fn declare(&mut self, key: &str, reference: Reference) {
        self.heights.insert(key.to_owned(), reference);
    }

    /// フォントを 1 つも持たない
    pub fn is_empty(&self) -> bool {
        self.named.is_empty() && self.default.is_none()
    }

    /// 番号のフォント
    fn at(&self, index: usize) -> Option<&Face<'a>> {
        match self.named.get(index) {
            Some((_, face)) => Some(face),
            None => self.default.as_ref(),
        }
    }

    /// 名前のフォント。番号（名前つきの並び、既定は最後）・フォント・既定で代えたか
    fn find(&self, name: Option<&str>) -> Option<(usize, &Face<'a>, bool)> {
        if let Some(name) = name.filter(|n| !n.is_empty()) {
            let found = self.named.iter().position(|(n, _)| n == name).or_else(|| {
                self.named
                    .iter()
                    .position(|(n, _)| n.eq_ignore_ascii_case(name))
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

/// 書式つきの文字を置くのに渡すフォント
struct SpanFonts<'s, 'a> {
    set: &'s FontSet<'a>,
    base: &'s Face<'a>,
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

/// 書式つきの文字を置いた結果。区間の位置ごとの字形と、上下に重ねた文字の線
pub(crate) struct LaidSpans {
    pub glyphs: BTreeMap<usize, Vec<GlyphItem>>,
    pub lines: BTreeMap<usize, [Xy; 2]>,
    /// 上下に重ねた文字の区間の、上の段の字数
    pub uppers: BTreeMap<usize, usize>,
    /// 宣言された高さを大文字の高さに直す倍率
    pub ratio: f64,
}

/// 要素の出所
pub(crate) fn source(visit: &Visit<'_>) -> Ref {
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

/// 表示と比べて決めた、形式の宣言値の高さの意味。呼び出し側が決めなければこれを使う
fn known(key: &str) -> Option<Reference> {
    match key {
        // AutoCAD 2027 で、TrueType の TEXT の高さは `A` の字形の上に当たる（決定 0246）
        "dxf.TEXT.40" => Some(Reference::CapitalA),
        _ => None,
    }
}

/// DXF・DWG の文字（高さの意味のキーが `dxf.` で始まる）なら、特殊な符号を解く（決定 0181）
fn decoded(content: &str, height: &Height, toggles: bool) -> String {
    match height.meaning {
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

impl Builder<'_, '_, '_> {
    /// 高さの意味に当たる寸法。決めていない宣言値は、表示と比べて決めた意味か、大文字の高さとみなす
    fn reference(&mut self, height: &Height, source: &Ref) -> Reference {
        match height.meaning {
            HeightMeaning::CapHeight => Reference::CapHeight,
            HeightMeaning::Em => Reference::Em,
            HeightMeaning::Frame => Reference::Frame,
            HeightMeaning::Declared(key) => {
                match self.fonts.heights.get(key).copied().or(known(key)) {
                    Some(reference) => reference,
                    None => {
                        self.cx.report(
                            Diagnostic::new(codes::SCENE_TEXT_HEIGHT_ASSUMED)
                                .with("key", key)
                                .at(source.clone()),
                        );
                        Reference::CapHeight
                    }
                }
            }
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
        let outline = self.fonts.at(face).and_then(|f| f.outline(id));
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
        visit: &Visit<'_>,
        text: &Text<P>,
        font: Option<&str>,
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
        if vertical {
            // 縦書きの 1 行の文字は、置き方（決定 0176）が扱わない
            self.unplaced(&source);
            return Ok(());
        }
        let reference = self.reference(&text.height, &source);
        let mut laid_text = text.clone();
        laid_text.content = decoded(&text.content, &text.height, true);
        let fonts = self.fonts;
        let laid = match fonts.at(index) {
            Some(face) => layout::place(&laid_text, |p| p.xy(), face, |_| Some(reference))
                .map(|laid| (laid, layout::unit(reference, face), face.metrics())),
            None => return Ok(()),
        };
        let Ok((laid, unit_value, metrics)) = laid else {
            self.unplaced(&source);
            return Ok(());
        };
        if laid.missing > 0 {
            self.report(codes::SCENE_GLYPH_MISSING, &source);
        }
        self.segments(laid.pens.len(), &source)?;
        let mut glyphs = Vec::with_capacity(laid.pens.len());
        for (ch, pen) in laid_text.content.chars().zip(&laid.pens) {
            let id = fonts.at(index).and_then(|f| f.glyph_id(ch)).unwrap_or(0);
            let transform = compose(visit.transform.m, advanced(laid.transform, *pen));
            let shape = self.glyph_shape(index, id, transform, &source)?;
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
        visit: &Visit<'_>,
        text: &FormattedText<P>,
        font: Option<&str>,
        vertical: bool,
    ) -> Result<Option<LaidSpans>, (Stop, Ref)> {
        if self.fonts.is_empty() {
            return Ok(None);
        }
        let source = source(visit);
        let Some(base) = self.typeface(font, &source) else {
            return Ok(None);
        };
        // 区間で指定されたフォント。合うものがなければ既定で、既定もなければ文字スタイルのフォントで置く
        let mut faces = Vec::with_capacity(text.spans.len());
        for span in &text.spans {
            let face = match &span.format.font {
                Some(wanted) if matches!(span.piece, Piece::Text(_) | Piece::Stack { .. }) => {
                    match self.fonts.find(Some(&wanted.name)) {
                        Some((index, _, false)) => index,
                        found => {
                            self.cx.report(
                                Diagnostic::new(codes::SCENE_FONT_ASSUMED)
                                    .with("name", wanted.name.as_str())
                                    .at(source.clone()),
                            );
                            found.map_or(base, |(index, _, _)| index)
                        }
                    }
                }
                _ => base,
            };
            faces.push(face);
        }
        let reference = self.reference(&text.height, &source);
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
        let fonts = self.fonts;
        let Some(base_face) = fonts.at(base) else {
            return Ok(None);
        };
        let metrics = base_face.metrics();
        let span_fonts = SpanFonts {
            set: fonts,
            base: base_face,
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
            ratio: metrics.cap_height / layout::unit(reference, base_face),
        };
        for glyph in &laid.glyphs {
            let index = faces.get(glyph.span).copied().unwrap_or(base);
            let face = fonts.at(index);
            let mut id = face.and_then(|f| f.glyph_id(glyph.ch)).unwrap_or(0);
            if glyph.substituted {
                id = face.map_or(id, |f| f.vertical_id(id));
            }
            let transform = compose(m, glyph.transform);
            let shape = self.glyph_shape(index, id, transform, &source)?;
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
        item: &mut TextItem,
        source: &Ref,
    ) -> Result<(), (Stop, Ref)> {
        if self.fonts.is_empty() {
            return Ok(());
        }
        let Some(index) = self.typeface(font, source) else {
            return Ok(());
        };
        // 線種の文字の高さは、文字スタイルの高さに倍率を掛けた値で、TEXT の高さと同じ意味とみなす（推定）
        let declared = Height {
            value: Length::Paper(PaperLength(height)),
            meaning: HeightMeaning::Declared("dxf.TEXT.40"),
        };
        let reference = self.reference(&declared, source);
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
        let fonts = self.fonts;
        let laid = match fonts.at(index) {
            Some(face) => layout::place(&text, |p| *p, face, |_| Some(reference))
                .map(|laid| (laid, layout::unit(reference, face), face.metrics())),
            None => return Ok(()),
        };
        let Ok((laid, unit_value, metrics)) = laid else {
            self.report(codes::SCENE_TEXT_LAYOUT_UNSUPPORTED, source);
            return Ok(());
        };
        if laid.missing > 0 {
            self.report(codes::SCENE_GLYPH_MISSING, source);
        }
        self.segments(laid.pens.len(), source)?;
        let mut glyphs = Vec::with_capacity(laid.pens.len());
        for (ch, pen) in content.chars().zip(&laid.pens) {
            let id = fonts.at(index).and_then(|f| f.glyph_id(ch)).unwrap_or(0);
            let transform = advanced(laid.transform, *pen);
            let shape = self.glyph_shape(index, id, transform, source)?;
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
