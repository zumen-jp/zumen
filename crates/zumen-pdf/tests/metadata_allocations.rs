//! 展開引数の表・名前・Object を、解析と読み直しの前に予約する（#776）。

#![cfg(feature = "drawing")]

use std::collections::BTreeMap;
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

type Outcome = (Result<Drawing2D, Error>, Budget, Vec<Diagnostic>);

#[cfg(test)]
fn ample() -> Limits {
    Limits::default()
        .with(Kind::Work, Limit::Unlimited)
        .with(Kind::DecodedBytes, Limit::Unlimited)
}

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
fn shapes(drawing: &Drawing2D) -> Vec<String> {
    drawing
        .sheets
        .iter()
        .flat_map(|s| &s.elements)
        .map(|e| format!("{:?} {:?}", e.shape, e.style))
        .collect()
}

#[cfg(test)]
fn stopped(source: &[u8], kind: Kind, limit: u64) -> (zumen_base::Exceeded, Budget) {
    let limits = ample().with(kind, Limit::Finite(limit));
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
fn exact_and_one_less(source: &[u8]) {
    let expected = run(source, ample(), false);
    assert!(expected.0.is_ok());
    for kind in [Kind::DecodedBytes, Kind::Work] {
        let used = expected.1.used(kind);
        assert!(used > 0);
        let actual = run(source, ample().with(kind, Limit::Finite(used)), false);
        assert_eq!(format!("{:?}", actual.0), format!("{:?}", expected.0));
        assert_eq!(actual.2, expected.2);
        assert_eq!(actual.1.used(kind), used);
        stopped(source, kind, used - 1);
    }
}

/// 同じ形を返す小さい引数の実測値と、増やした引数の実測値の間で止める。
#[cfg(test)]
fn added_cost(small: &[u8], large: &[u8], minimum_bytes: u64) {
    let baseline = run(small, ample(), false);
    let actual = run(large, ample(), false);
    assert!(baseline.0.is_ok() && actual.0.is_ok());
    assert_eq!(
        shapes(actual.0.as_ref().unwrap()),
        shapes(baseline.0.as_ref().unwrap())
    );
    assert_eq!(common::codes(&actual.2), common::codes(&baseline.2));
    assert!(
        actual.1.used(Kind::DecodedBytes) >= baseline.1.used(Kind::DecodedBytes) + minimum_bytes
    );
    for kind in [Kind::DecodedBytes, Kind::Work] {
        let low = baseline.1.used(kind);
        let high = actual.1.used(kind);
        assert!(high > low);
        let limit = low + (high - low) / 2;
        let (exceeded, budget) = stopped(large, kind, limit);
        assert!(exceeded.requested > budget.used(kind));
        // 実際に拒んだ予約要求の直前まで上げても、同じ予約の前で止まる。
        let (near, before) = stopped(large, kind, exceeded.requested - 1);
        assert_eq!(near.requested, exceeded.requested);
        assert_eq!(before.used(kind), budget.used(kind));
    }
    assert_eq!(
        format!("{actual:?}"),
        format!("{:?}", run(large, ample(), false))
    );
}

#[cfg(test)]
fn keys(count: usize) -> String {
    (0..count).map(|i| format!("/K{i} 0 ")).collect()
}

#[cfg(test)]
fn unknowns(count: usize, params: &str) -> String {
    format!(
        "/Filter [{}] /DP [{}]",
        "/Unknown ".repeat(count),
        format!("{params} ").repeat(count)
    )
}

#[cfg(test)]
fn flate(plain: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(plain).unwrap();
    encoder.finish().unwrap()
}

#[cfg(test)]
fn literal_run_length(plain: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for chunk in plain.chunks(128) {
        out.push((chunk.len() - 1) as u8);
        out.extend(chunk);
    }
    out.push(128);
    out
}

#[cfg(test)]
fn decoded(filter: &str, data: &[u8]) -> Option<Vec<u8>> {
    let source = common::stream(filter, data);
    let stream = Reader::new(&source)
        .read_with_context::<Stream<'_>>(&ReaderContext::dummy())
        .unwrap();
    stream.decoded_data().ok().map(|data| data.into_owned())
}

#[test]
fn unknown_filters_and_empty_dictionaries_are_reserved_at_every_entry() {
    assert_eq!(cases::COUNT, 22);
    let small = format!("/Filter [{}]", "/Unknown ".repeat(64));
    let large = unknowns(64, "<<>>");
    assert!("<<>> ".repeat(64).len() < 1024);
    for case in 0..cases::COUNT {
        let baseline = cases::document(case, &small, b"", false, 1);
        let source = cases::document(case, &large, b"", false, 1);
        // フィルターはすべて未知、出力は空。Stage Vec と出力容量の増加では説明できない。
        added_cost(&baseline, &source, 64 * 64);
    }
}

#[test]
fn empty_scalar_dictionaries_and_table_capacity_boundaries_have_a_cost() {
    let data = flate(b"");
    let absent = cases::document(0, "/Filter /Fl", &data, false, 1);
    let empty = cases::document(0, "/Filter /Fl /DP <<>>", &data, false, 1);
    added_cost(&absent, &empty, 128);
    for count in [1, 7, 8, 9, 14, 15, 28, 29, 56, 57] {
        let filter = format!("/Filter /Fl /DP << {} >>", keys(count));
        let source = cases::document(0, &filter, &data, false, 1);
        added_cost(&empty, &source, 1);
        exact_and_one_less(&source);
    }
}

#[test]
fn names_strings_and_other_array_objects_are_counted_even_for_unknown_filters() {
    let baseline = cases::document(0, &unknowns(16, "null"), b"", false, 1);
    let values = [
        format!("<{}>", "41".repeat(400)),
        format!("({})", r"\101".repeat(200)),
        format!("/{}", "#41".repeat(200)),
        format!("<< {} >>", keys(57)),
        "[17 true null <<>>]".into(),
    ];
    for value in values {
        let params = format!("{value} {}", "null ".repeat(15));
        assert!(params.len() < 1024);
        let filter = format!("/Filter [{}] /DP [{params}]", "/Unknown ".repeat(16));
        let source = cases::document(0, &filter, b"", false, 1);
        added_cost(&baseline, &source, 1);
        exact_and_one_less(&source);
    }
    for length in [22, 23, 24, 31, 32, 47, 48] {
        let short = cases::document(0, "/Filter /Unknown", b"", false, 1);
        let filter = format!("/Filter /{}", "#41".repeat(length));
        let source = cases::document(0, &filter, b"", false, 1);
        added_cost(&short, &source, 1);
    }
}

#[test]
fn failed_scalar_gets_and_typed_parameter_fallbacks_keep_values_and_costs() {
    let plain = b"0 0 m 10 10 l S";
    let data = flate(plain);
    let params = format!("<< {} >>", keys(28));
    let baseline = cases::document(
        0,
        &format!("/Filter /Fl /DecodeParms {params}"),
        &data,
        false,
        1,
    );
    for wrong in ["null", "[]", "(text)", "<4142>", "17", "/Name", "false"] {
        let filter = format!("/Filter /Fl /DP {wrong} /DecodeParms {params}");
        let source = cases::document(0, &filter, &data, false, 1);
        added_cost(&baseline, &source, 128);
    }
    let variants = [
        format!(
            "/Filter [/Unknown /Crypt /Fl] /DP [({}) <<>> {params}]",
            r"\101".repeat(40)
        ),
        format!("/F /Unknown /Filter [/Fl] /DP null /DecodeParms [{params}]"),
        format!("/Filter [/Fl] /DP <<>> /DecodeParms [{params}]"),
        format!("/Filter [/Fl] /DP [17] /DecodeParms [{params}]"),
        format!("/Filter [/Fl 17 /RL] /DP [{params} null <<>>]"),
        format!("/Filter /Fl /DP [{params}] /DecodeParms <<>>"),
    ];
    for filter in variants {
        let expected = decoded(&filter, &data).unwrap();
        assert_eq!(expected, plain);
        let source = cases::document(0, &filter, &data, false, 1);
        let actual = run(&source, ample(), false);
        let unfiltered = common::lift(&cases::document(0, "", &expected, false, 1));
        assert_eq!(shapes(actual.0.as_ref().unwrap()), shapes(&unfiltered.0));
        assert_eq!(common::codes(&actual.2), common::codes(&unfiltered.1));
        exact_and_one_less(&source);
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

#[cfg(test)]
fn member_parameter_document(data: &[u8], params: &str) -> Vec<u8> {
    let mut source = b"%PDF-1.7\n".to_vec();
    let mut entries = BTreeMap::new();
    let packed = format!("30 0 {params}");
    let container = common::stream(
        "/Type /ObjStm /N 1 /First 5 /Filter /Fl",
        flate(packed.as_bytes()),
    );
    let objects = [
        (1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()),
        (2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec()),
        (
            3,
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >>".to_vec(),
        ),
        (4, common::stream("/Filter /Fl /DP 30 0 R", data)),
        (5, container),
    ];
    for (number, body) in objects {
        entries.insert(number, (1u8, source.len() as u32));
        source.extend(format!("{number} 0 obj\n").as_bytes());
        source.extend(body);
        source.extend(b"\nendobj\n");
    }
    entries.insert(30, (2, 5));
    let xref = source.len();
    entries.insert(99, (1, xref as u32));
    let mut index = String::new();
    let mut table = Vec::new();
    for (number, (kind, field)) in entries {
        index.push_str(&format!("{number} 1 "));
        table.push(kind);
        table.extend(field.to_be_bytes());
        table.extend([0, 0]);
    }
    source.extend(b"99 0 obj\n");
    source.extend(common::stream(
        &format!("/Type /XRef /Size 100 /W [1 4 2] /Index [{index}] /Root 1 0 R"),
        table,
    ));
    source.extend(format!("\nendobj\nstartxref\n{xref}\n%%EOF\n").as_bytes());
    assert!(packed.len() > source.len());
    source
}

#[test]
fn parameter_references_and_compressed_member_context_are_reserved_before_resolution() {
    let plain = b"0 0 m 10 10 l S";
    let data = flate(plain);
    let params = format!("<< {} >>", keys(57));
    let direct = referenced_document(&format!("/Filter /Fl /DP {params}"), &data, &[]);
    for (filter, value) in [
        ("/Filter /Fl /DP 5 0 R", params.clone()),
        ("/Filter [/Fl] /DP [5 0 R]", params.clone()),
        (
            "/Filter /Fl /DP 5 0 R /DecodeParms <<>>",
            format!("<{}>", "41".repeat(240)),
        ),
        (
            "/Filter [/Fl] /DP [5 0 R]",
            format!("({})", r"\101".repeat(120)),
        ),
    ] {
        let source = referenced_document(filter, &data, &[value.into_bytes()]);
        let actual = run(&source, ample(), false);
        let expected = run(&direct, ample(), false);
        assert!(actual.0.is_ok());
        assert_eq!(
            shapes(actual.0.as_ref().unwrap()),
            shapes(expected.0.as_ref().unwrap())
        );
        assert_eq!(common::codes(&actual.2), common::codes(&expected.2));
        exact_and_one_less(&source);
    }
    let large_params = format!("<< {} >>", keys(512));
    let member = member_parameter_document(&data, &large_params);
    let actual = run(&member, ample(), false);
    let expected = run(&direct, ample(), false);
    assert!(actual.0.is_ok());
    assert_eq!(
        shapes(actual.0.as_ref().unwrap()),
        shapes(expected.0.as_ref().unwrap())
    );
    assert_eq!(common::codes(&actual.2), common::codes(&expected.2));
    exact_and_one_less(&member);
}

#[test]
fn one_parameter_reference_does_not_resolve_its_target_once_per_unknown_name() {
    let mut first = None;
    for count in [1, 16, 64] {
        let filter = format!("/Filter [{}] /DP [5 0 R]", "/Unknown ".repeat(count));
        let small = referenced_document(&filter, b"", &[b"<<>>".to_vec()]);
        let large =
            referenced_document(&filter, b"", &[format!("<< {} >>", keys(57)).into_bytes()]);
        let baseline = run(&small, ample(), false);
        let actual = run(&large, ample(), false);
        assert!(baseline.0.is_ok() && actual.0.is_ok());
        let extra = [Kind::DecodedBytes, Kind::Work]
            .map(|kind| actual.1.used(kind) - baseline.1.used(kind));
        if let Some(one) = first {
            for (extra, one) in extra.into_iter().zip(one) {
                // Object の局所的な試行回数とは別に、消費した参照 1 個の対象を数える。
                // ファイル全体の保守的な上界の差は許し、16・64 倍の繰り返しは許さない。
                assert!(extra < one * 4);
            }
        } else {
            assert!(extra.iter().all(|&n| n > 0));
            first = Some(extra);
        }
        assert_eq!(
            shapes(actual.0.as_ref().unwrap()),
            shapes(baseline.0.as_ref().unwrap())
        );
    }
}

#[cfg(test)]
fn encoded_variants(plain: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut variants = Vec::new();
    for mode in 0..4 {
        let (names, data, params) = match mode {
            0 => ("/Fl", flate(plain), format!("<< {} >>", keys(14))),
            1 | 2 => (
                "/LZW",
                lzw::encode(plain, mode == 2),
                format!("<< /EarlyChange {} {} >>", u8::from(mode == 2), keys(14)),
            ),
            _ => (
                "/Fl /RL",
                flate(&literal_run_length(plain)),
                format!("<< {} >> <<>>", keys(14)),
            ),
        };
        for prefix in 0..3 {
            if prefix == 0 && mode != 3 {
                variants.push((format!("/Filter {names} /DP {params}"), data.clone()));
            } else {
                let (ascii_name, encoded) = match prefix {
                    0 => ("", data.clone()),
                    1 => ("/AHx ", ascii::encode(&data, false)),
                    _ => ("/A85 ", ascii::encode(&data, true)),
                };
                let ascii_params = if prefix == 0 { "" } else { "<<>> " };
                variants.push((
                    format!(
                        "/Filter [/Crypt {ascii_name}{names}] /DP [<<>> {ascii_params}{params}]"
                    ),
                    encoded,
                ));
            }
        }
    }
    variants
}

#[test]
fn ample_metadata_budgets_keep_values_diagnostics_and_ids_for_all_entries_and_chains() {
    for case in 0..cases::COUNT {
        let plain = cases::plain(case);
        let baseline = common::lift(&cases::document(case, "", &plain, false, 2));
        for (filter, data) in encoded_variants(&plain) {
            let source = cases::document(case, &filter, &data, false, 2);
            let actual = run(&source, ample(), false);
            assert_eq!(shapes(actual.0.as_ref().unwrap()), shapes(&baseline.0));
            assert_eq!(common::codes(&actual.2), common::codes(&baseline.1));
            assert_eq!(
                format!("{actual:?}"),
                format!("{:?}", run(&source, ample(), false))
            );
        }
    }
}

#[test]
fn empty_and_failed_redecodes_keep_metadata_costs_in_content_arrays_and_forms() {
    for (small, large, data) in [
        (
            format!("/Filter [{}]", "/Unknown ".repeat(64)),
            unknowns(64, "<<>>"),
            Vec::new(),
        ),
        (
            "/Filter [/RL /Crypt] /DP [null null]".into(),
            format!("/Filter [/RL /Crypt] /DP [<< {} >> <<>>]", keys(28)),
            vec![128],
        ),
    ] {
        for case in [1, 2] {
            let base_once = run(
                &cases::document(case, &small, &data, false, 1),
                ample(),
                false,
            );
            let base_twice = run(
                &cases::document(case, &small, &data, false, 2),
                ample(),
                false,
            );
            let once = run(
                &cases::document(case, &large, &data, false, 1),
                ample(),
                false,
            );
            let source = cases::document(case, &large, &data, false, 2);
            let twice = run(&source, ample(), false);
            assert!(once.0.is_ok() && twice.0.is_ok());
            for kind in [Kind::DecodedBytes, Kind::Work] {
                let baseline = base_twice.1.used(kind) - base_once.1.used(kind);
                let added = twice.1.used(kind) - once.1.used(kind);
                assert!(added > baseline, "{case}, {kind:?}");
                stopped(&source, kind, once.1.used(kind));
            }
            exact_and_one_less(&source);
        }
    }
}

#[test]
fn metadata_capacity_boundaries_and_cancellation_are_repeatable() {
    for count in [1, 2, 3, 4, 8, 16, 32, 64] {
        let filter = unknowns(count, "<<>>");
        for case in [0, 1, 2, 3, 9, 21] {
            let source = cases::document(case, &filter, b"", false, 2);
            exact_and_one_less(&source);
            let actual = run(&source, ample(), true);
            assert!(matches!(actual.0, Err(Error::Stopped(Stop::Cancelled))));
            assert_eq!(actual.1.used(Kind::DecodedBytes), 0);
            assert_eq!(actual.1.used(Kind::Work), 0);
            assert_eq!(common::codes(&actual.2), [("call.request.cancelled", 1)]);
        }
    }
}

#[test]
fn existing_filter_and_reference_guards_remain_in_force() {
    for (case, filter) in [
        (0, unknowns(65, "<<>>")),
        (0, "/Filter [/RL /RL] /DP [5 0 R 5 0 R]".into()),
        (3, "/Filter /Fl /DP [5 0 R]".into()),
    ] {
        let source = cases::document(case, &filter, b"", false, 1);
        let actual = run(&source, ample(), false);
        assert!(matches!(actual.0, Err(Error::Unsupported)));
        assert_eq!(common::codes(&actual.2), [("pdf.document.unsupported", 1)]);
    }
    let source = referenced_document("/Filter [/RL] /DP [5 0 R]", &[128], &[b"<<>>".to_vec()]);
    assert!(run(&source, ample(), false).0.is_ok());
}

#[cfg(test)]
fn encrypted_page(params: &str) -> Vec<u8> {
    let mut builder = common::Builder::default();
    builder.object("<< /Type /Catalog /Pages 2 0 R >>");
    builder.object("<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    builder.object("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >>");
    let data = [
        0x39, 0x18, 0x09, 0xd8, 0x8c, 0x9c, 0x60, 0x9c, 0x5e, 0xa9, 0x88, 0x17, 0x1d, 0x6f, 0x8c,
        0x64, 0x28, 0xc2, 0xfb, 0xa7, 0x48, 0x9a, 0x1d, 0xe8, 0x38, 0x95, 0x38, 0x7c, 0xe5, 0x59,
        0x6d, 0x9a, 0x67, 0xa7, 0xca, 0xfa, 0xe2, 0x63, 0x37, 0xc0, 0x9c,
    ];
    builder.stream(&format!("/Filter [/AHx /Fl] /DP [{params}]"), data);
    builder.object("<< /Filter /Standard /V 1 /R 2 /P -4 /O <2055c756c72e1ad702608e8196acad447ad32d17cff583235f6dd15fed7dab67> /U <92c034164f1ef00c9edf3ba7595c16b11c50ff99a474b82f495eabb623e302f8> >>");
    builder.trailer = "/Encrypt 5 0 R /ID [<30303030303030303030303030303030> <30303030303030303030303030303030>]".into();
    builder.bytes(1)
}

#[test]
fn encrypted_metadata_keeps_values_and_reserves_additional_reads() {
    // 既存 ASCII の R2 合成入力と同じ暗号文。引数中の文字列は辞書へ変換せず捨てる。
    let small = encrypted_page("null <<>>");
    let params = format!("({}) << {} >>", r"\101".repeat(80), keys(14));
    let source = encrypted_page(&params);
    added_cost(&small, &source, 1);
    let actual = run(&source, ample(), false);
    assert_eq!(actual.0.unwrap().sheets[0].elements.len(), 1);
    exact_and_one_less(&source);
}
