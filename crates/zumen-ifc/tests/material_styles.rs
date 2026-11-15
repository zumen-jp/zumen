//! 材料の表示を文脈ごとに取得する。形への直接の StyledItem と混ぜない。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::SurfaceSide;
use zumen_ifc::{Ifc, IfcFile, MaterialPresentation, MaterialStyleContext};
use zumen_json::Writer;
use zumen_sdk::{Codec, Error};

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];

fn source(version: &str) -> String {
    let (material, transparency) = if version == "IFC2X3" {
        ("", "")
    } else {
        (",'Description','Category'", ",0.25")
    };
    let (first, second) = if version == "IFC2X3" {
        (303, 313)
    } else {
        (302, 312)
    };
    format!(
        "ISO-10303-21;\nHEADER;FILE_DESCRIPTION((''),'2;1');FILE_NAME('','',(''),(''),'','','');FILE_SCHEMA(('{version}'));ENDSEC;\nDATA;\n\
        #1=IFCPROJECT('0000000000000000000001',$,'Project',$,$,$,$,(#8),#9);\n\
        #8=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-5,#20,$);\n\
        #9=IFCUNITASSIGNMENT((#10));#10=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);\n\
        #20=IFCAXIS2PLACEMENT3D(#21,$,$);#21=IFCCARTESIANPOINT((0.,0.,0.));\n\
        #70=IFCREPRESENTATIONCONTEXT('General','Text');\n\
        #71=IFCGEOMETRICREPRESENTATIONSUBCONTEXT('Body','Model',*,*,*,*,#8,0.01,.MODEL_VIEW.,$);\n\
        #80=IFCMATERIAL('Concrete'{material});\n\
        #81=IFCMATERIALDEFINITIONREPRESENTATION('Appearance','Material appearance',(#82,#83,#84),#80);\n\
        #82=IFCSTYLEDREPRESENTATION(#8,'Body','Material',(#90,#91));\n\
        #83=IFCSTYLEDREPRESENTATION(#70,'Text',$,(#90));\n\
        #84=IFCSTYLEDREPRESENTATION(#71,$,$,(#91));\n\
        #90=IFCSTYLEDITEM($,(#{first}),'Positive');#91=IFCSTYLEDITEM($,(#{second}),'Negative');\n\
        #99=IFCMATERIAL('No representation'{material});\n\
        #300=IFCCOLOURRGB('Blue',0.,0.,1.);#301=IFCSURFACESTYLESHADING(#300{transparency});\n\
        #302=IFCSURFACESTYLE('Blue surface',.POSITIVE.,(#301));#303=IFCPRESENTATIONSTYLEASSIGNMENT((#302));\n\
        #310=IFCCOLOURRGB('Green',0.,1.,0.);#311=IFCSURFACESTYLESHADING(#310{transparency});\n\
        #312=IFCSURFACESTYLE('Green surface',.NEGATIVE.,(#311));#313=IFCPRESENTATIONSTYLEASSIGNMENT((#312));\n\
        ENDSEC;END-ISO-10303-21;\n"
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
fn get(file: &IfcFile) -> MaterialPresentation<'_> {
    let (result, diagnostics, budget) =
        with(Limits::default(), |cx| file.material_presentation(80, cx));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(budget.used(Kind::Depth), 0);
    result.unwrap()
}

#[test]
fn preserves_material_contexts_plural_items_and_all_sources() {
    for version in VERSIONS {
        let file = read(&source(version));
        let value = get(&file);
        assert_eq!(value.material.id(), Some(80));
        assert_eq!(value.source.unwrap().id(), Some(81));
        assert_eq!(value.name.as_deref(), Some("Appearance"));
        assert_eq!(value.description.as_deref(), Some("Material appearance"));
        assert_eq!(value.representations.len(), 3);
        let first = &value.representations[0];
        assert_eq!(first.source.id(), Some(82));
        assert_eq!(first.identifier.as_deref(), Some("Body"));
        assert_eq!(first.representation_type.as_deref(), Some("Material"));
        assert_eq!(first.items.len(), 2);
        let MaterialStyleContext::Geometric(context) = &first.context else {
            panic!()
        };
        assert_eq!(context.source.context.id(), Some(8));
        assert_eq!(context.definition.geometric.dimension, 3);
        assert_eq!(
            context.length_unit,
            Some(zumen_building::Unit {
                kind: "LENGTHUNIT".into(),
                prefix: None,
                name: "METRE".into(),
                scale: None,
            })
        );
        for (at, item) in first.items.iter().enumerate() {
            assert_eq!(item.source.id(), Some(90 + at as u64));
            assert_eq!(item.styles.len(), 1);
            let style = &item.styles[0];
            assert_eq!(style.source.style.id(), Some(302 + 10 * at as u64));
            assert_eq!(style.source.shading.id(), Some(301 + 10 * at as u64));
            assert_eq!(
                style
                    .source
                    .colours
                    .iter()
                    .map(|r| r.id())
                    .collect::<Vec<_>>(),
                [Some(300 + 10 * at as u64)]
            );
            assert_eq!(item.assignments.len(), usize::from(version == "IFC2X3"));
            assert_eq!(
                style.definition.side,
                if at == 0 {
                    SurfaceSide::Positive
                } else {
                    SurfaceSide::Negative
                }
            );
            assert_eq!(
                style.definition.shading.colour.rgb,
                if at == 0 { [0., 0., 1.] } else { [0., 1., 0.] }
            );
            assert_eq!(
                style.definition.shading.transparency,
                if version == "IFC2X3" {
                    None
                } else {
                    Some(0.25)
                }
            );
        }
        assert!(matches!(&value.representations[1].context,
            MaterialStyleContext::General { identifier, kind, source }
            if identifier.as_deref()==Some("General") && kind.as_deref()==Some("Text") && source.id()==Some(70)));
        let MaterialStyleContext::Geometric(context) = &value.representations[2].context else {
            panic!()
        };
        assert_eq!(context.source.parent.unwrap().id(), Some(8));
        assert_eq!(
            context.definition.view.as_ref().unwrap().target_scale,
            Some(0.01)
        );
        let empty = with(Limits::default(), |cx| file.material_presentation(99, cx))
            .0
            .unwrap();
        assert!(empty.source.is_none() && empty.representations.is_empty());
    }
}

#[test]
fn rejects_wrong_material_owners_targets_cycles_and_style_items() {
    for (from, to) in [
        ("(#82,#83,#84),#80", "(),#80"),
        ("(#82,#83,#84),#80", "(#81),#80"),
        ("(#82,#83,#84),#80", "(#999),#80"),
        ("#82=IFCSTYLEDREPRESENTATION", "#82=IFCSHAPEREPRESENTATION"),
        ("#90=IFCSTYLEDITEM($,", "#90=IFCSTYLEDITEM(#21,"),
        ("(#90,#91)", "(#90,#90)"),
        ("(#90,#91)", "(#90,#302)"),
        ("(#90,#91)", "()"),
        (
            "#70=IFCREPRESENTATIONCONTEXT('General','Text')",
            "#70=IFCREPRESENTATIONCONTEXT('General')",
        ),
        (
            "#81=IFCMATERIALDEFINITIONREPRESENTATION('Appearance','Material appearance',(#82,#83,#84),#80)",
            "#81=(IFCPRODUCTREPRESENTATION('Appearance',$,(#82))IFCMATERIALDEFINITIONREPRESENTATION(#80))",
        ),
        (
            "#99=IFCMATERIAL('No representation','Description','Category');",
            "#99=IFCMATERIALDEFINITIONREPRESENTATION($,$,(#82),#80);",
        ),
    ] {
        let file = read(&source("IFC4").replace(from, to));
        let (r, d, b) = with(Limits::default(), |cx| file.material_presentation(80, cx));
        assert!(matches!(r, Err(Error::Invalid)), "{from}: {r:?}");
        assert!(d.iter().any(|v| v.code() == codes::IFC_STYLE_INVALID));
        assert_eq!(b.used(Kind::Depth), 0);
    }
    let file = read(&source("IFC4"));
    for id in [0, 8, 81, 90, 999] {
        assert!(matches!(
            with(Limits::default(), |cx| file.material_presentation(id, cx)).0,
            Err(Error::Invalid)
        ));
    }
}

#[test]
fn unhandled_styles_fail_the_whole_material_and_keep_the_kind() {
    for version in VERSIONS {
        let file = read(&source(version).replace(
            "#312=IFCSURFACESTYLE('Green surface',.NEGATIVE.,(#311))",
            "#312=IFCCURVESTYLE('Unknown',$,$,$)",
        ));
        let (r, d, b) = with(Limits::default(), |cx| file.material_presentation(80, cx));
        assert!(matches!(r, Err(Error::Unsupported)));
        assert!(d.iter().any(|d| d.code() == codes::IFC_STYLE_UNSUPPORTED));
        assert!(format!("{d:?}").contains("IFCCURVESTYLE"));
        assert_eq!(b.used(Kind::Depth), 0);
    }
}

fn boundary(file: &IfcFile, id: u64, kind: Kind) -> u64 {
    if kind != Kind::Depth {
        return with(Limits::default(), |cx| file.material_presentation(id, cx))
            .2
            .used(kind);
    }
    (0..=64)
        .find(|n| {
            with(Limits::default().with(kind, Limit::Finite(*n)), |cx| {
                file.material_presentation(id, cx)
            })
            .0
            .is_ok()
        })
        .unwrap()
}

#[test]
fn shared_limits_cancel_and_expiry_return_no_partial_value() {
    for version in VERSIONS {
        let file = read(&source(version));
        for kind in Kind::ALL {
            let cap = boundary(&file, 80, kind);
            let (r, _, b) = with(Limits::default().with(kind, Limit::Finite(cap)), |cx| {
                file.material_presentation(80, cx)
            });
            assert!(r.is_ok(), "{version}/{kind:?}/{cap}");
            assert_eq!(b.used(Kind::Depth), 0);
            if cap > 0 {
                let (r, _, b) = with(Limits::default().with(kind, Limit::Finite(cap - 1)), |cx| {
                    file.material_presentation(80, cx)
                });
                assert!(
                    matches!(r, Err(Error::Stopped(_))),
                    "{version}/{kind:?}/{cap}"
                );
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
                file.material_presentation(80, cx)
            });
            assert!(matches!(r, Err(Error::Stopped(_))));
            assert_eq!(b.used(Kind::DecodedBytes), 0);
        }
    }
}

