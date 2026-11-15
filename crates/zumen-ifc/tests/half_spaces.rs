//! 半空間の元値・版差と、ブーリアンの順序・共有・停止を検査する。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{
    AxisPlacement3, BooleanOperation, Building, CurveKind, EnclosureCoordinates, HalfSpaceLimit,
};
use zumen_ifc::{HalfSpaceGeometry, Ifc, IfcFile, ProductShape, ShapeLinks, ShapeNode};
use zumen_json::Writer;
use zumen_sdk::{Codec, Error, Lift};

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];
fn source(version: &str) -> String {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('half spaces'),'2;1');\nFILE_NAME('halves','',(),(),'','','');\nFILE_SCHEMA(('{version}'));\nENDSEC;\nDATA;\n\
#1=IFCPROJECT('0000000000000000000001',$,$,$,$,$,$,$,#2);\n\
#2=IFCUNITASSIGNMENT((#3,#4));\n#3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n#4=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);\n\
#8=IFCGEOMETRICREPRESENTATIONCONTEXT('Model','Model',3,0.00001,#9,$);\n\
#9=IFCAXIS2PLACEMENT3D(#10,#11,#12);\n#10=IFCCARTESIANPOINT((10.,-0.,30.));\n\
#11=IFCDIRECTION((0.,0.,3.));\n#12=IFCDIRECTION((2.,0.,1.));\n#13=IFCPLANE(#9);\n\
#14=IFCCARTESIANPOINT((-1.,-2.,-3.));\n#15=IFCBOUNDINGBOX(#14,4.,5.,6.);\n\
#16=IFCAXIS2PLACEMENT3D(#14,$,$);\n\
#20=IFCHALFSPACESOLID(#13,.F.);\n#21=IFCHALFSPACESOLID(#13,.T.);\n#22=IFCBOXEDHALFSPACE(#13,.T.,#15);\n\
#23=IFCPOLYGONALBOUNDEDHALFSPACE(#13,.F.,#16,#33);\n#24=IFCPOLYGONALBOUNDEDHALFSPACE(#13,.T.,#9,#45);\n\
#30=IFCCARTESIANPOINT((0.,0.));\n#31=IFCCARTESIANPOINT((10.,0.));\n#32=IFCCARTESIANPOINT((0.,10.));\n\
#33=IFCPOLYLINE((#30,#31,#32,#30));\n#34=IFCAXIS2PLACEMENT2D(#30,$);\n#35=IFCCIRCLE(#34,5.);\n\
#36=IFCCARTESIANPOINT((5.,0.));\n#37=IFCCARTESIANPOINT((-5.,0.));\n\
#40=IFCTRIMMEDCURVE(#35,(#36,IFCPARAMETERVALUE(0.)),(#37,IFCPARAMETERVALUE(3.141592653589793)),.T.,.PARAMETER.);\n\
#41=IFCPOLYLINE((#37,#36));\n#43=IFCCOMPOSITECURVESEGMENT(.CONTINUOUS.,.T.,#40);\n\
#44=IFCCOMPOSITECURVESEGMENT(.CONTINUOUS.,.T.,#41);\n#45=IFCCOMPOSITECURVE((#43,#44),.F.);\n\
#50=IFCCARTESIANPOINTLIST2D(((0.,0.),(10.,0.),(0.,10.)));\n\
#51=IFCINDEXEDPOLYCURVE(#50,(IFCLINEINDEX((1,2,3,1))),.F.);\n#25=IFCPOLYGONALBOUNDEDHALFSPACE(#13,.T.,#9,#51);\n\
#60=IFCBOOLEANRESULT(.UNION.,#20,#22);\n#61=IFCBOOLEANRESULT(.INTERSECTION.,#60,#23);\n\
#62=IFCBOOLEANRESULT(.DIFFERENCE.,#61,#60);\n\
#100=IFCBEAM('0000000000000000000002',$,$,$,$,$,#101,$,.NOTDEFINED.);\n\
#101=IFCPRODUCTDEFINITIONSHAPE($,$,(#102));\n#102=IFCSHAPEREPRESENTATION(#8,'Body','CSG',(#62,#21,#24{indexed}));\n\
ENDSEC;\nEND-ISO-10303-21;\n",
        indexed = if version == "IFC4X3_ADD2" { ",#25" } else { "" }
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
fn value(file: &IfcFile, product_id: u64, id: u64) -> HalfSpaceGeometry<'_> {
    let shape = product(file, product_id);
    let (result, diag, _) = with(Limits::default(), |cx| {
        file.half_space(leaf(&shape, id), cx)
    });
    assert!(diag.is_empty(), "{diag:?}");
    result.unwrap()
}

