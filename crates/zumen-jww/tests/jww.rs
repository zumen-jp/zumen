//! JWW の読み書き（決定 0133）。入力は、どれも試験の中で作る。

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Ref, Stop, codes};
use zumen_jww::{Jww, JwwFile, RecordKind, Tag};
use zumen_sdk::{Codec, Error, testkit};

mod common;

use common::{Archive, header};

/// 図形を 1 つずつと、ブロックの定義を 1 つ持つ図面。版 700 からは画像を 1 つ持つ
fn drawing(version: u32) -> (Vec<u8>, Archive) {
    let mut archive = Archive::new(version);
    archive.count(9);
    archive.all_kinds();
    archive.count(1);
    archive.block("部品");
    if version >= 700 {
        archive.out.extend(1u32.to_le_bytes());
        archive.cstring("a.bmp");
        archive.out.extend(3u32.to_le_bytes());
        archive.out.extend(b"BMP");
    }
    let mut src = header(version);
    src.extend(&archive.out);
    (src, archive)
}

#[cfg(test)]
fn read(src: &[u8]) -> (JwwFile, Vec<zumen_base::Diagnostic>) {
    let (file, diag) = testkit::read::<Jww>(src);
    (file.unwrap(), diag)
}

fn kinds(file: &JwwFile) -> Vec<String> {
    file.records()
        .iter()
        .map(|record| String::from_utf8_lossy(file.kind_name(record)).into_owned())
        .collect()
}

#[test]
fn records_keep_the_bytes() {
    let (src, archive) = drawing(700);
    testkit::roundtrip::<Jww>(&src).unwrap();
    testkit::records_in_order::<Jww>(&src).unwrap();
    testkit::deterministic::<Jww>(&src).unwrap();
    let (file, diag) = read(&src);
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(file.version(), 700);
    assert_eq!(
        kinds(&file),
        [
            "header",
            "CDataSen",
            "CDataSen",
            "CDataEnko",
            "CDataTen",
            "CDataMoji",
            "CDataSolid",
            "CDataBlock",
            "CDataSunpou",
            "null",
            "CDataList",
            "CDataSen",
            "image-count",
            "image",
        ]
    );
    let records = file.records();
    let header_len = header(700).len();
    assert_eq!(records[0].span(), 0..header_len);
    for (record, start) in records[1..12].iter().zip(&archive.starts) {
        assert_eq!(record.span().start, header_len + start);
    }
    // 図形の一覧の件数は、最初の図形の前にある
    assert_eq!(records[1].span().start, header_len + 2);
    assert_eq!(records[1].tag().map(|(tag, _)| tag), Some(Tag::New));
    assert_eq!(records[2].tag().map(|(tag, _)| tag), Some(Tag::Known));
    // ブロックの定義は、中の一覧の件数までで、中の図形はその後ろ
    assert_eq!(records[10].kind(), RecordKind::Block);
    assert_eq!(records[10].span().end, records[11].span().start);
    assert_eq!(records[11].block(), Some(10));
    assert_eq!(records[1].block(), None);
    assert_eq!(file.bytes(records[13].body()), b"BMP");
    assert_eq!(records[13].span().end, src.len());
    let class = &file.classes()[0];
    assert_eq!(file.bytes(class.name()), b"CDataSen");
    assert_eq!(class.schema(), 1);
}

#[test]
fn older_versions_have_no_images_and_no_pen_width() {
    for version in [230, 300, 351, 420, 600] {
        let (src, _) = drawing(version);
        testkit::roundtrip::<Jww>(&src).unwrap();
        let (file, diag) = read(&src);
        assert!(diag.is_empty(), "{version}: {diag:?}");
        assert_eq!(file.records().len(), 12, "{version}");
        assert_eq!(file.records()[11].span().end, src.len());
    }
}

