//! I 形・中空矩形を代表要素から取得する。公開仕様から自作した小さい入力だけを使う。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{AreaProfileKind, Building, ElementRef, Hash, Unit};
use zumen_ifc::{Ifc, IfcFile, ProductShape, ProfiledExtrusion, ShapeNode};
use zumen_json::Writer;
use zumen_sdk::{Codec, Error, Lift};

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];

fn source(version: &str) -> String {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('sections'),'2;1');\nFILE_NAME('sections','',(),(),'','','');\nFILE_SCHEMA(('{version}'));\nENDSEC;\nDATA;\n\
#1=IFCPROJECT('0000000000000000000001',$,$,$,$,$,$,$,#2);\n\
#2=IFCUNITASSIGNMENT((#3,#4));\n#3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n\
#4=IFCCONVERSIONBASEDUNIT(#7,.PLANEANGLEUNIT.,'degree',#5);\n\
#5=IFCMEASUREWITHUNIT(IFCPLANEANGLEMEASURE(0.017453292519943295),#6);\n\
#6=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);\n#7=IFCDIMENSIONALEXPONENTS(0,0,0,0,0,0,0);\n\
#8=IFCGEOMETRICREPRESENTATIONCONTEXT('Model','Model',3,0.00001,#20,$);\n\
#10=IFCCARTESIANPOINT((2.,3.));\n#11=IFCDIRECTION((0.,4.));\n#12=IFCAXIS2PLACEMENT2D(#10,#11);\n\
#13=IFCISHAPEPROFILEDEF(.AREA.,'I shape',#12,15.,20.,1.,2.,0.5{flange});\n\
#14=IFCRECTANGLEHOLLOWPROFILEDEF(.AREA.,'Hollow rectangle',#12,15.,5.,1.,0.25,0.5);\n\
#20=IFCAXIS2PLACEMENT3D(#21,#22,#23);\n#21=IFCCARTESIANPOINT((10.,20.,30.));\n\
#22=IFCDIRECTION((0.,0.,2.));\n#23=IFCDIRECTION((3.,0.,0.));\n#24=IFCDIRECTION((2.,-3.,-4.));\n\
#30=IFCEXTRUDEDAREASOLID(#13,#20,#24,12.5);\n#31=IFCEXTRUDEDAREASOLID(#14,#20,#24,1.);\n\
#40=IFCMAPPEDITEM(#41,#43);\n#41=IFCREPRESENTATIONMAP(#20,#42);\n\
#42=IFCSHAPEREPRESENTATION(#8,'Body','SweptSolid',(#30,#31));\n\
#43=IFCCARTESIANTRANSFORMATIONOPERATOR3D($,$,#21,2.,$);\n#44=IFCMAPPEDITEM(#41,#43);\n\
#100=IFCBEAM('0000000000000000000002',$,$,$,$,$,#101,${extra});\n\
#101=IFCPRODUCTDEFINITIONSHAPE($,$,(#102));\n\
#102=IFCSHAPEREPRESENTATION(#8,'Body','MappedRepresentation',(#40,#44));\n\
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
fn value(file: &IfcFile, id: u64) -> ProfiledExtrusion<'_> {
    let shape = product(file);
    let (result, diag, _) = with(Limits::default(), |cx| file.extrusion(leaf(&shape, id), cx));
    assert!(diag.is_empty(), "{diag:?}");
    result.unwrap()
}

