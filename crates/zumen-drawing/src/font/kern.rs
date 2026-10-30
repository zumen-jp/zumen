//! カーニングの表を読む（決定 0186）。`GPOS` の `kern` の機能の pair adjustment（lookup type 2 の
//! 形式 1・2。extension の type 9 を通したものを含む）を読み、なければ `kern` の表の形式 0 を読む。
//!
//! 仕様は OpenType の仕様の GPOS・kern・OpenType Layout Common Table Formats の節に従う。
//! 文字の組ごとの作業は、lookup と副表の数と二分探索で抑える。副表の数は [`SUBTABLES`] で抑え、
//! 超える表は読まない（カーニングをしない）。
//!
//! 縦書きのカーニング（決定 0189）は、`GPOS` の `vkrn` の機能の pair adjustment の YAdvance を読み、
//! なければ `kern` の表の縦の向きの形式 0 を読む。

use core::cmp::Ordering;
use std::collections::BTreeSet;

use super::{i16_at, slice, u16_at, u32_at};

/// 読む副表の数の上限。表の位置は互いに指し合えるので、ファイルの大きさでは抑えられない
const SUBTABLES: usize = 1024;
/// GPOS の機能から lookup の番号を読む数の上限
const READS: u32 = 1 << 16;

/// 読んだカーニングの表
#[derive(Default)]
pub(super) struct Kerning<'a> {
    /// GPOS の lookup ごとの、pair adjustment の副表（lookup の順）
    lookups: Vec<Vec<&'a [u8]>>,
    /// GPOS がなければ、`kern` の形式 0 の副表
    kern: Vec<Pairs<'a>>,
    /// 縦書きのカーニング。GPOS の値は YAdvance を読む
    vertical: bool,
}

/// `kern` の形式 0 の組の並び
struct Pairs<'a> {
    pairs: &'a [u8],
    count: u16,
    /// 前の副表の値を置き換える（coverage の override）
    replace: bool,
}

impl<'a> Kerning<'a> {
    /// GPOS の pair adjustment があれば、`kern` の表は使わない
    pub(super) fn read(gpos: Option<&'a [u8]>, kern: Option<&'a [u8]>) -> Self {
        Self::from_tables(gpos, kern, false)
    }

    /// 縦書きのカーニング。GPOS の `vkrn` の pair adjustment があれば、`kern` の表は使わない
    pub(super) fn read_vertical(gpos: Option<&'a [u8]>, kern: Option<&'a [u8]>) -> Self {
        Self::from_tables(gpos, kern, true)
    }

    fn from_tables(gpos: Option<&'a [u8]>, kern: Option<&'a [u8]>, vertical: bool) -> Self {
        let tag = if vertical { b"vkrn" } else { b"kern" };
        let lookups = gpos
            .and_then(|table| lookups(table, tag))
            .unwrap_or_default();
        let kern = if lookups.is_empty() {
            kern.and_then(|table| kern_pairs(table, vertical))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        Self {
            lookups,
            kern,
            vertical,
        }
    }

    /// 字形 `left` の後に `right` が並ぶときに足す送り（字形の単位）。縦書きでは縦の送りに足す
    pub(super) fn value(&self, left: u16, right: u16) -> i32 {
        if !self.lookups.is_empty() {
            // lookup ごとに、最初に当たる副表を使い、lookup の間では足す
            return self
                .lookups
                .iter()
                .filter_map(|subtables| {
                    subtables
                        .iter()
                        .find_map(|subtable| adjust(subtable, left, right, self.vertical))
                })
                .map(i32::from)
                .sum();
        }
        let key = u32::from(left) << 16 | u32::from(right);
        self.kern.iter().fold(0, |total, table| {
            let found = find(usize::from(table.count), |i| {
                Some(u32_at(table.pairs, i * 6)?.cmp(&key))
            });
            match found.and_then(|i| i16_at(table.pairs, i * 6 + 4)) {
                Some(value) if table.replace => i32::from(value),
                Some(value) => total + i32::from(value),
                None => total,
            }
        })
    }
}

/// `data` の `at` からの残り
pub(super) fn from(data: &[u8], at: impl Into<u64>) -> Option<&[u8]> {
    data.get(usize::try_from(at.into()).ok()?..)
}

/// GPOS の機能 `tag`（`kern`・`vkrn`）の lookup のうち、pair adjustment の副表を lookup の順に集める。
/// 文字の並びの言語は分からないので、どの用字系の機能の lookup も使う
fn lookups<'a>(table: &'a [u8], tag: &[u8; 4]) -> Option<Vec<Vec<&'a [u8]>>> {
    let (indices, list) = feature_lookups(table, tag)?;
    subtables(list, indices, 2, 9)
}

