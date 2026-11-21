//! 一つの要素から形・写像・表示・通り芯を取得し、共有と停止を確認する。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{
    Building, ElementRef, Geometry, GeometryData, GeometryDefinition as OwnedValue, GeometryError,
    GeometryLinks, GeometryRef, Hash,
};
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
        assert!(
            matches!(g.value(reference(&g,102)), Some(GeometryValue::Context(v)) if v.definition.geometric.dimension == 3)
        );
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
    let v = with(Limits::default(), |cx| file.owned_geometry(100, cx))
        .0
        .unwrap();
    let Some(zumen_building::Acquired::Value(owned)) = &v.data().grid else {
        panic!()
    };
    assert_eq!(owned.layout, grid.layout);
    for (a, b) in owned.axes.iter().zip(&grid.axes) {
        assert_eq!(a.definition, b.definition);
        assert_eq!(a.curve, b.curve);
        assert_eq!(a.source.instance, b.source.id());
    }
    let mut changed = v.data().clone();
    if let Some(zumen_building::Acquired::Value(grid)) = &mut changed.grid {
        grid.axes[0].definition.same_sense = false;
    }
    assert_ne!(v.fingerprint(), rebuild(changed).fingerprint());
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
            Ifc::read(bytes, cx)?.owned_geometry(100, cx).map(drop)
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
        let owned = with(Limits::default(), |cx| file.owned_geometry(id, cx))
            .0
            .unwrap();
        assert_eq!(owned.data().nodes.len(), g.shape.nodes.len());
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

fn owned(file: &IfcFile) -> Geometry {
    let (r, d, _) = with(Limits::default(), |cx| file.owned_geometry(100, cx));
    assert!(d.is_empty(), "{d:?}");
    r.unwrap()
}
fn rebuild(data: GeometryData) -> Geometry {
    with(Limits::default(), |cx| Geometry::new(data, cx))
        .0
        .unwrap()
}

#[test]
fn owning_preserves_all_definitions_units_shared_links_and_sources() {
    for version in VERSIONS {
        let file = read(&source(version));
        let borrowed = get(&file);
        let owned = owned(&file);
        let data = owned.data();
        assert_eq!(data.nodes.len(), borrowed.values.len());
        for ((node, value), result) in borrowed
            .shape
            .nodes
            .iter()
            .zip(&borrowed.values)
            .zip(&data.nodes)
        {
            assert_eq!(node.source.id(), result.source.instance);
            match (value, &result.definition) {
                (GeometryValue::Links, OwnedValue::Links) => {}
                (GeometryValue::Curve(a), OwnedValue::Curve(b)) => assert_eq!(a, b),
                (
                    GeometryValue::Context(a),
                    OwnedValue::Context {
                        definition,
                        length_unit,
                    },
                ) => {
                    assert_eq!(&a.definition, definition);
                    assert_eq!(&a.length_unit, length_unit);
                }
                (
                    GeometryValue::Extrusion(a),
                    OwnedValue::Extrusion {
                        definition,
                        boundary,
                        length_unit,
                    },
                ) => {
                    assert_eq!(&a.definition, definition);
                    assert_eq!(&a.boundary, boundary);
                    assert_eq!(&a.length_unit, length_unit);
                }
                (
                    GeometryValue::Revolution(a),
                    OwnedValue::Revolution {
                        definition,
                        boundary,
                        length_unit,
                    },
                ) => {
                    assert_eq!(&a.definition, definition);
                    assert_eq!(&a.boundary, boundary);
                    assert_eq!(&a.length_unit, length_unit);
                }
                (
                    GeometryValue::SweptDisk(a),
                    OwnedValue::SweptDisk {
                        definition,
                        directrix,
                    },
                ) => {
                    assert_eq!(&a.definition, definition);
                    assert_eq!(&a.directrix, directrix);
                }
                (
                    GeometryValue::Faces(a),
                    OwnedValue::Faces {
                        definition,
                        length_unit,
                    },
                ) => {
                    assert_eq!(&a.definition, definition);
                    assert_eq!(&a.length_unit, length_unit);
                    assert_eq!(
                        result
                            .source
                            .groups
                            .iter()
                            .find(|g| g.role == "points")
                            .unwrap()
                            .instances,
                        a.source
                            .points
                            .iter()
                            .map(|r| r.id().unwrap())
                            .collect::<Vec<_>>()
                    );
                }
                (
                    GeometryValue::Tessellation(a),
                    OwnedValue::Tessellation {
                        definition,
                        length_unit,
                    },
                ) => {
                    assert_eq!(&a.definition, definition);
                    assert_eq!(&a.length_unit, length_unit);
                }
                (
                    GeometryValue::HalfSpace(a),
                    OwnedValue::HalfSpace {
                        definition,
                        boundary,
                        length_unit,
                    },
                ) => {
                    assert_eq!(&a.definition, definition);
                    assert_eq!(&a.boundary, boundary);
                    assert_eq!(&a.length_unit, length_unit);
                }
                (
                    GeometryValue::Mapping(a),
                    OwnedValue::Mapping {
                        definition,
                        length_unit,
                    },
                ) => {
                    assert_eq!(&a.definition, definition);
                    assert_eq!(&a.length_unit, length_unit);
                }
                (GeometryValue::Unsupported { kind }, OwnedValue::Unsupported(other)) => {
                    assert_eq!(kind, other)
                }
                _ => panic!("{value:?}: {:?}", result.definition),
            }
        }
        for (a, b) in borrowed.styles.iter().zip(&data.styles) {
            assert_eq!(a.item.0, b.item.0);
            assert_eq!(a.representation.map(|r| r.0), b.representation.map(|r| r.0));
            let (Retrieved::Value(a), zumen_building::Acquired::Value(b)) =
                (&a.presentation, &b.presentation)
            else {
                panic!()
            };
            assert_eq!(a.direct.name, b.name);
            assert_eq!(a.layer_source, b.layer_source);
            assert_eq!(a.surface_source, b.surface_source);
            assert_eq!(a.layers.len(), b.layers.len());
            for (a, b) in a.direct.styles.iter().zip(&b.direct) {
                assert_eq!(a.definition, b.definition);
                assert_eq!(a.source.style.id(), b.source.instance);
            }
            for (a, b) in a.layers.iter().zip(&b.layers) {
                assert_eq!(a.definition, b.definition);
                assert_eq!(a.source.id(), b.source.instance);
            }
        }
    }
}

