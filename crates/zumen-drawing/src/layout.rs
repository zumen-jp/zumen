//! 1 行の文字（[`Text`]）を、呼び出し側が渡したフォントの字形の寸法で置く試作（決定 0176）。
//!
//! フォントは同梱せず、[`Glyphs`] で寸法だけを受け取る（決定 0022）。結果は、字形の単位の座標を
//! 図面の座標へ写すアフィン変換と、文字ごとの送りの位置で返す。字形の輪郭は扱わない。
//!
//! 位置合わせの規則は、DXF の TEXT（72・73）の説明と旧版の知見から推定したもので、AutoCAD・Jw_cad の
//! 表示とはまだ比べていない。
//!
//! - 左・中央・右は、送りの合計の 0・半分・全部の位置。外形の中央（[`Horizontal::Middle`]）は、
//!   字形の外形の中央で、縦も外形の中央にする。
//! - 基準線・下・中央・上は、基準線・ディセンダーの線・大文字の高さの半分・大文字の高さの線。
//! - 2 点の間は、送りの合計が 2 点の距離になるよう幅を変え、Aligned は高さも同じ比で変える。
//! - 斜体は字形にだけ当て、位置合わせは斜体を当てる前の寸法で決める。左右・上下の反転は、
//!   置く点（2 点の間では 2 点の中点）のまわりで当てる。
//!
//! 書式つきの文字（[`FormattedText`]）の折り返し・行送り・区間ごとの書式、DXF の特殊な符号、カーニングの
//! 扱いは決定 0181 にある。縦書きは置かない。

