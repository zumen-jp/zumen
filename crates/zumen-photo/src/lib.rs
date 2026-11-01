//! 工事写真（JPEG）の Exif と XMP を読む（#387）。画像は復号しない。
//!
//! - JPEG のセグメント（L0、決定 0008）を、先頭の SOI から、画素のデータの始まり（SOS）か EOI までたどる。
//!   画素数は、フレームの見出し（SOF）から読む。
//! - Exif は APP1 の `Exif\0\0` の後の TIFF の構造で、0 番目の IFD・Exif の IFD・GPS の IFD を読む。縮小画像の
//!   IFD は読まない。
//! - XMP は APP1 の `http://ns.adobe.com/xap/1.0/\0` の後の XML で、`rdf:Description` の属性と、文字だけを持つ
//!   子の要素を、名前空間と名前と値の組にする。配列（`rdf:Alt`・`rdf:Seq`・`rdf:Bag`）は最初の項目の値にする。
//! - 位置は、GPS の緯度・経度を、`zumen-crs` の地理の点（度）にする。測地系の換算はしない。

use std::collections::BTreeSet;

use zumen_base::{Cx, Diagnostic, Kind, Stop, codes};
use zumen_crs::GeoPoint;
use zumen_sdk::Error;
use zumen_xml::{Document, Element, Node};

/// 1 つの IFD の項目の数の上限
const MAX_ENTRIES: u16 = 1024;

/// 写真の情報
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Photo {
    /// 画素の幅（SOF）
    pub width: Option<u16>,
    /// 画素の高さ（SOF）
    pub height: Option<u16>,
    /// 向き（Exif の Orientation、1〜8）
    pub orientation: Option<u16>,
    /// 撮影の日時（Exif の DateTimeOriginal を `YYYY-MM-DDTHH:MM:SS` にし、OffsetTimeOriginal があれば足す）
    pub taken: Option<String>,
    /// 機器の作り手
    pub make: Option<String>,
    /// 機器の型
    pub model: Option<String>,
    /// 撮った位置（GPS の緯度・経度）
    pub position: Option<GeoPoint>,
    /// 撮った位置の高さ（m、海面より下は負）
    pub altitude: Option<f64>,
    /// 撮った向き（度）
    pub direction: Option<f64>,
    /// 向きの基準（`T` は真北、`M` は磁北）
    pub direction_reference: Option<String>,
    /// GPS の測地系の名前（GPSMapDatum）
    pub datum: Option<String>,
    /// XMP の性質。文書の順
    pub xmp: Vec<Property>,
}

/// XMP の性質
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Property {
    /// 名前空間の URI
    pub namespace: String,
    /// 接頭辞を除いた名前
    pub name: String,
    /// 値
    pub value: String,
}

fn stopped(cx: &mut Cx, stop: Stop) -> Error {
    cx.report(stop.to_diagnostic());
    Error::Stopped(stop)
}

const EXIF: &[u8] = b"Exif\0\0";
const XMP: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";

/// JPEG を読む。SOI で始まらなければ `jpeg.file.invalid` を返して失敗する。壊れたセグメントがあれば、
/// `jpeg.segment.invalid` を返して、そこまでに読んだ値を返す
pub fn read(src: &[u8], cx: &mut Cx) -> Result<Photo, Error> {
    cx.check(Kind::InputBytes, src.len() as u64)
        .map_err(|stop| stopped(cx, stop))?;
    if !src.starts_with(&[0xFF, 0xD8]) {
        cx.report(Diagnostic::new(codes::JPEG_FILE_INVALID));
        return Err(Error::Invalid);
    }
    let mut photo = Photo::default();
    let mut at = 2;
    loop {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        // 印の前の埋めの 0xFF は飛ばす
        let Some(start) = src
            .get(at..)
            .and_then(|rest| rest.iter().position(|b| *b != 0xFF))
        else {
            break;
        };
        if src.get(at) != Some(&0xFF) {
            cx.report(Diagnostic::new(codes::JPEG_SEGMENT_INVALID).with("offset", at as u64));
            break;
        }
        at += start;
        let Some(&marker) = src.get(at) else {
            break;
        };
        at += 1;
        match marker {
            0xD9 => break,
            0x01 | 0xD0..=0xD7 => continue,
            _ => {}
        }
        let length = src
            .get(at..at + 2)
            .and_then(|b| <[u8; 2]>::try_from(b).ok())
            .map(|b| usize::from(u16::from_be_bytes(b)));
        let body = length
            .filter(|length| *length >= 2)
            .and_then(|length| src.get(at + 2..at + length));
        let (Some(length), Some(body)) = (length, body) else {
            cx.report(Diagnostic::new(codes::JPEG_SEGMENT_INVALID).with("offset", at as u64 - 1));
            break;
        };
        match marker {
            // 画素のデータの始まり。情報の区画は、ふつうこれより前にある
            0xDA => break,
            0xE1 => {
                if let Some(tiff) = body.strip_prefix(EXIF) {
                    exif(tiff, &mut photo, cx).map_err(|stop| stopped(cx, stop))?;
                } else if let Some(packet) = body.strip_prefix(XMP) {
                    xmp(packet, &mut photo, cx)?;
                }
            }
            0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) => {
                if let [_, h0, h1, w0, w1, ..] = body {
                    photo.height = Some(u16::from_be_bytes([*h0, *h1]));
                    photo.width = Some(u16::from_be_bytes([*w0, *w1]));
                }
            }
            _ => {}
        }
        at += length;
    }
    Ok(photo)
}

