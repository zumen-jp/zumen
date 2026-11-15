//! 自作の 3 版の代表要素から、共有写像を通して回転体の元値を取得する。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{AreaProfileKind, Building, ElementRef, Hash, Unit};
use zumen_ifc::{Ifc, IfcFile, ProductShape, RevolvedArea, ShapeNode};
use zumen_json::Writer;
use zumen_sdk::{Codec, Error, Lift};

#[path = "support/curve_json.rs"]
mod curve_json;

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];
fn source(version: &str) -> String {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('revolutions'),'2;1');\nFILE_NAME('revolutions','',(),(),'','','');\nFILE_SCHEMA(('{version}'));\nENDSEC;\nDATA;\n\
#1=IFCPROJECT('0000000000000000000001',$,$,$,$,$,$,$,#2);\n\
#2=IFCUNITASSIGNMENT((#3,#4));\n#3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n\
#4=IFCCONVERSIONBASEDUNIT(#7,.PLANEANGLEUNIT.,'degree',#5);\n\
#5=IFCMEASUREWITHUNIT(IFCPLANEANGLEMEASURE(0.017453292519943295),#6);\n\
#6=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);\n#7=IFCDIMENSIONALEXPONENTS(0,0,0,0,0,0,0);\n\
#8=IFCGEOMETRICREPRESENTATIONCONTEXT('Model','Model',3,0.00001,#20,$);\n\
#10=IFCCARTESIANPOINT((2.,3.));\n#11=IFCDIRECTION((0.,4.));\n#12=IFCAXIS2PLACEMENT2D(#10,#11);\n\
#13=IFCRECTANGLEPROFILEDEF(.AREA.,'Rectangle',#12,15.,5.);\n#14=IFCCIRCLEPROFILEDEF(.AREA.,'Circle',#12,2.5);\n\
#15=IFCISHAPEPROFILEDEF(.AREA.,'I shape',#12,15.,20.,1.,2.,0.5{flange});\n\
#16=IFCRECTANGLEHOLLOWPROFILEDEF(.AREA.,'Hollow rectangle',#12,15.,5.,1.,0.25,0.5);\n\
#17=IFCARBITRARYCLOSEDPROFILEDEF(.AREA.,'Triangle',#18);\n#18=IFCPOLYLINE((#10,#51,#52,#10));\n\
#20=IFCAXIS2PLACEMENT3D(#21,#22,#23);\n#21=IFCCARTESIANPOINT((10.,20.,30.));\n\
#22=IFCDIRECTION((0.,0.,2.));\n#23=IFCDIRECTION((3.,0.,0.));\n\
#25=IFCAXIS1PLACEMENT(#26,#27);\n#26=IFCCARTESIANPOINT((-20.,0.,0.));\n#27=IFCDIRECTION((0.,2.,0.));\n\
#30=IFCREVOLVEDAREASOLID(#13,#20,#25,180.);\n#31=IFCREVOLVEDAREASOLID(#14,#20,#25,180.);\n\
#32=IFCREVOLVEDAREASOLID(#15,#20,#25,180.);\n#33=IFCREVOLVEDAREASOLID(#16,#20,#25,180.);\n#34=IFCREVOLVEDAREASOLID(#17,#20,#25,180.);\n\
#40=IFCMAPPEDITEM(#41,#43);\n#41=IFCREPRESENTATIONMAP(#20,#42);\n\
#42=IFCSHAPEREPRESENTATION(#8,'Body','SweptSolid',(#30,#31,#32,#33,#34));\n\
#43=IFCCARTESIANTRANSFORMATIONOPERATOR3D($,$,#21,2.,$);\n#44=IFCMAPPEDITEM(#41,#43);\n\
#51=IFCCARTESIANPOINT((3.,3.));\n#52=IFCCARTESIANPOINT((2.,4.));\n\
#100=IFCBEAM('0000000000000000000002',$,$,$,$,$,#101,${extra});\n\
#101=IFCPRODUCTDEFINITIONSHAPE($,$,(#102));\n#102=IFCSHAPEREPRESENTATION(#8,'Body','MappedRepresentation',(#40,#44));\n\
ENDSEC;\nEND-ISO-10303-21;\n",
        flange = if version == "IFC2X3" { "" } else { ",0.25,5." },
        extra = if version == "IFC2X3" {
            ""
        } else {
            ",.NOTDEFINED."
        }
    )
}
fn with<T>(limits: Limits, f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics, Budget) {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let value = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (value, diag, budget)
}
fn read(src: &str) -> IfcFile {
    with(Limits::default(), |cx| Ifc::read(src.as_bytes(), cx))
        .0
        .unwrap()
}
fn product(file: &IfcFile) -> ProductShape<'_> {
    with(Limits::default(), |cx| file.product_shape(100, cx))
        .0
        .unwrap()
}
fn leaf<'s, 'f>(shape: &'s ProductShape<'f>, id: u64) -> &'s ShapeNode<'f> {
    shape
        .nodes
        .iter()
        .find(|n| n.source.id() == Some(id))
        .unwrap()
}
fn value(file: &IfcFile, id: u64) -> RevolvedArea<'_> {
    let shape = product(file);
    let (result, diag, _) = with(Limits::default(), |cx| {
        file.revolved_area(leaf(&shape, id), cx)
    });
    assert!(diag.is_empty(), "{diag:?}");
    result.unwrap()
}

