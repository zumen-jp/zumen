//! ページの大きさと向き、ページラベル。

use std::collections::BTreeSet;

use hayro_syntax::object::array::ArrayIter;
use hayro_syntax::object::{
    Array, Dict, MaybeRef, Name, Object, String as PdfString, dict_or_stream,
};
use hayro_syntax::page::{A4, Page, Rotation};
use kurbo::Affine;
use zumen_base::{Kind, Stop};
use zumen_sdk::Cx;

use crate::strings;
use crate::walk::{REFERENCE_BYTES, Reads, lookup_work};

/// 1 pt（既定のユーザー空間の 1 単位）の mm
pub(crate) const MM_PER_PT: f64 = 25.4 / 72.0;

/// これより短い辺（pt、単精度で比べる）は、大きさがないとみなす。hayro-syntax の `SCALAR_NEARLY_ZERO` と同じ
const NEARLY_ZERO: f32 = 1.0 / 4096.0;

/// ページラベルの数の木（7.9.7）をたどる深さの上限
const MAX_TREE_DEPTH: u32 = 32;

/// 文字で数えるページラベル（A〜Z、AA〜ZZ …）で、同じ文字を並べる数の上限。超えたら数字で書く
const MAX_LETTERS: u64 = 64;

/// ページラベルの範囲の 1 つのメモリ（[`Range`] と、配列を伸ばす間の写し）
const RANGE_BYTES: u64 = 256;

/// ページラベルの 1 つのメモリ。前に付ける文字列は、1 バイトが UTF-8 で 3 バイトまでになり、ラベルは図面モデルに
/// 写すので、前に付ける文字列の 1 バイトを 6 バイトと数え、数の文字列の分を足す
const LABEL_BYTES_PER_PREFIX: u64 = 6;
const LABEL_BYTES: u64 = 256;

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
    /// 90 度ずつ）、単位の大きさは `UserUnit`（7.7.3.3 の表）。メディアボックスのないページと、大きさのない範囲は
    /// A4（210 × 297 mm）、1 pt より短い辺は 1 pt とみなす（推定。hayro-syntax の `Page::base_dimensions` と同じ
    /// 決め方で、値は単精度に丸めずに使う）
    pub(crate) fn of(page: &Page<'_>) -> Self {
        let crop = page.intersected_crop_box();
        let (width, height) = (crop.width(), crop.height());
        let empty = |length: f64| (length as f32).abs() <= NEARLY_ZERO;
        let (width, height) = if empty(width) || empty(height) {
            (A4.width(), A4.height())
        } else {
            (width.max(1.0), height.max(1.0))
        };
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

/// ページラベルの範囲（12.4.2 の表）。hayro-syntax の辞書は鍵の数に比例するメモリを持つので、持たずに値だけを持つ
struct Range<'a> {
    /// 開始ページ（0 から）
    start: i64,
    /// 前に付ける文字列（`P`）
    prefix: Option<PdfString<'a>>,
    /// 始めの番号（`St`、既定 1）
    first: i64,
    /// 数の書き方（`S`）。1 バイトの名前だけを持つ
    style: Option<u8>,
}

impl<'a> Range<'a> {
    /// 範囲の辞書から値を読む。読むたびに参照を解くので、文字列と名前の大きさを `work` に数える
    fn read(start: i64, range: &Dict<'a>, cx: &mut Cx<'_>) -> Result<Self, Stop> {
        let prefix = range.get::<PdfString<'a>>(b"P");
        let style = range.get::<Name<'a>>(b"S");
        let length = prefix.as_ref().map_or(0, |prefix| prefix.as_bytes().len()) as u64;
        cx.check(
            Kind::Work,
            lookup_work(
                prefix
                    .as_ref()
                    .map_or(b"".as_slice(), |prefix| prefix.as_bytes()),
            )
            .saturating_add(lookup_work(
                style
                    .as_ref()
                    .map_or(b"".as_slice(), |style| style.as_ref()),
            )),
        )?;
        cx.check(Kind::DecodedBytes, RANGE_BYTES.saturating_add(length))?;
        Ok(Self {
            start,
            prefix,
            first: range
                .get::<i64>(b"St")
                .filter(|first| *first >= 1)
                .unwrap_or(1),
            style: style.as_deref().and_then(|style| match style {
                [byte] => Some(*byte),
                _ => None,
            }),
        })
    }
}

