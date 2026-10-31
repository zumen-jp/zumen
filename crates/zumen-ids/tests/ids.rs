//! IDS の読み取りと照合（決定 0217）。IDS と建物モデルは、試験の中で書いた小さなものだけを使う。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Limits, Severity, codes};
use zumen_building::{
    Building, Class, Classification, ClassificationReference, Document, ElementRef, Identity,
    Literal, Material, MaterialKind, MaterialPart, Measured, Membership, Opening, Parent,
    ParentKind, Properties, Property, PropertySet, Quantities, Quantity, QuantityKind, QuantitySet,
    TypeOf, TypedValue, Unit,
};
use zumen_ids::{Ids, Lookup, Missing, Report, Status, check};
use zumen_sdk::Error;

fn with<T>(f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics) {
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag)
}

fn ids(specifications: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <ids xmlns=\"http://standards.buildingsmart.org/IDS\" \
         xmlns:xs=\"http://www.w3.org/2001/XMLSchema\">\
         <info><title>試験</title></info><specifications>{specifications}</specifications></ids>"
    )
}

/// 必須の仕様
fn spec(name: &str, applicability: &str, requirements: &str) -> String {
    format!(
        "<specification name=\"{name}\" ifcVersion=\"IFC4\">\
         <applicability maxOccurs=\"unbounded\">{applicability}</applicability>\
         <requirements>{requirements}</requirements></specification>"
    )
}

fn entity(name: &str) -> String {
    format!("<entity><name><simpleValue>{name}</simpleValue></name></entity>")
}

fn simple(text: &str) -> String {
    format!("<simpleValue>{text}</simpleValue>")
}

fn class(name: &str, predefined: Option<&str>) -> Class {
    Class {
        name: name.to_owned(),
        predefined: predefined.map(str::to_owned),
        user_defined: None,
        original: name.to_owned(),
    }
}

fn millimetre() -> Unit {
    Unit {
        kind: "LENGTHUNIT".to_owned(),
        prefix: Some("MILLI".to_owned()),
        name: "METRE".to_owned(),
        scale: None,
    }
}

/// 壁 i1・扉 i2・階 i3・材料 i4・壁の型 i5・開口 i6・敷地 i7
fn building() -> Option<Building> {
    let mut document = Document::default();
    document.header.schemas = vec!["IFC4".to_owned()];
    let mut building = Building::new(document);
    let (refs, _) = with(|cx| {
        (1..=7)
            .map(|n| building.push(None, n, cx))
            .collect::<Option<Vec<ElementRef>>>()
    });
    let [wall, door, storey, concrete, wall_type, opening, site] = refs?.try_into().ok()?;
    let c = &mut building.components;
    c.class.insert(wall, class("IfcWall", Some("SOLIDWALL")));
    c.class.insert(door, class("IfcDoor", None));
    c.class.insert(storey, class("IfcBuildingStorey", None));
    c.class.insert(concrete, class("IfcMaterial", None));
    c.class
        .insert(wall_type, class("IfcWallType", Some("SHEAR")));
    c.class.insert(opening, class("IfcOpeningElement", None));
    c.class.insert(site, class("IfcSite", None));
    c.identity.insert(
        wall,
        Identity {
            name: Some("W1".to_owned()),
            ..Identity::default()
        },
    );
    c.identity.insert(
        concrete,
        Identity {
            name: Some("Concrete".to_owned()),
            ..Identity::default()
        },
    );
    let property = |name: &str, type_name: &str, value: Literal| Property {
        name: name.to_owned(),
        value: Some(TypedValue {
            type_name: Some(type_name.to_owned()),
            value,
        }),
        unit: None,
    };
    c.properties.insert(
        wall,
        Properties {
            sets: vec![PropertySet {
                name: "Pset_WallCommon".to_owned(),
                properties: vec![
                    property("IsExternal", "IfcBoolean", Literal::Boolean(true)),
                    property("FireRating", "IfcLabel", Literal::Text("F30".to_owned())),
                    property("Reference", "IfcIdentifier", Literal::Text(String::new())),
                ],
            }],
        },
    );
    c.properties.insert(
        wall_type,
        Properties {
            sets: vec![PropertySet {
                name: "Pset_WallCommon".to_owned(),
                properties: vec![
                    property("IsExternal", "IfcBoolean", Literal::Boolean(false)),
                    property("LoadBearing", "IfcBoolean", Literal::Boolean(false)),
                ],
            }],
        },
    );
    c.quantities.insert(
        wall,
        Quantities {
            sets: vec![QuantitySet {
                name: "Qto_WallBaseQuantities".to_owned(),
                method: None,
                quantities: vec![Quantity {
                    name: "Length".to_owned(),
                    kind: QuantityKind::Length,
                    value: Measured {
                        value: 2500.0,
                        unit: Some(millimetre()),
                    },
                    formula: None,
                }],
            }],
        },
    );
    c.material.insert(
        wall,
        Material {
            kind: MaterialKind::Single,
            parts: vec![MaterialPart {
                material: Some(concrete),
                name: None,
                thickness: None,
            }],
            direction: None,
            offset: None,
        },
    );
    c.classification.insert(
        wall,
        Classification {
            references: vec![ClassificationReference {
                system: Some("Uniclass".to_owned()),
                edition: None,
                code: Some("EF_25_10".to_owned()),
                name: None,
            }],
        },
    );
    c.parent.insert(
        wall,
        Parent {
            of: Some(Membership {
                target: storey,
                kind: ParentKind::Contained,
                order: None,
            }),
            referenced_in: Vec::new(),
        },
    );
    c.r#type.insert(wall, TypeOf { target: wall_type });
    c.opening.insert(opening, Opening::Voids(wall));
    c.opening.insert(door, Opening::Fills(opening));
    Some(building)
}

