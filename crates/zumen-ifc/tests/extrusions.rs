//! 要素の葉から押し出し・閉じた断面・曲線を一度に取得する。入力は合成と公開サンプルだけ。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{Building, Coordinates, CurveKind, Hash};
use zumen_ifc::{ExtrudedArea, Ifc, IfcFile, ProductShape, ShapeNode};
use zumen_json::Writer;
use zumen_sdk::{Codec, Error, Lift};

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];
fn source(schema: &str) -> String {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('extrusions'),'2;1');\nFILE_NAME('extrusions','',(),(),'','','');\nFILE_SCHEMA(('{schema}'));\nENDSEC;\nDATA;\n\
    #1=IFCPROJECT('0000000000000000000001',$,$,$,$,$,$,$,#2);\n\
    #2=IFCUNITASSIGNMENT((#3,#4));\n\
    #3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n\
    #4=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);\n\
    #8=IFCGEOMETRICREPRESENTATIONCONTEXT('Model','Model',3,0.00001,#20,$);\n\
    #10=IFCCARTESIANPOINT((0.,0.));\n\
    #11=IFCCARTESIANPOINT((5.,0.));\n\
    #12=IFCCARTESIANPOINT((0.,5.));\n\
    #14=IFCPOLYLINE((#10,#11,#12,#10));\n\
    #15=IFCARBITRARYCLOSEDPROFILEDEF(.AREA.,'Profile',#14);\n\
    #18=IFCCIRCLE(#19,2.);\n\
    #19=IFCAXIS2PLACEMENT2D(#10,$);\n\
    #20=IFCAXIS2PLACEMENT3D(#21,#22,#23);\n\
    #21=IFCCARTESIANPOINT((10.,20.,30.));\n\
    #22=IFCDIRECTION((0.,2.,0.));\n\
    #23=IFCDIRECTION((3.,0.,0.));\n\
    #24=IFCDIRECTION((2.,-3.,4.));\n\
    #25=IFCARBITRARYCLOSEDPROFILEDEF(.AREA.,$,#18);\n\
    #30=IFCEXTRUDEDAREASOLID(#15,#20,#24,12.5);\n\
    #31=IFCEXTRUDEDAREASOLID(#25,#20,#24,1.);\n\
    #40=IFCMAPPEDITEM(#41,#43);\n\
    #41=IFCREPRESENTATIONMAP(#20,#42);\n\
    #42=IFCSHAPEREPRESENTATION(#8,'Body','SweptSolid',(#30,#31));\n\
    #43=IFCCARTESIANTRANSFORMATIONOPERATOR3DNONUNIFORM($,$,#21,2.,$,3.,4.);\n\
    #44=IFCMAPPEDITEM(#41,#43);\n\
    #100=IFCWALL('0000000000000000000002',$,$,$,$,#200,#101,${wall_type});\n\
    #101=IFCPRODUCTDEFINITIONSHAPE($,$,(#102));\n\
    #102=IFCSHAPEREPRESENTATION(#8,'Body','MappedRepresentation',(#40,#44));\n\
    #200=IFCLOCALPLACEMENT($,#20);\n\
    {indexed}\nENDSEC;\nEND-ISO-10303-21;\n",
        wall_type = if schema == "IFC2X3" {
            ""
        } else {
            ",.NOTDEFINED."
        },
        indexed = if schema == "IFC2X3" {
            ""
        } else {
            "#16=IFCCARTESIANPOINTLIST2D(((0.,0.),(5.,0.),(5.,5.),(0.,5.),(0.,0.)));\n#17=IFCINDEXEDPOLYCURVE(#16,$,.F.);"
        }
    )
}
fn with<T>(limits: Limits, f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics, Budget) {
    let mut b = Budget::new(limits);
    let mut d = Diagnostics::new();
    let cancel = CancelToken::new();
    let value = f(&mut Cx::new(&mut b, &mut d, &cancel));
    (value, d, b)
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
fn value(file: &IfcFile, id: u64) -> ExtrudedArea<'_> {
    let shape = product(file, 100);
    let (v, d, _) = with(Limits::default(), |cx| {
        file.extruded_area(leaf(&shape, id), cx)
    });
    assert!(d.is_empty(), "{d:?}");
    v.unwrap()
}

