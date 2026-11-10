//! 代表要素から直接の面の表示を取得する。合成入力と再配布できる公開サンプルだけを使う。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{
    Building, ColourOrFactor, Hash, ReflectanceMethod, SpecularHighlight, SurfaceSide,
};
use zumen_ifc::{Ifc, IfcFile, ItemSurfaceStyles, ProductShape, ShapeNode};
use zumen_json::Writer;
use zumen_sdk::{Codec, Error, Lift};

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];

fn source(schema: &str) -> String {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\nFILE_NAME('','',(''),(''),'','','');\nFILE_SCHEMA(('{schema}'));\nENDSEC;\nDATA;\n\
    #1=IFCPROJECT('0000000000000000000001',$,'Project',$,$,$,$,(#8),#9);\n\
    #8=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-5,#20,$);\n\
    #9=IFCUNITASSIGNMENT((#10));\n\
    #10=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n\
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
    #300=IFCSTYLEDITEM(#30,({style}),'Item style');\n\
    #301=IFCSURFACESTYLE('Surface',.BOTH.,(#302));\n\
    #302=IFCSURFACESTYLERENDERING(#303,0.25,#304,IFCNORMALISEDRATIOMEASURE(0.3),$,IFCNORMALISEDRATIOMEASURE(0.4),IFCNORMALISEDRATIOMEASURE(0.5),IFCSPECULAREXPONENT(128.),.PHONG.);\n\
    #303=IFCCOLOURRGB('Base',0.1,0.2,0.3);\n\
    #304=IFCCOLOURRGB('Diffuse',0.4,0.5,0.6);\n\
    #305=IFCSURFACESTYLESHADING(#303{transparency});\n\
    #306=IFCSURFACESTYLE($,.NEGATIVE.,(#305));\n\
    #307=IFCPRESENTATIONSTYLEASSIGNMENT((#301));\n\
    ENDSEC;\nEND-ISO-10303-21;\n",
        wall_type = if schema == "IFC2X3" {
            ""
        } else {
            ",.NOTDEFINED."
        },
        style = if schema == "IFC2X3" { "#307" } else { "#301" },
        transparency = if schema == "IFC2X3" { "" } else { ",$" },
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
fn styles(file: &IfcFile) -> ItemSurfaceStyles<'_> {
    let shape = product(file, 100);
    let (result, diag, _) = with(Limits::default(), |cx| {
        file.surface_styles(leaf(&shape, 30), cx)
    });
    assert!(diag.is_empty(), "{diag:?}");
    result.unwrap()
}

#[test]
fn follows_the_existing_element_path_and_keeps_all_rendering_attributes() {
    for schema in VERSIONS {
        let file = read(&source(schema));
        let result = styles(&file);
        assert_eq!(result.name.as_deref(), Some("Item style"));
        assert_eq!(result.styles.len(), 1);
        let value = &result.styles[0].definition;
        assert_eq!(value.name.as_deref(), Some("Surface"));
        assert_eq!(value.side, SurfaceSide::Both);
        assert_eq!(value.shading.colour.name.as_deref(), Some("Base"));
        assert_eq!(value.shading.colour.rgb, [0.1, 0.2, 0.3]);
        assert_eq!(value.shading.transparency, Some(0.25));
        let rendering = value.shading.rendering.as_ref().unwrap();
        assert!(
            matches!(&rendering.diffuse,Some(ColourOrFactor::Colour(c)) if c.name.as_deref()==Some("Diffuse") && c.rgb==[0.4,0.5,0.6])
        );
        assert_eq!(rendering.transmission, Some(ColourOrFactor::Factor(0.3)));
        assert_eq!(rendering.diffuse_transmission, None);
        assert_eq!(rendering.reflection, Some(ColourOrFactor::Factor(0.4)));
        assert_eq!(rendering.specular, Some(ColourOrFactor::Factor(0.5)));
        assert_eq!(rendering.highlight, Some(SpecularHighlight::Exponent(128.)));
        assert_eq!(rendering.method, ReflectanceMethod::Phong);
        assert_eq!(result.source.item.id(), Some(30));
        assert_eq!(result.source.styled_item.unwrap().id(), Some(300));
        assert_eq!(
            result.source.assignments.len(),
            usize::from(schema == "IFC2X3")
        );
        let origin = &result.styles[0].source;
        assert!(std::ptr::eq(origin.style, file.instance(301).unwrap()));
        assert_eq!(origin.shading.id(), Some(302));
        assert_eq!(
            origin
                .colours
                .iter()
                .map(|r| r.id().unwrap())
                .collect::<Vec<_>>(),
            [303, 304]
        );
        let shape = product(&file, 100);
        let empty = with(Limits::default(), |cx| {
            file.surface_styles(leaf(&shape, 40), cx)
        })
        .0
        .unwrap();
        assert!(empty.styles.is_empty());
        assert!(empty.source.styled_item.is_none());
    }
}

