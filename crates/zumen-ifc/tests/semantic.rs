//! 階・区域・分類の、3 つの版の最小の IFC。入力はすべてテストの中で作る。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{Building, ElementRef, Id};
use zumen_ifc::Ifc;
use zumen_sdk::{Codec, Error, Lift};

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];

fn gid(n: u32) -> String {
    format!("{n:022}")
}

fn source(schema: &str, extra: &str) -> String {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_SCHEMA(('{schema}'));\nENDSEC;\nDATA;\n\
        #1=IFCPROJECT('{}',$,'Project',$,$,$,$,$,#2);\n\
        #2=IFCUNITASSIGNMENT((#3));\n\
        #3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n\
        #10=IFCBUILDING('{}',$,'Building',$,$,$,$,$,.ELEMENT.,100.,200.,$);\n\
        #11=IFCBUILDINGSTOREY('{}',$,'1FL',$,$,$,$,$,.ELEMENT.,3000.);\n\
        #12=IFCRELAGGREGATES('{}',$,$,$,#10,(#11));\n\
        #13=IFCSITE('{}',$,$,$,$,$,$,$,.ELEMENT.,$,$,123.,$,$);\n\
        #20=IFCSPACE('{}',$,'Room',$,$,$,$,$,.ELEMENT.,{},$);\n\
        #21=IFCZONE('{}',$,'Zone',$,${});\n\
        #22=IFCRELASSIGNSTOGROUP('{}',$,$,$,(#20),$,#21);\n\
        #30=IFCCLASSIFICATION('Source','2026',$,'Test'{});\n\
        #31=IFCCLASSIFICATIONREFERENCE($,'A-1','Item',#30{});\n\
        #32=IFCRELASSOCIATESCLASSIFICATION('{}',$,$,$,(#11,#20),#31);\n\
        {extra}\nENDSEC;\nEND-ISO-10303-21;\n",
        gid(1),
        gid(10),
        gid(11),
        gid(12),
        gid(13),
        gid(20),
        if schema == "IFC2X3" {
            ".INTERNAL."
        } else {
            ".SPACE."
        },
        gid(21),
        if schema == "IFC2X3" { "" } else { ",$" },
        gid(22),
        if schema == "IFC2X3" { "" } else { ",$,$,$" },
        if schema == "IFC2X3" { "" } else { ",$,$" },
        gid(32)
    )
}

fn lift(src: &str, limits: Limits) -> (Result<Building, Error>, Diagnostics, u64) {
    let cancel = CancelToken::new();
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let file = Ifc::read(
        src.as_bytes(),
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    )
    .unwrap();
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let result = file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag, budget.used(Kind::DecodedBytes))
}

fn element(b: &Building, n: u32) -> ElementRef {
    b.find(Id::Global(
        zumen_building::GlobalId::parse(&gid(n)).unwrap(),
    ))
    .unwrap()
}

fn has(diag: &Diagnostics, code: Code) -> bool {
    diag.iter().any(|d| d.code() == code)
}

#[test]
fn levels_zones_and_classifications_keep_values_and_sources_in_every_version() {
    for schema in VERSIONS {
        let src = source(schema, "");
        let (b, diag, _) = lift(&src, Limits::default());
        assert!(diag.is_empty(), "{schema}: {diag:?}");
        let b = b.unwrap();
        let (storey, room, zone) = (element(&b, 11), element(&b, 20), element(&b, 21));
        let level = b.components.level.get(storey).unwrap();
        assert_eq!(level.name.as_deref(), Some("1FL"));
        // 語彙のプロファイルがないので、名前から種類を推定しない。
        assert_eq!(level.kind, None);
        assert_eq!(level.height.value, 3000.);
        assert_eq!(
            level.height.unit.as_ref().unwrap().prefix.as_deref(),
            Some("MILLI")
        );
        assert_eq!(level.datum.as_deref(), Some(gid(10).as_str()));
        assert_eq!(
            b.components
                .spatial
                .get(element(&b, 10))
                .unwrap()
                .elevation
                .as_ref()
                .unwrap()
                .value,
            100.
        );
        assert!(b.components.level.get(element(&b, 10)).is_none());
        assert!(b.components.level.get(element(&b, 13)).is_none());
        let z = b.components.zone.get(zone).unwrap();
        assert_eq!(z.kind, None);
        assert_eq!(z.members, [room]);
        let s = b.element(zone).unwrap().source();
        assert_eq!(s.instance, Some(21));
        assert_eq!(
            (s.relations[0].instance, s.relations[0].position),
            (22, Some(0))
        );
        for (e, position) in [(storey, 0), (room, 1)] {
            let c = &b.components.classification.get(e).unwrap().references[0];
            assert_eq!(
                (
                    c.system.as_deref(),
                    c.edition.as_deref(),
                    c.code.as_deref(),
                    c.name.as_deref()
                ),
                (Some("Test"), Some("2026"), Some("A-1"), Some("Item"))
            );
            let s = b.element(e).unwrap().source();
            let r = s.relations.iter().find(|r| r.instance == 32).unwrap();
            assert_eq!(
                (r.global_id.as_deref(), r.position),
                (Some(gid(32).as_str()), Some(position))
            );
        }
        let (again, _, _) = lift(&src, Limits::default());
        assert_eq!(b, again.unwrap());
    }
}

