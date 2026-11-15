//! 添字付き面集合の座標・法線・対応表・タグと共有を、元値のまま検査する。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{Building, ElementRef, GlobalId, Hash, Id, IndexedFaces};
use zumen_ifc::{Ifc, IfcFile, ProductShape, ShapeNode, TessellatedFaces};
use zumen_json::Writer;
use zumen_sdk::{Codec, Error, Lift};

const VERSIONS: [&str; 2] = ["IFC4", "IFC4X3_ADD2"];
fn source(version: &str) -> String {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('indexed faces'),'2;1');\nFILE_NAME('faces','',(),(),'','','');\nFILE_SCHEMA(('{version}'));\nENDSEC;\nDATA;\n\
#1=IFCPROJECT('0000000000000000000001',$,$,$,$,$,$,$,#2);\n\
#2=IFCUNITASSIGNMENT((#3));\n#3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n\
#8=IFCGEOMETRICREPRESENTATIONCONTEXT('Model','Model',3,0.00001,#9,$);\n\
#9=IFCAXIS2PLACEMENT3D(#11,$,$);\n#11=IFCCARTESIANPOINT((0.,0.,0.));\n\
#10=IFCCARTESIANPOINTLIST3D(((0.,-0.,0.),(10.,0.,0.),(0.,10.,0.),(0.,0.,10.),(5.,5.,5.)){tags});\n\
#20=IFCTRIANGULATEDFACESET(#10,((2.,0.,0.),(0.,3.,0.),(0.,0.,4.),(1.,1.,0.)),.F.,((1,2,3),(1,3,4)),(4,2,1,3));\n\
#21=IFCTRIANGULATEDFACESET(#10,$,$,((1,2,3)),$);\n\
#30=IFCINDEXEDPOLYGONALFACE((1,2,3,4));\n#31=IFCINDEXEDPOLYGONALFACEWITHVOIDS((1,2,3),((2,4,5)));\n\
#40=IFCPOLYGONALFACESET(#10,.T.,(#30,#31,#30),(4,1,2,3,5));\n\
#70=IFCMAPPEDITEM(#71,#73);\n#71=IFCREPRESENTATIONMAP(#9,#72);\n\
#72=IFCSHAPEREPRESENTATION(#8,'Body','Tessellation',(#20,#21,#40));\n\
#73=IFCCARTESIANTRANSFORMATIONOPERATOR3D($,$,#11,2.,$);\n#74=IFCMAPPEDITEM(#71,#73);\n\
#100=IFCBEAM('0000000000000000000002',$,$,$,$,$,#101,$,.NOTDEFINED.);\n\
#101=IFCPRODUCTDEFINITIONSHAPE($,$,(#102));\n#102=IFCSHAPEREPRESENTATION(#8,'Body','MappedRepresentation',(#70,#74));\n\
ENDSEC;\nEND-ISO-10303-21;\n",
        tags = if version == "IFC4X3_ADD2" {
            ",('origin','\\X2\\6771\\X0\\','north','up','unused')"
        } else {
            ""
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
fn value(file: &IfcFile, product_id: u64, id: u64) -> TessellatedFaces<'_> {
    let shape = product(file, product_id);
    let (result, diag, _) = with(Limits::default(), |cx| {
        file.tessellated_faces(leaf(&shape, id), cx)
    });
    assert!(diag.is_empty(), "{diag:?}");
    result.unwrap()
}

