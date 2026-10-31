//! ISO 10303-21 の交換構造の読み取りと、文字列のエスケープの戻し方（決定 0120・0207）。入力は、どれも試験の中で作る。

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Ref, Stop, codes};
use zumen_sdk::Error;
use zumen_step::{Dialect, Exchange, Form, Quote, Record, Value, decode};

/// フィーチャのコメントを読まない読み方。診断は、どの形式のものでもよいので SXF のものを借りる
const PLAIN: Dialect = Dialect {
    record_invalid: codes::SXF_RECORD_INVALID,
    file_invalid: codes::SXF_FILE_INVALID,
    features: &[],
    plain_if_described: None,
};

/// `/*F2` と `/*F` のコメントをフィーチャとして読み、`PLAIN_MODE` と書いたファイルでは読まない読み方
const FEATURES: Dialect = Dialect {
    features: &[(b"/*F2", b"F2*/"), (b"/*F", b"F*/")],
    plain_if_described: Some(b"PLAIN_MODE"),
    ..PLAIN
};

const SRC: &[u8] = b"ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('a test'),'2;1');
FILE_NAME('x.stp','2026-10-31T00:00:00',('author'),('org'),'zumen','it''s',$);
FILE_SCHEMA(('SOME_SCHEMA { 1 0 10303 }'));
ENDSEC;
DATA;
/* a comment between instances */
#1=POINT('p',(0.,-1.5E2,+3),$,*,.T.,\"0FF\",#2);
#2 = /* inside */ LENGTH(LENGTH_MEASURE(1.),(#1,()));
#3=(A() B(.UNSET.) C('x'));
ENDSEC;
END-ISO-10303-21;
";

fn with_cx<T>(limits: Limits, f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics) {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let value = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (value, diag)
}

fn read(src: &[u8], dialect: &Dialect) -> (Result<Exchange, Error>, Diagnostics) {
    with_cx(Limits::default(), |cx| zumen_step::read(src, dialect, cx))
}

fn text<'s>(src: &'s [u8], range: &core::ops::Range<usize>) -> &'s str {
    src.get(range.clone())
        .and_then(|bytes| core::str::from_utf8(bytes).ok())
        .unwrap_or_default()
}

fn name<'s>(src: &'s [u8], record: &Record) -> Vec<&'s str> {
    record
        .parts()
        .iter()
        .map(|part| text(src, &part.name()))
        .collect()
}

#[test]
fn reads_the_header_and_data_sections_as_records() {
    let (exchange, diag) = read(SRC, &PLAIN);
    let exchange = exchange.unwrap();
    assert!(diag.is_empty());
    let names: Vec<_> = exchange
        .records
        .iter()
        .map(|record| name(SRC, record).join(" "))
        .collect();
    assert_eq!(
        names,
        [
            "ISO-10303-21",
            "HEADER",
            "FILE_DESCRIPTION",
            "FILE_NAME",
            "FILE_SCHEMA",
            "ENDSEC",
            "DATA",
            "POINT",
            "LENGTH",
            "A B C",
            "ENDSEC",
            "END-ISO-10303-21"
        ]
    );
    assert_eq!(text(SRC, &exchange.description.unwrap()), "a test");
    assert_eq!(
        text(SRC, &exchange.schema.unwrap()),
        "SOME_SCHEMA { 1 0 10303 }"
    );
    let header = &exchange.records[1];
    assert_eq!((header.form(), header.id()), (Form::Keyword, None));
    // 文のバイト範囲は `;` まで。コメントはレコードにしない
    let point = &exchange.records[7];
    assert_eq!(point.form(), Form::Entity);
    assert_eq!(point.id(), Some(1));
    assert_eq!(point.feature(), None);
    assert!(text(SRC, &point.span()).starts_with("#1=POINT("));
    assert!(text(SRC, &point.span()).ends_with("#2);"));
}

