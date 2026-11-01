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
    assert_eq!(report.rules.len(), 16);
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
