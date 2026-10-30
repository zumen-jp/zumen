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
//! - 外形: `loca` で引いた `glyf` の字形の見出しの xMin・yMin・xMax・yMax。輪郭はたどらない。
//!   `glyf` がなければ、`CFF `・`CFF2` の字形の命令をたどった外形（[`cff`]、決定 0186）
//! - カーニング: `GPOS` の pair adjustment か、なければ `kern` の形式 0（[`kern`]、決定 0186）
//! - 縦書き: `vhea` の numOfLongVerMetrics と `vmtx` の advanceHeight・topSideBearing、`GSUB` の
//!   `vrt2`（なければ `vert`）の single substitution（[`gsub`]、決定 0187）。どれも任意の表で、
//!   なければ、または壊れていれば、縦書きの既定（[`crate::layout::VerticalGlyph::upright`]）と
//!   置き換えなしで置く
//!
//! 受け取ったファイルは信用しない。位置と長さはすべて確かめ、足し算と掛け算は `u64` か
//! 溢れを確かめて行い、`usize` の幅（32 ビットの wasm と 64 ビット）で結果が変わらないようにする。
//! 文字ごとの作業は、副表の二分探索で抑える。外形を見出しから読むのは、複合の字形の入れ子を
//! たどると、作業量が入れ子の深さの指数で増えうるためである。CFF・CFF2 の字形の命令は、
//! 副ルーチンの入れ子と、字形ごとの作業量に上限を設けてたどる。

mod cff;
mod glyf;
mod gsub;
mod kern;

use crate::layout::{FontMetrics, Glyph, Glyphs, VerticalGlyph};

/// 読めない理由
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unread {
    /// TrueType・OpenType のファイルでない（先頭の識別子が違う）
    Magic,
    /// フォントのコレクション（TTC）に、指定した番号のフォントがない
    Index,
    /// ファイルの見出しか表の並びが壊れている
    Malformed,
    /// 要る表がないか、壊れている。表の名前（`head`・`hhea`・`maxp`・`hmtx`・`cmap`・`loca`・`glyf`・
    /// `CFF `・`CFF2`）
    Table(&'static str),
    /// 外形が CFF・CFF2 で、読まない形である（字形の命令が Type 2 でない、Font DICT が 256 を超える）
    Cff,
}

/// 字形の輪郭の区間。点は字形の単位で、前の区間の終わり（最初の区間は輪郭の始点）から続く（決定 0188）
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Segment {
    /// 直線。終わりの点
    Line([f64; 2]),
    /// 2 次の Bézier 曲線（TrueType）。制御点と終わりの点
    Quad([f64; 2], [f64; 2]),
    /// 3 次の Bézier 曲線（CFF・CFF2）。2 つの制御点と終わりの点
    Cubic([f64; 2], [f64; 2], [f64; 2]),
}

/// 字形の閉じた輪郭。最後の区間の終わりから始点へ閉じる。輪郭は非零の規則で塗る（決定 0188）
#[derive(Clone, Debug, PartialEq)]
pub struct Contour {
    /// 始点
    pub start: [f64; 2],
    /// 区間
    pub segments: Vec<Segment>,
}

/// 読んだフォント。[`Glyphs`] として [`crate::layout::place`] に渡す
pub struct Font<'a> {
    cmap: Cmap<'a>,
    hmtx: &'a [u8],
    /// `hhea` の numberOfHMetrics（1 以上）
    long_metrics: u16,
    /// `maxp` の numGlyphs（1 以上）
    glyph_count: u16,
    outline: Outline<'a>,
    kerning: kern::Kerning<'a>,
    /// 縦書きの送りと上の余白。`vhea`・`vmtx` がないか壊れていれば `None`
    vmtx: Option<Vmtx<'a>>,
    /// 縦書き用の字形への置き換え
    vertical: gsub::Substitution<'a>,
    metrics: FontMetrics,
    fallback: Glyph,
}

/// 字形の外形を読む表
enum Outline<'a> {
    Glyf {
        loca: &'a [u8],
        long_loca: bool,
        glyf: &'a [u8],
    },
    Cff(cff::Cff<'a>),
}

/// `vmtx` の表と、`vhea` の numOfLongVerMetrics（1 以上）。表は、字形の数だけの上の余白を持つ
struct Vmtx<'a> {
    table: &'a [u8],
    long_metrics: u16,
}

impl Vmtx<'_> {
    /// 字形の縦の送りと上の余白。numOfLongVerMetrics を超える字形は、最後の送りと、続く並びの上の余白
    fn get(&self, id: u16) -> Option<(u16, i16)> {
        let record = id.min(self.long_metrics - 1);
        let advance = u16_at(self.table, usize::from(record) * 4)?;
        let bearing = if id < self.long_metrics {
            i16_at(self.table, usize::from(id) * 4 + 2)?
        } else {
            let at = usize::from(self.long_metrics) * 4 + usize::from(id - self.long_metrics) * 2;
            i16_at(self.table, at)?
        };
        Some((advance, bearing))
    }
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
    cff: Option<&'a [u8]>,
    cff2: Option<&'a [u8]>,
    gpos: Option<&'a [u8]>,
    kern: Option<&'a [u8]>,
    vhea: Option<&'a [u8]>,
    vmtx: Option<&'a [u8]>,
    gsub: Option<&'a [u8]>,
    /// 表の並びで見つけた CFF・CFF2 の表の名前（ファイルの外を指していても）
    cff_tag: Option<&'static str>,
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
        let (outline, outline_name) = match (tables.loca, tables.glyf) {
            (Some(loca), Some(glyf)) => {
                let width = if long_loca { 4 } else { 2 };
                if (loca.len() as u64) < (u64::from(glyph_count) + 1) * width {
                    return Err(Unread::Table("loca"));
                }
                let glyf = Outline::Glyf {
                    loca,
                    long_loca,
                    glyf,
                };
                (glyf, "glyf")
            }
            _ if tables.cff_tag.is_some() => match (tables.cff, tables.cff2) {
                (Some(table), _) => (Outline::Cff(cff::Cff::parse(table, false)?), "CFF "),
                (None, Some(table)) => (Outline::Cff(cff::Cff::parse(table, true)?), "CFF2"),
                (None, None) => return Err(Unread::Table(tables.cff_tag.unwrap_or("CFF "))),
            },
            (None, _) => return Err(Unread::Table("loca")),
            (_, None) => return Err(Unread::Table("glyf")),
        };

        let mut font = Self {
            cmap,
            hmtx,
            long_metrics,
            glyph_count,
            outline,
            kerning: kern::Kerning::read(tables.gpos, tables.kern),
            vmtx: vmtx(tables.vhea, tables.vmtx, glyph_count),
            vertical: gsub::Substitution::read(tables.gsub),
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
        // 字形 0（.notdef）は、フォントにない文字に使う。読めなければ外形の表が壊れている
        font.fallback = font.by_id(0).ok_or(Unread::Table(outline_name))?;

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
    /// 外にあれば `None`。区間が空の字形（空白など）は外形を持たない。CFF・CFF2 では、字形の命令が
    /// 壊れているか上限を超えれば `None`、何も描かない字形は外形を持たない
    fn by_id(&self, id: u16) -> Option<Glyph> {
        if id >= self.glyph_count {
            return None;
        }
        let record = id.min(self.long_metrics - 1);
        let advance = u16_at(self.hmtx, usize::from(record) * 4)?;
        let bounds = match &self.outline {
            Outline::Glyf {
                loca,
                long_loca,
                glyf,
            } => glyf_bounds(loca, *long_loca, glyf, id)?,
            Outline::Cff(cff) => cff.bounds(id)?,
        };
        Some(Glyph {
            advance: f64::from(advance),
            bounds,
        })
    }
}

