//! 形式の権利の台帳を検査し、一覧を生成する（決定 0017）。

mod render;
#[cfg(test)]
pub(crate) mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::Command;

use serde::Deserialize;
use zumen_json::Value;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ledger {
    specs: BTreeMap<String, Spec>,
    formats: BTreeMap<String, Format>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    title: String,
    publisher: String,
    version: Option<String>,
    access: String,
    url: String,
    checked: String,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Format {
    name: String,
    family: String,
    domains: Vec<String>,
    specs: Vec<String>,
    rights: String,
    rights_note: String,
    place: String,
    repository: Option<String>,
    #[serde(default)]
    basis: Vec<String>,
    #[serde(default)]
    evidence: Vec<Evidence>,
    #[serde(default)]
    references: Vec<Reference>,
    #[serde(default)]
    verify: Vec<String>,
    #[serde(default)]
    samples: Vec<String>,
    #[serde(default)]
    known_crates: Vec<String>,
    #[serde(default)]
    legacy_decisions: Vec<String>,
    #[serde(default)]
    issues: Vec<String>,
    #[serde(default)]
    docs: Vec<String>,
    notes: Option<String>,
    /// 読みの確かめた度合い（決定 0254・0255）。読まない形式は持たない
    read: Option<Verification>,
    /// 書きの確かめた度合い。書かない形式は持たない
    write: Option<Verification>,
}

/// 読みか書きの確かめた度合いと、その根拠（決定 0254・0255）
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Verification {
    /// 段。根拠のうちもっとも強い種類で決まる
    level: String,
    /// 確かめた範囲と方法
    scope: String,
    checked: String,
    /// 読んだ公開サンプル。`samples/` の中の道筋（`/` で終われば、そのフォルダー）か URL。読みだけ
    #[serde(default)]
    public_samples: Vec<String>,
    /// 読んだ受け取った図面の種類と件数。読みだけ
    #[serde(default)]
    received: Vec<Received>,
    /// 読んだ公開サンプルか受け取った図面に添えられていて、読んだ値と照らした値。読みだけ
    #[serde(default)]
    attached_values: Vec<String>,
    /// 自前で作っていないファイルを読み、読んだ値を照らした外部の実装。書きでは、書いたファイルを読み、読んだ値が
    /// 書いた値と合った実装。読んだファイルの出どころは確かめられないので、scope に書く
    #[serde(default)]
    implementations: Vec<Tool>,
    /// 開いて確かめた、その形式の基準になるソフト
    #[serde(default)]
    software: Vec<Tool>,
}

/// 確かめるのに使った実装かソフトと、そのバージョン
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tool {
    name: String,
    version: String,
}

/// 受け取った図面。ファイル名・顧客の名前・中身は書かず、種類と件数だけを書く（決定 0254）
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Received {
    kind: String,
    count: u64,
}

/// 確かめた度合いの段（決定 0254）。弱いものから並べる
const LEVELS: &[&str] = &["synthetic", "external", "application"];

/// 形式の文書の、確かめた度合いの節の見出し
const SECTION: &str = "## 確かめた度合い";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    title: String,
    url: String,
    checked: String,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    name: String,
    license: String,
    url: String,
}

/// 形式の台帳と参照先、依存の条件を確かめ、形式と権利の一覧を生成する。
pub fn generate(root: &Path) -> Result<String, String> {
    let read = |path: &str| {
        fs::read_to_string(root.join(path)).map_err(|e| format!("{path} を読めない: {e}"))
    };
    let ledger: Ledger = toml::from_str(&read("formats/ledger.toml")?)
        .map_err(|e| format!("formats/ledger.toml: {e}"))?;
    let deny: toml::Value =
        toml::from_str(&read("deny.toml")?).map_err(|e| format!("deny.toml: {e}"))?;
    let samples = crate::samples::paths(root)?;
    let mut errors = check(&ledger, root, &samples);
    errors.extend(check_bans(&ledger, &deny));
    if errors.is_empty() {
        let metadata = metadata(root)?;
        errors.extend(check_packages(&ledger, &metadata));
        errors.extend(check_layers(&metadata));
    }
    if errors.is_empty() {
        Ok(render::render(&ledger))
    } else {
        Err(format!(
            "形式の権利と層の向きの検査で問題がある:\n- {}",
            errors.join("\n- ")
        ))
    }
}