#[test]
fn keeps_optional_shading_values_both_sides_and_legacy_assignment_without_defaults() {
    for schema in VERSIONS {
        let base = source(schema);
        let src = if schema == "IFC2X3" {
            base.replace("((#301));", "((#301,#306));")
        } else {
            base.replace("#30,(#301)", "#30,(#301,#306)")
        };
        let file = read(&src);
        let result = styles(&file);
        assert_eq!(result.styles.len(), 2);
        assert_eq!(result.styles[1].definition.name, None);
        assert_eq!(result.styles[1].definition.side, SurfaceSide::Negative);
        assert_eq!(result.styles[1].definition.shading.transparency, None);
        assert_eq!(result.styles[1].definition.shading.rendering, None);
        let explicit = base.replace(
            "#301=IFCSURFACESTYLE('Surface',.BOTH.,(#302))",
            "#301=IFCSURFACESTYLE('Surface',.POSITIVE.,(#305))",
        );
        let file = read(&explicit);
        assert_eq!(
            styles(&file).styles[0].definition.side,
            SurfaceSide::Positive
        );
    }
    let file = read(&source("IFC4").replace("#30,(#301)", "#30,(#307)"));
    assert_eq!(styles(&file).source.assignments[0].id(), Some(307));
    let file = read(
        &source("IFC4X3_ADD2")
            .replace(".PHONG.", ".PHYSICAL.")
            .replace("IFCSPECULAREXPONENT(128.)", "IFCSPECULARROUGHNESS(0.125)"),
    );
    let v = styles(&file);
    let r = v.styles[0].definition.shading.rendering.as_ref().unwrap();
    assert_eq!(r.method, ReflectanceMethod::Physical);
    assert_eq!(r.highlight, Some(SpecularHighlight::Roughness(0.125)));
    let file =
        read(&source("IFC4").replace("IFCSPECULAREXPONENT(128.)", "IFCSPECULAREXPONENT(-1.)"));
    assert_eq!(
        styles(&file).styles[0]
            .definition
            .shading
            .rendering
            .as_ref()
            .unwrap()
            .highlight,
        Some(SpecularHighlight::Exponent(-1.))
    );
    let file = read(&source("IFC4").replace(".BOTH.", ".both."));
    assert_eq!(styles(&file).styles[0].definition.side, SurfaceSide::Both);
}

