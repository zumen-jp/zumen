//! 単独 Flate・LZW と ASCII の前置で、予測子の配列と作業を確保前に予約する（#774）。

#![cfg(feature = "drawing")]

use std::io::Write;

use hayro_syntax::object::Stream;
use hayro_syntax::reader::{Reader, ReaderContext, ReaderExt};
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

#[derive(Clone, Copy, Debug)]
struct Predictor {
    number: u8,
    colors: u8,
    bits: u8,
    columns: usize,
}

const GENERIC: [Predictor; 4] = [
    Predictor {
        number: 2,
        colors: 5,
        bits: 1,
        columns: 8,
    },
    Predictor {
        number: 2,
        colors: 5,
        bits: 16,
        columns: 1,
    },
    Predictor {
        number: 12,
        colors: 5,
        bits: 8,
        columns: 1,
    },
    Predictor {
        number: 15,
        colors: 5,
        bits: 16,
        columns: 1,
    },
];

const LARGE: Predictor = Predictor {
    number: 12,
    colors: 5,
    bits: 8,
    columns: 209_715,
};

#[cfg(test)]
impl Predictor {
    fn row(self) -> usize {
        (self.columns * usize::from(self.colors) * usize::from(self.bits)).div_ceil(8)
    }

    fn params(self, early: bool) -> String {
        format!(
            "<< /Predictor {} /Colors {} /BitsPerComponent {} /Columns {} /EarlyChange {} >>",
            self.number,
            self.colors,
            self.bits,
            self.columns,
            u8::from(early)
        )
    }
}

#[cfg(test)]
impl Compression {
    fn name(self) -> &'static str {
        match self {
            Self::Flate | Self::Raw => "/Fl",
            Self::Lzw(_) => "/LZW",
        }
    }

    fn early(self) -> bool {
        !matches!(self, Self::Lzw(false))
    }

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

    fn filter(self, prefix: usize, crypt: bool, predictor: Predictor) -> String {
        let params = predictor.params(self.early());
        if prefix == 0 && !crypt {
            return format!("/Filter {} /DecodeParms {params}", self.name());
        }
        let mut names = Vec::new();
        let mut parameters = Vec::new();
        if crypt {
            names.push("/Crypt");
            parameters.push("null");
        }
        if prefix != 0 {
            names.push(ascii_name(prefix));
            parameters.push("null");
        }
        names.push(self.name());
        parameters.push(&params);
        format!(
            "/Filter [{}] /DP [{}]",
            names.join(" "),
            parameters.join(" ")
        )
    }
}

#[cfg(test)]
fn ascii_name(prefix: usize) -> &'static str {
    if prefix == 1 { "/AHx" } else { "/A85" }
}

#[cfg(test)]
fn wrapped(data: &[u8], prefix: usize) -> Vec<u8> {
    if prefix == 0 {
        data.to_vec()
    } else {
        ascii::encode(data, prefix == 2)
    }
}

type Outcome = (Result<Drawing2D, Error>, Budget, Vec<Diagnostic>);

