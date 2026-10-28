//! 見出しの変数の区切り（ODS 5.4.1 の 9）。今は、用紙空間とモデル空間のブロックのレコードの
//! ハンドルと、図面モデルへの変換に使う変数（INSUNITS・PLIMMIN・PLIMMAX・PDMODE）だけを取り出す。
//! どちらの空間の図形も、所有者のハンドルを持たない（entmode が 1・2）ので、所有者はこのハンドルで決まる。

use super::bits::{BitWriter, Bits, crc16};
use super::{OBJECT_CRC_SEED, Release, crc_invalid, le16, le32};
use core::ops::Range;
use zumen_base::{Cx, Diagnostic, Ref, SpecRef, codes};

const HEADER_VARIABLES: SpecRef = SpecRef {
    spec: "ods_dwg_5_4_1",
    section: "9 Data section AcDb:Header (HEADER VARIABLES)",
};

/// 区切りの記録の番号
pub(super) const NUMBER: u8 = 0;

/// 区切りの始まりの印
const START: [u8; 16] = [
    0xCF, 0x7B, 0x1F, 0x23, 0xFD, 0xDE, 0x38, 0xA9, 0x5F, 0x7C, 0x68, 0xB8, 0x4E, 0x6D, 0x33, 0x5F,
];

/// 区切りの終わりの印
const END: [u8; 16] = [
    0x30, 0x84, 0xE0, 0xDC, 0x02, 0x21, 0xC7, 0x56, 0xA0, 0x83, 0x97, 0x47, 0xB1, 0x92, 0xCC, 0xA0,
];

const PART: &str = "header variables";

/// 取り出した見出しの変数
#[derive(Clone, Copy, Debug)]
pub(super) struct Header {
    /// 用紙空間（`*Paper_Space`）のブロックのレコードのハンドル
    pub(super) paper: u64,
    /// モデル空間（`*Model_Space`）のブロックのレコードのハンドル
    pub(super) model: u64,
    /// INSUNITS
    pub(super) units: i16,
    /// 用紙空間の PLIMMIN（X・Y）と PLIMMAX（X・Y）
    pub(super) limits: [f64; 4],
    /// PDMODE（点の表し方）
    pub(super) point_mode: i16,
    /// データの中の、PLIMMIN と INSUNITS のビットの位置
    limits_at: usize,
    units_at: usize,
}

/// 読み飛ばす変数の型
#[derive(Clone, Copy)]
enum Field {
    B,
    Bs,
    Bl,
    Bd,
    Rd,
    Tv,
    H,
    /// R2007 からも主のデータにあるハンドル（HANDSEED）
    Seed,
    /// 色。R2000 は BS
    Cmc,
    Rc,
}

use Field::{B, Bd, Bl, Bs, Cmc, H, Rc, Rd, Seed, Tv};

/// 空間ごとの変数: INSBASE・EXTMIN・EXTMAX、LIMMIN・LIMMAX、ELEVATION・UCSORG・UCSXDIR・UCSYDIR、
/// UCSNAME・UCSORTHOREF、UCSORTHOVIEW、UCSBASE、6 つの向きの UCS の原点
const SPACE: [(Field, usize); 7] = [
    (Bd, 9),
    (Rd, 4),
    (Bd, 10),
    (H, 2),
    (Bs, 1),
    (H, 1),
    (Bd, 18),
];

/// 空間ごとの変数より前の変数の並び（型と数）
const FIRST: [(Field, usize); 13] = [
    // 不明な値
    (Bd, 4),
    (Tv, 4),
    (Bl, 2),
    // 今のビューポートの図形の見出し
    (H, 1),
    // DIMASO から PELLIPSE まで
    (B, 20),
    // PROXYGRAPHICS から TEXTQLTY まで
    (Bs, 27),
    // LTSCALE から CELTSCALE まで
    (Bd, 21),
    // MENUNAME
    (Tv, 1),
    // TDCREATE・TDUPDATE・TDINDWG・TDUSRTIMER（日とミリ秒）
    (Bl, 8),
    // CECOLOR
    (Cmc, 1),
    // HANDSEED、CLAYER・TEXTSTYLE・CELTYPE・DIMSTYLE・CMLSTYLE
    (Seed, 1),
    (H, 5),
    // PSVPSCALE
    (Bd, 1),
];

