//! 外輪・内輪を 1 つの曲線グラフで保持し、押し出しと回転体で共有する。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{
    AreaProfileKind, Building, CurveGraph, CurveKind, CurveRef, ElementRef, Hash,
};
use zumen_ifc::{Ifc, IfcFile, ProductShape, ShapeNode};
use zumen_sdk::{Codec, Error, Lift};

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];
fn source(version: &str) -> String {
    let mut shared = String::new();
    for (start, left, right) in [(70, 8, 12), (90, 28, 32)] {
        shared += &format!(
            "#{start}=IFCCARTESIANPOINT(({left}.,10.));\n#{r}=IFCCARTESIANPOINT(({right}.,10.));\n#{l_line}=IFCLINE(#{start},#57);\n#{r_line}=IFCLINE(#{r},#57);\n",
            r = start + 1,
            l_line = start + 2,
            r_line = start + 3
        );
        for (offset, basis, a, b, sense) in [
            (4, 54, left, right, true),
            (5, start + 3, 0, 4, true),
            (6, 55, left, right, false),
            (7, start + 2, 0, 4, false),
        ] {
            shared += &format!(
                "#{trim}=IFCTRIMMEDCURVE(#{basis},(IFCPARAMETERVALUE({a}.)),(IFCPARAMETERVALUE({b}.)),.T.,.PARAMETER.);\n#{seg}=IFCCOMPOSITECURVESEGMENT(.CONTINUOUS.,.{sense}.,#{trim});\n",
                trim = start + offset,
                seg = start + offset + 4,
                sense = if sense { "T" } else { "F" }
            );
        }
        shared += &format!(
            "#{root}=IFCCOMPOSITECURVE((#{a},#{b},#{c},#{d}),.F.);\n",
            root = start + 12,
            a = start + 8,
            b = start + 9,
            c = start + 10,
            d = start + 11
        );
    }
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('void profiles'),'2;1');\nFILE_NAME('voids','',(),(),'','','');\nFILE_SCHEMA(('{version}'));\nENDSEC;\nDATA;\n\
#1=IFCPROJECT('0000000000000000000001',$,$,$,$,$,$,$,#2);\n#2=IFCUNITASSIGNMENT((#3,#4));\n\
#3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n#4=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);\n\
#8=IFCGEOMETRICREPRESENTATIONCONTEXT('Model','Model',3,0.00001,#9,$);\n#9=IFCAXIS2PLACEMENT3D(#5,$,$);\n\
#5=IFCCARTESIANPOINT((0.,0.,0.));\n#6=IFCDIRECTION((0.,0.,2.));\n\
#10=IFCCARTESIANPOINT((0.,-0.));\n#11=IFCCARTESIANPOINT((40.,0.));\n#12=IFCCARTESIANPOINT((40.,30.));\n#13=IFCCARTESIANPOINT((0.,30.));\n#15=IFCPOLYLINE((#10,#11,#12,#13,#10));\n\
#20=IFCARBITRARYPROFILEDEFWITHVOIDS(.AREA.,'round and rectangular',#15,(#31,#35));\n\
#28=IFCCARTESIANPOINT((10.,12.));\n#29=IFCAXIS2PLACEMENT2D(#28,$);\n#31=IFCCIRCLE(#29,2.);\n\
#32=IFCCARTESIANPOINT((28.,10.));\n#33=IFCCARTESIANPOINT((32.,10.));\n#34=IFCCARTESIANPOINT((32.,14.));\n#36=IFCCARTESIANPOINT((28.,14.));\n#35=IFCPOLYLINE((#32,#33,#34,#36,#32));\n\
#40=IFCARBITRARYPROFILEDEFWITHVOIDS(.AREA.,'shared lines',#15,(#82,#102));\n\
#50=IFCCARTESIANPOINT((0.,10.));\n#51=IFCCARTESIANPOINT((0.,14.));\n#52=IFCDIRECTION((1.,0.));\n#53=IFCVECTOR(#52,1.);\n#54=IFCLINE(#50,#53);\n#55=IFCLINE(#51,#53);\n#56=IFCDIRECTION((0.,1.));\n#57=IFCVECTOR(#56,1.);\n\
{shared}\
#180=IFCCARTESIANPOINT((-5.,0.,0.));\n#181=IFCDIRECTION((0.,2.,0.));\n#182=IFCAXIS1PLACEMENT(#180,#181);\n\
#200=IFCEXTRUDEDAREASOLID(#40,#9,#6,5.);\n#201=IFCREVOLVEDAREASOLID(#40,#9,#182,3.141592653589793);\n\
#210=IFCEXTRUDEDAREASOLID(#20,#9,#6,5.);\n#211=IFCREVOLVEDAREASOLID(#20,#9,#182,3.141592653589793);\n\
#300=IFCBEAM('0000000000000000000002',$,$,$,$,$,#301,$,.NOTDEFINED.);\n#301=IFCPRODUCTDEFINITIONSHAPE($,$,(#302));\n#302=IFCSHAPEREPRESENTATION(#8,'Body','GeometricSet',(#200,#201,#210,#211));\n\
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
fn profile(
    file: &IfcFile,
    node: &ShapeNode<'_>,
    cx: &mut Cx,
) -> Result<(zumen_building::AreaProfile, CurveGraph), Error> {
    if file.kind(node.source) == b"IFCEXTRUDEDAREASOLID" {
        let v = file.extrusion(node, cx)?;
        Ok((v.definition.profile, v.boundary.unwrap()))
    } else {
        let v = file.revolved_area(node, cx)?;
        Ok((v.definition.profile, v.boundary.unwrap()))
    }
}
fn ids(graph: &CurveGraph, refs: &[CurveRef]) -> Vec<u64> {
    refs.iter()
        .map(|r| graph.item(*r).unwrap().source.instance.unwrap())
        .collect()
}

