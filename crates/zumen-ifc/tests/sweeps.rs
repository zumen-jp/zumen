//! 代表要素から掃引円盤の元属性を取得する。入力は自作だけ。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{Building, Coordinates, CurveKind, ElementRef, Hash};
use zumen_ifc::{Ifc, IfcFile, ProductShape, ShapeNode, SweptDisk};
use zumen_json::Writer;
use zumen_sdk::{Codec, Error, Lift};

#[path = "support/curve_json.rs"]
mod curve_json;

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];

fn source(version: &str) -> String {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('sweeps'),'2;1');\nFILE_NAME('sweeps','',(),(),'','','');\nFILE_SCHEMA(('{version}'));\nENDSEC;\nDATA;\n\
#1=IFCPROJECT('0000000000000000000001',$,$,$,$,$,$,$,#2);\n\
#2=IFCUNITASSIGNMENT((#3,#4));\n#3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n\
#4=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);\n\
#8=IFCGEOMETRICREPRESENTATIONCONTEXT('Model','Model',3,0.00001,#20,$);\n\
#10=IFCCARTESIANPOINT((1.,2.,3.));\n#11=IFCCARTESIANPOINT((4.,6.,8.));\n\
#12=IFCPOLYLINE((#10,#11));\n#13=IFCDIRECTION((0.,0.,2.));\n#14=IFCVECTOR(#13,10.);\n\
#15=IFCLINE(#10,#14);\n#16=IFCCIRCLE(#20,30.);\n\
#20=IFCAXIS2PLACEMENT3D(#10,$,$);\n\
#30=IFCSWEPTDISKSOLID(#12,2.5,1.25,0.,1.);\n\
#31=IFCSWEPTDISKSOLID(#15,3.,$,0.25,5.);\n\
#32=IFCSWEPTDISKSOLID(#16,1.,$,{parameters});\n\
#40=IFCMAPPEDITEM(#41,#43);\n#41=IFCREPRESENTATIONMAP(#20,#42);\n\
#42=IFCSHAPEREPRESENTATION(#8,'Body','AdvancedSweptSolid',(#30,#31,#32));\n\
#43=IFCCARTESIANTRANSFORMATIONOPERATOR3D($,$,#10,2.,$);\n\
#44=IFCMAPPEDITEM(#41,#43);\n\
#100=IFCWALL('0000000000000000000002',$,'sweep',$,$,$,#101,${extra});\n\
#101=IFCPRODUCTDEFINITIONSHAPE($,$,(#102));\n\
#102=IFCSHAPEREPRESENTATION(#8,'Body','MappedRepresentation',(#40,#44));\n\
ENDSEC;\nEND-ISO-10303-21;\n",
        parameters = if version == "IFC2X3" { "0.,6." } else { "$,$" },
        extra = if version == "IFC2X3" {
            ""
        } else {
            ",.NOTDEFINED."
        },
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

fn product(file: &IfcFile) -> ProductShape<'_> {
    with(Limits::default(), |cx| file.product_shape(100, cx))
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

fn sweep(file: &IfcFile, id: u64) -> SweptDisk<'_> {
    let shape = product(file);
    let (result, diag, _) = with(Limits::default(), |cx| {
        file.swept_disk(leaf(&shape, id), cx)
    });
    assert!(diag.is_empty(), "{diag:?}");
    result.unwrap()
}

#[test]
fn gets_disk_and_hollow_sweeps_through_shared_product_shapes() {
    for version in VERSIONS {
        let file = read(&source(version));
        let shape = product(&file);
        assert_eq!(
            shape
                .nodes
                .iter()
                .filter(|n| n.source.id() == Some(30))
                .count(),
            1
        );
        let value = sweep(&file, 30);
        assert_eq!(value.definition.radius, 2.5);
        assert_eq!(value.definition.inner_radius, Some(1.25));
        assert_eq!(value.definition.start_parameter, Some(0.));
        assert_eq!(value.definition.end_parameter, Some(1.));
        assert_eq!(value.definition.directrix, value.directrix.root);
        assert!(std::ptr::eq(value.source, file.instance(30).unwrap()));
        assert_eq!(
            value
                .directrix
                .length_unit
                .as_ref()
                .unwrap()
                .prefix
                .as_deref(),
            Some("MILLI")
        );
        let path = value.directrix.item(value.definition.directrix).unwrap();
        assert_eq!(path.source.instance, Some(12));
        assert_eq!(path.source.records, vec![10, 11]);
        assert_eq!(
            path.kind,
            CurveKind::Polyline(vec![
                Coordinates::Three([1., 2., 3.]),
                Coordinates::Three([4., 6., 8.])
            ])
        );
        let line = sweep(&file, 31);
        assert_eq!(line.definition.inner_radius, None);
        assert_eq!(line.definition.start_parameter, Some(0.25));
        assert_eq!(line.definition.end_parameter, Some(5.));
        assert!(matches!(
            line.directrix.items[0].kind,
            CurveKind::Line { magnitude: 10., .. }
        ));
        let circle = sweep(&file, 32);
        assert_eq!(
            circle.definition.start_parameter,
            if version == "IFC2X3" { Some(0.) } else { None }
        );
        assert_eq!(circle.directrix.angle_unit.as_ref().unwrap().name, "RADIAN");
    }
}