#[test]
fn rejects_broken_sets_values_versions_and_references_without_partial_results() {
    let base = source("IFC4");
    for (from, to) in [
        ("#30,(#301)", "#30,()"),
        ("#30,(#301)", "#30,(#301,#301)"),
        (".BOTH.", ".OTHER."),
        ("(#302));", "());"),
        ("(#302));", "(#302,#305));"),
        ("(#302));", "(#302,#302));"),
        ("(#302));", "(#303));"),
        ("#303,0.25", "#999,0.25"),
        ("0.1,0.2,0.3", "-0.1,0.2,0.3"),
        ("#303,0.25", "#303,1.01"),
        ("0.1,0.2,0.3", "1.E999,0.2,0.3"),
        (
            "IFCNORMALISEDRATIOMEASURE(0.3)",
            "IFCNORMALISEDRATIOMEASURE(2.)",
        ),
        ("IFCNORMALISEDRATIOMEASURE(0.3)", "0.3"),
        ("IFCSPECULAREXPONENT(128.)", "IFCSPECULARROUGHNESS(-0.1)"),
        (".PHONG.", ".PHYSICAL."),
        ("#303,0.25", "#303,*"),
        ("#30,(#301),'Item style'", "#30,(#301),42"),
    ] {
        let src = base.replace(from, to);
        let file = read(&src);
        let shape = product(&file, 100);
        let (result, diag, _) = with(Limits::default(), |cx| {
            file.surface_styles(leaf(&shape, 30), cx)
        });
        assert!(
            matches!(result, Err(Error::Invalid)),
            "{from} -> {to}: {diag:?}"
        );
        assert!(
            diag.iter().any(|d| d.code() == codes::IFC_STYLE_INVALID),
            "{diag:?}"
        );
    }
    for (schema, from, to) in [
        ("IFC2X3", "#30,(#307)", "#30,(#301)"),
        ("IFC4X3_ADD2", "#30,(#301)", "#30,(#307)"),
        ("IFC2X3", "#30,(#307)", "#30,(#307,#301)"),
    ] {
        let file = read(&source(schema).replace(from, to));
        let shape = product(&file, 100);
        assert!(matches!(
            with(Limits::default(), |cx| file
                .surface_styles(leaf(&shape, 30), cx))
            .0,
            Err(Error::Invalid)
        ));
    }
    let file = read(&base.replace(
        "#300=IFCSTYLEDITEM",
        "#299=IFCSTYLEDITEM(#30,(#301),$);\n#300=IFCSTYLEDITEM",
    ));
    let shape = product(&file, 100);
    assert!(matches!(
        with(Limits::default(), |cx| file
            .surface_styles(leaf(&shape, 30), cx))
        .0,
        Err(Error::Invalid)
    ));
}

#[test]
fn reports_unsupported_style_kinds_including_legal_null_and_additional_surface_elements() {
    for (schema, from, to, kind) in [
        (
            "IFC4",
            "#301=IFCSURFACESTYLE('Surface',.BOTH.,(#302))",
            "#301=IFCCURVESTYLE($,$,$,$,$)",
            "IFCCURVESTYLE",
        ),
        (
            "IFC4",
            "#302=IFCSURFACESTYLERENDERING(#303,0.25,#304,IFCNORMALISEDRATIOMEASURE(0.3),$,IFCNORMALISEDRATIOMEASURE(0.4),IFCNORMALISEDRATIOMEASURE(0.5),IFCSPECULAREXPONENT(128.),.PHONG.)",
            "#302=IFCSURFACESTYLELIGHTING(#303,#303,#303,#303)",
            "IFCSURFACESTYLELIGHTING",
        ),
        (
            "IFC4",
            "(#302));",
            "(#302,#309));\n#309=IFCSURFACESTYLEWITHTEXTURES(());",
            "IFCSURFACESTYLEWITHTEXTURES",
        ),
        (
            "IFC2X3",
            "((#301));",
            "((IFCNULLSTYLE(.NULL.)));",
            "IFCNULLSTYLE",
        ),
    ] {
        let file = read(&source(schema).replace(from, to));
        let shape = product(&file, 100);
        let (r, d, _) = with(Limits::default(), |cx| {
            file.surface_styles(leaf(&shape, 30), cx)
        });
        assert!(matches!(r, Err(Error::Unsupported)), "{d:?}");
        let diagnostic = d
            .iter()
            .find(|d| d.code() == codes::IFC_STYLE_UNSUPPORTED)
            .unwrap();
        assert!(format!("{diagnostic:?}").contains(kind));
        assert!(diagnostic.places()[0].bytes().is_some());
    }
}