/// 各ページのページラベル（12.4.2）。カタログの `PageLabels` の数の木から、ページの番号（0 から）以下でいちばん
/// 大きい開始ページの範囲の書き方で作る。ラベルのないページと、空のラベルは `None`。木の節と項目ごとに予算の
/// `work` を 1 と、読み直すバイト数（hayro-syntax は、参照を解くたびに読み直す）を使い、ラベルのメモリを
/// `DecodedBytes` に数える。深さは [`MAX_TREE_DEPTH`] まで、同じ参照は 2 度解かない（直接書いた節は、それを含む節を
/// たどるときに 1 度だけ読む）。範囲は二分探索で引く
pub(crate) fn labels(
    reads: Reads<'_>,
    catalog: Option<&Dict<'_>>,
    count: usize,
    cx: &mut Cx<'_>,
) -> Result<Vec<Option<String>>, Stop> {
    let mut ranges: Vec<Range<'_>> = Vec::new();
    if let Some(catalog) = catalog
        && let Some(tree) = catalog.get::<Dict<'_>>(b"PageLabels")
    {
        let mut seen: BTreeSet<_> = catalog
            .get_ref(b"PageLabels")
            .map(|id| (id.obj_number, id.gen_number))
            .into_iter()
            .collect();
        let mut open: Vec<ArrayIter<'_>> = Vec::new();
        cx.check(Kind::Work, lookup_work(tree.data()))?;
        node(reads, &tree, &mut ranges, &mut open, cx)?;
        while let Some(kids) = open.last_mut() {
            let Some(kid) = kids.next() else {
                open.pop();
                continue;
            };
            cx.check(Kind::Work, 1)?;
            let kid = match kid {
                MaybeRef::Ref(id) => {
                    if !seen.insert((id.obj_number, id.gen_number)) {
                        continue;
                    }
                    cx.check(Kind::DecodedBytes, REFERENCE_BYTES)?;
                    reads.xref.get::<Dict<'_>>(id.into())
                }
                MaybeRef::NotRef(object) => dict_or_stream(&object).map(|(kid, _)| kid.clone()),
            };
            // 辞書でない項目から後ろは、読まない
            let Some(kid) = kid else {
                open.pop();
                continue;
            };
            cx.check(Kind::Work, lookup_work(kid.data()))?;
            if open.len() as u32 <= MAX_TREE_DEPTH {
                node(reads, &kid, &mut ranges, &mut open, cx)?;
            }
        }
    }
    // 開始ページの順に並べる（安定な並べ替え）。同じ開始ページが重なれば、木の順で後のものを使う（推定）
    ranges.sort_by_key(|range| range.start);
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        let index = i64::try_from(index).unwrap_or(i64::MAX);
        // 開始ページが `index` 以下の範囲のうち、最後のもの
        let found = ranges
            .partition_point(|range| range.start <= index)
            .checked_sub(1)
            .and_then(|at| ranges.get(at));
        let prefix = found
            .and_then(|range| range.prefix.as_ref())
            .map_or(b"".as_slice(), |prefix| prefix.as_bytes());
        cx.check(Kind::Work, lookup_work(prefix).saturating_add(1))?;
        cx.check(
            Kind::DecodedBytes,
            (prefix.len() as u64)
                .saturating_mul(LABEL_BYTES_PER_PREFIX)
                .saturating_add(LABEL_BYTES),
        )?;
        out.push(
            found
                .map(|range| label(range, index.saturating_sub(range.start)))
                .filter(|label| !label.is_empty()),
        );
    }
    Ok(out)
}

/// 数の木の節（7.9.7）を読む。範囲を `ranges` に足し、子の配列を `open` に積む（木の順にたどる）
fn node<'a>(
    reads: Reads<'_>,
    node: &Dict<'a>,
    ranges: &mut Vec<Range<'a>>,
    open: &mut Vec<ArrayIter<'a>>,
    cx: &mut Cx<'_>,
) -> Result<(), Stop> {
    cx.check(
        Kind::Work,
        reads
            .value_work(node, b"Nums")
            .saturating_add(reads.value_work(node, b"Kids")),
    )?;
    if let Some(nums) = node.get::<Array<'a>>(b"Nums") {
        cx.check(Kind::Work, lookup_work(nums.data()))?;
        let mut items = nums.iter::<Object<'a>>();
        while let (Some(key), Some(value)) = (items.next(), items.next()) {
            cx.check(
                Kind::Work,
                reads
                    .read_work(&key)
                    .saturating_add(reads.reread_work(&value))
                    .saturating_add(1),
            )?;
            if let (Object::Number(key), Object::Dict(range)) = (key, value) {
                ranges.push(Range::read(key.as_i64(), &range, cx)?);
            }
        }
    }
    if let Some(kids) = node.get::<Array<'a>>(b"Kids") {
        cx.check(Kind::Work, lookup_work(kids.data()))?;
        open.push(kids.raw_iter());
    }
    Ok(())
}

/// 範囲の書き方（12.4.2 の表）で、範囲の先頭から `offset` 番目のページのラベル。`St` は始めの番号（既定 1）、
/// `S` は数の書き方、`P` は前に付ける文字列
fn label(range: &Range<'_>, offset: i64) -> String {
    let prefix = range
        .prefix
        .as_ref()
        .map(|prefix| strings::text(prefix.as_bytes()))
        .unwrap_or_default();
    let number = u64::try_from(range.first.saturating_add(offset))
        .unwrap_or(1)
        .max(1);
    let digits = match range.style {
        Some(b'D') => number.to_string(),
        Some(b'R') => roman(number),
        Some(b'r') => roman(number).to_lowercase(),
        Some(b'A') => letters(number, b'A'),
        Some(b'a') => letters(number, b'a'),
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