#[test]
fn keeps_raw_planes_agreement_enclosures_and_version_specific_coordinates() {
    for version in VERSIONS {
        let file = read(&source(version));
        let a = value(&file, 100, 20);
        assert!(!a.definition.agreement);
        assert!(matches!(a.definition.limit, HalfSpaceLimit::Unbounded));
        assert_eq!(
            a.definition.plane.location[1].to_bits(),
            (-0.0f64).to_bits()
        );
        assert_eq!(a.definition.plane.axis, Some([0., 0., 3.]));
        assert_eq!(a.definition.plane.reference_direction, Some([2., 0., 1.]));
        assert!(a.boundary.is_none());
        assert_eq!(a.length_unit.unwrap().prefix.as_deref(), Some("MILLI"));
        assert!(value(&file, 100, 21).definition.agreement);
        let b = value(&file, 100, 22);
        assert_eq!(a.definition.plane, b.definition.plane);
        assert_eq!(
            b.definition.limit,
            HalfSpaceLimit::Box {
                corner: [-1., -2., -3.],
                dimensions: [4., 5., 6.],
                coordinates: if version == "IFC2X3" {
                    EnclosureCoordinates::BaseSurface
                } else {
                    EnclosureCoordinates::Object
                },
            }
        );
        assert_eq!(b.source.surface.id(), Some(13));
        for r in &b.source.records {
            assert!(std::ptr::eq(*r, file.instance(r.id().unwrap()).unwrap()));
        }
        let p = value(&file, 100, 23);
        let HalfSpaceLimit::Polygonal { position, boundary } = p.definition.limit else {
            panic!()
        };
        assert_eq!(position.location, [-1., -2., -3.]);
        assert!(position.axis.is_none());
        assert_eq!(boundary, p.boundary.as_ref().unwrap().root);
        let c = value(&file, 100, 24);
        assert_eq!(c.boundary.as_ref().unwrap().items.len(), 4);
        assert!(matches!(
            c.boundary.as_ref().unwrap().items[0].kind,
            CurveKind::Composite { .. }
        ));
        if version == "IFC4X3_ADD2" {
            assert!(matches!(
                value(&file, 100, 25).boundary.unwrap().items[0].kind,
                CurveKind::Indexed { .. }
            ));
        }
    }
}

#[test]
fn boolean_view_preserves_operand_order_and_sharing_and_rejects_cycles() {
    for version in VERSIONS {
        let src = source(version);
        let file = read(&src);
        let shape = product(&file, 100);
        let ShapeLinks::Boolean {
            operation,
            first,
            second,
        } = leaf(&shape, 62).links
        else {
            panic!()
        };
        assert_eq!(operation, BooleanOperation::Difference);
        assert_eq!(shape.node(first).unwrap().source.id(), Some(61));
        assert_eq!(shape.node(second).unwrap().source.id(), Some(60));
        let ShapeLinks::Boolean {
            first, operation, ..
        } = leaf(&shape, 61).links
        else {
            panic!()
        };
        assert_eq!(operation, BooleanOperation::Intersection);
        assert_eq!(first, second);
        assert_eq!(
            shape
                .nodes
                .iter()
                .filter(|n| n.source.id() == Some(60))
                .count(),
            1
        );
        for (from, to) in [
            (".UNION.", ".XOR."),
            (".UNION.", "$"),
            (".UNION.,#20,#22", ".UNION.,#60,#22"),
            (".UNION.,#20,#22", ".UNION.,#62,#22"),
            (".UNION.,#20,#22", ".UNION.,#999,#22"),
            (".UNION.,#20,#22", ".UNION.,#20,#22,$"),
            (
                "IFCBOOLEANRESULT(.UNION.",
                "IFCBOOLEANCLIPPINGRESULT(.UNION.",
            ),
        ] {
            let file = read(&src.replace(from, to));
            let (result, diag, budget) = with(Limits::default(), |cx| file.product_shape(100, cx));
            assert!(matches!(result, Err(Error::Invalid)), "{to}: {result:?}");
            assert!(diag.iter().any(|d| d.code() == codes::IFC_SHAPE_INVALID));
            assert_eq!(budget.used(Kind::Depth), 0);
        }
        // L1 ビューは対象の型の組み合わせを検証しない。未知の葉を出所と診断で残す。
        let file = read(&src.replace("IFCHALFSPACESOLID(#13,.F.)", "IFCFUTURESOLID(#13)"));
        let (shape, diag, _) = with(Limits::default(), |cx| file.product_shape(100, cx));
        assert!(matches!(
            leaf(&shape.unwrap(), 20).links,
            ShapeLinks::Unsupported
        ));
        assert!(
            diag.iter()
                .any(|d| d.code() == codes::IFC_SHAPE_UNSUPPORTED)
        );
    }
}

