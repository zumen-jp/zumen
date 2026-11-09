//! 利用者が渡す EXPRESS でのチェック（決定 0216）。EXPRESS は、テストの中で書いた小さなものだけを使う。

use std::fmt::Write as _;

use zumen_base::{
    Budget, CancelToken, Code, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, Value, codes,
};
use zumen_ifc::{Express, Ifc, IfcFile, check};
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
