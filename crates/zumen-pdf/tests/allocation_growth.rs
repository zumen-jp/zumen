//! 単独 Flate・LZW の出力の Vec の伸長を、全量の確保前に数える（#772）。

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
#[path = "common/fonts.rs"]
mod fonts;
#[path = "common/lzw.rs"]
mod lzw;
#[path = "common/resources.rs"]
mod resources;

#[derive(Clone, Copy, Debug)]
enum Compression {
    Flate,
    Raw,
    Lzw(bool),
}

const COMPRESSIONS: [Compression; 3] = [
    Compression::Flate,
    Compression::Lzw(false),
    Compression::Lzw(true),
];

#[cfg(test)]
impl Compression {
    fn encode(self, plain: &[u8]) -> Vec<u8> {
        match self {
            Self::Flate => {
                let mut encoder =
                    flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
                encoder.write_all(plain).unwrap();
                encoder.finish().unwrap()
            }
            Self::Raw => {
                let mut encoder =
                    flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
                encoder.write_all(plain).unwrap();
                encoder.finish().unwrap()
            }
            Self::Lzw(early) => lzw::encode(plain, early),
        }
    }

    fn filter(self, prefix: usize, crypt: bool, predictor: &str) -> String {
        let name = match self {
            Self::Flate | Self::Raw => "FlateDecode",
            Self::Lzw(_) => "LZWDecode",
        };
        let early = match self {
            Self::Lzw(early) => u8::from(early),
            _ => 1,
        };
        let params = format!("<< /EarlyChange {early} {predictor} >>");
        if prefix == 0 && !crypt {
            return format!("/Filter /{name} /DecodeParms {params}");
        }
        let mut names = Vec::new();
        let mut parameters = Vec::new();
        if crypt {
            names.push("/Crypt");
            parameters.push("null".to_owned());
        }
        if prefix != 0 {
            names.push(if prefix == 1 { "/AHx" } else { "/A85" });
            parameters.push("null".to_owned());
        }
        names.push(match self {
            Self::Flate | Self::Raw => "/Fl",
            Self::Lzw(_) => "/LZW",
        });
        parameters.push(params);
        format!(
            "/Filter [{}] /DecodeParms [{}]",
            names.join(" "),
            parameters.join(" ")
        )
    }

    fn growth(self, output: usize) -> u64 {
        output as u64 * 4
            + match self {
                Self::Flate | Self::Raw => 256,
                Self::Lzw(_) => 32,
            }
    }
}

#[cfg(test)]
fn wrapped(data: &[u8], prefix: usize) -> Vec<u8> {
    match prefix {
        0 => data.to_vec(),
        _ => ascii::encode(data, prefix == 2),
    }
}

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

#[cfg(test)]
fn stopped(
    source: &[u8],
    kind: Kind,
    limit: u64,
) -> (zumen_base::Exceeded, Budget, Vec<Diagnostic>) {
    let limits = Limits::default().with(kind, Limit::Finite(limit));
    let result = run(source, limits, false);
    let Err(Error::Stopped(Stop::Exceeded(exceeded))) = &result.0 else {
        panic!("予算で止まらなかった");
    };
    assert_eq!(exceeded.kind, kind);
    assert!(result.1.used(kind) <= limit);
    assert_eq!(
        result
            .2
            .iter()
            .filter(|d| d.code() == kind.exceeded_code())
            .map(Diagnostic::count)
            .sum::<u64>(),
        1
    );
    assert_eq!(
        format!("{result:?}"),
        format!("{:?}", run(source, limits, false))
    );
    (*exceeded, result.1, result.2)
}

#[cfg(test)]
fn exact_and_one_less(source: &[u8]) {
    let expected = run(source, Limits::default(), false);
    assert!(expected.0.is_ok());
    for kind in [Kind::DecodedBytes, Kind::Work] {
        let used = expected.1.used(kind);
        assert!(used > 0);
        let exact = Limits::default().with(kind, Limit::Finite(used));
        let actual = run(source, exact, false);
        assert_eq!(format!("{:?}", actual.0), format!("{:?}", expected.0));
        assert_eq!(actual.2, expected.2);
        assert_eq!(actual.1.used(kind), used);
        stopped(source, kind, used - 1);
    }
}

