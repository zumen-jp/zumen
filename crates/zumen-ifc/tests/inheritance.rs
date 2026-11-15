//! 同じ共有形を使う経路ごとに、写像の面の表示を選ぶ。形と色の評価は外部照合と分ける。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_building::{Geometry, GeometryError, GeometryRef, SurfaceAppearance};
use zumen_ifc::{Ifc, IfcFile};
use zumen_sdk::Codec;

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];
const PATH: [u64; 9] = [101, 102, 50, 51, 52, 40, 41, 42, 30];

fn source(version: &str, bindings: &[(u64, u64)], extra: &str) -> String {
    let mut styles = String::new();
    for (at, (item, surface)) in bindings.iter().enumerate() {
        let at = 400 + at * 2;
        let target = if version == "IFC2X3" {
            styles.push_str(&format!(
                "#{}=IFCPRESENTATIONSTYLEASSIGNMENT((#{surface}));\n",
                at + 1
            ));
            at as u64 + 1
        } else {
            *surface
        };
        styles.push_str(&format!("#{at}=IFCSTYLEDITEM(#{item},(#{target}),$);\n"));
    }
    let transparency = if version == "IFC2X3" { "" } else { ",0." };
    let wall = if version == "IFC2X3" {
        ""
    } else {
        ",.NOTDEFINED."
    };
    format!(
        "ISO-10303-21;\nHEADER;FILE_DESCRIPTION((''),'2;1');FILE_NAME('','',(''),(''),'','','');FILE_SCHEMA(('{version}'));ENDSEC;\nDATA;\n\
    #1=IFCPROJECT('0000000000000000000001',$,'Project',$,$,$,$,(#8),#9);\n\
    #8=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-5,#20,$);\n\
    #9=IFCUNITASSIGNMENT((#10));#10=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);\n\
    #20=IFCAXIS2PLACEMENT3D(#21,$,$);#21=IFCCARTESIANPOINT((0.,0.,0.));\n\
    #24=IFCDIRECTION((0.,0.,1.));#25=IFCCARTESIANPOINT((0.,0.));#26=IFCAXIS2PLACEMENT2D(#25,$);\n\
    #27=IFCRECTANGLEPROFILEDEF(.AREA.,'R',#26,2.,3.);#30=IFCEXTRUDEDAREASOLID(#27,#20,#24,4.);\n\
    #40=IFCMAPPEDITEM(#41,#43);#41=IFCREPRESENTATIONMAP(#20,#42);\n\
    #42=IFCSHAPEREPRESENTATION(#8,'Body','SweptSolid',(#30));\n\
    #43=IFCCARTESIANTRANSFORMATIONOPERATOR3D($,$,#21,1.,$);\n\
    #50=IFCMAPPEDITEM(#51,#43);#51=IFCREPRESENTATIONMAP(#20,#52);\n\
    #52=IFCSHAPEREPRESENTATION(#8,'Body','MappedRepresentation',(#40));\n\
    #100=IFCWALL('0000000000000000000002',$,'Wall',$,$,$,#101,${wall});\n\
    #101=IFCPRODUCTDEFINITIONSHAPE($,$,(#102));\n\
    #102=IFCSHAPEREPRESENTATION(#8,'Body','MappedRepresentation',(#50));\n\
    #300=IFCCOLOURRGB('Red',1.,0.,0.);#301=IFCSURFACESTYLESHADING(#300{transparency});\n\
    #302=IFCSURFACESTYLE('Red',.BOTH.,(#301));\n\
    #310=IFCCOLOURRGB('Green',0.,1.,0.);#311=IFCSURFACESTYLESHADING(#310{transparency});\n\
    #312=IFCSURFACESTYLE('Green',.BOTH.,(#311));\n\
    #320=IFCCOLOURRGB('Blue',0.,0.,1.);#321=IFCSURFACESTYLESHADING(#320{transparency});\n\
    #322=IFCSURFACESTYLE('Blue',.BOTH.,(#321));\n\
    {styles}{extra}\nENDSEC;END-ISO-10303-21;\n"
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
fn path(g: &Geometry, ids: &[u64]) -> Vec<GeometryRef> {
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
fn selected(g: &Geometry, ids: &[u64]) -> Option<(u64, [f64; 3], usize)> {
    let path = path(g, ids);
    let (result, diag, budget) = with(Limits::default(), |cx| g.surface_appearance(&path, cx));
    assert!(diag.is_empty());
    assert_eq!(budget.used(Kind::DecodedBytes), 0);
    match result.unwrap() {
        SurfaceAppearance::Unspecified => None,
        SurfaceAppearance::Value {
            path_index,
            surfaces,
            ..
        } => {
            assert_eq!(surfaces.len(), 1);
            Some((
                surfaces[0].source.instance.unwrap(),
                surfaces[0].definition.shading.colour.rgb,
                path_index,
            ))
        }
        other => panic!("{other:?}"),
    }
}

type Case = (
    &'static str,
    &'static [(u64, u64)],
    Option<(u64, [f64; 3], usize)>,
);
const CASES: &[Case] = &[
    ("leaf", &[(30, 302)], Some((302, [1., 0., 0.], 8))),
    ("inner", &[(40, 312)], Some((312, [0., 1., 0.], 5))),
    ("outer", &[(50, 322)], Some((322, [0., 0., 1.], 2))),
    (
        "leaf-inner",
        &[(30, 302), (40, 312)],
        Some((302, [1., 0., 0.], 8)),
    ),
    (
        "inner-outer",
        &[(40, 312), (50, 322)],
        Some((312, [0., 1., 0.], 5)),
    ),
    (
        "all",
        &[(30, 302), (40, 312), (50, 322)],
        Some((302, [1., 0., 0.], 8)),
    ),
    ("none", &[], None),
];

#[test]
fn closest_item_supplies_surface_and_other_mapping_values_remain() {
    for version in VERSIONS {
        for &(name, bindings, expected) in CASES {
            let g = owned(&read(&source(version, bindings, "")));
            let hash = g.fingerprint();
            assert_eq!(selected(&g, &PATH), expected, "{version} {name}");
            assert_eq!(g.fingerprint(), hash);
            assert_eq!(selected(&g, &PATH), expected);
        }
    }
}

#[test]
fn occurrences_of_shared_mapping_keep_their_own_fallback() {
    for version in VERSIONS {
        let src = source(
            version,
            &[(50, 322), (60, 302)],
            "#60=IFCMAPPEDITEM(#51,#43);",
        )
        .replace(
            "'MappedRepresentation',(#50)",
            "'MappedRepresentation',(#50,#60)",
        );
        let g = owned(&read(&src));
        let hash = g.fingerprint();
        let mut other = PATH;
        other[2] = 60;
        assert_eq!(selected(&g, &PATH), Some((322, [0., 0., 1.], 2)));
        assert_eq!(selected(&g, &other), Some((302, [1., 0., 0.], 2)));
        assert_eq!(selected(&g, &PATH), Some((322, [0., 0., 1.], 2)));
        assert_eq!(g.fingerprint(), hash);
        assert_eq!(
            g.data()
                .nodes
                .iter()
                .filter(|n| n.source.instance == Some(30))
                .count(),
            1
        );
    }
}

#[test]
fn unsupported_and_competing_layers_do_not_silently_fall_back() {
    for version in VERSIONS {
        let g = owned(&read(&source(
            version,
            &[(30, 999), (50, 322)],
            "#999=IFCFILLAREASTYLE('Unsupported',());",
        )));
        let path = path(&g, &PATH);
        assert!(matches!(
            with(Limits::default(), |cx| g.surface_appearance(&path, cx)).0,
            Ok(SurfaceAppearance::Unsupported { path_index: 8, .. })
        ));
    }
    let extra = "#500=IFCPRESENTATIONLAYERWITHSTYLE('A',$,(#30),$,.T.,.F.,.F.,(#302));\n\
        #501=IFCPRESENTATIONLAYERWITHSTYLE('B',$,(#30),$,.T.,.F.,.F.,(#312));";
    let g = owned(&read(&source("IFC2X3", &[(50, 322)], extra)));
    let p = path(&g, &PATH);
    assert!(matches!(
        with(Limits::default(), |cx| g.surface_appearance(&p, cx)).0,
        Ok(SurfaceAppearance::Ambiguous { path_index: 8, .. })
    ));
    let g = owned(&read(&source("IFC2X3", &[(30, 302), (50, 322)], extra)));
    assert_eq!(selected(&g, &PATH), Some((302, [1., 0., 0.], 8)));
}

#[test]
fn paths_are_complete_and_bounded_and_restore_depth_on_error() {
    let g = owned(&read(&source("IFC4", &[(50, 322)], "")));
    let p = path(&g, &PATH);
    let mut skipped = p.clone();
    skipped.remove(3);
    let mut dangling = p.clone();
    dangling[8] = GeometryRef(u32::MAX);
    for invalid in [vec![], p[1..].to_vec(), p[..8].to_vec(), skipped, dangling] {
        let (r, _, b) = with(Limits::default(), |cx| g.surface_appearance(&invalid, cx));
        assert_eq!(r, Err(GeometryError::Reference));
        assert_eq!(b.used(Kind::Depth), 0);
    }
    let (_, _, baseline) = with(Limits::default(), |cx| g.surface_appearance(&p, cx));
    for kind in Kind::ALL {
        let required = if kind == Kind::Depth {
            p.len() as u64
        } else {
            baseline.used(kind)
        };
        let mut limits = Limits::default();
        limits.set(kind, Limit::Finite(required));
        let (r, _, b) = with(limits, |cx| g.surface_appearance(&p, cx));
        assert!(r.is_ok(), "{kind:?}");
        assert_eq!(b.used(Kind::Depth), 0);
        if required > 0 {
            limits.set(kind, Limit::Finite(required - 1));
            let (r, _, b) = with(limits, |cx| g.surface_appearance(&p, cx));
            assert!(matches!(r, Err(GeometryError::Stopped(_))), "{kind:?}");
            assert_eq!(b.used(Kind::Depth), 0);
        }
    }
    for expired in [false, true] {
        let (result, _, usage) = with(Limits::default(), |cx| {
            if expired {
                cx.cancel.expire();
            } else {
                cx.cancel.cancel();
            }
            g.surface_appearance(&p, cx)
        });
        assert!(matches!(result, Err(GeometryError::Stopped(_))));
        assert_eq!(usage.used(Kind::Depth), 0);
    }
}

#[test]
#[allow(clippy::disallowed_methods)] // 外部照合と WASI で比較するテストの出力先だけを読む。
fn exports_mapping_choices_and_budget_usage() {
    let Ok(dir) = std::env::var("ZUMEN_IFC_INHERITANCE_OUT") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    for version in VERSIONS {
        let mut records = Vec::new();
        let mut contract = String::new();
        for &(name, bindings, expected) in CASES {
            let src = source(version, bindings, "");
            let g = owned(&read(&src));
            let p = path(&g, &PATH);
            let (r, d, b) = with(Limits::default(), |cx| g.surface_appearance(&p, cx));
            let actual = selected(&g, &PATH);
            assert_eq!(actual, expected);
            contract.push_str(&format!("{name}: {r:?}; {d:?}; "));
            for kind in Kind::ALL {
                contract.push_str(&format!("{kind:?}={};", b.used(kind)));
            }
            contract.push('\n');
            for kind in Kind::ALL {
                let required = if kind == Kind::Depth {
                    p.len() as u64
                } else {
                    b.used(kind)
                };
                for cap in [required.saturating_sub(1), required] {
                    let (result, diagnostics, usage) =
                        with(Limits::default().with(kind, Limit::Finite(cap)), |cx| {
                            g.surface_appearance(&p, cx)
                        });
                    contract.push_str(&format!("{kind:?}={cap}: {result:?}; {diagnostics:?}; "));
                    for k in Kind::ALL {
                        contract.push_str(&format!("{k:?}={};", usage.used(k)));
                    }
                    contract.push('\n');
                }
            }
            let Some((_, rgb, _)) = actual else {
                continue;
            };
            let file = format!("{version}-{name}.ifc");
            std::fs::write(std::path::Path::new(&dir).join(&file), src).unwrap();
            records.push(format!(
                "{{\"file\":\"{file}\",\"colour\":[{},{},{}]}}",
                rgb[0], rgb[1], rgb[2]
            ));
        }
        std::fs::write(
            std::path::Path::new(&dir).join(format!("{version}.json")),
            format!("[{}]\n", records.join(",")),
        )
        .unwrap();
        std::fs::write(
            std::path::Path::new(&dir).join(format!("{version}.txt")),
            contract,
        )
        .unwrap();
    }
}
