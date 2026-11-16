//! 材料・プロパティ・単位・関係・建具の値の最小入力。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{Acquired, Building, ElementRef, Hash, Literal, MaterialKind};
use zumen_ifc::{Ifc, IfcFile};
use zumen_json::Writer;
use zumen_sdk::{Codec, Error, Lift};

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];
fn gid(n: u64) -> String {
    format!("{n:022}")
}
fn header(version: usize) -> String {
    format!(
        "ISO-10303-21;\nHEADER;FILE_DESCRIPTION((''),'2;1');FILE_NAME('','',(''),(''),'','','');FILE_SCHEMA(('{}'));ENDSEC;\nDATA;\n\
        #1=IFCPROJECT('{}',$,'Project',$,$,$,$,$,#2);\n\
        #2=IFCUNITASSIGNMENT((#3));#3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n",
        VERSIONS[version],
        gid(1)
    )
}
fn doors(version: usize) -> String {
    let mut src = header(version);
    let door_extra = if version == 0 {
        ""
    } else {
        ",.DOOR.,.SINGLE_SWING_LEFT.,$"
    };
    let window_extra = if version == 0 {
        ""
    } else {
        ",.WINDOW.,.SINGLE_PANEL.,$"
    };
    src.push_str(&format!(
        "#100=IFCDOOR('{}',$,'Door',$,$,$,$,$,2100.,900.{door_extra});\n\
        #101=IFCWINDOW('{}',$,'Window',$,$,$,$,$,1000.,800.{window_extra});\n\
        #102=IFCDOOR('{}',$,'Direct',$,$,$,$,$,2100.,900.{door_extra});\n",
        gid(100),
        gid(101),
        gid(102)
    ));
    if version == 0 {
        src.push_str(&format!("#110=IFCDOORSTYLE('{}',$,'Door type',$,$,(#200,#201),$,$,.SINGLE_SWING_LEFT.,.NOTDEFINED.,.F.,.F.);\n\
            #111=IFCWINDOWSTYLE('{}',$,'Window type',$,$,(#202,#203),$,$,.NOTDEFINED.,.SINGLE_PANEL.,.F.,.F.);\n",gid(110),gid(111)));
    } else {
        src.push_str(&format!("#110=IFCDOORTYPE('{}',$,'Door type',$,$,(#200,#201),$,$,$,.DOOR.,.SINGLE_SWING_LEFT.,.F.,$);\n\
            #111=IFCWINDOWTYPE('{}',$,'Window type',$,$,(#202,#203),$,$,$,.WINDOW.,.SINGLE_PANEL.,.F.,$);\n",gid(110),gid(111)));
    }
    let door_offsets = if version == 0 { "" } else { ",-0.3,0.4" };
    let window_offsets = if version == 0 { "" } else { ",-0.5,0.6,-0.7" };
    src.push_str(&format!("#200=IFCDOORLININGPROPERTIES('{}',$,'Door lining',$,1.,0.,2.,0.,3.,4.,-1.,-2.,5.,6.,${door_offsets});\n\
        #201=IFCDOORPANELPROPERTIES('{}',$,'Door panel',$,7.,.SWINGING.,0.5,.LEFT.,$);\n\
        #202=IFCWINDOWLININGPROPERTIES('{}',$,'Window lining',$,8.,0.,9.,10.,0.2,0.4,0.6,0.8,${window_offsets});\n\
        #203=IFCWINDOWPANELPROPERTIES('{}',$,'Window panel',$,.SIDEHUNGLEFTHAND.,.LEFT.,11.,12.,$);\n\
        #300=IFCRELDEFINESBYTYPE('{}',$,$,$,(#100),#110);\n\
        #301=IFCRELDEFINESBYTYPE('{}',$,$,$,(#101),#111);\n\
        #302=IFCRELDEFINESBYPROPERTIES('{}',$,$,$,(#102),#200);\n\
        ENDSEC;END-ISO-10303-21;\n",gid(200),gid(201),gid(202),gid(203),gid(300),gid(301),gid(302)));
    src
}

#[test]
fn geometry_lift_keeps_joinery_attributes_outside_property_sets() {
    for version in 0..3 {
        let file = read(&doors(version));
        let (plain, _, _) = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx));
        assert!(plain.unwrap().components.extension.is_empty());
        let (model, _, _) = with(Limits::default(), |cx| file.lift_geometry(cx));
        let model = model.unwrap();
        let get = |id, name| {
            model
                .components
                .extension
                .get(at(&model, id))
                .unwrap()
                .values
                .iter()
                .find(|v| v.name == name)
                .unwrap()
        };
        assert_eq!(get(100, "OverallWidth").value.value, Literal::Real(900.0));
        assert_eq!(get(101, "OverallHeight").value.value, Literal::Real(1000.0));
        let operation = get(110, "OperationType");
        assert_eq!(operation.namespace, "ifc.attribute");
        assert_eq!(
            operation.value.value,
            Literal::Enumeration("SINGLE_SWING_LEFT".into())
        );
        assert_eq!(
            operation.value.type_name.as_deref(),
            Some(if version == 0 {
                "IfcDoorStyleOperationEnum"
            } else {
                "IfcDoorTypeOperationEnum"
            })
        );
        if version > 0 {
            assert_eq!(
                get(101, "PartitioningType").value.value,
                Literal::Enumeration("SINGLE_PANEL".into())
            );
            assert!(
                model
                    .components
                    .extension
                    .get(at(&model, 101))
                    .unwrap()
                    .values
                    .iter()
                    .all(|v| v.name != "OperationType")
            );
        }
    }
}

