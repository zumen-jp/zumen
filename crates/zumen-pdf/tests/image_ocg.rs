//! #520: 画像のレイヤと、同じストリームを呼ぶ文脈の合成。
#![cfg(test)]
#![cfg(feature = "drawing")]

use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, codes};
use zumen_drawing::{Drawing2D, Element, Hash, LayerId, PaperPoint, Shape, id};
use zumen_pdf::Pdf;
use zumen_sdk::{Error, Lift, testkit};

mod common;
#[path = "common/image_ocg.rs"]
mod fixture;

fn elements(drawing: &Drawing2D) -> &[Element<PaperPoint>] {
    &drawing.sheets[0].elements
}

fn ids(drawing: &Drawing2D) -> Vec<String> {
    let mut namer = id::Namer::new("sheet/0");
    elements(drawing)
        .iter()
        .enumerate()
        .map(|(index, element)| namer.next(index, element))
        .collect()
}

fn count(diag: &[Diagnostic], code: &str) -> u64 {
    diag.iter()
        .filter(|d| d.code().name() == code)
        .map(Diagnostic::count)
        .sum()
}

fn check(
    fixture: &fixture::Fixture,
    layers: &[u32],
    approximated: u64,
) -> (Drawing2D, Vec<Diagnostic>) {
    let (drawing, diag) = common::lift(&fixture.bytes);
    assert_eq!(
        elements(&drawing)
            .iter()
            .map(|e| e.layer.0)
            .collect::<Vec<_>>(),
        layers
    );
    assert!(
        elements(&drawing)
            .iter()
            .all(|e| matches!(e.shape, Shape::Underlay(_)))
    );
    assert_eq!(
        count(&diag, "pdf.optional-content.approximated"),
        approximated,
        "{diag:?}"
    );
    let base = format!("sheet/0/r{}", fixture.content_object + 1);
    let expected: Vec<_> = (0..layers.len())
        .map(|index| {
            if index == 0 {
                base.clone()
            } else {
                format!("{base}/{index}")
            }
        })
        .collect();
    assert_eq!(ids(&drawing), expected);
    (drawing, diag)
}

#[test]
fn keeps_direct_repeated_images_and_their_original_ids_and_geometry() {
    let content = "q 10 0 0 10 10 20 cm /Im Do Q q 20 0 0 30 40 50 cm /Same Do Q";
    let input = fixture::make(content, "/OC 3 0 R");
    let (drawing, _) = check(&input, &[1, 1], 0);
    // 未知の同じ長さのキーに置き換えて、レコードの位置と元の形を変えず比較する。
    let plain = fixture::make(content, "/ZZ 3 0 R");
    let (baseline, _) = check(&plain, &[0, 0], 0);
    assert_eq!(ids(&drawing), ids(&baseline));
    for (retained, old) in elements(&drawing).iter().zip(elements(&baseline)) {
        assert_eq!(retained.source, old.source);
        assert_eq!(retained.shape, old.shape);
        assert_eq!(retained.style, old.style);
        assert_ne!(Hash::element(retained), Hash::element(old));
        let Shape::Underlay(image) = &retained.shape else {
            panic!("画像の下絵でない")
        };
        assert_eq!(image.resource.path, "8 0 R");
    }
}

#[test]
fn keeps_same_group_through_marked_content_and_forms() {
    for content in [
        "/OC /a BDC /Im Do EMC",
        "/Fm Do /Fm Do",
        "/OC /a BDC /Fm Do /Fm Do EMC",
        "/Im Do /Fm Do",
    ] {
        let input = fixture::make(content, "/OC 3 0 R");
        let count = if content == "/OC /a BDC /Im Do EMC" {
            1
        } else {
            2
        };
        check(&input, &[1, 1][..count], 0);
    }
    // フォーム自身の OC の診断は残り、画像を保持できたことだけで減らさない。
    check(&fixture::make("/Grouped Do", "/OC 3 0 R"), &[1], 1);
    check(&fixture::make("/PlainA Do", "/OC 3 0 R"), &[1], 1);
}

#[test]
fn does_not_mix_equal_form_contents_with_different_resource_bindings() {
    check(
        &fixture::make("/Fm Do /Rebound Do /Fm Do /Rebound Do", "/OC 3 0 R"),
        &[1, 2, 1, 2],
        0,
    );
}

