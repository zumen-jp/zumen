//! 代表要素から未評価の断面と押し出しを取得する。入力は自作と公開サンプルだけ。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{AreaProfileKind, Building, ElementRef, Hash};
use zumen_ifc::{Ifc, IfcFile, ProductShape, ProfiledExtrusion, ShapeNode};
use zumen_json::Writer;
use zumen_sdk::{Codec, Error, Lift};

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];
fn source(version: &str) -> String {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('profiles'),'2;1');\nFILE_NAME('profiles','',(),(),'','','');\nFILE_SCHEMA(('{version}'));\nENDSEC;\nDATA;\n\
#1=IFCPROJECT('0000000000000000000001',$,$,$,$,$,$,$,#2);\n\
#2=IFCUNITASSIGNMENT((#3));\n#3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n\
#8=IFCGEOMETRICREPRESENTATIONCONTEXT('Model','Model',3,0.00001,#20,$);\n\
#10=IFCCARTESIANPOINT((2.,3.));\n#11=IFCDIRECTION((0.,4.));\n#12=IFCAXIS2PLACEMENT2D(#10,#11);\n\
#13=IFCRECTANGLEPROFILEDEF(.AREA.,'Rectangle',#12,15.,5.);\n#14=IFCCIRCLEPROFILEDEF(.AREA.,'Circle',#12,2.5);\n\
#15=IFCCARTESIANPOINT((0.,0.));\n#16=IFCCARTESIANPOINT((1.,0.));\n#17=IFCCARTESIANPOINT((0.,1.));\n\
#18=IFCPOLYLINE((#15,#16,#17,#15));\n#19=IFCARBITRARYCLOSEDPROFILEDEF(.AREA.,'Closed',#18);\n\
#20=IFCAXIS2PLACEMENT3D(#21,#22,#23);\n#21=IFCCARTESIANPOINT((10.,20.,30.));\n\
#22=IFCDIRECTION((0.,0.,2.));\n#23=IFCDIRECTION((3.,0.,0.));\n#24=IFCDIRECTION((2.,-3.,-4.));\n\
#30=IFCEXTRUDEDAREASOLID(#13,#20,#24,12.5);\n#31=IFCEXTRUDEDAREASOLID(#14,#20,#24,1.);\n\
#32=IFCEXTRUDEDAREASOLID(#19,#20,#24,8.);\n\
#40=IFCMAPPEDITEM(#41,#43);\n#41=IFCREPRESENTATIONMAP(#20,#42);\n\
#42=IFCSHAPEREPRESENTATION(#8,'Body','SweptSolid',(#30,#31,#32));\n\
#43=IFCCARTESIANTRANSFORMATIONOPERATOR3D($,$,#21,2.,$);\n#44=IFCMAPPEDITEM(#41,#43);\n\
#100=IFCWALL('0000000000000000000002',$,$,$,$,$,#101,${extra});\n\
#101=IFCPRODUCTDEFINITIONSHAPE($,$,(#102));\n\
#102=IFCSHAPEREPRESENTATION(#8,'Body','MappedRepresentation',(#40,#44));\n\
ENDSEC;\nEND-ISO-10303-21;\n",
        extra = if version == "IFC2X3" {
            ""
        } else {
            ",.NOTDEFINED."
        }
    )
}
fn comparison_source(version: &str) -> String {
    if version == "IFC4" {
        conversion_source(version)
    } else {
        source(version)
    }
}
fn conversion_source(version: &str) -> String {
    source(version).replace("#3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);", "#3=IFCCONVERSIONBASEDUNIT(#6,.LENGTHUNIT.,'scaled_length',#4);\n#4=IFCMEASUREWITHUNIT(IFCLENGTHMEASURE(0.3048),#5);\n#5=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n#6=IFCDIMENSIONALEXPONENTS(1,0,0,0,0,0,0);")
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
fn product(file: &IfcFile, id: u64) -> ProductShape<'_> {
    with(Limits::default(), |cx| file.product_shape(id, cx))
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
    let shape = product(file, 100);
    let (result, diag, _) = with(Limits::default(), |cx| file.extrusion(leaf(&shape, id), cx));
    assert!(diag.is_empty(), "{diag:?}");
    result.unwrap()
}