fn materials(version: usize) -> String {
    let material_extra = if version == 0 { "" } else { ",$,$" };
    let layer_extra = if version == 0 { "" } else { ",'Layer',$,$,$" };
    let set_extra = if version == 0 { "" } else { ",$" };
    let mut data = format!(
        "#400=IFCMATERIAL('A'{material_extra});#401=IFCMATERIAL('B'{material_extra});\n\
         #420=IFCMATERIALLAYER(#400,10.,.U.{layer_extra});\n\
         #421=IFCMATERIALLAYER(#401,20.,.F.{layer_extra});\n\
         #430=IFCMATERIALLAYERSET((#420,#421),'Layers'{set_extra});\n\
         #432=IFCMATERIALLAYERSETUSAGE(#430,.AXIS2.,.POSITIVE.,-5.{set_extra});\n\
         #450=IFCBUILDINGELEMENTPROXY('{}',$,'Layered',$,$,$,$,$,.NOTDEFINED.);\n\
         #460=IFCRELASSOCIATESMATERIAL('{}',$,$,$,(#450),#432);\n\
         #471=IFCPROPERTYSINGLEVALUE('Rating',$,IFCLABEL('rated'),$);\n",
        gid(450),
        gid(460)
    );
    if version == 0 {
        data.push_str("#470=IFCEXTENDEDMATERIALPROPERTIES(#400,(#471),$,'Thermal');\n");
    } else {
        data.push_str(&format!(
            "#470=IFCMATERIALPROPERTIES('Thermal',$,(#471),#400);\n\
             #440=IFCMATERIALCONSTITUENTSET('Mix',$,(#441,#442));\n\
             #441=IFCMATERIALCONSTITUENT('First',$,#400,0.4,$);\n\
             #442=IFCMATERIALCONSTITUENT('Second',$,#401,0.6,$);\n\
             #443=IFCMATERIALPROFILESET('Profiles',$,(#444,#445),$);\n\
             #444=IFCMATERIALPROFILE('Rect',$,#400,#446,$,$);\n\
             #445=IFCMATERIALPROFILE('Round',$,#401,#447,$,$);\n\
             #446=IFCRECTANGLEPROFILEDEF(.AREA.,$,$,1.,2.);\n\
             #447=IFCCIRCLEPROFILEDEF(.AREA.,$,$,3.);\n\
             #451=IFCBUILDINGELEMENTPROXY('{}',$,'Mix',$,$,$,$,$,.NOTDEFINED.);\n\
             #452=IFCBUILDINGELEMENTPROXY('{}',$,'Profiles',$,$,$,$,$,.NOTDEFINED.);\n\
             #461=IFCRELASSOCIATESMATERIAL('{}',$,$,$,(#451),#440);\n\
             #462=IFCRELASSOCIATESMATERIAL('{}',$,$,$,(#452),#443);\n",
            gid(451),
            gid(452),
            gid(461),
            gid(462)
        ));
    }
    doors(version).replace(
        "ENDSEC;END-ISO-10303-21;",
        &(data + "ENDSEC;END-ISO-10303-21;"),
    )
}

fn property_values(version: usize) -> String {
    let data = format!(
        "#500=IFCPROPERTYSET('{}',$,'Values',$,(#501,#502,#503));\n\
         #501=IFCPROPERTYLISTVALUE('Lengths',$,(IFCLENGTHMEASURE(10.),IFCLENGTHMEASURE(20.)),$);\n\
         #502=IFCPROPERTYENUMERATEDVALUE('Choices',$,(IFCLENGTHMEASURE(20.)),#510);\n\
         #503=IFCPROPERTYLISTVALUE('Flags',$,(IFCBOOLEAN(.T.),IFCBOOLEAN(.F.)),$);\n\
         #510=IFCPROPERTYENUMERATION('Allowed',(IFCLENGTHMEASURE(10.),IFCLENGTHMEASURE(20.)),#511);\n\
         #511=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);\n\
         #512=IFCRELDEFINESBYPROPERTIES('{}',$,$,$,(#102),#500);\n\
         ENDSEC;END-ISO-10303-21;",
        gid(500),
        gid(512)
    );
    doors(version).replace("ENDSEC;END-ISO-10303-21;", &data)
}

fn structured_values(version: usize) -> String {
    let point = if version == 0 {
        ""
    } else {
        ",IFCLENGTHMEASURE(15.)"
    };
    let missing = if version == 0 { "" } else { ",$" };
    let data = format!(
        "#520=IFCPROPERTYBOUNDEDVALUE('Range',$,IFCLENGTHMEASURE(20.),IFCLENGTHMEASURE(10.),${point});\n\
        #521=IFCCOMPLEXPROPERTY('Assembly','Detail','Object',(#522,#523));\n\
        #522=IFCPROPERTYSINGLEVALUE('Size',$,IFCLENGTHMEASURE(5.),$);\n\
        #523=IFCCOMPLEXPROPERTY('Nested',$,'Usage',(#524,#522));\n\
        #524=IFCPROPERTYBOUNDEDVALUE('Lower only',$,$,IFCLENGTHMEASURE(-3.),#511{missing});\n\
        ENDSEC;END-ISO-10303-21;"
    );
    property_values(version)
        .replace("(#501,#502,#503)", "(#501,#502,#503,#520,#521)")
        .replace("ENDSEC;END-ISO-10303-21;", &data)
}
fn all_metadata(version: usize) -> String {
    let structured = structured_values(version);
    let extra = structured.split_once("#500=").unwrap().1;
    let mut src =
        materials(version).replace("ENDSEC;END-ISO-10303-21;", &("#500=".to_owned() + extra));
    let named = if version == 2 { ",'mm3/s'" } else { "" };
    let data = format!(
        "#600=IFCDIMENSIONALEXPONENTS(1,0,0,0,0,0,0);\n\
         #601=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);\n\
         #602=IFCCONVERSIONBASEDUNIT(#600,.LENGTHUNIT.,'inch',#603);\n\
         #603=IFCMEASUREWITHUNIT(IFCLENGTHMEASURE(0.0254),#601);\n\
         #604=IFCCONVERSIONBASEDUNIT(#600,.LENGTHUNIT.,'foot',#605);\n\
         #605=IFCMEASUREWITHUNIT(IFCLENGTHMEASURE(12.),#602);\n\
         #606=IFCSIUNIT(*,.TIMEUNIT.,$,.SECOND.);\n\
         #607=IFCDERIVEDUNITELEMENT(#3,3);#608=IFCDERIVEDUNITELEMENT(#606,-1);\n\
         #609=IFCDERIVEDUNIT((#607,#608),.VOLUMETRICFLOWRATEUNIT.,${named});\n\
         #610=IFCPROPERTYSINGLEVALUE('Inch',$,IFCLENGTHMEASURE(2.),#602);\n\
         #611=IFCPROPERTYSINGLEVALUE('Foot',$,IFCLENGTHMEASURE(3.),#604);\n\
         #612=IFCPROPERTYSINGLEVALUE('Flow',$,IFCVOLUMETRICFLOWRATEMEASURE(4.),$);\n\
         #613=IFCPROPERTYSET('{}',$,'Measures',$,(#610,#611,#612));\n\
         #614=IFCRELDEFINESBYPROPERTIES('{}',$,$,$,(#102),#613);\n\
         #700=IFCACTOR('{}',$,'Designer',$,'Role',$);\n\
         #701=IFCOCCUPANT('{}',$,'Owner',$,$,$,.OWNER.);\n\
         #702=IFCCOVERING('{}',$,'Covering',$,$,$,$,$,.CLADDING.);\n\
         #710=IFCRELCONNECTSELEMENTS('{}',$,$,$,$,#100,#101);\n\
         #711=IFCRELCOVERSBLDGELEMENTS('{}',$,$,$,#100,(#702));\n\
         #712=IFCRELASSIGNSTOACTOR('{}',$,$,$,(#100,#102),$,#700,$);\n\
         #713=IFCRELASSIGNSTOACTOR('{}',$,$,$,(#101),$,#701,$);\n\
         ENDSEC;END-ISO-10303-21;\n",
        gid(613),
        gid(614),
        gid(700),
        gid(701),
        gid(702),
        gid(710),
        gid(711),
        gid(712),
        gid(713)
    );
    src = src.replace("IFCUNITASSIGNMENT((#3))", "IFCUNITASSIGNMENT((#3,#609))");
    src.replace("ENDSEC;END-ISO-10303-21;", &data)
}