/// TIFF の構造の読み手
struct Tiff<'a> {
    src: &'a [u8],
    little: bool,
}

/// IFD の項目
struct Entry {
    tag: u16,
    kind: u16,
    count: u32,
    /// 値の始まり（TIFF の頭からのバイト）
    offset: usize,
}

impl Tiff<'_> {
    fn u16_at(&self, at: usize) -> Option<u16> {
        let b: [u8; 2] = self.src.get(at..at + 2)?.try_into().ok()?;
        Some(if self.little {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })
    }

    fn u32_at(&self, at: usize) -> Option<u32> {
        let b: [u8; 4] = self.src.get(at..at + 4)?.try_into().ok()?;
        Some(if self.little {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    }

    /// IFD の項目。範囲の外か、項目が多すぎれば `None`
    fn ifd(&self, at: usize) -> Option<Vec<Entry>> {
        let count = self.u16_at(at)?;
        if count > MAX_ENTRIES {
            return None;
        }
        let mut entries = Vec::with_capacity(usize::from(count));
        for index in 0..usize::from(count) {
            let base = at + 2 + index * 12;
            let (tag, kind, number) = (
                self.u16_at(base)?,
                self.u16_at(base + 2)?,
                self.u32_at(base + 4)?,
            );
            let size = match kind {
                1 | 2 | 6 | 7 => 1,
                3 | 8 => 2,
                4 | 9 | 11 => 4,
                5 | 10 | 12 => 8,
                // 知らない型の項目は飛ばす
                _ => continue,
            };
            let length = usize::try_from(number).ok()?.checked_mul(size)?;
            let offset = if length <= 4 {
                base + 8
            } else {
                usize::try_from(self.u32_at(base + 8)?).ok()?
            };
            self.src.get(offset..offset.checked_add(length)?)?;
            entries.push(Entry {
                tag,
                kind,
                count: number,
                offset,
            });
        }
        Some(entries)
    }

    fn text(&self, entry: &Entry) -> Option<String> {
        if entry.kind != 2 {
            return None;
        }
        let bytes = self
            .src
            .get(entry.offset..entry.offset + usize::try_from(entry.count).ok()?)?;
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        let text = core::str::from_utf8(bytes.get(..end)?).ok()?.trim();
        Some(text.to_string())
    }

    fn short(&self, entry: &Entry) -> Option<u16> {
        match entry.kind {
            1 | 7 => self.src.get(entry.offset).map(|b| u16::from(*b)),
            3 => self.u16_at(entry.offset),
            _ => None,
        }
    }

    fn long(&self, entry: &Entry) -> Option<u32> {
        match entry.kind {
            3 => self.u16_at(entry.offset).map(u32::from),
            4 => self.u32_at(entry.offset),
            _ => None,
        }
    }

    /// 符号のない有理数の `index` 番目
    fn rational(&self, entry: &Entry, index: usize) -> Option<f64> {
        if entry.kind != 5 || index >= usize::try_from(entry.count).ok()? {
            return None;
        }
        let at = entry.offset + index * 8;
        let (numerator, denominator) = (self.u32_at(at)?, self.u32_at(at + 4)?);
        (denominator != 0).then(|| f64::from(numerator) / f64::from(denominator))
    }
}

/// Exif の日時（`YYYY:MM:DD HH:MM:SS`）を `YYYY-MM-DDTHH:MM:SS` にする
fn date_time(text: &str) -> Option<String> {
    let b = text.as_bytes();
    let digits = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18];
    let ok = b.len() == 19
        && digits
            .iter()
            .all(|i| b.get(*i).is_some_and(u8::is_ascii_digit))
        && [(4, b':'), (7, b':'), (10, b' '), (13, b':'), (16, b':')]
            .iter()
            .all(|(i, c)| b.get(*i) == Some(c));
    if !ok {
        return None;
    }
    Some(format!(
        "{}-{}-{}T{}",
        text.get(0..4)?,
        text.get(5..7)?,
        text.get(8..10)?,
        text.get(11..19)?
    ))
}

