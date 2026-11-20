//! 建物モデルから切り出した平面図・断面図を、CAD製図基準向けの表記で DXF・SXF（P21）・JWW に書き出し、
//! `mlit-cad` の規則と変換診断・出所を確かめる（#397）。
#![cfg(feature = "building-drawing")]
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "固定した試験入力"
)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Value, codes};
use zumen_building::drawing::Side;
use zumen_building::{Building, GlobalId, SpatialKind, Transform};
use zumen_crs::ProjectPoint;
use zumen_drawing::{Drawing2D, Origin, Palette, Shape, SheetId, SpaceId};
use zumen_ifc::{
    Edit, FillingKind, Ifc, NewBeam, NewColumn, NewFilling, NewGrid, NewGridAxis, NewLayer,
    NewOpening, NewPlacement, NewSection, NewSlab, NewSpace, NewWall, SpaceKind, Target, apply,
};
use zumen_profile::Options;
use zumen_profile::building_drawing::{Drawing, DrawingProfile};
use zumen_sdk::conversion::ConversionFidelity;
use zumen_sdk::{Codec, Lift};

const MLIT_CAD: &[u8] = include_bytes!("../../../profiles/mlit-cad/2025-12/profile.json");
const SUBMISSION: &[u8] =
    include_bytes!("../../../profiles/mlit-cad-building-drawing/2025-12/profile.json");

fn limits() -> Limits {
    Limits::default()
        .with(Kind::DecodedBytes, Limit::Finite(16 * 1024 * 1024 * 1024))
        .with(Kind::Work, Limit::Finite(100_000_000_000))
}

fn run<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> T {
    let mut budget = Budget::new(limits());
    let (mut diagnostics, cancel) = (Diagnostics::new(), CancelToken::new());
    f(&mut Cx::new(&mut budget, &mut diagnostics, &cancel))
}

fn submission_profile() -> DrawingProfile {
    run(|cx| {
        DrawingProfile::load(
            &[("mlit-cad", MLIT_CAD), ("submission", SUBMISSION)],
            "mlit-cad-building-drawing",
            "2025-12",
            cx,
        )
        .unwrap()
    })
}

fn check_mlit_cad(
    drawing: &Drawing2D,
    origins: Vec<Origin>,
    file_name: Option<&str>,
) -> zumen_profile::Report {
    run(|cx| {
        zumen_profile::check(
            drawing,
            &[("mlit-cad", MLIT_CAD), ("submission", SUBMISSION)],
            "mlit-cad-building-drawing",
            "2025-12",
            &Options {
                family: "sxf",
                file_name,
                origins,
                palette: &Palette::default(),
            },
            cx,
        )
        .unwrap()
    })
}

fn global(n: u32) -> GlobalId {
    GlobalId::from_uuid(u128::from(n).to_be_bytes())
}