/// R2004 の、空間ごとの変数より前の変数の並び。R2000 と比べて、今のビューポートの図形の見出しが
/// なく、LIMCHECK と USRTIMER の間に B が 1 つ、PDMODE と USERI1 の間と TDUPDATE と TDINDWG の間に
/// BL が 3 つずつある（どれも使い道は分からない。試料の DWG と DXF の比較で確かめた）
const FIRST_R2004: [(Field, usize); 16] = [
    (Bd, 4),
    (Tv, 4),
    (Bl, 2),
    // DIMASO から PELLIPSE まで
    (B, 21),
    // PROXYGRAPHICS から PDMODE まで
    (Bs, 8),
    (Bl, 3),
    // USERI1 から TEXTQLTY まで
    (Bs, 19),
    // LTSCALE から CELTSCALE まで
    (Bd, 21),
    // MENUNAME
    (Tv, 1),
    // TDCREATE・TDUPDATE（日とミリ秒）、不明な 3 つ、TDINDWG・TDUSRTIMER
    (Bl, 4),
    (Bl, 3),
    (Bl, 4),
    // CECOLOR
    (Cmc, 1),
    // HANDSEED、CLAYER・TEXTSTYLE・CELTYPE・DIMSTYLE・CMLSTYLE
    (Seed, 1),
    (H, 5),
    // PSVPSCALE
    (Bd, 1),
];

/// 空間ごとの変数より後の、寸法の変数の並び
const LAST: [(Field, usize); 16] = [
    // DIMPOST・DIMAPOST
    (Tv, 2),
    // DIMSCALE から DIMTM まで
    (Bd, 9),
    // DIMTOL から DIMSE2 まで
    (B, 6),
    // DIMTAD・DIMZIN・DIMAZIN
    (Bs, 3),
    // DIMTXT から DIMALTRND まで
    (Bd, 9),
    // DIMALT・DIMALTD・DIMTOFL・DIMSAH・DIMTIX・DIMSOXD
    (B, 1),
    (Bs, 1),
    (B, 4),
    // DIMCLRD・DIMCLRE・DIMCLRT
    (Cmc, 3),
    // DIMADEC から DIMJUST まで
    (Bs, 11),
    // DIMSD1・DIMSD2・DIMTOLJ・DIMTZIN・DIMALTZ・DIMALTTZ・DIMUPT・DIMATFIT
    (B, 2),
    (Bs, 4),
    (B, 1),
    (Bs, 1),
    // DIMTXSTY・DIMLDRBLK・DIMBLK・DIMBLK1・DIMBLK2
    (H, 5),
    // DIMLWD・DIMLWE
    (Bs, 2),
];

/// R2010 からの、空間ごとの変数より後の、寸法の変数の並び。R2004 と比べて、DIMTM の後に
/// DIMFXL・DIMJOGANG・DIMTFILL・DIMTFILLCLR が、DIMAZIN の後に DIMARCSYM が、DIMATFIT の後に
/// 2 つの B（DIMFXLON・DIMTXTDIRECTION と見る）・DIMALTMZF・DIMALTMZS・DIMMZF・DIMMZS が、
/// DIMBLK2 の後に DIMLTYPE・DIMLTEX1・DIMLTEX2 がある（試料の DWG と DXF の比較で確かめた）
const LAST_R2010: [(Field, usize); 26] = [
    (Tv, 2),
    (Bd, 9),
    (Bd, 2),
    (Bs, 1),
    (Cmc, 1),
    (B, 6),
    (Bs, 3),
    (Bs, 1),
    (Bd, 9),
    (B, 1),
    (Bs, 1),
    (B, 4),
    (Cmc, 3),
    (Bs, 11),
    (B, 2),
    (Bs, 4),
    (B, 1),
    (Bs, 1),
    (B, 2),
    (Bd, 1),
    (Tv, 1),
    (Bd, 1),
    (Tv, 1),
    (H, 5),
    (H, 3),
    (Bs, 2),
];

/// R2000 の、寸法の変数より後、CEPSNTYPE より前の変数の並び
const TAIL: [(Field, usize); 6] = [
    // 9 つの表とビューポートの図形の見出しの管理オブジェクト、ACAD_GROUP・ACAD_MLINESTYLE・
    // 名前付きオブジェクトの辞書
    (H, 13),
    // TSTACKALIGN・TSTACKSIZE・HYPERLINKBASE・STYLESHEET
    (Bs, 2),
    (Tv, 2),
    // LAYOUTS・PLOTSETTINGS・PLOTSTYLES の辞書
    (H, 3),
    // 印（CELWEIGHT・ENDCAPS など）・INSUNITS
    (Bl, 1),
    (Bs, 1),
];