fn invalid_inputs() -> Vec<(u64, &'static str, &'static str)> {
    vec![
        (
            20,
            "IFCHALFSPACESOLID(#13,.F.)",
            "IFCHALFSPACESOLID(#13,.U.)",
        ),
        (
            20,
            "IFCHALFSPACESOLID(#13,.F.)",
            "IFCHALFSPACESOLID(#13,.F.,$)",
        ),
        (
            20,
            "IFCHALFSPACESOLID(#13,.F.)",
            "IFCHALFSPACESOLID(#999,.F.)",
        ),
        (20, "IFCPLANE(#9)", "IFCPLANE(#9,$)"),
        (20, "IFCPLANE(#9)", "IFCPLANE(#20)"),
        (20, "(10.,-0.,30.)", "(10.,-0.)"),
        (20, "(0.,0.,3.)", "(0.,0.,0.)"),
        (20, "(2.,0.,1.)", "(0.,0.,2.)"),
        (20, "#10,#11,#12", "#10,#11,$"),
        (20, "#10,#11,#12", "#10,#11,#12,$"),
        (22, "#14,4.,5.,6.", "#14,0.,5.,6."),
        (22, "#14,4.,5.,6.", "#14,4.,-5.,6."),
        (22, "#14,4.,5.,6.", "#14,4.,5.,1.E400"),
        (22, "#14,4.,5.,6.", "#14,4.,5.,6.,$"),
        (22, "(-1.,-2.,-3.)", "(-1.,-2.)"),
        (23, "(#30,#31,#32,#30)", "(#30,#31,#32)"),
        (23, "(#30,#31,#32,#30)", "(#30,#31,#30)"),
        (
            23,
            "IFCCARTESIANPOINT((0.,10.))",
            "IFCCARTESIANPOINT((0.,10.,0.))",
        ),
        (20, ".MILLI.,.METRE.", ".MILLI.,.SECOND."),
    ]
}
#[test]
fn rejects_invalid_values_without_partial_results() {
    for version in VERSIONS {
        for (id, from, to) in invalid_inputs() {
            let file = read(&source(version).replace(from, to));
            let shape = product(&file, 100);
            let (result, diag, budget) = with(Limits::default(), |cx| {
                file.half_space(leaf(&shape, id), cx)
            });
            assert!(
                matches!(result, Err(Error::Invalid)),
                "{version} {to}: {result:?}"
            );
            assert!(!diag.is_empty());
            assert_eq!(budget.used(Kind::Depth), 0);
        }
    }
}
#[test]
fn rejects_unsupported_surfaces_boundaries_versions_complex_and_foreign_sources() {
    for (from, to, id) in [
        ("IFCPLANE(#9)", "IFCCYLINDRICALSURFACE(#9,2.)", 20),
        ("#16,#33)", "#16,#35)", 23),
        ("#16,#33)", "#16,#51)", 23),
        ("IFCPLANE(#9)", "(IFCPLANE(#9))", 20),
        (
            "IFCAXIS2PLACEMENT3D(#10,#11,#12)",
            "(IFCAXIS2PLACEMENT3D(#10,#11,#12))",
            20,
        ),
        (
            "IFCCOMPOSITECURVESEGMENT(.CONTINUOUS.,.T.,#41)",
            "IFCCOMPOSITECURVESEGMENT(.CONTINUOUS.,.T.,#35)",
            24,
        ),
    ] {
        let file = read(&source("IFC4").replace(from, to));
        let shape = product(&file, 100);
        let (result, diag, budget) = with(Limits::default(), |cx| {
            file.half_space(leaf(&shape, id), cx)
        });
        assert!(
            matches!(result, Err(Error::Unsupported)),
            "{to}: {result:?}"
        );
        assert!(
            diag.iter()
                .any(|d| d.code() == codes::IFC_SHAPE_UNSUPPORTED)
        );
        assert_eq!(budget.used(Kind::Depth), 0);
    }
    let file = read(&source("IFC4"));
    let other = read(&source("IFC4"));
    let shape = product(&other, 100);
    assert!(matches!(
        with(Limits::default(), |cx| file
            .half_space(leaf(&shape, 20), cx))
        .0,
        Err(Error::Invalid)
    ));
    let file = read(&source("IFC4X2"));
    let node = ShapeNode {
        source: file.instance(20).unwrap(),
        links: ShapeLinks::Unsupported,
    };
    assert!(matches!(
        with(Limits::default(), |cx| file.half_space(&node, cx)).0,
        Err(Error::Unsupported)
    ));
}

