//! 複数のフィルターが続く系列で、各段を全量の確保前に数えることを検査する（#521・#659）。
//!
//! hayro-syntax 0.9.0 はフィルターを順に当て、前の段の出力の全量を確保してから次を当てる。後ろの段が失敗しても
//! 縮めても、前の段の確保は起きる。ただし、系列のどこかに画像のフィルターがあるか、`F` が外部ファイルを指せば、どの段も当てずに失敗するので数えない。

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
fn does_not_count_chains_that_hayro_does_not_decode() {
    // hayro-syntax 0.9.0 の `decoded_data` は、画像のフィルターがあればどの段も当てず、`F` が外部ファイルを指せば
    // フィルターの前に失敗する。前の段を数えないので、小さい予算でも十分な予算と同じ結果になる
    let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(32 * 1024));
    for (sequence, data) in sequences() {
        let mut filters: Vec<String> = ["/DCT", "/CCF", "/JBIG2Decode", "/JPXDecode"]
            .iter()
            .map(|image| format!("/Filter [{sequence} {image}]"))
            .collect();
        filters.push(format!("/F (external) /Filter [{sequence} /AHx]"));
        for filter in filters {
            for case in 0..cases::COUNT {
                let input = cases::document(case, &filter, &data, false, 1);
                let ample = run(&input, Limits::default());
                let small = run(&input, limits);
                assert_eq!(
                    format!("{:?}", small.0),
                    format!("{:?}", ample.0),
                    "{case}, {filter}"
                );
                assert_eq!(small.2, ample.2, "{case}, {filter}");
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

#[cfg(test)]
fn flate(data: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    std::io::Write::write_all(&mut encoder, data).unwrap();
    encoder.finish().unwrap()
}

#[test]
fn bounds_second_and_third_expanding_stages_at_every_entry() {
    let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(32 * 1024));
    for (last, inner) in heads() {
        // 後段の LZW は、32 ビットの符号位置の上限を確かめる guard が従来どおり断る
        if last == "/LZW" {
            continue;
        }
        // 第 1 段の出力は 2 KiB 未満。第 2・第 3 段の 64 KiB を確保する前に止める
        let chains = [
            (format!("/RL {last}"), literal_run_length(&inner)),
            (format!("/Fl {last}"), flate(&inner)),
            (
                format!("/RL /Fl {last}"),
                literal_run_length(&flate(&inner)),
            ),
        ];
        for (chain, data) in chains {
            assert!(data.len() < 4 * 1024, "{chain}");
            for (prefix, data) in [
                ("", data.clone()),
                ("/Crypt ", data.clone()),
                ("/AHx ", ascii::encode(&data, false)),
                ("/Crypt /A85 ", ascii::encode(&data, true)),
            ] {
                // 後ろが成功・失敗・縮小のどれでも、そこへ進む前の確保を数える
                for tail in ["", " /Crypt", " /AHx"] {
                    let filter = format!("/Filter [{prefix}{chain}{tail}]");
                    for case in 0..cases::COUNT {
                        let input = cases::document(case, &filter, &data, false, 2);
                        let result = run(&input, limits);
                        assert!(
                            matches!(&result.0, Err(Error::Stopped(Stop::Exceeded(e)))
                                if e.kind == Kind::DecodedBytes && e.requested < 48 * 1024),
                            "{case}, {filter}: {:?}",
                            result.0
                        );
                        assert_eq!(format!("{result:?}"), format!("{:?}", run(&input, limits)));
                    }
                }
            }
        }
    }
}

#[test]
fn keeps_rejecting_later_lzw_before_any_expanding_stage() {
    let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(32 * 1024));
    let inner = lzw::encode(&vec![b' '; PLAIN], true);
    for (first, data) in [("/RL", literal_run_length(&inner)), ("/Fl", flate(&inner))] {
        for (prefix, data) in [
            ("", data.clone()),
            ("/Crypt ", data.clone()),
            ("/AHx ", ascii::encode(&data, false)),
        ] {
            let filter = format!("/Filter [{prefix}{first} /LZW]");
            for case in 0..cases::COUNT {
                let input = cases::document(case, &filter, &data, false, 2);
                let small = run(&input, limits);
                let ample = run(&input, Limits::default());
                assert!(
                    matches!(small.0, Err(Error::Unsupported)),
                    "{case}, {filter}"
                );
                assert!(
                    matches!(ample.0, Err(Error::Unsupported)),
                    "{case}, {filter}"
                );
                assert_eq!(small.2, ample.2, "{case}, {filter}");
                assert_eq!(
                    common::codes(&small.2),
                    vec![("pdf.document.unsupported", 1)]
                );
                for kind in [Kind::DecodedBytes, Kind::Work] {
                    assert_eq!(small.1.used(kind), ample.1.used(kind), "{case}, {filter}");
                }
            }
        }
    }
}

#[test]
fn counts_expanded_stages_before_a_run_length_terminator() {
    // 第 2 段は 64 KiB に戻すが、第 3 段は先頭の EOD で空にする
    let plain = vec![128; PLAIN];
    let inner = flate(&plain);
    let data = literal_run_length(&inner);
    let filter = "/Filter [/RL /Fl /RL]";
    let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(32 * 1024));
    for case in 0..cases::COUNT {
        let input = cases::document(case, filter, &data, false, 2);
        let result = run(&input, limits);
        assert!(
            matches!(&result.0, Err(Error::Stopped(Stop::Exceeded(e)))
                if e.kind == Kind::DecodedBytes && e.requested < 48 * 1024),
            "{case}: {:?}",
            result.0
        );
        let ample = run(&input, Limits::default());
        assert!(ample.0.is_ok(), "{case}: {:?}", ample.0);
        assert!(ample.1.used(Kind::DecodedBytes) >= PLAIN as u64);
    }
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
                // 予算の境目で、ちょうどなら同じ結果、1 少なければ超過で止まる
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

#[test]
fn keeps_plain_values_for_later_flate_stages() {
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
        let inner = flate(&plain);
        for (first, encoded) in [
            ("/RL", literal_run_length(&inner)),
            ("/Fl", flate(&inner)),
            ("/LZW", lzw::encode(&inner, true)),
        ] {
            for (prefix, data) in [
                ("", encoded.clone()),
                ("/Crypt ", encoded.clone()),
                ("/AHx ", ascii::encode(&encoded, false)),
            ] {
                let filter = format!("/Filter [{prefix}{first} /Fl]");
                let input = cases::document(case, &filter, &data, false, 2);
                let (actual, _, diagnostics) = run(&input, Limits::default());
                let actual =
                    actual.unwrap_or_else(|e| panic!("{case}, {filter}: {e:?}, {diagnostics:?}"));
                assert_eq!(shapes(&actual), shapes(&expected.0), "{case}, {filter}");
                assert_eq!(
                    common::codes(&diagnostics),
                    common::codes(&expected.1),
                    "{case}, {filter}"
                );
            }
        }
    }
}