#[test]
fn gets_three_profiles_from_shared_product_shape_in_three_versions() {
    for version in VERSIONS {
        let file = read(&source(version));
        let shape = product(&file, 100);
        assert_eq!(
            shape
                .nodes
                .iter()
                .filter(|n| n.source.id() == Some(30))
                .count(),
            1
        );
        let rectangle = value(&file, 30);
        assert_eq!(
            rectangle.definition.profile.kind,
            AreaProfileKind::Rectangle {
                x_dim: 15.,
                y_dim: 5.
            }
        );
        let position = rectangle.definition.profile.position.as_ref().unwrap();
        assert_eq!(position.location, [2., 3.]);
        assert_eq!(position.reference_direction, Some([0., 4.]));
        assert_eq!(
            rectangle.definition.profile.name.as_deref(),
            Some("Rectangle")
        );
        assert_eq!(
            rectangle.definition.position.as_ref().unwrap().location,
            [10., 20., 30.]
        );
        assert_eq!(rectangle.definition.direction, [2., -3., -4.]);
        assert_eq!(rectangle.definition.depth, 12.5);
        assert_eq!(
            rectangle.length_unit.as_ref().unwrap().prefix.as_deref(),
            Some("MILLI")
        );
        assert!(rectangle.boundary.is_none());
        for id in [1, 2, 3, 10, 11, 12, 13, 20, 21, 22, 23, 24] {
            assert!(
                rectangle
                    .source
                    .records
                    .iter()
                    .any(|r| std::ptr::eq(*r, file.instance(id).unwrap())),
                "{id}"
            );
        }
        assert_eq!(rectangle.source.solid.id(), Some(30));
        assert_eq!(
            value(&file, 31).definition.profile.kind,
            AreaProfileKind::Circle { radius: 2.5 }
        );
        let arbitrary = value(&file, 32);
        let boundary = arbitrary.boundary.as_ref().unwrap();
        assert_eq!(
            arbitrary.definition.profile.kind,
            AreaProfileKind::ArbitraryClosed {
                boundary: boundary.root
            }
        );
        let old = with(Limits::default(), |cx| {
            file.extruded_area(leaf(&shape, 32), cx)
        })
        .0
        .unwrap();
        assert_eq!(old.definition.position, arbitrary.definition.position);
        assert_eq!(old.boundary.items[0].kind, boundary.items[0].kind);
        assert_eq!(old.boundary.length_unit, arbitrary.length_unit);
    }
}

#[test]
fn preserves_version_specific_optional_and_zero_placement_values() {
    for version in VERSIONS {
        let base = source(version);
        for (from, to, valid) in [
            ("'Rectangle',#12", "'Rectangle',$", version != "IFC2X3"),
            ("#13,#20,#24", "#13,$,#24", version != "IFC2X3"),
            (
                "IFCDIRECTION((0.,4.))",
                "IFCDIRECTION((0.,0.))",
                version == "IFC2X3",
            ),
            (
                "IFCAXIS2PLACEMENT2D(#10,#11)",
                "IFCAXIS2PLACEMENT2D(#10,$)",
                true,
            ),
        ] {
            let file = read(&base.replace(from, to));
            let shape = product(&file, 100);
            let (r, _, b) = with(Limits::default(), |cx| file.extrusion(leaf(&shape, 30), cx));
            assert_eq!(r.is_ok(), valid, "{version} {from}");
            assert_eq!(b.used(Kind::Depth), 0);
            if valid && from.starts_with("IFCDIRECTION") {
                assert_eq!(
                    r.unwrap()
                        .definition
                        .profile
                        .position
                        .unwrap()
                        .reference_direction,
                    Some([0., 0.])
                );
            }
        }
    }
}

#[test]
fn rejects_bad_dimensions_references_placements_and_unknown_attributes_without_partial_results() {
    for version in VERSIONS {
        for (from, to) in [
            ("#12,15.,5.)", "#12,0.,5.)"),
            ("#12,15.,5.)", "#12,-0.,5.)"),
            ("#12,15.,5.)", "#12,15.,-2.)"),
            ("#12,15.,5.)", "#12,1e999,5.)"),
            ("#12,2.5)", "#12,-1.)"),
            ("#12,15.,5.)", "#12,15.,5.,7.)"),
            ("'Rectangle',#12", "'Rectangle',#999"),
            (".AREA.,'Rectangle'", ".CURVE.,'Rectangle'"),
            (
                "IFCCARTESIANPOINT((2.,3.))",
                "IFCCARTESIANPOINT((2.,3.,4.))",
            ),
            ("IFCDIRECTION((0.,4.))", "IFCDIRECTION((0.,4.,1.))"),
            (
                "IFCAXIS2PLACEMENT2D(#10,#11)",
                "IFCAXIS2PLACEMENT2D(#12,#11)",
            ),
            ("#13,#20,#24,12.5)", "#13,#20,#24,-1.)"),
            ("IFCDIRECTION((2.,-3.,-4.))", "IFCDIRECTION((2.,-3.,0.))"),
        ] {
            let file = read(&source(version).replace(from, to));
            let shape = product(&file, 100);
            let target = if from == "#12,2.5)" { 31 } else { 30 };
            let (r, d, b) = with(Limits::default(), |cx| {
                file.extrusion(leaf(&shape, target), cx)
            });
            assert!(matches!(r, Err(Error::Invalid)), "{version} {from}: {r:?}");
            assert!(d.iter().any(|d| d.code() == codes::IFC_EXTRUSION_INVALID));
            assert_eq!(b.used(Kind::Depth), 0);
        }
    }
}

#[test]
fn unsupported_subtypes_and_complex_instances_report_the_kind_and_place() {
    let base = source("IFC4");
    for (from, to, place) in [
        ("IFCRECTANGLEPROFILEDEF", "IFCRECTANGLEHOLLOWPROFILEDEF", 13),
        ("IFCRECTANGLEPROFILEDEF", "IFCISHAPEPROFILEDEF", 13),
        (
            "IFCRECTANGLEPROFILEDEF",
            "IFCARBITRARYPROFILEDEFWITHVOIDS",
            13,
        ),
        (
            "#13=IFCRECTANGLEPROFILEDEF(.AREA.,'Rectangle',#12,15.,5.);",
            "#13=(IFCPROFILEDEF(.AREA.,'Rectangle') IFCRECTANGLEPROFILEDEF(15.,5.));",
            13,
        ),
    ] {
        let file = read(&base.replace(from, to));
        let shape = product(&file, 100);
        let (r, d, _) = with(Limits::default(), |cx| file.extrusion(leaf(&shape, 30), cx));
        assert!(matches!(r, Err(Error::Unsupported) | Err(Error::Invalid)));
        assert!(format!("{d:?}").contains(&format!("i{place}")));
        if !to.starts_with("#13=") {
            assert!(format!("{d:?}").contains(to));
        }
    }
}

