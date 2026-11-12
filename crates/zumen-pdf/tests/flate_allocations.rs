//! Flate の固定のデコーダーとバッファーの確保を、作る前に数える（#779）。

#![cfg(feature = "drawing")]
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::io::Write;

use zumen_base::{
    Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, Stop, codes,
};
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
#[allow(dead_code)]
#[path = "common/lzw.rs"]
mod lzw;
#[path = "common/resources.rs"]
mod resources;

/// 解放を戻さない累積確保量を測る計量アロケーター。スレッドごとの局所状態に積むため、
/// 並列テストでも他スレッドの確保が混ざらない。
struct CountingAlloc;

thread_local! {
    static STATE: Cell<(bool, u64, u64)> = const { Cell::new((false, 0, 0)) };
}

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = STATE.try_with(|s| {
            let (on, bytes, calls) = s.get();
            if on {
                s.set((on, bytes.saturating_add(layout.size() as u64), calls + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let _ = STATE.try_with(|s| {
            let (on, bytes, calls) = s.get();
            if on {
                s.set((on, bytes.saturating_add(layout.size() as u64), calls + 1));
            }
        });
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let _ = STATE.try_with(|s| {
            let (on, bytes, calls) = s.get();
            if on {
                s.set((on, bytes.saturating_add(new_size as u64), calls + 1));
            }
        });
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

/// preflight の `bufread` デコーダーと `leniently_inflated` の `Decompress` の予約単位
const DECODER_BYTES: u64 = 49_152;
const DECODER_WORK: u64 = DECODER_BYTES.div_ceil(16);
/// hayro-syntax 本展開の `flate2::read` デコーダーの予約単位
const READER_BYTES: u64 = 81_920;

const LINE: &[u8] = b"0 0 m 10 10 l S";
// ISO 32000-2:2020 の 7.6 に従う Standard R2/RC4 と R4/AESV2 の合成辞書（decryption_allocations.rs と同じ空パスワード鍵）。
const RC4_DICT: &str = "<< /Filter /Standard /V 1 /R 2 /P -4 /O <2055c756c72e1ad702608e8196acad447ad32d17cff583235f6dd15fed7dab67> /U <92c034164f1ef00c9edf3ba7595c16b11c50ff99a474b82f495eabb623e302f8> >>";
const AES_DICT: &str = "<< /Filter /Standard /V 4 /R 4 /Length 128 /P -4 /O <36451bd39d753b7c1d10922c28e6665aa4f3353fb0348b536893e3b1db5c579b> /U <acf99d8614017dd73cbe2bc3c79b668c00000000000000000000000000000000> /EncryptMetadata true /StmF /StdCF /StrF /StdCF /CF << /StdCF << /CFM /AESV2 /Length 16 >> >> >>";
const MASK4: &str = "0e2030bbbfaf55ac6d99bd277e57b9511bf2c89378aa7cd909f60049d3695daa5690faced25007a6a2e844038f8fa960c5b8a298cdaa1c334229d50fa2ba2fcd56263a401271adba4c3cc853dd98b63b287fa73943bdfd521784d2ed15fb7623b992270f3c2a59b81b72ca9fa0cdd557d38a0512d1b763d9f8add1abf6fecb6bbe1a9ca8529f262134a8db396f9b602f5933250ba2e13f7b2c476407d0782cace24d1fc8695cdea2062d018416eee9fed264bbce0992c9d295d66fcd8bf45b0201515a9dfd6fee217e6a2939c47db985b7d50630b57e8db14a7b53506c86ac4e38fdf398cf036e99324147fc190ae11e";
const AES_FLATE: &str = "0405060708090a0b0c0d0e0f10111213870d7a396736892523f4600752838846a7c9d0ae30e8b1bd8f83d2000b75b59c";
const AES_AHX_FLATE: &str = "0405060708090a0b0c0d0e0f10111213e98f74943257b7034ccaac38f51af952e69f0ababc9a75975faae6299e1165c356fe6da7de95587a81fa2c9653f70741";

#[cfg(test)]
fn measure<T>(f: impl FnOnce() -> T) -> (T, u64, u64) {
    let _ = STATE.try_with(|s| s.set((true, 0, 0)));
    let out = f();
    let (_, bytes, calls) = STATE
        .try_with(|s| {
            let current = s.get();
            s.set((false, 0, 0));
            current
        })
        .unwrap_or((false, 0, 0));
    (out, bytes, calls)
}

#[cfg(test)]
fn zlib_encode(plain: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(plain).unwrap();
    encoder.finish().unwrap()
}

#[cfg(test)]
fn deflate_encode(plain: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(plain).unwrap();
    encoder.finish().unwrap()
}

#[cfg(test)]
fn unhex(text: &str) -> Vec<u8> {
    assert!(text.len().is_multiple_of(2));
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|s| u8::from_str_radix(core::str::from_utf8(s).unwrap(), 16).unwrap())
        .collect()
}

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
fn ample() -> Limits {
    Limits::default()
        .with(Kind::Work, Limit::Unlimited)
        .with(Kind::DecodedBytes, Limit::Unlimited)
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
fn shapes(drawing: &Drawing2D) -> Vec<String> {
    drawing
        .sheets
        .iter()
        .flat_map(|s| &s.elements)
        .map(|e| format!("{:?} {:?}", e.shape, e.style))
        .collect()
}

#[cfg(test)]
fn stopped(
    source: &[u8],
    kind: Kind,
    limit: u64,
) -> (zumen_base::Exceeded, Budget, Vec<Diagnostic>) {
    let limits = ample().with(kind, Limit::Finite(limit));
    let actual = run(source, limits, false);
    let Err(Error::Stopped(Stop::Exceeded(exceeded))) = &actual.0 else {
        panic!(
            "予算で止まらなかった: {kind:?} limit={limit}: {:?}",
            actual.0
        );
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
    assert_eq!(
        format!("{actual:?}"),
        format!("{:?}", run(source, limits, false))
    );
    (*exceeded, actual.1, actual.2)
}

#[cfg(test)]
fn exact_and_one_less(source: &[u8]) {
    let expected = run(source, ample(), false);
    assert!(expected.0.is_ok(), "{:?}", expected.0);
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

#[test]
fn stops_before_constructing_any_flate_decoder_on_zero_budgets_and_cancellation() {
    let compressed = zlib_encode(LINE);
    assert_eq!(cases::COUNT, 22);
    for case in 0..cases::COUNT {
        let source = cases::document(case, "/Filter /FlateDecode", &compressed, false, 1);
        let file = testkit::read::<Pdf>(&source).0.unwrap();

        // Work=0・DecodedBytes=0・開始前の中断では、47,552 B 以上のデコーダー確保が 1 度も起きない。
        // Work=0 と開始前の中断は最初の検査で Work=0・DecodedBytes=0 のまま止まり、
        // DecodedBytes=0 は walk::settle の最初の DecodedBytes 検査で DecodedBytes=0 のまま止まる。
        for (limits, cancelled, expect_zero_work) in [
            (ample().with(Kind::Work, Limit::Finite(0)), false, true),
            (
                ample().with(Kind::DecodedBytes, Limit::Finite(0)),
                false,
                false,
            ),
            (ample(), true, true),
        ] {
            let mut budget = Budget::new(limits);
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            if cancelled {
                cancel.cancel();
            }
            let (result, alloc_bytes, _) =
                measure(|| file.lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel)));
            assert!(result.is_err(), "case={case}");
            assert_eq!(budget.used(Kind::DecodedBytes), 0, "case={case}");
            if expect_zero_work {
                assert_eq!(budget.used(Kind::Work), 0, "case={case}");
            }
            assert!(
                alloc_bytes < 32 * 1024,
                "case={case}, cancelled={cancelled}: allocated {alloc_bytes} bytes before decoder check"
            );
        }

        // 引数解析までの予算を通し、最初の固定デコーダー予約（49,152 B）の手前で止めると、
        // Flate デコーダー初期化の確保（47,552 B）が起きずにその予約自身で止まる。
        // 非 Flate 文書の Pdf::new / settle / ページ辞書分と比べることで、デコーダー確保 0 を確かめる。
        let plain = cases::plain(case);
        let baseline_doc = cases::document(case, "", &plain, false, 1);
        let baseline_file = testkit::read::<Pdf>(&baseline_doc).0.unwrap();
        let (_, baseline_alloc, _) = measure(|| {
            let mut b = Budget::new(ample());
            let mut d = Diagnostics::new();
            let c = CancelToken::new();
            let _ = baseline_file.lift(&mut Cx::new(&mut b, &mut d, &c));
        });

        let mut found_decoder_gate = false;
        for limit in (512..DECODER_BYTES + 16 * 1024).step_by(512) {
            let mut budget = Budget::new(ample().with(Kind::DecodedBytes, Limit::Finite(limit)));
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let (result, alloc_bytes, _) =
                measure(|| file.lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel)));
            let Err(Error::Stopped(Stop::Exceeded(exceeded))) = result else {
                panic!("case={case}, limit={limit}");
            };
            if exceeded.requested - budget.used(Kind::DecodedBytes) == DECODER_BYTES {
                assert!(
                    alloc_bytes < baseline_alloc + 8 * 1024,
                    "case={case}, limit={limit}: allocated {alloc_bytes} bytes vs baseline {baseline_alloc} before decoder init"
                );
                found_decoder_gate = true;
                break;
            }
        }
        assert!(
            found_decoder_gate,
            "case={case}: 固定デコーダーの予約（49,152 B）に到達しなかった"
        );
    }
}

#[test]
fn cumulative_non_refunded_allocations_stay_within_reserved_decoded_bytes() {
    let good_zlib = zlib_encode(LINE);
    let good_raw = deflate_encode(LINE);
    let truncated_zlib = good_zlib[..good_zlib.len() - 5].to_vec();
    let mut bad_adler = good_zlib.clone();
    *bad_adler.last_mut().unwrap() ^= 1;
    let mut corrupt = good_zlib.clone();
    corrupt[0] = 0xff;
    corrupt[1] = 0xff;

    let stream_variants: Vec<(&str, &str, Vec<u8>)> = vec![
        ("valid_zlib", "/Filter /FlateDecode", good_zlib.clone()),
        ("valid_raw", "/Filter /FlateDecode", good_raw.clone()),
        ("truncated_zlib", "/Filter /FlateDecode", truncated_zlib),
        ("bad_adler", "/Filter /FlateDecode", bad_adler.clone()),
        ("corrupt_fallback", "/Filter /FlateDecode", corrupt),
        (
            "whitespace_empty",
            "/Filter /FlateDecode",
            b" \t\r\n".to_vec(),
        ),
        (
            "ahx_zlib",
            "/Filter [/AHx /Fl]",
            ascii::encode(&good_zlib, false),
        ),
        (
            "a85_raw",
            "/Filter [/A85 /Fl]",
            ascii::encode(&good_raw, true),
        ),
        (
            "chain_fl_rl",
            "/Filter [/Fl /RL]",
            zlib_encode(&literal_run_length(LINE)),
        ),
        (
            "chain_rl_fl",
            "/Filter [/RL /Fl]",
            literal_run_length(&good_zlib),
        ),
        ("chain_fallback_fl_rl", "/Filter [/Fl /RL]", {
            let mut d = zlib_encode(&literal_run_length(LINE));
            *d.last_mut().unwrap() ^= 1;
            d
        }),
    ];

    // ストリーム単体の before + decoded_data (+ leniently_inflated) の直接の累積確保比較は
    // inflate::tests で検査し、ここでは公開の Pdf::lift 経路でも非圧縮ベースラインに対する
    // Flate 導入分の解放を戻さない累積確保の増分が DecodedBytes 予約の増分内に収まることを確かめる。
    for case in [0, 1, 2, 3] {
        let plain = cases::plain(case);
        let base_doc = cases::document(case, "", &plain, false, 2);
        let base_file = testkit::read::<Pdf>(&base_doc).0.unwrap();
        let mut base_budget = Budget::new(ample());
        let mut base_diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let (base_res, base_alloc, _) =
            measure(|| base_file.lift(&mut Cx::new(&mut base_budget, &mut base_diag, &cancel)));
        assert!(base_res.is_ok(), "baseline case={case}: {base_res:?}");
        let base_reserved = base_budget.used(Kind::DecodedBytes);

        for (label, filter, data) in &stream_variants {
            if case == 3
                && (label.starts_with("corrupt")
                    || label.starts_with("truncated")
                    || label.starts_with("whitespace"))
            {
                continue;
            }
            let payload = if case == 3 {
                let obj_plain = cases::plain(3);
                match *label {
                    "valid_zlib" => zlib_encode(&obj_plain),
                    "valid_raw" => deflate_encode(&obj_plain),
                    "bad_adler" => {
                        let mut d = zlib_encode(&obj_plain);
                        *d.last_mut().unwrap() ^= 1;
                        d
                    }
                    "ahx_zlib" => ascii::encode(&zlib_encode(&obj_plain), false),
                    "a85_raw" => ascii::encode(&deflate_encode(&obj_plain), true),
                    "chain_fl_rl" => zlib_encode(&literal_run_length(&obj_plain)),
                    "chain_rl_fl" => literal_run_length(&zlib_encode(&obj_plain)),
                    "chain_fallback_fl_rl" => {
                        let mut d = zlib_encode(&literal_run_length(&obj_plain));
                        *d.last_mut().unwrap() ^= 1;
                        d
                    }
                    _ => data.clone(),
                }
            } else {
                data.clone()
            };
            let source = cases::document(case, filter, &payload, false, 2);
            let file = testkit::read::<Pdf>(&source).0.unwrap();
            let mut budget = Budget::new(ample());
            let mut diagnostics = Diagnostics::new();
            let (result, alloc_bytes, _) =
                measure(|| file.lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel)));
            assert!(result.is_ok(), "{label}, case={case}: {result:?}");
            let reserved = budget.used(Kind::DecodedBytes);
            let extra_alloc = alloc_bytes.saturating_sub(base_alloc);
            let extra_reserved = reserved.saturating_sub(base_reserved);
            assert!(
                extra_alloc <= extra_reserved,
                "{label}, case={case}: extra allocated {extra_alloc} ({alloc_bytes} - {base_alloc}) > extra reserved {extra_reserved} ({reserved} - {base_reserved})"
            );
        }
    }

    // 平文・RC4・AESV2 × 単独 Flate / ASCIIHex+Flate の公開 Pdf::lift でも、
    // 同じ暗号モードの非圧縮ストリームに対する Flate 分の累積確保増分が予約増分内に収まることを確かめる。
    let clear_plain = b"0 0 m 1 1 l S\n";
    let aes_plain = "0405060708090a0b0c0d0e0f10111213ebf71b0e76395b5ae09233a0b68f6615";
    let clear_flate = unhex("78da33503050c85530543054c85108e60200143e02b9");
    let clear_ahx_flate = unhex(
        "37386461333335303330353063383535333035343330353463383531303865363032303031343365303262393e",
    );
    let mask4 = unhex(MASK4);
    for mode in 0..3 {
        let build_mode_doc = |filter: &str, clear: &[u8], aes_hex: &str| {
            let stored: Vec<u8> = match mode {
                0 => clear.to_vec(),
                1 => clear.iter().zip(&mask4).map(|(&a, &b)| a ^ b).collect(),
                _ => unhex(aes_hex),
            };
            let mut builder = common::Builder::default();
            builder.object("<< /Type /Catalog /Pages 2 0 R >>");
            builder.object("<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
            builder
                .object("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >>");
            builder.stream(filter, &stored);
            builder.object(match mode {
                0 => "null",
                1 => RC4_DICT,
                _ => AES_DICT,
            });
            if mode != 0 {
                builder.trailer = "/Encrypt 5 0 R /ID [<30303030303030303030303030303030> <30303030303030303030303030303030>]".into();
            }
            builder.bytes(1)
        };

        let base_src = build_mode_doc("", clear_plain, aes_plain);
        let base_file = testkit::read::<Pdf>(&base_src).0.unwrap();
        let mut base_budget = Budget::new(ample());
        let mut base_diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let (base_res, base_alloc, _) =
            measure(|| base_file.lift(&mut Cx::new(&mut base_budget, &mut base_diag, &cancel)));
        assert!(base_res.is_ok(), "mode={mode} baseline: {base_res:?}");
        let base_reserved = base_budget.used(Kind::DecodedBytes);

        for (filter, clear, aes) in [
            ("/Filter /Fl", clear_flate.as_slice(), AES_FLATE),
            (
                "/Filter [/AHx /Fl] /DP [null null]",
                clear_ahx_flate.as_slice(),
                AES_AHX_FLATE,
            ),
        ] {
            let source = build_mode_doc(filter, clear, aes);
            let file = testkit::read::<Pdf>(&source).0.unwrap();
            let mut budget = Budget::new(ample());
            let mut diagnostics = Diagnostics::new();
            let (result, alloc_bytes, _) =
                measure(|| file.lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel)));
            assert!(result.is_ok(), "mode={mode}, {filter}: {result:?}");
            let reserved = budget.used(Kind::DecodedBytes);
            let extra_alloc = alloc_bytes.saturating_sub(base_alloc);
            let extra_reserved = reserved.saturating_sub(base_reserved);
            assert!(
                extra_alloc <= extra_reserved,
                "mode={mode}, {filter}: extra allocated {extra_alloc} > extra reserved {extra_reserved}"
            );
        }
    }
}