#[test]
fn gets_extrusion_profile_and_curve_from_the_existing_element_path_in_three_versions() {
    for schema in VERSIONS {
        let file = read(&source(schema));
        let shape = product(&file, 100);
        assert_eq!(
            shape
                .nodes
                .iter()
                .filter(|n| n.source.id() == Some(30))
                .count(),
            1
        );
        let result = value(&file, 30);
        assert_eq!(result.definition.depth, 12.5);
        assert_eq!(result.definition.direction, [2., -3., 4.]);
        let position = result.definition.position.as_ref().unwrap();
        assert_eq!(position.location, [10., 20., 30.]);
        assert_eq!(position.axis, Some([0., 2., 0.]));
        assert_eq!(position.reference_direction, Some([3., 0., 0.]));
        assert_eq!(result.definition.profile.name.as_deref(), Some("Profile"));
        assert_eq!(result.definition.profile.boundary, result.boundary.root);
        assert_eq!(
            result
                .boundary
                .length_unit
                .as_ref()
                .unwrap()
                .prefix
                .as_deref(),
            Some("MILLI")
        );
        assert!(
            matches!(&result.boundary.items[0].kind,CurveKind::Polyline(points) if points==&[Coordinates::Two([0.,0.]),Coordinates::Two([5.,0.]),Coordinates::Two([0.,5.]),Coordinates::Two([0.,0.])])
        );
        assert!(std::ptr::eq(
            result.source.solid,
            file.instance(30).unwrap()
        ));
        assert_eq!(result.source.profile.id(), Some(15));
        assert!(result.source.records.iter().any(|r| r.id() == Some(20)));
        assert!(matches!(
            value(&file, 31).boundary.items[0].kind,
            CurveKind::Circle { radius: 2., .. }
        ));
        if schema != "IFC2X3" {
            let file = read(&source(schema).replace("'Profile',#14", "'Profile',#17"));
            assert!(matches!(
                value(&file, 30).boundary.items[0].kind,
                CurveKind::Indexed { .. }
            ));
        }
    }
}

#[test]
fn keeps_optional_position_and_unknown_units_without_applying_defaults() {
    for schema in VERSIONS {
        let src = source(schema).replace("#15,#20,#24,12.5", "#15,$,#24,12.5");
        let file = read(&src);
        let shape = product(&file, 100);
        let (r, _, b) = with(Limits::default(), |cx| {
            file.extruded_area(leaf(&shape, 30), cx)
        });
        if schema == "IFC2X3" {
            assert!(matches!(r, Err(Error::Invalid)));
        } else {
            assert!(r.unwrap().definition.position.is_none());
        }
        assert_eq!(b.used(Kind::Depth), 0);
    }
    let file = read(&source("IFC4").replace("$,$,$,#2);", "$,$,$,$);"));
    let shape = product(&file, 100);
    let (r, d, _) = with(Limits::default(), |cx| {
        file.extruded_area(leaf(&shape, 30), cx)
    });
    assert!(r.unwrap().boundary.length_unit.is_none());
    assert!(d.iter().any(|d| d.code() == codes::IFC_UNIT_MISSING));
    let file = read(&source("IFC4").replace(
        "#20=IFCAXIS2PLACEMENT3D(#21,#22,#23)",
        "#20=IFCAXIS2PLACEMENT3D(#21,$,$)",
    ));
    let r = value(&file, 30);
    assert_eq!(r.definition.position.unwrap().axis, None);
    let file = read(&source("IFC4").replace("(2.,-3.,4.)", "(0.,0.,1.)"));
    assert_eq!(value(&file, 30).definition.direction, [0., 0., 1.]); // 局所 z。元の配置の軸への dot ではない
}

