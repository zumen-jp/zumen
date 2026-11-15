//! 一つの要素から形・写像・表示・通り芯を取得し、共有と停止を確認する。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{Building, Hash};
use zumen_ifc::{GeometryValue, Ifc, IfcFile, ProductGeometry, Retrieved, ShapeLinks, ShapeRef};
use zumen_json::Writer;
use zumen_sdk::{Codec, Error, Lift};

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];
fn source(version: &str) -> String {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\nFILE_NAME('','',(''),(''),'','','');\nFILE_SCHEMA(('{version}'));\nENDSEC;\nDATA;\n\
    #1=IFCPROJECT('0000000000000000000001',$,'Project',$,$,$,$,(#8),#9);\n\
    #8=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-5,#20,$);\n\
    #9=IFCUNITASSIGNMENT((#10,#11));\n#10=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);\n#11=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);\n\
    #20=IFCAXIS2PLACEMENT3D(#21,$,$);\n#21=IFCCARTESIANPOINT((0.,0.,0.));\n\
    #22=IFCCARTESIANPOINT((1.,0.,0.));\n#23=IFCCARTESIANPOINT((0.,1.,0.));\n#24=IFCDIRECTION((0.,0.,1.));\n\
    #25=IFCCARTESIANPOINT((0.,0.));\n#26=IFCAXIS2PLACEMENT2D(#25,$);\n#27=IFCRECTANGLEPROFILEDEF(.AREA.,'R',#26,2.,3.);\n\
    #70=IFCDIRECTION((0.,1.,0.));\n#28=IFCAXIS1PLACEMENT(#21,#70);\n#29=IFCPOLYLINE((#21,#22));\n\
    #30=IFCEXTRUDEDAREASOLID(#27,#20,#24,4.);\n\
    #31=IFCREVOLVEDAREASOLID(#27,#20,#28,1.);\n\
    #32=IFCSWEPTDISKSOLID(#29,0.25,$,0.,1.);\n\
    #50=IFCPOLYLOOP((#21,#22,#23));\n#51=IFCFACEOUTERBOUND(#50,.T.);\n#52=IFCFACE((#51));\n\
    #53=IFCCLOSEDSHELL((#52));\n#54=IFCOPENSHELL((#52));\n#55=IFCCONNECTEDFACESET((#52));\n\
    #33=IFCFACETEDBREP(#53);\n#34=IFCSHELLBASEDSURFACEMODEL((#54));\n#35=IFCFACEBASEDSURFACEMODEL((#55));\n\
    #56=IFCPLANE(#20);\n#57=IFCBOUNDINGBOX(#21,2.,3.,4.);\n\
    #38=IFCHALFSPACESOLID(#56,.F.);\n#39=IFCBOXEDHALFSPACE(#56,.T.,#57);\n\
    #60=IFCCARTESIANPOINT((2.,0.));\n#61=IFCCARTESIANPOINT((0.,3.));\n#62=IFCPOLYLINE((#25,#60,#61,#25));\n\
    #40=IFCPOLYGONALBOUNDEDHALFSPACE(#56,.F.,#20,#62);\n\
    #41=IFCBOOLEANCLIPPINGRESULT(.DIFFERENCE.,#30,#38);\n\
    #58=IFCSHAPEREPRESENTATION(#8,'Body','SweptSolid',(#30));\n#59=IFCREPRESENTATIONMAP(#20,#58);\n\
    #63=IFCCARTESIANTRANSFORMATIONOPERATOR3D($,$,#21,1.,$);\n#42=IFCMAPPEDITEM(#59,#63);\n\
    #43=IFCGEOMETRICSET((#29));\n#44=IFCGEOMETRICCURVESET((#62));\n\
    #100=IFCWALL('0000000000000000000002',$,'Wall',$,$,#200,#101,${wall_type});\n\
    #101=IFCPRODUCTDEFINITIONSHAPE($,$,(#102));\n#102=IFCSHAPEREPRESENTATION(#8,'Body','GeometricSet',(#30,#31,#32,#33,#34,#35,#38,#39,#40,#41,#42,#43,#44{extra_items}));\n\
    #200=IFCLOCALPLACEMENT($,#20);\n\
    #300=IFCCOLOURRGB('Blue',0.,0.,1.);\n#301=IFCSURFACESTYLESHADING(#300{transparency});\n#302=IFCSURFACESTYLE('Surface',.BOTH.,(#301));\n\
    #303=IFCPRESENTATIONSTYLEASSIGNMENT((#302));\n#304=IFCSTYLEDITEM(#30,({surface}),$);\n\
    #305=IFCPRESENTATIONLAYERASSIGNMENT('Body',$,(#102),$);\n#306=IFCPRESENTATIONLAYERASSIGNMENT('Source',$,(#58),$);\n\
    {extra}\nENDSEC;\nEND-ISO-10303-21;\n",
        wall_type = if version == "IFC2X3" {
            ""
        } else {
            ",.NOTDEFINED."
        },
        transparency = if version == "IFC2X3" { "" } else { ",0.25" },
        surface = if version == "IFC2X3" { "#303" } else { "#302" },
        extra_items = if version == "IFC2X3" { "" } else { ",#36,#37" },
        extra = if version == "IFC2X3" {
            String::new()
        } else {
            format!(
                "#64=IFCCARTESIANPOINTLIST3D(((0.,0.,0.),(1.,0.,0.),(0.,1.,0.)){tags});\n#65=IFCINDEXEDPOLYGONALFACE((1,2,3));\n#36=IFCTRIANGULATEDFACESET(#64,$,.F.,((1,2,3)),$);\n#37=IFCPOLYGONALFACESET(#64,.F.,(#65),$);",
                tags = if version == "IFC4" { "" } else { ",$" }
            )
        }
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
fn get(file: &IfcFile) -> ProductGeometry<'_> {
    with(Limits::default(), |cx| file.product_geometry(100, cx))
        .0
        .unwrap()
}
fn reference(g: &ProductGeometry, id: u64) -> ShapeRef {
    ShapeRef(
        g.shape
            .nodes
            .iter()
            .position(|n| n.source.id() == Some(id))
            .unwrap() as u32,
    )
}