/// Cargo の全パッケージを読む。バージョンを固定し、任意の機能や別環境向けの依存を省かない。
/// https://doc.rust-lang.org/cargo/commands/cargo-metadata.html の Output・Feature Selection。
fn metadata(root: &Path) -> Result<Value, String> {
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--format-version",
            "1",
            "--all-features",
            "--locked",
        ])
        .current_dir(root)
        .output()
        .map_err(|e| format!("cargo metadata を起動できない: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata が失敗した:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    zumen_json::parse(&output.stdout)
        .map_err(|e| format!("cargo metadata の JSON を読めない（位置 {}）", e.offset))
}

/// `samples` は、公開サンプルの台帳にあるファイルの `samples/` からの道筋
fn check(ledger: &Ledger, root: &Path, samples: &BTreeSet<String>) -> Vec<String> {
    let mut errors = Vec::new();
    for (id, spec) in &ledger.specs {
        let at = format!("specs.{id}");
        if !is_key(id, true) {
            errors.push(format!("{at}: ID は英小文字・数字・_ にする"));
        }
        for (key, value) in [("title", &spec.title), ("publisher", &spec.publisher)] {
            nonempty(&mut errors, &at, key, value);
        }
        optional_text(&mut errors, &at, "version", spec.version.as_deref());
        optional_text(&mut errors, &at, "note", spec.note.as_deref());
        choice(
            &mut errors,
            &at,
            "access",
            &spec.access,
            &["public", "members", "paid"],
        );
        source(&mut errors, &at, &spec.url, &spec.checked);
    }
    for (id, format) in &ledger.formats {
        let at = format!("formats.{id}");
        if !is_key(id, false) {
            errors.push(format!("{at}: キーは英小文字・数字にする"));
        }
        for (key, value) in [
            ("name", &format.name),
            ("family", &format.family),
            ("rights-note", &format.rights_note),
        ] {
            nonempty(&mut errors, &at, key, value);
        }
        for (key, values) in [
            ("domains", &format.domains),
            ("specs", &format.specs),
            ("basis", &format.basis),
            ("known-crates", &format.known_crates),
            ("legacy-decisions", &format.legacy_decisions),
            ("issues", &format.issues),
            ("docs", &format.docs),
            ("verify", &format.verify),
            ("samples", &format.samples),
        ] {
            let mut seen = BTreeSet::new();
            for value in values {
                nonempty(&mut errors, &at, key, value);
                if !seen.insert(value) {
                    errors.push(format!("{at}: {key} の `{value}` が重なっている"));
                }
            }
        }
        if format.domains.is_empty() {
            errors.push(format!("{at}: domains を 1 つ以上指定する"));
        }
        for domain in &format.domains {
            choice(
                &mut errors,
                &at,
                "domains",
                domain,
                &[
                    "drawing",
                    "building",
                    "civil",
                    "geo",
                    "pointcloud",
                    "delivery",
                    "review",
                ],
            );
        }
        for spec in &format.specs {
            if !ledger.specs.contains_key(spec) {
                errors.push(format!("{at}: 仕様 `{spec}` が台帳にない"));
            }
        }
        choice(
            &mut errors,
            &at,
            "rights",
            &format.rights,
            &["public", "permission", "unknown"],
        );
        choice(
            &mut errors,
            &at,
            "place",
            &format.place,
            &["core", "plugin", "none"],
        );
        if format.place == "core" && format.rights != "public" {
            errors.push(format!("{at}: core に置ける権利は public だけ"));
        }
        if format.place == "plugin" && format.repository.is_none() {
            errors.push(format!("{at}: plugin には repository が要る"));
        }
        optional_text(&mut errors, &at, "repository", format.repository.as_deref());
        optional_text(&mut errors, &at, "notes", format.notes.as_deref());
        if format.place != "none" && format.basis.is_empty() {
            errors.push(format!("{at}: 実装する形式には basis が要る"));
        }
        for basis in &format.basis {
            choice(
                &mut errors,
                &at,
                "basis",
                basis,
                &["spec", "inferred", "reference"],
            );
            if basis == "spec" && format.specs.is_empty() {
                errors.push(format!("{at}: basis の spec には仕様の参照が要る"));
            }
            if basis == "reference" && format.references.is_empty() {
                errors.push(format!("{at}: basis の reference には references が要る"));
            }
        }
        for (i, reference) in format.references.iter().enumerate() {
            let at = format!("{at}.references[{i}]");
            nonempty(&mut errors, &at, "name", &reference.name);
            nonempty(&mut errors, &at, "license", &reference.license);
            url(&mut errors, &at, &reference.url);
        }
        for (i, evidence) in format.evidence.iter().enumerate() {
            let at = format!("{at}.evidence[{i}]");
            nonempty(&mut errors, &at, "title", &evidence.title);
            source(&mut errors, &at, &evidence.url, &evidence.checked);
            optional_text(&mut errors, &at, "note", evidence.note.as_deref());
        }
        for name in &format.known_crates {
            if !is_crate(name) {
                errors.push(format!(
                    "{at}: known-crates の `{name}` は crate の名前だけにする"
                ));
            }
        }
        for number in &format.legacy_decisions {
            if !is_decision(number) {
                errors.push(format!("{at}: 旧版の決定 `{number}` は 4 桁の番号にする"));
            }
        }
        for issue in &format.issues {
            if issue_url(issue).is_none() {
                errors.push(format!(
                    "{at}: issue `{issue}` は所有者/リポジトリ#番号にする"
                ));
            }
        }
        for doc in &format.docs {
            if !is_doc(root, doc) {
                errors.push(format!(
                    "{at}: docs の `{doc}` は docs/formats/ 内の実在する Markdown にする"
                ));
            }
        }
        for (direction, found) in [("read", &format.read), ("write", &format.write)] {
            let Some(found) = found else {
                continue;
            };
            let at = format!("{at}.{direction}");
            if format.place != "core" {
                errors.push(format!("{at}: 確かめた度合いは core の形式にだけ書く"));
            }
            verification(&mut errors, &at, direction == "write", found, samples);
        }
        if (format.read.is_some() || format.write.is_some()) && format.docs.is_empty() {
            errors.push(format!(
                "{at}: 確かめた度合いを持つ形式には、節を置く docs が要る"
            ));
        }
    }
    errors.extend(check_sections(ledger, root));
    errors
}

/// 確かめた度合いの段と根拠。段は、根拠のうちもっとも強い種類で決まる（決定 0254・0255）。公開サンプルと
/// 受け取った図面は、読んだだけでは段を上げない。書きの根拠は、書いたファイルを読ませた外部の実装か、開いたソフトだけ
fn verification(
    errors: &mut Vec<String>,
    at: &str,
    write: bool,
    found: &Verification,
    samples: &BTreeSet<String>,
) {
    choice(errors, at, "level", &found.level, LEVELS);
    let read_files = !(found.public_samples.is_empty() && found.received.is_empty());
    if write && (read_files || !found.attached_values.is_empty()) {
        errors.push(format!(
            "{at}: public-samples・received・attached-values は読みの根拠。書きには、書いたファイルを読ませた implementations か、開いた software を書く"
        ));
    }
    for (i, received) in found.received.iter().enumerate() {
        let at = format!("{at}.received[{i}]");
        nonempty(errors, &at, "kind", &received.kind);
        if received.count == 0 {
            errors.push(format!("{at}: count は 1 以上にする"));
        }
    }
    for value in &found.attached_values {
        nonempty(errors, at, "attached-values", value);
    }
    if !write && !found.attached_values.is_empty() && !read_files {
        errors.push(format!(
            "{at}: attached-values は、読んだ public-samples か received に添えられた値。どちらかを書く"
        ));
    }
    nonempty(errors, at, "scope", &found.scope);
    if !is_date(&found.checked) {
        errors.push(format!("{at}: checked は実在する日付 YYYY-MM-DD にする"));
    }
    let mut seen = BTreeSet::new();
    for sample in &found.public_samples {
        if !seen.insert(sample) {
            errors.push(format!("{at}: public-samples の `{sample}` が重なっている"));
        }
        if let Some(path) = sample.strip_prefix("samples/") {
            let listed = match path.strip_suffix('/') {
                Some(dir) => {
                    !dir.is_empty()
                        && samples
                            .iter()
                            .any(|p| p.strip_prefix(dir).is_some_and(|r| r.starts_with('/')))
                }
                None => samples.contains(path),
            };
            if !listed {
                errors.push(format!(
                    "{at}: public-samples の `{sample}` が samples/ledger.toml にない"
                ));
            }
        } else if !is_url(sample) {
            errors.push(format!(
                "{at}: public-samples の `{sample}` は samples/ の中の道筋か、HTTP または HTTPS の URL にする"
            ));
        }
    }
    for (key, tools) in [
        ("implementations", &found.implementations),
        ("software", &found.software),
    ] {
        for (i, tool) in tools.iter().enumerate() {
            let at = format!("{at}.{key}[{i}]");
            nonempty(errors, &at, "name", &tool.name);
            nonempty(errors, &at, "version", &tool.version);
        }
    }
    let expected = if !found.software.is_empty() {
        "application"
    } else if !found.implementations.is_empty() || (!write && !found.attached_values.is_empty()) {
        "external"
    } else {
        "synthetic"
    };
    if LEVELS.contains(&found.level.as_str()) && found.level != expected {
        errors.push(format!(
            "{at}: level の `{}` が根拠と合わない。software があれば application、implementations（読みでは attached-values も）があれば external、どれもなければ synthetic にする",
            found.level
        ));
    }
}

/// 形式の文書の、確かめた度合いの節の表を、台帳の段と照らす。表の行は `| \`キー\` | 読み | \`段\` |` の形で、
/// 文書を docs に持つ形式の、段を持つ向きを過不足なく並べる。段を持たない形式の文書も、節があれば照らす
fn check_sections(ledger: &Ledger, root: &Path) -> Vec<String> {
    let mut expected: BTreeMap<&str, BTreeSet<Row>> = BTreeMap::new();
    for (id, format) in &ledger.formats {
        for doc in &format.docs {
            let rows = expected.entry(doc).or_default();
            for (direction, found) in [("読み", &format.read), ("書き", &format.write)] {
                if let Some(found) = found {
                    rows.insert((id.clone(), direction.to_string(), found.level.clone()));
                }
            }
        }
    }
    let mut errors = Vec::new();
    for (doc, rows) in expected {
        let text = match fs::read_to_string(root.join(doc)) {
            Ok(text) => text,
            // 文書がないことは、docs の検査が報告する
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                errors.push(format!(
                    "{doc}: 確かめた度合いの節を照らす文書を読めない: {e}"
                ));
                continue;
            }
        };
        let Some((found, malformed)) = section_rows(&text) else {
            if !rows.is_empty() {
                errors.push(format!("{doc}: 確かめた度合いの節（`{SECTION}`）がない"));
            }
            continue;
        };
        for line in malformed {
            errors.push(format!(
                "{doc}: 確かめた度合いの表の行 `{line}` の形が違う。表の行は | `キー` | 読み | `段` | の形にする"
            ));
        }
        for (id, direction, level) in rows.difference(&found) {
            errors.push(format!(
                "{doc}: 確かめた度合いの表に `{id}` の{direction}の `{level}` がない"
            ));
        }
        for (id, direction, level) in found.difference(&rows) {
            errors.push(format!(
                "{doc}: 確かめた度合いの表の `{id}` の{direction}の `{level}` が台帳と合わない"
            ));
        }
    }
    errors
}

