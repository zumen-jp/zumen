//! 3D の共有写像の元属性と省略を、幾何の評価から分けて検査する。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{Building, ElementRef, Hash, MappingScale3};
use zumen_ifc::{Ifc, IfcFile, MappedShape3, ProductShape, ShapeLinks, ShapeNode};
use zumen_json::Writer;
use zumen_sdk::{Codec, Error, Lift};

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];
fn source(version: &str) -> String {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('mapping'),'2;1');\nFILE_NAME('mapping','',(),(),'','','');\nFILE_SCHEMA(('{version}'));\nENDSEC;\nDATA;\n\
#1=IFCPROJECT('0000000000000000000001',$,$,$,$,$,$,$,#2);\n\
#2=IFCUNITASSIGNMENT((#3));\n#3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n\
#8=IFCGEOMETRICREPRESENTATIONCONTEXT('Model','Model',3,0.00001,#9,$);\n\
#9=IFCAXIS2PLACEMENT3D(#10,#11,#12);\n#10=IFCCARTESIANPOINT((10.,-0.,30.));\n\
#11=IFCDIRECTION((0.,0.,3.));\n#12=IFCDIRECTION((2.,0.,1.));\n\
#13=IFCDIRECTION((0.,-4.,0.));\n#14=IFCCARTESIANPOINT((-1.,-2.,-3.));\n\
#15=IFCAXIS2PLACEMENT3D(#14,$,$);\n\
#20=IFCCARTESIANTRANSFORMATIONOPERATOR3D($,$,#14,$,$);\n\
#21=IFCCARTESIANTRANSFORMATIONOPERATOR3D(#12,#13,#14,2.,#11);\n\
#22=IFCCARTESIANTRANSFORMATIONOPERATOR3DNONUNIFORM(#12,#13,#14,2.,#11,3.,4.);\n\
#23=IFCCARTESIANTRANSFORMATIONOPERATOR3DNONUNIFORM($,#13,#14,$,$,$,5.);\n\
#30=IFCPOLYLINE((#10,#14));\n#31=IFCSHAPEREPRESENTATION(#8,'Axis','Curve3D',(#30));\n\
#32=IFCREPRESENTATIONMAP(#9,#31);\n#33=IFCREPRESENTATIONMAP(#15,#31);\n\
#40=IFCMAPPEDITEM(#32,#20);\n#41=IFCMAPPEDITEM(#32,#21);\n#42=IFCMAPPEDITEM(#32,#22);\n#43=IFCMAPPEDITEM(#33,#23);\n\
#44=IFCSHAPEREPRESENTATION(#8,'Body','MappedRepresentation',(#40,#41,#42,#43));\n#45=IFCREPRESENTATIONMAP(#15,#44);\n#46=IFCMAPPEDITEM(#45,#21);\n\
#100=IFCBEAM('0000000000000000000002',$,$,$,$,$,#101,$,.NOTDEFINED.);\n\
#101=IFCPRODUCTDEFINITIONSHAPE($,$,(#102));\n#102=IFCSHAPEREPRESENTATION(#8,'Body','MappedRepresentation',(#46,#40));\n\
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
fn get(file: &IfcFile, id: u64) -> (Result<MappedShape3<'_>, Error>, Diagnostics, Budget) {
    let node = ShapeNode {
        source: file.instance(id).unwrap(),
        links: ShapeLinks::Unsupported,
    };
    with(Limits::default(), |cx| file.mapped_shape3(&node, cx))
}
fn value(file: &IfcFile, id: u64) -> MappedShape3<'_> {
    let (r, d, _) = get(file, id);
    assert!(d.is_empty(), "{d:?}");
    r.unwrap()
}