#[test]
fn separates_meaning_from_step_numbers_and_keeps_existing_building_hashes() {
    let src = source("IFC4");
    let file = read(&src);
    let first = styles(&file);
    let shape = product(&file, 100);
    let clone = file.clone();
    assert!(matches!(
        with(Limits::default(), |cx| clone
            .surface_styles(leaf(&shape, 30), cx))
        .0,
        Err(Error::Invalid)
    ));
    let renumbered = src
        .replace("#301", "#1301")
        .replace("#302", "#1302")
        .replace("#303", "#1303")
        .replace("#304", "#1304");
    let other = read(&renumbered);
    let second = styles(&other);
    assert_eq!(first.styles[0].definition, second.styles[0].definition);
    assert_ne!(
        first.styles[0].source.style.id(),
        second.styles[0].source.style.id()
    );
    let changed = read(&src.replace("0.1,0.2,0.3", "0.9,0.2,0.3"));
    assert_ne!(
        first.styles[0].definition,
        styles(&changed).styles[0].definition
    );
    let b = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
        .0
        .unwrap();
    let hash = Hash::document_values(&b);
    let elements = b.elements().iter().map(|e| e.id()).collect::<Vec<_>>();
    let _ = styles(&file);
    let again = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
        .0
        .unwrap();
    assert_eq!(hash, Hash::document_values(&again));
    for i in 0..b.elements().len() {
        let element = zumen_building::ElementRef(i as u32);
        assert_eq!(Hash::element(&b, element), Hash::element(&again, element));
    }
    assert_eq!(
        elements,
        again.elements().iter().map(|e| e.id()).collect::<Vec<_>>()
    );
    assert!(again.components.style.iter().next().is_none());
}

#[test]
fn gets_the_public_column_surface_without_reading_step_arguments_in_the_caller() {
    let file = read(include_str!("../../../samples/ifc/gni/model_9.ifc"));
    let shape = product(&file, 131);
    let (result, diag, _) = with(Limits::default(), |cx| {
        file.surface_styles(leaf(&shape, 114), cx)
    });
    assert!(diag.is_empty(), "{diag:?}");
    let result = result.unwrap();
    assert_eq!(result.source.styled_item.unwrap().id(), Some(118));
    let value = &result.styles[0];
    assert_eq!(value.source.style.id(), Some(117));
    assert_eq!(value.source.shading.id(), Some(116));
    assert_eq!(value.source.colours[0].id(), Some(115));
    assert_eq!(value.definition.name.as_deref(), Some("StB - Ortbeton"));
    assert_eq!(value.definition.side, SurfaceSide::Both);
    assert_eq!(value.definition.shading.colour.rgb, [0.968627450980392; 3]);
    assert_eq!(value.definition.shading.transparency, Some(0.));
    let rendering = value.definition.shading.rendering.as_ref().unwrap();
    assert_eq!(rendering.specular, Some(ColourOrFactor::Factor(0.5)));
    assert_eq!(rendering.highlight, Some(SpecularHighlight::Exponent(128.)));
    assert_eq!(rendering.method, ReflectanceMethod::NotDefined);
}