/// `vhea`（numOfLongVerMetrics は 34）と `vmtx` を読む。`vmtx` が字形の数だけの記録を持たなければ使わない
fn vmtx<'a>(vhea: Option<&[u8]>, vmtx: Option<&'a [u8]>, glyph_count: u16) -> Option<Vmtx<'a>> {
    let long_metrics = u16_at(vhea?, 34)?.min(glyph_count);
    let table = vmtx?;
    let size = u64::from(long_metrics) * 4 + u64::from(glyph_count - long_metrics) * 2;
    (long_metrics > 0 && table.len() as u64 >= size).then_some(Vmtx {
        table,
        long_metrics,
    })
}

/// `glyf` の字形の見出しの外形
fn glyf_bounds(loca: &[u8], long_loca: bool, glyf: &[u8], id: u16) -> Option<Option<[f64; 4]>> {
    let (start, end) = if long_loca {
        let at = usize::from(id) * 4;
        (u32_at(loca, at)?, u32_at(loca, at + 4)?)
    } else {
        let at = usize::from(id) * 2;
        let [start, end] = [u16_at(loca, at)?, u16_at(loca, at + 2)?];
        (u32::from(start) * 2, u32::from(end) * 2)
    };
    match start.cmp(&end) {
        core::cmp::Ordering::Greater => None,
        core::cmp::Ordering::Equal => Some(None),
        core::cmp::Ordering::Less => {
            let glyph = slice(glyf, u64::from(start), u64::from(end - start))?;
            // 見出し: numberOfContours（0）・xMin（2）・yMin（4）・xMax（6）・yMax（8）
            let [l, b, r, t] = [2, 4, 6, 8].map(|at| i16_at(glyph, at).map(f64::from));
            Some(Some([l?, b?, r?, t?]))
        }
    }
}

impl Font<'_> {
    /// 文字の字形の番号。フォントにない文字は `None`。[`Glyphs::glyph`] が `None` を返す文字と同じ
    pub fn glyph_id(&self, ch: char) -> Option<u16> {
        self.id(ch).filter(|&id| self.by_id(id).is_some())
    }

    /// 縦書きで置く字形の番号。`GSUB` で置き換え、置き換えた先が読めなければ置き換えない
    /// （[`Glyphs::vertical`] と同じ規則）
    pub fn vertical_id(&self, id: u16) -> u16 {
        let replaced = self.vertical.apply(id);
        if self.by_id(replaced).is_some() {
            replaced
        } else {
            id
        }
    }

    /// 字形の輪郭（字形の単位、決定 0188）。番号が字形の数を超えるか、輪郭が壊れているか上限を超えれば
    /// `None`。何も描かない字形は空。`glyf` の複合の字形は部品を変換して並べ、CFF・CFF2 は字形の命令を
    /// たどる。可変フォントの変化は当てない
    pub fn outline(&self, id: u16) -> Option<Vec<Contour>> {
        if id >= self.glyph_count {
            return None;
        }
        match &self.outline {
            Outline::Glyf {
                loca,
                long_loca,
                glyf,
            } => glyf::outline(loca, *long_loca, glyf, self.glyph_count, id),
            Outline::Cff(cff) => cff.outline(id),
        }
    }

    /// 字形の番号から、縦書きの字形を読む
    fn vertical_by_id(&self, original: u16) -> Option<VerticalGlyph> {
        let replaced = self.vertical.apply(original);
        let (id, glyph) = match self.by_id(replaced) {
            Some(glyph) => (replaced, glyph),
            None => (original, self.by_id(original)?),
        };
        let mut out = VerticalGlyph::upright(glyph, &self.metrics);
        out.substituted = id != original;
        if let Some((advance, bearing)) = self.vmtx.as_ref().and_then(|vmtx| vmtx.get(id)) {
            out.advance = f64::from(advance);
            if let Some([_, _, _, top]) = glyph.bounds {
                out.top = top + f64::from(bearing);
            }
        }
        Some(out)
    }

    /// 文字の字形の番号。`cmap` になく、字形 0 か字形の数を超える番号に当たる文字は `None`
    fn id(&self, ch: char) -> Option<u16> {
        let id = match self.cmap {
            Cmap::Segments(table, count) => segment(table, count, ch)?,
            Cmap::Groups(groups, count) => group(groups, count, ch)?,
        };
        (id != 0 && id < self.glyph_count).then_some(id)
    }
}

