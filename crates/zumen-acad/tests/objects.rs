//! DXF を、系統で共有するオブジェクトの表として見る試作（決定 0023）を、試験の中で作った DXF で確かめる。

use zumen_acad::{AcadDb, Data, Dxf, Origin, Section};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_sdk::{Codec, Error};

#[cfg(test)]
fn dxf(lines: &[&str]) -> Vec<u8> {
    let mut out = String::new();
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
    out.into_bytes()
}

#[cfg(test)]
fn objects(src: &[u8], limits: Limits) -> (Result<AcadDb, Error>, Diagnostics) {
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let file = Dxf::read(
        src,
        &mut Cx::new(&mut Budget::default(), &mut diag, &cancel),
    )
    .unwrap();
    let mut budget = Budget::new(limits);
    let result = file.objects(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag)
}

/// R2000 の形の、表・ブロック・図形・オブジェクトを 1 つずつ持つ図面
#[cfg(test)]
fn sample() -> Vec<u8> {
    dxf(&[
        "999",
        "zumen test", //
        "  0",
        "SECTION",
        "  2",
        "HEADER",
        "  9",
        "$ACADVER",
        "  1",
        "AC1015", //
        "  0",
        "ENDSEC", //
        "  0",
        "SECTION",
        "  2",
        "TABLES", //
        "  0",
        "TABLE",
        "  2",
        "LAYER",
        "  5",
        "2",
        "330",
        "0",
        " 70",
        "1", //
        "  0",
        "LAYER",
        "  5",
        "10",
        "330",
        "2",
        "100",
        "AcDbSymbolTableRecord",
        "  2",
        "0",
        " 70",
        "0",
        " 62",
        "7",
        "370",
        "-3",
        "290",
        "1", //
        "  0",
        "ENDTAB", //
        "  0",
        "ENDSEC", //
        "  0",
        "SECTION",
        "  2",
        "ENTITIES", //
        "  0",
        "LINE",
        "  5",
        "2A",
        "330",
        "1F",
        "  8",
        "0",
        " 10",
        "1.5",
        " 20",
        "2",
        " 11",
        "1.0E+2",
        " 21",
        "50", //
        "  0",
        "ZUMEN_UNKNOWN",
        "  5",
        "2B",
        "1001",
        "ZUMEN",
        "1000",
        "keep me ",
        "1071",
        "123456",
        //
        "  0",
        "ENDSEC", //
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
        "310",
        "00FF", //
        "  0",
        "ENDSEC", //
        "  0",
        "EOF",
    ])
}

#[test]
fn records_in_sections_become_objects() {
    let (db, diag) = objects(&sample(), Limits::default());
    let db = db.unwrap();
    assert!(diag.is_empty());
    let found: Vec<_> = db
        .objects()
        .iter()
        .map(|o| (o.kind.as_slice(), o.section.clone(), o.origin))
        .collect();
    let at = |record| Origin::Dxf { record };
    assert_eq!(
        found,
        [
            (&b"TABLE"[..], Section::Tables, at(4)),
            (b"LAYER", Section::Tables, at(5)),
            (b"ENDTAB", Section::Tables, at(6)),
            (b"LINE", Section::Entities, at(9)),
            (b"ZUMEN_UNKNOWN", Section::Entities, at(10)),
            (b"DICTIONARY", Section::Objects, at(13)),
        ]
    );
}

#[test]
fn values_are_typed_by_group_code() {
    let (db, _) = objects(&sample(), Limits::default());
    let db = db.unwrap();
    let line = db.get(0x2A).unwrap();
    assert_eq!(line.kind, b"LINE");
    assert_eq!(line.owner(), Some(0x1F));
    assert_eq!(line.first(8), Some(&Data::Text(b"0".to_vec())));
    assert_eq!(line.real(10), Some(Some(1.5)));
    assert_eq!(line.real(11), Some(Some(100.0)));
    assert_eq!(line.real(30), None);

    let layer = db.get(0x10).unwrap();
    assert_eq!(layer.owner(), Some(2));
    assert_eq!(layer.first(62), Some(&Data::Int(7)));
    assert_eq!(layer.first(370), Some(&Data::Int(-3)));
    assert_eq!(layer.first(290), Some(&Data::Bool(true)));

    // 未知の種類も、拡張データも、そのまま持つ
    let unknown = db.get(0x2B).unwrap();
    assert_eq!(unknown.first(1000), Some(&Data::Text(b"keep me ".to_vec())));
    assert_eq!(unknown.first(1071), Some(&Data::Int(123_456)));

    let dictionary = db.get(0xC).unwrap();
    assert_eq!(dictionary.first(310), Some(&Data::Binary(vec![0, 0xFF])));
}

#[test]
fn objects_without_a_handle_are_kept() {
    // R12 の図形はハンドルを持たないことがある
    let src = dxf(&[
        "  0", "SECTION", "  2", "ENTITIES", "  0", "LINE", "  8", "0", "  0", "ENDSEC", "  0",
        "EOF",
    ]);
    let (db, _) = objects(&src, Limits::default());
    let db = db.unwrap();
    assert_eq!(db.objects().len(), 1);
    assert_eq!(db.objects()[0].handle(), None);
}

#[test]
fn unreadable_values_stay_as_text_and_are_reported() {
    let src = dxf(&[
        "  0", "SECTION", "  2", "ENTITIES", //
        "  0", "LINE", "  5", "XYZ", " 10", "abc", " 62", "1.5", "290", "2", //
        "  0", "ENDSEC", "  0", "EOF",
    ]);
    let (db, diag) = objects(&src, Limits::default());
    let db = db.unwrap();
    let line = &db.objects()[0];
    assert_eq!(line.handle(), None);
    assert_eq!(line.first(5), Some(&Data::Text(b"XYZ".to_vec())));
    assert_eq!(line.real(10), Some(None));
    assert_eq!(line.first(62), Some(&Data::Text(b"1.5".to_vec())));
    let reported: Vec<_> = diag
        .iter()
        .map(|d| (d.code(), d.param("code").cloned(), d.places()[0].record()))
        .collect();
    assert_eq!(reported.len(), 4);
    assert!(
        reported
            .iter()
            .all(|(code, _, record)| *code == codes::DXF_VALUE_INVALID && *record == Some(1))
    );
}

#[test]
fn building_the_table_stops_at_the_work_limit() {
    let (result, diag) = objects(
        &sample(),
        Limits::default().with(Kind::Work, Limit::Finite(3)),
    );
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(
        diag.iter().last().map(|d| d.code()),
        Some(codes::BUDGET_WORK_EXCEEDED)
    );
}