#[test]
fn separates_strict_attempts_fallback_and_diagnostic_reinflate_branches() {
    let valid_zlib = common::zlib(LINE);
    let valid_raw = valid_zlib[2..valid_zlib.len() - 4].to_vec();
    let mut bad_adler = valid_zlib.clone();
    *bad_adler.last_mut().unwrap() ^= 1;
    let truncated_zlib = valid_zlib[..valid_zlib.len() - 5].to_vec();

    // 1. ObjStm（leniently_inflated を通らない）では、正常な zlib は strict 1 回（49,152 + 81,920 B）、
    //    raw deflate は strict 2 回（2 * (49,152 + 81,920) B）を数え、寛容な表（>= 512 KiB）は数えない。
    let obj_plain = b"30 0 null";
    let obj_zlib = common::zlib(obj_plain);
    let obj_raw = obj_zlib[2..obj_zlib.len() - 4].to_vec();
    let (_, zlib_budget, _) = run(
        &cases::document(3, "/Filter /FlateDecode", &obj_zlib, false, 1),
        ample(),
        false,
    );
    let (_, raw_budget, _) = run(
        &cases::document(3, "/Filter /FlateDecode", &obj_raw, false, 1),
        ample(),
        false,
    );
    let diff = raw_budget.used(Kind::DecodedBytes) - zlib_budget.used(Kind::DecodedBytes);
    // raw deflate は preflight の DeflateDecoder（49,152 B）と hayro の DeflateDecoder（81,920 B）の
    // 2 回目の試行分だけ増え、512 KiB の fallback 表は含まない。
    assert!(
        (DECODER_BYTES + READER_BYTES - 32..DECODER_BYTES + READER_BYTES + 256).contains(&diff),
        "zlib={}, raw={}, diff={diff}",
        zlib_budget.used(Kind::DecodedBytes),
        raw_budget.used(Kind::DecodedBytes),
    );
    assert!(raw_budget.used(Kind::DecodedBytes) < 512 * 1024);

    // 2. 内容ストリームの診断（leniently_inflated）の成否と試行回数:
    //    - 正常な zlib: 1 回目の inflates_to(raw, true) で止まり、PDF_CONTENT_INVALID は出ない。
    //    - 正常な raw deflate: 2 回目の inflates_to(raw, false) で止まり、PDF_CONTENT_INVALID は出ない。
    //    - Adler-32 だけの損傷: 本展開は fallback まで進むが、leniently_inflated は 3 回目の
    //      inflates_to(&raw[2..], false) で長さ一致を確認し、PDF_CONTENT_INVALID は出さない。
    //    - 途中切断の zlib: 3 回とも StreamEnd に届かず、PDF_CONTENT_INVALID を 1 件出す。
    let (_, b_good, d_good) = run(
        &cases::document(0, "/Filter /FlateDecode", &valid_zlib, false, 1),
        ample(),
        false,
    );
    assert!(
        !d_good
            .iter()
            .any(|d| d.code() == codes::PDF_CONTENT_INVALID),
        "{d_good:?}"
    );

    let (_, b_raw, d_raw) = run(
        &cases::document(0, "/Filter /FlateDecode", &valid_raw, false, 1),
        ample(),
        false,
    );
    assert!(
        !d_raw.iter().any(|d| d.code() == codes::PDF_CONTENT_INVALID),
        "{d_raw:?}"
    );
    // 内容では raw deflate が leniently_inflated でもう 1 回多く Decompress（49,152 B）を作るため、
    // 差は 2 * DECODER_BYTES + READER_BYTES 付近になる。
    let content_diff = b_raw.used(Kind::DecodedBytes) - b_good.used(Kind::DecodedBytes);
    assert!(
        (2 * DECODER_BYTES + READER_BYTES - 32..2 * DECODER_BYTES + READER_BYTES + 256)
            .contains(&content_diff),
        "content_diff={content_diff}"
    );

    let (r_adler, b_adler, d_adler) = run(
        &cases::document(0, "/Filter /FlateDecode", &bad_adler, false, 1),
        ample(),
        false,
    );
    assert_eq!(
        shapes(r_adler.as_ref().unwrap()),
        shapes(&common::lift(&cases::document(0, "", LINE, false, 1)).0)
    );
    assert!(
        !d_adler
            .iter()
            .any(|d| d.code() == codes::PDF_CONTENT_INVALID),
        "{d_adler:?}"
    );
    assert!(b_adler.used(Kind::DecodedBytes) >= 512 * 1024);

    let (_, _, d_trunc) = run(
        &cases::document(0, "/Filter /FlateDecode", &truncated_zlib, false, 1),
        ample(),
        false,
    );
    assert_eq!(
        d_trunc
            .iter()
            .filter(|d| d.code() == codes::PDF_CONTENT_INVALID)
            .map(Diagnostic::count)
            .sum::<u64>(),
        1
    );

    // 3. 空白だけの空 Flate と、Predictor > 1 の Flate は leniently_inflated の Decompress を作らない。
    let (_, b_ws, d_ws) = run(
        &cases::document(0, "/Filter /FlateDecode", b"   ", false, 1),
        ample(),
        false,
    );
    assert!(!d_ws.iter().any(|d| d.code() == codes::PDF_CONTENT_INVALID));
    // 空白だけの長さ 3 B は fallback でも 3 B * 8 / 3 = 8 ブロックになるが、Decompress の 3 試行は足さない。
    exact_and_one_less(&cases::document(
        0,
        "/Filter /FlateDecode",
        b"   ",
        false,
        1,
    ));
    assert!(b_ws.used(Kind::DecodedBytes) > 0);

    let mut predicted_line = vec![0];
    predicted_line.extend_from_slice(LINE);
    let pred_zlib = zlib_encode(&predicted_line);
    let pred_filter = format!(
        "/Filter /FlateDecode /DecodeParms << /Predictor 12 /Columns {} >>",
        LINE.len()
    );
    let (_, b_pred, _) = run(
        &cases::document(0, &pred_filter, &pred_zlib, false, 1),
        ample(),
        false,
    );
    // Predictor > 1 は plain_flate が false を返すため、leniently_inflated の 49,152 B を予約しない。
    assert!(
        b_pred.used(Kind::DecodedBytes) + DECODER_BYTES / 2 < b_good.used(Kind::DecodedBytes),
        "pred={}, good={}",
        b_pred.used(Kind::DecodedBytes),
        b_good.used(Kind::DecodedBytes)
    );
}

