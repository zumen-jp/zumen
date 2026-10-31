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
                Err(e) => panic!("試験用のフォルダーを作れない: {e}"),
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
title = "試験用の仕様"
publisher = "試験用の公開元"
access = "public"
url = "https://example.com/spec"
checked = "2024-02-29"

[formats.dxf]
name = "DXF"
family = "acad"
domains = ["drawing"]
specs = ["dxf_1"]
rights = "public"
rights-note = "試験用の根拠"
place = "core"
basis = ["spec"]
decisions = ["0008"]
legacy-decisions = ["0003"]
issues = ["zumen-jp/zumen-legacy#140"]
docs = ["docs/formats/dxf.md"]

[formats.restricted]
name = "制限のある形式"
family = "restricted"
domains = ["building"]
specs = []
rights = "unknown"
rights-note = "試験用の未確認の条件"
place = "none"
known-crates = ["restricted-reader"]
"#;

fn fixture() -> (TempDir, Ledger, Decisions) {
    let dir = TempDir::new();
    dir.write("docs/decisions/0008-model.md", "# 試験用の決定\n");
    dir.write("docs/formats/dxf.md", "# 試験用の文書\n");
    let decisions = decision_files(&dir.0).unwrap();
    (dir, toml::from_str(LEDGER).unwrap(), decisions)
}

#[test]
fn accepts_public_core_and_restricted_none() {
    let (dir, ledger, decisions) = fixture();
    assert_eq!(check(&ledger, &dir.0, &decisions), Vec::<String>::new());
}

#[test]
fn rejects_missing_required_fields_unknown_fields_and_wrong_types() {
    for (from, to) in [
        ("publisher = \"試験用の公開元\"", ""),
        ("rights-note = \"試験用の根拠\"", ""),
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
}

#[test]
fn building_model_does_not_reach_ifc() {
    let graph = |building: &str| {
        packages(&format!(
            r#"{{"packages":[{{"name":"zumen-building","dependencies":[{building}]}},{{"name":"zumen-base","dependencies":[]}},{{"name":"middle","dependencies":[{{"name":"zumen-step","kind":null}}]}},{{"name":"helper","dependencies":[{{"name":"zumen-ifc","kind":"dev"}}]}}]}}"#
        ))
    };
    assert!(check_layers(&graph(r#"{"name":"zumen-base","kind":null}"#)).is_empty());
    // 依存の依存の、試験だけの依存はたどらない
    assert!(check_layers(&graph(r#"{"name":"helper","kind":null}"#)).is_empty());
    let errors = check_layers(&graph(r#"{"name":"middle","kind":null}"#));
    assert_eq!(errors.len(), 1);
    assert!(
        errors[0].ends_with("zumen-building → middle → zumen-step"),
        "{errors:?}"
    );
    // 自分の試験だけの依存はたどる
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
        "試験用の根拠",
        "2024-02-29",
        "`unknown` | `none`",
        "読み書きできる形式の一覧ではありません",
    ] {
        assert!(output.contains(expected), "{expected}");
    }
    assert_eq!(output, render::render(&ledger, &decisions));
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
