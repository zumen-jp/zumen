//! 試験の中で作った小さな ASCII の DXF で、L1 とコーデックの経路を確かめる。

use zumen_acad::{AcadDb, Data, Dxf, DxfFile, Line, Object, Origin, Section, Value, ViewError};
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_sdk::{Codec, Error, FileModel, Transact, Transaction, testkit};

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

/// バイナリの DXF。`wide` ならグループ・コードを 2 バイト（R13 から）、そうでなければ 1 バイト（R12）で書く。
/// `tags` の値は、型に合わせて符号化したバイト列
fn binary_dxf(wide: bool, tags: &[(i16, Vec<u8>)]) -> Vec<u8> {
    let mut out = b"AutoCAD Binary DXF\r\n\x1a\0".to_vec();
    for (code, value) in tags {
        match u8::try_from(*code) {
            _ if wide => out.extend_from_slice(&code.to_le_bytes()),
            Ok(code) if code < 255 => out.push(code),
            _ => {
                out.push(255);
                out.extend_from_slice(&code.to_le_bytes());
            }
        }
        out.extend_from_slice(value);
    }
    out
}

fn text(value: &str) -> Vec<u8> {
    [value.as_bytes(), b"\0"].concat()
}

/// 1 つの LINE だけを ENTITIES に持つバイナリの DXF。LINE は 1 番目のレコード
fn binary_line(wide: bool) -> Vec<u8> {
    binary_dxf(
        wide,
        &[
            (0, text("SECTION")),
            (2, text("ENTITIES")),
            (0, text("LINE")),
            (5, text("1F")),
            (8, text("0")),
            (10, 1.5f64.to_le_bytes().to_vec()),
            (20, 0.0f64.to_le_bytes().to_vec()),
            (11, (-2.25f64).to_le_bytes().to_vec()),
            (21, 1e-3f64.to_le_bytes().to_vec()),
            (62, 3i16.to_le_bytes().to_vec()),
            (290, vec![1]),
            (310, vec![2, 0xAB, 0xCD]),
            (1001, text("ZUMEN")),
            (1071, (-5i32).to_le_bytes().to_vec()),
            (0, text("ENDSEC")),
            (0, text("EOF")),
        ],
    )
}

#[test]
fn binary_dxf_is_read_as_its_tags() {
    for wide in [true, false] {
        let src = binary_line(wide);
        testkit::roundtrip::<Dxf>(&src).unwrap();
        testkit::records_in_order::<Dxf>(&src).unwrap();
        testkit::deterministic::<Dxf>(&src).unwrap();
        let file = read(&src);
        assert!(file.is_binary());
        let kinds: Vec<_> = file.records().iter().map(|r| file.kind(r)).collect();
        assert_eq!(kinds, [b"SECTION".as_slice(), b"LINE", b"ENDSEC", b"EOF"]);
        let line = &file.records()[1];
        let values: Vec<_> = line.tags().iter().map(|t| file.value(t)).collect();
        assert_eq!(
            values,
            [
                b"LINE".as_slice(),
                b"1F",
                b"0",
                b"1.5",
                b"0.0",
                b"-2.25",
                b"0.001",
                b"3",
                b"1",
                b"ABCD",
                b"ZUMEN",
                b"-5"
            ]
        );
        // レコードの範囲は、元のバイナリのバイト範囲
        let start = binary_dxf(wide, &[(0, text("SECTION")), (2, text("ENTITIES"))]).len();
        let bytes = file.record(1).unwrap().bytes.unwrap();
        assert_eq!(bytes.start, start as u64);
        let db = table(&file);
        let views: Vec<_> = db.objects().iter().filter_map(|o| o.line()).collect();
        assert_eq!(
            views,
            [Ok(Line {
                layer: b"0",
                start: [1.5, 0.0, 0.0],
                end: [-2.25, 0.001, 0.0],
            })]
        );
    }
}

#[test]
fn binary_dxf_edits_are_written_as_binary() {
    for wide in [true, false] {
        let src = binary_line(wide);
        testkit::edit_keeps_untouched_records::<Dxf>(&src, |file, cx| {
            let mut txn = file.transaction();
            txn.set_real(1, 11, 200.0)?;
            txn.insert_tag(1, 3, 39, b"2.5")?;
            txn.commit(cx)
        })
        .unwrap();
        let mut file = read(&src);
        let mut txn = file.transaction();
        txn.set_real(1, 11, 200.0).unwrap();
        txn.insert_tag(1, 3, 39, b"2.5").unwrap();
        let mut diag = Diagnostics::new();
        txn.commit(&mut cx_for(
            &mut Budget::default(),
            &mut diag,
            &CancelToken::new(),
        ))
        .unwrap();
        let out = testkit::write::<Dxf>(&file).unwrap();
        // 書いたものも、元と同じ幅のグループ・コードのバイナリの DXF
        assert_eq!(out.len(), src.len() + 1 + usize::from(wide) + 8);
        let again = read(&out);
        let line = &again.records()[1];
        assert_eq!(again.real(line, 11), Ok(200.0));
        assert_eq!(again.real(line, 39), Ok(2.5));
        assert_eq!(again.value(&line.tags()[10]), b"ABCD");
    }
}