#[test]
fn preserves_fallback_for_compound_and_mixed_uses_including_cache_hits() {
    check(
        &fixture::make("/OC /b BDC /Im Do EMC", "/OC 3 0 R"),
        &[2],
        1,
    );
    check(
        &fixture::make("/Im Do /OC /b BDC /Im Do EMC", "/OC 3 0 R"),
        &[0, 2],
        2,
    );
    check(
        &fixture::make("/Fm Do /OC /b BDC /Fm Do /Fm Do EMC", "/OC 3 0 R"),
        &[0, 2, 2],
        3,
    );
    check(
        &fixture::make("/PlainA Do /PlainB Do /PlainA Do", "/OC 3 0 R"),
        &[0, 0, 0],
        3,
    );
    check(&fixture::make("/Im Do", "/OC 6 0 R"), &[0], 1);
    check(
        &fixture::make("/OC /md BDC /Im Do EMC", "/OC 3 0 R"),
        &[1],
        2,
    );
}

#[test]
fn keeps_hidden_content_hidden_and_does_not_assume_unregistered_groups() {
    let (_, diag) = check(&fixture::with_off("/Im Do", "/OC 5 0 R"), &[], 0);
    assert_eq!(count(&diag, "pdf.optional-content.dropped"), 1);
    check(
        &fixture::with_off("/OC /off BDC /Im Do EMC", "/OC 3 0 R"),
        &[],
        0,
    );
    check(&fixture::make("/Im Do", "/OC 7 0 R"), &[0], 1);
    check(&fixture::make("/Im Do", "/OC 99 0 R"), &[0], 0);
    check(
        &fixture::make("/Im Do", "/OC << /Type /OCMD /OCGs [3 0 R] >>"),
        &[0],
        1,
    );
    check(&fixture::anonymous("/Im Do"), &[0], 1);
}

#[test]
fn handles_stencils_and_unbalanced_marked_content_without_claiming_a_single_layer() {
    let (_, diag) = check(&fixture::make("/Mask Do", ""), &[1], 0);
    assert_eq!(count(&diag, "pdf.image-mask.approximated"), 1);
    check(
        &fixture::make("/Broken Do /Im Do EMC", "/OC 3 0 R"),
        &[2, 2],
        2,
    );
}

#[test]
fn keeps_fallback_warning_counts_order_and_places_with_place_limits() {
    let input = fixture::make(
        "/Fm Do /OC /b BDC /Fm Do /Fm Do EMC /OC /md BDC /Plain Do EMC /Mask Do",
        "/OC 3 0 R",
    );
    let file = testkit::read::<Pdf>(&input.bytes).0.unwrap();
    let mut expected = None;
    for places in [0, 1, 16] {
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut diagnostics = Diagnostics::with_max_places(places);
        let drawing = file
            .lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel))
            .unwrap();
        let diagnostics = diagnostics.into_vec();
        assert_eq!(
            common::codes(&diagnostics),
            [
                ("pdf.optional-content.approximated", 4),
                ("pdf.image-mask.approximated", 1)
            ]
        );
        assert_eq!(count(&diagnostics, "pdf.optional-content.approximated"), 4);
        let warning = diagnostics
            .iter()
            .find(|d| d.code() == codes::PDF_OPTIONAL_CONTENT_APPROXIMATED)
            .unwrap();
        assert_eq!(warning.places().len(), places.min(4));
        if places > 0 {
            assert_eq!(warning.places()[0].record(), Some(9));
        }
        if places == 16 {
            assert_eq!(
                warning
                    .places()
                    .iter()
                    .map(|place| place.record())
                    .collect::<Vec<_>>(),
                [
                    Some(9),
                    Some(9),
                    Some(9),
                    Some(u64::from(input.content_object) + 1)
                ]
            );
        }
        if let Some(expected) = &expected {
            assert_eq!(&drawing, expected);
        } else {
            expected = Some(drawing);
        }
    }
}

#[test]
fn stops_at_work_and_memory_boundaries_and_allows_retry() {
    let input = fixture::make("/Im Do /Fm Do /Fm Do", "/OC 3 0 R");
    let file = testkit::read::<Pdf>(&input.bytes).0.unwrap();
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let expected = file
        .lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel))
        .unwrap();
    assert_eq!(
        elements(&expected)
            .iter()
            .map(|e| e.layer)
            .collect::<Vec<_>>(),
        [LayerId(1); 3]
    );
    for kind in [Kind::Work, Kind::DecodedBytes] {
        let used = budget.used(kind);
        for limit in [used - 1, used] {
            let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
            let mut diagnostics = Diagnostics::new();
            let result = file.lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
            if limit == used {
                assert_eq!(result.unwrap(), expected);
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
    let (again, _) = common::lift(&input.bytes);
    assert_eq!(again, expected);
}
