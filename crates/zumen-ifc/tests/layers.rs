//! 公開 IFC と自作入力で、項目・表現のレイヤと直接の面の表示を照らす。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{Building, Hash, LayerState};
use zumen_ifc::{
    Ifc, IfcFile, ItemPresentation, LayerAssignmentLevel as Level, ProductShape, ShapeNode,
    SurfaceStyleSelection as Selection,
};
use zumen_json::Writer;
use zumen_sdk::{Codec, Error, Lift};

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];
const PUBLIC: &str = include_str!("../../../samples/ifc/gni/model_9.ifc");

fn source(schema: &str) -> String {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\nFILE_NAME('','',(''),(''),'','','');\nFILE_SCHEMA(('{schema}'));\nENDSEC;\nDATA;\n\
    #1=IFCPROJECT('0000000000000000000001',$,'Project',$,$,$,$,(#8),#9);\n\
    #8=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-5,#20,$);\n\
    #9=IFCUNITASSIGNMENT((#10));\n\
    #10=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);\n\
    #20=IFCAXIS2PLACEMENT3D(#21,$,$);\n\
    #21=IFCCARTESIANPOINT((0.,0.,0.));\n\
    #22=IFCCARTESIANPOINT((5.,0.,0.));\n\
    #30=IFCPOLYLINE((#21,#22));\n\
    #40=IFCMAPPEDITEM(#41,#43);\n\
    #41=IFCREPRESENTATIONMAP(#20,#42);\n\
    #42=IFCSHAPEREPRESENTATION(#8,'Body','Curve3D',(#30));\n\
    #43=IFCCARTESIANTRANSFORMATIONOPERATOR3D($,$,#21,1.,$);\n\
    #100=IFCWALL('0000000000000000000002',$,'Wall',$,$,#200,#101,${wall_type});\n\
    #101=IFCPRODUCTDEFINITIONSHAPE($,$,(#102));\n\
    #102=IFCSHAPEREPRESENTATION(#8,'Body','MappedRepresentation',(#40));\n\
    #200=IFCLOCALPLACEMENT($,#20);\n\
    #300=IFCSTYLEDITEM(#30,({direct}),'Direct');\n\
    #301=IFCSURFACESTYLE('Direct surface',.BOTH.,(#302));\n\
    #302=IFCSURFACESTYLESHADING(#303{transparency});\n\
    #303=IFCCOLOURRGB('Red',1.,0.,0.);\n\
    #307=IFCPRESENTATIONSTYLEASSIGNMENT((#301));\n\
    #310=IFCSURFACESTYLE('Layer surface',.BOTH.,(#311));\n\
    #311=IFCSURFACESTYLESHADING(#312{transparency});\n\
    #312=IFCCOLOURRGB('Blue',0.,0.,1.);\n\
    #400=IFCPRESENTATIONLAYERASSIGNMENT('Representation','Base',(#42,#102),'R');\n\
    #401=IFCPRESENTATIONLAYERWITHSTYLE('Item','Control',(#30),'I',.T.,.F.,.U.,(#310));\n\
    #402=IFCPRESENTATIONLAYERWITHSTYLE('Mapped',$,(#40),$,.F.,.U.,.T.,());\n\
    ENDSEC;\nEND-ISO-10303-21;\n",
        wall_type = if schema == "IFC2X3" {
            ""
        } else {
            ",.NOTDEFINED."
        },
        direct = if schema == "IFC2X3" { "#307" } else { "#301" },
        transparency = if schema == "IFC2X3" { "" } else { ",0.125" }
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
fn leaf<'s, 'f>(s: &'s ProductShape<'f>, id: u64) -> &'s ShapeNode<'f> {
    s.nodes.iter().find(|n| n.source.id() == Some(id)).unwrap()
}
fn get(file: &IfcFile) -> ItemPresentation<'_> {
    let s = product(file, 100);
    let (r, d, _) = with(Limits::default(), |cx| {
        file.item_presentation(leaf(&s, 30), Some(leaf(&s, 42)), cx)
    });
    assert!(d.is_empty(), "{d:?}");
    r.unwrap()
}