fn authored_two_storey_building() -> Building {
    let seed = format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\nFILE_NAME('a.ifc','',(''),(''),'','','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n#1=IFCPROJECT('{}',#2,'Project',$,$,$,$,$,#3);\n#2=IFCOWNERHISTORY($,$,$,.ADDED.,$,$,$,0);\n#3=IFCUNITASSIGNMENT((#4));\n#4=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\nENDSEC;\nEND-ISO-10303-21;\n",
        global(1)
    );
    let layer = |material: &str, thickness: f64| NewLayer {
        material: material.to_owned(),
        thickness,
    };
    let axis = |tag: &str, start: [f64; 2], end: [f64; 2]| NewGridAxis {
        tag: tag.to_owned(),
        start,
        end,
    };
    let storey = |n: u32, elevation: f64| {
        Edit::Space(NewSpace {
            kind: SpaceKind::Storey,
            name: Some(format!("{}FL", n - 1000)),
            parent: Target::Added(0),
            placement: Some(NewPlacement {
                relative_to: Some(Target::Added(0)),
                local: Transform::translation(0.0, 0.0, elevation),
            }),
            elevation: Some(elevation),
            global_id: Some(global(n)),
        })
    };

    let mut edits = vec![
        Edit::Space(NewSpace {
            kind: SpaceKind::Building,
            name: Some("本棟".to_owned()),
            parent: Target::Instance(1),
            placement: Some(NewPlacement {
                relative_to: None,
                local: Transform::translation(0.0, 0.0, 0.0),
            }),
            elevation: None,
            global_id: Some(global(1000)),
        }),
        storey(1001, 0.0),
        storey(1002, 3000.0),
        Edit::Grid(NewGrid {
            storey: Target::Added(1),
            u: vec![
                axis("X1", [0.0, -500.0], [0.0, 4500.0]),
                axis("X2", [4000.0, -500.0], [4000.0, 4500.0]),
            ],
            v: vec![
                axis("Y1", [-500.0, 0.0], [4500.0, 0.0]),
                axis("Y2", [-500.0, 4000.0], [4500.0, 4000.0]),
            ],
            elevation: None,
            name: Some("通り芯".to_owned()),
            global_id: Some(global(1003)),
        }),
    ];

    let corners = [[0.0, 0.0], [4000.0, 0.0], [4000.0, 4000.0], [0.0, 4000.0]];
    for level in [1u32, 2] {
        let base = 1000 * (level + 1);
        let on = Target::Added(level as usize);
        let first = edits.len();
        for (i, start) in corners.iter().enumerate() {
            edits.push(Edit::Wall(NewWall {
                storey: on,
                start: *start,
                end: corners[(i + 1) % corners.len()],
                height: 2800.0,
                layers: vec![layer("Concrete", 200.0)],
                offset: None,
                type_of: None,
                name: Some(format!("W{}", base + i as u32)),
                global_id: Some(global(base + i as u32)),
            }));
        }
        edits.extend([
            Edit::Slab(NewSlab {
                storey: on,
                outline: corners.to_vec(),
                layers: vec![layer("Concrete", 180.0)],
                elevation: None,
                type_of: None,
                name: Some("床スラブ".to_owned()),
                global_id: Some(global(base + 10)),
            }),
            Edit::Column(NewColumn {
                storey: on,
                position: [2000.0, 2000.0],
                base: None,
                height: 2800.0,
                section: NewSection {
                    width: 400.0,
                    depth: 400.0,
                },
                direction: Some([0.0, 1.0]),
                material: Some("Concrete".to_owned()),
                type_of: None,
                name: Some("C1".to_owned()),
                global_id: Some(global(base + 11)),
            }),
            Edit::Beam(NewBeam {
                storey: on,
                start: [0.0, 2000.0, 2800.0],
                end: [4000.0, 2000.0, 2800.0],
                section: NewSection {
                    width: 300.0,
                    depth: 500.0,
                },
                material: Some("Steel".to_owned()),
                type_of: None,
                name: Some("G1".to_owned()),
                global_id: Some(global(base + 12)),
            }),
        ]);
        let (kind, sill, size) = if level == 1 {
            (FillingKind::Door, 0.0, [900.0, 2100.0])
        } else {
            (FillingKind::Window, 900.0, [1200.0, 1100.0])
        };
        edits.push(Edit::Opening(NewOpening {
            host: Target::Added(first),
            distance: 800.0,
            sill,
            width: size[0],
            height: size[1],
            name: None,
            global_id: Some(global(base + 20)),
        }));
        edits.push(Edit::Filling(NewFilling {
            opening: Target::Added(edits.len() - 1),
            kind,
            width: Some(size[0]),
            height: Some(size[1]),
            type_of: None,
            name: None,
            global_id: Some(global(base + 21)),
        }));
    }

    let file = run(|cx| Ifc::read(seed.as_bytes(), cx).unwrap());
    let applied = run(|cx| apply(&file, &edits, b"sub-seed", cx).unwrap());
    run(|cx| {
        Ifc::read(&applied.bytes, cx)
            .unwrap()
            .lift_geometry(cx)
            .unwrap()
    })
}

fn lower_dxf(drawing: &Drawing2D) -> (zumen_acad::Lowered, Diagnostics) {
    let mut budget = Budget::new(limits());
    let (mut diagnostics, cancel) = (Diagnostics::with_max_places(1 << 20), CancelToken::new());
    let lowered = zumen_acad::lower_to_dxf_with_report(
        drawing,
        &zumen_acad::Reseat,
        256,
        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
    )
    .unwrap();
    (lowered, diagnostics)
}

fn lower_p21(drawing: &Drawing2D) -> (zumen_sxf::Lowered, Diagnostics) {
    let mut budget = Budget::new(limits());
    let (mut diagnostics, cancel) = (Diagnostics::with_max_places(1 << 20), CancelToken::new());
    let lowered = zumen_sxf::lower_to_p21(
        drawing,
        256,
        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
    )
    .unwrap();
    (lowered, diagnostics)
}

