//! 試験の中で作った小さな ASCII の DXF で、L1 とコーデックの経路を確かめる。

use zumen_acad::{Dxf, DxfFile};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_sdk::{Codec, Error, FileModel, testkit};

/// 見出し・未知の図形・線分・文字を持つ最小の図面。グループ・コードは仕様の例のように右寄せにする
fn sample(newline: &str) -> Vec<u8> {
    let lines = [
        "999",
        "zumen test", //
        "  0",
        "SECTION",
        "  2",
        "HEADER", //
        "  9",
        "$ACADVER",
        "  1",
        "AC1032", //
        "  9",
        "$ZUMEN_UNKNOWN_VAR",
        " 70",
        "     7", //
        "  0",
        "ENDSEC", //
        "  0",
        "SECTION",
        "  2",
        "ENTITIES", //
        "  0",
        "LINE",
        "  8",
        "0",
        " 10",
        "0.0",
        " 20",
        "0.0",
        " 11",
        "100.0",
        " 21",
        "50.0",
        "  0",
        "ZUMEN_UNKNOWN_ENTITY",
        "  8",
        "0",
        "1001",
        "ZUMEN",
        "1000",
        "keep me ",
        "  0",
        "TEXT",
        "  8",
        "0",
        " 10",
        "1.0",
        " 20",
        "2.0",
        " 40",
        "2.5",
        "  1",
        "図面",
        "  0",
        "ENDSEC", //
        "  0",
        "EOF",
    ];
    let mut out = String::new();
    for line in lines {
        out.push_str(line);
        out.push_str(newline);
    }
    out.into_bytes()
}

#[cfg(test)]
fn read(src: &[u8]) -> DxfFile {
    testkit::read::<Dxf>(src).0.unwrap()
}

#[test]
fn unchanged_files_round_trip_byte_for_byte() {
    for newline in ["\n", "\r\n"] {
        let src = sample(newline);
        testkit::roundtrip::<Dxf>(&src).unwrap();
        testkit::records_in_order::<Dxf>(&src).unwrap();
        testkit::deterministic::<Dxf>(&src).unwrap();
    }
}

#[test]
fn every_record_is_kept_including_unknown_ones() {
    let file = read(&sample("\n"));
    let kinds: Vec<&[u8]> = (0..file.record_count())
        .map(|i| file.record(i).unwrap().kind)
        .collect();
    let expected: [&[u8]; 9] = [
        b"",
        b"SECTION",
        b"ENDSEC",
        b"SECTION",
        b"LINE",
        b"ZUMEN_UNKNOWN_ENTITY",
        b"TEXT",
        b"ENDSEC",
        b"EOF",
    ];
    assert_eq!(kinds, expected);

    let unknown = &file.records()[5];
    let values: Vec<(i16, &[u8])> = unknown
        .tags()
        .iter()
        .map(|t| (t.code(), file.value(t)))
        .collect();
    assert_eq!(
        values,
        vec![
            (0, &b"ZUMEN_UNKNOWN_ENTITY"[..]),
            (8, b"0"),
            (1001, b"ZUMEN"),
            (1000, b"keep me "),
        ]
    );
}

#[test]
fn bytes_after_eof_are_kept() {
    let mut src = sample("\r\n");
    src.extend_from_slice(b"\x1a\0garbage");
    let file = read(&src);
    assert_eq!(file.trailer(), b"\x1a\0garbage");
    testkit::roundtrip::<Dxf>(&src).unwrap();
}

#[test]
fn values_keep_their_bytes() {
    // Shift_JIS の「図」。L1 では文字コードを解釈しない
    let src = b"  0\nTEXT\n  1\n\x90\x7d\n  0\nEOF\n";
    let file = read(src);
    assert_eq!(file.value(&file.records()[0].tags()[1]), b"\x90\x7d");
    testkit::roundtrip::<Dxf>(src).unwrap();
}

#[test]
fn broken_tags_are_reported_with_their_line() {
    for (src, line, start) in [
        (&b"  0\nSECTION\n  X\nHEADER\n"[..], 3u64, 12),
        (&b"  0\nSECTION\n  2\n"[..], 3, 12),
    ] {
        let (result, diag) = testkit::read::<Dxf>(src);
        assert_eq!(result.unwrap_err(), Error::Invalid);
        assert_eq!(diag.len(), 1);
        assert_eq!(diag[0].code(), codes::DXF_TAG_INVALID);
        assert_eq!(diag[0].param("line"), Some(&line.into()));
        assert_eq!(diag[0].places()[0].bytes().map(|b| b.start), Some(start));
    }
}

#[test]
fn binary_dxf_is_not_read_yet() {
    let (result, diag) = testkit::read::<Dxf>(b"AutoCAD Binary DXF\r\n\x1a\0\0\0SECTION\0");
    assert_eq!(result.unwrap_err(), Error::Unsupported);
    assert_eq!(diag[0].code(), codes::DXF_BINARY_UNSUPPORTED);
}

#[test]
fn reading_stops_at_the_record_limit() {
    let src = sample("\n");
    let mut budget = Budget::new(Limits::default().with(Kind::Records, Limit::Finite(3)));
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = Dxf::read(&src, &mut Cx::new(&mut budget, &mut diag, &cancel));
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(
        diag.iter().next().map(|d| d.code()),
        Some(codes::BUDGET_RECORDS_EXCEEDED)
    );
}

#[test]
fn the_signature_finds_ascii_dxf() {
    let src = sample("\r\n");
    let found = |head: &[u8]| Dxf::SIGNATURES.iter().any(|s| s.matches(head));
    // 999 のコメントで始まるものは、まだ判定しない（docs/formats/dxf.md）
    assert!(!found(&src));
    assert!(found(&src[b"999\r\nzumen test\r\n".len()..]));
    assert!(!found(b"PK\x03\x04"));
    assert_eq!(Dxf::FORMAT, "dxf");
}
