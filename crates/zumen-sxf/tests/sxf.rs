//! SXF の交換構造の読み書き（決定 0120）。入力は、どれも試験の中で作る。

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Ref, Stop, codes};
use zumen_sdk::{Codec, Error, testkit};
use zumen_sxf::{Form, Marker, Mode, Quote, SfcRecord, Sxf, SxfFile, Value};

const SFC: &[u8] = b"ISO-10303-21;\r
HEADER;\r
FILE_DESCRIPTION(('SCADEC level2 feature_mode'),'2;1');\r
FILE_NAME('nosheet.sfc','2026-10-18T00:00:00',('author'),('org'),'zumen','test','');\r
FILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\r
ENDSEC;\r
DATA;\r
/*SXF3\r
#10 = line_feature('1','1','1','1','0.000000','0.000000','10.000000','0.000000')\r
SXF3*/\r
/*SXF3\r
#20 = circle_feature('1','2','1','1','5.000000','5.000000','3.000000')\r
SXF3*/\r
ENDSEC;\r
END-ISO-10303-21;\r
";

const P21: &[u8] = b"ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('SCADEC level2 AP202_mode'),'2;1');
FILE_NAME('nosheet.p21','2026-10-18T00:00:00',('author'),('org'),'zumen','zumen \\X2\\30C630B930C8\\X0\\ it''s','');
FILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));
ENDSEC;
DATA;
/* comment; with a semicolon */
#5=(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.));
#40=CARTESIAN_POINT(' ',(0.000000,-1.5E-3));
#45=AXIS2_PLACEMENT_2D(' ',#40,$);
#46=CIRCLE(' ',#45,5.000000);
#47=LENGTH_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.),#5);
ENDSEC;
END-ISO-10303-21;
";

#[cfg(test)]
fn read(src: &[u8]) -> SxfFile {
    testkit::read::<Sxf>(src).0.unwrap()
}

#[cfg(test)]
fn kinds(file: &SxfFile) -> Vec<String> {
    file.records()
        .iter()
        .map(|record| String::from_utf8_lossy(file.kind(record)).into_owned())
        .collect()
}

#[test]
fn both_modes_pass_the_codec_kit() {
    for src in [SFC, P21] {
        testkit::roundtrip::<Sxf>(src).unwrap();
        testkit::records_in_order::<Sxf>(src).unwrap();
        testkit::deterministic::<Sxf>(src).unwrap();
    }
    assert!(Sxf::SIGNATURES.iter().all(|s| s.matches(SFC)));
}

#[test]
fn reads_sfc_features_as_records() {
    let file = read(SFC);
    assert_eq!(file.mode(), Mode::Sfc);
    assert_eq!(
        kinds(&file),
        [
            "ISO-10303-21",
            "HEADER",
            "FILE_DESCRIPTION",
            "FILE_NAME",
            "FILE_SCHEMA",
            "ENDSEC",
            "DATA",
            "line_feature",
            "circle_feature",
            "ENDSEC",
            "END-ISO-10303-21"
        ]
    );
    let line = &file.records()[7];
    assert_eq!(line.id(), Some(10));
    assert_eq!(line.marker(), Some(Marker::Sxf3));
    assert!(file.bytes(line.span()).starts_with(b"/*SXF3"));
    assert!(file.bytes(line.span()).ends_with(b"SXF3*/"));
    let args = line.parts()[0].args();
    assert_eq!(args.len(), 8);
    let Value::Text { raw, quote } = &args[6] else {
        panic!("{:?}", args[6]);
    };
    assert_eq!(*quote, Quote::Plain);
    assert_eq!(file.bytes(raw.clone()), b"10.000000");
    assert_eq!(file.records()[3].form(), Form::Entity);
    assert_eq!(file.records()[1].form(), Form::Keyword);
}