#[test]
fn keeps_overridden_values_and_selects_direct_surface_before_layer_surface() {
    for version in VERSIONS {
        let file = read(&source(version));
        let v = get(&file);
        assert_eq!(v.layer_source, Level::Item);
        assert_eq!(v.surface_source, Selection::Direct);
        assert_eq!(
            v.direct.styles[0].definition.shading.colour.rgb,
            [1., 0., 0.]
        );
        assert_eq!(v.layers.len(), 2);
        assert_eq!(v.layers[0].level, Level::Representation);
        let l = &v.layers[1];
        assert_eq!(l.definition.name, "Item");
        assert_eq!(l.definition.description.as_deref(), Some("Control"));
        assert_eq!(l.definition.identifier.as_deref(), Some("I"));
        assert_eq!(
            l.definition.state,
            Some(LayerState {
                on: Some(true),
                frozen: Some(false),
                blocked: None
            })
        );
        assert_eq!(l.styles[0].definition.shading.colour.rgb, [0., 0., 1.]);
        assert!(std::ptr::eq(l.source, file.instance(401).unwrap()));
        assert_eq!(l.styles[0].source.style.id(), Some(310));
        let no_direct =
            source(version).replace("#300=IFCSTYLEDITEM(#30,", "#300=IFCSTYLEDITEM(#40,");
        let file = read(&no_direct);
        assert_eq!(get(&file).surface_source, Selection::Layer(1));
        let s = product(&file, 100);
        let v = with(Limits::default(), |cx| {
            file.item_presentation(leaf(&s, 40), Some(leaf(&s, 102)), cx)
        })
        .0
        .unwrap();
        assert_eq!(v.layers[1].source.id(), Some(402));
        assert_eq!(
            v.layers[1].definition.state,
            Some(LayerState {
                on: Some(false),
                frozen: None,
                blocked: Some(true)
            })
        );
        assert_eq!(v.surface_source, Selection::Direct);
    }
}

#[test]
fn item_layer_replaces_representation_layers_without_inventing_defaults() {
    for version in VERSIONS {
        let src=source(version).replace("#300=IFCSTYLEDITEM(#30,","#300=IFCSTYLEDITEM(#40,")
            .replace("#401=IFCPRESENTATIONLAYERWITHSTYLE('Item','Control',(#30),'I',.T.,.F.,.U.,(#310));",
                     "#401=IFCPRESENTATIONLAYERASSIGNMENT('',$,(#30),$);");
        let file = read(&src);
        let v = get(&file);
        assert_eq!(v.layer_source, Level::Item);
        assert_eq!(v.surface_source, Selection::None);
        assert_eq!(v.layers[1].definition.name, "");
        assert_eq!(v.layers[1].definition.state, None);
        let no_item = src.replace("('',$,(#30),$)", "('',$,(#40),$)").replace(
            "#402=IFCPRESENTATIONLAYERWITHSTYLE('Mapped',$,(#40),$,.F.,.U.,.T.,());",
            "",
        );
        let file = read(&no_item);
        assert_eq!(get(&file).layer_source, Level::Representation);
        let s = product(&file, 100);
        let v = with(Limits::default(), |cx| {
            file.item_presentation(leaf(&s, 30), None, cx)
        })
        .0
        .unwrap();
        assert!(v.layers.is_empty());
        assert_eq!(v.layer_source, Level::None);
        assert_eq!(v.surface_source, Selection::None);
    }
    let src=source("IFC2X3").replace("#300=IFCSTYLEDITEM(#30,","#300=IFCSTYLEDITEM(#40,")
        .replace("#400=IFCPRESENTATIONLAYERASSIGNMENT('Representation','Base',(#42,#102),'R');",
                 "#400=IFCPRESENTATIONLAYERWITHSTYLE('Representation','Base',(#42,#102),'R',.T.,.U.,.F.,(#301));")
        .replace("#401=IFCPRESENTATIONLAYERWITHSTYLE('Item','Control',(#30),'I',.T.,.F.,.U.,(#310));",
                 "#401=IFCPRESENTATIONLAYERASSIGNMENT('Item','Control',(#30),'I');");
    let file = read(&src);
    assert_eq!(get(&file).surface_source, Selection::None);
    let file = read(&src.replace(
        "#401=IFCPRESENTATIONLAYERASSIGNMENT('Item','Control',(#30),'I');",
        "",
    ));
    assert_eq!(get(&file).surface_source, Selection::Layer(0));
}