#[test]
fn rejects_invalid_depth_direction_placement_profile_and_closedness() {
    for schema in VERSIONS {
        let src = source(schema);
        for (from, to) in [
            ("#24,12.5)", "#24,0.)"),
            ("#24,12.5)", "#24,-1.)"),
            ("#24,12.5)", "#24,1.E999)"),
            ("#24,12.5)", "#24,$)"),
            ("(2.,-3.,4.)", "(1.,2.,0.)"),
            ("(2.,-3.,4.)", "(0.,0.,0.)"),
            ("(2.,-3.,4.)", "(1.,2.)"),
            ("(2.,-3.,4.)", "(1.,2.,1.E999)"),
            ("#21,#22,#23)", "#21,#22,$)"),
            ("#21,#22,#23)", "#21,#22,#22)"),
            ("(10.,20.,30.)", "(10.,20.)"),
            (".AREA.,'Profile'", ".CURVE.,'Profile'"),
            (".AREA.,'Profile'", ".AREA.,1."),
            ("(#10,#11,#12,#10)", "(#10,#11,#12)"),
            ("(#10,#11,#12,#10)", "(#10,#11,#10)"),
            ("(0.,0.)", "(0.,0.,0.)"),
            ("#15,#20,#24,12.5", "#999,#20,#24,12.5"),
            ("'Profile',#14", "'Profile',#999"),
        ] {
            let file = read(&src.replace(from, to));
            let shape = product(&file, 100);
            let (r, d, b) = with(Limits::default(), |cx| {
                file.extruded_area(leaf(&shape, 30), cx)
            });
            assert!(matches!(r, Err(Error::Invalid)), "{schema} {from} => {to}");
            assert!(!d.is_empty());
            assert_eq!(b.used(Kind::Depth), 0);
        }
        let file = read(
            &src.replace("'Profile',#14", "'Profile',#18")
                .replace("IFCAXIS2PLACEMENT2D(#10,$)", "IFCAXIS2PLACEMENT3D(#21,$,$)"),
        );
        let shape = product(&file, 100);
        assert!(matches!(
            with(Limits::default(), |cx| file
                .extruded_area(leaf(&shape, 30), cx))
            .0,
            Err(Error::Invalid)
        ));
    }
}

#[test]
fn reports_unsupported_solid_profile_and_unverified_boundaries_without_partial_results() {
    let src = source("IFC4");
    for (from, to) in [
        ("IFCEXTRUDEDAREASOLID(#15", "IFCREVOLVEDAREASOLID(#15"),
        (
            "IFCARBITRARYCLOSEDPROFILEDEF(.AREA.,'Profile',#14)",
            "IFCRECTANGLEPROFILEDEF(.AREA.,'Profile',$,5.,5.)",
        ),
        ("IFCPOLYLINE((#10,#11,#12,#10))", "IFCELLIPSE(#19,2.,3.)"),
    ] {
        let file = read(&src.replace(from, to));
        let shape = product(&file, 100);
        let (r, d, b) = with(Limits::default(), |cx| {
            file.extruded_area(leaf(&shape, 30), cx)
        });
        assert!(matches!(r, Err(Error::Unsupported)));
        assert!(
            d.iter()
                .any(|d| d.code() == codes::IFC_EXTRUSION_UNSUPPORTED)
        );
        assert_eq!(b.used(Kind::Depth), 0);
    }
    let file=read(&src.replace("IFCPOLYLINE((#10,#11,#12,#10))","IFCCOMPOSITECURVE((#60,#61),.F.)").replace("ENDSEC;\nEND-ISO","#60=IFCCOMPOSITECURVESEGMENT(.CONTINUOUS.,.T.,#18);\n#61=IFCCOMPOSITECURVESEGMENT(.CONTINUOUS.,.F.,#18);\nENDSEC;\nEND-ISO"));
    let shape = product(&file, 100);
    let (r, _, b) = with(Limits::default(), |cx| {
        file.extruded_area(leaf(&shape, 30), cx)
    });
    assert!(matches!(r, Err(Error::Unsupported)));
    assert_eq!(b.used(Kind::Depth), 0);
}