/// 確かめた度合いの表の行（形式のキー・向き・段）
type Row = (String, String, String);

/// 確かめた度合いの節の表の行と、見出しと区切りのほかで表の行の形に合わない行。節がなければ `None`
fn section_rows(text: &str) -> Option<(BTreeSet<Row>, Vec<String>)> {
    let mut lines = text.lines().skip_while(|line| line.trim_end() != SECTION);
    lines.next()?;
    let code = |cell: &str| {
        cell.strip_prefix('`')
            .and_then(|c| c.strip_suffix('`'))
            .filter(|c| !c.is_empty())
            .map(str::to_string)
    };
    let mut rows = BTreeSet::new();
    let mut malformed = Vec::new();
    for line in lines.take_while(|line| !line.starts_with("# ") && !line.starts_with("## ")) {
        let line = line.trim();
        let Some(inner) = line.strip_prefix('|') else {
            continue;
        };
        let cells: Vec<_> = inner
            .strip_suffix('|')
            .unwrap_or(inner)
            .split('|')
            .map(str::trim)
            .collect();
        let rule = |cell: &&str| !cell.is_empty() && cell.chars().all(|c| matches!(c, '-' | ':'));
        if cells == ["形式", "向き", "段"] || cells.iter().all(rule) {
            continue;
        }
        let row = match cells.as_slice() {
            [id, direction @ ("読み" | "書き"), level] => code(id)
                .zip(code(level))
                .map(|(id, level)| (id, direction.to_string(), level)),
            _ => None,
        };
        match row {
            Some(row) => {
                rows.insert(row);
            }
            None => malformed.push(line.to_string()),
        }
    }
    Some((rows, malformed))
}

