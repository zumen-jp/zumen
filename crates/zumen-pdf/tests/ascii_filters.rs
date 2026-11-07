//! ASCII を前置した圧縮の、全量の確保前の予算と再利用を検査する（#623）。

#![cfg(feature = "drawing")]

use std::io::Write;
use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, Stop};
use zumen_drawing::Drawing2D;
use zumen_pdf::Pdf;
use zumen_sdk::{Error, Lift, testkit};

#[path = "common/ascii.rs"]
mod ascii;
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

// 7.6、空パスワードの Standard R2。固定の ID は ASCII の 0 が 16 個で、4 0 obj の
// 平文は「0 0 m 10 10 l S」を zlib・ASCIIHex の順にしたもの。終端の後ろは描画に影響しない。
fn encrypted_page(length: usize, declaration: &str) -> Vec<u8> {
    let mut b = common::Builder::default();
    b.object("<< /Type /Catalog /Pages 2 0 R >>");
    b.object("<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    b.object("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >>");
    let mut body = format!("<< {declaration} /Filter [/AHx /Fl] >>\nstream\n").into_bytes();
    let start = body.len();
    body.extend([
        0x39, 0x18, 0x09, 0xd8, 0x8c, 0x9c, 0x60, 0x9c, 0x5e, 0xa9, 0x88, 0x17, 0x1d, 0x6f, 0x8c,
        0x64, 0x28, 0xc2, 0xfb, 0xa7, 0x48, 0x9a, 0x1d, 0xe8, 0x38, 0x95, 0x38, 0x7c, 0xe5, 0x59,
        0x6d, 0x9a, 0x67, 0xa7, 0xca, 0xfa, 0xe2, 0x63, 0x37, 0xc0, 0x9c,
    ]);
    body.resize(start + length, b'X');
    body.extend(b"\nendstream");
    b.object(body);
    b.object("<< /Filter /Standard /V 1 /R 2 /P -4 /O <2055c756c72e1ad702608e8196acad447ad32d17cff583235f6dd15fed7dab67> /U <92c034164f1ef00c9edf3ba7595c16b11c50ff99a474b82f495eabb623e302f8> >>");
    b.object(length.to_string());
    b.trailer = "/Encrypt 5 0 R /ID [<30303030303030303030303030303030> <30303030303030303030303030303030>]".into();
    b.bytes(1)
}

#[test]
fn reserves_decryption_with_correct_wrong_missing_and_indirect_lengths() {
    for declaration in ["/Length 65535", "/Length 1", "", "/Length 6 0 R"] {
        let input = encrypted_page(65535, declaration);
        let (drawing, budget, _) = run(&input, Limits::default(), false);
        assert_eq!(
            drawing.unwrap().sheets[0].elements.len(),
            1,
            "{declaration}"
        );
        assert!(budget.used(Kind::DecodedBytes) >= 65535 * 2);
        let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(32 * 1024));
        assert!(matches!(run(&input, limits, false).0,
            Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == Kind::DecodedBytes && e.requested >= 65535 * 2));
    }
}

#[test]
fn charges_external_file_strings_before_hayro_materializes_them() {
    for value in [
        format!("<{}>", "41".repeat(65535)),
        format!("({})", r"\101".repeat(65535)),
    ] {
        // 直接の文字列、F の間接参照、同じ辞書に別の参照がある直接の文字列。
        for (indirect, extra) in [(false, ""), (true, ""), (false, "/X 5 0 R")] {
            let mut b = common::Builder::default();
            b.object("<< /Type /Catalog /Pages 2 0 R >>");
            b.object("<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
            b.object("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >>");
            let file = if indirect { "5 0 R" } else { &value };
            b.stream(&format!("/F {file} {extra} /Filter [/AHx /Fl]"), b"00>");
            if indirect {
                b.object(&value);
            } else if !extra.is_empty() {
                b.object("42");
            }
            let input = b.bytes(1);
            let ample = run(&input, Limits::default(), false);
            assert!(ample.0.unwrap().sheets[0].elements.is_empty());
            assert!(ample.1.used(Kind::DecodedBytes) >= 65535);
            let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(32 * 1024));
            assert!(
                matches!(run(&input, limits, false).0, Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == Kind::DecodedBytes)
            );
        }
    }
}