#[test]
fn reserves_before_allocation_and_stops_at_every_kind_boundary() {
    for schema in VERSIONS {
        let file = read(&source(schema));
        let shape = product(&file, 100);
        let node = leaf(&shape, 30);
        let (value, diag, full) = with(Limits::default(), |cx| file.surface_styles(node, cx));
        assert!(value.is_ok());
        assert!(diag.is_empty());
        assert_eq!(full.used(Kind::Depth), 0);
        for kind in Kind::ALL {
            if kind == Kind::Depth {
                let peak = (0..16)
                    .find(|&limit| {
                        with(Limits::default().with(kind, Limit::Finite(limit)), |cx| {
                            file.surface_styles(node, cx)
                        })
                        .0
                        .is_ok()
                    })
                    .unwrap();
                assert!(peak > 0);
                let (r, _, b) = with(
                    Limits::default().with(kind, Limit::Finite(peak - 1)),
                    |cx| file.surface_styles(node, cx),
                );
                assert!(
                    matches!(r,Err(Error::Stopped(zumen_base::Stop::Exceeded(e))) if e.kind==kind)
                );
                assert_eq!(b.used(Kind::Depth), 0);
                continue;
            }
            let used = full.used(kind);
            let (r, _, b) = with(Limits::default().with(kind, Limit::Finite(used)), |cx| {
                file.surface_styles(node, cx)
            });
            assert!(r.is_ok(), "{kind:?}");
            assert_eq!(b.used(kind), used);
            assert_eq!(b.used(Kind::Depth), 0);
            if used > 0 {
                let (r, _, b) = with(
                    Limits::default().with(kind, Limit::Finite(used - 1)),
                    |cx| file.surface_styles(node, cx),
                );
                assert!(
                    matches!(r,Err(Error::Stopped(zumen_base::Stop::Exceeded(e))) if e.kind==kind),
                    "{kind:?}"
                );
                assert_eq!(b.used(Kind::Depth), 0);
            }
        }
        let (r, _, b) = with(
            Limits::default().with(Kind::DecodedBytes, Limit::Finite(0)),
            |cx| file.surface_styles(node, cx),
        );
        assert!(matches!(r, Err(Error::Stopped(_))));
        assert_eq!(b.used(Kind::DecodedBytes), 0);
        for expired in [false, true] {
            let (r, _, b) = with(Limits::default(), |cx| {
                if expired {
                    cx.cancel.expire()
                } else {
                    cx.cancel.cancel()
                };
                file.surface_styles(node, cx)
            });
            assert!(matches!(r, Err(Error::Stopped(_))));
            assert_eq!(b.used(Kind::DecodedBytes), 0);
            assert_eq!(b.used(Kind::Work), 0);
        }
        let (_, _, b) = with(Limits::default(), |cx| {
            cx.enter().unwrap();
            file.surface_styles(node, cx).unwrap()
        });
        assert_eq!(b.used(Kind::Depth), 1);
    }
}

#[test]
fn rejects_typed_reference_cycles_and_stops_long_lists_without_partial_values() {
    for (from, to) in [
        ("(#302));", "(#301));"),
        ("#303,0.25", "#302,0.25"),
        ("#30,(#301)", "#30,(#307)"),
    ] {
        let src = source("IFC4")
            .replace(from, to)
            .replace("((#301));", "((#307));");
        let file = read(&src);
        let shape = product(&file, 100);
        let (r, _, b) = with(Limits::default(), |cx| {
            file.surface_styles(leaf(&shape, 30), cx)
        });
        assert!(matches!(r, Err(Error::Invalid)));
        assert_eq!(b.used(Kind::Depth), 0);
    }
    let mut src = source("IFC4").replace("#30,(#301)", "#30,(#307)");
    let mut definitions = String::new();
    let mut list = String::new();
    for id in 1000..1256 {
        if !list.is_empty() {
            list.push(',')
        };
        list.push_str(&format!("#{id}"));
        definitions.push_str(&format!("#{id}=IFCSURFACESTYLE($,.BOTH.,(#302));\n"));
    }
    src = src.replace(
        "#307=IFCPRESENTATIONSTYLEASSIGNMENT((#301));",
        &format!("#307=IFCPRESENTATIONSTYLEASSIGNMENT(({list}));\n{definitions}"),
    );
    let file = read(&src);
    let shape = product(&file, 100);
    let (r, _, b) = with(
        Limits::default().with(Kind::DecodedBytes, Limit::Finite(20000)),
        |cx| file.surface_styles(leaf(&shape, 30), cx),
    );
    assert!(matches!(r, Err(Error::Stopped(_))));
    assert_eq!(b.used(Kind::Depth), 0);
    let result = with(Limits::default(), |cx| {
        file.surface_styles(leaf(&shape, 30), cx)
    })
    .0
    .unwrap();
    assert_eq!(result.styles.len(), 256);
    assert!(std::ptr::eq(
        result.styles[0].source.shading,
        result.styles[255].source.shading
    ));
}