fn lower_jww(drawing: &Drawing2D) -> (zumen_jww::Lowered, Diagnostics) {
    let mut budget = Budget::new(limits());
    let (mut diagnostics, cancel) = (Diagnostics::with_max_places(1 << 20), CancelToken::new());
    let lowered = zumen_jww::lower_to_jww(
        drawing,
        256,
        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
    )
    .unwrap();
    (lowered, diagnostics)
}

fn reread_dxf(bytes: &[u8]) -> Drawing2D {
    run(|cx| zumen_acad::Dxf::read(bytes, cx).unwrap().lift(cx).unwrap())
}

fn reread_p21(bytes: &[u8]) -> Drawing2D {
    run(|cx| zumen_sxf::Sxf::read(bytes, cx).unwrap().lift(cx).unwrap())
}

fn reread_jww(bytes: &[u8]) -> Drawing2D {
    run(|cx| zumen_jww::Jww::read(bytes, cx).unwrap().lift(cx).unwrap())
}

fn rule_summary<'a>(report: &'a zumen_profile::Report, id: &str) -> &'a zumen_profile::RuleSummary {
    report
        .rules
        .iter()
        .find(|r| r.id == id)
        .unwrap_or_else(|| panic!("missing rule {id}"))
}

fn verify_exports_and_checks(label: &str, raw: &Drawing, file_name: &str, expect_grids: bool) {
    let scaled = run(|cx| raw.at_view_scale(cx).unwrap());
    assert_eq!(scaled.chain.len(), 2, "{label}");
    assert_eq!(scaled.chain[0].id, "mlit-cad");
    assert_eq!(scaled.chain[1].id, "mlit-cad-building-drawing");

    // 1. 切り出した図面（実寸換算後）そのものを mlit-cad 全 11 規則で照合する。
    let cut_report = check_mlit_cad(
        &scaled.output.drawing,
        vec![Origin::Sheet(SheetId(0))],
        Some(file_name),
    );
    assert_eq!(cut_report.rules.len(), 11, "{label}");
    for rule in &cut_report.rules {
        assert_eq!(
            (rule.findings, rule.unsupported),
            (0, 0),
            "{label} cut rule {}: {:?}",
            rule.id,
            cut_report.findings
        );
    }

    let hatches: Vec<_> = scaled
        .decorations
        .iter()
        .filter(|d| d.kind == "hatch")
        .collect();
    let grid_labels: Vec<_> = scaled
        .decorations
        .iter()
        .filter(|d| d.kind == "grid-label")
        .collect();
    if expect_grids {
        assert!(!grid_labels.is_empty(), "{label}: expected grid labels");
    }

    // 2. DXF への書き出しと再読・mlit-cad チェック。
    let (dxf, dxf_diag) = lower_dxf(&scaled.output.drawing);
    assert_eq!(lower_dxf(&scaled.output.drawing).0.bytes, dxf.bytes);
    assert!(
        dxf_diag
            .iter()
            .all(|d| d.code() != codes::CONVERT_ELEMENT_UNSUPPORTED),
        "{label} dxf dropped: {dxf_diag:?}"
    );
    let dxf_back = reread_dxf(&dxf.bytes);
    assert_eq!(
        dxf_back.spaces[0].elements.len(),
        scaled.output.drawing.spaces[0].elements.len(),
        "{label}"
    );
    let dxf_report = check_mlit_cad(&dxf_back, vec![Origin::Sheet(SheetId(0))], Some(file_name));
    for rule in &dxf_report.rules {
        assert_eq!(
            rule.findings, 0,
            "{label} dxf rule {}: {:?}",
            rule.id, dxf_report.findings
        );
    }

    // 3. SXF（P21）への書き出しと再読・mlit-cad チェック。
    // 紙上 mm の図面と at_view_scale 後の図面の両方で、通り芯の文字が落ちずハッチングだけが出所付きで落ちる。
    for (variant, source_drawing) in [
        ("raw", &raw.output.drawing),
        ("scaled", &scaled.output.drawing),
    ] {
        let (p21, p21_diag) = lower_p21(source_drawing);
        assert_eq!(lower_p21(source_drawing).0.bytes, p21.bytes);
        let shape_kinds = ["line", "arc", "circle", "polyline", "text", "hatch"];
        let unsupported: Vec<_> = p21_diag
            .iter()
            .filter(|d| {
                d.code() == codes::CONVERT_ELEMENT_UNSUPPORTED
                    && shape_kinds
                        .iter()
                        .any(|k| d.params().contains(&("kind", Value::Key(k))))
            })
            .collect();
        if hatches.is_empty() {
            assert!(unsupported.is_empty(), "{label} {variant}: {unsupported:?}");
        } else {
            assert_eq!(unsupported.len(), 1, "{label} {variant}: {unsupported:?}");
            assert!(
                unsupported[0]
                    .params()
                    .contains(&("kind", Value::Key("hatch")))
            );
            assert_eq!(unsupported[0].count(), hatches.len() as u64);
            for hatch in &hatches {
                assert!(
                    unsupported[0]
                        .places()
                        .iter()
                        .any(|p| p.id() == Some(hatch.id.as_str())),
                    "{label} {variant}: missing hatch source {}",
                    hatch.id
                );
            }
        }
        let expected_p21 = source_drawing.spaces[0].elements.len() - hatches.len();
        assert_eq!(p21.elements, expected_p21 as u64, "{label} {variant}");

        let p21_back = reread_p21(&p21.bytes);
        let p21_sheet = &p21_back.sheets[0];
        assert_eq!(p21_sheet.elements.len(), expected_p21, "{label} {variant}");
        assert!(
            p21_sheet.elements.iter().all(|e| matches!(
                p21_back.color_of(e),
                zumen_drawing::Inherit::Own(c) if c.rgb == Some([0, 0, 0])
            )),
            "{label} {variant}: P21 elements must keep black RGB"
        );
        let written_texts = p21_sheet
            .elements
            .iter()
            .filter(|e| matches!(e.shape, Shape::Text(_) | Shape::Formatted(_)))
            .count();
        assert_eq!(
            written_texts,
            grid_labels.len(),
            "{label} {variant}: grid labels must be written to P21"
        );

        let p21_report =
            check_mlit_cad(&p21_back, vec![Origin::Sheet(SheetId(0))], Some(file_name));
        // lower_to_p21 は COLOUR_RGB（利用者定義色 17）と CURVE_STYLE_FONT（利用者定義線種）で書くため、
        // 再読後の P21 では既定義色・既定義線種以外の 8 規則で違反 0 を確かめる。
        for rule_id in [
            "layer-name",
            "line-width",
            "line-width-by-sheet",
            "text-height",
            "characters",
            "sheet-size",
            "orientation",
            "file-name",
        ] {
            assert_eq!(
                rule_summary(&p21_report, rule_id).findings,
                0,
                "{label} {variant} p21 rule {rule_id}: {:?}",
                p21_report.findings
            );
        }
    }

    // 4. JWW への書き出しと再読・mlit-cad チェック。
    let (jww, jww_diag) = lower_jww(&scaled.output.drawing);
    assert_eq!(lower_jww(&scaled.output.drawing).0.bytes, jww.bytes);
    let unsupported_jww: Vec<_> = jww_diag
        .iter()
        .filter(|d| d.code() == codes::CONVERT_ELEMENT_UNSUPPORTED)
        .collect();
    for (kind, expected_items) in [("hatch", &hatches), ("text", &grid_labels)] {
        let found = unsupported_jww
            .iter()
            .find(|d| d.params().contains(&("kind", Value::Key(kind))));
        if expected_items.is_empty() {
            assert!(found.is_none(), "{label} jww unexpected {kind}: {found:?}");
        } else {
            let diag = found.unwrap_or_else(|| panic!("{label} jww missing {kind}"));
            assert_eq!(diag.count(), expected_items.len() as u64);
            for item in expected_items {
                assert!(
                    diag.places()
                        .iter()
                        .any(|p| p.id() == Some(item.id.as_str())),
                    "{label} jww missing {kind} source {}",
                    item.id
                );
            }
        }
    }
    let dropped_layers = jww
        .report
        .counts
        .iter()
        .find(|c| c.kind == "layer" && c.fidelity == ConversionFidelity::Dropped)
        .map_or(0, |c| c.count);
    assert!(
        dropped_layers > 0,
        "{label}: unused layers are marked Dropped in JWW"
    );

    let jww_back = reread_jww(&jww.bytes);
    let expected_jww =
        scaled.output.drawing.spaces[0].elements.len() - hatches.len() - grid_labels.len();
    assert_eq!(
        jww_back.spaces[0].elements.len(),
        expected_jww,
        "{label} jww elements"
    );
    let jww_report = check_mlit_cad(&jww_back, vec![Origin::Space(SpaceId(0))], Some(file_name));
    for rule_id in [
        "layer-name",
        "color",
        "line-width",
        "characters",
        "file-name",
    ] {
        assert_eq!(
            rule_summary(&jww_report, rule_id).findings,
            0,
            "{label} jww rule {rule_id}: {:?}",
            jww_report.findings
        );
    }
}