#[test]
fn binary_dxf_writing_stops_without_adding_partial_output() {
    let file = read(&binary_line(true));
    for limit in [0, 1] {
        let prefix = b"existing output".to_vec();
        let mut out = prefix.clone();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(limit)));
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let result = Dxf::write(
            &file,
            &mut out,
            &mut Cx::new(&mut budget, &mut diag, &cancel),
        );
        assert!(matches!(result, Err(Error::Stopped(_))));
        assert_eq!(out, prefix, "{limit}");
        assert_eq!(
            diag.iter().next().map(|d| d.code()),
            Some(codes::BUDGET_WORK_EXCEEDED)
        );
    }
}

#[test]
fn binary_dxf_values_must_fit_their_type() {
    let cases: [(i16, &[u8]); 5] = [
        (62, b"x"),
        (62, b"70000"),
        (310, b"ABC"),
        (8, b"a^@b"),
        (290, b"256"),
    ];
    for (code, value) in cases {
        let mut file = read(&binary_line(true));
        let mut txn = file.transaction();
        let tag = file_tag(code);
        assert_eq!(txn.set_value(1, tag, value), Err(Error::Rejected));
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let result = txn.commit(&mut cx_for(&mut Budget::default(), &mut diag, &cancel));
        assert_eq!(result, Err(Error::Rejected));
        let found: Vec<Code> = diag.iter().map(|d| d.code()).collect();
        assert_eq!(found, [codes::DXF_BINARY_VALUE_UNSUPPORTED], "{code}");
    }
    let mut file = read(&binary_line(true));
    let mut txn = file.transaction();
    assert_eq!(txn.insert_tag(1, 3, 40, b"x"), Err(Error::Rejected));
}

/// [`binary_line`] の LINE のレコードで、`code` のタグの位置
#[cfg(test)]
fn file_tag(code: i16) -> usize {
    [0, 5, 8, 10, 20, 11, 21, 62, 290, 310, 1001, 1071]
        .iter()
        .position(|&c| c == code)
        .unwrap()
}

#[test]
fn broken_binary_dxf_is_reported_with_its_offset() {
    let src = binary_line(true);
    let eof = src.len() - b"\0\0EOF\0".len();
    let (result, diag) = testkit::read::<Dxf>(&src[..src.len() - 1]);
    assert_eq!(result.unwrap_err(), Error::Invalid);
    assert_eq!(diag[0].code(), codes::DXF_BINARY_INVALID);
    assert_eq!(diag[0].param("offset"), Some(&(eof as u64).into()));
}

#[test]
fn binary_dxf_versions_are_not_changed_yet() {
    let src = binary_dxf(
        true,
        &[
            (0, text("SECTION")),
            (2, text("HEADER")),
            (9, text("$ACADVER")),
            (1, text("AC1015")),
            (0, text("ENDSEC")),
            (0, text("EOF")),
        ],
    );
    let (result, diag) = write_as(&src, b"AC1015");
    assert_eq!(result.unwrap(), src);
    assert_eq!(diag.len(), 0);
    let (result, diag) = write_as(&src, b"AC1009");
    assert_eq!(result.unwrap_err(), Error::Unsupported);
    let found: Vec<Code> = diag.iter().map(|d| d.code()).collect();
    assert_eq!(found, [codes::DXF_VERSION_UNSUPPORTED]);
}

#[test]
fn binary_dxf_control_characters_are_written_with_carets() {
    let src = binary_dxf(
        true,
        &[(0, text("SECTION")), (2, text("A\nB^C")), (0, text("EOF"))],
    );
    testkit::roundtrip::<Dxf>(&src).unwrap();
    let mut file = read(&src);
    let section = &file.records()[0];
    assert_eq!(file.value(&section.tags()[1]), b"A^JB^ C");
    let mut txn = file.transaction();
    txn.set_value(0, 1, b"X^MY").unwrap();
    // `^` の後に、読みが作らない文字は続かない
    assert_eq!(txn.set_value(0, 1, b"^a"), Err(Error::Rejected));
    let mut file = read(&src);
    let mut txn = file.transaction();
    txn.set_value(0, 1, b"X^MY").unwrap();
    let mut diag = Diagnostics::new();
    txn.commit(&mut cx_for(
        &mut Budget::default(),
        &mut diag,
        &CancelToken::new(),
    ))
    .unwrap();
    let out = testkit::write::<Dxf>(&file).unwrap();
    assert!(out.windows(4).any(|w| w == b"X\rY\0"));
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
    // 止めた診断は、4 つ目のレコードを始めるタグのバイト範囲を場所に持つ（決定 0119）
    let places = diag.iter().next().map(|d| d.places().to_vec()).unwrap();
    let bytes = places[0].bytes().unwrap();
    let tag = std::str::from_utf8(&src[bytes.start as usize..bytes.end as usize]).unwrap();
    assert_eq!(tag.lines().next().map(str::trim), Some("0"), "{tag:?}");
}

