//! 要素と型の材料から表示を継承し、文脈・出所・停止を保つ。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{
    Acquired, ElementRef, Geometry, GeometryError, GeometryLinks, GeometryMaterials, GeometryRef,
    MaterialAssignmentLevel, MaterialContextMatch, ResolvedSurface, SurfaceAppearance,
};
use zumen_ifc::{Ifc, IfcFile};
use zumen_json::Writer;
use zumen_sdk::{Codec, Error};

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];
const CASES: [&str; 12] = [
    "occurrence",
    "type",
    "override",
    "item",
    "parent",
    "exact",
    "unrelated",
    "multiple",
    "layer",
    "empty",
    "unsupported",
    "mapped",
];

fn source(version: &str, case: &str) -> String {
    let old = version == "IFC2X3";
    let extra = if old { "" } else { ",.NOTDEFINED." };
    let material = if old { "" } else { ",$,$" };
    let transparency = if old { "" } else { ",0.25" };
    let (green, blue, red) = if old {
        (303, 313, 323)
    } else {
        (302, 312, 322)
    };
    let body_context = if matches!(case, "parent" | "exact") {
        80
    } else {
        8
    };
    let material_context = if case == "unrelated" { 81 } else { 8 };
    let owner = if matches!(case, "type" | "override" | "empty" | "unsupported") {
        800
    } else {
        100
    };
    let selected = if case == "layer" { 632 } else { 600 };
    let layer_extra = if old { "" } else { ", $,$,$,$" };
    let set_extra = if old { "" } else { ",$" };
    let usage_extra = if old { "" } else { ",$" };
    let mut additions = String::new();
    if matches!(case, "override" | "empty" | "unsupported") {
        let target = match case {
            "empty" => 620,
            "unsupported" => 640,
            _ => 610,
        };
        additions.push_str(&format!(
            "#701=IFCRELASSOCIATESMATERIAL('0000000000000000000013',$,$,$,(#100),#{target});\n"
        ));
    }
    if case == "item" {
        additions.push_str(&format!("#400=IFCSTYLEDITEM(#30,(#{red}),$);\n"));
    }
    if case == "exact" {
        additions.push_str(&format!("#604=IFCSTYLEDREPRESENTATION(#80,'Body','Material',(#605));#605=IFCSTYLEDITEM($,(#{blue}),$);\n"));
    }
    if case == "multiple" {
        additions.push_str(
            "#701=IFCRELASSOCIATESMATERIAL('0000000000000000000013',$,$,$,(#100),#610);\n",
        );
    }
    if case == "mapped" {
        additions.push_str(&format!("#40=IFCMAPPEDITEM(#41,#43);#41=IFCREPRESENTATIONMAP(#20,#42);#42=IFCSHAPEREPRESENTATION(#8,'Body','SweptSolid',(#30));#43=IFCCARTESIANTRANSFORMATIONOPERATOR3D($,$,#21,1.,$);#400=IFCSTYLEDITEM(#40,(#{red}),$);\n"));
    }
    let reps = if case == "exact" { "#602,#604" } else { "#602" };
    let (body_kind, body_item) = if case == "mapped" {
        ("MappedRepresentation", 40)
    } else {
        ("SweptSolid", 30)
    };
    format!(
        "ISO-10303-21;\nHEADER;FILE_DESCRIPTION((''),'2;1');FILE_NAME('','',(''),(''),'','','');FILE_SCHEMA(('{version}'));ENDSEC;\nDATA;\n\
    #1=IFCPROJECT('0000000000000000000001',$,'Project',$,$,$,$,(#8),#9);\n\
    #8=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-5,#20,$);\n\
    #9=IFCUNITASSIGNMENT((#10));#10=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);\n\
    #20=IFCAXIS2PLACEMENT3D(#21,$,$);#21=IFCCARTESIANPOINT((0.,0.,0.));\n\
    #24=IFCDIRECTION((0.,0.,1.));#25=IFCCARTESIANPOINT((0.,0.));#26=IFCAXIS2PLACEMENT2D(#25,$);\n\
    #27=IFCRECTANGLEPROFILEDEF(.AREA.,'R',#26,2.,3.);#30=IFCEXTRUDEDAREASOLID(#27,#20,#24,4.);\n\
    #80=IFCGEOMETRICREPRESENTATIONSUBCONTEXT('Body','Model',*,*,*,*,#8,$,.MODEL_VIEW.,$);\n\
    #81=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-5,#20,$);\n\
    #100=IFCWALL('0000000000000000000002',$,'Wall',$,$,$,#101,${extra});\n\
    #101=IFCPRODUCTDEFINITIONSHAPE($,$,(#102));#102=IFCSHAPEREPRESENTATION(#{body_context},'Body','{body_kind}',(#{body_item}));\n\
    #300=IFCCOLOURRGB('Green',0.,1.,0.);#301=IFCSURFACESTYLESHADING(#300{transparency});#302=IFCSURFACESTYLE('Green',.BOTH.,(#301));#303=IFCPRESENTATIONSTYLEASSIGNMENT((#302));\n\
    #310=IFCCOLOURRGB('Blue',0.,0.,1.);#311=IFCSURFACESTYLESHADING(#310{transparency});#312=IFCSURFACESTYLE('Blue',.BOTH.,(#311));#313=IFCPRESENTATIONSTYLEASSIGNMENT((#312));\n\
    #320=IFCCOLOURRGB('Red',1.,0.,0.);#321=IFCSURFACESTYLESHADING(#320{transparency});#322=IFCSURFACESTYLE('Red',.BOTH.,(#321));#323=IFCPRESENTATIONSTYLEASSIGNMENT((#322));\n\
    #600=IFCMATERIAL('Concrete'{material});#601=IFCMATERIALDEFINITIONREPRESENTATION('Appearance','Green appearance',({reps}),#600);\n\
    #602=IFCSTYLEDREPRESENTATION(#{material_context},'Body','Material',(#603));#603=IFCSTYLEDITEM($,(#{green}),'Green');\n\
    #610=IFCMATERIAL('Steel'{material});#611=IFCMATERIALDEFINITIONREPRESENTATION('Override',$,(#612),#610);\n\
    #612=IFCSTYLEDREPRESENTATION(#8,'Body','Material',(#613));#613=IFCSTYLEDITEM($,(#{blue}),'Blue');\n\
    #620=IFCMATERIAL('No appearance'{material});\n\
    #630=IFCMATERIALLAYER(#600,0.2,.U.{layer_extra});#631=IFCMATERIALLAYERSET((#630),'Layer'{set_extra});\n\
    #632=IFCMATERIALLAYERSETUSAGE(#631,.AXIS2.,.POSITIVE.,0.{usage_extra});#640=IFCMATERIALUNKNOWN();\n\
    #700=IFCRELASSOCIATESMATERIAL('0000000000000000000012',$,$,$,(#{owner}),#{selected});\n\
    #800=IFCWALLTYPE('0000000000000000000010',$,'Type',$,$,$,$,$,$,.NOTDEFINED.);\n\
    #801=IFCRELDEFINESBYTYPE('0000000000000000000011',$,$,$,(#100),#800);\n\
    {additions}ENDSEC;END-ISO-10303-21;\n"
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
fn owned(file: &IfcFile) -> Geometry {
    with(Limits::default(), |cx| file.owned_geometry(100, cx))
        .0
        .unwrap()
}
fn materials(g: &Geometry) -> &GeometryMaterials {
    let Some(Acquired::Value(value)) = g.materials() else {
        panic!()
    };
    value
}
fn path(g: &Geometry, case: &str) -> Vec<GeometryRef> {
    let ids: &[u64] = if case == "mapped" {
        &[101, 102, 40, 41, 42, 30]
    } else {
        &[101, 102, 30]
    };
    ids.iter()
        .map(|id| {
            GeometryRef(
                g.data()
                    .nodes
                    .iter()
                    .position(|n| n.source.instance == Some(*id))
                    .unwrap() as u32,
            )
        })
        .collect()
}
fn selection(g: &Geometry, case: &str) -> (&'static str, Option<[f64; 3]>) {
    let (r, _, b) = with(Limits::default(), |cx| {
        g.resolved_surface(&path(g, case), cx)
    });
    assert_eq!(b.used(Kind::Depth), 0);
    assert_eq!(b.used(Kind::DecodedBytes), 0);
    match r.unwrap() {
        ResolvedSurface::Material { representation, .. } => (
            "material",
            Some(
                representation.items[0].surfaces[0]
                    .definition
                    .shading
                    .colour
                    .rgb,
            ),
        ),
        ResolvedSurface::Item(SurfaceAppearance::Value { surfaces, .. }) => {
            ("item", Some(surfaces[0].definition.shading.colour.rgb))
        }
        ResolvedSurface::Unspecified => ("none", None),
        ResolvedSurface::Unsupported => ("unsupported", None),
        ResolvedSurface::Ambiguous { .. } => ("ambiguous", None),
        ResolvedSurface::Item(SurfaceAppearance::Unsupported { .. }) => ("item-unsupported", None),
        r => panic!("{r:?}"),
    }
}

/// 同じ形と型を参照する 2 枚の壁。材料だけ、レイヤの面指定、写像の面指定を切り替える。
fn shared_walls(version: &str, case: &str) -> String {
    let extra = if version == "IFC2X3" {
        ""
    } else {
        ",.NOTDEFINED."
    };
    let layer = if case == "layer" {
        "#450=IFCPRESENTATIONLAYERWITHSTYLE('Shared',$,(#30),$,.T.,.F.,.F.,(#322));"
    } else {
        ""
    };
    let base = if case == "mapped" { "mapped" } else { "type" };
    source(version, base)
        .replace("(#100),#600", "(#800),#600")
        .replace("(#100),#800", "(#100,#110),#800")
        .replace("'Wall',$,$,$,#101", "'Wall',$,$,#200,#101")
        .replace(
            "ENDSEC;END-ISO-10303-21;",
            &format!(
                "#110=IFCWALL('0000000000000000000003',$,'Sibling',$,$,#200,#101,${extra});\n\
             #200=IFCLOCALPLACEMENT($,#20);\n{layer}\nENDSEC;END-ISO-10303-21;"
            ),
        )
}

fn check_wall_appearance(g: &Geometry, case: &str, assignment: Option<(u64, u64)>, new_axis: bool) {
    if let Some((owner, material)) = assignment {
        let m = materials(g);
        assert_eq!(m.source.instance, Some(owner));
        assert_eq!(m.assignments.len(), 1);
        let members = &m.assignments[0].members;
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].source.instance, Some(material));
    } else {
        assert!(g.materials().is_none());
    }
    if new_axis {
        let data = g.data();
        let root = data.root.unwrap();
        let GeometryLinks::Definition(reps) = &data.nodes[root.0 as usize].links else {
            panic!("形の根に表現がない");
        };
        let [rep] = reps.as_slice() else {
            panic!("芯の表現数")
        };
        let GeometryLinks::Representation {
            identifier, items, ..
        } = &data.nodes[rep.0 as usize].links
        else {
            panic!("芯の表現がない");
        };
        assert_eq!(identifier.as_deref(), Some("Axis"));
        let [line] = items.as_slice() else {
            panic!("芯の線数")
        };
        let (selected, _, _) = with(Limits::default(), |cx| {
            g.resolved_surface(&[root, *rep, *line], cx)
        });
        assert_eq!(selected.unwrap(), ResolvedSurface::Unspecified);
        return;
    }
    let appearance = match (case, assignment) {
        ("layer" | "mapped", _) => ("item", Some([1., 0., 0.])),
        (_, Some((_, 600))) => ("material", Some([0., 1., 0.])),
        (_, Some((_, 610))) => ("material", Some([0., 0., 1.])),
        (_, None) => ("none", None),
        (_, Some((_, material))) => panic!("想定していない材料 #{material}"),
    };
    assert_eq!(selection(g, case), appearance);
}

