//! 設備の要素・型・型のプロパティと関係を、各版の最小入力で確かめる。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{Acquired, Building, ElementRef, Hash, Literal, MaterialAssignmentLevel};
use zumen_ifc::{Ifc, IfcFile};
use zumen_json::Writer;
use zumen_sdk::{Codec, Error, Lift};

#[path = "services/cases.rs"]
mod cases;
use cases::CASES;

struct Case {
    name: &'static str,
    /// 各版の種類の位置。0 は属性なし、255 はその版に実体なし
    predefined: [u8; 3],
}
const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];

/// 旧版の一般の要素と設備の型の、実際の対応を使って型との関係を作る。
const PAIRS: &[(&str, &str)] = &[
    ("IfcPipeSegmentType", "IfcFlowSegment"),
    ("IfcPipeFittingType", "IfcFlowFitting"),
    ("IfcDuctSegmentType", "IfcFlowSegment"),
    ("IfcDuctFittingType", "IfcFlowFitting"),
    ("IfcCableCarrierSegmentType", "IfcFlowSegment"),
    ("IfcCableCarrierFittingType", "IfcFlowFitting"),
    ("IfcCableSegmentType", "IfcFlowSegment"),
    ("IfcAirTerminalType", "IfcFlowTerminal"),
    ("IfcLightFixtureType", "IfcFlowTerminal"),
    ("IfcSanitaryTerminalType", "IfcFlowTerminal"),
    ("IfcValveType", "IfcFlowController"),
    ("IfcDamperType", "IfcFlowController"),
    ("IfcSwitchingDeviceType", "IfcFlowController"),
    ("IfcFlowMeterType", "IfcFlowController"),
    ("IfcFanType", "IfcFlowMovingDevice"),
    ("IfcPumpType", "IfcFlowMovingDevice"),
    ("IfcUnitaryEquipmentType", "IfcEnergyConversionDevice"),
    ("IfcDuctSilencerType", "IfcFlowTreatmentDevice"),
    ("IfcActuatorType", "IfcDistributionControlElement"),
    ("IfcSensorType", "IfcDistributionControlElement"),
    ("IfcDistributionElementType", "IfcDistributionElement"),
    ("IfcDiscreteAccessoryType", "IfcDiscreteAccessory"),
    ("IfcTransportElementType", "IfcTransportElement"),
];

