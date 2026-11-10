//! IDS の照合に、EXPRESS の属性の名前で L1 の属性を渡す（決定 0217）。EXPRESS・IFC・IDS は、テストの中で書いた
//! 小さなものだけを使う。
#![cfg(feature = "ids")]

use std::fmt::Write as _;

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::Building;
use zumen_ids::{Ids, Report, Status, check};
use zumen_ifc::{Express, IdsLookup, Ifc, IfcFile};
use zumen_sdk::{Codec, Error, Lift};

/// IFC4 の一部を、属性の並びだけ写した小さなスキーマ
const EXPRESS: &str = "SCHEMA IFC4;
TYPE IfcGloballyUniqueId = STRING (22) FIXED; END_TYPE;
TYPE IfcLabel = STRING; END_TYPE;
TYPE IfcText = STRING; END_TYPE;
TYPE IfcIdentifier = STRING; END_TYPE;
TYPE IfcPositiveLengthMeasure = IfcLengthMeasure; END_TYPE;
TYPE IfcLengthMeasure = REAL; END_TYPE;
TYPE IfcBoolean = BOOLEAN; END_TYPE;
TYPE IfcDoorTypeEnum = ENUMERATION OF (DOOR, GATE, USERDEFINED, NOTDEFINED); END_TYPE;
ENTITY IfcRoot ABSTRACT;
  GlobalId : IfcGloballyUniqueId;
  OwnerHistory : OPTIONAL IfcRoot;
  Name : OPTIONAL IfcLabel;
  Description : OPTIONAL IfcText;
END_ENTITY;
ENTITY IfcProject SUBTYPE OF (IfcRoot);
  ObjectType : OPTIONAL IfcLabel;
  LongName : OPTIONAL IfcLabel;
  Phase : OPTIONAL IfcLabel;
  RepresentationContexts : OPTIONAL SET [1:?] OF IfcRoot;
  UnitsInContext : OPTIONAL IfcRoot;
END_ENTITY;
ENTITY IfcDoor SUBTYPE OF (IfcRoot);
  ObjectType : OPTIONAL IfcLabel;
  ObjectPlacement : OPTIONAL IfcRoot;
  Representation : OPTIONAL IfcRoot;
  Tag : OPTIONAL IfcIdentifier;
  OverallHeight : OPTIONAL IfcPositiveLengthMeasure;
  OverallWidth : OPTIONAL IfcPositiveLengthMeasure;
  PredefinedType : OPTIONAL IfcDoorTypeEnum;
  OperationType : OPTIONAL IfcLabel;
  UserDefinedOperationType : OPTIONAL IfcLabel;
END_ENTITY;
END_SCHEMA;
";

const IFC: &str = "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\n\
FILE_NAME('','',(''),(''),'','','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n\
#1=IFCPROJECT('0000000000000000000001',$,'Project',$,$,$,$,$,#10);\n\
#10=IFCUNITASSIGNMENT((#11));\n\
#11=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n\
#80=IFCDOOR('0000000000000000000080',$,'Door',$,$,$,$,'D-1',2100.,$,.DOOR.,$,$);\n\
ENDSEC;\nEND-ISO-10303-21;\n";

fn with<T>(f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics) {
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag)
}

fn inputs() -> Result<(IfcFile, Building, Express), Error> {
    let file = with(|cx| Ifc::read(IFC.as_bytes(), cx)).0?;
    let building = with(|cx| file.lift(cx)).0?;
    let express = with(|cx| Express::read(EXPRESS.as_bytes(), cx)).0?;
    Ok((file, building, express))
}

fn attribute(spec: &str, name: &str, value: Option<&str>) -> String {
    let value = value.map_or(String::new(), |v| format!("<value>{v}</value>"));
    format!(
        "<specification name=\"{spec}\" ifcVersion=\"IFC4\">\
         <applicability maxOccurs=\"unbounded\"><entity><name><simpleValue>IFCDOOR</simpleValue></name></entity></applicability>\
         <requirements><attribute><name><simpleValue>{name}</simpleValue></name>{value}</attribute></requirements>\
         </specification>"
    )
}

fn run(specifications: &str, with_express: bool) -> Result<Report, Error> {
    let (file, building, express) = inputs()?;
    let text = format!(
        "<ids xmlns=\"http://standards.buildingsmart.org/IDS\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\">\
         <info><title>t</title></info><specifications>{specifications}</specifications></ids>"
    );
    let ids = with(|cx| Ids::read(text.as_bytes(), cx)).0?;
    let lookup = IdsLookup::new(&file, with_express.then_some(&express));
    with(|cx| check(&building, &ids, &lookup, cx)).0
}