fn read(text: &str) -> (Result<Ids, Error>, Diagnostics) {
    with(|cx| Ids::read(text.as_bytes(), cx))
}

fn run(specifications: &str, lookup: Option<&dyn Lookup>) -> Result<Report, Error> {
    let model = building().ok_or(Error::Invalid)?;
    let parsed = read(&ids(specifications)).0?;
    let lookup = lookup.unwrap_or(&model);
    with(|cx| check(&model, &parsed, lookup, cx)).0
}

fn statuses(report: &Report) -> Vec<(&str, Status)> {
    report
        .specifications
        .iter()
        .map(|s| (s.name.as_str(), s.status))
        .collect()
}

fn findings(report: &Report) -> Vec<(Code, Severity, Option<&str>)> {
    report
        .findings
        .iter()
        .map(|f| {
            (
                f.diagnostic.code(),
                f.level,
                f.diagnostic.places().first().and_then(|p| p.id()),
            )
        })
        .collect()
}

/// 分類・材料の名前・単一の値でないプロパティを持たない作り手
struct Lifted;

impl Lookup for Lifted {
    fn missing(&self, _what: Missing) -> bool {
        true
    }
}

#[test]
fn reads_specifications() {
    let (parsed, diag) = read(&ids(&spec("壁", &entity("IFCWALL"), "")));
    let parsed = parsed.unwrap();
    assert_eq!(parsed.title(), Some("試験"));
    assert_eq!(parsed.specifications().len(), 1);
    assert_eq!(parsed.specifications()[0].name(), "壁");
    assert_eq!(parsed.specifications()[0].versions(), ["IFC4"]);
    assert!(diag.is_empty());
}

#[test]
fn entity_and_predefined_type() {
    let typed = |predefined: &str| {
        format!(
            "<entity><name>{}</name><predefinedType>{}</predefinedType></entity>",
            simple("IFCWALL"),
            simple(predefined)
        )
    };
    let report = run(
        &[
            spec("壁", &entity("IFCWALL"), ""),
            // 型の種類が要素の種類に勝つ
            spec("型の種類", &typed("SHEAR"), ""),
            spec("要素の種類", &typed("SOLIDWALL"), ""),
            spec("梁", &entity("IFCBEAM"), ""),
        ]
        .concat(),
        None,
    )
    .unwrap();
    assert_eq!(
        statuses(&report),
        [
            ("壁", Status::Passed),
            ("型の種類", Status::Passed),
            ("要素の種類", Status::Failed),
            ("梁", Status::Failed),
        ]
    );
    assert_eq!(report.specifications[0].applicable, 1);
    assert!(
        findings(&report)
            .iter()
            .all(|f| f.0 == codes::IDS_APPLICABILITY_MISSING && f.1 == Severity::Error)
    );
}