#[test]
fn rejects_a_foreign_source_and_curve_cycles() {
    let file = read(&source("IFC4"));
    let other = file.clone();
    let shape = product(&file, 100);
    assert!(matches!(
        with(Limits::default(), |cx| other
            .extruded_area(leaf(&shape, 30), cx))
        .0,
        Err(Error::Invalid)
    ));
    let src = source("IFC4").replace(
        "IFCPOLYLINE((#10,#11,#12,#10))",
        "IFCTRIMMEDCURVE(#14,(IFCPARAMETERVALUE(0.)),(IFCPARAMETERVALUE(1.)),.T.,.PARAMETER.)",
    );
    let file = read(&src);
    let shape = product(&file, 100);
    let (r, d, b) = with(Limits::default(), |cx| {
        file.extruded_area(leaf(&shape, 30), cx)
    });
    assert!(matches!(r, Err(Error::Invalid)));
    assert!(d.iter().any(|d| d.code() == codes::IFC_CURVE_INVALID));
    assert_eq!(b.used(Kind::Depth), 0);
}

#[test]
fn reserves_before_allocation_and_restores_depth_for_limits_and_cancel() {
    let file = read(&source("IFC4"));
    let shape = product(&file, 100);
    let node = leaf(&shape, 30);
    for kind in [Kind::DecodedBytes, Kind::Depth, Kind::Work] {
        let (r, _, b) = with(Limits::default().with(kind, Limit::Finite(0)), |cx| {
            file.extruded_area(node, cx)
        });
        assert!(matches!(r, Err(Error::Stopped(_))));
        assert_eq!(b.used(Kind::Depth), 0);
        if kind == Kind::DecodedBytes {
            assert_eq!(b.used(kind), 0);
        }
    }
    let (r, _, b) = with(Limits::default(), |cx| {
        cx.cancel.cancel();
        file.extruded_area(node, cx)
    });
    assert!(matches!(
        r,
        Err(Error::Stopped(zumen_base::Stop::Cancelled))
    ));
    assert_eq!(b.used(Kind::DecodedBytes), 0);
    let (_, _, full) = with(Limits::default(), |cx| file.extruded_area(node, cx));
    for kind in [Kind::DecodedBytes, Kind::Work] {
        for n in [full.used(kind) - 1, full.used(kind)] {
            let (r, _, b) = with(Limits::default().with(kind, Limit::Finite(n)), |cx| {
                file.extruded_area(node, cx)
            });
            assert_eq!(r.is_ok(), n == full.used(kind));
            assert_eq!(b.used(Kind::Depth), 0);
        }
    }
}

#[test]
fn meaning_is_separate_from_source_numbers_and_existing_building_hashes() {
    let src = source("IFC4");
    let file = read(&src);
    let before = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
        .0
        .unwrap();
    let hashes: Vec<_> = before
        .elements()
        .iter()
        .enumerate()
        .map(|(i, e)| {
            (
                e.id(),
                Hash::element(&before, zumen_building::ElementRef(i as u32)),
            )
        })
        .collect();
    let first = value(&file, 30);
    value(&file, 31);
    value(&file, 30);
    let after = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
        .0
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        hashes,
        after
            .elements()
            .iter()
            .enumerate()
            .map(|(i, e)| (
                e.id(),
                Hash::element(&after, zumen_building::ElementRef(i as u32))
            ))
            .collect::<Vec<_>>()
    );
    let renamed = read(
        &src.replace("#15", "#915")
            .replace("#14", "#914")
            .replace("#20", "#920"),
    );
    let second = value(&renamed, 30);
    assert_eq!(first.definition, second.definition);
    assert_eq!(first.boundary.items[0].kind, second.boundary.items[0].kind);
    assert_ne!(first.source.profile.id(), second.source.profile.id());
    assert_ne!(
        first.boundary.items[0].source.instance,
        second.boundary.items[0].source.instance
    );
    let changed = read(&src.replace("#24,12.5)", "#24,13.5)"));
    assert_ne!(first.definition, value(&changed, 30).definition);
    let changed = read(&src.replace("(5.,0.)", "(6.,0.)"));
    assert_ne!(
        first.boundary.items[0].kind,
        value(&changed, 30).boundary.items[0].kind
    );
    let changed = read(&src.replace(".MILLI.", ".CENTI."));
    assert_eq!(first.definition, value(&changed, 30).definition);
    assert_ne!(
        first.boundary.length_unit,
        value(&changed, 30).boundary.length_unit
    );
}