#[test]
fn rich_lift_keeps_metadata_ids_and_hash_v1_and_survives_dropping_l1() {
    for version in VERSIONS {
        let file = read(&source(version));
        let old = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
            .0
            .unwrap();
        let rich = with(Limits::default(), |cx| file.lift_geometry(cx))
            .0
            .unwrap();
        assert_eq!(old.document, rich.document);
        assert_eq!(old.components, rich.components);
        assert_eq!(old.elements(), rich.elements());
        assert_eq!(Hash::VERSION, 1);
        for at in 0..old.elements().len() {
            let at = ElementRef(at as u32);
            assert!(old.geometry(at).is_none());
            if rich.geometry(at).is_none() {
                assert_eq!(Hash::element(&old, at), Hash::element(&rich, at));
            } else {
                assert_ne!(Hash::element(&old, at), Hash::element(&rich, at));
            }
        }
        let at = ElementRef(1);
        let before = Hash::element(&rich, at).unwrap();
        let geometry = rich.geometry(at).unwrap();
        let shape_hash = geometry.fingerprint();
        assert_eq!(
            shape_hash.to_string(),
            if version == "IFC2X3" {
                "v1:b76a756759be66968d21a7e875c906fd"
            } else {
                "v1:87c9a18933475469b30f6ae83314ce3c"
            }
        );
        assert_eq!(
            before.to_string(),
            if version == "IFC2X3" {
                "v1:3e57d654a87f081a1100aafa81a6935c"
            } else {
                "v1:32099b0e506e7fa3fe687953ddb39046"
            }
        );
        file.product_shape(
            100,
            &mut Cx::new(
                &mut Budget::new(Limits::default()),
                &mut Diagnostics::new(),
                &CancelToken::new(),
            ),
        )
        .unwrap();
        assert_eq!(rich.geometry(at).unwrap().fingerprint(), shape_hash);
        assert_eq!(Hash::element(&rich, at), Some(before));
        let changed = read(&source(version).replace(
            "#30=IFCEXTRUDEDAREASOLID(#27,#20,#24,4.)",
            "#30=IFCEXTRUDEDAREASOLID(#27,#20,#24,5.)",
        ));
        let changed = with(Limits::default(), |cx| changed.lift_geometry(cx))
            .0
            .unwrap();
        assert_ne!(Hash::element(&changed, at), Some(before));
        assert_eq!(
            with(Limits::default(), |cx| Hash::element_checked(&rich, at, cx))
                .0
                .unwrap(),
            Some(before)
        );
        drop(file);
        assert!(!rich.geometry(at).unwrap().data().nodes.is_empty());
        assert_eq!(Hash::element(&rich, at), Some(before));
    }
}

