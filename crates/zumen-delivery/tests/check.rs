//! 試験の中で作った成果品を、工事完成図書の電子納品等要領のプロファイルと照らす（決定 0241、#384）。要領の
//! 作成例は転載が禁じられているので、要素の名前と構成は試験のために決めたもの。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostic, Diagnostics, Severity, Value, codes};
use zumen_delivery::{Report, check, read};

const PROFILE: &[u8] = include_bytes!("../../../profiles/mlit-delivery-const/2026-02/profile.json");

fn index(standard: &str, doctype: &str, extra: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE constdata SYSTEM "{doctype}">
<constdata>
  <基礎情報>
    <適用要領基準>{standard}</適用要領基準>
    <工事名称>道路改良工事{extra}</工事名称>
  </基礎情報>
</constdata>
"#
    )
}

const PLAN: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plandata SYSTEM "PLAN05.DTD">
<plandata>
  <施工計画書情報>
    <オリジナルファイル名>PLA01_01.PDF</オリジナルファイル名>
    <オリジナルファイル名>PLA02_01.PDF</オリジナルファイル名>
  </施工計画書情報>
</plandata>
"#;

/// 64×48 画素で、Exif の撮影の日時が 2026-10-31T09:15:30+09:00 の JPEG（適合試験の photo と同じ合成のもの）
const JPEG: &str = "ffd8ffe1011245786966000049492a000800000004000f010200060000003e00000012010300010000000100000069870400010000004400000025880400010000007d000000000000007a756d656e00020003900200140000006200000011900200070000007600000000000000323032363a31303a33312030393a31353a3330002b30393a303000060001000200020000004e0000000200050003000000cb0000000300020002000000450000000400050003000000e30000000600050001000000fb00000012000200070000000301000000000000230000000100000028000000010000001e000000010000008b000000010000002d0000000100000000000000010000007d0000000a0000005747532d383400ffe1010d687474703a2f2f6e732e61646f62652e636f6d2f7861702f312e302f003c783a786d706d65746120786d6c6e733a783d2261646f62653a6e733a6d6574612f223e3c7264663a52444620786d6c6e733a7264663d22687474703a2f2f7777772e77332e6f72672f313939392f30322f32322d7264662d73796e7461782d6e7323223e3c7264663a4465736372697074696f6e207264663a61626f75743d222220786d6c6e733a786d703d22687474703a2f2f6e732e61646f62652e636f6d2f7861702f312e302f223e3c786d703a526174696e673e333c2f786d703a526174696e673e3c2f7264663a4465736372697074696f6e3e3c2f7264663a5244463e3c2f783a786d706d6574613effc0000b080030004001011100ffda0008010100003f001234ffd9";

const PHOTO: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE photodata SYSTEM "PHOTO05.DTD">
<photodata DTD_version="05">
  <基礎情報><写真フォルダ名>PHOTO/PIC</写真フォルダ名><適用要領基準>土木202303-01</適用要領基準></基礎情報>
  <写真情報>
    <写真ファイル情報><シリアル番号>1</シリアル番号><写真ファイル名>P0000001.JPG</写真ファイル名>
      <写真ファイル日本語名>着手前.JPG</写真ファイル日本語名><メディア番号>1</メディア番号></写真ファイル情報>
    <付加情報><参考図ファイル名>D0000001.JPG</参考図ファイル名><参考図タイトル>平面図</参考図タイトル></付加情報>
    <撮影情報><撮影年月日>2026-10-31</撮影年月日></撮影情報>
  </写真情報>
  <写真情報>
    <写真ファイル情報><シリアル番号>2</シリアル番号><写真ファイル名>P0000002.JPG</写真ファイル名>
      <メディア番号>1</メディア番号></写真ファイル情報>
    <撮影情報><撮影年月日>2026-11-01</撮影年月日></撮影情報>
  </写真情報>
</photodata>
"#;

#[cfg(test)]
fn bytes(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[cfg(test)]
fn with_cx<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Vec<Code>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let out = {
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        f(&mut cx)
    };
    (out, diag.iter().map(|d| d.code()).collect())
}

#[cfg(test)]
fn run(files: &[(&str, &[u8])]) -> Report {
    let (report, diag) = with_cx(|cx| {
        let package = read(files, cx).unwrap();
        let (_, values) = zumen_profile::rules(
            &[("profile.json", PROFILE)],
            "mlit-delivery-const",
            "2026-02",
            cx,
        )
        .unwrap();
        check(&package, &values, cx).unwrap()
    });
    assert!(diag.is_empty(), "{diag:?}");
    report
}

#[cfg(test)]
fn name(diagnostic: &Diagnostic) -> Option<&str> {
    match diagnostic.param("name")? {
        Value::Text(text) => Some(text),
        _ => None,
    }
}

#[cfg(test)]
fn found(report: &Report) -> Vec<(String, Code)> {
    report
        .findings
        .iter()
        .map(|finding| (finding.rule.clone(), finding.diagnostic.code()))
        .collect()
}

#[test]
fn a_package_that_follows_the_rules_has_no_findings() {
    let index = index("土木202602-01", "INDE_C09.DTD", "");
    let report = run(&[
        ("INDEX_C.XML", index.as_bytes()),
        ("PLAN/PLAN.XML", PLAN.as_bytes()),
        ("PLAN/ORG/PLA01_01.PDF", b"%PDF"),
        ("PLAN/ORG/PLA02_01.PDF", b"%PDF"),
        ("DRAWINGF/D0PL001Z.P21", b""),
    ]);
    assert_eq!(found(&report), []);
    assert_eq!(report.rules.len(), 26);
    assert!(report.rules.iter().all(|rule| rule.findings == 0));
}

