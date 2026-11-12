//! 要素の形状表現を L1 の参照で取得する。入力は合成と再配布できる公開サンプルだけ。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{Building, Coordinates, CurveKind, Hash};
use zumen_ifc::{Ifc, IfcFile, ProductShape, Record, ShapeLinks, ShapeRef, Value};
use zumen_json::Writer;
use zumen_sdk::{Codec, Error, Lift};

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];
fn source(schema: &str) -> String {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('shapes'),'2;1');\nFILE_NAME('shapes','',(),(),'','','');\nFILE_SCHEMA(('{schema}'));\nENDSEC;\nDATA;\n\
    #1=IFCCARTESIANPOINT((0.,0.,0.));\n\
    #2=IFCLOCALPLACEMENT($,#3);\n\
    #3=IFCAXIS2PLACEMENT3D(#1,$,$);\n\
    #4=IFCDIRECTION((2.,0.,0.));\n\
    #5=IFCDIRECTION((0.,3.,0.));\n\
    #6=IFCCARTESIANPOINT((10.,20.,30.));\n\
    #7=IFCCARTESIANTRANSFORMATIONOPERATOR3DNONUNIFORM(#4,#5,#6,2.,$,3.,4.);\n\
    #8=IFCGEOMETRICREPRESENTATIONCONTEXT('Model','Model',3,0.00001,#3,$);\n\
    #10=IFCCARTESIANPOINT((0.,0.));\n\
    #11=IFCCARTESIANPOINT((5.,0.));\n\
    #12=IFCPOLYLINE((#10,#11));\n\
    #20=IFCSHAPEREPRESENTATION(#8,'Axis','Curve2D',(#12));\n\
    #21=IFCREPRESENTATIONMAP(#3,#20);\n\
    #22=IFCMAPPEDITEM(#21,#7);\n\
    #23=IFCMAPPEDITEM(#21,#7);\n\
    #24=IFCGEOMETRICCURVESET((#12));\n\
    #25=IFCSHAPEREPRESENTATION(#8,'FootPrint','GeometricCurveSet',(#24));\n\
    #100=IFCWALL('0000000000000000000001',$,$,$,$,#2,#101,${wall_type});\n\
    #101=IFCPRODUCTDEFINITIONSHAPE($,$,(#102,#25));\n\
    #102=IFCSHAPEREPRESENTATION(#8,'Body','MappedRepresentation',(#22,#23));\n\
    ENDSEC;\nEND-ISO-10303-21;\n",
        wall_type = if schema == "IFC2X3" {
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
fn graph(file: &IfcFile, id: u64) -> ProductShape<'_> {
    let (value, diag, _) = with(Limits::default(), |cx| file.product_shape(id, cx));
    assert!(diag.is_empty(), "{diag:?}");
    value.unwrap()
}
fn node<'a>(shape: &'a ProductShape<'a>, id: u64) -> &'a zumen_ifc::ShapeNode<'a> {
    shape
        .nodes
        .iter()
        .find(|node| node.source.id() == Some(id))
        .unwrap()
}

