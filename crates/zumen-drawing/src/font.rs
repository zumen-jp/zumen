//! 呼び出し側が渡した TrueType・OpenType のファイルから、文字の配置（[`crate::layout`]）に要る
//! 寸法を読む（決定 0180）。
//!
//! 依存を足さず、要る表だけを OpenType の仕様に従って読む。読む表と値は次のとおり。
//!
//! - em: `head` の unitsPerEm（16〜16384）
//! - アセンダー・ディセンダー: `OS/2`（版 4 以降）の fsSelection の USE_TYPO_METRICS（ビット 7）が
//!   立てば sTypoAscender・sTypoDescender、立たなければ `hhea` の ascender・descender
//! - 大文字の高さ: `OS/2`（版 2 以降）の sCapHeight。0 以下かなければ `H` の字形の外形の上、それも
//!   なければアセンダー
//! - 文字から字形: `cmap` の Unicode の副表（プラットフォーム 0、または 3 の符号化 1・10）の
//!   形式 12（なければ形式 4）
//! - 送り: `hmtx` の advanceWidth。numberOfHMetrics を超える字形は最後の送り
//! - 外形: `loca` で引いた `glyf` の字形の見出しの xMin・yMin・xMax・yMax。輪郭はたどらない
//!
//! 受け取ったファイルは信用しない。位置と長さはすべて確かめ、足し算と掛け算は `u64` か
//! 溢れを確かめて行い、`usize` の幅（32 ビットの wasm と 64 ビット）で結果が変わらないようにする。
//! 文字ごとの作業は、副表の二分探索で抑える。外形を見出しから読むのは、複合の字形の入れ子を
//! たどると、作業量が入れ子の深さの指数で増えうるためである。CFF・CFF2 の外形は読まない。

use crate::layout::{FontMetrics, Glyph, Glyphs};

/// 読めない理由
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unread {
    /// TrueType・OpenType のファイルでない（先頭の識別子が違う）
    Magic,
    /// フォントのコレクション（TTC）に、指定した番号のフォントがない
    Index,
    /// ファイルの見出しか表の並びが壊れている
    Malformed,
    /// 要る表がないか、壊れている。表の名前（`head`・`hhea`・`maxp`・`hmtx`・`cmap`・`loca`・`glyf`）
    Table(&'static str),
    /// 外形が `glyf` でなく CFF・CFF2 である。まだ読まない
    Cff,
}

/// 読んだフォント。[`Glyphs`] として [`crate::layout::place`] に渡す
pub struct Font<'a> {
    cmap: Cmap<'a>,
    hmtx: &'a [u8],
    /// `hhea` の numberOfHMetrics（1 以上）
    long_metrics: u16,
    /// `maxp` の numGlyphs（1 以上）
    glyph_count: u16,
    loca: &'a [u8],
    long_loca: bool,
    glyf: &'a [u8],
    metrics: FontMetrics,
    fallback: Glyph,
}

/// 文字から字形の番号を引く `cmap` の副表
#[derive(Clone, Copy)]
enum Cmap<'a> {
    /// 形式 4（BMP の区間）。副表の全体と区間の数
    Segments(&'a [u8], u16),
    /// 形式 12（区間ごとの連続した字形）。群の並びと群の数
    Groups(&'a [u8], u32),
}

/// 必要な表
#[derive(Default)]
struct Tables<'a> {
    head: Option<&'a [u8]>,
    hhea: Option<&'a [u8]>,
    maxp: Option<&'a [u8]>,
    os2: Option<&'a [u8]>,
    hmtx: Option<&'a [u8]>,
    cmap: Option<&'a [u8]>,
    loca: Option<&'a [u8]>,
    glyf: Option<&'a [u8]>,
    cff: bool,
}

const TRUETYPE: u32 = 0x0001_0000;
const APPLE_TRUETYPE: u32 = u32::from_be_bytes(*b"true");
const OPENTYPE_CFF: u32 = u32::from_be_bytes(*b"OTTO");
const COLLECTION: u32 = u32::from_be_bytes(*b"ttcf");

