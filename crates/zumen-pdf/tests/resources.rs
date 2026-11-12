//! 関数と描画資源の展開を、全出力の確保前に止める（決定 0273）。

#![cfg(feature = "drawing")]

use std::io::Write;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Stop, codes};
use zumen_pdf::Pdf;
use zumen_sdk::{Error, Lift, testkit};

mod common;
#[path = "common/resources.rs"]
mod resources;

#[cfg(test)]
fn compressed(data: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

#[test]
fn bounds_resource_flate_before_allocating_the_whole_output() {
    // 修正前も手元で安全に失敗する 128 KiB に限定する。
    let data = compressed(&vec![b'H'; 128 * 1024]);
    assert!(
        !data
            .iter()
            .any(|b| matches!(b, b'(' | b')' | b'<' | b'>' | b'[' | b']'))
    );
    assert!(data.len() < 256);
    for filter in [
        "/Filter /FlateDecode",
        "/F /Fl",
        "/Filter [/FlateDecode]",
        "/Filter [/Unknown /FlateDecode]",
        "/Filter [/Crypt /FlateDecode]",
        "/F [/Crypt /Fl] /DP [<< /Name /Identity >> null]",
        "/Filter /FlateDecode /DecodeParms << /Predictor 12 /Columns 1 >>",
    ] {
        for kind in 0..5 {
            for direct in [false, true] {
                let source = resources::document(kind, filter, &data, direct, 1);
                let file = testkit::read::<Pdf>(&source).0.unwrap();
                let run = || {
                    let mut budget = Budget::new(
                        Limits::default().with(Kind::DecodedBytes, Limit::Finite(80 * 1024)),
                    );
                    let mut diagnostics = Diagnostics::new();
                    let cancel = CancelToken::new();
                    let result = file.lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
                    assert!(
                        matches!(result, Err(Error::Stopped(Stop::Exceeded(e)))
                        if e.kind == Kind::DecodedBytes && e.requested < 96 * 1024),
                        "{filter}, {kind}, {direct}: {result:?}"
                    );
                    let diagnostics = diagnostics.into_vec();
                    assert_eq!(diagnostics[0].code(), codes::BUDGET_DECODED_BYTES_EXCEEDED);
                    format!("{result:?} {budget:?} {diagnostics:?}")
                };
                assert_eq!(run(), run());
            }
        }
    }
}

#[test]
fn keeps_resource_results_with_damage_and_predictors() {
    for kind in 0..5 {
        let plain = resources::plain(kind);
        let good = common::zlib(plain);
        let mut wrong = good.clone();
        *wrong.last_mut().unwrap() ^= 0xff;
        let mut predicted = vec![0];
        predicted.extend(plain);
        let predictor = format!(
            "/Filter [/FlateDecode] /DecodeParms [<< /Predictor 12 /Columns {} >>]",
            plain.len()
        );
        for direct in [false, true] {
            // 直接のストリームは仕様外で、符号化によってページごと読めなくなるので、同じ入力の再現性を確かめる。
            let expected =
                (!direct).then(|| common::lift(&resources::document(kind, "", plain, false, 2)));
            for (filter, data) in [
                ("/Filter /FlateDecode", good.clone()),
                ("/Filter [/Crypt /FlateDecode]", good.clone()),
                (
                    "/F [/Crypt /Fl] /DP [<< /Name /Identity >> null]",
                    good.clone(),
                ),
                ("/F /Fl", good[2..good.len() - 4].to_vec()),
                ("/Filter /FlateDecode", wrong.clone()),
                ("/Filter /FlateDecode", good[..good.len() - 4].to_vec()),
                (predictor.as_str(), common::zlib(&predicted)),
            ] {
                let source = resources::document(kind, filter, &data, direct, 2);
                let actual = common::lift(&source);
                // 圧縮の仕方が異なる入力の出所は比べず、描いた形・色と診断の code・件数を比べる。
                let shapes = |drawing: &zumen_drawing::Drawing2D| {
                    drawing
                        .sheets
                        .iter()
                        .flat_map(|sheet| &sheet.elements)
                        .map(|e| format!("{:?} {:?}", e.shape, e.style))
                        .collect::<Vec<_>>()
                };
                if let Some(expected) = &expected {
                    assert!(!shapes(&expected.0).is_empty(), "{kind}");
                    assert_eq!(shapes(&actual.0), shapes(&expected.0), "{kind}, {filter}");
                    assert_eq!(common::codes(&actual.1), common::codes(&expected.1));
                }
                // 同じ入力なら出所と ID を含めて一致する。
                assert_eq!(actual, common::lift(&source));
            }
        }
    }
}

#[test]
fn resource_budgets_and_cancellation_are_repeatable() {
    for kind in 0..5 {
        for direct in [false, true] {
            let data = compressed(resources::plain(kind));
            let source = resources::document(kind, "/Filter /FlateDecode", &data, direct, 3);
            let file = testkit::read::<Pdf>(&source).0.unwrap();
            let run = |limits, cancelled| {
                let mut budget = Budget::new(limits);
                let mut diagnostics = Diagnostics::new();
                let cancel = CancelToken::new();
                if cancelled {
                    cancel.cancel();
                }
                let result = file.lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
                (result, budget, diagnostics.into_vec())
            };
            let (expected, total, _) = run(Limits::default(), false);
            assert!(expected.is_ok());
            for kind in [Kind::DecodedBytes, Kind::Work] {
                let used = total.used(kind);
                let exact = Limits::default().with(kind, Limit::Finite(used));
                assert_eq!(run(exact, false).0.unwrap(), expected.clone().unwrap());
                let short = Limits::default().with(kind, Limit::Finite(used - 1));
                let stopped = run(short, false);
                assert!(
                    matches!(stopped.0, Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == kind)
                );
                assert_eq!(format!("{stopped:?}"), format!("{:?}", run(short, false)));
            }
            let cancelled = run(Limits::default(), true);
            assert!(matches!(cancelled.0, Err(Error::Stopped(Stop::Cancelled))));
            assert_eq!(cancelled.1.used(Kind::DecodedBytes), 0);
            assert_eq!(cancelled.2[0].code(), codes::CALL_REQUEST_CANCELLED);
        }
    }
}

#[test]
fn counts_repeated_resource_decoding_before_predictors_even_on_failure() {
    for kind in 0..5 {
        for predictor in [1, 12, 5] {
            let used = |calls| {
                // 予測子 12 は行頭を除き、未対応の 5 は展開後に失敗する。
                let filter = format!(
                    "/Filter /FlateDecode /DecodeParms << /Predictor {predictor} /Columns 1 >>"
                );
                let data = compressed(&vec![0; 8192]);
                let source = resources::document(kind, &filter, &data, false, calls);
                let file = testkit::read::<Pdf>(&source).0.unwrap();
                let mut budget = Budget::default();
                let mut diagnostics = Diagnostics::new();
                let cancel = CancelToken::new();
                file.lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel))
                    .unwrap();
                (budget.used(Kind::DecodedBytes), budget.used(Kind::Work))
            };
            let one = used(1);
            let two = used(2);
            assert!(
                two.0 - one.0 >= 8192,
                "{kind}, {predictor}: {one:?}, {two:?}"
            );
            assert!(
                two.1 - one.1 >= 1026,
                "{kind}, {predictor}: {one:?}, {two:?}"
            );
        }
    }
}