#[test]
fn preserves_raw_points_normals_indices_tags_holes_and_face_sharing() {
    for version in VERSIONS {
        let file = read(&source(version));
        let shape = product(&file, 100);
        assert_eq!(
            shape
                .nodes
                .iter()
                .filter(|n| n.source.id() == Some(20))
                .count(),
            1
        );
        let triangles = value(&file, 100, 20);
        let def = &triangles.definition;
        assert_eq!(def.points.len(), 5);
        assert_eq!(def.points[0][1].to_bits(), (-0.0f64).to_bits());
        assert_eq!(def.point_index, Some(vec![4, 2, 1, 3]));
        assert_eq!(def.closed, Some(false));
        let IndexedFaces::Triangles { indices, normals } = &def.faces else {
            panic!("三角形ではない");
        };
        assert_eq!(indices, &[[1, 2, 3], [1, 3, 4]]);
        assert_eq!(normals.as_ref().unwrap()[0], [2., 0., 0.]);
        assert_eq!(def.points[0], [0., -0., 0.]);
        assert_eq!(
            triangles.length_unit.as_ref().unwrap().prefix.as_deref(),
            Some("MILLI")
        );
        assert_eq!(triangles.source.coordinates.id(), Some(10));
        assert!(triangles.source.faces.is_empty());
        if version == "IFC4X3_ADD2" {
            assert_eq!(def.tags.as_ref().unwrap()[1], "東");
        } else {
            assert!(def.tags.is_none());
        }
        let absent = value(&file, 100, 21);
        assert_eq!(absent.definition.closed, None);
        assert!(absent.definition.point_index.is_none());
        assert!(matches!(
            absent.definition.faces,
            IndexedFaces::Triangles { normals: None, .. }
        ));
        let polygons = value(&file, 100, 40);
        let IndexedFaces::Polygons { indices, faces } = &polygons.definition.faces else {
            panic!("多角形ではない");
        };
        assert_eq!(faces.len(), 2);
        assert_eq!(indices.len(), 3);
        assert_eq!(indices[0], indices[2]);
        assert_eq!(faces[0].boundary, [1, 2, 3, 4]);
        assert!(faces[0].holes.is_empty());
        assert_eq!(faces[1].holes, [vec![2, 4, 5]]);
        assert_eq!(
            polygons
                .source
                .faces
                .iter()
                .map(|r| r.id().unwrap())
                .collect::<Vec<_>>(),
            [30, 31]
        );
        for record in polygons.source.faces.iter().chain(&polygons.source.units) {
            assert!(std::ptr::eq(
                *record,
                file.instance(record.id().unwrap()).unwrap()
            ));
        }
    }
}

