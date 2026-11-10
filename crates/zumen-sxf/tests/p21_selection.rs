//! P21 の実体・用紙・レイヤの選択と、採用しない所の診断。入力はテスト内で作る。
#![cfg(feature = "drawing")]

use zumen_base::{
    Budget, CancelToken, Code, Cx, Diagnostic, Diagnostics, Key, Kind, Limit, Limits, Ref, Value,
    codes,
};
use zumen_drawing::{Changes, Drawing2D, id};
use zumen_sdk::{Apply, Error, Lift, testkit};
use zumen_sxf::{Sxf, SxfFile};

const GEOMETRY: &str = concat!(
    "#1=PRESENTATION_STYLE_ASSIGNMENT(());\n",
    "#2=CARTESIAN_POINT(' ',(1.,2.));\n",
    "#3=CARTESIAN_POINT(' ',(4.,6.));\n",
    "#4=POLYLINE(' ',(#2,#3));\n",
    "#5=ANNOTATION_CURVE_OCCURRENCE(' ',(#1),#4);\n",
);
const LAYER: &str = "#20=PRESENTATION_LAYER_ASSIGNMENT('L',' ',(#5));\n";
const SHEET: &str = "#70=DRAWING_SHEET_REVISION('A3_horizontal',(#5),$,'01');\n";

#[cfg(test)]
fn p21(instances: &str) -> Vec<u8> {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('SCADEC level2 AP202_mode'),'2;1');\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n{instances}ENDSEC;\nEND-ISO-10303-21;\n"
    )
    .into_bytes()
}

#[cfg(test)]
fn read(src: &[u8]) -> SxfFile {
    let (file, diagnostics) = testkit::read::<Sxf>(src);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    file.unwrap()
}

#[cfg(test)]
fn run(
    file: &SxfFile,
    limit: Limit,
    cancelled: bool,
) -> (Result<Drawing2D, Error>, u64, Vec<Diagnostic>) {
    run_with_places(file, limit, cancelled, Diagnostics::DEFAULT_MAX_PLACES)
}

#[cfg(test)]
fn run_with_places(
    file: &SxfFile,
    limit: Limit,
    cancelled: bool,
    max_places: usize,
) -> (Result<Drawing2D, Error>, u64, Vec<Diagnostic>) {
    let mut budget = Budget::new(Limits::default().with(Kind::Work, limit));
    let mut diagnostics = Diagnostics::with_max_places(max_places);
    let cancel = CancelToken::new();
    if cancelled {
        cancel.cancel();
    }
    let result = file.lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
    (result, budget.used(Kind::Work), diagnostics.into_vec())
}

#[cfg(test)]
fn places(file: &SxfFile, id: u64) -> Vec<Ref> {
    file.records()
        .iter()
        .enumerate()
        .filter(|(_, record)| record.id() == Some(id))
        .map(|(at, _)| Ref::at_record(at as u64).with_key(Key::Instance(id)))
        .collect()
}

#[cfg(test)]
fn diagnostic(diagnostics: &[Diagnostic], code: Code, id: u64) -> &Diagnostic {
    diagnostics
        .iter()
        .find(|d| d.code() == code && d.param("id") == Some(&Value::UInt(id)))
        .unwrap_or_else(|| panic!("{code:?} #{id}: {diagnostics:?}"))
}

#[cfg(test)]
fn ids(drawing: &Drawing2D) -> Vec<String> {
    let mut ids = Vec::new();
    id::each_element(drawing, |_, id, _| {
        ids.push(id);
        Ok::<(), ()>(())
    })
    .unwrap();
    ids
}