impl<'a> Font<'a> {
    /// ファイルのバイト列を読む。`index` はフォントのコレクション（TTC）の中の番号で、
    /// コレクションでなければ 0 にする。
    ///
    /// 作業量は表の数（最大 65535）で抑えられ、文字ごとの作業も二分探索で抑えられるので、
    /// 予算（決定 0010）は数えない。
    pub fn parse(data: &'a [u8], index: u32) -> Result<Self, Unread> {
        let tables = directory(data, index)?;
        let head = tables.head.ok_or(Unread::Table("head"))?;
        let hhea = tables.hhea.ok_or(Unread::Table("hhea"))?;
        let maxp = tables.maxp.ok_or(Unread::Table("maxp"))?;

        // head: unitsPerEm（18）・indexToLocFormat（50）
        let units_per_em = u16_at(head, 18).ok_or(Unread::Table("head"))?;
        let long_loca = match (head.len() >= 54, u16_at(head, 50)) {
            (true, Some(0)) => false,
            (true, Some(1)) => true,
            _ => return Err(Unread::Table("head")),
        };
        if !(16..=16384).contains(&units_per_em) {
            return Err(Unread::Table("head"));
        }
        // hhea: ascender（4）・descender（6）・numberOfHMetrics（34）
        let (Some(ascender), Some(descender), Some(long_metrics)) =
            (i16_at(hhea, 4), i16_at(hhea, 6), u16_at(hhea, 34))
        else {
            return Err(Unread::Table("hhea"));
        };
        if long_metrics == 0 {
            return Err(Unread::Table("hhea"));
        }
        // maxp: 版（0）・numGlyphs（4）
        let glyph_count = match (u32_at(maxp, 0), u16_at(maxp, 4)) {
            (Some(0x0000_5000 | 0x0001_0000), Some(count)) if count > 0 => count,
            _ => return Err(Unread::Table("maxp")),
        };

        let hmtx = tables.hmtx.ok_or(Unread::Table("hmtx"))?;
        if (hmtx.len() as u64) < u64::from(long_metrics) * 4 {
            return Err(Unread::Table("hmtx"));
        }
        let cmap = cmap(tables.cmap.ok_or(Unread::Table("cmap"))?)?;
        let (loca, glyf) = match (tables.loca, tables.glyf) {
            (Some(loca), Some(glyf)) => (loca, glyf),
            _ if tables.cff => return Err(Unread::Cff),
            (None, _) => return Err(Unread::Table("loca")),
            (_, None) => return Err(Unread::Table("glyf")),
        };
        let width = if long_loca { 4 } else { 2 };
        if (loca.len() as u64) < (u64::from(glyph_count) + 1) * width {
            return Err(Unread::Table("loca"));
        }

        let mut font = Self {
            cmap,
            hmtx,
            long_metrics,
            glyph_count,
            loca,
            long_loca,
            glyf,
            metrics: FontMetrics {
                units_per_em: 0.0,
                ascent: 0.0,
                descent: 0.0,
                cap_height: 0.0,
            },
            fallback: Glyph {
                advance: 0.0,
                bounds: None,
            },
        };
        // 字形 0（.notdef）は、フォントにない文字に使う。読めなければ glyf が壊れている
        font.fallback = font.by_id(0).ok_or(Unread::Table("glyf"))?;

        // OS/2: 版（0）・fsSelection（62）・sTypoAscender（68）・sTypoDescender（70）・sCapHeight（88）
        let os2 = tables.os2.unwrap_or(&[]);
        let version = u16_at(os2, 0).unwrap_or(0);
        let typographic = version >= 4 && u16_at(os2, 62).is_some_and(|flags| flags & 0x80 != 0);
        let (ascent, descent) = match (typographic, i16_at(os2, 68), i16_at(os2, 70)) {
            (true, Some(ascent), Some(descent)) => (ascent, descent),
            _ => (ascender, descender),
        };
        let ascent = f64::from(ascent);
        let cap_height = Some(version)
            .filter(|&version| version >= 2)
            .and_then(|_| i16_at(os2, 88))
            .filter(|&value| value > 0)
            .map(f64::from)
            .or_else(|| {
                let [_, _, _, top] = font.glyph('H')?.bounds?;
                (top > 0.0).then_some(top)
            })
            .unwrap_or(ascent);
        font.metrics = FontMetrics {
            units_per_em: f64::from(units_per_em),
            ascent,
            descent: -f64::from(descent),
            cap_height,
        };
        Ok(font)
    }