fn nonempty(errors: &mut Vec<String>, at: &str, key: &str, value: &str) {
    if value.trim().is_empty() {
        errors.push(format!("{at}: {key} が空"));
    }
}

fn optional_text(errors: &mut Vec<String>, at: &str, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        nonempty(errors, at, key, value);
    }
}

fn choice(errors: &mut Vec<String>, at: &str, key: &str, value: &str, allowed: &[&str]) {
    if !allowed.contains(&value) {
        errors.push(format!("{at}: {key} の `{value}` は使えない"));
    }
}

fn is_key(value: &str, underscore: bool) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || (underscore && c == b'_'))
}

fn is_crate(value: &str) -> bool {
    value
        .bytes()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}

fn is_decision(value: &str) -> bool {
    value.len() == 4 && value.bytes().all(|c| c.is_ascii_digit()) && value != "0000"
}

fn url(errors: &mut Vec<String>, at: &str, value: &str) {
    if !is_url(value) {
        errors.push(format!(
            "{at}: url はホストのある HTTP または HTTPS の URL にする"
        ));
    }
}

/// ホストのある HTTP または HTTPS の URL か
pub(crate) fn is_url(value: &str) -> bool {
    let host = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
        .and_then(|rest| rest.split(['/', '?', '#']).next());
    host.is_some_and(|h| !h.is_empty() && !h.starts_with(':') && !h.contains('@'))
        && !value
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
}

