//! 文脈の座標系・親・表示尺度と、共有する予算を確かめる。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{ContextPlacement, TargetView};
use zumen_ifc::{ContextGeometry, Ifc, IfcFile};
use zumen_json::Writer;
use zumen_sdk::{Codec, Error};

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];
fn source(version: &str) -> String {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\nFILE_NAME('','',(''),(''),'','','');\nFILE_SCHEMA(('{version}'));\nENDSEC;\nDATA;\n\
    #1=IFCPROJECT('0000000000000000000001',$,'P',$,$,$,$,(#8,#18),#2);\n\
    #2=IFCUNITASSIGNMENT((#3));\n#3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n\
    #8=IFCGEOMETRICREPRESENTATIONCONTEXT('Parent','Model',3,0.001,#20,#26);\n\
    #10=IFCGEOMETRICREPRESENTATIONSUBCONTEXT('Body','Model',*,*,*,*,#8,0.01,.MODEL_VIEW.,$);\n\
    #18=IFCGEOMETRICREPRESENTATIONCONTEXT('Drawing','Plan',2,$,#30,$);\n\
    #19=IFCGEOMETRICREPRESENTATIONSUBCONTEXT('FootPrint','Plan',*,*,*,*,#18,$,.PLAN_VIEW.,$);\n\
    #20=IFCAXIS2PLACEMENT3D(#21,#24,#25);\n#21=IFCCARTESIANPOINT((100.,200.,300.));\n\
    #24=IFCDIRECTION((0.,0.,2.));\n#25=IFCDIRECTION((3.,1.,0.));\n#26=IFCDIRECTION((1.,2.));\n\
    #30=IFCAXIS2PLACEMENT2D(#31,#32);\n#31=IFCCARTESIANPOINT((10.,20.));\n#32=IFCDIRECTION((2.,1.));\n\
    ENDSEC;\nEND-ISO-10303-21;\n"
    )
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
fn get(file: &IfcFile, id: u64) -> ContextGeometry<'_> {
    let (r, d, b) = with(Limits::default(), |cx| file.representation_context(id, cx));
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(b.used(Kind::Depth), 0);
    r.unwrap()
}

#[test]
fn retains_parent_coordinates_and_child_scale_without_normalizing_or_defaulting() {
    for version in VERSIONS {
        let file = read(&source(version));
        let base = get(&file, 8);
        let child = get(&file, 10);
        assert_eq!(base.definition.geometric, child.definition.geometric);
        assert!(base.definition.view.is_none());
        assert_eq!(child.source.context.id(), Some(10));
        assert_eq!(child.source.parent.unwrap().id(), Some(8));
        assert_eq!(
            child.length_unit.as_ref().unwrap().prefix.as_deref(),
            Some("MILLI")
        );
        let g = &child.definition.geometric;
        assert_eq!(g.identifier.as_deref(), Some("Parent"));
        assert_eq!(g.dimension, 3);
        assert_eq!(g.precision, Some(0.001));
        assert_eq!(g.true_north, Some([1., 2.]));
        let ContextPlacement::Space(p) = &g.world_coordinate_system else {
            panic!()
        };
        assert_eq!(p.location, [100., 200., 300.]);
        assert_eq!(p.axis, Some([0., 0., 2.]));
        assert_eq!(p.reference_direction, Some([3., 1., 0.]));
        let v = child.definition.view.unwrap();
        assert_eq!(v.identifier.as_deref(), Some("Body"));
        assert_eq!(v.target_scale, Some(0.01));
        assert_eq!(v.target_view, TargetView::Model);
        let plane = get(&file, 19);
        assert_eq!(plane.source.parent.unwrap().id(), Some(18));
        assert!(plane.definition.geometric.precision.is_none());
        assert!(plane.definition.geometric.true_north.is_none());
        assert!(
            plane
                .definition
                .view
                .as_ref()
                .unwrap()
                .target_scale
                .is_none()
        );
        let ContextPlacement::Plane(p) = plane.definition.geometric.world_coordinate_system else {
            panic!()
        };
        assert_eq!(p.location, [10., 20.]);
        assert_eq!(p.reference_direction, Some([2., 1.]));
    }
}

