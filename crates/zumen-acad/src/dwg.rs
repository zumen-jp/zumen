//! DWG（Open Design Specification for .dwg files 5.4.1）。R2000（AC1015）、R2004（AC1018）、
//! R2010（AC1024）、R2013（AC1027）、R2018（AC1032）を読む。R2004 から後は、入れ物
//! （4「R2004 DWG FILE FORMAT ORGANIZATION」）を展開してから読む。R2007（AC1021）は読めない。
//!
//! 読むのは、ファイルの見出し（3「R13-R15 DWG FILE FORMAT ORGANIZATION」）と、オブジェクトの地図
//! （23「OBJECT MAP」）と、地図が指すオブジェクトの区切りと、クラス（10「Data section AcDb:Classes」）。オブジェクトの中身は、[`DwgFile::objects`] で
//! 系統で共有するオブジェクトの表に置く（決定 0023）。
//!
//! 書くときは、元のバイトをそのまま並べる。R2000 は、[`DwgTxn`] でオブジェクトのデータを置き換えられ（決定 0028）、[`DwgFile::write_back`] で共有の表の変更を書き戻せる（決定 0029）。
//! R2004 も、[`DwgTxn`] でオブジェクトのデータを置き換え、除き、足せ（決定 0071）、図形・表のレコード・図形でないオブジェクトの値の一部と、図形の除きと足しと、見出しの変数を書き戻せる（決定 0072・0075〜0082）。R2010 から後も、[`DwgTxn`] でオブジェクトのデータを置き換え、除き、足せ（決定 0083）、見出しの変数を変えられ（決定 0084）、図形・表のレコード・図形でないオブジェクトの値の一部の変更と、図形の除きと一部の種類の足しを書き戻せる（決定 0085〜0095）。

use core::ops::Range;

use zumen_base::{Cx, Diagnostic, Kind, Ref, SpecRef, codes};
use zumen_sdk::{ByteRange, Codec, Error, FileModel, RecordInfo, Signature};

mod bits;
mod edit;
mod header;
mod objects;
mod r2004;
mod write_back;

use bits::{Bits, crc16, modular_char, modular_short};
pub use edit::DwgTxn;

const FILE_HEADER: SpecRef = SpecRef {
    spec: "ods_dwg_5_4_1",
    section: "3 R13-R15 DWG FILE FORMAT ORGANIZATION",
};

const OBJECT_MAP: SpecRef = SpecRef {
    spec: "ods_dwg_5_4_1",
    section: "23 Data section AcDb:Handles (OBJECT MAP)",
};

const CLASSES: SpecRef = SpecRef {
    spec: "ods_dwg_5_4_1",
    section: "10 Data section AcDb:Classes",
};

const COMMON_OBJECT: SpecRef = SpecRef {
    spec: "ods_dwg_5_4_1",
    section: "20.1 Common non-entity object format",
};

/// 見出しの終わりの印
const HEADER_SENTINEL: [u8; 16] = [
    0x95, 0xA0, 0x4E, 0x28, 0x99, 0x82, 0x1A, 0xE5, 0x5E, 0x41, 0xE0, 0x5F, 0x9D, 0x3A, 0x4D, 0x00,
];

/// クラスの区切りの始まりの印
const CLASSES_START: [u8; 16] = [
    0x8D, 0xA1, 0xC4, 0xB8, 0xC4, 0xA9, 0xF8, 0xC5, 0xC0, 0xDC, 0xF4, 0x5F, 0xE7, 0xCF, 0xB6, 0x8A,
];

/// クラスの区切りの終わりの印
const CLASSES_END: [u8; 16] = [
    0x72, 0x5E, 0x3B, 0x47, 0x3B, 0x56, 0x07, 0x3A, 0x3F, 0x23, 0x0B, 0xA0, 0x18, 0x30, 0x49, 0x75,
];

/// クラスの区切りの番号
const CLASSES_NUMBER: u8 = 1;

/// 区切りの記録が始まる位置
const LOCATORS_START: usize = 0x19;

/// 区切りの記録の大きさ
const LOCATOR_SIZE: usize = 9;