fn statuses(report: &Report) -> Vec<(&str, Status)> {
    report
        .specifications
        .iter()
        .map(|s| (s.name.as_str(), s.status))
        .collect()
}

fn codes_of(report: &Report) -> Vec<Code> {
    report
        .findings
        .iter()
        .map(|f| f.diagnostic.code())
        .collect()
}

#[test]
fn checks_attributes_by_express_names() {
    let specifications = [
        // 2100 mm は 2.1 m
        attribute("高さ", "OverallHeight", Some("<simpleValue>2.1</simpleValue>")),
        attribute("違う高さ", "OverallHeight", Some("<simpleValue>2100</simpleValue>")),
        attribute("幅", "OverallWidth", None),
        attribute("タグ", "Tag", Some("<simpleValue>D-1</simpleValue>")),
        attribute("種類", "PredefinedType", Some("<simpleValue>DOOR</simpleValue>")),
        attribute("知らない属性", "FireExit", None),
        attribute(
            "式の名前",
            "Overall",
            None,
        )
        .replace(
            "<name><simpleValue>Overall</simpleValue></name>",
            "<name><xs:restriction base=\"xs:string\"><xs:pattern value=\"Overall.*\"/></xs:restriction></name>",
        ),
    ]
    .concat();
    let report = run(&specifications, true).unwrap();
    assert_eq!(
        statuses(&report),
        [
            ("高さ", Status::Passed),
            ("違う高さ", Status::Failed),
            // `$` は値がない
            ("幅", Status::Failed),
            ("タグ", Status::Passed),
            ("種類", Status::Passed),
            ("知らない属性", Status::Failed),
            // 名前の合う属性の 1 つが `$`
            ("式の名前", Status::Failed),
        ]
    );
    assert!(
        codes_of(&report)
            .iter()
            .all(|c| *c == codes::IDS_REQUIREMENT_INVALID)
    );

    // EXPRESS がなければ、コンポーネントにない属性は確かめられない
    let report = run(&specifications, false).unwrap();
    assert_eq!(
        statuses(&report),
        [
            ("高さ", Status::Unknown),
            ("違う高さ", Status::Unknown),
            ("幅", Status::Unknown),
            ("タグ", Status::Passed),
            ("種類", Status::Passed),
            ("知らない属性", Status::Unknown),
            ("式の名前", Status::Unknown),
        ]
    );
}