#[test]
fn gets_five_profiles_raw_axes_angles_units_and_sources_through_shared_maps() {
    for version in VERSIONS {
        let file = read(&source(version));
        let shape = product(&file);
        for id in 30..=34 {
            assert_eq!(
                shape
                    .nodes
                    .iter()
                    .filter(|n| n.source.id() == Some(id))
                    .count(),
                1
            );
            let result = value(&file, id);
            assert!(std::ptr::eq(
                result.source.solid,
                file.instance(id).unwrap()
            ));
            assert!(std::ptr::eq(
                result.source.profile,
                file.instance(id - 17).unwrap()
            ));
            assert!(
                result
                    .source
                    .records
                    .iter()
                    .all(|r| std::ptr::eq(*r, file.instance(r.id().unwrap()).unwrap()))
            );
            for at in [1, 2, 3, 4, 5, 6, 7, 20, 21, 22, 23, 25, 26, 27] {
                assert!(result.source.records.iter().any(|r| r.id() == Some(at)));
            }
            assert_eq!(result.definition.axis.location, [-20., 0., 0.]);
            assert_eq!(result.definition.axis.direction, [0., 2., 0.]);
            assert_eq!(
                result.definition.position.as_ref().unwrap().location,
                [10., 20., 30.]
            );
            assert_eq!(result.definition.angle.value, 180.);
            assert_eq!(
                result.definition.angle.unit.as_ref().unwrap().name,
                "degree"
            );
            assert_eq!(
                result.definition.angle.unit.as_ref().unwrap().scale,
                Some(0.017453292519943295)
            );
            assert_eq!(
                result.length_unit.as_ref().unwrap().prefix.as_deref(),
                Some("MILLI")
            );
            assert_eq!(result.boundary.is_some(), id == 34);
            assert!(match result.definition.profile.kind {
                AreaProfileKind::Rectangle { x_dim, y_dim } =>
                    id == 30 && x_dim == 15. && y_dim == 5.,
                AreaProfileKind::Circle { radius } => id == 31 && radius == 2.5,
                AreaProfileKind::IShape { overall_width, .. } => id == 32 && overall_width == 15.,
                AreaProfileKind::RectangleHollow { wall_thickness, .. } =>
                    id == 33 && wall_thickness == 1.,
                AreaProfileKind::ArbitraryClosed { boundary } =>
                    id == 34 && result.boundary.as_ref().unwrap().root == boundary,
            });
        }
    }
}

