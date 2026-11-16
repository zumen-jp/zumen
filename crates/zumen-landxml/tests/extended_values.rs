//! 追加距離・左右の符号・元の場所を、既知値を持つ最小の交換データで確かめる。
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "固定入力の検査"
)]
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_civil::{CantKind, CivilModel};
use zumen_landxml::LandXml;
use zumen_sdk::{Codec, Lift};

fn document(body: &str) -> String {
    format!(
        r#"<LandXML xmlns="http://www.landxml.org/schema/LandXML-1.2"><CoordinateSystem horizontalDatum="JGD2011" verticalDatum="T.P" horizontalCoordinateSystemName="9(X,Y)"/><Alignments><Alignment name="a" staStart="1000" length="200">{body}</Alignment></Alignments></LandXML>"#
    )
}
fn read(src: &str) -> (CivilModel, Diagnostics) {
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let model = LandXml::read(src.as_bytes(), &mut cx)
        .unwrap()
        .lift(&mut cx)
        .unwrap();
    (model, diagnostics)
}

#[test]
fn slope_list_keeps_signed_percent_and_replaces_superelevation_in_either_order() {
    for single in [false, true] {
        let values = if single {
            ["1010 -2", "1025 3", "1040 0"]
        } else {
            ["1010 -2 2", "1025 3 4", "1040 0 -1"]
        };
        let config = format!(
            r#"<Feature name="SuperelevationConfig"><Property label="singleLaneRoad" value="{single}"/><Property label="useSlopeList" value="true"/></Feature>"#
        );
        let list = format!(
            r#"<Feature name="SlopeList">{}</Feature>"#,
            values
                .iter()
                .map(|v| format!(r#"<Property label="slopeValue" value="{v}"/>"#))
                .collect::<String>()
        );
        let replaced = r#"<Superelevation staStart="1010" staEnd="1040"><FullSuperelev>8</FullSuperelev></Superelevation>"#;
        for body in [
            format!("{config}{list}{replaced}"),
            format!("{replaced}{list}{config}"),
        ] {
            let src = document(&body);
            let (model, diagnostics) = read(&src);
            let cant = &model.alignments[0].cant;
            assert_eq!(cant.len(), 2);
            assert_eq!((cant[0].start_distance, cant[0].length), (10.0, 15.0));
            assert_eq!((cant[1].start_distance, cant[1].length), (25.0, 15.0));
            assert_eq!(
                cant[0].kind,
                CantKind::Superelevation {
                    start: if single { [-2.0; 2] } else { [-2.0, 2.0] },
                    end: if single { [3.0; 2] } else { [3.0, 4.0] }
                }
            );
            assert_eq!(
                cant[1].kind,
                CantKind::Superelevation {
                    start: if single { [3.0; 2] } else { [3.0, 4.0] },
                    end: if single { [0.0; 2] } else { [0.0, -1.0] }
                }
            );
            assert_eq!(cant[0].transition.as_deref(), Some("linear"));
            let span = cant[0].source.bytes().unwrap();
            assert_eq!(
                src.get(span.start as usize..span.end as usize).unwrap(),
                format!(r#"<Property label="slopeValue" value="{}"/>"#, values[0])
            );
            assert_eq!(diagnostics.iter().map(|d| d.count()).sum::<u64>(), 1);
            assert_eq!(
                diagnostics.iter().next().unwrap().code(),
                codes::LANDXML_ELEMENT_UNSUPPORTED
            );
        }
    }
}

#[test]
fn invalid_slope_list_does_not_bridge_bad_or_repeated_points() {
    for bad in [
        "1020 bad 3",
        "1010 1 2",
        "1009 1 2",
        "1020 1",
        "NaN 1 2",
        "1020 1 2 3",
    ] {
        let src = document(&format!(
            r#"<Feature name="SlopeList"><Property label="slopeValue" value="1010 -2 2"/><Property label="slopeValue" value="{bad}"/><Property label="slopeValue" value="1040 3 3"/></Feature>"#
        ));
        let (model, diagnostics) = read(&src);
        assert!(model.alignments[0].cant.is_empty(), "{bad}");
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code() == codes::LANDXML_VALUE_INVALID)
        );
    }
}

#[test]
fn transition_knots_keep_normal_crown_and_both_signed_slopes() {
    for full in [-6.0, 6.0] {
        for reverse in [false, true] {
            let bends = if reverse {
                r#"<Feature name="ReverseCrown"><Property label="sta" value="1035"/><Property label="sta" value="1115"/></Feature>"#
            } else {
                ""
            };
            let src = document(&format!(
                r#"<Superelevation><BeginRunoutSta>1010</BeginRunoutSta><BeginRunoffSta>1020</BeginRunoffSta><FullSuperSta>1050</FullSuperSta><FullSuperelev>{full}</FullSuperelev><RunoffSta>1100</RunoffSta><StartofRunoutSta>1130</StartofRunoutSta><EndofRunoutSta>1140</EndofRunoutSta>{bends}</Superelevation><Feature name="SuperelevationConfig"><Property label="normalCrown" value="2"/></Feature>"#
            ));
            let (model, diag) = read(&src);
            assert!(diag.is_empty(), "{diag:?}");
            let cant = &model.alignments[0].cant;
            assert_eq!(cant.len(), 7);
            let same = if full < 0.0 { [-2.0; 2] } else { [2.0; 2] };
            let flat = if full < 0.0 { [-2.0, 0.0] } else { [0.0, 2.0] };
            let knots = [
                (10.0, [-2.0, 2.0]),
                (20.0, flat),
                (if reverse { 35.0 } else { 30.0 }, same),
                (50.0, [full; 2]),
                (100.0, [full; 2]),
                (if reverse { 115.0 } else { 120.0 }, same),
                (130.0, flat),
                (140.0, [-2.0, 2.0]),
            ];
            for (part, expected) in cant.iter().zip(knots.windows(2)) {
                assert_eq!(part.start_distance, expected[0].0);
                assert_eq!(part.length, expected[1].0 - expected[0].0);
                assert_eq!(
                    part.kind,
                    CantKind::Superelevation {
                        start: expected[0].1,
                        end: expected[1].1
                    }
                );
                assert!(part.source.bytes().is_some());
            }
        }
    }
}

#[test]
fn non_adverse_and_single_lane_transitions_do_not_add_a_waiting_interval() {
    for single in [false, true] {
        let normal = if single { -2.0 } else { 2.0 };
        let src = document(&format!(
            r#"<Feature name="SuperelevationConfig"><Property label="normalCrown" value="{normal}"/><Property label="singleLaneRoad" value="{single}"/></Feature><Superelevation><BeginRunoutSta>1010</BeginRunoutSta><FullSuperSta>1050</FullSuperSta><FullSuperelev>6</FullSuperelev><RunoffSta>1100</RunoffSta><EndofRunoutSta>1140</EndofRunoutSta><AdverseSE>non-adverse</AdverseSE></Superelevation>"#
        ));
        let (model, diag) = read(&src);
        assert!(diag.is_empty(), "{diag:?}");
        let cant = &model.alignments[0].cant;
        assert_eq!(cant.len(), 3);
        let normal = if single { [-2.0; 2] } else { [-2.0, 2.0] };
        assert_eq!(
            cant[0].kind,
            CantKind::Superelevation {
                start: normal,
                end: [6.0; 2]
            }
        );
        assert_eq!(
            cant[2].kind,
            CantKind::Superelevation {
                start: [6.0; 2],
                end: normal
            }
        );
    }
}

#[test]
fn normal_crown_alone_covers_the_explicit_alignment_length() {
    for single in [false, true] {
        let normal = if single { -1.5 } else { 1.5 };
        let src = document(&format!(
            r#"<Feature name="SuperelevationConfig"><Property label="normalCrown" value="{normal}"/><Property label="singleLaneRoad" value="{single}"/></Feature>"#
        ));
        let (model, diag) = read(&src);
        assert!(diag.is_empty());
        let [cant] = model.alignments[0].cant.as_slice() else {
            panic!("constant")
        };
        assert_eq!((cant.start_distance, cant.length), (0.0, 200.0));
        let slopes = if single { [-1.5; 2] } else { [-1.5, 1.5] };
        assert_eq!(
            cant.kind,
            CantKind::Superelevation {
                start: slopes,
                end: slopes
            }
        );
    }
}

#[test]
fn adjacent_curves_follow_flat_and_reverse_crown_stations() {
    for (next_slope, bends, expected) in [
        (-4.0, false, vec![(20.0, 6.0), (40.0, 0.0), (60.0, -4.0)]),
        (
            -4.0,
            true,
            vec![
                (20.0, 6.0),
                (30.0, 2.0),
                (40.0, 0.0),
                (50.0, -2.0),
                (60.0, -4.0),
            ],
        ),
        (
            4.0,
            true,
            vec![(20.0, 6.0), (30.0, 2.0), (50.0, 2.0), (60.0, 4.0)],
        ),
    ] {
        let first_reverse = if bends {
            r#"<Feature name="ReverseCrown"><Property label="sta" value="1030"/></Feature>"#
        } else {
            ""
        };
        let next_reverse = if bends {
            r#"<Feature name="ReverseCrown"><Property label="sta" value="1050"/></Feature>"#
        } else {
            ""
        };
        let flat = if next_slope < 0.0 {
            r#"<Feature name="FlatSta"><Property label="sta" value="1040"/></Feature>"#
        } else {
            ""
        };
        let body = format!(
            r#"<Feature name="SuperelevationConfig"><Property label="normalCrown" value="2"/></Feature><Superelevation staStart="1010" staEnd="1020"><FullSuperelev>6</FullSuperelev>{first_reverse}{flat}</Superelevation><Superelevation staStart="1060" staEnd="1080"><FullSuperelev>{next_slope}</FullSuperelev>{next_reverse}</Superelevation>"#
        );
        let (model, diag) = read(&document(&body));
        assert!(diag.is_empty(), "{diag:?}");
        let cant = &model.alignments[0].cant;
        assert_eq!(cant.len(), expected.len() + 1);
        for (part, pair) in cant[1..cant.len() - 1].iter().zip(expected.windows(2)) {
            assert_eq!(part.start_distance, pair[0].0);
            assert_eq!(part.length, pair[1].0 - pair[0].0);
            assert_eq!(
                part.kind,
                CantKind::Superelevation {
                    start: [pair[0].1; 2],
                    end: [pair[1].1; 2]
                }
            );
        }
        if bends && next_slope < 0.0 {
            let (model, diag) = read(&document(&body.replace(flat, "")));
            assert_eq!(model.alignments[0].cant.len(), 2);
            assert!(
                diag.iter()
                    .any(|d| d.code() == codes::LANDXML_VALUE_INVALID)
            );
        }
    }
}

#[test]
fn slope_list_reserves_memory_work_and_elements_before_returning_values() {
    let src = document(
        r#"<Feature name="SlopeList"><Property label="slopeValue" value="1010 -2 2"/><Property label="slopeValue" value="1040 3 3"/></Feature>"#,
    );
    let mut budget = Budget::default();
    let file = LandXml::read(
        src.as_bytes(),
        &mut Cx::new(&mut budget, &mut Diagnostics::new(), &CancelToken::new()),
    )
    .unwrap();
    for kind in [Kind::DecodedBytes, Kind::Work, Kind::Elements] {
        let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(0)));
        assert!(
            file.lift(&mut Cx::new(
                &mut budget,
                &mut Diagnostics::new(),
                &CancelToken::new()
            ))
            .is_err()
        );
    }
}
