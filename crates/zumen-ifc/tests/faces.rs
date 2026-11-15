//! 3 版の自作入力と公開例で、面集合の元値・共有・出所と予算を確かめる。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{Building, ElementRef, FaceModelKind, GlobalId, Hash};
use zumen_ifc::{FacetedFaces, Ifc, IfcFile, ProductShape, ShapeNode};
use zumen_json::Writer;
use zumen_sdk::{Codec, Error, Lift};

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];
fn source(version: &str) -> String {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('faces'),'2;1');\nFILE_NAME('faces','',(),(),'','','');\nFILE_SCHEMA(('{version}'));\nENDSEC;\nDATA;\n\
#1=IFCPROJECT('0000000000000000000001',$,$,$,$,$,$,$,#2);\n\
#2=IFCUNITASSIGNMENT((#3));\n#3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n\
#8=IFCGEOMETRICREPRESENTATIONCONTEXT('Model','Model',3,0.00001,#9,$);\n\
#9=IFCAXIS2PLACEMENT3D(#10,$,$);\n\
#10=IFCCARTESIANPOINT((0.,-0.,0.));\n#11=IFCCARTESIANPOINT((10.,0.,0.));\n\
#12=IFCCARTESIANPOINT((0.,10.,0.));\n#13=IFCCARTESIANPOINT((0.,0.,10.));\n\
#14=IFCCARTESIANPOINT((1.,1.,0.));\n#15=IFCCARTESIANPOINT((2.,1.,0.));\n#16=IFCCARTESIANPOINT((1.,2.,0.));\n\
#20=IFCPOLYLOOP((#10,#12,#11));\n#21=IFCPOLYLOOP((#10,#11,#13));\n\
#22=IFCPOLYLOOP((#10,#13,#12));\n#23=IFCPOLYLOOP((#11,#12,#13));\n\
#24=IFCPOLYLOOP((#14,#15,#16));\n\
#30=IFCFACEOUTERBOUND(#20,.T.);\n#31=IFCFACEOUTERBOUND(#21,.T.);\n\
#32=IFCFACEOUTERBOUND(#22,.T.);\n#33=IFCFACEOUTERBOUND(#23,.T.);\n\
#34=IFCFACEBOUND(#24,.F.);\n#35=IFCFACEBOUND(#20,.F.);\n\
#40=IFCFACE((#30));\n#41=IFCFACE((#31));\n#42=IFCFACE((#32));\n#43=IFCFACE((#33));\n\
#44=IFCFACE((#30,#34));\n#45=IFCFACE((#35));\n\
#50=IFCCLOSEDSHELL((#40,#41,#42,#43));\n#51=IFCOPENSHELL((#44,#45));\n\
#52=IFCCONNECTEDFACESET((#44,#40));\n\
#60=IFCFACETEDBREP(#50);\n#61=IFCSHELLBASEDSURFACEMODEL((#50,#51));\n#62=IFCFACEBASEDSURFACEMODEL((#52,#51));\n\
#70=IFCMAPPEDITEM(#71,#73);\n#71=IFCREPRESENTATIONMAP(#9,#72);\n\
#72=IFCSHAPEREPRESENTATION(#8,'Body','SurfaceModel',(#60,#61,#62));\n\
#73=IFCCARTESIANTRANSFORMATIONOPERATOR3D($,$,#10,2.,$);\n#74=IFCMAPPEDITEM(#71,#73);\n\
#100=IFCBEAM('0000000000000000000002',$,$,$,$,$,#101,${extra});\n\
#101=IFCPRODUCTDEFINITIONSHAPE($,$,(#102));\n#102=IFCSHAPEREPRESENTATION(#8,'Body','MappedRepresentation',(#70,#74));\n\
ENDSEC;\nEND-ISO-10303-21;\n",
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
fn value(file: &IfcFile, product_id: u64, id: u64) -> FacetedFaces<'_> {
    let shape = product(file, product_id);
    let (result, diag, _) = with(Limits::default(), |cx| {
        file.faceted_faces(leaf(&shape, id), cx)
    });
    assert!(diag.is_empty(), "{diag:?}");
    result.unwrap()
}