#[test]
fn checks_angles_in_the_declared_unit_and_keeps_omission_and_signed_zero() {
    for version in VERSIONS {
        for angle in ["0.", "-0.", "360."] {
            let file = read(&source(version).replace(",180.)", &format!(",{angle})")));
            assert_eq!(
                value(&file, 30).definition.angle.value.to_bits(),
                angle.parse::<f64>().unwrap().to_bits()
            );
        }
        let src = source(version).replace("(#3,#4)", "(#3,#6)");
        let radian = read(&src.replace(",180.)", ",6.283185307179586)"));
        assert_eq!(
            value(&radian, 30).definition.angle.value,
            core::f64::consts::TAU
        );
        let mut milliradian = src.replace(
            ".PLANEANGLEUNIT.,$,.RADIAN.",
            ".PLANEANGLEUNIT.,.MILLI.,.RADIAN.",
        );
        milliradian = milliradian.replace(",180.)", ",6000.)");
        assert_eq!(value(&read(&milliradian), 30).definition.angle.value, 6000.);
        let absent = read(
            &source(version)
                .replace("(#3,#4)", "(#3)")
                .replace(",180.)", ",720.)"),
        );
        let shape = product(&absent);
        let (result, diag, _) = with(Limits::default(), |cx| {
            absent.revolved_area(leaf(&shape, 30), cx)
        });
        let result = result.unwrap();
        assert_eq!(result.definition.angle.value, 720.);
        assert!(result.definition.angle.unit.is_none());
        assert!(diag.iter().any(|d| d.code() == codes::IFC_UNIT_MISSING));
        let absent_place = read(&source(version).replace("#13,#20,#25", "#13,$,#25"));
        let shape = product(&absent_place);
        let result = with(Limits::default(), |cx| {
            absent_place.revolved_area(leaf(&shape, 30), cx)
        })
        .0;
        if version == "IFC2X3" {
            assert!(matches!(result, Err(Error::Invalid)));
        } else {
            assert!(result.unwrap().definition.position.is_none());
        }
    }
}

fn invalid_inputs() -> Vec<(u64, &'static str, &'static str)> {
    vec![
        (30, ",180.)", ",-1.)"),
        (30, ",180.)", ",361.)"),
        (30, ",180.)", ",1.E400)"),
        (30, ",180.)", ",$)"),
        (30, ",180.)", ",180.,$)"),
        (30, "IFCAXIS1PLACEMENT(#26,#27)", "IFCAXIS1PLACEMENT(#26,$)"),
        (
            30,
            "IFCAXIS1PLACEMENT(#26,#27)",
            "IFCAXIS1PLACEMENT(#25,#27)",
        ),
        (
            30,
            "IFCAXIS1PLACEMENT(#26,#27)",
            "IFCAXIS1PLACEMENT(#26,#999)",
        ),
        (
            30,
            "IFCCARTESIANPOINT((-20.,0.,0.))",
            "IFCCARTESIANPOINT((-20.,0.,1.))",
        ),
        (
            30,
            "IFCCARTESIANPOINT((-20.,0.,0.))",
            "IFCCARTESIANPOINT((-20.,0.))",
        ),
        (30, "IFCDIRECTION((0.,2.,0.))", "IFCDIRECTION((0.,2.,1.))"),
        (30, "IFCDIRECTION((0.,2.,0.))", "IFCDIRECTION((0.,0.,0.))"),
        (30, "IFCDIRECTION((0.,2.,0.))", "IFCDIRECTION((0.,2.))"),
        (
            30,
            "IFCDIMENSIONALEXPONENTS(0,0,0,0,0,0,0)",
            "IFCDIMENSIONALEXPONENTS(1,0,0,0,0,0,0)",
        ),
        (30, "(#3,#4)", "(#3,#4,#6)"),
        (30, "0.017453292519943295),#6", "0.017453292519943295),#4"),
        (
            34,
            "IFCPOLYLINE((#10,#51,#52,#10))",
            "IFCPOLYLINE((#10,#51,#52))",
        ),
        (
            34,
            "#17=IFCARBITRARYCLOSEDPROFILEDEF(.AREA.",
            "#17=IFCARBITRARYCLOSEDPROFILEDEF(.CURVE.",
        ),
    ]
}