#[test]
fn the_signature_finds_ascii_dxf() {
    let src = sample("\r\n");
    let found = |head: &[u8]| Dxf::SIGNATURES.iter().any(|s| s.matches(head));
    // 999 のコメントで始まるものは、まだ判定しない（docs/formats/dxf.md）
    assert!(!found(&src));
    assert!(found(&src[b"999\r\nzumen test\r\n".len()..]));
    assert!(!found(b"PK\x03\x04"));
    assert!(found(&binary_line(true)));
    assert_eq!(Dxf::FORMAT, "dxf");
}

/// 見本の LINE と TEXT のレコードの位置
const LINE: usize = 4;
const TEXT: usize = 6;

/// 1 つの LINE だけを ENTITIES に持つ DXF。`tags` は LINE の 0 の後のタグ
#[cfg(test)]
fn entities(tags: &[u8]) -> DxfFile {
    let head = b"  0\nSECTION\n  2\nENTITIES\n  0\nLINE\n";
    read(&[head.as_slice(), tags, b"  0\nENDSEC\n  0\nEOF\n"].concat())
}

/// 表を作る
#[cfg(test)]
fn table(file: &DxfFile) -> AcadDb {
    file.objects(&mut Cx::new(
        &mut Budget::default(),
        &mut Diagnostics::new(),
        &CancelToken::new(),
    ))
    .unwrap()
}

#[test]
fn lines_have_a_typed_view_on_the_table() {
    let file = read(&sample("\n"));
    let db = table(&file);
    let views: Vec<_> = db.objects().iter().filter_map(|o| o.line()).collect();
    assert_eq!(
        views,
        [Ok(Line {
            layer: b"0",
            start: [0.0, 0.0, 0.0],
            end: [100.0, 50.0, 0.0],
        })]
    );
    let records = file.records();
    assert_eq!(file.real(&records[TEXT], 40), Ok(2.5));
    assert_eq!(file.real(&records[TEXT], 50), Err(ViewError::Missing(50)));

    let broken = entities(b"  8\n0\n 10\nx\n 20\n0\n 11\n1\n 21\n1\n");
    let db = table(&broken);
    assert_eq!(
        db.objects().iter().find_map(|o| o.line()),
        Some(Err(ViewError::NotReal(10)))
    );
    let no_layer = entities(b" 10\n0\n 20\n0\n 11\n1\n 21\n1\n");
    let db = table(&no_layer);
    assert_eq!(
        db.objects().iter().find_map(|o| o.line()),
        Some(Err(ViewError::Missing(8)))
    );
}

#[test]
fn edits_rewrite_only_the_touched_records() {
    let src = sample("\r\n");
    testkit::edit_keeps_untouched_records::<Dxf>(&src, |file, cx| {
        let mut txn = file.transaction();
        txn.set_real(LINE, 11, 200.0)?;
        txn.set_value(TEXT, 5, "変更".as_bytes())?;
        txn.commit(cx)
    })
    .unwrap();

    let mut file = read(&src);
    let mut txn = file.transaction();
    txn.set_real(LINE, 11, 200.0).unwrap();
    txn.commit(&mut Cx::new(
        &mut Budget::default(),
        &mut Diagnostics::new(),
        &CancelToken::new(),
    ))
    .unwrap();
    assert_eq!(file.record(LINE).unwrap().bytes, None);
    assert!(file.record(TEXT).unwrap().bytes.is_some());
    assert_eq!(file.real(&file.records()[LINE], 11), Ok(200.0));

    // 書き直したレコードも、グループ・コードの右寄せと CR LF を保つ
    let out = testkit::write::<Dxf>(&file).unwrap();
    let line =
        b"  0\r\nLINE\r\n  8\r\n0\r\n 10\r\n0.0\r\n 20\r\n0.0\r\n 11\r\n200.0\r\n 21\r\n50.0\r\n";
    assert!(out.windows(line.len()).any(|w| w == line));
    assert_eq!(out.len(), src.len());
}

