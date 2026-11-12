//! 利用者が渡す EXPRESS でのチェック（決定 0216）。EXPRESS は、テストの中で書いた小さなものだけを使う。

use std::fmt::Write as _;

use zumen_base::{
    Budget, CancelToken, Code, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, Value, codes,
};
use zumen_ifc::{Express, Findings, Ifc, IfcFile, check, check_range};
use zumen_sdk::{Codec, Error};

const EXPRESS: &str = "(* テスト用の小さなスキーマ (* 入れ子の注釈 *) *)
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

/// EXPRESS を読み、結果・診断・使った work を返す
fn read(src: &str, limits: Limits) -> (Result<Express, Error>, Diagnostics, u64) {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = Express::read(
        src.as_bytes(),
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    );
    (result, diag, budget.used(Kind::Work))
}

/// 定義された型 `T` と、実体 `E` の属性 `A` に、集まりの型をそれぞれの段の数だけ入れ子にした EXPRESS
fn nested(type_levels: usize, attribute_levels: usize) -> String {
    let ty = |levels: usize| format!("{}INTEGER", "LIST [1:?] OF ".repeat(levels));
    format!(
        "SCHEMA IFCDEEP;\nTYPE T = {};\nEND_TYPE;\nENTITY E;\n  A : {};\n  B : T;\nEND_ENTITY;\nEND_SCHEMA;\n",
        ty(type_levels),
        ty(attribute_levels)
    )
}

