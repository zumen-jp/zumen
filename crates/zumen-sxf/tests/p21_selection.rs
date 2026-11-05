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
    let mut budget = Budget::new(Limits::default().with(Kind::Work, limit));
    let mut diagnostics = Diagnostics::new();
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
        // 診断は番号の順。場所は元のファイルのレコードを指す。
        for (dropped, id) in diagnostics.iter().zip([80, 90]) {
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
        "#30=PRESENTATION_LAYER_ASSIGNMENT('LATER',' ',(#5,#5));\n",
        "#20=PRESENTATION_LAYER_ASSIGNMENT('FIRST',' ',(#5,#5));\n",
        "#10=PRESENTATION_LAYER_ASSIGNMENT('$$SXF_dummy',' ',(#5));\n",
        "#40=PRESENTATION_LAYER_ASSIGNMENT('FIRST',' ',(#5));\n",
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
            places(&file, 30)[0].clone(),
            places(&file, 30)[0].clone(),
            places(&file, 40)[0].clone()
        ]
    );
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
            before_tables + 3,
            places(&file, 30)[0].clone(),
            codes::SXF_P21_LAYER_MEMBER_DROPPED,
        ),
        (
            before_tables + 4,
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
        assert!(
            run(&file, Limit::Finite(limit + 1), false)
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