impl Glyphs for Font<'_> {
    fn metrics(&self) -> FontMetrics {
        self.metrics
    }

    /// 字形 0 に当たる文字・字形の数を超える番号に当たる文字・字形の区間が壊れた文字は、
    /// フォントにないとする
    fn glyph(&self, ch: char) -> Option<Glyph> {
        self.by_id(self.id(ch)?)
    }

    fn fallback(&self) -> Glyph {
        self.fallback
    }

    /// `GSUB` で置き換えた字形を、`vmtx` の縦の送りで置く。縦の原点の高さは、外形の上に上の余白を
    /// 足した値。外形のない字形と `vmtx` のないフォントは、既定の値にする。置き換えた先の字形が
    /// 読めなければ置き換えない
    fn vertical(&self, ch: char) -> Option<VerticalGlyph> {
        self.vertical_by_id(self.id(ch)?)
    }

    fn vertical_fallback(&self) -> VerticalGlyph {
        self.vertical_by_id(0)
            .unwrap_or_else(|| VerticalGlyph::upright(self.fallback, &self.metrics))
    }

    /// `GPOS` か `kern` の表の値。どちらかの文字がフォントになければ詰めない
    fn kerning(&self, left: char, right: char) -> f64 {
        match (self.id(left), self.id(right)) {
            (Some(left), Some(right)) => f64::from(self.kerning.value(left, right)),
            _ => 0.0,
        }
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
            b"GPOS" => &mut tables.gpos,
            b"kern" => &mut tables.kern,
            b"vhea" => &mut tables.vhea,
            b"vmtx" => &mut tables.vmtx,
            b"GSUB" => &mut tables.gsub,
            b"CFF " | b"CFF2" => {
                let (slot, name) = if d == b' ' {
                    (&mut tables.cff, "CFF ")
                } else {
                    (&mut tables.cff2, "CFF2")
                };
                tables.cff_tag = tables.cff_tag.or(Some(name));
                slot
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
        // 外形が CFF のフォントで、CFF の表が見出しだけ
        let mut cff: Tables = tables()
            .into_iter()
            .filter(|(tag, _)| tag != b"glyf" && tag != b"loca")
            .collect();
        cff.push((*b"CFF ", vec![1, 0, 4, 4]));
        assert_eq!(reason(&assemble(cff)), Unread::Table("CFF "));
    }

    #[test]
    fn survives_truncated_and_corrupted_bytes() {
        let probe = |bytes: &[u8]| {
            if let Ok(font) = Font::parse(bytes, 0) {
                let _ = font.metrics();
                let _ = font.fallback();
                for ch in [' ', 'H', 'g', 'X', 'Z', 'Q'] {
                    let _ = font.glyph(ch);
                    for right in ['H', 'g'] {
                        let _ = font.kerning(ch, right);
                    }
                    let _ = font.vertical(ch);
                }
                let _ = font.vertical_fallback();
            }
        };
        let mut kerned = tables();
        kerned.push((*b"GPOS", gpos(&[])));
        kerned.push((*b"kern", kern()));
        kerned.extend(vertical_tables(&gsub(
            &[(b"vert", &[0, 1]), (b"liga", &[2])],
            1,
        )));
        // TrueType・CFF・CFF2・カーニングの表のあるフォント
        for data in [
            assemble(tables()),
            assemble(cff_font(cff_table())),
            assemble(cff_font(cff2())),
            assemble(kerned),
        ] {
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

    /// CFF の INDEX。位置の大きさは、いちばん大きい位置に合わせる。`long` なら数は 32 ビット（CFF2）
    fn cff_index(objects: &[Vec<u8>], long: bool) -> Vec<u8> {
        let mut out = Vec::new();
        if long {
            put32(&mut out, objects.len() as u32);
        } else {
            put16(&mut out, objects.len() as i32);
        }
        if objects.is_empty() {
            return out;
        }
        let last = 1 + objects.iter().map(Vec::len).sum::<usize>();
        let size = match last {
            0..=0xFF => 1,
            0x100..=0xFFFF => 2,
            _ => 4,
        };
        out.push(size as u8);
        let mut offset = 1usize;
        for object in std::iter::once(&Vec::new()).chain(objects) {
            offset += object.len();
            out.extend_from_slice(&(offset as u32).to_be_bytes()[4 - size..]);
        }
        for object in objects {
            out.extend_from_slice(object);
        }
        out
    }

    /// DICT の整数（5 バイトの形にし、値によらず長さを決める）と演算子
    fn dict(entries: &[(&[i32], &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        for (operands, op) in entries {
            for &value in *operands {
                out.push(29);
                out.extend_from_slice(&value.to_be_bytes());
            }
            out.extend_from_slice(op);
        }
        out
    }

    /// 字形の命令を、数と演算子の名前を空白で区切った文から組み立てる。`0x` で始まる語は生のバイト
    fn charstring(program: &str) -> Vec<u8> {
        let mut out = Vec::new();
        for token in program.split_whitespace() {
            if let Some(hex) = token.strip_prefix("0x") {
                out.push(u8::from_str_radix(hex, 16).unwrap());
            } else if let Ok(value) = token.parse::<i32>() {
                match value {
                    -107..=107 => out.push((value + 139) as u8),
                    108..=1131 => {
                        out.push(((value - 108) / 256 + 247) as u8);
                        out.push(((value - 108) % 256) as u8);
                    }
                    -1131..=-108 => {
                        out.push(((-value - 108) / 256 + 251) as u8);
                        out.push(((-value - 108) % 256) as u8);
                    }
                    _ => {
                        out.push(28);
                        out.extend_from_slice(&(value as i16).to_be_bytes());
                    }
                }
            } else if let Ok(value) = token.parse::<f64>() {
                out.push(255);
                out.extend_from_slice(&((value * 65536.0) as i32).to_be_bytes());
            } else {
                let op: &[u8] = match token {
                    "hstem" => &[1],
                    "vstem" => &[3],
                    "vmoveto" => &[4],
                    "rlineto" => &[5],
                    "hlineto" => &[6],
                    "vlineto" => &[7],
                    "rrcurveto" => &[8],
                    "callsubr" => &[10],
                    "return" => &[11],
                    "endchar" => &[14],
                    "blend" => &[16],
                    "hstemhm" => &[18],
                    "hintmask" => &[19],
                    "rmoveto" => &[21],
                    "hmoveto" => &[22],
                    "rcurveline" => &[24],
                    "rlinecurve" => &[25],
                    "vvcurveto" => &[26],
                    "hhcurveto" => &[27],
                    "callgsubr" => &[29],
                    "vhcurveto" => &[30],
                    "hvcurveto" => &[31],
                    "add" => &[12, 10],
                    "hflex" => &[12, 34],
                    "flex" => &[12, 35],
                    "hflex1" => &[12, 36],
                    "flex1" => &[12, 37],
                    _ => panic!("{token}"),
                };
                out.extend_from_slice(op);
            }
        }
        out
    }

    /// CFF の字形 4 つ。0 は .notdef（送りの引数つき、外形 50〜450 × 0〜700）、1 は `H`（ヒントと
    /// hintmask つき、外形 0〜700 × 0〜720）、2 は `g`（局所と全体の副ルーチンで、制御点が 400 まで
    /// 上がる曲線を描く。外形 100〜400 × 0〜300）、3 は空白（送りの引数だけ）
    fn glyphs() -> Vec<String> {
        [
            "500 50 0 rmoveto 400 700 -400 hlineto endchar",
            "0 50 100 50 hstemhm 0 700 hintmask 0xE0 0 0 rmoveto 700 720 -700 hlineto endchar",
            "-107 callsubr -107 callgsubr endchar",
            "250 endchar",
        ]
        .map(str::to_owned)
        .to_vec()
    }

    /// `glyphs` の字形と、`g` が呼ぶ副ルーチンを持つ CFF の表
    fn cff_table() -> Vec<u8> {
        cff(
            &glyphs(),
            &["0 400 300 0 0 -400 rrcurveto return"],
            &["100 0 rmoveto return"],
            &[],
        )
    }

    /// CFF の表。`top` は Top DICT に足す項目
    fn cff(glyphs: &[String], globals: &[&str], locals: &[&str], top: &[u8]) -> Vec<u8> {
        let objects = |programs: &mut dyn Iterator<Item = &str>| -> Vec<Vec<u8>> {
            programs.map(charstring).collect()
        };
        let charstrings = cff_index(&objects(&mut glyphs.iter().map(String::as_str)), false);
        let globals = cff_index(&objects(&mut globals.iter().copied()), false);
        let locals = cff_index(&objects(&mut locals.iter().copied()), false);
        let private = dict(&[(&[6], &[19])]);
        let top_dict = |at: i32| {
            let mut out = dict(&[
                (&[at], &[17]),
                (
                    &[private.len() as i32, at + charstrings.len() as i32],
                    &[18],
                ),
            ]);
            out.extend_from_slice(top);
            out
        };
        let mut head = vec![1, 0, 4, 4];
        head.extend(cff_index(&[b"Test".to_vec()], false));
        let at = head.len()
            + cff_index(&[top_dict(0)], false).len()
            + cff_index(&[], false).len()
            + globals.len();
        head.extend(cff_index(&[top_dict(at as i32)], false));
        head.extend(cff_index(&[], false));
        head.extend(globals);
        head.extend(charstrings);
        head.extend(private);
        head.extend(locals);
        head
    }

    /// CFF2 の表。字形は CFF と同じ形で、Font DICT は 2 つ。字形 0・1 は Font DICT 0、2・3 は 1 で、
    /// `g` は Font DICT 1 の局所の副ルーチンで描く。`H` は blend の差分を含む
    fn cff2() -> Vec<u8> {
        let programs = [
            "50 0 rmoveto 400 700 -400 hlineto",
            "0 0 5 5 5 5 2 blend rmoveto 700 720 -700 hlineto",
            "-107 callsubr",
            "",
        ];
        let charstrings = cff_index(&programs.map(charstring), true);
        let subrs = [
            cff_index(&[charstring("5 5 rlineto")], true),
            cff_index(
                &[charstring("100 0 rmoveto 0 400 300 0 0 -400 rrcurveto")],
                true,
            ),
        ];
        // FDSelect の形式 3: 区間 2 つと番兵
        let mut select = vec![3];
        for value in [2, 0] {
            put16(&mut select, value);
        }
        select.push(0);
        put16(&mut select, 2);
        select.push(1);
        put16(&mut select, 4);
        // vstore: 長さと ItemVariationStore（ItemVariationData が 1 つで、領域は 2 つ）
        let mut store = Vec::new();
        for value in [1, 0, 0] {
            put16(&mut store, value);
        }
        put16(&mut store, 1);
        put32(&mut store, 12);
        for value in [0, 0, 2, 0, 0] {
            put16(&mut store, value);
        }
        let mut vstore = Vec::new();
        put16(&mut vstore, store.len() as i32);
        vstore.extend(store);

        let build = |at: [i32; 4], privates: [i32; 2]| {
            let top = dict(&[
                (&[at[0]], &[17]),
                (&[at[1]], &[12, 36]),
                (&[at[2]], &[12, 37]),
                (&[at[3]], &[24]),
            ]);
            let private = dict(&[(&[6], &[19])]);
            let fonts = cff_index(
                &[
                    dict(&[(&[private.len() as i32, privates[0]], &[18])]),
                    dict(&[(&[private.len() as i32, privates[1]], &[18])]),
                ],
                true,
            );
            let mut out = vec![2, 0, 5];
            put16(&mut out, top.len() as i32);
            out.extend(top);
            out.extend(cff_index(&[], true));
            let mut at = [0; 4];
            at[0] = out.len() as i32;
            out.extend(&charstrings);
            at[1] = out.len() as i32;
            out.extend(fonts);
            at[2] = out.len() as i32;
            out.extend(&select);
            at[3] = out.len() as i32;
            out.extend(&vstore);
            let mut privates = [0; 2];
            for (i, subrs) in subrs.iter().enumerate() {
                privates[i] = out.len() as i32;
                out.extend(&private);
                out.extend(subrs);
            }
            (out, at, privates)
        };
        let (_, at, privates) = build([0; 4], [0; 2]);
        build(at, privates).0
    }

    /// TrueType のフォントの `glyf`・`loca` を、CFF か CFF2 の表に置き換える
    fn cff_font(table: Vec<u8>) -> Tables {
        let tag = if table[0] == 2 { *b"CFF2" } else { *b"CFF " };
        let mut tables: Tables = tables()
            .into_iter()
            .filter(|(tag, _)| tag != b"glyf" && tag != b"loca")
            .collect();
        tables.push((tag, table));
        tables
    }

    /// CFF のフォントの字形 2（`g`）を `program` に替え、その外形を読む。読めなければ `None`
    fn cff_bounds(program: &str, globals: &[&str], locals: &[&str]) -> Option<Option<[f64; 4]>> {
        let mut glyphs = glyphs();
        glyphs[2] = program.to_owned();
        let data = assemble(cff_font(cff(&glyphs, globals, locals, &[])));
        let font = Font::parse(&data, 0).unwrap();
        font.glyph('g').map(|glyph| glyph.bounds)
    }

    #[test]
    fn reads_cff_outlines() {
        for table in [cff_table(), cff2()] {
            let data = assemble(cff_font(table));
            let font = Font::parse(&data, 0).unwrap();
            assert_eq!(
                font.glyph('H'),
                Some(Glyph {
                    advance: 700.0,
                    bounds: Some([0.0, 0.0, 700.0, 720.0]),
                })
            );
            // 制御点は 400 まで上がるが、曲線の頂は 300
            assert_eq!(
                font.glyph('g'),
                Some(Glyph {
                    advance: 550.0,
                    bounds: Some([100.0, 0.0, 400.0, 300.0]),
                })
            );
            assert_eq!(
                font.glyph(' '),
                Some(Glyph {
                    advance: 250.0,
                    bounds: None,
                })
            );
            assert_eq!(font.fallback().bounds, Some([50.0, 0.0, 450.0, 700.0]));
            assert_eq!(font.metrics().cap_height, 700.0);
        }
        // OS/2 がなければ、大文字の高さは CFF の H の外形の上
        let tables: Tables = cff_font(cff_table())
            .into_iter()
            .filter(|(tag, _)| tag != b"OS/2")
            .collect();
        let data = assemble(tables);
        assert_eq!(Font::parse(&data, 0).unwrap().metrics().cap_height, 720.0);
    }

    #[test]
    fn reads_every_cff_drawing_operator() {
        // 省略形の演算子は、同じ曲線を rrcurveto・rlineto で書いたものと同じ外形になる
        let pairs = [
            (
                "1 2 3 4 5 6 7 8 9 hvcurveto",
                "1 0 2 3 0 4 0 5 6 7 8 9 rrcurveto",
            ),
            (
                "1 2 3 4 5 6 7 8 9 vhcurveto",
                "0 1 2 3 4 0 5 0 6 7 9 8 rrcurveto",
            ),
            (
                "1 2 3 4 5 6 7 8 9 vvcurveto",
                "1 2 3 4 0 5 0 6 7 8 0 9 rrcurveto",
            ),
            (
                "1 2 3 4 5 6 7 8 9 hhcurveto",
                "2 1 3 4 5 0 6 0 7 8 9 0 rrcurveto",
            ),
            (
                "1 2 3 4 5 6 7 8 rcurveline",
                "1 2 3 4 5 6 rrcurveto 7 8 rlineto",
            ),
            (
                "1 2 3 4 5 6 7 8 rlinecurve",
                "1 2 rlineto 3 4 5 6 7 8 rrcurveto",
            ),
            (
                "1 2 3 4 5 6 7 8 9 10 11 12 50 flex",
                "1 2 3 4 5 6 7 8 9 10 11 12 rrcurveto",
            ),
            ("1 2 3 4 5 6 7 hflex", "1 0 2 3 4 0 5 0 6 -3 7 0 rrcurveto"),
            (
                "1 2 3 4 5 6 7 8 9 hflex1",
                "1 2 3 4 5 0 6 0 7 8 9 -14 rrcurveto",
            ),
            (
                "1 2 3 4 5 6 7 8 9 10 11 flex1",
                "1 2 3 4 5 6 7 8 9 10 -25 11 rrcurveto",
            ),
            (
                "-30 -40 rlineto 70 hlineto 90 vlineto",
                "-30 -40 rlineto 70 0 0 90 rlineto",
            ),
        ];
        for (short, long) in pairs {
            let program = |body: &str| format!("0 0 rmoveto {body} endchar");
            let expected = cff_bounds(&program(long), &[], &[]).unwrap();
            assert!(expected.is_some(), "{long}");
            assert_eq!(
                cff_bounds(&program(short), &[], &[]),
                Some(expected),
                "{short}"
            );
        }
        // 1 バイト・2 バイト・3 バイトの整数と、固定小数点の数
        assert_eq!(
            cff_bounds("10 20 rmoveto 500 -500 2000 1000 rlineto endchar", &[], &[]),
            Some(Some([10.0, -480.0, 2510.0, 520.0]))
        );
        assert_eq!(
            cff_bounds("0.5 0.25 rlineto endchar", &[], &[]),
            Some(Some([0.0, 0.0, 0.5, 0.25]))
        );
        // hmoveto・vmoveto と、移動だけの字形
        assert_eq!(
            cff_bounds("5 hmoveto 6 vmoveto 1 1 rlineto endchar", &[], &[]),
            Some(Some([5.0, 6.0, 6.0, 7.0]))
        );
        assert_eq!(cff_bounds("5 hmoveto endchar", &[], &[]), Some(None));
    }

    #[test]
    fn limits_cff_nesting_stack_and_work() {
        // 副ルーチンを順に呼ぶ鎖。最後の副ルーチンが描く
        let chain = |depth: usize| {
            let mut locals: Vec<String> = (1..depth)
                .map(|i| format!("{} callsubr return", i as i32 - 107))
                .collect();
            locals.push("10 10 rlineto return".to_owned());
            locals
        };
        for (depth, expected) in [(10, Some(Some([0.0, 0.0, 10.0, 10.0]))), (11, None)] {
            let locals = chain(depth);
            let locals: Vec<&str> = locals.iter().map(String::as_str).collect();
            assert_eq!(
                cff_bounds("-107 callsubr endchar", &[], &locals),
                expected,
                "{depth}"
            );
        }
        // 自分を呼ぶ副ルーチン
        assert_eq!(
            cff_bounds("-107 callsubr endchar", &[], &["-107 callsubr return"]),
            None
        );
        // 16 回ずつ次を呼ぶ 10 段の副ルーチンは、作業量の上限で止まる
        let locals: Vec<String> = (0..10)
            .map(|i| {
                if i == 9 {
                    "1 1 rlineto return".to_owned()
                } else {
                    format!("{} return", format!("{} callsubr ", i - 106).repeat(16))
                }
            })
            .collect();
        let locals: Vec<&str> = locals.iter().map(String::as_str).collect();
        assert_eq!(cff_bounds("-107 callsubr endchar", &[], &locals), None);
        // 引数のスタックは 48 まで
        let numbers = |count: usize| "1 ".repeat(count);
        assert!(cff_bounds(&format!("{}hstem endchar", numbers(48)), &[], &[]).is_some());
        assert_eq!(
            cff_bounds(&format!("{}hstem endchar", numbers(49)), &[], &[]),
            None
        );
    }

    #[test]
    fn rejects_broken_cff_glyphs_and_tables() {
        for program in [
            // 引数の数が合わない
            "1 2 3 rlineto endchar",
            "1 2 3 4 5 rrcurveto endchar",
            "1 2 3 flex endchar",
            // ない副ルーチン・return の多すぎ
            "-106 callsubr endchar",
            "return",
            // 廃止された seac と算術の演算子
            "0 0 0 65 66 endchar",
            "1 2 add endchar",
            // hintmask の印が字形の外に出る
            "0 10 hstem hintmask",
            // CFF2 の blend は vstore がなければ読めない
            "1 2 1 blend rmoveto endchar",
        ] {
            assert_eq!(cff_bounds(program, &[], &[]), None, "{program}");
        }
        // 字形の命令が Type 2 でない（CharstringType が 1）
        let data = assemble(cff_font(cff(
            &glyphs(),
            &[],
            &[],
            &dict(&[(&[1], &[12, 6])]),
        )));
        assert_eq!(reason(&data), Unread::Cff);
        // .notdef が読めなければ、CFF の表が壊れている
        let mut broken = glyphs();
        broken[0] = "1 rlineto endchar".to_owned();
        let data = assemble(cff_font(cff(&broken, &[], &[], &[])));
        assert_eq!(reason(&data), Unread::Table("CFF "));
        // CFF2 の表がファイルの外を指す
        let mut tables = cff_font(cff2());
        tables.last_mut().unwrap().1.truncate(3);
        assert_eq!(reason(&assemble(tables)), Unread::Table("CFF2"));
    }

    /// `kern` の表: 形式 0 の副表 1 つに、`H` `g` を -50、`g` `H` を 30 にする組
    fn kern() -> Vec<u8> {
        let pairs = [(1, 2, -50), (2, 1, 30)];
        let mut out = Vec::new();
        for value in [
            0,
            1,
            0,
            14 + 6 * pairs.len() as i32,
            0x0001,
            pairs.len() as i32,
            0,
            0,
            0,
        ] {
            put16(&mut out, value);
        }
        for (left, right, value) in pairs {
            for value in [left, right, value] {
                put16(&mut out, value);
            }
        }
        out
    }

    /// GPOS の表。`kern` の機能は lookup 0（extension を通した形式 1: `H` `g` を -40）と lookup 1
    /// （形式 2: 組 (H, H) を 15、(g, H) を -20）を指す。`liga` の機能の lookup 2 は使わない。
    /// `extra` を与えれば、lookup 1 の副表の位置をその数だけ並べる
    fn gpos(extra: &[()]) -> Vec<u8> {
        let words = |values: &[i32]| {
            let mut out = Vec::new();
            for &value in values {
                put16(&mut out, value);
            }
            out
        };
        // 形式 1: 見出し 12 バイト、coverage（12）、PairSet（18）。valueFormat1 は XPlacement と XAdvance
        let mut format1 = words(&[1, 12, 5, 0, 1, 18]);
        format1.extend(words(&[1, 1, 1]));
        format1.extend(words(&[1, 2, 7, -40]));
        let mut extension = words(&[1, 2]);
        put32(&mut extension, 8);
        extension.extend(&format1);
        // 形式 2: 見出し 16 バイト、組の値（16）、coverage（28）、ClassDef 1（38）、ClassDef 2（48）
        let mut format2 = words(&[2, 28, 4, 0, 38, 48, 3, 2]);
        format2.extend(words(&[0, 0, 0, 15, 0, -20]));
        format2.extend(words(&[2, 1, 1, 2, 0]));
        format2.extend(words(&[1, 1, 2, 1, 2]));
        format2.extend(words(&[2, 1, 1, 1, 1]));
        let mut liga = words(&[1, 12, 4, 0, 1, 18]);
        liga.extend(words(&[1, 1, 1]));
        liga.extend(words(&[1, 2, 1000]));
        let lookup = |kind: i32, subtable: &[u8], copies: usize| {
            let mut out = words(&[kind, 0, copies as i32]);
            let at = 6 + 2 * copies as i32;
            for _ in 0..copies {
                put16(&mut out, at);
            }
            out.extend(subtable);
            out
        };
        let lookups = [
            lookup(9, &extension, 1),
            lookup(2, &format2, 1 + extra.len()),
            lookup(2, &liga, 1),
        ];
        let mut list = words(&[3]);
        let mut at = 8;
        for lookup in &lookups {
            put16(&mut list, at);
            at += lookup.len() as i32;
        }
        for lookup in &lookups {
            list.extend(lookup);
        }
        let mut features = words(&[2]);
        features.extend(b"kern");
        put16(&mut features, 14);
        features.extend(b"liga");
        put16(&mut features, 22);
        features.extend(words(&[0, 2, 0, 1]));
        features.extend(words(&[0, 1, 2]));
        let mut out = words(&[1, 0, 0, 10, 10 + features.len() as i32]);
        out.extend(features);
        out.extend(list);
        out
    }

    #[test]
    fn reads_kern_table() {
        let mut tables = tables();
        tables.push((*b"kern", kern()));
        let data = assemble(tables);
        let font = Font::parse(&data, 0).unwrap();
        assert_eq!(font.kerning('H', 'g'), -50.0);
        assert_eq!(font.kerning('g', 'H'), 30.0);
        assert_eq!(font.kerning('H', 'H'), 0.0);
        // フォントにない文字は詰めない
        assert_eq!(font.kerning('Q', 'g'), 0.0);
        // 最小値の副表と、縦の向きの副表は使わない
        for coverage in [0x0003, 0x0005] {
            let mut table = kern();
            table[8..10].copy_from_slice(&(coverage as u16).to_be_bytes());
            let mut tables = self::tables();
            tables.push((*b"kern", table));
            let data = assemble(tables);
            assert_eq!(Font::parse(&data, 0).unwrap().kerning('H', 'g'), 0.0);
        }
    }

    #[test]
    fn reads_gpos_pair_adjustments_before_kern() {
        let mut tables = tables();
        tables.push((*b"GPOS", gpos(&[])));
        tables.push((*b"kern", kern()));
        let data = assemble(tables);
        let font = Font::parse(&data, 0).unwrap();
        // GPOS があれば kern の表は使わない。liga の機能の lookup は使わない
        assert_eq!(font.kerning('H', 'g'), -40.0);
        assert_eq!(font.kerning('H', 'H'), 15.0);
        assert_eq!(font.kerning('g', 'H'), -20.0);
        assert_eq!(font.kerning('g', 'g'), 0.0);
        assert_eq!(font.kerning(' ', 'H'), 0.0);

        // カーニングは配置の送りに効く
        let text = Text {
            content: "Hg".to_owned(),
            height: Height {
                value: Length::Paper(PaperLength(7.0)),
                meaning: HeightMeaning::CapHeight,
            },
            placement: Placement::Anchor {
                point: [0.0, 0.0],
                horizontal: Horizontal::Left,
                vertical: Vertical::Baseline,
            },
            rotation: 0.0,
            width_factor: 1.0,
            oblique: 0.0,
            backward: false,
            upside_down: false,
            style: String::new(),
        };
        let laid = place(&text, |p| *p, &font, |_| None::<Reference>).unwrap();
        assert_eq!(laid.pens, vec![0.0, 660.0]);

        // 副表が多すぎる GPOS は読まず、kern の表を使う
        let mut tables = self::tables();
        tables.push((*b"GPOS", gpos(&[(); 1024])));
        tables.push((*b"kern", kern()));
        let data = assemble(tables);
        assert_eq!(Font::parse(&data, 0).unwrap().kerning('H', 'g'), -50.0);
    }

    /// 16 ビットの値の並び
    fn halfwords(values: &[i32]) -> Vec<u8> {
        let mut out = Vec::new();
        for &value in values {
            put16(&mut out, value);
        }
        out
    }

    /// GSUB の表。lookup 0 は extension（type 7）を通した形式 1 で `H`（1）を +1 して `g`（2）に、
    /// lookup 1 は形式 2 で空白（3）を `H`（1）に、lookup 2 は形式 1 で `g` を -1 して `H` にする。
    /// lookup 1 には、同じ副表の位置を `copies` 個並べる。`features` は機能の名前と lookup の番号
    fn gsub(features: &[(&[u8; 4], &[i32])], copies: usize) -> Vec<u8> {
        let coverage = |glyph| halfwords(&[1, 1, glyph]);
        let mut format1 = halfwords(&[1, 6, 1]);
        format1.extend(coverage(1));
        let mut extension = halfwords(&[1, 1]);
        put32(&mut extension, 8);
        extension.extend(&format1);
        let mut format2 = halfwords(&[2, 8, 1, 1]);
        format2.extend(coverage(3));
        let mut back = halfwords(&[1, 6, -1]);
        back.extend(coverage(2));
        let lookup = |kind: i32, subtable: &[u8], copies: usize| {
            let mut out = halfwords(&[kind, 0, copies as i32]);
            for _ in 0..copies {
                put16(&mut out, 6 + 2 * copies as i32);
            }
            out.extend(subtable);
            out
        };
        let lookups = [
            lookup(7, &extension, 1),
            lookup(1, &format2, copies),
            lookup(1, &back, 1),
        ];
        let mut list = halfwords(&[lookups.len() as i32]);
        let mut at = 2 + 2 * lookups.len() as i32;
        for lookup in &lookups {
            put16(&mut list, at);
            at += lookup.len() as i32;
        }
        for lookup in &lookups {
            list.extend(lookup);
        }
        let mut records = halfwords(&[features.len() as i32]);
        let mut bodies = Vec::new();
        let mut at = 2 + 6 * features.len() as i32;
        for (tag, indices) in features {
            records.extend(*tag);
            put16(&mut records, at);
            let mut body = halfwords(&[0, indices.len() as i32]);
            body.extend(halfwords(indices));
            at += body.len() as i32;
            bodies.extend(body);
        }
        records.extend(bodies);
        let mut out = halfwords(&[1, 0, 0, 10, 10 + records.len() as i32]);
        out.extend(records);
        out.extend(list);
        out
    }

    /// `vhea`・`vmtx`・`GSUB`。縦の送りは字形 0 が 1000、1 が 900（numOfLongVerMetrics は 2）で、
    /// 上の余白は 100・80・50・0
    fn vertical_tables(gsub: &[u8]) -> Tables {
        let mut vhea = vec![0; 36];
        vhea[0..4].copy_from_slice(&0x0001_1000u32.to_be_bytes());
        vhea[34..36].copy_from_slice(&2u16.to_be_bytes());
        let vmtx = halfwords(&[1000, 100, 900, 80, 50, 0]);
        vec![
            (*b"vhea", vhea),
            (*b"vmtx", vmtx),
            (*b"GSUB", gsub.to_vec()),
        ]
    }

    fn with_vertical(gsub: &[u8]) -> Vec<u8> {
        let mut tables = tables();
        tables.extend(vertical_tables(gsub));
        assemble(tables)
    }

    #[test]
    fn reads_vertical_metrics_and_substitutions() {
        let data = with_vertical(&gsub(&[(b"vert", &[0, 1]), (b"liga", &[2])], 1));
        let font = Font::parse(&data, 0).unwrap();
        // H は g に置き換え、g の縦の送り 900（最後の送り）と、外形の上 480 に上の余白 50 を足した原点
        assert_eq!(
            font.vertical('H'),
            Some(VerticalGlyph {
                glyph: font.glyph('g').unwrap(),
                advance: 900.0,
                top: 530.0,
                substituted: true,
            })
        );
        // 空白は H に置き換え、H の上の余白 80
        let space = font.vertical(' ').unwrap();
        assert_eq!([space.advance, space.top], [900.0, 800.0]);
        assert!(space.substituted);
        // liga の lookup は使わない
        assert!(!font.vertical('g').unwrap().substituted);
        assert_eq!(font.vertical('Q'), None);
        let fallback = font.vertical_fallback();
        assert_eq!([fallback.advance, fallback.top], [1000.0, 800.0]);

        // vrt2 があれば vert は使わない
        let data = with_vertical(&gsub(&[(b"vert", &[0]), (b"vrt2", &[1])], 1));
        let font = Font::parse(&data, 0).unwrap();
        assert!(!font.vertical('H').unwrap().substituted);
        assert!(font.vertical(' ').unwrap().substituted);

        // 縦書きの表がなければ、送りはアセンダーとディセンダーの和、原点はアセンダー
        let data = assemble(tables());
        let font = Font::parse(&data, 0).unwrap();
        let glyph = font.vertical('H').unwrap();
        assert_eq!([glyph.advance, glyph.top], [1150.0, 900.0]);
        assert!(!glyph.substituted);
    }

    #[test]
    fn ignores_broken_or_oversized_vertical_tables() {
        // vmtx が字形の数だけの記録を持たなければ使わない
        let mut tables = tables();
        let mut vertical = vertical_tables(&gsub(&[(b"vert", &[0])], 1));
        vertical[1].1.truncate(10);
        tables.extend(vertical);
        let data = assemble(tables);
        let glyph = Font::parse(&data, 0).unwrap().vertical('g').unwrap();
        assert_eq!([glyph.advance, glyph.top], [1150.0, 900.0]);

        // 副表が 1024 を超える GSUB は読まない
        let data = with_vertical(&gsub(&[(b"vert", &[0, 1])], 1024));
        let font = Font::parse(&data, 0).unwrap();
        assert!(!font.vertical('H').unwrap().substituted);
        let data = with_vertical(&gsub(&[(b"vert", &[0, 1])], 1023));
        assert!(
            Font::parse(&data, 0)
                .unwrap()
                .vertical('H')
                .unwrap()
                .substituted
        );

        // 置き換えた先が字形の数を超えれば置き換えない。lookup の番号が lookup の数を超えれば読まない
        let mut table = gsub(&[(b"vert", &[0])], 1);
        for broken in [gsub(&[(b"vert", &[5])], 1), {
            // lookup 0 の差分を +100 にする
            let needle = halfwords(&[1, 6, 1, 1, 1, 1]);
            let position = table
                .windows(needle.len())
                .position(|window| window == needle.as_slice())
                .unwrap();
            table[position + 4..position + 6].copy_from_slice(&100u16.to_be_bytes());
            table
        }] {
            let data = with_vertical(&broken);
            let font = Font::parse(&data, 0).unwrap();
            assert!(!font.vertical('H').unwrap().substituted);
        }
    }

    /// 単純な字形を符号にする。点は（x, y, 曲線の上か）、`ends` は輪郭の終わりの点の番号。差が 0 なら
    /// 同じ値の旗、255 以下なら 1 バイト、それより大きければ 2 バイトにし、同じ旗は繰り返しにまとめる
    pub(crate) fn simple_glyph(
        bounds: [i32; 4],
        points: &[(i32, i32, bool)],
        ends: &[u16],
    ) -> Vec<u8> {
        let mut out = Vec::new();
        put16(&mut out, ends.len() as i32);
        for value in bounds {
            put16(&mut out, value);
        }
        for &end in ends {
            put16(&mut out, i32::from(end));
        }
        put16(&mut out, 0); // 命令はない
        let (mut flags, mut xs, mut ys) = (Vec::new(), Vec::new(), Vec::new());
        let (mut x0, mut y0) = (0, 0);
        for &(x, y, on) in points {
            let mut flag = u8::from(on);
            for (delta, short, same, out) in
                [(x - x0, 0x02, 0x10, &mut xs), (y - y0, 0x04, 0x20, &mut ys)]
            {
                if delta == 0 {
                    flag |= same;
                } else if delta.abs() < 256 {
                    flag |= short;
                    if delta > 0 {
                        flag |= same;
                    }
                    out.push(delta.unsigned_abs() as u8);
                } else {
                    put16(out, delta);
                }
            }
            (x0, y0) = (x, y);
            flags.push(flag);
        }
        let mut i = 0;
        while i < flags.len() {
            let run = flags[i..]
                .iter()
                .take_while(|f| **f == flags[i])
                .count()
                .min(256);
            if run > 1 {
                out.push(flags[i] | 0x08);
                out.push((run - 1) as u8);
            } else {
                out.push(flags[i]);
            }
            i += run;
        }
        out.extend_from_slice(&xs);
        out.extend_from_slice(&ys);
        out
    }

    /// [`tables`] の字形に輪郭を持たせたフォント。0 は四角、1 の `H` は直線だけの 2 つの輪郭、2 の `g` は
    /// 曲線の外の点が続く輪郭、3 は空白。4 は `H` を半分にして (10, 20) へ動かし、`g` を (-5, 3) へ動かした
    /// 複合の字形で、`I` に当てる
    pub(crate) fn outlined() -> Vec<u8> {
        let notdef = simple_glyph(
            [50, 0, 450, 700],
            &[
                (50, 0, true),
                (450, 0, true),
                (450, 700, true),
                (50, 700, true),
            ],
            &[3],
        );
        let h = simple_glyph(
            [0, 0, 700, 720],
            &[
                (0, 0, true),
                (100, 0, true),
                (100, 720, true),
                (0, 720, true),
                (600, 0, true),
                (700, 0, true),
                (700, 720, true),
                (600, 720, true),
            ],
            &[3, 7],
        );
        // 外の点だけの輪郭（菱形の 4 点）。始点は最初と最後の中点
        let g = simple_glyph(
            [30, -210, 520, 480],
            &[
                (275, -210, false),
                (520, 135, false),
                (275, 480, false),
                (30, 135, false),
            ],
            &[3],
        );
        let mut composite = Vec::new();
        put16(&mut composite, -1);
        for value in [0, 0, 700, 720] {
            put16(&mut composite, value);
        }
        // ARG_1_AND_2_ARE_WORDS・ARGS_ARE_XY_VALUES・WE_HAVE_A_SCALE・MORE_COMPONENTS
        put16(&mut composite, 0x0001 | 0x0002 | 0x0008 | 0x0020);
        put16(&mut composite, 1);
        put16(&mut composite, 10);
        put16(&mut composite, 20);
        put16(&mut composite, 0x2000); // 0.5
        put16(&mut composite, 0x0002);
        put16(&mut composite, 2);
        composite.push(-5i8 as u8);
        composite.push(3);
        let mut glyf = Vec::new();
        let mut loca = Vec::new();
        for glyph in [notdef, h, g, Vec::new(), composite] {
            put32(&mut loca, glyf.len() as u32);
            glyf.extend_from_slice(&glyph);
            while glyf.len() % 4 != 0 {
                glyf.push(0);
            }
        }
        put32(&mut loca, glyf.len() as u32);
        let groups: [(u32, u32); 4] = [(0x20, 3), (0x48, 1), (0x49, 4), (0x67, 2)];
        let mut cmap = Vec::new();
        put16(&mut cmap, 0);
        put16(&mut cmap, 1);
        put16(&mut cmap, 3);
        put16(&mut cmap, 10);
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
        let mut tables = tables();
        for (tag, data) in &mut tables {
            match &*tag {
                b"glyf" => *data = glyf.clone(),
                b"loca" => *data = loca.clone(),
                b"cmap" => *data = cmap.clone(),
                b"maxp" => data[4..6].copy_from_slice(&5u16.to_be_bytes()),
                _ => {}
            }
        }
        assemble(tables)
    }

    #[test]
    fn reads_glyf_outlines() {
        let data = outlined();
        let font = Font::parse(&data, 0).unwrap();
        assert_eq!(font.glyph_id('H'), Some(1));
        assert_eq!(font.glyph_id('Q'), None);
        let line = |x: f64, y: f64| Segment::Line([x, y]);
        let h = font.outline(1).unwrap();
        assert_eq!(
            h,
            vec![
                Contour {
                    start: [0.0, 0.0],
                    segments: vec![line(100.0, 0.0), line(100.0, 720.0), line(0.0, 720.0)],
                },
                Contour {
                    start: [600.0, 0.0],
                    segments: vec![line(700.0, 0.0), line(700.0, 720.0), line(600.0, 720.0)],
                },
            ]
        );
        let g = font.outline(2).unwrap();
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].start, [152.5, -37.5]);
        assert_eq!(
            g[0].segments,
            vec![
                Segment::Quad([275.0, -210.0], [397.5, -37.5]),
                Segment::Quad([520.0, 135.0], [397.5, 307.5]),
                Segment::Quad([275.0, 480.0], [152.5, 307.5]),
                Segment::Quad([30.0, 135.0], [152.5, -37.5]),
            ]
        );
        assert_eq!(font.outline(3), Some(Vec::new()));
        let composite = font.outline(4).unwrap();
        assert_eq!(composite.len(), 3);
        assert_eq!(composite[0].start, [10.0, 20.0]);
        assert_eq!(composite[1].segments[0], line(360.0, 20.0));
        assert_eq!(composite[2].start, [147.5, -34.5]);
        assert_eq!(font.outline(5), None);
    }

    #[test]
    fn rejects_broken_and_deep_glyf_outlines() {
        // 自分を参照する複合の字形は、入れ子の上限で止まる
        let mut data = outlined();
        let font = Font::parse(&data, 0).unwrap();
        assert!(font.outline(4).is_some());
        let at = data
            .windows(4)
            .position(|w| w == [0x00, 0x2B, 0x00, 0x01])
            .unwrap();
        data[at + 3] = 4;
        let font = Font::parse(&data, 0).unwrap();
        assert_eq!(font.outline(4), None);
        // 切り詰めても、1 バイトずつ変えても、パニックしない
        let data = outlined();
        for cut in 0..data.len() {
            if let Ok(font) = Font::parse(&data[..cut], 0) {
                for id in 0..6 {
                    let _ = font.outline(id);
                }
            }
        }
        for at in 0..data.len() {
            let mut broken = data.clone();
            broken[at] ^= 0xA5;
            if let Ok(font) = Font::parse(&broken, 0) {
                for id in 0..6 {
                    let _ = font.outline(id);
                }
            }
        }
    }

    #[test]
    fn reads_cff_outline_paths() {
        for table in [cff_table(), cff2()] {
            let data = assemble(cff_font(table));
            let font = Font::parse(&data, 0).unwrap();
            let h = font.outline(font.glyph_id('H').unwrap()).unwrap();
            assert!(!h.is_empty());
            assert!(
                h.iter()
                    .all(|c| c.segments.iter().all(|s| matches!(s, Segment::Line(_))))
            );
            let g = font.outline(font.glyph_id('g').unwrap()).unwrap();
            assert!(
                g.iter()
                    .flat_map(|c| &c.segments)
                    .any(|s| matches!(s, Segment::Cubic(..)))
            );
            assert_eq!(font.vertical_id(1), 1);
        }
    }
}