/// R2004 の、寸法の変数より後、CEPSNTYPE より前の変数の並び。R2000 と比べて、ビューポートの図形の
/// 見出しの管理オブジェクトがなく、PLOTSTYLES の辞書の後にハンドルが 2 つある（使い道は分からない。
/// 試料の DWG と DXF の比較で確かめた）
const TAIL_R2004: [(Field, usize); 6] = [(H, 12), (Bs, 2), (Tv, 2), (H, 5), (Bl, 1), (Bs, 1)];

/// R2010 の、寸法の変数より後、CEPSNTYPE より前の変数の並び。R2004 と比べて、PLOTSTYLES の
/// 辞書の後のハンドルが 2 つ多い（VISUALSTYLE の辞書などと見る）。R2013 からは、さらに 1 つ多い
const TAIL_R2010: [(Field, usize); 6] = [(H, 12), (Bs, 2), (Tv, 2), (H, 7), (Bl, 1), (Bs, 1)];
const TAIL_R2013: [(Field, usize); 6] = [(H, 12), (Bs, 2), (Tv, 2), (H, 8), (Bl, 1), (Bs, 1)];

/// R2004 の、VERSIONGUID の後、空間のブロックのレコードより前の変数の並び: SORTENTS・INDEXCTL・
/// HIDETEXT・XCLIPFRAME・DIMASSOC・HALOGAP、OBSCOLOR・INTERSECTIONCOLOR、OBSLTYPE・
/// INTERSECTIONDISPLAY、PROJECTNAME
const GUIDS_R2004: [(Field, usize); 4] = [(Rc, 6), (Bs, 2), (Rc, 2), (Tv, 1)];

/// 見出しの変数の区切りを読み、取り出した変数を返す。区切りがないか大きさが 0 なら、
/// 何も返さない。壊れていれば、診断を返して何も返さない
pub(super) fn header(
    src: &[u8],
    span: Range<usize>,
    release: Release,
    cx: &mut Cx,
) -> Option<Header> {
    let start = span.start;
    let found = section(src, start, release).and_then(|(data, crc_at)| {
        // CRC は、大きさの RL からデータの終わりまでを 0xC0C1 から計算する（クラスの区切りと同じ）
        let crc = crc16(
            OBJECT_CRC_SEED,
            src.get(start + 16..crc_at).unwrap_or_default(),
        );
        if le16(src, crc_at) != Some(crc) {
            cx.report(crc_invalid(PART, start..crc_at + 18, HEADER_VARIABLES));
        }
        variables(&mut streams(data, release)?)
    });
    if found.is_none() {
        cx.report(
            Diagnostic::new(codes::DWG_FILE_INVALID)
                .with("part", PART)
                .at(Ref::at_bytes(span.start as u64, span.end as u64))
                .cite(HEADER_VARIABLES),
        );
    }
    found
}

/// 始まりと終わりの印を確かめ、データと CRC の位置を返す。R2010 からは、大きさの RL の後に
/// 上位の RL があることがある（同じ版でも、ない図面がある）。終わりの印が、データと CRC の後に
/// ある方をとる
fn section(src: &[u8], start: usize, release: Release) -> Option<(&[u8], usize)> {
    if src.get(start..start.saturating_add(16))? != START {
        return None;
    }
    let size = usize::try_from(le32(src, start + 16)?).ok()?;
    let ends_at = |data_at: usize| {
        let crc_at = data_at.checked_add(size)?;
        (src.get(crc_at + 2..crc_at + 18)? == END).then_some(crc_at)
    };
    let data_at = if release >= Release::R2010 && ends_at(start + 24).is_some() {
        start + 24
    } else {
        start + 20
    };
    let crc_at = ends_at(data_at)?;
    Some((src.get(data_at..crc_at)?, crc_at))
}