#[test]
fn conversion_chains_derived_units_and_relations_keep_values_and_sources() {
    for version in 0..3 {
        let (b, d, budget) = with(Limits::default(), |cx| {
            Lift::<Building>::lift(&read(&all_metadata(version)), cx)
        });
        let b = b.unwrap();
        assert!(
            d.iter().all(|d| d.code() == codes::IFC_ENTITY_DROPPED),
            "{d:?}"
        );
        assert_eq!(budget.used(Kind::Depth), 0);
        let properties = &b.components.properties.get(at(&b, 102)).unwrap().sets[2].properties;
        for (p, scale) in properties.iter().zip([0.0254, 0.3048, 1.0e-9]) {
            assert!((p.unit.as_ref().unwrap().scale.unwrap() / scale - 1.).abs() < 1.0e-14);
        }
        assert_eq!(
            properties[2].value.as_ref().unwrap().type_name.as_deref(),
            Some("IfcVolumetricFlowRateMeasure")
        );
        let links = &b.components.connection.get(at(&b, 100)).unwrap().links;
        assert_eq!(
            links.iter().map(|l| l.kind.as_str()).collect::<Vec<_>>(),
            [
                "IfcRelConnectsElements",
                "IfcRelCoversBldgElements",
                "IfcRelAssignsToActor"
            ]
        );
        assert_eq!(
            links.iter().map(|l| l.target).collect::<Vec<_>>(),
            [at(&b, 101), at(&b, 702), at(&b, 700)]
        );
        assert_eq!(
            b.components
                .class
                .get(at(&b, 701))
                .unwrap()
                .predefined
                .as_deref(),
            Some("OWNER")
        );
        for (owner, relation, position) in [
            (100, 710, None),
            (101, 710, None),
            (702, 711, Some(0)),
            (100, 712, Some(0)),
            (102, 712, Some(1)),
        ] {
            assert!(
                b.element(at(&b, owner))
                    .unwrap()
                    .source()
                    .relations
                    .iter()
                    .any(|r| r.instance == relation && r.position == position)
            );
        }
    }
}

#[test]
fn unit_graph_cycles_overflow_and_malformed_relations_are_diagnosed() {
    for version in 0..3 {
        let src = all_metadata(version);
        for (before, after) in [
            ("IFCLENGTHMEASURE(12.),#602", "IFCLENGTHMEASURE(12.),#604"),
            ("IFCLENGTHMEASURE(12.)", "IFCLENGTHMEASURE(1.E999)"),
            (
                "IFCDERIVEDUNITELEMENT(#3,3)",
                "IFCDERIVEDUNITELEMENT(#3,999999)",
            ),
            (
                "IFCDERIVEDUNITELEMENT(#3,3)",
                "IFCDERIVEDUNITELEMENT(#3,1.5)",
            ),
        ] {
            let (b, d, budget) = with(Limits::default(), |cx| {
                Lift::<Building>::lift(&read(&src.replace(before, after)), cx)
            });
            assert!(b.is_ok());
            assert!(
                d.iter().any(|d| d.code() == codes::IFC_UNIT_INVALID),
                "{d:?}"
            );
            assert_eq!(budget.used(Kind::Depth), 0);
        }
        let (b, d, _) = with(Limits::default(), |cx| {
            Lift::<Building>::lift(
                &read(&src.replace("(#100,#102),$,#700", "(#99999,7,#102),$,#700")),
                cx,
            )
        });
        let b = b.unwrap();
        assert!(d.iter().any(|d| d.code() == codes::IFC_ATTRIBUTE_INVALID));
        assert!(
            b.element(at(&b, 102))
                .unwrap()
                .source()
                .relations
                .iter()
                .any(|r| r.instance == 712 && r.position == Some(2))
        );
        let file = read(&src);
        let (r, _, budget) = with(
            Limits::default().with(Kind::Depth, Limit::Finite(1)),
            |cx| Lift::<Building>::lift(&file, cx),
        );
        assert!(matches!(r, Err(Error::Stopped(_))));
        assert_eq!(budget.used(Kind::Depth), 0);
    }
}

