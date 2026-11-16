//! #468 の保持値・元の位置・参照の評価と、同じ予算での停止を確かめる。
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "固定入力と既知値の検査"
)]
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Stop, codes};
use zumen_civil::{CivilModel, CrossSectionHeight};
use zumen_landxml::LandXml;
use zumen_sdk::{Codec, Error, Lift};

const INPUT: &str = include_str!("fixtures/domain_values.xml");

fn run(
    src: &str,
    limits: Limits,
    read_first: bool,
    cancel: bool,
) -> (Result<CivilModel, Error>, Diagnostics, Budget) {
    let mut budget = Budget::new(limits);
    let mut diagnostics = Diagnostics::new();
    let token = CancelToken::new();
    if cancel {
        token.cancel();
    }
    let file = if read_first {
        LandXml::read(
            src.as_bytes(),
            &mut Cx::new(
                &mut Budget::default(),
                &mut Diagnostics::new(),
                &CancelToken::new(),
            ),
        )
    } else {
        LandXml::read(
            src.as_bytes(),
            &mut Cx::new(&mut budget, &mut diagnostics, &token),
        )
    };
    let result =
        file.and_then(|file| file.lift(&mut Cx::new(&mut budget, &mut diagnostics, &token)));
    (result, diagnostics, budget)
}

fn verify_values(model: &CivilModel) -> String {
    let reference = model.height_reference.as_ref().unwrap();
    assert_eq!(reference.name.as_deref(), Some("local"));
    assert_eq!(reference.tokyo_bay_offset, Some(-1.25));
    assert_eq!(model.height_datum, None);
    let points = &model.point_sets[0].points;
    assert_eq!(points[0].class.as_deref(), Some("3級基準点"));
    assert_eq!(
        (
            points[0].point.northing,
            points[0].point.easting,
            points[0].height
        ),
        (123.0, 456.0, Some(42.5))
    );
    assert_eq!(points[1].height, None);
    let a = &model.alignments[0];
    assert_eq!(a.id, "road");
    assert_eq!(a.intersection_points.len(), 3);
    assert_eq!(
        a.intersection_points[1].description.as_deref(),
        Some("交点")
    );
    assert_eq!(a.intersection_points[1].point.easting, 470.0);
    assert_eq!(a.ground_profiles.len(), 2);
    assert_eq!(
        a.ground_profiles[0].points,
        [[0.0, 39.0], [25.0, 43.0], [100.0, 59.0]]
    );
    assert_eq!(a.ground_profiles[1].points, [[10.0, 38.0], [30.0, 42.0]]);
    let section = &a.cross_sections[0];
    assert_eq!(section.distance, 25.0);
    assert_eq!(section.profile.as_deref(), Some("design-a"));
    let line = &section.lines[0];
    assert_eq!(line.height_reference, CrossSectionHeight::FormationOffset);
    assert_eq!(line.side.as_deref(), Some("left"));
    assert_eq!(line.description.as_deref(), Some("道路面"));
    assert_eq!(line.points, [[-2.0, -0.5], [3.0, 0.75]]);
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let offset = zumen_civil::cross_section_height_offset(a, section, line, &mut cx)
        .unwrap()
        .unwrap();
    assert_eq!(offset, 45.0);
    for (source, fragment) in [
        (&points[0].source, "<CgPoint name=\"survey-a\""),
        (&a.intersection_points[1].source, "<PI name=\"ip-a\""),
        (&a.ground_profiles[0].source, "<PntList2D>1000"),
        (&line.source, "<DesignCrossSectSurf"),
    ] {
        let range = source.bytes().unwrap();
        assert!(
            INPUT
                .get(range.start as usize..range.end as usize)
                .unwrap()
                .starts_with(fragment)
        );
    }
    let mut out = format!("height_offset {:016x}\n", offset.to_bits());
    for kind in Kind::ALL {
        out.push_str(&format!(
            "evaluation {} {}\n",
            kind.name(),
            budget.used(kind)
        ));
    }
    out
}

fn snapshot(
    result: &Result<CivilModel, Error>,
    diagnostics: &Diagnostics,
    budget: &Budget,
) -> String {
    let mut out = format!("{result:?}\n{diagnostics:?}\n");
    for kind in Kind::ALL {
        out.push_str(&format!("{} {}\n", kind.name(), budget.used(kind)));
    }
    out.push_str(&format!("{:?}\n", budget.exceeded()));
    out
}

/// 環境間で比較する値・出所・診断・全予算の記録。形式の正しさは上の既知値でも検査する。
pub fn verify_contracts() -> String {
    let mut out = String::new();
    for read_first in [false, true] {
        let (result, diagnostics, budget) = run(INPUT, Limits::default(), read_first, false);
        out.push_str(&verify_values(result.as_ref().unwrap()));
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code() == codes::LANDXML_HEIGHT_DATUM_UNSUPPORTED)
        );
        let baseline = snapshot(&result, &diagnostics, &budget);
        out.push_str(&format!("read_first {read_first}\n{baseline}"));
        for kind in Kind::ALL {
            let boundary = if kind == Kind::Depth {
                (0..64)
                    .find(|n| {
                        run(
                            INPUT,
                            Limits::default().with(kind, Limit::Finite(*n)),
                            read_first,
                            false,
                        )
                        .0
                        .is_ok()
                    })
                    .unwrap()
            } else {
                budget.used(kind)
            };
            let (result, diagnostics, exact) = run(
                INPUT,
                Limits::default().with(kind, Limit::Finite(boundary)),
                read_first,
                false,
            );
            assert_eq!(snapshot(&result, &diagnostics, &exact), baseline);
            if boundary != 0 {
                let (result, diagnostics, below) = run(
                    INPUT,
                    Limits::default().with(kind, Limit::Finite(boundary - 1)),
                    read_first,
                    false,
                );
                assert!(
                    matches!(result, Err(Error::Stopped(Stop::Exceeded(ref e))) if e.kind == kind)
                );
                out.push_str(&snapshot(&result, &diagnostics, &below));
            }
        }
        let (result, diagnostics, budget) = run(INPUT, Limits::default(), read_first, true);
        assert!(matches!(result, Err(Error::Stopped(Stop::Cancelled))));
        out.push_str(&snapshot(&result, &diagnostics, &budget));
    }
    out
}

