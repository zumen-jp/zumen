//! 見出しの変数の区切り（ODS 5.4.1 の 9）。今は、用紙空間とモデル空間のブロックのレコードの
//! ハンドルと、図面モデルへの変換に使う変数（INSUNITS・PLIMMIN・PLIMMAX）だけを取り出す。
//! どちらの空間の図形も、所有者のハンドルを持たない（entmode が 1・2）ので、所有者はこのハンドルで決まる。

use super::bits::{Bits, crc16};
use super::{Locator, OBJECT_CRC_SEED, crc_invalid, le16, le32};
use zumen_base::{Cx, Diagnostic, Ref, SpecRef, codes};

const HEADER_VARIABLES: SpecRef = SpecRef {
    spec: "ods_dwg_5_4_1",
    section: "9 Data section AcDb:Header (HEADER VARIABLES)",
};

/// 区切りの記録の番号
const NUMBER: u8 = 0;

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
}

use Field::{B, Bd, Bl, Bs, H, Rd, Tv};

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
const FIRST: [(Field, usize); 12] = [
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
    (Bs, 1),
    // HANDSEED・CLAYER・TEXTSTYLE・CELTYPE・DIMSTYLE・CMLSTYLE
    (H, 6),
    // PSVPSCALE
    (Bd, 1),
];

/// 空間ごとの変数より後、CEPSNTYPE より前の変数の並び
const LAST: [(Field, usize); 22] = [
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
    (Bs, 3),
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

/// 見出しの変数の区切りを読み、取り出した変数を返す。区切りがないか大きさが 0 なら、
/// 何も返さない。壊れていれば、診断を返して何も返さない
pub(super) fn header(src: &[u8], locators: &[Locator], cx: &mut Cx) -> Option<Header> {
    let locator = locators.iter().find(|l| l.number == NUMBER && l.size > 0)?;
    let start = locator.seeker as usize;
    let span = start..start.saturating_add(locator.size as usize);
    let found = section(src, start).and_then(|(data, crc_at)| {
        // CRC は、大きさの RL からデータの終わりまでを 0xC0C1 から計算する（クラスの区切りと同じ）
        let crc = crc16(
            OBJECT_CRC_SEED,
            src.get(start + 16..crc_at).unwrap_or_default(),
        );
        if le16(src, crc_at) != Some(crc) {
            cx.report(crc_invalid(PART, start..crc_at + 18, HEADER_VARIABLES));
        }
        variables(&mut Bits::new(data))
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

/// 始まりと終わりの印を確かめ、データと CRC の位置を返す
fn section(src: &[u8], start: usize) -> Option<(&[u8], usize)> {
    if src.get(start..start.saturating_add(16))? != START {
        return None;
    }
    let size = usize::try_from(le32(src, start + 16)?).ok()?;
    let data_at = start + 20;
    let crc_at = data_at.checked_add(size)?;
    let data = src.get(data_at..crc_at)?;
    if src.get(crc_at + 2..crc_at + 18)? != END {
        return None;
    }
    Some((data, crc_at))
}

/// 変数を順に読み、取り出した変数を返す。見出しのハンドルは絶対の値
fn variables(bits: &mut Bits) -> Option<Header> {
    skip_all(bits, &FIRST)?;
    // 用紙空間、続いてモデル空間。用紙空間の LIMMIN・LIMMAX が PLIMMIN・PLIMMAX
    skip_all(bits, &SPACE[..1])?;
    let limits = [bits.rd()?, bits.rd()?, bits.rd()?, bits.rd()?];
    skip_all(bits, &SPACE[2..])?;
    skip_all(bits, &SPACE)?;
    // LAST の最後の BS が INSUNITS
    skip_all(bits, LAST.split_last()?.1)?;
    let units = bits.bs()?;
    // CEPSNTYPE が 3 なら、CPSNID のハンドルが続く
    if bits.bs()? == 3 {
        bits.handle()?;
    }
    // FINGERPRINTGUID・VERSIONGUID
    bits.text()?;
    bits.text()?;
    let paper = bits.handle()?.value;
    let model = bits.handle()?.value;
    Some(Header {
        paper,
        model,
        units,
        limits,
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
    }
}