#[test]
fn preserves_raw_axes_scales_omission_shared_maps_and_nested_mapping() {
    for version in VERSIONS {
        let file = read(&source(version));
        let shape = product(&file, 100);
        assert_eq!(
            shape
                .nodes
                .iter()
                .filter(|n| n.source.id() == Some(32))
                .count(),
            1
        );
        let a = value(&file, 40);
        let b = value(&file, 41);
        let c = value(&file, 42);
        let d = value(&file, 43);
        assert_eq!(
            a.definition.origin.location[1].to_bits(),
            (-0.0f64).to_bits()
        );
        assert_eq!(a.definition.origin.axis, Some([0., 0., 3.]));
        assert_eq!(a.definition.origin.reference_direction, Some([2., 0., 1.]));
        assert_eq!(a.definition.target.local_origin, [-1., -2., -3.]);
        assert_eq!(a.definition.target.axis1, None);
        assert_eq!(a.definition.target.axis2, None);
        assert_eq!(a.definition.target.axis3, None);
        assert_eq!(a.definition.target.scale, MappingScale3::Uniform(None));
        assert_eq!(b.definition.target.axis1, Some([2., 0., 1.]));
        assert_eq!(b.definition.target.axis2, Some([0., -4., 0.]));
        assert_eq!(b.definition.target.axis3, Some([0., 0., 3.]));
        assert_eq!(b.definition.target.scale, MappingScale3::Uniform(Some(2.)));
        assert_eq!(
            c.definition.target.scale,
            MappingScale3::NonUniform {
                x: Some(2.),
                y: Some(3.),
                z: Some(4.)
            }
        );
        assert_eq!(
            d.definition.target.scale,
            MappingScale3::NonUniform {
                x: None,
                y: None,
                z: Some(5.)
            }
        );
        assert_eq!(d.definition.origin.location, [-1., -2., -3.]);
        assert_eq!(d.definition.origin.axis, None);
        assert!(std::ptr::eq(a.source.map, b.source.map));
        assert!(std::ptr::eq(a.source.map, c.source.map));
        assert!(std::ptr::eq(
            a.source.representation,
            d.source.representation
        ));
        assert_eq!(value(&file, 46).source.representation.id(), Some(44));
        for id in [40, 41, 42, 43, 46] {
            let node = leaf(&shape, id);
            let from_shape = with(Limits::default(), |cx| file.mapped_shape3(node, cx))
                .0
                .unwrap();
            assert_eq!(value(&file, id).definition, from_shape.definition);
        }
        assert_eq!(a.length_unit.unwrap().prefix.as_deref(), Some("MILLI"));
        for r in c.source.records {
            assert!(std::ptr::eq(r, file.instance(r.id().unwrap()).unwrap()));
        }
    }
}
fn invalid_inputs() -> Vec<(u64, &'static str, &'static str)> {
    vec![
        (40, "IFCMAPPEDITEM(#32,#20)", "IFCMAPPEDITEM(#32,#20,$)"),
        (40, "IFCMAPPEDITEM(#32,#20)", "IFCMAPPEDITEM(#999,#20)"),
        (
            40,
            "IFCREPRESENTATIONMAP(#9,#31)",
            "IFCREPRESENTATIONMAP(#9,#31,$)",
        ),
        (
            40,
            "IFCREPRESENTATIONMAP(#9,#31)",
            "IFCREPRESENTATIONMAP(#40,#31)",
        ),
        (
            40,
            "IFCREPRESENTATIONMAP(#9,#31)",
            "IFCREPRESENTATIONMAP(#9,#999)",
        ),
        (40, "#10,#11,#12)", "#10,#11,#12,$)"),
        (40, "#10,#11,#12)", "#10,#11,$)"),
        (40, "(10.,-0.,30.)", "(10.,-0.)"),
        (40, "(-1.,-2.,-3.)", "(-1.,-2.,1.E400)"),
        (40, "($,$,#14,$,$)", "($,$,#14,$,$,$)"),
        (41, "#14,2.,#11)", "#14,0.,#11)"),
        (41, "#14,2.,#11)", "#14,-1.,#11)"),
        (42, "#11,3.,4.)", "#11,$,0.)"),
        (42, "#11,3.,4.)", "#11,1.E400,$)"),
        (42, "#11,3.,4.)", "#11,3.)"),
        (42, "(0.,-4.,0.)", "(0.,-4.)"),
        (
            42,
            "IFCDIRECTION((0.,-4.,0.))",
            "IFCDIRECTION((0.,-4.,0.),$)",
        ),
        (42, "#12,#13,#14,2.,#11,3.,4.", "#12,#999,#14,2.,#11,3.,4."),
        (42, ".MILLI.,.METRE.", ".MILLI.,.SECOND."),
    ]
}
#[test]
fn rejects_bad_attributes_references_dimensions_and_scales_without_partial_values() {
    for version in VERSIONS {
        for (id, from, to) in invalid_inputs() {
            let file = read(&source(version).replace(from, to));
            let (r, d, b) = get(&file, id);
            assert!(matches!(r, Err(Error::Invalid)), "{version} {to}: {r:?}");
            assert!(d.iter().any(|d| d.code() == codes::IFC_SHAPE_INVALID));
            assert_eq!(b.used(Kind::Depth), 0);
        }
    }
}
#[test]
fn retains_ifc2x3_zero_target_direction_but_rejects_it_in_later_versions() {
    for version in VERSIONS {
        let file = read(&source(version).replace("(0.,-4.,0.)", "(0.,0.,0.)"));
        let (r, d, _) = get(&file, 42);
        if version == "IFC2X3" {
            assert_eq!(r.unwrap().definition.target.axis2, Some([0.; 3]));
            assert!(d.is_empty());
        } else {
            assert!(matches!(r, Err(Error::Invalid)));
            assert!(d.iter().any(|d| d.code() == codes::IFC_SHAPE_INVALID));
        }
    }
}
#[test]
fn rejects_2d_unknown_versions_complex_records_and_foreign_sources() {
    for (from, to) in [
        (
            "IFCAXIS2PLACEMENT3D(#10,#11,#12)",
            "IFCAXIS2PLACEMENT2D(#10,$)",
        ),
        (
            "IFCCARTESIANTRANSFORMATIONOPERATOR3D($,$,#14,$,$)",
            "IFCCARTESIANTRANSFORMATIONOPERATOR2D($,$,#14,$)",
        ),
        ("IFCMAPPEDITEM(#32,#20)", "(IFCMAPPEDITEM(#32,#20))"),
        (
            "IFCREPRESENTATIONMAP(#9,#31)",
            "(IFCREPRESENTATIONMAP(#9,#31))",
        ),
        (
            "IFCAXIS2PLACEMENT3D(#10,#11,#12)",
            "(IFCAXIS2PLACEMENT3D(#10,#11,#12))",
        ),
        (
            "IFCCARTESIANPOINT((-1.,-2.,-3.))",
            "(IFCCARTESIANPOINT((-1.,-2.,-3.)))",
        ),
    ] {
        let file = read(&source("IFC4").replace(from, to));
        let (r, d, b) = get(&file, 40);
        assert!(matches!(r, Err(Error::Unsupported)), "{to}: {r:?}");
        assert!(d.iter().any(|d| d.code() == codes::IFC_SHAPE_UNSUPPORTED));
        assert_eq!(b.used(Kind::Depth), 0);
    }
    let a = read(&source("IFC4"));
    let b = read(&source("IFC4"));
    let shape = product(&b, 100);
    assert!(matches!(
        with(Limits::default(), |cx| a
            .mapped_shape3(leaf(&shape, 40), cx))
        .0,
        Err(Error::Invalid)
    ));
    let file = read(&source("IFC4X2"));
    assert!(matches!(get(&file, 40).0, Err(Error::Unsupported)));
    let file = read(&source("IFC4"));
    assert!(matches!(get(&file, 32).0, Err(Error::Unsupported)));
}
#[test]
fn getter_keeps_existing_building_hashes_and_separates_sources_from_meaning() {
    for version in VERSIONS {
        let src = source(version);
        let file = read(&src);
        let before = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
            .0
            .unwrap();
        for id in [46, 43, 42, 41, 40] {
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
        let changed = read(&src.replace("#32", "#932").replace("#22", "#922"));
        assert_eq!(value(&file, 42).definition, value(&changed, 42).definition);
        let changed = read(&src.replace("#11,3.,4.)", "#11,3.,5.)"));
        assert_ne!(value(&file, 42).definition, value(&changed, 42).definition);
        let changed = read(&src.replace("$,#2)", "$,$)"));
        let (r, d, _) = get(&changed, 42);
        assert!(r.unwrap().length_unit.is_none());
        assert!(d.iter().any(|d| d.code() == codes::IFC_UNIT_MISSING));
    }
}
#[test]
fn budgets_before_allocation_and_cancellation_restore_callers_depth() {
    for version in VERSIONS {
        let file = read(&source(version));
        let shape = product(&file, 100);
        for id in [40, 41, 42, 43, 46] {
            let node = leaf(&shape, id);
            let (_, _, full) = get(&file, id);
            for kind in Kind::ALL {
                let cap = if kind == Kind::Depth {
                    2
                } else {
                    full.used(kind)
                };
                let (r, _, b) = with(Limits::default().with(kind, Limit::Finite(cap)), |cx| {
                    file.mapped_shape3(node, cx)
                });
                assert!(r.is_ok(), "{id} {kind:?}: {r:?}");
                assert_eq!(b.used(Kind::Depth), 0);
                if cap > 0 {
                    let (r, d, b) =
                        with(Limits::default().with(kind, Limit::Finite(cap - 1)), |cx| {
                            file.mapped_shape3(node, cx)
                        });
                    assert!(matches!(r, Err(Error::Stopped(_))));
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
                file.mapped_shape3(node, &mut cx),
                Err(Error::Stopped(_))
            ));
            cx.leave();
            assert_eq!(b.used(Kind::Depth), 0);
        }
    }
}
const PUBLIC: &str = include_str!("../../../samples/ifc/gni/model_9.ifc");
#[test]
fn public_column_reaches_mapping_origin_target_and_shared_representation() {
    let file = read(PUBLIC);
    let shape = product(&file, 131);
    let r = with(Limits::default(), |cx| {
        file.mapped_shape3(leaf(&shape, 125), cx)
    })
    .0
    .unwrap();
    assert_eq!(r.source.map.id(), Some(121));
    assert_eq!(r.source.representation.id(), Some(119));
    assert_eq!(r.source.operator.id(), Some(124));
    assert_eq!(r.definition.target.scale, MappingScale3::Uniform(Some(1.)));
}
#[test]
fn survives_mutation_truncation_and_one_shared_budget() {
    let run = |bytes: &[u8], cx: &mut Cx| {
        let f = Ifc::read(bytes, cx)?;
        let shape = f.product_shape(100, cx)?;
        for node in &shape.nodes {
            cx.check(Kind::Work, 1)?;
            if matches!(node.links, ShapeLinks::Mapped { .. }) {
                f.mapped_shape3(node, cx)?;
            }
        }
        Ok(())
    };
    for version in VERSIONS {
        let bytes = source(version).into_bytes();
        for end in (0..bytes.len()).step_by(53) {
            zumen_sdk::testkit::survives_with(&bytes[..end], run).unwrap();
        }
        for at in (0..bytes.len()).step_by(47) {
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
fn optional_numbers(out: &mut Writer, values: Option<&[f64; 3]>) {
    if let Some(v) = values {
        numbers(out, v)
    } else {
        out.null()
    }
}
fn optional_number(out: &mut Writer, value: Option<f64>) {
    if let Some(v) = value {
        out.float(v)
    } else {
        out.null()
    }
}
fn json_item(out: &mut Writer, v: &MappedShape3<'_>) {
    out.begin_object();
    out.key("instance");
    out.uint(v.source.item.id().unwrap());
    out.key("map");
    out.uint(v.source.map.id().unwrap());
    out.key("representation");
    out.uint(v.source.representation.id().unwrap());
    out.key("operator");
    out.uint(v.source.operator.id().unwrap());
    out.key("origin");
    out.begin_object();
    out.key("location");
    numbers(out, &v.definition.origin.location);
    out.key("axis");
    optional_numbers(out, v.definition.origin.axis.as_ref());
    out.key("reference_direction");
    optional_numbers(out, v.definition.origin.reference_direction.as_ref());
    out.end_object();
    let t = &v.definition.target;
    out.key("target");
    out.begin_object();
    out.key("local_origin");
    numbers(out, &t.local_origin);
    out.key("axis1");
    optional_numbers(out, t.axis1.as_ref());
    out.key("axis2");
    optional_numbers(out, t.axis2.as_ref());
    out.key("axis3");
    optional_numbers(out, t.axis3.as_ref());
    out.key("scale");
    out.begin_array();
    match t.scale {
        MappingScale3::Uniform(v) => optional_number(out, v),
        MappingScale3::NonUniform { x, y, z } => {
            optional_number(out, x);
            optional_number(out, y);
            optional_number(out, z);
        }
    }
    out.end_array();
    out.end_object();
    out.end_object();
}
#[test]
#[allow(clippy::disallowed_methods)] // 自作入力と公開例の結果を指定先へ出す。
fn exports_raw_attributes_diagnostics_and_budget_contracts() {
    let Ok(folder) = std::env::var("ZUMEN_IFC_MAPPINGS_OUT") else {
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
            out.key("items");
            out.begin_array();
            for &id in ids {
                let node = leaf(&shape, id);
                let (r, d, b) = with(Limits::default(), |cx| file.mapped_shape3(node, cx));
                let v = r.unwrap();
                json_item(&mut out, &v);
                contract += &format!(
                    "{name}:{id}:ok:{v:?}:{d:?}:{:?}\n",
                    Kind::ALL.map(|k| b.used(k))
                );
                for kind in Kind::ALL {
                    let used = if kind == Kind::Depth { 2 } else { b.used(kind) };
                    for cap in [0, used.saturating_sub(1), used] {
                        let (r, d, b) =
                            with(Limits::default().with(kind, Limit::Finite(cap)), |cx| {
                                file.mapped_shape3(node, cx)
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
            100,
            "0000000000000000000002",
            &[40, 41, 42, 43, 46],
        );
        std::fs::write(folder.join(format!("{version}.ifc")), src).unwrap();
    }
    export(
        &read(PUBLIC),
        "../../samples/ifc/gni/model_9.ifc",
        "public_column",
        131,
        "2q2CBrfQTCKPY0zxCdZygb",
        &[125],
    );
    for version in VERSIONS {
        for (id, from, to) in invalid_inputs() {
            let file = read(&source(version).replace(from, to));
            let (r, d, b) = get(&file, id);
            contract += &format!(
                "{version}:{to}:{:?}:{d:?}:{:?}\n",
                r.err(),
                Kind::ALL.map(|k| b.used(k))
            );
        }
    }
    std::fs::write(folder.join("contract.txt"), contract).unwrap();
}