#[test]
fn gets_raw_points_faces_holes_orientation_and_sharing_through_mapped_items() {
    for version in VERSIONS {
        let file = read(&source(version));
        let brep = value(&file, 100, 60);
        assert_eq!(brep.definition.kind, FaceModelKind::FacetedSolid);
        assert_eq!(brep.definition.points.len(), 4);
        assert_eq!(brep.definition.faces.len(), 4);
        assert_eq!(brep.definition.sets[0].closed, Some(true));
        assert_eq!(brep.definition.points[0][1].to_bits(), (-0.0f64).to_bits());
        let surface = value(&file, 100, 61);
        let def = &surface.definition;
        assert_eq!(def.kind, FaceModelKind::ShellSurface);
        assert_eq!(def.points.len(), 7);
        assert_eq!(def.loops.len(), 5);
        assert_eq!(def.bounds.len(), 6);
        assert_eq!(def.faces.len(), 6);
        assert_eq!(def.sets.len(), 2);
        assert_eq!(def.sets[1].closed, Some(false));
        assert_eq!(def.faces[4].bounds[0], def.faces[0].bounds[0]);
        assert!(!def.bounds[4].orientation);
        assert!(!def.bounds[4].outer);
        assert!(!def.bounds[5].outer);
        assert_eq!(def.bounds[5].polygon, def.bounds[0].polygon);
        assert_eq!(def.loops[0].points.len(), 3);
        assert_eq!(def.points[def.loops[0].points[1].0 as usize], [0., 10., 0.]);
        assert_eq!(
            surface.length_unit.as_ref().unwrap().prefix.as_deref(),
            Some("MILLI")
        );
        for records in [
            &surface.source.sets,
            &surface.source.faces,
            &surface.source.bounds,
            &surface.source.loops,
            &surface.source.points,
            &surface.source.units,
        ] {
            assert!(
                records
                    .iter()
                    .all(|r| std::ptr::eq(*r, file.instance(r.id().unwrap()).unwrap()))
            );
        }
        let faces = value(&file, 100, 62);
        assert_eq!(faces.definition.kind, FaceModelKind::FaceSurface);
        assert_eq!(faces.definition.sets[0].closed, None);
        assert_eq!(
            faces.definition.sets[0].faces[0],
            faces.definition.sets[1].faces[0]
        );
        let shape = product(&file, 100);
        assert_eq!(
            shape
                .nodes
                .iter()
                .filter(|n| n.source.id() == Some(60))
                .count(),
            1
        );
    }
}

fn invalid_inputs() -> Vec<(&'static str, &'static str)> {
    vec![
        ("IFCFACETEDBREP(#50)", "IFCFACETEDBREP(#51)"),
        (
            "IFCSHELLBASEDSURFACEMODEL((#50,#51))",
            "IFCSHELLBASEDSURFACEMODEL((#52))",
        ),
        ("IFCCLOSEDSHELL((#40,#41,#42,#43))", "IFCCLOSEDSHELL(())"),
        ("IFCFACE((#30))", "IFCFACE((#30,#30))"),
        ("IFCFACE((#30))", "IFCFACE((#30,#31))"),
        ("IFCFACE((#30))", "IFCFACE(())"),
        ("IFCFACE((#30))", "IFCFACE((#999))"),
        ("IFCFACE((#30))", "IFCFACE((#40))"),
        ("IFCFACEOUTERBOUND(#20,.T.)", "IFCFACEOUTERBOUND(#20,.U.)"),
        ("IFCFACEOUTERBOUND(#20,.T.)", "IFCFACEOUTERBOUND(#20,$)"),
        ("IFCFACEOUTERBOUND(#20,.T.)", "IFCFACEOUTERBOUND(#20,.T.,$)"),
        ("IFCPOLYLOOP((#10,#12,#11))", "IFCPOLYLOOP((#10,#12))"),
        (
            "IFCPOLYLOOP((#10,#12,#11))",
            "IFCPOLYLOOP((#10,#12,#11,#10))",
        ),
        ("IFCPOLYLOOP((#10,#12,#11))", "IFCPOLYLOOP((#20,#12,#11))"),
        (
            "IFCCARTESIANPOINT((10.,0.,0.))",
            "IFCCARTESIANPOINT((0.,0.,0.))",
        ),
        (
            "IFCCARTESIANPOINT((10.,0.,0.))",
            "IFCCARTESIANPOINT((10.,0.))",
        ),
        (
            "IFCCARTESIANPOINT((10.,0.,0.))",
            "IFCCARTESIANPOINT((10.,1.E400,0.))",
        ),
        (
            "IFCCARTESIANPOINT((10.,0.,0.))",
            "IFCCARTESIANPOINT((10.,0.,0.),$)",
        ),
        ("IFCFACETEDBREP(#50)", "IFCFACETEDBREP(#60)"),
        (".MILLI.,.METRE.", ".MILLI.,.SECOND."),
    ]
}
#[test]
fn rejects_bad_topology_references_dimensions_units_and_cycles_without_partial_results() {
    for version in VERSIONS {
        for (from, to) in invalid_inputs() {
            let file = read(&source(version).replace(from, to));
            let shape = product(&file, 100);
            let (result, diag, budget) = with(Limits::default(), |cx| {
                file.faceted_faces(leaf(&shape, 61), cx)
            });
            // 立体の外殻の違反は、外殻を要求する立体から読む。
            let (result, diag, budget) = if from.starts_with("IFCFACETEDBREP") {
                with(Limits::default(), |cx| {
                    file.faceted_faces(leaf(&shape, 60), cx)
                })
            } else {
                (result, diag, budget)
            };
            assert!(
                matches!(result, Err(Error::Invalid)),
                "{version} {to}: {result:?}"
            );
            assert!(diag.iter().any(|d| d.code() == codes::IFC_SHAPE_INVALID));
            assert_eq!(budget.used(Kind::Depth), 0);
        }
    }
}