#[test]
fn resident_shapes_styles_and_diagnostics_follow_edits_and_history() {
    use zumen_building::Transform;
    use zumen_ifc::{Edit, IfcLive, NewLiteral, NewPlacement, NewValue, RootAttribute, Target};
    let edits = [
        Edit::Attribute {
            element: 100,
            attribute: RootAttribute::Name,
            value: Some("New wall".into()),
        },
        Edit::Property {
            element: 100,
            set: "Common".into(),
            name: "Rating".into(),
            value: NewValue {
                type_name: "IFCINTEGER".into(),
                literal: NewLiteral::Integer(1),
            },
        },
        Edit::Place {
            element: Target::Instance(100),
            placement: NewPlacement {
                relative_to: None,
                local: Transform([[1., 0., 0., 2.], [0., 1., 0., 3.], [0., 0., 1., 4.]]),
            },
        },
    ];
    for version in VERSIONS {
        // 未使用のレコードを残し、属性編集で移った診断の場所も照合する。
        let input = source(version).replace("#300=", "#699=IFCCARTESIANPOINT((0.,0.));\n#300=");
        let mut states = vec![input.as_bytes().to_vec()];
        for edit in &edits {
            let file = with(Limits::default(), |cx| {
                Ifc::read(states.last().unwrap(), cx)
            })
            .0
            .unwrap();
            states.push(
                with(Limits::default(), |cx| {
                    zumen_ifc::apply(&file, std::slice::from_ref(edit), b"shape", cx)
                })
                .0
                .unwrap()
                .bytes,
            );
        }
        let zip = zumen_zip::write(&[("model.ifc", input.as_bytes())]).unwrap();
        for (bytes, threads) in [(input.as_bytes(), 1), (zip.as_slice(), 4)] {
            let mut live = with(Limits::default(), |cx| {
                IfcLive::open_with_geometry(bytes, threads, cx)
            })
            .0
            .unwrap();
            let check = |live: &IfcLive, spf: &[u8]| {
                let file = with(Limits::default(), |cx| Ifc::read(spf, cx)).0.unwrap();
                let (model, expected, _) = with(Limits::default(), |cx| file.lift_geometry(cx));
                let at = live.element_by_instance(100).unwrap();
                assert!(
                    !live
                        .building()
                        .geometry(at)
                        .unwrap()
                        .data()
                        .styles
                        .is_empty()
                );
                assert_eq!(live.building(), &model.unwrap());
                assert_eq!(
                    with(Limits::default(), |cx| live.save_spf(cx)).0.unwrap(),
                    spf
                );
                let (result, actual, _) =
                    with(Limits::default(), |cx| live.report_lift_diagnostics(cx));
                result.unwrap();
                assert_eq!(actual.into_vec(), expected.into_vec());
            };
            check(&live, &states[0]);
            for (i, edit) in edits.iter().enumerate() {
                let stopped = with(Limits::default().with(Kind::Work, Limit::Finite(0)), |cx| {
                    live.apply(std::slice::from_ref(edit), b"shape", cx)
                })
                .0;
                assert!(matches!(stopped, Err(Error::Stopped(_))));
                check(&live, &states[i]);
                with(Limits::default(), |cx| {
                    live.apply(std::slice::from_ref(edit), b"shape", cx)
                })
                .0
                .unwrap();
                check(&live, &states[i + 1]);
            }
            for state in states[..edits.len()].iter().rev() {
                assert!(with(Limits::default(), |cx| live.undo(cx)).0.unwrap());
                check(&live, state);
            }
            assert_eq!(
                with(Limits::default(), |cx| live.save(cx)).0.unwrap(),
                bytes
            );
            for state in &states[1..] {
                assert!(with(Limits::default(), |cx| live.redo(cx)).0.unwrap());
                check(&live, state);
            }
        }
    }
}