#[test]
fn mutated_inputs_finish_with_repeatable_diagnostics_and_usage() {
    let src = source("IFC4");
    for at in (0..src.len()).step_by(41) {
        let mut input = src.as_bytes().to_vec();
        input[at] = b'$';
        zumen_sdk::testkit::survives_with(&input, |bytes, cx| {
            let file = Ifc::read(bytes, cx)?;
            let shape = match file.product_shape(100, cx) {
                Ok(s) => s,
                Err(Error::Stopped(s)) => return Err(Error::Stopped(s)),
                _ => return Ok(()),
            };
            if let Some(node) = shape.nodes.iter().find(|n| n.source.id() == Some(30))
                && let Err(Error::Stopped(stop)) = file.surface_styles(node, cx)
            {
                return Err(Error::Stopped(stop));
            }
            Ok(())
        })
        .unwrap();
    }
}

fn optional_text(out: &mut Writer, value: Option<&str>) {
    if let Some(v) = value {
        out.string(v)
    } else {
        out.null()
    }
}
fn number(out: &mut Writer, value: f64, bits: &mut Vec<u64>) {
    out.float(value);
    bits.push(value.to_bits());
}
fn colour_json(out: &mut Writer, value: &zumen_building::SurfaceColour, bits: &mut Vec<u64>) {
    out.begin_object();
    out.key("name");
    optional_text(out, value.name.as_deref());
    out.key("rgb");
    out.begin_array();
    for v in value.rgb {
        number(out, v, bits);
    }
    out.end_array();
    out.end_object();
}
fn select_json(out: &mut Writer, value: Option<&ColourOrFactor>, bits: &mut Vec<u64>) {
    match value {
        None => out.null(),
        Some(ColourOrFactor::Colour(c)) => {
            out.begin_object();
            out.key("colour");
            colour_json(out, c, bits);
            out.end_object();
        }
        Some(ColourOrFactor::Factor(f)) => {
            out.begin_object();
            out.key("factor");
            number(out, *f, bits);
            out.end_object();
        }
    }
}
fn surface_json(out: &mut Writer, value: &zumen_ifc::SurfaceStyleBinding<'_>, bits: &mut Vec<u64>) {
    out.begin_object();
    out.key("style");
    out.uint(value.source.style.id().unwrap());
    out.key("shading");
    out.uint(value.source.shading.id().unwrap());
    out.key("colour_records");
    out.begin_array();
    for record in &value.source.colours {
        out.uint(record.id().unwrap());
    }
    out.end_array();
    let d = &value.definition;
    out.key("name");
    optional_text(out, d.name.as_deref());
    out.key("side");
    out.string(&format!("{:?}", d.side).to_ascii_uppercase());
    out.key("colour");
    colour_json(out, &d.shading.colour, bits);
    out.key("transparency");
    if let Some(v) = d.shading.transparency {
        number(out, v, bits)
    } else {
        out.null()
    };
    out.key("rendering");
    if let Some(r) = &d.shading.rendering {
        out.begin_object();
        for (k, v) in [
            ("diffuse", &r.diffuse),
            ("transmission", &r.transmission),
            ("diffuse_transmission", &r.diffuse_transmission),
            ("reflection", &r.reflection),
            ("specular", &r.specular),
        ] {
            out.key(k);
            select_json(out, v.as_ref(), bits);
        }
        out.key("highlight");
        match r.highlight {
            None => out.null(),
            Some(SpecularHighlight::Exponent(v)) => {
                out.begin_object();
                out.key("exponent");
                number(out, v, bits);
                out.end_object();
            }
            Some(SpecularHighlight::Roughness(v)) => {
                out.begin_object();
                out.key("roughness");
                number(out, v, bits);
                out.end_object();
            }
        }
        out.key("method");
        out.string(&format!("{:?}", r.method).to_ascii_uppercase());
        out.end_object();
    } else {
        out.null()
    };
    out.end_object();
}
fn write_styles(out: &mut Writer, file: &IfcFile, product_id: u64, item_id: u64) {
    let shape = product(file, product_id);
    let (result, diag, budget) = with(Limits::default(), |cx| {
        file.surface_styles(leaf(&shape, item_id), cx)
    });
    assert!(diag.is_empty(), "{diag:?}");
    let result = result.unwrap();
    let mut bits = Vec::new();
    out.begin_object();
    out.key("instance");
    out.uint(item_id);
    out.key("styled_item");
    out.uint(result.source.styled_item.unwrap().id().unwrap());
    out.key("name");
    optional_text(out, result.name.as_deref());
    out.key("assignments");
    out.begin_array();
    for r in &result.source.assignments {
        out.uint(r.id().unwrap());
    }
    out.end_array();
    out.key("styles");
    out.begin_array();
    for s in &result.styles {
        surface_json(out, s, &mut bits);
    }
    out.end_array();
    out.key("bits");
    out.begin_array();
    for value in bits {
        out.uint(value);
    }
    out.end_array();
    out.key("budgets");
    out.begin_array();
    for kind in Kind::ALL {
        out.uint(budget.used(kind));
    }
    out.end_array();
    out.end_object();
}