#[test]
fn spatial_zones_and_nested_zones_keep_direct_members() {
    for schema in ["IFC4", "IFC4X3_ADD2"] {
        let extra = format!(
            "#40=IFCSPATIALZONE('{}',$,'Spatial zone',$,$,$,$,$,.FIRESAFETY.);\n\
            #41=IFCRELREFERENCEDINSPATIALSTRUCTURE('{}',$,$,$,(#20),#40);\n\
            #42=IFCRELASSIGNSTOGROUP('{}',$,$,$,(#40),$,#21);",
            gid(40),
            gid(41),
            gid(42)
        );
        let (b, diag, _) = lift(&source(schema, &extra), Limits::default());
        assert!(diag.is_empty(), "{diag:?}");
        let b = b.unwrap();
        let z = element(&b, 40);
        assert_eq!(b.components.zone.get(z).unwrap().members, [element(&b, 20)]);
        assert_eq!(
            b.components.zone.get(element(&b, 21)).unwrap().members,
            [element(&b, 20), z]
        );
        assert_eq!(b.element(z).unwrap().source().relations[0].instance, 41);
        // IFC の定義された種類は class に保ち、プロファイルの zone.kind と混ぜない。
        assert_eq!(
            b.components.class.get(z).unwrap().predefined.as_deref(),
            Some("FIRESAFETY")
        );
        assert_eq!(b.components.zone.get(z).unwrap().kind, None);
    }
}

#[test]
fn optional_baseline_and_lightweight_classification_stay_absent() {
    for schema in VERSIONS {
        let src = source(schema, "")
            .replace(".ELEMENT.,100.,200.,$)", ".ELEMENT.,$,200.,$)")
            .replace("'Item',#30", "'Item',$");
        let (b, _, _) = lift(&src, Limits::default());
        let b = b.unwrap();
        assert_eq!(b.components.level.get(element(&b, 11)).unwrap().datum, None);
        let c = &b
            .components
            .classification
            .get(element(&b, 11))
            .unwrap()
            .references[0];
        assert_eq!((c.system.as_deref(), c.edition.as_deref()), (None, None));
        assert_eq!(c.code.as_deref(), Some("A-1"));
    }
}

#[test]
fn classification_hierarchies_and_system_assignments_follow_the_version() {
    for schema in ["IFC4", "IFC4X3_ADD2"] {
        let extra = format!(
            "#40=IFCCLASSIFICATIONREFERENCE($,'A','Parent',#30,$,$);\n\
            #41=IFCRELASSOCIATESCLASSIFICATION('{}',$,$,$,(#11),#30);",
            gid(41)
        );
        let src = source(schema, &extra).replace("'Item',#30", "'Item',#40");
        let (b, diag, _) = lift(&src, Limits::default());
        assert!(diag.is_empty(), "{diag:?}");
        let b = b.unwrap();
        let refs = &b
            .components
            .classification
            .get(element(&b, 11))
            .unwrap()
            .references;
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].code.as_deref(), Some("A-1"));
        assert_eq!(refs[0].system.as_deref(), Some("Test"));
        assert_eq!(refs[1].code, None);
        assert_eq!(refs[1].system.as_deref(), Some("Test"));
    }
}