fn reverse_nodes(data: &mut GeometryData) {
    let n = data.nodes.len() as u32;
    let remap = |r: &mut GeometryRef| {
        r.0 = n - 1 - r.0;
    };
    if let Some(r) = &mut data.root {
        remap(r);
    }
    data.nodes.reverse();
    for node in &mut data.nodes {
        match &mut node.links {
            GeometryLinks::Definition(items)
            | GeometryLinks::Representation { items, .. }
            | GeometryLinks::Set { items, .. } => {
                for r in items {
                    remap(r);
                }
            }
            GeometryLinks::Mapped(r) | GeometryLinks::Map(r) => remap(r),
            GeometryLinks::Boolean { first, second, .. } => {
                remap(first);
                remap(second);
            }
            GeometryLinks::Leaf => {}
        }
        node.source.instance = Some(u64::MAX);
        node.source.groups.clear();
    }
    for s in &mut data.styles {
        remap(&mut s.item);
        if let Some(r) = &mut s.representation {
            remap(r);
        }
    }
    data.source.instance = None;
    data.source.groups.clear();
}

#[test]
fn hashes_ignore_step_numbers_graph_storage_order_and_all_source_groups() {
    let src = source("IFC4");
    let file = read(&src);
    let a = owned(&file);
    let shifted = read(
        &src.replace("#30", "#1300")
            .replace("#21", "#1210")
            .replace("#59", "#1590"),
    );
    let b = owned(&shifted);
    assert_eq!(a.fingerprint(), b.fingerprint());
    let mut data = a.data().clone();
    reverse_nodes(&mut data);
    assert_eq!(a.fingerprint(), rebuild(data).fingerprint());
    for (before, after) in [
        (
            "#30=IFCEXTRUDEDAREASOLID(#27,#20,#24,4.)",
            "#30=IFCEXTRUDEDAREASOLID(#27,#20,#24,5.)",
        ),
        (
            "#21=IFCCARTESIANPOINT((0.,0.,0.))",
            "#21=IFCCARTESIANPOINT((0.1,0.,0.))",
        ),
        (
            "#8=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-5",
            "#8=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-4",
        ),
        (
            "#301=IFCSURFACESTYLESHADING(#300,0.25)",
            "#301=IFCSURFACESTYLESHADING(#300,0.5)",
        ),
        (".LENGTHUNIT.,$,.METRE.", ".LENGTHUNIT.,.MILLI.,.METRE."),
    ] {
        assert!(src.contains(before));
        assert_ne!(
            a.fingerprint(),
            owned(&read(&src.replace(before, after))).fingerprint()
        );
    }
}

#[test]
fn nested_curve_and_face_storage_order_does_not_change_hash() {
    use zumen_building::{
        CurveItem, CurveKind, CurveRef, CurveSegment, CurveSource, CurveTransition,
    };
    let mut data = owned(&read(&source("IFC4"))).data().clone();
    for node in &mut data.nodes {
        if let OwnedValue::Curve(graph) = &mut node.definition {
            graph.items.push(CurveItem {
                kind: CurveKind::Composite {
                    segments: vec![CurveSegment {
                        curve: graph.root,
                        same_sense: true,
                        transition: CurveTransition::Continuous,
                    }],
                    self_intersect: None,
                },
                source: CurveSource::default(),
            });
            graph.root = CurveRef((graph.items.len() - 1) as u32);
        }
    }
    let expected = rebuild(data.clone()).fingerprint();
    for node in &mut data.nodes {
        if let OwnedValue::Curve(graph) = &mut node.definition {
            let n = graph.items.len() as u32;
            graph.root.0 = n - 1 - graph.root.0;
            graph.items.reverse();
            for item in &mut graph.items {
                item.source = CurveSource::default();
                if let CurveKind::Composite { segments, .. } = &mut item.kind {
                    for s in segments {
                        s.curve.0 = n - 1 - s.curve.0;
                    }
                }
            }
        }
        if let OwnedValue::Faces { definition, .. } = &mut node.definition {
            let n = definition.points.len() as u32;
            definition.points.reverse();
            for p in &mut definition.loops {
                for r in &mut p.points {
                    r.0 = n - 1 - r.0;
                }
            }
        }
    }
    assert_eq!(expected, rebuild(data).fingerprint());
}