#[test]
fn names_files_standards_characters_and_dtds_are_checked() {
    // 全角の英字、NEC の特殊文字
    let index = index("土木 202603-01", "INDE_C05.DTD", "Ａ①");
    let report = run(&[
        ("INDEX_C.XML", index.as_bytes()),
        ("PLAN/PLAN.XML", PLAN.as_bytes()),
        ("PLAN/ORG/PLA01_01.PDF", b"%PDF"),
        ("PLAN/ORG/extra.pdf", b"%PDF"),
        ("PLAN/ORG/SUB/PLA03.PDF", b"%PDF"),
        ("MEET/ORG/M0001_01.PDF", b"%PDF"),
    ]);
    let found = found(&report);
    let expected = [
        ("name", codes::PROFILE_DELIVERY_NAME_INVALID),
        ("depth", codes::PROFILE_DELIVERY_NAME_INVALID),
        ("meet", codes::PROFILE_DELIVERY_REQUIRED_INVALID),
        ("link", codes::PROFILE_DELIVERY_LINK_INVALID),
        ("unreferenced", codes::PROFILE_DELIVERY_UNREFERENCED_INVALID),
        ("standard", codes::PROFILE_DELIVERY_STANDARD_INVALID),
        ("characters", codes::PROFILE_DELIVERY_CHARACTERS_INVALID),
        ("doctype-index", codes::PROFILE_DELIVERY_DOCTYPE_INVALID),
    ];
    for (rule, code) in expected {
        assert!(
            found.iter().any(|(r, c)| r == rule && *c == code),
            "{rule} {found:?}"
        );
    }
    // 小文字の名前と、PLAN/ORG の下の階層
    let names: Vec<&str> = report
        .findings
        .iter()
        .filter(|finding| finding.rule == "name" || finding.rule == "depth")
        .filter_map(|finding| name(&finding.diagnostic))
        .collect();
    assert_eq!(names, ["PLAN/ORG/extra.pdf", "PLAN/ORG/SUB/PLA03.PDF"]);
    // PLA02_01.PDF を指すが、成果品にない
    let link = report
        .findings
        .iter()
        .find(|finding| finding.rule == "link")
        .unwrap();
    assert_eq!(name(&link.diagnostic), Some("PLAN/PLA02_01.PDF"));
    assert_eq!(link.level, Severity::Error);
}

#[test]
fn a_missing_standard_element_cannot_be_checked() {
    let index = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE constdata SYSTEM "INDE_C09.DTD">
<constdata><基礎情報/></constdata>
"#;
    let report = run(&[("INDEX_C.XML", index.as_bytes())]);
    let standard = report
        .rules
        .iter()
        .find(|rule| rule.id == "standard")
        .unwrap();
    assert_eq!((standard.findings, standard.unsupported), (0, 1));
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.rule == "standard")
        .unwrap();
    assert_eq!(
        finding.diagnostic.code(),
        codes::PROFILE_DELIVERY_STANDARD_UNSUPPORTED
    );
    assert_eq!(finding.level, Severity::Info);
}

#[test]
fn photos_are_checked_against_the_management_file() {
    let jpeg = bytes(JPEG);
    // Exif のない JPEG（フレームの見出しだけ）
    let plain = bytes("ffd8ffc0000b080030004001011100ffd9");
    let report = run(&[
        ("PHOTO/PHOTO.XML", PHOTO.as_bytes()),
        ("PHOTO/PIC/P0000001.JPG", &jpeg),
        ("PHOTO/PIC/P0000002.JPG", &jpeg),
        ("PHOTO/PIC/X1.JPG", &plain),
        ("PHOTO/DRA/D0000001.JPG", &plain),
    ]);
    let mut photo: Vec<(String, Code, Option<String>)> = report
        .findings
        .iter()
        .filter(|finding| finding.rule.starts_with("photo-"))
        .map(|finding| {
            (
                finding.rule.clone(),
                finding.diagnostic.code(),
                name(&finding.diagnostic).map(str::to_string),
            )
        })
        .collect();
    photo.sort_by(|a, b| (&a.0, &a.2).cmp(&(&b.0, &b.2)));
    let text = |s: &str| Some(s.to_string());
    assert_eq!(
        photo,
        [
            (
                "photo-date".to_string(),
                codes::PROFILE_DELIVERY_PHOTO_DATE_INVALID,
                text("PHOTO/PIC/P0000002.JPG")
            ),
            (
                "photo-name".to_string(),
                codes::PROFILE_DELIVERY_NAME_INVALID,
                text("PHOTO/PIC/X1.JPG")
            ),
            (
                "photo-pixels".to_string(),
                codes::PROFILE_DELIVERY_PHOTO_PIXELS_INVALID,
                text("PHOTO/PIC/P0000001.JPG")
            ),
            (
                "photo-pixels".to_string(),
                codes::PROFILE_DELIVERY_PHOTO_PIXELS_INVALID,
                text("PHOTO/PIC/P0000002.JPG")
            ),
            (
                "photo-pixels".to_string(),
                codes::PROFILE_DELIVERY_PHOTO_PIXELS_INVALID,
                text("PHOTO/PIC/X1.JPG")
            ),
            (
                "photo-unreferenced".to_string(),
                codes::PROFILE_DELIVERY_UNREFERENCED_INVALID,
                text("PHOTO/PIC/X1.JPG")
            ),
        ]
    );
    let pixels = report
        .findings
        .iter()
        .find(|finding| finding.rule == "photo-pixels")
        .unwrap();
    assert_eq!(pixels.level, Severity::Info);
}