#[test]
fn later_instances_are_reported_without_changing_the_first_geometry_or_ids() {
    let original = p21(&format!("{GEOMETRY}{LAYER}{SHEET}"));
    let expected = run(&read(&original), Limit::Unlimited, false).0.unwrap();
    let src = p21(&format!(
        "{GEOMETRY}{LAYER}{SHEET}{}",
        concat!(
            "#2=CARTESIAN_POINT(' ',(20.,30.));\n",
            "#2=CARTESIAN_POINT(' ',(40.,50.));\n",
            "#5=ANNOTATION_CURVE_OCCURRENCE(' ',(#1),#2);\n",
            "#20=PRESENTATION_LAYER_ASSIGNMENT('OTHER',' ',());\n",
            "#70=DRAWING_SHEET_REVISION('A4_vertical',(),$,'02');\n",
        )
    ));
    let file = read(&src);
    let (drawing, _, diagnostics) = run(&file, Limit::Unlimited, false);
    let drawing = drawing.unwrap();
    assert_eq!(drawing, expected);
    assert_eq!(ids(&drawing), ["sheet/0/i5"]);
    assert_eq!(diagnostics.len(), 4, "{diagnostics:?}");
    for id in [2, 5, 20, 70] {
        let dropped = diagnostic(&diagnostics, codes::SXF_P21_INSTANCE_DROPPED, id);
        let expected = places(&file, id);
        assert_eq!(dropped.count(), expected.len() as u64 - 1);
        assert_eq!(dropped.places(), &expected[1..]);
    }
    assert_eq!(testkit::write::<Sxf>(&file).unwrap(), src);
}

#[test]
fn the_smallest_sheet_number_wins_in_either_file_order() {
    let later = concat!(
        "#90=DRAWING_SHEET_REVISION('A4_vertical',(),$,'02');\n",
        "#80=DRAWING_SHEET_REVISION('A2_vertical',(#5,#5),$,'03');\n",
    );
    for sheets in [format!("{SHEET}{later}"), format!("{later}{SHEET}")] {
        let src = p21(&format!("{GEOMETRY}{LAYER}{sheets}"));
        let file = read(&src);
        let expected = p21(&format!("{GEOMETRY}{LAYER}{sheets}")
            .replace("#80=DRAWING_SHEET_REVISION", "#80=UNUSED_SHEET")
            .replace("#90=DRAWING_SHEET_REVISION", "#90=UNUSED_SHEET"));
        let (drawing, _, diagnostics) = run(&file, Limit::Unlimited, false);
        let drawing = drawing.unwrap();
        assert_eq!(
            drawing,
            run(&read(&expected), Limit::Unlimited, false).0.unwrap()
        );
        assert_eq!(ids(&drawing), ["sheet/0/i5"]);
        assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
        // 採用は番号の順、診断は入力順。場所は元のファイルのレコードを指す。
        for (dropped, id) in diagnostics.iter().zip([90, 80]) {
            assert_eq!(dropped.code(), codes::SXF_P21_SHEET_DROPPED);
            assert_eq!(dropped.param("id"), Some(&Value::UInt(id)));
            assert_eq!(dropped.count(), 1);
            assert_eq!(dropped.places(), places(&file, id));
        }
        assert_eq!(testkit::write::<Sxf>(&file).unwrap(), src);
    }
}

#[test]
fn an_empty_first_sheet_is_not_replaced_by_a_later_sheet() {
    let src = p21(&format!(
        "{GEOMETRY}{LAYER}{}{}",
        "#80=DRAWING_SHEET_REVISION('A4_vertical',(#5),$,'02');\n",
        "#70=DRAWING_SHEET_REVISION('A3_horizontal',(),$,'01');\n",
    ));
    let file = read(&src);
    let (drawing, _, diagnostics) = run(&file, Limit::Unlimited, false);
    let drawing = drawing.unwrap();
    assert!(drawing.sheets[0].elements.is_empty());
    let dropped = diagnostic(&diagnostics, codes::SXF_P21_SHEET_DROPPED, 80);
    assert_eq!(dropped.count(), 1);
    assert_eq!(dropped.places(), places(&file, 80));
}