#[test]
fn owned_graph_rejects_cycles_bad_references_and_unreachable_values() {
    let base = owned(&read(&source("IFC4")));
    let mut cycle = base.data().clone();
    let root = cycle.root.unwrap();
    cycle.nodes[root.0 as usize].links = GeometryLinks::Definition(vec![root]);
    let mut dangling = base.data().clone();
    dangling.nodes[root.0 as usize].links = GeometryLinks::Definition(vec![GeometryRef(u32::MAX)]);
    let mut unreachable = base.data().clone();
    unreachable.nodes[root.0 as usize].links = GeometryLinks::Definition(vec![]);
    let mut style = base.data().clone();
    style.styles[0].item = GeometryRef(u32::MAX);
    for (data, expected) in [
        (cycle, GeometryError::Cycle),
        (dangling, GeometryError::Reference),
        (unreachable, GeometryError::Unreachable),
        (style, GeometryError::Reference),
    ] {
        let (r, _, b) = with(Limits::default(), |cx| Geometry::new(data, cx));
        assert_eq!(r.unwrap_err(), expected);
        assert_eq!(b.used(Kind::Depth), 0);
    }
    for which in 0..3 {
        let mut data = base.data().clone();
        let mut changed = false;
        for node in &mut data.nodes {
            match (&mut node.definition, which) {
                (OwnedValue::Curve(g), 0) => {
                    g.root.0 = u32::MAX;
                    changed = true;
                }
                (OwnedValue::Faces { definition, .. }, 1) => {
                    definition.loops[0].points[0].0 = u32::MAX;
                    changed = true;
                }
                (OwnedValue::Tessellation { definition, .. }, 2) => {
                    if let zumen_building::IndexedFaces::Polygons { indices, .. } =
                        &mut definition.faces
                    {
                        indices[0].0 = u32::MAX;
                        changed = true;
                    }
                }
                _ => {}
            }
        }
        assert!(changed);
        let (r, _, b) = with(Limits::default(), |cx| Geometry::new(data, cx));
        assert_eq!(r.unwrap_err(), GeometryError::Reference);
        assert_eq!(b.used(Kind::Depth), 0);
    }
}

#[test]
fn rich_lift_reports_only_unread_records_and_preserves_unknown_kind_counts() {
    let src = source("IFC4");
    let file = read(&src);
    let (r, d, _) = with(Limits::default(), |cx| file.lift_geometry(cx));
    r.unwrap();
    for d in d.iter().filter(|d| d.code() == codes::IFC_ENTITY_DROPPED) {
        assert!(
            matches!(d.param("kind"), Some(zumen_base::Value::Text(k)) if k == "IFCPRESENTATIONSTYLEASSIGNMENT"),
            "{d:?}"
        );
    }
    let src = src
        .replace("#32=IFCSWEPTDISKSOLID", "#32=IFCUNKNOWNGEOMETRY")
        .replace(
            "ENDSEC;\nEND-ISO",
            "#900=IFCEXTRUDEDAREASOLID(#27,#20,#24,4.);\nENDSEC;\nEND-ISO",
        );
    let file = read(&src);
    let (r, d, _) = with(Limits::default(), |cx| file.lift_geometry(cx));
    let building = r.unwrap();
    assert!(
        building
            .geometry(ElementRef(1))
            .unwrap()
            .data()
            .nodes
            .iter()
            .any(
                |n| matches!(&n.definition, OwnedValue::Unsupported(k) if k == "IFCUNKNOWNGEOMETRY")
            )
    );
    for kind in ["IFCUNKNOWNGEOMETRY", "IFCEXTRUDEDAREASOLID"] {
        assert_eq!(
            d.iter()
                .filter(|d| d.code() == codes::IFC_ENTITY_DROPPED
                    && matches!(d.param("kind"), Some(zumen_base::Value::Text(k)) if k == kind))
                .map(|d| d.count())
                .sum::<u64>(),
            1
        );
    }
    assert_eq!(
        d.iter()
            .filter(|d| d.code() == codes::IFC_SHAPE_UNSUPPORTED)
            .map(|d| d.count())
            .sum::<u64>(),
        1
    );
}