#[test]
fn bounds_ascii_compressed_streams_before_whole_output_allocation() {
    for base85 in [false, true] {
        for mode in 0..3 {
            let data = ascii::encode(&compressed(&vec![b' '; 64 * 1024], mode), base85);
            assert!(data.len() < 1024);
            for filter in filters(base85, mode, "") {
                for case in 0..cases::COUNT {
                    let input = cases::document(case, &filter, &data, false, 2);
                    let limits =
                        Limits::default().with(Kind::DecodedBytes, Limit::Finite(32 * 1024));
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
}

#[cfg(test)]
fn compressed(data: &[u8], mode: u8) -> Vec<u8> {
    if mode != 0 {
        return encoding::encode(data, mode == 2);
    }
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

fn filters(base85: bool, mode: u8, predictor: &str) -> Vec<String> {
    let ascii = if base85 {
        "ASCII85Decode"
    } else {
        "ASCIIHexDecode"
    };
    let short = if base85 { "A85" } else { "AHx" };
    let compression = if mode == 0 {
        "FlateDecode"
    } else {
        "LZWDecode"
    };
    let alias = if mode == 0 { "Fl" } else { "LZW" };
    let params = format!("<< /EarlyChange {} {predictor} >>", u8::from(mode == 2));
    vec![
        format!("/Filter [/{ascii} /{compression}] /DecodeParms [null {params}]"),
        format!("/F [/{short} /{alias}] /DP [null {params}]"),
        format!("/Filter [/Crypt /{ascii} /{compression}] /DP [null null {params}]"),
        format!("/F [/Unknown /{ascii} /Unknown /{compression}] /DP [null null null {params}]"),
        format!("/F /Unknown /Filter [/{short} /{alias}] /DP null /DecodeParms [null {params}]"),
        format!("/Filter [/{ascii} /{compression} 0 /Fl] /DP [null {params}]"),
    ]
}

#[test]
fn keeps_output_and_ids_with_predictors_parameters_and_repeated_entries() {
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
        for base85 in [false, true] {
            for mode in 0..3 {
                for direct in [false, true] {
                    let expected = common::lift(&cases::document(case, "", &plain, direct, 3));
                    for predictor in [false, true] {
                        let mut predicted = Vec::new();
                        if predictor {
                            predicted.push(0);
                        }
                        predicted.extend(&plain);
                        let data = ascii::encode(&compressed(&predicted, mode), base85);
                        let param = if predictor {
                            format!("/Predictor 12 /Columns {}", plain.len())
                        } else {
                            String::new()
                        };
                        for filter in filters(base85, mode, &param) {
                            let input = cases::document(case, &filter, &data, direct, 3);
                            let actual = common::lift(&input);
                            if !direct {
                                assert_eq!(
                                    shapes(&actual.0),
                                    shapes(&expected.0),
                                    "{case}, {filter}"
                                );
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
    }
}

#[test]
fn exact_budgets_failure_and_cancellation_are_repeatable() {
    for case in 0..cases::COUNT {
        let plain = cases::plain(case);
        for base85 in [false, true] {
            for mode in 0..3 {
                for predictor in ["", "/Predictor 3"] {
                    let filter = &filters(base85, mode, predictor)[0];
                    let data = ascii::encode(&compressed(&plain, mode), base85);
                    let input = cases::document(case, filter, &data, false, 3);
                    let (expected, budget, _) = run(&input, Limits::default(), false);
                    for kind in [Kind::DecodedBytes, Kind::Work] {
                        let exact = Limits::default().with(kind, Limit::Finite(budget.used(kind)));
                        assert_eq!(
                            format!("{:?}", run(&input, exact, false).0),
                            format!("{expected:?}")
                        );
                        let short =
                            Limits::default().with(kind, Limit::Finite(budget.used(kind) - 1));
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
                    let cancelled = run(&input, Limits::default(), true);
                    assert!(matches!(cancelled.0, Err(Error::Stopped(Stop::Cancelled))));
                    assert_eq!(cancelled.1.used(Kind::DecodedBytes), 0);
                }
            }
        }
    }
}

#[test]
fn repeated_forms_and_content_arrays_keep_empty_and_failed_ascii_costs() {
    for base85 in [false, true] {
        for mode in 0..3 {
            for invalid in [false, true] {
                let mut data = vec![b' '; 16 * 1024];
                if invalid {
                    data.push(b'v');
                }
                data.extend(ascii::encode(&compressed(b"", mode), base85));
                let filter = &filters(base85, mode, "")[0];
                let single = run(
                    &cases::document(0, filter, &data, false, 1),
                    Limits::default(),
                    false,
                );
                let array = run(
                    &cases::document(1, filter, &data, false, 1),
                    Limits::default(),
                    false,
                );
                assert!(single.0.is_ok() && array.0.is_ok());
                assert!(array.1.used(Kind::Work) >= single.1.used(Kind::Work) + 2048);
                let once = run(
                    &cases::document(2, filter, &data, false, 1),
                    Limits::default(),
                    false,
                );
                let twice_input = cases::document(2, filter, &data, false, 2);
                let twice = run(&twice_input, Limits::default(), false);
                assert!(once.0.is_ok() && twice.0.is_ok());
                assert!(twice.1.used(Kind::Work) >= once.1.used(Kind::Work) + 2048);
                assert!(twice.1.used(Kind::DecodedBytes) > once.1.used(Kind::DecodedBytes));
                let limit = Limits::default()
                    .with(Kind::Work, Limit::Finite(once.1.used(Kind::Work) + 150));
                assert!(
                    matches!(run(&twice_input, limit, false).0, Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == Kind::Work)
                );
            }
        }
    }
}

#[test]
fn external_file_specifications_do_not_run_the_filters_on_embedded_bytes() {
    for file in ["(external)", "<< /Type /Filespec /F (external) >>"] {
        for base85 in [false, true] {
            for mode in 0..3 {
                let ascii = ascii::encode(&compressed(&vec![b' '; 64 * 1024], mode), base85);
                let filter = format!("/F {file} {}", filters(base85, mode, "")[0]);
                let input = cases::document(0, &filter, &ascii, false, 1);
                let ample = run(&input, Limits::default(), false);
                assert!(ample.0.is_ok());
                let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(32 * 1024));
                let small = run(&input, limits, false);
                assert_eq!(format!("{:?}", small.0), format!("{:?}", ample.0));
                assert_eq!(small.2, ample.2);
            }
        }
    }
}
