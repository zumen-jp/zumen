//! #520: 注釈外観の採用、配置、独立した描画状態と既存 PAGE の維持。
#![cfg(test)]
#![cfg(feature = "drawing")]

use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, Stop};
use zumen_drawing::{Drawing2D, Element, Hash, LayerId, PaperPoint, Shape, id};
use zumen_pdf::Pdf;
use zumen_sdk::{Error, Lift, testkit};

mod common;
#[path = "common/appearance.rs"]
mod fixture;

use fixture::{Annotation, Appearance, LINE};

fn elements(drawing: &Drawing2D) -> &[Element<PaperPoint>] {
    &drawing.sheets[0].elements
}

fn count(diagnostics: &[Diagnostic], code: &str) -> u64 {
    diagnostics
        .iter()
        .filter(|d| d.code().name() == code)
        .map(Diagnostic::count)
        .sum()
}

fn ids(drawing: &Drawing2D, sheet: usize) -> Vec<String> {
    let place = format!("sheet/{sheet}");
    let mut namer = id::Namer::new(&place);
    drawing.sheets[sheet]
        .elements
        .iter()
        .enumerate()
        .map(|(index, element)| namer.next(index, element))
        .collect()
}

fn point(found: &PaperPoint, expected: [f64; 2]) {
    assert!(
        common::near(found.x, common::mm(expected[0]))
            && common::near(found.y, common::mm(expected[1])),
        "{found:?}: {expected:?}"
    );
}

fn line(element: &Element<PaperPoint>, start: [f64; 2], end: [f64; 2]) {
    let Shape::Line {
        start: found_start,
        end: found_end,
    } = &element.shape
    else {
        panic!("線分でない: {:?}", element.shape)
    };
    point(found_start, start);
    point(found_end, end);
}

fn adopted(input: &fixture::Fixture, count_expected: usize) -> (Drawing2D, Vec<Diagnostic>) {
    let (drawing, diagnostics) = common::lift(&input.bytes);
    assert_eq!(elements(&drawing).len(), count_expected, "{diagnostics:?}");
    assert_eq!(
        count(&diagnostics, "pdf.annotation.unsupported"),
        0,
        "{diagnostics:?}"
    );
    let drawn = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code().name() == "pdf.annotation.drawn")
        .unwrap();
    assert_eq!(drawn.fidelity(), Some(zumen_base::Fidelity::Drawn));
    assert_eq!(drawn.count(), input.annotation_objects.len() as u64);
    assert_eq!(
        drawn.param("kind"),
        Some(&zumen_base::Value::Text("Stamp".to_owned()))
    );
    assert_eq!(
        drawn
            .places()
            .iter()
            .map(|place| place.record())
            .collect::<Vec<_>>(),
        input
            .annotation_objects
            .iter()
            .map(|&object| Some(u64::from(object) + 1))
            .collect::<Vec<_>>()
    );
    (drawing, diagnostics)
}

fn fallback(input: &fixture::Fixture, annotations: u64) {
    let (drawing, diagnostics) = common::lift(&input.bytes);
    let (expected, old_diagnostics) = common::lift(&input.without_annotations);
    assert_eq!(drawing, expected, "{diagnostics:?}");
    assert_eq!(
        count(&diagnostics, "pdf.annotation.unsupported"),
        annotations,
        "{diagnostics:?}"
    );
    let other: Vec<_> = diagnostics
        .into_iter()
        .filter(|d| d.code().name() != "pdf.annotation.unsupported")
        .collect();
    assert_eq!(
        other, old_diagnostics,
        "AP を採らないときに走査の診断を混ぜない"
    );
}

#[test]
fn places_normal_streams_using_rect_bbox_matrix_and_page_rotation() {
    for (appearance, rect, start, end) in [
        (
            Appearance::new(LINE),
            "[20 30 60 50]",
            [20.0, 30.0],
            [60.0, 50.0],
        ),
        (
            Appearance::new(LINE),
            "[60 50 20 30]",
            [20.0, 30.0],
            [60.0, 50.0],
        ),
        (
            Appearance::new("10 20 m 20 30 l S").bbox("[10 20 20 30]"),
            "[20 30 60 50]",
            [20.0, 30.0],
            [60.0, 50.0],
        ),
        (
            Appearance::new(LINE).extra("/Matrix [0 1 -1 0 10 0]"),
            "[20 30 60 50]",
            [60.0, 30.0],
            [20.0, 50.0],
        ),
    ] {
        // AP に Type/Subtype がなくても、通常外観として描く。
        let input = fixture::make("", &[appearance], &[Annotation::new(0).rect(rect)]);
        let (drawing, _) = adopted(&input, 1);
        line(&elements(&drawing)[0], start, end);
    }
    let input = fixture::with_page_extra(
        "",
        "/Rotate 90",
        &[Appearance::new(LINE)],
        &[Annotation::new(0).rect("[20 30 60 50]")],
    );
    let (drawing, _) = adopted(&input, 1);
    line(&elements(&drawing)[0], [30.0, 180.0], [50.0, 140.0]);
}