#[test]
fn owned_geometry_and_rich_lift_charge_shared_budgets_and_restore_depth() {
    let file = read(&source("IFC4"));
    for rich in [false, true] {
        let call = |cx: &mut Cx| {
            if rich {
                file.lift_geometry(cx).map(drop)
            } else {
                file.owned_geometry(100, cx).map(drop)
            }
        };
        let (r, _, b) = with(Limits::default(), call);
        r.unwrap();
        for kind in Kind::ALL {
            let used = if kind == Kind::Depth {
                (0..64)
                    .find(|&n| {
                        with(Limits::default().with(kind, Limit::Finite(n)), call)
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
                assert!(matches!(r, Err(Error::Stopped(_))), "{kind:?}: {r:?}");
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
    }
}

#[test]
fn shared_boolean_graph_hashes_in_linear_work_and_without_recursive_ownership() {
    use zumen_building::{BooleanOperation, GeometryNode, GeometrySource};
    let mut data = GeometryData {
        root: Some(GeometryRef(0)),
        nodes: Vec::new(),
        styles: Vec::new(),
        grid: None,
        source: GeometrySource::default(),
    };
    for i in 0..100 {
        data.nodes.push(GeometryNode {
            links: if i == 99 {
                GeometryLinks::Leaf
            } else {
                GeometryLinks::Boolean {
                    operation: BooleanOperation::Union,
                    first: GeometryRef(i + 1),
                    second: GeometryRef(i + 1),
                }
            },
            definition: if i == 99 {
                OwnedValue::Unsupported("Example".into())
            } else {
                OwnedValue::Links
            },
            source: GeometrySource::default(),
        });
    }
    let (r, _, b) = with(
        Limits::default().with(Kind::Depth, Limit::Finite(100)),
        |cx| Geometry::new(data.clone(), cx),
    );
    assert!(r.is_ok());
    assert!(b.used(Kind::Work) < 1000);
    assert_eq!(b.used(Kind::Depth), 0);
    let (r, _, b) = with(
        Limits::default().with(Kind::Depth, Limit::Finite(99)),
        |cx| Geometry::new(data, cx),
    );
    assert!(matches!(r, Err(GeometryError::Stopped(_))));
    assert_eq!(b.used(Kind::Depth), 0);
}

#[test]
#[allow(clippy::disallowed_methods)] // 自作入力と公開例の値・ハッシュ・予算だけを指定先へ出す。
fn exports_owned_values_hashes_and_shared_budget_contract() {
    let Ok(folder) = std::env::var("ZUMEN_IFC_OWNED_OUT") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    let export = |name: &str, src: &str, id: u64, rich: bool| {
        let file = read(src);
        let call = |cx: &mut Cx| file.owned_geometry(id, cx);
        let (v, d, b) = with(Limits::default(), call);
        let v = v.unwrap();
        let mut out = format!(
            "{name}:{}:{v:?}:{d:?}:{:?}\n",
            v.fingerprint(),
            Kind::ALL.map(|k| b.used(k))
        );
        for kind in Kind::ALL {
            let used = if kind == Kind::Depth {
                (0..64)
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
                out += &format!(
                    "{kind:?}:{cap}:{:?}:{d:?}:{:?}\n",
                    r.err(),
                    Kind::ALL.map(|k| b.used(k))
                );
            }
        }
        if rich {
            let (r, d, b) = with(Limits::default(), |cx| file.lift_geometry(cx));
            let building = r.unwrap();
            out += &format!("lift:{d:?}:{:?}\n", Kind::ALL.map(|k| b.used(k)));
            for (at, element) in building.elements().iter().enumerate() {
                out += &format!(
                    "{}:{}\n",
                    element.id(),
                    Hash::element(&building, ElementRef(at as u32)).unwrap()
                );
            }
        }
        std::fs::write(folder.join(format!("{name}.txt")), out).unwrap();
    };
    for version in VERSIONS {
        export(version, &source(version), 100, true);
    }
    export(
        "public_column",
        include_str!("../../../samples/ifc/gni/model_9.ifc"),
        131,
        false,
    );
    export(
        "public_wall",
        include_str!("../../../samples/ifc/bim-whale/TallBuilding.ifc"),
        615,
        false,
    );
}