#[test]
fn reports_unsupported_kinds_complex_instances_and_foreign_nodes() {
    for version in VERSIONS {
        for (from, to) in [
            ("IFCFACETEDBREP(#50)", "IFCFACETEDBREPWITHVOIDS(#50,(#51))"),
            ("IFCFACE((#30))", "IFCFACESURFACE((#30),#9,.T.)"),
            ("IFCPOLYLOOP((#10,#12,#11))", "IFCEDGELOOP((#10,#12,#11))"),
            ("IFCFACETEDBREP(#50)", "(IFCFACETEDBREP(#50))"),
            (
                "IFCCARTESIANPOINT((10.,0.,0.))",
                "(IFCCARTESIANPOINT((10.,0.,0.)))",
            ),
            (
                "IFCPROJECT('0000000000000000000001',$,$,$,$,$,$,$,#2)",
                "(IFCPROJECT('0000000000000000000001',$,$,$,$,$,$,$,#2))",
            ),
        ] {
            let file = read(&source(version).replace(from, to));
            let node = ShapeNode {
                source: file.instance(60).unwrap(),
                links: zumen_ifc::ShapeLinks::Unsupported,
            };
            let (result, diag, budget) =
                with(Limits::default(), |cx| file.faceted_faces(&node, cx));
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
        let src = source(version);
        let file = read(&src);
        let other = read(&src);
        let shape = product(&other, 100);
        assert!(matches!(
            with(Limits::default(), |cx| file
                .faceted_faces(leaf(&shape, 60), cx))
            .0,
            Err(Error::Invalid)
        ));
        let missing = read(&src.replace("$,#2)", "$,$)"));
        let shape = product(&missing, 100);
        let (result, diag, _) = with(Limits::default(), |cx| {
            missing.faceted_faces(leaf(&shape, 60), cx)
        });
        assert!(result.unwrap().length_unit.is_none());
        assert!(diag.iter().any(|d| d.code() == codes::IFC_UNIT_MISSING));
    }
}

#[test]
fn separates_meaning_from_source_numbers_and_keeps_existing_ids_and_hashes() {
    for version in VERSIONS {
        let src = source(version);
        let file = read(&src);
        let before = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
            .0
            .unwrap();
        let first = value(&file, 100, 61);
        value(&file, 100, 62);
        value(&file, 100, 60);
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
        let renamed = read(
            &src.replace("#10=", "#910=")
                .replace("#10,", "#910,")
                .replace("(#10)", "(#910)")
                .replace("#20", "#920")
                .replace("#50", "#950"),
        );
        let second = value(&renamed, 100, 61);
        assert_eq!(first.definition, second.definition);
        assert_ne!(first.source.points[0].id(), second.source.points[0].id());
        let changed = read(&src.replace("IFCFACEBOUND(#24,.F.)", "IFCFACEBOUND(#24,.T.)"));
        assert_ne!(first.definition, value(&changed, 100, 61).definition);
    }
}

#[test]
fn rejects_unknown_versions_without_leaving_caller_depth() {
    let file = read(&source("IFC4X2"));
    let node = ShapeNode {
        source: file.instance(60).unwrap(),
        links: zumen_ifc::ShapeLinks::Unsupported,
    };
    let (result, diag, budget) = with(Limits::default(), |cx| {
        cx.enter().unwrap();
        let result = file.faceted_faces(&node, cx);
        cx.leave();
        result
    });
    assert!(matches!(result, Err(Error::Unsupported)));
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::IFC_VERSION_UNSUPPORTED)
    );
    assert_eq!(budget.used(Kind::Depth), 0);
}