#[test]
fn checks_indexed_endpoint_coordinates_without_appending_or_reversing_points() {
    let src = source("IFC4").replace("'Profile',#14", "'Profile',#17");
    let file = read(&src.replace(
        "#17=IFCINDEXEDPOLYCURVE(#16,$,.F.)",
        "#17=IFCINDEXEDPOLYCURVE(#16,(IFCLINEINDEX((1,2,3)),IFCLINEINDEX((3,4,5))),.F.)",
    ));
    assert!(matches!(
        value(&file, 30).boundary.items[0].kind,
        CurveKind::Indexed { .. }
    ));
    let file = read(&src.replace(
        "#17=IFCINDEXEDPOLYCURVE(#16,$,.F.)",
        "#17=IFCINDEXEDPOLYCURVE(#16,(IFCLINEINDEX((1,2,3,4))),.F.)",
    ));
    let shape = product(&file, 100);
    assert!(matches!(
        with(Limits::default(), |cx| file
            .extruded_area(leaf(&shape, 30), cx))
        .0,
        Err(Error::Invalid)
    ));
    let file = read(&src.replace(
        "#17=IFCINDEXEDPOLYCURVE(#16,$,.F.)",
        "#17=IFCINDEXEDPOLYCURVE(#16,(IFCLINEINDEX((1,2)),IFCLINEINDEX((3,4,1))),.F.)",
    ));
    let shape = product(&file, 100);
    assert!(matches!(
        with(Limits::default(), |cx| file
            .extruded_area(leaf(&shape, 30), cx))
        .0,
        Err(Error::Invalid)
    ));
}

#[test]
fn restores_the_callers_existing_depth_on_success_and_stop() {
    let file = read(&source("IFC4"));
    let shape = product(&file, 100);
    let (_, _, b) = with(Limits::default(), |cx| {
        cx.enter().unwrap();
        file.extruded_area(leaf(&shape, 30), cx).unwrap()
    });
    assert_eq!(b.used(Kind::Depth), 1);
    let (_, _, b) = with(
        Limits::default().with(Kind::Depth, Limit::Finite(2)),
        |cx| {
            cx.enter().unwrap();
            assert!(matches!(
                file.extruded_area(leaf(&shape, 30), cx),
                Err(Error::Stopped(_))
            ));
        },
    );
    assert_eq!(b.used(Kind::Depth), 1);
}