#[test]
fn missing_heights_and_broken_classification_references_are_diagnosed() {
    for schema in VERSIONS {
        for target in ["#999", "#20", "#31"] {
            let src = source(schema, "")
                .replace(".ELEMENT.,3000.)", ".ELEMENT.,$)")
                .replace("'Item',#30", &format!("'Item',{target}"));
            let (b, diag, _) = lift(&src, Limits::default());
            let b = b.unwrap();
            assert!(b.components.level.is_empty());
            assert!(b.components.classification.is_empty());
            assert!(
                has(&diag, codes::IFC_ATTRIBUTE_INVALID),
                "{schema}: {diag:?}"
            );
            assert!(diag.iter().any(|d| d.code() == codes::IFC_ATTRIBUTE_INVALID
                && d.param("position") == Some(&zumen_base::Value::UInt(10))));
        }
    }
}

#[test]
fn classification_depth_and_copies_use_budgets() {
    let extra = "#40=IFCCLASSIFICATIONREFERENCE($,'A','Parent',#30,$,$);";
    let src = source("IFC4", extra).replace("'Item',#30", "'Item',#40");
    let (b, _, bytes) = lift(&src, Limits::default());
    assert!(b.is_ok());
    let (b, diag, _) = lift(&src, Limits::default().with(Kind::Depth, Limit::Finite(2)));
    assert!(b.is_err());
    assert!(has(&diag, codes::BUDGET_DEPTH_EXCEEDED));
    let (b, diag, _) = lift(
        &src,
        Limits::default().with(Kind::DecodedBytes, Limit::Finite(bytes - 1)),
    );
    assert!(b.is_err());
    assert!(has(&diag, codes::BUDGET_DECODED_BYTES_EXCEEDED));
    let (b, _, _) = lift(
        &src,
        Limits::default().with(Kind::DecodedBytes, Limit::Finite(bytes)),
    );
    assert!(b.is_ok());
}

#[test]
fn shared_classification_copies_are_charged_for_each_element() {
    let src = |count: u32| {
        let objects: String = (100..100 + count)
            .map(|n| format!("#{}=IFCZONE('{}',$,$,$,$,$);\n", n, gid(n)))
            .collect();
        let related = (100..100 + count)
            .map(|n| format!("#{n}"))
            .collect::<Vec<_>>()
            .join(",");
        source("IFC4", &objects)
            .replace("(#11,#20),#31", &format!("({related}),#31"))
            .replace("'A-1'", &format!("'{}'", "A".repeat(2048)))
    };
    let (_, _, few) = lift(&src(2), Limits::default());
    let (many, _, bytes) = lift(&src(202), Limits::default());
    assert!(many.is_ok());
    assert!(bytes - few >= 200 * 2048);
    let (result, diag, _) = lift(
        &src(202),
        Limits::default().with(Kind::DecodedBytes, Limit::Finite(few + 20 * 2048)),
    );
    assert!(result.is_err());
    assert!(has(&diag, codes::BUDGET_DECODED_BYTES_EXCEEDED));
}

#[test]
fn incorrectly_typed_classification_text_is_not_silently_dropped() {
    for schema in VERSIONS {
        for (from, to) in [("'2026'", "2026"), ("'A-1'", "42"), ("'Item'", ".ITEM.")] {
            let (b, diag, _) = lift(&source(schema, "").replace(from, to), Limits::default());
            assert!(b.unwrap().components.classification.is_empty());
            assert!(has(&diag, codes::IFC_ATTRIBUTE_INVALID));
        }
    }
}

