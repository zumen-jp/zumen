//! 利用者が渡す EXPRESS でのチェック（決定 0216）。EXPRESS は、試験の中で書いた小さなものだけを使う。

use zumen_base::{
    Budget, CancelToken, Code, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, Value, codes,
};
use zumen_ifc::{Express, Ifc, IfcFile, check};
use zumen_sdk::{Codec, Error};

const EXPRESS: &str = "(* 試験用の小さなスキーマ (* 入れ子の注釈 *) *)
SCHEMA IFCTINY '{ tiny 1 }';
TYPE LabelT = STRING; END_TYPE;
TYPE LengthT = REAL; WHERE WR1 : SELF > 0.; END_TYPE;
TYPE CountT = INTEGER; END_TYPE;
TYPE KindE = ENUMERATION OF (STANDARD, NOTDEFINED); END_TYPE;
TYPE ValueS = SELECT (LabelT, LengthT); END_TYPE;
TYPE ThingS = SELECT (Wall, Inner); END_TYPE;
TYPE Inner = SELECT (Point); END_TYPE;

ENTITY Root ABSTRACT SUPERTYPE OF (ONEOF (Wall, Point));
  Name : OPTIONAL LabelT;
END_ENTITY;

ENTITY Wall SUBTYPE OF (Root);
  Kind : KindE;
  Height : LengthT;
  Placement : OPTIONAL Point;
  Tags : OPTIONAL SET [1:?] OF LabelT;
  Flag : BOOLEAN;
UNIQUE UR1 : Name;
END_ENTITY;

ENTITY Point SUBTYPE OF (Root);
  Coordinates : LIST [2:3] OF LengthT;
DERIVE
  Dim : CountT := SIZEOF(Coordinates);
  SELF\\Root.Name : LabelT := 'pt';
END_ENTITY;

ENTITY Prop;
  Value : OPTIONAL ValueS;
  Target : ThingS;
  Counts : LIST [0:?] OF OPTIONAL CountT;
  Raw : BINARY (32) FIXED;
  Logic : LOGICAL;
END_ENTITY;

ENTITY Named;
  Unit : KindE;
END_ENTITY;

ENTITY Si;
  Prefix : OPTIONAL KindE;
END_ENTITY;

FUNCTION Twice (X : INTEGER) : INTEGER;
  IF X > 0 THEN RETURN (X * 2); END_IF;
  RETURN (0);
END_FUNCTION;

RULE OneWall FOR (Wall);
WHERE
  WR1 : SIZEOF(Wall) >= 0;
END_RULE;
END_SCHEMA;
";

fn ifc(schema: &str, data: &str) -> String {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\n\
         FILE_NAME('','',(''),(''),'','','');\nFILE_SCHEMA(('{schema}'));\nENDSEC;\nDATA;\n\
         {data}ENDSEC;\nEND-ISO-10303-21;\n"
    )
}

/// 決まりに合うインスタンス
const GOOD: &str = "#1=WALL('w',.STANDARD.,2500.,#2,('a','b'),.T.);\n\
#2=POINT(*,(0.,0.,1.5E-3));\n\
#3=PROP(LENGTHT(1.),#2,(1,$,3),\"0FF\",.U.);\n\
#4=PROP($,#1,(),\"0\",.F.);\n\
#5=(NAMED(.STANDARD.)SI($));\n\
#6=WALL($,.NOTDEFINED.,1,$,$,.F.);\n";

fn with<T>(limits: Limits, f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics) {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag)
}

fn schema() -> Result<Express, Error> {
    with(Limits::default(), |cx| {
        Express::read(EXPRESS.as_bytes(), cx)
    })
    .0
}

fn file(data: &str) -> Result<IfcFile, Error> {
    with(Limits::default(), |cx| {
        Ifc::read(ifc("IFCTINY", data).as_bytes(), cx)
    })
    .0
}