#[test]
fn failed_edits_change_nothing() {
    type Edit = fn(&mut zumen_acad::DxfTxn<'_>) -> Result<(), Error>;
    let cases: [(Edit, Code); 15] = [
        (
            |t| t.set_value(TEXT, 5, b"two\nlines"),
            codes::DXF_VALUE_UNSAFE,
        ),
        (|t| t.set_value(LINE, 0, b"CIRCLE"), codes::DXF_VALUE_UNSAFE),
        (|t| t.set_value(99, 0, b"x"), codes::DXF_RECORD_MISSING),
        (|t| t.set_value(LINE, 99, b"x"), codes::DXF_TAG_MISSING),
        (|t| t.set_real(LINE, 40, 1.0), codes::DXF_GROUP_CODE_MISSING),
        (
            |t| t.set_real(LINE, 10, f64::NAN),
            codes::DXF_REAL_UNSUPPORTED,
        ),
        (
            |t| t.insert_record(LINE, &[(8, b"0")]),
            codes::DXF_VALUE_UNSAFE,
        ),
        (
            |t| t.insert_record(LINE, &[(0, b"POINT"), (0, b"POINT")]),
            codes::DXF_VALUE_UNSAFE,
        ),
        (
            |t| t.insert_record(LINE, &[(0, b"ENDSEC")]),
            codes::DXF_RECORD_UNSAFE,
        ),
        (
            |t| t.insert_record(99, &[(0, b"POINT")]),
            codes::DXF_RECORD_MISSING,
        ),
        (|t| t.remove_record(1), codes::DXF_RECORD_UNSAFE),
        (
            |t| {
                t.remove_record(TEXT)?;
                t.set_value(TEXT, 5, b"x")
            },
            codes::DXF_RECORD_MISSING,
        ),
        (|t| t.insert_tag(LINE, 0, 8, b"0"), codes::DXF_TAG_MISSING),
        (|t| t.insert_tag(LINE, 1, 0, b"X"), codes::DXF_VALUE_UNSAFE),
        (|t| t.remove_tag(LINE, 0), codes::DXF_VALUE_UNSAFE),
    ];
    for (edit, code) in cases {
        let diag = testkit::failed_edit_changes_nothing::<Dxf>(&sample("\n"), |file, cx| {
            let mut txn = file.transaction();
            // 先に成功した変更も、確定できなければ残らない
            txn.set_real(LINE, 10, 5.0)?;
            assert_eq!(edit(&mut txn), Err(Error::Rejected));
            // 一度失敗したら、ほかの変更も受け付けない
            assert_eq!(txn.set_real(LINE, 20, 5.0), Err(Error::Rejected));
            txn.commit(cx)
        })
        .unwrap();
        assert_eq!(diag.iter().map(|d| d.code()).collect::<Vec<_>>(), [code]);
    }
}

#[test]
fn records_and_tags_can_be_inserted_and_removed() {
    const UNKNOWN: usize = 5;
    let src = sample("\r\n");
    let edit = |file: &mut DxfFile, cx: &mut Cx| {
        let mut txn = file.transaction();
        txn.insert_record(
            TEXT,
            &[(0, b"POINT"), (8, b"0"), (10, b"1.0"), (20, b"2.0")],
        )?;
        txn.remove_record(UNKNOWN)?;
        txn.insert_tag(LINE, 2, 62, b"1")?;
        txn.remove_tag(TEXT, 1)?;
        txn.commit(cx)
    };
    testkit::edit_keeps_untouched_records::<Dxf>(&src, edit).unwrap();

    let mut file = read(&src);
    edit(
        &mut file,
        &mut Cx::new(
            &mut Budget::default(),
            &mut Diagnostics::new(),
            &CancelToken::new(),
        ),
    )
    .unwrap();
    let kinds: Vec<_> = file.records().iter().map(|r| file.kind(r)).collect();
    assert_eq!(kinds[LINE..=TEXT], [&b"LINE"[..], b"POINT", b"TEXT"]);
    assert_eq!(file.record(LINE + 1).unwrap().bytes, None);

    // 足したタグは、グループ・コードを右寄せし、元のファイルの改行で書く
    let out = testkit::write::<Dxf>(&file).unwrap();
    let expected: &[&[u8]] = &[
        b"  0\r\nLINE\r\n  8\r\n0\r\n 62\r\n1\r\n 10\r\n",
        b"  0\r\nPOINT\r\n  8\r\n0\r\n 10\r\n1.0\r\n 20\r\n2.0\r\n  0\r\nTEXT\r\n 10\r\n",
    ];
    for part in expected {
        assert!(out.windows(part.len()).any(|w| w == *part));
    }
    assert!(!out.windows(20).any(|w| w == b"ZUMEN_UNKNOWN_ENTITY"));
}

#[test]
fn new_handles_advance_the_handle_seed() {
    let src = String::from_utf8(sample("\n"))
        .unwrap()
        .replace("$ACADVER", "$HANDSEED\n  5\n2A\n  9\n$ACADVER")
        .into_bytes();
    let mut file = read(&src);
    let mut txn = file.transaction();
    assert_eq!(txn.new_handle(), Ok(0x2A));
    assert_eq!(txn.new_handle(), Ok(0x2B));
    txn.commit(&mut Cx::new(
        &mut Budget::default(),
        &mut Diagnostics::new(),
        &CancelToken::new(),
    ))
    .unwrap();
    let out = testkit::write::<Dxf>(&file).unwrap();
    let seed = b"  9\n$HANDSEED\n  5\n2C\n";
    assert!(out.windows(seed.len()).any(|w| w == seed));

    let diag = testkit::failed_edit_changes_nothing::<Dxf>(&sample("\n"), |file, cx| {
        let mut txn = file.transaction();
        assert_eq!(txn.new_handle(), Err(Error::Rejected));
        txn.commit(cx)
    })
    .unwrap();
    assert_eq!(
        diag.iter().map(|d| d.code()).collect::<Vec<_>>(),
        [codes::DXF_HANDLE_SEED_MISSING]
    );
}

#[test]
fn uncommitted_or_stopped_edits_change_nothing() {
    let src = sample("\n");
    let mut file = read(&src);
    file.transaction().set_real(LINE, 10, 5.0).unwrap();
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);

    let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(1)));
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut txn = file.transaction();
    txn.set_real(LINE, 10, 5.0).unwrap();
    txn.set_value(TEXT, 5, b"x").unwrap();
    let result = txn.commit(&mut Cx::new(&mut budget, &mut diag, &cancel));
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(
        diag.iter().next().map(|d| d.code()),
        Some(codes::BUDGET_WORK_EXCEEDED)
    );
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}