#[test]
fn gets_dimensions_fillets_slope_units_placements_and_sources_in_three_versions() {
    for version in VERSIONS {
        let file = read(&source(version));
        let shape = product(&file);
        for id in [30, 31] {
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
            assert!(result.boundary.is_none());
            assert_eq!(result.definition.direction, [2., -3., -4.]);
            let place = result.definition.position.as_ref().unwrap();
            assert_eq!(place.location, [10., 20., 30.]);
            assert_eq!(place.axis, Some([0., 0., 2.]));
            assert_eq!(place.reference_direction, Some([3., 0., 0.]));
            let place = result.definition.profile.position.as_ref().unwrap();
            assert_eq!(place.location, [2., 3.]);
            assert_eq!(place.reference_direction, Some([0., 4.]));
            assert_eq!(
                result.length_unit.as_ref().unwrap().prefix.as_deref(),
                Some("MILLI")
            );
            if id == 30 {
                let AreaProfileKind::IShape {
                    overall_width,
                    overall_depth,
                    web_thickness,
                    flange_thickness,
                    fillet_radius,
                    flange_edge_radius,
                    flange_slope,
                } = &result.definition.profile.kind
                else {
                    panic!()
                };
                assert_eq!(
                    [
                        *overall_width,
                        *overall_depth,
                        *web_thickness,
                        *flange_thickness
                    ],
                    [15., 20., 1., 2.]
                );
                assert_eq!(*fillet_radius, Some(0.5));
                assert_eq!(result.definition.profile.name.as_deref(), Some("I shape"));
                assert_eq!(result.definition.depth, 12.5);
                if version == "IFC2X3" {
                    assert!(flange_edge_radius.is_none() && flange_slope.is_none());
                } else {
                    assert_eq!(*flange_edge_radius, Some(0.25));
                    let slope = flange_slope.as_ref().unwrap();
                    assert_eq!(slope.value, 5.);
                    assert_eq!(
                        slope.unit,
                        Some(Unit {
                            kind: "PLANEANGLEUNIT".into(),
                            name: "degree".into(),
                            prefix: None,
                            scale: Some(0.017453292519943295)
                        })
                    );
                    for id in 1..=7 {
                        assert!(result.source.records.iter().any(|r| r.id() == Some(id)));
                    }
                }
            } else {
                assert_eq!(
                    result.definition.profile.kind,
                    AreaProfileKind::RectangleHollow {
                        x_dim: 15.,
                        y_dim: 5.,
                        wall_thickness: 1.,
                        inner_fillet_radius: Some(0.25),
                        outer_fillet_radius: Some(0.5)
                    }
                );
                assert_eq!(
                    result.definition.profile.name.as_deref(),
                    Some("Hollow rectangle")
                );
                assert_eq!(result.definition.depth, 1.);
            }
        }
    }
}

#[test]
fn preserves_optional_fillets_and_distinguishes_zero_by_version() {
    for version in VERSIONS {
        let src = source(version);
        let omitted = src
            .replace("1.,2.,0.5", "1.,2.,$")
            .replace(",0.25,5.)", ",$,$)")
            .replace("5.,1.,0.25,0.5)", "5.,1.,$,$)");
        let file = read(&omitted);
        assert!(matches!(
            value(&file, 30).definition.profile.kind,
            AreaProfileKind::IShape {
                fillet_radius: None,
                flange_edge_radius: None,
                flange_slope: None,
                ..
            }
        ));
        assert!(matches!(
            value(&file, 31).definition.profile.kind,
            AreaProfileKind::RectangleHollow {
                inner_fillet_radius: None,
                outer_fillet_radius: None,
                ..
            }
        ));
        let zero = src
            .replace("1.,2.,0.5", "1.,2.,0.")
            .replace(",0.25,5.)", ",0.,0.)")
            .replace("5.,1.,0.25,0.5)", "5.,1.,0.,0.)");
        let file = read(&zero);
        let shape = product(&file);
        for id in [30, 31] {
            let (result, diag, _) =
                with(Limits::default(), |cx| file.extrusion(leaf(&shape, id), cx));
            if version == "IFC2X3" {
                assert!(matches!(result, Err(Error::Invalid)));
                assert!(format!("{diag:?}").contains("profile-fillet"));
            } else {
                assert!(diag.is_empty());
                let result = result.unwrap();
                if id == 30 {
                    assert!(
                        matches!(result.definition.profile.kind, AreaProfileKind::IShape { fillet_radius: Some(0.), flange_edge_radius: Some(0.), flange_slope: Some(ref s), .. } if s.value == 0.)
                    );
                } else {
                    assert!(matches!(
                        result.definition.profile.kind,
                        AreaProfileKind::RectangleHollow {
                            inner_fillet_radius: Some(0.),
                            outer_fillet_radius: Some(0.),
                            ..
                        }
                    ));
                }
            }
        }
    }
}