#[test]
fn accepts_all_projection_names_and_retains_optional_axes_and_zero_precision() {
    for version in VERSIONS {
        for (name, expected) in [
            ("GRAPH_VIEW", TargetView::Graph),
            ("SKETCH_VIEW", TargetView::Sketch),
            ("MODEL_VIEW", TargetView::Model),
            ("PLAN_VIEW", TargetView::Plan),
            ("REFLECTED_PLAN_VIEW", TargetView::ReflectedPlan),
            ("SECTION_VIEW", TargetView::Section),
            ("ELEVATION_VIEW", TargetView::Elevation),
            ("USERDEFINED", TargetView::UserDefined),
            ("NOTDEFINED", TargetView::NotDefined),
        ] {
            let src = source(version).replace(".MODEL_VIEW.,$", &format!(".{name}.,'Custom'"));
            assert_eq!(
                get(&read(&src), 10).definition.view.unwrap().target_view,
                expected
            );
        }
        let src = source(version)
            .replace("3,0.001", "3,0.")
            .replace(
                "#20=IFCAXIS2PLACEMENT3D(#21,#24,#25)",
                "#20=IFCAXIS2PLACEMENT3D(#21,$,$)",
            )
            .replace(
                "#30=IFCAXIS2PLACEMENT2D(#31,#32)",
                "#30=IFCAXIS2PLACEMENT2D(#31,$)",
            );
        let file = read(&src);
        let base = get(&file, 8);
        assert_eq!(base.definition.geometric.precision, Some(0.));
        assert!(
            matches!(base.definition.geometric.world_coordinate_system, ContextPlacement::Space(p) if p.axis.is_none() && p.reference_direction.is_none())
        );
        assert!(
            matches!(get(&file, 18).definition.geometric.world_coordinate_system, ContextPlacement::Plane(p) if p.reference_direction.is_none())
        );
    }
}

fn invalid_cases(src: &str) -> Vec<String> {
    [
        ("*,*,*,*,#8", "*,*,*,*,#10"),
        ("*,*,*,*,#8", "*,*,*,*,#19"),
        ("*,*,*,*,#8", "*,*,*,*,#999"),
        ("*,*,*,*,#8", "*,*,*,*,#21"),
        ("*,*,*,*,#8", "$,$,$,$,#8"),
        ("3,0.001", "4,0.001"),
        ("3,0.001", "3.0,0.001"),
        ("3,0.001", "3,1.E999"),
        ("#8,0.01", "#8,0."),
        ("#8,0.01", "#8,-1."),
        (".MODEL_VIEW.", ".UNKNOWN."),
        (".MODEL_VIEW.", ".USERDEFINED."),
        ("#26=IFCDIRECTION((1.,2.))", "#26=IFCDIRECTION((1.,2.,0.))"),
        (
            "#20=IFCAXIS2PLACEMENT3D(#21,#24,#25)",
            "#20=IFCAXIS2PLACEMENT3D(#21,#24,$)",
        ),
        (
            "#25=IFCDIRECTION((3.,1.,0.))",
            "#25=IFCDIRECTION((0.,0.,1.))",
        ),
        (
            "#21=IFCCARTESIANPOINT((100.,200.,300.))",
            "#21=IFCCARTESIANPOINT((100.,200.,300.),$)",
        ),
        ("3,0.001,#20,#26)", "3,0.001,#20,#26,$)"),
    ]
    .into_iter()
    .map(|(a, b)| {
        assert!(src.contains(a));
        src.replace(a, b)
    })
    .collect()
}

#[test]
fn rejects_invalid_parents_derived_attributes_dimensions_and_values_with_a_place() {
    for version in VERSIONS {
        for src in invalid_cases(&source(version)) {
            let file = read(&src);
            let (r, d, b) = with(Limits::default(), |cx| file.representation_context(10, cx));
            assert!(matches!(r, Err(Error::Invalid)), "{r:?}");
            assert!(d.iter().any(|d| d.code() == codes::IFC_SHAPE_INVALID));
            assert_eq!(b.used(Kind::Depth), 0);
        }
        let file = read(
            &source(version).replace("#26=IFCDIRECTION((1.,2.))", "#26=IFCDIRECTION((0.,0.))"),
        );
        let result = with(Limits::default(), |cx| file.representation_context(10, cx)).0;
        assert_eq!(result.is_ok(), version == "IFC2X3");
    }
}

#[test]
fn reports_missing_units_and_unsupported_context_instead_of_inventing_values() {
    let file = read(&source("IFC4").replace("(#8,#18),#2", "(#8,#18),$"));
    let (r, d, _) = with(Limits::default(), |cx| file.representation_context(10, cx));
    assert!(r.unwrap().length_unit.is_none());
    assert!(d.iter().any(|d| d.code() == codes::IFC_UNIT_MISSING));
    let file = read(&source("IFC4").replace(
        "#10=IFCGEOMETRICREPRESENTATIONSUBCONTEXT('Body','Model',*,*,*,*,#8,0.01,.MODEL_VIEW.,$)",
        "#10=IFCREPRESENTATIONCONTEXT('Body','Model')",
    ));
    assert!(matches!(
        with(Limits::default(), |cx| file.representation_context(10, cx)).0,
        Err(Error::Unsupported)
    ));
    let file = read(&source("IFC4X2"));
    assert!(matches!(
        with(Limits::default(), |cx| file.representation_context(10, cx)).0,
        Err(Error::Unsupported)
    ));
}