    /// 字形の番号から、送りと外形を読む。番号が字形の数を超えるか、`loca` の区間が逆か `glyf` の
    /// 外にあれば `None`。区間が空の字形（空白など）は外形を持たない
    fn by_id(&self, id: u16) -> Option<Glyph> {
        if id >= self.glyph_count {
            return None;
        }
        let record = id.min(self.long_metrics - 1);
        let advance = u16_at(self.hmtx, usize::from(record) * 4)?;
        let (start, end) = if self.long_loca {
            let at = usize::from(id) * 4;
            (u32_at(self.loca, at)?, u32_at(self.loca, at + 4)?)
        } else {
            let at = usize::from(id) * 2;
            let [start, end] = [u16_at(self.loca, at)?, u16_at(self.loca, at + 2)?];
            (u32::from(start) * 2, u32::from(end) * 2)
        };
        let bounds = match start.cmp(&end) {
            core::cmp::Ordering::Greater => return None,
            core::cmp::Ordering::Equal => None,
            core::cmp::Ordering::Less => {
                let glyph = slice(self.glyf, u64::from(start), u64::from(end - start))?;
                // 見出し: numberOfContours（0）・xMin（2）・yMin（4）・xMax（6）・yMax（8）
                let [l, b, r, t] = [2, 4, 6, 8].map(|at| i16_at(glyph, at).map(f64::from));
                Some([l?, b?, r?, t?])
            }
        };
        Some(Glyph {
            advance: f64::from(advance),
            bounds,
        })
    }
}

impl Glyphs for Font<'_> {
    fn metrics(&self) -> FontMetrics {
        self.metrics
    }

    /// 字形 0 に当たる文字・字形の数を超える番号に当たる文字・字形の区間が壊れた文字は、
    /// フォントにないとする
    fn glyph(&self, ch: char) -> Option<Glyph> {
        let id = match self.cmap {
            Cmap::Segments(table, count) => segment(table, count, ch)?,
            Cmap::Groups(groups, count) => group(groups, count, ch)?,
        };
        if id == 0 {
            return None;
        }
        self.by_id(id)
    }

    fn fallback(&self) -> Glyph {
        self.fallback
    }
}

/// ファイルの見出しと表の並びを読み、要る表を切り出す（OpenType の仕様の Font file の節）
fn directory(data: &[u8], index: u32) -> Result<Tables<'_>, Unread> {
    let magic = u32_at(data, 0).ok_or(Unread::Malformed)?;
    let start = match magic {
        COLLECTION => {
            // TTC の見出し: ttcTag（0）・版（4）・numFonts（8）・tableDirectoryOffsets（12）
            let count = u32_at(data, 8).ok_or(Unread::Malformed)?;
            if index >= count {
                return Err(Unread::Index);
            }
            let at = usize::try_from(12 + u64::from(index) * 4).map_err(|_| Unread::Malformed)?;
            u32_at(data, at).ok_or(Unread::Malformed)?
        }
        TRUETYPE | APPLE_TRUETYPE | OPENTYPE_CFF if index == 0 => 0,
        TRUETYPE | APPLE_TRUETYPE | OPENTYPE_CFF => return Err(Unread::Index),
        _ => return Err(Unread::Magic),
    };
    let header = slice(data, u64::from(start), 12).ok_or(Unread::Malformed)?;
    if !matches!(
        u32_at(header, 0),
        Some(TRUETYPE | APPLE_TRUETYPE | OPENTYPE_CFF)
    ) {
        return Err(Unread::Malformed);
    }
    let count = u16_at(header, 4).ok_or(Unread::Malformed)?;
    let records =
        slice(data, u64::from(start) + 12, u64::from(count) * 16).ok_or(Unread::Malformed)?;

    let mut tables = Tables::default();
    // 表の記録: tableTag（0）・checksum（4）・offset（8）・length（12）
    for record in records.as_chunks::<16>().0 {
        let (Some(offset), Some(length)) = (u32_at(record, 8), u32_at(record, 12)) else {
            return Err(Unread::Malformed);
        };
        let table = slice(data, u64::from(offset), u64::from(length));
        let [a, b, c, d, ..] = *record;
        let slot = match &[a, b, c, d] {
            b"head" => &mut tables.head,
            b"hhea" => &mut tables.hhea,
            b"maxp" => &mut tables.maxp,
            b"OS/2" => &mut tables.os2,
            b"hmtx" => &mut tables.hmtx,
            b"cmap" => &mut tables.cmap,
            b"loca" => &mut tables.loca,
            b"glyf" => &mut tables.glyf,
            b"CFF " | b"CFF2" => {
                tables.cff = true;
                continue;
            }
            _ => continue,
        };
        // 同じ名前の表は、最初のものを使う。ファイルの外を指す表は、ないものとする
        if slot.is_none() {
            *slot = table;
        }
    }
    Ok(tables)
}