#[test]
fn compares_twice_the_thickness_without_rounding_half_a_subnormal_dimension() {
    for version in VERSIONS {
        let src = source(version)
            .replace("15.,20.,1.,2.,0.5", "3.E-323,2.5E-323,5.E-324,1.E-323,$")
            .replace(",0.25,5.)", ",$,$)")
            .replace("15.,5.,1.,0.25,0.5)", "2.5E-323,2.5E-323,1.E-323,$,$)");
        let file = read(&src);
        value(&file, 30);
        value(&file, 31);
    }
}

#[test]
fn reads_angle_units_only_when_slope_is_present_and_keeps_missing_units_explicit() {
    let src = source("IFC4");
    for (from, to, expected) in [
        (
            "#2=IFCUNITASSIGNMENT((#3,#4))",
            "#2=IFCUNITASSIGNMENT((#3,#6))",
            Some(("RADIAN", None, None)),
        ),
        (
            "#6=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.)",
            "#6=IFCSIUNIT(*,.PLANEANGLEUNIT.,.MILLI.,.RADIAN.)",
            Some(("degree", None, Some(0.017453292519943295 * 0.001))),
        ),
        (
            "#2=IFCUNITASSIGNMENT((#3,#4))",
            "#2=IFCUNITASSIGNMENT((#3))",
            None,
        ),
    ] {
        let file = read(&src.replace(from, to));
        let shape = product(&file);
        let (result, diag, _) = with(Limits::default(), |cx| file.extrusion(leaf(&shape, 30), cx));
        let AreaProfileKind::IShape {
            flange_slope: Some(slope),
            ..
        } = result.unwrap().definition.profile.kind
        else {
            panic!()
        };
        assert_eq!(slope.value, 5.);
        assert_eq!(
            slope
                .unit
                .as_ref()
                .map(|u| (u.name.as_str(), u.prefix.as_deref(), u.scale)),
            expected
        );
        assert_eq!(
            diag.iter()
                .filter(|d| d.code() == codes::IFC_UNIT_MISSING)
                .count(),
            usize::from(expected.is_none())
        );
    }
    for (from, to) in [
        ("(#3,#4)", "(#3,#4,#6)"),
        (
            "IFCDIMENSIONALEXPONENTS(0,0,0,0,0,0,0)",
            "IFCDIMENSIONALEXPONENTS(1,0,0,0,0,0,0)",
        ),
        (
            "IFCPLANEANGLEMEASURE(0.017453292519943295)",
            "IFCPLANEANGLEMEASURE(IFCPLANEANGLEMEASURE(1.))",
        ),
        (
            "IFCPLANEANGLEMEASURE(0.017453292519943295),#6",
            "IFCPLANEANGLEMEASURE(1.),#4",
        ),
        (
            "IFCPLANEANGLEMEASURE(0.017453292519943295)",
            "IFCPLANEANGLEMEASURE(-1.)",
        ),
        (".PLANEANGLEUNIT.,$,.RADIAN.", ".PLANEANGLEUNIT.,$,.METRE."),
        (
            ".PLANEANGLEUNIT.,$,.RADIAN.",
            ".PLANEANGLEUNIT.,.INVALID.,.RADIAN.",
        ),
    ] {
        let bad = src.replace(from, to);
        let file = read(&bad);
        let shape = product(&file);
        assert!(
            with(Limits::default(), |cx| file.extrusion(leaf(&shape, 30), cx))
                .0
                .is_err(),
            "{to}"
        );
        // 対象にしない角度の換算を追わない。長さと断面の値は取得できる。
        value(&file, 31);
        let no_slope = read(&bad.replace(",0.25,5.)", ",0.25,$)"));
        value(&no_slope, 30);
    }
}

