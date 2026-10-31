//! IDS の照合に、EXPRESS の属性の名前で L1 の属性を渡す（決定 0217）。EXPRESS・IFC・IDS は、試験の中で書いた
//! 小さなものだけを使う。
#![cfg(feature = "ids")]

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Limits, codes};
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

    // EXPRESS がなければ、成分にない属性は確かめられない
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