#[cfg(test)]
fn shapes(drawing: &Drawing2D) -> Vec<String> {
    drawing
        .sheets
        .iter()
        .flat_map(|s| &s.elements)
        .map(|e| format!("{:?} {:?}", e.shape, e.style))
        .collect()
}

#[test]
fn reserves_output_capacity_before_materialization_at_every_entry() {
    const LENGTH: usize = 32 * 1024;
    const LIMIT: u64 = 128 * 1024;
    let plain = vec![b' '; LENGTH];
    assert_eq!(cases::COUNT, 22);
    for compression in COMPRESSIONS {
        let compressed = compression.encode(&plain);
        for prefix in 0..3 {
            let data = wrapped(&compressed, prefix);
            let filter = compression.filter(prefix, false, "");
            for case in 0..cases::COUNT {
                let source = cases::document(case, &filter, &data, false, 2);
                let (exceeded, budget, _) = stopped(&source, Kind::DecodedBytes, LIMIT);
                // 出力長の確認は通過し、全確保の予約自身で止まる。
                assert_eq!(
                    exceeded.requested - budget.used(Kind::DecodedBytes),
                    compression.growth(LENGTH),
                    "{case}, {compression:?}, {prefix}"
                );
            }
        }
    }
}

#[test]
fn ample_budgets_preserve_values_diagnostics_and_ids_at_every_entry() {
    for case in 0..cases::COUNT {
        let plain = cases::plain(case);
        let expected = common::lift(&cases::document(case, "", &plain, false, 2));
        for compression in COMPRESSIONS {
            let compressed = compression.encode(&plain);
            for prefix in 0..3 {
                let data = wrapped(&compressed, prefix);
                let filter = compression.filter(prefix, true, "");
                let source = cases::document(case, &filter, &data, false, 2);
                let actual = run(&source, Limits::default(), false);
                let drawing = actual.0.as_ref().unwrap();
                assert_eq!(shapes(drawing), shapes(&expected.0));
                assert_eq!(common::codes(&actual.2), common::codes(&expected.1));
                assert_eq!(
                    format!("{actual:?}"),
                    format!("{:?}", run(&source, Limits::default(), false))
                );
            }
        }
    }
}

#[test]
fn capacity_boundaries_obey_exact_bytes_and_work_limits() {
    for length in [
        0, 1, 7, 8, 9, 31, 32, 33, 8191, 8192, 8193, 16383, 16384, 16385,
    ] {
        let plain = vec![b' '; length];
        for compression in [
            Compression::Flate,
            Compression::Raw,
            Compression::Lzw(false),
            Compression::Lzw(true),
        ] {
            let compressed = compression.encode(&plain);
            for prefix in 0..3 {
                let filter = compression.filter(prefix, false, "");
                let data = wrapped(&compressed, prefix);
                // 内容と ObjStm は、文字の解釈やフォントの確保を混ぜずに境目を通せる。
                for case in [0, 3] {
                    let source = cases::document(case, &filter, &data, false, 1);
                    exact_and_one_less(&source);
                }
            }
        }
    }
}

#[test]
fn cancellation_stops_before_any_decode_allocation_at_every_entry() {
    let plain = vec![b' '; 32 * 1024];
    for compression in COMPRESSIONS {
        let compressed = compression.encode(&plain);
        for prefix in 0..3 {
            let filter = compression.filter(prefix, false, "");
            let data = wrapped(&compressed, prefix);
            for case in 0..cases::COUNT {
                let source = cases::document(case, &filter, &data, false, 2);
                let actual = run(&source, Limits::default(), true);
                assert!(matches!(actual.0, Err(Error::Stopped(Stop::Cancelled))));
                assert_eq!(actual.1.used(Kind::DecodedBytes), 0);
                assert_eq!(actual.1.used(Kind::Work), 0);
                assert_eq!(
                    format!("{actual:?}"),
                    format!("{:?}", run(&source, Limits::default(), true))
                );
            }
        }
    }
}