#[test]
fn plans_and_sections_of_authored_two_storey_building_pass_mlit_cad_and_export() {
    let profile = submission_profile();
    let building = authored_two_storey_building();

    // 1FL・2FL 平面図
    for (idx, z, expect_grids) in [(1, 0.0, true), (2, 3000.0, false)] {
        let plan = run(|cx| {
            let view = profile
                .default_plan(&building, ProjectPoint { x: 0.0, y: 0.0, z }, cx)
                .unwrap();
            profile.cut(&building, view, 0.1, 5.0, None, cx).unwrap()
        });
        assert!(
            plan.decorations.iter().any(|d| d.kind == "hatch"),
            "storey {idx} plan should have concrete hatches"
        );
        verify_exports_and_checks(
            &format!("authored-plan-{idx}f"),
            &plan,
            &format!("00{idx}D1PL0.P21"),
            expect_grids,
        );
    }

    // 建物中央を東西に切る断面図（壁・スラブ・梁・開口・建具を切断）
    let section = run(|cx| {
        let view = profile
            .default_section(
                &building,
                ProjectPoint {
                    x: -500.0,
                    y: 2000.0,
                    z: 0.0,
                },
                ProjectPoint {
                    x: 4500.0,
                    y: 2000.0,
                    z: 0.0,
                },
                Side::Right,
                cx,
            )
            .unwrap();
        profile.cut(&building, view, 0.1, 5.0, None, cx).unwrap()
    });
    assert!(!section.output.lines.is_empty());
    assert!(
        section.decorations.iter().any(|d| d.kind == "hatch"),
        "section should have concrete hatches"
    );
    verify_exports_and_checks("authored-section", &section, "003D1SC0.P21", false);
}