#[test]
fn budgets_all_kinds_before_allocations_and_restores_the_incoming_depth() {
    for src in [source("IFC4"), conversion_source("IFC4")] {
        for id in [30, 31, 32] {
            let file = read(&src);
            let shape = product(&file, 100);
            let node = leaf(&shape, id);
            let (_, _, full) = with(Limits::default(), |cx| file.extrusion(node, cx));
            for kind in Kind::ALL {
                let limit = if kind == Kind::Depth {
                    (0..16)
                        .find(|&n| {
                            with(Limits::default().with(kind, Limit::Finite(n)), |cx| {
                                file.extrusion(node, cx)
                            })
                            .0
                            .is_ok()
                        })
                        .unwrap()
                } else {
                    full.used(kind)
                };
                assert!(
                    with(Limits::default().with(kind, Limit::Finite(limit)), |cx| {
                        file.extrusion(node, cx)
                    })
                    .0
                    .is_ok()
                );
                if limit > 0 {
                    let (r, _, b) = with(
                        Limits::default().with(kind, Limit::Finite(limit - 1)),
                        |cx| {
                            cx.enter().unwrap();
                            file.extrusion(node, cx)
                        },
                    );
                    assert!(matches!(r, Err(Error::Stopped(_))), "{id}/{kind:?}");
                    assert_eq!(b.used(Kind::Depth), 1);
                }
            }
            for expired in [false, true] {
                let (r, _, b) = with(Limits::default(), |cx| {
                    if expired {
                        cx.cancel.expire()
                    } else {
                        cx.cancel.cancel()
                    };
                    file.extrusion(node, cx)
                });
                assert!(matches!(r, Err(Error::Stopped(_))));
                assert_eq!(b.used(Kind::Work), 0);
                assert_eq!(b.used(Kind::DecodedBytes), 0);
            }
            let (_, _, b) = with(Limits::default(), |cx| {
                cx.enter().unwrap();
                file.extrusion(node, cx).unwrap()
            });
            assert_eq!(b.used(Kind::Depth), 1);
        }
    }
    let huge = source("IFC4").replace("'Rectangle'", &format!("'{}'", "x".repeat(100_000)));
    let file = read(&huge);
    let shape = product(&file, 100);
    let (r, _, b) = with(
        Limits::default().with(Kind::DecodedBytes, Limit::Finite(32)),
        |cx| file.extrusion(leaf(&shape, 30), cx),
    );
    assert!(matches!(r, Err(Error::Stopped(_))));
    assert_eq!(b.used(Kind::DecodedBytes), 32);
}

#[test]
fn keeps_old_ids_hashes_and_source_independent_meaning() {
    let src = source("IFC4");
    let file = read(&src);
    let before = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
        .0
        .unwrap();
    let first = value(&file, 30);
    value(&file, 32);
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
    let renamed = read(
        &src.replace("#13", "#913")
            .replace("#12", "#912")
            .replace("#10", "#910"),
    );
    // #100/#101/#102 も同じ置換を受けるので、取得する要素の番号も変える。
    let shape = product(&renamed, 9100);
    let second = with(Limits::default(), |cx| {
        renamed.extrusion(leaf(&shape, 30), cx)
    })
    .0
    .unwrap();
    assert_eq!(first.definition, second.definition);
    assert_eq!(first.length_unit, second.length_unit);
    assert_ne!(first.source.profile.id(), second.source.profile.id());
    let changed = read(&src.replace("#12,15.,5.)", "#12,16.,5.)"));
    assert_ne!(first.definition, value(&changed, 30).definition);
}

#[test]
fn rejects_foreign_nodes_curve_cycles_and_nested_unit_factors() {
    let file = read(&source("IFC4"));
    let other = read(&source("IFC4"));
    let shape = product(&other, 100);
    assert!(matches!(
        with(Limits::default(), |cx| file.extrusion(leaf(&shape, 30), cx)).0,
        Err(Error::Invalid)
    ));
    let src = source("IFC4").replace(
        "IFCPOLYLINE((#15,#16,#17,#15))",
        "IFCTRIMMEDCURVE(#18,(IFCPARAMETERVALUE(0.)),(IFCPARAMETERVALUE(1.)),.T.,.PARAMETER.)",
    );
    let file = read(&src);
    let shape = product(&file, 100);
    let (r, _, b) = with(Limits::default(), |cx| file.extrusion(leaf(&shape, 32), cx));
    assert!(r.is_err());
    assert_eq!(b.used(Kind::Depth), 0);
    let src=source("IFC4").replace("#3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);","#3=IFCCONVERSIONBASEDUNIT(#6,.LENGTHUNIT.,'foot',#4);\n#4=IFCMEASUREWITHUNIT(IFCLENGTHMEASURE(IFCLENGTHMEASURE(0.3048)),#5);\n#5=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);\n#6=IFCDIMENSIONALEXPONENTS(1,0,0,0,0,0,0);");
    let file = read(&src);
    let shape = product(&file, 100);
    assert!(matches!(
        with(Limits::default(), |cx| file.extrusion(leaf(&shape, 30), cx)).0,
        Err(Error::Invalid)
    ));
}