#[test]
fn attribute_facet() {
    let attribute = |name: &str, value: Option<&str>, cardinality: &str| {
        let value = value.map_or(String::new(), |v| format!("<value>{}</value>", simple(v)));
        format!(
            "<attribute cardinality=\"{cardinality}\"><name>{}</name>{value}</attribute>",
            simple(name)
        )
    };
    let wall = entity("IFCWALL");
    let report = run(
        &[
            spec("名前", &wall, &attribute("Name", Some("W1"), "required")),
            spec(
                "違う名前",
                &wall,
                &attribute("Name", Some("W2"), "required"),
            ),
            spec(
                "説明なし",
                &wall,
                &attribute("Description", None, "required"),
            ),
            spec(
                "任意の説明",
                &wall,
                &attribute("Description", None, "optional"),
            ),
            spec("禁じた名前", &wall, &attribute("Name", None, "prohibited")),
            // 成分にない属性は、EXPRESS がなければ確かめられない
            spec("高さ", &wall, &attribute("OverallHeight", None, "required")),
        ]
        .concat(),
        None,
    )
    .unwrap();
    assert_eq!(
        statuses(&report),
        [
            ("名前", Status::Passed),
            ("違う名前", Status::Failed),
            ("説明なし", Status::Failed),
            ("任意の説明", Status::Passed),
            ("禁じた名前", Status::Failed),
            ("高さ", Status::Unknown),
        ]
    );
    assert_eq!(
        findings(&report),
        [
            (codes::IDS_REQUIREMENT_INVALID, Severity::Error, Some("i1")),
            (codes::IDS_REQUIREMENT_INVALID, Severity::Error, Some("i1")),
            (codes::IDS_REQUIREMENT_INVALID, Severity::Error, Some("i1")),
            (
                codes::IDS_REQUIREMENT_UNSUPPORTED,
                Severity::Info,
                Some("i1")
            ),
        ]
    );
}

#[test]
fn property_facet_with_type_and_units() {
    let property = |set: &str, name: &str, data_type: &str, value: Option<&str>| {
        let value = value.map_or(String::new(), |v| format!("<value>{v}</value>"));
        format!(
            "<property dataType=\"{data_type}\"><propertySet>{}</propertySet>\
             <baseName>{}</baseName>{value}</property>",
            simple(set),
            simple(name)
        )
    };
    let wall = entity("IFCWALL");
    let report = run(
        &[
            // 要素の値が型の値に代わる
            spec("外壁", &wall, &property("Pset_WallCommon", "IsExternal", "IFCBOOLEAN", Some(&simple("true")))),
            // 型から受け継ぐ
            spec("耐力", &wall, &property("Pset_WallCommon", "LoadBearing", "IFCBOOLEAN", Some(&simple("false")))),
            spec("型の違い", &wall, &property("Pset_WallCommon", "FireRating", "IFCTEXT", None)),
            spec("ないもの", &wall, &property("Pset_WallCommon", "Combustible", "IFCBOOLEAN", None)),
            // 空の文字は値がないのでなく、合わない
            spec("空の参照", &wall, &property("Pset_WallCommon", "Reference", "IFCIDENTIFIER", None)),
            // 2500 mm は 2.5 m
            spec("長さ", &wall, &property("Qto_WallBaseQuantities", "Length", "IFCLENGTHMEASURE", Some(&simple("2.5")))),
            spec("長さの範囲", &wall, &property("Qto_WallBaseQuantities", "Length", "IFCLENGTHMEASURE",
                Some("<xs:restriction base=\"xs:double\"><xs:minInclusive value=\"2\"/><xs:maxExclusive value=\"2.5\"/></xs:restriction>"))),
            spec("耐火の式", &wall, &property("Pset_WallCommon", "FireRating", "IFCLABEL",
                Some("<xs:restriction base=\"xs:string\"><xs:pattern value=\"F[0-9]+\"/></xs:restriction>"))),
        ]
        .concat(),
        None,
    )
    .unwrap();
    assert_eq!(
        statuses(&report),
        [
            ("外壁", Status::Passed),
            ("耐力", Status::Passed),
            ("型の違い", Status::Failed),
            ("ないもの", Status::Failed),
            ("空の参照", Status::Failed),
            ("長さ", Status::Passed),
            ("長さの範囲", Status::Failed),
            ("耐火の式", Status::Passed),
        ]
    );
    // 単一の値でないプロパティを持たない作り手では、ないプロパティは確かめられない
    let report = run(
        &spec(
            "ないもの",
            &wall,
            &property("Pset_WallCommon", "Combustible", "IFCBOOLEAN", None),
        ),
        Some(&Lifted),
    )
    .unwrap();
    assert_eq!(statuses(&report), [("ないもの", Status::Unknown)]);
}