#[test]
fn deep_flat_property_and_unit_graphs_stop_without_recursive_ownership() {
    let mut extra = String::new();
    for i in 1000..2000 {
        extra.push_str(&format!(
            "#{i}=IFCCOMPLEXPROPERTY('Nested',$,'Usage',(#{}));\n",
            i + 1
        ));
    }
    extra.push_str("#2000=IFCPROPERTYSINGLEVALUE('Leaf',$,IFCLENGTHMEASURE(1.),$);\n");
    for i in 3000..4000 {
        extra.push_str(&format!("#{i}=IFCCONVERSIONBASEDUNIT(#600,.LENGTHUNIT.,'Chain',#{});#{}=IFCMEASUREWITHUNIT(IFCLENGTHMEASURE(1.),#{});\n",i+2000,i+2000,i+1));
    }
    extra.push_str("#4000=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);\n");
    let src = all_metadata(1)
        .replace(
            "(#501,#502,#503,#520,#521)",
            "(#501,#502,#503,#520,#521,#1000)",
        )
        .replace("IFCLENGTHMEASURE(3.),#604", "IFCLENGTHMEASURE(3.),#3000")
        .replace(
            "ENDSEC;END-ISO-10303-21;",
            &(extra + "ENDSEC;END-ISO-10303-21;"),
        );
    let file = read(&src);
    let limits = Limits::default().with(Kind::Depth, Limit::Finite(1100));
    let (b, d, used) = with(limits, |cx| Lift::<Building>::lift(&file, cx));
    let b = b.unwrap();
    assert_eq!(
        b.components.property_details.get(at(&b, 102)).unwrap().sets[0]
            .properties
            .len(),
        1010
    );
    assert!(
        d.iter().all(|d| d.code() == codes::IFC_ENTITY_DROPPED),
        "{d:?}"
    );
    assert_eq!(used.used(Kind::Depth), 0);
    let (r, _, used) = with(
        Limits::default().with(Kind::Depth, Limit::Finite(32)),
        |cx| Lift::<Building>::lift(&file, cx),
    );
    assert!(matches!(r, Err(Error::Stopped(_))));
    assert_eq!(used.used(Kind::Depth), 0);
}

#[test]
fn thermal_conductivity_uses_the_declared_conductance_unit() {
    for version in 0..3 {
        let src = all_metadata(version)
            .replace(".VOLUMETRICFLOWRATEUNIT.", ".THERMALCONDUCTANCEUNIT.")
            .replace("IFCVOLUMETRICFLOWRATEMEASURE", "IFCTHERMALCONDUCTIVITYMEASURE")
            .replace("IFCDERIVEDUNITELEMENT(#3,3)", "IFCDERIVEDUNITELEMENT(#615,1)")
            .replace("IFCDERIVEDUNITELEMENT(#606,-1)", "IFCDERIVEDUNITELEMENT(#601,-1)")
            .replace("(#607,#608)", "(#607,#608,#617)")
            .replace("ENDSEC;END-ISO-10303-21;", "#615=IFCSIUNIT(*,.POWERUNIT.,$,.WATT.);#616=IFCSIUNIT(*,.THERMODYNAMICTEMPERATUREUNIT.,$,.KELVIN.);#617=IFCDERIVEDUNITELEMENT(#616,-1);ENDSEC;END-ISO-10303-21;");
        let b = lift(&read(&src));
        let value = &b.components.properties.get(at(&b, 102)).unwrap().sets[2].properties[2];
        let unit = value.unit.as_ref().unwrap();
        assert_eq!(unit.kind, "THERMALCONDUCTANCEUNIT");
        assert_eq!(unit.scale, Some(1.));
    }
}
fn with<T>(limits: Limits, f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics, Budget) {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag, budget)
}
fn read(src: &str) -> IfcFile {
    with(Limits::default(), |cx| Ifc::read(src.as_bytes(), cx))
        .0
        .unwrap()
}
fn lift(file: &IfcFile) -> Building {
    with(Limits::default(), |cx| Lift::<Building>::lift(file, cx))
        .0
        .unwrap()
}
fn at(b: &Building, id: u64) -> ElementRef {
    ElementRef(
        b.elements()
            .iter()
            .position(|e| e.source().instance == Some(id))
            .unwrap() as u32,
    )
}

#[test]
fn structured_properties_keep_bounds_parentage_shared_values_and_meaning_hashes() {
    use zumen_building::PropertyDetailValue;
    for version in 0..3 {
        let src = structured_values(version);
        let (b, d, _) = with(Limits::default(), |cx| {
            Lift::<Building>::lift(&read(&src), cx)
        });
        let mut b = b.unwrap();
        assert!(d.is_empty(), "{d:?}");
        let element = at(&b, 102);
        let details = b.components.property_details.get(element).unwrap();
        let p = &details.sets[0].properties;
        assert_eq!(p.len(), 9);
        assert_eq!(
            p.iter().map(|p| p.parent).collect::<Vec<_>>(),
            [
                None,
                None,
                None,
                None,
                None,
                Some(4),
                Some(4),
                Some(6),
                Some(6)
            ]
        );
        let PropertyDetailValue::Bounded {
            lower: Some(lower),
            upper: Some(upper),
            set_point,
            unit: Some(unit),
        } = &p[3].value
        else {
            panic!()
        };
        assert_eq!(lower.value, Literal::Real(10.));
        assert_eq!(upper.value, Literal::Real(20.));
        assert_eq!(
            set_point.as_ref().map(|v| &v.value),
            if version == 0 {
                None
            } else {
                Some(&Literal::Real(15.))
            }
        );
        assert_eq!(unit.prefix.as_deref(), Some("MILLI"));
        assert_eq!(p[5].value, p[8].value);
        let PropertyDetailValue::Bounded {
            upper,
            lower: Some(lower),
            unit: Some(unit),
            ..
        } = &p[7].value
        else {
            panic!()
        };
        assert!(upper.is_none());
        assert_eq!(lower.value, Literal::Real(-3.));
        assert!(unit.prefix.is_none());
        let hash = Hash::element(&b, element);
        let mut changed = details.clone();
        for set in &mut changed.sets {
            set.source_instance = Some(9000);
            for p in &mut set.properties {
                p.source_instance = Some(9001);
            }
        }
        b.components
            .property_details
            .insert(element, changed.clone());
        assert_eq!(Hash::element(&b, element), hash);
        changed.sets[0].properties[6].parent = Some(0);
        b.components.property_details.insert(element, changed);
        assert_ne!(Hash::element(&b, element), hash);
        b.components.property_details.remove(element);
        let mut simple = lift(&read(&property_values(version)));
        let simple_element = at(&simple, 102);
        simple.components.property_details.remove(simple_element);
        assert_eq!(
            Hash::element(&b, element),
            Hash::element(&simple, simple_element)
        );
        let renamed = src
            .replace("#520", "#1520")
            .replace("#521", "#1521")
            .replace("#522", "#1522")
            .replace("#523", "#1523")
            .replace("#524", "#1524");
        let renamed = lift(&read(&renamed));
        assert_eq!(Hash::element(&renamed, at(&renamed, 102)), hash);
    }
}