#[test]
fn live_edits_follow_materials_and_presentation_on_two_walls() {
    use zumen_ifc::{Edit, IfcLive, NewMaterial, NewShape, Target};

    let edits = [
        Edit::Material {
            element: Target::Instance(800),
            material: NewMaterial::Single("Steel".into()),
        },
        Edit::Material {
            element: Target::Instance(110),
            material: NewMaterial::Single("Concrete".into()),
        },
        Edit::Type {
            element: Target::Instance(100),
            type_of: None,
        },
        Edit::Shape {
            element: Target::Instance(100),
            shape: NewShape {
                axis: Some(vec![[0., 0.], [2., 0.]]),
                body: vec![],
            },
        },
    ];
    // ある段で各壁が参照する材料の所有者と材料の番号。2 枚目への直接割り当ては型より優先する。
    let expected = [
        [Some((800, 600)), Some((800, 600))],
        [Some((800, 610)), Some((800, 610))],
        [Some((800, 610)), Some((110, 600))],
        [None, Some((110, 600))],
        [None, Some((110, 600))],
    ];
    for version in VERSIONS {
        for case in ["type", "layer", "mapped"] {
            let input = shared_walls(version, case);
            let mut live = with(Limits::default(), |cx| {
                IfcLive::open_with_geometry(input.as_bytes(), 1, cx)
            })
            .0
            .unwrap();
            let mut saved = vec![input.into_bytes()];
            let check = |live: &IfcLive, bytes: &[u8], stage: usize| {
                assert_eq!(
                    with(Limits::default(), |cx| live.save_spf(cx)).0.unwrap(),
                    bytes
                );
                let file = with(Limits::default(), |cx| Ifc::read(bytes, cx))
                    .0
                    .unwrap();
                let (model, diagnostics, _) = with(Limits::default(), |cx| file.lift_geometry(cx));
                assert_eq!(live.building(), &model.unwrap());
                let (reported, actual, _) =
                    with(Limits::default(), |cx| live.report_lift_diagnostics(cx));
                reported.unwrap();
                assert_eq!(actual.into_vec(), diagnostics.into_vec());
                for (id, assignment) in [100, 110].into_iter().zip(expected[stage]) {
                    let g = live
                        .building()
                        .geometry(live.element_by_instance(id).unwrap())
                        .unwrap();
                    check_wall_appearance(g, case, assignment, stage == edits.len() && id == 100);
                }
            };
            check(&live, &saved[0], 0);
            for (i, edit) in edits.iter().enumerate() {
                let file = with(Limits::default(), |cx| Ifc::read(&saved[i], cx))
                    .0
                    .unwrap();
                let edit = std::slice::from_ref(edit);
                let (batch, expected_notes, _) = with(Limits::default(), |cx| {
                    zumen_ifc::apply(&file, edit, b"shared", cx)
                });
                let batch = batch.unwrap();
                let (changed, notes, _) =
                    with(Limits::default(), |cx| live.apply(edit, b"shared", cx));
                assert_eq!(changed.unwrap(), batch.changed);
                assert_eq!(notes.into_vec(), expected_notes.into_vec());
                saved.push(batch.bytes);
                check(&live, &saved[i + 1], i + 1);
            }
            for stage in (0..edits.len()).rev() {
                assert!(with(Limits::default(), |cx| live.undo(cx)).0.unwrap());
                check(&live, &saved[stage], stage);
            }
            for (stage, bytes) in saved.iter().enumerate().skip(1) {
                assert!(with(Limits::default(), |cx| live.redo(cx)).0.unwrap());
                check(&live, bytes, stage);
            }
        }
    }
}