#[test]
fn public_ifc_samples_pass_mlit_cad_and_export_plans_and_sections() {
    let profile = submission_profile();
    for (name, bytes, grids, section_span) in [
        (
            "text2bim",
            &include_bytes!("../../../samples/ifc/text2bim/sample_model.ifc")[..],
            false,
            ([-5.0, 2.0, 0.0], [15.0, 2.0, 0.0], 0.01, 1.0),
        ),
        (
            "gni-model9",
            &include_bytes!("../../../samples/ifc/gni/model_9.ifc")[..],
            true,
            ([-5.0, 5.0, 0.0], [25.0, 5.0, 0.0], 0.01, 1.0),
        ),
    ] {
        let building = run(|cx| Ifc::read(bytes, cx).unwrap().lift_geometry(cx).unwrap());
        let first_z = building
            .components
            .spatial
            .iter()
            .filter(|(_, s)| s.kind == SpatialKind::Storey)
            .find_map(|(_, s)| Some(s.elevation.as_ref()?.value))
            .unwrap();
        let plan = run(|cx| {
            let view = profile
                .default_plan(
                    &building,
                    ProjectPoint {
                        x: 0.0,
                        y: 0.0,
                        z: first_z,
                    },
                    cx,
                )
                .unwrap();
            profile
                .cut(&building, view, section_span.2, section_span.3, None, cx)
                .unwrap()
        });
        verify_exports_and_checks(&format!("{name}-plan"), &plan, "010D1PL0.P21", grids);

        let section = run(|cx| {
            let view = profile
                .default_section(
                    &building,
                    ProjectPoint {
                        x: section_span.0[0],
                        y: section_span.0[1],
                        z: section_span.0[2],
                    },
                    ProjectPoint {
                        x: section_span.1[0],
                        y: section_span.1[1],
                        z: section_span.1[2],
                    },
                    Side::Right,
                    cx,
                )
                .unwrap();
            profile
                .cut(&building, view, section_span.2, section_span.3, None, cx)
                .unwrap()
        });
        verify_exports_and_checks(&format!("{name}-section"), &section, "011D1SC0.P21", false);
    }
}

#[test]
fn invalid_grid_font_is_rejected_on_load() {
    for bad in [r#""font": """#, r#""font": "MS\nGothic""#, r#""font": 123"#] {
        let patched = std::str::from_utf8(SUBMISSION)
            .unwrap()
            .replace(r#""font": "MS Gothic""#, bad);
        let result = run(|cx| {
            DrawingProfile::load(
                &[("mlit-cad", MLIT_CAD), ("bad", patched.as_bytes())],
                "mlit-cad-building-drawing",
                "2025-12",
                cx,
            )
        });
        assert!(result.is_err(), "{bad}");
    }
}