fn invalid_inputs() -> Vec<(u64, &'static str, &'static str)> {
    vec![
        (30, "15.,20.,1.,2.,0.5", "0.,20.,1.,2.,0.5"),
        (30, "15.,20.,1.,2.,0.5", "15.,0.,1.,2.,0.5"),
        (30, "15.,20.,1.,2.,0.5", "15.,20.,15.,2.,0.5"),
        (30, "15.,20.,1.,2.,0.5", "15.,20.,1.,10.,0.5"),
        (30, "15.,20.,1.,2.,0.5", "15.,20.,-1.,2.,0.5"),
        (30, "15.,20.,1.,2.,0.5", "15.,20.,1.,0.,0.5"),
        (30, "15.,20.,1.,2.,0.5", "15.,20.,1.,2.,-0.5"),
        (30, "15.,20.,1.,2.,0.5", "15.,20.,1.,2.,1.E400"),
        (30, "15.,20.,1.,2.,0.5", "15.,20.,1.,2.,*"),
        (
            30,
            "IFCISHAPEPROFILEDEF(.AREA.",
            "IFCISHAPEPROFILEDEF(.CURVE.",
        ),
        (31, "15.,5.,1.,0.25,0.5)", "15.,5.,2.5,0.25,0.5)"),
        (31, "15.,5.,1.,0.25,0.5)", "2.,5.,1.,0.25,0.5)"),
        (31, "15.,5.,1.,0.25,0.5)", "15.,5.,-1.,0.25,0.5)"),
        (31, "15.,5.,1.,0.25,0.5)", "15.,5.,1.,-0.25,0.5)"),
        (31, "15.,5.,1.,0.25,0.5)", "15.,5.,1.,0.25,-0.5)"),
        (31, "15.,5.,1.,0.25,0.5)", "15.,5.,1.,0.25,0.5,$)"),
        (31, "15.,5.,1.,0.25,0.5)", "15.,5.,1.,0.25)"),
        (30, "'I shape',#12", "'I shape',#13"),
    ]
}

#[test]
fn rejects_invalid_dimensions_thickness_fillets_attributes_and_cycles() {
    for version in VERSIONS {
        for (id, from, to) in invalid_inputs() {
            let file = read(&source(version).replace(from, to));
            let shape = product(&file);
            let (result, diag, budget) =
                with(Limits::default(), |cx| file.extrusion(leaf(&shape, id), cx));
            assert!(
                matches!(result, Err(Error::Invalid)),
                "{version} {to}: {result:?}"
            );
            assert!(
                diag.iter()
                    .any(|d| d.code() == codes::IFC_EXTRUSION_INVALID)
            );
            assert!(format!("{diag:?}").contains(if id == 30 { "i13" } else { "i14" }));
            assert_eq!(budget.used(Kind::Depth), 0);
        }
        let src = source(version);
        let wrong_count = if version == "IFC2X3" {
            src.replace("2.,0.5)", "2.,0.5,$,$)")
        } else {
            src.replace("0.5,0.25,5.)", "0.5)")
        };
        let file = read(&wrong_count);
        let shape = product(&file);
        assert!(matches!(
            with(Limits::default(), |cx| file.extrusion(leaf(&shape, 30), cx)).0,
            Err(Error::Invalid)
        ));
    }
}

#[test]
fn preserves_old_hashes_ids_and_source_independent_values() {
    for version in VERSIONS {
        let src = source(version);
        let file = read(&src);
        let before = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
            .0
            .unwrap();
        let first = value(&file, 30);
        value(&file, 31);
        let after = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
            .0
            .unwrap();
        assert_eq!(before, after);
        assert_eq!(
            Hash::document_values(&before),
            Hash::document_values(&after)
        );
        for i in 0..before.elements().len() {
            assert_eq!(
                Hash::element(&before, ElementRef(i as u32)),
                Hash::element(&after, ElementRef(i as u32))
            );
        }
        let renamed = read(&src.replace("#13", "#913").replace("#12", "#912"));
        let second = value(&renamed, 30);
        assert_eq!(first.definition, second.definition);
        assert_eq!(first.length_unit, second.length_unit);
        assert_ne!(first.source.profile.id(), second.source.profile.id());
        let foreign = product(&renamed);
        assert!(matches!(
            with(Limits::default(), |cx| file
                .extrusion(leaf(&foreign, 30), cx))
            .0,
            Err(Error::Invalid)
        ));
        let changed = read(&src.replace("15.,20.,1.,2.,0.5", "16.,20.,1.,2.,0.5"));
        assert_ne!(first.definition, value(&changed, 30).definition);
    }
}