#[test]
fn occurrence_overrides_type_and_retains_assignment_sources() {
    for version in VERSIONS {
        for (case, owner, relation, material) in [
            ("occurrence", 100, 700, 600),
            ("type", 800, 700, 600),
            ("override", 100, 701, 610),
            ("empty", 100, 701, 620),
            ("unsupported", 100, 701, 640),
        ] {
            let file = read(&source(version, case));
            let (r, d, b) = with(Limits::default(), |cx| file.material_assignments(100, cx));
            assert!(d.is_empty());
            assert_eq!(b.used(Kind::Depth), 0);
            let a = r.unwrap();
            assert_eq!(a.product.id(), Some(100));
            assert_eq!(a.owner.id(), Some(owner));
            assert_eq!(
                a.type_relation.map(|r| r.id()),
                if owner == 800 { Some(Some(801)) } else { None }
            );
            assert_eq!(a.assignments.len(), 1);
            assert_eq!(a.assignments[0].source.id(), Some(relation));
            assert_eq!(a.assignments[0].selection.id(), Some(material));
            let g = owned(&file);
            let m = materials(&g);
            assert_eq!(
                m.level,
                if owner == 800 {
                    MaterialAssignmentLevel::Type
                } else {
                    MaterialAssignmentLevel::Occurrence
                }
            );
            assert_eq!(m.source.instance, Some(owner));
            assert_eq!(m.assignments[0].members[0].source.instance, Some(material));
        }
    }
}