#[test]
fn keeps_outer_inner_and_shared_basis_curves_for_extrusions_and_revolutions() {
    for version in VERSIONS {
        let file = read(&source(version));
        let shape = product(&file, 300);
        for id in [200, 201, 210, 211] {
            let (r, d, _) = with(Limits::default(), |cx| profile(&file, leaf(&shape, id), cx));
            assert!(d.is_empty(), "{d:?}");
            let (p, g) = r.unwrap();
            let AreaProfileKind::ArbitraryWithVoids { boundary, holes } = &p.kind else {
                panic!()
            };
            assert!(p.position.is_none());
            assert_eq!(g.root, *boundary);
            assert_eq!(ids(&g, &[*boundary]), [15]);
            assert_eq!(holes.len(), 2);
            assert_eq!(
                g.length_unit.as_ref().unwrap().prefix.as_deref(),
                Some("MILLI")
            );
            if id < 210 {
                assert_eq!(ids(&g, holes), [82, 102]);
                assert_eq!(g.items.len(), 17);
                for base in [54, 55] {
                    assert_eq!(
                        g.items
                            .iter()
                            .filter(|i| i.source.instance == Some(base))
                            .count(),
                        1
                    );
                }
                let first = g
                    .items
                    .iter()
                    .find(|i| i.source.instance == Some(74))
                    .unwrap();
                let second = g
                    .items
                    .iter()
                    .find(|i| i.source.instance == Some(94))
                    .unwrap();
                let (CurveKind::Trimmed { basis: a, .. }, CurveKind::Trimmed { basis: b, .. }) =
                    (&first.kind, &second.kind)
                else {
                    panic!()
                };
                assert_eq!(a, b);
            } else {
                assert_eq!(ids(&g, holes), [31, 35]);
                assert_eq!(g.items.len(), 3);
            }
        }
        let ext = with(Limits::default(), |cx| {
            file.extrusion(leaf(&shape, 200), cx)
        })
        .0
        .unwrap();
        let rev = with(Limits::default(), |cx| {
            file.revolved_area(leaf(&shape, 201), cx)
        })
        .0
        .unwrap();
        assert_eq!(ext.definition.profile, rev.definition.profile);
        assert_eq!(ext.boundary, rev.boundary);
        assert!(matches!(
            with(Limits::default(), |cx| file
                .extruded_area(leaf(&shape, 200), cx))
            .0,
            Err(Error::Unsupported)
        ));
    }
}
fn invalid_inputs() -> Vec<(u64, &'static str, &'static str)> {
    vec![
        (210, "#15,(#31,#35)", "#15,()"),
        (210, "#15,(#31,#35)", "#15,$"),
        (210, "#15,(#31,#35)", "#15,(#31,#31)"),
        (210, "#15,(#31,#35)", "#15,(#15,#35)"),
        (210, "#15,(#31,#35)", "#15,(#31,#999)"),
        (210, "#15,(#31,#35)", "#15,(#31,#35),$"),
        (210, ".AREA.,'round", ".CURVE.,'round"),
        (210, "#32,#33,#34,#36,#32", "#32,#33,#34,#36,#33"),
        (210, "IFCCIRCLE(#29,2.)", "IFCCIRCLE(#29,0.)"),
        (210, "(28.,14.)", "(28.,14.,0.)"),
        (200, ".CONTINUOUS.", ".DISCONTINUOUS."),
        (200, "IFCLINE(#50,#53)", "IFCCOMPOSITECURVE((#78),.F.)"),
        (200, "IFCLINE(#50,#53)", "IFCLINE(#50,#999)"),
        (200, ".MILLI.,.METRE.", ".MILLI.,.SECOND."),
    ]
}
#[test]
fn rejects_empty_repeated_open_non_2d_and_cyclic_boundaries_without_partial_values() {
    for version in VERSIONS {
        for (id, from, to) in invalid_inputs() {
            let file = read(&source(version).replace(from, to));
            let shape = product(&file, 300);
            for at in [id, id + 1] {
                let (r, d, b) = with(Limits::default(), |cx| profile(&file, leaf(&shape, at), cx));
                assert!(
                    matches!(r, Err(Error::Invalid)),
                    "{version} {at} {to}: {r:?}"
                );
                assert!(!d.is_empty());
                assert_eq!(b.used(Kind::Depth), 0);
            }
        }
    }
}
#[test]
fn complex_profiles_and_curves_are_unsupported_and_missing_units_remain_unknown() {
    for (from, to) in [
        (
            "IFCARBITRARYPROFILEDEFWITHVOIDS(.AREA.,'round and rectangular',#15,(#31,#35))",
            "(IFCARBITRARYPROFILEDEFWITHVOIDS(.AREA.,'round and rectangular',#15,(#31,#35)))",
        ),
        ("IFCCIRCLE(#29,2.)", "(IFCCIRCLE(#29,2.))"),
        ("IFCCIRCLE(#29,2.)", "IFCELLIPSE(#29,2.,1.)"),
    ] {
        let file = read(&source("IFC4").replace(from, to));
        let shape = product(&file, 300);
        let (r, d, b) = with(Limits::default(), |cx| {
            profile(&file, leaf(&shape, 210), cx)
        });
        assert!(matches!(r, Err(Error::Unsupported)), "{to}: {r:?}");
        assert!(!d.is_empty());
        assert_eq!(b.used(Kind::Depth), 0);
    }
    let file = read(&source("IFC4").replace("$,#2)", "$,$)"));
    let shape = product(&file, 300);
    let (r, d, _) = with(Limits::default(), |cx| {
        profile(&file, leaf(&shape, 210), cx)
    });
    assert!(r.unwrap().1.length_unit.is_none());
    assert!(d.iter().any(|d| d.code() == codes::IFC_UNIT_MISSING));
}
#[test]
fn preserves_existing_building_ids_hashes_and_source_independent_definitions() {
    for version in VERSIONS {
        let src = source(version);
        let file = read(&src);
        let shape = product(&file, 300);
        let before = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
            .0
            .unwrap();
        let first = with(Limits::default(), |cx| {
            profile(&file, leaf(&shape, 200), cx)
        })
        .0
        .unwrap();
        for id in [211, 201, 210] {
            with(Limits::default(), |cx| profile(&file, leaf(&shape, id), cx))
                .0
                .unwrap();
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
        let changed = read(&src.replace("#54", "#954").replace("#40", "#940"));
        let shape = product(&changed, 300);
        let result = with(Limits::default(), |cx| {
            profile(&changed, leaf(&shape, 200), cx)
        })
        .0
        .unwrap();
        assert_eq!(first.0, result.0);
        assert_eq!(
            first.1.items.iter().map(|i| &i.kind).collect::<Vec<_>>(),
            result.1.items.iter().map(|i| &i.kind).collect::<Vec<_>>()
        );
        let changed = read(&src.replace("#15,(#82,#102)", "#15,(#102,#82)"));
        let shape = product(&changed, 300);
        let (_, g) = with(Limits::default(), |cx| {
            profile(&changed, leaf(&shape, 200), cx)
        })
        .0
        .unwrap();
        assert_eq!(
            g.items
                .iter()
                .filter(|i| matches!(i.kind, CurveKind::Composite { .. }))
                .map(|i| i.source.instance.unwrap())
                .collect::<Vec<_>>(),
            [102, 82]
        );
    }
}
#[test]
fn all_budget_limits_and_cancellation_restore_the_callers_depth() {
    for version in VERSIONS {
        let file = read(&source(version));
        let shape = product(&file, 300);
        for id in [200, 201, 210, 211] {
            let node = leaf(&shape, id);
            let (_, _, full) = with(Limits::default(), |cx| profile(&file, node, cx));
            for kind in Kind::ALL {
                let cap = if kind == Kind::Depth {
                    if id < 210 { 5 } else { 3 }
                } else {
                    full.used(kind)
                };
                let (r, _, b) = with(Limits::default().with(kind, Limit::Finite(cap)), |cx| {
                    profile(&file, node, cx)
                });
                assert!(r.is_ok(), "{id} {kind:?}: {r:?}");
                assert_eq!(b.used(Kind::Depth), 0);
                if cap > 0 {
                    let (r, d, b) =
                        with(Limits::default().with(kind, Limit::Finite(cap - 1)), |cx| {
                            profile(&file, node, cx)
                        });
                    assert!(matches!(r, Err(Error::Stopped(_))), "{id} {kind:?}: {r:?}");
                    assert!(d.iter().any(|d| d.code() == kind.exceeded_code()));
                    assert_eq!(b.used(Kind::Depth), 0);
                }
            }
            let mut b = Budget::new(Limits::default());
            let mut d = Diagnostics::new();
            let c = CancelToken::new();
            let mut cx = Cx::new(&mut b, &mut d, &c);
            cx.enter().unwrap();
            c.cancel();
            assert!(matches!(
                profile(&file, node, &mut cx),
                Err(Error::Stopped(_))
            ));
            cx.leave();
            assert_eq!(b.used(Kind::Depth), 0);
        }
    }
}
const PUBLIC: &str = include_str!("../../../samples/ifc/bim-whale/TallBuilding.ifc");
#[test]
fn public_window_reaches_its_shared_extruded_profile_and_inner_boundary() {
    let file = read(PUBLIC);
    let shape = product(&file, 13257);
    let (r, d, _) = with(Limits::default(), |cx| {
        profile(&file, leaf(&shape, 13207), cx)
    });
    assert!(d.is_empty(), "{d:?}");
    let (p, g) = r.unwrap();
    let AreaProfileKind::ArbitraryWithVoids { boundary, holes } = p.kind else {
        panic!()
    };
    assert_eq!(holes.len(), 1);
    assert!(matches!(
        g.item(boundary).unwrap().kind,
        CurveKind::Polyline(_)
    ));
    assert!(matches!(
        g.item(holes[0]).unwrap().kind,
        CurveKind::Polyline(_)
    ));
}
#[test]
fn mutations_and_truncation_finish_with_one_budget() {
    let run = |bytes: &[u8], cx: &mut Cx| {
        let file = Ifc::read(bytes, cx)?;
        let shape = file.product_shape(300, cx)?;
        for node in &shape.nodes {
            cx.check(Kind::Work, 1)?;
            if [b"IFCEXTRUDEDAREASOLID".as_slice(), b"IFCREVOLVEDAREASOLID"]
                .iter()
                .any(|k| file.kind(node.source) == *k)
            {
                profile(&file, node, cx)?;
            }
        }
        Ok(())
    };
    for version in VERSIONS {
        let bytes = source(version).into_bytes();
        for end in (0..bytes.len()).step_by(97) {
            zumen_sdk::testkit::survives_with(&bytes[..end], run).unwrap();
        }
        for at in (0..bytes.len()).step_by(89) {
            for byte in [0, b'$', b'#', 255] {
                let mut b = bytes.clone();
                b[at] = byte;
                zumen_sdk::testkit::survives_with(&b, run).unwrap();
            }
        }
    }
}

use zumen_building::{Coordinates, CurveTrim};
use zumen_json::Writer;

fn numbers(out: &mut Writer, values: &[f64]) {
    out.begin_array();
    for &v in values {
        out.float(v);
    }
    out.end_array();
}
fn coordinates(out: &mut Writer, value: Coordinates) {
    match value {
        Coordinates::Two(v) => numbers(out, &v),
        Coordinates::Three(v) => numbers(out, &v),
    }
}
fn optional_bool(out: &mut Writer, value: Option<bool>) {
    if let Some(v) = value {
        out.bool(v)
    } else {
        out.null()
    }
}
fn trims(out: &mut Writer, values: &[CurveTrim]) {
    out.begin_array();
    for v in values {
        out.begin_object();
        match v {
            CurveTrim::Point(p) => {
                out.key("point");
                coordinates(out, *p)
            }
            CurveTrim::Parameter(p) => {
                out.key("parameter");
                out.float(*p)
            }
        }
        out.end_object();
    }
    out.end_array();
}
fn json_profile(out: &mut Writer, p: &zumen_building::AreaProfile, g: &CurveGraph) {
    out.key("name");
    if let Some(name) = &p.name {
        out.string(name)
    } else {
        out.null()
    }
    let AreaProfileKind::ArbitraryWithVoids { boundary, holes } = &p.kind else {
        panic!()
    };
    out.key("outer");
    out.uint(g.item(*boundary).unwrap().source.instance.unwrap());
    out.key("holes");
    out.begin_array();
    for id in ids(g, holes) {
        out.uint(id);
    }
    out.end_array();
    out.key("curves");
    out.begin_array();
    for item in &g.items {
        out.begin_object();
        out.key("instance");
        out.uint(item.source.instance.unwrap());
        match &item.kind {
            CurveKind::Polyline(points) => {
                out.key("kind");
                out.string("polyline");
                out.key("points");
                out.begin_array();
                for &p in points {
                    coordinates(out, p);
                }
                out.end_array();
            }
            CurveKind::Line {
                point,
                direction,
                magnitude,
            } => {
                out.key("kind");
                out.string("line");
                out.key("point");
                coordinates(out, *point);
                out.key("direction");
                coordinates(out, *direction);
                out.key("magnitude");
                out.float(*magnitude);
            }
            CurveKind::Circle { position, radius } => {
                out.key("kind");
                out.string("circle");
                out.key("location");
                coordinates(out, position.location);
                out.key("reference_direction");
                if let Some(v) = position.reference_direction {
                    coordinates(out, v)
                } else {
                    out.null()
                }
                out.key("radius");
                out.float(*radius);
            }
            CurveKind::Trimmed {
                basis,
                trim1,
                trim2,
                sense_agreement,
                preference,
            } => {
                out.key("kind");
                out.string("trimmed");
                out.key("basis");
                out.uint(g.item(*basis).unwrap().source.instance.unwrap());
                out.key("trim1");
                trims(out, trim1);
                out.key("trim2");
                trims(out, trim2);
                out.key("sense");
                out.bool(*sense_agreement);
                out.key("preference");
                out.string(match preference {
                    zumen_building::TrimmingPreference::Cartesian => "CARTESIAN",
                    zumen_building::TrimmingPreference::Parameter => "PARAMETER",
                    zumen_building::TrimmingPreference::Unspecified => "UNSPECIFIED",
                });
            }
            CurveKind::Composite {
                segments,
                self_intersect,
            } => {
                out.key("kind");
                out.string("composite");
                out.key("self_intersect");
                optional_bool(out, *self_intersect);
                out.key("segments");
                out.begin_array();
                for s in segments {
                    out.begin_object();
                    out.key("curve");
                    out.uint(g.item(s.curve).unwrap().source.instance.unwrap());
                    out.key("same_sense");
                    out.bool(s.same_sense);
                    out.key("transition");
                    out.string(match s.transition {
                        zumen_building::CurveTransition::Discontinuous => "DISCONTINUOUS",
                        zumen_building::CurveTransition::Continuous => "CONTINUOUS",
                        zumen_building::CurveTransition::SameGradient => "CONTSAMEGRADIENT",
                        zumen_building::CurveTransition::SameGradientSameCurvature => {
                            "CONTSAMEGRADIENTSAMECURVATURE"
                        }
                    });
                    out.end_object();
                }
                out.end_array();
            }
            _ => panic!("照合用の入力にない曲線"),
        }
        out.end_object();
    }
    out.end_array();
}
fn position(out: &mut Writer, p: Option<&zumen_building::AxisPlacement3>) {
    if let Some(p) = p {
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
    } else {
        out.null()
    }
}
#[test]
#[allow(clippy::disallowed_methods)] // 自作入力と公開例の結果だけを指定先へ出す。
fn exports_attributes_sources_diagnostics_and_all_budget_kinds() {
    let Ok(folder) = std::env::var("ZUMEN_IFC_VOID_PROFILES_OUT") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    let mut contract = String::new();
    let mut export =
        |file: &IfcFile, path: &str, name: &str, product_id: u64, gid: &str, items: &[u64]| {
            let shape = product(file, product_id);
            let mut out = Writer::new();
            out.begin_object();
            out.key("file");
            out.string(path);
            out.key("product");
            out.uint(product_id);
            out.key("global_id");
            out.string(gid);
            out.key("items");
            out.begin_array();
            for &id in items {
                let node = leaf(&shape, id);
                out.begin_object();
                out.key("instance");
                out.uint(id);
                let (debug, d, b) = if file.kind(node.source) == b"IFCEXTRUDEDAREASOLID" {
                    let (r, d, b) = with(Limits::default(), |cx| file.extrusion(node, cx));
                    let v = r.unwrap();
                    out.key("kind");
                    out.string("extrusion");
                    out.key("profile");
                    out.uint(v.source.profile.id().unwrap());
                    out.key("position");
                    position(&mut out, v.definition.position.as_ref());
                    out.key("direction");
                    numbers(&mut out, &v.definition.direction);
                    out.key("depth");
                    out.float(v.definition.depth);
                    json_profile(
                        &mut out,
                        &v.definition.profile,
                        v.boundary.as_ref().unwrap(),
                    );
                    (format!("{v:?}"), d, b)
                } else {
                    let (r, d, b) = with(Limits::default(), |cx| file.revolved_area(node, cx));
                    let v = r.unwrap();
                    out.key("kind");
                    out.string("revolution");
                    out.key("profile");
                    out.uint(v.source.profile.id().unwrap());
                    out.key("position");
                    position(&mut out, v.definition.position.as_ref());
                    out.key("axis_location");
                    numbers(&mut out, &v.definition.axis.location);
                    out.key("axis_direction");
                    numbers(&mut out, &v.definition.axis.direction);
                    out.key("angle");
                    out.float(v.definition.angle.value);
                    json_profile(
                        &mut out,
                        &v.definition.profile,
                        v.boundary.as_ref().unwrap(),
                    );
                    (format!("{v:?}"), d, b)
                };
                out.end_object();
                contract += &format!(
                    "{name}:{id}:ok:{debug}:{d:?}:{:?}\n",
                    Kind::ALL.map(|k| b.used(k))
                );
                for kind in Kind::ALL {
                    let used = if kind == Kind::Depth {
                        if id == 200 || id == 201 { 5 } else { 3 }
                    } else {
                        b.used(kind)
                    };
                    for cap in [0, used.saturating_sub(1), used] {
                        let (r, d, b) =
                            with(Limits::default().with(kind, Limit::Finite(cap)), |cx| {
                                profile(file, node, cx)
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
        export(
            &read(&src),
            &format!("{version}.ifc"),
            version,
            300,
            "0000000000000000000002",
            &[200, 201, 210, 211],
        );
        std::fs::write(folder.join(format!("{version}.ifc")), src).unwrap();
    }
    export(
        &read(PUBLIC),
        "../../samples/ifc/bim-whale/TallBuilding.ifc",
        "public_window",
        13257,
        "0ZBert7zf3GhThbO52NKQt",
        &[13207],
    );
    for version in VERSIONS {
        for (id, from, to) in invalid_inputs() {
            let file = read(&source(version).replace(from, to));
            let shape = product(&file, 300);
            for at in [id, id + 1] {
                let (r, d, b) = with(Limits::default(), |cx| profile(&file, leaf(&shape, at), cx));
                contract += &format!(
                    "{version}:{at}:{to}:{:?}:{d:?}:{:?}\n",
                    r.err(),
                    Kind::ALL.map(|k| b.used(k))
                );
            }
        }
    }
    std::fs::write(folder.join("contract.txt"), contract).unwrap();
}
