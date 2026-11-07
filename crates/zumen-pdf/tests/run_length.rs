//! 単独 RunLength と、ASCII を前置した RunLength の、全量の確保前の予算と再利用を検査する（#629）。

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
#[path = "common/resources.rs"]
mod resources;

/// 単独、ASCIIHex を前置、ASCII85 を前置
const SEQUENCES: [Option<bool>; 3] = [None, Some(false), Some(true)];

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

/// 合成入力を RunLength（7.4.5）にする。同じバイトが 2 個以上続けばくり返し、ほかはリテラルにする
#[cfg(test)]
fn encode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let run = data[i..]
            .iter()
            .take(128)
            .take_while(|&&byte| byte == data[i])
            .count();
        if run >= 2 {
            out.extend([(257 - run) as u8, data[i]]);
            i += run;
            continue;
        }
        let start = i;
        while i < data.len() && i - start < 128 && data.get(i + 1) != Some(&data[i]) {
            i += 1;
        }
        out.push((i - start - 1) as u8);
        out.extend(&data[start..i]);
    }
    out.push(128);
    out
}

fn encoded(plain: &[u8], ascii: Option<bool>) -> Vec<u8> {
    let data = encode(plain);
    match ascii {
        Some(base85) => ascii::encode(&data, base85),
        None => data,
    }
}

/// hayro-syntax 0.9.0 が、先頭 Crypt を除いて RunLength だけか、ASCII と RunLength を当てる値
fn filters(ascii: Option<bool>) -> Vec<String> {
    let Some(base85) = ascii else {
        return [
            "/Filter /RunLengthDecode",
            "/F /RL",
            "/Filter [/RunLengthDecode] /DecodeParms [null]",
            "/F [/Crypt /RL] /DP [null null]",
            "/Filter [/Unknown /RunLengthDecode]",
            "/F /Unknown /Filter [/RL]",
            "/Filter [/RL 0 /Fl]",
        ]
        .map(String::from)
        .to_vec();
    };
    let (ascii, short) = if base85 {
        ("ASCII85Decode", "A85")
    } else {
        ("ASCIIHexDecode", "AHx")
    };
    vec![
        format!("/Filter [/{ascii} /RunLengthDecode]"),
        format!("/F [/{short} /RL] /DP [null null]"),
        format!("/Filter [/Crypt /{ascii} /RunLengthDecode]"),
        format!("/F [/Unknown /{ascii} /Unknown /RL]"),
        format!("/F /Unknown /Filter [/{short} /RL]"),
        format!("/Filter [/{ascii} /RunLengthDecode 0 /Fl]"),
    ]
}

#[test]
fn bounds_run_length_at_every_preflight_entry_before_whole_output_allocation() {
    for (ascii, length) in SEQUENCES.into_iter().zip([1025, 2051, 1284]) {
        // 128 個の空白のくり返しを 512 個と終端。64 KiB に展開する
        let data = encoded(&vec![b' '; 64 * 1024], ascii);
        assert_eq!(data.len(), length);
        for filter in filters(ascii) {
            for case in 0..cases::COUNT {
                // 全量の確保は仕様どおりの間接ストリームで検査する。直接の値は下で再現性を比べる
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
fn keeps_shapes_and_diagnostics_with_aliases_and_repeated_entries() {
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
        for direct in [false, true] {
            let expected = common::lift(&cases::document(case, "", &plain, direct, 3));
            for ascii in SEQUENCES {
                let data = encoded(&plain, ascii);
                for filter in filters(ascii) {
                    let input = cases::document(case, &filter, &data, direct, 3);
                    let actual = common::lift(&input);
                    if !direct {
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
fn exact_budgets_damage_and_cancellation_are_repeatable() {
    for case in 0..cases::COUNT {
        let plain = cases::plain(case);
        let whole = encode(&plain);
        let body = &whole[..whole.len() - 1];
        // 正しい入力、終端がない、くり返すバイトがない、途中で切れたリテラル
        let damaged = [
            whole.clone(),
            body.to_vec(),
            [body, &[200]].concat(),
            [body, &[5, b' ']].concat(),
        ];
        for data in damaged {
            for ascii in SEQUENCES {
                let filter = &filters(ascii)[0];
                let data = match ascii {
                    Some(base85) => ascii::encode(&data, base85),
                    None => data.clone(),
                };
                let input = cases::document(case, filter, &data, false, 3);
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
                let cancelled = run(&input, Limits::default(), true);
                assert!(matches!(cancelled.0, Err(Error::Stopped(Stop::Cancelled))));
                assert_eq!(cancelled.1.used(Kind::DecodedBytes), 0);
            }
        }
    }
}

#[test]
fn repeated_forms_and_content_arrays_keep_empty_and_failed_run_length_costs() {
    // 終端の後ろに長い入力が続く空の出力と、終端がなく失敗する出力。どちらも描くものはない
    let empty = [&[128][..], &[b' '; 16 * 1024][..]].concat();
    let failed = [0, b' '].repeat(8 * 1024);
    for data in [empty, failed] {
        for ascii in SEQUENCES {
            let filter = &filters(ascii)[0];
            let data = match ascii {
                Some(base85) => ascii::encode(&data, base85),
                None => data.clone(),
            };
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
            assert!(array.1.used(Kind::Work) >= single.1.used(Kind::Work) + 1024);
            let once = run(
                &cases::document(2, filter, &data, false, 1),
                Limits::default(),
                false,
            );
            let twice_input = cases::document(2, filter, &data, false, 2);
            let twice = run(&twice_input, Limits::default(), false);
            assert!(once.0.is_ok() && twice.0.is_ok());
            assert!(twice.1.used(Kind::Work) >= once.1.used(Kind::Work) + 1024);
            assert!(twice.1.used(Kind::DecodedBytes) >= once.1.used(Kind::DecodedBytes));
            let limit =
                Limits::default().with(Kind::Work, Limit::Finite(once.1.used(Kind::Work) + 150));
            assert!(
                matches!(run(&twice_input, limit, false).0, Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == Kind::Work)
            );
        }
    }
}

#[test]
fn external_file_specifications_do_not_run_ascii_and_run_length_on_embedded_bytes() {
    for file in ["(external)", "<< /Type /Filespec /F (external) >>"] {
        for base85 in [false, true] {
            let data = encoded(&vec![b' '; 64 * 1024], Some(base85));
            let filter = format!("/F {file} {}", filters(Some(base85))[0]);
            let input = cases::document(0, &filter, &data, false, 1);
            let ample = run(&input, Limits::default(), false);
            assert!(ample.0.is_ok());
            let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(32 * 1024));
            let small = run(&input, limits, false);
            assert_eq!(format!("{:?}", small.0), format!("{:?}", ample.0));
            assert_eq!(small.2, ample.2);
        }
    }
}
