//! 単独 LZW の全量の確保前の予算と再利用を検査する（#616）。

#![cfg(feature = "drawing")]

use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, Stop};
use zumen_drawing::Drawing2D;
use zumen_pdf::Pdf;
use zumen_sdk::{Error, Lift, testkit};

#[path = "common/lzw_cases.rs"]
mod cases;
mod common;
#[path = "common/lzw.rs"]
mod encoding;
#[path = "common/fonts.rs"]
mod fonts;
#[path = "common/resources.rs"]
mod resources;

#[cfg(test)]
fn run(
    source: &[u8],
    limits: Limits,
    cancelled: bool,
) -> (Result<Drawing2D, Error>, Budget, Vec<Diagnostic>) {
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
fn bounds_lzw_at_every_preflight_entry_before_whole_output_allocation() {
    let plain = vec![b' '; 64 * 1024];
    for early in [0, 1] {
        let data = encoding::encode(&plain, early != 0);
        assert!(data.len() < 512);
        for filter in [
            format!("/Filter /LZWDecode /DecodeParms << /EarlyChange {early} >>"),
            format!("/F /LZW /DP << /EarlyChange {early} >>"),
            format!("/Filter [/LZWDecode] /DP [<< /EarlyChange {early} >>]"),
            format!("/Filter [/Unknown /LZWDecode] /DP [null << /EarlyChange {early} >>]"),
            format!("/F [/Crypt /LZW] /DP [null << /EarlyChange {early} >>]"),
            format!("/Filter /LZWDecode /DP << /EarlyChange {early} /Predictor 12 /Columns 1 >>"),
        ] {
            for case in 0..cases::COUNT {
                // 全量の確保は仕様どおりの間接ストリームで検査する。直接の値は符号のバイト列に
                // よってページの辞書ごと読めなくなるため、下の検査で結果と停止の再現性を比べる
                let input = cases::document(case, &filter, &data, false, 2);
                let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(32 * 1024));
                let result = run(&input, limits, false);
                assert!(
                    matches!(result.0, Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == Kind::DecodedBytes && e.requested < 48 * 1024),
                    "{case}, {filter}: {:?}",
                    result.0
                );
                assert_eq!(
                    format!("{result:?}"),
                    format!("{:?}", run(&input, limits, false))
                );
            }
        }
    }
}

#[test]
fn keeps_shapes_and_diagnostics_with_predictors_and_repeated_resources() {
    let shapes = |drawing: &Drawing2D| {
        drawing
            .sheets
            .iter()
            .flat_map(|s| &s.elements)
            .map(|e| format!("{:?} {:?}", e.shape, e.style))
            .collect::<Vec<_>>()
    };
    for case in 0..cases::COUNT {
        let plain = cases::plain(case);
        for early in [0, 1] {
            let good = encoding::encode(&plain, early != 0);
            let mut predicted = vec![0];
            predicted.extend(&plain);
            for direct in [false, true] {
                let expected = common::lift(&cases::document(case, "", &plain, direct, 3));
                for (filter, data) in [
                    (
                        format!("/Filter /LZWDecode /DP << /EarlyChange {early} >>"),
                        good.clone(),
                    ),
                    (
                        format!(
                            "/Filter [/Crypt /LZWDecode] /DP [null << /EarlyChange {early} >>]"
                        ),
                        good.clone(),
                    ),
                    (
                        format!(
                            "/Filter /LZWDecode /DP << /EarlyChange {early} /Predictor 12 /Columns {} >>",
                            plain.len()
                        ),
                        encoding::encode(&predicted, early != 0),
                    ),
                ] {
                    let input = cases::document(case, &filter, &data, direct, 3);
                    let actual = common::lift(&input);
                    if !direct {
                        assert!(!shapes(&expected.0).is_empty());
                        assert_eq!(shapes(&actual.0), shapes(&expected.0), "{case}, {filter}");
                        assert_eq!(
                            common::codes(&actual.1),
                            common::codes(&expected.1),
                            "{case}, {filter}"
                        );
                    }
                    assert_eq!(actual, common::lift(&input));
                }
            }
        }
    }
}

