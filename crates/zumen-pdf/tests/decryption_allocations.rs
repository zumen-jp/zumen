//! F・Type の判定と本文復号を、値を作る前に予約する（#778）。

#![cfg(feature = "drawing")]

use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, Stop};
use zumen_drawing::Drawing2D;
use zumen_pdf::Pdf;
use zumen_sdk::{Error, Lift, testkit};

mod common;

const LINE: &[u8] = b"0 0 m 1 1 l S\n";
// Node の標準 crypto で自己生成。空の owner/user password、固定 ID と IV、obj4/6。
// ISO 32000-2:2020 の 7.6 に従う、Standard R2/RC4 と R4/AESV2 の実装上の合成入力。
// R2/R4 の手順は hayro-syntax 0.9.0 の crypto と同じ。受領図面・秘密・第三者フォントは使わない。
const RC4_DICT: &str = "<< /Filter /Standard /V 1 /R 2 /P -4 /O <2055c756c72e1ad702608e8196acad447ad32d17cff583235f6dd15fed7dab67> /U <92c034164f1ef00c9edf3ba7595c16b11c50ff99a474b82f495eabb623e302f8> >>";
const AES_DICT: &str = "<< /Filter /Standard /V 4 /R 4 /Length 128 /P -4 /O <36451bd39d753b7c1d10922c28e6665aa4f3353fb0348b536893e3b1db5c579b> /U <acf99d8614017dd73cbe2bc3c79b668c00000000000000000000000000000000> /EncryptMetadata true /StmF /StdCF /StrF /StdCF /CF << /StdCF << /CFM /AESV2 /Length 16 >> >> >>";
const MASK4: &str = "0e2030bbbfaf55ac6d99bd277e57b9511bf2c89378aa7cd909f60049d3695daa5690faced25007a6a2e844038f8fa960c5b8a298cdaa1c334229d50fa2ba2fcd56263a401271adba4c3cc853dd98b63b287fa73943bdfd521784d2ed15fb7623b992270f3c2a59b81b72ca9fa0cdd557d38a0512d1b763d9f8add1abf6fecb6bbe1a9ca8529f262134a8db396f9b602f5933250ba2e13f7b2c476407d0782cace24d1fc8695cdea2062d018416eee9fed264bbce0992c9d295d66fcd8bf45b0201515a9dfd6fee217e6a2939c47db985b7d50630b57e8db14a7b53506c86ac4e38fdf398cf036e99324147fc190ae11e";
const MASK6: &str = "847c2e803af6e08161b59cf70927d28bdc5f07e67acece9f8032401bd7111b5e79";
const AES_SPACES: &[(usize, &str)] = &[
    (
        0,
        "0405060708090a0b0c0d0e0f10111213e5c82f019ae28c4f392b72c274c62bea",
    ),
    (
        1,
        "0405060708090a0b0c0d0e0f101112136586de31822874633da2c9d9af84c1d0",
    ),
    (
        15,
        "0405060708090a0b0c0d0e0f10111213edb0cf1e4508e6903228a5576a69adcd",
    ),
    (
        16,
        "0405060708090a0b0c0d0e0f101112130efe67c4ed9797c34bf22c2c90dd661ef444dfc7d50d44f5fcbb1fa9dc1ce7b8",
    ),
    (
        17,
        "0405060708090a0b0c0d0e0f101112130efe67c4ed9797c34bf22c2c90dd661efa2efe9728e427183b210064408da427",
    ),
    (
        31,
        "0405060708090a0b0c0d0e0f101112130efe67c4ed9797c34bf22c2c90dd661e9e0dde64949638018219abf162a6fbf6",
    ),
    (
        32,
        "0405060708090a0b0c0d0e0f101112130efe67c4ed9797c34bf22c2c90dd661e193135a50b6462958ecb97dea694e86bddbd8271958a91e2a6f1e2544bf7be99",
    ),
    (
        33,
        "0405060708090a0b0c0d0e0f101112130efe67c4ed9797c34bf22c2c90dd661e193135a50b6462958ecb97dea694e86b50d2892663bc4016a674ed7dc4a47cec",
    ),
];
// クリアのフィルター入力と、その AESV2 の暗号文。LZW は clear + 9bit literal + EOD。
const PAYLOADS: &[(&str, &str, &str)] = &[
    (
        "",
        "302030206d20312031206c20530a",
        "0405060708090a0b0c0d0e0f10111213ebf71b0e76395b5ae09233a0b68f6615",
    ),
    (
        "/Filter /Fl",
        "78da33503050c85530543054c85108e60200143e02b9",
        "0405060708090a0b0c0d0e0f10111213870d7a396736892523f4600752838846a7c9d0ae30e8b1bd8f83d2000b75b59c",
    ),
    (
        "/Filter /LZW",
        "800c04030101b44031100c4406c1014c1501",
        "0405060708090a0b0c0d0e0f10111213ae0a6aa6ebfd68bb61fc6ec96f77c5239ab30fb18106eb80e99be67703564c93",
    ),
    (
        "/Filter /RL",
        "0d302030206d20312031206c20530a80",
        "0405060708090a0b0c0d0e0f101112139ab4cba0e4cc0302420604cee376cd536ee38eb211b73a7bed655f1fb5e5f25b",
    ),
    (
        "/Filter /AHx",
        "333032303330323036643230333132303331323036633230353330613e",
        "0405060708090a0b0c0d0e0f101112138f38c267a140d8dfc36f6542e4c875938f3ea20db6e6f8139d094ee5eda762aa",
    ),
    (
        "/Filter [/AHx /Fl] /DP [null null]",
        "37386461333335303330353063383535333035343330353463383531303865363032303031343365303262393e",
        "0405060708090a0b0c0d0e0f10111213e98f74943257b7034ccaac38f51af952e69f0ababc9a75975faae6299e1165c356fe6da7de95587a81fa2c9653f70741",
    ),
    (
        "/Filter [/RL /RL]",
        "0f0d302030206d20312031206c20530a8080",
        "0405060708090a0b0c0d0e0f101112136dfc5b579a8690ff4457d0905fc83cc565640e92f8e4b1e3d26939d17ccbdc79",
    ),
];
const AES_XREF: &str = "0405060708090a0b0c0d0e0f10111213d55e8f966c27480523b2ffd0da15e0e2";
const AES_TYPE: &str = "0405060708090a0b0c0d0e0f10111213bcc160427c50fa8297c18f1590b67ba2";
const AES_LONG: &str = "0405060708090a0b0c0d0e0f1011121376e8b4ce0e49d73babf34d4c3641c611e5c098a54b614b1dabc506dcfc43dc1430a11b613f00935d58c79402e0edcae513565c430d0bf37f5045baf92448a4eb46e0941a0121cd3eb54ee3baeb2cedc14c423b1f7b62e998acbb009da679af12aeac4ef4fae84fbac4a4f1f845c7d3a8e99604937922fa75fefd1e81dda1dd1acf2f8560d9fe7dffee91cc109a31a89fbd5383f459c29f0812baa71d242fb0760f799d5c044ae02bf7003f9b6d8be0fd024e5dacfc7368cb59803c2b7e427bc71c117af837bf8526bbed85c440430d72c5fe56f604a3fedc3983c8b003406e0bc1bf9956bc5a42dfa4035734b1ab344d67bfc052173477df8f621e6e4a2fefdd";
const AES_F: &str = "0405060708090a0b0c0d0e0f1011121325c4842f70fa1540d9f956c7f7bc4ab1";
const AES_F6: &str = "060708090a0b0c0d0e0f101112131415cfb9673c660865bcdedea63516c167e8";
const AES_OPS: [&str; 2] = [
    "060708090a0b0c0d0e0f101112131415ad3eea1c4dff40f8838632c877cb8c7b",
    "060708090a0b0c0d0e0f1011121314158a5306562ff5b125cf9f13d5244c55cb",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Security {
    Plain,
    Rc4,
    Aes,
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
fn encrypted(sec: Security, clear: &[u8], aes: &str, object: u32) -> Vec<u8> {
    match sec {
        Security::Plain => clear.to_vec(),
        Security::Aes => unhex(aes),
        Security::Rc4 => {
            let mask = unhex(if object == 4 { MASK4 } else { MASK6 });
            assert!(mask.len() >= clear.len());
            clear.iter().zip(mask).map(|(&a, b)| a ^ b).collect()
        }
    }
}

#[cfg(test)]
fn hex_string(bytes: &[u8]) -> String {
    format!(
        "<{}>",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}

#[cfg(test)]
fn escaped_string(bytes: &[u8]) -> String {
    format!(
        "({})",
        bytes
            .iter()
            .map(|b| format!("\\{b:03o}"))
            .collect::<String>()
    )
}

#[cfg(test)]
fn document(
    sec: Security,
    attrs: &str,
    stored: &[u8],
    extras: &[String],
    calls: usize,
    form: bool,
) -> Vec<u8> {
    let mut b = common::Builder::default();
    b.object("<< /Type /Catalog /Pages 2 0 R >>");
    b.object("<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    let contents = if form {
        "/Resources << /XObject << /Fm 4 0 R >> >> /Contents 6 0 R".into()
    } else if calls == 1 {
        "/Contents 4 0 R".into()
    } else {
        format!("/Contents [{}]", "4 0 R ".repeat(calls))
    };
    b.object(format!(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] {contents} >>"
    ));
    let kind = if form {
        "/Type /XObject /Subtype /Form /BBox [0 0 100 100]"
    } else {
        ""
    };
    b.stream(&format!("{kind} {attrs}"), stored);
    b.object(match sec {
        Security::Plain => "null",
        Security::Rc4 => RC4_DICT,
        Security::Aes => AES_DICT,
    });
    if form {
        assert!(calls == 1 || calls == 2);
        let ops = "/Fm Do\n".repeat(calls);
        b.stream("", encrypted(sec, ops.as_bytes(), AES_OPS[calls - 1], 6));
    } else {
        for extra in extras {
            b.object(extra);
        }
    }
    if sec != Security::Plain {
        b.trailer = "/Encrypt 5 0 R /ID [<30303030303030303030303030303030> <30303030303030303030303030303030>]".into();
    }
    b.bytes(1)
}

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
fn shapes(d: &Drawing2D) -> Vec<String> {
    d.sheets
        .iter()
        .flat_map(|s| &s.elements)
        .map(|e| format!("{:?} {:?}", e.shape, e.style))
        .collect()
}

#[cfg(test)]
fn stopped(source: &[u8], kind: Kind, limit: u64) {
    let limits = ample().with(kind, Limit::Finite(limit));
    let actual = run(source, limits, false);
    assert!(matches!(actual.0, Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == kind));
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
}

#[cfg(test)]
fn boundaries(source: &[u8]) {
    let actual = run(source, ample(), false);
    assert!(actual.0.is_ok());
    for kind in [Kind::DecodedBytes, Kind::Work] {
        let used = actual.1.used(kind);
        assert!(used > 0);
        let exact = run(source, ample().with(kind, Limit::Finite(used)), false);
        assert_eq!(format!("{:?}", exact.0), format!("{:?}", actual.0));
        assert_eq!(exact.2, actual.2);
        stopped(source, kind, used - 1);
    }
    let cancelled = run(source, ample(), true);
    assert!(matches!(cancelled.0, Err(Error::Stopped(Stop::Cancelled))));
    assert_eq!(cancelled.1.used(Kind::DecodedBytes), 0);
    assert_eq!(cancelled.1.used(Kind::Work), 0);
    assert_eq!(
        format!("{actual:?}"),
        format!("{:?}", run(source, ample(), false))
    );
}

#[cfg(test)]
fn added_cost(small: &[u8], large: &[u8]) {
    let base = run(small, ample(), false);
    let actual = run(large, ample(), false);
    assert!(base.0.is_ok() && actual.0.is_ok());
    assert_eq!(
        shapes(base.0.as_ref().unwrap()),
        shapes(actual.0.as_ref().unwrap())
    );
    assert_eq!(common::codes(&base.2), common::codes(&actual.2));
    for kind in [Kind::DecodedBytes, Kind::Work] {
        let lower = base.1.used(kind);
        let upper = actual.1.used(kind);
        assert!(upper > lower, "{kind:?}");
        stopped(large, kind, lower + (upper - lower) / 2);
    }
}

#[test]
fn short_rc4_and_aes_payloads_obey_exact_limits_and_cancel() {
    for sec in [Security::Rc4, Security::Aes] {
        for &(length, aes) in AES_SPACES {
            let stored = encrypted(sec, &vec![b' '; length], aes, 4);
            let source = document(sec, "", &stored, &[], 1, false);
            let actual = run(&source, ample(), false);
            assert!(actual.0.unwrap().sheets[0].elements.is_empty());
            assert!(actual.2.is_empty());
            boundaries(&source);
        }
    }
}

#[test]
fn ordinary_before_false_and_compressed_branches_keep_shapes_and_ids() {
    let expected = run(
        &document(Security::Plain, "", LINE, &[], 1, false),
        ample(),
        false,
    );
    for sec in [Security::Rc4, Security::Aes] {
        for &(attrs, clear, aes) in PAYLOADS {
            let stored = encrypted(sec, &unhex(clear), aes, 4);
            let source = document(sec, attrs, &stored, &[], 1, false);
            let actual = run(&source, ample(), false);
            assert_eq!(
                shapes(actual.0.as_ref().unwrap()),
                shapes(expected.0.as_ref().unwrap())
            );
            assert_eq!(actual.2, expected.2);
            boundaries(&source);
        }
        let source = document(
            sec,
            "/Filter /FutureFilter",
            &encrypted(sec, LINE, PAYLOADS[0].2, 4),
            &[],
            1,
            false,
        );
        assert_eq!(
            shapes(run(&source, ample(), false).0.as_ref().unwrap()),
            shapes(expected.0.as_ref().unwrap())
        );
        boundaries(&source);
    }
}

#[test]
fn zero_length_raw_reads_still_charge_key_setup_before_allocating() {
    for sec in [Security::Rc4, Security::Aes] {
        let stored = encrypted(sec, b"", AES_SPACES[0].1, 4);
        let image = document(sec, "/Filter /DCT", &stored, &[], 1, false);
        let source = document(sec, "", &stored, &[], 1, false);
        let skipped = run(&image, ample(), false);
        let read = run(&source, ample(), false);
        assert!(skipped.0.is_ok() && read.0.is_ok());
        // 画像は本文を読まない。空の本文を読む方は、出力が空でも鍵と MD5 の固定確保を持つ。
        for kind in [Kind::DecodedBytes, Kind::Work] {
            let lower = skipped.1.used(kind);
            let upper = read.1.used(kind);
            assert!(upper > lower, "{sec:?}, {kind:?}");
            stopped(&source, kind, lower + (upper - lower) / 2);
        }
    }
}

#[test]
fn long_type_strings_are_budgeted_in_both_lexical_forms() {
    for sec in [Security::Rc4, Security::Aes] {
        let stored = encrypted(sec, b"", AES_SPACES[0].1, 4);
        let short = encrypted(sec, b"Content", AES_TYPE, 4);
        let long = encrypted(sec, &[b'A'; 240], AES_LONG, 4);
        let baseline = document(
            sec,
            &format!("/Type {}", hex_string(&short)),
            &stored,
            &[],
            1,
            false,
        );
        for value in [hex_string(&long), escaped_string(&long)] {
            let source = document(sec, &format!("/Type {value}"), &stored, &[], 1, false);
            added_cost(&baseline, &source);
            boundaries(&source);
        }
    }
}

#[test]
fn name_xref_and_decrypted_string_xref_keep_their_distinct_body_rules() {
    for sec in [Security::Rc4, Security::Aes] {
        let encrypted_line = encrypted(sec, LINE, PAYLOADS[0].2, 4);
        let named = document(sec, "/Type /XRef", &encrypted_line, &[], 1, false);
        let value = encrypted(sec, b"XRef", AES_XREF, 4);
        let string = document(
            sec,
            &format!("/Type {}", hex_string(&value)),
            LINE,
            &[],
            1,
            false,
        );
        let left = run(&named, ample(), false);
        let right = run(&string, ample(), false);
        assert_eq!(left.0.as_ref().unwrap().sheets[0].elements.len(), 1);
        assert_eq!(
            shapes(left.0.as_ref().unwrap()),
            shapes(right.0.as_ref().unwrap())
        );
        assert_eq!(left.2, right.2);
        boundaries(&named);
        boundaries(&string);
        let refused = document(
            sec,
            "/Type 6 0 R",
            &encrypted_line,
            &[hex_string(&encrypted(sec, b"external.bin", AES_F6, 6))],
            1,
            false,
        );
        assert!(matches!(
            run(&refused, ample(), false).0,
            Err(Error::Unsupported)
        ));
    }
}

#[test]
fn external_f_is_budgeted_even_when_no_payload_is_decrypted() {
    for sec in [Security::Rc4, Security::Aes] {
        let stored = encrypted(sec, b"", AES_SPACES[0].1, 4);
        let short = encrypted(sec, b"external.bin", AES_F, 4);
        let long = encrypted(sec, &[b'A'; 240], AES_LONG, 4);
        let baseline = document(
            sec,
            &format!("/F {}", hex_string(&short)),
            &stored,
            &[],
            1,
            false,
        );
        for value in [hex_string(&long), escaped_string(&long)] {
            let source = document(sec, &format!("/F {value}"), &stored, &[], 1, false);
            added_cost(&baseline, &source);
            boundaries(&source);
        }
        for (attrs, extras) in [
            ("/F <<>>".into(), Vec::new()),
            (format!("/F << /UF {} >>", hex_string(&short)), Vec::new()),
            (
                "/F 6 0 R".into(),
                vec![hex_string(&encrypted(sec, b"external.bin", AES_F6, 6))],
            ),
        ] {
            let source = document(sec, &attrs, &stored, &extras, 1, false);
            assert!(
                run(&source, ample(), false).0.unwrap().sheets[0]
                    .elements
                    .is_empty()
            );
            boundaries(&source);
        }
        for mode in [1, 5, 6] {
            let (attrs, clear, aes) = PAYLOADS[mode];
            let source = document(
                sec,
                &format!("/F {} {attrs}", hex_string(&short)),
                &encrypted(sec, &unhex(clear), aes, 4),
                &[],
                1,
                false,
            );
            assert!(
                run(&source, ample(), false).0.unwrap().sheets[0]
                    .elements
                    .is_empty()
            );
            boundaries(&source);
        }
    }
}

#[test]
fn diagnostic_and_redecode_paths_keep_decryption_costs_after_failure() {
    for sec in [Security::Rc4, Security::Aes] {
        for predictor in [1, 300, -1] {
            let stored = encrypted(sec, &unhex(PAYLOADS[1].1), PAYLOADS[1].2, 4);
            let attrs = format!("/Filter /Fl /DP << /Predictor {predictor} >>");
            let source = document(sec, &attrs, &stored, &[], 1, false);
            assert_eq!(
                run(&source, ample(), false).0.unwrap().sheets[0]
                    .elements
                    .len(),
                1
            );
            boundaries(&source);
        }
        for (attrs, clear, aes) in [
            PAYLOADS[0],
            (
                "/Filter /Fl /DP << /Predictor 3 >>",
                PAYLOADS[1].1,
                PAYLOADS[1].2,
            ),
        ] {
            let stored = encrypted(sec, &unhex(clear), aes, 4);
            for form in [false, true] {
                let once = document(sec, attrs, &stored, &[], 1, form);
                let twice = document(sec, attrs, &stored, &[], 2, form);
                let first = run(&once, ample(), false);
                let second = run(&twice, ample(), false);
                assert!(first.0.is_ok() && second.0.is_ok());
                for kind in [Kind::DecodedBytes, Kind::Work] {
                    assert!(second.1.used(kind) > first.1.used(kind));
                    stopped(&twice, kind, first.1.used(kind));
                }
                boundaries(&twice);
            }
        }
    }
}

#[test]
fn encrypted_empty_parameter_strings_charge_their_fixed_keys_without_stages() {
    let names = "/Unknown ".repeat(64);
    let stored = encrypted(Security::Rc4, b"", "", 4);
    let nulls = document(
        Security::Rc4,
        &format!("/Filter [{names}] /DP [{}]", "null ".repeat(64)),
        &stored,
        &[],
        1,
        false,
    );
    let strings = document(
        Security::Rc4,
        &format!("/Filter [{names}] /DP [{}]", "() ".repeat(64)),
        &stored,
        &[],
        1,
        false,
    );
    let baseline = run(&nulls, ample(), false);
    let actual = run(&strings, ample(), false);
    assert!(baseline.0.is_ok() && actual.0.is_ok());
    assert_eq!(
        shapes(baseline.0.as_ref().unwrap()),
        shapes(actual.0.as_ref().unwrap())
    );
    assert_eq!(common::codes(&baseline.2), common::codes(&actual.2));
    // () は null より短く、配列全体の保守的な容量予約は減る。鍵生成の Work は増える。
    let lower = baseline.1.used(Kind::Work);
    let upper = actual.1.used(Kind::Work);
    assert!(upper > lower);
    stopped(&strings, Kind::Work, lower + (upper - lower) / 2);
    boundaries(&strings);
}
