//! 1 行の文字（[`Text`]）を、呼び出し側が渡したフォントの字形の寸法で置く試作（決定 0176）。
//!
//! フォントは同梱せず、[`Glyphs`] で寸法だけを受け取る（決定 0022）。結果は、字形の単位の座標を
//! 図面の座標へ写すアフィン変換と、文字ごとの送りの位置で返す。字形の輪郭は扱わない。
//!
//! 位置合わせの規則は、DXF の TEXT（72・73）の説明から推定し、AutoCAD 2027 の表示と比べて確かめた
//! （決定 0246）。Jw_cad の表示とはまだ比べていない。
//!
//! - 左・中央・右は、送りの合計の 0・半分・全部の位置。外形の中央（[`Horizontal::Middle`]）は、
//!   横は送りの合計の半分、縦は字形の外形の中央にする。
//! - 基準線・下・中央・上は、基準線・ディセンダーの線・大文字の高さの半分・大文字の高さの線。高さが
//!   [`Reference::CapitalA`] に当たるなら、大文字の高さに `A` の字形の外形の上を使う。
//! - 2 点の間は、送りの合計が 2 点の距離になるよう幅を変え、Aligned は高さも同じ比で変える。
//! - 斜体は字形にだけ当て、位置合わせは斜体を当てる前の寸法で決める。左右・上下の反転は、
//!   置く点（2 点の間では 2 点の中点）のまわりで当てる。
//!
//! 書式つきの文字（[`FormattedText`]）の折り返し・行送り・区間ごとの書式、DXF の特殊な符号、カーニングの
//! 扱いは決定 0181 にある。縦書き・上下に重ねた文字・禁則は決定 0187 にある。

use std::borrow::Cow;

use crate::{
    Fit, Flow, Font, Format, FormattedText, HeightMeaning, Horizontal, Length, Piece, Placement,
    Rotation, Scaled, StackKind, Text, Vertical,
};

/// フォント全体の寸法。値は字形の単位で、上を正とする
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontMetrics {
    /// em の大きさ
    pub units_per_em: f64,
    /// アセンダー（基準線から上の高さ）
    pub ascent: f64,
    /// ディセンダー（基準線から下の深さ。正の値）
    pub descent: f64,
    /// 大文字の高さ
    pub cap_height: f64,
}

/// 1 文字の字形の寸法。値は字形の単位
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    /// 送りの幅
    pub advance: f64,
    /// 外形（左・下・右・上）。空白などの外形のない字形は `None`
    pub bounds: Option<[f64; 4]>,
}

/// 呼び出し側が渡すフォント
pub trait Glyphs {
    /// フォント全体の寸法
    fn metrics(&self) -> FontMetrics;
    /// 文字の字形。フォントになければ `None`
    fn glyph(&self, ch: char) -> Option<Glyph>;
    /// フォントにない文字に使う字形（TrueType の `.notdef` など）
    fn fallback(&self) -> Glyph;
    /// 並んだ 2 文字の間に足す送り（字形の単位、負なら詰める）。既定は詰めない（決定 0181）
    fn kerning(&self, left: char, right: char) -> f64 {
        let _ = (left, right);
        0.0
    }
    /// 縦書きで並んだ 2 文字の間に足す縦の送り（字形の単位、負なら詰める）。既定は詰めない（決定 0189）
    fn vertical_kerning(&self, left: char, right: char) -> f64 {
        let _ = (left, right);
        0.0
    }
    /// 縦書きの字形。フォントになければ `None`。既定は横書きの字形を [`VerticalGlyph::upright`] で
    /// 置く（決定 0187）
    fn vertical(&self, ch: char) -> Option<VerticalGlyph> {
        self.glyph(ch)
            .map(|glyph| VerticalGlyph::upright(glyph, &self.metrics()))
    }
    /// フォントにない文字に使う、縦書きの字形。既定は [`Glyphs::fallback`] を
    /// [`VerticalGlyph::upright`] で置く
    fn vertical_fallback(&self) -> VerticalGlyph {
        VerticalGlyph::upright(self.fallback(), &self.metrics())
    }
}

/// 縦書きの 1 文字の字形の寸法。値は字形の単位（決定 0187）
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VerticalGlyph {
    /// 置く字形（縦書き用の字形に置き換えた後）の横の送りと外形
    pub glyph: Glyph,
    /// 縦の送り
    pub advance: f64,
    /// 縦の原点の高さ。縦の原点は、字形の座標で（横の送りの半分, `top`）にあり、列の中心線の上で
    /// 送りの始まりに合わせる
    pub top: f64,
    /// 縦書き用の字形に置き換えた。描く側も同じ置き換えを当てる
    pub substituted: bool,
}

impl VerticalGlyph {
    /// 縦書きの寸法を持たないフォントの既定（推定）。縦の送りはアセンダーとディセンダーの和、
    /// 縦の原点はアセンダーの高さにし、置き換えない
    pub fn upright(glyph: Glyph, metrics: &FontMetrics) -> Self {
        Self {
            glyph,
            advance: metrics.ascent + metrics.descent,
            top: metrics.ascent,
            substituted: false,
        }
    }
}

/// 宣言された高さが、フォントのどの寸法に当たるか
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reference {
    /// 大文字の高さ
    CapHeight,
    /// em
    Em,
    /// アセンダーとディセンダーを合わせた高さ
    Frame,
    /// `A` の字形の外形の上。`A` がないか外形を持たなければ、大文字の高さ。AutoCAD の TrueType の
    /// TEXT の高さ（決定 0246）
    CapitalA,
}

/// 置いた結果
#[derive(Clone, Debug, PartialEq)]
pub struct Laid {
    /// 字形の単位の座標（基準線の始まりが原点）を、図面の座標へ写す変換 `[a, b, c, d, e, f]`。
    /// `X = a·u + c·v + e`、`Y = b·u + d·v + f`
    pub transform: [f64; 6],
    /// 文字ごとの送りの始まり（字形の単位）。内容の文字の順
    pub pens: Vec<f64>,
    /// 送りの合計（字形の単位）
    pub advance: f64,
    /// 字形の単位から図面の長さへの倍率（横・縦）。反転は含まない
    pub scale: [f64; 2],
    /// フォントになく、代わりの字形で置いた文字の数
    pub missing: usize,
}