#[test]
fn follows_shared_maps_and_passes_local_curves_to_the_existing_getter_in_three_versions() {
    for schema in VERSIONS {
        let file = read(&source(schema));
        let shape = graph(&file, 100);
        assert!(std::ptr::eq(shape.product, file.instance(100).unwrap()));
        assert!(std::ptr::eq(
            shape.placement.unwrap(),
            file.instance(2).unwrap()
        ));
        let ShapeLinks::Definition { representations } =
            &shape.node(shape.definition.unwrap()).unwrap().links
        else {
            panic!()
        };
        assert_eq!(
            representations
                .iter()
                .map(|&r| shape.node(r).unwrap().source.id().unwrap())
                .collect::<Vec<_>>(),
            [102, 25]
        );
        let ShapeLinks::Mapped {
            source: first,
            target,
        } = node(&shape, 22).links
        else {
            panic!()
        };
        let ShapeLinks::Mapped { source: second, .. } = node(&shape, 23).links else {
            panic!()
        };
        assert_eq!(first, second);
        assert!(std::ptr::eq(target, file.instance(7).unwrap()));
        assert_eq!(target.parts()[0].args().len(), 7); // 元の非一様尺度も省略した軸も残る
        let ShapeLinks::Map {
            origin,
            representation,
        } = shape.node(first).unwrap().links
        else {
            panic!()
        };
        assert_eq!(origin.id(), Some(3));
        assert_eq!(shape.node(representation).unwrap().source.id(), Some(20));
        assert_eq!(
            shape
                .nodes
                .iter()
                .filter(|n| n.source.id() == Some(12))
                .count(),
            1
        );
        assert_eq!(
            shape
                .nodes
                .iter()
                .filter(|n| n.source.id() == Some(21))
                .count(),
            1
        );
        let curve = node(&shape, 12);
        assert!(matches!(curve.links, ShapeLinks::Curve));
        let (curve, diag, _) = with(Limits::default(), |cx| {
            file.curve(curve.source.id().unwrap(), cx)
        });
        assert!(diag.is_empty());
        assert_eq!(
            curve.unwrap().items[0].kind,
            CurveKind::Polyline(vec![Coordinates::Two([0., 0.]), Coordinates::Two([5., 0.])])
        );
        assert!(shape.node(ShapeRef(u32::MAX)).is_none());
    }
}

#[test]
fn diagnoses_unknown_items_and_omission_and_does_not_silently_validate_curves() {
    assert!(graph(&read(&source("ifc4")), 100).definition.is_some());
    let unsupported = read(&source("IFC4X1"));
    let (result, diag, _) = with(Limits::default(), |cx| unsupported.product_shape(100, cx));
    assert!(matches!(result, Err(Error::Unsupported)));
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::IFC_VERSION_UNSUPPORTED)
    );
    assert!(matches!(
        with(
            Limits::default().with(Kind::DecodedBytes, Limit::Finite(0)),
            |cx| unsupported.product_shape(100, cx)
        )
        .0,
        Err(Error::Stopped(_))
    ));
    for schema in VERSIONS {
        let src = source(schema);
        let file = read(&src.replace("IFCPOLYLINE((#10,#11))", "IFCEXTRUDEDAREASOLID($,$,$,$)"));
        let (result, diag, _) = with(Limits::default(), |cx| file.product_shape(100, cx));
        assert!(matches!(
            node(&result.unwrap(), 12).links,
            ShapeLinks::Unsupported
        ));
        assert_eq!(
            diag.iter()
                .filter(|e| e.code() == codes::IFC_SHAPE_UNSUPPORTED)
                .count(),
            1
        );
        let file = read(&src.replace("$,#2,#101,$", "$,$,$,$"));
        let (result, diag, _) = with(Limits::default(), |cx| file.product_shape(100, cx));
        let result = result.unwrap();
        assert!(
            result.placement.is_none() && result.definition.is_none() && result.nodes.is_empty()
        );
        assert_eq!(
            diag.iter()
                .filter(|e| e.code() == codes::IFC_SHAPE_MISSING)
                .count(),
            2
        );
        let file = read(&src.replace("'Axis','Curve2D'", "$,$"));
        let (result, diag, _) = with(Limits::default(), |cx| file.product_shape(100, cx));
        assert!(result.is_ok());
        assert_eq!(
            diag.iter()
                .filter(|e| e.code() == codes::IFC_SHAPE_MISSING)
                .count(),
            2
        );
        let file = read(&src.replace("IFCPOLYLINE((#10,#11))", "IFCPOLYLINE((#10,#999))"));
        let shape = graph(&file, 100); // 葉の曲線の値はまだ読まない
        let id = node(&shape, 12).source.id().unwrap();
        assert!(matches!(
            with(Limits::default(), |cx| file.curve(id, cx)).0,
            Err(Error::Invalid)
        ));
    }
}