#[test]
fn appends_appearances_in_annotation_order_and_reuses_normal_streams() {
    let input = fixture::make(
        "1 1 m 2 2 l S",
        &[Appearance::new(LINE)],
        &[
            Annotation::new(0).rect("[20 30 60 50]"),
            Annotation::new(0).rect("[80 90 100 120]"),
        ],
    );
    let (drawing, _) = adopted(&input, 3);
    let (baseline, _) = common::lift(&input.without_annotations);
    assert_eq!(elements(&drawing)[0], elements(&baseline)[0]);
    assert_eq!(ids(&drawing, 0)[0], ids(&baseline, 0)[0]);
    assert_eq!(
        Hash::element(&elements(&drawing)[0]),
        Hash::element(&elements(&baseline)[0])
    );
    line(&elements(&drawing)[1], [20.0, 30.0], [60.0, 50.0]);
    line(&elements(&drawing)[2], [80.0, 90.0], [100.0, 120.0]);
    assert_eq!(drawing.sheets[0].order.len(), 3);
}

#[test]
fn inherits_page_resources_and_uses_form_and_appearance_overrides() {
    let input = fixture::make(
        "/Im Do",
        &[
            Appearance::new("/Fm Do /Rebound Do"),
            Appearance::new("/Im Do").extra("/Resources << /XObject << /Im 8 0 R >> >>"),
        ],
        &[Annotation::new(0), Annotation::new(1)],
    );
    let (drawing, _) = adopted(&input, 4);
    let paths: Vec<_> = elements(&drawing)
        .iter()
        .map(|element| {
            let Shape::Underlay(image) = &element.shape else {
                panic!("画像でない")
            };
            image.resource.path.as_str()
        })
        .collect();
    assert_eq!(paths, ["7 0 R", "7 0 R", "8 0 R", "8 0 R"]);
    assert_eq!(
        elements(&drawing)
            .iter()
            .map(|element| element.layer.0)
            .collect::<Vec<_>>(),
        [1, 1, 2, 2]
    );
}

#[test]
fn selects_as_then_off_in_state_dictionaries() {
    for (states, selected, start, end) in [
        (
            vec![("On", 0), ("Off", 1)],
            Some("On"),
            [0.0, 0.0],
            [10.0, 10.0],
        ),
        (
            vec![("On", 0), ("Off", 1)],
            Some("Off"),
            [0.0, 10.0],
            [10.0, 0.0],
        ),
        (
            vec![("On", 0), ("Off", 1)],
            Some("Missing"),
            [0.0, 10.0],
            [10.0, 0.0],
        ),
        (vec![("On", 0), ("Off", 1)], None, [0.0, 10.0], [10.0, 0.0]),
        // AS の参照が stream に解けない場合も Off を試す。
        (
            vec![("On", 999), ("Off", 1)],
            Some("On"),
            [0.0, 10.0],
            [10.0, 0.0],
        ),
    ] {
        let input = fixture::make(
            "",
            &[Appearance::new(LINE), Appearance::new("0 10 m 10 0 l S")],
            &[Annotation::states(&states, selected)],
        );
        let (drawing, _) = adopted(&input, 1);
        line(&elements(&drawing)[0], start, end);
    }
    fallback(
        &fixture::make(
            "1 1 m 2 2 l S",
            &[Appearance::new(LINE)],
            &[Annotation::states(&[("On", 0)], None)],
        ),
        1,
    );
}