fn text(out: &mut Writer, value: Option<&str>) {
    if let Some(v) = value {
        out.string(v)
    } else {
        out.null()
    }
}
fn json(value: &MaterialPresentation) -> Vec<u8> {
    let mut out = Writer::new();
    out.begin_object();
    out.key("material");
    out.uint(value.material.id().unwrap());
    out.key("definition");
    out.uint(value.source.unwrap().id().unwrap());
    out.key("name");
    text(&mut out, value.name.as_deref());
    out.key("description");
    text(&mut out, value.description.as_deref());
    out.key("representations");
    out.begin_array();
    for rep in &value.representations {
        out.begin_object();
        out.key("id");
        out.uint(rep.source.id().unwrap());
        out.key("identifier");
        text(&mut out, rep.identifier.as_deref());
        out.key("type");
        text(&mut out, rep.representation_type.as_deref());
        out.key("context");
        out.uint(match &rep.context {
            MaterialStyleContext::General { source, .. } => source.id().unwrap(),
            MaterialStyleContext::Geometric(v) => v.source.context.id().unwrap(),
        });
        out.key("items");
        out.begin_array();
        for item in &rep.items {
            out.begin_object();
            out.key("id");
            out.uint(item.source.id().unwrap());
            out.key("name");
            text(&mut out, item.name.as_deref());
            out.key("assignments");
            out.begin_array();
            for a in &item.assignments {
                out.uint(a.id().unwrap());
            }
            out.end_array();
            out.key("styles");
            out.begin_array();
            for style in &item.styles {
                out.begin_object();
                out.key("id");
                out.uint(style.source.style.id().unwrap());
                out.key("shading");
                out.uint(style.source.shading.id().unwrap());
                out.key("colours");
                out.begin_array();
                for c in &style.source.colours {
                    out.uint(c.id().unwrap());
                }
                out.end_array();
                out.key("name");
                text(&mut out, style.definition.name.as_deref());
                out.key("side");
                out.string(match style.definition.side {
                    SurfaceSide::Positive => "POSITIVE",
                    SurfaceSide::Negative => "NEGATIVE",
                    SurfaceSide::Both => "BOTH",
                });
                out.key("rgb");
                out.begin_array();
                for c in style.definition.shading.colour.rgb {
                    out.float(c);
                }
                out.end_array();
                out.key("transparency");
                if let Some(v) = style.definition.shading.transparency {
                    out.float(v)
                } else {
                    out.null()
                };
                out.end_object();
            }
            out.end_array();
            out.end_object();
        }
        out.end_array();
        out.end_object();
    }
    out.end_array();
    out.end_object();
    out.into_bytes()
}

