//! 塗りと線の色を、図面モデルの色にする（決定 0256）。
//!
//! hayro-interpret の RGB への換算は、ICC の換算（moxcms。CPU を見て SIMD の経路を選ぶ）と std の累乗の関数を
//! 使うので、環境で値が変わりうる（決定 0007）。そこで、色空間の系統を見分け、四則演算だけで色を作る。
//! 系統は hayro-interpret が公開しないので、色空間の `Debug` の表記の頭で見分ける。表記は hayro-interpret の版に
//! 結びつくので、版を固定し（`Cargo.toml`）、系統ごとの試験で確かめる。

use core::fmt::{self, Write as _};

use hayro_interpret::color::{Color, ColorSpace};

/// 色空間の系統（8.6）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Family {
    DeviceGray,
    DeviceRgb,
    DeviceCmyk,
    CalGray,
    CalRgb,
    IccBased,
    Lab,
    Indexed,
    Separation,
    DeviceN,
    Pattern,
    Unknown,
}

impl Family {
    /// 色空間の系統。`Debug` の表記の頭（`ColorSpace(DeviceRgb(…` など）で見分ける
    pub(crate) fn of(space: &ColorSpace) -> Self {
        let mut head = Head::default();
        // 頭の分を書いたら、残りは書かずに止める
        let _ = write!(head, "{space:?}");
        let name = head.text().strip_prefix("ColorSpace(").unwrap_or_default();
        let name = name
            .split(|c: char| !c.is_ascii_alphanumeric())
            .next()
            .unwrap_or_default();
        match name {
            "DeviceGray" => Family::DeviceGray,
            "DeviceRgb" => Family::DeviceRgb,
            "DeviceCmyk" => Family::DeviceCmyk,
            "CalGray" => Family::CalGray,
            "CalRgb" => Family::CalRgb,
            "ICCBased" => Family::IccBased,
            "Lab" => Family::Lab,
            "Indexed" => Family::Indexed,
            "Separation" => Family::Separation,
            "DeviceN" => Family::DeviceN,
            "Pattern" => Family::Pattern,
            _ => Family::Unknown,
        }
    }

    /// 色空間の系統の、PDF の名前（8.6.3〜8.6.6）
    pub(crate) fn pdf_name(self) -> &'static str {
        match self {
            Family::DeviceGray => "DeviceGray",
            Family::DeviceRgb => "DeviceRGB",
            Family::DeviceCmyk => "DeviceCMYK",
            Family::CalGray => "CalGray",
            Family::CalRgb => "CalRGB",
            Family::IccBased => "ICCBased",
            Family::Lab => "Lab",
            Family::Indexed => "Indexed",
            Family::Separation => "Separation",
            Family::DeviceN => "DeviceN",
            Family::Pattern => "Pattern",
            Family::Unknown => "",
        }
    }
}

/// 書いた文字列の頭だけを持ち、それより後ろは書かせない
#[derive(Default)]
struct Head {
    bytes: [u8; 32],
    len: usize,
}

impl Head {
    fn text(&self) -> &str {
        self.bytes
            .get(..self.len)
            .and_then(|bytes| core::str::from_utf8(bytes).ok())
            .unwrap_or_default()
    }
}

impl fmt::Write for Head {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for &byte in s.as_bytes() {
            let Some(slot) = self.bytes.get_mut(self.len) else {
                return Err(fmt::Error);
            };
            *slot = byte;
            self.len += 1;
        }
        Ok(())
    }
}

/// 図面モデルに写した色
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mapped {
    /// ファイルが持つ RGB（DeviceGray・DeviceRGB と、sRGB の ICC プロファイル）
    Exact([u8; 3]),
    /// 四則演算で RGB にした近似。系統の PDF の名前を添える
    Approximated([u8; 3], &'static str),
    /// RGB にしない系統。系統の PDF の名前を添える
    Dropped(&'static str),
}

/// 色と不透明度（0 は透明、255 は不透明）
pub(crate) fn map(color: &Color) -> (Mapped, u8) {
    let alpha = color.to_rgba().to_rgba8()[3];
    let family = Family::of(color.color_space());
    let c = color.components();
    let component = |index: usize| unit(c.get(index).copied().unwrap_or(0.0));
    let gray = || {
        let g = to_byte(component(0));
        [g, g, g]
    };
    let rgb = || [0, 1, 2].map(|index| to_byte(component(index)));
    let cmyk = || cmyk_to_rgb([component(0), component(1), component(2), component(3)]);
    let mapped = match family {
        Family::DeviceGray => Mapped::Exact(gray()),
        Family::DeviceRgb => Mapped::Exact(rgb()),
        Family::DeviceCmyk => Mapped::Approximated(cmyk(), family.pdf_name()),
        Family::CalGray => Mapped::Approximated(gray(), family.pdf_name()),
        Family::CalRgb => Mapped::Approximated(rgb(), family.pdf_name()),
        // ICC プロファイルの代わりに、成分の数の装置の色空間として読む（8.6.5.5 の Alternate の既定と同じ）
        Family::IccBased => match c.len() {
            1 => Mapped::Approximated(gray(), family.pdf_name()),
            3 => Mapped::Approximated(rgb(), family.pdf_name()),
            4 => Mapped::Approximated(cmyk(), family.pdf_name()),
            _ => Mapped::Dropped(family.pdf_name()),
        },
        Family::Lab
        | Family::Indexed
        | Family::Separation
        | Family::DeviceN
        | Family::Pattern
        | Family::Unknown => Mapped::Dropped(family.pdf_name()),
    };
    (mapped, alpha)
}

/// 0〜1 に収める。NaN は 0
fn unit(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

/// 0〜1 の値を 0〜255 にする（四捨五入）
fn to_byte(value: f32) -> u8 {
    (value * 255.0 + 0.5) as u8
}

/// DeviceCMYK から DeviceRGB への、ISO 32000-2 の 10 章の換算（赤 = 1 − min(1, シアン + 黒) など）
fn cmyk_to_rgb([c, m, y, k]: [f32; 4]) -> [u8; 3] {
    [c, m, y].map(|value| to_byte(1.0 - (value + k).min(1.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_cmyk_by_the_specification() {
        assert_eq!(cmyk_to_rgb([0.0, 0.0, 0.0, 0.0]), [255, 255, 255]);
        assert_eq!(cmyk_to_rgb([1.0, 0.0, 0.0, 0.0]), [0, 255, 255]);
        assert_eq!(cmyk_to_rgb([0.0, 0.0, 0.0, 1.0]), [0, 0, 0]);
        assert_eq!(cmyk_to_rgb([0.5, 0.25, 0.0, 0.25]), [64, 128, 191]);
    }

    #[test]
    fn head_keeps_only_the_start() {
        let mut head = Head::default();
        assert!(write!(head, "{}", "x".repeat(40)).is_err());
        assert_eq!(head.text().len(), 32);
    }
}
