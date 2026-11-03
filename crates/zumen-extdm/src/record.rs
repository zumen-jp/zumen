//! レコードのフィールドの読み方と、ヘッダーのレコードの後に続く実データのレコードの数（数値地形図データファイル仕様の
//! 「レコードの構成」と各レコードの書式）。

use std::ops::Range;

use crate::Line;

/// 2 次元座標レコードの座標の数（(8)、I7 の X・Y が 6 組）
pub(crate) const PAIRS: usize = 6;
/// 3 次元座標レコードの座標の数（(7)、I7 の X・Y・Z が 4 組）
pub(crate) const TRIPLES: usize = 4;
/// グリッドレコードと不整三角網レコードの数値の数（(11)・(12)、I7 が 12 個）
pub(crate) const VALUES: usize = 12;

/// ヘッダーのレコードのレコードタイプ。実データのレコードなら `None`
pub(crate) fn kind(record: &[u8]) -> Option<&'static [u8]> {
    Some(match record.get(..2)? {
        b"I " => b"I",
        b"M " => b"M",
        b"H " => b"H",
        b"G " => b"G",
        b"T " => b"T",
        b"E1" => b"E1",
        b"E2" => b"E2",
        b"E3" => b"E3",
        b"E4" => b"E4",
        b"E5" => b"E5",
        b"E6" => b"E6",
        b"E7" => b"E7",
        b"E8" => b"E8",
        _ => return None,
    })
}

/// カラムの `start` から `width` バイトのフィールド（1 カラム目は 0）。レコードが短ければ、ある所まで
pub(crate) fn field(record: &[u8], start: usize, width: usize) -> &[u8] {
    let end = start.saturating_add(width).min(record.len());
    record.get(start..end).unwrap_or_default()
}

/// 整数型（I）のフィールド。空白だけなら `Ok(None)`、整数でなければ `Err(())`
pub(crate) fn int(record: &[u8], start: usize, width: usize) -> Result<Option<i64>, ()> {
    let text = field(record, start, width).trim_ascii();
    if text.is_empty() {
        return Ok(None);
    }
    std::str::from_utf8(text)
        .ok()
        .and_then(|text| text.parse::<i64>().ok())
        .map(Some)
        .ok_or(())
}

/// 整数型のフィールドの数。空白か読めなければ 0（「入力値が無い場合 0」）
fn count(record: &[u8], start: usize, width: usize) -> usize {
    int(record, start, width)
        .ok()
        .flatten()
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0)
}

/// レコードを、ヘッダーのレコードと、その後に続く実データのレコードに分ける
pub(crate) fn classify(src: &[u8], ranges: Vec<Range<usize>>) -> Vec<Line> {
    let records: Vec<&[u8]> = ranges
        .iter()
        .map(|range| src.get(range.clone()).unwrap_or_default())
        .collect();
    let mut lines = Vec::with_capacity(ranges.len());
    let mut index = 0;
    while let Some(range) = ranges.get(index) {
        let record = records.get(index).copied().unwrap_or_default();
        let kind = kind(record);
        lines.push(Line {
            range: range.clone(),
            kind,
        });
        let rest = records.get(index + 1..).unwrap_or_default();
        let follows = kind
            .map_or(0, |kind| follows(kind, record, rest))
            .min(rest.len());
        for range in ranges.iter().skip(index + 1).take(follows) {
            lines.push(Line {
                range: range.clone(),
                kind: None,
            });
        }
        index += 1 + follows;
    }
    lines
}

/// ヘッダーのレコードの後に続く実データのレコードの数。`rest` はヘッダーの後のレコード
pub(crate) fn follows(kind: &[u8], record: &[u8], rest: &[&[u8]]) -> usize {
    match kind {
        // インデックスレコード (b) は図郭識別番号レコード数、(c) は使用分類コード数だけ続く
        b"I" => count(record, 37, 2).saturating_add(count(record, 39, 4)),
        // 図郭レコード (b)・(c) と、修正回数より 1 つ多い (d)・(e)・(f) の組。(f) は (d) のレコード数だけ続く
        b"M" => {
            let revisions = count(record, 65, 2).saturating_add(1);
            let mut at = 2;
            for _ in 0..revisions {
                let Some(d) = rest.get(at) else {
                    break;
                };
                at = at.saturating_add(2).saturating_add(count(d, 9, 1));
            }
            at
        }
        // 要素レコードのレコード数（当該要素が持つ実データレコード数）
        b"E1" | b"E2" | b"E3" | b"E4" | b"E5" | b"E6" | b"E7" | b"E8" => count(record, 31, 4),
        // グリッドの行数×列数の数値を、1 レコードに 12 個ずつ
        b"G" => count(record, 18, 4)
            .saturating_mul(count(record, 22, 4))
            .div_ceil(VALUES),
        // 三角形ごとに 3 点の X・Y・Z を、1 レコードに 12 個ずつ
        b"T" => count(record, 20, 6).saturating_mul(9).div_ceil(VALUES),
        _ => 0,
    }
}