/// オブジェクトの地図の区切りの番号
const OBJECT_MAP_NUMBER: u8 = 2;

/// オブジェクトの CRC の初めの値
const OBJECT_CRC_SEED: u16 = 0xC0C1;

/// R2004 の入れ物に要る区切り
const R2004_SECTIONS: [&[u8]; 4] = [
    b"AcDb:Header",
    b"AcDb:Classes",
    b"AcDb:Handles",
    b"AcDb:AcDbObjects",
];

/// DWG のコーデック
#[derive(Clone, Copy, Debug)]
pub struct Dwg;

/// 読んだ DWG。元のバイト列と、その上のオブジェクトの区切り
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DwgFile {
    src: Vec<u8>,
    release: Release,
    code_page: u16,
    locators: Vec<Locator>,
    /// R2004 の入れ物の区切り。R2000 は `None`
    container: Option<r2004::Container>,
    classes: Vec<Class>,
    /// R2000 は `src` の中、R2004 は `AcDb:AcDbObjects` の区切りの中の位置
    entries: Vec<Entry>,
}

/// 読める版。ビット列の欄の違いを決める。後の版ほど大きい
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Release {
    /// AC1015
    R2000,
    /// AC1018
    R2004,
    /// AC1024
    R2010,
    /// AC1027
    R2013,
    /// AC1032
    R2018,
}

/// クラスの区切りの 1 つのクラス。型が 500 以上のオブジェクトは、番号が型と同じクラスで決まる
#[derive(Clone, Debug, PartialEq, Eq)]
struct Class {
    number: u16,
    /// DXF の名前
    name: Vec<u8>,
    /// 図形を作るクラスか（itemclassid が 0x1F2）
    entity: bool,
}

/// 見出しの区切りの記録（ODS 5.4.1 の 3.2.6）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Locator {
    /// 番号。0 は見出しの変数、1 はクラス、2 はオブジェクトの地図
    pub number: u8,
    /// ファイルの先頭からの位置
    pub seeker: u32,
    /// 大きさ
    pub size: u32,
}

/// オブジェクトの地図が指す 1 つのオブジェクト
#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    handle: u64,
    /// 大きさ（MS）から CRC の終わりまで
    span: Range<usize>,
    /// 大きさ（MS）の後の、オブジェクトのデータ。R2010 からは、ハンドルの流れの大きさ（MC）の後
    data: Range<usize>,
    /// R2010 からの、データの中のハンドルの流れが始まるビット
    handles: Option<usize>,
    type_code: u16,
}

impl DwgFile {
    /// 版の文字列（`AC1015`）
    pub fn version(&self) -> &[u8] {
        self.src.get(..6).unwrap_or_default()
    }

    /// 見出しの `$DWGCODEPAGE` の値
    pub fn code_page(&self) -> u16 {
        self.code_page
    }

    /// 見出しの区切りの記録
    pub fn locators(&self) -> &[Locator] {
        &self.locators
    }

    /// `record` 番目のオブジェクトのデータ（大きさ（MS）の後から CRC の前まで）。[`DwgTxn::set_data`] に
    /// 渡す形と同じ。R2010 からは、ハンドルの流れの大きさ（MC）から始まる（決定 0083）
    pub fn object_data(&self, record: usize) -> Option<&[u8]> {
        let entry = self.entries.get(record)?;
        let mut start = entry.span.start;
        modular_short(self.body(), &mut start)?;
        self.body().get(start..entry.data.end)
    }

    fn data(&self, entry: &Entry) -> &[u8] {
        self.body().get(entry.data.clone()).unwrap_or_default()
    }

    /// オブジェクトが並ぶバイト列
    fn body(&self) -> &[u8] {
        match &self.container {
            Some(container) => container.section(R2004_SECTIONS[3]).unwrap_or_default(),
            None => &self.src,
        }
    }