fn run(data: &str) -> Result<(Vec<Diagnostic>, Diagnostics), Error> {
    let (file, schema) = (file(data)?, schema()?);
    let (findings, diag) = with(Limits::default(), |cx| check(&file, &schema, cx));
    Ok((findings?, diag))
}

fn found(findings: &[Diagnostic]) -> Vec<(Code, Option<String>)> {
    findings
        .iter()
        .map(|d| {
            (
                d.code(),
                d.places().first().and_then(|p| p.id()).map(str::to_owned),
            )
        })
        .collect()
}

#[test]
fn reads_the_schema_and_counts_rules_it_does_not_evaluate() {
    let schema = schema().unwrap();
    assert_eq!(schema.name(), "IFCTINY");
    // 型の WHERE・実体の UNIQUE・大域の RULE
    assert_eq!(schema.rules(), 3);
}

#[test]
fn finds_nothing_in_a_file_that_follows_the_schema() {
    let (findings, diag) = run(GOOD).unwrap();
    assert!(findings.is_empty(), "{findings:?}");
    let codes_found: Vec<_> = diag.iter().map(|d| d.code()).collect();
    assert_eq!(codes_found, [codes::EXPRESS_RULE_UNSUPPORTED]);
    assert_eq!(
        diag.iter().next().unwrap().param("rules"),
        Some(&Value::UInt(3))
    );
}

#[test]
fn finds_values_that_do_not_follow_the_schema() {
    let data = "#1=WALL('w',.OTHER.,'tall',#3,(),1);\n\
#2=POINT('p',(0.));\n\
#3=PROP(LENGTHT('x'),#3,(1.5),\"0\",.T.);\n\
#4=PROP(COUNTT(1),$,(),\"0\",.T.);\n\
#5=ROOT($);\n\
#6=GHOST(1);\n\
#7=WALL('w',.STANDARD.,1.);\n\
#8=PROP($,#2,(),\"0\",*);\n";
    let (findings, _) = run(data).unwrap();
    let id = |n: u32| Some(format!("#{n}"));
    assert_eq!(
        found(&findings),
        [
            // 列挙にない値・数でない値・違う実体の参照・空の集まり・数の BOOLEAN
            (codes::EXPRESS_VALUE_INVALID, id(1)),
            (codes::EXPRESS_VALUE_INVALID, id(1)),
            (codes::EXPRESS_VALUE_INVALID, id(1)),
            (codes::EXPRESS_VALUE_INVALID, id(1)),
            (codes::EXPRESS_VALUE_INVALID, id(1)),
            // 導く属性に値を書いた・集まりの数が足りない
            (codes::EXPRESS_VALUE_INVALID, id(2)),
            (codes::EXPRESS_VALUE_INVALID, id(2)),
            // 型を付けた値の中身・選択にない実体・整数でない数
            (codes::EXPRESS_VALUE_INVALID, id(3)),
            (codes::EXPRESS_VALUE_INVALID, id(3)),
            (codes::EXPRESS_VALUE_INVALID, id(3)),
            // 選択にない定義された型・省略できない属性の $
            (codes::EXPRESS_VALUE_INVALID, id(4)),
            (codes::EXPRESS_VALUE_MISSING, id(4)),
            (codes::EXPRESS_ABSTRACT_INVALID, id(5)),
            (codes::EXPRESS_ENTITY_MISSING, id(6)),
            (codes::EXPRESS_ATTRIBUTE_COUNT_INVALID, id(7)),
            // 導く属性でないところの *
            (codes::EXPRESS_VALUE_INVALID, id(8)),
        ]
    );
    let first = &findings[0];
    assert_eq!(first.param("kind"), Some(&Value::Text("WALL".to_owned())));
    assert_eq!(first.param("position"), Some(&Value::UInt(2)));
    assert_eq!(
        first.param("attribute"),
        Some(&Value::Text("KIND".to_owned()))
    );
    assert_eq!(
        first.param("expected"),
        Some(&Value::Text("KINDE".to_owned()))
    );
    let count = &findings[14];
    assert_eq!(count.param("expected"), Some(&Value::UInt(6)));
    assert_eq!(count.param("found"), Some(&Value::UInt(3)));
}