/// `cmap` から使う副表を選ぶ（OpenType の仕様の cmap の節）。形式 12 を形式 4 より先にする
fn cmap(table: &[u8]) -> Result<Cmap<'_>, Unread> {
    let broken = Unread::Table("cmap");
    let count = u16_at(table, 2).ok_or(broken)?;
    let records = slice(table, 4, u64::from(count) * 8).ok_or(broken)?;
    let mut segments = None;
    // 符号化の記録: platformID（0）・encodingID（2）・subtableOffset（4）
    for record in records.as_chunks::<8>().0 {
        let (Some(platform), Some(encoding), Some(offset)) =
            (u16_at(record, 0), u16_at(record, 2), u32_at(record, 4))
        else {
            return Err(broken);
        };
        // プラットフォーム 0 の符号化 5 は異体字の副表（形式 14）で、字形を引かない
        let unicode =
            (platform == 0 && encoding != 5) || (platform == 3 && matches!(encoding, 1 | 10));
        if !unicode {
            continue;
        }
        let Some(subtable) = usize::try_from(offset).ok().and_then(|at| table.get(at..)) else {
            continue;
        };
        match u16_at(subtable, 0) {
            // 形式 12: numGroups（12）、群（16 から 12 バイトずつ）
            Some(12) => {
                let Some(groups) = u32_at(subtable, 12) else {
                    continue;
                };
                if let Some(data) = slice(subtable, 16, u64::from(groups) * 12) {
                    return Ok(Cmap::Groups(data, groups));
                }
            }
            // 形式 4: segCountX2（6）。endCode（14）・reservedPad・startCode・idDelta・idRangeOffset
            Some(4) if segments.is_none() => {
                let Some(doubled) = u16_at(subtable, 6) else {
                    continue;
                };
                let count = doubled / 2;
                if count > 0 && slice(subtable, 0, 16 + u64::from(count) * 8).is_some() {
                    segments = Some(Cmap::Segments(subtable, count));
                }
            }
            _ => {}
        }
    }
    segments.ok_or(broken)
}