/// GSUB・GPOS の、機能 `tag` が指す lookup の番号（どの用字系のものも）と LookupList
/// （Common Table Formats の Feature List・Lookup List の節）
pub(super) fn feature_lookups<'a>(
    table: &'a [u8],
    tag: &[u8; 4],
) -> Option<(BTreeSet<u16>, &'a [u8])> {
    // 見出し: majorVersion（0）・featureListOffset（6）・lookupListOffset（8）
    if u16_at(table, 0)? != 1 {
        return None;
    }
    let features = from(table, u16_at(table, 6)?)?;
    let list = from(table, u16_at(table, 8)?)?;
    let mut reads = 0u32;
    let mut indices = BTreeSet::new();
    // FeatureList: featureCount（0）、FeatureRecord（tag・featureOffset、6 バイト）
    for i in 0..usize::from(u16_at(features, 0)?) {
        let record = 2 + i * 6;
        if features.get(record..record + 4)? != tag {
            continue;
        }
        // Feature: featureParamsOffset（0）・lookupIndexCount（2）・lookupListIndices（4）
        let feature = from(features, u16_at(features, record + 4)?)?;
        for j in 0..usize::from(u16_at(feature, 2)?) {
            reads += 1;
            if reads > READS {
                return None;
            }
            indices.insert(u16_at(feature, 4 + j * 2)?);
        }
    }
    Some((indices, list))
}

/// lookup の番号の順に、種類 `kind`（extension の種類 `extension` を通したものを含む）の形式 1・2 の
/// 副表を集める。副表のない lookup は除く。副表が [`SUBTABLES`] を超えれば `None`
pub(super) fn subtables(
    list: &[u8],
    indices: BTreeSet<u16>,
    kind: u16,
    extension: u16,
) -> Option<Vec<Vec<&[u8]>>> {
    // LookupList: lookupCount（0）・lookupOffsets（2）
    let count = u16_at(list, 0)?;
    let (mut lookups, mut total) = (Vec::new(), 0);
    for index in indices {
        if index >= count {
            return None;
        }
        // Lookup: lookupType（0）・lookupFlag（2）・subTableCount（4）・subtableOffsets（6）
        let lookup = from(list, u16_at(list, 2 + usize::from(index) * 2)?)?;
        let found = u16_at(lookup, 0)?;
        if found != kind && found != extension {
            continue;
        }
        let mut subtables = Vec::new();
        for k in 0..usize::from(u16_at(lookup, 4)?) {
            let mut subtable = from(lookup, u16_at(lookup, 6 + k * 2)?)?;
            if found == extension {
                // Extension: format（0）・extensionLookupType（2）・extensionOffset（4）
                if u16_at(subtable, 0)? != 1 || u16_at(subtable, 2)? != kind {
                    continue;
                }
                subtable = from(subtable, u32_at(subtable, 4)?)?;
            }
            if !matches!(u16_at(subtable, 0), Some(1 | 2)) {
                continue;
            }
            total += 1;
            if total > SUBTABLES {
                return None;
            }
            subtables.push(subtable);
        }
        if !subtables.is_empty() {
            lookups.push(subtables);
        }
    }
    Some(lookups)
}

/// pair adjustment の副表を当てる。最初の字形が副表に入らなければ `None`。値は最初の字形の
/// XAdvance（縦書きでは YAdvance）だけを読み、2 番目の字形の値と、位置の値（XPlacement など）は読まない
fn adjust(subtable: &[u8], first: u16, second: u16, vertical: bool) -> Option<i16> {
    // 見出し: posFormat（0）・coverageOffset（2）・valueFormat1（4）・valueFormat2（6）
    let covered = coverage(from(subtable, u16_at(subtable, 2)?)?, first)?;
    let (format1, format2) = (u16_at(subtable, 4)?, u16_at(subtable, 6)?);
    let (size1, size2) = (record_size(format1), record_size(format2));
    match u16_at(subtable, 0)? {
        // 形式 1: pairSetCount（8）・pairSetOffsets（10）。PairSet: pairValueCount（0）と
        // PairValueRecord（secondGlyph・valueRecord1・valueRecord2）の並び
        1 => {
            if covered >= u16_at(subtable, 8)? {
                return None;
            }
            let set = from(subtable, u16_at(subtable, 10 + usize::from(covered) * 2)?)?;
            let stride = 2 + size1 + size2;
            let i = find(usize::from(u16_at(set, 0)?), |i| {
                Some(u16_at(set, 2 + i * stride)?.cmp(&second))
            })?;
            advance(set, 2 + i * stride + 2, format1, vertical)
        }
        // 形式 2: classDef1Offset（8）・classDef2Offset（10）・class1Count（12）・class2Count（14）、
        // Class1Record の並び（16）
        2 => {
            let class1 = class(from(subtable, u16_at(subtable, 8)?)?, first)?;
            let class2 = class(from(subtable, u16_at(subtable, 10)?)?, second)?;
            let (count1, count2) = (u16_at(subtable, 12)?, u16_at(subtable, 14)?);
            if class1 >= count1 || class2 >= count2 {
                return None;
            }
            let record = (u64::from(class1) * u64::from(count2) + u64::from(class2))
                * (size1 + size2) as u64;
            let at = usize::try_from(16 + record).ok()?;
            advance(subtable, at, format1, vertical)
        }
        _ => None,
    }
}

/// ValueRecord の大きさ。valueFormat の下位 8 ビットの立った数 × 2 バイト
fn record_size(format: u16) -> usize {
    (format & 0xFF).count_ones() as usize * 2
}