#[test]
fn enforces_parameter_presence_without_evaluating_or_reordering_parameters() {
    for version in VERSIONS {
        for (from, to, valid) in [
            (
                "#12,2.5,1.25,0.,1.",
                "#12,2.5,1.25,$,$",
                version != "IFC2X3",
            ),
            (
                "#12,2.5,1.25,0.,1.",
                "#12,2.5,1.25,$,1.",
                version != "IFC2X3",
            ),
            (
                "#12,2.5,1.25,0.,1.",
                "#12,2.5,1.25,0.,$",
                version != "IFC2X3",
            ),
            ("#12,2.5,1.25,0.,1.", "#15,2.5,1.25,$,1.", false),
            ("#12,2.5,1.25,0.,1.", "#15,2.5,1.25,0.,$", false),
            ("#12,2.5,1.25,0.,1.", "#12,2.5,1.25,4.,-1.", true),
        ] {
            let file = read(&source(version).replace(from, to));
            let shape = product(&file);
            let (result, _, budget) = with(Limits::default(), |cx| {
                file.swept_disk(leaf(&shape, 30), cx)
            });
            assert_eq!(result.is_ok(), valid, "{version} {to}: {result:?}");
            assert_eq!(budget.used(Kind::Depth), 0);
        }
    }
}

fn invalid_inputs() -> Vec<(&'static str, &'static str)> {
    vec![
        ("#12,2.5,1.25,0.,1.", "#999,2.5,1.25,0.,1."),
        ("#12,2.5,1.25,0.,1.", "$ ,2.5,1.25,0.,1."),
        ("#12,2.5,1.25,0.,1.", "#12,0.,$,0.,1."),
        ("#12,2.5,1.25,0.,1.", "#12,-1.,$,0.,1."),
        ("#12,2.5,1.25,0.,1.", "#12,2.5,0.,0.,1."),
        ("#12,2.5,1.25,0.,1.", "#12,2.5,2.5,0.,1."),
        ("#12,2.5,1.25,0.,1.", "#12,2.5,3.,0.,1."),
        ("#12,2.5,1.25,0.,1.", "#12,2.5,-1.,0.,1."),
        ("#12,2.5,1.25,0.,1.", "#12,1.E999,$,0.,1."),
        ("#12,2.5,1.25,0.,1.", "#12,2.5,1.E999,0.,1."),
        ("#12,2.5,1.25,0.,1.", "#12,2.5,$,1.E999,1."),
        ("#12,2.5,1.25,0.,1.", "#12,2.5,$,0.,'bad'"),
        ("#12,2.5,1.25,0.,1.", "#12,2.5,$,0.,*"),
        ("#12,2.5,1.25,0.,1.", "#12,2.5,$,0."),
        ("#12,2.5,1.25,0.,1.", "#12,2.5,$,0.,1.,0."),
        ("(1.,2.,3.)", "(1.,2.)"),
    ]
}

#[test]
fn rejects_invalid_values_with_sources_and_no_partial_result() {
    for version in VERSIONS {
        for (from, to) in invalid_inputs() {
            let file = read(&source(version).replace(from, to));
            let shape = product(&file);
            let (result, diagnostics, budget) = with(Limits::default(), |cx| {
                file.swept_disk(leaf(&shape, 30), cx)
            });
            assert!(
                matches!(result, Err(Error::Invalid)),
                "{version} {to}: {result:?}"
            );
            assert_eq!(budget.used(Kind::Depth), 0);
            assert!(!diagnostics.is_empty());
            let text = format!("{diagnostics:?}");
            assert!(text.contains("bytes: Some"), "{text}");
        }
        let file = read(
            &source(version)
                .replace("(4.,6.,8.)", "(4.,6.)")
                .replace("(1.,2.,3.)", "(1.,2.)"),
        );
        let shape = product(&file);
        let (result, diagnostics, _) = with(Limits::default(), |cx| {
            file.swept_disk(leaf(&shape, 30), cx)
        });
        assert!(matches!(result, Err(Error::Invalid)));
        assert!(format!("{diagnostics:?}").contains("sweep-dimension"));
    }
}