#[test]
fn the_smallest_non_dummy_layer_wins_and_only_other_layers_are_reported() {
    let layers = concat!(
        "#40=PRESENTATION_LAYER_ASSIGNMENT('FIRST',' ',(#5));\n",
        "#30=PRESENTATION_LAYER_ASSIGNMENT('LATER',' ',(#5,#5));\n",
        "#20=PRESENTATION_LAYER_ASSIGNMENT('FIRST',' ',(#5,#5));\n",
        "#10=PRESENTATION_LAYER_ASSIGNMENT('$$SXF_dummy',' ',(#5));\n",
    );
    let src = p21(&format!("{GEOMETRY}{layers}{SHEET}"));
    let file = read(&src);
    let expected = p21(&format!("{GEOMETRY}{layers}{SHEET}")
        .replace("'LATER',' ',(#5,#5)", "'LATER',' ',()")
        .replace(
            "#40=PRESENTATION_LAYER_ASSIGNMENT('FIRST',' ',(#5))",
            "#40=PRESENTATION_LAYER_ASSIGNMENT('FIRST',' ',())",
        ));
    let (drawing, _, diagnostics) = run(&file, Limit::Unlimited, false);
    let drawing = drawing.unwrap();
    assert_eq!(
        drawing,
        run(&read(&expected), Limit::Unlimited, false).0.unwrap()
    );
    assert_eq!(ids(&drawing), ["sheet/0/i5"]);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    let dropped = diagnostic(&diagnostics, codes::SXF_P21_LAYER_MEMBER_DROPPED, 5);
    assert_eq!(dropped.count(), 3);
    assert_eq!(
        dropped.places(),
        [
            places(&file, 40)[0].clone(),
            places(&file, 30)[0].clone(),
            places(&file, 30)[0].clone(),
        ]
    );
    for max_places in [0, 1] {
        let limited = run_with_places(&file, Limit::Unlimited, false, max_places);
        assert_eq!(limited.0.unwrap(), drawing);
        assert_eq!(limited.2[0].count(), 3);
        assert_eq!(limited.2[0].places(), &places(&file, 40)[..max_places]);
    }
    assert_eq!(testkit::write::<Sxf>(&file).unwrap(), src);
}