#[test]
fn gathers_supported_values_once_and_keeps_shared_graph_links() {
    for version in VERSIONS {
        let file = read(&source(version));
        let (r, d, _) = with(Limits::default(), |cx| file.product_geometry(100, cx));
        assert!(d.is_empty(), "{version}:{d:?}");
        let g = r.unwrap();
        assert_eq!(g.values.len(), g.shape.nodes.len());
        assert_eq!(
            g.shape
                .nodes
                .iter()
                .filter(|n| n.source.id() == Some(30))
                .count(),
            1
        );
        assert!(
            matches!(g.value(reference(&g,30)),Some(GeometryValue::Extrusion(v))if v.definition.depth==4.)
        );
        assert!(matches!(
            g.value(reference(&g, 31)),
            Some(GeometryValue::Revolution(_))
        ));
        assert!(matches!(
            g.value(reference(&g, 32)),
            Some(GeometryValue::SweptDisk(_))
        ));
        for id in [33, 34, 35] {
            assert!(matches!(
                g.value(reference(&g, id)),
                Some(GeometryValue::Faces(_))
            ));
        }
        for id in [38, 39, 40] {
            assert!(matches!(
                g.value(reference(&g, id)),
                Some(GeometryValue::HalfSpace(_))
            ));
        }
        for id in [29, 62] {
            assert!(matches!(
                g.value(reference(&g, id)),
                Some(GeometryValue::Curve(_))
            ));
        }
        if version != "IFC2X3" {
            for id in [36, 37] {
                assert!(matches!(
                    g.value(reference(&g, id)),
                    Some(GeometryValue::Tessellation(_))
                ));
            }
        }
        assert!(matches!(
            g.value(reference(&g, 42)),
            Some(GeometryValue::Mapping(_))
        ));
        let n = g.shape.node(reference(&g, 41)).unwrap();
        assert!(matches!(n.links,ShapeLinks::Boolean{first,..}if first==reference(&g,30)));
        let a = g
            .styles
            .iter()
            .filter(|s| s.item == reference(&g, 30))
            .collect::<Vec<_>>();
        assert_eq!(a.len(), 2);
        for (style, rep, layer) in [(a[0], 102, "Body"), (a[1], 58, "Source")] {
            assert_eq!(style.representation, Some(reference(&g, rep)));
            let Retrieved::Value(p) = &style.presentation else {
                panic!()
            };
            assert_eq!(p.layers[0].definition.name, layer);
            assert_eq!(p.direct.styles[0].source.style.id(), Some(302));
        }
        assert!(g.grid.is_none());
        assert!(g.value(ShapeRef(u32::MAX)).is_none());
    }
}

