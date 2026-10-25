//! DWG（Open Design Specification for .dwg files 5.4.1）。今は R2000（AC1015）だけを読む。
//!
//! 読むのは、ファイルの見出し（3「R13-R15 DWG FILE FORMAT ORGANIZATION」）と、オブジェクトの地図
//! （23「OBJECT MAP」）と、地図が指すオブジェクトの区切り。オブジェクトの中身は、[`DwgFile::objects`] で
//! 系統で共有するオブジェクトの表に置く（決定 0023）。
//!
//! 書くときは、元のバイトをそのまま並べる。変更はまだできない。

use core::ops::Range;

use zumen_base::{Cx, Diagnostic, Kind, Ref, SpecRef, codes};
use zumen_sdk::{ByteRange, Codec, Error, FileModel, RecordInfo, Signature};

mod bits;
mod objects;

use bits::{Bits, crc16, modular_char, modular_short};

const FILE_HEADER: SpecRef = SpecRef {
    spec: "ods_dwg_5_4_1",
    section: "3 R13-R15 DWG FILE FORMAT ORGANIZATION",
};

const OBJECT_MAP: SpecRef = SpecRef {
    spec: "ods_dwg_5_4_1",
    section: "23 Data section AcDb:Handles (OBJECT MAP)",
};

const COMMON_OBJECT: SpecRef = SpecRef {
    spec: "ods_dwg_5_4_1",
    section: "20.1 Common non-entity object format",
};

/// 見出しの終わりの印
const HEADER_SENTINEL: [u8; 16] = [
    0x95, 0xA0, 0x4E, 0x28, 0x99, 0x82, 0x1A, 0xE5, 0x5E, 0x41, 0xE0, 0x5F, 0x9D, 0x3A, 0x4D, 0x00,
];

/// 区切りの記録が始まる位置
const LOCATORS_START: usize = 0x19;

/// 区切りの記録の大きさ
const LOCATOR_SIZE: usize = 9;

/// オブジェクトの地図の区切りの番号
const OBJECT_MAP_NUMBER: u8 = 2;

/// オブジェクトの CRC の初めの値
const OBJECT_CRC_SEED: u16 = 0xC0C1;

/// DWG のコーデック
#[derive(Clone, Copy, Debug)]
pub struct Dwg;

/// 読んだ DWG。元のバイト列と、その上のオブジェクトの区切り
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DwgFile {
    src: Vec<u8>,
    code_page: u16,
    locators: Vec<Locator>,
    entries: Vec<Entry>,
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
    /// 大きさ（MS）の後の、オブジェクトのデータ
    data: Range<usize>,
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

    fn data(&self, entry: &Entry) -> &[u8] {
        self.src.get(entry.data.clone()).unwrap_or_default()
    }
}

impl FileModel for DwgFile {
    fn record_count(&self) -> usize {
        self.entries.len()
    }

    fn record(&self, index: usize) -> Option<RecordInfo<'_>> {
        let entry = self.entries.get(index)?;
        Some(RecordInfo {
            kind: objects::kind(entry.type_code).map_or(b"", |(kind, _)| kind),
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
        let entries = object_map(src, &locators, cx)?;
        Ok(DwgFile {
            src: src.to_vec(),
            code_page,
            locators,
            entries,
        })
    }

    fn write(file: &DwgFile, out: &mut Vec<u8>, cx: &mut Cx) -> Result<(), Error> {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        out.extend_from_slice(&file.src);
        Ok(())
    }
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
    let mask = match count {
        3 => 0xA598,
        4 => 0x8101,
        5 => 0x3CC4,
        6 => 0x8461,
        _ => 0,
    };
    let stored = le16(src, crc_at);
    let found = crc16(0, src.get(..crc_at).unwrap_or_default()) ^ mask;
    if stored != Some(found) {
        cx.report(crc_invalid("header", 0..crc_at + 2, FILE_HEADER));
    }
    let sentinel = crc_at + 2..crc_at + 18;
    if src.get(sentinel.clone()) != Some(&HEADER_SENTINEL[..]) {
        return Err(invalid(cx, "sentinel", sentinel));
    }
    Ok((code_page, locators))
}

/// オブジェクトの地図を読み、地図が指すオブジェクトの区切りを並べる
fn object_map(src: &[u8], locators: &[Locator], cx: &mut Cx) -> Result<Vec<Entry>, Error> {
    let map = locators
        .iter()
        .find(|l| l.number == OBJECT_MAP_NUMBER)
        .ok_or_else(|| invalid(cx, "object-map", 0..src.len()))?;
    let start = map.seeker as usize;
    let end = start.saturating_add(map.size as usize).min(src.len());
    let mut entries = Vec::new();
    let mut handle: u64 = 0;
    let mut location: i64 = 0;
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
            entries.push(entry(src, handle, offset, cx)?);
        }
        pos = crc_at + 2;
    }
    Ok(entries)
}

/// `offset` から始まるオブジェクトの区切りと型を読み、CRC を確かめる
fn entry(src: &[u8], handle: u64, offset: usize, cx: &mut Cx) -> Result<Entry, Error> {
    let mut pos = offset;
    let size = modular_short(src, &mut pos)
        .and_then(|size| usize::try_from(size).ok())
        .ok_or_else(|| invalid(cx, "object", offset..src.len()))?;
    let data = pos..pos.saturating_add(size);
    let span = offset..data.end.saturating_add(2);
    if span.end > src.len() {
        return Err(invalid(cx, "object", offset..src.len()));
    }
    let type_code = Bits::new(src.get(data.clone()).unwrap_or_default())
        .bs()
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