#[test]
fn reads_every_kind_of_value() {
    let (exchange, _) = read(SRC, &PLAIN);
    let exchange = exchange.unwrap();
    let args = exchange.records[7].parts()[0].args();
    let Value::Text { raw, quote } = &args[0] else {
        panic!("{args:?}");
    };
    assert_eq!((text(SRC, raw), *quote), ("p", Quote::Plain));
    let Value::List(numbers) = &args[1] else {
        panic!("{args:?}");
    };
    let numbers: Vec<_> = numbers
        .iter()
        .map(|value| match value {
            Value::Number(range) => text(SRC, range),
            _ => panic!("{value:?}"),
        })
        .collect();
    assert_eq!(numbers, ["0.", "-1.5E2", "+3"]);
    assert_eq!(args[2], Value::Null);
    assert_eq!(args[3], Value::Derived);
    let Value::Enum(e) = &args[4] else {
        panic!("{args:?}");
    };
    assert_eq!(text(SRC, e), "T");
    let Value::Binary(b) = &args[5] else {
        panic!("{args:?}");
    };
    assert_eq!(text(SRC, b), "0FF");
    assert_eq!(args[6], Value::Ref(2));

    // 型を付けた値と、入れ子の並び。文の中のコメントは飛ばす
    let length = &exchange.records[8];
    assert_eq!(length.id(), Some(2));
    let args = length.parts()[0].args();
    let Value::Typed(measure) = &args[0] else {
        panic!("{args:?}");
    };
    assert_eq!(text(SRC, &measure.name()), "LENGTH_MEASURE");
    assert_eq!(
        args[1],
        Value::List(vec![Value::Ref(1), Value::List(Vec::new())])
    );

    // `''` は文字列の中に残し、エスケープは戻さない
    let file_name = &exchange.records[3].parts()[0].args()[5];
    let Value::Text { raw, .. } = file_name else {
        panic!("{file_name:?}");
    };
    assert_eq!(text(SRC, raw), "it''s");
}

#[test]
fn reads_complex_instances_in_order() {
    let (exchange, _) = read(SRC, &PLAIN);
    let complex = &exchange.unwrap().records[9];
    assert_eq!(complex.form(), Form::Complex);
    assert_eq!(complex.id(), Some(3));
    let args: Vec<usize> = complex.parts().iter().map(|p| p.args().len()).collect();
    assert_eq!(args, [0, 1, 1]);
}

const FEATURE_SRC: &[u8] = b"ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('FEATURE_MODE'),'2;1');
ENDSEC;
DATA;
/*F2
#10 = line_feature('1',\\'a\\'b\\', 2.5 ,x y)
F2*/
/*F
#20 = point_feature('\\')
F*/
/* not a feature */
ENDSEC;
END-ISO-10303-21;
";

#[test]
fn reads_feature_comments_given_by_the_dialect() {
    let (exchange, diag) = read(FEATURE_SRC, &FEATURES);
    let exchange = exchange.unwrap();
    assert!(diag.is_empty());
    let features: Vec<_> = exchange
        .records
        .iter()
        .filter_map(|record| Some((record.feature()?, record.id()?)))
        .collect();
    assert_eq!(features, [(0, 10), (1, 20)]);
    let line = exchange
        .records
        .iter()
        .find(|record| record.id() == Some(10))
        .unwrap();
    assert!(text(FEATURE_SRC, &line.span()).ends_with("F2*/"));
    let args = line.parts()[0].args();
    let Value::Text { raw, quote } = &args[1] else {
        panic!("{args:?}");
    };
    // `\'…\'` は、後に `,` か `)` が続く `\'` で閉じる
    assert_eq!(
        (text(FEATURE_SRC, raw), *quote),
        ("a\\'b", Quote::Backslash)
    );
    let tokens: Vec<_> = args[2..]
        .iter()
        .map(|value| match value {
            Value::Token(range) => text(FEATURE_SRC, range),
            _ => panic!("{value:?}"),
        })
        .collect();
    assert_eq!(tokens, ["2.5", "x y"]);
    // フィーチャの中の `'` は、次の `'` で閉じる
    let point = exchange
        .records
        .iter()
        .find(|record| record.id() == Some(20))
        .unwrap();
    let Value::Text { raw, .. } = &point.parts()[0].args()[0] else {
        panic!("{point:?}");
    };
    assert_eq!(text(FEATURE_SRC, raw), "\\");

    // 読み方が名指す語を説明に書いたファイルでは、フィーチャのコメントも読まない
    let plain = String::from_utf8_lossy(FEATURE_SRC).replace("FEATURE_MODE", "PLAIN_MODE");
    let (exchange, diag) = read(plain.as_bytes(), &FEATURES);
    assert!(diag.is_empty());
    assert!(
        exchange
            .unwrap()
            .records
            .iter()
            .all(|record| record.feature().is_none())
    );
    // 読み方がフィーチャを持たなければ、ただのコメント
    let (exchange, _) = read(FEATURE_SRC, &PLAIN);
    assert_eq!(exchange.unwrap().records.len(), 7);
}

