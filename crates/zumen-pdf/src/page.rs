//! ページの大きさと向き、ページラベル。

use std::collections::BTreeSet;

use hayro_interpret::hayro_syntax::object::{Array, Dict, Name, Object, String as PdfString};
use hayro_interpret::hayro_syntax::page::{Page, Rotation};
use hayro_interpret::kurbo::Affine;
use zumen_base::{Kind, Stop};
use zumen_sdk::Cx;

use crate::strings;

/// 1 pt（既定のユーザー空間の 1 単位）の mm
pub(crate) const MM_PER_PT: f64 = 25.4 / 72.0;

/// 大きさのないページの大きさ（A4、pt）。hayro と同じ
const A4: (f64, f64) = (595.0, 842.0);

/// ページラベルの数の木（7.9.7）をたどる深さの上限
const MAX_TREE_DEPTH: u32 = 32;

/// 文字で数えるページラベル（A〜Z、AA〜ZZ …）で、同じ文字を並べる数の上限。超えたら数字で書く
const MAX_LETTERS: u64 = 64;

/// ページの大きさと、既定のユーザー空間から用紙の mm への変換
#[derive(Clone, Copy, Debug)]
pub(crate) struct Geometry {
    /// 回転を済ませた用紙の幅（mm）
    pub width: f64,
    /// 回転を済ませた用紙の高さ（mm）
    pub height: f64,
    /// 既定のユーザー空間 → 用紙の mm。クロップボックスの左下が原点、Y が上、ページの回転を済ませた向き
    pub user_space: Affine,
}

impl Geometry {
    /// 見える範囲はクロップボックスとメディアボックスの重なり（7.7.3.3、14.11.2）、向きは `Rotate`（時計回りに
    /// 90 度ずつ）、単位の大きさは `UserUnit`（7.7.3.3 の表）。大きさのない範囲は A4、1 pt より短い辺は 1 pt
    /// とみなす（hayro と同じ、推定）
    pub(crate) fn of(page: &Page<'_>) -> Self {
        let crop = page.intersected_crop_box();
        let (mut width, mut height) = (crop.x1 - crop.x0, crop.y1 - crop.y0);
        if !(width.abs() >= 1e-6 && height.abs() >= 1e-6) {
            (width, height) = A4;
        }
        let (width, height) = (width.max(1.0), height.max(1.0));
        let user_unit = page
            .raw()
            .get::<f64>(b"UserUnit")
            .filter(|unit| unit.is_finite() && *unit > 0.0)
            .unwrap_or(1.0);
        let scale = MM_PER_PT * user_unit;
        // クロップボックスの左下を原点にしてから、表示の向きに回す（Y は上のまま）
        let origin = Affine::translate((-crop.x0, -crop.y0));
        let (rotation, (shown_width, shown_height)) = match page.rotation() {
            Rotation::None => (Affine::IDENTITY, (width, height)),
            // 時計回りに 90 度: (u, v) → (v, w − u)
            Rotation::Horizontal => (
                Affine::new([0.0, -1.0, 1.0, 0.0, 0.0, width]),
                (height, width),
            ),
            Rotation::Flipped => (
                Affine::new([-1.0, 0.0, 0.0, -1.0, width, height]),
                (width, height),
            ),
            // 時計回りに 270 度: (u, v) → (h − v, u)
            Rotation::FlippedHorizontal => (
                Affine::new([0.0, 1.0, -1.0, 0.0, height, 0.0]),
                (height, width),
            ),
        };
        Self {
            width: shown_width * scale,
            height: shown_height * scale,
            user_space: Affine::scale(scale) * rotation * origin,
        }
    }
}