fn invalid_inputs() -> Vec<(&'static str, &'static str)> {
    vec![
        ("'I',.T.,.F.,.U.", "'I',$,.F.,.U."),
        ("'I',.T.,.F.,.U.", "'I',.X.,.F.,.U."),
        ("('Item','Control'", "($,'Control'"),
        ("'Control',(#30)", "'Control',(#30,#30)"),
        ("'Control',(#30)", "'Control',#30"),
        (".U.,(#310)", ".U.,(#310,#310)"),
        (".U.,(#310)", ".U.,(#9999)"),
        (".U.,(#310)", ".U.,(#401)"),
        (".U.,(#310)", ".U.,(#307)"),
        (".U.,(#310)", ".U.,$"),
    ]
}
#[test]
fn rejects_bad_values_membership_foreign_nodes_and_multiple_item_layers() {
    for version in VERSIONS {
        for (from, to) in invalid_inputs() {
            let file = read(&source(version).replace(from, to));
            let s = product(&file, 100);
            let (r, d, b) = with(Limits::default(), |cx| {
                file.item_presentation(leaf(&s, 30), Some(leaf(&s, 42)), cx)
            });
            assert!(matches!(r, Err(Error::Invalid)), "{version}:{to}:{r:?}");
            assert!(d.iter().any(|d| d.code() == codes::IFC_STYLE_INVALID));
            assert_eq!(b.used(Kind::Depth), 0);
        }
        let file = read(&source(version));
        let other = read(&source(version));
        let s = product(&file, 100);
        let t = product(&other, 100);
        for rep in [leaf(&s, 102), leaf(&s, 101), leaf(&t, 42)] {
            assert!(matches!(
                with(Limits::default(), |cx| file.item_presentation(
                    leaf(&s, 30),
                    Some(rep),
                    cx
                ))
                .0,
                Err(Error::Invalid)
            ));
        }
        let src = source(version)
            .replace("#300=IFCSTYLEDITEM(#30,", "#300=IFCSTYLEDITEM(#40,")
            .replace(
                "#402=IFCPRESENTATIONLAYERWITHSTYLE('Mapped',$,(#40),$,.F.,.U.,.T.,());",
                "#402=IFCPRESENTATIONLAYERWITHSTYLE('Another',$,(#30),$,.F.,.U.,.T.,(#301));",
            );
        let file = read(&src);
        let s = product(&file, 100);
        let r = with(Limits::default(), |cx| {
            file.item_presentation(leaf(&s, 30), Some(leaf(&s, 42)), cx)
        })
        .0;
        if version == "IFC2X3" {
            assert_eq!(r.unwrap().surface_source, Selection::MultipleLayers)
        } else {
            assert!(matches!(r, Err(Error::Invalid)));
        }
        if version != "IFC2X3" {
            let file = read(&source(version).replace("'Control',(#30)", "'Control',(#30,#42)"));
            let s = product(&file, 100);
            assert!(matches!(
                with(Limits::default(), |cx| file.item_presentation(
                    leaf(&s, 30),
                    Some(leaf(&s, 42)),
                    cx
                ))
                .0,
                Err(Error::Invalid)
            ));
        }
    }
}

#[test]
fn does_not_hide_unknown_style_or_complex_layer_assignment() {
    for version in VERSIONS {
        let src = source(version).replace(
            "#310=IFCSURFACESTYLE('Layer surface',.BOTH.,(#311));",
            "#310=IFCCURVESTYLE($,$,$,$);",
        );
        let file = read(&src);
        let s = product(&file, 100);
        let (r, d, _) = with(Limits::default(), |cx| {
            file.item_presentation(leaf(&s, 30), None, cx)
        });
        assert!(matches!(r, Err(Error::Unsupported)));
        assert!(d.iter().any(|d| d.code() == codes::IFC_STYLE_UNSUPPORTED));
        let src=source(version).replace("#401=IFCPRESENTATIONLAYERWITHSTYLE('Item','Control',(#30),'I',.T.,.F.,.U.,(#310));","#401=(IFCPRESENTATIONLAYERASSIGNMENT('Item','Control',(#30),'I') IFCPRESENTATIONLAYERWITHSTYLE(.T.,.F.,.U.,(#310)));");
        let file = read(&src);
        let s = product(&file, 100);
        assert!(matches!(
            with(Limits::default(), |cx| file.item_presentation(
                leaf(&s, 30),
                None,
                cx
            ))
            .0,
            Err(Error::Invalid)
        ));
        let src = source(version).replace(
            "#401=IFCPRESENTATIONLAYERWITHSTYLE('Item','Control',(#30),'I',.T.,.F.,.U.,(#310));",
            "#401=(IFCPRESENTATIONLAYERASSIGNMENT('A',$,(#30),$) IFCPRESENTATIONLAYERASSIGNMENT('B',$,(#40),$));",
        );
        let file = read(&src);
        let s = product(&file, 100);
        assert!(matches!(
            with(Limits::default(), |cx| file.item_presentation(
                leaf(&s, 30),
                None,
                cx
            ))
            .0,
            Err(Error::Invalid)
        ));
    }
    let file = read(&source("IFC2X3").replace(".U.,(#310)", ".U.,(.NULL.)"));
    let s = product(&file, 100);
    assert!(matches!(
        with(Limits::default(), |cx| file.item_presentation(
            leaf(&s, 30),
            None,
            cx
        ))
        .0,
        Err(Error::Unsupported)
    ));
}