#[test]
fn rejects_bad_structures_references_and_cycles_without_partial_views() {
    for schema in VERSIONS {
        let src = source(schema);
        for (from, to) in [
            ("$,#2,#101,$", "$,#12,#101,$"),
            ("IFCMAPPEDITEM(#21,#7)", "IFCMAPPEDITEM(#999,#7)"),
            ("IFCMAPPEDITEM(#21,#7)", "IFCMAPPEDITEM(#12,#7)"),
            ("IFCMAPPEDITEM(#21,#7)", "IFCMAPPEDITEM(#21,#10)"),
            (
                "IFCREPRESENTATIONMAP(#3,#20)",
                "IFCREPRESENTATIONMAP(#10,#20)",
            ),
            ("'Axis','Curve2D',(#12)", "'Axis','Curve2D',(#22)"),
            ("IFCGEOMETRICCURVESET((#12))", "IFCGEOMETRICCURVESET((#24))"),
            ("#8,'Axis'", "#10,'Axis'"),
            ("(#102,#25)", "()"),
            ("(#102,#25)", "$"),
            (
                "IFCGEOMETRICCURVESET((#12))",
                "IFCGEOMETRICCURVESET((#12,$))",
            ),
            ("'Axis','Curve2D'", "1.,'Curve2D'"),
        ] {
            let file = read(&src.replace(from, to));
            let (result, diag, budget) = with(Limits::default(), |cx| file.product_shape(100, cx));
            assert!(
                matches!(result, Err(Error::Invalid)),
                "{schema} {from} -> {to}"
            );
            assert!(diag.iter().any(|e| e.code() == codes::IFC_SHAPE_INVALID));
            assert_eq!(budget.used(Kind::Depth), 0);
        }
        assert!(matches!(
            with(Limits::default(), |cx| read(&src)
                .product_shape(999, cx)
                .map(drop))
            .0,
            Err(Error::Invalid)
        ));
        assert!(matches!(
            with(Limits::default(), |cx| read(&src)
                .product_shape(12, cx)
                .map(drop))
            .0,
            Err(Error::Invalid)
        ));
    }
}

#[test]
fn reserves_before_allocation_and_restores_depth_on_budget_and_cancel() {
    let file = read(&source("IFC4"));
    for kind in [Kind::Work, Kind::Depth, Kind::DecodedBytes] {
        let (result, _, budget) = with(Limits::default().with(kind, Limit::Finite(0)), |cx| {
            file.product_shape(100, cx)
        });
        assert!(matches!(result, Err(Error::Stopped(_))));
        assert_eq!(budget.used(Kind::Depth), 0);
        if kind == Kind::DecodedBytes {
            assert_eq!(budget.used(kind), 0);
        }
    }
    let (result, _, budget) = with(Limits::default(), |cx| {
        cx.cancel.cancel();
        file.product_shape(100, cx)
    });
    assert!(matches!(
        result,
        Err(Error::Stopped(zumen_base::Stop::Cancelled))
    ));
    assert_eq!(budget.used(Kind::DecodedBytes), 0);
    let (_, _, full) = with(Limits::default(), |cx| file.product_shape(100, cx));
    for kind in [Kind::Work, Kind::DecodedBytes] {
        let used = full.used(kind);
        for limit in [used - 1, used] {
            let (result, _, budget) =
                with(Limits::default().with(kind, Limit::Finite(limit)), |cx| {
                    file.product_shape(100, cx)
                });
            assert_eq!(result.is_ok(), limit == used);
            assert_eq!(budget.used(Kind::Depth), 0);
        }
    }
}