#[test]
fn selects_material_context_and_preserves_item_priority_and_ambiguity() {
    for version in VERSIONS {
        for case in CASES {
            let g = owned(&read(&source(version, case)));
            let expected = match case {
                "occurrence" | "type" | "parent" | "layer" => ("material", Some([0., 1., 0.])),
                "override" | "exact" => ("material", Some([0., 0., 1.])),
                "item" | "mapped" => ("item", Some([1., 0., 0.])),
                "empty" | "unrelated" => ("none", None),
                "multiple" => ("ambiguous", None),
                "unsupported" => ("unsupported", None),
                _ => panic!(),
            };
            assert_eq!(selection(&g, case), expected, "{version}/{case}");
            let m = materials(&g);
            if case == "parent" {
                let Some(Acquired::Value(p)) = &m.assignments[0].members[0].presentation else {
                    panic!()
                };
                assert_eq!(
                    p.representations[0].bindings[0].context,
                    MaterialContextMatch::Parent
                );
            }
            if case == "exact" {
                let mut m = m.clone();
                let Some(Acquired::Value(p)) = &mut m.assignments[0].members[0].presentation else {
                    panic!()
                };
                p.representations.reverse();
                let changed = with(Limits::default(), |cx| {
                    Geometry::new_with_materials(g.data().clone(), Some(Acquired::Value(m)), cx)
                })
                .0
                .unwrap();
                assert_eq!(selection(&changed, case), expected);
                let mut m = materials(&changed).clone();
                let Some(Acquired::Value(p)) = &mut m.assignments[0].members[0].presentation else {
                    panic!()
                };
                p.representations.push(p.representations[0].clone());
                let ambiguous = with(Limits::default(), |cx| {
                    Geometry::new_with_materials(
                        changed.data().clone(),
                        Some(Acquired::Value(m)),
                        cx,
                    )
                })
                .0
                .unwrap();
                assert_eq!(selection(&ambiguous, case), ("ambiguous", None));
            }
        }
    }
}