fn gid(id: u64) -> String {
    format!("{id:022}")
}
fn record(case: &Case, version: usize, id: u64) -> String {
    let typed = case.name.ends_with("Type");
    let position = case.predefined[version];
    assert_ne!(position, 255);
    let mut args = vec![
        format!("'{}'", gid(id)),
        "$".into(),
        format!("'{}'", case.name),
        "'Description'".into(),
    ];
    if typed {
        args.extend([
            "'Applicable'".into(),
            "(#40)".into(),
            "$".into(),
            "'Tag'".into(),
            "'Element type'".into(),
        ]);
    } else {
        args.extend([
            "'Object type'".into(),
            "#20".into(),
            "$".into(),
            "'Tag'".into(),
        ]);
    }
    if position > 0 {
        assert_eq!(position as usize, args.len() + 1);
        args.push(".USERDEFINED.".into());
    }
    if version == 0 {
        match case.name {
            "IfcElectricDistributionPoint" => args.push("'Function'".into()),
            "IfcTransportElement" => args.extend(["1000.".into(), "12".into()]),
            "IfcDistributionControlElement" => args.push("'Control'".into()),
            _ => {}
        }
    }
    format!(
        "#{id}={}({});\n",
        case.name.to_ascii_uppercase(),
        args.join(",")
    )
}
fn header(version: usize) -> String {
    format!(
        "ISO-10303-21;\nHEADER;FILE_DESCRIPTION((''),'2;1');FILE_NAME('','',(''),(''),'','','');FILE_SCHEMA(('{}'));ENDSEC;\nDATA;\n\
    #1=IFCPROJECT('{}',$,'Project',$,$,$,$,(#8),#9);\n\
    #8=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-5,#21,$);\n\
    #9=IFCUNITASSIGNMENT((#10));#10=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);\n\
    #20=IFCLOCALPLACEMENT($,#21);#21=IFCAXIS2PLACEMENT3D(#22,$,$);#22=IFCCARTESIANPOINT((1.,2.,3.));\n\
    #40=IFCPROPERTYSET('{}',$,'Pset_ServiceTest',$,(#41));#41=IFCPROPERTYSINGLEVALUE('Probe',$,IFCLABEL('From type'),$);\n",
        VERSIONS[version],
        gid(1),
        gid(40)
    )
}
fn source(version: usize) -> String {
    let mut text = header(version);
    for (at, case) in CASES.iter().enumerate() {
        if case.predefined[version] != 255 {
            text.push_str(&record(case, version, 1000 + at as u64));
        }
    }
    for (at, (type_name, old_name)) in PAIRS.iter().enumerate() {
        let name = if version == 0 {
            *old_name
        } else {
            type_name.strip_suffix("Type").unwrap()
        };
        let case = CASES.iter().find(|c| c.name == name).unwrap();
        let id = 2000 + at as u64;
        text.push_str(&record(case, version, id));
        let target = 1000 + CASES.iter().position(|c| c.name == *type_name).unwrap() as u64;
        let relation = 3000 + at as u64;
        text.push_str(&format!(
            "#{relation}=IFCRELDEFINESBYTYPE('{}',$,$,$,(#{id}),#{target});\n",
            gid(relation)
        ));
    }
    text.push_str("ENDSEC;END-ISO-10303-21;\n");
    text
}
fn with<T>(limits: Limits, f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics, Budget) {
    let mut b = Budget::new(limits);
    let mut d = Diagnostics::new();
    let c = CancelToken::new();
    let r = f(&mut Cx::new(&mut b, &mut d, &c));
    (r, d, b)
}
fn read(src: &str) -> IfcFile {
    with(Limits::default(), |cx| Ifc::read(src.as_bytes(), cx))
        .0
        .unwrap()
}
fn lifted(file: &IfcFile) -> Building {
    with(Limits::default(), |cx| Lift::<Building>::lift(file, cx))
        .0
        .unwrap()
}
fn at(building: &Building, instance: u64) -> ElementRef {
    ElementRef(
        building
            .elements()
            .iter()
            .position(|e| e.source().instance == Some(instance))
            .unwrap() as u32,
    )
}

#[test]
fn all_service_entities_keep_identity_versioned_kind_type_properties_and_links() {
    for version in 0..3 {
        let file = read(&source(version));
        let (b, d, _) = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx));
        let b = b.unwrap();
        assert!(d.is_empty(), "{version}: {d:?}");
        let supported = CASES
            .iter()
            .filter(|c| c.predefined[version] != 255)
            .count();
        assert_eq!(supported, [87, 162, 162][version]);
        assert_eq!(b.elements().len(), 1 + supported + PAIRS.len());
        for (index, case) in CASES.iter().enumerate() {
            if case.predefined[version] == 255 {
                continue;
            }
            let element = at(&b, 1000 + index as u64);
            let class = b.components.class.get(element).unwrap();
            assert_eq!(class.name, case.name);
            assert_eq!(class.original, case.name);
            assert_eq!(
                class.predefined.as_deref(),
                if case.predefined[version] == 0 {
                    None
                } else {
                    Some("USERDEFINED")
                },
                "{version}/{}",
                case.name
            );
            assert_eq!(
                class.user_defined.as_deref(),
                Some(if case.name == "IfcElectricDistributionPoint" {
                    "Function"
                } else if case.name.ends_with("Type") {
                    "Element type"
                } else {
                    "Object type"
                })
            );
            let identity = b.components.identity.get(element).unwrap();
            assert_eq!(identity.name.as_deref(), Some(case.name));
            assert_eq!(identity.tag.as_deref(), Some("Tag"));
            if case.name.ends_with("Type") {
                let sets = &b.components.properties.get(element).unwrap().sets;
                assert_eq!(sets.len(), 1);
                assert_eq!(sets[0].name, "Pset_ServiceTest");
                assert_eq!(sets[0].properties[0].name, "Probe");
                assert_eq!(
                    sets[0].properties[0].value.as_ref().unwrap().value,
                    Literal::Text("From type".into())
                );
            } else {
                assert!(b.components.placement.get(element).is_some());
            }
        }
        for (index, (name, _)) in PAIRS.iter().enumerate() {
            let element = at(&b, 2000 + index as u64);
            let target = at(
                &b,
                1000 + CASES.iter().position(|c| c.name == *name).unwrap() as u64,
            );
            assert_eq!(b.components.r#type.get(element).unwrap().target, target);
            assert!(
                b.element(element)
                    .unwrap()
                    .source()
                    .relations
                    .iter()
                    .any(|r| r.instance == 3000 + index as u64 && r.position == Some(0))
            );
        }
    }
}