#[test]
fn rejects_bad_axis_angle_units_profiles_and_references_without_partial_results() {
    for version in VERSIONS {
        for (id, from, to) in invalid_inputs() {
            let file = read(&source(version).replace(from, to));
            let shape = product(&file);
            let (result, diag, budget) = with(Limits::default(), |cx| {
                file.revolved_area(leaf(&shape, id), cx)
            });
            assert!(result.is_err(), "{version} {to}");
            assert!(!diag.is_empty());
            assert_eq!(budget.used(Kind::Depth), 0);
        }
        let file =
            read(&source(version).replace("IFCREVOLVEDAREASOLID", "IFCREVOLVEDAREASOLIDTAPERED"));
        let shape = product(&file);
        let (result, diag, _) = with(Limits::default(), |cx| {
            file.revolved_area(leaf(&shape, 30), cx)
        });
        assert!(matches!(result, Err(Error::Unsupported)));
        assert!(format!("{diag:?}").contains("IFCREVOLVEDAREASOLIDTAPERED"));
        let other = read(&source(version));
        assert!(matches!(
            with(Limits::default(), |cx| other
                .revolved_area(leaf(&shape, 30), cx))
            .0,
            Err(Error::Invalid)
        ));
    }
}

#[test]
fn rejects_single_part_complex_solids_profiles_and_axes_without_guessing_layouts() {
    for version in VERSIONS {
        for (from, to) in [
            (
                "#30=IFCREVOLVEDAREASOLID(#13,#20,#25,180.);",
                "#30=(IFCREVOLVEDAREASOLID(#13,#20,#25,180.));",
            ),
            (
                "#13=IFCRECTANGLEPROFILEDEF(.AREA.,'Rectangle',#12,15.,5.);",
                "#13=(IFCRECTANGLEPROFILEDEF(.AREA.,'Rectangle',#12,15.,5.));",
            ),
            (
                "#25=IFCAXIS1PLACEMENT(#26,#27);",
                "#25=(IFCAXIS1PLACEMENT(#26,#27));",
            ),
            (
                "#26=IFCCARTESIANPOINT((-20.,0.,0.));",
                "#26=(IFCCARTESIANPOINT((-20.,0.,0.)));",
            ),
        ] {
            let file = read(&source(version).replace(from, to));
            let node = ShapeNode {
                source: file.instance(30).unwrap(),
                links: zumen_ifc::ShapeLinks::Unsupported,
            };
            let (result, diag, budget) =
                with(Limits::default(), |cx| file.revolved_area(&node, cx));
            assert!(
                matches!(result, Err(Error::Unsupported)),
                "{version} {to}: {result:?}"
            );
            assert!(
                diag.iter()
                    .any(|d| d.code() == codes::IFC_SHAPE_UNSUPPORTED)
            );
            assert_eq!(budget.used(Kind::Depth), 0);
        }
    }
}

#[test]
fn keeps_source_numbers_separate_from_values_and_existing_ids_and_hashes() {
    for version in VERSIONS {
        let src = source(version);
        let file = read(&src);
        let before = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
            .0
            .unwrap();
        let first = value(&file, 30);
        for id in (31..=34).rev() {
            value(&file, id);
        }
        let after = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
            .0
            .unwrap();
        assert_eq!(before, after);
        for i in 0..before.elements().len() {
            assert_eq!(
                Hash::element(&before, ElementRef(i as u32)),
                Hash::element(&after, ElementRef(i as u32))
            );
        }
        let renamed = read(&src.replace("#25", "#925").replace("#13", "#913"));
        assert_eq!(first.definition, value(&renamed, 30).definition);
        assert_ne!(
            first.source.profile.id(),
            value(&renamed, 30).source.profile.id()
        );
        let changed = read(&src.replace(",180.)", ",90.)"));
        assert_ne!(first.definition, value(&changed, 30).definition);
    }
}

