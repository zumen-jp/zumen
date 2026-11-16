//! 申請前のチェックと、区分の合計後の丸めを自作のモデルで確かめる。
#![cfg(feature = "building")]
#![allow(clippy::unwrap_used, reason = "試験の固定入力")]
#[path = "support/building.rs"]
mod input;
use input::{building, profile, with};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Severity};
use zumen_building::*;
use zumen_profile::building::{AreaGroup, AreaGroupKind as Group, BuildingProfile, Error};

fn counts(report: &zumen_profile::Report, id: &str) -> (u64, u64) {
    let rule = report.rules.iter().find(|rule| rule.id == id).unwrap();
    (rule.findings, rule.unsupported)
}

#[test]
fn base_has_seven_rules_and_one_unknown_for_each_unmapped_property() {
    let b = building();
    let p = profile(false);
    let report = with(|cx| p.check(&[&b, &b], cx)).unwrap();
    assert_eq!(report.rules.len(), 7);
    assert_eq!(report.findings.len(), 4);
    for id in [
        "room-use",
        "area-region-code",
        "floor-area-ratio",
        "fire-compartment",
    ] {
        assert_eq!(counts(&report, id), (0, 1));
    }
    assert!(report.findings.iter().all(|f| f.level == Severity::Info));
    let configured = profile(true);
    let report = with(|cx| configured.check(&[&b, &b], cx)).unwrap();
    assert!(report.findings.is_empty(), "{report:?}");
    assert_eq!(report.chain.len(), 2);
    assert_ne!(p.hash(), configured.hash());
    assert_eq!(configured.hash(), profile(true).hash());
}

#[test]
fn schema_has_invalid_unknown_and_valid_results_per_model() {
    let a = building();
    let mut b = building();
    b.document.header.schemas = vec!["IFC4".into()];
    let mut c = building();
    c.document.header.schemas.clear();
    let r = with(|cx| profile(false).check(&[&a, &b, &c], cx)).unwrap();
    assert_eq!(counts(&r, "ifc-schema"), (1, 1));
    let findings: Vec<_> = r
        .findings
        .iter()
        .filter(|f| f.rule == "ifc-schema")
        .collect();
    assert_eq!(
        findings[0]
            .diagnostic
            .params()
            .iter()
            .find(|(key, _)| *key == "model")
            .map(|(_, value)| value),
        Some(&zumen_base::Value::UInt(2))
    );
    assert_eq!(findings[1].level, Severity::Info);
}

#[test]
fn alignment_checks_units_placement_and_context_without_guessing() {
    let p = profile(false);
    let a = building();
    for change in 0..5 {
        let mut b = building();
        match change {
            0 => b.document.units[0].prefix = Some("MILLI".into()),
            1 => {
                b.components.placement.insert(
                    ElementRef(0),
                    Placement {
                        relative_to: None,
                        local: Local::Transform(Transform::translation(0.01, 0.0, 0.0)),
                    },
                );
            }
            2 => {
                b.components.placement.insert(
                    ElementRef(0),
                    Placement {
                        relative_to: None,
                        local: Local::Transform(Transform([
                            [0.0, -1.0, 0.0, 0.0],
                            [1.0, 0.0, 0.0, 0.0],
                            [0.0, 0.0, 1.0, 0.0],
                        ])),
                    },
                );
            }
            3 => {
                b.components.spatial.remove(ElementRef(0));
            }
            _ => {
                b.document.units.clear();
            }
        }
        let r = with(|cx| p.check(&[&a, &b], cx)).unwrap();
        assert_eq!(
            counts(&r, "models-aligned"),
            if change < 3 { (1, 0) } else { (0, 1) },
            "{change}: {r:?}"
        );
    }
    let mut b = building();
    let mut g = input::geometry(1.0);
    if let GeometryDefinition::Context { definition, .. } = &mut g.nodes[1].definition {
        definition.geometric.world_coordinate_system =
            ContextPlacement::Space(input::axis([2.0, 0.0, 0.0]));
    }
    with(|cx| b.insert_geometry(ElementRef(0), Geometry::new(g, cx).unwrap(), cx)).unwrap();
    let r = with(|cx| p.check(&[&a, &b], cx)).unwrap();
    assert_eq!(counts(&r, "models-aligned"), (0, 1)); // 異なる文脈を 1 つに決めない。
    assert_eq!(
        counts(&with(|cx| p.check(&[&a], cx)).unwrap(), "models-aligned"),
        (0, 0)
    );
    assert_eq!(
        counts(&with(|cx| p.check(&[], cx)).unwrap(), "models-aligned"),
        (0, 1)
    );
}

#[test]
fn representation_missing_and_unknown_class_keep_element_locations() {
    let mut b = building();
    b.components.representation.remove(ElementRef(1));
    b.components.class.remove(ElementRef(2));
    let r = with(|cx| profile(false).check(&[&b], cx)).unwrap();
    assert_eq!(counts(&r, "representation-present"), (1, 1));
    for finding in r
        .findings
        .iter()
        .filter(|f| f.rule == "representation-present")
    {
        assert_eq!(finding.diagnostic.places().len(), 1);
    }
}