/// 2018 の版の構造を持つ図面
const MODERN: &str = concat!(
    "  0\nSECTION\n  2\nHEADER\n",
    "  9\n$ACADVER\n  1\nAC1032\n",
    "  9\n$HANDSEED\n  5\n2A\n",
    "  9\n$ZUMEN_WIDE\n 90\n7\n",
    "  0\nENDSEC\n",
    "  0\nSECTION\n  2\nCLASSES\n  0\nCLASS\n  1\nX\n  0\nENDSEC\n",
    "  0\nSECTION\n  2\nTABLES\n",
    "  0\nTABLE\n  2\nBLOCK_RECORD\n  0\nBLOCK_RECORD\n  2\n*Model_Space\n  0\nENDTAB\n",
    "  0\nTABLE\n  2\nLAYER\n  5\n2\n330\n0\n100\nAcDbSymbolTable\n 70\n1\n",
    "  0\nLAYER\n  5\n10\n330\n2\n100\nAcDbSymbolTableRecord\n100\nAcDbLayerTableRecord\n",
    "  2\n0\n 70\n0\n 62\n7\n  6\nCONTINUOUS\n370\n-3\n",
    "  0\nENDTAB\n  0\nENDSEC\n",
    "  0\nSECTION\n  2\nENTITIES\n",
    "  0\nLINE\n  5\n20\n102\n{ACAD_REACTORS\n330\n1F\n102\n}\n330\n1F\n100\nAcDbEntity\n  8\n0\n",
    "370\n25\n100\nAcDbLine\n 10\n0.0\n 20\n0.0\n 11\n1.0\n 21\n1.0\n",
    "  0\nMTEXT\n  5\n21\n  1\nX\n",
    "  0\nTEXT\n  5\n22\n  8\n0\n  1\n図面\n",
    "  0\nENDSEC\n",
    "  0\nSECTION\n  2\nOBJECTS\n  0\nDICTIONARY\n  5\nC\n  0\nENDSEC\n",
    "  0\nEOF\n",
);

/// [`MODERN`] を R12 に変えたもの
const MODERN_R12: &str = concat!(
    "  0\nSECTION\n  2\nHEADER\n",
    "  9\n$ACADVER\n  1\nAC1009\n",
    "  9\n$HANDSEED\n  5\n2A\n",
    "  9\n$HANDLING\n 70\n1\n",
    "  0\nENDSEC\n",
    "  0\nSECTION\n  2\nTABLES\n",
    "  0\nTABLE\n  2\nLAYER\n 70\n1\n",
    "  0\nLAYER\n  2\n0\n 70\n0\n 62\n7\n  6\nCONTINUOUS\n",
    "  0\nENDTAB\n  0\nENDSEC\n",
    "  0\nSECTION\n  2\nENTITIES\n",
    "  0\nLINE\n  5\n20\n  8\n0\n 10\n0.0\n 20\n0.0\n 11\n1.0\n 21\n1.0\n",
    "  0\nTEXT\n  5\n22\n  8\n0\n  1\n\\U+56F3\\U+9762\n",
    "  0\nENDSEC\n",
    "  0\nEOF\n",
);

fn write_as(src: &[u8], version: &[u8]) -> (Result<Vec<u8>, Error>, Diagnostics) {
    let file = read(src);
    let mut out = Vec::new();
    let mut diag = Diagnostics::new();
    let result = Dxf::write_as(
        &file,
        version,
        &mut out,
        &mut Cx::new(&mut Budget::default(), &mut diag, &CancelToken::new()),
    );
    (result.map(|()| out), diag)
}