#[test]
fn missing_units_and_retrieval_order_do_not_change_existing_building() {
    for version in VERSIONS {
        let src = source(version);
        let file = read(&src);
        let before = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
            .0
            .unwrap();
        for id in [24, 22, 23, 20, 21] {
            value(&file, 100, id);
        }
        let after = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
            .0
            .unwrap();
        assert_eq!(before, after);
        let renamed = read(&src.replace("#13", "#913").replace("#15", "#915"));
        assert_eq!(
            value(&file, 100, 22).definition,
            value(&renamed, 100, 22).definition
        );
        let file = read(&src.replace("$,#2)", "$,$)"));
        let shape = product(&file, 100);
        let (result, diag, _) = with(Limits::default(), |cx| {
            file.half_space(leaf(&shape, 20), cx)
        });
        assert!(result.unwrap().length_unit.is_none());
        assert!(diag.iter().any(|d| d.code() == codes::IFC_UNIT_MISSING));
    }
}

fn depth(id: u64) -> u64 {
    if id == 24 {
        5
    } else if id == 23 || id == 25 {
        3
    } else {
        2
    }
}
#[test]
fn charges_all_budgets_and_restores_depth_after_limits_and_cancellation() {
    for version in VERSIONS {
        let file = read(&source(version));
        let shape = product(&file, 100);
        let mut ids = vec![20, 21, 22, 23, 24];
        if version == "IFC4X3_ADD2" {
            ids.push(25);
        }
        for id in ids {
            let node = leaf(&shape, id);
            let (_, _, full) = with(Limits::default(), |cx| file.half_space(node, cx));
            for kind in Kind::ALL {
                let cap = if kind == Kind::Depth {
                    depth(id)
                } else {
                    full.used(kind)
                };
                let (result, _, budget) =
                    with(Limits::default().with(kind, Limit::Finite(cap)), |cx| {
                        file.half_space(node, cx)
                    });
                assert!(result.is_ok(), "{version} {id} {kind:?}: {result:?}");
                assert_eq!(budget.used(Kind::Depth), 0);
                if cap > 0 {
                    let (result, diag, budget) =
                        with(Limits::default().with(kind, Limit::Finite(cap - 1)), |cx| {
                            file.half_space(node, cx)
                        });
                    assert!(
                        matches!(result, Err(Error::Stopped(_))),
                        "{id} {kind:?}: {result:?}"
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
                file.half_space(node, &mut cx),
                Err(Error::Stopped(_))
            ));
            cx.leave();
            assert_eq!(budget.used(Kind::Depth), 0);
        }
        let (_, _, full) = with(Limits::default(), |cx| file.product_shape(100, cx));
        for kind in [Kind::Depth, Kind::Work, Kind::DecodedBytes] {
            let cap = if kind == Kind::Depth {
                6
            } else {
                full.used(kind)
            };
            assert!(
                with(Limits::default().with(kind, Limit::Finite(cap)), |cx| file
                    .product_shape(100, cx))
                .0
                .is_ok()
            );
            assert!(matches!(
                with(Limits::default().with(kind, Limit::Finite(cap - 1)), |cx| {
                    file.product_shape(100, cx)
                })
                .0,
                Err(Error::Stopped(_))
            ));
        }
    }
}

const PUBLIC: &str = include_str!("../../../samples/ifc/bim-whale/TallBuilding.ifc");
#[test]
fn public_wall_reaches_both_half_spaces_through_clipping_results() {
    let file = read(PUBLIC);
    let shape = product(&file, 615);
    assert!(
        shape
            .nodes
            .iter()
            .filter(|n| matches!(n.links, ShapeLinks::Boolean { .. }))
            .count()
            >= 2
    );
    assert!(matches!(
        value(&file, 615, 600).definition.limit,
        HalfSpaceLimit::Unbounded
    ));
    assert!(matches!(
        value(&file, 615, 596).definition.limit,
        HalfSpaceLimit::Polygonal { .. }
    ));
    assert!(value(&file, 615, 596).definition.agreement);
}

#[test]
fn survives_mutated_and_truncated_inputs_with_one_budget() {
    let run = |bytes: &[u8], cx: &mut Cx| {
        let file = Ifc::read(bytes, cx)?;
        let shape = file.product_shape(100, cx)?;
        for node in &shape.nodes {
            cx.check(Kind::Work, 1)?;
            if file
                .kind(node.source)
                .eq_ignore_ascii_case(b"IFCHALFSPACESOLID")
                || file.kind(node.source).ends_with(b"HALFSPACE")
            {
                file.half_space(node, cx)?;
            }
        }
        Ok(())
    };
    for version in VERSIONS {
        let bytes = source(version).into_bytes();
        for end in (0..bytes.len()).step_by(59) {
            zumen_sdk::testkit::survives_with(&bytes[..end], run).unwrap();
        }
        for at in (0..bytes.len()).step_by(53) {
            for byte in [0, b'$', b'#', 255] {
                let mut b = bytes.clone();
                b[at] = byte;
                zumen_sdk::testkit::survives_with(&b, run).unwrap();
            }
        }
    }
}

fn numbers(out: &mut Writer, values: &[f64]) {
    out.begin_array();
    for &v in values {
        out.float(v);
    }
    out.end_array();
}
fn placement(out: &mut Writer, p: &AxisPlacement3) {
    out.begin_object();
    out.key("location");
    numbers(out, &p.location);
    out.key("axis");
    if let Some(v) = p.axis {
        numbers(out, &v)
    } else {
        out.null()
    }
    out.key("reference_direction");
    if let Some(v) = p.reference_direction {
        numbers(out, &v)
    } else {
        out.null()
    }
    out.end_object();
}
fn json_item(out: &mut Writer, value: &HalfSpaceGeometry<'_>) {
    let d = &value.definition;
    out.begin_object();
    out.key("instance");
    out.uint(value.source.solid.id().unwrap());
    out.key("plane");
    placement(out, &d.plane);
    out.key("agreement");
    out.bool(d.agreement);
    out.key("limit");
    match &d.limit {
        HalfSpaceLimit::Unbounded => out.null(),
        HalfSpaceLimit::Box {
            corner,
            dimensions,
            coordinates,
        } => {
            out.begin_object();
            out.key("corner");
            numbers(out, corner);
            out.key("dimensions");
            numbers(out, dimensions);
            out.key("coordinates");
            out.string(match coordinates {
                EnclosureCoordinates::BaseSurface => "base-surface",
                EnclosureCoordinates::Object => "object",
            });
            out.end_object();
        }
        HalfSpaceLimit::Polygonal { position, boundary } => {
            out.begin_object();
            out.key("position");
            placement(out, position);
            out.key("boundary");
            out.uint(
                value
                    .boundary
                    .as_ref()
                    .unwrap()
                    .item(*boundary)
                    .unwrap()
                    .source
                    .instance
                    .unwrap(),
            );
            out.end_object();
        }
    }
    out.key("curves");
    out.begin_array();
    if let Some(graph) = &value.boundary {
        for item in &graph.items {
            out.begin_object();
            out.key("instance");
            out.uint(item.source.instance.unwrap());
            match &item.kind {
                CurveKind::Polyline(points) | CurveKind::Indexed { points, .. } => {
                    out.key("points");
                    out.begin_array();
                    for p in points {
                        match p {
                            zumen_building::Coordinates::Two(v) => numbers(out, v),
                            zumen_building::Coordinates::Three(v) => numbers(out, v),
                        }
                    }
                    out.end_array();
                }
                CurveKind::Circle { radius, position } => {
                    out.key("radius");
                    out.float(*radius);
                    out.key("center");
                    match position.location {
                        zumen_building::Coordinates::Two(v) => numbers(out, &v),
                        zumen_building::Coordinates::Three(v) => numbers(out, &v),
                    }
                }
                CurveKind::Composite { segments, .. } => {
                    out.key("segments");
                    out.begin_array();
                    for s in segments {
                        out.begin_array();
                        out.uint(graph.item(s.curve).unwrap().source.instance.unwrap());
                        out.bool(s.same_sense);
                        out.end_array();
                    }
                    out.end_array();
                }
                CurveKind::Trimmed {
                    basis,
                    sense_agreement,
                    ..
                } => {
                    out.key("basis");
                    out.uint(graph.item(*basis).unwrap().source.instance.unwrap());
                    out.key("sense");
                    out.bool(*sense_agreement);
                }
                _ => panic!("この照合用入力にない曲線"),
            }
            out.end_object();
        }
    }
    out.end_array();
    out.end_object();
}

#[test]
#[allow(clippy::disallowed_methods)] // 自作入力と公開例の結果だけを指定先へ出す。
fn exports_raw_values_diagnostics_and_budget_contracts() {
    let Ok(folder) = std::env::var("ZUMEN_IFC_HALF_SPACES_OUT") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    let mut contract = String::new();
    let mut export =
        |file: &IfcFile, path: &str, name: &str, product_id: u64, gid: &str, ids: &[u64]| {
            let shape = product(file, product_id);
            let mut out = Writer::new();
            out.begin_object();
            out.key("file");
            out.string(path);
            out.key("product");
            out.uint(product_id);
            out.key("global_id");
            out.string(gid);
            out.key("booleans");
            out.begin_array();
            for node in &shape.nodes {
                if let ShapeLinks::Boolean {
                    operation,
                    first,
                    second,
                } = node.links
                {
                    out.begin_object();
                    out.key("instance");
                    out.uint(node.source.id().unwrap());
                    out.key("operation");
                    out.string(match operation {
                        BooleanOperation::Union => "UNION",
                        BooleanOperation::Intersection => "INTERSECTION",
                        BooleanOperation::Difference => "DIFFERENCE",
                    });
                    out.key("first");
                    out.uint(shape.node(first).unwrap().source.id().unwrap());
                    out.key("second");
                    out.uint(shape.node(second).unwrap().source.id().unwrap());
                    out.end_object();
                }
            }
            out.end_array();
            out.key("items");
            out.begin_array();
            contract += &format!("{name}:shape:{shape:?}\n");
            for &id in ids {
                let node = leaf(&shape, id);
                let (result, diag, budget) =
                    with(Limits::default(), |cx| file.half_space(node, cx));
                let value = result.unwrap();
                json_item(&mut out, &value);
                contract += &format!(
                    "{name}:{id}:ok:{value:?}:{diag:?}:{:?}\n",
                    Kind::ALL.map(|k| budget.used(k))
                );
                for kind in Kind::ALL {
                    let used = if kind == Kind::Depth {
                        if id > 100 { 3 } else { depth(id) }
                    } else {
                        budget.used(kind)
                    };
                    for cap in [0, used.saturating_sub(1), used] {
                        let (r, d, b) =
                            with(Limits::default().with(kind, Limit::Finite(cap)), |cx| {
                                file.half_space(node, cx)
                            });
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
        let file = read(&src);
        let mut ids = vec![20, 21, 22, 23, 24];
        if version == "IFC4X3_ADD2" {
            ids.push(25);
        }
        export(
            &file,
            &format!("{version}.ifc"),
            version,
            100,
            "0000000000000000000002",
            &ids,
        );
        std::fs::write(folder.join(format!("{version}.ifc")), src).unwrap();
    }
    export(
        &read(PUBLIC),
        "../../samples/ifc/bim-whale/TallBuilding.ifc",
        "public_wall",
        615,
        "0ZBert7zf3GhThdO12NKAr",
        &[600, 596],
    );
    for version in VERSIONS {
        for (id, from, to) in invalid_inputs() {
            let file = read(&source(version).replace(from, to));
            let shape = product(&file, 100);
            let (r, d, b) = with(Limits::default(), |cx| {
                file.half_space(leaf(&shape, id), cx)
            });
            contract += &format!(
                "{version}:{to}:{:?}:{d:?}:{:?}\n",
                r.err(),
                Kind::ALL.map(|k| b.used(k))
            );
        }
    }
    std::fs::write(folder.join("contract.txt"), contract).unwrap();
}