use crate::{
    Fit, Flow, Font, Format, FormattedText, HeightMeaning, Horizontal, Length, Piece, Placement,
    Rotation, Scaled, Text, Vertical,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unplaced {
    /// 宣言された高さが、フォントのどの寸法に当たるか決まらない。形式と欄を指すキー
    Height(&'static str),
    /// 高さ・幅の倍率・字の間隔・行送り・枠の幅・向き・フォントの寸法・2 点の距離・送りの合計が 0 か
    /// 有限でないか、斜体の角度が 90 度以上
    Degenerate,
    /// 縦書き。縦書きの字形の寸法を受け取らないので置かない（決定 0181）
    Vertical,
}

/// 文字を置く。`xy` は点を図面の座標にし、`declared` は形式の宣言値の高さの意味を決める
pub fn place<P>(
    text: &Text<P>,
    xy: impl Fn(&P) -> [f64; 2],
    font: &impl Glyphs,
    declared: impl Fn(&'static str) -> Option<Reference>,
) -> Result<Laid, Unplaced> {
    let metrics = font.metrics();
    let reference = reference(&text.height.meaning, declared)?;
    let scale = length(&text.height.value) / unit(reference, &metrics);
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
            let ink_center = ink.map(|[l, b, r, t]| [(l + r) / 2.0 * sx, (b + t) / 2.0 * sy]);
            let x = match *horizontal {
                Horizontal::Left => 0.0,
                Horizontal::Center => width / 2.0,
                Horizontal::Right => width,
                Horizontal::Middle => ink_center.map_or(width / 2.0, |[x, _]| x),
            };
            let y = match (*horizontal, *vertical) {
                (Horizontal::Middle, _) => {
                    ink_center.map_or(metrics.cap_height / 2.0 * sy, |[_, y]| y)
                }
                (_, Vertical::Baseline) => 0.0,
                (_, Vertical::Bottom) => -metrics.descent * sy,
                (_, Vertical::Middle) => metrics.cap_height / 2.0 * sy,
                (_, Vertical::Top) => metrics.cap_height * sy,
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
    /// 置いた字形。内容の順
    pub glyphs: Vec<PlacedGlyph>,
    /// 行。上から順
    pub lines: Vec<LineBox>,
    /// 枠の幅と、内容の高さ（図面の長さ）
    pub size: [f64; 2],
    /// フォントになく、代わりの字形で置いた文字の数
    pub missing: usize,
    /// 区間で指定されたフォントが渡されず、文字スタイルのフォントで置いた区間の数
    pub substituted: usize,
    /// 置かなかった区間の位置（上下に重ねた文字・測定値・解釈できない符号など）。入力の順
    pub skipped: Vec<usize>,
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
}

/// 行。値は図面の長さで、枠の左上から測る
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

/// 書式つきの文字を置く。枠の幅で折り返し、行送りと区間ごとの書式（高さ・幅の倍率・字の間隔・斜体・
/// フォント・行の中の位置合わせ）を当てる。DXF の特殊な符号は解かないので、呼び出し側が [`decode`] で
/// 解いてから渡す（決定 0181）
pub fn place_formatted<P>(
    text: &FormattedText<P>,
    xy: impl Fn(&P) -> [f64; 2],
    fonts: &impl Fonts,
    declared: impl Fn(&'static str) -> Option<Reference>,
) -> Result<LaidFormatted, Unplaced> {
    let vertical = match text.flow {
        Flow::LeftToRight => false,
        Flow::TopToBottom => true,
        Flow::ByStyle => fonts.vertical(),
    };
    if vertical {
        return Err(Unplaced::Vertical);
    }
    let reference = reference(&text.height.meaning, declared)?;
    let height = length(&text.height.value);
    let base = fonts.base().metrics();
    let base_scale = height / unit(reference, &base);
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
    };
    let mut tokens = Vec::new();
    for (index, span) in text.spans.iter().enumerate() {
        match &span.piece {
            Piece::Text(content) => {
                let font = match &span.format.font {
                    Some(wanted) => fonts.font(wanted).unwrap_or_else(|| {
                        out.substituted += 1;
                        fonts.base()
                    }),
                    None => fonts.base(),
                };
                let run = SpanRun::new(&span.format, font, reference, height)?;
                let mut previous = None;
                for ch in content.chars() {
                    let glyph = font.glyph(ch).unwrap_or_else(|| {
                        out.missing += 1;
                        font.fallback()
                    });
                    let kern = previous.map_or(0.0, |left| font.kerning(left, ch) * run.sx);
                    previous = Some(ch);
                    tokens.push(Some(Item {
                        span: index,
                        ch,
                        advance: glyph.advance * run.sx * run.tracking,
                        kern,
                        space: ch.is_whitespace(),
                        run,
                    }));
                }
            }
            // 段組みは移さない（決定 0109）ので、段の区切りは段落の区切りとして置く
            Piece::Paragraph | Piece::Column => tokens.push(None),
            Piece::Measurement | Piece::Below | Piece::Stack { .. } | Piece::Unparsed(_) => {
                out.skipped.push(index);
            }
        }
    }

    let lines = wrap(tokens, limit);
    let base_cap = base.cap_height * base_scale;
    let base_descent = base.descent * base_scale;
    let mut depth = 0.0;
    let mut descent = base_descent;
    let mut boxes = Vec::new();
    for (number, line) in lines.iter().enumerate() {
        let cap = line
            .iter()
            .map(|item| item.run.cap)
            .fold(base_cap, f64::max);
        let tallest = line
            .iter()
            .map(|item| item.run.height)
            .fold(height, f64::max);
        descent = line
            .iter()
            .map(|item| item.run.descent)
            .fold(base_descent, f64::max);
        depth += if number == 0 {
            cap
        } else if text.line_spacing.exact {
            LINE_PITCH * spacing * height
        } else {
            LINE_PITCH * spacing * tallest
        };
        let width = extent(
            line.iter().rev().skip_while(|item| item.space).count(),
            line,
        );
        boxes.push((depth, cap, width));
    }
    let frame = limit.unwrap_or_else(|| boxes.iter().map(|b| b.2).fold(0.0, f64::max));
    let total = depth + descent;
    let left = match text.horizontal {
        Horizontal::Left => 0.0,
        Horizontal::Center | Horizontal::Middle => -frame / 2.0,
        Horizontal::Right => -frame,
    };
    let top = match text.vertical {
        Vertical::Top => 0.0,
        Vertical::Middle => total / 2.0,
        Vertical::Bottom | Vertical::Baseline => total,
    };
    let [px, py] = xy(&text.point);
    for (line, (depth, cap, width)) in lines.iter().zip(boxes) {
        let offset = match text.horizontal {
            Horizontal::Left => 0.0,
            Horizontal::Center | Horizontal::Middle => (frame - width) / 2.0,
            Horizontal::Right => frame - width,
        };
        let start = out.glyphs.len();
        let mut pen = 0.0;
        for (position, item) in line.iter().enumerate() {
            if position > 0 {
                pen += item.kern;
            }
            let run = &item.run;
            let shift = match run.align {
                Vertical::Middle => (cap - run.cap) / 2.0,
                Vertical::Top => cap - run.cap,
                Vertical::Baseline | Vertical::Bottom => 0.0,
            };
            let x = left + offset + pen;
            let y = top - depth + shift;
            out.glyphs.push(PlacedGlyph {
                span: item.span,
                ch: item.ch,
                transform: [
                    cos * run.sx,
                    sin * run.sx,
                    cos * run.sy * run.shear - sin * run.sy,
                    sin * run.sy * run.shear + cos * run.sy,
                    px + cos * x - sin * y,
                    py + sin * x + cos * y,
                ],
            });
            pen += item.advance;
        }
        out.lines.push(LineBox {
            glyphs: [start, out.glyphs.len()],
            baseline: depth,
            offset,
            width,
        });
    }
    out.size = [frame, total];
    Ok(out)
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
        let sy = height / unit(reference, &metrics);
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
            align: format.align.unwrap_or(Vertical::Baseline),
        })
    }
}

/// 並べる 1 文字。送りと詰めは図面の長さ
#[derive(Clone, Copy, Debug)]
struct Item {
    span: usize,
    ch: char,
    advance: f64,
    /// 前の文字との間に足す送り。行の始まりでは使わない
    kern: f64,
    space: bool,
    run: SpanRun,
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
/// 空白の後と、全角の文字の前後に限り、なければ文字の間で切る。空白は行の終わりにぶら下げる
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
            !item.space
                && !line.is_empty()
                && extent(line.len(), line) + item.kern + item.advance > limit * (1.0 + 1e-9)
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

/// 2 文字の間で折り返せるか
fn breakable(before: &Item, after: &Item) -> bool {
    !after.space && (before.space || wide(before.ch) || wide(after.ch))
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
    declared: impl Fn(&'static str) -> Option<Reference>,
) -> Result<Reference, Unplaced> {
    Ok(match *meaning {
        HeightMeaning::CapHeight => Reference::CapHeight,
        HeightMeaning::Em => Reference::Em,
        HeightMeaning::Frame => Reference::Frame,
        HeightMeaning::Declared(key) => declared(key).ok_or(Unplaced::Height(key))?,
    })
}

/// 宣言された高さに当たる、フォントの寸法（字形の単位）
fn unit(reference: Reference, metrics: &FontMetrics) -> f64 {
    match reference {
        Reference::CapHeight => metrics.cap_height,
        Reference::Em => metrics.units_per_em,
        Reference::Frame => metrics.ascent + metrics.descent,
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
            // 外形は 0〜1050 × -200〜700 なので、中央は 5.25・2.5
            (Horizontal::Middle, Vertical::Baseline, [5.25, 2.5]),
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

        text.height.meaning = HeightMeaning::Declared("dxf.TEXT.40");
        assert_eq!(
            place(&text, |p| *p, &Synthetic, |_| None),
            Err(Unplaced::Height("dxf.TEXT.40"))
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

    #[test]
    fn formatted_skips_and_refuses() {
        let text = formatted(
            vec![
                words("A"),
                span(Piece::Stack {
                    upper: "1".to_owned(),
                    lower: "2".to_owned(),
                    kind: StackKind::Horizontal,
                }),
                span(Piece::Unparsed("\\pxqc;".to_owned())),
                span(Piece::Measurement),
            ],
            None,
        );
        let laid = flowed(&text);
        assert_eq!(laid.skipped, vec![1, 2, 3]);
        assert_eq!(laid.glyphs.len(), 1);

        let mut vertical = text.clone();
        vertical.flow = Flow::TopToBottom;
        assert_eq!(
            place_formatted(&vertical, |p| *p, &HORIZONTAL, |_| None),
            Err(Unplaced::Vertical)
        );
        vertical.flow = Flow::ByStyle;
        assert_eq!(
            place_formatted(&vertical, |p| *p, &Set { vertical: true }, |_| None),
            Err(Unplaced::Vertical)
        );
        assert!(place_formatted(&vertical, |p| *p, &HORIZONTAL, |_| None).is_ok());

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
}