#[test]
fn charges_every_budget_kind_before_allocation_and_unwinds_depth() {
    for version in VERSIONS {
        let file = read(&source(version));
        let shape = product(&file, 100);
        for id in 60..=62 {
            let node = leaf(&shape, id);
            let (_, _, full) = with(Limits::default(), |cx| file.faceted_faces(node, cx));
            for kind in Kind::ALL {
                let cap = if kind == Kind::Depth {
                    6
                } else {
                    full.used(kind)
                };
                let (ok, _, budget) =
                    with(Limits::default().with(kind, Limit::Finite(cap)), |cx| {
                        file.faceted_faces(node, cx)
                    });
                assert!(ok.is_ok(), "{version} {kind:?}: {ok:?}");
                assert_eq!(budget.used(Kind::Depth), 0);
                if cap > 0 {
                    let (fail, diag, budget) =
                        with(Limits::default().with(kind, Limit::Finite(cap - 1)), |cx| {
                            file.faceted_faces(node, cx)
                        });
                    assert!(matches!(fail, Err(Error::Stopped(_))), "{version} {kind:?}");
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
                file.faceted_faces(node, &mut cx),
                Err(Error::Stopped(_))
            ));
            cx.leave();
            assert_eq!(budget.used(Kind::Depth), 0);
        }
    }
}

fn public_cases() -> [(&'static str, u64, u64, &'static str, &'static str); 2] {
    [
        (
            "samples/ifc/bim-whale/TallBuilding.ifc",
            10949,
            10896,
            "0ZBert7zf3GhThbO52NKFT",
            "public_tall",
        ),
        (
            "samples/ifc/text2bim/sample_model.ifc",
            263,
            260,
            "13v5xDIG0HxwZGA6irHSbP",
            "public_text2bim",
        ),
    ]
}
fn public_source(path: &str) -> &'static str {
    if path == "samples/ifc/bim-whale/TallBuilding.ifc" {
        include_str!("../../../samples/ifc/bim-whale/TallBuilding.ifc")
    } else {
        assert_eq!(path, "samples/ifc/text2bim/sample_model.ifc");
        include_str!("../../../samples/ifc/text2bim/sample_model.ifc")
    }
}
#[test]
fn public_products_reach_face_graphs_by_global_id_and_step_reference() {
    for (path, product_id, item, gid, _) in public_cases() {
        let file = read(public_source(path));
        let building = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
            .0
            .unwrap();
        assert!(
            building
                .elements()
                .iter()
                .any(|e| e.source().instance == Some(product_id)
                    && e.id() == zumen_building::Id::Global(GlobalId::parse(gid).unwrap()))
        );
        let result = value(&file, product_id, item);
        assert_eq!(
            result.definition.kind,
            if item == 10896 {
                FaceModelKind::FacetedSolid
            } else {
                FaceModelKind::ShellSurface
            }
        );
        assert!(result.definition.faces.len() >= 4);
        assert!(result.definition.points.len() >= 4);
    }
}

#[test]
fn survives_mutations_and_truncations_with_one_shared_budget() {
    let run = |bytes: &[u8], cx: &mut Cx| {
        let file = Ifc::read(bytes, cx)?;
        let shape = file.product_shape(100, cx)?;
        for node in &shape.nodes {
            cx.check(Kind::Work, 1)?;
            if file
                .kind(node.source)
                .eq_ignore_ascii_case(b"IFCFACETEDBREP")
            {
                file.faceted_faces(node, cx)?;
            }
        }
        Ok(())
    };
    for version in VERSIONS {
        let bytes = source(version).into_bytes();
        for end in (0..bytes.len()).step_by(43) {
            zumen_sdk::testkit::survives_with(&bytes[..end], run).unwrap();
        }
        for at in (0..bytes.len()).step_by(47) {
            for byte in [0, b'$', b'#', 255] {
                let mut changed = bytes.clone();
                changed[at] = byte;
                zumen_sdk::testkit::survives_with(&changed, run).unwrap();
            }
        }
    }
}