#[test]
fn preserves_curve_sharing_and_rejects_cycles_unknown_curves_and_subtypes() {
    for version in VERSIONS {
        let base = source(version).replace("#12=IFCPOLYLINE((#10,#11));", "#12=IFCCOMPOSITECURVE((#50,#51),.F.);\n#50=IFCCOMPOSITECURVESEGMENT(.CONTINUOUS.,.T.,#52);\n#51=IFCCOMPOSITECURVESEGMENT(.CONTINUOUS.,.F.,#52);\n#52=IFCPOLYLINE((#10,#11));");
        let file = read(&base);
        let value = sweep(&file, 30);
        assert_eq!(value.directrix.items.len(), 2);
        let CurveKind::Composite { segments, .. } = &value.directrix.items[0].kind else {
            panic!()
        };
        assert_eq!(segments[0].curve, segments[1].curve);
        for (src, invalid) in [
            (base.replace(".T.,#52", ".T.,#12"), true),
            (
                base.replace("IFCPOLYLINE((#10,#11))", "IFCUNSUPPORTEDCURVE()"),
                false,
            ),
            (
                source(version)
                    .replace("IFCSWEPTDISKSOLID(#12,", "IFCSWEPTDISKSOLIDPOLYGONAL(#12,"),
                false,
            ),
            (
                source(version).replace(
                    "IFCSWEPTDISKSOLID(#12,2.5,1.25,0.,1.)",
                    "(IFCSWEPTDISKSOLID(#12,2.5,1.25,0.,1.))",
                ),
                false,
            ),
        ] {
            let file = read(&src);
            let shape = product(&file);
            let (result, diag, budget) = with(Limits::default(), |cx| {
                file.swept_disk(leaf(&shape, 30), cx)
            });
            if invalid {
                assert!(matches!(result, Err(Error::Invalid)), "{result:?}");
            } else {
                assert!(matches!(result, Err(Error::Unsupported)), "{result:?}");
            }
            assert!(!diag.is_empty());
            assert_eq!(budget.used(Kind::Depth), 0);
        }
    }
}

#[test]
fn preserves_existing_building_ids_hashes_and_semantics_under_renumbering() {
    for version in VERSIONS {
        let file = read(&source(version));
        let before = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
            .0
            .unwrap();
        let value = sweep(&file, 30);
        for id in [32, 31, 30, 32] {
            sweep(&file, id);
        }
        let after = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
            .0
            .unwrap();
        for n in 0..before.elements().len() {
            let element = ElementRef(n as u32);
            assert_eq!(before.id(element), after.id(element));
            assert_eq!(
                Hash::element(&before, element),
                Hash::element(&after, element)
            );
        }
        let renumbered = read(&source(version).replace('#', "#7"));
        let shape = with(Limits::default(), |cx| renumbered.product_shape(7100, cx))
            .0
            .unwrap();
        let result = with(Limits::default(), |cx| {
            renumbered.swept_disk(leaf(&shape, 730), cx)
        })
        .0
        .unwrap();
        assert_eq!(value.definition, result.definition);
        assert_eq!(value.directrix.length_unit, result.directrix.length_unit);
        assert_eq!(
            value.directrix.items[0].kind,
            result.directrix.items[0].kind
        );
        let foreign = product(&file);
        let (result, diag, _) = with(Limits::default(), |cx| {
            renumbered.swept_disk(leaf(&foreign, 30), cx)
        });
        assert!(matches!(result, Err(Error::Invalid)));
        assert!(format!("{diag:?}").contains("sweep-source"));
    }
}

#[test]
fn shares_budgets_and_restores_incoming_depth_on_every_exit() {
    for version in VERSIONS {
        let file = read(&source(version));
        let shape = product(&file);
        for id in [30, 31, 32] {
            let node = leaf(&shape, id);
            let (_, _, full) = with(Limits::default(), |cx| file.swept_disk(node, cx));
            for kind in Kind::ALL {
                let cap = if kind == Kind::Depth {
                    2
                } else {
                    full.used(kind)
                };
                let (ok, _, budget) =
                    with(Limits::default().with(kind, Limit::Finite(cap)), |cx| {
                        file.swept_disk(node, cx)
                    });
                assert!(ok.is_ok(), "{version} {id} {kind:?}: {ok:?}");
                assert_eq!(budget.used(Kind::Depth), 0);
                if cap == 0 {
                    continue;
                }
                let (fail, diag, budget) =
                    with(Limits::default().with(kind, Limit::Finite(cap - 1)), |cx| {
                        file.swept_disk(node, cx)
                    });
                assert!(
                    matches!(fail, Err(Error::Stopped(_))),
                    "{version} {id} {kind:?}"
                );
                assert!(diag.iter().any(|d| d.code() == kind.exceeded_code()));
                assert_eq!(budget.used(Kind::Depth), 0);
            }
            for limit in [0, 1, 12, 64, 128, 256, 1024] {
                let (_, _, budget) = with(
                    Limits::default().with(Kind::DecodedBytes, Limit::Finite(limit)),
                    |cx| {
                        cx.enter().unwrap();
                        let result = file.swept_disk(node, cx);
                        cx.leave();
                        result
                    },
                );
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
            file.swept_disk(leaf(&shape, 30), &mut cx),
            Err(Error::Stopped(_))
        ));
        cx.leave();
        assert_eq!(budget.used(Kind::Depth), 0);
        assert!(
            diag.iter()
                .any(|d| d.code() == codes::CALL_REQUEST_CANCELLED)
        );
    }
}