#[test]
fn building_element_is_an_entry_without_mutation_and_checks_original_identity() {
    let src = source("IFC4");
    let file = read(&src);
    let building = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
        .0
        .unwrap();
    let (at, element) = building
        .elements()
        .iter()
        .enumerate()
        .find(|(_, e)| e.source().instance == Some(100))
        .unwrap();
    let at = zumen_building::ElementRef(at as u32);
    let before = Hash::element(&building, at);
    let g = with(Limits::default(), |cx| file.element_geometry(element, cx))
        .0
        .unwrap();
    assert_eq!(g.shape.product.id(), Some(100));
    assert_eq!(Hash::element(&building, at), before);
    let changed = read(&src.replace("0000000000000000000002", "0000000000000000000003"));
    assert!(matches!(
        with(Limits::default(), |cx| changed
            .element_geometry(element, cx))
        .0,
        Err(Error::Invalid)
    ));
    let other = read(&src.replace("#100=IFCWALL", "#1100=IFCWALL"));
    assert!(matches!(
        with(Limits::default(), |cx| other.element_geometry(element, cx)).0,
        Err(Error::Invalid)
    ));
}

#[test]
fn keeps_unknown_shapes_and_styles_explicit_and_rejects_invalid_values() {
    let src = source("IFC4").replace("#32=IFCSWEPTDISKSOLID", "#32=IFCUNKNOWNGEOMETRY");
    let file = read(&src);
    let (r, d, _) = with(Limits::default(), |cx| file.product_geometry(100, cx));
    let g = r.unwrap();
    assert!(
        matches!(g.value(reference(&g,32)),Some(GeometryValue::Unsupported{kind})if kind=="IFCUNKNOWNGEOMETRY")
    );
    assert_eq!(
        d.iter()
            .filter(|d| d.code() == codes::IFC_SHAPE_UNSUPPORTED)
            .map(|d| d.count())
            .sum::<u64>(),
        1
    );
    let src = source("IFC4").replace("#27=IFCRECTANGLEPROFILEDEF", "#27=IFCUNKNOWNPROFILE");
    let file = read(&src);
    let g = get(&file);
    for id in [30, 31] {
        assert!(matches!(
            g.value(reference(&g, id)),
            Some(GeometryValue::Unsupported { .. })
        ));
    }
    let src = source("IFC4").replace(
        "#302=IFCSURFACESTYLE('Surface',.BOTH.,(#301));",
        "#302=IFCCURVESTYLE($,$,$,$);",
    );
    let file = read(&src);
    let g = get(&file);
    let styles = g
        .styles
        .iter()
        .filter(|s| s.item == reference(&g, 30))
        .collect::<Vec<_>>();
    assert!(
        styles
            .iter()
            .all(|s| matches!(s.presentation, Retrieved::Unsupported))
    );
    for (from, to) in [
        ("#24,4.", "#24,-4."),
        ("(#59,#63)", "(#59,#9999)"),
        (".DIFFERENCE.,#30,#38", ".DIFFERENCE.,#41,#38"),
    ] {
        let file = read(&source("IFC4").replace(from, to));
        let (r, _, b) = with(Limits::default(), |cx| file.product_geometry(100, cx));
        assert!(matches!(r, Err(Error::Invalid)), "{to}:{r:?}");
        assert_eq!(b.used(Kind::Depth), 0);
    }
}