/// データを読むビット列を作る。R2010 からは、データの初めに文字列の流れの終わりまでのビットの数
/// （RL）があり、その位置からハンドルの流れが始まる。HANDSEED のほかのハンドルと文字列は、主の
/// データにない。R2013 からは、変数の前に REQUIREDVERSIONS（BLL）がある（試料の DWG と DXF の
/// 比較で確かめた）
fn streams(data: &[u8], release: Release) -> Option<Bits<'_>> {
    let mut bits = Bits::with_release(data, release);
    if release >= Release::R2010 {
        let end = usize::try_from(le32(data, 0)?).ok()?;
        bits.split_strings(end)?;
        bits.split_handles(end);
        bits.seek(32);
        if release >= Release::R2013 {
            bits.bll()?;
        }
    }
    Some(bits)
}

/// 版 `release` の、空間ごとの変数より前の変数の並び。R2010 からは、R2004 と同じ並びのハンドルを
/// 別の流れから読む（決定 0084）
fn first_fields(release: Release) -> Option<&'static [(Field, usize)]> {
    match release {
        Release::R2000 => Some(&FIRST),
        _ => Some(&FIRST_R2004),
    }
}

/// 変数を順に読み、取り出した変数を返す。見出しのハンドルは絶対の値
fn variables(bits: &mut Bits) -> Option<Header> {
    let first: &[(Field, usize)] = match bits.release() {
        Release::R2000 => &FIRST,
        _ => &FIRST_R2004,
    };
    // PROXYGRAPHICS から始まる BS の並びの 8 つ目が PDMODE（ODS 5.4.1 の 9。R2000 からは
    // PROXYGRAPHICS・TREEDEPTH・LUNITS・LUPREC・AUNITS・AUPREC・ATTMODE の後）
    let at = first.iter().position(|(field, _)| matches!(field, Bs))?;
    skip_all(bits, first.get(..at)?)?;
    skip_all(bits, &[(Bs, 7)])?;
    let point_mode = bits.bs()?;
    let (_, rest) = first.get(at)?;
    skip_all(bits, &[(Bs, rest.checked_sub(8)?)])?;
    skip_all(bits, first.get(at + 1..)?)?;
    // 用紙空間、続いてモデル空間。用紙空間の LIMMIN・LIMMAX が PLIMMIN・PLIMMAX
    skip_all(bits, &SPACE[..1])?;
    let limits_at = bits.pos();
    let limits = [bits.rd()?, bits.rd()?, bits.rd()?, bits.rd()?];
    skip_all(bits, &SPACE[2..])?;
    skip_all(bits, &SPACE)?;
    skip_all(
        bits,
        if bits.release() >= Release::R2010 {
            &LAST_R2010
        } else {
            &LAST
        },
    )?;
    let tail: &[(Field, usize)] = match bits.release() {
        Release::R2000 => &TAIL,
        Release::R2004 => &TAIL_R2004,
        Release::R2010 => &TAIL_R2010,
        Release::R2013 | Release::R2018 => &TAIL_R2013,
    };
    // 最後の BS が INSUNITS
    skip_all(bits, tail.split_last()?.1)?;
    let units_at = bits.pos();
    let units = bits.bs()?;
    // CEPSNTYPE が 3 なら、CPSNID のハンドルが続く
    if bits.bs()? == 3 {
        bits.handle()?;
    }
    // FINGERPRINTGUID・VERSIONGUID
    bits.text()?;
    bits.text()?;
    if bits.release() >= Release::R2004 {
        skip_all(bits, &GUIDS_R2004)?;
    }
    let paper = bits.handle()?.value;
    let model = bits.handle()?.value;
    Some(Header {
        paper,
        model,
        units,
        limits,
        point_mode,
        limits_at,
        units_at,
    })
}

fn skip_all(bits: &mut Bits, fields: &[(Field, usize)]) -> Option<()> {
    for &(field, count) in fields {
        for _ in 0..count {
            skip(bits, field)?;
        }
    }
    Some(())
}

fn skip(bits: &mut Bits, field: Field) -> Option<()> {
    match field {
        B => bits.bit().map(drop),
        Bs => bits.bs().map(drop),
        Bl => bits.bl().map(drop),
        Bd => bits.bd().map(drop),
        Rd => bits.rd().map(drop),
        Tv => bits.text().map(drop),
        H => bits.handle().map(drop),
        Seed => bits.main_handle().map(drop),
        Cmc => bits.cmc().map(drop),
        Rc => bits.rc().map(drop),
    }
}

