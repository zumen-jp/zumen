//! フォントの読み取り（[`font`]・[`shx`]）と文字の配置（[`layout`]）（L3、#851・#875）。
//!
//! 図面の領域モデル（[`zumen_drawing`]）の文字と文字スタイルを受け取り、呼び出し側が渡したフォントの寸法と
//! 輪郭・ストロークで文字を置く。フォント自体は同梱しない（決定 0022）。

pub mod font;
pub mod layout;
pub mod shx;

use zumen_base::{Cx, Stop};

use crate::layout::{FontMetrics, Glyph, Glyphs, VerticalGlyph};

/// 呼び出し側が渡したフォントの統一表現（OpenType・TrueType・TTC、または AutoCAD の `.shx`）
pub enum Typeface<'a> {
    /// TrueType・OpenType・TTC フォント（塗る閉じた輪郭）
    OpenType(Box<font::Font<'a>>),
    /// AutoCAD の `.shx` フォント（線で描くストローク）
    Shx(shx::ShxFont<'a>),
}

impl<'a> From<font::Font<'a>> for Typeface<'a> {
    fn from(font: font::Font<'a>) -> Self {
        Self::OpenType(Box::new(font))
    }
}

impl<'a> From<shx::ShxFont<'a>> for Typeface<'a> {
    fn from(font: shx::ShxFont<'a>) -> Self {
        Self::Shx(font)
    }
}

/// フォントを読めない理由
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypefaceError {
    /// 形式が合わないか、中身が壊れている
    Invalid,
    /// 予算か中断の上限に達した
    Budget(Stop),
}

impl<'a> Typeface<'a> {
    /// バイト列から OpenType/TrueType/TTC または `.shx` フォントを読む。`.shx` はコレクションを持たないため
    /// `index == 0` のときだけ受理する
    pub fn parse(bytes: &'a [u8], index: u32, cx: &mut Cx<'_>) -> Result<Self, TypefaceError> {
        match shx::ShxFont::parse(bytes, cx) {
            Ok(shx) => {
                if index != 0 {
                    return Err(TypefaceError::Invalid);
                }
                Ok(Self::Shx(shx))
            }
            Err(shx::ShxError::Budget(stop)) => Err(TypefaceError::Budget(stop)),
            Err(shx::ShxError::Malformed) => Err(TypefaceError::Invalid),
            Err(shx::ShxError::Magic) => font::Font::parse(bytes, index)
                .map(|f| Self::OpenType(Box::new(f)))
                .map_err(|_| TypefaceError::Invalid),
        }
    }

    /// `.shx` フォントか
    pub fn is_shx(&self) -> bool {
        matches!(self, Self::Shx(_))
    }

    /// OpenType フォントへの参照
    pub fn as_opentype(&self) -> Option<&font::Font<'a>> {
        match self {
            Self::OpenType(f) => Some(f),
            Self::Shx(_) => None,
        }
    }

    /// `.shx` フォントへの参照
    pub fn as_shx(&self) -> Option<&shx::ShxFont<'a>> {
        match self {
            Self::Shx(f) => Some(f),
            Self::OpenType(_) => None,
        }
    }
}

impl Glyphs for Typeface<'_> {
    fn metrics(&self) -> FontMetrics {
        match self {
            Self::OpenType(f) => f.metrics(),
            Self::Shx(f) => f.metrics(),
        }
    }

    fn glyph(&self, ch: char) -> Option<Glyph> {
        match self {
            Self::OpenType(f) => f.glyph(ch),
            Self::Shx(f) => f.glyph(ch),
        }
    }

    fn fallback(&self) -> Glyph {
        match self {
            Self::OpenType(f) => f.fallback(),
            Self::Shx(f) => f.fallback(),
        }
    }

    fn kerning(&self, left: char, right: char) -> f64 {
        match self {
            Self::OpenType(f) => f.kerning(left, right),
            Self::Shx(f) => f.kerning(left, right),
        }
    }

    fn vertical_kerning(&self, left: char, right: char) -> f64 {
        match self {
            Self::OpenType(f) => f.vertical_kerning(left, right),
            Self::Shx(f) => f.vertical_kerning(left, right),
        }
    }

    fn vertical(&self, ch: char) -> Option<VerticalGlyph> {
        match self {
            Self::OpenType(f) => f.vertical(ch),
            Self::Shx(f) => f.vertical(ch),
        }
    }

    fn vertical_fallback(&self) -> VerticalGlyph {
        match self {
            Self::OpenType(f) => f.vertical_fallback(),
            Self::Shx(f) => f.vertical_fallback(),
        }
    }
}
