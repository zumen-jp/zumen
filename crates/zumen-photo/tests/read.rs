//! 試験の中で作った JPEG（個人情報のない合成のもの）の Exif と XMP を読む（#387）。画素のデータは持たない。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, codes};
use zumen_photo::{Photo, read};

/// IFD の項目の値
#[cfg(test)]
enum V {
    Ascii(&'static str),
    Short(u16),
    Byte(u8),
    Rationals(Vec<(u32, u32)>),
    /// 別の IFD の位置。書くときに決める
    Pointer(usize),
}

/// 型の番号・数・値のバイト列
#[cfg(test)]
fn encode(value: &V, little: bool, pointers: &[u32]) -> (u16, u32, Vec<u8>) {
    let u16b = |v: u16| {
        if little {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    };
    let u32b = |v: u32| {
        if little {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    };
    match value {
        V::Ascii(text) => {
            let mut bytes = text.as_bytes().to_vec();
            bytes.push(0);
            (2, bytes.len() as u32, bytes)
        }
        V::Short(v) => (3, 1, u16b(*v).to_vec()),
        V::Byte(v) => (1, 1, vec![*v]),
        V::Rationals(values) => {
            let mut bytes = Vec::new();
            for (n, d) in values {
                bytes.extend(u32b(*n));
                bytes.extend(u32b(*d));
            }
            (5, values.len() as u32, bytes)
        }
        V::Pointer(index) => (4, 1, u32b(pointers[*index]).to_vec()),
    }
}

/// IFD の列から TIFF を書く。0 番目の IFD を最初に置き、`V::Pointer(i)` は i 番目の IFD を指す
#[cfg(test)]
fn tiff(ifds: &[Vec<(u16, V)>], little: bool) -> Vec<u8> {
    let u16b = |v: u16| {
        if little {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    };
    let u32b = |v: u32| {
        if little {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    };
    // IFD の大きさ（値の外の領域を含む）から位置を決める
    let size = |ifd: &Vec<(u16, V)>| {
        let extra: usize = ifd
            .iter()
            .map(|(_, v)| encode(v, little, &[0; 8]).2.len())
            .filter(|len| *len > 4)
            .sum();
        2 + ifd.len() * 12 + 4 + extra
    };
    let mut pointers = Vec::new();
    let mut at = 8;
    for ifd in ifds {
        pointers.push(at as u32);
        at += size(ifd);
    }
    let mut out = if little {
        b"II*\0".to_vec()
    } else {
        b"MM\0*".to_vec()
    };
    out.extend(u32b(8));
    for (index, ifd) in ifds.iter().enumerate() {
        let start = pointers[index] as usize;
        let mut extra_at = start + 2 + ifd.len() * 12 + 4;
        let mut extra = Vec::new();
        out.extend(u16b(ifd.len() as u16));
        for (tag, value) in ifd {
            let (kind, count, bytes) = encode(value, little, &pointers);
            out.extend(u16b(*tag));
            out.extend(u16b(kind));
            out.extend(u32b(count));
            if bytes.len() <= 4 {
                let mut inline = bytes.clone();
                inline.resize(4, 0);
                out.extend(inline);
            } else {
                out.extend(u32b(extra_at as u32));
                extra_at += bytes.len();
                extra.extend(bytes);
            }
        }
        out.extend(u32b(0));
        out.extend(extra);
    }
    out
}

#[cfg(test)]
fn segment(marker: u8, body: &[u8]) -> Vec<u8> {
    let mut out = vec![0xFF, marker];
    out.extend(((body.len() + 2) as u16).to_be_bytes());
    out.extend(body);
    out
}

const XMP: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmp:CreatorTool="試験">
<dc:title><rdf:Alt><rdf:li xml:lang="x-default">工事写真</rdf:li></rdf:Alt></dc:title>
<xmp:Rating>3</xmp:Rating>
</rdf:Description></rdf:RDF></x:xmpmeta>"#;

#[cfg(test)]
fn jpeg(little: bool) -> Vec<u8> {
    let ifds = vec![
        vec![
            (0x010F, V::Ascii("zumen")),
            (0x0112, V::Short(6)),
            (0x8769, V::Pointer(1)),
            (0x8825, V::Pointer(2)),
        ],
        vec![
            (0x9003, V::Ascii("2026:10:31 09:15:30")),
            (0x9011, V::Ascii("+09:00")),
        ],
        vec![
            (0x0001, V::Ascii("N")),
            (0x0002, V::Rationals(vec![(35, 1), (40, 1), (3000, 100)])),
            (0x0003, V::Ascii("E")),
            (0x0004, V::Rationals(vec![(139, 1), (45, 1), (0, 1)])),
            (0x0005, V::Byte(1)),
            (0x0006, V::Rationals(vec![(125, 10)])),
            (0x0010, V::Ascii("T")),
            (0x0011, V::Rationals(vec![(90, 1)])),
            (0x0012, V::Ascii("WGS-84")),
        ],
    ];
    let mut exif = b"Exif\0\0".to_vec();
    exif.extend(tiff(&ifds, little));
    let mut xmp = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
    xmp.extend(XMP.as_bytes());
    let mut out = vec![0xFF, 0xD8];
    out.extend(segment(0xE1, &exif));
    out.extend(segment(0xE1, &xmp));
    out.extend(segment(0xC0, &[8, 0x01, 0xE0, 0x02, 0x80, 1, 1, 0x11, 0]));
    out.extend(segment(0xDA, &[1, 1, 0, 0, 0x3F, 0]));
    out.extend([0x12, 0x34, 0xFF, 0xD9]);
    out
}

#[cfg(test)]
fn with_cx<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Vec<Code>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let out = {
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        f(&mut cx)
    };
    (out, diag.iter().map(|d| d.code()).collect())
}

#[test]
fn exif_and_xmp_are_read_in_both_byte_orders() {
    for little in [true, false] {
        let (photo, diag) = with_cx(|cx| read(&jpeg(little), cx));
        assert!(diag.is_empty(), "{diag:?}");
        let photo: Photo = photo.unwrap();
        assert_eq!((photo.width, photo.height), (Some(640), Some(480)));
        assert_eq!(photo.orientation, Some(6));
        assert_eq!(photo.make.as_deref(), Some("zumen"));
        assert_eq!(photo.taken.as_deref(), Some("2026-10-31T09:15:30+09:00"));
        let position = photo.position.unwrap();
        assert!((position.latitude - (35.0 + 40.0 / 60.0 + 30.0 / 3600.0)).abs() < 1e-12);
        assert!((position.longitude - 139.75).abs() < 1e-12);
        // 海面より下
        assert_eq!(photo.altitude, Some(-12.5));
        assert_eq!(photo.direction, Some(90.0));
        assert_eq!(photo.direction_reference.as_deref(), Some("T"));
        assert_eq!(photo.datum.as_deref(), Some("WGS-84"));
        let xmp: Vec<(&str, &str)> = photo
            .xmp
            .iter()
            .map(|p| (p.name.as_str(), p.value.as_str()))
            .collect();
        assert_eq!(
            xmp,
            [
                ("CreatorTool", "試験"),
                ("title", "工事写真"),
                ("Rating", "3")
            ]
        );
        assert_eq!(photo.xmp[1].namespace, "http://purl.org/dc/elements/1.1/");
    }
}

#[test]
fn broken_parts_are_reported() {
    let (photo, diag) = with_cx(|cx| read(b"not a jpeg", cx));
    assert!(photo.is_err());
    assert_eq!(diag, [codes::JPEG_FILE_INVALID]);
    // 途中で切れたセグメント
    let mut cut = jpeg(true);
    cut.truncate(40);
    let (photo, diag) = with_cx(|cx| read(&cut, cx));
    assert!(photo.unwrap().make.is_none());
    assert_eq!(diag, [codes::JPEG_SEGMENT_INVALID]);
    // 読めない日時と、自分を指す IFD
    let ifds = vec![vec![(0x8769, V::Pointer(0)), (0x0112, V::Short(9))]];
    let mut exif = b"Exif\0\0".to_vec();
    exif.extend(tiff(&ifds, true));
    let mut src = vec![0xFF, 0xD8];
    src.extend(segment(0xE1, &exif));
    src.extend([0xFF, 0xD9]);
    let (photo, diag) = with_cx(|cx| read(&src, cx));
    assert_eq!(photo.unwrap().orientation, None);
    assert_eq!(diag, [codes::JPEG_EXIF_INVALID]);
}