fn invalid_inputs() -> Vec<(u64, &'static str, &'static str)> {
    vec![
        (20, "(4,2,1,3)", "(4,2,1,6)"),
        (20, "(4,2,1,3)", "(4,2,1,0)"),
        (20, "(4,2,1,3)", "()"),
        (20, "((1,2,3),(1,3,4))", "((1,2,3),(1,3,5))"),
        (20, "((1,2,3),(1,3,4))", "((1,2,3),(1,3,0))"),
        (20, "((1,2,3),(1,3,4))", "((1,2),(1,3,4))"),
        (20, "((1,2,3),(1,3,4))", "((1,2,3,4))"),
        (20, "((1,2,3),(1,3,4))", "()"),
        (20, "(0.,0.,4.),(1.,1.,0.)", "(0.,0.,4.)"),
        (20, "(2.,0.,0.)", "(2.,0.)"),
        (20, "(2.,0.,0.)", "(2.,1.E400,0.)"),
        (20, "),.F.,((1,2,3)", "),.U.,((1,2,3)"),
        (20, "((0.,-0.,0.),", "((0.,-0.),"),
        (
            20,
            "IFCTRIANGULATEDFACESET(#10,",
            "IFCTRIANGULATEDFACESET(#20,",
        ),
        (
            20,
            "IFCTRIANGULATEDFACESET(#10,",
            "IFCTRIANGULATEDFACESET(#999,",
        ),
        (20, ".MILLI.,.METRE.", ".MILLI.,.SECOND."),
        (40, "(1,2,3,4)", "(1,2,3,0)"),
        (40, "(1,2,3,4)", "(1,2,3,1.0)"),
        (40, "(1,2,3,4)", "(1,2,3,4294967296)"),
        (40, "((2,4,5))", "((2,4,6))"),
        (40, "((2,4,5))", "((2,4))"),
        (40, "((2,4,5))", "()"),
        (40, "(#30,#31,#30)", "()"),
    ]
}
#[test]
fn validates_coordinate_and_normal_indices_separately_without_partial_values() {
    for version in VERSIONS {
        for (id, from, to) in invalid_inputs() {
            let file = read(&source(version).replace(from, to));
            let shape = product(&file, 100);
            let (result, diag, budget) = with(Limits::default(), |cx| {
                file.tessellated_faces(leaf(&shape, id), cx)
            });
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
fn keeps_version_specific_tags_omission_and_text_diagnostics() {
    let src = source("IFC4X3_ADD2");
    let absent = read(&src.replace("('origin','\\X2\\6771\\X0\\','north','up','unused')", "$"));
    assert!(value(&absent, 100, 20).definition.tags.is_none());
    for (from, to) in [
        ("'up','unused'", "'up'"),
        ("'unused'", "12"),
        ("'unused')", "'unused'),$"),
    ] {
        let file = read(&src.replace(from, to));
        let shape = product(&file, 100);
        assert!(matches!(
            with(Limits::default(), |cx| file
                .tessellated_faces(leaf(&shape, 20), cx))
            .0,
            Err(Error::Invalid)
        ));
    }
    let file = read(&src.replace("\\X2\\6771\\X0\\", "\\X2\\D800\\X0\\"));
    let shape = product(&file, 100);
    let (result, diag, _) = with(Limits::default(), |cx| {
        file.tessellated_faces(leaf(&shape, 20), cx)
    });
    assert!(result.is_ok());
    assert!(diag.iter().any(|d| d.code() == codes::IFC_TEXT_INVALID));
    let file = read(&source("IFC4").replace("(5.,5.,5.)))", "(5.,5.,5.)),$)"));
    let shape = product(&file, 100);
    assert!(matches!(
        with(Limits::default(), |cx| file
            .tessellated_faces(leaf(&shape, 20), cx))
        .0,
        Err(Error::Invalid)
    ));
}

#[test]
fn rejects_unsupported_versions_types_complex_records_and_foreign_nodes() {
    for version in ["IFC2X3", "IFC4X2"] {
        let file = read(&source(version));
        let node = ShapeNode {
            source: file.instance(20).unwrap(),
            links: zumen_ifc::ShapeLinks::Unsupported,
        };
        assert!(matches!(
            with(Limits::default(), |cx| file.tessellated_faces(&node, cx)).0,
            Err(Error::Unsupported)
        ));
    }
    for version in VERSIONS {
        for (id, from, to) in [
            (
                21,
                "IFCTRIANGULATEDFACESET(#10,$,$,((1,2,3)),$)",
                "(IFCTRIANGULATEDFACESET(#10,$,$,((1,2,3)),$))",
            ),
            (
                20,
                "IFCTRIANGULATEDFACESET(",
                "IFCTRIANGULATEDIRREGULARNETWORK(",
            ),
            (
                40,
                "IFCINDEXEDPOLYGONALFACE((1,2,3,4))",
                "(IFCINDEXEDPOLYGONALFACE((1,2,3,4)))",
            ),
            (
                40,
                "IFCINDEXEDPOLYGONALFACE((1,2,3,4))",
                "IFCFACE((1,2,3,4))",
            ),
        ] {
            let file = read(&source(version).replace(from, to));
            let node = ShapeNode {
                source: file.instance(id).unwrap(),
                links: zumen_ifc::ShapeLinks::Unsupported,
            };
            let (result, diag, budget) =
                with(Limits::default(), |cx| file.tessellated_faces(&node, cx));
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
        let file = read(&source(version));
        let other = read(&source(version));
        let shape = product(&other, 100);
        assert!(matches!(
            with(Limits::default(), |cx| file
                .tessellated_faces(leaf(&shape, 20), cx))
            .0,
            Err(Error::Invalid)
        ));
        let missing = read(&source(version).replace("$,#2)", "$,$)"));
        let shape = product(&missing, 100);
        let (result, diag, _) = with(Limits::default(), |cx| {
            missing.tessellated_faces(leaf(&shape, 20), cx)
        });
        assert!(result.unwrap().length_unit.is_none());
        assert!(diag.iter().any(|d| d.code() == codes::IFC_UNIT_MISSING));
    }
}

#[test]
fn leaves_existing_ids_hashes_and_read_order_unchanged() {
    for version in VERSIONS {
        let src = source(version);
        let file = read(&src);
        let before = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
            .0
            .unwrap();
        let first = value(&file, 100, 40);
        value(&file, 100, 21);
        value(&file, 100, 20);
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
        let changed = read(&src.replace("#30", "#930").replace("#31", "#931"));
        assert_eq!(first.definition, value(&changed, 100, 40).definition);
        let changed = read(&src.replace("(4,1,2,3,5)", "(1,4,2,3,5)"));
        assert_ne!(first.definition, value(&changed, 100, 40).definition);
    }
}

#[test]
fn charges_shared_budgets_before_allocating_and_restores_depth() {
    for version in VERSIONS {
        let file = read(&source(version));
        let shape = product(&file, 100);
        for id in [20, 21, 40] {
            let node = leaf(&shape, id);
            let (_, _, full) = with(Limits::default(), |cx| file.tessellated_faces(node, cx));
            for kind in Kind::ALL {
                let cap = if kind == Kind::Depth {
                    2
                } else {
                    full.used(kind)
                };
                let (result, _, budget) =
                    with(Limits::default().with(kind, Limit::Finite(cap)), |cx| {
                        file.tessellated_faces(node, cx)
                    });
                assert!(result.is_ok(), "{version} {kind:?}: {result:?}");
                assert_eq!(budget.used(Kind::Depth), 0);
                if cap > 0 {
                    let (result, diag, budget) =
                        with(Limits::default().with(kind, Limit::Finite(cap - 1)), |cx| {
                            file.tessellated_faces(node, cx)
                        });
                    assert!(
                        matches!(result, Err(Error::Stopped(_))),
                        "{version} {kind:?}"
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
                file.tessellated_faces(node, &mut cx),
                Err(Error::Stopped(_))
            ));
            cx.leave();
            assert_eq!(budget.used(Kind::Depth), 0);
        }
    }
}

const PUBLIC: &str = include_str!("../../../samples/ifc/gni/model_9.ifc");
#[test]
fn public_wall_reaches_fourteen_faces_and_holes_from_its_global_id() {
    let file = read(PUBLIC);
    let building = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
        .0
        .unwrap();
    let at = building
        .find(Id::Global(
            GlobalId::parse("2q2CBrfQTCKPY0zxCdZwK$").unwrap(),
        ))
        .unwrap();
    assert_eq!(building.element(at).unwrap().source().instance, Some(628));
    let result = value(&file, 628, 623);
    assert_eq!(result.definition.points.len(), 28);
    assert!(result.definition.point_index.is_none());
    let IndexedFaces::Polygons { indices, faces } = &result.definition.faces else {
        panic!("多角形ではない");
    };
    assert_eq!(indices.len(), 14);
    assert_eq!(faces.len(), 14);
    assert_eq!(faces[0].holes.len(), 2);
    assert_eq!(result.source.faces[0].id(), Some(608));
    assert_eq!(result.source.coordinates.id(), Some(622));
}

#[test]
fn survives_mutated_and_truncated_inputs_with_one_budget() {
    let run = |bytes: &[u8], cx: &mut Cx| {
        let file = Ifc::read(bytes, cx)?;
        let shape = file.product_shape(100, cx)?;
        for node in &shape.nodes {
            cx.check(Kind::Work, 1)?;
            if [b"IFCTRIANGULATEDFACESET".as_slice(), b"IFCPOLYGONALFACESET"]
                .iter()
                .any(|k| file.kind(node.source).eq_ignore_ascii_case(k))
            {
                file.tessellated_faces(node, cx)?;
            }
        }
        Ok(())
    };
    for version in VERSIONS {
        let bytes = source(version).into_bytes();
        for end in (0..bytes.len()).step_by(41) {
            zumen_sdk::testkit::survives_with(&bytes[..end], run).unwrap();
        }
        for at in (0..bytes.len()).step_by(37) {
            for byte in [0, b'$', b'#', 255] {
                let mut changed = bytes.clone();
                changed[at] = byte;
                zumen_sdk::testkit::survives_with(&changed, run).unwrap();
            }
        }
    }
}

fn ids(out: &mut Writer, values: impl Iterator<Item = u32>) {
    out.begin_array();
    for value in values {
        out.uint(value.into());
    }
    out.end_array();
}
fn vectors(out: &mut Writer, values: &[[f64; 3]]) {
    out.begin_array();
    for row in values {
        out.begin_array();
        for &value in row {
            out.float(value);
        }
        out.end_array();
    }
    out.end_array();
}
fn json_item(out: &mut Writer, value: &TessellatedFaces<'_>) {
    let def = &value.definition;
    out.begin_object();
    out.key("instance");
    out.uint(value.source.item.id().unwrap());
    out.key("coordinates");
    out.uint(value.source.coordinates.id().unwrap());
    out.key("points");
    vectors(out, &def.points);
    out.key("tags");
    if let Some(tags) = &def.tags {
        out.begin_array();
        for tag in tags {
            out.string(tag);
        }
        out.end_array();
    } else {
        out.null();
    }
    out.key("closed");
    if let Some(closed) = def.closed {
        out.bool(closed);
    } else {
        out.null();
    }
    out.key("point_index");
    if let Some(index) = &def.point_index {
        ids(out, index.iter().copied());
    } else {
        out.null();
    }
    match &def.faces {
        IndexedFaces::Triangles { indices, normals } => {
            out.key("kind");
            out.string("triangles");
            out.key("indices");
            out.begin_array();
            for row in indices {
                ids(out, row.iter().copied());
            }
            out.end_array();
            out.key("normals");
            if let Some(normals) = normals {
                vectors(out, normals);
            } else {
                out.null();
            }
        }
        IndexedFaces::Polygons { indices, faces } => {
            out.key("kind");
            out.string("polygons");
            out.key("indices");
            out.begin_array();
            for index in indices {
                out.uint(value.source.faces[index.0 as usize].id().unwrap());
            }
            out.end_array();
            out.key("faces");
            out.begin_array();
            for (face, source) in faces.iter().zip(&value.source.faces) {
                out.begin_object();
                out.key("instance");
                out.uint(source.id().unwrap());
                out.key("boundary");
                ids(out, face.boundary.iter().copied());
                out.key("holes");
                out.begin_array();
                for hole in &face.holes {
                    ids(out, hole.iter().copied());
                }
                out.end_array();
                out.end_object();
            }
            out.end_array();
        }
    }
    let unit = value.length_unit.as_ref().unwrap();
    out.key("length_unit");
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
#[allow(clippy::disallowed_methods)] // 自作入力と公開例の取得結果だけを指定先へ出す。
fn exports_raw_attributes_diagnostics_and_budget_limits() {
    let Ok(folder) = std::env::var("ZUMEN_IFC_INDEXED_FACES_OUT") else {
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
                let (result, diag, budget) =
                    with(Limits::default(), |cx| file.tessellated_faces(node, cx));
                json_item(&mut out, &result.unwrap());
                contract += &format!(
                    "{name}:{id}:ok:{diag:?}:{:?}\n",
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
                                file.tessellated_faces(node, cx)
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
            &[20, 21, 40],
        );
        std::fs::write(folder.join(format!("{version}.ifc")), src).unwrap();
    }
    export(
        &read(PUBLIC),
        "../../samples/ifc/gni/model_9.ifc",
        "public_wall",
        628,
        "2q2CBrfQTCKPY0zxCdZwK$",
        &[623],
    );
    for version in VERSIONS {
        for (id, from, to) in invalid_inputs() {
            let file = read(&source(version).replace(from, to));
            let shape = product(&file, 100);
            let (result, diag, budget) = with(Limits::default(), |cx| {
                file.tessellated_faces(leaf(&shape, id), cx)
            });
            contract += &format!(
                "{version}:{to}:{:?}:{diag:?}:{:?}\n",
                result.err(),
                Kind::ALL.map(|k| budget.used(k))
            );
        }
    }
    std::fs::write(folder.join("contract.txt"), contract).unwrap();
}