/// ValueRecord の XAdvance（valueFormat のビット 2）か YAdvance（ビット 3）。前のビットの値の後に並ぶ。
/// YAdvance は字形の座標では上が正だが、縦書きの送りは下へ進むので、OpenType の実装の慣行どおり
/// 値をそのまま縦の送りに足す（負なら詰める）
fn advance(data: &[u8], at: usize, format: u16, vertical: bool) -> Option<i16> {
    let bit: u16 = if vertical { 0x8 } else { 0x4 };
    if format & bit == 0 {
        return Some(0);
    }
    i16_at(data, at + (format & (bit - 1)).count_ones() as usize * 2)
}

/// Coverage の番号（Common Table Formats の Coverage の節）
pub(super) fn coverage(table: &[u8], glyph: u16) -> Option<u16> {
    let count = usize::from(u16_at(table, 2)?);
    match u16_at(table, 0)? {
        // 形式 1: glyphCount（2）・glyphArray（4）
        1 => {
            let i = find(count, |i| Some(u16_at(table, 4 + i * 2)?.cmp(&glyph)))?;
            u16::try_from(i).ok()
        }
        // 形式 2: rangeCount（2）・RangeRecord（startGlyphID・endGlyphID・startCoverageIndex）
        2 => {
            let i = find(count, |i| range(table, 4 + i * 6, glyph))?;
            let [start, index] = [u16_at(table, 4 + i * 6)?, u16_at(table, 4 + i * 6 + 4)?];
            u16::try_from(u32::from(index) + u32::from(glyph - start)).ok()
        }
        _ => None,
    }
}

/// ClassDef の組（Common Table Formats の Class Definition の節）。どの組にも入らない字形は 0
fn class(table: &[u8], glyph: u16) -> Option<u16> {
    match u16_at(table, 0)? {
        // 形式 1: startGlyphID（2）・glyphCount（4）・classValueArray（6）
        1 => {
            let (start, count) = (u16_at(table, 2)?, u16_at(table, 4)?);
            match glyph.checked_sub(start) {
                Some(i) if i < count => u16_at(table, 6 + usize::from(i) * 2),
                _ => Some(0),
            }
        }
        // 形式 2: classRangeCount（2）・ClassRangeRecord（startGlyphID・endGlyphID・class）
        2 => {
            let count = usize::from(u16_at(table, 2)?);
            match find(count, |i| range(table, 4 + i * 6, glyph)) {
                Some(i) => u16_at(table, 4 + i * 6 + 4),
                None => Some(0),
            }
        }
        _ => None,
    }
}

/// `at` の区間（start・end）と字形の前後
fn range(table: &[u8], at: usize, glyph: u16) -> Option<Ordering> {
    let (start, end) = (u16_at(table, at)?, u16_at(table, at + 2)?);
    Some(if end < glyph {
        Ordering::Less
    } else if start > glyph {
        Ordering::Greater
    } else {
        Ordering::Equal
    })
}

/// 昇順に並んだ `count` 個の記録を二分探索する。`order` は、記録が探す値より前なら `Less` を返す。
/// 読めない記録があれば見つからないとする
pub(super) fn find(count: usize, order: impl Fn(usize) -> Option<Ordering>) -> Option<usize> {
    let (mut low, mut high) = (0, count);
    while low < high {
        let middle = low + (high - low) / 2;
        match order(middle)? {
            Ordering::Less => low = middle + 1,
            Ordering::Greater => high = middle,
            Ordering::Equal => return Some(middle),
        }
    }
    None
}

/// `kern` の表（Microsoft の版 0）の、横書き（`vertical` なら縦書き）の形式 0 の副表を集める。
/// Apple の版 1 は読まない
fn kern_pairs(table: &[u8], vertical: bool) -> Option<Vec<Pairs<'_>>> {
    // 見出し: version（0）・nTables（2）。副表: version（0）・length（2）・coverage（4）
    if u16_at(table, 0)? != 0 {
        return None;
    }
    let mut out = Vec::new();
    let mut at = 4u64;
    for _ in 0..u16_at(table, 2)? {
        let Some(subtable) = from(table, at) else {
            break;
        };
        let (Some(length), Some(coverage)) = (u16_at(subtable, 2), u16_at(subtable, 4)) else {
            break;
        };
        // 形式（上位 8 ビット）が 0 で、横書き（ビット 0。縦書きでは立たない）で、最小値（ビット 1）・
        // 交差の向き（ビット 2）でない
        if coverage >> 8 == 0 && coverage & 0x7 == u16::from(!vertical) {
            // 形式 0: nPairs（6）・searchRange・entrySelector・rangeShift、組（14 から 6 バイトずつ）
            let count = u16_at(subtable, 6)?;
            let pairs = slice(subtable, 14, u64::from(count) * 6)?;
            if out.len() >= SUBTABLES {
                return None;
            }
            out.push(Pairs {
                pairs,
                count,
                replace: coverage & 0x8 != 0,
            });
        }
        if length < 6 {
            break;
        }
        at += u64::from(length);
    }
    Some(out)
}
