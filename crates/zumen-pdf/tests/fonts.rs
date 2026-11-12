//! フォントと ICC の単独 Flate を全量の確保前に止める（#610）。

#![cfg(feature = "drawing")]

use std::io::Write;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Stop};
use zumen_drawing::Drawing2D;
use zumen_pdf::Pdf;
use zumen_sdk::{Error, Lift, testkit};

mod common;
#[path = "common/fonts.rs"]
mod fonts;

#[cfg(test)]
fn compressed(data: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

#[cfg(test)]
fn run(
    source: &[u8],
    limits: Limits,
    cancelled: bool,
) -> (
    Result<Drawing2D, Error>,
    Budget,
    Vec<zumen_base::Diagnostic>,
) {
    let file = testkit::read::<Pdf>(source).0.unwrap();
    let mut budget = Budget::new(limits);
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    if cancelled {
        cancel.cancel();
    }
    let result = file.lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
    (result, budget, diagnostics.into_vec())
}

#[test]
fn bounds_font_and_icc_flate_before_allocating_the_whole_output() {
    let data = compressed(&vec![0; 64 * 1024]);
    assert!(data.len() < 256);
    for filter in [
        "/Filter /FlateDecode",
        "/F /Fl",
        "/Filter [/FlateDecode]",
        "/Filter [/Crypt /FlateDecode]",
        "/F [/Crypt /Fl] /DP [<< /Name /Identity >> null]",
        "/Filter /FlateDecode /DecodeParms << /Predictor 12 /Columns 1 >>",
    ] {
        for kind in fonts::KINDS {
            for direct in [false, true] {
                let source = fonts::document(
                    kind,
                    filter,
                    &data,
                    fonts::Uses {
                        direct,
                        ..Default::default()
                    },
                );
                let result = run(
                    &source,
                    Limits::default().with(Kind::DecodedBytes, Limit::Finite(32 * 1024)),
                    false,
                );
                assert!(
                    matches!(result.0, Err(Error::Stopped(Stop::Exceeded(e)))
                    if e.kind == Kind::DecodedBytes && e.requested < 48 * 1024),
                    "{kind:?}, {direct}, {filter}: {result:?}"
                );
            }
        }
    }
}

#[test]
fn preserves_font_and_icc_results_with_damage_and_predictors() {
    // 寛容な Flate の表を含む保守的な上限は既定予算を超えうる。この検査では内容を比べる。
    let lift = |source: &[u8]| {
        let (file, mut diagnostics) = testkit::read::<Pdf>(source);
        let limits = Limits::default()
            .with(Kind::Work, Limit::Unlimited)
            .with(Kind::DecodedBytes, Limit::Unlimited);
        let mut budget = Budget::new(limits);
        let mut lifted = Diagnostics::new();
        let cancel = CancelToken::new();
        let drawing = file
            .unwrap()
            .lift(&mut Cx::new(&mut budget, &mut lifted, &cancel))
            .unwrap();
        diagnostics.extend(lifted.into_vec());
        (drawing, diagnostics)
    };
    for kind in fonts::KINDS {
        let plain = fonts::plain(kind);
        let good = compressed(&plain);
        let mut wrong = good.clone();
        *wrong.last_mut().unwrap() ^= 0xff;
        let mut predicted = vec![0];
        predicted.extend(&plain);
        let predictor = format!(
            "/Filter /FlateDecode /DecodeParms << /Predictor 12 /Columns {} >>",
            plain.len()
        );
        for direct in [false, true] {
            let uses = fonts::Uses {
                direct,
                calls: 2,
                ..Default::default()
            };
            let expected = common::lift(&fonts::document(kind, "", &plain, uses));
            for (filter, data) in [
                ("/Filter /FlateDecode", good.clone()),
                ("/F /Fl", good[2..good.len() - 4].to_vec()),
                ("/Filter [/Crypt /FlateDecode]", good.clone()),
                (
                    "/F [/Crypt /Fl] /DP [<< /Name /Identity >> null]",
                    good.clone(),
                ),
                ("/Filter /FlateDecode", wrong.clone()),
                ("/Filter /FlateDecode", good[..good.len() - 4].to_vec()),
                (predictor.as_str(), compressed(&predicted)),
            ] {
                let source = fonts::document(kind, filter, &data, uses);
                let actual = lift(&source);
                let shapes = |drawing: &Drawing2D| {
                    drawing
                        .sheets
                        .iter()
                        .flat_map(|sheet| &sheet.elements)
                        .map(|element| format!("{:?} {:?}", element.shape, element.style))
                        .collect::<Vec<_>>()
                };
                if !direct {
                    assert!(!shapes(&expected.0).is_empty(), "{kind:?}");
                    assert_eq!(shapes(&actual.0), shapes(&expected.0), "{kind:?}, {filter}");
                    assert_eq!(common::codes(&actual.1), common::codes(&expected.1));
                }
                // 直接のストリームは仕様外なので、同じバイト列の出所と ID も含めた再現性を比べる。
                assert_eq!(actual, lift(&source));
            }
        }
    }
}

#[test]
fn font_and_icc_budgets_and_cancellation_are_repeatable() {
    for kind in fonts::KINDS {
        for direct in [false, true] {
            for clip in [false, true] {
                let data = compressed(&fonts::plain(kind));
                let source = fonts::document(
                    kind,
                    "/Filter /FlateDecode",
                    &data,
                    fonts::Uses {
                        direct,
                        calls: 2,
                        pages: 2,
                        clip,
                        ..Default::default()
                    },
                );
                let (expected, total, _) = run(&source, Limits::default(), false);
                assert!(expected.is_ok(), "{kind:?}, {direct}, {clip}");
                for budget_kind in [Kind::DecodedBytes, Kind::Work] {
                    let used = total.used(budget_kind);
                    let exact = Limits::default().with(budget_kind, Limit::Finite(used));
                    assert_eq!(
                        run(&source, exact, false).0.unwrap(),
                        expected.clone().unwrap()
                    );
                    let short = Limits::default().with(budget_kind, Limit::Finite(used - 1));
                    let stopped = run(&source, short, false);
                    assert!(
                        matches!(stopped.0, Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == budget_kind)
                    );
                    assert_eq!(
                        format!("{stopped:?}"),
                        format!("{:?}", run(&source, short, false))
                    );
                }
                let cancelled = run(&source, Limits::default(), true);
                assert!(matches!(cancelled.0, Err(Error::Stopped(Stop::Cancelled))));
                assert_eq!(cancelled.1.used(Kind::DecodedBytes), 0);
                assert_eq!(
                    cancelled.2[0].code(),
                    zumen_base::codes::CALL_REQUEST_CANCELLED
                );
            }
        }
    }
}

#[test]
fn counts_font_and_icc_caches_before_predictors_even_on_failure() {
    for kind in fonts::KINDS {
        for predictor in [1, 12, 5] {
            let filter = format!(
                "/Filter /FlateDecode /DecodeParms << /Predictor {predictor} /Columns 1 >>"
            );
            let data = compressed(&vec![0; 8192]);
            let used = |uses| {
                let source = fonts::document(kind, &filter, &data, uses);
                let (result, budget, _) = run(&source, Limits::default(), false);
                assert!(result.is_ok());
                budget.used(Kind::DecodedBytes)
            };
            let one = used(fonts::Uses::default());
            let calls = used(fonts::Uses {
                calls: 2,
                ..Default::default()
            });
            let pages = used(fonts::Uses {
                pages: 2,
                ..Default::default()
            });
            assert!(
                calls - one < 8192,
                "same font {kind:?}, {predictor}: {one}, {calls}"
            );
            assert!(
                pages - one < 8192,
                "same document {kind:?}, {predictor}: {one}, {pages}"
            );
            let fonts = used(fonts::Uses {
                fonts: 2,
                ..Default::default()
            });
            if matches!(
                kind,
                fonts::Kind::Encoding
                    | fonts::Kind::CidToGid
                    | fonts::Kind::Type1Unicode
                    | fonts::Kind::TrueTypeUnicode
                    | fonts::Kind::CidUnicode
                    | fonts::Kind::Type3Unicode
            ) {
                // 対応表は各フォントで読み直す。事前確認・走査の FontCache・解釈の FontCache の 3 回分。
                assert!(
                    fonts - one >= 3 * 8192,
                    "distinct fonts {kind:?}, {predictor}: {one}, {fonts}"
                );
            } else {
                // 埋め込み program と ICC は、別の辞書から同じ間接ストリームを使っても読み直さない。
                assert!(
                    fonts - one < 8192,
                    "shared program/profile {kind:?}, {predictor}: {one}, {fonts}"
                );
            }
            let direct_one = used(fonts::Uses {
                direct: true,
                ..Default::default()
            });
            let direct_two = used(fonts::Uses {
                direct: true,
                fonts: 2,
                ..Default::default()
            });
            assert!(
                direct_two - direct_one >= 2 * 8192,
                "direct streams {kind:?}, {predictor}: {direct_one}, {direct_two}"
            );
        }
    }
}

#[test]
fn stops_font_and_icc_flate_for_work_before_the_whole_output() {
    let data = compressed(&vec![0; 64 * 1024]);
    for kind in fonts::KINDS {
        let source = fonts::document(kind, "/Filter /FlateDecode", &data, fonts::Uses::default());
        let (result, budget, _) = run(
            &source,
            Limits::default().with(Kind::Work, Limit::Finite(2000)),
            false,
        );
        assert!(
            matches!(result, Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == Kind::Work),
            "{kind:?}"
        );
        assert!(budget.used(Kind::DecodedBytes) < 48 * 1024, "{kind:?}");
    }
}

#[test]
fn does_not_merge_a_direct_icc_profile_with_its_container() {
    let data = compressed(&vec![0; 64 * 1024]);
    let inner = common::stream("/N 3 /Filter /FlateDecode", &data);
    let mut outer = b"<< /N 3 /Alternate [/ICCBased ".to_vec();
    outer.extend(inner);
    outer.extend(b"] /Length 1 >>\nstream\nx\nendstream");
    let mut document = common::Document::new(vec![common::Page::new("/Sh sh").resources(
        "/Shading << /Sh << /ShadingType 2 /ColorSpace [/ICCBased 3 0 R] \
            /Coords [0 0 100 0] /Function << /FunctionType 2 /Domain [0 1] \
            /C0 [0 0 0] /C1 [1 1 1] /N 1 >> >> >>",
    )]);
    document.objects.push(outer);
    // 内側の直接のストリームにも obj_id=3 が付く。外側を数えたことだけで内側を飛ばしてはいけない。
    let result = run(
        &document.bytes(),
        Limits::default().with(Kind::DecodedBytes, Limit::Finite(32 * 1024)),
        false,
    );
    assert!(matches!(result.0, Err(Error::Stopped(Stop::Exceeded(e)))
        if e.kind == Kind::DecodedBytes && e.requested < 48 * 1024));
}