#[test]
fn versions_do_not_promote_unavailable_entities_or_prefixes() {
    for version in 0..3 {
        for case in CASES.iter().filter(|c| c.predefined[version] == 255) {
            let actual = case.predefined.iter().position(|p| *p != 255).unwrap();
            let src = format!(
                "{}{}ENDSEC;END-ISO-10303-21;",
                header(version),
                record(case, actual, 1000)
            );
            let file = read(&src);
            let (b, d, _) = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx));
            let b = b.unwrap();
            assert!(
                !b.elements()
                    .iter()
                    .any(|e| e.source().instance == Some(1000)),
                "{version}/{}",
                case.name
            );
            assert!(d.iter().any(|d| d.code() == codes::IFC_ENTITY_DROPPED));
        }
        let src = source(version).replace("IFCPUMP", "IFCPUMPFUTURE");
        let b = lifted(&read(&src));
        assert!(
            !b.components
                .class
                .iter()
                .any(|(_, c)| c.name.starts_with("IfcPump"))
        );
    }
}

#[test]
fn service_types_are_entries_for_owned_geometry_and_material_inheritance() {
    for version in 0..3 {
        let mut src = source(version);
        let typed = 1000
            + CASES
                .iter()
                .position(|c| c.name == "IfcPipeSegmentType")
                .unwrap() as u64;
        let material = if version == 0 { "" } else { ",$,$" };
        let assignment = format!(
            "#4000=IFCMATERIAL('Pipe'{material});\n\
             #4001=IFCRELASSOCIATESMATERIAL('{}',$,$,$,(#{typed}),#4000);\n\
             ENDSEC;END-ISO-10303-21;",
            gid(4001)
        );
        src = src.replace("ENDSEC;END-ISO-10303-21;", &assignment);
        let file = read(&src);
        let b = with(Limits::default(), |cx| file.lift_geometry(cx))
            .0
            .unwrap();
        let g = b.geometry(at(&b, 2000)).unwrap();
        let Some(Acquired::Value(m)) = g.materials() else {
            panic!()
        };
        assert_eq!(m.level, MaterialAssignmentLevel::Type);
        assert_eq!(m.source.instance, Some(typed));
        assert_eq!(m.assignments[0].members[0].source.instance, Some(4000));
    }
}