#[test]
fn keeps_page_and_appearance_image_contexts_separate() {
    let input = fixture::make(
        "/Plain Do",
        &[Appearance::new("/Plain Do").extra("/OC 6 0 R")],
        &[Annotation::new(0)],
    );
    let (drawing, diagnostics) = adopted(&input, 2);
    assert_eq!(
        elements(&drawing)
            .iter()
            .map(|e| e.layer.0)
            .collect::<Vec<_>>(),
        [0, 2]
    );
    assert_eq!(count(&diagnostics, "pdf.optional-content.approximated"), 1);
    let (baseline, _) = common::lift(&input.without_annotations);
    assert_eq!(elements(&drawing)[0], elements(&baseline)[0]);
    // AP 自身の OC が既定で非表示なら、外観を描かず既存の OCG 診断を保つ。
    let hidden = fixture::with_off(
        "1 1 m 2 2 l S",
        &[Appearance::new(LINE).extra("/OC 6 0 R")],
        &[Annotation::new(0)],
    );
    let (_, diagnostics) = adopted(&hidden, 1);
    assert_eq!(count(&diagnostics, "pdf.optional-content.dropped"), 1);
}

#[test]
fn does_not_carry_broken_page_marks_or_graphics_into_appearances() {
    for page in [
        "/OC /b BDC 1 1 m 2 2 l S",
        "Q 1 0 0 1 100 0 cm 1 1 m 2 2 l S",
    ] {
        let input = fixture::make(page, &[Appearance::new(LINE)], &[Annotation::new(0)]);
        let (drawing, _) = adopted(&input, 2);
        let (baseline, _) = common::lift(&input.without_annotations);
        assert_eq!(elements(&drawing)[0], elements(&baseline)[0]);
        assert_eq!(elements(&drawing)[1].layer, LayerId(0));
        line(&elements(&drawing)[1], [0.0, 0.0], [10.0, 10.0]);
    }
}

#[test]
fn does_not_merge_text_across_page_appearance_or_nested_form_boundaries() {
    let page_ap = fixture::make(
        fixture::TEXT_FILL,
        &[Appearance::new(fixture::TEXT_STROKE)],
        &[Annotation::new(0)],
    );
    let ap_ap = fixture::make(
        "",
        &[
            Appearance::new(fixture::TEXT_FILL),
            Appearance::new(fixture::TEXT_STROKE),
        ],
        &[Annotation::new(0), Annotation::new(1)],
    );
    let parent = format!("{} /Nested Do", fixture::TEXT_FILL);
    let nested = fixture::make(
        "",
        &[
            Appearance::new(&parent)
                .extra("/Resources << /Font << /F 3 0 R >> /XObject << /Nested 15 0 R >> >>"),
            Appearance::new(fixture::TEXT_STROKE).extra("/Subtype /Form"),
        ],
        &[Annotation::new(0)],
    );
    let parent_after = format!("/Nested Do {}", fixture::TEXT_STROKE);
    let nested_after = fixture::make(
        "",
        &[
            Appearance::new(&parent_after)
                .extra("/Resources << /Font << /F 3 0 R >> /XObject << /Nested 15 0 R >> >>"),
            Appearance::new(fixture::TEXT_FILL).extra("/Subtype /Form"),
        ],
        &[Annotation::new(0)],
    );
    for input in [&page_ap, &ap_ap, &nested, &nested_after] {
        let (drawing, _) = adopted(input, 2);
        assert!(
            elements(&drawing)
                .iter()
                .all(|element| matches!(&element.shape, Shape::Text(text) if text.content == "A"))
        );
    }
    let both = fixture::make(
        "",
        &[Appearance::new(fixture::TEXT_BOTH)],
        &[Annotation::new(0)],
    );
    adopted(&both, 1);
}

#[test]
fn falls_back_as_a_group_for_unsupported_annotations_and_bad_geometry() {
    let page = "1 1 m 2 2 l S";
    let infinite = "9".repeat(400);
    for annotation in [
        Annotation::new(0).extra("/OC 17"),
        Annotation::new(0).extra("/OC 99 0 R"),
        Annotation::missing(),
        Annotation::raw("99 0 R"),
        Annotation::raw("17"),
        Annotation::new(0).without_rect(),
        Annotation::new(0).rect("[0 0 0 10]"),
        Annotation::new(0).rect("[0 0 10 10000000000000000000000000000000000000000]"),
    ] {
        fallback(
            &fixture::make(
                page,
                &[Appearance::new(LINE)],
                &[Annotation::new(0), annotation],
            ),
            2,
        );
    }
    for appearance in [
        Appearance::new(LINE).without_bbox(),
        Appearance::new(LINE).bbox("[0 0 0 10]"),
        Appearance::new(LINE).bbox("[0 0 10 10000000000000000000000000000000000000000]"),
        Appearance::new(LINE).extra("/Matrix [0 0 0 0 0 0]"),
        Appearance::new(LINE).extra(&format!("/Matrix [{infinite} 0 0 1 0 0]")),
    ] {
        fallback(
            &fixture::make(
                page,
                &[Appearance::new(LINE), appearance],
                &[Annotation::new(0), Annotation::new(1)],
            ),
            2,
        );
    }
    // Invisible は hayro の Hidden/NoView 判定に含まれない。
    adopted(
        &fixture::make(
            "",
            &[Appearance::new(LINE)],
            &[Annotation::new(0).extra("/F 1")],
        ),
        1,
    );
}

