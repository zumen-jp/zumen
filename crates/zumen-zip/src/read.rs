//! 中央ディレクトリを読む（PKWARE APPNOTE 6.3.10 の 4.3）。

use core::ops::Range;
use std::borrow::Cow;
use std::collections::BTreeSet;

use zumen_base::name::is_safe;
use zumen_base::{Code, Cx, Diagnostic, Kind, Ref, codes};
use zumen_sdk::Error;

use crate::{Archive, Entry, Method};

/// 中央ディレクトリの終わりの記録
const END: u32 = 0x0605_4b50;
const END_SIZE: usize = 22;
/// ZIP64 の中央ディレクトリの終わりの記録と、その位置の記録
const END64: u32 = 0x0606_4b50;
const LOCATOR: u32 = 0x0706_4b50;
const LOCATOR_SIZE: usize = 20;
/// 中央ディレクトリの見出し
const CENTRAL: u32 = 0x0201_4b50;
const CENTRAL_SIZE: usize = 46;
/// 局所の見出し
const LOCAL: u32 = 0x0403_4b50;
const LOCAL_SIZE: usize = 30;
/// ZIP64 の拡張の欄
const ZIP64_EXTRA: u16 = 0x0001;

/// 書庫を読む。入力のバイト数を `input-bytes`、項目を `records` で数える
pub fn read<'a>(src: &'a [u8], cx: &mut Cx) -> Result<Archive<'a>, Error> {
    if let Err(stop) = cx.check(Kind::InputBytes, src.len() as u64) {
        cx.report(stop.to_diagnostic());
        return Err(Error::Stopped(stop));
    }
    directory(Cow::Borrowed(src), cx)
}

/// 中央ディレクトリの終わり
struct End {
    /// 記録の位置。中央ディレクトリは、これより前で終わる
    at: usize,
    entries: u64,
    size: u64,
    offset: u64,
}

pub(crate) fn directory<'a>(src: Cow<'a, [u8]>, cx: &mut Cx) -> Result<Archive<'a>, Error> {
    let bytes: &[u8] = &src;
    let end = match end(bytes) {
        Ok(end) => end,
        Err(error) => {
            let code = match error {
                Error::Unsupported => codes::ZIP_SPLIT_UNSUPPORTED,
                _ => codes::ZIP_ARCHIVE_INVALID,
            };
            cx.report(Diagnostic::new(code));
            return Err(error);
        }
    };
    let invalid = |cx: &mut Cx, range: Range<usize>| {
        cx.report(Diagnostic::new(codes::ZIP_ARCHIVE_INVALID).at(at(range)));
        Error::Invalid
    };
    let whole = 0..end.at;
    let range = usize::try_from(end.offset).ok().and_then(|start| {
        let stop = start.checked_add(usize::try_from(end.size).ok()?)?;
        (stop <= end.at).then_some((start, stop))
    });
    let Some((start, stop)) = range else {
        return Err(invalid(cx, whole));
    };
    // 見出しは 46 バイト以上なので、項目の数は中央ディレクトリの大きさで抑えられる
    if end.entries > end.size / CENTRAL_SIZE as u64 {
        return Err(invalid(cx, whole));
    }

    let mut found = Vec::new();
    let mut pos = start;
    for _ in 0..end.entries {
        if let Err(stop) = cx.check(Kind::Records, 1) {
            cx.report(stop.to_diagnostic().at(at(pos..pos + CENTRAL_SIZE)));
            return Err(Error::Stopped(stop));
        }
        let Some((header, next)) = central(bytes, pos, stop) else {
            return Err(invalid(cx, pos..pos.saturating_add(CENTRAL_SIZE)));
        };
        let place = pos..next;
        pos = next;
        let skip = |cx: &mut Cx, code: Code| {
            cx.report(
                Diagnostic::new(code)
                    .with("name", header.name.as_str())
                    .at(at(place.clone())),
            );
        };
        let path = header.name.strip_suffix('/').unwrap_or(&header.name);
        if !header.name_ok || !is_safe(path) {
            skip(cx, codes::ZIP_PATH_UNSAFE);
            continue;
        }
        if header.flags & 1 != 0 {
            skip(cx, codes::ZIP_ENCRYPTION_UNSUPPORTED);
            continue;
        }
        let method = match header.method {
            0 => Method::Stored,
            8 => Method::Deflate,
            other => {
                cx.report(
                    Diagnostic::new(codes::ZIP_METHOD_UNSUPPORTED)
                        .with("name", header.name.as_str())
                        .with("method", u64::from(other))
                        .at(at(place.clone())),
                );
                continue;
            }
        };
        let Some((local, data)) = local(bytes, header.offset, header.compressed, start) else {
            skip(cx, codes::ZIP_ENTRY_INVALID);
            continue;
        };
        if method == Method::Stored && header.compressed != header.size {
            skip(cx, codes::ZIP_ENTRY_INVALID);
            continue;
        }
        found.push((
            Entry {
                name: header.name.clone(),
                method,
                crc: header.crc,
                size: header.size,
                header: local,
                data,
            },
            place,
        ));
    }

    // 範囲の重なる項目は、どれも読まない。書庫の爆弾と、名前の違う同じデータを避ける
    let mut order: Vec<usize> = (0..found.len()).collect();
    order.sort_by_key(|&i| found.get(i).map(|(entry, _)| entry.header.start));
    let mut overlapping = BTreeSet::new();
    let mut reach: Option<(usize, usize)> = None;
    for &i in &order {
        let Some((entry, _)) = found.get(i) else {
            continue;
        };
        let span = entry.span();
        if let Some((end, owner)) = reach {
            if span.start < end {
                overlapping.insert(i);
                overlapping.insert(owner);
            }
            if span.end > end {
                reach = Some((span.end, i));
            }
        } else {
            reach = Some((span.end, i));
        }
    }
    let mut names = BTreeSet::new();
    let mut entries = Vec::with_capacity(found.len());
    for (i, (entry, place)) in found.into_iter().enumerate() {
        let code = if overlapping.contains(&i) {
            codes::ZIP_RANGE_UNSAFE
        } else if !names.insert(entry.name.clone()) {
            codes::ZIP_DUPLICATE_DROPPED
        } else {
            entries.push(entry);
            continue;
        };
        cx.report(
            Diagnostic::new(code)
                .with("name", entry.name.as_str())
                .at(at(place)),
        );
    }
    Ok(Archive {
        src,
        entries,
        depth: 0,
    })
}