#[cfg(test)]
fn run(source: &[u8], limits: Limits, cancelled: bool) -> Outcome {
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
fn stopped(source: &[u8], kind: Kind, limit: u64) -> (zumen_base::Exceeded, Budget) {
    let limits = Limits::default().with(kind, Limit::Finite(limit));
    let actual = run(source, limits, false);
    let Err(Error::Stopped(Stop::Exceeded(exceeded))) = actual.0 else {
        panic!("予算で止まらなかった");
    };
    assert_eq!(exceeded.kind, kind);
    assert!(actual.1.used(kind) <= limit);
    assert_eq!(
        actual
            .2
            .iter()
            .filter(|d| d.code() == kind.exceeded_code())
            .map(Diagnostic::count)
            .sum::<u64>(),
        1
    );
    let again = run(source, limits, false);
    assert_eq!(actual.1, again.1);
    assert_eq!(actual.2, again.2);
    assert!(matches!(again.0, Err(Error::Stopped(Stop::Exceeded(e))) if e == exceeded));
    (exceeded, actual.1)
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

#[cfg(test)]
fn decoded(filter: &str, data: &[u8]) -> Option<Vec<u8>> {
    let source = common::stream(filter, data);
    let stream = Reader::new(&source)
        .read_with_context::<Stream<'_>>(&ReaderContext::dummy())
        .unwrap();
    stream.decoded_data().ok().map(|data| data.into_owned())
}

#[cfg(test)]
fn exact_and_one_less(source: &[u8]) {
    let expected = run(source, Limits::default(), false);
    assert!(expected.0.is_ok());
    for kind in [Kind::DecodedBytes, Kind::Work] {
        let used = expected.1.used(kind);
        assert!(used > 0);
        let actual = run(
            source,
            Limits::default().with(kind, Limit::Finite(used)),
            false,
        );
        assert_eq!(format!("{:?}", actual.0), format!("{:?}", expected.0));
        assert_eq!(actual.2, expected.2);
        assert_eq!(actual.1.used(kind), used);
        stopped(source, kind, used - 1);
    }
}

#[test]
fn generic_predictors_reserve_their_own_cost_at_every_entry() {
    let plain = vec![0; 1024];
    assert_eq!(cases::COUNT, 22);
    for predictor in GENERIC {
        for compression in COMPRESSIONS {
            let compressed = compression.encode(&plain);
            for prefix in 0..3 {
                let data = wrapped(&compressed, prefix);
                let filter = compression.filter(prefix, prefix != 0, predictor);
                for case in 0..cases::COUNT {
                    let source = cases::document(case, &filter, &data, false, 1);
                    let inactive = Predictor {
                        number: 1,
                        ..predictor
                    };
                    let baseline = cases::document(
                        case,
                        &compression.filter(prefix, prefix != 0, inactive),
                        &data,
                        false,
                        1,
                    );
                    for kind in [Kind::DecodedBytes, Kind::Work] {
                        let limit = if kind == Kind::DecodedBytes {
                            192 * 1024
                        } else {
                            64 * 1024
                        };
                        // 前段の費用は内容入口で確認する。資源の後段の解析費用は、この停止とは分ける。
                        if case == 0 {
                            assert!(
                                run(
                                    &baseline,
                                    Limits::default().with(kind, Limit::Finite(limit)),
                                    false
                                )
                                .0
                                .is_ok(),
                                "baseline: {compression:?}, {prefix}, {predictor:?}, {kind:?}"
                            );
                        }
                        let (exceeded, budget) = stopped(&source, kind, limit);
                        // 出力長・容量の予約は小さい。予測子自身の予約で止まったことを確認する。
                        let reservation = exceeded.requested - budget.used(kind);
                        assert!(
                            reservation >= 128 * 1024,
                            "{case}, {compression:?}, {prefix}, {predictor:?}, {kind:?}"
                        );
                        // LZW の辞書や Flate の出力容量を、予測子の入力長に含めない。
                        let per_byte = if kind == Kind::DecodedBytes { 512 } else { 256 };
                        assert!(
                            reservation
                                <= plain.len() as u64 * per_byte
                                    + predictor.row() as u64
                                    + 16 * 1024
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn generic_tiff_and_png_values_follow_the_existing_decoder() {
    let mut predictors = Vec::new();
    for bits in [1, 2, 4, 8, 16] {
        predictors.push(Predictor {
            number: 2,
            colors: 5,
            bits,
            columns: 8,
        });
    }
    predictors.extend([
        Predictor {
            number: 12,
            colors: 5,
            bits: 8,
            columns: 3,
        },
        Predictor {
            number: 15,
            colors: 5,
            bits: 16,
            columns: 3,
        },
        Predictor {
            number: 12,
            colors: 3,
            bits: 4,
            columns: 2,
        },
    ]);
    for predictor in predictors {
        let row = predictor.row();
        let mut plain = Vec::new();
        let rows = if predictor.number >= 10 { 6 } else { 3 };
        for index in 0..rows {
            if predictor.number >= 10 {
                plain.push([0, 1, 2, 3, 4, 255][index]);
            }
            plain.extend((0..row).map(|i| (i * 37 + index * 11) as u8));
        }
        let filter = Compression::Flate.filter(0, false, predictor);
        let expected = decoded(&filter, &Compression::Flate.encode(&plain)).unwrap();
        for compression in [
            Compression::Flate,
            Compression::Raw,
            Compression::Lzw(false),
            Compression::Lzw(true),
        ] {
            let compressed = compression.encode(&plain);
            for prefix in 0..3 {
                let filter = compression.filter(prefix, true, predictor);
                let data = wrapped(&compressed, prefix);
                assert_eq!(decoded(&filter, &data), Some(expected.clone()));
                let source = cases::document(0, &filter, &data, false, 1);
                let actual = run(&source, Limits::default(), false);
                let baseline = common::lift(&cases::document(0, "", &expected, false, 1));
                assert_eq!(shapes(actual.0.as_ref().unwrap()), shapes(&baseline.0));
                assert_eq!(common::codes(&actual.2), common::codes(&baseline.1));
            }
        }
    }
}

#[test]
fn ample_budgets_keep_shapes_diagnostics_and_ids_at_every_entry() {
    for case in 0..cases::COUNT {
        for predictor in [GENERIC[2], GENERIC[3]] {
            let mut plain = cases::plain(case);
            let row = predictor.row();
            plain.resize(plain.len().div_ceil(row) * row, b' ');
            let baseline = common::lift(&cases::document(case, "", &plain, false, 2));
            let mut predicted = Vec::new();
            for chunk in plain.chunks(row) {
                predicted.push(0);
                predicted.extend(chunk);
            }
            for compression in COMPRESSIONS {
                let compressed = compression.encode(&predicted);
                for prefix in 0..3 {
                    let filter = compression.filter(prefix, true, predictor);
                    let data = wrapped(&compressed, prefix);
                    let source = cases::document(case, &filter, &data, false, 2);
                    let actual = run(&source, Limits::default(), false);
                    assert_eq!(shapes(actual.0.as_ref().unwrap()), shapes(&baseline.0));
                    assert_eq!(common::codes(&actual.2), common::codes(&baseline.1));
                    assert_eq!(
                        format!("{actual:?}"),
                        format!("{:?}", run(&source, Limits::default(), false))
                    );
                }
            }
        }
    }
}

#[cfg(test)]
fn typed_variants(compression: Compression, prefix: usize) -> Vec<(String, bool)> {
    let generic = LARGE.params(compression.early());
    let fast = format!(
        "<< /Predictor 1 /EarlyChange {} >>",
        u8::from(compression.early())
    );
    let head = compression.name();
    let (names, generic_params, fast_params, bad_item) = if prefix == 0 {
        (
            head.to_owned(),
            generic.clone(),
            fast.clone(),
            "17".to_owned(),
        )
    } else {
        (
            format!("{} {head}", ascii_name(prefix)),
            format!("null {generic}"),
            format!("null {fast}"),
            "null 17".to_owned(),
        )
    };
    let mut variants = vec![
        (
            format!("/Filter [{names}] /DP [{generic_params}] /DecodeParms [{fast_params}]"),
            true,
        ),
        (
            format!("/Filter [{names}] /DP [{fast_params}] /DecodeParms [{generic_params}]"),
            false,
        ),
        (
            format!("/Filter [{names}] /DP null /DecodeParms [{generic_params}]"),
            true,
        ),
        (
            format!("/Filter [{names}] /DP {fast} /DecodeParms [{generic_params}]"),
            true,
        ),
        (
            format!("/Filter [{names}] /DP [{bad_item}] /DecodeParms [{generic_params}]"),
            false,
        ),
        (
            format!("/F /Unknown /Filter [{names}] /DP [{generic_params}]"),
            true,
        ),
        (
            format!("/F 17 /Filter [{names}] /DP [{generic_params}]"),
            true,
        ),
        (
            format!("/F [/Crypt] /Filter [{names}] /DP [{generic_params}]"),
            false,
        ),
        (
            format!(
                "/Filter [/Unknown /Crypt /Unknown {names}] /DP [null null null {generic_params}]"
            ),
            true,
        ),
        (
            format!("/Filter [{names} 17 /LZW] /DP [{generic_params} null {fast}]"),
            true,
        ),
        (
            format!("/Filter [{names}] /DP [{fast_params} {generic}]"),
            false,
        ),
    ];
    if prefix == 0 {
        variants.extend([
            (format!("/F {head} /Filter /RL /DP {generic}"), true),
            (format!("/F /Unknown /Filter {head} /DP {generic}"), false),
            (
                format!("/Filter {head} /DP {fast} /DecodeParms {generic}"),
                false,
            ),
            (
                format!("/Filter {head} /DP null /DecodeParms {generic}"),
                true,
            ),
            (
                format!("/Filter {head} /DP [{fast}] /DecodeParms {generic}"),
                true,
            ),
        ]);
    }
    variants
}

#[test]
fn typed_priority_unknown_names_and_array_positions_choose_the_same_params() {
    for compression in COMPRESSIONS {
        let compressed = compression.encode(b"");
        for prefix in 0..3 {
            let data = wrapped(&compressed, prefix);
            for (filter, generic) in typed_variants(compression, prefix) {
                let source = cases::document(0, &filter, &data, false, 1);
                let baseline = cases::document(
                    0,
                    &filter.replace("/Predictor 12", "/Predictor 1"),
                    &data,
                    false,
                    1,
                );
                let actual = run(&source, Limits::default(), false);
                assert!(actual.0.is_ok());
                for kind in [Kind::DecodedBytes, Kind::Work] {
                    let limit = if kind == Kind::DecodedBytes {
                        256 * 1024
                    } else {
                        64 * 1024
                    };
                    assert!(
                        run(
                            &baseline,
                            Limits::default().with(kind, Limit::Finite(limit)),
                            false
                        )
                        .0
                        .is_ok()
                    );
                    if generic {
                        let (exceeded, budget) = stopped(&source, kind, limit);
                        assert!(
                            exceeded.requested - budget.used(kind) >= LARGE.row() as u64,
                            "{compression:?}, {prefix}, {filter}"
                        );
                    } else {
                        assert!(
                            run(
                                &source,
                                Limits::default().with(kind, Limit::Finite(limit)),
                                false
                            )
                            .0
                            .is_ok(),
                            "{compression:?}, {prefix}, {filter}"
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
fn referenced_document(filter: &str, data: &[u8], definitions: &[Vec<u8>]) -> Vec<u8> {
    let mut builder = common::Builder::default();
    builder.object("<< /Type /Catalog /Pages 2 0 R >>");
    builder.object("<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    builder.object("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >>");
    builder.stream(filter, data);
    for definition in definitions {
        builder.object(definition);
    }
    builder.bytes(1)
}

#[test]
fn indirect_filters_parameter_arrays_dictionaries_and_fields_keep_reservations() {
    for compression in COMPRESSIONS {
        let compressed = compression.encode(b"");
        let generic = LARGE.params(compression.early());
        let head = compression.name();
        let fields = format!(
            "<< /Predictor 9 0 R /Colors 6 0 R /BitsPerComponent 7 0 R /Columns 8 0 R /EarlyChange {} >>",
            u8::from(compression.early())
        );
        let variants = [
            (
                "/F 5 0 R /DP 6 0 R".into(),
                vec![head.as_bytes().to_vec(), generic.as_bytes().to_vec()],
                LARGE.row(),
            ),
            (
                "/Filter 5 0 R /DecodeParms 6 0 R".into(),
                vec![
                    format!("[/Crypt {head}]").into_bytes(),
                    b"[null 7 0 R]".to_vec(),
                    generic.as_bytes().to_vec(),
                ],
                LARGE.row(),
            ),
            (
                format!("/Filter [{head}] /DP [5 0 R]"),
                vec![generic.as_bytes().to_vec()],
                LARGE.row(),
            ),
            (
                format!("/Filter {head} /DP 5 0 R"),
                vec![
                    fields.into_bytes(),
                    b"5".to_vec(),
                    b"8".to_vec(),
                    b"50000".to_vec(),
                    b"12".to_vec(),
                ],
                250_000,
            ),
        ];
        for (index, (filter, definitions, row)) in variants.into_iter().enumerate() {
            let source = referenced_document(&filter, &compressed, &definitions);
            let actual = run(&source, Limits::default(), false);
            if index < 2 && matches!(compression, Compression::Lzw(_)) {
                // 現ガードは、間接のフィルターの候補に LZW の名前がある場合を断る。
                assert!(matches!(actual.0, Err(Error::Unsupported)));
                assert_eq!(common::codes(&actual.2), [("pdf.document.unsupported", 1)]);
                continue;
            }
            assert!(
                actual.0.is_ok(),
                "{compression:?}, {filter}: {:?}",
                actual.0
            );
            let inactive_filter = filter.replace("/Predictor 12", "/Predictor 1");
            let inactive_definitions = definitions
                .iter()
                .map(|value| {
                    if value == b"12" {
                        b"1".to_vec()
                    } else {
                        core::str::from_utf8(value)
                            .unwrap()
                            .replace("/Predictor 12", "/Predictor 1")
                            .into_bytes()
                    }
                })
                .collect::<Vec<_>>();
            let baseline =
                referenced_document(&inactive_filter, &compressed, &inactive_definitions);
            let baseline_result = run(&baseline, Limits::default(), false);
            assert!(baseline_result.0.is_ok());
            for kind in [Kind::DecodedBytes, Kind::Work] {
                let limit = if kind == Kind::DecodedBytes {
                    256 * 1024
                } else {
                    64 * 1024
                }
                .max(baseline_result.1.used(kind));
                assert!(
                    run(
                        &baseline,
                        Limits::default().with(kind, Limit::Finite(limit)),
                        false
                    )
                    .0
                    .is_ok(),
                    "baseline: {compression:?}, {filter}, {kind:?}"
                );
                let (exceeded, budget) = stopped(&source, kind, limit);
                assert!(exceeded.requested - budget.used(kind) >= row as u64);
            }
            exact_and_one_less(&source);
        }
        let source = referenced_document(&format!("/Filter {head} /DP 99 0 R"), &compressed, &[]);
        assert!(
            run(
                &source,
                Limits::default().with(Kind::DecodedBytes, Limit::Finite(256 * 1024)),
                false
            )
            .0
            .is_ok()
        );
    }
}

#[test]
fn empty_and_short_data_still_reserve_large_generic_rows() {
    for predictor in [
        LARGE,
        Predictor {
            number: 12,
            colors: 5,
            bits: 16,
            columns: 104_857,
        },
    ] {
        for plain in [&b""[..], &b"\0"[..]] {
            for compression in COMPRESSIONS {
                let compressed = compression.encode(plain);
                for prefix in 0..3 {
                    let filter = compression.filter(prefix, true, predictor);
                    let data = wrapped(&compressed, prefix);
                    for case in 0..cases::COUNT {
                        let source = cases::document(case, &filter, &data, false, 1);
                        let inactive = Predictor {
                            number: 1,
                            ..predictor
                        };
                        let baseline = cases::document(
                            case,
                            &compression.filter(prefix, true, inactive),
                            &data,
                            false,
                            1,
                        );
                        for kind in [Kind::DecodedBytes, Kind::Work] {
                            let limit = if kind == Kind::DecodedBytes {
                                256 * 1024
                            } else {
                                64 * 1024
                            };
                            assert!(
                                run(
                                    &baseline,
                                    Limits::default().with(kind, Limit::Finite(limit)),
                                    false
                                )
                                .0
                                .is_ok(),
                                "baseline: {case}, {compression:?}, {prefix}, {predictor:?}, {kind:?}"
                            );
                            let (exceeded, budget) = stopped(&source, kind, limit);
                            assert!(
                                exceeded.requested - budget.used(kind) >= predictor.row() as u64,
                                "{case}, {compression:?}, {prefix}, {predictor:?}, {kind:?}"
                            );
                            assert!(
                                exceeded.requested - budget.used(kind)
                                    <= predictor.row() as u64 + 16 * 1024
                            );
                        }
                    }
                }
            }
        }
    }
    // PNG はビット数より先に 8 ビットの群へ変換する。大きい行でも専用処理は配列を作らない。
    for predictor in [
        Predictor {
            number: 12,
            colors: 4,
            bits: 8,
            columns: 262_144,
        },
        Predictor {
            number: 12,
            colors: 3,
            bits: 16,
            columns: 174_762,
        },
        Predictor {
            number: 12,
            colors: 3,
            bits: 4,
            columns: 1,
        },
    ] {
        for compression in COMPRESSIONS {
            let compressed = compression.encode(b"\0");
            let filter = compression.filter(0, false, predictor);
            let source = cases::document(0, &filter, &compressed, false, 1);
            for kind in [Kind::DecodedBytes, Kind::Work] {
                let limit = if kind == Kind::DecodedBytes {
                    256 * 1024
                } else {
                    64 * 1024
                };
                assert!(
                    run(
                        &source,
                        Limits::default().with(kind, Limit::Finite(limit)),
                        false
                    )
                    .0
                    .is_ok()
                );
            }
        }
    }
}

#[test]
fn failures_and_redecoding_keep_predictor_costs() {
    for (predictor, plain) in [
        (
            Predictor {
                number: 3,
                colors: 5,
                bits: 8,
                columns: 1,
            },
            vec![0; 1024],
        ),
        (GENERIC[2], vec![0]),
    ] {
        for compression in COMPRESSIONS {
            let compressed = compression.encode(&plain);
            for prefix in 0..3 {
                let filter = compression.filter(prefix, true, predictor);
                let data = wrapped(&compressed, prefix);
                assert_eq!(decoded(&filter, &data).is_none(), predictor.number == 3);
                for case in [1, 2] {
                    let once = cases::document(case, &filter, &data, false, 1);
                    let twice = cases::document(case, &filter, &data, false, 2);
                    let first = run(&once, Limits::default(), false);
                    let second = run(&twice, Limits::default(), false);
                    assert!(first.0.is_ok() && second.0.is_ok());
                    assert!(second.1.used(Kind::DecodedBytes) > first.1.used(Kind::DecodedBytes));
                    assert!(second.1.used(Kind::Work) > first.1.used(Kind::Work));
                    for kind in [Kind::DecodedBytes, Kind::Work] {
                        stopped(&twice, kind, first.1.used(kind));
                    }
                    exact_and_one_less(&twice);
                }
            }
        }
    }
}

#[test]
fn exact_limits_and_cancellation_reproduce_results_at_every_entry() {
    let plain = vec![0; 40];
    for compression in COMPRESSIONS {
        let compressed = compression.encode(&plain);
        for prefix in 0..3 {
            let filter = compression.filter(prefix, true, GENERIC[0]);
            let data = wrapped(&compressed, prefix);
            for case in 0..cases::COUNT {
                let source = cases::document(case, &filter, &data, false, 2);
                exact_and_one_less(&source);
                let actual = run(&source, Limits::default(), true);
                assert!(matches!(actual.0, Err(Error::Stopped(Stop::Cancelled))));
                assert_eq!(actual.1.used(Kind::DecodedBytes), 0);
                assert_eq!(actual.1.used(Kind::Work), 0);
                assert_eq!(common::codes(&actual.2), [("call.request.cancelled", 1)]);
            }
        }
    }
}

#[test]
fn unsafe_parameters_keep_the_existing_guard_and_inactive_params_stay_inactive() {
    let compressed = Compression::Flate.encode(b"");
    for params in [
        "<< /Predictor 2 /Columns 0 >>",
        "<< /Predictor 2 /Colors 0 >>",
        "<< /Predictor 12 /Colors 16 /BitsPerComponent 16 >>",
        "<< /Predictor 12 /Colors 4 /BitsPerComponent 16 /Columns 131073 >>",
        "<< /Predictor 12 /Columns 4294967296 >>",
    ] {
        let filter = format!("/Filter /Fl /DP {params}");
        let actual = run(
            &cases::document(0, &filter, &compressed, false, 1),
            Limits::default(),
            false,
        );
        assert!(matches!(actual.0, Err(Error::Unsupported)));
        assert_eq!(common::codes(&actual.2), [("pdf.document.unsupported", 1)]);
    }
    for params in [
        "<< /Predictor 1 /Colors 0 /BitsPerComponent 0 /Columns 0 >>",
        "<< /Predictor 300 /Colors 0 /Columns 0 >>",
        "<< /Predictor 12 /Colors 5 /BitsPerComponent 3 /Columns 209715 >>",
    ] {
        let filter = format!("/Filter /Fl /DP {params}");
        let source = cases::document(0, &filter, &compressed, false, 1);
        for kind in [Kind::DecodedBytes, Kind::Work] {
            assert!(
                run(
                    &source,
                    Limits::default().with(kind, Limit::Finite(32 * 1024)),
                    false
                )
                .0
                .is_ok()
            );
        }
    }
}

#[test]
fn predictor_reservations_use_raw_output_instead_of_dictionary_or_capacity_costs() {
    for length in [0, 1, 3, 40] {
        let plain = vec![0; length];
        for compression in [
            Compression::Flate,
            Compression::Raw,
            Compression::Lzw(false),
            Compression::Lzw(true),
        ] {
            let compressed = compression.encode(&plain);
            for prefix in 0..3 {
                let data = wrapped(&compressed, prefix);
                let inactive = Predictor { number: 1, ..LARGE };
                let base_filter = compression.filter(prefix, true, inactive);
                let base_source = cases::document(0, &base_filter, &data, false, 1);
                let baseline = run(&base_source, Limits::default(), false);
                let filter = compression.filter(prefix, true, LARGE);
                let source = cases::document(0, &filter, &data, false, 1);
                let actual = run(&source, Limits::default(), false);
                assert!(baseline.0.is_ok() && actual.0.is_ok());
                assert_eq!(
                    shapes(actual.0.as_ref().unwrap()),
                    shapes(baseline.0.as_ref().unwrap())
                );
                for kind in [Kind::DecodedBytes, Kind::Work] {
                    let extra = actual.1.used(kind) - baseline.1.used(kind);
                    assert!(extra >= LARGE.row() as u64);
                    let per_byte = if kind == Kind::DecodedBytes { 512 } else { 256 };
                    assert!(
                        extra <= LARGE.row() as u64 + length as u64 * per_byte + 16 * 1024,
                        "{length}, {compression:?}, {prefix}, {kind:?}"
                    );
                }
            }
        }
    }
}