#[test]
fn related_objects_keep_original_positions_and_diagnose_invalid_values() {
    for schema in VERSIONS {
        let src = source(schema, "")
            .replace("(#20),$,#21", "(7,#20),$,#21")
            .replace("(#11,#20),#31", "(7,#11,.BAD.,#20),#31");
        let (b, diag, _) = lift(&src, Limits::default());
        let b = b.unwrap();
        assert_eq!(
            b.components.zone.get(element(&b, 21)).unwrap().members,
            [element(&b, 20)]
        );
        for (id, relation, position) in [(21, 22, 1), (20, 22, 1), (11, 32, 1), (20, 32, 3)] {
            let source = b.element(element(&b, id)).unwrap().source();
            assert_eq!(
                source
                    .relations
                    .iter()
                    .find(|r| r.instance == relation)
                    .unwrap()
                    .position,
                Some(position)
            );
        }
        assert_eq!(
            diag.iter()
                .filter(|d| d.code() == codes::IFC_ATTRIBUTE_INVALID)
                .map(|d| d.count())
                .sum::<u64>(),
            3
        );
        for invalid in ["$", "42", "()", "(42)", "#20", "(#999)"] {
            let src = source(schema, "")
                .replace("(#20),$,#21", &format!("{invalid},$,#21"))
                .replace("(#11,#20),#31", &format!("{invalid},#31"));
            let (b, diag, _) = lift(&src, Limits::default());
            let b = b.unwrap();
            assert!(
                b.components.zone.get(element(&b, 21)).is_none(),
                "{schema}: {invalid}"
            );
            assert!(
                b.components.classification.is_empty(),
                "{schema}: {invalid}"
            );
            assert!(has(&diag, codes::IFC_ATTRIBUTE_INVALID));
        }
    }
}

#[cfg(feature = "ids")]
#[test]
fn ids_checks_all_classifications_of_a_type_and_respects_instance_overrides() {
    use zumen_ids::{Ids, Status, check};
    use zumen_ifc::IdsLookup;

    for schema in VERSIONS {
        for own in [false, true] {
            let later = |value: &str| if schema == "IFC2X3" { "" } else { value }.to_owned();
            let mut extra = format!(
                "#40=IFCWALLTYPE('{}',$,'Type',$,$,$,$,$,$,.STANDARD.);\n\
                #41=IFCRELDEFINESBYTYPE('{}',$,$,$,(#50),#40);\n\
                #43=IFCCLASSIFICATIONREFERENCE($,'B-2','Second',#30{});\n\
                #44=IFCRELASSOCIATESCLASSIFICATION('{}',$,$,$,(#40),#31);\n\
                #45=IFCRELASSOCIATESCLASSIFICATION('{}',$,$,$,(#40),#43);\n\
                #50=IFCWALL('{}',$,'Wall',$,$,$,$,${});\n\
                #60=IFCCLASSIFICATION('Source','2026',$,'Other'{});\n\
                #61=IFCCLASSIFICATIONREFERENCE($,'C-3','Other item',#60{});\n\
                #62=IFCRELASSOCIATESCLASSIFICATION('{}',$,$,$,(#40),#61);\n",
                gid(40),
                gid(41),
                later(",$,$"),
                gid(44),
                gid(45),
                gid(50),
                later(",.STANDARD."),
                later(",$,$,$"),
                later(",$,$"),
                gid(62)
            );
            if own {
                extra.push_str(&format!(
                    "#46=IFCCLASSIFICATIONREFERENCE($,'Own','Own item',#30{});\n\
                    #47=IFCRELASSOCIATESCLASSIFICATION('{}',$,$,$,(#50),#46);",
                    later(",$,$"),
                    gid(47)
                ));
            }
            let mut specs = vec![
                ("first", "A-1", "Test"),
                ("second", "B-2", "Test"),
                ("other", "C-3", "Other"),
            ];
            if own {
                specs.push(("own", "Own", "Test"));
            }
            let specs:String=specs.iter().map(|(name,value,system)|format!(
                "<specification name=\"{name}\" ifcVersion=\"{schema}\"><applicability maxOccurs=\"unbounded\"><entity><name><simpleValue>IFCWALL</simpleValue></name></entity></applicability><requirements><classification><value><simpleValue>{value}</simpleValue></value><system><simpleValue>{system}</simpleValue></system></classification></requirements></specification>"
            )).collect();
            let xml = format!(
                "<ids xmlns=\"http://standards.buildingsmart.org/IDS\"><info><title>type classifications</title></info><specifications>{specs}</specifications></ids>"
            );
            let cancel = CancelToken::new();
            let mut budget = Budget::new(Limits::default());
            let mut diag = Diagnostics::new();
            let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
            let file = Ifc::read(source(schema, &extra).as_bytes(), &mut cx).unwrap();
            let building: Building = file.lift(&mut cx).unwrap();
            let ids = Ids::read(xml.as_bytes(), &mut cx).unwrap();
            let report = check(&building, &ids, &IdsLookup::new(&file, None), &mut cx).unwrap();
            let statuses: Vec<_> = report.specifications.iter().map(|s| s.status).collect();
            assert_eq!(
                statuses,
                if own {
                    vec![
                        Status::Unknown,
                        Status::Unknown,
                        Status::Passed,
                        Status::Passed,
                    ]
                } else {
                    vec![Status::Passed; 3]
                },
                "{schema}"
            );
        }
    }
}