#[test]
fn shares_all_budget_kinds_and_restores_incoming_depth_on_stop_and_cancel() {
    for version in VERSIONS {
        let file = read(&source(version));
        let shape = product(&file);
        for id in [30, 31] {
            let node = leaf(&shape, id);
            let (_, _, full) = with(Limits::default(), |cx| file.extrusion(node, cx));
            for kind in Kind::ALL {
                let cap = if kind == Kind::Depth {
                    2
                } else {
                    full.used(kind)
                };
                let (ok, _, budget) =
                    with(Limits::default().with(kind, Limit::Finite(cap)), |cx| {
                        file.extrusion(node, cx)
                    });
                assert!(ok.is_ok(), "{version} {id} {kind:?}: {ok:?}");
                assert_eq!(budget.used(Kind::Depth), 0);
                if cap > 0 {
                    let (fail, diag, budget) =
                        with(Limits::default().with(kind, Limit::Finite(cap - 1)), |cx| {
                            file.extrusion(node, cx)
                        });
                    assert!(
                        matches!(fail, Err(Error::Stopped(_))),
                        "{version} {id} {kind:?}"
                    );
                    assert!(diag.iter().any(|d| d.code() == kind.exceeded_code()));
                    assert_eq!(budget.used(Kind::Depth), 0);
                }
            }
            for cap in [0, 1, 64, 128, 256, 512, 1024] {
                let (_, _, budget) = with(
                    Limits::default().with(Kind::DecodedBytes, Limit::Finite(cap)),
                    |cx| {
                        cx.enter().unwrap();
                        let result = file.extrusion(node, cx);
                        cx.leave();
                        result
                    },
                );
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
            file.extrusion(leaf(&shape, 30), &mut cx),
            Err(Error::Stopped(_))
        ));
        cx.leave();
        assert_eq!(budget.used(Kind::Depth), 0);
        assert!(
            diag.iter()
                .any(|d| d.code() == codes::CALL_REQUEST_CANCELLED)
        );
    }
}

#[test]
fn survives_truncated_and_mutated_inputs_with_shared_budgets() {
    let run = |src: &[u8], cx: &mut Cx| {
        let file = Ifc::read(src, cx)?;
        let shape = file.product_shape(100, cx)?;
        for node in &shape.nodes {
            cx.check(Kind::Work, 1)?;
            if file
                .kind(node.source)
                .eq_ignore_ascii_case(b"IFCEXTRUDEDAREASOLID")
            {
                file.extrusion(node, cx)?;
            }
        }
        Ok(())
    };
    for version in VERSIONS {
        let bytes = source(version).into_bytes();
        for end in (0..bytes.len()).step_by(29) {
            zumen_sdk::testkit::survives_with(&bytes[..end], run).unwrap();
        }
        for at in (0..bytes.len()).step_by(31) {
            for byte in [0, b'$', b'#', 255] {
                let mut changed = bytes.clone();
                changed[at] = byte;
                zumen_sdk::testkit::survives_with(&changed, run).unwrap();
            }
        }
    }
}