#[test]
fn values_sources_and_budget_repeat() {
    assert_eq!(verify_contracts(), verify_contracts());
}

#[test]
fn malformed_lists_do_not_bridge_points_and_offsets_do_not_become_elevations() {
    for bad in [
        "1000 39 bad 43 1100 59",
        "1000 39 1025 NaN",
        "1000 39 999 43",
        "1000 39 1025",
    ] {
        let src = INPUT.replace("1000 39 1025 43 1100 59", bad);
        let (model, diagnostics, _) = run(&src, Limits::default(), false, false);
        assert_eq!(model.unwrap().alignments[0].ground_profiles.len(), 1);
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code() == codes::LANDXML_VALUE_INVALID)
        );
    }
    for bad in ["slope", "", "unknown"] {
        let src = INPUT.replace("value=\"fhOffset\"", &format!("value=\"{bad}\""));
        let (model, diagnostics, _) = run(&src, Limits::default(), false, false);
        assert_eq!(
            model.unwrap().alignments[0].cross_sections[0].lines.len(),
            1
        );
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code() == codes::LANDXML_ELEMENT_UNSUPPORTED)
        );
    }
}

#[test]
fn duplicated_or_missing_references_are_not_guessed() {
    for (before, after) in [
        ("featureRef=\"class-a\"", "featureRef=\"absent\""),
        (
            "<Feature name=\"class-a\">",
            "<Feature name=\"class-a\"/><Feature name=\"class-a\">",
        ),
    ] {
        let (model, diagnostics, _) = run(
            &INPUT.replace(before, after),
            Limits::default(),
            false,
            false,
        );
        assert_eq!(model.unwrap().point_sets[0].points[0].class, None);
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code() == codes::LANDXML_VALUE_INVALID)
        );
    }
    for reference in ["absent", ""] {
        let src = INPUT.replace(
            "label=\"profAlignRefs\" value=\"design-a\"",
            &format!("label=\"profAlignRefs\" value=\"{reference}\""),
        );
        let model = run(&src, Limits::default(), false, false).0.unwrap();
        let a = &model.alignments[0];
        let section = &a.cross_sections[0];
        let error = zumen_civil::cross_section_height_offset(
            a,
            section,
            &section.lines[0],
            &mut Cx::new(
                &mut Budget::default(),
                &mut Diagnostics::new(),
                &CancelToken::new(),
            ),
        )
        .unwrap()
        .unwrap_err();
        assert_eq!(error.code(), codes::CIVIL_CROSS_SECTION_HEIGHT_UNSUPPORTED);
        assert_eq!(section.lines[0].points[0][1], -0.5);
    }
}

#[test]
fn duplicate_or_nonfinite_height_differences_remain_unknown() {
    for value in ["NaN", "Infinity", "bad"] {
        let model = run(
            &INPUT.replace("-1.25", value),
            Limits::default(),
            false,
            false,
        )
        .0
        .unwrap();
        assert_eq!(model.height_reference.unwrap().tokyo_bay_offset, None);
    }
    let model = run(&INPUT.replace("<Property label=\"differTP\" value=\"-1.25\"/>", "<Property label=\"differTP\" value=\"-1.25\"/><Property label=\"differTP\" value=\"0\"/>"), Limits::default(), false, false).0.unwrap();
    assert_eq!(model.height_reference.unwrap().tokyo_bay_offset, None);
}

#[test]
fn duplicate_profile_names_preserve_the_reference_but_do_not_resolve_it() {
    let src = INPUT.replace(
        "</Profile>",
        "<ProfAlign name=\"design-a\"><PVI>1000 70</PVI><PVI>1100 90</PVI></ProfAlign></Profile>",
    );
    let model = run(&src, Limits::default(), false, false).0.unwrap();
    let a = &model.alignments[0];
    assert!(a.vertical_name_ambiguous);
    assert_eq!(a.vertical_name.as_deref(), Some("design-a"));
    let section = &a.cross_sections[0];
    assert_eq!(section.profile.as_deref(), Some("design-a"));
    let error = zumen_civil::cross_section_height_offset(
        a,
        section,
        &section.lines[0],
        &mut Cx::new(
            &mut Budget::default(),
            &mut Diagnostics::new(),
            &CancelToken::new(),
        ),
    )
    .unwrap()
    .unwrap_err();
    assert_eq!(error.code(), codes::CIVIL_CROSS_SECTION_HEIGHT_UNSUPPORTED);
}

#[test]
fn small_mutations_are_bounded_and_repeatable() {
    for index in 0..64 {
        let mut bytes = INPUT.as_bytes().to_vec();
        let at = index * bytes.len() / 64;
        bytes[at] ^= 0x7f;
        zumen_sdk::testkit::survives_lift::<LandXml, CivilModel>(&bytes).unwrap();
    }
}