#[test]
fn structured_property_cycles_and_depth_limits_keep_locations_and_restore_depth() {
    for version in 0..3 {
        let src = structured_values(version);
        let cyclic = src.replace("(#524,#522)", "(#524,#522,#521)");
        let (b, d, budget) = with(Limits::default(), |cx| {
            Lift::<Building>::lift(&read(&cyclic), cx)
        });
        assert!(b.is_ok());
        assert_eq!(budget.used(Kind::Depth), 0);
        let d = d
            .iter()
            .find(|d| d.code() == codes::IFC_ATTRIBUTE_INVALID)
            .unwrap();
        assert_eq!(
            d.places()[0].bytes().unwrap().start,
            cyclic.find("#523=").unwrap() as u64
        );
        let file = read(&src);
        let (r, _, budget) = with(
            Limits::default().with(Kind::Depth, Limit::Finite(1)),
            |cx| Lift::<Building>::lift(&file, cx),
        );
        assert!(matches!(r, Err(Error::Stopped(_))));
        assert_eq!(budget.used(Kind::Depth), 0);
        let bad = src.replace("IFCLENGTHMEASURE(10.),$", "IFCLENGTHMEASURE(10.),'unit'");
        let (b, d, _) = with(Limits::default(), |cx| {
            Lift::<Building>::lift(&read(&bad), cx)
        });
        assert!(d.iter().any(|d| d.code() == codes::IFC_ATTRIBUTE_INVALID));
        let b = b.unwrap();
        let value =
            &b.components.property_details.get(at(&b, 102)).unwrap().sets[0].properties[3].value;
        assert!(matches!(
            value,
            zumen_building::PropertyDetailValue::Bounded { unit: None, .. }
        ));
    }
}

#[test]
fn list_and_enumerated_properties_keep_types_order_and_explicit_units() {
    for version in 0..3 {
        let file = read(&property_values(version));
        let (b, d, _) = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx));
        let b = b.unwrap();
        let values = &b.components.properties.get(at(&b, 102)).unwrap().sets[1].properties;
        assert_eq!(values.len(), 3);
        assert_eq!(
            values[0].value.as_ref().unwrap().type_name.as_deref(),
            Some("IfcLengthMeasure")
        );
        assert_eq!(
            values[0].value.as_ref().unwrap().value,
            Literal::List(vec![Literal::Real(10.), Literal::Real(20.)])
        );
        assert_eq!(
            values[0].unit.as_ref().unwrap().prefix.as_deref(),
            Some("MILLI")
        );
        assert_eq!(
            values[1].value.as_ref().unwrap().value,
            Literal::List(vec![Literal::Real(20.)])
        );
        assert_eq!(values[1].unit.as_ref().unwrap().prefix, None);
        assert_eq!(
            values[2].value.as_ref().unwrap().value,
            Literal::List(vec![Literal::Boolean(true), Literal::Boolean(false)])
        );
        assert_eq!(values[2].unit, None);
        assert!(d.is_empty(), "{d:?}");
        let details = &b.components.property_details.get(at(&b, 102)).unwrap().sets[0];
        assert_eq!(details.name, "Values");
        let zumen_building::PropertyDetailValue::Enumerated {
            choices: Some(choices),
            ..
        } = &details.properties[1].value
        else {
            panic!()
        };
        assert_eq!(choices.name, "Allowed");
        assert_eq!(choices.values.len(), 2);
        assert_eq!(choices.unit.as_ref().unwrap().prefix, None);
        assert_eq!(choices.source_instance, Some(510));
    }
}

#[test]
fn malformed_lists_and_enumeration_references_do_not_guess_values_or_units() {
    for version in 0..3 {
        for replacement in [
            "()",
            "(IFCLENGTHMEASURE(10.),IFCLABEL('mixed'))",
            "(IFCLENGTHMEASURE(1.E999))",
        ] {
            let src = property_values(version).replace(
                "(IFCLENGTHMEASURE(10.),IFCLENGTHMEASURE(20.)),$",
                &format!("{replacement},$"),
            );
            let (b, d, _) = with(Limits::default(), |cx| {
                Lift::<Building>::lift(&read(&src), cx)
            });
            let b = b.unwrap();
            assert!(
                b.components.properties.get(at(&b, 102)).unwrap().sets[1].properties[0]
                    .value
                    .is_none()
            );
            assert_eq!(
                d.iter()
                    .filter(|d| d.code() == codes::IFC_ATTRIBUTE_INVALID)
                    .map(|d| d.count())
                    .sum::<u64>(),
                1
            );
        }
        let src = property_values(version).replace(
            "(IFCLENGTHMEASURE(20.)),#510",
            "(IFCLENGTHMEASURE(20.)),#500",
        );
        let (b, d, _) = with(Limits::default(), |cx| {
            Lift::<Building>::lift(&read(&src), cx)
        });
        let b = b.unwrap();
        assert!(
            b.components.properties.get(at(&b, 102)).unwrap().sets[1].properties[1]
                .unit
                .is_none()
        );
        assert!(d.iter().any(|d| d.code() == codes::IFC_ATTRIBUTE_INVALID));
        for (before, after) in [
            ("IFCLENGTHMEASURE(20.)),$", "IFCLENGTHMEASURE(20.)),'unit'"),
            (
                "IFCLENGTHMEASURE(20.)),#511",
                "IFCLENGTHMEASURE(20.)),'unit'",
            ),
        ] {
            let (b, d, _) = with(Limits::default(), |cx| {
                Lift::<Building>::lift(&read(&property_values(version).replace(before, after)), cx)
            });
            let b = b.unwrap();
            let index = usize::from(before.ends_with("#511"));
            assert!(
                b.components.properties.get(at(&b, 102)).unwrap().sets[1].properties[index]
                    .unit
                    .is_none()
            );
            assert_eq!(
                d.iter()
                    .filter(|d| d.code() == codes::IFC_ATTRIBUTE_INVALID)
                    .map(|d| d.count())
                    .sum::<u64>(),
                1
            );
        }
    }
}