#[test]
fn gets_grid_axes_when_the_product_has_no_shape() {
    let src=source("IFC4").replace("#100=IFCWALL('0000000000000000000002',$,'Wall',$,$,#200,#101,$,.NOTDEFINED.);","#100=IFCGRID('0000000000000000000002',$,'Grid',$,$,#200,$,(#401),(#402),$,.RECTANGULAR.);\n#401=IFCGRIDAXIS('U',#62,.T.);\n#402=IFCGRIDAXIS('V',#62,.F.);");
    let file = read(&src);
    let g = get(&file);
    assert!(g.values.is_empty());
    assert!(g.styles.is_empty());
    let Some(Retrieved::Value(grid)) = g.grid else {
        panic!()
    };
    assert_eq!(grid.axes.len(), 2);
}

#[test]
fn all_kinds_stop_without_returning_a_partial_product_and_restore_depth() {
    for version in VERSIONS {
        let file = read(&source(version));
        let call = |cx: &mut Cx| file.product_geometry(100, cx);
        let (r, _, b) = with(Limits::default(), call);
        assert!(r.is_ok());
        for kind in Kind::ALL {
            let used = if kind == Kind::Depth {
                (0..32)
                    .find(|&cap| {
                        with(Limits::default().with(kind, Limit::Finite(cap)), call)
                            .0
                            .is_ok()
                    })
                    .unwrap()
            } else {
                b.used(kind)
            };
            assert!(
                with(Limits::default().with(kind, Limit::Finite(used)), call)
                    .0
                    .is_ok()
            );
            if used > 0 {
                let (r, _, b) = with(Limits::default().with(kind, Limit::Finite(used - 1)), call);
                assert!(
                    matches!(r,Err(Error::Stopped(zumen_base::Stop::Exceeded(e)))if e.kind==kind)
                );
                assert_eq!(b.used(Kind::Depth), 0);
            }
        }
        for expire in [false, true] {
            let (r, _, b) = with(Limits::default(), |cx| {
                if expire {
                    cx.cancel.expire()
                } else {
                    cx.cancel.cancel()
                };
                call(cx)
            });
            assert!(matches!(r, Err(Error::Stopped(_))));
            assert_eq!(b.used(Kind::DecodedBytes), 0);
            assert_eq!(b.used(Kind::Depth), 0);
        }
    }
}

#[test]
fn repeated_and_mutated_reads_keep_diagnostics_usage_and_source() {
    let src = source("IFC4");
    let file = read(&src);
    let a = get(&file);
    let _ = with(Limits::default(), |cx| file.product_shape(100, cx));
    let b = get(&file);
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
    for at in (0..src.len()).step_by(89) {
        let mut bytes = src.as_bytes().to_vec();
        bytes[at] = b'$';
        zumen_sdk::testkit::survives_with(&bytes, |bytes, cx| {
            Ifc::read(bytes, cx)?.product_geometry(100, cx).map(drop)
        })
        .unwrap();
    }
}

#[test]
fn public_column_and_wall_keep_shared_sources_without_unexpanded_shape_diagnostics() {
    for (input, id, solid) in [
        (
            include_str!("../../../samples/ifc/gni/model_9.ifc"),
            131,
            114,
        ),
        (
            include_str!("../../../samples/ifc/bim-whale/TallBuilding.ifc"),
            615,
            600,
        ),
    ] {
        let file = read(input);
        let (r, d, _) = with(Limits::default(), |cx| file.product_geometry(id, cx));
        assert!(
            d.iter().all(|d| d.code() != codes::IFC_SHAPE_UNSUPPORTED),
            "{d:?}"
        );
        let g = r.unwrap();
        assert!(!matches!(
            g.value(reference(&g, solid)),
            Some(GeometryValue::Unsupported { .. })
        ));
        assert_eq!(g.values.len(), g.shape.nodes.len());
    }
}