fn source(errors: &mut Vec<String>, at: &str, value: &str, checked: &str) {
    url(errors, at, value);
    if !is_date(checked) {
        errors.push(format!("{at}: checked は実在する日付 YYYY-MM-DD にする"));
    }
}

/// 実在する日付 YYYY-MM-DD か
pub(crate) fn is_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes
            .iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    {
        return false;
    }
    let year: u32 = value[..4].parse().unwrap_or(0);
    let month: usize = value[5..7].parse().unwrap_or(0);
    let day: u32 = value[8..].parse().unwrap_or(0);
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = [
        0,
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    year > 0 && day > 0 && days.get(month).is_some_and(|max| day <= *max)
}

fn is_doc(root: &Path, value: &str) -> bool {
    if !value.starts_with("docs/formats/")
        || !value.ends_with(".md")
        || value.contains('\\')
        || value
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return false;
    }
    match (
        root.join(value).canonicalize(),
        root.join("docs/formats").canonicalize(),
    ) {
        (Ok(file), Ok(dir)) => file.starts_with(dir) && file.is_file(),
        _ => false,
    }
}

fn issue_url(value: &str) -> Option<String> {
    let (repo, number) = value.rsplit_once('#')?;
    let (owner, name) = repo.split_once('/')?;
    let valid = |s: &str| {
        !s.is_empty()
            && s != "."
            && s != ".."
            && s.bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
    };
    if !valid(owner)
        || !valid(name)
        || number.is_empty()
        || !number.bytes().all(|c| c.is_ascii_digit())
        || number.parse::<u64>().ok()? == 0
    {
        return None;
    }
    Some(format!("https://github.com/{repo}/issues/{number}"))
}