#[test]
fn material_sets_keep_member_order_thickness_and_material_properties() {
    for version in 0..3 {
        let file = read(&materials(version));
        let (b, d, _) = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx));
        let b = b.unwrap();
        assert!(!d.iter().any(|d| d.code() == codes::IFC_ATTRIBUTE_INVALID));
        let layered = b.components.material.get(at(&b, 450)).unwrap();
        assert_eq!(layered.kind, MaterialKind::Layers);
        assert_eq!(layered.direction.as_deref(), Some("AXIS2"));
        assert_eq!(layered.offset.as_ref().unwrap().value, -5.);
        assert_eq!(
            layered
                .parts
                .iter()
                .map(|p| p.thickness.as_ref().unwrap().value)
                .collect::<Vec<_>>(),
            [10., 20.]
        );
        assert_eq!(
            layered
                .parts
                .iter()
                .map(|p| p.material.unwrap())
                .collect::<Vec<_>>(),
            [at(&b, 400), at(&b, 401)]
        );
        let p = &b.components.properties.get(at(&b, 400)).unwrap().sets[0];
        assert_eq!(p.name, "Thermal");
        assert_eq!(p.properties[0].name, "Rating");
        assert_eq!(
            p.properties[0].value.as_ref().unwrap().value,
            Literal::Text("rated".into())
        );
        if version > 0 {
            for (id, kind, names) in [
                (451, MaterialKind::Constituents, ["First", "Second"]),
                (452, MaterialKind::Profiles, ["Rect", "Round"]),
            ] {
                let m = b.components.material.get(at(&b, id)).unwrap();
                assert_eq!(m.kind, kind);
                assert_eq!(
                    m.parts
                        .iter()
                        .map(|p| p.name.as_deref().unwrap())
                        .collect::<Vec<_>>(),
                    names
                );
                assert_eq!(
                    m.parts
                        .iter()
                        .map(|p| p.material.unwrap())
                        .collect::<Vec<_>>(),
                    [at(&b, 400), at(&b, 401)]
                );
                assert!(m.parts.iter().all(|p| p.thickness.is_none()));
            }
            for id in [451, 452] {
                let (g, d, _) = with(Limits::default(), |cx| file.owned_geometry(id, cx));
                assert_eq!(d.iter().count(), 2);
                assert!(
                    d.iter().all(|d| d.code() == codes::IFC_SHAPE_MISSING),
                    "{d:?}"
                );
                let g = g.unwrap();
                let Some(Acquired::Value(m)) = g.materials() else {
                    panic!()
                };
                assert_eq!(m.assignments.len(), 1);
                assert_eq!(m.assignments[0].members.len(), 2);
                assert_eq!(
                    m.assignments[0]
                        .members
                        .iter()
                        .map(|m| m.source.instance.unwrap())
                        .collect::<Vec<_>>(),
                    [400, 401]
                );
            }
        }
    }
}

#[test]
fn owned_material_sets_distinguish_wrong_members_duplicates_and_omitted_materials() {
    for version in 1..3 {
        for (before, after) in [
            ("(#441,#442)", "(#441,#441)"),
            ("(#441,#442)", "(#400,#442)"),
        ] {
            let file = read(&materials(version).replace(before, after));
            let (result, diag, _) = with(Limits::default(), |cx| file.owned_geometry(451, cx));
            assert!(matches!(result, Err(Error::Invalid)));
            assert!(diag.iter().any(|d| d.code() == codes::IFC_SHAPE_INVALID));
        }
        let file = read(&materials(version).replace("'Round',$,#401,#447", "'Round',$,$,#447"));
        let g = with(Limits::default(), |cx| file.owned_geometry(452, cx))
            .0
            .unwrap();
        let Some(Acquired::Value(m)) = g.materials() else {
            panic!()
        };
        assert!(m.assignments[0].members[1].presentation.is_none());
        let file = read(&materials(version).replace("'Mix',$,(#441,#442)", "'Mix',$,$"));
        let b = lift(&file);
        assert!(
            b.components
                .material
                .get(at(&b, 451))
                .unwrap()
                .parts
                .is_empty()
        );
        let g = with(Limits::default(), |cx| file.owned_geometry(451, cx))
            .0
            .unwrap();
        let Some(Acquired::Value(m)) = g.materials() else {
            panic!()
        };
        assert!(m.assignments[0].members.is_empty());
    }
    let file = read(&materials(1).replace("FILE_SCHEMA(('IFC4'))", "FILE_SCHEMA(('IFC2X3'))"));
    let g = with(Limits::default(), |cx| file.owned_geometry(451, cx))
        .0
        .unwrap();
    let Some(Acquired::Value(m)) = g.materials() else {
        panic!()
    };
    assert!(matches!(
        m.assignments[0].members[0].presentation,
        Some(Acquired::Unsupported)
    ));
}

#[test]
fn predefined_door_and_window_sets_keep_values_units_types_and_origins() {
    for version in 0..3 {
        let file = read(&doors(version));
        let (b, d, _) = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx));
        let b = b.unwrap();
        assert!(d.is_empty(), "{version}: {d:?}");
        for (id, counts) in [
            (110, [if version == 0 { 10 } else { 12 }, 4]),
            (111, [if version == 0 { 8 } else { 11 }, 4]),
        ] {
            let p = b.components.properties.get(at(&b, id)).unwrap();
            assert_eq!(p.sets.len(), 2);
            for (set, count) in p.sets.iter().zip(counts) {
                assert_eq!(set.properties.len(), count);
                for property in &set.properties {
                    let v = property.value.as_ref().unwrap();
                    if v.type_name.as_ref().unwrap().ends_with("LengthMeasure") {
                        let unit = property.unit.as_ref().unwrap();
                        assert_eq!(unit.kind, "LENGTHUNIT");
                        assert_eq!(unit.prefix.as_deref(), Some("MILLI"));
                    } else {
                        assert!(property.unit.is_none());
                    }
                }
            }
        }
        let p = &b.components.properties.get(at(&b, 110)).unwrap().sets[0].properties;
        assert_eq!(p[1].name, "LiningThickness");
        assert_eq!(p[1].value.as_ref().unwrap().value, Literal::Real(0.));
        assert_eq!(p[6].value.as_ref().unwrap().value, Literal::Real(-1.));
        assert_eq!(
            p[1].value.as_ref().unwrap().type_name.as_deref(),
            Some(if version == 0 {
                "IfcPositiveLengthMeasure"
            } else {
                "IfcNonNegativeLengthMeasure"
            })
        );
        let panel = &b.components.properties.get(at(&b, 110)).unwrap().sets[1].properties;
        assert_eq!(
            panel[1].value.as_ref().unwrap().value,
            Literal::Enumeration("SWINGING".into())
        );
        assert_eq!(panel[2].value.as_ref().unwrap().value, Literal::Real(0.5));
        assert_eq!(
            b.components.properties.get(at(&b, 102)).unwrap().sets[0],
            b.components.properties.get(at(&b, 110)).unwrap().sets[0]
        );
        assert!(
            b.element(at(&b, 102))
                .unwrap()
                .source()
                .relations
                .iter()
                .any(|r| r.instance == 302 && r.position == Some(0))
        );
        assert_eq!(
            b.components.r#type.get(at(&b, 100)).unwrap().target,
            at(&b, 110)
        );
    }
}