#[test]
fn selection_reference_and_lifter_diagnostics_share_input_order_and_first_places() {
    let src = p21(&format!(
        "{GEOMETRY}{}",
        concat!(
            "#110=ANNOTATION_CURVE_OCCURRENCE(' ',(#1),#999);\n",
            "#50=PRESENTATION_LAYER_ASSIGNMENT('OTHER50',' ',(#5,#110));\n",
            "#90=DRAWING_SHEET_REVISION('A4_vertical',(),$,'02');\n",
            "#101=ANNOTATION_CURVE_OCCURRENCE(' ',(#1),#998);\n",
            "#2=CARTESIAN_POINT(' ',(20.,30.));\n",
            "#40=PRESENTATION_LAYER_ASSIGNMENT('OTHER40',' ',(#5,#110));\n",
            "#80=DRAWING_SHEET_REVISION('A2_vertical',(),$,'03');\n",
            "#20=PRESENTATION_LAYER_ASSIGNMENT('L',' ',(#5,#110,#101));\n",
            // 用紙内の並びも入力順と逆にして、Lifter の集約場所を確かめる。
            "#70=DRAWING_SHEET_REVISION('A3_horizontal',(#101,#110,#5),$,'01');\n",
        )
    ));
    let file = read(&src);
    let (drawing, _, full) = run(&file, Limit::Unlimited, false);
    let drawing = drawing.unwrap();
    assert_eq!(ids(&drawing), ["sheet/0/i5"]);
    assert_eq!(
        full.iter()
            .map(|d| (d.code(), d.places()[0].record().unwrap()))
            .collect::<Vec<_>>(),
        [
            (codes::SXF_P21_REFERENCE_MISSING, 11),
            (codes::SXF_LIFT_DROPPED, 11),
            (codes::SXF_P21_LAYER_MEMBER_DROPPED, 12),
            (codes::SXF_P21_LAYER_MEMBER_DROPPED, 12),
            (codes::SXF_P21_SHEET_DROPPED, 13),
            (codes::SXF_P21_INSTANCE_DROPPED, 15),
            (codes::SXF_P21_SHEET_DROPPED, 17),
        ],
        "{full:?}",
    );
    assert_eq!(full[0].count(), 2);
    assert_eq!(full[0].param("id"), Some(&Value::UInt(999)));
    assert_eq!(full[0].places(), [Ref::at_record(11), Ref::at_record(14)]);
    assert_eq!(full[1].count(), 2);
    assert_eq!(
        full[1].places(),
        [places(&file, 110)[0].clone(), places(&file, 101)[0].clone()]
    );
    assert_eq!(full[2].param("id"), Some(&Value::UInt(5)));
    assert_eq!(full[3].param("id"), Some(&Value::UInt(110)));
    for dropped in &full[2..4] {
        assert_eq!(dropped.count(), 2);
        assert_eq!(
            dropped.places(),
            [places(&file, 50)[0].clone(), places(&file, 40)[0].clone()]
        );
    }
    for max_places in [0, 1] {
        let (limited, _, diagnostics) = run_with_places(&file, Limit::Unlimited, false, max_places);
        assert_eq!(limited.unwrap(), drawing);
        assert_eq!(diagnostics.len(), full.len());
        for (limited, original) in diagnostics.iter().zip(&full) {
            assert_eq!(limited.code(), original.code());
            assert_eq!(limited.params(), original.params());
            assert_eq!(limited.count(), original.count());
            assert_eq!(limited.places(), &original.places()[..max_places]);
        }
    }
    assert_eq!(testkit::write::<Sxf>(&file).unwrap(), src);
}

/// SFC も、表と図形の変換の段階をまたいで、診断を元のレコードの順に返す。同じ診断の場所も入力の先頭から残す（#624）
#[test]
fn sfc_returns_diagnostics_in_record_order() {
    let src = String::from_utf8(p21(concat!(
        "/*SXF3\n#10=future_feature()\nSXF3*/\n",
        "/*SXF3\n#20=layer_feature()\nSXF3*/\n",
        "/*SXF3\n#40=future_feature()\nSXF3*/\n",
        "/*SXF3\n#30=drawing_sheet_feature(\'S\','3','1','0','0')\nSXF3*/\n",
    )))
    .unwrap()
    .replace("AP202_mode", "feature_mode");
    let file = read(src.as_bytes());
    let (drawing, _, full) = run_with_places(&file, Limit::Unlimited, false, usize::MAX);
    let drawing = drawing.unwrap();
    assert_eq!(
        full.iter()
            .map(|d| (d.code(), d.count(), d.places().to_vec()))
            .collect::<Vec<_>>(),
        [
            (
                codes::SXF_LIFT_DROPPED,
                2,
                [places(&file, 10), places(&file, 40)].concat()
            ),
            (codes::SXF_LIFT_INVALID, 1, places(&file, 20)),
        ]
    );
    for max_places in [0, 1] {
        // 集める手間は、残す場所の数で変わる
        let (limited, used, diagnostics) =
            run_with_places(&file, Limit::Unlimited, false, max_places);
        assert_eq!(limited.unwrap(), drawing);
        assert_eq!(diagnostics.len(), full.len());
        for (limited, original) in diagnostics.iter().zip(&full) {
            assert_eq!(limited.code(), original.code());
            assert_eq!(limited.params(), original.params());
            assert_eq!(limited.count(), original.count());
            assert_eq!(limited.places(), &original.places()[..max_places]);
        }
        let (exact, exact_used, _) = run_with_places(&file, Limit::Finite(used), false, max_places);
        assert!(exact.is_ok());
        assert_eq!(exact_used, used);
        let (short, _, stopped) =
            run_with_places(&file, Limit::Finite(used - 1), false, max_places);
        assert!(matches!(short, Err(Error::Stopped(_))));
        assert!(
            stopped
                .iter()
                .any(|d| d.code() == codes::BUDGET_WORK_EXCEEDED)
        );
        let (cancelled, _, _) = run_with_places(&file, Limit::Unlimited, true, max_places);
        assert!(matches!(cancelled, Err(Error::Stopped(_))));
    }
    assert_eq!(testkit::write::<Sxf>(&file).unwrap(), src.as_bytes());
}