#[test]
fn each_property_rule_distinguishes_absent_present_and_unresolved_values() {
    for (id, name, at) in [
        ("room-use", "use", 1),
        ("area-region-code", "code", 1),
        ("floor-area-ratio", "ratio", 1),
        ("fire-compartment", "fire", 3),
    ] {
        for (value, expected) in [
            (None, (1, 0)),
            (Some(Literal::Text(" \t".into())), (1, 0)),
            (Some(Literal::Integer(0)), (0, 0)),
            (Some(Literal::Boolean(false)), (0, 0)),
            (Some(Literal::Logical(None)), (0, 1)),
            (Some(Literal::Real(f64::NAN)), (0, 1)),
            (Some(Literal::List(vec![Literal::Logical(None)])), (0, 1)),
            (Some(Literal::Element(ElementRef(900))), (0, 1)),
        ] {
            let mut b = building();
            input::properties(&mut b, ElementRef(at), &[(name, value)]);
            let r = with(|cx| profile(true).check(&[&b], cx)).unwrap();
            assert_eq!(counts(&r, id), expected, "{id}: {r:?}");
        }
    }
}

#[test]
fn occurrence_property_overrides_type_and_duplicate_is_unknown() {
    let mut b = building();
    let kind = input::add(&mut b, "IfcSpaceType", 1.0);
    input::properties(
        &mut b,
        kind,
        &[("use", Some(Literal::Text("inherited".into())))],
    );
    b.components
        .r#type
        .insert(ElementRef(1), TypeOf { target: kind });
    b.components.properties.remove(ElementRef(1));
    let p = profile(true);
    assert_eq!(
        counts(&with(|cx| p.check(&[&b], cx)).unwrap(), "room-use"),
        (0, 0)
    );
    input::properties(&mut b, ElementRef(1), &[("use", None)]);
    assert_eq!(
        counts(&with(|cx| p.check(&[&b], cx)).unwrap(), "room-use"),
        (1, 0)
    );
    input::properties(&mut b, ElementRef(1), &[("use", None), ("use", None)]);
    assert_eq!(
        counts(&with(|cx| p.check(&[&b], cx)).unwrap(), "room-use"),
        (0, 1)
    );
}

#[test]
fn areas_sum_before_truncating_each_group_and_keep_raw_values_and_identity() {
    let p = profile(false);
    let b = building();
    let elements = [ElementRef(1), ElementRef(2)];
    let groups = [
        AreaGroup {
            kind: Group::Storey,
            key: "1",
            elements: &elements,
            origin: input::origin(),
        },
        AreaGroup {
            kind: Group::Use,
            key: "1/a",
            elements: &elements[..1],
            origin: input::origin(),
        },
        AreaGroup {
            kind: Group::Use,
            key: "1/b",
            elements: &elements[1..],
            origin: input::origin(),
        },
        AreaGroup {
            kind: Group::Building,
            key: "building",
            elements: &elements,
            origin: input::origin(),
        },
    ];
    let r = with(|cx| p.areas(&b, &groups, input::tolerance(), cx)).unwrap();
    assert_eq!(r.groups.len(), 4);
    for (group, expected) in r.groups.iter().zip(["2.01", "1.00", "1.00", "2.01"]) {
        assert_eq!(
            group.measurement.rounded_total.unwrap().to_string(),
            expected
        );
        assert!(
            group
                .measurement
                .measurements
                .iter()
                .all(|v| v.rounded.is_none())
        );
        assert_eq!(group.measurement.profile.as_ref().unwrap().hash, p.hash());
    }
    assert!((r.groups[0].measurement.raw_total.unwrap() - 2.012).abs() < 1e-12);
    assert_eq!(r.findings.len(), 2);
    assert!(r.findings.iter().all(|f| f.level == Severity::Info));
    let duplicate = [groups[0], groups[0]];
    assert_eq!(
        with(|cx| p.areas(&b, &duplicate, input::tolerance(), cx)).unwrap_err(),
        Error::Input("area_group_duplicate")
    );
    let duplicate_element = [
        groups[0],
        AreaGroup {
            key: "2",
            ..groups[0]
        },
    ];
    assert_eq!(
        with(|cx| p.areas(&b, &duplicate_element, input::tolerance(), cx)).unwrap_err(),
        Error::Input("area_element_duplicate")
    );
}

#[test]
fn budget_and_cancellation_stop_before_returning_partial_results() {
    let p = profile(true);
    let b = building();
    for kind in [Kind::Work, Kind::DecodedBytes, Kind::Elements] {
        let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(0)));
        let result = p.check(
            &[&b],
            &mut Cx::new(&mut budget, &mut Diagnostics::new(), &CancelToken::new()),
        );
        assert!(matches!(result, Err(Error::Stopped(_))));
    }
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(
        p.check(
            &[&b],
            &mut Cx::new(&mut Budget::default(), &mut Diagnostics::new(), &cancel)
        ),
        Err(Error::Stopped(_))
    ));
    let mut budget = Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(0)));
    assert!(matches!(
        BuildingProfile::load(
            &[("b", input::BASE)],
            "mlit-bim-drawing",
            "2026-03",
            &mut Cx::new(&mut budget, &mut Diagnostics::new(), &CancelToken::new())
        ),
        Err(Error::Stopped(_))
    ));
}