/// 時差（`+09:00`）の形か
fn is_offset(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 6
        && matches!(b.first(), Some(b'+' | b'-'))
        && b.get(3) == Some(&b':')
        && [1, 2, 4, 5]
            .iter()
            .all(|i| b.get(*i).is_some_and(u8::is_ascii_digit))
}

fn invalid(cx: &mut Cx, tag: u16) {
    cx.report(Diagnostic::new(codes::JPEG_EXIF_INVALID).with("tag", u64::from(tag)));
}

/// 度・分・秒の 3 つの有理数を度にする
fn degrees(tiff: &Tiff, entry: &Entry) -> Option<f64> {
    let (d, m, s) = (
        tiff.rational(entry, 0)?,
        tiff.rational(entry, 1)?,
        tiff.rational(entry, 2)?,
    );
    Some(d + m / 60.0 + s / 3600.0)
}

fn exif(src: &[u8], photo: &mut Photo, cx: &mut Cx) -> Result<(), Stop> {
    let little = match src.get(..4) {
        Some(b"II*\0") => true,
        Some(b"MM\0*") => false,
        _ => {
            invalid(cx, 0);
            return Ok(());
        }
    };
    let tiff = Tiff { src, little };
    let Some(first) = tiff.u32_at(4).and_then(|at| usize::try_from(at).ok()) else {
        invalid(cx, 0);
        return Ok(());
    };
    // 読む IFD（0 番目・Exif・GPS）と、読んだ位置。同じ位置を二度読まない
    let mut pending = vec![(first, 0_u8)];
    let mut seen = BTreeSet::new();
    let (mut original, mut offset) = (None, None);
    let (mut latitude, mut longitude) = (None, None);
    let (mut north, mut east, mut below) = (true, true, false);
    while let Some((at, kind)) = pending.pop() {
        if !seen.insert(at) {
            continue;
        }
        let Some(entries) = tiff.ifd(at) else {
            invalid(cx, 0);
            continue;
        };
        for entry in &entries {
            cx.check(Kind::Work, 1)?;
            let pointer = || tiff.long(entry).and_then(|at| usize::try_from(at).ok());
            match (kind, entry.tag) {
                (0, 0x010F) => photo.make = tiff.text(entry),
                (0, 0x0110) => photo.model = tiff.text(entry),
                (0, 0x0112) => match tiff.short(entry).filter(|o| (1..=8).contains(o)) {
                    Some(orientation) => photo.orientation = Some(orientation),
                    None => invalid(cx, entry.tag),
                },
                (0, 0x8769) => match pointer() {
                    Some(at) => pending.push((at, 1)),
                    None => invalid(cx, entry.tag),
                },
                (0, 0x8825) => match pointer() {
                    Some(at) => pending.push((at, 2)),
                    None => invalid(cx, entry.tag),
                },
                (1, 0x9003) => match tiff.text(entry).as_deref().and_then(date_time) {
                    Some(taken) => original = Some(taken),
                    None => invalid(cx, entry.tag),
                },
                (1, 0x9011) => match tiff.text(entry).filter(|text| is_offset(text)) {
                    Some(text) => offset = Some(text),
                    None => invalid(cx, entry.tag),
                },
                (2, 0x0001) => north = tiff.text(entry).as_deref() != Some("S"),
                (2, 0x0002) => latitude = degrees(&tiff, entry),
                (2, 0x0003) => east = tiff.text(entry).as_deref() != Some("W"),
                (2, 0x0004) => longitude = degrees(&tiff, entry),
                (2, 0x0005) => below = tiff.short(entry) == Some(1),
                (2, 0x0006) => photo.altitude = tiff.rational(entry, 0),
                (2, 0x0010) => photo.direction_reference = tiff.text(entry),
                (2, 0x0011) => photo.direction = tiff.rational(entry, 0),
                (2, 0x0012) => photo.datum = tiff.text(entry).filter(|text| !text.is_empty()),
                _ => {}
            }
        }
    }
    photo.taken = original.map(|taken| match offset {
        Some(offset) => format!("{taken}{offset}"),
        None => taken,
    });
    match (latitude, longitude) {
        (Some(latitude), Some(longitude))
            if (0.0..=90.0).contains(&latitude) && (0.0..=180.0).contains(&longitude) =>
        {
            photo.position = Some(GeoPoint {
                latitude: if north { latitude } else { -latitude },
                longitude: if east { longitude } else { -longitude },
            });
        }
        (None, None) => {}
        _ => invalid(cx, 0x0002),
    }
    if below {
        photo.altitude = photo.altitude.map(|altitude| -altitude);
    }
    Ok(())
}