#[test]
fn repeated_content_arrays_and_forms_and_exact_boundaries_across_all_entries() {
    let compressed = zlib_encode(LINE);
    let raw = deflate_encode(LINE);

    // 22 入口すべてで、十分な予算の図面・診断・ID を保ち、ちょうどの予算と 1 少ない境界・中断を再現する。
    for case in 0..cases::COUNT {
        let plain = cases::plain(case);
        let expected = common::lift(&cases::document(case, "", &plain, false, 2));
        let encoded = zlib_encode(&plain);
        for (prefix, data) in [
            ("", encoded.clone()),
            ("/AHx ", ascii::encode(&encoded, false)),
            ("/A85 ", ascii::encode(&encoded, true)),
        ] {
            let filter = if prefix.is_empty() {
                "/Filter /FlateDecode".to_owned()
            } else {
                format!("/Filter [{prefix}/Fl]")
            };
            let source = cases::document(case, &filter, &data, false, 2);
            let actual = run(&source, ample(), false);
            let drawing = actual.0.as_ref().unwrap();
            assert_eq!(
                shapes(drawing),
                shapes(&expected.0),
                "case={case}, {filter}"
            );
            assert_eq!(
                common::codes(&actual.2),
                common::codes(&expected.1),
                "case={case}, {filter}"
            );
            // フォントの複雑な輪郭計算を含まない入口（0..9）は全組み合わせで exact / used - 1 を検査し、
            // フォント・ICC 入口（9..22）は単独 Flate で exact / used - 1 を検査する。
            if case < 9 || prefix.is_empty() {
                exact_and_one_less(&source);
            }
        }
    }

    // 内容配列（case=1）とフォーム（case=2）の 2 回呼び出しは、2 回目の hayro 本展開と
    // 引き継いだデコーダー費用（DECODER_BYTES + READER_BYTES）を省略せず数える。
    for data in [&compressed, &raw] {
        for case in [1, 2] {
            let once = cases::document(case, "/Filter /FlateDecode", data, false, 1);
            let twice = cases::document(case, "/Filter /FlateDecode", data, false, 2);
            let first = run(&once, ample(), false);
            let second = run(&twice, ample(), false);
            assert!(first.0.is_ok() && second.0.is_ok());
            assert!(
                second.1.used(Kind::DecodedBytes) - first.1.used(Kind::DecodedBytes)
                    >= DECODER_BYTES + READER_BYTES,
                "case={case}"
            );
            assert!(
                second.1.used(Kind::Work) - first.1.used(Kind::Work) >= DECODER_WORK,
                "case={case}"
            );
            for kind in [Kind::DecodedBytes, Kind::Work] {
                stopped(&twice, kind, first.1.used(kind));
            }
            exact_and_one_less(&twice);
        }
    }
}

#[test]
fn diagnostic_reinflate_bounds_large_stream_work_per_chunk() {
    // 64 KiB の正常な内容ストリームで、本展開までは通るが leniently_inflated のチャンク走査の途中で
    // Work 上限に達する境界でも、決定的に停止して再試行できることを確かめる。
    let mut plain = b"0 0 m 10 10 l S\n".to_vec();
    plain.resize(64 * 1024, b' ');
    let data = zlib_encode(&plain);
    let source = cases::document(0, "/Filter /FlateDecode", &data, false, 1);
    let full = run(&source, ample(), false);
    assert!(full.0.is_ok());
    let total_work = full.1.used(Kind::Work);
    // 最後の数チャンク分の Work を削ると、leniently_inflated のループ内で止まる。
    stopped(&source, Kind::Work, total_work - 200);
    exact_and_one_less(&source);
}