    /// 見出しの変数の区切りを含むバイト列と、区切りの範囲
    fn header_section(&self) -> Option<(&[u8], Range<usize>)> {
        match &self.container {
            Some(container) => {
                let section = container.section(R2004_SECTIONS[0])?;
                Some((section, 0..section.len()))
            }
            None => {
                let locator = self
                    .locators
                    .iter()
                    .find(|l| l.number == header::NUMBER && l.size > 0)?;
                let start = locator.seeker as usize;
                Some((
                    &self.src,
                    start..start.saturating_add(locator.size as usize),
                ))
            }
        }
    }
}

impl FileModel for DwgFile {
    fn record_count(&self) -> usize {
        self.entries.len()
    }

    fn record(&self, index: usize) -> Option<RecordInfo<'_>> {
        let entry = self.entries.get(index)?;
        Some(RecordInfo {
            kind: self.kind(entry.type_code).map_or(b"", |(kind, _)| kind),
            bytes: Some(ByteRange {
                start: entry.span.start as u64,
                end: entry.span.end as u64,
            }),
        })
    }
}

impl Codec for Dwg {
    type File = DwgFile;

    const FORMAT: &'static str = "dwg";

    const SIGNATURES: &'static [Signature] = &[
        Signature::Bytes {
            offset: 0,
            bytes: b"AC1012",
        },
        Signature::Bytes {
            offset: 0,
            bytes: b"AC1014",
        },
        Signature::Bytes {
            offset: 0,
            bytes: b"AC1015",
        },
        Signature::Bytes {
            offset: 0,
            bytes: b"AC1018",
        },
        Signature::Bytes {
            offset: 0,
            bytes: b"AC1021",
        },
        Signature::Bytes {
            offset: 0,
            bytes: b"AC1024",
        },
        Signature::Bytes {
            offset: 0,
            bytes: b"AC1027",
        },
        Signature::Bytes {
            offset: 0,
            bytes: b"AC1032",
        },
    ];

    fn read(src: &[u8], cx: &mut Cx) -> Result<DwgFile, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)
            .map_err(|stop| stopped(cx, stop))?;
        let version = src.get(..6).unwrap_or(src);
        let release = match version {
            b"AC1018" => Some(Release::R2004),
            b"AC1024" => Some(Release::R2010),
            b"AC1027" => Some(Release::R2013),
            b"AC1032" => Some(Release::R2018),
            _ => None,
        };
        if let Some(release) = release {
            return read_r2004(src, release, cx);
        }
        if version != b"AC1015" {
            let known = Self::SIGNATURES
                .iter()
                .any(|s| matches!(s, Signature::Bytes { bytes, .. } if *bytes == version));
            if !known {
                return Err(invalid(cx, "version", 0..version.len()));
            }
            cx.report(
                Diagnostic::new(codes::DWG_VERSION_UNSUPPORTED)
                    .with("version", String::from_utf8_lossy(version).into_owned()),
            );
            return Err(Error::Unsupported);
        }
        let (code_page, locators) = header(src, cx)?;
        let classes = match locators
            .iter()
            .find(|l| l.number == CLASSES_NUMBER && l.size > 0)
        {
            Some(locator) => classes(src, locator.seeker as usize, Release::R2000, cx)?,
            None => Vec::new(),
        };
        let map = locators
            .iter()
            .find(|l| l.number == OBJECT_MAP_NUMBER)
            .ok_or_else(|| invalid(cx, "object-map", 0..src.len()))?;
        let start = map.seeker as usize;
        let end = start.saturating_add(map.size as usize).min(src.len());
        let entries = object_map(src, start..end, src, Release::R2000, cx)?;
        Ok(DwgFile {
            src: src.to_vec(),
            release: Release::R2000,
            code_page,
            locators,
            container: None,
            classes,
            entries,
        })
    }

    fn write(file: &DwgFile, out: &mut Vec<u8>, cx: &mut Cx) -> Result<(), Error> {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        // R2004 の入れ物は、区切りを置き換えたときだけ組み直す（決定 0070）
        match &file.container {
            Some(container) if container.replaced() => {
                let rebuilt = r2004::write(&file.src, container)
                    .ok_or_else(|| r2004::invalid(cx, "r2004 container", 0..file.src.len()))?;
                out.extend_from_slice(&rebuilt);
            }
            _ => out.extend_from_slice(&file.src),
        }
        Ok(())
    }
}

