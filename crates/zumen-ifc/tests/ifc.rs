//! IFC の L1 の読み書き（決定 0211）。ファイルは、どれも試験の中で作る。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, Value, codes};
use zumen_ifc::{Form, Ifc, IfcFile};
use zumen_sdk::{Codec, Error, testkit};

/// スキーマの名前で見出しを変えた、最小の IFC。知らない実体・複合のインスタンス・コメントを含む
fn ifc(schema: &str) -> String {
    format!(
        "ISO-10303-21;\r\nHEADER;\r\n\
         FILE_DESCRIPTION(('ViewDefinition [CoordinationView]'),'2;1');\r\n\
         FILE_NAME('model.ifc','2026-10-31T00:00:00',('author'),('org'),'pre','app','');\r\n\
         FILE_SCHEMA(('{schema}'));\r\nENDSEC;\r\n\r\nDATA;\r\n\
         #1=IFCPROJECT('0YvctVUKr0kugbFTf53O9L',$,'Project',$,$,$,$,(#4),#5);\r\n\
         /* comment */\r\n\
         #2=IFCWALL('1YvctVUKr0kugbFTf53O9L',$,'Wall',$,$,#3,$,$,.STANDARD.);\r\n\
         #3=IFCLOCALPLACEMENT($,#6);\r\n\
         #4=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-05,#6,$);\r\n\
         #5=IFCUNITASSIGNMENT((#7,#8));\r\n\
         #6=IFCAXIS2PLACEMENT3D(#9,$,$);\r\n\
         #7=(IFCNAMEDUNIT(*,.LENGTHUNIT.)IFCSIUNIT(.MILLI.,.METRE.));\r\n\
         #8=IFCSIUNIT(*,.AREAUNIT.,$,.SQUARE_METRE.);\r\n\
         #9=IFCCARTESIANPOINT((0.,0.,0.));\r\n\
         #10=IFCPROPERTYSINGLEVALUE('Height',$,IFCLENGTHMEASURE(2500.),$);\r\n\
         #11=IFCFUTUREENTITY(#2,(#3,(#9)),IFCREF(#10),'it''s');\r\n\
         ENDSEC;\r\nEND-ISO-10303-21;\r\n"
    )
}

const SCHEMAS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];

fn with<T>(limits: Limits, f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics) {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag)
}

fn read(src: &[u8]) -> (Result<IfcFile, Error>, Diagnostics) {
    with(Limits::default(), |cx| Ifc::read(src, cx))
}

fn found(diag: &Diagnostics) -> Vec<Code> {
    diag.iter().map(|d| d.code()).collect()
}

#[test]
fn reads_every_version_and_writes_the_same_bytes() {
    for schema in SCHEMAS {
        let src = ifc(schema);
        let (file, diag) = read(src.as_bytes());
        let file = file.unwrap();
        assert!(diag.is_empty(), "{schema}: {diag:?}");
        assert_eq!(file.schema(), schema.as_bytes());
        testkit::roundtrip::<Ifc>(src.as_bytes()).unwrap();
        testkit::records_in_order::<Ifc>(src.as_bytes()).unwrap();
        testkit::deterministic::<Ifc>(src.as_bytes()).unwrap();
        assert_eq!(testkit::write::<Ifc>(&file).unwrap(), src.as_bytes());
        assert!(Ifc::SIGNATURES.iter().all(|s| s.matches(src.as_bytes())));
    }
}

#[test]
fn keeps_unknown_and_complex_instances() {
    let (file, _) = read(ifc("IFC4").as_bytes());
    let file = file.unwrap();
    // 節の語の 6 つ（始まり・HEADER・ENDSEC・DATA・ENDSEC・終わり）と、見出しの 3 つの文と、11 のインスタンス
    assert_eq!(file.records().len(), 20);
    let keywords = file
        .records()
        .iter()
        .filter(|r| r.form() == Form::Keyword)
        .count();
    assert_eq!(keywords, 6);
    assert_eq!(file.ids().collect::<Vec<_>>(), (1..=11).collect::<Vec<_>>());
    let unknown = file.instance(11).unwrap();
    assert_eq!(file.kind(unknown), b"IFCFUTUREENTITY");
    let unit = file.instance(7).unwrap();
    assert_eq!(unit.form(), Form::Complex);
    let names: Vec<_> = unit.parts().iter().map(|p| file.bytes(p.name())).collect();
    assert_eq!(names, [&b"IFCNAMEDUNIT"[..], b"IFCSIUNIT"]);
    assert_eq!(file.kind(unit), b"IFCNAMEDUNIT");
    assert_eq!(
        file.bytes(unit.span()),
        b"#7=(IFCNAMEDUNIT(*,.LENGTHUNIT.)IFCSIUNIT(.MILLI.,.METRE.));"
    );
}

#[test]
fn follows_references_both_ways() {
    let (file, _) = read(ifc("IFC4").as_bytes());
    let file = file.unwrap();
    // 列の中と、型を付けた値の中の参照も、書いた順に引く
    assert_eq!(file.references(file.instance(11).unwrap()), [2, 3, 9, 10]);
    assert_eq!(file.references(file.instance(1).unwrap()), [4, 5]);
    assert_eq!(
        file.references(file.instance(9).unwrap()),
        Vec::<u64>::new()
    );
    let (inverse, _) = with(Limits::default(), |cx| file.inverse(cx));
    let inverse = inverse.unwrap();
    assert_eq!(inverse.referrers(6), [3, 4]);
    assert_eq!(inverse.referrers(9), [6, 11]);
    assert_eq!(inverse.referrers(2), [11]);
    assert_eq!(inverse.referrers(1), Vec::<u64>::new().as_slice());

    // 逆向きの参照は、インスタンスと参照の数を work で数える
    let (result, diag) = with(Limits::default().with(Kind::Work, Limit::Finite(5)), |cx| {
        file.inverse(cx)
    });
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(found(&diag), [codes::BUDGET_WORK_EXCEEDED]);
}

