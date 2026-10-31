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
    decisions: Vec<String>,
    #[serde(default)]
    legacy_decisions: Vec<String>,
    #[serde(default)]
    issues: Vec<String>,
    #[serde(default)]
    docs: Vec<String>,
    notes: Option<String>,
}

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

/// 台帳の決定番号と実在するファイル。生成するリンクにも同じ一覧を使う。
type Decisions = BTreeMap<String, String>;

pub fn generate(root: &Path) -> Result<String, String> {
    let read = |path: &str| {
        fs::read_to_string(root.join(path)).map_err(|e| format!("{path} を読めない: {e}"))
    };
    let ledger: Ledger = toml::from_str(&read("formats/ledger.toml")?)
        .map_err(|e| format!("formats/ledger.toml: {e}"))?;
    let deny: toml::Value =
        toml::from_str(&read("deny.toml")?).map_err(|e| format!("deny.toml: {e}"))?;
    let decisions = decision_files(root)?;
    let mut errors = check(&ledger, root, &decisions);
    errors.extend(check_bans(&ledger, &deny));
    if errors.is_empty() {
        let metadata = metadata(root)?;
        errors.extend(check_packages(&ledger, &metadata));
        errors.extend(check_layers(&metadata));
    }
    if errors.is_empty() {
        Ok(render::render(&ledger, &decisions))
    } else {
        Err(format!(
            "形式の権利と層の向きの検査で問題がある:\n- {}",
            errors.join("\n- ")
        ))
    }
}

fn decision_files(root: &Path) -> Result<Decisions, String> {
    let mut decisions = Decisions::new();
    for entry in fs::read_dir(root.join("docs/decisions"))
        .map_err(|e| format!("決定の記録を読めない: {e}"))?
    {
        let entry = entry.map_err(|e| format!("決定の記録を読めない: {e}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((number, _)) = name.split_once('-') else {
            continue;
        };
        if !is_decision(number) || !name.ends_with(".md") || !entry.path().is_file() {
            continue;
        }
        if decisions.insert(number.to_string(), name.clone()).is_some() {
            return Err(format!("決定 {number} のファイルが重なっている"));
        }
    }
    Ok(decisions)
}

/// Cargo の全パッケージを読む。版を固定し、任意の機能や別環境向けの依存を省かない。
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

fn check(ledger: &Ledger, root: &Path, decisions: &Decisions) -> Vec<String> {
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
            ("decisions", &format.decisions),
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
        for number in &format.decisions {
            if !is_decision(number) || !decisions.contains_key(number) {
                errors.push(format!("{at}: 決定 `{number}` のファイルがない"));
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
    }
    errors
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

/// 例外や版の指定のない禁止だけを数える。cargo-deny の Package Specs・bans.deny に従う。
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
        .map(|name| format!("deny.toml: `{name}` を [bans].deny で例外・版の制限なく禁止する"))
        .collect()
}

/// 層の向き（決定 0008・0207）。左の crate は、右の crate に、依存をたどっても依存しない
const LAYERS: &[(&str, &[&str])] = &[("zumen-building", &["zumen-ifc", "zumen-step"])];

/// [`LAYERS`] の crate から依存をたどり、依存してはならない crate に届けば、その道筋を返す。左の crate では
/// 試験だけの依存（dev）もたどる
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
                _ => {}
            }
        }
    }
    errors.sort();
    errors
}
