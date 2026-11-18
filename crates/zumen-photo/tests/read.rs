//! テストの中で作った JPEG（個人情報のない合成のもの）の Exif と XMP を読む（#387）。画素のデータは持たない。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, codes};
use zumen_photo::{Photo, read};

mod common;
use common::{V, segment, tiff};

#[test]
fn shift_jis_xmp_property_names_are_decoded() {
    let text = "<?xml version='1.0' encoding='Shift_JIS'?><x:xmpmeta xmlns:x='adobe:ns:meta/'><rdf:RDF xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'><rdf:Description xmlns:ソ='urn:test' ソ:ゼー='value'><ソ:ゾ>text</ソ:ゾ></rdf:Description></rdf:RDF></x:xmpmeta>";
    let (encoded, _, error) = encoding_rs::SHIFT_JIS.encode(text);
    assert!(!error);
    let mut packet = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
    packet.extend_from_slice(&encoded);
    let mut src = jpeg(true);
    src.splice(2..2, segment(0xe1, &packet));
    let (photo, diag) = with_cx(|cx| read(&src, cx));
    assert!(diag.is_empty(), "{diag:?}");
    let photo = photo.unwrap();
    assert_eq!(
        photo
            .xmp
            .iter()
            .filter(|p| p.namespace == "urn:test")
            .map(|p| (p.name.as_str(), p.value.as_str()))
            .collect::<Vec<_>>(),
        [("ゼー", "value"), ("ゾ", "text")]
    );
}

const XMP: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmp:CreatorTool="テスト">
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
                ("CreatorTool", "テスト"),
                ("title", "工事写真"),
                ("Rating", "3")
            ]
        );
        assert_eq!(photo.xmp[1].namespace, "http://purl.org/dc/elements/1.1/");
    }
}

#[test]
fn utf16_xmp_packets_keep_namespaces_property_names_and_japanese_values() {
    let text = format!("<?xml version=\"1.0\" encoding=\"UTF-16\"?>{XMP}");
    for little in [true, false] {
        let mut packet = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
        packet.extend(if little { [0xff, 0xfe] } else { [0xfe, 0xff] });
        for unit in text.encode_utf16() {
            packet.extend(if little {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            });
        }
        let mut src = vec![0xff, 0xd8];
        src.extend(segment(0xe1, &packet));
        src.extend(segment(0xc0, &[8, 0x01, 0xe0, 0x02, 0x80, 1, 1, 0x11, 0]));
        src.extend([0xff, 0xd9]);
        let (photo, diag) = with_cx(|cx| read(&src, cx));
        assert!(diag.is_empty(), "{diag:?}");
        let photo = photo.unwrap();
        assert_eq!((photo.width, photo.height), (Some(640), Some(480)));
        assert_eq!(
            photo
                .xmp
                .iter()
                .map(|p| (p.name.as_str(), p.value.as_str()))
                .collect::<Vec<_>>(),
            [
                ("CreatorTool", "テスト"),
                ("title", "工事写真"),
                ("Rating", "3")
            ]
        );
        assert_eq!(photo.xmp[0].namespace, "http://ns.adobe.com/xap/1.0/");
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