/// 例外やバージョンの指定のない禁止だけを数える。cargo-deny の Package Specs・bans.deny に従う。
/// https://embarkstudios.github.io/cargo-deny/checks/bans/cfg.html#deny
fn full_ban(value: &toml::Value, name: &str) -> bool {
    if value.as_str() == Some(name) {
        return true;
    }
    let Some(table) = value.as_table() else {
        return false;
    };
    let named = table.get("crate").and_then(toml::Value::as_str) == Some(name)
        || (table.get("name").and_then(toml::Value::as_str) == Some(name)
            && table.get("version").is_none_or(|v| v.as_str() == Some("*")));
    named
        && table.keys().all(|k| {
            [
                "crate",
                "name",
                "version",
                "reason",
                "use-instead",
                "wrappers",
                "deny-multiple-versions",
            ]
            .contains(&k.as_str())
        })
        && !(table.contains_key("crate")
            && (table.contains_key("name") || table.contains_key("version")))
        && table
            .get("wrappers")
            .is_none_or(|v| v.as_array().is_some_and(Vec::is_empty))
        && table
            .get("deny-multiple-versions")
            .is_none_or(|v| v.as_bool() == Some(false))
}

fn restricted_crates(ledger: &Ledger) -> BTreeSet<&str> {
    ledger
        .formats
        .values()
        .filter(|f| f.rights != "public")
        .flat_map(|f| f.known_crates.iter().map(String::as_str))
        .collect()
}

fn check_bans(ledger: &Ledger, deny: &toml::Value) -> Vec<String> {
    let bans = deny
        .get("bans")
        .and_then(|b| b.get("deny"))
        .and_then(toml::Value::as_array);
    restricted_crates(ledger)
        .into_iter()
        .filter(|name| bans.is_none_or(|b| !b.iter().any(|v| full_ban(v, name))))
        .map(|name| {
            format!("deny.toml: `{name}` を [bans].deny で例外・バージョンの制限なく禁止する")
        })
        .collect()
}

/// 層の向き（決定 0008・0207・0214）。左の crate は、右の crate に、依存をたどっても依存しない
const LAYERS: &[(&str, &[&str])] = &[
    ("zumen-building", &["zumen-ifc", "zumen-step"]),
    ("zumen-ids", &["zumen-ifc", "zumen-step"]),
    (
        "zumen-bcf",
        &[
            "zumen-acad",
            "zumen-building",
            "zumen-ifc",
            "zumen-jww",
            "zumen-step",
            "zumen-sxf",
        ],
    ),
    (
        "zumen-review",
        &[
            "zumen-acad",
            "zumen-ifc",
            "zumen-jww",
            "zumen-step",
            "zumen-sxf",
        ],
    ),
];