/// R2004 の入れ物を読み、展開した区切りからクラスとオブジェクトの地図を読む（決定 0026）。
/// R2010 からも同じ入れ物を使う（決定 0027）
fn read_r2004(src: &[u8], release: Release, cx: &mut Cx) -> Result<DwgFile, Error> {
    let container = r2004::read(src, cx)?;
    let mut sections = [&[][..]; 4];
    for (found, name) in sections.iter_mut().zip(R2004_SECTIONS) {
        *found = container
            .section(name)
            .filter(|section| !section.is_empty())
            .ok_or_else(|| r2004::invalid(cx, "section", 0..6))?;
    }
    let [_, class_section, map, body] = sections;
    let classes = classes(class_section, 0, release, cx)?;
    let entries = object_map(map, 0..map.len(), body, release, cx)?;
    let code_page = le16(src, 0x13).ok_or_else(|| invalid(cx, "header", 0..src.len()))?;
    Ok(DwgFile {
        src: src.to_vec(),
        release,
        code_page,
        locators: Vec::new(),
        container: Some(container),
        classes,
        entries,
    })
}

/// 見出しのコード・ページと区切りの記録を読み、CRC と終わりの印を確かめる
fn header(src: &[u8], cx: &mut Cx) -> Result<(u16, Vec<Locator>), Error> {
    let code_page = le16(src, 0x13).ok_or_else(|| invalid(cx, "header", 0..src.len()))?;
    let count = le32(src, 0x15).ok_or_else(|| invalid(cx, "header", 0..src.len()))?;
    let count = usize::try_from(count).unwrap_or(usize::MAX);
    let crc_at = count
        .checked_mul(LOCATOR_SIZE)
        .and_then(|size| size.checked_add(LOCATORS_START))
        .filter(|end| end.saturating_add(18) <= src.len())
        .ok_or_else(|| invalid(cx, "header", 0..src.len()))?;
    cx.check(Kind::Records, count as u64)
        .map_err(|stop| stopped(cx, stop))?;
    let locators = (0..count)
        .filter_map(|i| {
            let at = LOCATORS_START + i * LOCATOR_SIZE;
            Some(Locator {
                number: *src.get(at)?,
                seeker: le32(src, at + 1)?,
                size: le32(src, at + 5)?,
            })
        })
        .collect();
    let stored = le16(src, crc_at);
    let found = crc16(0, src.get(..crc_at).unwrap_or_default()) ^ header_crc_mask(count);
    if stored != Some(found) {
        cx.report(crc_invalid("header", 0..crc_at + 2, FILE_HEADER));
    }
    let sentinel = crc_at + 2..crc_at + 18;
    if src.get(sentinel.clone()) != Some(&HEADER_SENTINEL[..]) {
        return Err(invalid(cx, "sentinel", sentinel));
    }
    Ok((code_page, locators))
}