/// R2000・R2004 の見出しの変数の HANDSEED（決定 0052・0078）
#[derive(Clone, Copy, Debug)]
pub(super) struct HandleSeed {
    /// 区切りの始まり（始まりの印の位置）
    start: usize,
    /// データの始まりと CRC の位置
    data_at: usize,
    crc_at: usize,
    /// データの中の、HANDSEED のビットの位置と、その大きさ（バイト）
    at: usize,
    counter: u8,
    code: u8,
    /// 値
    pub(super) value: u64,
}

/// `start` から始まる版 `release` の見出しの変数の区切りの、HANDSEED。読めなければ `None`
pub(super) fn handle_seed(src: &[u8], start: usize, release: Release) -> Option<HandleSeed> {
    let first = first_fields(release)?;
    let (data, crc_at) = section(src, start, release)?;
    let mut bits = streams(data, release)?;
    let before = first.iter().position(|(field, _)| matches!(field, Seed))?;
    skip_all(&mut bits, first.get(..before)?)?;
    let at = bits.pos();
    let seed = bits.main_handle()?;
    Some(HandleSeed {
        start,
        data_at: crc_at - data.len(),
        crc_at,
        at,
        counter: u8::try_from((bits.pos() - at - 8) / 8).ok()?,
        code: seed.code,
        value: seed.value,
    })
}

/// `src` の見出しの変数の HANDSEED を `value` にし、区切りの CRC を直す。HANDSEED の大きさ（バイト）が
/// 変わる値なら `None`
pub(super) fn set_handle_seed(src: &mut [u8], seed: &HandleSeed, value: u64) -> Option<()> {
    let size = (u64::BITS - value.leading_zeros()).div_ceil(8);
    if size != u32::from(seed.counter) {
        return None;
    }
    let data = src.get(seed.data_at..seed.crc_at)?;
    let mut out = BitWriter::default();
    out.copy(data, 0..seed.at)?;
    out.handle(seed.code, value);
    out.copy(
        data,
        seed.at + 8 + usize::from(seed.counter) * 8..data.len() * 8,
    )?;
    store(src, seed.start, seed.data_at, seed.crc_at, out)
}

/// `start` から始まる版 `release` の見出しの変数の区切りの、INSUNITS を `units` に、用紙空間の
/// PLIMMIN・PLIMMAX を `limits` にし、区切りの CRC を直す（決定 0069・0078・0084）。`None` の変数は変えない。
/// データの大きさ（バイト）が変わる値なら `None`。R2010 からは、主のデータの後の文字列とハンドルの流れが
/// ずれないよう、ビットの数が変わる値でも `None`
pub(super) fn set_values(
    src: &mut [u8],
    start: usize,
    release: Release,
    units: Option<i16>,
    limits: Option<[f64; 4]>,
) -> Option<()> {
    first_fields(release)?;
    let (data, crc_at) = section(src, start, release)?;
    let data = data.to_vec();
    let data_at = crc_at - data.len();
    let found = variables(&mut streams(&data, release)?)?;
    let mut bits = Bits::with_release(&data, release);
    bits.seek(found.units_at);
    bits.bs()?;
    let mut out = BitWriter::default();
    out.copy(&data, 0..found.limits_at)?;
    for value in limits.unwrap_or(found.limits) {
        out.rd(value);
    }
    out.copy(&data, found.limits_at + 4 * 64..found.units_at)?;
    out.bs(units.unwrap_or(found.units));
    if release >= Release::R2010 && out.len() != bits.pos() {
        return None;
    }
    out.copy(&data, bits.pos()..data.len() * 8)?;
    store(src, start, data_at, crc_at, out)
}

/// 見出しの変数の区切りのデータを、同じ大きさ（バイト）の `out` に置き換え、CRC を直す。大きさが違えば `None`
fn store(
    src: &mut [u8],
    start: usize,
    data_at: usize,
    crc_at: usize,
    out: BitWriter,
) -> Option<()> {
    let out = out.into_bytes();
    if out.len() != crc_at - data_at {
        return None;
    }
    src.get_mut(data_at..crc_at)?.copy_from_slice(&out);
    let crc = crc16(OBJECT_CRC_SEED, src.get(start + 16..crc_at)?);
    src.get_mut(crc_at..crc_at + 2)?
        .copy_from_slice(&crc.to_le_bytes());
    Some(())
}