/// [`LAYERS`] の crate から依存をたどり、依存してはならない crate に届けば、その道筋を返す。左の crate では
/// テストだけの依存（dev）もたどる
fn check_layers(metadata: &Value) -> Vec<String> {
    let Some(Value::Array(packages)) = metadata.pointer("/packages") else {
        return vec!["cargo metadata: packages の配列がない".to_string()];
    };
    let mut dependencies: BTreeMap<&str, Vec<(&str, bool)>> = BTreeMap::new();
    for package in packages {
        let Some(Value::String(name)) = package.pointer("/name") else {
            continue;
        };
        let list = dependencies.entry(name.as_str()).or_default();
        if let Some(Value::Array(found)) = package.pointer("/dependencies") {
            for dependency in found {
                if let Some(Value::String(target)) = dependency.pointer("/name") {
                    let dev =
                        matches!(dependency.pointer("/kind"), Some(Value::String(k)) if k == "dev");
                    list.push((target.as_str(), dev));
                }
            }
        }
    }
    let mut errors = Vec::new();
    for (from, forbidden) in LAYERS {
        // 幅優先にたどり、最初に届いた道筋を報告する
        let mut came: BTreeMap<&str, &str> = BTreeMap::new();
        let mut queue = std::collections::VecDeque::from([*from]);
        while let Some(name) = queue.pop_front() {
            for (target, dev) in dependencies.get(name).into_iter().flatten() {
                if (*dev && name != *from) || *target == *from || came.contains_key(target) {
                    continue;
                }
                came.insert(target, name);
                queue.push_back(target);
            }
        }
        for banned in *forbidden {
            if !came.contains_key(banned) {
                continue;
            }
            let mut path = vec![*banned];
            while let Some(previous) = path.last().and_then(|n| came.get(n)) {
                path.push(previous);
                if *previous == *from {
                    break;
                }
            }
            path.reverse();
            errors.push(format!(
                "crate `{from}` は `{banned}` に依存してはならない（決定 0008・0207）: {}",
                path.join(" → ")
            ));
        }
    }
    errors
}

fn check_packages(ledger: &Ledger, metadata: &Value) -> Vec<String> {
    let Some(Value::Array(packages)) = metadata.pointer("/packages") else {
        return vec!["cargo metadata: packages の配列がない".to_string()];
    };
    let mut errors = Vec::new();
    let restricted = restricted_crates(ledger);
    for package in packages {
        let Some(Value::String(name)) = package.pointer("/name") else {
            errors.push("cargo metadata: パッケージの名前がない".to_string());
            continue;
        };
        if restricted.contains(name.as_str()) {
            errors.push(format!(
                "crate `{name}` は権利が public でない形式の既知の crate"
            ));
        }
        let Some(zumen) = package.pointer("/metadata/zumen") else {
            continue;
        };
        let Value::Object(zumen) = zumen else {
            errors.push(format!("crate `{name}`: metadata.zumen は表にする"));
            continue;
        };
        let Some(formats) = zumen.get("formats") else {
            continue;
        };
        let Value::Array(formats) = formats else {
            errors.push(format!(
                "crate `{name}`: metadata.zumen.formats は文字列の配列にする"
            ));
            continue;
        };
        let mut seen = BTreeSet::new();
        for value in formats {
            let Value::String(id) = value else {
                errors.push(format!("crate `{name}`: formats の項目は文字列にする"));
                continue;
            };
            if !seen.insert(id) {
                errors.push(format!("crate `{name}`: 形式 `{id}` が重なっている"));
            }
            match ledger.formats.get(id) {
                None => errors.push(format!("crate `{name}`: 形式 `{id}` が台帳にない")),
                Some(format) if format.place != "core" || format.rights != "public" => {
                    errors.push(format!(
                        "crate `{name}`: 形式 `{id}` は public の core ではない"
                    ));
                }
                Some(format) if format.read.is_none() && format.write.is_none() => {
                    errors.push(format!(
                        "crate `{name}`: 実装する形式 `{id}` に、台帳の read か write の確かめた度合いがない（決定 0254）"
                    ));
                }
                _ => {}
            }
        }
    }
    errors.sort();
    errors
}