#[test]
fn skips_hidden_annotations_and_reports_lost_annotation_semantics() {
    for flag in [2, 32] {
        // 非表示の注釈に AP がなくても、見える外観の採用を妨げない。
        let input = fixture::make(
            "1 1 m 2 2 l S",
            &[Appearance::new(LINE)],
            &[
                Annotation::new(0).extra("/Contents (Review note) /T (Reviewer) /Subj (Check)"),
                Annotation::missing().extra(&format!("/F {flag}")),
            ],
        );
        let (drawing, diagnostics) = common::lift(&input.bytes);
        assert_eq!(elements(&drawing).len(), 2, "{diagnostics:?}");
        assert_eq!(count(&diagnostics, "pdf.annotation.drawn"), 1);
        assert_eq!(count(&diagnostics, "pdf.annotation.unsupported"), 1);
        let drawn = diagnostics
            .iter()
            .find(|d| d.code().name() == "pdf.annotation.drawn")
            .unwrap();
        assert_eq!(
            drawn.param("kind"),
            Some(&zumen_base::Value::Text("Stamp".to_owned()))
        );
        assert_eq!(
            drawn.places()[0].record(),
            Some(u64::from(input.annotation_objects[0]) + 1)
        );
        let unsupported = diagnostics
            .iter()
            .find(|d| d.code().name() == "pdf.annotation.unsupported")
            .unwrap();
        assert_eq!(
            unsupported.places()[0].record(),
            Some(u64::from(input.annotation_objects[1]) + 1)
        );
    }
}

#[test]
fn rejects_unbalanced_or_unreadable_appearance_contents_and_deep_forms() {
    for content in [
        "q 0 0 m 10 10 l S",
        "Q 0 0 m 10 10 l S",
        "/OC /a BDC /Im Do",
        "EMC /Im Do",
        "/Broken Do",
        "0 0 m (",
    ] {
        fallback(
            &fixture::make(
                "1 1 m 2 2 l S",
                &[Appearance::new(LINE), Appearance::new(content)],
                &[Annotation::new(0), Annotation::new(1)],
            ),
            2,
        );
    }
    adopted(&fixture::nested(50), 2);
    fallback(&fixture::nested(51), 1);
    let dangerous = Appearance::new(fixture::DANGEROUS_TEXT).extra(&format!(
        "/Resources << /Font << /Bad {} >> >>",
        fixture::dangerous_font()
    ));
    fallback(
        &fixture::make(
            "1 1 m 2 2 l S",
            &[Appearance::new(LINE), dangerous],
            &[Annotation::new(0), Annotation::new(1)],
        ),
        2,
    );
}

#[test]
fn keeps_annotation_diagnostics_when_the_page_is_refused() {
    let input = fixture::refused_page();
    fallback(&input, 2);
    let (drawing, diagnostics) = common::lift(&input.bytes);
    assert!(elements(&drawing).is_empty());
    assert_eq!(count(&diagnostics, "pdf.page.dropped"), 1);
    assert_eq!(count(&diagnostics, "pdf.annotation.drawn"), 0);
    let unsupported = diagnostics
        .iter()
        .find(|d| d.code().name() == "pdf.annotation.unsupported")
        .unwrap();
    assert_eq!(
        unsupported
            .places()
            .iter()
            .map(|place| place.record())
            .collect::<Vec<_>>(),
        [Some(6), Some(7)]
    );
}