#[test]
fn charges_shared_budgets_and_restores_depth_on_success_stop_and_cancel() {
    for version in VERSIONS {
        let file = read(&source(version));
        let shape = product(&file);
        for id in 30..=34 {
            let node = leaf(&shape, id);
            let (_, _, full) = with(Limits::default(), |cx| file.revolved_area(node, cx));
            for kind in Kind::ALL {
                let cap = if kind == Kind::Depth {
                    if id == 34 { 3 } else { 2 }
                } else {
                    full.used(kind)
                };
                let (ok, _, budget) =
                    with(Limits::default().with(kind, Limit::Finite(cap)), |cx| {
                        file.revolved_area(node, cx)
                    });
                assert!(ok.is_ok(), "{version} {id} {kind:?}: {ok:?}");
                assert_eq!(budget.used(Kind::Depth), 0);
                if cap > 0 {
                    let (fail, diag, budget) =
                        with(Limits::default().with(kind, Limit::Finite(cap - 1)), |cx| {
                            file.revolved_area(node, cx)
                        });
                    assert!(
                        matches!(fail, Err(Error::Stopped(_))),
                        "{version} {id} {kind:?}"
                    );
                    assert!(diag.iter().any(|d| d.code() == kind.exceeded_code()));
                    assert_eq!(budget.used(Kind::Depth), 0);
                }
            }
            let mut budget = Budget::new(Limits::default());
            let mut diag = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
            cx.enter().unwrap();
            cancel.cancel();
            assert!(matches!(
                file.revolved_area(node, &mut cx),
                Err(Error::Stopped(_))
            ));
            cx.leave();
            assert_eq!(budget.used(Kind::Depth), 0);
        }
    }
}

#[test]
fn survives_truncated_and_mutated_inputs_with_shared_budgets() {
    let run = |bytes: &[u8], cx: &mut Cx| {
        let file = Ifc::read(bytes, cx)?;
        let shape = file.product_shape(100, cx)?;
        for node in &shape.nodes {
            cx.check(Kind::Work, 1)?;
            if file
                .kind(node.source)
                .eq_ignore_ascii_case(b"IFCREVOLVEDAREASOLID")
            {
                file.revolved_area(node, cx)?;
            }
        }
        Ok(())
    };
    for version in VERSIONS {
        let bytes = source(version).into_bytes();
        for end in (0..bytes.len()).step_by(41) {
            zumen_sdk::testkit::survives_with(&bytes[..end], run).unwrap();
        }
        for at in (0..bytes.len()).step_by(37) {
            for byte in [0, b'$', b'#', 255] {
                let mut changed = bytes.clone();
                changed[at] = byte;
                zumen_sdk::testkit::survives_with(&changed, run).unwrap();
            }
        }
    }
}