#[test]
fn mutations_finish_with_repeatable_results_diagnostics_and_budget_usage() {
    let src = source("IFC4");
    for at in (0..src.len()).step_by(29) {
        let mut input = src.as_bytes().to_vec();
        input[at] = b'$';
        zumen_sdk::testkit::survives_with(&input, |src, cx| {
            let file = Ifc::read(src, cx)?;
            if let Err(Error::Stopped(stop)) = file.product_shape(100, cx) {
                return Err(Error::Stopped(stop));
            }
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn getter_does_not_change_existing_building_ids_or_hashes_and_keeps_source_numbers_separate() {
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
    let shape = graph(&file, 100);
    let curve = with(Limits::default(), |cx| file.curve(12, cx)).0.unwrap();
    let _again = graph(&file, 100);
    let after = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
        .0
        .unwrap();
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
    let renamed = read(&src.replace("#21", "#921"));
    let renamed = graph(&renamed, 100);
    let ShapeLinks::Mapped { source: a, .. } = node(&shape, 22).links else {
        panic!()
    };
    let ShapeLinks::Mapped { source: b, .. } = node(&renamed, 22).links else {
        panic!()
    };
    assert_eq!(a, b);
    assert_eq!(shape.node(a).unwrap().source.id(), Some(21));
    assert_eq!(renamed.node(b).unwrap().source.id(), Some(921));
    let file = read(&src.replace("(5.,0.)", "(6.,0.)"));
    assert_ne!(
        curve.items[0].kind,
        with(Limits::default(), |cx| file.curve(12, cx))
            .0
            .unwrap()
            .items[0]
            .kind
    );
}

#[test]
fn follows_public_column_mapping_and_then_l1_profile_references_to_the_curve() {
    let file = read(include_str!("../../../samples/ifc/gni/model_9.ifc"));
    let (shape, diag, _) = with(Limits::default(), |cx| file.product_shape(131, cx));
    let shape = shape.unwrap();
    assert_eq!(shape.product.id(), Some(131));
    assert_eq!(shape.placement.unwrap().id(), Some(130));
    let Value::Text { raw, .. } = &shape.product.parts()[0].args()[0] else {
        panic!()
    };
    assert_eq!(file.bytes(raw.clone()), b"2q2CBrfQTCKPY0zxCdZygb");
    assert_eq!(
        shape.node(shape.definition.unwrap()).unwrap().source.id(),
        Some(127)
    );
    let ShapeLinks::Mapped { source, target } = node(&shape, 125).links else {
        panic!()
    };
    assert_eq!(target.id(), Some(124));
    assert_eq!(shape.node(source).unwrap().source.id(), Some(121));
    let ShapeLinks::Map {
        origin,
        representation,
    } = node(&shape, 121).links
    else {
        panic!()
    };
    assert_eq!(origin.id(), Some(120));
    assert_eq!(shape.node(representation).unwrap().source.id(), Some(119));
    let leaf = node(&shape, 114);
    assert!(matches!(leaf.links, ShapeLinks::Unsupported));
    assert!(
        diag.iter()
            .any(|e| e.code() == codes::IFC_SHAPE_UNSUPPORTED)
    );
    // 立体・断面の型は未実装。ここからは既存 L1 の属性をたどる利用例
    let Value::Ref(profile) = leaf.source.parts()[0].args()[0] else {
        panic!()
    };
    assert_eq!(profile, 112);
    let Value::Ref(curve) = file.instance(profile).unwrap().parts()[0].args()[2] else {
        panic!()
    };
    assert_eq!(curve, 111);
    let curve = with(Limits::default(), |cx| file.curve(curve, cx))
        .0
        .unwrap();
    assert!(matches!(&curve.items[0].kind,CurveKind::Indexed{points,..} if points.len()==5));
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn deep_shared_maps_are_flat_during_read_clone_and_drop() {
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(|| {
            let mut src = source("IFC4").replace(
                "#21=IFCREPRESENTATIONMAP(#3,#20)",
                "#21=IFCREPRESENTATIONMAP(#3,#1000)",
            );
            let mut extra = String::new();
            for id in 1000..2025 {
                let item = if id == 2024 { 12 } else { id + 10000 };
                extra.push_str(&format!(
                    "#{id}=IFCSHAPEREPRESENTATION(#8,'Axis','Curve2D',(#{item}));\n"
                ));
                if id != 2024 {
                    extra.push_str(&format!(
                        "#{}=IFCMAPPEDITEM(#{},#7);\n#{}=IFCREPRESENTATIONMAP(#3,#{});\n",
                        id + 10000,
                        id + 20000,
                        id + 20000,
                        id + 1
                    ));
                }
            }
            src = src.replace("ENDSEC;\nEND-ISO", &(extra + "ENDSEC;\nEND-ISO"));
            let file = read(&src);
            let (shape, _, budget) = with(
                Limits::default().with(Kind::Depth, Limit::Unlimited),
                |cx| file.product_shape(100, cx),
            );
            let shape = shape.unwrap();
            assert!(shape.nodes.len() > 3000);
            drop(shape.clone());
            drop(shape);
            assert_eq!(budget.used(Kind::Depth), 0);
            assert!(matches!(
                with(Limits::default(), |cx| file.product_shape(100, cx)).0,
                Err(Error::Stopped(_))
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}

// 以下は外部のリーダーと照らすための出力。合成と公開の既知の属性だけを出す。
fn reference_json(out: &mut Writer, record: Option<&Record>) {
    if let Some(id) = record.and_then(Record::id) {
        out.uint(id);
    } else {
        out.null();
    }
}
fn refs_json(out: &mut Writer, shape: &ProductShape<'_>, refs: &[ShapeRef]) {
    out.begin_array();
    for &at in refs {
        reference_json(out, Some(shape.node(at).unwrap().source));
    }
    out.end_array();
}
fn text_json(out: &mut Writer, text: Option<&str>) {
    if let Some(text) = text {
        out.string(text);
    } else {
        out.null();
    }
}
fn value_json(out: &mut Writer, file: &IfcFile, value: &Value) {
    match value {
        Value::Null | Value::Derived => out.null(),
        Value::Ref(id) => {
            out.begin_object();
            out.key("ref");
            out.uint(*id);
            out.end_object();
        }
        Value::Number(raw) => out.float(
            std::str::from_utf8(file.bytes(raw.clone()))
                .unwrap()
                .parse()
                .unwrap(),
        ),
        Value::Enum(raw) => out.string(std::str::from_utf8(file.bytes(raw.clone())).unwrap()),
        Value::Text { raw, .. } => out
            .string(&zumen_step::decode(file.bytes(raw.clone()), |rest| {
                (String::from_utf8_lossy(&rest[..1]).into_owned(), 1)
            })),
        Value::List(values) => {
            out.begin_array();
            for v in values {
                value_json(out, file, v);
            }
            out.end_array();
        }
        _ => panic!("照合対象の補助レコードに予期しない属性"),
    }
}
fn write_shape(file: &IfcFile, id: u64, curve_ids: &[u64], name: &str) -> Vec<u8> {
    let (shape, diag, budget) = with(Limits::default(), |cx| file.product_shape(id, cx));
    let shape = shape.unwrap();
    let mut out = Writer::new();
    out.begin_object();
    out.key("file");
    out.string(name);
    out.key("product");
    out.uint(id);
    out.key("placement");
    reference_json(&mut out, shape.placement);
    out.key("definition");
    reference_json(
        &mut out,
        shape.definition.map(|r| shape.node(r).unwrap().source),
    );
    out.key("budgets");
    out.begin_array();
    for kind in Kind::ALL {
        out.uint(budget.used(kind));
    }
    out.end_array();
    out.key("diagnostics");
    out.begin_array();
    for d in diag.iter() {
        out.string(&format!("{d:?}"));
    }
    out.end_array();
    let mut aux = std::collections::BTreeSet::new();
    if let Some(r) = shape.placement {
        aux.insert(r.id().unwrap());
    }
    out.key("nodes");
    out.begin_array();
    for node in &shape.nodes {
        out.begin_object();
        out.key("instance");
        out.uint(node.source.id().unwrap());
        out.key("kind");
        out.string(std::str::from_utf8(file.kind(node.source)).unwrap());
        match &node.links {
            ShapeLinks::Definition { representations } => {
                out.key("representations");
                refs_json(&mut out, &shape, representations);
            }
            ShapeLinks::Representation {
                context,
                identifier,
                representation_type,
                items,
            } => {
                out.key("context");
                reference_json(&mut out, Some(context));
                aux.insert(context.id().unwrap());
                out.key("identifier");
                text_json(&mut out, identifier.as_deref());
                out.key("representation_type");
                text_json(&mut out, representation_type.as_deref());
                out.key("items");
                refs_json(&mut out, &shape, items);
            }
            ShapeLinks::Mapped { source, target } => {
                out.key("source");
                reference_json(&mut out, Some(shape.node(*source).unwrap().source));
                out.key("target");
                reference_json(&mut out, Some(target));
                aux.insert(target.id().unwrap());
            }
            ShapeLinks::Map {
                origin,
                representation,
            } => {
                out.key("origin");
                reference_json(&mut out, Some(origin));
                aux.insert(origin.id().unwrap());
                out.key("representation");
                reference_json(&mut out, Some(shape.node(*representation).unwrap().source));
            }
            ShapeLinks::Set { elements } => {
                out.key("elements");
                refs_json(&mut out, &shape, elements);
            }
            ShapeLinks::Curve | ShapeLinks::Unsupported => {}
        }
        out.end_object();
    }
    out.end_array();
    out.key("records");
    out.begin_array();
    // 元の配置・文脈・写像の値と参照をたどり、借用した record が保つ値を独立に比較する
    let mut pending: Vec<_> = aux.iter().copied().collect();
    let mut written = std::collections::BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !written.insert(id) {
            continue;
        }
        let r = file.instance(id).unwrap();
        out.begin_object();
        out.key("instance");
        out.uint(id);
        out.key("kind");
        out.string(std::str::from_utf8(file.kind(r)).unwrap());
        out.key("args");
        out.begin_array();
        for v in r.parts()[0].args() {
            value_json(&mut out, file, v);
        }
        out.end_array();
        out.end_object();
        pending.extend(file.references(r));
    }
    out.end_array();
    out.key("curves");
    out.begin_array();
    for &id in curve_ids {
        let (curve, _, budget) = with(Limits::default(), |cx| file.curve(id, cx));
        let curve = curve.unwrap();
        out.begin_object();
        out.key("instance");
        out.uint(id);
        out.key("budgets");
        out.begin_array();
        for k in Kind::ALL {
            out.uint(budget.used(k));
        }
        out.end_array();
        out.key("points");
        out.begin_array();
        let points = match &curve.items[0].kind {
            CurveKind::Polyline(p) | CurveKind::Indexed { points: p, .. } => p,
            _ => panic!(),
        };
        for p in points {
            out.begin_array();
            match p {
                Coordinates::Two(v) => {
                    for &n in v {
                        out.float(n)
                    }
                }
                Coordinates::Three(v) => {
                    for &n in v {
                        out.float(n)
                    }
                }
            }
            out.end_array();
        }
        out.end_array();
        out.end_object();
    }
    out.end_array();
    out.end_object();
    out.into_bytes()
}

#[test]
#[allow(clippy::disallowed_methods)] // 外部照合の出力先だけを受け取る
fn writes_product_shapes_for_other_readers() {
    let Some(folder) = std::env::var_os("ZUMEN_IFC_SHAPES_OUT") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    for schema in VERSIONS {
        let src = source(schema);
        let name = format!("{schema}.ifc");
        let file = read(&src);
        std::fs::write(folder.join(&name), src).unwrap();
        std::fs::write(
            folder.join(format!("{schema}.json")),
            write_shape(&file, 100, &[12], &name),
        )
        .unwrap();
    }
    let src = include_str!("../../../samples/ifc/gni/model_9.ifc");
    let file = read(src);
    std::fs::write(folder.join("public.ifc"), src).unwrap();
    std::fs::write(
        folder.join("public.json"),
        write_shape(&file, 131, &[111], "public.ifc"),
    )
    .unwrap();
}