#[test]
fn reports_missing_references_and_duplicate_ids() {
    let src = ifc("IFC4").replace(
        "#10=IFCPROPERTYSINGLEVALUE",
        "#3=IFCOTHER(#99,(#98,#99));\r\n#10=IFCPROPERTYSINGLEVALUE",
    );
    let (file, diag) = read(src.as_bytes());
    let file = file.unwrap();
    // 同じ番号は、初めのものを引く
    assert_eq!(file.kind(file.instance(3).unwrap()), b"IFCLOCALPLACEMENT");
    assert_eq!(file.ids().filter(|id| *id == 3).count(), 1);
    let codes_found = found(&diag);
    assert_eq!(
        codes_found,
        [codes::IFC_DUPLICATE_DROPPED, codes::IFC_REFERENCE_MISSING]
    );
    let missing = diag
        .iter()
        .find(|d| d.code() == codes::IFC_REFERENCE_MISSING)
        .unwrap();
    assert_eq!(missing.count(), 3);
    assert_eq!(missing.param("id"), Some(&Value::UInt(99)));
    // 変えずに書くと、同じ番号のインスタンスも元のまま
    testkit::roundtrip::<Ifc>(src.as_bytes()).unwrap();
}

#[test]
fn skips_broken_statements_and_rejects_broken_files() {
    let src = ifc("IFC4").replace(
        "#9=IFCCARTESIANPOINT((0.,0.,0.));",
        "#9=IFCCARTESIANPOINT((0.,0.,0.);",
    );
    let (file, diag) = read(src.as_bytes());
    let file = file.unwrap();
    assert!(found(&diag).contains(&codes::IFC_RECORD_INVALID));
    assert!(file.instance(9).is_none());
    testkit::roundtrip::<Ifc>(src.as_bytes()).unwrap();

    for broken in [
        ifc("IFC4").replace("DATA;", "DATA_;"),
        ifc("IFC4").replace("END-ISO-10303-21;", ""),
        ifc("IFC4").replace("ISO-10303-21;\r\nHEADER", "HEADER"),
    ] {
        let (result, diag) = read(broken.as_bytes());
        assert!(matches!(result, Err(Error::Invalid)));
        assert!(found(&diag).contains(&codes::IFC_FILE_INVALID));
    }
}

#[test]
fn does_not_take_sxf_for_ifc() {
    let sxf = ifc("ASSOCIATIVE_DRAUGHTING");
    assert!(!Ifc::SIGNATURES.iter().any(|s| s.matches(sxf.as_bytes())));
    let (result, diag) = read(sxf.as_bytes());
    assert!(matches!(result, Err(Error::Unsupported)));
    assert_eq!(found(&diag), [codes::IFC_SCHEMA_UNSUPPORTED]);
    assert_eq!(
        diag.iter().next().unwrap().param("schema"),
        Some(&Value::Text("ASSOCIATIVE_DRAUGHTING".to_owned()))
    );
    // SXF のコーデックも、IFC を読まない
    for schema in SCHEMAS {
        let src = ifc(schema);
        let (result, diag) = with(Limits::default(), |cx| {
            zumen_sxf::Sxf::read(src.as_bytes(), cx)
        });
        assert!(matches!(result, Err(Error::Unsupported)));
        assert_eq!(found(&diag), [codes::SXF_SCHEMA_UNSUPPORTED]);
    }
    // FILE_SCHEMA のないものは、IFC でない
    let none = ifc("IFC4").replace("FILE_SCHEMA(('IFC4'));\r\n", "");
    assert!(!Ifc::SIGNATURES.iter().any(|s| s.matches(none.as_bytes())));
    let (result, _) = read(none.as_bytes());
    assert!(matches!(result, Err(Error::Unsupported)));
}

#[test]
fn budgets_stop_reading() {
    let src = ifc("IFC4");
    for (kind, code) in [
        (Kind::InputBytes, codes::BUDGET_INPUT_BYTES_EXCEEDED),
        (Kind::Records, codes::BUDGET_RECORDS_EXCEEDED),
        (Kind::Work, codes::BUDGET_WORK_EXCEEDED),
    ] {
        let (result, diag) = with(Limits::default().with(kind, Limit::Finite(5)), |cx| {
            Ifc::read(src.as_bytes(), cx)
        });
        assert!(matches!(result, Err(Error::Stopped(_))), "{kind:?}");
        assert_eq!(found(&diag), [code], "{kind:?}");
    }
}

/// どこで切っても、どのバイトを変えても、パニックせずに読むか断る
#[test]
fn truncated_and_mutated_files_do_not_panic() {
    let src = ifc("IFC4").into_bytes();
    let try_all = |src: &[u8]| {
        let (file, _) = read(src);
        if let Ok(file) = file {
            let _ = with(Limits::default(), |cx| file.inverse(cx));
            let _ = testkit::write::<Ifc>(&file);
        }
    };
    for end in 0..src.len() {
        try_all(&src[..end]);
    }
    for at in 0..src.len() {
        for byte in *b"#()';0" {
            let mut changed = src.clone();
            changed[at] = byte;
            try_all(&changed);
        }
    }
}