fn number(out: &mut Writer, value: Option<f64>) {
    if let Some(value) = value {
        out.float(value);
    } else {
        out.null();
    }
}
fn numbers(out: &mut Writer, values: &[f64]) {
    out.begin_array();
    for &value in values {
        out.float(value);
    }
    out.end_array();
}
fn unit(out: &mut Writer, value: Option<&Unit>) {
    if let Some(value) = value {
        out.begin_object();
        out.key("kind");
        out.string(&value.kind);
        out.key("name");
        out.string(&value.name);
        out.key("prefix");
        if let Some(prefix) = &value.prefix {
            out.string(prefix);
        } else {
            out.null();
        }
        out.key("scale");
        number(out, value.scale);
        out.end_object();
    } else {
        out.null();
    }
}
fn json_item(out: &mut Writer, value: &ProfiledExtrusion<'_>) {
    let def = &value.definition;
    out.begin_object();
    out.key("instance");
    out.uint(value.source.solid.id().unwrap());
    out.key("profile");
    out.uint(value.source.profile.id().unwrap());
    out.key("name");
    out.string(def.profile.name.as_deref().unwrap());
    out.key("profile_position");
    numbers(out, &def.profile.position.as_ref().unwrap().location);
    out.key("profile_direction");
    numbers(
        out,
        &def.profile
            .position
            .as_ref()
            .unwrap()
            .reference_direction
            .unwrap(),
    );
    out.key("position");
    numbers(out, &def.position.as_ref().unwrap().location);
    out.key("axis");
    numbers(out, &def.position.as_ref().unwrap().axis.unwrap());
    out.key("reference_direction");
    numbers(
        out,
        &def.position.as_ref().unwrap().reference_direction.unwrap(),
    );
    out.key("direction");
    numbers(out, &def.direction);
    out.key("depth");
    out.float(def.depth);
    out.key("length_unit");
    unit(out, value.length_unit.as_ref());
    match &def.profile.kind {
        AreaProfileKind::IShape {
            overall_width,
            overall_depth,
            web_thickness,
            flange_thickness,
            fillet_radius,
            flange_edge_radius,
            flange_slope,
        } => {
            out.key("kind");
            out.string("i-shape");
            out.key("dimensions");
            numbers(
                out,
                &[
                    *overall_width,
                    *overall_depth,
                    *web_thickness,
                    *flange_thickness,
                ],
            );
            out.key("fillet_radius");
            number(out, *fillet_radius);
            out.key("edge_radius");
            number(out, *flange_edge_radius);
            out.key("slope");
            number(out, flange_slope.as_ref().map(|s| s.value));
            out.key("angle_unit");
            unit(out, flange_slope.as_ref().and_then(|s| s.unit.as_ref()));
        }
        AreaProfileKind::RectangleHollow {
            x_dim,
            y_dim,
            wall_thickness,
            inner_fillet_radius,
            outer_fillet_radius,
        } => {
            out.key("kind");
            out.string("rectangle-hollow");
            out.key("dimensions");
            numbers(out, &[*x_dim, *y_dim, *wall_thickness]);
            out.key("inner_radius");
            number(out, *inner_fillet_radius);
            out.key("outer_radius");
            number(out, *outer_fillet_radius);
        }
        _ => panic!("この照合では扱わない断面"),
    }
    out.end_object();
}

#[test]
#[allow(clippy::disallowed_methods)] // 合成入力と照合結果だけを指定先へ出す。
fn exports_values_diagnostics_and_all_budget_kinds_for_comparison() {
    let Ok(folder) = std::env::var("ZUMEN_IFC_SECTIONS_OUT") else {
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
        for id in [30, 31] {
            let node = leaf(&shape, id);
            let (value, diag, budget) = with(Limits::default(), |cx| file.extrusion(node, cx));
            json_item(&mut out, &value.unwrap());
            contract += &format!(
                "{version}:{id}:ok:{diag:?}:{:?}\n",
                Kind::ALL.map(|k| budget.used(k))
            );
            for kind in Kind::ALL {
                let used = if kind == Kind::Depth {
                    2
                } else {
                    budget.used(kind)
                };
                for cap in [0, used.saturating_sub(1), used] {
                    let (result, diag, budget) =
                        with(Limits::default().with(kind, Limit::Finite(cap)), |cx| {
                            file.extrusion(node, cx)
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
            let (result, diag, budget) =
                with(Limits::default(), |cx| bad.extrusion(leaf(&shape, id), cx));
            contract += &format!(
                "{version}:{to}:{:?}:{diag:?}:{:?}\n",
                result.err(),
                Kind::ALL.map(|k| budget.used(k))
            );
        }
    }
    std::fs::write(folder.join("contract.txt"), contract).unwrap();
}