#[test]
#[allow(clippy::disallowed_methods)] // 比較するテストの出力先だけを読む。
fn exports_material_values_sources_diagnostics_and_boundaries() {
    let Ok(dir) = std::env::var("ZUMEN_IFC_MATERIAL_OUT") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let export = |name: &str, src: &str, id: u64| {
        let file = read(src);
        let (result, diagnostics, _) =
            with(Limits::default(), |cx| file.material_presentation(id, cx));
        assert!(diagnostics.is_empty());
        let value = result.unwrap();
        std::fs::write(std::path::Path::new(&dir).join(format!("{name}.ifc")), src).unwrap();
        std::fs::write(
            std::path::Path::new(&dir).join(format!("{name}.json")),
            json(&value),
        )
        .unwrap();
        let mut contract = format!("{value:?}\n");
        for kind in Kind::ALL {
            let required = boundary(&file, id, kind);
            for cap in [required.saturating_sub(1), required] {
                let (r, d, b) = with(Limits::default().with(kind, Limit::Finite(cap)), |cx| {
                    file.material_presentation(id, cx)
                });
                contract.push_str(&format!("{kind:?}={cap}: {r:?}; {d:?}; "));
                for k in Kind::ALL {
                    contract.push_str(&format!("{k:?}={};", b.used(k)));
                }
                contract.push('\n');
            }
        }
        std::fs::write(
            std::path::Path::new(&dir).join(format!("{name}.txt")),
            contract,
        )
        .unwrap();
    };
    for version in VERSIONS {
        export(version, &source(version), 80);
    }
    export(
        "public_material",
        include_str!("../../../samples/ifc/bim-whale/TallBuilding.ifc"),
        638,
    );
}

#[test]
fn public_material_keeps_the_original_definition_and_style_references() {
    let file = read(include_str!(
        "../../../samples/ifc/bim-whale/TallBuilding.ifc"
    ));
    let (r, d, _) = with(Limits::default(), |cx| file.material_presentation(638, cx));
    assert!(d.is_empty());
    let value = r.unwrap();
    assert_eq!(value.source.unwrap().id(), Some(648));
    assert_eq!(value.representations.len(), 1);
    let rep = &value.representations[0];
    assert_eq!(rep.source.id(), Some(645));
    assert_eq!(rep.items[0].source.id(), Some(643));
    assert_eq!(rep.items[0].assignments[0].id(), Some(641));
    assert_eq!(rep.items[0].styles[0].source.style.id(), Some(604));
}
