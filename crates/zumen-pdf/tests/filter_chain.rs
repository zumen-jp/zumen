//! 確保前に数えられる段の後ろに別のフィルターが続く系列で、前の段を全量の確保前に数えることを検査する（#659）。
//!
//! hayro-syntax 0.9.0 はフィルターを順に当て、前の段の出力の全量を確保してから次を当てる。後ろの段が失敗しても
//! 縮めても、前の段の確保は起きる。

#![cfg(feature = "drawing")]

use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, Stop};
use zumen_drawing::Drawing2D;
use zumen_pdf::Pdf;
use zumen_sdk::{Error, Lift, testkit};

#[path = "common/ascii.rs"]
mod ascii;
#[path = "common/lzw_cases.rs"]
mod cases;
mod common;
#[path = "common/fonts.rs"]
mod fonts;
#[path = "common/lzw.rs"]
mod lzw;
#[path = "common/resources.rs"]
mod resources;

/// 64 KiB の空白。32 KiB の予算を超える
const PLAIN: usize = 64 * 1024;

/// 前の段の後ろに続ける段。先頭でない Crypt（失敗する）、ASCIIHex（空白を捨てて縮める）、RunLength（終端で縮める）
const TRAILING: [&str; 3] = ["/Crypt", "/AHx", "/RL"];

#[cfg(test)]
fn run(source: &[u8], limits: Limits) -> (Result<Drawing2D, Error>, Budget, Vec<Diagnostic>) {
    let file = testkit::read::<Pdf>(source).0.unwrap();
    let mut budget = Budget::new(limits);
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = file.lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
    (result, budget, diagnostics.into_vec())
}

/// 数えられる段の名前と、その段で 64 KiB の空白になる入力
#[cfg(test)]
fn heads() -> [(&'static str, Vec<u8>); 3] {
    // 128 個の空白のくり返し（7.4.5）を 512 個と終端
    let mut run_length = [129, b' '].repeat(PLAIN / 128);
    run_length.push(128);
    let plain = vec![b' '; PLAIN];
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    std::io::Write::write_all(&mut encoder, &plain).unwrap();
    [
        ("/RL", run_length),
        ("/Fl", encoder.finish().unwrap()),
        ("/LZW", lzw::encode(&plain, true)),
    ]
}

/// 前置しない、ASCIIHex を前置、ASCII85 を前置した、数えられる段とその入力
#[cfg(test)]
fn sequences() -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for (head, data) in heads() {
        out.push((head.to_owned(), data.clone()));
        out.push((format!("/AHx {head}"), ascii::encode(&data, false)));
        out.push((format!("/A85 {head}"), ascii::encode(&data, true)));
    }
    out
}

#[test]
fn counts_the_first_stage_before_trailing_filters_at_every_entry() {
    let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(32 * 1024));
    for (sequence, data) in sequences() {
        assert!(data.len() < 4 * 1024, "{sequence}");
        for trailing in TRAILING {
            for crypt in ["", "/Crypt "] {
                let filter = format!("/Filter [{crypt}{sequence} {trailing}]");
                for case in 0..cases::COUNT {
                    let input = cases::document(case, &filter, &data, false, 2);
                    let result = run(&input, limits);
                    assert!(
                        matches!(&result.0, Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == Kind::DecodedBytes && e.requested < 48 * 1024),
                        "{case}, {filter}: {:?}",
                        result.0
                    );
                    assert_eq!(format!("{result:?}"), format!("{:?}", run(&input, limits)));
                }
            }
        }
    }
}

#[test]
fn keeps_results_of_trailing_filters_with_ample_budgets() {
    let shapes = |drawing: &Drawing2D| {
        drawing
            .sheets
            .iter()
            .flat_map(|s| &s.elements)
            .map(|e| format!("{:?} {:?}", e.shape, e.style))
            .collect::<Vec<_>>()
    };
    for (sequence, data) in sequences() {
        for trailing in TRAILING {
            let filter = format!("/Filter [{sequence} {trailing}]");
            for case in 0..cases::COUNT {
                let input = cases::document(case, &filter, &data, false, 1);
                let (result, budget, diagnostics) = run(&input, Limits::default());
                let drawing = result.unwrap_or_else(|e| panic!("{case}, {filter}: {e:?}"));
                // 前の段の 64 KiB を、後ろの段の結果によらず数える
                assert!(
                    budget.used(Kind::DecodedBytes) >= PLAIN as u64,
                    "{case}, {filter}"
                );
                let again = run(&input, Limits::default());
                assert_eq!(
                    shapes(&drawing),
                    shapes(again.0.as_ref().unwrap()),
                    "{case}, {filter}"
                );
                assert_eq!(diagnostics, again.2);
                assert_eq!(budget, again.1);
            }
        }
    }
}

/// リテラルだけの RunLength（7.4.5）
#[cfg(test)]
fn literal_run_length(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for chunk in data.chunks(128) {
        out.push((chunk.len() - 1) as u8);
        out.extend(chunk);
    }
    out.push(128);
    out
}

#[test]
fn decodes_two_stage_sequences_like_the_plain_streams() {
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
        let expected = common::lift(&cases::document(case, "", &plain, false, 2));
        // 後ろの段（RunLength）で元に戻る入力を、前の段で符号化する
        let inner = literal_run_length(&plain);
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
        std::io::Write::write_all(&mut encoder, &inner).unwrap();
        for (head, data) in [
            ("/RL", literal_run_length(&inner)),
            ("/Fl", encoder.finish().unwrap()),
            ("/LZW", lzw::encode(&inner, true)),
        ] {
            for (prefix, data) in [
                ("", data.clone()),
                ("/AHx ", ascii::encode(&data, false)),
                ("/A85 ", ascii::encode(&data, true)),
            ] {
                let filter = format!("/Filter [{prefix}{head} /RL]");
                let input = cases::document(case, &filter, &data, false, 2);
                let actual = common::lift(&input);
                assert_eq!(shapes(&actual.0), shapes(&expected.0), "{case}, {filter}");
                assert_eq!(
                    common::codes(&actual.1),
                    common::codes(&expected.1),
                    "{case}, {filter}"
                );
                // 予算の境い目で、ちょうどなら同じ結果、1 少なければ超過で止まる
                let (result, budget, _) = run(&input, Limits::default());
                for kind in [Kind::DecodedBytes, Kind::Work] {
                    let exact = Limits::default().with(kind, Limit::Finite(budget.used(kind)));
                    assert_eq!(
                        format!("{:?}", run(&input, exact).0),
                        format!("{result:?}"),
                        "{case}, {filter}"
                    );
                    let short = Limits::default().with(kind, Limit::Finite(budget.used(kind) - 1));
                    assert!(
                        matches!(run(&input, short).0, Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == kind),
                        "{case}, {filter}"
                    );
                }
            }
        }
    }
}