#[test]
fn checks_the_parts_of_complex_instances_by_their_own_attributes() {
    let (findings, _) = run("#1=(NAMED(.STANDARD.)SI($,1));\n#2=(NAMED(1)SI($));\n").unwrap();
    assert_eq!(
        found(&findings),
        [
            (
                codes::EXPRESS_ATTRIBUTE_COUNT_INVALID,
                Some("#1".to_owned())
            ),
            (codes::EXPRESS_VALUE_INVALID, Some("#2".to_owned())),
        ]
    );
}

#[test]
fn reports_a_different_schema_name_and_still_checks() {
    let file = with(Limits::default(), |cx| {
        Ifc::read(ifc("IFC4", GOOD).as_bytes(), cx)
    })
    .0
    .unwrap();
    let (findings, diag) = with(Limits::default(), |cx| check(&file, &schema().unwrap(), cx));
    assert!(findings.unwrap().is_empty());
    let mixed = diag
        .iter()
        .find(|d| d.code() == codes::EXPRESS_SCHEMA_MIXED)
        .unwrap();
    assert_eq!(
        mixed.param("file_schema"),
        Some(&Value::Text("IFC4".to_owned()))
    );
}

#[test]
fn rejects_express_it_cannot_read() {
    for broken in [
        EXPRESS.replace("END_SCHEMA;", ""),
        EXPRESS.replace("(* 試験用", "(* (* 試験用"),
        EXPRESS.replace("Height : LengthT;", "Height : MissingT;"),
        EXPRESS.replace("ENTITY Prop;", "ENTITY Prop SUBTYPE OF (Prop);"),
        EXPRESS.replace("TYPE CountT = INTEGER;", "TYPE CountT = CountT;"),
        EXPRESS.replace("Kind : KindE;", "Kind KindE;"),
        "not express".to_owned(),
    ] {
        let (result, diag) = with(Limits::default(), |cx| Express::read(broken.as_bytes(), cx));
        assert!(matches!(result, Err(Error::Invalid)), "{broken}");
        let codes_found: Vec<_> = diag.iter().map(|d| d.code()).collect();
        assert_eq!(codes_found, [codes::EXPRESS_FILE_INVALID]);
    }
}

#[test]
fn budgets_stop_reading_and_checking() {
    let (result, diag) = with(
        Limits::default().with(Kind::Work, Limit::Finite(10)),
        |cx| Express::read(EXPRESS.as_bytes(), cx),
    );
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(
        diag.iter().next().unwrap().code(),
        codes::BUDGET_WORK_EXCEEDED
    );

    let file = file(GOOD).unwrap();
    let schema = schema().unwrap();
    let (result, diag) = with(
        Limits::default().with(Kind::Work, Limit::Finite(10)),
        |cx| check(&file, &schema, cx),
    );
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert!(diag.iter().any(|d| d.code() == codes::BUDGET_WORK_EXCEEDED));
}

/// どこで切っても、どのバイトを変えても、パニックせずに読むか断る
#[test]
fn truncated_and_mutated_express_does_not_panic() {
    let src = EXPRESS.as_bytes();
    let file = file(GOOD).unwrap();
    let try_all = |src: &[u8]| {
        let (schema, _) = with(Limits::default(), |cx| Express::read(src, cx));
        if let Ok(schema) = schema {
            let _ = with(Limits::default(), |cx| check(&file, &schema, cx));
        }
    };
    for end in 0..src.len() {
        try_all(&src[..end]);
    }
    for at in 0..src.len() {
        for byte in *b"(;'*-" {
            let mut changed = src.to_vec();
            changed[at] = byte;
            try_all(&changed);
        }
    }
}