fn number(out: &mut Writer, value: Option<f64>) {
    if let Some(value) = value {
        out.float(value);
    } else {
        out.null();
    }
}

#[test]
fn fuzzes_truncated_and_mutated_inputs_with_shared_budgets() {
    let run = |src: &[u8], cx: &mut Cx| {
        let file = Ifc::read(src, cx)?;
        let shape = file.product_shape(100, cx)?;
        for node in &shape.nodes {
            cx.check(Kind::Work, 1)?;
            if file
                .kind(node.source)
                .eq_ignore_ascii_case(b"IFCSWEPTDISKSOLID")
            {
                file.swept_disk(node, cx)?;
            }
        }
        Ok(())
    };
    for version in VERSIONS {
        let bytes = source(version).into_bytes();
        for end in (0..bytes.len()).step_by(19) {
            zumen_sdk::testkit::survives_with(&bytes[..end], run).unwrap();
        }
        for at in (0..bytes.len()).step_by(17) {
            for byte in [0, b'$', b'#', 255] {
                let mut changed = bytes.clone();
                changed[at] = byte;
                zumen_sdk::testkit::survives_with(&changed, run).unwrap();
            }
        }
    }
}

fn json_item(out: &mut Writer, value: &SweptDisk<'_>) {
    out.begin_object();
    out.key("instance");
    out.uint(value.source.id().unwrap());
    out.key("radius");
    out.float(value.definition.radius);
    out.key("inner_radius");
    number(out, value.definition.inner_radius);
    out.key("start");
    number(out, value.definition.start_parameter);
    out.key("end");
    number(out, value.definition.end_parameter);
    out.key("directrix");
    out.uint(
        value.directrix.items[value.directrix.root.0 as usize]
            .source
            .instance
            .unwrap(),
    );
    out.key("curves");
    out.begin_array();
    curve_json::write_graph(out, &value.directrix);
    out.end_array();
    out.key("length_unit");
    if let Some(unit) = &value.directrix.length_unit {
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
        number(out, unit.scale);
        out.end_object();
    } else {
        out.null();
    }
    out.end_object();
}

#[test]
#[allow(clippy::disallowed_methods)] // テストの照合結果だけを指定先へ出す。
fn exports_values_diagnostics_and_all_budget_kinds_for_comparison() {
    let Ok(folder) = std::env::var("ZUMEN_IFC_SWEEPS_OUT") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    let mut contract = String::new();
    for version in VERSIONS {
        let source = source(version);
        let file = read(&source);
        let shape = product(&file);
        let mut out = Writer::new();
        out.begin_object();
        out.key("file");
        out.string(&format!("{version}.ifc"));
        out.key("product");
        out.uint(100);
        out.key("items");
        out.begin_array();
        for id in [30, 31, 32] {
            let node = leaf(&shape, id);
            let (value, diag, budget) = with(Limits::default(), |cx| file.swept_disk(node, cx));
            json_item(&mut out, &value.unwrap());
            contract += &format!(
                "{version}:{id}:ok:{diag:?}:{:?}\n",
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
                            file.swept_disk(node, cx)
                        });
                    contract += &format!(
                        "{version}:{id}:{kind:?}:{cap}:{:?}:{diag:?}:{:?}\n",
                        result.err(),
                        Kind::ALL.map(|k| budget.used(k))
                    );
                }
            }
        }
        out.end_array();
        out.end_object();
        std::fs::write(folder.join(format!("{version}.json")), out.into_bytes()).unwrap();
        std::fs::write(folder.join(format!("{version}.ifc")), &source).unwrap();
        for (from, to) in invalid_inputs() {
            let bad = read(&source.replace(from, to));
            let shape = product(&bad);
            let (result, diag, budget) =
                with(Limits::default(), |cx| bad.swept_disk(leaf(&shape, 30), cx));
            contract += &format!(
                "{version}:{to}:{:?}:{diag:?}:{:?}\n",
                result.err(),
                Kind::ALL.map(|k| budget.used(k))
            );
        }
    }
    std::fs::write(folder.join("contract.txt"), contract).unwrap();
}