fn resource_source(schema: &str, count: u32, on_type: bool, omitted: bool) -> String {
    let mut extra = format!(
        "#101=IFCPROPERTYSET('{}',$,'Set',$,(#102));\n\
        #102=IFCPROPERTYSINGLEVALUE('Value',$,IFCLABEL('Text'),$);\n",
        gid(101)
    );
    if on_type {
        extra.push_str(&format!(
            "#110=IFCWALLTYPE('{}',$,'Type',$,$,(#101),$,$,$,.STANDARD.);\n",
            gid(110)
        ));
    } else {
        extra.push_str(&format!(
            "#103=IFCRELDEFINESBYPROPERTIES('{}',$,$,$,(#11),#101);\n",
            gid(103)
        ));
    }
    let mut related = vec!["#11".to_owned()];
    if omitted {
        extra.push_str(&format!(
            "#100=IFCDISTRIBUTIONFLOWELEMENT('{}',$,'Element',$,$,$,$,$);\n",
            gid(100)
        ));
        related.push("#100".to_owned());
    }
    related.extend((0..count).map(|_| "#101".to_owned()));
    source(schema, &extra).replace("(#11,#20),#31", &format!("({}),#31", related.join(",")))
}

#[test]
fn unmapped_relation_targets_are_reported_once_with_original_positions() {
    for schema in VERSIONS {
        for on_type in [false, true] {
            let src = resource_source(schema, 1, on_type, true);
            let (b, diag, _) = lift(&src, Limits::default());
            let b = b.unwrap();
            assert!(!has(&diag, codes::IFC_ATTRIBUTE_INVALID));
            let warnings: Vec<_> = diag
                .iter()
                .filter(|d| d.code() == codes::IFC_RELATION_TARGET_UNSUPPORTED)
                .collect();
            assert_eq!(warnings.len(), 1);
            let d = warnings[0];
            assert_eq!(d.count(), 1);
            assert_eq!(
                d.param("kind"),
                Some(&zumen_base::Value::Text(
                    "IFCRELASSOCIATESCLASSIFICATION".to_owned()
                ))
            );
            assert_eq!(
                d.param("target_kind"),
                Some(&zumen_base::Value::Text("IFCPROPERTYSET".to_owned()))
            );
            assert_eq!(d.param("position"), Some(&zumen_base::Value::UInt(2)));
            assert_eq!(
                d.places()[0].bytes().unwrap().start,
                src.find("#32=").unwrap() as u64
            );
            let dropped: Vec<_> = diag
                .iter()
                .filter(|d| d.code() == codes::IFC_ENTITY_DROPPED)
                .collect();
            assert_eq!(dropped.len(), 1);
            assert_eq!(
                dropped[0].param("kind"),
                Some(&zumen_base::Value::Text(
                    "IFCDISTRIBUTIONFLOWELEMENT".to_owned()
                ))
            );
            assert_eq!(dropped[0].count(), 1);
            let owner = element(&b, if on_type { 110 } else { 11 });
            assert_eq!(
                b.components.properties.get(owner).unwrap().sets[0].name,
                "Set"
            );
        }
    }
}

#[test]
fn pending_relation_targets_use_budgets() {
    let (_, _, few) = lift(&resource_source("IFC4", 2, true, false), Limits::default());
    let src = resource_source("IFC4", 202, true, false);
    let (b, diag, many) = lift(&src, Limits::default());
    assert!(b.is_ok());
    assert_eq!(
        diag.iter()
            .filter(|d| d.code() == codes::IFC_RELATION_TARGET_UNSUPPORTED)
            .map(|d| d.count())
            .sum::<u64>(),
        202
    );
    assert!(many - few >= 200 * 32);
    let (b, diag, _) = lift(
        &src,
        Limits::default().with(Kind::DecodedBytes, Limit::Finite(few + 10 * 32)),
    );
    assert!(b.is_err());
    assert!(has(&diag, codes::BUDGET_DECODED_BYTES_EXCEEDED));
}