#[test]
fn public_products_reach_typed_profiles_without_caller_argument_reads() {
    for ((src, product_id, solid, profile, name, kind), global_id) in
        public_cases().into_iter().zip([
            "1soz90sYzCp8a2Do0djRBF",
            "0pTNkIYt5EyfHBAi6yiASg",
            "2q2CBrfQTCKPY0zxCdZygb",
        ])
    {
        let file = read(src);
        let building = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
            .0
            .unwrap();
        let at = building
            .find(zumen_building::Id::Global(
                zumen_building::GlobalId::parse(global_id).unwrap(),
            ))
            .unwrap();
        assert_eq!(
            building.element(at).unwrap().source().instance,
            Some(product_id)
        );
        let shape = product(&file, product_id);
        let (result, d, _) = with(Limits::default(), |cx| {
            file.extrusion(leaf(&shape, solid), cx)
        });
        assert!(d.is_empty(), "{name}: {d:?}");
        let result = result.unwrap();
        assert_eq!(result.source.profile.id(), Some(profile));
        assert_eq!(result.boundary.is_some(), kind == "arbitrary-closed");
        match result.definition.profile.kind {
            AreaProfileKind::Rectangle { x_dim, y_dim } => {
                assert_eq!(x_dim, 15.);
                assert_eq!(y_dim, 4.99999999999995);
            }
            AreaProfileKind::Circle { radius } => {
                assert!(radius > 0.02539 && radius < 0.02541);
                assert_eq!(
                    result.definition.profile.name.as_deref(),
                    Some("2100x830x380mm")
                );
            }
            AreaProfileKind::ArbitraryClosed { .. } => assert_eq!(result.definition.depth, 8.8),
        }
    }
}
fn public_cases() -> [(&'static str, u64, u64, u64, &'static str, &'static str); 3] {
    let model9 = include_str!("../../../samples/ifc/gni/model_9.ifc");
    [
        (model9, 1163, 1137, 1134, "public_rectangle", "rectangle"),
        (
            include_str!("../../../samples/ifc/gni/model_18.ifc"),
            21873,
            21764,
            21761,
            "public_circle",
            "circle",
        ),
        (
            model9,
            131,
            114,
            112,
            "public_arbitrary",
            "arbitrary-closed",
        ),
    ]
}

#[test]
fn keeps_length_units_and_does_not_evaluate_unrequested_units() {
    let base = source("IFC4");
    let src=base.replace("#2=IFCUNITASSIGNMENT((#3));", "#2=IFCUNITASSIGNMENT((#3,#7));\n#7=IFCDERIVEDUNIT((),.THERMALTRANSMITTANCEUNIT.,$);")
        .replace("#3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);", "#3=IFCCONVERSIONBASEDUNIT(#6,.LENGTHUNIT.,'foot',#4);\n#4=IFCMEASUREWITHUNIT(IFCLENGTHMEASURE(0.3048),#5);\n#5=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);\n#6=IFCDIMENSIONALEXPONENTS(1,0,0,0,0,0,0);");
    let file = read(&src);
    let result = value(&file, 30);
    let unit = result.length_unit.unwrap();
    assert_eq!(unit.name, "foot");
    assert_eq!(unit.scale, Some(0.3048));
    for id in [3, 4, 5, 7] {
        assert!(result.source.records.iter().any(|r| r.id() == Some(id)));
    }
    let no_project = base
        .lines()
        .filter(|line| !line.starts_with("#1="))
        .collect::<Vec<_>>()
        .join("\n");
    let file = read(&no_project);
    let shape = product(&file, 100);
    let (r, d, _) = with(Limits::default(), |cx| file.extrusion(leaf(&shape, 30), cx));
    assert!(r.unwrap().length_unit.is_none());
    assert!(d.iter().any(|d| d.code() == codes::IFC_UNIT_MISSING));
    let file = read(&base.replace(
        "IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.)",
        "IFCCONTEXTDEPENDENTUNIT($,.LENGTHUNIT.,'unknown')",
    ));
    let shape = product(&file, 100);
    assert!(matches!(
        with(Limits::default(), |cx| file.extrusion(leaf(&shape, 30), cx)).0,
        Err(Error::Unsupported)
    ));
}

fn invalid_unit_cases(version: &str) -> Vec<(&'static str, String, u64)> {
    let src = source(version);
    let conversion=src.replace("#3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);", "#3=IFCCONVERSIONBASEDUNIT(#6,.LENGTHUNIT.,'length',#4);\n#4=IFCMEASUREWITHUNIT(IFCLENGTHMEASURE(0.3048),#5);\n#5=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n#6=IFCDIMENSIONALEXPONENTS(1,0,0,0,0,0,0);");
    vec![
        ("si-unknown-prefix", src.replace(".MILLI.", ".BOGUS."), 3),
        ("si-numeric-prefix", src.replace(".MILLI.", "1."), 3),
        (
            "si-missing-prefix",
            src.replace("*,.LENGTHUNIT.,.MILLI.,.METRE.)", "*,.LENGTHUNIT.,.METRE.)"),
            3,
        ),
        ("si-derived-prefix", src.replace(".MILLI.", "*"), 3),
        ("si-lower-prefix", src.replace(".MILLI.", ".milli."), 3),
        ("si-area-name", src.replace(".METRE.", ".SQUARE_METRE."), 3),
        ("si-unknown-name", src.replace(".METRE.", ".BOGUS."), 3),
        ("si-numeric-name", src.replace(".METRE.", "1."), 3),
        (
            "si-explicit-dimensions",
            src.replace("IFCSIUNIT(*,", "IFCSIUNIT($,"),
            3,
        ),
        (
            "base-unknown-prefix",
            conversion.replace(".MILLI.", ".BOGUS."),
            5,
        ),
        (
            "base-numeric-prefix",
            conversion.replace(".MILLI.", "1."),
            5,
        ),
        ("base-derived-prefix", conversion.replace(".MILLI.", "*"), 5),
        (
            "base-missing-prefix",
            conversion.replace("*,.LENGTHUNIT.,.MILLI.,.METRE.)", "*,.LENGTHUNIT.,.METRE.)"),
            5,
        ),
        (
            "base-area-name",
            conversion.replace(".METRE.", ".SQUARE_METRE."),
            5,
        ),
        (
            "conversion-no-dimensions",
            conversion.replace("IFCCONVERSIONBASEDUNIT(#6,", "IFCCONVERSIONBASEDUNIT($,"),
            3,
        ),
        (
            "conversion-missing-dimensions-reference",
            conversion.replace("IFCCONVERSIONBASEDUNIT(#6,", "IFCCONVERSIONBASEDUNIT(#999,"),
            3,
        ),
        (
            "conversion-numeric-dimensions-reference",
            conversion.replace("IFCCONVERSIONBASEDUNIT(#6,", "IFCCONVERSIONBASEDUNIT(1,"),
            3,
        ),
        (
            "conversion-area-dimensions",
            conversion.replace("EXPONENTS(1,0,0,0,0,0,0)", "EXPONENTS(2,0,0,0,0,0,0)"),
            6,
        ),
        (
            "conversion-mass-dimensions",
            conversion.replace("EXPONENTS(1,0,0,0,0,0,0)", "EXPONENTS(1,1,0,0,0,0,0)"),
            6,
        ),
        (
            "conversion-last-dimension",
            conversion.replace("EXPONENTS(1,0,0,0,0,0,0)", "EXPONENTS(1,0,0,0,0,0,1)"),
            6,
        ),
        (
            "conversion-short-dimensions",
            conversion.replace("EXPONENTS(1,0,0,0,0,0,0)", "EXPONENTS(1,0,0)"),
            6,
        ),
        (
            "conversion-real-dimensions",
            conversion.replace("EXPONENTS(1,0,0,0,0,0,0)", "EXPONENTS(1.,0,0,0,0,0,0)"),
            6,
        ),
        (
            "conversion-cyclic-dimensions",
            conversion.replace("IFCCONVERSIONBASEDUNIT(#6,", "IFCCONVERSIONBASEDUNIT(#3,"),
            3,
        ),
    ]
}

#[test]
fn rejects_invalid_si_prefix_names_and_conversion_dimensions_in_three_versions() {
    let mut unexpected = Vec::new();
    for version in VERSIONS {
        for (label, src, place) in invalid_unit_cases(version) {
            let file = read(&src);
            let shape = product(&file, 100);
            for id in [30, 31, 32] {
                let (result, diag, budget) =
                    with(Limits::default(), |cx| file.extrusion(leaf(&shape, id), cx));
                if !matches!(result, Err(Error::Invalid)) {
                    unexpected.push(format!("{version}/{id}/{label}"));
                    continue;
                }
                assert_eq!(budget.used(Kind::Depth), 0);
                let span = file.instance(place).unwrap().span();
                let expected = zumen_base::Ref::at_bytes(span.start as u64, span.end as u64);
                assert!(
                    diag.iter()
                        .filter(|d| d.code() == codes::IFC_EXTRUSION_INVALID)
                        .flat_map(|d| d.places())
                        .any(|p| p.bytes() == expected.bytes()),
                    "{label}/{diag:?}"
                );
            }
        }
    }
    assert!(
        unexpected.is_empty(),
        "不正単位の拒否が契約と異なる: {unexpected:?}"
    );
}

#[test]
fn keeps_all_sixteen_si_prefixes_and_null_for_direct_and_conversion_base_units() {
    for version in VERSIONS {
        for prefix in [
            "$", ".EXA.", ".PETA.", ".TERA.", ".GIGA.", ".MEGA.", ".KILO.", ".HECTO.", ".DECA.",
            ".DECI.", ".CENTI.", ".MILLI.", ".MICRO.", ".NANO.", ".PICO.", ".FEMTO.", ".ATTO.",
        ] {
            let direct = read(&source(version).replace(".MILLI.", prefix));
            let converted = read(&conversion_source(version).replace(".MILLI.", prefix));
            for id in [30, 31, 32] {
                let direct = value(&direct, id).length_unit.unwrap();
                assert_eq!(
                    direct.prefix.as_deref(),
                    if prefix == "$" {
                        None
                    } else {
                        Some(prefix.trim_matches('.'))
                    }
                );
                assert_eq!(direct.name, "METRE");
                assert_eq!(direct.scale, None);
                let converted = value(&converted, id);
                let unit = converted.length_unit.as_ref().unwrap();
                assert_eq!(unit.name, "scaled_length");
                assert_eq!(unit.prefix, None);
                assert!(unit.scale.is_some_and(|s| s.is_finite() && s > 0.));
                assert!(
                    converted
                        .source
                        .records
                        .iter()
                        .any(|r| r.id() == Some(6) && converted.source.solid.id() == Some(id))
                );
            }
        }
    }
}

fn dimension_stop_trace(file: &IfcFile, id: u64) -> String {
    let shape = product(file, 100);
    let node = leaf(&shape, id);
    let span = file.instance(6).unwrap().span();
    let place = zumen_base::Ref::at_bytes(span.start as u64, span.end as u64);
    let mut trace = String::new();
    for kind in [Kind::Work, Kind::DecodedBytes] {
        let mut limit = 0;
        let mut finished = false;
        let mut dimension_stops = 0;
        for checkpoint in 0..256 {
            let (result, diag, budget) =
                with(Limits::default().with(kind, Limit::Finite(limit)), |cx| {
                    file.extrusion(node, cx)
                });
            assert_eq!(budget.used(Kind::Depth), 0);
            match result {
                Ok(result) => {
                    assert!(result.source.records.iter().any(|r| r.id() == Some(6)));
                    finished = true;
                    break;
                }
                Err(Error::Stopped(zumen_base::Stop::Exceeded(exceeded)))
                    if exceeded.kind == kind =>
                {
                    assert!(exceeded.requested > limit);
                    if diag
                        .iter()
                        .flat_map(|d| d.places())
                        .any(|p| p.bytes() == place.bytes())
                    {
                        dimension_stops += 1;
                        if kind == Kind::DecodedBytes {
                            assert_eq!(exceeded.requested, budget.used(kind) + 32);
                        }
                        trace += &format!(
                            "{kind:?}/{checkpoint}/{limit}/{exceeded:?}\t{diag:?}\t{:?}\n",
                            Kind::ALL.map(|k| budget.used(k))
                        );
                    }
                    limit = exceeded.requested;
                }
                other => panic!("次元参照の停止: {other:?}/{diag:?}"),
            }
        }
        assert!(finished);
        assert!(dimension_stops > 0, "{id}/{kind:?}");
    }
    trace
}

#[test]
fn reserves_conversion_dimensions_before_reading_and_reports_their_stop_span() {
    for version in VERSIONS {
        let file = read(&conversion_source(version));
        for id in [30, 32] {
            assert!(!dimension_stop_trace(&file, id).is_empty());
        }
    }
}

fn scan_limits(version: &str) -> Vec<(usize, u64)> {
    let src = source(version)
        .lines()
        .filter(|line| !line.starts_with("#1="))
        .collect::<Vec<_>>()
        .join("\n");
    let file = read(&src);
    let shape = product(&file, 100);
    let (_, diag, b) = with(Limits::default(), |cx| file.extrusion(leaf(&shape, 30), cx));
    // 単位なしの診断の収集に使う Work を走査の前半へ含めない。
    let (_, _, diagnostic_budget) = with(Limits::default(), |cx| {
        cx.with_diagnostics_in_record_order::<(), Error>(|cx| {
            for diagnostic in diag.iter() {
                cx.report(diagnostic.clone());
            }
            Ok(())
        })
    });
    let charge = |r: &zumen_ifc::Record| {
        if r.id().is_none() {
            1
        } else {
            1 + r.parts().len() as u64
                + r.parts().iter().map(|p| p.name().len() as u64).sum::<u64>()
        }
    };
    let total = file.records().iter().map(charge).sum::<u64>();
    let mut limit = b.used(Kind::Work) - total - diagnostic_budget.used(Kind::Work);
    let last_instance = file
        .records()
        .iter()
        .rposition(|r| r.id().is_some())
        .unwrap();
    file.records()
        .iter()
        .enumerate()
        .take(last_instance + 1)
        .map(|(index, record)| {
            let pair = (index, limit);
            limit += charge(record);
            pair
        })
        .collect()
}
fn late_project_source(version: &str) -> String {
    let src = source(version);
    let project = src.lines().find(|line| line.starts_with("#1=")).unwrap();
    src.lines()
        .filter(|line| !line.starts_with("#1="))
        .collect::<Vec<_>>()
        .join("\n")
        .replace("ENDSEC;\nEND-ISO", &format!("{project}\nENDSEC;\nEND-ISO"))
}

#[test]
fn unit_scan_stops_at_the_visited_record_in_multiple_positions() {
    for version in VERSIONS {
        let file = read(&late_project_source(version));
        let shape = product(&file, 100);
        let positions = scan_limits(version);
        for index in [0, positions.len() / 2, positions.len() - 1] {
            let (record_index, limit) = positions[index];
            let (r, d, b) = with(
                Limits::default().with(Kind::Work, Limit::Finite(limit)),
                |cx| file.extrusion(leaf(&shape, 30), cx),
            );
            assert!(
                matches!(r, Err(Error::Stopped(_))),
                "{version}/{record_index}/{limit}"
            );
            let span = file.records()[record_index].span();
            assert_eq!(
                d.iter().next().unwrap().places(),
                &[zumen_base::Ref::at_bytes(
                    span.start as u64,
                    span.end as u64
                )]
            );
            assert_eq!(b.used(Kind::Depth), 0);
            assert_eq!(b.used(Kind::Work), limit);
        }
        let (last_index, last_limit) = *positions.last().unwrap();
        let last = &file.records()[last_index];
        let start = last_limit
            + 1
            + last.parts().len() as u64
            + last
                .parts()
                .iter()
                .map(|p| p.name().len() as u64)
                .sum::<u64>();
        let src=late_project_source(version).replace("#1=IFCPROJECT","#3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n#3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n#1=IFCPROJECT");
        let duplicate = read(&src);
        let shape = product(&duplicate, 100);
        for offset in 0..2 {
            let (r, d, b) = with(
                Limits::default().with(Kind::Work, Limit::Finite(start + offset as u64)),
                |cx| duplicate.extrusion(leaf(&shape, 30), cx),
            );
            assert!(matches!(r, Err(Error::Stopped(_))));
            let span = duplicate.records()[positions.len() + offset].span();
            assert_eq!(
                d.iter().next().unwrap().places(),
                &[zumen_base::Ref::at_bytes(
                    span.start as u64,
                    span.end as u64
                )]
            );
            assert_eq!(b.used(Kind::Work), start + offset as u64);
        }
    }
}

#[test]
fn controlled_mutations_finish_with_repeatable_diagnostics_and_usage() {
    let src = source("IFC4");
    for (from, to) in [
        ("#12,15.,5.)", "#12,0.,5.)"),
        ("#12,15.,5.)", "#12,$,5.)"),
        ("#10,#11)", "#12,#11)"),
        ("#13,#20,#24", "#13,#30,#24"),
        ("#13,#20,#24", "#13,#20,#999"),
        ("IFCRECTANGLEPROFILEDEF", "IFCISHAPEPROFILEDEF"),
    ] {
        let file = read(&src.replace(from, to));
        let shape = product(&file, 100);
        let execute = || {
            with(
                Limits::default().with(Kind::Work, Limit::Finite(20_000)),
                |cx| file.extrusion(leaf(&shape, 30), cx),
            )
        };
        let (a, da, ba) = execute();
        let (b, db, bb) = execute();
        assert!(a.is_err());
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
        assert_eq!(format!("{da:?}"), format!("{db:?}"));
        for kind in Kind::ALL {
            assert_eq!(ba.used(kind), bb.used(kind));
        }
        assert_eq!(ba.used(Kind::Depth), 0);
    }
}

fn numbers(out: &mut Writer, v: &[f64]) {
    out.begin_array();
    for &n in v {
        out.float(n);
    }
    out.end_array();
}
fn optional_numbers(out: &mut Writer, v: Option<&[f64]>) {
    if let Some(v) = v {
        numbers(out, v)
    } else {
        out.null()
    }
}
fn text(out: &mut Writer, v: Option<&str>) {
    if let Some(v) = v {
        out.string(v)
    } else {
        out.null()
    }
}
fn unit(out: &mut Writer, v: Option<&zumen_building::Unit>) {
    if let Some(v) = v {
        out.begin_object();
        out.key("kind");
        out.string(&v.kind);
        out.key("name");
        out.string(&v.name);
        out.key("prefix");
        text(out, v.prefix.as_deref());
        out.key("scale");
        if let Some(n) = v.scale {
            out.float(n)
        } else {
            out.null()
        };
        out.end_object();
    } else {
        out.null()
    }
}
#[path = "support/curve_json.rs"]
mod curve_json;
fn write_item(out: &mut Writer, file: &IfcFile, shape: &ProductShape<'_>, id: u64) {
    let (result, d, b) = with(Limits::default(), |cx| file.extrusion(leaf(shape, id), cx));
    assert!(d.is_empty(), "{d:?}");
    let result = result.unwrap();
    let def = &result.definition;
    out.begin_object();
    out.key("instance");
    out.uint(id);
    out.key("profile");
    out.uint(result.source.profile.id().unwrap());
    out.key("profile_name");
    text(out, def.profile.name.as_deref());
    out.key("profile_position");
    if let Some(p) = &def.profile.position {
        out.begin_object();
        out.key("location");
        numbers(out, &p.location);
        out.key("reference_direction");
        optional_numbers(out, p.reference_direction.as_ref().map(|v| v.as_slice()));
        out.end_object();
    } else {
        out.null()
    }
    out.key("profile_kind");
    out.string(match def.profile.kind {
        AreaProfileKind::Rectangle { .. } => "rectangle",
        AreaProfileKind::Circle { .. } => "circle",
        AreaProfileKind::ArbitraryClosed { .. } => "arbitrary-closed",
    });
    out.key("dimensions");
    match def.profile.kind {
        AreaProfileKind::Rectangle { x_dim, y_dim } => numbers(out, &[x_dim, y_dim]),
        AreaProfileKind::Circle { radius } => numbers(out, &[radius]),
        _ => numbers(out, &[]),
    };
    out.key("position");
    if let Some(p) = &def.position {
        out.begin_object();
        out.key("location");
        numbers(out, &p.location);
        out.key("axis");
        optional_numbers(out, p.axis.as_ref().map(|v| v.as_slice()));
        out.key("reference_direction");
        optional_numbers(out, p.reference_direction.as_ref().map(|v| v.as_slice()));
        out.end_object();
    } else {
        out.null()
    }
    out.key("direction");
    numbers(out, &def.direction);
    out.key("depth");
    out.float(def.depth);
    out.key("unit");
    unit(out, result.length_unit.as_ref());
    out.key("boundary");
    if let Some(graph) = &result.boundary {
        out.begin_array();
        curve_json::write_graph(out, graph);
        out.end_array();
    } else {
        out.null()
    }
    out.key("source_records");
    out.begin_array();
    for r in &result.source.records {
        out.uint(r.id().unwrap());
    }
    out.end_array();
    out.key("budgets");
    out.begin_array();
    for kind in Kind::ALL {
        out.uint(b.used(kind));
    }
    out.end_array();
    out.end_object();
}
fn contract_line(src: &str, label: &str, id: u64, limits: Limits, signal: u8) -> String {
    let file = read(src);
    let shape = product(&file, 100);
    let (r, d, b) = with(limits, |cx| {
        match signal {
            1 => cx.cancel.cancel(),
            2 => cx.cancel.expire(),
            _ => {}
        };
        file.extrusion(leaf(&shape, id), cx)
    });
    format!(
        "{label}\t{}\t{d:?}\t{:?}\n",
        match r {
            Ok(_) => "ok".to_owned(),
            Err(e) => format!("{e:?}"),
        },
        Kind::ALL.map(|kind| b.used(kind))
    )
}
fn contract_export() -> String {
    let mut out = String::new();
    for version in VERSIONS {
        let src = source(version);
        let file = read(&src);
        let shape = product(&file, 100);
        for id in [30, 31, 32] {
            let (_, _, b) = with(Limits::default(), |cx| file.extrusion(leaf(&shape, id), cx));
            for kind in Kind::ALL {
                let n = if kind == Kind::Depth { 3 } else { b.used(kind) };
                for limit in [n.saturating_sub(1), n] {
                    out += &contract_line(
                        &src,
                        &format!("{version}/{id}/{kind:?}/{limit}"),
                        id,
                        Limits::default().with(kind, Limit::Finite(limit)),
                        0,
                    );
                }
            }
            for signal in 1..=2 {
                out += &contract_line(
                    &src,
                    &format!("{version}/{id}/signal{signal}"),
                    id,
                    Limits::default(),
                    signal,
                );
            }
        }
        let src = late_project_source(version);
        for limit in 0..160 {
            out += &contract_line(
                &src,
                &format!("{version}/scan/{limit}"),
                30,
                Limits::default().with(Kind::Work, Limit::Finite(limit)),
                0,
            );
        }
        for (id, limit) in scan_limits(version) {
            out += &contract_line(
                &src,
                &format!("{version}/scan-record/{id}"),
                30,
                Limits::default().with(Kind::Work, Limit::Finite(limit)),
                0,
            );
        }
        for (from, to) in [
            ("#12,15.,5.)", "#12,-0.,5.)"),
            ("IFCDIRECTION((0.,4.))", "IFCDIRECTION((0.,0.))"),
            ("'Rectangle',#12", "'Rectangle',$"),
            ("IFCRECTANGLEPROFILEDEF", "IFCISHAPEPROFILEDEF"),
        ] {
            out += &contract_line(
                &source(version).replace(from, to),
                &format!("{version}/{to}"),
                30,
                Limits::default(),
                0,
            );
        }
    }
    for id in [30, 32] {
        out += &contract_line(
            &comparison_source("IFC4"),
            &format!("IFC4/converted/{id}"),
            id,
            Limits::default(),
            0,
        );
    }
    for version in VERSIONS {
        for (label, src, _) in invalid_unit_cases(version) {
            for id in [30, 31, 32] {
                out += &contract_line(
                    &src,
                    &format!("{version}/unit/{label}/{id}"),
                    id,
                    Limits::default(),
                    0,
                );
            }
        }
        let src = conversion_source(version);
        let file = read(&src);
        for id in [30, 32] {
            out += &format!(
                "{version}/dimensions/{id}\n{}",
                dimension_stop_trace(&file, id)
            );
            for signal in 1..=2 {
                out += &contract_line(
                    &src,
                    &format!("{version}/converted/{id}/signal{signal}"),
                    id,
                    Limits::default(),
                    signal,
                );
            }
        }
    }
    out
}

#[test]
#[allow(clippy::disallowed_methods)] // 外部照合の出力先だけを受け取る
fn writes_profiles_for_other_readers() {
    let Some(folder) = std::env::var_os("ZUMEN_IFC_PROFILES_OUT") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    for version in VERSIONS {
        let src = comparison_source(version);
        let file = read(&src);
        let shape = product(&file, 100);
        let name = format!("{version}.ifc");
        std::fs::write(folder.join(&name), src).unwrap();
        let mut out = Writer::new();
        out.begin_object();
        out.key("file");
        out.string(&name);
        out.key("product");
        out.uint(100);
        out.key("items");
        out.begin_array();
        for id in [30, 31, 32] {
            write_item(&mut out, &file, &shape, id)
        }
        out.end_array();
        out.end_object();
        std::fs::write(folder.join(format!("{version}.json")), out.into_bytes()).unwrap();
    }
    for (src, product_id, solid, _, name, _) in public_cases() {
        let file = read(src);
        let shape = product(&file, product_id);
        std::fs::write(folder.join(format!("{name}.ifc")), src).unwrap();
        let mut out = Writer::new();
        out.begin_object();
        out.key("file");
        out.string(&format!("{name}.ifc"));
        out.key("product");
        out.uint(product_id);
        out.key("items");
        out.begin_array();
        write_item(&mut out, &file, &shape, solid);
        out.end_array();
        out.end_object();
        std::fs::write(folder.join(format!("{name}.json")), out.into_bytes()).unwrap();
    }
    std::fs::write(folder.join("contract.txt"), contract_export()).unwrap();
}