#[test]
#[allow(clippy::disallowed_methods)] // 外部照合の出力先だけを受け取る
fn writes_styles_for_other_readers() {
    let Some(folder) = std::env::var_os("ZUMEN_IFC_STYLES_OUT") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    for schema in VERSIONS {
        let src = if schema == "IFC2X3" {
            source(schema).replace("((#301));", "((#301,#306));")
        } else {
            source(schema).replace("#30,(#301)", "#30,(#301,#306)")
        };
        let file = read(&src);
        let name = format!("{schema}.ifc");
        std::fs::write(folder.join(&name), &src).unwrap();
        let mut out = Writer::new();
        out.begin_object();
        out.key("file");
        out.string(&name);
        out.key("items");
        out.begin_array();
        write_styles(&mut out, &file, 100, 30);
        out.end_array();
        out.end_object();
        std::fs::write(folder.join(format!("{schema}.json")), out.into_bytes()).unwrap();
    }
    let src = include_str!("../../../samples/ifc/gni/model_9.ifc");
    let file = read(src);
    std::fs::write(folder.join("public.ifc"), src).unwrap();
    let mut out = Writer::new();
    out.begin_object();
    out.key("file");
    out.string("public.ifc");
    out.key("items");
    out.begin_array();
    write_styles(&mut out, &file, 131, 114);
    out.end_array();
    out.end_object();
    std::fs::write(folder.join("public.json"), out.into_bytes()).unwrap();
    std::fs::write(folder.join("contract.txt"), contract_export()).unwrap();
}

#[test]
fn keeps_unicode_names_omission_signed_zero_and_each_colour_select() {
    let base = source("IFC4");
    let file = read(
        &base
            .replace("'Surface'", "'\\X2\\8868793A\\X0\\'")
            .replace("#303,0.25", "#303,-0."),
    );
    let result = styles(&file);
    assert_eq!(result.styles[0].definition.name.as_deref(), Some("表示"));
    assert_eq!(
        result.styles[0]
            .definition
            .shading
            .transparency
            .unwrap()
            .to_bits(),
        (-0.0_f64).to_bits()
    );
    let file = read(&base.replace("#303,0.25", "#303,$"));
    assert_eq!(
        styles(&file).styles[0].definition.shading.transparency,
        None
    );
    for (from, at) in [
        ("#304", 0),
        ("IFCNORMALISEDRATIOMEASURE(0.3)", 1),
        ("IFCNORMALISEDRATIOMEASURE(0.4)", 2),
        ("IFCNORMALISEDRATIOMEASURE(0.5)", 3),
    ] {
        let src = if from == "#304" {
            base.replace(",#304,", ",IFCNORMALISEDRATIOMEASURE(0.75),")
        } else {
            base.replace(from, "#304")
        };
        let file = read(&src);
        let result = styles(&file);
        let r = result.styles[0]
            .definition
            .shading
            .rendering
            .as_ref()
            .unwrap();
        let value = [&r.diffuse, &r.transmission, &r.reflection, &r.specular][at];
        if at == 0 {
            assert_eq!(*value, Some(ColourOrFactor::Factor(0.75)));
        } else {
            assert!(matches!(value,Some(ColourOrFactor::Colour(c)) if c.rgb==[0.4,0.5,0.6]));
        }
    }
    let file = read(&base.replace(
        "(0.3),$,IFCNORMALISEDRATIOMEASURE(0.4)",
        "(0.3),#304,IFCNORMALISEDRATIOMEASURE(0.4)",
    ));
    assert!(
        matches!(&styles(&file).styles[0].definition.shading.rendering.as_ref().unwrap().diffuse_transmission,Some(ColourOrFactor::Colour(c)) if c.rgb==[0.4,0.5,0.6])
    );
}