#[test]
fn material_classification_and_part_of() {
    let wall = entity("IFCWALL");
    let material = |value: &str| format!("<material><value>{}</value></material>", simple(value));
    let classification = |system: &str, value: &str| {
        format!(
            "<classification><value>{}</value><system>{}</system></classification>",
            simple(value),
            simple(system)
        )
    };
    let part_of = |entity_name: &str, relation: &str| {
        format!(
            "<partOf relation=\"{relation}\">{}</partOf>",
            entity(entity_name)
        )
    };
    let report = run(
        &[
            spec("コンクリート", &wall, &material("Concrete")),
            spec("鋼", &wall, &material("Steel")),
            spec("分類", &wall, &classification("Uniclass", "EF_25_10")),
            spec(
                "ほかの体系",
                &wall,
                &classification("OmniClass", "EF_25_10"),
            ),
            spec(
                "階",
                &wall,
                &part_of("IFCBUILDINGSTOREY", "IFCRELCONTAINEDINSPATIALSTRUCTURE"),
            ),
            spec(
                "敷地",
                &wall,
                &part_of("IFCSITE", "IFCRELCONTAINEDINSPATIALSTRUCTURE"),
            ),
            // 扉は開口を埋め、開口は壁にあく
            spec(
                "扉の壁",
                &entity("IFCDOOR"),
                &part_of("IFCWALL", "IFCRELVOIDSELEMENT IFCRELFILLSELEMENT"),
            ),
            spec(
                "扉の階",
                &entity("IFCDOOR"),
                &part_of("IFCWALL", "IFCRELAGGREGATES"),
            ),
        ]
        .concat(),
        None,
    )
    .unwrap();
    assert_eq!(
        statuses(&report),
        [
            ("コンクリート", Status::Passed),
            ("鋼", Status::Failed),
            ("分類", Status::Passed),
            ("ほかの体系", Status::Failed),
            ("階", Status::Passed),
            ("敷地", Status::Failed),
            ("扉の壁", Status::Passed),
            ("扉の階", Status::Failed),
        ]
    );
    // 分類と材料の名前を持たない作り手では、見つからなければ確かめられない
    let report = run(
        &[
            spec("鋼", &wall, &material("Steel")),
            spec(
                "ほかの体系",
                &wall,
                &classification("OmniClass", "EF_25_10"),
            ),
        ]
        .concat(),
        Some(&Lifted),
    )
    .unwrap();
    assert_eq!(
        statuses(&report),
        [("鋼", Status::Unknown), ("ほかの体系", Status::Unknown)]
    );
}

#[test]
fn specification_occurrence() {
    let occurs = |name: &str, min: u32, max: &str, applicability: &str| {
        format!(
            "<specification name=\"{name}\" ifcVersion=\"IFC4\">\
             <applicability minOccurs=\"{min}\" maxOccurs=\"{max}\">{applicability}</applicability>\
             </specification>"
        )
    };
    let report = run(
        &[
            occurs("任意の梁", 0, "unbounded", &entity("IFCBEAM")),
            occurs("禁じた梁", 0, "0", &entity("IFCBEAM")),
            occurs("禁じた壁", 0, "0", &entity("IFCWALL")),
        ]
        .concat(),
        None,
    )
    .unwrap();
    assert_eq!(
        statuses(&report),
        [
            ("任意の梁", Status::Passed),
            ("禁じた梁", Status::Passed),
            ("禁じた壁", Status::Failed),
        ]
    );
    assert_eq!(
        findings(&report),
        [(codes::IDS_APPLICABILITY_EXISTS, Severity::Error, Some("i1"))]
    );
}