#[test]
fn predefined_sets_distinguish_omission_invalid_values_and_unknown_subtypes() {
    for version in 0..3 {
        let src = doors(version);
        let changed = src.replace("1.,0.,2.,0.,3.", "$,'bad',2.,0.,3.");
        let (b, d, _) = with(Limits::default(), |cx| {
            Lift::<Building>::lift(&read(&changed), cx)
        });
        let b = b.unwrap();
        let p = &b.components.properties.get(at(&b, 110)).unwrap().sets[0].properties;
        assert!(
            !p.iter()
                .any(|p| matches!(p.name.as_str(), "LiningDepth" | "LiningThickness"))
        );
        assert!(d.iter().any(|d| d.code() == codes::IFC_ATTRIBUTE_INVALID));
        let lower = src.replace("IFCDOORLININGPROPERTIES", "ifcdoorliningproperties");
        assert_eq!(lift(&read(&lower)), lift(&read(&src)));
        let future = src.replace("IFCDOORLININGPROPERTIES", "IFCDOORLININGPROPERTIESFUTURE");
        let (b, d, _) = with(Limits::default(), |cx| {
            Lift::<Building>::lift(&read(&future), cx)
        });
        let b = b.unwrap();
        assert_eq!(
            b.components.properties.get(at(&b, 110)).unwrap().sets.len(),
            1
        );
        assert!(d.iter().any(|d| d.code() == codes::IFC_ENTITY_DROPPED));
    }
}

#[test]
fn metadata_keeps_hashes_across_record_order_and_stops_at_shared_budget_boundaries() {
    for version in 0..3 {
        let src = all_metadata(version);
        let file = read(&src);
        let (b, d, used) = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx));
        let b = b.unwrap();
        assert!(d.iter().all(|d| d.code() == codes::IFC_ENTITY_DROPPED));
        let (prefix, records) = src.split_once("DATA;\n").unwrap();
        let records = records
            .trim_end()
            .strip_suffix("ENDSEC;END-ISO-10303-21;")
            .unwrap();
        // 関係が作るプロパティセットとリンクの列の順は保ち、参照先の定義だけを並べ替える。
        let (relations, definitions): (Vec<_>, Vec<_>) =
            records.lines().partition(|line| line.contains("=IFCREL"));
        let reordered = format!(
            "{prefix}DATA;\n{}\n{}\nENDSEC;END-ISO-10303-21;\n",
            definitions.into_iter().rev().collect::<Vec<_>>().join("\n"),
            relations.join("\n")
        );
        let other = lift(&read(&reordered));
        for e in b.elements() {
            assert_eq!(
                Hash::element(&b, b.find(e.id()).unwrap()),
                Hash::element(&other, other.find(e.id()).unwrap())
            );
        }
        for kind in [Kind::Work, Kind::Elements, Kind::DecodedBytes] {
            let cap = used.used(kind) - 1;
            let (r, _, budget) = with(Limits::default().with(kind, Limit::Finite(cap)), |cx| {
                Lift::<Building>::lift(&file, cx)
            });
            assert!(matches!(r, Err(Error::Stopped(_))), "{version}/{kind:?}");
            assert_eq!(budget.used(Kind::Depth), 0);
        }
        let (r, _, _) = with(Limits::default(), |cx| {
            cx.cancel.cancel();
            Lift::<Building>::lift(&file, cx)
        });
        assert!(matches!(r, Err(Error::Stopped(_))));
    }
}