#[test]
fn preserves_existing_page_styles_hashes_and_ids_across_multiple_pages() {
    let input = fixture::multiple_pages();
    let (drawing, diagnostics) = common::lift(&input.bytes);
    let (baseline, _) = common::lift(&input.without_annotations);
    assert_eq!(
        count(&diagnostics, "pdf.annotation.unsupported"),
        0,
        "{diagnostics:?}"
    );
    assert_eq!(baseline.sheets.len(), 2);
    for (index, old_sheet) in baseline.sheets.iter().enumerate() {
        let old_elements = &old_sheet.elements;
        assert_eq!(
            &drawing.sheets[index].elements[..old_elements.len()],
            old_elements
        );
        assert_eq!(
            &ids(&drawing, index)[..old_elements.len()],
            ids(&baseline, index)
        );
        for (retained, old) in drawing.sheets[index].elements.iter().zip(old_elements) {
            assert_eq!(Hash::element(retained), Hash::element(old));
        }
    }
    assert_eq!(
        drawing.sheets[0].elements.len(),
        baseline.sheets[0].elements.len() + 2
    );
    assert_eq!(drawing.sheets[1], baseline.sheets[1]);
    assert_eq!(baseline.line_types.len(), 2);
    assert_eq!(
        &drawing.line_types[..baseline.line_types.len()],
        baseline.line_types
    );
    assert_eq!(drawing.line_types.len(), 3);
    assert_eq!(
        baseline
            .text_styles
            .iter()
            .map(|style| style.name.as_str())
            .collect::<Vec<_>>(),
        ["PageOne", "PageTwo"]
    );
    assert_eq!(
        &drawing.text_styles[..baseline.text_styles.len()],
        baseline.text_styles
    );
    assert_eq!(
        drawing
            .text_styles
            .iter()
            .map(|style| style.name.as_str())
            .collect::<Vec<_>>(),
        ["PageOne", "PageTwo", "Appearance"]
    );
}

#[test]
fn keeps_page_once_diagnostics_and_does_not_publish_rejected_appearance_warnings() {
    let text = "BT /Missing 4 Tf 2 3 Td (A) Tj ET";
    let input = fixture::make(
        text,
        &[Appearance::new(text)],
        &[Annotation::new(0), Annotation::new(0)],
    );
    for places in [0, 1, 16] {
        let file = testkit::read::<Pdf>(&input.bytes).0.unwrap();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut diagnostics = Diagnostics::with_max_places(places);
        let drawing = file
            .lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel))
            .unwrap();
        assert_eq!(elements(&drawing).len(), 3);
        let diagnostics = diagnostics.into_vec();
        assert_eq!(
            count(&diagnostics, "pdf.font.missing"),
            1,
            "{diagnostics:?}"
        );
        assert_eq!(count(&diagnostics, "pdf.annotation.unsupported"), 0);
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.places().len() <= places)
        );
    }
    let rejected = fixture::make(
        "1 1 m 2 2 l S",
        &[Appearance::new(text), Appearance::new("Q")],
        &[Annotation::new(0), Annotation::new(1)],
    );
    fallback(&rejected, 2);
    let ordered = fixture::make(
        fixture::TEXT_FILL,
        &[Appearance::new("/Absent Do")],
        &[Annotation::new(0)],
    );
    let (_, diagnostics) = adopted(&ordered, 1);
    let codes = common::codes(&diagnostics);
    let page = codes
        .iter()
        .position(|(code, _)| *code == "pdf.font-name.dropped")
        .unwrap();
    let appearance = codes
        .iter()
        .position(|(code, _)| *code == "pdf.xobject.missing")
        .unwrap();
    assert!(page < appearance, "{diagnostics:?}");
}

#[test]
fn stops_at_exact_budget_boundaries_and_retries_without_partial_results() {
    for input in [
        fixture::multiple_pages(),
        fixture::make(
            "/Im Do",
            &[
                Appearance::new("/Fm Do /Rebound Do"),
                Appearance::new(fixture::TEXT_BOTH),
            ],
            &[Annotation::new(0), Annotation::new(0), Annotation::new(1)],
        ),
    ] {
        let file = testkit::read::<Pdf>(&input.bytes).0.unwrap();
        let cancel = CancelToken::new();
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let expected = file
            .lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel))
            .unwrap();
        let expected_diagnostics = diagnostics.into_vec();
        for kind in [Kind::Work, Kind::DecodedBytes] {
            let used = budget.used(kind);
            for limit in [used - 1, used] {
                let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
                let mut diagnostics = Diagnostics::new();
                let result = file.lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
                if limit == used {
                    assert_eq!(result.unwrap(), expected);
                    assert_eq!(diagnostics.into_vec(), expected_diagnostics);
                } else {
                    assert!(
                        matches!(result, Err(Error::Stopped(_))),
                        "{kind:?}: {result:?}"
                    );
                }
            }
        }
        cancel.cancel();
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        assert!(matches!(
            file.lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel)),
            Err(Error::Stopped(_))
        ));
        assert_eq!(common::lift(&input.bytes).0, expected);
    }
}