#[test]
fn budgets_the_unsupported_diagnostic_before_allocating_its_kind() {
    let src = source("IFC4").replace(
        "#302=IFCSURFACESTYLERENDERING",
        "#302=IFCSURFACESTYLELIGHTING",
    );
    let file = read(&src);
    let shape = product(&file, 100);
    let node = leaf(&shape, 30);
    let (r, diag, full) = with(Limits::default(), |cx| file.surface_styles(node, cx));
    assert!(matches!(r, Err(Error::Unsupported)));
    assert_eq!(diag.len(), 1);
    for kind in [Kind::Work, Kind::DecodedBytes] {
        let (r, diag, b) = with(
            Limits::default().with(kind, Limit::Finite(full.used(kind) - 1)),
            |cx| file.surface_styles(node, cx),
        );
        assert!(matches!(r, Err(Error::Stopped(_))));
        assert!(
            diag.iter()
                .all(|d| d.code() != codes::IFC_STYLE_UNSUPPORTED)
        );
        assert_eq!(b.used(Kind::Depth), 0);
    }
}

fn contract_line(
    file: &IfcFile,
    node: &ShapeNode<'_>,
    label: &str,
    limits: Limits,
    signal: u8,
) -> String {
    let (result, diag, budget) = with(limits, |cx| {
        match signal {
            1 => cx.cancel.cancel(),
            2 => cx.cancel.expire(),
            _ => {}
        }
        file.surface_styles(node, cx)
    });
    let status = match result {
        Ok(_) => "ok".to_owned(),
        Err(e) => format!("{e:?}"),
    };
    let used = Kind::ALL.map(|kind| budget.used(kind));
    format!("{label}\t{status}\t{diag:?}\t{used:?}\n")
}
fn contract_export() -> String {
    let mut output = String::new();
    for schema in VERSIONS {
        let src = source(schema);
        let file = read(&src);
        let shape = product(&file, 100);
        let node = leaf(&shape, 30);
        let (_, _, full) = with(Limits::default(), |cx| file.surface_styles(node, cx));
        output.push_str(&contract_line(
            &file,
            node,
            &format!("{schema}/full"),
            Limits::default(),
            0,
        ));
        for kind in Kind::ALL {
            let limit = if kind == Kind::Depth {
                (0..16)
                    .find(|&n| {
                        with(Limits::default().with(kind, Limit::Finite(n)), |cx| {
                            file.surface_styles(node, cx)
                        })
                        .0
                        .is_ok()
                    })
                    .unwrap()
            } else {
                full.used(kind)
            };
            for n in [Some(limit), limit.checked_sub(1)].into_iter().flatten() {
                output.push_str(&contract_line(
                    &file,
                    node,
                    &format!("{schema}/{kind:?}/{n}"),
                    Limits::default().with(kind, Limit::Finite(n)),
                    0,
                ));
            }
        }
        for signal in [1, 2] {
            output.push_str(&contract_line(
                &file,
                node,
                &format!("{schema}/signal/{signal}"),
                Limits::default(),
                signal,
            ));
        }
        let clone = file.clone();
        output.push_str(&contract_line(
            &clone,
            node,
            &format!("{schema}/foreign"),
            Limits::default(),
            0,
        ));
        for (label, from, to) in [
            ("range", "#303,0.25", "#303,1.1"),
            ("cycle", "(#302));", "(#301));"),
            (
                "unsupported",
                "#302=IFCSURFACESTYLERENDERING",
                "#302=IFCSURFACESTYLELIGHTING",
            ),
        ] {
            let file = read(&src.replace(from, to));
            let shape = product(&file, 100);
            output.push_str(&contract_line(
                &file,
                leaf(&shape, 30),
                &format!("{schema}/{label}"),
                Limits::default(),
                0,
            ));
        }
    }
    output
}