fn ids(out: &mut Writer, ids: impl Iterator<Item = u64>) {
    out.begin_array();
    for id in ids {
        out.uint(id);
    }
    out.end_array();
}
fn json_item(out: &mut Writer, value: &FacetedFaces<'_>) {
    let g = &value.definition;
    let s = &value.source;
    out.begin_object();
    out.key("instance");
    out.uint(s.item.id().unwrap());
    out.key("kind");
    out.string(match g.kind {
        FaceModelKind::FacetedSolid => "faceted-solid",
        FaceModelKind::ShellSurface => "shell-surface",
        FaceModelKind::FaceSurface => "face-surface",
    });
    out.key("roots");
    ids(
        out,
        g.roots.iter().map(|r| s.sets[r.0 as usize].id().unwrap()),
    );
    out.key("sets");
    out.begin_array();
    for (set, src) in g.sets.iter().zip(&s.sets) {
        out.begin_object();
        out.key("instance");
        out.uint(src.id().unwrap());
        out.key("closed");
        if let Some(closed) = set.closed {
            out.bool(closed);
        } else {
            out.null();
        }
        out.key("faces");
        ids(
            out,
            set.faces
                .iter()
                .map(|r| s.faces[r.0 as usize].id().unwrap()),
        );
        out.end_object();
    }
    out.end_array();
    out.key("faces");
    out.begin_array();
    for (face, src) in g.faces.iter().zip(&s.faces) {
        out.begin_object();
        out.key("instance");
        out.uint(src.id().unwrap());
        out.key("bounds");
        ids(
            out,
            face.bounds
                .iter()
                .map(|r| s.bounds[r.0 as usize].id().unwrap()),
        );
        out.end_object();
    }
    out.end_array();
    out.key("bounds");
    out.begin_array();
    for (bound, src) in g.bounds.iter().zip(&s.bounds) {
        out.begin_object();
        out.key("instance");
        out.uint(src.id().unwrap());
        out.key("polygon");
        out.uint(s.loops[bound.polygon.0 as usize].id().unwrap());
        out.key("orientation");
        out.bool(bound.orientation);
        out.key("outer");
        out.bool(bound.outer);
        out.end_object();
    }
    out.end_array();
    out.key("loops");
    out.begin_array();
    for (polygon, src) in g.loops.iter().zip(&s.loops) {
        out.begin_object();
        out.key("instance");
        out.uint(src.id().unwrap());
        out.key("points");
        ids(
            out,
            polygon
                .points
                .iter()
                .map(|r| s.points[r.0 as usize].id().unwrap()),
        );
        out.end_object();
    }
    out.end_array();
    out.key("points");
    out.begin_array();
    for (point, src) in g.points.iter().zip(&s.points) {
        out.begin_object();
        out.key("instance");
        out.uint(src.id().unwrap());
        out.key("coordinates");
        out.begin_array();
        for &v in point {
            out.float(v);
        }
        out.end_array();
        out.end_object();
    }
    out.end_array();
    out.key("length_unit");
    let unit = value.length_unit.as_ref().unwrap();
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
    out.end_object();
}

#[test]
#[allow(clippy::disallowed_methods)] // 合成入力と公開例の取得結果だけを指定先へ出す。
fn exports_values_sources_diagnostics_and_all_budget_kinds() {
    let Ok(folder) = std::env::var("ZUMEN_IFC_FACES_OUT") else {
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
                let (value, diag, budget) =
                    with(Limits::default(), |cx| file.faceted_faces(node, cx));
                json_item(&mut out, &value.unwrap());
                contract += &format!(
                    "{name}:{id}:ok:{diag:?}:{:?}\n",
                    Kind::ALL.map(|k| budget.used(k))
                );
                for kind in Kind::ALL {
                    let used = if kind == Kind::Depth {
                        6
                    } else {
                        budget.used(kind)
                    };
                    for cap in [0, used.saturating_sub(1), used] {
                        let (result, diag, budget) =
                            with(Limits::default().with(kind, Limit::Finite(cap)), |cx| {
                                file.faceted_faces(node, cx)
                            });
                        contract += &format!(
                            "{name}:{id}:{kind:?}:{cap}:{:?}:{diag:?}:{:?}\n",
                            result.err(),
                            Kind::ALL.map(|k| budget.used(k))
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
        export(
            &file,
            &format!("{version}.ifc"),
            version,
            100,
            "0000000000000000000002",
            &[60, 61, 62],
        );
        std::fs::write(folder.join(format!("{version}.ifc")), src).unwrap();
    }
    for (path, product_id, item, gid, name) in public_cases() {
        let file = read(public_source(path));
        export(
            &file,
            &format!("../../{path}"),
            name,
            product_id,
            gid,
            &[item],
        );
    }
    for version in VERSIONS {
        for (from, to) in invalid_inputs() {
            let file = read(&source(version).replace(from, to));
            let shape = product(&file, 100);
            let node = leaf(
                &shape,
                if from.starts_with("IFCFACETEDBREP") {
                    60
                } else {
                    61
                },
            );
            let (result, diag, budget) = with(Limits::default(), |cx| file.faceted_faces(node, cx));
            contract += &format!(
                "{version}:{to}:{:?}:{diag:?}:{:?}\n",
                result.err(),
                Kind::ALL.map(|k| budget.used(k))
            );
        }
    }
    std::fs::write(folder.join("contract.txt"), contract).unwrap();
}