#[test]
fn keeps_each_stage_decode_parameters_and_predictor_output() {
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
        // 両段に PNG の予測子を付ける。前の段では次の符号列を、後ろの段では元の値を戻す
        let mut predicted = vec![0];
        predicted.extend(&plain);
        for early in [0, 1] {
            let inner = flate(&predicted);
            let mut predicted_inner = vec![0];
            predicted_inner.extend(&inner);
            for (first, encoded, first_early) in [
                ("/Fl", flate(&predicted_inner), String::new()),
                (
                    "/LZW",
                    lzw::encode(&predicted_inner, early != 0),
                    format!(" /EarlyChange {early}"),
                ),
            ] {
                // 先頭の Crypt と ASCII も DecodeParms の位置を 1 つずつ進める
                let filter = format!(
                    "/Filter [/Crypt /AHx {first} /Fl] /DecodeParms [null null << /Predictor 12 /Columns {}{first_early} >> << /Predictor 12 /Columns {} >>]",
                    inner.len(),
                    plain.len()
                );
                let data = ascii::encode(&encoded, false);
                let input = cases::document(case, &filter, &data, false, 2);
                let (actual, _, diagnostics) = run(&input, Limits::default());
                let actual =
                    actual.unwrap_or_else(|e| panic!("{case}, {filter}: {e:?}, {diagnostics:?}"));
                assert_eq!(shapes(&actual), shapes(&expected.0), "{case}, {filter}");
                assert_eq!(
                    common::codes(&diagnostics),
                    common::codes(&expected.1),
                    "{case}, {filter}"
                );
                let (result, budget, diagnostics) = run(&input, Limits::default());
                for kind in [Kind::DecodedBytes, Kind::Work] {
                    let exact = Limits::default().with(kind, Limit::Finite(budget.used(kind)));
                    let actual = run(&input, exact);
                    assert_eq!(format!("{:?}", actual.0), format!("{result:?}"));
                    assert_eq!(actual.2, diagnostics);
                    let short = Limits::default().with(kind, Limit::Finite(budget.used(kind) - 1));
                    assert!(
                        matches!(run(&input, short).0, Err(Error::Stopped(Stop::Exceeded(e)))
                            if e.kind == kind),
                        "{case}, {filter}"
                    );
                }
            }
        }
    }
}

#[test]
fn lzw_keeps_its_early_change_before_a_later_flate_stage() {
    // コメントを長くして、LZW で 9 ビットから 10 ビットへの切り替えを通す
    let mut plain = String::from("%");
    for value in 0..1024 {
        use std::fmt::Write;
        write!(&mut plain, "{value:04x}").unwrap();
    }
    plain.push_str("\n0 0 m 10 10 l S");
    let inner = common::zlib(plain.as_bytes());
    assert_ne!(lzw::encode(&inner, false), lzw::encode(&inner, true));
    let expected = common::lift(&cases::document(0, "", plain.as_bytes(), false, 1));
    for early in [0, 1] {
        let data = lzw::encode(&inner, early != 0);
        let filter = format!(
            "/Filter [/LZW /Fl] /DecodeParms [<< /EarlyChange {early} >> << /EarlyChange {} >>]",
            1 - early,
        );
        let input = cases::document(0, &filter, &data, false, 1);
        let (actual, _, diagnostics) = run(&input, Limits::default());
        let actual = actual.unwrap_or_else(|e| panic!("{filter}: {e:?}, {diagnostics:?}"));
        let shapes = |drawing: &Drawing2D| {
            drawing
                .sheets
                .iter()
                .flat_map(|s| &s.elements)
                .map(|e| format!("{:?} {:?}", e.shape, e.style))
                .collect::<Vec<_>>()
        };
        assert_eq!(shapes(&actual), shapes(&expected.0), "{filter}");
        assert_eq!(common::codes(&diagnostics), common::codes(&expected.1));
    }
}