fn measured(
    input: &fixture::Fixture,
    decoded_limit: Option<u64>,
) -> (Result<Drawing2D, Error>, Budget, Vec<Diagnostic>) {
    let file = testkit::read::<Pdf>(&input.bytes).0.unwrap();
    let limits = decoded_limit.map_or_else(Limits::default, |limit| {
        Limits::default().with(Kind::DecodedBytes, Limit::Finite(limit))
    });
    let mut budget = Budget::new(limits);
    let cancel = CancelToken::new();
    let mut diagnostics = Diagnostics::new();
    let result = file.lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
    (result, budget, diagnostics.into_vec())
}

/// scalar の長さで後続レコードのバイト位置は動く。形・ID・レコードの出所・診断の値を比べる。
fn same_values(left: &(Drawing2D, Vec<Diagnostic>), right: &(Drawing2D, Vec<Diagnostic>)) {
    assert_eq!(left.0.sheets.len(), right.0.sheets.len());
    for (index, (left_sheet, right_sheet)) in left.0.sheets.iter().zip(&right.0.sheets).enumerate()
    {
        assert_eq!(left_sheet.elements.len(), right_sheet.elements.len());
        assert_eq!(ids(&left.0, index), ids(&right.0, index));
        for (left, right) in left_sheet.elements.iter().zip(&right_sheet.elements) {
            assert_eq!(left.shape, right.shape);
            assert_eq!(left.style, right.style);
            assert_eq!(left.layer, right.layer);
            assert_eq!(left.source.record(), right.source.record());
        }
    }
    assert_eq!(left.0.line_types, right.0.line_types);
    assert_eq!(left.0.text_styles, right.0.text_styles);
    assert_eq!(left.1.len(), right.1.len());
    for (left, right) in left.1.iter().zip(&right.1) {
        assert_eq!(left.code(), right.code());
        assert_eq!(left.count(), right.count());
        assert_eq!(left.params(), right.params());
        assert_eq!(
            left.places()
                .iter()
                .map(|place| place.record())
                .collect::<Vec<_>>(),
            right
                .places()
                .iter()
                .map(|place| place.record())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn reserves_used_resource_scalars_and_repeated_references_before_decoding_them() {
    use fixture::ScalarCase::{DashReferences, TransferDirect, TransferReference};

    const LARGE: usize = 32 * 1024;
    for case in [TransferDirect, TransferReference, DashReferences(8)] {
        let small = fixture::scalar(case, 16);
        let large = fixture::scalar(case, LARGE);
        let small_result = measured(&small, None);
        let (drawing, diagnostics) = adopted(&large, 2);
        let large_result = measured(&large, None);
        assert_eq!(large_result.0.as_ref().unwrap(), &drawing);
        same_values(
            &(small_result.0.unwrap(), small_result.2),
            &(drawing.clone(), diagnostics.clone()),
        );
        let expected_transfer = u64::from(matches!(case, TransferDirect | TransferReference));
        assert_eq!(
            count(&diagnostics, "pdf.transfer.dropped"),
            expected_transfer
        );
        assert_eq!(
            diagnostics.len(),
            1 + usize::try_from(expected_transfer).unwrap()
        );

        // 通常値を扱える予算を与えても、32 KiB の scalar の予約そのもので止まる。
        let low = small_result.1.used(Kind::DecodedBytes) + 64 * 1024;
        let stopped = measured(&large, Some(low));
        let Err(Error::Stopped(Stop::Exceeded(exceeded))) = &stopped.0 else {
            panic!("{case:?}: {stopped:?}")
        };
        assert_eq!(exceeded.kind, Kind::DecodedBytes);
        assert!(
            exceeded.requested - stopped.1.used(Kind::DecodedBytes) >= LARGE as u64,
            "{case:?}: {stopped:?}"
        );
        assert_eq!(
            format!("{stopped:?}"),
            format!("{:?}", measured(&large, Some(low)))
        );

        let used = large_result.1.used(Kind::DecodedBytes);
        assert_eq!(measured(&large, Some(used)).0.unwrap(), drawing);
        assert!(matches!(
            measured(&large, Some(used - 1)).0,
            Err(Error::Stopped(_))
        ));
        assert_eq!(common::lift(&large.bytes).0, drawing);
    }
    let once = measured(&fixture::scalar(DashReferences(1), LARGE), None);
    let repeated = measured(&fixture::scalar(DashReferences(8), LARGE), None);
    assert!(once.0.is_ok() && repeated.0.is_ok());
    assert!(
        repeated.1.used(Kind::DecodedBytes) >= once.1.used(Kind::DecodedBytes) + 7 * LARGE as u64,
        "同じ参照先を反復回数分数える"
    );
}

#[test]
fn leaves_unrelated_large_dictionary_scalars_opaque() {
    use fixture::ScalarCase::{UnknownHex, UnknownLiteral};

    for case in [UnknownLiteral, UnknownHex] {
        let small = fixture::scalar(case, 16);
        let large = fixture::scalar(case, 32 * 1024);
        let expected = common::lift(&small.bytes);
        let (drawing, diagnostics) = adopted(&large, 2);
        same_values(&(drawing.clone(), diagnostics.clone()), &expected);
        assert_eq!(diagnostics.len(), 1);
        // 未知の値は描画に使わない。偽の参照文字列を含む literal も、通常の小予算で読める。
        let limited = measured(&large, Some(2 * 1024 * 1024));
        assert_eq!(limited.0.unwrap(), drawing);
        assert_eq!(limited.2, diagnostics);
    }
}

#[test]
fn stops_annotation_iteration_at_scalar_and_keeps_escaped_or_referenced_subtype() {
    for referenced in [false, true] {
        for tail in ["17", "<12ab>"] {
            let input = fixture::annotation_scalars(referenced, tail);
            let (drawing, diagnostics) = common::lift(&input.bytes);
            assert_eq!(elements(&drawing).len(), 2, "{diagnostics:?}");
            line(&elements(&drawing)[1], [0.0, 0.0], [10.0, 10.0]);
            assert_eq!(common::codes(&diagnostics), [("pdf.annotation.drawn", 1)]);
            assert_eq!(
                diagnostics[0].param("kind"),
                Some(&zumen_base::Value::Text("Stamp".to_owned()))
            );
            assert_eq!(
                diagnostics[0]
                    .places()
                    .iter()
                    .map(|place| place.record())
                    .collect::<Vec<_>>(),
                [Some(6)]
            );
        }
    }
}

#[test]
fn reserves_deep_balanced_graphics_stacks_and_can_retry_after_stopping() {
    let input = fixture::stacked(4096);
    let (expected, expected_diagnostics) = adopted(&input, 1);
    line(&elements(&expected)[0], [0.0, 0.0], [10.0, 10.0]);
    let file = testkit::read::<Pdf>(&input.bytes).0.unwrap();
    let cancel = CancelToken::new();
    // 内容は約 16 KiB。内容長に比例する予約だけなら通る予算でも、積む状態の分で止める。
    let mut budget =
        Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(1024 * 1024)));
    let mut diagnostics = Diagnostics::new();
    let stopped = file.lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
    assert!(
        matches!(stopped, Err(Error::Stopped(Stop::Exceeded(exceeded))) if exceeded.kind == Kind::DecodedBytes),
        "{stopped:?}"
    );
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let retried = file
        .lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel))
        .unwrap();
    assert_eq!(retried, expected);
    assert_eq!(diagnostics.into_vec(), expected_diagnostics);
}