#[test]
fn compound_members_keep_order_missing_materials_and_unhandled_values() {
    for version in VERSIONS {
        let src = source(version, "layer");
        let g = owned(&read(&src));
        let member = &materials(&g).assignments[0].members[0];
        assert_eq!(
            member
                .source
                .groups
                .iter()
                .find(|g| g.role == "route")
                .unwrap()
                .instances,
            [632, 631, 630]
        );
        let missing = owned(&read(
            &src.replace("IFCMATERIALLAYER(#600,", "IFCMATERIALLAYER($,"),
        ));
        assert_eq!(selection(&missing, "layer"), ("none", None));
        let multiple = owned(&read(&src.replace(
            "IFCMATERIALLAYERSET((#630)",
            "IFCMATERIALLAYERSET((#630,#630)",
        )));
        assert_eq!(materials(&multiple).assignments[0].members.len(), 2);
        assert_eq!(selection(&multiple, "layer"), ("ambiguous", None));
        let list = source(version, "occurrence")
            .replace("(#100),#600);", "(#100),#650);")
            .replace(
                "ENDSEC;END-ISO-10303-21;",
                "#650=IFCMATERIALLIST((#600,#610,#600));ENDSEC;END-ISO-10303-21;",
            );
        let list = owned(&read(&list));
        assert_eq!(
            materials(&list).assignments[0]
                .members
                .iter()
                .map(|m| m.source.instance.unwrap())
                .collect::<Vec<_>>(),
            [600, 610, 600]
        );
        assert_eq!(selection(&list, "occurrence"), ("ambiguous", None));
        let unknown = src.replace("IFCMATERIALLAYER(#600,", "IFCMATERIALLAYER(#640,");
        let (r, d, _) = with(Limits::default(), |cx| {
            read(&unknown).owned_geometry(100, cx)
        });
        assert_eq!(selection(&r.unwrap(), "layer"), ("unsupported", None));
        assert!(d.iter().any(|d| d.code() == codes::IFC_STYLE_UNSUPPORTED));
    }
}

fn renumber(src: &str) -> String {
    let mut out = String::new();
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        out.push(c);
        if c == '#' {
            let mut n = 0u64;
            while let Some(c) = chars.peek().copied().filter(char::is_ascii_digit) {
                n = n * 10 + u64::from(c as u8 - b'0');
                chars.next();
            }
            out.push_str(&(n + 1000).to_string());
        }
    }
    out
}