#[test]
fn keeps_values_after_step_renumbering_and_independent_lookup_order() {
    let src = source("IFC4");
    let file = read(&src);
    let expected = get(&file, 10);
    get(&file, 19);
    assert_eq!(format!("{expected:?}"), format!("{:?}", get(&file, 10)));
    let shifted = read(
        &src.replace("#8", "#800")
            .replace("#20", "#2000")
            .replace("#21", "#2100"),
    );
    let actual = get(&shifted, 10);
    assert_eq!(expected.definition, actual.definition);
    assert_eq!(expected.length_unit, actual.length_unit);
    assert_ne!(
        expected.source.parent.unwrap().id(),
        actual.source.parent.unwrap().id()
    );
    let changed = read(&src.replace("100.,200.,300.", "101.,200.,300."));
    assert_ne!(expected.definition, get(&changed, 10).definition);
}

#[test]
fn charges_boundaries_before_allocating_and_restores_shared_depth() {
    for version in VERSIONS {
        let file = read(&source(version));
        let call = |cx: &mut Cx| file.representation_context(10, cx);
        let (_, _, b) = with(Limits::default(), call);
        for kind in Kind::ALL {
            let used = if kind == Kind::Depth { 2 } else { b.used(kind) };
            assert!(
                with(Limits::default().with(kind, Limit::Finite(used)), call)
                    .0
                    .is_ok()
            );
            if used > 0 {
                let (r, _, b) = with(Limits::default().with(kind, Limit::Finite(used - 1)), call);
                assert!(
                    matches!(r, Err(Error::Stopped(zumen_base::Stop::Exceeded(e))) if e.kind == kind)
                );
                assert_eq!(b.used(Kind::Depth), 0);
            }
        }
        for expired in [false, true] {
            let (r, _, b) = with(Limits::default(), |cx| {
                if expired {
                    cx.cancel.expire()
                } else {
                    cx.cancel.cancel()
                };
                call(cx)
            });
            assert!(matches!(r, Err(Error::Stopped(_))));
            assert_eq!(b.used(Kind::DecodedBytes), 0);
        }
        let (_, _, b) = with(Limits::default(), |cx| {
            cx.enter().unwrap();
            call(cx).unwrap()
        });
        assert_eq!(b.used(Kind::Depth), 1);
    }
}

#[test]
fn altered_inputs_terminate_repeatably() {
    let src = source("IFC4");
    for at in (0..src.len()).step_by(41) {
        let mut bytes = src.as_bytes().to_vec();
        bytes[at] = b'$';
        zumen_sdk::testkit::survives_with(&bytes, |bytes, cx| {
            Ifc::read(bytes, cx)?
                .representation_context(10, cx)
                .map(drop)
        })
        .unwrap();
    }
}