const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
const XMLNS: &str = "http://www.w3.org/2000/xmlns/";

/// 要素の文字だけの値。子の要素があれば `None`
fn simple(doc: &Document, element: &Element) -> Option<String> {
    let mut out = String::new();
    for node in element.children() {
        match node {
            Node::Element(_) => return None,
            Node::Text(range) => out.push_str(&doc.text(range.clone())),
            Node::CData(range) => out.push_str(&doc.cdata(range.clone())),
        }
    }
    Some(out.trim().to_string())
}

/// 配列（`rdf:Alt`・`rdf:Seq`・`rdf:Bag`）の最初の項目の値
fn first_item(doc: &Document, element: &Element) -> Option<String> {
    let nodes: Vec<&Node> = element
        .children()
        .iter()
        .filter(
            |node| !matches!(node, Node::Text(range) if doc.text(range.clone()).trim().is_empty()),
        )
        .collect();
    let [Node::Element(list)] = nodes.as_slice() else {
        return None;
    };
    let list = doc.element(*list)?;
    if !["Alt", "Seq", "Bag"]
        .iter()
        .any(|local| doc.is(list.name(), Some(RDF), local.as_bytes()))
    {
        return None;
    }
    list.children().iter().find_map(|node| match node {
        Node::Element(item) => doc
            .element(*item)
            .filter(|item| doc.is(item.name(), Some(RDF), b"li"))
            .and_then(|item| simple(doc, item)),
        _ => None,
    })
}

fn xmp(packet: &[u8], photo: &mut Photo, cx: &mut Cx) -> Result<(), Error> {
    let doc = match zumen_xml::read(packet, cx) {
        Ok(doc) => doc,
        Err(Error::Stopped(stop)) => return Err(Error::Stopped(stop)),
        Err(_) => {
            cx.report(Diagnostic::new(codes::JPEG_XMP_INVALID));
            return Ok(());
        }
    };
    for element in doc.elements() {
        if !doc.is(element.name(), Some(RDF), b"Description") {
            continue;
        }
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        for attribute in element.attributes() {
            let Some(namespace) = doc.namespace(attribute.name()) else {
                continue;
            };
            if namespace == RDF || namespace == XMLNS {
                continue;
            }
            photo.xmp.push(Property {
                namespace,
                name: String::from_utf8_lossy(doc.local_name(attribute.name())).into_owned(),
                value: doc.value(attribute),
            });
        }
        for node in element.children() {
            let Some(child) = (match node {
                Node::Element(index) => doc.element(*index),
                _ => None,
            }) else {
                continue;
            };
            let Some(namespace) = doc.namespace(child.name()) else {
                continue;
            };
            let Some(value) = simple(&doc, child).or_else(|| first_item(&doc, child)) else {
                continue;
            };
            photo.xmp.push(Property {
                namespace,
                name: String::from_utf8_lossy(doc.local_name(child.name())).into_owned(),
                value,
            });
        }
    }
    Ok(())
}