#[test]
#[allow(clippy::disallowed_methods)] // 合成入力の照合の出力先だけを読む。
fn exports_metadata_for_other_readers() {
    let Ok(dir) = std::env::var("ZUMEN_IFC_METADATA_OUT") else {
        return;
    };
    let dir = std::path::Path::new(&dir);
    std::fs::create_dir_all(dir).unwrap();
    for (version, name) in VERSIONS.iter().enumerate() {
        let src = all_metadata(version);
        let file = read(&src);
        let (b, d, budget) = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx));
        let b = b.unwrap();
        let mut values = Writer::new();
        values.begin_object();
        values.key("properties");
        values.begin_array();
        for (element, sets) in b.components.properties.iter() {
            for set in &sets.sets {
                for property in &set.properties {
                    values.begin_object();
                    values.key("owner");
                    values.uint(b.element(element).unwrap().source().instance.unwrap());
                    values.key("set");
                    values.string(&set.name);
                    values.key("name");
                    values.string(&property.name);
                    values.key("value");
                    write_literal(&mut values, &property.value.as_ref().unwrap().value);
                    values.key("unit");
                    write_unit(&mut values, property.unit.as_ref());
                    values.end_object();
                }
            }
        }
        values.end_array();
        values.key("details");
        values.begin_array();
        for (_, details) in b.components.property_details.iter() {
            for set in &details.sets {
                for property in &set.properties {
                    values.begin_object();
                    values.key("source");
                    values.uint(property.source_instance.unwrap());
                    values.key("parent");
                    if let Some(index) = property.parent {
                        values.uint(set.properties[index as usize].source_instance.unwrap());
                    } else {
                        values.null();
                    }
                    use zumen_building::PropertyDetailValue as P;
                    match &property.value {
                        P::Single { value, unit } => {
                            values.key("value");
                            write_typed(&mut values, value.as_ref());
                            values.key("unit");
                            write_unit(&mut values, unit.as_ref());
                        }
                        P::List { values: list, unit } => {
                            values.key("values");
                            write_values(&mut values, list.as_deref());
                            values.key("unit");
                            write_unit(&mut values, unit.as_ref());
                        }
                        P::Enumerated {
                            values: list,
                            choices,
                        } => {
                            values.key("values");
                            write_values(&mut values, list.as_deref());
                            if let Some(choices) = choices {
                                values.key("choices");
                                write_values(&mut values, Some(&choices.values));
                                values.key("choiceName");
                                values.string(&choices.name);
                                values.key("unit");
                                write_unit(&mut values, choices.unit.as_ref());
                            }
                        }
                        P::Bounded {
                            lower,
                            upper,
                            set_point,
                            unit,
                        } => {
                            for (name, value) in
                                [("lower", lower), ("upper", upper), ("setPoint", set_point)]
                            {
                                values.key(name);
                                write_typed(&mut values, value.as_ref());
                            }
                            values.key("unit");
                            write_unit(&mut values, unit.as_ref());
                        }
                        P::Complex { usage_name } => {
                            values.key("usage");
                            values.string(usage_name);
                        }
                        P::Unsupported { .. } => panic!(),
                    }
                    values.end_object();
                }
            }
        }
        values.end_array();
        values.key("materials");
        values.begin_array();
        for (element, material) in b.components.material.iter() {
            values.begin_object();
            values.key("owner");
            values.uint(source_id(&b, element));
            values.key("direction");
            if let Some(s) = &material.direction {
                values.string(s);
            } else {
                values.null();
            }
            values.key("offset");
            if let Some(v) = &material.offset {
                values.float(v.value);
            } else {
                values.null();
            }
            values.key("parts");
            values.begin_array();
            for part in &material.parts {
                values.begin_object();
                values.key("material");
                if let Some(e) = part.material {
                    values.uint(source_id(&b, e));
                } else {
                    values.null();
                }
                values.key("name");
                if let Some(s) = &part.name {
                    values.string(s);
                } else {
                    values.null();
                }
                values.key("thickness");
                if let Some(v) = &part.thickness {
                    values.float(v.value);
                } else {
                    values.null();
                }
                values.end_object();
            }
            values.end_array();
            values.end_object();
        }
        values.end_array();
        values.key("connections");
        values.begin_array();
        for (element, connection) in b.components.connection.iter() {
            for link in &connection.links {
                values.begin_array();
                values.uint(source_id(&b, element));
                values.string(&link.kind);
                values.uint(source_id(&b, link.target));
                values.end_array();
            }
        }
        values.end_array();
        values.end_object();
        let mut contract = format!("{b:?}\n{d:?}\n");
        contract.push_str(&format!("document={:?}\n", Hash::document_values(&b)));
        for e in b.elements() {
            contract.push_str(&format!(
                "{}={:?}\n",
                e.id(),
                Hash::element(&b, b.find(e.id()).unwrap())
            ));
        }
        for k in Kind::ALL {
            contract.push_str(&format!("{k:?}={};", budget.used(k)))
        }
        for kind in [Kind::Work, Kind::Elements, Kind::DecodedBytes] {
            for limit in [0, budget.used(kind) - 1, budget.used(kind)] {
                let (result, diag, budget) =
                    with(Limits::default().with(kind, Limit::Finite(limit)), |cx| {
                        Lift::<Building>::lift(&file, cx)
                    });
                contract.push_str(&format!(
                    "\n{kind:?}/{limit}: {:?}/{diag:?}/{:?}\n",
                    result.map(|b| Hash::document_values(&b)),
                    Kind::ALL.map(|k| budget.used(k))
                ));
            }
        }
        for expire in [false, true] {
            let (result, diag, budget) = with(Limits::default(), |cx| {
                if expire {
                    cx.cancel.expire();
                } else {
                    cx.cancel.cancel();
                }
                Lift::<Building>::lift(&file, cx)
            });
            contract.push_str(&format!(
                "{result:?}/{diag:?}/{:?}\n",
                Kind::ALL.map(|k| budget.used(k))
            ));
        }
        std::fs::write(dir.join(format!("{name}.ifc")), src).unwrap();
        std::fs::write(dir.join(format!("{name}.json")), values.into_bytes()).unwrap();
        std::fs::write(dir.join(format!("{name}.txt")), contract).unwrap();
    }
}

fn source_id(b: &Building, element: ElementRef) -> u64 {
    b.element(element).unwrap().source().instance.unwrap()
}
fn write_literal(w: &mut Writer, v: &Literal) {
    match v {
        Literal::Real(v) => w.float(*v),
        Literal::Integer(v) => w.int(*v),
        Literal::Text(v) | Literal::Enumeration(v) => w.string(v),
        Literal::Boolean(v) => w.bool(*v),
        Literal::Logical(v) => {
            if let Some(v) = v {
                w.bool(*v)
            } else {
                w.null()
            }
        }
        Literal::List(v) => {
            w.begin_array();
            for v in v {
                write_literal(w, v);
            }
            w.end_array();
        }
        Literal::Element(_) => panic!(),
    }
}
fn write_typed(w: &mut Writer, v: Option<&zumen_building::TypedValue>) {
    if let Some(v) = v {
        write_literal(w, &v.value);
    } else {
        w.null();
    }
}
fn write_values(w: &mut Writer, v: Option<&[zumen_building::TypedValue]>) {
    if let Some(v) = v {
        w.begin_array();
        for v in v {
            write_literal(w, &v.value);
        }
        w.end_array();
    } else {
        w.null();
    }
}
fn write_unit(w: &mut Writer, unit: Option<&zumen_building::Unit>) {
    if let Some(u) = unit {
        w.begin_array();
        w.string(&u.kind);
        if let Some(p) = &u.prefix {
            w.string(p);
        } else {
            w.null();
        }
        w.string(&u.name);
        if let Some(s) = u.scale {
            w.float(s);
        } else {
            w.null();
        }
        w.end_array();
    } else {
        w.null();
    }
}