#[test]
fn p21_lift_inside_record_order_keeps_the_same_budget_stops_and_reports() {
    let src = p21(&format!(
        "{GEOMETRY}{LAYER}{SHEET}{}",
        concat!(
            "#2=CARTESIAN_POINT(' ',(20.,30.));\n",
            "#30=PRESENTATION_LAYER_ASSIGNMENT('LATER',' ',(#5));\n",
            "#80=DRAWING_SHEET_REVISION('A4_vertical',(),$,'02');\n",
        )
    ));
    let file = read(&src);
    for max_places in [0, 1] {
        let full = run_with_places(&file, Limit::Unlimited, false, max_places);
        for (limit, cancelled) in [
            (Limit::Unlimited, false),
            (Limit::Finite(full.1), false),
            (Limit::Finite(full.1 - 1), false),
            (Limit::Finite(18), false),
            (Limit::Unlimited, true),
        ] {
            let expected = run_with_places(&file, limit, cancelled, max_places);
            let mut budget = Budget::new(Limits::default().with(Kind::Work, limit));
            let mut diagnostics = Diagnostics::with_max_places(max_places);
            let cancel = CancelToken::new();
            if cancelled {
                cancel.cancel();
            }
            let result = Cx::new(&mut budget, &mut diagnostics, &cancel)
                .with_diagnostics_in_record_order(|cx| {
                    let inner = file.lift(cx);
                    assert_eq!(inner, expected.0);
                    inner
                });
            assert_eq!(
                (result, budget.used(Kind::Work), diagnostics.into_vec()),
                expected
            );
        }
        // 呼び出し側が先に報告した後方の場所も、P21 の診断と同じ入力順に並ぶ。
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::with_max_places(max_places);
        let cancel = CancelToken::new();
        let result = Cx::new(&mut budget, &mut diagnostics, &cancel)
            .with_diagnostics_in_record_order(|cx| {
                cx.report(
                    Diagnostic::new(codes::ZIP_PATH_UNSAFE)
                        .with("name", "outer")
                        .at(Ref::at_record(16)),
                );
                file.lift(cx)
            });
        assert_eq!(result, full.0);
        assert_eq!(budget.used(Kind::Work), full.1 + 4 + max_places as u64);
        let diagnostics = diagnostics.into_vec();
        assert_eq!(&diagnostics[..full.2.len()], full.2);
        assert_eq!(diagnostics.last().unwrap().code(), codes::ZIP_PATH_UNSAFE);
        assert_eq!(
            diagnostics.last().unwrap().places(),
            &[Ref::at_record(16)][..max_places]
        );
    }
    assert_eq!(testkit::write::<Sxf>(&file).unwrap(), src);
}