fn at(range: Range<usize>) -> Ref {
    Ref::at_bytes(range.start as u64, range.end as u64)
}

fn le16(src: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        src.get(at..at.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn le32(src: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        src.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn le64(src: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        src.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
}

/// 中央ディレクトリの終わりを、後ろから探す。注記は 65535 バイトまで
fn end(src: &[u8]) -> Result<End, Error> {
    let last = src.len().checked_sub(END_SIZE).ok_or(Error::Invalid)?;
    let first = last.saturating_sub(usize::from(u16::MAX));
    let at = (first..=last)
        .rev()
        .find(|&at| {
            le32(src, at) == Some(END)
                && le16(src, at + 20)
                    .is_some_and(|comment| at + END_SIZE + usize::from(comment) <= src.len())
        })
        .ok_or(Error::Invalid)?;
    let field16 = |offset| le16(src, at + offset).ok_or(Error::Invalid);
    let field32 = |offset| le32(src, at + offset).ok_or(Error::Invalid);
    let (disk, disk_start) = (field16(4)?, field16(6)?);
    let (here, entries) = (field16(8)?, field16(10)?);
    let (size, offset) = (field32(12)?, field32(16)?);
    let locator = at
        .checked_sub(LOCATOR_SIZE)
        .filter(|&locator| le32(src, locator) == Some(LOCATOR));
    let Some(locator) = locator else {
        if disk != 0 || disk_start != 0 || here != entries {
            return Err(Error::Unsupported);
        }
        return Ok(End {
            at,
            entries: u64::from(entries),
            size: u64::from(size),
            offset: u64::from(offset),
        });
    };
    // ZIP64（4.3.14・4.3.15）
    let disks = le32(src, locator + 16).ok_or(Error::Invalid)?;
    if le32(src, locator + 4) != Some(0) || disks > 1 {
        return Err(Error::Unsupported);
    }
    let start = le64(src, locator + 8)
        .and_then(|start| usize::try_from(start).ok())
        .filter(|&start| start < locator && le32(src, start) == Some(END64))
        .ok_or(Error::Invalid)?;
    let field = |offset| le64(src, start + offset).ok_or(Error::Invalid);
    let (disk, disk_start) = (
        le32(src, start + 16).ok_or(Error::Invalid)?,
        le32(src, start + 20).ok_or(Error::Invalid)?,
    );
    let (here, entries) = (field(24)?, field(32)?);
    if disk != 0 || disk_start != 0 || here != entries {
        return Err(Error::Unsupported);
    }
    Ok(End {
        at: start,
        entries,
        size: field(40)?,
        offset: field(48)?,
    })
}

/// 中央ディレクトリの見出し
struct Central {
    name: String,
    /// 名前を、壊さずに文字にできたら真
    name_ok: bool,
    flags: u16,
    method: u16,
    crc: u32,
    compressed: u64,
    size: u64,
    offset: u64,
}

/// `at` の見出しと、次の見出しの位置。見出しは中央ディレクトリの終わり（`stop`）より前で終わる
fn central(src: &[u8], at: usize, stop: usize) -> Option<(Central, usize)> {
    if le32(src, at)? != CENTRAL {
        return None;
    }
    let flags = le16(src, at + 8)?;
    let method = le16(src, at + 10)?;
    let crc = le32(src, at + 16)?;
    let mut compressed = u64::from(le32(src, at + 20)?);
    let mut size = u64::from(le32(src, at + 24)?);
    let name_len = usize::from(le16(src, at + 28)?);
    let extra_len = usize::from(le16(src, at + 30)?);
    let comment_len = usize::from(le16(src, at + 32)?);
    let mut offset = u64::from(le32(src, at + 42)?);
    let name_at = at + CENTRAL_SIZE;
    let extra_at = name_at + name_len;
    let next = extra_at + extra_len + comment_len;
    let raw = src.get(name_at..extra_at)?;
    let extra = src.get(extra_at..extra_at + extra_len)?;
    if next > stop {
        return None;
    }

    // ZIP64 の拡張の欄は、32 ビットで足りなかった値だけを、この順で持つ（4.5.3）
    if let Some(mut field) = fields(extra)
        .find(|(id, _)| *id == ZIP64_EXTRA)
        .map(|(_, f)| f)
    {
        for value in [&mut size, &mut compressed, &mut offset] {
            if *value == u64::from(u32::MAX) {
                *value = le64(field, 0)?;
                field = field.get(8..)?;
            }
        }
    }
    // 名前は、UTF-8 の印（ビット 11）があれば UTF-8、なければ Shift_JIS として読む
    let (name, name_ok) = if flags & (1 << 11) != 0 {
        match core::str::from_utf8(raw) {
            Ok(name) => (name.to_owned(), true),
            Err(_) => (String::from_utf8_lossy(raw).into_owned(), false),
        }
    } else {
        let (name, had_errors) = encoding_rs::SHIFT_JIS.decode_without_bom_handling(raw);
        (name.into_owned(), !had_errors)
    };
    Some((
        Central {
            name,
            name_ok,
            flags,
            method,
            crc,
            compressed,
            size,
            offset,
        },
        next,
    ))
}

/// 拡張の欄の、識別子と中身の組
fn fields(mut extra: &[u8]) -> impl Iterator<Item = (u16, &[u8])> {
    core::iter::from_fn(move || {
        let id = le16(extra, 0)?;
        let len = usize::from(le16(extra, 2)?);
        let body = extra.get(4..4 + len)?;
        extra = extra.get(4 + len..)?;
        Some((id, body))
    })
}

/// 局所の見出しの範囲とデータの範囲。データは中央ディレクトリ（`stop`）より前で終わる
fn local(
    src: &[u8],
    offset: u64,
    compressed: u64,
    stop: usize,
) -> Option<(Range<usize>, Range<usize>)> {
    let at = usize::try_from(offset).ok()?;
    if le32(src, at)? != LOCAL {
        return None;
    }
    let name_len = usize::from(le16(src, at + 26)?);
    let extra_len = usize::from(le16(src, at + 28)?);
    let start = at.checked_add(LOCAL_SIZE + name_len + extra_len)?;
    let end = start.checked_add(usize::try_from(compressed).ok()?)?;
    (end <= stop).then_some((at..start, start..end))
}