fn numbers(out: &mut Writer, values: &[f64]) {
    out.begin_array();
    for &value in values {
        out.float(value);
    }
    out.end_array();
}
fn unit(out: &mut Writer, unit: Option<&Unit>) {
    if let Some(unit) = unit {
        out.begin_object();
        out.key("kind");
        out.string(&unit.kind);
        out.key("name");
        out.string(&unit.name);
        out.key("prefix");
        if let Some(prefix) = &unit.prefix {
            out.string(prefix);
        } else {
            out.null();
        }
        out.key("scale");
        if let Some(scale) = unit.scale {
            out.float(scale);
        } else {
            out.null();
        }
        out.end_object();
    } else {
        out.null();
    }
}
fn json_item(out: &mut Writer, value: &RevolvedArea<'_>) {
    let def = &value.definition;
    out.begin_object();
    out.key("instance");
    out.uint(value.source.solid.id().unwrap());
    out.key("profile");
    out.uint(value.source.profile.id().unwrap());
    out.key("profile_name");
    out.string(def.profile.name.as_deref().unwrap());
    out.key("profile_kind");
    out.string(match def.profile.kind {
        AreaProfileKind::Rectangle { .. } => "rectangle",
        AreaProfileKind::Circle { .. } => "circle",
        AreaProfileKind::IShape { .. } => "i-shape",
        AreaProfileKind::RectangleHollow { .. } => "rectangle-hollow",
        AreaProfileKind::ArbitraryClosed { .. } => "arbitrary-closed",
    });
    out.key("profile_position");
    if let Some(position) = &def.profile.position {
        out.begin_object();
        out.key("location");
        numbers(out, &position.location);
        out.key("reference_direction");
        numbers(out, &position.reference_direction.unwrap());
        out.end_object();
    } else {
        out.null();
    }
    out.key("position");
    let position = def.position.as_ref().unwrap();
    out.begin_object();
    out.key("location");
    numbers(out, &position.location);
    out.key("axis");
    numbers(out, &position.axis.unwrap());
    out.key("reference_direction");
    numbers(out, &position.reference_direction.unwrap());
    out.end_object();
    out.key("axis_location");
    numbers(out, &def.axis.location);
    out.key("axis_direction");
    numbers(out, &def.axis.direction);
    out.key("angle");
    out.float(def.angle.value);
    out.key("angle_unit");
    unit(out, def.angle.unit.as_ref());
    out.key("length_unit");
    unit(out, value.length_unit.as_ref());
    out.key("boundary");
    if let Some(graph) = &value.boundary {
        out.begin_array();
        curve_json::write_graph(out, graph);
        out.end_array();
    } else {
        out.null();
    }
    out.end_object();
}

#[test]
#[allow(clippy::disallowed_methods)] // 合成入力と照合結果だけを指定先へ出す。
fn exports_values_diagnostics_and_all_budget_kinds_for_comparison() {
    let Ok(folder) = std::env::var("ZUMEN_IFC_REVOLUTIONS_OUT") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    let mut contract = String::new();
    for version in VERSIONS {
        let src = source(version);
        let file = read(&src);
        let shape = product(&file);
        let mut out = Writer::new();
        out.begin_object();
        out.key("file");
        out.string(&format!("{version}.ifc"));
        out.key("product");
        out.uint(100);
        out.key("items");
        out.begin_array();
        for id in 30..=34 {
            let node = leaf(&shape, id);
            let (value, diag, budget) = with(Limits::default(), |cx| file.revolved_area(node, cx));
            json_item(&mut out, &value.unwrap());
            contract += &format!(
                "{version}:{id}:ok:{diag:?}:{:?}\n",
                Kind::ALL.map(|k| budget.used(k))
            );
            for kind in Kind::ALL {
                let used = if kind == Kind::Depth {
                    if id == 34 { 3 } else { 2 }
                } else {
                    budget.used(kind)
                };
                for cap in [0, used.saturating_sub(1), used] {
                    let (result, diag, budget) =
                        with(Limits::default().with(kind, Limit::Finite(cap)), |cx| {
                            file.revolved_area(node, cx)
                        });
                    contract += &format!(
                        "{version}:{id}:{kind:?}:{cap}:{:?}:{diag:?}:{:?}\n",
                        result.err(),
                        Kind::ALL.map(|k| budget.used(k))
                    );
                }
            }
        }
        out.end_array();
        out.end_object();
        std::fs::write(folder.join(format!("{version}.json")), out.into_bytes()).unwrap();
        std::fs::write(folder.join(format!("{version}.ifc")), &src).unwrap();
        for (id, from, to) in invalid_inputs() {
            let bad = read(&src.replace(from, to));
            let shape = product(&bad);
            let (result, diag, budget) = with(Limits::default(), |cx| {
                bad.revolved_area(leaf(&shape, id), cx)
            });
            contract += &format!(
                "{version}:{to}:{:?}:{diag:?}:{:?}\n",
                result.err(),
                Kind::ALL.map(|k| budget.used(k))
            );
        }
    }
    std::fs::write(folder.join("contract.txt"), contract).unwrap();
}