#[test]
fn ignored_instances_sheets_and_layer_members_obey_work_and_cancellation() {
    let src = p21(&format!(
        "{GEOMETRY}{LAYER}{SHEET}{}",
        concat!(
            "#2=CARTESIAN_POINT(' ',(20.,30.));\n",
            "#30=PRESENTATION_LAYER_ASSIGNMENT('LATER',' ',(#5));\n",
            "#80=DRAWING_SHEET_REVISION('A4_vertical',(),$,'02');\n",
        )
    ));
    let file = read(&src);
    let (full, used, _) = run(&file, Limit::Unlimited, false);
    assert!(full.is_ok());
    let duplicate_at = places(&file, 2)[1].record().unwrap();
    // レコードの走査と、採用した実体の参照の検査の後に、レイヤと用紙を処理する。
    let before_tables = file.records().len() as u64
        + file
            .records()
            .iter()
            .filter_map(|r| r.id())
            .collect::<std::collections::BTreeSet<_>>()
            .len() as u64;
    for (limit, place, code) in [
        (
            duplicate_at,
            places(&file, 2)[1].clone(),
            codes::SXF_P21_INSTANCE_DROPPED,
        ),
        (
            before_tables + 3 + 5,
            places(&file, 30)[0].clone(),
            codes::SXF_P21_LAYER_MEMBER_DROPPED,
        ),
        (
            before_tables + 4 + 10,
            places(&file, 80)[0].clone(),
            codes::SXF_P21_SHEET_DROPPED,
        ),
    ] {
        let result = run(&file, Limit::Finite(limit), false);
        assert!(
            matches!(result.0, Err(Error::Stopped(_))),
            "{limit}: {result:?}"
        );
        let stopped = result
            .2
            .iter()
            .find(|d| d.code() == codes::BUDGET_WORK_EXCEEDED)
            .unwrap();
        assert_eq!(stopped.places(), [place]);
        assert!(result.2.iter().all(|d| d.code() != code));
        assert_eq!(result, run(&file, Limit::Finite(limit), false));
        // 元の処理 1 回を通っても、収集・整列・転送の 5 Work に足りなければ診断を保持しない。
        let collecting = run(&file, Limit::Finite(limit + 1 + 4), false);
        assert!(matches!(collecting.0, Err(Error::Stopped(_))));
        assert!(collecting.2.iter().all(|d| d.code() != code));
        assert_eq!(
            collecting
                .2
                .iter()
                .find(|d| d.code() == codes::BUDGET_WORK_EXCEEDED)
                .unwrap()
                .places(),
            stopped.places()
        );
        assert!(
            run(&file, Limit::Finite(limit + 1 + 5), false)
                .2
                .iter()
                .any(|d| d.code() == code)
        );
    }
    assert!(run(&file, Limit::Finite(used), false).0.is_ok());
    assert!(matches!(
        run(&file, Limit::Finite(used - 1), false).0,
        Err(Error::Stopped(_))
    ));
    let cancelled = run(&file, Limit::Unlimited, true);
    assert!(matches!(cancelled.0, Err(Error::Stopped(_))));
    assert_eq!(cancelled.1, 0);
    assert_eq!(cancelled.2.len(), 1);
    assert_eq!(cancelled.2[0].code(), codes::CALL_REQUEST_CANCELLED);
    assert_eq!(cancelled.2[0].places(), [Ref::at_record(0)]);
    assert_eq!(testkit::write::<Sxf>(&file).unwrap(), src);
}

#[test]
fn applying_no_changes_preserves_all_ignored_records_without_duplicate_reports() {
    let src = p21(&format!(
        "{GEOMETRY}{LAYER}{SHEET}{}",
        concat!(
            "#2=CARTESIAN_POINT(' ',(20.,30.));\n",
            "#30=PRESENTATION_LAYER_ASSIGNMENT('LATER',' ',(#5));\n",
            "#80=DRAWING_SHEET_REVISION('A4_vertical',(),$,'02');\n",
        )
    ));
    let mut file = read(&src);
    let expected = run(&file, Limit::Unlimited, false).0.unwrap();
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let applied = file
        .apply(
            &Changes {
                edits: Vec::new(),
                document: None,
            },
            &mut Cx::new(&mut budget, &mut diagnostics, &CancelToken::new()),
        )
        .unwrap();
    assert!(applied.is_empty());
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(testkit::write::<Sxf>(&file).unwrap(), src);
    assert_eq!(run(&file, Limit::Unlimited, false).0.unwrap(), expected);
}