#[test]
fn keeps_meaning_and_building_ids_independent_of_lookup_and_step_numbers() {
    let src = source("IFC4");
    let file = read(&src);
    let before = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
        .0
        .unwrap();
    let v = get(&file);
    let s = product(&file, 100);
    with(Limits::default(), |cx| {
        file.item_presentation(leaf(&s, 40), Some(leaf(&s, 102)), cx)
    })
    .0
    .unwrap();
    let after = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
        .0
        .unwrap();
    assert_eq!(
        Hash::document_values(&before),
        Hash::document_values(&after)
    );
    for (at, e) in before.elements().iter().enumerate() {
        assert_eq!(e.id(), after.elements()[at].id());
        let at = zumen_building::ElementRef(at as u32);
        assert_eq!(Hash::element(&before, at), Hash::element(&after, at));
    }
    let shifted = read(
        &src.replace("#401", "#1401")
            .replace("#310", "#1310")
            .replace("#30,", "#130,")
            .replace("#30)", "#130)")
            .replace("#30=", "#130="),
    );
    let s = product(&shifted, 100);
    let w = with(Limits::default(), |cx| {
        shifted.item_presentation(leaf(&s, 130), Some(leaf(&s, 42)), cx)
    })
    .0
    .unwrap();
    assert_eq!(v.layers[1].definition, w.layers[1].definition);
    assert_eq!(
        v.layers[1].styles[0].definition,
        w.layers[1].styles[0].definition
    );
    assert_ne!(v.layers[1].source.id(), w.layers[1].source.id());
    assert_eq!(v.surface_source, w.surface_source);
}