#[test]
fn reads_p21_instances_and_values() {
    let file = read(P21);
    assert_eq!(file.mode(), Mode::P21);
    let records = file.records();
    let name = &records[3].parts()[0].args()[5];
    let Value::Text { raw, .. } = name else {
        panic!("{name:?}");
    };
    // `''` は文字列を閉じない。エスケープは戻さない
    assert_eq!(
        file.bytes(raw.clone()),
        b"zumen \\X2\\30C630B930C8\\X0\\ it''s"
    );

    let unit = &records[7];
    assert_eq!(unit.form(), Form::Complex);
    assert_eq!(unit.id(), Some(5));
    let parts: Vec<_> = unit.parts().iter().map(|p| file.bytes(p.name())).collect();
    assert_eq!(parts, [&b"LENGTH_UNIT"[..], b"NAMED_UNIT", b"SI_UNIT"]);
    assert_eq!(unit.parts()[1].args(), [Value::Derived]);
    let Value::Enum(milli) = &unit.parts()[2].args()[0] else {
        panic!();
    };
    assert_eq!(file.bytes(milli.clone()), b"MILLI");

    let Value::List(point) = &records[8].parts()[0].args()[1] else {
        panic!();
    };
    let Value::Number(y) = &point[1] else {
        panic!();
    };
    assert_eq!(file.bytes(y.clone()), b"-1.5E-3");
    assert_eq!(
        records[9].parts()[0].args()[1..],
        [Value::Ref(40), Value::Null]
    );
    let Value::Typed(measure) = &records[11].parts()[0].args()[0] else {
        panic!();
    };
    assert_eq!(file.bytes(measure.name()), b"LENGTH_MEASURE");
}

#[test]
fn reads_sfc_text_with_shift_jis_and_backslashes() {
    // 「表」は 0x95 0x5C。2 バイト目の `\` と、閉じない `\'` を、文字列の中として読む
    let mut src = SFC.to_vec();
    let at = src.windows(6).position(|w| w == b"/*SXF3").unwrap();
    let text: &[u8] = b"/*SXF\r\n#30 = text_string_feature('1',\\'\x95\x5c\\'s a\\\\b\\','x', ( 1 ,2 ),\r\n plain )\r\nSXF*/\r\n";
    src.splice(at..at, text.iter().copied());
    let file = read(&src);
    testkit::roundtrip::<Sxf>(&src).unwrap();
    let record = &file.records()[7];
    assert_eq!(record.marker(), Some(Marker::Sxf));
    let args = record.parts()[0].args();
    let Value::Text { raw, quote } = &args[1] else {
        panic!("{:?}", args[1]);
    };
    assert_eq!(*quote, Quote::Backslash);
    assert_eq!(file.bytes(raw.clone()), b"\x95\x5c\\'s a\\\\b");
    assert!(matches!(&args[3], Value::List(items) if items.len() == 2));
    let Value::Token(token) = &args[4] else {
        panic!("{:?}", args[4]);
    };
    assert_eq!(file.bytes(token.clone()), b"plain");
}

#[test]
fn skips_a_broken_record_and_reads_the_next() {
    let mut src = P21.to_vec();
    let at = src.windows(4).position(|w| w == b"#40=").unwrap();
    src.splice(at..at, b"#41=BROKEN(,);\n".iter().copied());
    let (file, diag) = testkit::read::<Sxf>(&src);
    let file = file.unwrap();
    testkit::roundtrip::<Sxf>(&src).unwrap();
    assert!(kinds(&file).contains(&"CARTESIAN_POINT".to_string()));
    assert!(!kinds(&file).contains(&"BROKEN".to_string()));
    assert_eq!(diag.len(), 1);
    assert_eq!(diag[0].code(), codes::SXF_RECORD_INVALID);
    let place = diag[0].places()[0].bytes().unwrap();
    assert_eq!(
        &src[place.start as usize..place.end as usize],
        b"#41=BROKEN(,);"
    );
}