/// `start` から始まるクラスの区切りを読む
fn classes(src: &[u8], start: usize, release: Release, cx: &mut Cx) -> Result<Vec<Class>, Error> {
    let span = start..src.len();
    if src.get(start..start.saturating_add(16)) != Some(&CLASSES_START[..]) {
        return Err(invalid(cx, "classes", span));
    }
    let size = le32(src, start + 16)
        .and_then(|size| usize::try_from(size).ok())
        .ok_or_else(|| invalid(cx, "classes", span.clone()))?;
    // R2010 からは、大きさの RL の後に上位の RL があることがある（同じ版でも、ない図面がある）。
    // 終わりの印が、データと CRC の後にある方をとる
    let ends_at = |data_at: usize| {
        let crc_at = data_at.saturating_add(size);
        src.get(crc_at + 2..crc_at + 18) == Some(&CLASSES_END[..])
    };
    let data_at = if release >= Release::R2010 && ends_at(start + 24) {
        start + 24
    } else {
        start + 20
    };
    let crc_at = data_at.saturating_add(size);
    let Some(data) = src.get(data_at..crc_at) else {
        return Err(invalid(cx, "classes", span));
    };
    if !ends_at(data_at) {
        return Err(invalid(cx, "classes", span));
    }
    // CRC は、大きさの RL からデータの終わりまでを 0xC0C1 から計算する（推定）
    let found = crc16(
        OBJECT_CRC_SEED,
        src.get(start + 16..crc_at).unwrap_or_default(),
    );
    if le16(src, crc_at) != Some(found) {
        cx.report(crc_invalid("classes", start..crc_at + 18, CLASSES));
    }
    let mut bits = Bits::with_release(data, release);
    // 大きさはバイトで数えるので、終わりの 8 ビットより短い余りは埋め草
    let mut end = size.saturating_mul(8);
    // R2010 からは、データの初めに、文字列の流れの終わりまでのビットの数（RL）がある
    if release >= Release::R2010 {
        let strings_end = le32(data, 0)
            .and_then(|bits| usize::try_from(bits).ok())
            .ok_or_else(|| invalid(cx, "classes", data_at..crc_at))?;
        bits.seek(32);
        end = bits
            .split_strings(strings_end)
            .ok_or_else(|| invalid(cx, "classes", data_at..crc_at))?;
    }
    // R2004 からは、クラスの前に、最大のクラスの番号（BS）・0 の RC・0 の RC・真の B がある
    if release >= Release::R2004 {
        let preface = bits.bs().and(bits.rc()).and(bits.rc()).and(bits.bit());
        preface.ok_or_else(|| invalid(cx, "classes", data_at..crc_at))?;
    }
    let mut classes = Vec::new();
    while bits.pos().saturating_add(8) <= end {
        cx.check(Kind::Records, 1)
            .map_err(|stop| stopped(cx, stop))?;
        let class =
            class(&mut bits, release).ok_or_else(|| invalid(cx, "classes", data_at..crc_at))?;
        classes.push(class);
    }
    Ok(classes)
}

/// 1 つのクラス: 番号・版・アプリケーションの名前・C++ の名前・DXF の名前・ゾンビだったか・
/// itemclassid
fn class(bits: &mut Bits, release: Release) -> Option<Class> {
    let number = u16::try_from(bits.bs()?).ok()?;
    bits.bs()?;
    bits.text()?;
    bits.text()?;
    let name = bits.text()?;
    bits.bit()?;
    let entity = bits.bs()? == 0x1F2;
    // R2004 からは、オブジェクトの数（BL）・版（BL）・保守の版（BL）・不明な 2 つの BL が続く
    if release >= Release::R2004 {
        for _ in 0..5 {
            bits.bl()?;
        }
    }
    Some(Class {
        number,
        name,
        entity,
    })
}

/// 見出しの CRC に重ねる値。区切りの記録の数で決まる
fn header_crc_mask(count: usize) -> u16 {
    match count {
        3 => 0xA598,
        4 => 0x8101,
        5 => 0x3CC4,
        6 => 0x8461,
        _ => 0,
    }
}

/// `src` の `range` にあるオブジェクトの地図を読み、地図が指す `objects` の中のオブジェクトの区切りを並べる
fn object_map(
    src: &[u8],
    range: Range<usize>,
    objects: &[u8],
    release: Release,
    cx: &mut Cx,
) -> Result<Vec<Entry>, Error> {
    let Range { start, end } = range;
    let mut entries = Vec::new();
    let mut pos = start;
    loop {
        // 区切りの大きさは上位のバイトから並び、大きさの 2 バイトを含む（推定）
        let size = be16(src, pos).ok_or_else(|| invalid(cx, "object-map", pos..end))?;
        let size = usize::from(size);
        let body = pos + 2..pos + size;
        let crc_at = pos + size;
        if size < 2 || crc_at.saturating_add(2) > end {
            return Err(invalid(cx, "object-map", pos..end));
        }
        let found = crc16(OBJECT_CRC_SEED, src.get(pos..crc_at).unwrap_or_default());
        if be16(src, crc_at) != Some(found) {
            cx.report(crc_invalid("object-map", pos..crc_at + 2, OBJECT_MAP));
        }
        if size == 2 {
            break;
        }
        let section = src.get(body.clone()).unwrap_or_default();
        // ハンドルと位置の差分は、区切りごとに 0 から数える
        let mut handle: u64 = 0;
        let mut location: i64 = 0;
        let mut at = 0;
        while at < section.len() {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            let pair =
                modular_char(section, &mut at, false).zip(modular_char(section, &mut at, true));
            let Some((handle_offset, location_offset)) = pair else {
                return Err(invalid(cx, "object-map", body));
            };
            handle = handle.saturating_add(handle_offset.unsigned_abs());
            location = location.saturating_add(location_offset);
            cx.check(Kind::Records, 1)
                .map_err(|stop| stopped(cx, stop))?;
            let offset = usize::try_from(location).unwrap_or(usize::MAX);
            entries.push(entry(objects, handle, offset, release, cx)?);
        }
        pos = crc_at + 2;
    }
    Ok(entries)
}