#[test]
fn redecoding_keeps_capacity_costs_after_predictor_failure() {
    const LENGTH: usize = 8193;
    let plain = vec![b' '; LENGTH];
    for compression in COMPRESSIONS {
        let compressed = compression.encode(&plain);
        for prefix in 0..3 {
            let filter = compression.filter(prefix, false, "/Predictor 3");
            let data = wrapped(&compressed, prefix);
            for case in [1, 2] {
                let once = cases::document(case, &filter, &data, false, 1);
                let twice = cases::document(case, &filter, &data, false, 2);
                let first = run(&once, Limits::default(), false);
                let second = run(&twice, Limits::default(), false);
                assert!(first.0.is_ok() && second.0.is_ok());
                assert!(
                    second.1.used(Kind::DecodedBytes) - first.1.used(Kind::DecodedBytes)
                        >= compression.growth(LENGTH),
                    "{case}, {compression:?}, {prefix}"
                );
                assert!(
                    second.1.used(Kind::Work) - first.1.used(Kind::Work)
                        >= compression.growth(LENGTH).div_ceil(16)
                );
                for kind in [Kind::DecodedBytes, Kind::Work] {
                    stopped(&twice, kind, first.1.used(kind));
                }
            }
        }
    }
}

#[test]
fn fallback_reserves_capacity_and_tables_before_materialization() {
    let plain = b"0 0 m 10 10 l S";
    let mut compressed = Compression::Flate.encode(plain);
    // 検査和だけを変え、両方の厳しい試みの後に寛容な展開へ進ませる。
    *compressed.last_mut().unwrap() ^= 1;
    let blocks = (compressed.len() as u64 * 8).div_ceil(3);
    let ample = Limits::default()
        .with(Kind::Work, Limit::Unlimited)
        .with(Kind::DecodedBytes, Limit::Unlimited);
    for prefix in 0..3 {
        let filter = Compression::Flate.filter(prefix, true, "");
        let data = wrapped(&compressed, prefix);
        for case in [0, 2] {
            let source = cases::document(case, &filter, &data, false, 1);
            let actual = run(&source, ample, false);
            let expected = common::lift(&cases::document(case, "", plain, false, 1));
            assert_eq!(shapes(actual.0.as_ref().unwrap()), shapes(&expected.0));
            assert_eq!(common::codes(&actual.2), common::codes(&expected.1));
            assert!(actual.1.used(Kind::DecodedBytes) >= blocks * (1 << 19));
            assert!(actual.1.used(Kind::Work) >= blocks * (1 << 24));
            assert_eq!(
                format!("{actual:?}"),
                format!("{:?}", run(&source, ample, false))
            );
            for kind in [Kind::DecodedBytes, Kind::Work] {
                let (exceeded, budget, _) = stopped(&source, kind, 512 * 1024);
                assert!(exceeded.requested - budget.used(kind) >= 512 * 1024);
            }
        }
    }
}

#[test]
fn invalid_lzw_keeps_partial_output_capacity_at_every_entry() {
    // 1〜256 バイトの語を出した後、まだ登録されていない符号で失敗する。
    let length = 256 * 257 / 2;
    let mut codes = vec![256, u16::from(b' ')];
    codes.extend(258..=512);
    codes.push(800);
    for early in [false, true] {
        let compression = Compression::Lzw(early);
        let compressed = lzw::pack(&codes, early);
        for prefix in 0..3 {
            let filter = compression.filter(prefix, false, "");
            let data = wrapped(&compressed, prefix);
            for case in 0..cases::COUNT {
                let source = cases::document(case, &filter, &data, false, 1);
                let (exceeded, budget, _) = stopped(&source, Kind::DecodedBytes, 128 * 1024);
                assert_eq!(
                    exceeded.requested - budget.used(Kind::DecodedBytes),
                    compression.growth(length),
                    "{case}, {early}, {prefix}"
                );
            }
            // 失敗までの確保を数えても、十分な予算での読み方と診断は再現する。
            let source = cases::document(2, &filter, &data, false, 1);
            exact_and_one_less(&source);
            let actual = run(&source, Limits::default(), false);
            assert!(actual.0.unwrap().sheets[0].elements.is_empty());
        }
    }
}