#[test]
#[allow(clippy::disallowed_methods)] // 自作入力と公開例の値・予算だけを指定先へ出す。
fn exports_integrated_values_diagnostics_and_budgets() {
    let Ok(folder) = std::env::var("ZUMEN_IFC_GEOMETRY_OUT") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    let mut contract = String::new();
    let mut export = |name: &str, path: &str, file: &IfcFile, id: u64| {
        let call = |cx: &mut Cx| file.product_geometry(id, cx);
        let (v, d, b) = with(Limits::default(), call);
        let v = v.unwrap();
        let mut out = Writer::new();
        out.begin_object();
        out.key("file");
        out.string(path);
        out.key("product");
        out.uint(id);
        out.key("nodes");
        out.begin_array();
        for (node, value) in v.shape.nodes.iter().zip(&v.values) {
            out.begin_object();
            out.key("instance");
            out.uint(node.source.id().unwrap());
            out.key("kind");
            out.string(std::str::from_utf8(file.kind(node.source)).unwrap());
            out.key("sample");
            match value {
                GeometryValue::Extrusion(v) => out.float(v.definition.depth),
                GeometryValue::Revolution(v) => out.float(v.definition.angle.value),
                GeometryValue::SweptDisk(v) => out.float(v.definition.radius),
                GeometryValue::Mapping(v) => out.uint(v.source.operator.id().unwrap()),
                GeometryValue::HalfSpace(v) => out.bool(v.definition.agreement),
                _ => out.null(),
            };
            out.end_object();
        }
        out.end_array();
        out.key("styles");
        out.begin_array();
        for s in &v.styles {
            out.begin_object();
            out.key("item");
            out.uint(v.shape.node(s.item).unwrap().source.id().unwrap());
            out.key("representation");
            if let Some(r) = s.representation {
                out.uint(v.shape.node(r).unwrap().source.id().unwrap())
            } else {
                out.null()
            };
            let Retrieved::Value(p) = &s.presentation else {
                panic!()
            };
            out.key("styled_item");
            if let Some(r) = p.direct.source.styled_item {
                out.uint(r.id().unwrap())
            } else {
                out.null()
            };
            out.key("layers");
            out.begin_array();
            for l in &p.layers {
                out.uint(l.source.id().unwrap());
            }
            out.end_array();
            out.end_object();
        }
        out.end_array();
        out.end_object();
        std::fs::write(folder.join(format!("{name}.json")), out.into_bytes()).unwrap();
        contract += &format!(
            "{name}:ok:{:?}:{d:?}:{:?}\n",
            v,
            Kind::ALL.map(|k| b.used(k))
        );
        for kind in Kind::ALL {
            let used = if kind == Kind::Depth {
                (0..32)
                    .find(|&cap| {
                        with(Limits::default().with(kind, Limit::Finite(cap)), call)
                            .0
                            .is_ok()
                    })
                    .unwrap()
            } else {
                b.used(kind)
            };
            for cap in [0, used.saturating_sub(1), used] {
                let (r, d, b) = with(Limits::default().with(kind, Limit::Finite(cap)), call);
                contract += &format!(
                    "{name}:{kind:?}:{cap}:{:?}:{d:?}:{:?}\n",
                    r.err(),
                    Kind::ALL.map(|k| b.used(k))
                );
            }
        }
    };
    for version in VERSIONS {
        let src = source(version);
        export(version, &format!("{version}.ifc"), &read(&src), 100);
        std::fs::write(folder.join(format!("{version}.ifc")), src).unwrap();
    }
    export(
        "public_column",
        "../../samples/ifc/gni/model_9.ifc",
        &read(include_str!("../../../samples/ifc/gni/model_9.ifc")),
        131,
    );
    export(
        "public_wall",
        "../../samples/ifc/bim-whale/TallBuilding.ifc",
        &read(include_str!(
            "../../../samples/ifc/bim-whale/TallBuilding.ifc"
        )),
        615,
    );
    std::fs::write(folder.join("contract.txt"), contract).unwrap();
}