/// `offset` から始まるオブジェクトの区切りと型を読み、CRC を確かめる
fn entry(
    src: &[u8],
    handle: u64,
    offset: usize,
    release: Release,
    cx: &mut Cx,
) -> Result<Entry, Error> {
    let mut pos = offset;
    let size = modular_short(src, &mut pos)
        .and_then(|size| usize::try_from(size).ok())
        .ok_or_else(|| invalid(cx, "object", offset..src.len()))?;
    // R2010 からは、大きさの後に、ハンドルの流れのビットの数（MC）がある。ハンドルの流れは
    // データの終わりにある（試料の DWG と DXF の比較で確かめた）
    let handles = if release >= Release::R2010 {
        let bits = modular_char(src, &mut pos, false)
            .and_then(|bits| usize::try_from(bits).ok())
            .ok_or_else(|| invalid(cx, "object", offset..src.len()))?;
        let at = size.checked_mul(8).and_then(|all| all.checked_sub(bits));
        Some(at.ok_or_else(|| invalid(cx, "object", offset..src.len()))?)
    } else {
        None
    };
    let data = pos..pos.saturating_add(size);
    let span = offset..data.end.saturating_add(2);
    if span.end > src.len() {
        return Err(invalid(cx, "object", offset..src.len()));
    }
    let mut bits = Bits::new(src.get(data.clone()).unwrap_or_default());
    let type_code = if release >= Release::R2010 {
        bits.bot()
    } else {
        bits.bs()
    };
    let type_code = type_code
        .and_then(|code| u16::try_from(code).ok())
        .ok_or_else(|| invalid(cx, "object", span.clone()))?;
    let found = crc16(
        OBJECT_CRC_SEED,
        src.get(offset..data.end).unwrap_or_default(),
    );
    if le16(src, data.end) != Some(found) {
        cx.report(crc_invalid("object", span.clone(), COMMON_OBJECT));
    }
    Ok(Entry {
        handle,
        span,
        data,
        handles,
        type_code,
    })
}

fn le16(src: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*src.get(at)?, *src.get(at + 1)?]))
}

fn be16(src: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*src.get(at)?, *src.get(at + 1)?]))
}

fn le32(src: &[u8], at: usize) -> Option<u32> {
    let bytes = src.get(at..at + 4)?;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

fn crc_invalid(part: &str, at: Range<usize>, spec: SpecRef) -> Diagnostic {
    Diagnostic::new(codes::DWG_CRC_INVALID)
        .with("part", part)
        .at(Ref::at_bytes(at.start as u64, at.end as u64))
        .cite(spec)
}

fn invalid(cx: &mut Cx, part: &str, at: Range<usize>) -> Error {
    cx.report(
        Diagnostic::new(codes::DWG_FILE_INVALID)
            .with("part", part)
            .at(Ref::at_bytes(at.start as u64, at.end as u64))
            .cite(FILE_HEADER),
    );
    Error::Invalid
}

fn stopped(cx: &mut Cx, stop: zumen_base::Stop) -> Error {
    cx.report(stop.to_diagnostic());
    Error::Stopped(stop)
}