#[test]
fn new_elements_keep_ids_on_record_reordering_and_share_budget_stops() {
    for version in 0..3 {
        let src = source(version);
        let file = read(&src);
        let b = lifted(&file);
        assert_eq!(b, lifted(&file));
        let (prefix, rest) = src.split_once("DATA;\n").unwrap();
        let records = rest.strip_suffix("ENDSEC;END-ISO-10303-21;\n").unwrap();
        let reversed = format!(
            "{prefix}DATA;\n{}ENDSEC;END-ISO-10303-21;\n",
            records.lines().rev().collect::<Vec<_>>().join("\n")
        );
        let changed = lifted(&read(&reversed));
        for e in b.elements() {
            let previous = b.find(e.id()).unwrap();
            let next = changed.find(e.id()).unwrap();
            assert_eq!(Hash::element(&b, previous), Hash::element(&changed, next));
        }
        for kind in [Kind::DecodedBytes, Kind::Elements, Kind::Work] {
            let used = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
                .2
                .used(kind);
            assert!(used > 0);
            let (r, _, budget) = with(
                Limits::default().with(kind, Limit::Finite(used - 1)),
                |cx| Lift::<Building>::lift(&file, cx),
            );
            assert!(matches!(r, Err(Error::Stopped(_))), "{version}/{kind:?}");
            assert_eq!(budget.used(Kind::Depth), 0);
        }
        let (r, _, budget) = with(Limits::default(), |cx| {
            cx.cancel.cancel();
            Lift::<Building>::lift(&file, cx)
        });
        assert!(matches!(r, Err(Error::Stopped(_))));
        assert_eq!(budget.used(Kind::Elements), 0);
    }
}

fn json(building: &Building) -> Vec<u8> {
    let mut out = Writer::new();
    out.begin_array();
    for (at, e) in building.elements().iter().enumerate() {
        let at = ElementRef(at as u32);
        let class = building.components.class.get(at).unwrap();
        out.begin_object();
        out.key("source");
        out.uint(e.source().instance.unwrap());
        out.key("id");
        out.string(&e.id().to_string());
        out.key("class");
        out.string(&class.name);
        out.key("predefined");
        if let Some(p) = &class.predefined {
            out.string(p)
        } else {
            out.null()
        };
        out.key("user_defined");
        if let Some(p) = &class.user_defined {
            out.string(p)
        } else {
            out.null()
        };
        out.key("type");
        if let Some(t) = building.components.r#type.get(at) {
            out.uint(
                building
                    .element(t.target)
                    .unwrap()
                    .source()
                    .instance
                    .unwrap(),
            )
        } else {
            out.null()
        };
        out.key("property");
        if let Some(p) = building.components.properties.get(at) {
            let Literal::Text(value) = &p.sets[0].properties[0].value.as_ref().unwrap().value
            else {
                panic!()
            };
            out.string(value)
        } else {
            out.null()
        };
        out.key("hash");
        out.string(&Hash::element(building, at).unwrap().to_string());
        out.end_object();
    }
    out.end_array();
    out.into_bytes()
}

#[test]
#[allow(clippy::disallowed_methods)] // 合成入力の照合の出力先だけを読む。
fn exports_services_values_and_diagnostics_for_other_readers() {
    let Ok(dir) = std::env::var("ZUMEN_IFC_SERVICES_OUT") else {
        return;
    };
    let dir = std::path::Path::new(&dir);
    std::fs::create_dir_all(dir).unwrap();
    for (version, name) in VERSIONS.iter().enumerate() {
        let src = source(version);
        let file = read(&src);
        let (b, d, budget) = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx));
        let b = b.unwrap();
        std::fs::write(dir.join(format!("{name}.ifc")), src).unwrap();
        std::fs::write(dir.join(format!("{name}.json")), json(&b)).unwrap();
        let mut contract = format!("{b:?}\n{d:?}\n");
        for k in Kind::ALL {
            contract.push_str(&format!("{k:?}={};", budget.used(k)))
        }
        for k in [Kind::DecodedBytes, Kind::Elements, Kind::Work] {
            let cap = budget.used(k) - 1;
            let (r, d, b) = with(Limits::default().with(k, Limit::Finite(cap)), |cx| {
                Lift::<Building>::lift(&file, cx)
            });
            contract.push_str(&format!("\n{k:?}={cap}: {r:?}; {d:?}; "));
            for k in Kind::ALL {
                contract.push_str(&format!("{k:?}={};", b.used(k)))
            }
        }
        std::fs::write(dir.join(format!("{name}.txt")), contract).unwrap();
    }
}