#[test]
fn optional_requirement_fails_as_warning() {
    let report = run(
        &spec(
            "任意の名前",
            &entity("IFCWALL"),
            &format!(
                "<attribute cardinality=\"optional\"><name>{}</name><value>{}</value></attribute>",
                simple("Name"),
                simple("W2")
            ),
        ),
        None,
    )
    .unwrap();
    assert_eq!(
        findings(&report),
        [(
            codes::IDS_REQUIREMENT_INVALID,
            Severity::Warning,
            Some("i1")
        )]
    );
}

#[test]
fn applicability_that_cannot_be_checked() {
    let report = run(
        &spec(
            "分類した壁",
            &[
                entity("IFCWALL"),
                "<classification><system><simpleValue>X</simpleValue></system></classification>"
                    .to_owned(),
            ]
            .concat(),
            "",
        ),
        Some(&Lifted),
    )
    .unwrap();
    assert_eq!(statuses(&report), [("分類した壁", Status::Unknown)]);
    assert_eq!(
        findings(&report),
        [(
            codes::IDS_APPLICABILITY_UNSUPPORTED,
            Severity::Info,
            Some("i1")
        )]
    );
}

#[test]
fn rejects_unreadable_ids() {
    for text in [
        // IDS の名前空間でない
        "<ids xmlns=\"urn:other\"><specifications/></ids>".to_owned(),
        // 仕様の名前がない
        ids("<specification ifcVersion=\"IFC4\"><applicability/></specification>"),
        // 値の決まりに型がない
        ids(&spec(
            "型なし",
            &entity("IFCWALL"),
            "<property><propertySet><simpleValue>P</simpleValue></propertySet>\
             <baseName><simpleValue>B</simpleValue></baseName>\
             <value><simpleValue>1</simpleValue></value></property>",
        )),
        // 禁じる仕様に要件がある
        ids(&format!(
            "<specification name=\"x\" ifcVersion=\"IFC4\"><applicability minOccurs=\"0\" maxOccurs=\"0\">{}</applicability>\
             <requirements>{}</requirements></specification>",
            entity("IFCWALL"),
            entity("IFCWALL")
        )),
    ] {
        let (result, diag) = read(&text);
        assert_eq!(result, Err(Error::Invalid), "{text}");
        assert!(
            diag.iter().any(|d| d.code() == codes::IDS_FILE_INVALID),
            "{text}"
        );
    }
}

#[test]
fn reports_unknown_facets_and_restrictions() {
    let text = ids(&spec(
        "知らないもの",
        &entity("IFCWALL"),
        &[
            "<color><value><simpleValue>red</simpleValue></value></color>",
            "<attribute><name><simpleValue>Name</simpleValue></name><value>\
             <xs:restriction base=\"xs:string\"><xs:annotation/><xs:totalDigits value=\"2\"/></xs:restriction>\
             </value></attribute>",
        ]
        .concat(),
    ));
    let (parsed, diag) = read(&text);
    assert!(parsed.is_ok());
    let found: Vec<_> = diag.iter().map(|d| d.code()).collect();
    assert_eq!(
        found,
        [
            codes::IDS_FACET_UNSUPPORTED,
            codes::IDS_RESTRICTION_UNSUPPORTED
        ]
    );
    let report = run(
        &spec(
            "知らないもの",
            &entity("IFCWALL"),
            "<color><value><simpleValue>red</simpleValue></value></color>",
        ),
        None,
    )
    .unwrap();
    assert_eq!(statuses(&report), [("知らないもの", Status::Unknown)]);
}

#[test]
fn stops_on_budget() {
    let model = building().unwrap();
    let parsed = read(&ids(&spec("壁", &entity("IFCWALL"), ""))).0.unwrap();
    let mut budget =
        Budget::new(Limits::default().with(zumen_base::Kind::Work, zumen_base::Limit::Finite(1)));
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = check(
        &model,
        &parsed,
        &model,
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    );
    assert!(matches!(result, Err(Error::Stopped(_))));
}