#[test]
fn an_unclosed_feature_is_reported_and_not_read() {
    let src = b"ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n/*SXF3\n#10 = line_feature('1')\nENDSEC;\nEND-ISO-10303-21;\n";
    let (result, diag) = testkit::read::<Sxf>(src);
    // 閉じのないコメントは、終わりまでを飲み込むので、終わりの文がないことになる
    assert_eq!(result.unwrap_err(), Error::Invalid);
    assert_eq!(diag[0].code(), codes::SXF_RECORD_INVALID);
    assert_eq!(diag[1].code(), codes::SXF_FILE_INVALID);
}

#[test]
fn rejects_files_without_the_exchange_structure() {
    let no_start = b"HEADER;\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n";
    let no_end = b"ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\nENDSEC;\n";
    let no_data = b"ISO-10303-21;\nHEADER;\nENDSEC;\nEND-ISO-10303-21;\n";
    for src in [&no_start[..], no_end, no_data] {
        let (result, diag) = testkit::read::<Sxf>(src);
        assert_eq!(result.unwrap_err(), Error::Invalid);
        assert_eq!(diag.last().unwrap().code(), codes::SXF_FILE_INVALID);
    }
}

#[test]
fn rejects_other_schemas() {
    let src = String::from_utf8(P21.to_vec())
        .unwrap()
        .replace("'ASSOCIATIVE_DRAUGHTING'", "'IFC4'");
    let (result, diag) = testkit::read::<Sxf>(src.as_bytes());
    assert_eq!(result.unwrap_err(), Error::Unsupported);
    assert_eq!(diag[0].code(), codes::SXF_SCHEMA_UNSUPPORTED);
    // 版の識別子を後に付けたスキーマの名前は、名前だけを比べる
    let src = String::from_utf8(P21.to_vec()).unwrap().replace(
        "'ASSOCIATIVE_DRAUGHTING'",
        "'associative_draughting { 1 0 10303 202 }'",
    );
    read(src.as_bytes());
}

#[cfg(test)]
fn read_with(src: &[u8], kind: Kind, limit: u64) -> (Result<SxfFile, Error>, Diagnostics) {
    let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = Sxf::read(src, &mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag)
}

#[test]
fn copying_a_large_source_counts_work() {
    let make = |length| {
        format!(
            "ISO-10303-21;\nHEADER;\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\n/*{}*/\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n",
            "x".repeat(length)
        )
    };
    let short = make(1);
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    Sxf::read(
        short.as_bytes(),
        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
    )
    .unwrap();
    let used = budget.used(Kind::Work);
    let long = make(100_000);
    let (result, _) = read_with(long.as_bytes(), Kind::Work, used);
    assert!(matches!(result, Err(Error::Stopped(_))));
}

#[test]
fn stopping_for_the_budget_names_the_record() {
    let (result, diag) = read_with(SFC, Kind::Records, 7);
    assert!(matches!(result, Err(Error::Stopped(Stop::Exceeded(_)))));
    let stop = diag.iter().next().unwrap();
    assert_eq!(stop.code(), codes::BUDGET_RECORDS_EXCEEDED);
    let Some(place) = stop.places().first().and_then(Ref::bytes) else {
        panic!("{stop:?}");
    };
    assert!(SFC[place.start as usize..place.end as usize].starts_with(b"/*SXF3\r\n#10"));

    // 引数の入れ子は、深さの予算で止める
    let deep = format!(
        "ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n#1=A({}{});\nENDSEC;\nEND-ISO-10303-21;\n",
        "(".repeat(10),
        ")".repeat(10)
    );
    let (result, diag) = read_with(deep.as_bytes(), Kind::Depth, 5);
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(
        diag.iter().next().unwrap().code(),
        codes::BUDGET_DEPTH_EXCEEDED
    );
    let (result, _) = read_with(deep.as_bytes(), Kind::Depth, 11);
    result.unwrap();
}