#[test]
fn mutated_inputs_finish_with_repeatable_diagnostics_and_usage() {
    let src = source("IFC4");
    for at in (0..src.len()).step_by(37) {
        let mut bytes = src.as_bytes().to_vec();
        bytes[at] = b'$';
        zumen_sdk::testkit::survives_with(&bytes, |src, cx| {
            let file = Ifc::read(src, cx)?;
            let shape = match file.product_shape(100, cx) {
                Ok(s) => s,
                Err(Error::Stopped(s)) => return Err(Error::Stopped(s)),
                _ => return Ok(()),
            };
            if let Some(n) = shape.nodes.iter().find(|n| n.source.id() == Some(30))
                && let Err(Error::Stopped(s)) = file.extruded_area(n, cx)
            {
                return Err(Error::Stopped(s));
            }
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn gets_the_public_column_profile_without_reading_step_arguments_in_the_caller() {
    let file = read(include_str!("../../../samples/ifc/gni/model_9.ifc"));
    let shape = product(&file, 131);
    let (result, diag, _) = with(Limits::default(), |cx| {
        file.extruded_area(leaf(&shape, 114), cx)
    });
    assert!(diag.is_empty(), "{diag:?}");
    let result = result.unwrap();
    assert_eq!(result.definition.depth, 8.8);
    assert_eq!(result.definition.direction, [0., 0., 1.]);
    assert_eq!(
        result.definition.profile.name.as_deref(),
        Some("BETON 30X30")
    );
    assert_eq!(result.source.profile.id(), Some(112));
    assert_eq!(result.boundary.items[0].source.instance, Some(111));
    assert!(
        matches!(&result.boundary.items[0].kind,CurveKind::Indexed{points,..} if points.len()==5)
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn deep_curve_failure_unwinds_without_recursive_ownership_or_partial_values() {
    std::thread::Builder::new().stack_size(1<<20).spawn(||{
        let mut src=source("IFC4").replace("'Profile',#14","'Profile',#1000");let mut extra=String::new();
        for id in 1000..2024{extra.push_str(&format!("#{id}=IFCTRIMMEDCURVE(#{},(IFCPARAMETERVALUE(0.)),(IFCPARAMETERVALUE(1.)),.T.,.PARAMETER.);\n",if id==2023{18}else{id+1}));}
        src=src.replace("ENDSEC;\nEND-ISO",&(extra+"ENDSEC;\nEND-ISO"));let file=read(&src);let shape=product(&file,100);
        let (r,_,b)=with(Limits::default().with(Kind::Depth,Limit::Unlimited),|cx|file.extruded_area(leaf(&shape,30),cx));
        assert!(matches!(r,Err(Error::Unsupported)));assert_eq!(b.used(Kind::Depth),0);
        let (r,_,b)=with(Limits::default(),|cx|file.extruded_area(leaf(&shape,30),cx));assert!(matches!(r,Err(Error::Stopped(_))));assert_eq!(b.used(Kind::Depth),0);
    }).unwrap().join().unwrap();
}

fn numbers(out: &mut Writer, values: &[f64]) {
    out.begin_array();
    for &n in values {
        out.float(n);
    }
    out.end_array();
}
fn optional_numbers(out: &mut Writer, values: Option<&[f64]>) {
    if let Some(v) = values {
        numbers(out, v)
    } else {
        out.null()
    }
}
fn coordinates(out: &mut Writer, value: Coordinates) {
    match value {
        Coordinates::Two(v) => numbers(out, &v),
        Coordinates::Three(v) => numbers(out, &v),
    }
}
fn unit(out: &mut Writer, value: Option<&zumen_building::Unit>) {
    if let Some(v) = value {
        out.begin_object();
        out.key("kind");
        out.string(&v.kind);
        out.key("name");
        out.string(&v.name);
        out.key("prefix");
        if let Some(p) = &v.prefix {
            out.string(p)
        } else {
            out.null()
        };
        out.key("scale");
        if let Some(n) = v.scale {
            out.float(n)
        } else {
            out.null()
        };
        out.end_object();
    } else {
        out.null();
    }
}
fn boundary_json(out: &mut Writer, area: &ExtrudedArea<'_>) {
    let root = area.boundary.item(area.boundary.root).unwrap();
    out.begin_object();
    out.key("instance");
    out.uint(root.source.instance.unwrap());
    out.key("kind");
    match &root.kind {
        CurveKind::Polyline(points) => {
            out.string("polyline");
            out.key("points");
            out.begin_array();
            for &p in points {
                coordinates(out, p)
            }
            out.end_array();
        }
        CurveKind::Indexed {
            points,
            segments,
            self_intersect,
        } => {
            out.string("indexed");
            out.key("points");
            out.begin_array();
            for &p in points {
                coordinates(out, p)
            }
            out.end_array();
            out.key("segments");
            if let Some(segments) = segments {
                out.begin_array();
                for s in segments {
                    out.begin_object();
                    out.key("kind");
                    let (kind, indices) = match s {
                        zumen_building::IndexedSegment::Line(v) => ("line", v.as_slice()),
                        zumen_building::IndexedSegment::Arc(v) => ("arc", v.as_slice()),
                    };
                    out.string(kind);
                    out.key("indices");
                    out.begin_array();
                    for &i in indices {
                        out.uint(u64::from(i))
                    }
                    out.end_array();
                    out.end_object();
                }
                out.end_array();
            } else {
                out.null();
            }
            out.key("self_intersect");
            if let Some(b) = self_intersect {
                out.bool(*b)
            } else {
                out.null();
            }
        }
        CurveKind::Circle { position, radius } => {
            out.string("circle");
            out.key("location");
            coordinates(out, position.location);
            out.key("axis");
            optional_numbers(out, position.axis.as_ref().map(|a| a.as_slice()));
            out.key("reference_direction");
            if let Some(d) = position.reference_direction {
                coordinates(out, d)
            } else {
                out.null();
            }
            out.key("radius");
            out.float(*radius);
        }
        _ => panic!("閉鎖の対象にない曲線"),
    }
    out.end_object();
}
fn write_area(out: &mut Writer, file: &IfcFile, shape: &ProductShape<'_>, id: u64) {
    let (r, d, b) = with(Limits::default(), |cx| {
        file.extruded_area(leaf(shape, id), cx)
    });
    assert!(d.is_empty(), "{d:?}");
    let area = r.unwrap();
    out.begin_object();
    out.key("instance");
    out.uint(area.source.solid.id().unwrap());
    out.key("profile");
    out.uint(area.source.profile.id().unwrap());
    out.key("profile_name");
    if let Some(name) = &area.definition.profile.name {
        out.string(name)
    } else {
        out.null();
    }
    out.key("direction");
    numbers(out, &area.definition.direction);
    out.key("depth");
    out.float(area.definition.depth);
    out.key("position");
    if let Some(p) = &area.definition.position {
        out.begin_object();
        out.key("location");
        numbers(out, &p.location);
        out.key("axis");
        optional_numbers(out, p.axis.as_ref().map(|a| a.as_slice()));
        out.key("reference_direction");
        optional_numbers(out, p.reference_direction.as_ref().map(|a| a.as_slice()));
        out.end_object();
    } else {
        out.null();
    }
    out.key("unit");
    unit(out, area.boundary.length_unit.as_ref());
    out.key("angle_unit");
    unit(out, area.boundary.angle_unit.as_ref());
    out.key("boundary");
    boundary_json(out, &area);
    out.key("source_records");
    out.begin_array();
    for r in &area.source.records {
        out.uint(r.id().unwrap());
    }
    out.end_array();
    out.key("curve_source");
    out.begin_array();
    for r in &area.boundary.items {
        out.begin_array();
        out.uint(r.source.instance.unwrap());
        for &id in &r.source.records {
            out.uint(id);
        }
        out.end_array();
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

#[test]
#[allow(clippy::disallowed_methods)] // 外部照合の出力先だけを受け取る
fn writes_extrusions_for_other_readers() {
    let Some(folder) = std::env::var_os("ZUMEN_IFC_EXTRUSIONS_OUT") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    for schema in VERSIONS {
        let mut src = source(schema);
        if schema != "IFC2X3" {
            src = src.replace("'Profile',#14", "'Profile',#17");
        }
        let file = read(&src);
        let shape = product(&file, 100);
        let name = format!("{schema}.ifc");
        std::fs::write(folder.join(&name), &src).unwrap();
        let mut out = Writer::new();
        out.begin_object();
        out.key("file");
        out.string(&name);
        out.key("items");
        out.begin_array();
        for id in [30, 31] {
            write_area(&mut out, &file, &shape, id);
        }
        out.end_array();
        out.end_object();
        std::fs::write(folder.join(format!("{schema}.json")), out.into_bytes()).unwrap();
    }
    let src = include_str!("../../../samples/ifc/gni/model_9.ifc");
    let file = read(src);
    let shape = product(&file, 131);
    std::fs::write(folder.join("public.ifc"), src).unwrap();
    let mut out = Writer::new();
    out.begin_object();
    out.key("file");
    out.string("public.ifc");
    out.key("items");
    out.begin_array();
    write_area(&mut out, &file, &shape, 114);
    out.end_array();
    out.end_object();
    std::fs::write(folder.join("public.json"), out.into_bytes()).unwrap();
}