#[test]
fn evaluates_annotation_optional_content_and_records_per_annotation_provenance() {
    // 2 つの注釈が同じ AP ストリームを共有し、DeviceCMYK を描く。
    // 要素と装置診断の出所はそれぞれの注釈レコードになり、ページ本体の出所と ID は変わらない。
    let shared = fixture::make(
        "1 1 m 2 2 l S",
        &[Appearance::new("1 0 0 0 K 0 0 m 10 10 l S")],
        &[
            Annotation::new(0).rect("[0 0 10 10]"),
            Annotation::new(0).rect("[20 20 30 30]").extra("/OC 5 0 R"),
        ],
    );
    let (drawing, diagnostics) = adopted(&shared, 3);
    let (baseline, _) = common::lift(&shared.without_annotations);
    assert_eq!(ids(&drawing, 0)[0], ids(&baseline, 0)[0]);
    assert_eq!(
        elements(&drawing)
            .iter()
            .map(|element| element.source.record())
            .collect::<Vec<_>>(),
        [
            Some(u64::from(shared.content_object) + 1),
            Some(u64::from(shared.annotation_objects[0]) + 1),
            Some(u64::from(shared.annotation_objects[1]) + 1),
        ]
    );
    assert_eq!(
        ids(&drawing, 0),
        vec![
            format!("sheet/0/r{}", shared.content_object + 1),
            format!("sheet/0/r{}", shared.annotation_objects[0] + 1),
            format!("sheet/0/r{}", shared.annotation_objects[1] + 1),
        ]
    );
    let color_diag = diagnostics
        .iter()
        .find(|d| d.code().name() == "pdf.color.approximated")
        .unwrap();
    assert_eq!(color_diag.count(), 2);
    assert_eq!(
        color_diag
            .places()
            .iter()
            .map(|place| place.record())
            .collect::<Vec<_>>(),
        [
            Some(u64::from(shared.annotation_objects[0]) + 1),
            Some(u64::from(shared.annotation_objects[1]) + 1),
        ]
    );
    let oc_approx = diagnostics
        .iter()
        .find(|d| d.code().name() == "pdf.optional-content.approximated")
        .unwrap();
    assert_eq!(
        oc_approx
            .places()
            .iter()
            .map(|place| place.record())
            .collect::<Vec<_>>(),
        [Some(u64::from(shared.annotation_objects[1]) + 1)]
    );

    // 注釈の単一 OCG の下で描く画像は、その OCG のレイヤを引き継ぎ、optional-content.approximated は注釈自身の線等の近似として残す。
    let image_oc = fixture::make(
        "/Plain Do",
        &[Appearance::new("/Plain Do")],
        &[Annotation::new(0).extra("/OC 6 0 R")],
    );
    let (img_drawing, img_diag) = adopted(&image_oc, 2);
    assert_eq!(
        elements(&img_drawing)
            .iter()
            .map(|e| e.layer.0)
            .collect::<Vec<_>>(),
        [0, 2]
    );
    assert_eq!(count(&img_diag, "pdf.optional-content.approximated"), 1);

    // 注釈自身の OC が既定で OFF の OCG なら、その注釈の外観だけを抑止し、同じページの別の可視注釈はそのまま描く。
    let mixed_hidden = fixture::with_off(
        "1 1 m 2 2 l S",
        &[Appearance::new("1 0 0 0 K 0 0 m 10 10 l S")],
        &[
            Annotation::new(0).rect("[0 0 10 10]").extra("/OC 6 0 R"),
            Annotation::new(0).rect("[20 20 30 30]"),
        ],
    );
    let (mixed_drawing, mixed_diag) = adopted(&mixed_hidden, 2);
    line(&elements(&mixed_drawing)[1], [20.0, 20.0], [30.0, 30.0]);
    assert_eq!(
        elements(&mixed_drawing)[1].source.record(),
        Some(u64::from(mixed_hidden.annotation_objects[1]) + 1)
    );
    assert_eq!(count(&mixed_diag, "pdf.optional-content.dropped"), 1);
    assert_eq!(count(&mixed_diag, "pdf.color.approximated"), 1);

    // OCMD と VE: 隠れる OCMD は pdf.membership.dropped、VE を持つ OCMD は pdf.visibility-expression.approximated を返す。
    let ocmd_annot = fixture::with_off(
        "1 1 m 2 2 l S",
        &[Appearance::new(LINE)],
        &[
            Annotation::new(0)
                .rect("[0 0 10 10]")
                .extra("/OC << /Type /OCMD /OCGs [6 0 R] /P /AllOn /VE [/Not 6 0 R] >>"),
            Annotation::new(0)
                .rect("[20 20 30 30]")
                .extra("/OC << /Type /OCMD /OCGs [5 0 R] /P /AnyOn /VE [/And 5 0 R 6 0 R] >>"),
        ],
    );
    let (ocmd_drawing, ocmd_diag) = adopted(&ocmd_annot, 2);
    line(&elements(&ocmd_drawing)[1], [20.0, 20.0], [30.0, 30.0]);
    assert_eq!(count(&ocmd_diag, "pdf.membership.dropped"), 1);
    assert_eq!(count(&ocmd_diag, "pdf.optional-content.approximated"), 1);
    assert_eq!(
        count(&ocmd_diag, "pdf.visibility-expression.approximated"),
        2
    );
}