/// 診断の初めの場所のバイトの範囲
fn bytes(diagnostic: &Diagnostic) -> Option<(u64, u64)> {
    let range = diagnostic.places().first()?.bytes()?;
    Some((range.start, range.end))
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

/// EXPRESS と IFC の DATA を読み、`limits` の予算で `range` の指摘を求めて、結果・診断・チェックで使った work を返す。
/// IFC の `FILE_SCHEMA` は、EXPRESS のスキーマの名前にする
fn check_work(
    express: &str,
    data: &str,
    range: core::ops::Range<u64>,
    limits: Limits,
) -> Result<(Result<Findings, Error>, Diagnostics, u64), Error> {
    let schema = with(Limits::default(), |cx| {
        Express::read(express.as_bytes(), cx)
    })
    .0?;
    let file = with(Limits::default(), |cx| {
        Ifc::read(ifc(schema.name(), data).as_bytes(), cx)
    })
    .0?;
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = check_range(
        &file,
        &schema,
        range,
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    );
    Ok((result, diag, budget.used(Kind::Work)))
}

/// `E{i} SUBTYPE OF (E{i+1})` の鎖（`E0` が鎖の端の、もっとも下の型）。`attribute` があれば、実体ごとに
/// `A{i} : 型` を 1 つ宣言する。`more` は鎖の後ろに足す宣言
fn chain(n: usize, attribute: Option<&str>, more: &str) -> String {
    let mut src = String::from("SCHEMA IFCCHAIN;\n");
    let declare = |i: usize| attribute.map_or(String::new(), |ty| format!("  A{i} : {ty};\n"));
    for i in 0..n {
        let _ = write!(
            src,
            "ENTITY E{i} SUBTYPE OF (E{});\n{}END_ENTITY;\n",
            i + 1,
            declare(i)
        );
    }
    let _ = write!(
        src,
        "ENTITY E{n};\n{}END_ENTITY;\n{more}END_SCHEMA;\n",
        declare(n)
    );
    src
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
        EXPRESS.replace("(* テスト用", "(* (* テスト用"),
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

/// 集まりの型の入れ子は、定義された型でも属性でも、1 段ごとに予算の depth で数え、型を読み終えたら出る（#668）
#[test]
fn nested_aggregates_stop_at_the_depth_budget() {
    let limits = Limits::default().with(Kind::Depth, Limit::Finite(8));
    let (result, diag, _) = read(&nested(8, 8), limits);
    assert!(result.is_ok(), "{diag:?}");
    // 定義された型の 9 段目か、属性の 9 段目の `LIST` を指して止める
    for (src, nth) in [(nested(9, 8), 8), (nested(8, 9), 16)] {
        let (result, diag, _) = read(&src, limits);
        assert!(matches!(result, Err(Error::Stopped(_))));
        let codes_found: Vec<_> = diag.iter().map(|d| d.code()).collect();
        assert_eq!(codes_found, [codes::BUDGET_DEPTH_EXCEEDED]);
        let stop = diag.iter().next().unwrap();
        assert_eq!(stop.param("limit"), Some(&Value::UInt(8)));
        assert_eq!(stop.param("requested"), Some(&Value::UInt(9)));
        let list = src.match_indices("LIST").nth(nth).unwrap().0 as u64;
        assert_eq!(bytes(stop), Some((list, list + 4)));
    }
}

/// 既定の予算の深さを大きく超える入れ子も、自分を呼ばずに読んで止める。既定の深さまでの入れ子は、小さいスタック
/// （Windows のメインのスレッドと wasm の既定の 1 MiB）でも、読み・チェック・写し・比べ・`Debug` で書き・捨てられる（#668）
#[test]
fn deep_nesting_does_not_exhaust_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(|| {
            let (result, diag, _) = read(&nested(100_000, 0), Limits::default());
            assert!(matches!(result, Err(Error::Stopped(_))));
            assert_eq!(
                diag.iter().next().unwrap().code(),
                codes::BUDGET_DEPTH_EXCEEDED
            );

            // 既定の深さ（決定 0010 の仮の値）ちょうどの入れ子。既定を変えても、上限ちょうどを確かめる
            let Limit::Finite(depth) = Limits::default().get(Kind::Depth) else {
                panic!("既定の depth に上限がない");
            };
            let depth = usize::try_from(depth).unwrap();
            let (schema, diag, _) = read(&nested(depth, depth), Limits::default());
            let schema = schema.unwrap_or_else(|e| panic!("{e:?} {diag:?}"));
            let file = with(Limits::default(), |cx| {
                Ifc::read(ifc("IFCDEEP", "#1=E(1,1);\n").as_bytes(), cx)
            })
            .0
            .unwrap();
            let (findings, diag) = with(Limits::default(), |cx| check(&file, &schema, cx));
            let findings = findings.unwrap_or_else(|e| panic!("{e:?} {diag:?}"));
            let expected: Vec<_> = findings.iter().map(|d| d.param("expected")).collect();
            let deep = format!("{}INTEGER", "LIST [1:?] OF ".repeat(depth));
            assert_eq!(
                expected,
                [Some(&Value::Text(deep)), Some(&Value::Text("T".to_owned()))]
            );
            let copy = schema.clone();
            assert!(copy == schema);
            assert!(!format!("{copy:?}").is_empty());
            drop(copy);
            drop(schema);

            // 呼び出し側が上限を引き上げても、読み取った型の各段は同じ配列に並ぶ
            let raised = 4 * depth;
            let limits = Limits::default().with(Kind::Depth, Limit::Finite(raised as u64));
            let (schema, diag, _) = read(&nested(raised, raised), limits);
            let schema = schema.unwrap_or_else(|e| panic!("{e:?} {diag:?}"));
            let copy = schema.clone();
            assert!(schema == copy);
            assert!(!format!("{copy:?}").is_empty());
            drop(copy);
            drop(schema);
        })
        .unwrap()
        .join()
        .unwrap();
}

/// 定義された型の長い鎖は、型ごとに 1 度だけたどり、1 歩ごとに work で数える（#668）
#[test]
fn long_chains_of_defined_types_are_followed_once() {
    let n = 2000;
    let chain = |last: &str| {
        let mut src = String::from("SCHEMA CHAIN;\n");
        for i in 0..n {
            let _ = writeln!(src, "TYPE T{i} = T{};\nEND_TYPE;", i + 1);
        }
        let _ = writeln!(src, "TYPE T{n} = {last};\nEND_TYPE;\nEND_SCHEMA;");
        src
    };
    // 字句はバイトごと。鎖は `T0` から 1 度だけたどる
    let steps = n as u64 + 1;
    let src = chain("INTEGER");
    let (result, diag, work) = read(&src, Limits::default());
    assert!(result.is_ok(), "{diag:?}");
    assert_eq!(work, src.len() as u64 + steps);
    // 鎖をたどる手順も、予算で止める
    let limits = Limits::default().with(Kind::Work, Limit::Finite(src.len() as u64 + steps - 1));
    let (result, diag, _) = read(&src, limits);
    assert!(matches!(result, Err(Error::Stopped(_))));
    let codes_found: Vec<_> = diag.iter().map(|d| d.code()).collect();
    assert_eq!(codes_found, [codes::BUDGET_WORK_EXCEEDED]);
    // 鎖の終わりが始まりに戻れば、循環として断る
    let src = chain("T0");
    let (result, diag, work) = read(&src, Limits::default());
    assert!(matches!(result, Err(Error::Invalid)));
    let invalid = diag.iter().next().unwrap();
    assert_eq!(invalid.code(), codes::EXPRESS_FILE_INVALID);
    assert_eq!(invalid.param("near"), Some(&Value::Text("T0".to_owned())));
    assert_eq!(work, src.len() as u64 + steps + 1);
    // 名前の順とたどる順が食い違う組（`A{i} = Z{i}`）も、型ごとに 1 度だけたどる。たどり終えた型を覚える所は、
    // 覚えた型の全体を作り直さない（作り直すと、work に出ずに手間が 2 乗になる）
    let mut src = String::from("SCHEMA PAIRS;\n");
    for i in 0..n {
        let _ = writeln!(
            src,
            "TYPE A{i} = Z{i};\nEND_TYPE;\nTYPE Z{i} = INTEGER;\nEND_TYPE;"
        );
    }
    src.push_str("END_SCHEMA;\n");
    let (result, diag, work) = read(&src, Limits::default());
    assert!(result.is_ok(), "{diag:?}");
    assert_eq!(work, src.len() as u64 + 2 * n as u64);
}

/// 上位の型の長い鎖も、実体ごとに 1 度入り、上位の型ごとに 1 度たどって、1 歩ごとに work で数える（#668）
#[test]
fn long_chains_of_supertypes_are_followed_once() {
    let n = 2000;
    let chain = |last: &str| {
        let mut src = String::from("SCHEMA CHAIN;\n");
        for i in 0..n {
            let _ = writeln!(src, "ENTITY E{i} SUBTYPE OF (E{});\nEND_ENTITY;", i + 1);
        }
        let _ = writeln!(src, "ENTITY E{n}{last};\nEND_ENTITY;\nEND_SCHEMA;");
        src
    };
    // 実体の n + 1 と、上位の型の n
    let steps = 2 * n as u64 + 1;
    let src = chain("");
    let (result, diag, work) = read(&src, Limits::default());
    assert!(result.is_ok(), "{diag:?}");
    assert_eq!(work, src.len() as u64 + steps);
    let limits = Limits::default().with(Kind::Work, Limit::Finite(src.len() as u64 + steps - 1));
    let (result, diag, _) = read(&src, limits);
    assert!(matches!(result, Err(Error::Stopped(_))));
    let codes_found: Vec<_> = diag.iter().map(|d| d.code()).collect();
    assert_eq!(codes_found, [codes::BUDGET_WORK_EXCEEDED]);
    // 鎖の終わりが始まりに戻れば、循環として断る
    let (result, diag, _) = read(&chain(" SUBTYPE OF (E0)"), Limits::default());
    assert!(matches!(result, Err(Error::Invalid)));
    let invalid = diag.iter().next().unwrap();
    assert_eq!(invalid.param("near"), Some(&Value::Text("E0".to_owned())));
}

/// 上位の型の循環は、循環の中にある実体のうち、名前の順で最初のものを指す。循環に入るだけの実体は指さない
#[test]
fn reports_the_first_entity_on_a_cycle_of_supertypes() {
    let schema = |entities: &str| format!("SCHEMA LOOPS;\n{entities}END_SCHEMA;\n");
    for (src, name) in [
        (schema("ENTITY P SUBTYPE OF (P);\nEND_ENTITY;\n"), "P"),
        (
            schema(
                "ENTITY A SUBTYPE OF (C);\nEND_ENTITY;\nENTITY B SUBTYPE OF (C);\nEND_ENTITY;\n\
                 ENTITY C SUBTYPE OF (B);\nEND_ENTITY;\n",
            ),
            "B",
        ),
        // 先にたどる循環（Y と Z）より、名前の順で前の実体（M）が、後の循環にある
        (
            schema(
                "ENTITY A SUBTYPE OF (Y);\nEND_ENTITY;\nENTITY Y SUBTYPE OF (Z);\nEND_ENTITY;\n\
                 ENTITY Z SUBTYPE OF (Y);\nEND_ENTITY;\nENTITY M SUBTYPE OF (N);\nEND_ENTITY;\n\
                 ENTITY N SUBTYPE OF (M, A);\nEND_ENTITY;\n",
            ),
            "M",
        ),
    ] {
        let (result, diag, _) = read(&src, Limits::default());
        assert!(matches!(result, Err(Error::Invalid)), "{src}");
        let invalid = diag.iter().next().unwrap();
        assert_eq!(invalid.code(), codes::EXPRESS_FILE_INVALID);
        assert_eq!(invalid.param("near"), Some(&Value::Text(name.to_owned())));
    }
}

/// 同じ名前の型・実体を 2 度宣言した EXPRESS は、どちらを使うか決められないので、後の宣言の名前を指して断る（#668）
#[test]
fn rejects_declarations_with_the_same_name() {
    let before_si =
        |declaration: &str| EXPRESS.replace("ENTITY Si;", &format!("{declaration}\nENTITY Si;"));
    // 後の宣言は、元の `ENTITY Si;` の行から始まる
    let line = EXPRESS.lines().position(|l| l == "ENTITY Si;").unwrap() as u64 + 1;
    for (changed, name) in [
        (before_si("TYPE COUNTT = REAL; END_TYPE;"), "COUNTT"),
        (
            before_si("ENTITY Named;\n  Other : LabelT;\nEND_ENTITY;"),
            "NAMED",
        ),
        // 実体と同じ名前の型と、型と同じ名前の実体
        (before_si("TYPE Named = STRING; END_TYPE;"), "NAMED"),
        (before_si("ENTITY LabelT;\nEND_ENTITY;"), "LABELT"),
    ] {
        let (result, diag, _) = read(&changed, Limits::default());
        assert!(matches!(result, Err(Error::Invalid)), "{changed}");
        let codes_found: Vec<_> = diag.iter().map(|d| d.code()).collect();
        assert_eq!(codes_found, [codes::EXPRESS_FILE_INVALID]);
        let invalid = diag.iter().next().unwrap();
        assert_eq!(invalid.param("near"), Some(&Value::Text(name.to_owned())));
        assert_eq!(invalid.param("line"), Some(&Value::UInt(line)));
    }
}

/// 宣言のない名前は、実体の名前の順に、上位の型・属性の型・宣言し直した属性の型の順で見て、最初のものを指し、
/// 実体の後に型を見る。1 つの宣言に並べた名前は型を共有し、続く同じ型は 1 度だけたどる（#668）
#[test]
fn reports_the_first_unresolved_name() {
    let schema = |declarations: &str| format!("SCHEMA MISSING;\n{declarations}END_SCHEMA;\n");
    for (src, name) in [
        (
            schema("ENTITY E;\n  A, B, C : LIST [1:?] OF MissingT;\nEND_ENTITY;\n"),
            "MISSINGT",
        ),
        (
            schema("ENTITY E SUBTYPE OF (MissingE);\n  A, B : MissingT;\nEND_ENTITY;\n"),
            "MISSINGE",
        ),
        // 宣言し直した属性の型は、先に書いても、属性の型の後に見る
        (
            schema(
                "ENTITY P;\n  X : INTEGER;\nEND_ENTITY;\nENTITY Q SUBTYPE OF (P);\n  \
                 SELF\\P.X : MissingX;\n  A : MissingA;\nEND_ENTITY;\n",
            ),
            "MISSINGA",
        ),
        // 実体は、書いた順ではなく名前の順に見る
        (
            schema(
                "ENTITY B;\n  X : MissingB;\nEND_ENTITY;\nENTITY A;\n  Y : MissingA;\nEND_ENTITY;\n",
            ),
            "MISSINGA",
        ),
        (
            schema("TYPE T = MissingT;\nEND_TYPE;\nENTITY E;\n  X : MissingE;\nEND_ENTITY;\n"),
            "MISSINGE",
        ),
    ] {
        let (result, diag, _) = read(&src, Limits::default());
        assert!(matches!(result, Err(Error::Invalid)), "{src}");
        let invalid = diag.iter().next().unwrap();
        assert_eq!(invalid.code(), codes::EXPRESS_FILE_INVALID);
        assert_eq!(invalid.param("line"), Some(&Value::UInt(0)));
        assert_eq!(invalid.param("near"), Some(&Value::Text(name.to_owned())));
    }
}

/// 字句に分けるときは、注釈も含めて、EXPRESS のバイトごとに work で数える（決定 0216、#668）
#[test]
fn counts_each_byte_of_the_express_as_work() {
    let comment = format!("(* {} *)\n", "x".repeat(10_000));
    let commented = format!("{comment}{EXPRESS}");
    let (result, _, plain_work) = read(EXPRESS, Limits::default());
    assert!(result.is_ok());
    let (result, _, commented_work) = read(&commented, Limits::default());
    assert!(result.is_ok());
    assert_eq!(commented_work - plain_work, comment.len() as u64);
    // 注釈の途中までの予算なら、注釈を指して止める
    let limits = Limits::default().with(Kind::Work, Limit::Finite(10_000));
    let (result, diag, _) = read(&commented, limits);
    assert!(matches!(result, Err(Error::Stopped(_))));
    let stop = diag.iter().next().unwrap();
    assert_eq!(stop.code(), codes::BUDGET_WORK_EXCEEDED);
    assert_eq!(bytes(stop), Some((0, comment.len() as u64 - 1)));
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

/// 指摘の一部だけを求めると、範囲の外の指摘は作らずに数え、範囲にある指摘だけを、すべてを求めたときと同じ形で返す
/// （#672）
#[test]
fn check_range_keeps_only_the_findings_in_the_range() {
    let data = "#1=WALL('w',.OTHER.,'tall',#3,(),1);\n#2=POINT('p',(0.));\n\
#3=PROP(LENGTHT('x'),#3,(1.5),\"0\",.T.);\n#4=PROP(COUNTT(1),$,(),\"0\",.T.);\n#5=ROOT($);\n\
#6=GHOST(1);\n#7=WALL('w',.STANDARD.,1.);\n#8=PROP($,#2,(),\"0\",*);\n";
    let (file, schema) = (file(data).unwrap(), schema().unwrap());
    let (all, _) = with(Limits::default(), |cx| check(&file, &schema, cx));
    let all = all.unwrap();
    assert_eq!(all.len(), 16);
    for range in [0..3, 2..5, 15..100, 16..20, 0..0, 5..u64::MAX] {
        let (found, diag) = with(Limits::default(), |cx| {
            check_range(&file, &schema, range.clone(), cx)
        });
        let found = found.unwrap();
        assert_eq!(found.total, 16, "{range:?}");
        let start = usize::try_from(range.start).unwrap().min(all.len());
        let end = usize::try_from(range.end)
            .unwrap_or(usize::MAX)
            .min(all.len());
        assert_eq!(found.findings, all[start..end], "{range:?}");
        // 文脈に足す診断は、範囲によらない
        let codes_found: Vec<_> = diag.iter().map(|d| d.code()).collect();
        assert_eq!(codes_found, [codes::EXPRESS_RULE_UNSUPPORTED]);
    }
}

/// 合わない値が多く、型の書き方が長くても（既定の深さの `ARRAY` で約 13.5 KB）、作る指摘は範囲の数までにし、
/// ほかは数えるだけにする（#672）
#[test]
fn many_findings_with_long_types_keep_only_the_range() {
    let Limit::Finite(depth) = Limits::default().get(Kind::Depth) else {
        panic!("既定の depth に上限がない");
    };
    let depth = usize::try_from(depth).unwrap();
    let level = "ARRAY [18446744073709551615:18446744073709551615] OF ";
    let express = format!(
        "SCHEMA IFCLONG;\nENTITY E;\n  A : {}INTEGER;\nEND_ENTITY;\nEND_SCHEMA;\n",
        level.repeat(depth)
    );
    let m = 4000;
    let mut data = String::new();
    for k in 0..m {
        let _ = writeln!(data, "#{}=E(1);", k + 1);
    }
    let (result, diag, work) = check_work(&express, &data, 0..10, Limits::default()).unwrap();
    let found = result.unwrap_or_else(|e| panic!("{e:?} {diag:?}"));
    assert_eq!(found.total, m);
    assert_eq!(found.findings.len(), 10);
    let tenth = &found.findings[9];
    let Some(Value::Text(expected)) = tenth.param("expected") else {
        panic!("型の書き方がない");
    };
    assert_eq!(expected.len(), level.len() * depth + "INTEGER".len());
    assert_eq!(tenth.places()[0].id(), Some("#10"));
    // インスタンスと値ごとに 1 つと、実体の属性の数と並びを 1 度ずつ
    assert_eq!(work, 2 * m + 1 + 3);
}

/// 上位の型の長い鎖のどの実体にもインスタンスがあっても、属性の数は、実体ごとに 1 度だけ求めて 1 つ数える。
/// インスタンスごとに属性の並びを作らず、写さない。#672 で測った形（n = 2000、値を書かないインスタンス）で、使う work
/// は、インスタンスの数と鎖の実体の数の和になる（#672）
#[test]
fn long_chains_of_supertypes_count_each_entity_once() {
    let n = 2000;
    let express = chain(n, Some("INTEGER"), "");
    // どの実体にも 1 つずつと、鎖の端の実体に n 個
    let mut data = String::new();
    for i in 0..=n {
        let _ = writeln!(data, "#{}=E{i}();", i + 1);
    }
    for k in 0..n {
        let _ = writeln!(data, "#{}=E0();", n + 2 + k);
    }
    let (result, diag, work) = check_work(&express, &data, 0..u64::MAX, Limits::default()).unwrap();
    let found = result.unwrap_or_else(|e| panic!("{e:?} {diag:?}"));
    let (count, instances) = (n as u64, 2 * n as u64 + 1);
    assert_eq!(found.total, instances);
    assert!(
        found
            .findings
            .iter()
            .all(|d| d.code() == codes::EXPRESS_ATTRIBUTE_COUNT_INVALID)
    );
    assert_eq!(
        found.findings[0].param("expected"),
        Some(&Value::UInt(count + 1))
    );
    assert_eq!(found.findings[n].param("expected"), Some(&Value::UInt(1)));
    assert_eq!(work, instances + count + 1);
    // 1 つ少ない予算では止める
    let limits = Limits::default().with(Kind::Work, Limit::Finite(work - 1));
    let (result, diag, _) = check_work(&express, &data, 0..u64::MAX, limits).unwrap();
    assert!(matches!(result, Err(Error::Stopped(_))));
    let codes_found: Vec<_> = diag.iter().map(|d| d.code()).collect();
    assert_eq!(codes_found, [codes::BUDGET_WORK_EXCEEDED]);
}

/// 属性の数の合うインスタンスがあれば、属性の並びは実体ごとに 1 度だけ作り、作る手順（上位の型をたどる行きと帰りの
/// 1 歩・上位の型の並びの名前・属性ごと）を work で数える。並びは、インスタンスごとに作り直さない（#672）
#[test]
fn slots_are_built_once_per_entity() {
    let (n, m) = (2000, 20);
    let express = chain(n, Some("INTEGER"), "");
    let args = vec!["1"; n + 1].join(",");
    let mut data = String::new();
    for k in 0..m {
        let _ = writeln!(data, "#{}=E0({args});", k + 1);
    }
    // 最後のインスタンスは、初めに並ぶ鎖の根の属性だけが合わない
    let _ = writeln!(data, "#{}=E0('x'{});", m + 1, ",1".repeat(n));
    let (result, diag, work) = check_work(&express, &data, 0..u64::MAX, Limits::default()).unwrap();
    let found = result.unwrap_or_else(|e| panic!("{e:?} {diag:?}"));
    assert_eq!(found.total, 1);
    let invalid = &found.findings[0];
    assert_eq!(invalid.param("position"), Some(&Value::UInt(1)));
    assert_eq!(
        invalid.param("attribute"),
        Some(&Value::Text(format!("A{n}")))
    );
    let (n, m) = (n as u64, m as u64 + 1);
    // インスタンスと値ごと、属性の数を求める実体ごと、並びを作る手順（行きと帰りの 1 歩と属性が実体ごとに 3 つ、
    // E0 から E{n-1} までの上位の型の並びの名前が 1 つずつ）
    assert_eq!(work, m * (n + 2) + (n + 1) + 3 * (n + 1) + n);
}

/// 参照の先の実体が、求める実体かその下位の型かは、（実体、求める実体）の組ごとに 1 度だけ上位の型をたどって判定し、
/// たどる 1 歩と上位の型の並びの名前ごとに work で数える。参照ごとに上位の型を集めて写さない（#672）
#[test]
fn subtypes_are_judged_once_per_pair() {
    let (n, m) = (2000, 1000);
    let express = chain(
        n,
        None,
        &format!("ENTITY R;\n  T : E{n};\nEND_ENTITY;\nENTITY Q;\nEND_ENTITY;\n"),
    );
    let mut data = String::from("#1=E0();\n#2=Q();\n");
    for k in 0..m {
        let _ = writeln!(data, "#{}=R(#1);", k + 3);
    }
    // 下位の型でない実体への参照は指摘する
    let _ = writeln!(data, "#{}=R(#2);", m + 3);
    let (result, diag, work) = check_work(&express, &data, 0..u64::MAX, Limits::default()).unwrap();
    let found = result.unwrap_or_else(|e| panic!("{e:?} {diag:?}"));
    assert_eq!(
        self::found(&found.findings),
        [(codes::EXPRESS_VALUE_INVALID, Some(format!("#{}", m + 3)))]
    );
    let (n, m) = (n as u64, m as u64);
    // インスタンスごと、属性の数（E0 の鎖・Q・R）、R の並び、値ごと、E0 から E{n} までと Q を 1 度ずつたどり、
    // E0 から E{n-1} までの上位の型の並びの名前を 1 つずつ見る
    assert_eq!(
        work,
        (m + 3) + (n + 1) + 1 + 1 + 3 + (m + 1) + (n + 1) + 1 + n
    );
}

/// 選択が受けるかは、（選択、実体）と（選択、型）の組ごとに 1 度だけ、入れ子の選択と上位の型をたどって判定し、
/// たどる 1 歩と、選択の項目と上位の型の並びの名前ごとに work で数える。値ごとに選択の項目を集めて写さない（#672）
#[test]
fn selects_are_judged_once_per_pair() {
    let (n, m) = (2000, 1000);
    let mut more = String::new();
    for i in 0..n {
        let _ = writeln!(more, "TYPE S{i} = SELECT (S{});\nEND_TYPE;", i + 1);
    }
    let _ = writeln!(
        more,
        "TYPE S{n} = SELECT (E1, L);\nEND_TYPE;\nTYPE L = STRING;\nEND_TYPE;\n\
         ENTITY R;\n  T : S0;\nEND_ENTITY;"
    );
    let express = chain(1, None, &more);
    let mut data = String::from("#1=E0();\n");
    for k in 0..m {
        let _ = writeln!(data, "#{}=R(#1);\n#{}=R(L('x'));", 2 * k + 2, 2 * k + 3);
    }
    // 選択の型を付けた値は、選択が受けない
    let _ = writeln!(data, "#{}=R(S0('x'));", 2 * m + 2);
    let (result, diag, work) = check_work(&express, &data, 0..u64::MAX, Limits::default()).unwrap();
    let found = result.unwrap_or_else(|e| panic!("{e:?} {diag:?}"));
    assert_eq!(
        self::found(&found.findings),
        [(
            codes::EXPRESS_VALUE_INVALID,
            Some(format!("#{}", 2 * m + 2))
        )]
    );
    let (selects, m) = (n as u64 + 1, m as u64);
    // 組ごとに見る選択の項目の並びの名前（S0 から S{n-1} までの 1 つずつと、S{n} の 2 つ）
    let items = selects + 1;
    // インスタンスごと、属性の数（E0 の鎖と R）、R の並び、値ごとと型を付けた値の中身、（S0、E0）・（S0、L）・
    // （S0、S0）を 1 度ずつ。（S0、E0）では、E0 の上位の型の並びの名前も見る
    assert_eq!(
        work,
        (2 * m + 2)
            + 2
            + 1
            + 3
            + (2 * m + 1)
            + m
            + (selects + 4 + items + 1)
            + (selects + 1 + items)
            + (selects + 2 + items)
    );
}

/// 同じ名前を長く並べた上位の型の並びを、先にその名前をたどる多くの実体から見ても、並びの名前ごとに work で数える。
/// すでにたどった名前を飛ばす所を数えないと、手間は実体か組の数と並びの長さの積になるのに、work は実体か組の数に
/// しか比例しない。属性の数（上位の型が 2 つ以上の実体）・属性の並び・参照の先の判定のどれでも数える（#672）
#[test]
fn long_supertype_lists_are_counted_per_name() {
    let k = 100;
    let express = |l: usize| {
        let mut src = String::from("SCHEMA IFCWIDE;\nENTITY C;\n  A : INTEGER;\nEND_ENTITY;\n");
        let _ = writeln!(
            src,
            "ENTITY B SUBTYPE OF ({});\nEND_ENTITY;",
            vec!["C"; l].join(", ")
        );
        for i in 0..k {
            let _ = writeln!(src, "ENTITY X{i} SUBTYPE OF (C, B, C);\nEND_ENTITY;");
        }
        src.push_str("ENTITY D;\nEND_ENTITY;\nENTITY R;\n  P : D;\nEND_ENTITY;\nEND_SCHEMA;\n");
        src
    };
    // 属性の数の合う X{i} のインスタンスと、それを参照する R（X{i} は D の下位の型でないので合わない）を 1 つずつ
    let mut data = String::new();
    for i in 0..k {
        let _ = writeln!(
            data,
            "#{}=X{i}(1);\n#{}=R(#{});",
            2 * i + 1,
            2 * i + 2,
            2 * i + 1
        );
    }
    let k = k as u64;
    for l in [1000, 2000] {
        let express = express(l);
        let (result, diag, work) =
            check_work(&express, &data, 0..u64::MAX, Limits::default()).unwrap();
        let found = result.unwrap_or_else(|e| panic!("{e:?} {diag:?}"));
        assert_eq!(found.total, k);
        // 探索ごとに見る上位の型の並びの名前（X{i} の 3 つと B の l）
        let names = 3 + l as u64;
        // X{i}: インスタンスと値、属性の数（求める 1 つと、X{i}・C・B・C の 4 歩）、属性の並び（7 歩と属性 1 つ）。
        // R: インスタンスと値、（X{i}、D）の組（4 歩）。R の属性の数と並びは 1 度だけ。3 つの探索で並びの名前を見る
        assert_eq!(
            work,
            k * (2 + (1 + 4 + names) + (8 + names) + 2 + (4 + names)) + 1 + 3
        );
        // 並びを見る手間に見合う予算では止める
        let limits = Limits::default().with(Kind::Work, Limit::Finite(k * l as u64));
        let (result, diag, _) = check_work(&express, &data, 0..u64::MAX, limits).unwrap();
        assert!(matches!(result, Err(Error::Stopped(_))));
        let codes_found: Vec<_> = diag.iter().map(|d| d.code()).collect();
        assert_eq!(codes_found, [codes::BUDGET_WORK_EXCEEDED]);
    }
}

/// 同じ名前を長く並べた選択の項目の並びを、先にその名前をたどる入れ子の選択から、多くの組で見ても、並びの名前ごとに
/// work で数える（#672）
#[test]
fn long_select_lists_are_counted_per_name() {
    let k = 100;
    let express = |l: usize| {
        let mut src = format!(
            "SCHEMA IFCSELECTS;\nTYPE T = STRING;\nEND_TYPE;\nTYPE S1 = SELECT ({});\nEND_TYPE;\n\
             TYPE S0 = SELECT (S1, T);\nEND_TYPE;\nENTITY R;\n  A : S0;\nEND_ENTITY;\n",
            vec!["T"; l].join(", ")
        );
        for i in 0..k {
            let _ = writeln!(src, "ENTITY X{i};\nEND_ENTITY;");
        }
        src.push_str("END_SCHEMA;\n");
        src
    };
    // X{i} のインスタンスと、それを参照する R（選択が受けない実体なので合わない）を 1 つずつ
    let mut data = String::new();
    for i in 0..k {
        let _ = writeln!(
            data,
            "#{}=X{i}();\n#{}=R(#{});",
            2 * i + 1,
            2 * i + 2,
            2 * i + 1
        );
    }
    let k = k as u64;
    for l in [1000, 2000] {
        let express = express(l);
        let (result, diag, work) =
            check_work(&express, &data, 0..u64::MAX, Limits::default()).unwrap();
        let found = result.unwrap_or_else(|e| panic!("{e:?} {diag:?}"));
        assert_eq!(found.total, k);
        // 組ごとに見る選択の項目の並びの名前（S0 の 2 つと S1 の l）
        let names = 2 + l as u64;
        // X{i}: インスタンスと属性の数。R: インスタンスと値、（S0、X{i}）の組（S0・T・S1 の 3 歩と、X{i} の 1 歩）。
        // R の属性の数と並びは 1 度だけ
        assert_eq!(work, k * (2 + 2 + (4 + names)) + 1 + 3);
        // 並びを見る手間に見合う予算では止める
        let limits = Limits::default().with(Kind::Work, Limit::Finite(k * l as u64));
        let (result, diag, _) = check_work(&express, &data, 0..u64::MAX, limits).unwrap();
        assert!(matches!(result, Err(Error::Stopped(_))));
        let codes_found: Vec<_> = diag.iter().map(|d| d.code()).collect();
        assert_eq!(codes_found, [codes::BUDGET_WORK_EXCEEDED]);
    }
}

/// 列挙の値は、項目の数によらず、値ごとに 1 つ数える。項目を比べる回数は work に出ないので、項目を前から比べずに
/// 集合で引くことは、この試験では確かめず、列挙の項目の型（`BTreeSet`）で守る（#672）
#[test]
fn enumeration_values_count_one_per_value() {
    let (items, m) = (50_000, 20_000);
    let names: Vec<String> = (0..items).map(|i| format!("V{i}")).collect();
    let express = format!(
        "SCHEMA IFCENUM;\nTYPE K = ENUMERATION OF ({});\nEND_TYPE;\nENTITY E;\n  A : K;\n\
         END_ENTITY;\nEND_SCHEMA;\n",
        names.join(", ")
    );
    let mut data = String::new();
    for k in 0..m {
        let _ = writeln!(data, "#{}=E(.v{}.);", k + 1, items - 1);
    }
    let _ = writeln!(data, "#{}=E(.W.);", m + 1);
    let (result, diag, work) = check_work(&express, &data, 0..u64::MAX, Limits::default()).unwrap();
    let found = result.unwrap_or_else(|e| panic!("{e:?} {diag:?}"));
    assert_eq!(
        self::found(&found.findings),
        [(codes::EXPRESS_VALUE_INVALID, Some(format!("#{}", m + 1)))]
    );
    let m = m as u64 + 1;
    assert_eq!(work, 2 * m + 1 + 3);
}

/// 部分が 1 つでないインスタンスへの参照は、どれかの部分の実体が合えばよい
#[test]
fn references_to_complex_instances_check_each_part() {
    let data = "#1=(NAMED(.STANDARD.)SI($));\n#2=(NAMED(.STANDARD.)WALL(.STANDARD.,1.,$,$,.T.));\n\
#3=PROP($,#1,(),\"0\",.T.);\n#4=PROP($,#2,(),\"0\",.T.);\n";
    let (findings, _) = run(data).unwrap();
    assert_eq!(
        found(&findings),
        [(codes::EXPRESS_VALUE_INVALID, Some("#3".to_owned()))]
    );
    assert_eq!(
        findings[0].param("attribute"),
        Some(&Value::Text("TARGET".to_owned()))
    );
}

/// 複合のインスタンスの部分は、ほかの部分の実体が導く属性にした属性に `*` を置く
#[test]
fn complex_instances_take_attributes_derived_by_other_parts() {
    let data = "#1=(POINT((0.,0.))ROOT(*));\n#2=(POINT((0.,0.))ROOT('x'));\n";
    let (findings, _) = run(data).unwrap();
    assert_eq!(
        found(&findings),
        [(codes::EXPRESS_VALUE_INVALID, Some("#2".to_owned()))]
    );
    let invalid = &findings[0];
    assert_eq!(invalid.param("kind"), Some(&Value::Text("ROOT".to_owned())));
    assert_eq!(
        invalid.param("attribute"),
        Some(&Value::Text("NAME".to_owned()))
    );
    assert_eq!(
        invalid.param("expected"),
        Some(&Value::Text("*".to_owned()))
    );
}