#[test]
fn owns_material_values_in_building_and_hashes_meaning_and_context_links() {
    for version in VERSIONS {
        let src = source(version, "type");
        let file = read(&src);
        let g = owned(&file);
        let building = with(Limits::default(), |cx| file.lift_geometry(cx))
            .0
            .unwrap();
        let element = building
            .elements()
            .iter()
            .position(|e| e.source().instance == Some(100))
            .unwrap();
        assert_eq!(building.geometry(ElementRef(element as u32)).unwrap(), &g);
        assert!(format!("{g:?}").contains("Green appearance"));
        let changed = read(&renumber(&src));
        let changed = with(Limits::default(), |cx| changed.owned_geometry(1100, cx))
            .0
            .unwrap();
        assert_eq!(g.fingerprint(), changed.fingerprint());
        let colour = owned(&read(&src.replace("'Green',0.,1.,0.", "'Green',0.,0.5,0.")));
        assert_ne!(g.fingerprint(), colour.fingerprint());
        let unrelated = owned(&read(&src.replace(
            "#602=IFCSTYLEDREPRESENTATION(#8,",
            "#602=IFCSTYLEDREPRESENTATION(#81,",
        )));
        let Some(Acquired::Value(a)) = &materials(&g).assignments[0].members[0].presentation else {
            panic!()
        };
        let Some(Acquired::Value(b)) =
            &materials(&unrelated).assignments[0].members[0].presentation
        else {
            panic!()
        };
        assert_eq!(a.representations[0].context, b.representations[0].context);
        assert_ne!(g.fingerprint(), unrelated.fingerprint());
        let before = g.fingerprint();
        selection(&g, "type");
        assert_eq!(g.fingerprint(), before);
        let mut data = g.data().clone();
        let mut m = materials(&g).clone();
        let count = data.nodes.len() as u32;
        let change = |r: &mut GeometryRef| r.0 = count - 1 - r.0;
        data.nodes.reverse();
        if let Some(r) = &mut data.root {
            change(r)
        };
        for node in &mut data.nodes {
            match &mut node.links {
                GeometryLinks::Definition(v)
                | GeometryLinks::Representation { items: v, .. }
                | GeometryLinks::Set { items: v, .. } => {
                    for r in v {
                        change(r)
                    }
                }
                GeometryLinks::Mapped(r) | GeometryLinks::Map(r) => change(r),
                GeometryLinks::Boolean { first, second, .. } => {
                    change(first);
                    change(second)
                }
                _ => {}
            }
        }
        for s in &mut data.styles {
            change(&mut s.item);
            if let Some(r) = &mut s.representation {
                change(r)
            }
        }
        for a in &mut m.assignments {
            for m in &mut a.members {
                if let Some(Acquired::Value(p)) = &mut m.presentation {
                    for r in &mut p.representations {
                        for b in &mut r.bindings {
                            change(&mut b.representation)
                        }
                    }
                }
            }
        }
        let reordered = with(Limits::default(), |cx| {
            Geometry::new_with_materials(data, Some(Acquired::Value(m)), cx)
        })
        .0
        .unwrap();
        assert_eq!(g.fingerprint(), reordered.fingerprint());
    }
}

#[test]
fn rejects_invalid_relations_and_compositions_without_falling_back() {
    for version in VERSIONS {
        let base = source(version, "type");
        for (from, to) in [
            ("(#800),#600)", "(#800),#9999)"),
            ("(#800),#600)", "(#800,#800),#600)"),
            ("(#100),#800)", "(#100),#9999)"),
            (
                "#700=IFCRELASSOCIATESMATERIAL('0000000000000000000012',$,$,$,(#800),#600);",
                "#700=(IFCRELASSOCIATES((#800)) IFCRELASSOCIATESMATERIAL(#600));",
            ),
            (
                "#801=IFCRELDEFINESBYTYPE('0000000000000000000011',$,$,$,(#100),#800);",
                "#801=(IFCRELDEFINES((#100)) IFCRELDEFINESBYTYPE(#800));",
            ),
            (
                "ENDSEC;END-ISO-10303-21;",
                "#802=IFCRELDEFINESBYTYPE('0000000000000000000014',$,$,$,(#100),#800);ENDSEC;END-ISO-10303-21;",
            ),
        ] {
            assert!(base.contains(from));
            let file = read(&base.replace(from, to));
            let (r, _, b) = with(Limits::default(), |cx| file.owned_geometry(100, cx));
            assert!(matches!(r, Err(Error::Invalid)), "{version}/{to}: {r:?}");
            assert_eq!(b.used(Kind::Depth), 0);
        }
        let base = source(version, "layer");
        for (from, to) in [
            (
                "IFCMATERIALLAYERSETUSAGE(#631,",
                "IFCMATERIALLAYERSETUSAGE(#632,",
            ),
            ("IFCMATERIALLAYERSET((#630)", "IFCMATERIALLAYERSET((#631)"),
            ("IFCMATERIALLAYERSET((#630)", "IFCMATERIALLAYERSET(()"),
            ("IFCMATERIALLAYER(#600,", "IFCMATERIALLAYER(#630,"),
            ("IFCMATERIALLAYER(#600,", "IFCMATERIALLAYER(#9999,"),
        ] {
            assert!(base.contains(from));
            let file = read(&base.replace(from, to));
            assert!(
                matches!(
                    with(Limits::default(), |cx| file.owned_geometry(100, cx)).0,
                    Err(Error::Invalid)
                ),
                "{version}/{to}"
            );
        }
        // 要素にある複合の割り当てを無視して、型の既知の材料を採らない。
        let complex=source(version,"type").replace("ENDSEC;END-ISO-10303-21;","#701=(IFCRELASSOCIATES((#100)) IFCRELASSOCIATESMATERIAL(#610));ENDSEC;END-ISO-10303-21;");
        let file = read(&complex);
        assert!(matches!(
            with(Limits::default(), |cx| file.material_assignments(100, cx)).0,
            Err(Error::Invalid)
        ));
        // 関係しない複合の割り当ては、所有者を確認して通り過ぎる。
        let unrelated = complex.replace("IFCRELASSOCIATES((#100))", "IFCRELASSOCIATES((#9999))");
        assert_eq!(
            selection(&owned(&read(&unrelated)), "type"),
            ("material", Some([0., 1., 0.]))
        );
        let unknown = source(version, "item").replace(
            "#322=IFCSURFACESTYLE('Red',.BOTH.,(#321))",
            "#322=IFCCURVESTYLE('Unknown',$,$,$)",
        );
        assert_eq!(
            selection(&owned(&read(&unknown)), "item"),
            ("item-unsupported", None)
        );
    }
}