/// 形式 4 で文字を引く。区間は endCode の昇順と仮定して二分探索する
fn segment(table: &[u8], count: u16, ch: char) -> Option<u16> {
    let code = u16::try_from(u32::from(ch)).ok()?;
    let count = usize::from(count);
    let end_at = |i: usize| 14 + i * 2;
    // endCode が code 以上の最初の区間
    let (mut low, mut high) = (0, count);
    while low < high {
        let middle = low + (high - low) / 2;
        if u16_at(table, end_at(middle))? < code {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    if low == count {
        return None;
    }
    let start = u16_at(table, end_at(count) + 2 + low * 2)?;
    if code < start {
        return None;
    }
    let delta = u16_at(table, end_at(count) + 2 + count * 2 + low * 2)?;
    let range_at = end_at(count) + 2 + count * 4 + low * 2;
    let range = u16_at(table, range_at)?;
    if range == 0 {
        return Some(code.wrapping_add(delta));
    }
    // idRangeOffset は、自分の位置からの相対のバイト数
    let at = range_at + usize::from(range) + usize::from(code - start) * 2;
    let id = u16_at(table, at)?;
    (id != 0).then(|| id.wrapping_add(delta))
}

/// 形式 12 で文字を引く。群は startCharCode の昇順と仮定して二分探索する
fn group(groups: &[u8], count: u32, ch: char) -> Option<u16> {
    let code = u32::from(ch);
    // 群: startCharCode（0）・endCharCode（4）・startGlyphID（8）
    let read = |i: u32, at: usize| u32_at(groups, usize::try_from(i).ok()? * 12 + at);
    let (mut low, mut high) = (0, count);
    while low < high {
        let middle = low + (high - low) / 2;
        let (start, end) = (read(middle, 0)?, read(middle, 4)?);
        if code < start {
            high = middle;
        } else if code > end {
            low = middle + 1;
        } else {
            let id = u64::from(read(middle, 8)?) + u64::from(code - start);
            return u16::try_from(id).ok();
        }
    }
    None
}

/// `data` の `start` から `length` バイト。範囲が外に出れば `None`
fn slice(data: &[u8], start: u64, length: u64) -> Option<&[u8]> {
    let end = start.checked_add(length)?;
    if end > data.len() as u64 {
        return None;
    }
    data.get(usize::try_from(start).ok()?..usize::try_from(end).ok()?)
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    let bytes = data.get(at..at.checked_add(2)?)?;
    Some(u16::from_be_bytes(bytes.try_into().ok()?))
}

fn i16_at(data: &[u8], at: usize) -> Option<i16> {
    u16_at(data, at).map(|value| value as i16)
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    let bytes = data.get(at..at.checked_add(4)?)?;
    Some(u32::from_be_bytes(bytes.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Reference, place};
    use crate::{
        Height, HeightMeaning, Horizontal, Length, PaperLength, Placement, Text, Vertical,
    };

    /// 表の名前と中身
    type Tables = Vec<([u8; 4], Vec<u8>)>;

    fn put16(out: &mut Vec<u8>, value: i32) {
        out.extend_from_slice(&(value as u16).to_be_bytes());
    }

    fn put32(out: &mut Vec<u8>, value: u32) {
        out.extend_from_slice(&value.to_be_bytes());
    }

    /// 字形は 4 つ: 0 は .notdef（送り 500・外形 50〜450 × 0〜700）、1 は `H`（送り 700・外形
    /// 0〜700 × 0〜720）、2 は `g`（送り 550・外形 30〜520 × -210〜480）、3 は空白（送り 250・外形なし）。
    /// `cmap` は、空白・`H`・`g` のほか、`X` を字形の数を超える 9 に、`Z` を字形 0 に当てる
    fn tables() -> Tables {
        let mut head = vec![0; 54];
        head[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
        head[12..16].copy_from_slice(&0x5F0F_3CF5u32.to_be_bytes());
        head[18..20].copy_from_slice(&1000u16.to_be_bytes());
        head[50..52].copy_from_slice(&1u16.to_be_bytes()); // loca は 32 ビット

        let mut hhea = vec![0; 36];
        hhea[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
        hhea[4..6].copy_from_slice(&900i16.to_be_bytes());
        hhea[6..8].copy_from_slice(&(-250i16).to_be_bytes());
        hhea[34..36].copy_from_slice(&4u16.to_be_bytes());

        let mut maxp = Vec::new();
        put32(&mut maxp, 0x0000_5000);
        put16(&mut maxp, 4);

        // OS/2 の版 4。USE_TYPO_METRICS は立てない
        let mut os2 = vec![0; 96];
        os2[0..2].copy_from_slice(&4u16.to_be_bytes());
        os2[68..70].copy_from_slice(&800i16.to_be_bytes());
        os2[70..72].copy_from_slice(&(-200i16).to_be_bytes());
        os2[88..90].copy_from_slice(&700i16.to_be_bytes());

        let mut hmtx = Vec::new();
        for advance in [500, 700, 550, 250] {
            put16(&mut hmtx, advance);
            put16(&mut hmtx, 0);
        }

        let mut glyf = Vec::new();
        for [l, b, r, t] in [[50, 0, 450, 700], [0, 0, 700, 720], [30, -210, 520, 480]] {
            // 字形の見出しだけを置く。外形は見出しから読み、輪郭は読まない
            put16(&mut glyf, 1);
            for value in [l, b, r, t] {
                put16(&mut glyf, value);
            }
        }
        let mut loca = Vec::new();
        for offset in [0, 10, 20, 30, 30] {
            put32(&mut loca, offset);
        }

        let groups: [(u32, u32); 5] = [(0x20, 3), (0x48, 1), (0x58, 9), (0x5A, 0), (0x67, 2)];
        let mut cmap = Vec::new();
        put16(&mut cmap, 0);
        put16(&mut cmap, 1);
        put16(&mut cmap, 3); // Windows
        put16(&mut cmap, 10); // Unicode の全体
        put32(&mut cmap, 12);
        put16(&mut cmap, 12);
        put16(&mut cmap, 0);
        put32(&mut cmap, 16 + 12 * groups.len() as u32);
        put32(&mut cmap, 0);
        put32(&mut cmap, groups.len() as u32);
        for (code, id) in groups {
            put32(&mut cmap, code);
            put32(&mut cmap, code);
            put32(&mut cmap, id);
        }

        vec![
            (*b"OS/2", os2),
            (*b"cmap", cmap),
            (*b"glyf", glyf),
            (*b"head", head),
            (*b"hhea", hhea),
            (*b"hmtx", hmtx),
            (*b"loca", loca),
            (*b"maxp", maxp),
        ]
    }

    /// 表を並べてファイルにする。表の並びは名前の順にする
    fn assemble(mut tables: Tables) -> Vec<u8> {
        tables.sort_by_key(|(tag, _)| *tag);
        let mut out = Vec::new();
        put32(&mut out, 0x0001_0000);
        put16(&mut out, tables.len() as i32);
        for _ in 0..3 {
            put16(&mut out, 0);
        }
        let mut offset = 12 + 16 * tables.len();
        let mut body = Vec::new();
        for (tag, data) in &tables {
            out.extend_from_slice(tag);
            put32(&mut out, 0);
            put32(&mut out, offset as u32);
            put32(&mut out, data.len() as u32);
            body.extend_from_slice(data);
            while body.len() % 4 != 0 {
                body.push(0);
            }
            offset = 12 + 16 * tables.len() + body.len();
        }
        out.extend_from_slice(&body);
        out
    }

    fn without(tag: &[u8; 4]) -> Tables {
        tables().into_iter().filter(|(t, _)| t != tag).collect()
    }

    fn edit(tag: &[u8; 4], f: impl Fn(&mut Vec<u8>)) -> Tables {
        let mut tables = tables();
        for (t, data) in &mut tables {
            if t == tag {
                f(data);
            }
        }
        tables
    }

    fn reason(data: &[u8]) -> Unread {
        Font::parse(data, 0).err().expect("読めないはず")
    }

    #[test]
    fn reads_metrics_from_hhea_and_os2() {
        let data = assemble(tables());
        let font = Font::parse(&data, 0).unwrap();
        assert_eq!(
            font.metrics(),
            FontMetrics {
                units_per_em: 1000.0,
                ascent: 900.0,
                descent: 250.0,
                cap_height: 700.0,
            }
        );
    }

    #[test]
    fn uses_typographic_metrics_when_flagged() {
        let data = assemble(edit(b"OS/2", |os2| {
            os2[62..64].copy_from_slice(&0x80u16.to_be_bytes())
        }));
        let metrics = Font::parse(&data, 0).unwrap().metrics();
        assert_eq!([metrics.ascent, metrics.descent], [800.0, 200.0]);
    }

    #[test]
    fn cap_height_falls_back_to_h_then_ascent() {
        // OS/2 がなければ、H の外形の上
        let data = assemble(without(b"OS/2"));
        assert_eq!(Font::parse(&data, 0).unwrap().metrics().cap_height, 720.0);
        // OS/2 の版 1 は sCapHeight を持たない
        let data = assemble(edit(b"OS/2", |os2| {
            os2[0..2].copy_from_slice(&1u16.to_be_bytes());
            os2.truncate(86);
        }));
        assert_eq!(Font::parse(&data, 0).unwrap().metrics().cap_height, 720.0);
        // H もなければ、アセンダー
        let data = assemble(
            without(b"OS/2")
                .into_iter()
                .map(|(tag, mut data)| {
                    if &tag == b"cmap" {
                        // H（0x48）を I（0x49）に当て直す
                        let at = 12 + 16 + 12;
                        data[at..at + 4].copy_from_slice(&0x49u32.to_be_bytes());
                        data[at + 4..at + 8].copy_from_slice(&0x49u32.to_be_bytes());
                    }
                    (tag, data)
                })
                .collect(),
        );
        let font = Font::parse(&data, 0).unwrap();
        assert_eq!(font.metrics().cap_height, 900.0);
        assert!(font.glyph('H').is_none());
    }

    #[test]
    fn reads_advances_and_bounds() {
        let data = assemble(tables());
        let font = Font::parse(&data, 0).unwrap();
        assert_eq!(
            font.glyph('H'),
            Some(Glyph {
                advance: 700.0,
                bounds: Some([0.0, 0.0, 700.0, 720.0]),
            })
        );
        assert_eq!(
            font.glyph('g'),
            Some(Glyph {
                advance: 550.0,
                bounds: Some([30.0, -210.0, 520.0, 480.0]),
            })
        );
        // 空白は字形のデータが空で、外形を持たない
        assert_eq!(
            font.glyph(' '),
            Some(Glyph {
                advance: 250.0,
                bounds: None,
            })
        );
        assert_eq!(
            font.fallback(),
            Glyph {
                advance: 500.0,
                bounds: Some([50.0, 0.0, 450.0, 700.0]),
            }
        );
    }

    #[test]
    fn reads_segment_cmap_short_loca_and_shared_advances() {
        // cmap の形式 4: 空白は idDelta、H は idRangeOffset で字形の配列を引き、g は idDelta
        let mut cmap = Vec::new();
        for value in [0, 1, 3, 1, 0, 12] {
            put16(&mut cmap, value);
        }
        for value in [4, 0, 0, 8, 0, 0, 0] {
            put16(&mut cmap, value);
        }
        for end in [0x20, 0x48, 0x67, 0xFFFF] {
            put16(&mut cmap, end);
        }
        put16(&mut cmap, 0);
        for start in [0x20, 0x48, 0x67, 0xFFFF] {
            put16(&mut cmap, start);
        }
        for delta in [3 - 0x20, 0, 2 - 0x67, 1] {
            put16(&mut cmap, delta);
        }
        for range in [0, 6, 0, 0] {
            put16(&mut cmap, range);
        }
        put16(&mut cmap, 1);
        let mut tables: Tables = tables()
            .into_iter()
            .map(|(tag, mut data)| {
                match &tag {
                    b"cmap" => data = cmap.clone(),
                    // loca を 16 ビットにする
                    b"head" => data[50..52].fill(0),
                    b"loca" => {
                        data.clear();
                        for offset in [0, 5, 10, 15, 15] {
                            put16(&mut data, offset);
                        }
                    }
                    // 送りの記録を 2 つにし、字形 2・3 は字形 1 の送りを使う
                    b"hhea" => data[34..36].copy_from_slice(&2u16.to_be_bytes()),
                    _ => {}
                }
                (tag, data)
            })
            .collect();
        tables.retain(|(tag, _)| tag != b"OS/2");
        let data = assemble(tables);
        let font = Font::parse(&data, 0).unwrap();
        assert_eq!(
            font.glyph('H'),
            Some(Glyph {
                advance: 700.0,
                bounds: Some([0.0, 0.0, 700.0, 720.0]),
            })
        );
        assert_eq!(
            font.glyph('g'),
            Some(Glyph {
                advance: 700.0,
                bounds: Some([30.0, -210.0, 520.0, 480.0]),
            })
        );
        assert_eq!(font.glyph(' ').map(|g| g.bounds), Some(None));
        assert_eq!(font.glyph('Q'), None);
        assert_eq!(font.glyph('\u{1F600}'), None);
        assert_eq!(font.metrics().cap_height, 720.0);
    }

    #[test]
    fn reads_a_font_in_a_collection() {
        let single = assemble(tables());
        // TTC の見出し（12 バイト）と、2 つの番号。番号 1 のフォントだけを置き、番号 0 は外を指す
        let mut data = Vec::new();
        data.extend_from_slice(b"ttcf");
        put32(&mut data, 0x0001_0000);
        put32(&mut data, 2);
        put32(&mut data, 0xFFFF_FFF0);
        put32(&mut data, 0);
        // TTC の中の表の位置はファイルの頭から数えるので、フォントの表の位置を見出しの分ずらす
        let shift = 20;
        let mut font = single.clone();
        let count = usize::from(u16::from_be_bytes([font[4], font[5]]));
        for i in 0..count {
            let at = 12 + 16 * i + 8;
            let offset = u32::from_be_bytes(font[at..at + 4].try_into().unwrap()) + shift;
            font[at..at + 4].copy_from_slice(&offset.to_be_bytes());
        }
        data[16..20].copy_from_slice(&shift.to_be_bytes());
        data.extend_from_slice(&font);
        assert_eq!(Font::parse(&data, 0).err(), Some(Unread::Malformed));
        assert_eq!(Font::parse(&data, 2).err(), Some(Unread::Index));
        let font = Font::parse(&data, 1).unwrap();
        assert_eq!(font.metrics().units_per_em, 1000.0);
        assert_eq!(font.glyph('g').map(|g| g.advance), Some(550.0));
    }

    #[test]
    fn treats_unmapped_notdef_and_out_of_range_as_missing() {
        let data = assemble(tables());
        let font = Font::parse(&data, 0).unwrap();
        // cmap にない・字形 0 に当たる・字形の数を超える番号に当たる
        for ch in ['Q', 'Z', 'X', '図'] {
            assert_eq!(font.glyph(ch), None, "{ch}");
        }
    }

    #[test]
    fn rejects_broken_files() {
        assert_eq!(reason(b""), Unread::Malformed);
        assert_eq!(reason(b"%PDF-1.7 not a font"), Unread::Magic);
        let data = assemble(tables());
        assert_eq!(Font::parse(&data, 1).err(), Some(Unread::Index));
        for tag in [
            b"head", b"hhea", b"maxp", b"hmtx", b"cmap", b"loca", b"glyf",
        ] {
            let name = core::str::from_utf8(tag).unwrap();
            assert_eq!(
                reason(&assemble(without(tag))),
                Unread::Table(name),
                "{name}"
            );
        }
        assert_eq!(reason(&assemble(without(b"loca"))), Unread::Table("loca"));
        // em が 0 の head は壊れている
        let data = assemble(edit(b"head", |head| head[18..20].fill(0)));
        assert_eq!(reason(&data), Unread::Table("head"));
        // hmtx が短い
        let data = assemble(edit(b"hmtx", |hmtx| hmtx.truncate(6)));
        assert_eq!(reason(&data), Unread::Table("hmtx"));
        // 外形が CFF のフォント
        let mut cff: Tables = tables()
            .into_iter()
            .filter(|(tag, _)| tag != b"glyf" && tag != b"loca")
            .collect();
        cff.push((*b"CFF ", vec![1, 0, 4, 4]));
        assert_eq!(reason(&assemble(cff)), Unread::Cff);
    }

    #[test]
    fn survives_truncated_and_corrupted_bytes() {
        let data = assemble(tables());
        let probe = |bytes: &[u8]| {
            if let Ok(font) = Font::parse(bytes, 0) {
                let _ = font.metrics();
                let _ = font.fallback();
                for ch in [' ', 'H', 'g', 'X', 'Z', 'Q'] {
                    let _ = font.glyph(ch);
                }
            }
        };
        for end in 0..data.len() {
            probe(&data[..end]);
        }
        for at in 0..data.len() {
            for mask in [0x01, 0x80, 0xFF] {
                let mut bytes = data.clone();
                bytes[at] ^= mask;
                probe(&bytes);
            }
        }
    }

    #[test]
    fn places_text_with_a_read_font() {
        let data = assemble(tables());
        let font = Font::parse(&data, 0).unwrap();
        let text = Text {
            content: "HgQ".to_owned(),
            height: Height {
                value: Length::Paper(PaperLength(7.0)),
                meaning: HeightMeaning::CapHeight,
            },
            placement: Placement::Anchor {
                point: [10.0, 20.0],
                horizontal: Horizontal::Center,
                vertical: Vertical::Top,
            },
            rotation: 0.0,
            width_factor: 1.0,
            oblique: 0.0,
            backward: false,
            upside_down: false,
            style: String::new(),
        };
        let laid = place(&text, |p| *p, &font, |_| None::<Reference>).unwrap();
        // 大文字の高さ 700 を 7 にする倍率 0.01。送りは 700 + 550 + 代わりの字形の 500
        assert_eq!(laid.pens, vec![0.0, 700.0, 1250.0]);
        assert_eq!(laid.advance, 1750.0);
        assert_eq!(laid.missing, 1);
        assert_eq!(laid.scale, [0.01, 0.01]);
        let [a, b, c, d, e, f] = laid.transform;
        assert_eq!([a, b, c, d], [0.01, 0.0, 0.0, 0.01]);
        assert!((e - (10.0 - 8.75)).abs() < 1e-12, "{e}");
        assert!((f - (20.0 - 7.0)).abs() < 1e-12, "{f}");
    }
}
