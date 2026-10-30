//! 縦書き用の字形への置き換えを読む（決定 0187）。`GSUB` の `vrt2` の機能（なければ `vert`）が指す
//! lookup のうち、single substitution（lookup type 1 の形式 1・2。extension の type 7 を通したものを
//! 含む）を読む。
//!
//! 仕様は OpenType の仕様の GSUB と OpenType Layout Common Table Formats の節に従う。lookup の数と
//! 副表の数は、カーニングと同じ上限（[`super::kern`]）で抑え、超える表は読まない（置き換えない）。

use super::kern::{coverage, feature_lookups, from, subtables};
use super::u16_at;

/// 読んだ置き換え
#[derive(Default)]
pub(super) struct Substitution<'a> {
    /// lookup ごとの single substitution の副表（lookup の順）
    lookups: Vec<Vec<&'a [u8]>>,
}

impl<'a> Substitution<'a> {
    /// `vrt2` の機能があればその lookup を、なければ `vert` の機能の lookup を使う。`vrt2` は `vert` を
    /// 含めて作られた機能なので、両方は当てない（OpenType の仕様の Feature Tags の節）
    pub(super) fn read(gsub: Option<&'a [u8]>) -> Self {
        let lookups = gsub.and_then(vertical).unwrap_or_default();
        Self { lookups }
    }

    /// 字形を置き換える。lookup の順に当て、lookup の中では最初に当たる副表を使う。前の lookup で
    /// 置き換えた字形を、次の lookup に渡す。どれにも当たらなければ元の字形
    pub(super) fn apply(&self, glyph: u16) -> u16 {
        self.lookups.iter().fold(glyph, |glyph, subtables| {
            subtables
                .iter()
                .find_map(|subtable| single(subtable, glyph))
                .unwrap_or(glyph)
        })
    }
}

fn vertical(table: &[u8]) -> Option<Vec<Vec<&[u8]>>> {
    let (indices, list) = match feature_lookups(table, b"vrt2")? {
        (indices, list) if !indices.is_empty() => (indices, list),
        _ => feature_lookups(table, b"vert")?,
    };
    subtables(list, indices, 1, 7)
}

/// single substitution の副表を当てる。字形が副表に入らなければ `None`
fn single(subtable: &[u8], glyph: u16) -> Option<u16> {
    // 見出し: substFormat（0）・coverageOffset（2）
    let covered = coverage(from(subtable, u16_at(subtable, 2)?)?, glyph)?;
    match u16_at(subtable, 0)? {
        // 形式 1: deltaGlyphID（4）。足した値は 65536 で割った余り
        1 => Some(glyph.wrapping_add(u16_at(subtable, 4)?)),
        // 形式 2: glyphCount（4）・substituteGlyphIDs（6）
        2 => {
            if covered >= u16_at(subtable, 4)? {
                return None;
            }
            u16_at(subtable, 6 + usize::from(covered) * 2)
        }
        _ => None,
    }
}