fn text(out: &mut Writer, value: Option<&str>) {
    if let Some(v) = value {
        out.string(v)
    } else {
        out.null()
    }
}
fn number(out: &mut Writer, value: Option<f64>) {
    if let Some(v) = value {
        out.float(v)
    } else {
        out.null()
    }
}
fn vector(out: &mut Writer, value: Option<&[f64]>) {
    if let Some(v) = value {
        out.begin_array();
        for n in v {
            out.float(*n);
        }
        out.end_array();
    } else {
        out.null();
    }
}
fn json(out: &mut Writer, value: &ContextGeometry) {
    let g = &value.definition.geometric;
    out.begin_object();
    out.key("id");
    out.uint(value.source.context.id().unwrap());
    out.key("parent");
    if let Some(p) = value.source.parent {
        out.uint(p.id().unwrap())
    } else {
        out.null()
    };
    out.key("identifier");
    text(out, g.identifier.as_deref());
    out.key("kind");
    text(out, g.kind.as_deref());
    out.key("dimension");
    out.uint(g.dimension.into());
    out.key("precision");
    number(out, g.precision);
    out.key("north");
    vector(out, g.true_north.as_ref().map(|v| v.as_slice()));
    out.key("world");
    out.begin_object();
    let (location, axis, direction): (&[f64], Option<&[f64]>, Option<&[f64]>) =
        match &g.world_coordinate_system {
            ContextPlacement::Plane(v) => (
                &v.location,
                None,
                v.reference_direction.as_ref().map(|v| v.as_slice()),
            ),
            ContextPlacement::Space(v) => (
                &v.location,
                v.axis.as_ref().map(|v| v.as_slice()),
                v.reference_direction.as_ref().map(|v| v.as_slice()),
            ),
        };
    out.key("location");
    vector(out, Some(location));
    out.key("axis");
    vector(out, axis);
    out.key("direction");
    vector(out, direction);
    out.end_object();
    out.key("view");
    if let Some(v) = &value.definition.view {
        out.begin_object();
        out.key("identifier");
        text(out, v.identifier.as_deref());
        out.key("kind");
        text(out, v.kind.as_deref());
        out.key("scale");
        number(out, v.target_scale);
        out.key("target");
        out.string(match v.target_view {
            TargetView::Graph => "GRAPH_VIEW",
            TargetView::Sketch => "SKETCH_VIEW",
            TargetView::Model => "MODEL_VIEW",
            TargetView::Plan => "PLAN_VIEW",
            TargetView::ReflectedPlan => "REFLECTED_PLAN_VIEW",
            TargetView::Section => "SECTION_VIEW",
            TargetView::Elevation => "ELEVATION_VIEW",
            TargetView::UserDefined => "USERDEFINED",
            TargetView::NotDefined => "NOTDEFINED",
        });
        out.key("name");
        text(out, v.user_defined_target_view.as_deref());
        out.end_object();
    } else {
        out.null();
    }
    out.key("unit");
    if let Some(u) = &value.length_unit {
        out.begin_array();
        out.string(&u.kind);
        text(out, u.prefix.as_deref());
        out.string(&u.name);
        number(out, u.scale);
        out.end_array();
    } else {
        out.null();
    }
    out.end_object();
}

#[test]
#[allow(clippy::disallowed_methods)] // 自作入力と公開例の元値・診断・予算を指定先へ出す。
fn exports_values_diagnostics_and_budget_boundaries() {
    let Ok(folder) = std::env::var("ZUMEN_IFC_CONTEXT_OUT") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    let mut contract = String::new();
    let mut export = |name: &str, path: &str, file: &IfcFile, ids: &[u64]| {
        let mut out = Writer::new();
        out.begin_object();
        out.key("file");
        out.string(path);
        out.key("contexts");
        out.begin_array();
        for &id in ids {
            let call = |cx: &mut Cx| file.representation_context(id, cx);
            let (v, d, b) = with(Limits::default(), call);
            let v = v.unwrap();
            assert!(d.is_empty(), "{d:?}");
            json(&mut out, &v);
            contract += &format!(
                "{name}:{id}:ok:{v:?}:{d:?}:{:?}\n",
                Kind::ALL.map(|k| b.used(k))
            );
            for kind in Kind::ALL {
                let used = if kind == Kind::Depth {
                    if v.source.parent.is_some() { 2 } else { 1 }
                } else {
                    b.used(kind)
                };
                for cap in [0, used.saturating_sub(1), used] {
                    let (r, d, b) = with(Limits::default().with(kind, Limit::Finite(cap)), call);
                    contract += &format!(
                        "{name}:{id}:{kind:?}:{cap}:{:?}:{d:?}:{:?}\n",
                        r.err(),
                        Kind::ALL.map(|k| b.used(k))
                    );
                }
            }
        }
        out.end_array();
        out.end_object();
        std::fs::write(folder.join(format!("{name}.json")), out.into_bytes()).unwrap();
    };
    for version in VERSIONS {
        let src = source(version);
        export(
            version,
            &format!("{version}.ifc"),
            &read(&src),
            &[8, 10, 18, 19],
        );
        std::fs::write(folder.join(format!("{version}.ifc")), src).unwrap();
    }
    export(
        "public_column",
        "../../samples/ifc/gni/model_9.ifc",
        &read(include_str!("../../../samples/ifc/gni/model_9.ifc")),
        &[24],
    );
    export(
        "public_wall",
        "../../samples/ifc/bim-whale/TallBuilding.ifc",
        &read(include_str!(
            "../../../samples/ifc/bim-whale/TallBuilding.ifc"
        )),
        &[118],
    );
    for version in VERSIONS {
        for (i, src) in invalid_cases(&source(version)).iter().enumerate() {
            let file = read(src);
            let (r, d, b) = with(Limits::default(), |cx| file.representation_context(10, cx));
            contract += &format!(
                "{version}:invalid:{i}:{:?}:{d:?}:{:?}\n",
                r.err(),
                Kind::ALL.map(|k| b.used(k))
            );
        }
    }
    std::fs::write(folder.join("contract.txt"), contract).unwrap();
}