/// `doors` 個の扉を、扉にない属性を求める `facets` 個の facet で照らし、結果・診断・照合で使った work を返す。`grow` は、
/// 扉の名前の値に足す文字の数、EXPRESS の属性の名前に足す文字の数、扉と `IfcRoot` の間に足す上位の型の数、単位の
/// 割り当てで長さの単位の前に足す面積の単位の数
fn lookups(
    (doors, facets): (u64, u64),
    (value, suffix, chain, units): (usize, usize, usize, usize),
    limits: Limits,
) -> Result<(Result<Report, Error>, Diagnostics, u64), Error> {
    let mut express = String::new();
    for line in EXPRESS.lines() {
        match line
            .strip_prefix("  ")
            .and_then(|attribute| attribute.split_once(" : "))
        {
            Some((name, ty)) => {
                let _ = writeln!(express, "  {name}{} : {ty}", "X".repeat(suffix));
            }
            None => {
                let _ = writeln!(express, "{line}");
            }
        }
    }
    let mut middle = String::new();
    for i in 0..chain {
        let above = if i + 1 < chain {
            format!("Mid{}", i + 1)
        } else {
            "IfcRoot".to_owned()
        };
        let _ = writeln!(middle, "ENTITY Mid{i} SUBTYPE OF ({above});\nEND_ENTITY;");
    }
    let first = if chain > 0 { "Mid0" } else { "IfcRoot" };
    let express = express.replace(
        "ENTITY IfcDoor SUBTYPE OF (IfcRoot);",
        &format!("{middle}ENTITY IfcDoor SUBTYPE OF ({first});"),
    );
    let mut ifc = String::from(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\n\
         FILE_NAME('','',(''),(''),'','','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n\
         #1=IFCPROJECT('0000000000000000000001',$,'Project',$,$,$,$,$,#10);\n",
    );
    let areas: Vec<String> = (0..units).map(|i| format!("#{}", 20 + i)).collect();
    let mut assigned = areas.clone();
    assigned.push("#11".to_owned());
    let _ = writeln!(ifc, "#10=IFCUNITASSIGNMENT(({}));", assigned.join(","));
    let _ = writeln!(ifc, "#11=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);");
    for area in &areas {
        let _ = writeln!(ifc, "{area}=IFCSIUNIT(*,.AREAUNIT.,$,.SQUARE_METRE.);");
    }
    let name = format!("Door{}", "x".repeat(value));
    for id in 100..100 + doors {
        let _ = writeln!(
            ifc,
            "#{id}=IFCDOOR('{id:0>22}',$,'{name}',$,$,$,$,'D-1',2100.,$,.DOOR.,$,$);"
        );
    }
    ifc.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    let mut requirements = String::new();
    for i in 0..facets {
        let _ = write!(
            requirements,
            "<attribute><name><simpleValue>Absent{i}</simpleValue></name></attribute>"
        );
    }
    let text = format!(
        "<ids xmlns=\"http://standards.buildingsmart.org/IDS\"><info><title>t</title></info><specifications>\
         <specification name=\"扉\" ifcVersion=\"IFC4\"><applicability maxOccurs=\"unbounded\">\
         <entity><name><simpleValue>IFCDOOR</simpleValue></name></entity></applicability>\
         <requirements>{requirements}</requirements></specification></specifications></ids>"
    );
    let file = with(|cx| Ifc::read(ifc.as_bytes(), cx)).0?;
    let building: Building = with(|cx| file.lift(cx)).0?;
    let express = with(|cx| Express::read(express.as_bytes(), cx)).0?;
    let ids = with(|cx| Ids::read(text.as_bytes(), cx)).0?;
    let lookup = IdsLookup::new(&file, Some(&express));
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let report = check(
        &building,
        &ids,
        &lookup,
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    );
    Ok((report, diag, budget.used(Kind::Work)))
}

/// 照合は、要素と属性の facet ごとに、要素の属性を EXPRESS の名前で引く。引くたびに、値を読む手間（インスタンスの
/// バイトの数）と、名前を写す手間（属性の名前の長さ）と、量の値の単位を探す手間（前から探した単位の数）を work で
/// 数え、手間に見合う予算で止める。属性の並びを作る手順は、実体ごとに 1 度だけ数える。数え方を変えても、結果は
/// 変わらない（#672）
#[test]
fn attribute_lookups_count_the_values_and_names() {
    let (doors, facets) = (10, 4);
    let run = |grow, limits| lookups((doors, facets), grow, limits).unwrap();
    let (report, diag, base) = run((0, 0, 0, 0), Limits::default());
    let report = report.unwrap_or_else(|e| panic!("{e:?} {diag:?}"));
    // どの扉も、求めた属性を持たない
    assert_eq!(statuses(&report), [("扉", Status::Failed)]);
    assert_eq!(report.findings.len() as u64, doors * facets);
    let calls = doors * facets;
    let (value, suffix, chain, units) = (10_000, 160, 50, 30);
    // 扉の名前の値を長くすると、引くたびに読むインスタンスのバイトが増える
    let (longer, _, work) = run((value, 0, 0, 0), Limits::default());
    assert_eq!(longer.unwrap(), report);
    assert_eq!(work - base, calls * value as u64);
    // 属性の名前を長くすると、引くたびに写す 13 の名前が長くなる。照合は、名前を比べるときに、名前の文字を読む手間
    // （16 バイトで 1）も数える（#732）
    let (renamed, _, work) = run((0, suffix, 0, 0), Limits::default());
    assert_eq!(renamed.unwrap(), report);
    assert_eq!(
        work - base,
        calls * 13 * (suffix as u64 + suffix as u64 / 16)
    );
    // 扉と IfcRoot の間に上位の型を足すと、属性の数を求める歩数と、並びを作る行きと帰りの歩数と、上位の型の並びの
    // 名前が、1 度だけ増える
    let (chained, _, work) = run((0, 0, chain, 0), Limits::default());
    assert_eq!(chained.unwrap(), report);
    assert_eq!(work - base, (1 + 2 + 1) * chain as u64);
    // 長さの単位の前に単位を足すと、扉の高さ（長さの量）を引くたびに、前から探す単位が増える
    let (searched, _, work) = run((0, 0, 0, units), Limits::default());
    assert_eq!(searched.unwrap(), report);
    assert_eq!(work - base, calls * units as u64);
    // 値を読む手間に見合う予算では止める
    let limits = Limits::default().with(Kind::Work, Limit::Finite(calls * value as u64));
    let (stopped, diag, _) = run((value, 0, 0, 0), limits);
    assert!(matches!(stopped, Err(Error::Stopped(_))));
    let codes_found: Vec<_> = diag.iter().map(|d| d.code()).collect();
    assert_eq!(codes_found, [codes::BUDGET_WORK_EXCEEDED]);
}