#[test]
fn charges_every_budget_kind_and_restores_depth_on_failure_and_cancel() {
    for version in VERSIONS {
        let file = read(&source(version));
        let s = product(&file, 100);
        let call = |cx: &mut Cx| file.item_presentation(leaf(&s, 30), Some(leaf(&s, 42)), cx);
        let (r, d, b) = with(Limits::default(), call);
        assert!(r.is_ok());
        assert!(d.is_empty());
        assert_eq!(b.used(Kind::Depth), 0);
        for kind in Kind::ALL {
            let used = if kind == Kind::Depth {
                (0..16)
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
        for expired in [true, false] {
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
            assert_eq!(b.used(Kind::Work), 0);
        }
        let (_, _, b) = with(Limits::default(), |cx| {
            cx.enter().unwrap();
            call(cx).unwrap()
        });
        assert_eq!(b.used(Kind::Depth), 1);
    }
}

#[test]
fn public_column_keeps_representation_layer_and_direct_surface() {
    let file = read(PUBLIC);
    let s = product(&file, 131);
    let (r, d, _) = with(Limits::default(), |cx| {
        file.item_presentation(leaf(&s, 114), Some(leaf(&s, 119)), cx)
    });
    assert!(d.is_empty());
    let v = r.unwrap();
    assert_eq!(v.layer_source, Level::Representation);
    assert_eq!(v.surface_source, Selection::Direct);
    assert_eq!(v.layers.len(), 1);
    assert_eq!(v.layers[0].source.id(), Some(5883));
    assert_eq!(v.layers[0].definition.name, "S-COLS-____-OTLN");
    assert_eq!(v.direct.styles[0].source.style.id(), Some(117));
}

#[test]
fn mutated_inputs_finish_with_repeatable_usage_and_diagnostics() {
    let src = source("IFC4");
    for at in (0..src.len()).step_by(47) {
        let mut bytes = src.as_bytes().to_vec();
        bytes[at] = b'$';
        zumen_sdk::testkit::survives_with(&bytes, |bytes, cx| {
            let file = Ifc::read(bytes, cx)?;
            let s = file.product_shape(100, cx)?;
            if let Some(node) = s.nodes.iter().find(|n| n.source.id() == Some(30)) {
                file.item_presentation(node, None, cx)?;
            }
            Ok::<_, Error>(())
        })
        .unwrap();
    }
}

fn optional(out: &mut Writer, text: Option<&str>) {
    if let Some(t) = text {
        out.string(t)
    } else {
        out.null()
    }
}
fn logical(out: &mut Writer, value: Option<bool>) {
    if let Some(v) = value {
        out.bool(v)
    } else {
        out.null()
    }
}
#[test]
#[allow(clippy::disallowed_methods)] // 自作入力と公開例の元値だけを指定先へ出す。
fn exports_raw_layers_sources_precedence_and_budget_contract() {
    let Ok(folder) = std::env::var("ZUMEN_IFC_LAYERS_OUT") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    let mut contract = String::new();
    let mut export =
        |file: &IfcFile, name: &str, path: &str, product_id: u64, item: u64, rep: u64| {
            let s = product(file, product_id);
            let call =
                |cx: &mut Cx| file.item_presentation(leaf(&s, item), Some(leaf(&s, rep)), cx);
            let (v, d, b) = with(Limits::default(), call);
            let v = v.unwrap();
            contract += &format!("{name}:ok:{v:?}:{d:?}:{:?}\n", Kind::ALL.map(|k| b.used(k)));
            let mut o = Writer::new();
            o.begin_object();
            o.key("file");
            o.string(path);
            o.key("product");
            o.uint(product_id);
            o.key("item");
            o.uint(item);
            o.key("representation");
            o.uint(rep);
            o.key("styled_item");
            if let Some(r) = v.direct.source.styled_item {
                o.uint(r.id().unwrap())
            } else {
                o.null()
            };
            o.key("layers");
            o.begin_array();
            for l in &v.layers {
                o.begin_object();
                o.key("instance");
                o.uint(l.source.id().unwrap());
                o.key("name");
                o.string(&l.definition.name);
                o.key("description");
                optional(&mut o, l.definition.description.as_deref());
                o.key("identifier");
                optional(&mut o, l.definition.identifier.as_deref());
                o.key("state");
                if let Some(s) = l.definition.state {
                    o.begin_array();
                    logical(&mut o, s.on);
                    logical(&mut o, s.frozen);
                    logical(&mut o, s.blocked);
                    o.end_array()
                } else {
                    o.null()
                };
                o.key("styles");
                o.begin_array();
                for s in &l.styles {
                    o.begin_object();
                    o.key("instance");
                    o.uint(s.source.style.id().unwrap());
                    o.key("name");
                    optional(&mut o, s.definition.name.as_deref());
                    o.key("rgb");
                    o.begin_array();
                    for n in s.definition.shading.colour.rgb {
                        o.float(n);
                    }
                    o.end_array();
                    o.key("transparency");
                    if let Some(n) = s.definition.shading.transparency {
                        o.float(n)
                    } else {
                        o.null()
                    };
                    o.end_object();
                }
                o.end_array();
                o.end_object();
            }
            o.end_array();
            o.end_object();
            std::fs::write(folder.join(format!("{name}.json")), o.into_bytes()).unwrap();
            for kind in Kind::ALL {
                let used = if kind == Kind::Depth {
                    (0..16)
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
        export(&read(&src), version, &format!("{version}.ifc"), 100, 30, 42);
        std::fs::write(folder.join(format!("{version}.ifc")), src).unwrap();
    }
    export(
        &read(PUBLIC),
        "public",
        "../../samples/ifc/gni/model_9.ifc",
        131,
        114,
        119,
    );
    for version in VERSIONS {
        for (from, to) in invalid_inputs() {
            let file = read(&source(version).replace(from, to));
            let s = product(&file, 100);
            let (r, d, b) = with(Limits::default(), |cx| {
                file.item_presentation(leaf(&s, 30), Some(leaf(&s, 42)), cx)
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