#[test]
fn profile_depth_and_unknown_rules_fail_explicitly() {
    let mut budget = Budget::new(Limits::default().with(Kind::Depth, Limit::Finite(2)));
    let result = BuildingProfile::load(
        &[("b", input::BASE)],
        "mlit-bim-drawing",
        "2026-03",
        &mut Cx::new(&mut budget, &mut Diagnostics::new(), &CancelToken::new()),
    );
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(budget.used(Kind::Depth), 0);
    let data = br#"{"format":"0.1","id":"test","edition":"1","revision":1,"rules":[{"id":"future","check":"future","level":"warning"}]}"#;
    let p = with(|cx| BuildingProfile::load(&[("test", data)], "test", "1", cx)).unwrap();
    let report = with(|cx| p.check(&[&building()], cx)).unwrap();
    assert_eq!(counts(&report, "future"), (0, 1));
    assert!(matches!(
        with(|cx| p.areas(&building(), &[], input::tolerance(), cx)),
        Err(Error::Profile(_))
    ));
    for invalid in [b"{".as_slice(),br#"{"format":"0.1","id":"test","edition":"1","revision":1,"extends":{"id":"test","edition":"1"}}"#] {
        assert!(with(|cx| BuildingProfile::load(&[("test",invalid)],"test","1",cx)).is_err());
    }
}

#[test]
fn inherited_area_policy_rejects_unknown_units_and_rounding_stages() {
    for (field, value) in [
        ("unit", "ft2"),
        ("boundary", "wall-center"),
        ("stage", "element"),
        ("rounding", "unknown"),
    ] {
        let data = r#"{"format":"0.1","id":"test","edition":"1","revision":1,"extends":{"id":"mlit-bim-drawing","edition":"2026-03"},"vocabularies":{"building-area":{"values":[{"boundary":"shape","unit":"m2","rounding":"truncate","digits":2,"stage":"total","groups":["storey"]}]}}}"#.replace(&format!("\"{field}\":\"{}\"", match field { "unit" => "m2", "boundary" => "shape", "stage" => "total", _ => "truncate" }), &format!("\"{field}\":\"{value}\""));
        let p = with(|cx| {
            BuildingProfile::load(
                &[("b", input::BASE), ("c", data.as_bytes())],
                "test",
                "1",
                cx,
            )
        })
        .unwrap();
        let groups = [AreaGroup {
            kind: Group::Storey,
            key: "1",
            elements: &[ElementRef(1)],
            origin: input::origin(),
        }];
        assert!(matches!(
            with(|cx| p.areas(&building(), &groups, input::tolerance(), cx)),
            Err(Error::Profile(_))
        ));
    }
}

#[test]
fn alignment_resolves_parent_order_and_does_not_guess_missing_parents_or_map_frames() {
    let a = building();
    let mut b = building();
    b.components.placement.insert(
        ElementRef(0),
        Placement {
            relative_to: Some(ElementRef(1)),
            local: Local::Transform(Transform::translation(-1.0, 0.0, 0.0)),
        },
    );
    b.components.placement.insert(
        ElementRef(1),
        Placement {
            relative_to: None,
            local: Local::Transform(Transform::translation(1.0, 0.0, 0.0)),
        },
    );
    let p = profile(false);
    assert_eq!(
        counts(
            &with(|cx| p.check(&[&a, &b], cx)).unwrap(),
            "models-aligned"
        ),
        (0, 0)
    );
    b.components.placement.insert(
        ElementRef(1),
        Placement {
            relative_to: Some(ElementRef(0)),
            local: Local::Transform(Transform::IDENTITY),
        },
    );
    assert_eq!(
        counts(
            &with(|cx| p.check(&[&a, &b], cx)).unwrap(),
            "models-aligned"
        ),
        (0, 1)
    );
    b = building();
    b.document.map = Some(MapConversion {
        crs_name: Some("test".into()),
        eastings: 0.0,
        northings: 0.0,
        height: 0.0,
        x_axis_abscissa: None,
        x_axis_ordinate: None,
        scale: None,
    });
    assert_eq!(
        counts(
            &with(|cx| p.check(&[&a, &b], cx)).unwrap(),
            "models-aligned"
        ),
        (0, 1)
    );
}

#[test]
fn logical_allocation_slots_cover_native_and_wasm_layouts() {
    assert!(std::mem::size_of::<zumen_profile::Identity>() <= 64);
    assert!(std::mem::size_of::<zumen_profile::RuleSummary>() <= 64);
    assert!(std::mem::size_of::<zumen_profile::Finding>() <= 256);
    assert!(std::mem::size_of::<zumen_profile::building::GroupArea>() <= 512);
}