fn boundary(file: &IfcFile, kind: Kind) -> u64 {
    if kind != Kind::Depth {
        return with(Limits::default(), |cx| file.owned_geometry(100, cx))
            .2
            .used(kind);
    }
    (0..=64)
        .find(|n| {
            with(Limits::default().with(kind, Limit::Finite(*n)), |cx| {
                file.owned_geometry(100, cx)
            })
            .0
            .is_ok()
        })
        .unwrap()
}

#[test]
fn shares_all_budget_limits_cancel_and_expiry_and_validates_bindings() {
    for version in VERSIONS {
        let file = read(&source(version, "exact"));
        for kind in Kind::ALL {
            let cap = boundary(&file, kind);
            let (r, _, b) = with(Limits::default().with(kind, Limit::Finite(cap)), |cx| {
                file.owned_geometry(100, cx)
            });
            assert!(r.is_ok(), "{version}/{kind:?}/{cap}");
            assert_eq!(b.used(Kind::Depth), 0);
            if cap > 0 {
                let (r, _, b) = with(Limits::default().with(kind, Limit::Finite(cap - 1)), |cx| {
                    file.owned_geometry(100, cx)
                });
                assert!(
                    matches!(r, Err(Error::Stopped(_))),
                    "{version}/{kind:?}/{cap}"
                );
                assert_eq!(b.used(Kind::Depth), 0);
            }
        }
        let g = owned(&file);
        let p = path(&g, "exact");
        for expired in [false, true] {
            let (r, _, b) = with(Limits::default(), |cx| {
                if expired {
                    cx.cancel.expire()
                } else {
                    cx.cancel.cancel()
                };
                file.owned_geometry(100, cx)
            });
            assert!(matches!(r, Err(Error::Stopped(_))));
            assert_eq!(b.used(Kind::DecodedBytes), 0);
            let (r, _, b) = with(Limits::default(), |cx| {
                if expired {
                    cx.cancel.expire()
                } else {
                    cx.cancel.cancel()
                };
                g.resolved_surface(&p, cx)
            });
            assert!(matches!(r, Err(GeometryError::Stopped(_))));
            assert_eq!(b.used(Kind::Depth), 0);
        }
        let required = with(Limits::default(), |cx| g.resolved_surface(&p, cx))
            .2
            .used(Kind::Work);
        let (r, _, b) = with(
            Limits::default().with(Kind::Work, Limit::Finite(required - 1)),
            |cx| g.resolved_surface(&p, cx),
        );
        assert!(matches!(r, Err(GeometryError::Stopped(_))));
        assert_eq!(b.used(Kind::Depth), 0);
        for target in [g.data().root.unwrap(), GeometryRef(u32::MAX)] {
            let mut m = materials(&g).clone();
            let Some(Acquired::Value(v)) = &mut m.assignments[0].members[0].presentation else {
                panic!()
            };
            v.representations[0].bindings[0].representation = target;
            assert!(matches!(
                with(Limits::default(), |cx| Geometry::new_with_materials(
                    g.data().clone(),
                    Some(Acquired::Value(m)),
                    cx
                ))
                .0,
                Err(GeometryError::Reference)
            ));
        }
        let mut m = materials(&g).clone();
        let Some(Acquired::Value(v)) = &mut m.assignments[0].members[0].presentation else {
            panic!()
        };
        let duplicate = v.representations[0].bindings[0];
        v.representations[0].bindings.push(duplicate);
        assert!(matches!(
            with(Limits::default(), |cx| {
                Geometry::new_with_materials(g.data().clone(), Some(Acquired::Value(m)), cx)
            })
            .0,
            Err(GeometryError::Reference)
        ));
    }
}