#[test]
fn an_empty_drawing_ends_with_the_image_count() {
    let mut src = header(700);
    src.extend([0, 0, 0, 0, 0, 0, 0, 0]);
    testkit::roundtrip::<Jww>(&src).unwrap();
    let (file, diag) = read(&src);
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(kinds(&file), ["header", "image-count"]);
    assert_eq!(file.records()[0].span(), 0..src.len() - 8);
}

#[test]
fn unknown_header_fields_are_found_by_the_list_start() {
    let (src, _) = drawing(700);
    let at = header(700).len();
    let mut longer = src[..at].to_vec();
    longer.extend([1; 20]);
    longer.extend(&src[at..]);
    testkit::roundtrip::<Jww>(&longer).unwrap();
    let (file, diag) = read(&longer);
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(file.records()[0].span(), 0..at + 20);
    assert_eq!(file.records().len(), 14);

    // ブロックの定義だけがある図面
    let mut archive = Archive::new(700);
    archive.count(0);
    archive.count(1);
    archive.block("部品");
    archive.out.extend(0u32.to_le_bytes());
    let mut src = header(700);
    src.extend([1; 20]);
    src.extend(&archive.out);
    let (file, diag) = read(&src);
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(
        kinds(&file),
        ["header", "CDataList", "CDataSen", "image-count"]
    );
}

#[test]
fn unreadable_images_are_kept_as_they_are() {
    let (src, _) = drawing(700);
    let mut broken = src.clone();
    // 画像の大きさを、残りより大きくする
    let size = broken.len() - 7;
    broken[size..size + 4].copy_from_slice(&100u32.to_le_bytes());
    testkit::roundtrip::<Jww>(&broken).unwrap();
    let (file, diag) = read(&broken);
    assert_eq!(diag.len(), 1);
    assert_eq!(diag[0].code(), codes::JWW_TRAILER_INVALID);
    let trailer = file.records().last().unwrap();
    assert_eq!(trailer.kind(), RecordKind::Trailer);
    assert_eq!(trailer.span().end, broken.len());
    assert_eq!(
        diag[0]
            .places()
            .first()
            .and_then(Ref::bytes)
            .map(|r| r.start),
        Some(trailer.span().start as u64)
    );
}

#[test]
fn unreadable_files_are_reported() {
    let (result, diag) = testkit::read::<Jww>(b"JwwData");
    assert!(matches!(result, Err(Error::Invalid)));
    assert_eq!(diag[0].code(), codes::JWW_FILE_INVALID);

    // 知らないクラスは読めない
    let mut src = header(700);
    src.extend([1, 0, 0xFF, 0xFF, 1, 0, 8, 0]);
    src.extend(b"CDataXyz");
    src.extend([0; 40]);
    let (result, diag) = testkit::read::<Jww>(&src);
    assert!(matches!(result, Err(Error::Invalid)));
    assert_eq!(diag[0].code(), codes::JWW_BODY_INVALID);

    // 図形の一覧の途中で終わる
    let (src, _) = drawing(600);
    let (result, diag) = testkit::read::<Jww>(&src[..src.len() - 10]);
    assert!(matches!(result, Err(Error::Invalid)));
    assert_eq!(diag[0].code(), codes::JWW_BODY_INVALID);
}

#[test]
fn stopping_for_the_budget_names_the_record() {
    let (src, archive) = drawing(700);
    let mut budget = Budget::new(Limits::default().with(Kind::Records, Limit::Finite(2)));
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = Jww::read(&src, &mut Cx::new(&mut budget, &mut diag, &cancel));
    assert!(matches!(result, Err(Error::Stopped(Stop::Exceeded(_)))));
    let stop = diag.iter().next().unwrap();
    assert_eq!(stop.code(), codes::BUDGET_RECORDS_EXCEEDED);
    let place = stop.places().first().and_then(Ref::bytes).unwrap();
    assert_eq!(place.start as usize, header(700).len() + archive.starts[2]);
}