#[test]
fn r12_keeps_what_it_can_hold_and_reports_the_rest() {
    for newline in ["\n", "\r\n"] {
        let src = MODERN.replace('\n', newline);
        assert_eq!(read(src.as_bytes()).version(), Some(&b"AC1032"[..]));
        let (out, diag) = write_as(src.as_bytes(), b"AC1009");
        assert_eq!(
            String::from_utf8(out.unwrap()).unwrap(),
            MODERN_R12.replace('\n', newline)
        );
        let found: Vec<_> = diag
            .iter()
            .map(|d| {
                let params: Vec<String> =
                    d.params().iter().map(|(_, v)| format!("{v:?}")).collect();
                (d.code(), params.join(" "), d.count())
            })
            .collect();
        let expected: Vec<(Code, &str, u64)> = vec![
            (
                codes::DXF_HEADER_VARIABLE_DROPPED,
                r#"Text("$ZUMEN_WIDE")"#,
                1,
            ),
            (codes::DXF_SECTION_DROPPED, r#"Text("CLASSES")"#, 1),
            (codes::DXF_TABLE_DROPPED, r#"Text("BLOCK_RECORD")"#, 1),
            (
                codes::DXF_GROUP_CODE_DROPPED,
                r#"Text("LAYER") Int(370)"#,
                1,
            ),
            (codes::DXF_GROUP_CODE_DROPPED, r#"Text("LINE") Int(370)"#, 1),
            (codes::DXF_ENTITY_DROPPED, r#"Text("MTEXT")"#, 1),
            (codes::DXF_TEXT_EQUIVALENT, "", 1),
            (codes::DXF_SECTION_DROPPED, r#"Text("OBJECTS")"#, 1),
        ];
        let expected: Vec<_> = expected
            .into_iter()
            .map(|(c, p, n)| (c, p.to_string(), n))
            .collect();
        assert_eq!(found, expected);
    }
}

#[test]
fn the_same_version_is_written_unchanged() {
    let src = sample("\r\n");
    let (out, diag) = write_as(&src, b"AC1032");
    assert_eq!(out.unwrap(), src);
    assert!(diag.is_empty());
}

#[test]
fn other_version_changes_are_not_supported_yet() {
    let src = sample("\n");
    for (src, version) in [
        (&src[..], &b"AC1027"[..]),
        (&src[..], b"AC1009X"),
        (
            b"  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1006\n  0\nENDSEC\n  0\nEOF\n",
            b"AC1009",
        ),
    ] {
        let (out, diag) = write_as(src, version);
        assert_eq!(out.unwrap_err(), Error::Unsupported);
        assert_eq!(diag.len(), 1);
        assert_eq!(
            diag.iter().next().unwrap().code(),
            codes::DXF_VERSION_UNSUPPORTED
        );
    }
}

#[test]
fn invalid_utf8_is_kept_as_bytes() {
    let src = b"  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1021\n  9\n$HANDLING\n 70\n1\n  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n  0\nTEXT\n  1\n\xff\n  0\nENDSEC\n  0\nEOF\n";
    let (out, diag) = write_as(src, b"AC1009");
    let out = out.unwrap();
    assert!(out.windows(3).any(|w| w == b"\n\xff\n"));
    assert!(out.windows(6).any(|w| w == b"AC1009"));
    let codes: Vec<Code> = diag.iter().map(|d| d.code()).collect();
    assert_eq!(codes, [codes::DXF_TEXT_INVALID]);
}

#[cfg(test)]
fn cx_for<'a>(
    budget: &'a mut Budget,
    diag: &'a mut Diagnostics,
    cancel: &'a CancelToken,
) -> Cx<'a> {
    Cx::new(budget, diag, cancel)
}

/// 表に足す点
#[cfg(test)]
fn new_point(section: Section, x: f64) -> Object {
    Object {
        kind: b"POINT".to_vec(),
        section,
        values: vec![
            Value {
                code: 8,
                data: Data::Text(b"0".to_vec()),
            },
            Value {
                code: 10,
                data: Data::Real(x),
            },
        ],
        origin: Origin::New,
    }
}

/// 表の LINE（最初のオブジェクト）を変える
#[cfg(test)]
fn edit_line(db: &mut AcadDb, edit: impl FnOnce(&mut Vec<Value>)) {
    edit(&mut db.object_mut(0).unwrap().values);
}

#[test]
fn table_changes_are_written_back_to_the_touched_records() {
    let src = sample("\r\n");
    let file = read(&src);
    let mut db = table(&file);
    // 終点の X を変え、色（62）を足す。見出しの変数を 1 つ変え、1 つ足す
    edit_line(&mut db, |values| {
        values[3].data = Data::Real(200.0);
        values.insert(
            1,
            Value {
                code: 62,
                data: Data::Int(1),
            },
        );
    });
    db.set_variable(
        b"$ZUMEN_UNKNOWN_VAR",
        vec![Value {
            code: 70,
            data: Data::Int(8),
        }],
    );
    db.set_variable(
        b"$INSUNITS",
        vec![Value {
            code: 70,
            data: Data::Int(4),
        }],
    );
    let written = db.clone();
    testkit::edit_keeps_untouched_records::<Dxf>(&src, |file, cx| file.write_back(&written, cx))
        .unwrap();

    let mut file = read(&src);
    let (mut budget, mut diag, cancel) =
        (Budget::default(), Diagnostics::new(), CancelToken::new());
    file.write_back(&db, &mut cx_for(&mut budget, &mut diag, &cancel))
        .unwrap();
    let out = testkit::write::<Dxf>(&file).unwrap();
    let line = b"  0\r\nLINE\r\n  8\r\n0\r\n 62\r\n1\r\n 10\r\n0.0\r\n 20\r\n0.0\r\n 11\r\n200.0\r\n 21\r\n50.0\r\n";
    let header = b"  9\r\n$ZUMEN_UNKNOWN_VAR\r\n 70\r\n8\r\n  9\r\n$INSUNITS\r\n";
    for part in [
        &line[..],
        b"  9\r\n$INSUNITS\r\n 70\r\n4\r\n  0\r\nENDSEC",
        header,
    ] {
        assert!(
            out.windows(part.len()).any(|w| w == part),
            "{}",
            String::from_utf8_lossy(part)
        );
    }
    // 読み直した表は、書き戻した表と同じになる
    assert_eq!(table(&read(&out)), table(&file));
    assert_eq!(table(&file).objects(), db.objects());
    assert_eq!(
        table(&file).variable(b"$INSUNITS"),
        db.variable(b"$INSUNITS")
    );
}

#[test]
fn table_additions_and_removals_are_written_back() {
    let src = sample("\n");
    let mut db = table(&read(&src));
    // 未知の図形を除き、線分の前と最後に点を足す
    db.remove(1).unwrap();
    db.insert(0, new_point(Section::Entities, 1.0));
    db.push(new_point(Section::Entities, 2.0));
    let written = db.clone();
    testkit::edit_keeps_untouched_records::<Dxf>(&src, |file, cx| file.write_back(&written, cx))
        .unwrap();

    let mut file = read(&src);
    let (mut budget, mut diag, cancel) =
        (Budget::default(), Diagnostics::new(), CancelToken::new());
    file.write_back(&db, &mut cx_for(&mut budget, &mut diag, &cancel))
        .unwrap();
    let out = testkit::write::<Dxf>(&file).unwrap();
    for part in [
        &b"ENTITIES\n  0\nPOINT\n  8\n0\n 10\n1.0\n  0\nLINE\n"[..],
        b"\n  0\nPOINT\n  8\n0\n 10\n2.0\n  0\nENDSEC\n",
    ] {
        assert!(
            out.windows(part.len()).any(|w| w == part),
            "{}",
            String::from_utf8_lossy(part)
        );
    }
    assert!(!out.windows(20).any(|w| w == b"ZUMEN_UNKNOWN_ENTITY"));
    // 読み直した表は、元の場所のほかは書き戻した表と同じになる
    let reread = table(&read(&out));
    let strip = |db: &AcadDb| {
        db.objects()
            .iter()
            .map(|o| (o.kind.clone(), o.section.clone(), o.values.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(strip(&reread), strip(&db));
}

#[test]
fn written_back_handles_advance_the_handle_seed() {
    for (handle, seed) in [(0x2A, "2B"), (0x0F, "10")] {
        let src = sample("\n");
        let mut db = table(&read(&src));
        db.set_variable(
            b"$HANDSEED",
            vec![Value {
                code: 5,
                data: Data::Handle(0x10),
            }],
        );
        let mut point = new_point(Section::Entities, 1.0);
        point.values.insert(
            0,
            Value {
                code: 5,
                data: Data::Handle(handle),
            },
        );
        db.push(point);
        let mut file = read(&src);
        let (mut budget, mut diag, cancel) =
            (Budget::default(), Diagnostics::new(), CancelToken::new());
        file.write_back(&db, &mut cx_for(&mut budget, &mut diag, &cancel))
            .unwrap();
        let out = testkit::write::<Dxf>(&file).unwrap();
        let part = format!("  9\n$HANDSEED\n  5\n{seed}\n  0\nENDSEC\n");
        assert!(
            out.windows(part.len()).any(|w| w == part.as_bytes()),
            "{part}"
        );
    }
}

/// 10・20・30 の実数の点
#[cfg(test)]
fn xyz(x: f64, y: f64, z: f64) -> Vec<Value> {
    [(10, x), (20, y), (30, z)]
        .map(|(code, v)| Value {
            code,
            data: Data::Real(v),
        })
        .to_vec()
}

#[test]
fn written_back_entities_widen_the_extents() {
    type Edit = fn(&mut AcadDb);
    let cases: [(Edit, [f64; 3], [f64; 3]); 3] = [
        // 線分の終点を外に出し、外に円を足す
        (
            |db| {
                edit_line(db, |v| v[3].data = Data::Real(200.0));
                let mut circle = Object {
                    kind: b"CIRCLE".to_vec(),
                    ..new_point(Section::Entities, 0.0)
                };
                circle.values = xyz(-10.0, 10.0, 0.0);
                circle.values.push(Value {
                    code: 40,
                    data: Data::Real(5.0),
                });
                db.push(circle);
            },
            [-15.0, 0.0, 0.0],
            [200.0, 50.0, 0.0],
        ),
        // 範囲の中の変更では、範囲は変わらない
        (
            |db| edit_line(db, |v| v[3].data = Data::Real(10.0)),
            [0.0, 0.0, 0.0],
            [100.0, 50.0, 0.0],
        ),
        // 用紙空間の図形は見ない
        (
            |db| {
                let mut point = new_point(Section::Entities, 0.0);
                point.values = xyz(500.0, 500.0, 0.0);
                point.values.push(Value {
                    code: 67,
                    data: Data::Int(1),
                });
                db.push(point);
            },
            [0.0, 0.0, 0.0],
            [100.0, 50.0, 0.0],
        ),
    ];
    for (edit, min, max) in cases {
        let src = sample("\n");
        let mut db = table(&read(&src));
        db.set_variable(b"$EXTMIN", xyz(0.0, 0.0, 0.0));
        db.set_variable(b"$EXTMAX", xyz(100.0, 50.0, 0.0));
        edit(&mut db);
        let mut file = read(&src);
        let (mut budget, mut diag, cancel) =
            (Budget::default(), Diagnostics::new(), CancelToken::new());
        file.write_back(&db, &mut cx_for(&mut budget, &mut diag, &cancel))
            .unwrap();
        let reread = table(&read(&testkit::write::<Dxf>(&file).unwrap()));
        let [x, y, z] = min;
        assert_eq!(reread.variable(b"$EXTMIN"), Some(&xyz(x, y, z)[..]));
        let [x, y, z] = max;
        assert_eq!(reread.variable(b"$EXTMAX"), Some(&xyz(x, y, z)[..]));
    }
}

#[test]
fn removing_an_object_removes_its_dictionary_entries_and_reactors() {
    let lines = [
        "  0",
        "SECTION",
        "  2",
        "OBJECTS", //
        "  0",
        "DICTIONARY",
        "  5",
        "C",
        "330",
        "0",
        "  3",
        "A",
        "350",
        "D",
        "  3",
        "B",
        "350",
        "E", //
        "  0",
        "XRECORD",
        "  5",
        "D",
        "102",
        "{ACAD_REACTORS",
        "330",
        "C",
        "102",
        "}",
        "330",
        "C", //
        "  0",
        "XRECORD",
        "  5",
        "E",
        "102",
        "{ACAD_REACTORS",
        "330",
        "C",
        "330",
        "D",
        "102",
        "}", //
        "102",
        "{ACAD_XDICTIONARY",
        "360",
        "D",
        "102",
        "}",
        "330",
        "C", //
        "  0",
        "ENDSEC",
        "  0",
        "EOF",
    ];
    let src = format!("{}\n", lines.join("\n")).into_bytes();
    let mut file = read(&src);
    let mut db = table(&file);
    db.remove(1).unwrap();
    // 辞書の項目 A と、E の中の D への参照が除かれる。空になった組は組ごと除く。組の外の所有者は残る
    let (mut budget, mut diag, cancel) =
        (Budget::default(), Diagnostics::new(), CancelToken::new());
    file.write_back(&db, &mut cx_for(&mut budget, &mut diag, &cancel))
        .unwrap();
    let expected = [
        "  0",
        "SECTION",
        "  2",
        "OBJECTS", //
        "  0",
        "DICTIONARY",
        "  5",
        "C",
        "330",
        "0",
        "  3",
        "B",
        "350",
        "E", //
        "  0",
        "XRECORD",
        "  5",
        "E",
        "102",
        "{ACAD_REACTORS",
        "330",
        "C",
        "102",
        "}", //
        "330",
        "C", //
        "  0",
        "ENDSEC",
        "  0",
        "EOF",
    ];
    assert_eq!(
        String::from_utf8(testkit::write::<Dxf>(&file).unwrap()).unwrap(),
        format!("{}\n", expected.join("\n"))
    );
}

#[test]
fn an_unchanged_table_writes_the_same_bytes() {
    let src = sample("\n");
    let mut file = read(&src);
    let db = table(&file);
    let (mut budget, mut diag, cancel) =
        (Budget::default(), Diagnostics::new(), CancelToken::new());
    file.write_back(&db, &mut cx_for(&mut budget, &mut diag, &cancel))
        .unwrap();
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
    assert!(diag.is_empty());
}

#[test]
fn table_changes_that_cannot_be_written_back_change_nothing() {
    type Edit = fn(&mut AcadDb);
    let cases: [(Edit, Code); 6] = [
        // 同じレコードから読んだオブジェクトが 2 つある
        (
            |db| {
                let line = db.objects()[0].clone();
                db.push(line);
            },
            codes::DXF_OBJECT_UNSUPPORTED,
        ),
        // 足す所が、このファイルにない
        (
            |db| db.push(new_point(Section::Objects, 1.0)),
            codes::DXF_OBJECT_UNSUPPORTED,
        ),
        (
            |db| db.object_mut(0).unwrap().kind = b"CIRCLE".to_vec(),
            codes::DXF_OBJECT_UNSUPPORTED,
        ),
        (
            |db| edit_line(db, |v| v[1].data = Data::Real(f64::INFINITY)),
            codes::DXF_REAL_UNSUPPORTED,
        ),
        (
            |db| edit_line(db, |v| v[0].data = Data::Text(b"a\nb".to_vec())),
            codes::DXF_VALUE_UNSAFE,
        ),
        (
            |db| {
                edit_line(db, |v| {
                    v.push(Value {
                        code: 0,
                        data: Data::Text(b"X".to_vec()),
                    })
                })
            },
            codes::DXF_VALUE_UNSAFE,
        ),
    ];
    for (edit, code) in cases {
        let src = sample("\n");
        let mut db = table(&read(&src));
        // 先に書き戻せる変更も、残らない
        db.set_variable(
            b"$INSUNITS",
            vec![Value {
                code: 70,
                data: Data::Int(4),
            }],
        );
        edit(&mut db);
        let diag =
            testkit::failed_edit_changes_nothing::<Dxf>(&src, |file, cx| file.write_back(&db, cx))
                .unwrap();
        assert_eq!(diag.iter().map(|d| d.code()).collect::<Vec<_>>(), [code]);
    }
}