/// 各ページのページラベル（12.4.2）。カタログの `PageLabels` の数の木から、ページの番号（0 から）以下でいちばん
/// 大きい開始ページの範囲の書き方で作る。ラベルのないページと、空のラベルは `None`。木の節ごとに予算の `work` を
/// 1 使い、深さは [`MAX_TREE_DEPTH`] まで、同じ節は 2 度たどらない
pub(crate) fn labels(
    catalog: Option<&Dict<'_>>,
    count: usize,
    cx: &mut Cx<'_>,
) -> Result<Vec<Option<String>>, Stop> {
    let mut ranges: Vec<(i64, Dict<'_>)> = Vec::new();
    if let Some(tree) = catalog.and_then(|catalog| catalog.get::<Dict<'_>>(b"PageLabels")) {
        let mut seen = BTreeSet::new();
        let mut stack = vec![(tree, 0u32)];
        while let Some((node, depth)) = stack.pop() {
            cx.check(Kind::Work, 1)?;
            if depth > MAX_TREE_DEPTH {
                continue;
            }
            if let Some(id) = node.obj_id()
                && !seen.insert((id.obj_number, id.gen_number))
            {
                continue;
            }
            if let Some(nums) = node.get::<Array<'_>>(b"Nums") {
                let mut items = nums.iter::<Object<'_>>();
                while let (Some(key), Some(value)) = (items.next(), items.next()) {
                    cx.check(Kind::Work, 1)?;
                    if let (Object::Number(key), Object::Dict(range)) = (key, value) {
                        ranges.push((key.as_i64(), range));
                    }
                }
            }
            if let Some(kids) = node.get::<Array<'_>>(b"Kids") {
                let kids: Vec<Dict<'_>> = kids.iter::<Dict<'_>>().collect();
                // 後ろから積むので、木の順にたどる
                for kid in kids.into_iter().rev() {
                    cx.check(Kind::Work, 1)?;
                    stack.push((kid, depth + 1));
                }
            }
        }
    }
    // 開始ページの順に並べる（安定な並べ替え）。同じ開始ページが重なれば、木の順で後のものを使う（推定）
    ranges.sort_by_key(|(start, _)| *start);
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        cx.check(Kind::Work, 1)?;
        let index = i64::try_from(index).unwrap_or(i64::MAX);
        let found = ranges
            .iter()
            .rev()
            .find(|(start, _)| *start <= index)
            .map(|(start, range)| label(range, index.saturating_sub(*start)))
            .filter(|label| !label.is_empty());
        out.push(found);
    }
    Ok(out)
}

/// 範囲の書き方（12.4.2 の表）で、範囲の先頭から `offset` 番目のページのラベル。`St` は始めの番号（既定 1）、
/// `S` は数の書き方、`P` は前に付ける文字列
fn label(range: &Dict<'_>, offset: i64) -> String {
    let prefix = range
        .get::<PdfString<'_>>(b"P")
        .map(|prefix| strings::text(prefix.as_bytes()))
        .unwrap_or_default();
    let start = range
        .get::<i64>(b"St")
        .filter(|start| *start >= 1)
        .unwrap_or(1);
    let number = u64::try_from(start.saturating_add(offset))
        .unwrap_or(1)
        .max(1);
    let style = range.get::<Name<'_>>(b"S");
    let digits = match style.as_deref() {
        Some(b"D") => number.to_string(),
        Some(b"R") => roman(number),
        Some(b"r") => roman(number).to_lowercase(),
        Some(b"A") => letters(number, b'A'),
        Some(b"a") => letters(number, b'a'),
        _ => String::new(),
    };
    format!("{prefix}{digits}")
}

/// ローマ数字（I、IV、MMXXVI …）。千の位が長くなりすぎるものは数字で書く
fn roman(mut number: u64) -> String {
    const NUMERALS: [(u64, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    if number / 1000 > MAX_LETTERS {
        return number.to_string();
    }
    let mut out = String::new();
    for (value, numeral) in NUMERALS {
        while number >= value {
            out.push_str(numeral);
            number -= value;
        }
    }
    out
}

/// A〜Z のあとは AA〜ZZ、AAA〜ZZZ …（12.4.2 の表の A と a）
fn letters(number: u64, first: u8) -> String {
    let repeat = (number - 1) / 26 + 1;
    if repeat > MAX_LETTERS {
        return number.to_string();
    }
    let letter = char::from(first + ((number - 1) % 26) as u8);
    std::iter::repeat_n(letter, repeat as usize).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_numbers_in_each_style() {
        assert_eq!(roman(1), "I");
        assert_eq!(roman(2026), "MMXXVI");
        assert_eq!(roman(49), "XLIX");
        assert_eq!(letters(1, b'A'), "A");
        assert_eq!(letters(26, b'a'), "z");
        assert_eq!(letters(27, b'A'), "AA");
        assert_eq!(letters(53, b'A'), "AAA");
        assert_eq!(letters(26 * 65, b'A'), (26 * 65).to_string());
    }
}