#[test]
fn repeated_forms_keep_decode_work_even_for_empty_and_failed_output() {
    for clears in [10, 1000, 10_000] {
        for early in [0, 1] {
            for end in [257, 500] {
                let mut codes = vec![256; clears];
                codes.push(end);
                let data = encoding::pack(&codes, early != 0);
                for predictor in ["", "/Predictor 3"] {
                    let filter =
                        format!("/Filter /LZWDecode /DP << /EarlyChange {early} {predictor} >>");
                    let once = cases::document(2, &filter, &data, false, 1);
                    let twice = cases::document(2, &filter, &data, false, 2);
                    let first = run(&once, Limits::default(), false);
                    let second = run(&twice, Limits::default(), false);
                    assert!(first.0.is_ok() && second.0.is_ok());
                    assert!(
                        second.1.used(Kind::Work) >= first.1.used(Kind::Work) + clears as u64,
                        "{clears}, {early}, {end}, {predictor}"
                    );
                    let limits = Limits::default()
                        .with(Kind::Work, Limit::Finite(first.1.used(Kind::Work) + 150));
                    let stopped = run(&twice, limits, false);
                    assert!(matches!(
                        stopped.0,
                        Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == Kind::Work
                    ));
                    assert_eq!(
                        format!("{stopped:?}"),
                        format!("{:?}", run(&twice, limits, false))
                    );
                }
            }
        }
    }
}

#[test]
fn content_arrays_keep_work_for_the_second_decode() {
    for early in [0, 1] {
        for end in [257, 500] {
            let mut codes = vec![256; 10_000];
            codes.push(end);
            let data = encoding::pack(&codes, early != 0);
            for predictor in ["", "/Predictor 3"] {
                let filter =
                    format!("/Filter /LZWDecode /DP << /EarlyChange {early} {predictor} >>");
                let single = cases::document(0, &filter, &data, false, 1);
                let array = cases::document(1, &filter, &data, false, 1);
                let first = run(&single, Limits::default(), false);
                let second = run(&array, Limits::default(), false);
                assert!(first.0.is_ok() && second.0.is_ok());
                assert!(second.1.used(Kind::Work) >= first.1.used(Kind::Work) + 10_000);
                let limits = Limits::default()
                    .with(Kind::Work, Limit::Finite(first.1.used(Kind::Work) + 150));
                assert!(matches!(
                    run(&array, limits, false).0,
                    Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == Kind::Work
                ));
            }
        }
    }
}

#[test]
fn lzw_reuse_failure_budgets_and_cancellation_are_repeatable() {
    for case in 0..cases::COUNT {
        let plain = cases::plain(case);
        for direct in [false, true] {
            for filter in [
                "/Filter /LZWDecode",
                "/Filter /LZWDecode /DP << /Predictor 3 >>",
            ] {
                let input =
                    cases::document(case, filter, &encoding::encode(&plain, true), direct, 3);
                let (expected, budget, _) = run(&input, Limits::default(), false);
                for kind in [Kind::DecodedBytes, Kind::Work] {
                    let exact = Limits::default().with(kind, Limit::Finite(budget.used(kind)));
                    assert_eq!(
                        format!("{:?}", run(&input, exact, false).0),
                        format!("{expected:?}")
                    );
                    let short = Limits::default().with(kind, Limit::Finite(budget.used(kind) - 1));
                    let stopped = run(&input, short, false);
                    assert!(
                        matches!(stopped.0, Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == kind),
                        "{case}, {filter}"
                    );
                    assert_eq!(
                        format!("{stopped:?}"),
                        format!("{:?}", run(&input, short, false))
                    );
                }
                let stopped = run(&input, Limits::default(), true);
                assert!(matches!(stopped.0, Err(Error::Stopped(Stop::Cancelled))));
                assert_eq!(stopped.1.used(Kind::DecodedBytes), 0);
            }
        }
    }
    // 辞書・出力を作った後で予測子が失敗しても、再び使う分の見積もりは残す。
    let data = encoding::encode(&vec![0; 2048], true);
    for case in [2, 4, 5, 6, 7, 8] {
        let once = cases::document(
            case,
            "/Filter /LZWDecode /DP << /Predictor 3 >>",
            &data,
            false,
            1,
        );
        let twice = cases::document(
            case,
            "/Filter /LZWDecode /DP << /Predictor 3 >>",
            &data,
            false,
            2,
        );
        assert!(
            run(&twice, Limits::default(), false)
                .1
                .used(Kind::DecodedBytes)
                > run(&once, Limits::default(), false)
                    .1
                    .used(Kind::DecodedBytes)
        );
    }
}