/// 置けない理由
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unplaced {
    /// 宣言された高さが、フォントのどの寸法に当たるか決まらない。形式と欄を指すキー
    Height(Cow<'static, str>),
    /// 高さ・幅の倍率・字の間隔・行送り・枠の幅・向き・フォントの寸法・2 点の距離・送りの合計が 0 か
    /// 有限でないか、斜体の角度が 90 度以上
    Degenerate,
    /// 縦書き。決定 0181 で使い、決定 0187 で縦書きを置くようにしてからは返さない
    Vertical,
}

/// 文字を置く。`xy` は点を図面の座標にし、`declared` は形式の宣言値の高さの意味を決める
pub fn place<P>(
    text: &Text<P>,
    xy: impl Fn(&P) -> [f64; 2],
    font: &impl Glyphs,
    declared: impl Fn(&str) -> Option<Reference>,
) -> Result<Laid, Unplaced> {
    let metrics = font.metrics();
    let reference = reference(&text.height.meaning, declared)?;
    let scale = length(&text.height.value) / unit(reference, font);
    let cap = match reference {
        Reference::CapitalA => unit(reference, font),
        _ => metrics.cap_height,
    };
    if !positive(scale) || !positive(text.width_factor) || !finite(&metrics) {
        return Err(Unplaced::Degenerate);
    }

    let (pens, advance, ink, missing) = run(&text.content, font);
    let mut sx = scale * text.width_factor;
    let mut sy = scale;

    let (point, anchor, (sin, cos)) = match &text.placement {
        Placement::Anchor {
            point,
            horizontal,
            vertical,
        } => {
            let width = advance * sx;
            let ink_middle = ink.map(|[_, b, _, t]| (b + t) / 2.0 * sy);
            let x = match *horizontal {
                Horizontal::Left => 0.0,
                Horizontal::Center | Horizontal::Middle => width / 2.0,
                Horizontal::Right => width,
            };
            let y = match (*horizontal, *vertical) {
                (Horizontal::Middle, _) => ink_middle.unwrap_or(cap / 2.0 * sy),
                (_, Vertical::Baseline) => 0.0,
                (_, Vertical::Bottom) => -metrics.descent * sy,
                (_, Vertical::Middle) => cap / 2.0 * sy,
                (_, Vertical::Top) => cap * sy,
            };
            let radians = text.rotation.to_radians();
            (xy(point), [x, y], (libm::sin(radians), libm::cos(radians)))
        }
        Placement::Between { first, second, fit } => {
            let [x0, y0] = xy(first);
            let [x1, y1] = xy(second);
            let distance = libm::hypot(x1 - x0, y1 - y0);
            let width = advance * sx;
            if !positive(distance) || !positive(width) {
                return Err(Unplaced::Degenerate);
            }
            let ratio = distance / width;
            sx *= ratio;
            if *fit == Fit::Aligned {
                sy *= ratio;
            }
            let direction = ((y1 - y0) / distance, (x1 - x0) / distance);
            // 反転を 2 点の中点のまわりで当て、2 点の間に収める
            let middle = [(x0 + x1) / 2.0, (y0 + y1) / 2.0];
            (middle, [distance / 2.0, 0.0], direction)
        }
    };

    // 斜体は 90 度より小さい角度に限る
    if !text.oblique.is_finite() || text.oblique.abs() >= 90.0 {
        return Err(Unplaced::Degenerate);
    }
    let shear = libm::tan(text.oblique.to_radians());
    let mx = if text.backward { -1.0 } else { 1.0 };
    let my = if text.upside_down { -1.0 } else { 1.0 };
    let [ax, ay] = anchor;
    let transform = [
        cos * mx * sx,
        sin * mx * sx,
        cos * mx * sy * shear - sin * my * sy,
        sin * mx * sy * shear + cos * my * sy,
        -cos * mx * ax + sin * my * ay + point[0],
        -sin * mx * ax - cos * my * ay + point[1],
    ];
    Ok(Laid {
        transform,
        pens,
        advance,
        scale: [sx, sy],
        missing,
    })
}

/// 文字を左から並べ、送りの始まり・送りの合計・外形・フォントにない文字の数を返す
fn run(content: &str, font: &impl Glyphs) -> (Vec<f64>, f64, Option<[f64; 4]>, usize) {
    let mut pens = Vec::new();
    let mut pen = 0.0;
    let mut ink: Option<[f64; 4]> = None;
    let mut missing = 0;
    let mut previous = None;
    for ch in content.chars() {
        let glyph = font.glyph(ch).unwrap_or_else(|| {
            missing += 1;
            font.fallback()
        });
        if let Some(left) = previous {
            pen += font.kerning(left, ch);
        }
        previous = Some(ch);
        pens.push(pen);
        if let Some([l, b, r, t]) = glyph.bounds {
            let [l, r] = [pen + l, pen + r];
            ink = Some(match ink {
                Some([l0, b0, r0, t0]) => [l0.min(l), b0.min(b), r0.max(r), t0.max(t)],
                None => [l, b, r, t],
            });
        }
        pen += glyph.advance;
    }
    (pens, pen, ink, missing)
}

/// 書式つきの文字を置くのに、呼び出し側が渡すフォント（決定 0181）
pub trait Fonts {
    /// 文字スタイルのフォント
    fn base(&self) -> &dyn Glyphs;
    /// 区間で指定されたフォント。渡せなければ `None` を返し、文字スタイルのフォントで置く
    fn font(&self, font: &Font) -> Option<&dyn Glyphs>;
    /// 文字スタイルが縦書きか。書き進める向きが [`Flow::ByStyle`] のときに使う
    fn vertical(&self) -> bool;
}

/// 書式つきの文字を置いた結果
#[derive(Clone, Debug, PartialEq)]
pub struct LaidFormatted {
    /// 置いた字形。内容の順で、上下に重ねた文字は上・下の順
    pub glyphs: Vec<PlacedGlyph>,
    /// 行。横書きでは上から、縦書きでは右から順
    pub lines: Vec<LineBox>,
    /// 横書きでは枠の幅と内容の高さ、縦書きでは内容の幅と枠の長さ（図面の長さ）
    pub size: [f64; 2],
    /// フォントになく、代わりの字形で置いた文字の数
    pub missing: usize,
    /// 区間で指定されたフォントが渡されず、文字スタイルのフォントで置いた区間の数
    pub substituted: usize,
    /// 置かなかった区間の位置（測定値・解釈できない符号など）。入力の順
    pub skipped: Vec<usize>,
    /// 置いた上下に重ねた文字。入力の順（決定 0187）
    pub stacks: Vec<PlacedStack>,
}

/// 置いた 1 文字
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedGlyph {
    /// 文字が属する区間の位置
    pub span: usize,
    /// 文字
    pub ch: char,
    /// 字形の単位の座標（字形の基準線の始まりが原点）を、図面の座標へ写す変換。形は [`Laid::transform`] と同じ
    pub transform: [f64; 6],
    /// 縦書き用の字形に置き換えた（[`VerticalGlyph::substituted`]）
    pub substituted: bool,
}

/// 置いた上下に重ねた文字
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedStack {
    /// 区間の位置
    pub span: usize,
    /// 上下を分ける横線か斜線の両端（図面の座標）。公差は線を引かないので `None`
    pub line: Option<[[f64; 2]; 2]>,
}

/// 行。値は図面の長さ。横書きでは枠の左上から測る。縦書きでは、`baseline` は内容の右から列の中心線まで、
/// `offset` は枠の上から列の始まりまで、`width` は列の長さ
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineBox {
    /// 行の字形の範囲（[`LaidFormatted::glyphs`] の位置、始まりと終わり）
    pub glyphs: [usize; 2],
    /// 枠の上から基準線までの深さ
    pub baseline: f64,
    /// 枠の左から行の始まりまで
    pub offset: f64,
    /// 行の幅。行の終わりの空白は含めない
    pub width: f64,
}

/// 既定の行送りの、文字の高さへの倍率（AutoCAD のヘルプの「1 行の行送りは文字の高さの 1.66 倍」から推定）
const LINE_PITCH: f64 = 5.0 / 3.0;

/// 上下に重ねた文字の、上下の段の高さの倍率（推定。AutoCAD のヘルプの重ねた文字の既定の大きさ 70% から）
const STACK_SCALE: f64 = 0.7;
/// 横線・公差で、中心の線から上下の段までの隙間の、区間の大文字の高さへの倍率（推定）
const STACK_GAP: f64 = 0.1;
/// 斜線の横の幅の、区間の大文字の高さへの倍率（推定）
const STACK_SLANT: f64 = 0.5;

/// 書式つきの文字を置く。枠の幅で折り返し、行送りと区間ごとの書式（高さ・幅の倍率・字の間隔・斜体・
/// フォント・行の中の位置合わせ）を当てる。DXF の特殊な符号は解かないので、呼び出し側が [`decode`] で
/// 解いてから渡す（決定 0181）。縦書き・上下に重ねた文字・禁則は決定 0187 による
pub fn place_formatted<P>(
    text: &FormattedText<P>,
    xy: impl Fn(&P) -> [f64; 2],
    fonts: &impl Fonts,
    declared: impl Fn(&str) -> Option<Reference>,
) -> Result<LaidFormatted, Unplaced> {
    let vertical = match text.flow {
        Flow::LeftToRight => false,
        Flow::TopToBottom => true,
        Flow::ByStyle => fonts.vertical(),
    };
    let reference = reference(&text.height.meaning, declared)?;
    let height = length(&text.height.value);
    let base = fonts.base().metrics();
    let base_scale = height / unit(reference, fonts.base());
    let spacing = text.line_spacing.factor;
    if !positive(base_scale) || !finite(&base) || !positive(spacing) {
        return Err(Unplaced::Degenerate);
    }
    let limit = match &text.width {
        Some(width) => {
            let width = length(width);
            if !width.is_finite() {
                return Err(Unplaced::Degenerate);
            }
            (width > 0.0).then_some(width)
        }
        None => None,
    };
    let (sin, cos) = match text.rotation {
        Rotation::Angle(degrees) if degrees.is_finite() => {
            let radians = degrees.to_radians();
            (libm::sin(radians), libm::cos(radians))
        }
        Rotation::Direction([x, y]) => {
            let norm = libm::hypot(x, y);
            if !positive(norm) {
                return Err(Unplaced::Degenerate);
            }
            (y / norm, x / norm)
        }
        Rotation::Angle(_) => return Err(Unplaced::Degenerate),
    };

    let mut out = LaidFormatted {
        glyphs: Vec::new(),
        lines: Vec::new(),
        size: [0.0, 0.0],
        missing: 0,
        substituted: 0,
        skipped: Vec::new(),
        stacks: Vec::new(),
    };
    let mut tokens = Vec::new();
    for (index, span) in text.spans.iter().enumerate() {
        let font = || match &span.format.font {
            Some(wanted) => fonts.font(wanted).ok_or(()),
            None => Ok(fonts.base()),
        };
        match &span.piece {
            Piece::Text(content) => {
                let font = font().unwrap_or_else(|()| {
                    out.substituted += 1;
                    fonts.base()
                });
                let run = SpanRun::new(&span.format, font, reference, height)?;
                let mut previous = None;
                for ch in content.chars() {
                    let item = if vertical {
                        run.upright(index, ch, previous, font, &mut out.missing)
                    } else {
                        run.item(index, ch, previous, font, &mut out.missing)
                    };
                    previous = Some(ch);
                    tokens.push(Some(item));
                }
            }
            // 段組みは移さない（決定 0109）ので、段の区切りは段落の区切りとして置く
            Piece::Paragraph | Piece::Column => tokens.push(None),
            Piece::Stack { upper, lower, kind } => {
                let font = font().unwrap_or_else(|()| {
                    out.substituted += 1;
                    fonts.base()
                });
                let run = SpanRun::new(&span.format, font, reference, height)?;
                let mut item = run.stack(
                    index,
                    [upper.as_str(), lower.as_str()],
                    *kind,
                    font,
                    &mut out.missing,
                );
                // 縦書きでは立てたまま置き、上の段の上から下の段の下までを縦の送りにする（決定 0189）
                if vertical {
                    item.advance = item.rise + item.drop;
                }
                tokens.push(Some(item));
            }
            Piece::Measurement | Piece::Below | Piece::Unparsed(_) => {
                out.skipped.push(index);
            }
        }
    }

    let lines = wrap(tokens, limit);
    let frame = Frame {
        point: xy(&text.point),
        sin,
        cos,
    };
    if vertical {
        columns(text, &lines, &base, base_scale, limit, &frame, &mut out);
    } else {
        rows(text, &lines, &base, base_scale, limit, &frame, &mut out);
    }
    Ok(out)
}

/// 置く点と向き。枠の座標（右・上を正）を図面の座標へ写す
struct Frame {
    point: [f64; 2],
    sin: f64,
    cos: f64,
}

impl Frame {
    fn map(&self, [x, y]: [f64; 2]) -> [f64; 2] {
        [
            self.point[0] + self.cos * x - self.sin * y,
            self.point[1] + self.sin * x + self.cos * y,
        ]
    }

    /// 字形の基準線の始まりを枠の座標 `at` に置き、倍率 `sx`・`sy` と斜体を当てる変換
    fn glyph(&self, at: [f64; 2], sx: f64, sy: f64, shear: f64) -> [f64; 6] {
        let [e, f] = self.map(at);
        let (sin, cos) = (self.sin, self.cos);
        [
            cos * sx,
            sin * sx,
            cos * sy * shear - sin * sy,
            sin * sy * shear + cos * sy,
            e,
            f,
        ]
    }

    /// 縦書きで時計回りに 90 度倒した字形の変換（決定 0189）。字形の横の向きを列の下へ、縦の向きを
    /// 右へ写す。字形の u は枠の (0, -sx)、v は枠の (sy, -shear·sy) になる
    fn turned(&self, at: [f64; 2], sx: f64, sy: f64, shear: f64) -> [f64; 6] {
        let [e, f] = self.map(at);
        let (sin, cos) = (self.sin, self.cos);
        [
            sin * sx,
            -cos * sx,
            cos * sy + sin * sy * shear,
            sin * sy - cos * sy * shear,
            e,
            f,
        ]
    }

    /// 上下に重ねた文字を、送りの始まりと基準線を枠の座標 `[x, y]` に置いて足す
    fn stack(&self, item: &Item, stack: &Stack, [x, y]: [f64; 2], out: &mut LaidFormatted) {
        let run = &item.run;
        for &(ch, [gx, gy]) in &stack.glyphs {
            out.glyphs.push(PlacedGlyph {
                span: item.span,
                ch,
                transform: self.glyph(
                    [x + gx, y + gy],
                    run.sx * STACK_SCALE,
                    run.sy * STACK_SCALE,
                    run.shear,
                ),
                substituted: false,
            });
        }
        out.stacks.push(PlacedStack {
            span: item.span,
            line: stack
                .line
                .map(|ends| ends.map(|[lx, ly]| self.map([x + lx, y + ly]))),
        });
    }
}

/// 行の送りの増分。2 行目からは、文字の高さ（固定でなければ行でいちばん高い字の高さ）の行送り
fn pitch(text: &FormattedText<impl Sized>, height: f64, line: &[Item]) -> f64 {
    let spacing = text.line_spacing.factor;
    if text.line_spacing.exact {
        LINE_PITCH * spacing * height
    } else {
        let tallest = line
            .iter()
            .map(|item| item.run.height)
            .fold(height, f64::max);
        LINE_PITCH * spacing * tallest
    }
}

/// 横書きの行を置く（決定 0181）
fn rows<P>(
    text: &FormattedText<P>,
    lines: &[Vec<Item>],
    base: &FontMetrics,
    base_scale: f64,
    limit: Option<f64>,
    frame: &Frame,
    out: &mut LaidFormatted,
) {
    let height = length(&text.height.value);
    let base_cap = base.cap_height * base_scale;
    let base_descent = base.descent * base_scale;
    let mut depth = 0.0;
    let mut descent = base_descent;
    let mut boxes = Vec::new();
    for (number, line) in lines.iter().enumerate() {
        let cap = line.iter().map(|item| item.rise).fold(base_cap, f64::max);
        descent = line
            .iter()
            .map(|item| item.drop)
            .fold(base_descent, f64::max);
        depth += if number == 0 {
            cap
        } else {
            pitch(text, height, line)
        };
        boxes.push((depth, cap, measure(line, limit)));
    }
    let width = limit.unwrap_or_else(|| boxes.iter().map(|b| b.2).fold(0.0, f64::max));
    let total = depth + descent;
    let left = match text.horizontal {
        Horizontal::Left => 0.0,
        Horizontal::Center | Horizontal::Middle => -width / 2.0,
        Horizontal::Right => -width,
    };
    let top = match text.vertical {
        Vertical::Top => 0.0,
        Vertical::Middle => total / 2.0,
        Vertical::Bottom | Vertical::Baseline => total,
    };
    for (line, (depth, cap, line_width)) in lines.iter().zip(boxes) {
        let offset = match text.horizontal {
            Horizontal::Left => 0.0,
            Horizontal::Center | Horizontal::Middle => (width - line_width) / 2.0,
            Horizontal::Right => width - line_width,
        };
        let start = out.glyphs.len();
        let mut pen = 0.0;
        for (position, item) in line.iter().enumerate() {
            if position > 0 {
                pen += item.kern;
            }
            let run = &item.run;
            let shift = match run.align {
                Vertical::Middle => (cap - item.rise) / 2.0,
                Vertical::Top => cap - item.rise,
                Vertical::Baseline | Vertical::Bottom => 0.0,
            };
            let x = left + offset + pen;
            let y = top - depth + shift;
            match &item.content {
                Content::Char { ch, .. } => out.glyphs.push(PlacedGlyph {
                    span: item.span,
                    ch: *ch,
                    transform: frame.glyph([x, y], run.sx, run.sy, run.shear),
                    substituted: false,
                }),
                Content::Stack(stack) => frame.stack(item, stack, [x, y], out),
            }
            pen += item.advance;
        }
        out.lines.push(LineBox {
            glyphs: [start, out.glyphs.len()],
            baseline: depth,
            offset,
            width: line_width,
        });
    }
    out.size = [width, total];
}

/// 縦書きの列を置く（決定 0187）。列は上から下へ書き進め、右から左へ並べる
fn columns<P>(
    text: &FormattedText<P>,
    lines: &[Vec<Item>],
    base: &FontMetrics,
    base_scale: f64,
    limit: Option<f64>,
    frame: &Frame,
    out: &mut LaidFormatted,
) {
    let height = length(&text.height.value);
    let base_em = base.units_per_em * base_scale;
    let mut center = 0.0;
    let mut half = base_em / 2.0;
    let mut boxes = Vec::new();
    for (number, line) in lines.iter().enumerate() {
        // 列の太さは、列でいちばん幅の広い em
        let thick = line.iter().map(|item| item.run.em).fold(base_em, f64::max);
        half = thick / 2.0;
        center += if number == 0 {
            half
        } else {
            pitch(text, height, line)
        };
        boxes.push((center, measure(line, limit)));
    }
    let total = center + half;
    let span = limit.unwrap_or_else(|| boxes.iter().map(|b| b.1).fold(0.0, f64::max));
    let left = match text.horizontal {
        Horizontal::Left => 0.0,
        Horizontal::Center | Horizontal::Middle => -total / 2.0,
        Horizontal::Right => -total,
    };
    let top = match text.vertical {
        Vertical::Top => 0.0,
        Vertical::Middle => span / 2.0,
        Vertical::Bottom | Vertical::Baseline => span,
    };
    for (line, (center, length)) in lines.iter().zip(boxes) {
        let offset = match text.vertical {
            Vertical::Top => 0.0,
            Vertical::Middle => (span - length) / 2.0,
            Vertical::Bottom | Vertical::Baseline => span - length,
        };
        let x = left + total - center;
        let start = out.glyphs.len();
        let mut pen = 0.0;
        for (position, item) in line.iter().enumerate() {
            if position > 0 {
                pen += item.kern;
            }
            let run = &item.run;
            match &item.content {
                &Content::Char {
                    ch,
                    center,
                    top: rise,
                    substituted,
                    turned,
                } => {
                    // 縦の原点（横の送りの半分, 縦の原点の高さ）を、列の中心線の送りの位置に合わせる。
                    // 倒した字形は、基準線を中心線から `center` だけ左に置く
                    let at = [x - center, top - offset - pen - rise];
                    let transform = if turned {
                        frame.turned(at, run.sx, run.sy, run.shear)
                    } else {
                        frame.glyph(at, run.sx, run.sy, run.shear)
                    };
                    out.glyphs.push(PlacedGlyph {
                        span: item.span,
                        ch,
                        transform,
                        substituted,
                    });
                }
                // 上下に重ねた文字は立てたまま、列の中心に揃え、上の段の上を送りの始まりに合わせる
                Content::Stack(stack) => frame.stack(
                    item,
                    stack,
                    [x - stack.width / 2.0, top - offset - pen - item.rise],
                    out,
                ),
            }
            pen += item.advance;
        }
        out.lines.push(LineBox {
            glyphs: [start, out.glyphs.len()],
            baseline: center,
            offset,
            width: length,
        });
    }
    out.size = [total, span];
}

/// 区間の書式を当てた、字形の単位から図面の長さへの倍率と、行を決める寸法
#[derive(Clone, Copy, Debug)]
struct SpanRun {
    sx: f64,
    sy: f64,
    shear: f64,
    tracking: f64,
    /// 宣言された高さ（図面の長さ）
    height: f64,
    /// 大文字の高さ（図面の長さ）
    cap: f64,
    /// ディセンダー（図面の長さ）
    descent: f64,
    /// em の横の長さ（図面の長さ）。縦書きの列の太さに使う
    em: f64,
    align: Vertical,
}

impl SpanRun {
    fn new(
        format: &Format,
        font: &dyn Glyphs,
        reference: Reference,
        height: f64,
    ) -> Result<Self, Unplaced> {
        let metrics = font.metrics();
        let height = match format.height {
            Some(Scaled::Value(value)) => length(&value),
            Some(Scaled::Factor(factor)) => height * factor,
            None => height,
        };
        let scaled = |value: Option<Scaled<f64>>| match value {
            Some(Scaled::Value(value)) => value,
            Some(Scaled::Factor(factor)) => factor,
            None => 1.0,
        };
        let width = scaled(format.width_factor);
        let tracking = scaled(format.tracking);
        let oblique = format.oblique.unwrap_or(0.0);
        let sy = height / unit(reference, font);
        if !positive(sy)
            || !finite(&metrics)
            || !positive(width)
            || !positive(tracking)
            || !oblique.is_finite()
            || oblique.abs() >= 90.0
        {
            return Err(Unplaced::Degenerate);
        }
        Ok(Self {
            sx: sy * width,
            sy,
            shear: libm::tan(oblique.to_radians()),
            tracking,
            height,
            cap: metrics.cap_height * sy,
            descent: metrics.descent * sy,
            em: metrics.units_per_em * sy * width,
            align: format.align.unwrap_or(Vertical::Baseline),
        })
    }

    /// 横書きの 1 文字
    fn item(
        self,
        span: usize,
        ch: char,
        previous: Option<char>,
        font: &dyn Glyphs,
        missing: &mut usize,
    ) -> Item {
        let glyph = font.glyph(ch).unwrap_or_else(|| {
            *missing += 1;
            font.fallback()
        });
        Item {
            span,
            content: Content::Char {
                ch,
                center: 0.0,
                top: 0.0,
                substituted: false,
                turned: false,
            },
            advance: glyph.advance * self.sx * self.tracking,
            kern: previous.map_or(0.0, |left| font.kerning(left, ch) * self.sx),
            space: ch.is_whitespace(),
            rise: self.cap,
            drop: self.descent,
            run: self,
        }
    }

    /// 縦書きの 1 文字。立てて置く文字の間は縦書きのカーニングで詰める。横に倒す文字（[`sideways`]）は
    /// [`SpanRun::turned`] で置く
    fn upright(
        self,
        span: usize,
        ch: char,
        previous: Option<char>,
        font: &dyn Glyphs,
        missing: &mut usize,
    ) -> Item {
        if sideways(ch) {
            return self.turned(span, ch, previous, font, missing);
        }
        let glyph = font.vertical(ch).unwrap_or_else(|| {
            *missing += 1;
            font.vertical_fallback()
        });
        Item {
            span,
            content: Content::Char {
                ch,
                center: glyph.glyph.advance / 2.0 * self.sx,
                top: glyph.top * self.sy,
                substituted: glyph.substituted,
                turned: false,
            },
            advance: glyph.advance * self.sy * self.tracking,
            kern: previous
                .filter(|&left| !sideways(left))
                .map_or(0.0, |left| font.vertical_kerning(left, ch) * self.sy),
            space: ch.is_whitespace(),
            rise: self.cap,
            drop: self.descent,
            run: self,
        }
    }

    /// 縦書きで時計回りに 90 度倒す 1 文字（決定 0189、推定）。横書きの字形を、横の送りで列の下へ進め、
    /// アセンダーとディセンダーの中央を列の中心線に合わせる。倒した文字の間は横書きのカーニングで詰める
    fn turned(
        self,
        span: usize,
        ch: char,
        previous: Option<char>,
        font: &dyn Glyphs,
        missing: &mut usize,
    ) -> Item {
        let glyph = font.glyph(ch).unwrap_or_else(|| {
            *missing += 1;
            font.fallback()
        });
        let metrics = font.metrics();
        Item {
            span,
            content: Content::Char {
                ch,
                center: (metrics.ascent - metrics.descent) / 2.0 * self.sy,
                top: 0.0,
                substituted: false,
                turned: true,
            },
            advance: glyph.advance * self.sx * self.tracking,
            kern: previous
                .filter(|&left| sideways(left))
                .map_or(0.0, |left| font.kerning(left, ch) * self.sx),
            space: ch.is_whitespace(),
            rise: self.cap,
            drop: self.descent,
            run: self,
        }
    }

    /// 上下に重ねた文字（推定）。上下の段は区間の大きさの [`STACK_SCALE`] 倍で置く。
    /// 横線と公差は、上下の段を区間の大文字の高さの半分の線から [`STACK_GAP`] だけ離して上下に置き、
    /// 横線は短い段を中央に揃えて段の幅の線を引き、公差は左に揃えて線を引かない。斜線は上の段の上を
    /// 大文字の高さに、下の段を基準線に置き、間に斜線を引く
    fn stack(
        self,
        span: usize,
        [upper, lower]: [&str; 2],
        kind: StackKind,
        font: &dyn Glyphs,
        missing: &mut usize,
    ) -> Item {
        let sx = self.sx * STACK_SCALE;
        let part = |content: &str, missing: &mut usize| {
            let mut pen = 0.0;
            let mut glyphs = Vec::new();
            let mut previous = None;
            for ch in content.chars() {
                if let Some(left) = previous {
                    pen += font.kerning(left, ch) * sx;
                }
                previous = Some(ch);
                glyphs.push((ch, pen));
                let glyph = font.glyph(ch).unwrap_or_else(|| {
                    *missing += 1;
                    font.fallback()
                });
                pen += glyph.advance * sx * self.tracking;
            }
            (glyphs, pen)
        };
        let (top, top_width) = part(upper, missing);
        let (bottom, bottom_width) = part(lower, missing);
        let cap = self.cap;
        let part_cap = cap * STACK_SCALE;
        let part_descent = self.descent * STACK_SCALE;
        let (width, [top_x, bottom_x], [top_y, bottom_y], line) = match kind {
            StackKind::Horizontal | StackKind::Tolerance => {
                let width = top_width.max(bottom_width);
                let axis = cap / 2.0;
                let gap = cap * STACK_GAP;
                let (xs, line) = if kind == StackKind::Horizontal {
                    let xs = [(width - top_width) / 2.0, (width - bottom_width) / 2.0];
                    (xs, Some([[0.0, axis], [width, axis]]))
                } else {
                    ([0.0, 0.0], None)
                };
                (width, xs, [axis + gap, axis - gap - part_cap], line)
            }
            StackKind::Diagonal => {
                let slant = cap * STACK_SLANT;
                let line = [[top_width, 0.0], [top_width + slant, cap]];
                let width = top_width + slant + bottom_width;
                (
                    width,
                    [0.0, top_width + slant],
                    [cap - part_cap, 0.0],
                    Some(line),
                )
            }
        };
        let glyphs = top
            .into_iter()
            .map(|(ch, x)| (ch, [top_x + x, top_y]))
            .chain(
                bottom
                    .into_iter()
                    .map(|(ch, x)| (ch, [bottom_x + x, bottom_y])),
            )
            .collect();
        Item {
            span,
            content: Content::Stack(Box::new(Stack {
                glyphs,
                line,
                width,
            })),
            advance: width,
            kern: 0.0,
            space: false,
            rise: (top_y + part_cap).max(cap),
            drop: (part_descent - bottom_y).max(self.descent),
            run: self,
        }
    }
}

/// 並べる 1 文字か、上下に重ねた文字。送りと詰めは図面の長さ
#[derive(Clone, Debug)]
struct Item {
    span: usize,
    content: Content,
    /// 送り。縦書きでは縦の送り
    advance: f64,
    /// 前の文字との間に足す送り。行の始まりでは使わない
    kern: f64,
    space: bool,
    /// 基準線から上の高さ。行でいちばん高いものを、行の大文字の高さにする
    rise: f64,
    /// 基準線から下の深さ。最後の行でいちばん深いものを、内容の下にする
    drop: f64,
    run: SpanRun,
}

/// 並べるものの中身
#[derive(Clone, Debug)]
enum Content {
    /// 1 文字。縦書きでは、縦の原点の横の位置（`center`）と高さ（`top`）を図面の長さで持つ。
    /// `turned` は縦書きで横に倒した文字で、`center` は基準線から列の中心線までの距離
    Char {
        ch: char,
        center: f64,
        top: f64,
        substituted: bool,
        turned: bool,
    },
    /// 上下に重ねた文字
    Stack(Box<Stack>),
}

/// 上下に重ねた文字の字形と線。位置は、重ねた文字の送りの始まりと基準線を原点にした図面の長さ
#[derive(Clone, Debug)]
struct Stack {
    glyphs: Vec<(char, [f64; 2])>,
    line: Option<[[f64; 2]; 2]>,
    /// 横の幅。縦書きで列の中心に揃えるのに使う
    width: f64,
}

impl Item {
    /// 禁則の判断に使う文字。上下に重ねた文字は、英数字と同じく扱う
    fn ch(&self) -> char {
        match self.content {
            Content::Char { ch, .. } => ch,
            Content::Stack(_) => '0',
        }
    }
}

/// 行の始まりから `count` 文字の幅
fn extent(count: usize, line: &[Item]) -> f64 {
    line.iter()
        .take(count)
        .enumerate()
        .map(|(position, item)| {
            if position > 0 {
                item.kern + item.advance
            } else {
                item.advance
            }
        })
        .sum()
}

/// 文字を行に分ける。`None` は段落の区切り。`limit` があれば、その幅を超える前で折り返す。折り返す所は、
/// 空白の後と、全角の文字の前後に限り、禁則（[`breakable`]）を守る。なければ文字の間で切る。
/// 空白は行の終わりにぶら下げる
fn wrap(tokens: Vec<Option<Item>>, limit: Option<f64>) -> Vec<Vec<Item>> {
    let mut lines: Vec<Vec<Item>> = vec![Vec::new()];
    for token in tokens {
        let Some(item) = token else {
            lines.push(Vec::new());
            continue;
        };
        let line = match lines.last_mut() {
            Some(line) => line,
            None => continue,
        };
        let over = limit.is_some_and(|limit| {
            let limit = limit * (1.0 + 1e-9);
            let before = extent(line.len(), line);
            // 句読点は、前までが収まれば行の終わりにぶら下げる（決定 0189）
            !item.space
                && !line.is_empty()
                && before + item.kern + item.advance > limit
                && !(hangs(item.ch()) && before <= limit)
        });
        if over {
            let at = (1..=line.len())
                .rev()
                .find(|&at| {
                    let next = line.get(at).unwrap_or(&item);
                    line.get(at - 1)
                        .is_some_and(|before| breakable(before, next))
                })
                .unwrap_or(line.len());
            let mut tail = line.split_off(at);
            tail.push(item);
            lines.push(tail);
        } else {
            line.push(item);
        }
    }
    lines
}

/// 行の長さ。終わりの空白と、枠からはみ出してぶら下げた句読点は数えない
fn measure(line: &[Item], limit: Option<f64>) -> f64 {
    let mut count = line.iter().rev().skip_while(|item| item.space).count();
    let hanging = count
        .checked_sub(1)
        .and_then(|last| line.get(last))
        .is_some_and(|item| hangs(item.ch()));
    if hanging && limit.is_some_and(|limit| extent(count, line) > limit * (1.0 + 1e-9)) {
        count -= 1;
    }
    extent(count, line)
}

/// 行の終わりにぶら下げる句読点（決定 0189）。読点・句点（半角形は互換分解の先に従う）と、全角のコンマ・
/// ピリオド
fn hangs(ch: char) -> bool {
    matches!(ch, '\u{FF0C}' | '\u{FF0E}') || matches!(narrow(ch), '\u{3001}' | '\u{3002}')
}

/// 縦書きで横に倒して置く文字（決定 0189、推定）。全角の文字（[`wide`]）と半角形（U+FF61〜U+FFEF。
/// 半角の片仮名など）は立てて置き、それ以外（欧文・数字・半角の記号など）は倒す
fn sideways(ch: char) -> bool {
    !wide(ch) && !matches!(u32::from(ch), 0xFF61..=0xFFEF)
}

/// 2 文字の間で折り返せるか。行の始まりに置かない文字の前と、行の終わりに置かない文字の後では
/// 折り返さない（禁則、決定 0187）
fn breakable(before: &Item, after: &Item) -> bool {
    let (left, right) = (before.ch(), after.ch());
    !after.space && (before.space || wide(left) || wide(right)) && !no_start(right) && !no_end(left)
}

/// 行の始まりに置かない文字（禁則、決定 0187）。閉じ括弧（Unicode の一般カテゴリ Pe）と、
/// 句読点・感嘆符・疑問符・コロン・セミコロン。全角形・半角形は、互換分解の先の文字に従う。
/// 小書きの仮名（[`small_kana`]）と長音の記号（U+30FC と、その半角形 U+FF70）も置かない（決定 0189）
fn no_start(ch: char) -> bool {
    if small_kana(ch) || matches!(ch, '\u{30FC}' | '\u{FF70}') {
        return true;
    }
    match narrow(ch) {
        ')' | ']' | '}' | '!' | ',' | '.' | ':' | ';' | '?' => true,
        // 読点・句点
        '\u{3001}' | '\u{3002}' => true,
        // 〈〉《》「」『』【】と〔〕〖〗〘〙〚〛は、開きと閉じが交互に並び、奇数が閉じ
        ch @ ('\u{3008}'..='\u{3011}' | '\u{3014}'..='\u{301B}') => u32::from(ch) % 2 == 1,
        // 〞〟は閉じ、白の二重括弧の閉じ
        '\u{301E}' | '\u{301F}' | '\u{FF60}' => true,
        _ => false,
    }
}

/// 小書きの仮名（決定 0189）。Unicode の名前に SMALL を持つ平仮名・片仮名で、次の規則で選ぶ。
///
/// - 平仮名（U+3041〜U+3096）で小書きの仮名は、すぐ後ろの符号位置に同じ音の普通の仮名がある文字
///   （ぁ U+3041 と あ U+3042 など）と、区画の終わりに足された ゕ・ゖ（U+3095・U+3096）
/// - 片仮名（U+30A1〜U+30F6）は、平仮名と同じ並びで 0x60 後ろにあるので、平仮名に戻して同じ規則で選ぶ
/// - 片仮名の拡張（U+31F0〜U+31FF）はすべて小書き。半角形の ｧ〜ｯ（U+FF67〜U+FF6F）は、互換分解の
///   先が小書きの片仮名。小書きの仮名の拡張（U+1B132・U+1B150〜U+1B152・U+1B155・U+1B164〜U+1B167）
fn small_kana(ch: char) -> bool {
    let code = u32::from(ch);
    let kana = match code {
        0x30A1..=0x30F6 => code - 0x60,
        _ => code,
    };
    matches!(
        kana,
        0x3041
            | 0x3043
            | 0x3045
            | 0x3047
            | 0x3049
            | 0x3063
            | 0x3083
            | 0x3085
            | 0x3087
            | 0x308E
            | 0x3095
            | 0x3096
    ) || matches!(
        code,
        0x31F0..=0x31FF | 0xFF67..=0xFF6F | 0x1B132 | 0x1B150..=0x1B152 | 0x1B155 | 0x1B164..=0x1B167
    )
}

/// 行の終わりに置かない文字（禁則、決定 0187）。開き括弧（Unicode の一般カテゴリ Ps）。
/// 全角形・半角形は、互換分解の先の文字に従う
fn no_end(ch: char) -> bool {
    match narrow(ch) {
        '(' | '[' | '{' => true,
        ch @ ('\u{3008}'..='\u{3011}' | '\u{3014}'..='\u{301B}') => u32::from(ch) % 2 == 0,
        '\u{301D}' | '\u{FF5F}' => true,
        _ => false,
    }
}

/// 全角形（U+FF01〜U+FF5E）を ASCII に、半角の句読点と鉤括弧（U+FF61〜U+FF64）を全角に戻す
/// （Unicode の互換分解の <wide>・<narrow> の対応）
fn narrow(ch: char) -> char {
    let code = u32::from(ch);
    let mapped = match code {
        0xFF01..=0xFF5E => code - 0xFEE0,
        0xFF61 => 0x3002,
        0xFF62 => 0x300C,
        0xFF63 => 0x300D,
        0xFF64 => 0x3001,
        _ => code,
    };
    char::from_u32(mapped).unwrap_or(ch)
}

/// 全角の文字（漢字・かな・ハングル・全角形など）か
fn wide(ch: char) -> bool {
    matches!(
        u32::from(ch),
        0x1100..=0x115F
            | 0x2E80..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x20000..=0x3FFFD
    )
}

/// DXF の特殊な符号を解いた内容（決定 0181）
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Decoded {
    /// 符号を文字に置き換えた内容
    pub text: String,
    /// 下線を引く範囲。解いた内容の文字の位置で、始まりと終わり
    pub underline: Vec<[usize; 2]>,
    /// 上線を引く範囲
    pub overline: Vec<[usize; 2]>,
    /// 解けず、内容にそのまま残した符号の数
    pub unknown: usize,
}

/// DXF の特殊な符号（`%%d`・`%%p`・`%%c`・`%%%`・`%%nnn`、`toggles` なら `%%u`・`%%o` も）を解く。
/// `%%d`・`%%p`・`%%c` は、AutoCAD のヘルプが同じ記号に当てる U+00B0・U+00B1・U+2205 にする。
/// `%%nnn` は 127 以下の値だけを文字にし、それより大きい値は符号のまま残す（文字コードのページに依るため）。
/// `%%u`・`%%o` は下線・上線の開始と終了を切り替える。1 行の文字（TEXT）では `toggles` を真にする
pub fn decode(content: &str, toggles: bool) -> Decoded {
    let chars: Vec<char> = content.chars().collect();
    let mut out = Decoded::default();
    let mut text = Vec::with_capacity(chars.len());
    let mut underline = None;
    let mut overline = None;
    let mut at = 0;
    while let Some(&ch) = chars.get(at) {
        if ch != '%' || chars.get(at + 1) != Some(&'%') {
            text.push(ch);
            at += 1;
            continue;
        }
        let code = chars.get(at + 2).copied();
        let symbol = match code {
            Some('d' | 'D') => Some('\u{B0}'),
            Some('p' | 'P') => Some('\u{B1}'),
            Some('c' | 'C') => Some('\u{2205}'),
            Some('%') => Some('%'),
            _ => None,
        };
        let toggle = match code {
            Some('u' | 'U') if toggles => Some((&mut underline, &mut out.underline)),
            Some('o' | 'O') if toggles => Some((&mut overline, &mut out.overline)),
            _ => None,
        };
        let digits = chars
            .get(at + 2..at + 5)
            .and_then(|digits| {
                digits.iter().try_fold(0_u32, |value, digit| {
                    digit.to_digit(10).map(|digit| value * 10 + digit)
                })
            })
            .filter(|value| *value <= 127)
            .and_then(char::from_u32);
        if let Some(symbol) = symbol {
            text.push(symbol);
            at += 3;
        } else if let Some((open, ranges)) = toggle {
            match open.take() {
                Some(start) => ranges.push([start, text.len()]),
                None => *open = Some(text.len()),
            }
            at += 3;
        } else if let Some(ch) = digits {
            text.push(ch);
            at += 5;
        } else {
            // 解けない符号は `%%` だけを残し、続く文字は内容として足す
            out.unknown += 1;
            text.extend(['%', '%']);
            at += 2;
        }
    }
    // 閉じていない線は、内容の終わりまで引く
    if let Some(start) = underline {
        out.underline.push([start, text.len()]);
    }
    if let Some(start) = overline {
        out.overline.push([start, text.len()]);
    }
    out.text = text.into_iter().collect();
    out
}

/// 宣言された高さの意味を、フォントの寸法に当てる
fn reference(
    meaning: &HeightMeaning,
    declared: impl Fn(&str) -> Option<Reference>,
) -> Result<Reference, Unplaced> {
    Ok(match meaning {
        HeightMeaning::CapHeight => Reference::CapHeight,
        HeightMeaning::Em => Reference::Em,
        HeightMeaning::Frame => Reference::Frame,
        HeightMeaning::Declared(key) => {
            declared(key).ok_or_else(|| Unplaced::Height(key.clone()))?
        }
    })
}

/// 宣言された高さに当たる、フォントの寸法（字形の単位）
pub fn unit(reference: Reference, font: &(impl Glyphs + ?Sized)) -> f64 {
    let metrics = font.metrics();
    match reference {
        Reference::CapHeight => metrics.cap_height,
        Reference::Em => metrics.units_per_em,
        Reference::Frame => metrics.ascent + metrics.descent,
        Reference::CapitalA => font
            .glyph('A')
            .and_then(|glyph| glyph.bounds)
            .map_or(metrics.cap_height, |[_, _, _, top]| top),
    }
}

/// 長さの値。空間は呼び出し側が揃える
fn length(length: &Length) -> f64 {
    match length {
        Length::Paper(length) => length.0,
        Length::Model(length) => length.value,
    }
}

fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn finite(metrics: &FontMetrics) -> bool {
    [
        metrics.units_per_em,
        metrics.ascent,
        metrics.descent,
        metrics.cap_height,
    ]
    .iter()
    .all(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FontKind, Height, LineSpacing, PaperLength, Span, StackKind};

    /// 合成のフォント。em は 1000、大文字の高さ 700、アセンダー 800、ディセンダー 200。
    /// `A` は送り 600・外形 0〜600 × 0〜700、`g` は送り 500・外形 50〜450 × -200〜500、空白は送り 250
    struct Synthetic;

    impl Glyphs for Synthetic {
        fn metrics(&self) -> FontMetrics {
            FontMetrics {
                units_per_em: 1000.0,
                ascent: 800.0,
                descent: 200.0,
                cap_height: 700.0,
            }
        }

        fn glyph(&self, ch: char) -> Option<Glyph> {
            match ch {
                'A' => Some(Glyph {
                    advance: 600.0,
                    bounds: Some([0.0, 0.0, 600.0, 700.0]),
                }),
                'g' => Some(Glyph {
                    advance: 500.0,
                    bounds: Some([50.0, -200.0, 450.0, 500.0]),
                }),
                ' ' => Some(Glyph {
                    advance: 250.0,
                    bounds: None,
                }),
                _ => None,
            }
        }

        fn fallback(&self) -> Glyph {
            Glyph {
                advance: 1000.0,
                bounds: Some([0.0, 0.0, 1000.0, 700.0]),
            }
        }
    }

    fn text(content: &str, placement: Placement<[f64; 2]>) -> Text<[f64; 2]> {
        Text {
            content: content.to_owned(),
            height: Height {
                value: Length::Paper(PaperLength(7.0)),
                meaning: HeightMeaning::CapHeight,
            },
            placement,
            rotation: 0.0,
            width_factor: 1.0,
            oblique: 0.0,
            backward: false,
            upside_down: false,
            style: String::new(),
        }
    }

    fn anchor(point: [f64; 2], horizontal: Horizontal, vertical: Vertical) -> Placement<[f64; 2]> {
        Placement::Anchor {
            point,
            horizontal,
            vertical,
        }
    }

    fn laid(text: &Text<[f64; 2]>) -> Laid {
        place(text, |p| *p, &Synthetic, |_| None).unwrap()
    }

    /// 字形の単位の点を、結果の変換で図面の座標へ写す
    fn map(laid: &Laid, [u, v]: [f64; 2]) -> [f64; 2] {
        let [a, b, c, d, e, f] = laid.transform;
        [a * u + c * v + e, b * u + d * v + f]
    }

    fn assert_near([x, y]: [f64; 2], [ex, ey]: [f64; 2]) {
        assert!(
            (x - ex).abs() < 1e-9 && (y - ey).abs() < 1e-9,
            "({x}, {y}) != ({ex}, {ey})"
        );
    }

    #[test]
    fn nine_points_and_baseline() {
        // 送りの合計 1100、倍率 0.01 で幅 11、大文字の高さ 7、ディセンダー 2
        let cases = [
            (Horizontal::Left, Vertical::Baseline, [0.0, 0.0]),
            (Horizontal::Center, Vertical::Baseline, [5.5, 0.0]),
            (Horizontal::Right, Vertical::Baseline, [11.0, 0.0]),
            (Horizontal::Left, Vertical::Bottom, [0.0, -2.0]),
            (Horizontal::Center, Vertical::Middle, [5.5, 3.5]),
            (Horizontal::Right, Vertical::Top, [11.0, 7.0]),
            // 横は送りの合計の半分、縦は外形（-200〜700）の中央（AutoCAD 2027 と同じ。決定 0246）
            (Horizontal::Middle, Vertical::Baseline, [5.5, 2.5]),
        ];
        for (horizontal, vertical, offset) in cases {
            let laid = laid(&text("Ag", anchor([100.0, 50.0], horizontal, vertical)));
            // 置く点に当たる字形の座標が、置く点に写る
            let at = [offset[0] / 0.01, offset[1] / 0.01];
            assert_near(map(&laid, at), [100.0, 50.0]);
            assert_near(
                map(&laid, [0.0, 0.0]),
                [100.0 - offset[0], 50.0 - offset[1]],
            );
        }
    }

    #[test]
    fn pens_and_missing_glyphs() {
        let laid = laid(&text(
            "A g字",
            anchor([0.0, 0.0], Horizontal::Left, Vertical::Baseline),
        ));
        assert_eq!(laid.pens, vec![0.0, 600.0, 850.0, 1350.0]);
        assert_eq!(laid.advance, 2350.0);
        assert_eq!(laid.missing, 1);
    }

    #[test]
    fn rotation_width_factor_and_oblique() {
        let mut text = text(
            "A",
            anchor([10.0, 0.0], Horizontal::Left, Vertical::Baseline),
        );
        text.rotation = 90.0;
        text.width_factor = 2.0;
        text.oblique = 45.0;
        let laid = laid(&text);
        assert_eq!(laid.scale, [0.02, 0.01]);
        // 送りの終わり（600, 0）は 12 だけ上へ、大文字の上（0, 700）は斜体で 7 だけ前へ寄り、7 だけ左へ
        assert_near(map(&laid, [600.0, 0.0]), [10.0, 12.0]);
        assert_near(map(&laid, [0.0, 700.0]), [3.0, 7.0]);
    }

    #[test]
    fn between_two_points() {
        let placement = |fit| Placement::Between {
            first: [0.0, 0.0],
            second: [0.0, 22.0],
            fit,
        };
        // 送りの合計 1100 を、距離 22 に合わせる
        let aligned = laid(&text("Ag", placement(Fit::Aligned)));
        assert_near([aligned.scale[0], aligned.scale[1]], [0.02, 0.02]);
        assert_near(map(&aligned, [1100.0, 0.0]), [0.0, 22.0]);
        assert_near(map(&aligned, [0.0, 700.0]), [-14.0, 0.0]);

        let fit = laid(&text("Ag", placement(Fit::Fit)));
        assert_near([fit.scale[0], fit.scale[1]], [0.02, 0.01]);
        assert_near(map(&fit, [0.0, 0.0]), [0.0, 0.0]);
        assert_near(map(&fit, [0.0, 700.0]), [-7.0, 0.0]);
    }

    #[test]
    fn mirrors_about_the_point() {
        let mut text = text(
            "A",
            anchor([0.0, 0.0], Horizontal::Left, Vertical::Baseline),
        );
        text.backward = true;
        text.upside_down = true;
        let laid = laid(&text);
        assert_near(map(&laid, [600.0, 700.0]), [-6.0, -7.0]);

        let mut between = self::text(
            "A",
            Placement::Between {
                first: [0.0, 0.0],
                second: [6.0, 0.0],
                fit: Fit::Fit,
            },
        );
        between.backward = true;
        let laid = place(&between, |p| *p, &Synthetic, |_| None).unwrap();
        // 反転しても 2 点の間に収まり、始まりが 2 点目に来る
        assert_near(map(&laid, [0.0, 0.0]), [6.0, 0.0]);
        assert_near(map(&laid, [600.0, 0.0]), [0.0, 0.0]);
    }

    #[test]
    fn height_meanings() {
        let mut text = text(
            "A",
            anchor([0.0, 0.0], Horizontal::Left, Vertical::Baseline),
        );
        text.height.meaning = HeightMeaning::Em;
        assert_eq!(laid(&text).scale, [0.007, 0.007]);
        text.height.meaning = HeightMeaning::Frame;
        assert_eq!(laid(&text).scale, [0.007, 0.007]);

        text.height.meaning = HeightMeaning::Declared("dxf.TEXT.40".into());
        assert_eq!(
            place(&text, |p| *p, &Synthetic, |_| None),
            Err(Unplaced::Height("dxf.TEXT.40".into()))
        );
        let laid = place(
            &text,
            |p| *p,
            &Synthetic,
            |key| (key == "dxf.TEXT.40").then_some(Reference::CapHeight),
        )
        .unwrap();
        assert_eq!(laid.scale, [0.01, 0.01]);
    }

    /// `A` の上が大文字の高さより高いフォント。ほかは [`Synthetic`] と同じ
    struct TallA;

    impl Glyphs for TallA {
        fn metrics(&self) -> FontMetrics {
            Synthetic.metrics()
        }

        fn glyph(&self, ch: char) -> Option<Glyph> {
            match ch {
                'A' => Some(Glyph {
                    advance: 600.0,
                    bounds: Some([0.0, 0.0, 600.0, 875.0]),
                }),
                _ => Synthetic.glyph(ch),
            }
        }

        fn fallback(&self) -> Glyph {
            Synthetic.fallback()
        }
    }

    #[test]
    fn capital_a_maps_the_height_to_the_top_of_a() {
        // AutoCAD 2027 の TrueType の TEXT は、高さを `A` の上に当て、上と中央もその高さで決める
        // （Times New Roman では大文字の高さ 1356 でなく `A` の 1387。決定 0246）
        let mut text = text("Ag", anchor([0.0, 0.0], Horizontal::Left, Vertical::Top));
        text.height.meaning = HeightMeaning::Declared("dxf.TEXT.40".into());
        let declared = |_: &str| Some(Reference::CapitalA);
        let laid = place(&text, |p| *p, &TallA, declared).unwrap();
        assert_eq!(laid.scale, [0.008, 0.008]);
        assert_near(map(&laid, [0.0, 875.0]), [0.0, 0.0]);
        text.placement = anchor([0.0, 0.0], Horizontal::Right, Vertical::Middle);
        let laid = place(&text, |p| *p, &TallA, declared).unwrap();
        assert_near(map(&laid, [1100.0, 437.5]), [0.0, 0.0]);
        // 下はディセンダーの線のまま
        text.placement = anchor([0.0, 0.0], Horizontal::Left, Vertical::Bottom);
        let laid = place(&text, |p| *p, &TallA, declared).unwrap();
        assert_near(map(&laid, [0.0, -200.0]), [0.0, 0.0]);
        // `A` がないフォントでは大文字の高さ
        assert_eq!(unit(Reference::CapitalA, &Empty), 700.0);
        assert_eq!(unit(Reference::CapitalA, &TallA), 875.0);
    }

    /// `A` を持たないフォント
    struct Empty;

    impl Glyphs for Empty {
        fn metrics(&self) -> FontMetrics {
            Synthetic.metrics()
        }

        fn glyph(&self, _: char) -> Option<Glyph> {
            None
        }

        fn fallback(&self) -> Glyph {
            Synthetic.fallback()
        }
    }

    #[test]
    fn degenerate_inputs() {
        let mut text = text(
            "",
            Placement::Between {
                first: [0.0, 0.0],
                second: [1.0, 0.0],
                fit: Fit::Fit,
            },
        );
        let attempt = |text: &Text<[f64; 2]>| place(text, |p| *p, &Synthetic, |_| None);
        assert_eq!(attempt(&text), Err(Unplaced::Degenerate));
        text.content = "A".to_owned();
        assert!(attempt(&text).is_ok());
        text.width_factor = 0.0;
        assert_eq!(attempt(&text), Err(Unplaced::Degenerate));
        text.width_factor = 1.0;
        text.oblique = -90.0;
        assert_eq!(attempt(&text), Err(Unplaced::Degenerate));
        text.oblique = 0.0;
        text.height.value = Length::Paper(PaperLength(f64::NAN));
        assert_eq!(attempt(&text), Err(Unplaced::Degenerate));
    }

    /// 合成のフォントに、`A` と `g` の間を 100 詰めるカーニングを足したもの
    struct Kerned;

    impl Glyphs for Kerned {
        fn metrics(&self) -> FontMetrics {
            Synthetic.metrics()
        }

        fn glyph(&self, ch: char) -> Option<Glyph> {
            Synthetic.glyph(ch)
        }

        fn fallback(&self) -> Glyph {
            Synthetic.fallback()
        }

        fn kerning(&self, left: char, right: char) -> f64 {
            if (left, right) == ('A', 'g') {
                -100.0
            } else {
                0.0
            }
        }
    }

    /// 文字スタイルのフォントは合成のフォントで、名前 `kern` の区間にだけカーニングのあるフォントを渡す
    struct Set {
        vertical: bool,
    }

    impl Fonts for Set {
        fn base(&self) -> &dyn Glyphs {
            &Synthetic
        }

        fn font(&self, font: &Font) -> Option<&dyn Glyphs> {
            (font.name == "kern").then_some(&Kerned as &dyn Glyphs)
        }

        fn vertical(&self) -> bool {
            self.vertical
        }
    }

    const HORIZONTAL: Set = Set { vertical: false };

    fn span(piece: Piece) -> Span {
        Span {
            piece,
            format: Format::default(),
        }
    }

    fn words(content: &str) -> Span {
        span(Piece::Text(content.to_owned()))
    }

    fn formatted(spans: Vec<Span>, width: Option<f64>) -> FormattedText<[f64; 2]> {
        FormattedText {
            content: String::new(),
            spans,
            height: Height {
                value: Length::Paper(PaperLength(7.0)),
                meaning: HeightMeaning::CapHeight,
            },
            point: [0.0, 0.0],
            horizontal: Horizontal::Left,
            vertical: Vertical::Top,
            rotation: Rotation::Angle(0.0),
            width: width.map(|width| Length::Paper(PaperLength(width))),
            frame_height: None,
            flow: Flow::LeftToRight,
            line_spacing: LineSpacing {
                factor: 1.0,
                exact: false,
            },
            background: None,
            style: String::new(),
        }
    }

    fn flowed(text: &FormattedText<[f64; 2]>) -> LaidFormatted {
        place_formatted(text, |p| *p, &HORIZONTAL, |_| None).unwrap()
    }

    /// 字形の基準線の始まりが写る点
    fn origin(glyph: &PlacedGlyph) -> [f64; 2] {
        [glyph.transform[4], glyph.transform[5]]
    }

    /// 行ごとの文字列
    fn line_texts(laid: &LaidFormatted) -> Vec<String> {
        laid.lines
            .iter()
            .map(|line| {
                laid.glyphs[line.glyphs[0]..line.glyphs[1]]
                    .iter()
                    .map(|glyph| glyph.ch)
                    .collect()
            })
            .collect()
    }

    #[test]
    fn formatted_wraps_at_spaces_and_spaces_lines() {
        // A は幅 6、空白は 2.5。「AA AA 」の次の AA で 30 を超える
        let laid = flowed(&formatted(vec![words("AA AA AA")], Some(30.0)));
        assert_eq!(line_texts(&laid), ["AA AA ", "AA"]);
        assert_near(
            [laid.lines[0].width, laid.lines[1].width],
            [14.0 + 12.5, 12.0],
        );
        // 1 行目の基準線は大文字の高さ 7、2 行目は行送り 7 × 5/3 だけ下、内容の高さはディセンダー 2 を足す
        let pitch = 7.0 * 5.0 / 3.0;
        assert_near(
            [laid.lines[0].baseline, laid.lines[1].baseline],
            [7.0, 7.0 + pitch],
        );
        assert_near(laid.size, [30.0, 9.0 + pitch]);
        assert_near(origin(&laid.glyphs[3]), [14.5, -7.0]);
        assert_near(origin(&laid.glyphs[6]), [0.0, -7.0 - pitch]);
    }

    #[test]
    fn formatted_wraps_wide_and_long_words() {
        // フォントにない「字」は代わりの字形で幅 10。全角の文字の間で折り返す
        let laid = flowed(&formatted(vec![words("字字字字")], Some(25.0)));
        assert_eq!(line_texts(&laid), ["字字", "字字"]);
        assert_eq!(laid.missing, 4);
        // 折り返す所のない語は、文字の間で切る
        let laid = flowed(&formatted(vec![words("AAAAAA")], Some(20.0)));
        assert_eq!(line_texts(&laid), ["AAA", "AAA"]);
        // 幅がなければ折り返さない
        let laid = flowed(&formatted(vec![words("AA AA AA")], None));
        assert_eq!(line_texts(&laid), ["AA AA AA"]);
    }

    #[test]
    fn formatted_paragraphs_and_alignment() {
        let mut text = formatted(vec![words("A"), span(Piece::Paragraph), words("AA")], None);
        text.horizontal = Horizontal::Right;
        text.vertical = Vertical::Bottom;
        text.point = [100.0, 50.0];
        let laid = flowed(&text);
        assert_eq!(line_texts(&laid), ["A", "AA"]);
        // 枠の幅は長い行の 12。右揃えで 1 行目は 6 だけ右から始まり、枠の下が置く点に来る
        let total = 7.0 + 7.0 * 5.0 / 3.0 + 2.0;
        assert_near(laid.size, [12.0, total]);
        assert_near(origin(&laid.glyphs[0]), [94.0, 50.0 + total - 7.0]);
        assert_near(origin(&laid.glyphs[1]), [88.0, 52.0]);

        // 向きは X 軸の向きでも渡せる。90 度なら、送りは上へ進む
        text.rotation = Rotation::Direction([0.0, 2.0]);
        let laid = flowed(&text);
        assert_near(
            [laid.glyphs[0].transform[0], laid.glyphs[0].transform[1]],
            [0.0, 0.01],
        );
    }

    #[test]
    fn formatted_span_formats() {
        let mut large = words("AA");
        large.format.height = Some(Scaled::Factor(2.0));
        large.format.width_factor = Some(Scaled::Value(0.5));
        large.format.tracking = Some(Scaled::Factor(2.0));
        large.format.oblique = Some(45.0);
        let mut small = words("A");
        small.format.align = Some(Vertical::Middle);
        let mut text = formatted(
            vec![small, large.clone(), span(Piece::Paragraph), large],
            None,
        );
        let laid = flowed(&text);
        // 大きい区間は高さ 14・幅 0.5 で倍率 0.01 × 0.02、字の間隔 2 倍で送りは 12
        let big = laid.glyphs[1].transform;
        assert_near([big[0], big[3]], [0.01, 0.02]);
        assert_near([big[2], 0.0], [0.02, 0.0]);
        assert_near(origin(&laid.glyphs[2]), [18.0, -14.0]);
        // 小さい区間は、行の大文字の高さ 14 の中央に寄せて 3.5 上がる
        assert_near(origin(&laid.glyphs[0]), [0.0, -10.5]);
        // 行送りは、大きい字のある 2 行目で広がり、固定なら広がらない
        assert_near(
            [laid.lines[1].baseline, 0.0],
            [14.0 + 14.0 * 5.0 / 3.0, 0.0],
        );
        text.line_spacing = LineSpacing {
            factor: 1.5,
            exact: true,
        };
        let laid = flowed(&text);
        assert_near([laid.lines[1].baseline, 0.0], [14.0 + 7.0 * 2.5, 0.0]);
    }

    #[test]
    fn kerning_in_text_and_spans() {
        let laid = place(
            &text(
                "Ag",
                anchor([0.0, 0.0], Horizontal::Left, Vertical::Baseline),
            ),
            |p| *p,
            &Kerned,
            |_| None,
        )
        .unwrap();
        assert_eq!(laid.pens, vec![0.0, 500.0]);
        assert_eq!(laid.advance, 1000.0);

        let mut kerned = words("Ag");
        kerned.format.font = Some(Font {
            name: "kern".to_owned(),
            kind: FontKind::Shape,
        });
        let mut other = kerned.clone();
        other.format.font = Some(Font {
            name: "other".to_owned(),
            kind: FontKind::Shape,
        });
        let laid = flowed(&formatted(vec![kerned, other], None));
        assert_near(origin(&laid.glyphs[1]), [5.0, -7.0]);
        // 渡されないフォントは文字スタイルのフォントで置き、数える。区間をまたいでは詰めない
        assert_near(origin(&laid.glyphs[2]), [10.0, -7.0]);
        assert_near(origin(&laid.glyphs[3]), [16.0, -7.0]);
        assert_eq!(laid.substituted, 1);
    }

    fn stack(upper: &str, lower: &str, kind: StackKind) -> Span {
        span(Piece::Stack {
            upper: upper.to_owned(),
            lower: lower.to_owned(),
            kind,
        })
    }

    #[test]
    fn formatted_skips_and_refuses() {
        let text = formatted(
            vec![
                words("A"),
                stack("1", "2", StackKind::Horizontal),
                span(Piece::Unparsed("\\pxqc;".to_owned())),
                span(Piece::Measurement),
            ],
            None,
        );
        let laid = flowed(&text);
        assert_eq!(laid.skipped, vec![2, 3]);
        // 「1」「2」はフォントになく、代わりの字形で置く
        assert_eq!(laid.glyphs.len(), 3);
        assert_eq!(laid.missing, 2);

        // 縦書きでも、上下に重ねた文字を置く（決定 0189）
        let mut vertical = text.clone();
        vertical.flow = Flow::TopToBottom;
        let laid = place_formatted(&vertical, |p| *p, &HORIZONTAL, |_| None).unwrap();
        assert_eq!(laid.skipped, vec![2, 3]);
        assert_eq!(laid.stacks.len(), 1);
        vertical.flow = Flow::ByStyle;
        let by_style = place_formatted(&vertical, |p| *p, &Set { vertical: true }, |_| None);
        assert_eq!(by_style.unwrap().skipped, vec![2, 3]);

        let mut degenerate = text;
        degenerate.line_spacing.factor = 0.0;
        assert_eq!(
            place_formatted(&degenerate, |p| *p, &HORIZONTAL, |_| None),
            Err(Unplaced::Degenerate)
        );
    }

    #[test]
    fn decodes_special_codes() {
        let decoded = decode("%%d%%P%%c%%%", true);
        assert_eq!(decoded.text, "\u{B0}\u{B1}\u{2205}%");
        assert_eq!(decoded.unknown, 0);

        let decoded = decode("%%uAB%%u C%%o D", true);
        assert_eq!(decoded.text, "AB C D");
        assert_eq!(decoded.underline, vec![[0, 2]]);
        assert_eq!(decoded.overline, vec![[4, 6]]);

        assert_eq!(decode("%%065字", true).text, "A字");
        // 127 を超える値と、知らない符号は残して数える
        let decoded = decode("%%200 %%v0.1 %%", true);
        assert_eq!(decoded.text, "%%200 %%v0.1 %%");
        assert_eq!(decoded.unknown, 3);
        // 書式つきの文字では、下線と上線の符号を解かない
        let decoded = decode("%%uA%%d", false);
        assert_eq!(decoded.text, "%%uA\u{B0}");
        assert!(decoded.underline.is_empty());
        assert_eq!(decoded.unknown, 1);
    }

    /// 縦書きの寸法を持つ合成のフォント。「（」は縦書き用の字形に置き換え、縦の送り 1000・原点の高さ 880。
    /// 「あ」「い」は `A` と同じ字形。ほかの文字は合成のフォントの既定（縦の送り 1000、原点の高さ 800）。
    /// 縦書きでは「あ」「い」の間を 200 詰め、横書きのカーニングは `A` と `g` の間を 100 詰める
    struct Upright;

    impl Glyphs for Upright {
        fn metrics(&self) -> FontMetrics {
            Synthetic.metrics()
        }

        fn glyph(&self, ch: char) -> Option<Glyph> {
            match ch {
                '（' => Some(Glyph {
                    advance: 1000.0,
                    bounds: Some([600.0, -100.0, 900.0, 800.0]),
                }),
                'あ' | 'い' => Synthetic.glyph('A'),
                _ => Synthetic.glyph(ch),
            }
        }

        fn fallback(&self) -> Glyph {
            Synthetic.fallback()
        }

        fn kerning(&self, left: char, right: char) -> f64 {
            Kerned.kerning(left, right)
        }

        fn vertical_kerning(&self, left: char, right: char) -> f64 {
            if (left, right) == ('あ', 'い') {
                -200.0
            } else {
                0.0
            }
        }

        fn vertical(&self, ch: char) -> Option<VerticalGlyph> {
            if ch == '（' {
                return Some(VerticalGlyph {
                    glyph: Glyph {
                        advance: 1000.0,
                        bounds: Some([100.0, 0.0, 900.0, 300.0]),
                    },
                    advance: 1000.0,
                    top: 880.0,
                    substituted: true,
                });
            }
            self.glyph(ch)
                .map(|glyph| VerticalGlyph::upright(glyph, &self.metrics()))
        }
    }

    struct UprightSet;

    impl Fonts for UprightSet {
        fn base(&self) -> &dyn Glyphs {
            &Upright
        }

        fn font(&self, _: &Font) -> Option<&dyn Glyphs> {
            None
        }

        fn vertical(&self) -> bool {
            true
        }
    }

    fn columned(text: &FormattedText<[f64; 2]>) -> LaidFormatted {
        let mut text = text.clone();
        text.flow = Flow::ByStyle;
        place_formatted(&text, |p| *p, &UprightSet, |_| None).unwrap()
    }

    #[test]
    fn vertical_columns_run_down_and_to_the_left() {
        let laid = columned(&formatted(
            vec![words("あ（"), span(Piece::Paragraph), words("あ")],
            None,
        ));
        assert_eq!(line_texts(&laid), ["あ（", "あ"]);
        // 列の太さは em の 10。1 列目の中心は右から 5、2 列目は行送り 35/3 だけ左
        let pitch = 7.0 * 5.0 / 3.0;
        let total = 10.0 + pitch;
        assert_near(laid.size, [total, 20.0]);
        assert_near(
            [laid.lines[0].baseline, laid.lines[1].baseline],
            [5.0, 5.0 + pitch],
        );
        assert_near([laid.lines[0].width, laid.lines[1].width], [20.0, 10.0]);
        // A は送りの半分 3 だけ中心から左、原点の高さ 8 だけ送りの始まりから下
        assert_near(origin(&laid.glyphs[0]), [total - 5.0 - 3.0, -8.0]);
        // 「（」は縦書き用の字形に置き換え、送りの半分 5、原点の高さ 8.8
        assert!(laid.glyphs[1].substituted && !laid.glyphs[0].substituted);
        assert_near(origin(&laid.glyphs[1]), [total - 10.0, -10.0 - 8.8]);
        assert_near(origin(&laid.glyphs[2]), [total - 5.0 - pitch - 3.0, -8.0]);

        // 右下を置く点に合わせ、短い列は下に揃える
        let mut text = formatted(
            vec![words("ああ"), span(Piece::Paragraph), words("あ")],
            None,
        );
        text.horizontal = Horizontal::Right;
        text.vertical = Vertical::Bottom;
        let laid = columned(&text);
        assert_near([laid.lines[1].offset, 0.0], [10.0, 0.0]);
        assert_near(origin(&laid.glyphs[0]), [-8.0, 20.0 - 8.0]);
        assert_near(origin(&laid.glyphs[2]), [-5.0 - pitch - 3.0, 10.0 - 8.0]);
    }

    #[test]
    fn vertical_columns_wrap_at_the_frame() {
        // 枠の幅は列の長さに当てる。送り 10 の文字を 25 で折る
        let laid = columned(&formatted(vec![words("あああ")], Some(25.0)));
        assert_eq!(line_texts(&laid), ["ああ", "あ"]);
        assert_near(laid.size, [10.0 + 7.0 * 5.0 / 3.0, 25.0]);
        // 横書きの向きと同じく、回す
        let mut text = formatted(vec![words("あ")], None);
        text.rotation = Rotation::Angle(90.0);
        let laid = columned(&text);
        assert_near(origin(&laid.glyphs[0]), [8.0, 2.0]);
    }

    #[test]
    fn stacks_fractions_and_tolerances() {
        // 上下の段は 0.7 倍。A の幅は 4.2。横線は大文字の高さの半分 3.5 にあり、段は 0.7 だけ離れる
        let laid = flowed(&formatted(
            vec![words("A"), stack("A", "AA", StackKind::Horizontal)],
            None,
        ));
        assert_eq!(line_texts(&laid), ["AAAA"]);
        assert!(laid.skipped.is_empty());
        // 上の段の上 9.1 が行の大文字の高さになり、下の段の下 2.1 + 1.4 が内容の下になる
        assert_near([laid.lines[0].baseline, laid.size[1]], [9.1, 9.1 + 3.5]);
        assert_near(laid.size, [6.0 + 8.4, 12.6]);
        let scale = laid.glyphs[1].transform;
        assert_near([scale[0], scale[3]], [0.007, 0.007]);
        // 短い上の段は中央に揃う
        assert_near(origin(&laid.glyphs[1]), [8.1, -9.1 + 4.2]);
        assert_near(origin(&laid.glyphs[2]), [6.0, -9.1 - 2.1]);
        assert_near(origin(&laid.glyphs[3]), [10.2, -9.1 - 2.1]);
        assert_eq!(laid.stacks.len(), 1);
        assert_eq!(laid.stacks[0].span, 1);
        let [start, end] = laid.stacks[0].line.unwrap();
        assert_near(start, [6.0, -5.6]);
        assert_near(end, [14.4, -5.6]);

        // 公差は左に揃え、線を引かない
        let laid = flowed(&formatted(
            vec![stack("A", "AA", StackKind::Tolerance)],
            None,
        ));
        assert_near(origin(&laid.glyphs[0]), [0.0, -9.1 + 4.2]);
        assert_eq!(laid.stacks[0].line, None);

        // 斜線は、上の段の上を大文字の高さに、下の段を基準線に置き、間に幅 3.5 の線を引く
        let laid = flowed(&formatted(vec![stack("A", "A", StackKind::Diagonal)], None));
        assert_near(laid.size, [4.2 + 3.5 + 4.2, 7.0 + 2.0]);
        assert_near(origin(&laid.glyphs[0]), [0.0, -7.0 + 2.1]);
        assert_near(origin(&laid.glyphs[1]), [7.7, -7.0]);
        let [start, end] = laid.stacks[0].line.unwrap();
        assert_near(start, [4.2, -7.0]);
        assert_near(end, [7.7, 0.0]);
    }

    #[test]
    fn line_breaks_keep_kinsoku() {
        // フォントにない文字は幅 10。閉じ括弧・句点は行の始まりに、開き括弧は行の終わりに置かない
        for (content, expected) in [
            ("字字」", ["字", "字」"]),
            ("字字！", ["字", "字！"]),
            ("字字ょ", ["字", "字ょ"]),
            ("字字ー", ["字", "字ー"]),
            ("字字ｯ", ["字", "字ｯ"]),
            ("字「字", ["字", "「字"]),
            ("字（字", ["字", "（字"]),
        ] {
            let laid = flowed(&formatted(vec![words(content)], Some(25.0)));
            assert_eq!(line_texts(&laid), expected, "{content}");
        }
        // 折り返す所がなければ、文字の間で切る
        let laid = flowed(&formatted(vec![words("字」」")], Some(25.0)));
        assert_eq!(line_texts(&laid), ["字」", "」"]);

        for ch in [
            '）', '〕', '』', '】', '〉', '》', ')', ',', '、', '?', '〟',
        ] {
            assert!(no_start(ch) && !no_end(ch), "{ch}");
        }
        for ch in ['（', '〔', '『', '【', '〈', '《', '(', '［', '｢', '〝'] {
            assert!(no_end(ch) && !no_start(ch), "{ch}");
        }
        for ch in [
            '字',
            'A',
            'あ',
            'ア',
            'ゔ',
            'ヷ',
            'ｱ',
            '・',
            '〜',
            '\u{1B000}',
        ] {
            assert!(!no_start(ch) && !no_end(ch), "{ch}");
        }
        // 小書きの仮名と長音の記号は、行の始まりに置かない（決定 0189）
        for ch in [
            'ぁ',
            'ぃ',
            'ぅ',
            'ぇ',
            'ぉ',
            'っ',
            'ゃ',
            'ゅ',
            'ょ',
            'ゎ',
            'ゕ',
            'ゖ',
            'ァ',
            'ッ',
            'ャ',
            'ヮ',
            'ヵ',
            'ヶ',
            'ㇰ',
            'ㇿ',
            'ｧ',
            'ｯ',
            'ー',
            'ｰ',
            '\u{1B132}',
            '\u{1B155}',
            '\u{1B167}',
        ] {
            assert!(no_start(ch) && !no_end(ch), "{ch}");
        }
    }

    #[test]
    fn punctuation_hangs_past_the_frame() {
        // 読点・句点は、前までが収まれば枠の外にぶら下げ、行の長さに数えない（決定 0189）
        for content in ["字字。", "字字、", "字字｡", "字字，", "字字．"] {
            let laid = flowed(&formatted(vec![words(content)], Some(25.0)));
            assert_eq!(line_texts(&laid), [content], "{content}");
            assert_near([laid.lines[0].width, laid.size[0]], [20.0, 25.0]);
        }
        // ぶら下げるのは 1 文字まで。2 つ目の句点は行の始まりに置けないので、前の字と送る
        let laid = flowed(&formatted(vec![words("字字。。")], Some(25.0)));
        assert_eq!(line_texts(&laid), ["字", "字。。"]);
        // 収まる句点は、行の長さに数える
        let laid = flowed(&formatted(vec![words("字。")], Some(25.0)));
        assert_near([laid.lines[0].width, 0.0], [20.0, 0.0]);
        // 縦書きでも、列の下にぶら下げる
        let laid = columned(&formatted(vec![words("字字。")], Some(25.0)));
        assert_eq!(line_texts(&laid), ["字字。"]);
        assert_near([laid.lines[0].width, 0.0], [20.0, 0.0]);
    }

    #[test]
    fn vertical_kerning_turned_latin_and_stacks() {
        // 立てた文字の間は縦書きのカーニングで 2 詰める。A・g は時計回りに倒し、横書きのカーニングで 1 詰める
        let laid = columned(&formatted(vec![words("あいAgい")], None));
        assert_eq!(line_texts(&laid), ["あいAgい"]);
        // 縦の送りは 10、倒した A は 6、g は 5
        let length = 10.0 - 2.0 + 10.0 + 6.0 - 1.0 + 5.0 + 10.0;
        assert_near([laid.lines[0].width, laid.size[1]], [length, length]);
        let x = laid.size[0] - 5.0;
        assert_near(origin(&laid.glyphs[1]), [x - 3.0, -8.0 - 8.0]);
        // 倒した字形の基準線は、中心線から（アセンダー 8 - ディセンダー 2）/ 2 だけ左。
        // 字形の横の向きは列の下へ、縦の向きは右へ写る
        assert_near(origin(&laid.glyphs[2]), [x - 3.0, -18.0]);
        let t = laid.glyphs[2].transform;
        assert_near([t[0], t[1]], [0.0, -0.01]);
        assert_near([t[2], t[3]], [0.01, 0.0]);
        assert!(!laid.glyphs[2].substituted);
        assert_near(origin(&laid.glyphs[3]), [x - 3.0, -18.0 - 6.0 + 1.0]);
        // 倒した文字と立てた文字の間は詰めない
        assert_near(origin(&laid.glyphs[4]), [x - 3.0, -28.0 - 8.0]);

        // 回した枠でも、倒した字形は枠と一緒に回る
        let mut text = formatted(vec![words("A")], None);
        text.rotation = Rotation::Angle(90.0);
        let laid = columned(&text);
        let t = laid.glyphs[0].transform;
        assert_near([t[0], t[1]], [0.01, 0.0]);
        assert_near([t[2], t[3]], [0.0, 0.01]);

        // 上下に重ねた文字は立てたまま、列の中心に揃え、上の段の上から下の段の下までを縦の送りにする
        let laid = columned(&formatted(
            vec![
                words("あ"),
                stack("A", "AA", StackKind::Horizontal),
                words("あ"),
            ],
            None,
        ));
        assert!(laid.skipped.is_empty());
        assert_eq!(laid.stacks.len(), 1);
        // 段の幅は 8.4、上の段の上は基準線から 9.1、下の段の下は 3.5
        let x = laid.size[0] - 5.0;
        assert_near(origin(&laid.glyphs[1]), [x - 4.2 + 2.1, -10.0 - 9.1 + 4.2]);
        assert_near(origin(&laid.glyphs[2]), [x - 4.2, -10.0 - 9.1 - 2.1]);
        let [start, end] = laid.stacks[0].line.unwrap();
        assert_near(start, [x - 4.2, -10.0 - 9.1 + 3.5]);
        assert_near(end, [x + 4.2, -10.0 - 9.1 + 3.5]);
        assert_near(origin(&laid.glyphs[4]), [x - 3.0, -10.0 - 12.6 - 8.0]);
        assert_near([laid.lines[0].width, 0.0], [10.0 + 12.6 + 10.0, 0.0]);
    }
}