#[test]
fn broken_statements_are_reported_and_skipped() {
    let src = b"ISO-10303-21;
HEADER;
ENDSEC;
DATA;
#1=POINT(;
#2=POINT(1.);
#3=;
/*F
#4 = broken_feature(
F*/
ENDSEC;
END-ISO-10303-21;
";
    let (exchange, diag) = read(src, &FEATURES);
    let ids: Vec<_> = exchange
        .unwrap()
        .records
        .iter()
        .filter_map(Record::id)
        .collect();
    assert_eq!(ids, [2]);
    // 同じ診断は 1 つにまとまり、場所を並べる
    let found: Vec<_> = diag.iter().collect();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].code(), codes::SXF_RECORD_INVALID);
    assert_eq!(found[0].count(), 3);
    let places: Vec<_> = found[0]
        .places()
        .iter()
        .map(|place| {
            let bytes = place.bytes().unwrap();
            String::from_utf8_lossy(&src[bytes.start as usize..bytes.end as usize]).into_owned()
        })
        .collect();
    assert_eq!(
        places,
        ["#1=POINT(;", "#3=;", "/*F\n#4 = broken_feature(\nF*/"]
    );
}

#[test]
fn files_without_the_exchange_structure_are_invalid() {
    for src in [
        &b"HEADER;\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n"[..],
        b"ISO-10303-21;\nHEADER;\nENDSEC;\nENDSEC;\nEND-ISO-10303-21;\n",
        b"ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\nENDSEC;\n",
        b"not a step file",
        b"",
    ] {
        let (result, diag) = read(src, &PLAIN);
        assert!(matches!(result, Err(Error::Invalid)), "{src:?}");
        let found: Vec<_> = diag.iter().map(|d| d.code()).collect();
        assert_eq!(found, [codes::SXF_FILE_INVALID], "{src:?}");
    }
}

#[test]
fn budgets_stop_at_the_record_and_the_depth() {
    let (result, diag) = with_cx(
        Limits::default().with(Kind::Records, Limit::Finite(7)),
        |cx| zumen_step::read(SRC, &PLAIN, cx),
    );
    assert!(matches!(result, Err(Error::Stopped(Stop::Exceeded(_)))));
    let stop = diag.iter().next().unwrap();
    assert_eq!(stop.code(), codes::BUDGET_RECORDS_EXCEEDED);
    let place = stop.places().first().and_then(Ref::bytes).unwrap();
    assert!(SRC[place.start as usize..place.end as usize].starts_with(b"#1=POINT("));

    let deep = format!(
        "ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n#1=A({}{});\nENDSEC;\nEND-ISO-10303-21;\n",
        "(".repeat(10),
        ")".repeat(10)
    );
    let (result, diag) = with_cx(
        Limits::default().with(Kind::Depth, Limit::Finite(5)),
        |cx| zumen_step::read(deep.as_bytes(), &PLAIN, cx),
    );
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(
        diag.iter().next().unwrap().code(),
        codes::BUDGET_DEPTH_EXCEEDED
    );
    let (result, _) = with_cx(
        Limits::default().with(Kind::Depth, Limit::Finite(11)),
        |cx| zumen_step::read(deep.as_bytes(), &PLAIN, cx),
    );
    result.unwrap();
}

/// 0x80 以上のバイトを UTF-8 として読む。読めない所は置き換えの文字にする
fn utf8(rest: &[u8]) -> (String, usize) {
    let len = rest.iter().take_while(|b| **b >= 0x80).count();
    (
        String::from_utf8_lossy(rest.get(..len).unwrap_or_default()).into_owned(),
        len,
    )
}

#[test]
fn escapes_are_decoded() {
    assert_eq!(decode(b"it''s", utf8), "it's");
    assert_eq!(decode(br"\X2\30C630B930C8\X0\", utf8), "テスト");
    // 基本多言語面の外は、\X2\ のサロゲートの組か \X4\ で書く
    assert_eq!(decode(br"\X2\D83DDE00\X0\", utf8), "\u{1F600}");
    assert_eq!(decode(br"\X4\0001F600\X0\", utf8), "\u{1F600}");
    assert_eq!(decode(br"\X\E9t\X\E9", utf8), "été");
    assert_eq!(decode(br"\S\i", utf8), "é");
    // \P?\ は文字の集まりを選ぶだけで、文字にならない
    assert_eq!(decode(br"\PA\a\\b", utf8), "a\\b");
    // 閉じのない並びや、桁の合わない並びは戻さない
    assert_eq!(decode(br"\X2\30C\X0\", utf8), r"\X2\30C\X0\");
    assert_eq!(decode(br"\X2\30C6", utf8), r"\X2\30C6");
    // 規格の外の 8 ビットのバイトは、渡した読み方で読む
    assert_eq!(decode("a日本b".as_bytes(), utf8), "a日本b");
    assert_eq!(decode(b"a\xffb", utf8), "a\u{FFFD}b");
    // 読み方が 0 バイトを返しても、1 バイトずつ進む
    assert_eq!(decode(b"\x80\x81z", |_| (String::from("?"), 0)), "??z");
}
