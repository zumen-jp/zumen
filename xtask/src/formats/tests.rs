use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) struct TempDir(pub PathBuf);

impl TempDir {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "zumen-formats-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("テスト用のフォルダーを作れない: {e}"),
            }
        }
    }

    fn write(&self, path: &str, contents: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const LEDGER: &str = r#"
[specs.dxf_1]
title = "テスト用の仕様"
publisher = "テスト用の公開元"
access = "public"
url = "https://example.com/spec"
checked = "2024-02-29"

[formats.dxf]
name = "DXF"
family = "acad"
domains = ["drawing"]
specs = ["dxf_1"]
rights = "public"
rights-note = "テスト用の根拠"
place = "core"
basis = ["spec"]
decisions = ["0008"]
legacy-decisions = ["0003"]
issues = ["zumen-jp/zumen-legacy#140"]
docs = ["docs/formats/dxf.md"]
read = { level = "synthetic", scope = "テストの中で作ったファイル", checked = "2026-11-03" }

[formats.restricted]
name = "制限のある形式"
family = "restricted"
domains = ["building"]
specs = []
rights = "unknown"
rights-note = "テスト用の未確認の条件"
place = "none"
known-crates = ["restricted-reader"]
"#;

/// 形式の文書。確かめた度合いの節の表に `rows` を並べる
fn document(rows: &str) -> String {
    format!(
        "# テスト用の文書\n\n## 確かめた度合い\n\n| 形式 | 向き | 段 |\n| --- | --- | --- |\n{rows}\n## 次の節\n\n| `dxf` | 書き | `synthetic` |\n"
    )
}

fn fixture() -> (TempDir, Ledger, Decisions) {
    let dir = TempDir::new();
    dir.write("docs/decisions/0008-model.md", "# テスト用の決定\n");
    dir.write(
        "docs/formats/dxf.md",
        &document("| `dxf` | 読み | `synthetic` |\n"),
    );
    let decisions = decision_files(&dir.0).unwrap();
    (dir, toml::from_str(LEDGER).unwrap(), decisions)
}

/// 公開サンプルの台帳にあるとする道筋で検査する
fn check(ledger: &Ledger, root: &Path, decisions: &Decisions) -> Vec<String> {
    let samples = ["dwg/a.dwg", "dwg/a_ascii.dxf", "dxf/b.dxf"]
        .map(String::from)
        .into();
    super::check(ledger, root, decisions, &samples)
}

#[test]
fn accepts_public_core_and_restricted_none() {
    let (dir, ledger, decisions) = fixture();
    assert_eq!(check(&ledger, &dir.0, &decisions), Vec::<String>::new());
}

#[test]
fn rejects_missing_required_fields_unknown_fields_and_wrong_types() {
    for (from, to) in [
        ("publisher = \"テスト用の公開元\"", ""),
        ("rights-note = \"テスト用の根拠\"", ""),
        ("domains = [\"drawing\"]", "domains = \"drawing\""),
        (
            "place = \"core\"",
            "place = \"core\"\nrights_note = \"誤記\"",
        ),
        (
            "known-crates = [\"restricted-reader\"]",
            "known-crates = [12]",
        ),
    ] {
        assert!(
            toml::from_str::<Ledger>(&LEDGER.replace(from, to)).is_err(),
            "{from}"
        );
    }
}

#[test]
fn rejects_invalid_values_and_inconsistent_references() {
    let (dir, _, decisions) = fixture();
    for (from, to, expected) in [
        ("[specs.dxf_1]", "[specs.DXF]", "ID"),
        ("[formats.dxf]", "[formats.dxf-file]", "キー"),
        ("name = \"DXF\"", "name = \" \"", "name が空"),
        ("access = \"public\"", "access = \"free\"", "access"),
        (
            "rights = \"public\"",
            "rights = \"permission\"",
            "core に置ける",
        ),
        ("rights = \"public\"", "rights = \"open\"", "rights"),
        ("place = \"core\"", "place = \"internal\"", "place"),
        ("domains = [\"drawing\"]", "domains = []", "domains"),
        (
            "domains = [\"drawing\"]",
            "domains = [\"drawing\", \"drawing\"]",
            "重なって",
        ),
        (
            "domains = [\"drawing\"]",
            "domains = [\"image\"]",
            "domains",
        ),
        (
            "specs = [\"dxf_1\"]",
            "specs = [\"missing\"]",
            "仕様 `missing`",
        ),
        ("basis = [\"spec\"]", "basis = []", "basis"),
        ("basis = [\"spec\"]", "basis = [\"guessed\"]", "basis"),
        ("specs = [\"dxf_1\"]", "specs = []", "仕様の参照"),
        ("place = \"core\"", "place = \"plugin\"", "repository"),
        (
            "basis = [\"spec\"]",
            "basis = [\"reference\"]",
            "references",
        ),
        (
            "checked = \"2024-02-29\"",
            "checked = \"2023-02-29\"",
            "checked",
        ),
        ("https://example.com/spec", "https:///spec", "url"),
        (
            "decisions = [\"0008\"]",
            "decisions = [\"9999\"]",
            "決定 `9999`",
        ),
        (
            "legacy-decisions = [\"0003\"]",
            "legacy-decisions = [\"3\"]",
            "旧版の決定",
        ),
        ("zumen-jp/zumen-legacy#140", "zumen-legacy#140", "issue"),
        ("docs/formats/dxf.md", "docs/formats/missing.md", "実在する"),
        ("restricted-reader", "restricted-reader@1.0", "crate の名前"),
        (
            "level = \"synthetic\"",
            "level = \"tested\"",
            "level の `tested`",
        ),
        (
            "level = \"synthetic\"",
            "level = \"application\"",
            "根拠と合わない",
        ),
        (
            "scope = \"テストの中で作ったファイル\"",
            "scope = \" \"",
            "scope が空",
        ),
        (
            "checked = \"2026-11-03\"",
            "checked = \"2026-11-31\"",
            "dxf.read: checked",
        ),
        ("docs = [\"docs/formats/dxf.md\"]", "", "節を置く docs"),
        (
            "known-crates = [\"restricted-reader\"]",
            "known-crates = [\"restricted-reader\"]\nwrite = { level = \"synthetic\", scope = \"範囲\", checked = \"2026-11-03\" }",
            "core の形式にだけ",
        ),
    ] {
        let ledger: Ledger = toml::from_str(&LEDGER.replace(from, to)).unwrap();
        let errors = check(&ledger, &dir.0, &decisions);
        assert!(
            errors.iter().any(|e| e.contains(expected)),
            "{from}: {errors:?}"
        );
    }
}

#[test]
fn checks_evidence_and_implementation_sources() {
    let (dir, mut ledger, decisions) = fixture();
    let f = ledger.formats.get_mut("dxf").unwrap();
    f.place = "plugin".into();
    f.read = None;
    // 段がなければ、文書の節の表に行を置かない
    dir.write("docs/formats/dxf.md", &document(""));
    f.repository = Some("example/plugin".into());
    f.basis = vec!["reference".into()];
    f.references.push(Reference {
        name: "参照元".into(),
        license: "MIT".into(),
        url: "https://example.com/reader".into(),
    });
    f.evidence.push(Evidence {
        title: "根拠".into(),
        url: "https://example.com/terms".into(),
        checked: "2026-10-25".into(),
        note: None,
    });
    assert!(check(&ledger, &dir.0, &decisions).is_empty());
    let f = ledger.formats.get_mut("dxf").unwrap();
    f.references[0].license.clear();
    f.evidence[0].checked.clear();
    f.evidence[0].url = "file:///private".into();
    f.repository = Some(" ".into());
    let errors = check(&ledger, &dir.0, &decisions);
    for key in ["license", "checked", "url", "repository"] {
        assert!(errors.iter().any(|e| e.contains(key)), "{errors:?}");
    }
}

#[test]
fn derives_the_level_from_the_strongest_evidence() {
    fn read(ledger: &mut Ledger) -> &mut Verification {
        ledger
            .formats
            .get_mut("dxf")
            .unwrap()
            .read
            .as_mut()
            .unwrap()
    }
    let (dir, mut ledger, decisions) = fixture();
    let tool = |name: &str| Tool {
        name: name.into(),
        version: "1.0".into(),
    };
    let strings = |values: &[&str]| values.iter().map(|v| v.to_string()).collect::<Vec<_>>();
    let set = |ledger: &mut Ledger,
               level: &str,
               samples: &[&str],
               attached: &[&str],
               tools: Vec<Tool>,
               software| {
        let found = read(ledger);
        found.level = level.into();
        found.public_samples = strings(samples);
        found.attached_values = strings(attached);
        found.implementations = tools;
        found.software = software;
    };
    let ok = |ledger: &Ledger| {
        dir.write(
            "docs/formats/dxf.md",
            &document(&format!(
                "| `dxf` | 読み | `{}` |\n",
                ledger.formats["dxf"].read.as_ref().unwrap().level
            )),
        );
        check(ledger, &dir.0, &decisions)
    };
    // 公開サンプルを読んだだけでは、synthetic のまま
    let sample = &["samples/dxf/b.dxf"][..];
    set(&mut ledger, "synthetic", sample, &[], vec![], vec![]);
    assert_eq!(ok(&ledger), Vec::<String>::new());
    // 添えられた値か、外部の実装が読んだ値と照らせば external
    set(
        &mut ledger,
        "external",
        sample,
        &["要素の数"],
        vec![],
        vec![],
    );
    assert_eq!(ok(&ledger), Vec::<String>::new());
    set(
        &mut ledger,
        "external",
        &[],
        &[],
        vec![tool("ezdxf")],
        vec![],
    );
    assert_eq!(ok(&ledger), Vec::<String>::new());
    // フォルダーと URL のサンプル
    set(
        &mut ledger,
        "application",
        &["samples/dwg/", "https://example.com/sample.dxf"],
        &[],
        vec![],
        vec![tool("AutoCAD")],
    );
    assert_eq!(ok(&ledger), Vec::<String>::new());
    for (level, attached, tools, software) in [
        ("external", &[][..], vec![], vec![]),
        ("synthetic", &["要素の数"][..], vec![], vec![]),
        ("synthetic", &[][..], vec![tool("ezdxf")], vec![]),
        ("external", &[][..], vec![], vec![tool("AutoCAD")]),
        ("application", &[][..], vec![tool("ezdxf")], vec![]),
    ] {
        set(&mut ledger, level, sample, attached, tools, software);
        let errors = ok(&ledger);
        assert!(
            errors.iter().any(|e| e.contains("根拠と合わない")),
            "{level} {attached:?}: {errors:?}"
        );
    }
    for (sample, expected) in [
        ("samples/dxf/missing.dxf", "ledger.toml にない"),
        ("samples/dw/", "ledger.toml にない"),
        ("samples/dxf", "ledger.toml にない"),
        ("samples//", "ledger.toml にない"),
        ("dxf/b.dxf", "URL にする"),
        ("file:///b.dxf", "URL にする"),
    ] {
        set(&mut ledger, "synthetic", &[sample], &[], vec![], vec![]);
        let errors = ok(&ledger);
        assert!(
            errors.iter().any(|e| e.contains(expected)),
            "{sample}: {errors:?}"
        );
    }
    set(
        &mut ledger,
        "synthetic",
        &["samples/dxf/b.dxf", "samples/dxf/b.dxf"],
        &[],
        vec![],
        vec![],
    );
    assert!(ok(&ledger).iter().any(|e| e.contains("重なって")));
    set(
        &mut ledger,
        "application",
        &[],
        &[],
        vec![tool(" ")],
        vec![Tool {
            name: "AutoCAD".into(),
            version: String::new(),
        }],
    );
    let errors = ok(&ledger);
    for expected in ["implementations[0]: name", "software[0]: version"] {
        assert!(errors.iter().any(|e| e.contains(expected)), "{errors:?}");
    }
    // 添えられた値は、読んだ公開サンプルか受け取った図面のもの
    set(&mut ledger, "external", &[], &["要素の数"], vec![], vec![]);
    assert!(ok(&ledger).iter().any(|e| e.contains("attached-values は")));
    set(&mut ledger, "external", sample, &[" "], vec![], vec![]);
    assert!(
        ok(&ledger)
            .iter()
            .any(|e| e.contains("attached-values が空"))
    );
    // 受け取った図面は種類と件数だけを書き、読んだだけでは段を上げない
    let received = |ledger: &mut Ledger, kind: &str, count| {
        read(ledger).received = vec![Received {
            kind: kind.into(),
            count,
        }];
    };
    set(&mut ledger, "synthetic", &[], &[], vec![], vec![]);
    received(&mut ledger, "R2000 の DXF", 3);
    assert_eq!(ok(&ledger), Vec::<String>::new());
    set(
        &mut ledger,
        "external",
        &[],
        &["印刷に書かれた寸法"],
        vec![],
        vec![],
    );
    assert_eq!(ok(&ledger), Vec::<String>::new());
    received(&mut ledger, " ", 0);
    let errors = ok(&ledger);
    for expected in ["received[0]: kind が空", "received[0]: count"] {
        assert!(errors.iter().any(|e| e.contains(expected)), "{errors:?}");
    }
}

#[test]
fn counts_only_readers_of_the_written_file_for_writing() {
    let (dir, mut ledger, decisions) = fixture();
    let errors = |ledger: &Ledger| {
        let found = ledger.formats["dxf"].write.as_ref().unwrap();
        dir.write(
            "docs/formats/dxf.md",
            &document(&format!(
                "| `dxf` | 読み | `synthetic` |\n| `dxf` | 書き | `{}` |\n",
                found.level
            )),
        );
        check(ledger, &dir.0, &decisions)
    };
    let write = |level: &str, samples: Vec<String>, received, attached_values, implementations| {
        Verification {
            level: level.into(),
            scope: "書いて読ませた".into(),
            checked: "2026-11-03".into(),
            public_samples: samples,
            received,
            attached_values,
            implementations,
            software: vec![],
        }
    };
    let ezdxf = || {
        vec![Tool {
            name: "ezdxf".into(),
            version: "1.4.2".into(),
        }]
    };
    let f = ledger.formats.get_mut("dxf").unwrap();
    f.write = Some(write("external", vec![], vec![], vec![], ezdxf()));
    assert_eq!(errors(&ledger), Vec::<String>::new());
    // 書き戻したサンプルを自前で読み直しただけでは、synthetic のまま
    for (level, samples, received, attached) in [
        (
            "external",
            vec!["samples/dxf/b.dxf".to_string()],
            vec![],
            vec![],
        ),
        (
            "synthetic",
            vec![],
            vec![Received {
                kind: "DXF".into(),
                count: 1,
            }],
            vec![],
        ),
        ("external", vec![], vec![], vec!["書いた値".to_string()]),
    ] {
        ledger.formats.get_mut("dxf").unwrap().write =
            Some(write(level, samples, received, attached, vec![]));
        let found = errors(&ledger);
        assert!(
            found.iter().any(|e| e.contains("読みの根拠")),
            "{level}: {found:?}"
        );
    }
}

#[test]
fn checks_the_section_of_each_document_against_the_ledger() {
    let (dir, mut ledger, decisions) = fixture();
    let errors = |dir: &TempDir, ledger: &Ledger, text: &str| {
        dir.write("docs/formats/dxf.md", text);
        check(ledger, &dir.0, &decisions)
    };
    assert!(
        errors(&dir, &ledger, "# 文書\n\n## 確かめた度合いの考え方\n")
            .iter()
            .any(|e| e.contains("節（`## 確かめた度合い`）がない"))
    );
    for (rows, expected) in [
        ("", "`dxf` の読みの `synthetic` がない"),
        ("| `dxf` | 読み | `external` |\n", "が台帳と合わない"),
        (
            "| `dxf` | 読み | `synthetic` |\n| `dxf` | 書き | `synthetic` |\n",
            "`dxf` の書きの `synthetic` が台帳と合わない",
        ),
        (
            "| `dxf` | 読み | `synthetic` |\n| `dwg` | 読み | `synthetic` |\n",
            "`dwg` の読み",
        ),
        (
            "| dxf | 読み | synthetic |\n",
            "`dxf` の読みの `synthetic` がない",
        ),
        // 形の崩れた行は、読み飛ばさずに落とす
        (
            "| `dxf` | 読み | `synthetic` |\n| `dxf` | 書き | synthetic |\n",
            "`| `dxf` | 書き | synthetic |` の形が違う",
        ),
        (
            "| `dxf` | 読み | `synthetic` |\n| `dxf` | 読む | `synthetic` |\n",
            "の形が違う",
        ),
    ] {
        let found = errors(&dir, &ledger, &document(rows));
        assert!(
            found.iter().any(|e| e.contains(expected)),
            "{rows}: {found:?}"
        );
    }
    // 2 つの形式が同じ文書を持てば、表に両方を並べる
    let mut other: Format = toml::from_str(
        r#"
name = "DWG"
family = "acad"
domains = ["drawing"]
specs = []
rights = "public"
rights-note = "テスト用の根拠"
place = "core"
basis = ["inferred"]
docs = ["docs/formats/dxf.md"]
write = { level = "synthetic", scope = "範囲", checked = "2026-11-03" }
"#,
    )
    .unwrap();
    ledger.formats.insert("dwg".into(), other);
    assert!(
        errors(&dir, &ledger, &document("| `dxf` | 読み | `synthetic` |\n"))
            .iter()
            .any(|e| e.contains("`dwg` の書きの `synthetic` がない"))
    );
    assert_eq!(
        errors(
            &dir,
            &ledger,
            &document("| `dxf` | 読み | `synthetic` |\n| `dwg` | 書き | `synthetic` |\n")
        ),
        Vec::<String>::new()
    );
    // 段を持たない形式の文書には、節が要らない。節があれば、表の行も台帳に合わせる
    other = ledger.formats.remove("dwg").unwrap();
    other.write = None;
    other.docs = vec![];
    ledger.formats.insert("dwg".into(), other);
    ledger.formats.get_mut("dxf").unwrap().read = None;
    assert_eq!(errors(&dir, &ledger, "# 文書\n"), Vec::<String>::new());
    assert_eq!(errors(&dir, &ledger, &document("")), Vec::<String>::new());
    assert!(
        errors(&dir, &ledger, &document("| `dxf` | 読み | `synthetic` |\n"))
            .iter()
            .any(|e| e.contains("`dxf` の読みの `synthetic` が台帳と合わない"))
    );
    // あるのに読めない文書は、黙って飛ばさない
    std::fs::write(dir.0.join("docs/formats/dxf.md"), [0xff, 0xfe]).unwrap();
    assert!(
        check(&ledger, &dir.0, &decisions)
            .iter()
            .any(|e| e.contains("dxf.md: 確かめた度合いの節を照らす文書を読めない"))
    );
}

#[test]
fn reads_rows_only_from_the_section() {
    let (rows, malformed) = section_rows(&document(
        "| `dxf` | 読み | `synthetic` |\n| 形式 | 向き | 段 |\n| :-- | --- | --: |\n",
    ))
    .unwrap();
    assert_eq!(
        rows.into_iter().collect::<Vec<_>>(),
        vec![(
            "dxf".to_string(),
            "読み".to_string(),
            "synthetic".to_string()
        )]
    );
    assert_eq!(malformed, Vec::<String>::new());
    let (rows, malformed) =
        section_rows("## 確かめた度合い\n\n| `` | 読み | `synthetic` |\n| `dxf` | 読み |\n")
            .unwrap();
    assert!(rows.is_empty());
    assert_eq!(
        malformed,
        ["| `` | 読み | `synthetic` |", "| `dxf` | 読み |"]
    );
    assert!(section_rows("## 確かめた度合い\n").unwrap().0.is_empty());
    assert!(section_rows("### 確かめた度合い\n| `dxf` | 読み | `synthetic` |\n").is_none());
}

#[test]
fn validates_dates_without_using_the_current_clock() {
    for date in ["2000-02-29", "2024-02-29", "2026-12-31"] {
        assert!(is_date(date));
    }
    for date in [
        "1900-02-29",
        "2026-04-31",
        "2026-00-01",
        "2026-13-01",
        "2026-01-00",
        "0000-01-01",
        "2026-1-01",
        "２０２６-01-01",
        "2026-10-25T00:00:00Z",
    ] {
        assert!(!is_date(date), "{date}");
    }
}

#[test]
fn rejects_paths_outside_format_documents_and_duplicate_decisions() {
    let (dir, _, _) = fixture();
    for doc in [
        "docs/formats/../decisions/0008-model.md",
        "/docs/formats/dxf.md",
        "docs\\formats\\dxf.md",
        "docs/formats/./dxf.md",
        "docs/formats/",
    ] {
        assert!(!is_doc(&dir.0, doc), "{doc}");
    }
    dir.write("docs/decisions/0008-other.md", "# 重複\n");
    assert!(decision_files(&dir.0).unwrap_err().contains("重なって"));
}

#[test]
fn requires_an_unconditional_ban_for_every_known_restricted_crate() {
    let (_, ledger, _) = fixture();
    for entry in [
        "\"restricted-reader\"",
        "{ crate = \"restricted-reader\", reason = \"理由\" }",
        "{ name = \"restricted-reader\", version = \"*\" }",
    ] {
        let deny = toml::from_str(&format!("[bans]\ndeny = [{entry}]")).unwrap();
        assert!(check_bans(&ledger, &deny).is_empty(), "{entry}");
    }
    for entry in [
        "",
        "\"other\"",
        "\"restricted-reader@1.0.0\"",
        "{ crate = \"restricted-reader\", wrappers = [\"allowed\"] }",
        "{ crate = \"restricted-reader\", deny-multiple-versions = true }",
        "{ name = \"restricted-reader\", version = \"1\" }",
        "{ crate = \"restricted-reader\", version = \"1\" }",
        "{ crate = \"restricted-reader\", unknown = true }",
    ] {
        let deny = toml::from_str(&format!("[bans]\ndeny = [{entry}]")).unwrap();
        assert_eq!(check_bans(&ledger, &deny).len(), 1, "{entry}");
    }
    assert_eq!(check_bans(&ledger, &toml::from_str("").unwrap()).len(), 1);
}

fn packages(source: &str) -> Value {
    zumen_json::parse(source.as_bytes()).unwrap()
}

#[test]
fn checks_all_packages_and_rejects_malformed_declarations() {
    let (_, ledger, _) = fixture();
    assert!(check_packages(&ledger, &packages(r#"{"packages":[{"name":"app","metadata":{"zumen":{"formats":["dxf"]}}},{"name":"other","metadata":{"tool":{}}}]}"#)).is_empty());
    for (declaration, expected) in [
        (r#"["missing"]"#, "台帳にない"),
        (r#"["restricted"]"#, "public の core"),
        (r#""dxf""#, "文字列の配列"),
        (r#"[12]"#, "文字列"),
        (r#"["dxf","dxf"]"#, "重なって"),
    ] {
        let input = packages(&format!(
            r#"{{"packages":[{{"name":"transitive","metadata":{{"zumen":{{"formats":{declaration}}}}}}}]}}"#
        ));
        assert!(
            check_packages(&ledger, &input)
                .iter()
                .any(|e| e.contains(expected)),
            "{declaration}"
        );
    }
    assert!(
        !check_packages(
            &ledger,
            &packages(r#"{"packages":[{"name":"bad","metadata":{"zumen":null}}]}"#)
        )
        .is_empty()
    );
    assert!(
        !check_packages(
            &ledger,
            &packages(r#"{"packages":[{"name":"restricted-reader","metadata":null}]}"#)
        )
        .is_empty()
    );
    assert!(!check_packages(&ledger, &packages(r#"{}"#)).is_empty());
    // 実装する形式は、確かめた度合いを持つ（決定 0254）
    let mut ledger = ledger;
    ledger.formats.get_mut("dxf").unwrap().read = None;
    let errors = check_packages(
        &ledger,
        &packages(r#"{"packages":[{"name":"app","metadata":{"zumen":{"formats":["dxf"]}}}]}"#),
    );
    assert!(
        errors.iter().any(|e| e.contains("確かめた度合いがない")),
        "{errors:?}"
    );
}

#[test]
fn building_model_does_not_reach_ifc() {
    let graph = |building: &str| {
        packages(&format!(
            r#"{{"packages":[{{"name":"zumen-building","dependencies":[{building}]}},{{"name":"zumen-base","dependencies":[]}},{{"name":"middle","dependencies":[{{"name":"zumen-step","kind":null}}]}},{{"name":"helper","dependencies":[{{"name":"zumen-ifc","kind":"dev"}}]}}]}}"#
        ))
    };
    assert!(check_layers(&graph(r#"{"name":"zumen-base","kind":null}"#)).is_empty());
    // 依存の依存の、テストだけの依存はたどらない
    assert!(check_layers(&graph(r#"{"name":"helper","kind":null}"#)).is_empty());
    let errors = check_layers(&graph(r#"{"name":"middle","kind":null}"#));
    assert_eq!(errors.len(), 1);
    assert!(
        errors[0].ends_with("zumen-building → middle → zumen-step"),
        "{errors:?}"
    );
    // 自分のテストだけの依存はたどる
    let errors = check_layers(&graph(r#"{"name":"zumen-ifc","kind":"dev"}"#));
    assert!(errors[0].contains("`zumen-ifc`"), "{errors:?}");
    assert!(!check_layers(&packages(r#"{}"#)).is_empty());
}

#[test]
fn renders_records_in_key_order_with_working_links() {
    let (_, ledger, decisions) = fixture();
    let output = render::render(&ledger, &decisions);
    assert!(output.find("### format-dxf").unwrap() < output.find("### format-restricted").unwrap());
    for expected in [
        "[dxf\\_1](#spec-dxf_1)",
        "[決定 0008](../decisions/0008-model.md)",
        "https://github.com/zumen-jp/zumen-legacy/issues/140",
        "../../docs/formats/dxf.md",
        "テスト用の根拠",
        "2024-02-29",
        "`unknown` | `none` |  |  |",
        "読み書きできる形式の一覧ではありません",
        "| 読み | 書き |",
        "`public` | `core` | `synthetic` |  |",
        "- 読みの確かめた度合い: `synthetic`（確認日: 2026-11-03）",
        "- 読みの確かめた範囲: テストの中で作ったファイル",
    ] {
        assert!(output.contains(expected), "{expected}");
    }
    assert_eq!(output, render::render(&ledger, &decisions));
}

#[test]
fn renders_the_evidence_of_each_direction() {
    let (_, mut ledger, decisions) = fixture();
    let f = ledger.formats.get_mut("dxf").unwrap();
    let read = f.read.as_mut().unwrap();
    read.level = "external".into();
    read.public_samples = vec!["samples/dwg/".into(), "https://example.com/a.dxf".into()];
    read.received = vec![Received {
        kind: "R2000 の DXF".into(),
        count: 3,
    }];
    read.attached_values = vec!["サンプルに添えられた要素の数".into()];
    f.write = Some(Verification {
        level: "application".into(),
        scope: "書き戻して開いた".into(),
        checked: "2026-11-02".into(),
        public_samples: vec![],
        received: vec![],
        attached_values: vec![],
        implementations: vec![Tool {
            name: "ezdxf".into(),
            version: "1.4.2".into(),
        }],
        software: vec![Tool {
            name: "AutoCAD".into(),
            version: "2027".into(),
        }],
    });
    let output = render::render(&ledger, &decisions);
    for expected in [
        "`public` | `core` | `external` | `application` |",
        "- 読みで使った公開サンプル: [samples/dwg/](../../samples/dwg/)・[https://example.com/a.dxf](https://example.com/a.dxf)",
        "- 読みで使った受け取った図面: R2000 の DXF 3 件",
        "- 読みで照らした、ファイルに添えられた値: サンプルに添えられた要素の数",
        "- 書きの確かめた度合い: `application`（確認日: 2026-11-02）",
        "- 書きの確かめた範囲: 書き戻して開いた",
        "- 書きで照らした外部の実装: ezdxf 1.4.2",
        "- 書きで開いたソフト: AutoCAD 2027",
    ] {
        assert!(output.contains(expected), "{expected}\n{output}");
    }
    assert!(
        output.find("- 読みの確かめた度合い").unwrap()
            < output.find("- 書きの確かめた度合い").unwrap()
    );
}

#[test]
fn cargo_metadata_includes_optional_target_build_dev_and_transitive_dependencies() {
    let dir = TempDir::new();
    dir.write(
        "Cargo.toml",
        "[workspace]\nmembers = [\"app\"]\nexclude = [\"deps/direct\", \"deps/optional\", \"deps/target\", \"deps/build\", \"deps/dev\", \"deps/transitive\"]\nresolver = \"3\"\n",
    );
    dir.write(
        "app/Cargo.toml",
        r#"
[package]
name = "app"
version = "0.0.0"
edition = "2024"
[dependencies]
direct = { path = "../deps/direct" }
optional = { path = "../deps/optional", optional = true }
[target.'cfg(target_os = "none")'.dependencies]
target = { path = "../deps/target" }
[build-dependencies]
build = { path = "../deps/build" }
[dev-dependencies]
dev = { path = "../deps/dev" }
"#,
    );
    dir.write("app/src/lib.rs", "");
    for name in ["direct", "optional", "target", "build", "dev", "transitive"] {
        let extra = if name == "direct" {
            "[dependencies]\ntransitive = { path = \"../transitive\" }\n"
        } else {
            ""
        };
        dir.write(&format!("deps/{name}/Cargo.toml"), &format!("[package]\nname = \"{name}\"\nversion = \"0.0.0\"\n[package.metadata.zumen]\nformats = [\"restricted\"]\n{extra}"));
        dir.write(&format!("deps/{name}/src/lib.rs"), "");
    }
    let output = Command::new(env!("CARGO"))
        .args(["generate-lockfile", "--offline"])
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata = metadata(&dir.0).unwrap();
    let Some(Value::Array(members)) = metadata.pointer("/workspace_members") else {
        panic!("workspace_members がない")
    };
    assert_eq!(members.len(), 1);
    let (_, ledger, _) = fixture();
    let errors = check_packages(&ledger, &metadata);
    assert_eq!(errors.len(), 6, "{errors:?}");
    for name in ["direct", "optional", "target", "build", "dev", "transitive"] {
        assert!(
            errors
                .iter()
                .any(|e| e.contains(&format!("crate `{name}`"))),
            "{errors:?}"
        );
    }
}