#[test]
fn public_wall_retains_its_layer_material_and_surface_sources() {
    let file = read(include_str!(
        "../../../samples/ifc/bim-whale/TallBuilding.ifc"
    ));
    let g = with(Limits::default(), |cx| file.owned_geometry(615, cx))
        .0
        .unwrap();
    let m = materials(&g);
    assert_eq!(m.level, MaterialAssignmentLevel::Occurrence);
    let a = &m.assignments[0];
    assert_eq!(a.source.instance, Some(16398));
    assert_eq!(a.kind, "IFCMATERIALLAYERSETUSAGE");
    let m = &a.members[0];
    assert_eq!(m.source.instance, Some(638));
    assert_eq!(
        m.source
            .groups
            .iter()
            .find(|g| g.role == "route")
            .unwrap()
            .instances,
        [656, 653, 651]
    );
    let Some(Acquired::Value(p)) = &m.presentation else {
        panic!()
    };
    assert_eq!(p.source.groups[0].instances, [648]);
    assert_eq!(p.representations[0].source.instance, Some(645));
    assert_eq!(p.representations[0].items[0].source.instance, Some(643));
    assert_eq!(
        p.representations[0].items[0].surfaces[0].source.instance,
        Some(604)
    );
}

#[test]
#[allow(clippy::disallowed_methods)] // 比較するテストの出力先だけを読む。
fn exports_inheritance_values_sources_selection_and_all_boundaries() {
    let Ok(dir) = std::env::var("ZUMEN_IFC_MATERIAL_INHERITANCE_OUT") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let dir = std::path::Path::new(&dir);
    for version in VERSIONS {
        let mut json = Writer::new();
        json.begin_array();
        let mut contract = String::new();
        for case in CASES {
            let src = source(version, case);
            let file = read(&src);
            let (g, d, b) = with(Limits::default(), |cx| file.owned_geometry(100, cx));
            let g = g.unwrap();
            let a = with(Limits::default(), |cx| file.material_assignments(100, cx))
                .0
                .unwrap();
            let (kind, colour) = selection(&g, case);
            let name = format!("{version}-{case}.ifc");
            std::fs::write(dir.join(&name), src).unwrap();
            json.begin_object();
            json.key("case");
            json.string(case);
            json.key("file");
            json.string(&name);
            json.key("owner");
            json.uint(a.owner.id().unwrap());
            json.key("materials");
            json.begin_array();
            for a in &a.assignments {
                json.uint(a.selection.id().unwrap())
            }
            json.end_array();
            json.key("kind");
            json.string(kind);
            json.key("colour");
            if let Some(colour) = colour {
                json.begin_array();
                for c in colour {
                    json.float(c)
                }
                json.end_array()
            } else {
                json.null()
            }
            json.end_object();
            contract.push_str(&format!(
                "{case}: {g:?}; {d:?}; selection={:?}\n",
                with(Limits::default(), |cx| g
                    .resolved_surface(&path(&g, case), cx))
                .0
            ));
            for k in Kind::ALL {
                contract.push_str(&format!("{k:?}={};", b.used(k)))
            }
            contract.push('\n');
            if case == "exact" {
                for kind in Kind::ALL {
                    let required = boundary(&file, kind);
                    for cap in [required.saturating_sub(1), required] {
                        let (r, d, b) =
                            with(Limits::default().with(kind, Limit::Finite(cap)), |cx| {
                                file.owned_geometry(100, cx)
                            });
                        contract.push_str(&format!("{kind:?}={cap}: {r:?}; {d:?}; "));
                        for k in Kind::ALL {
                            contract.push_str(&format!("{k:?}={};", b.used(k)))
                        }
                        contract.push('\n');
                    }
                }
            }
        }
        json.end_array();
        std::fs::write(dir.join(format!("{version}.json")), json.into_bytes()).unwrap();
        std::fs::write(dir.join(format!("{version}.txt")), contract).unwrap();
    }
}
