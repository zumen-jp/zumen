//! PDF のファイルモデル（L1）の読み書きと、テストキットの契約（決定 0010・0156・0255）。入力は、どれも試験の中で
//! 作る。

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, codes};
use zumen_drawing::{Drawing2D, id};
use zumen_pdf::{Pdf, PdfFile, RecordKind};
use zumen_sdk::{Codec, Error, FileModel, Lift, testkit};

mod common;

use common::{Document, Page};

fn read(src: &[u8]) -> (PdfFile, Vec<zumen_base::Diagnostic>) {
    let (file, diag) = testkit::read::<Pdf>(src);
    (file.unwrap(), diag)
}

fn kinds(file: &PdfFile) -> Vec<RecordKind> {
    file.records().iter().map(|record| record.kind).collect()
}

/// 線・文字・画像・表示グループを持つ 2 ページの文書
fn sample() -> Vec<u8> {
    let mut document = Document::new(vec![
        Page::new(
            "1 w 0 0 m 72 0 l S /OC /oc1 BDC 10 10 50 30 re f EMC \
             BT /F1 12 Tf 72 720 Td (Plan) Tj ET q 20 0 0 20 100 100 cm /Im1 Do Q",
        )
        .resources(
            "/Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> \
             /Properties << /oc1 3 0 R >> /XObject << /Im1 4 0 R >>",
        ),
        Page::new("0 0 m 10 10 20 10 30 0 c S"),
    ]);
    document.objects = vec![
        b"<< /Type /OCG /Name (Walls) >>".to_vec(),
        common::stream(
            "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray \
             /BitsPerComponent 8 /Filter /ASCIIHexDecode",
            "80>",
        ),
    ];
    document.catalog = "/OCProperties << /OCGs [3 0 R] /D << >> >>";
    document.bytes()
}

#[test]
fn records_keep_the_bytes() {
    let src = sample();
    testkit::roundtrip::<Pdf>(&src).unwrap();
    testkit::records_in_order::<Pdf>(&src).unwrap();
    testkit::deterministic::<Pdf>(&src).unwrap();
    let (file, diag) = read(&src);
    assert!(diag.is_empty(), "{diag:?}");
    let mut expected = vec![RecordKind::Header, RecordKind::Comment];
    expected.extend([RecordKind::Object; 8]);
    expected.extend([
        RecordKind::Xref,
        RecordKind::Trailer,
        RecordKind::StartXref,
        RecordKind::Comment,
    ]);
    assert_eq!(kinds(&file), expected);
    // オブジェクトの番号と世代番号
    let objects: Vec<_> = file
        .records()
        .iter()
        .filter_map(|record| record.object)
        .collect();
    assert_eq!(objects, (1..=8).map(|n| (n, 0)).collect::<Vec<_>>());
    // オブジェクトのレコードは `n g obj` から `endobj` まで
    let first = &file.records()[2];
    let bytes = file.bytes(first.start, first.end);
    assert!(bytes.starts_with(b"1 0 obj") && bytes.ends_with(b"endobj"));
    let info = file.record(2).unwrap();
    assert_eq!(info.kind, b"obj");
}

#[test]
fn streams_may_hold_any_bytes() {
    // ストリームの中の `endobj` と、長さの後ろの空白は、オブジェクトを区切らない
    let mut builder = common::Builder::default();
    builder.object("<< /Type /Catalog /Pages 2 0 R >>");
    builder.object("<< /Type /Pages /Kids [] /Count 0 >>");
    builder.stream("", "endobj 3 0 obj (\\)) %");
    builder.object("(a string with endobj and ) inside) ");
    let src = builder.bytes(1);
    let (file, diag) = read(&src);
    assert!(diag.is_empty(), "{diag:?}");
    let objects = file
        .records()
        .iter()
        .filter(|record| record.kind == RecordKind::Object)
        .count();
    assert_eq!(objects, 4);
    testkit::roundtrip::<Pdf>(&src).unwrap();
}

#[test]
fn repairs_objects_without_endobj() {
    let src = b"%PDF-1.4\n1 0 obj << /Type /Catalog >>\n2 0 obj (two) endobj\n3 0 obj [3".to_vec();
    let (file, diag) = read(&src);
    let repaired = diag
        .iter()
        .find(|d| d.code() == codes::PDF_OBJECT_REPAIRED)
        .unwrap();
    assert_eq!(repaired.count(), 2);
    let objects: Vec<_> = file
        .records()
        .iter()
        .filter(|record| record.kind == RecordKind::Object)
        .map(|record| (record.object, file.bytes(record.start, record.end).to_vec()))
        .collect();
    assert_eq!(
        objects,
        [
            (Some((1, 0)), b"1 0 obj << /Type /Catalog >>".to_vec()),
            (Some((2, 0)), b"2 0 obj (two) endobj".to_vec()),
            (Some((3, 0)), b"3 0 obj [3".to_vec()),
        ]
    );
    testkit::roundtrip::<Pdf>(&src).unwrap();
}

#[test]
fn keeps_unknown_bytes_and_a_late_header() {
    // 見出しの前の余計なバイトと、構造の外のバイトは、未知のレコードにする
    let src = b"junk before\n%PDF-2.0\nnot an object\n1 0 obj null endobj\n".to_vec();
    let (file, diag) = read(&src);
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(
        kinds(&file),
        [
            RecordKind::Unknown,
            RecordKind::Header,
            RecordKind::Unknown,
            RecordKind::Object
        ]
    );
    testkit::roundtrip::<Pdf>(&src).unwrap();
}

#[test]
fn rejects_files_without_a_header() {
    let (file, diag) = testkit::read::<Pdf>(b"1 0 obj null endobj");
    assert!(matches!(file, Err(Error::Invalid)));
    assert_eq!(diag[0].code(), codes::PDF_FILE_INVALID);
    // 見出しは先頭の 1,024 バイトの中に要る
    let mut late = vec![b' '; 1024];
    late.extend(b"%PDF-1.7\n");
    assert!(matches!(testkit::read::<Pdf>(&late).0, Err(Error::Invalid)));
}

#[test]
fn detects_the_header() {
    assert_eq!(Pdf::FORMAT, "pdf");
    assert!(matches!(
        Pdf::SIGNATURES,
        [zumen_sdk::Signature::Bytes {
            offset: 0,
            bytes: b"%PDF-"
        }]
    ));
}

/// 読んで図面モデルにし、要素の ID を振る順に並べる
fn ids(src: &[u8]) -> Result<Vec<String>, String> {
    let file = testkit::read::<Pdf>(src)
        .0
        .map_err(|e| format!("読めない: {e:?}"))?;
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let drawing: Drawing2D = file
        .lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
        .map_err(|e| format!("図面モデルにできない: {e:?}"))?;
    let mut out = Vec::new();
    id::each_element(&drawing, |_, id, _| {
        out.push(id);
        Ok::<(), ()>(())
    })
    .map_err(|()| "ID を振れない".to_string())?;
    Ok(out)
}

#[test]
fn keeps_the_budget_and_the_ids() {
    let src = sample();
    testkit::stops_at_same_place::<Pdf>(&src).unwrap();
    testkit::ids_match(&src, include_str!("ids/pdf.txt"), ids).unwrap();
}

#[test]
fn survives_cut_and_broken_files() {
    let src = sample();
    for end in (0..src.len()).step_by(7) {
        testkit::survives::<Pdf>(&src[..end]).unwrap();
    }
    // 壊れた相互参照・ページの木を指さないカタログ・閉じない辞書と文字列
    for broken in [
        b"%PDF-1.7\nxref\n0 1\nnot a table\ntrailer << /Root 9 0 R >>".as_slice(),
        b"%PDF-1.7\n1 0 obj << /Type /Catalog /Pages 1 0 R >> endobj\ntrailer << /Root 1 0 R >>",
        b"%PDF-1.7\n1 0 obj << << << (((",
        b"%PDF-1.7\n1 0 obj << /Length 99999999999999999999 >> stream\nabc",
    ] {
        testkit::survives::<Pdf>(broken).unwrap();
    }
}
