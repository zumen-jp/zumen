//! プロファイルを検査し、配る JSON を生成する（決定 0138）。
//!
//! 元は `profiles/<id>/<edition>/profile.toml`。同じ場所に `profile.json` を作る。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::fs;
use std::path::Path;

use serde::Deserialize;

// 読める書式の大版と、照合の種類と欄の一覧は、照合の crate から引く（決定 0173）
use zumen_profile::{CHECKS, FORMAT_MAJOR, WITHOUT_VOCABULARY};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Profile {
    format: String,
    id: String,
    edition: String,
    revision: u32,
    title: String,
    standard_code: Option<String>,
    license: String,
    extends: Option<Extends>,
    sources: BTreeMap<String, Source>,
    #[serde(default)]
    vocabularies: BTreeMap<String, Vocabulary>,
    #[serde(default)]
    rules: Vec<Rule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Extends {
    id: String,
    edition: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    title: Option<String>,
    publisher: Option<String>,
    version: Option<String>,
    url: Option<String>,
    checked: Option<String>,
    spec: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    document: String,
    section: Option<String>,
    item: Option<String>,
    basis: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Vocabulary {
    source: Reference,
    unit: Option<String>,
    values: Vec<toml::Value>,
}

#[derive(Debug, Deserialize)]
struct Rule {
    id: String,
    check: String,
    level: String,
    source: Reference,
    vocabulary: Option<String>,
    #[serde(flatten)]
    params: BTreeMap<String, toml::Value>,
}

/// `profiles/` のプロファイルを検査し、生成する JSON の置き場所と中身を返す。
pub fn generate(root: &Path) -> Result<Vec<(String, String)>, String> {
    let specs = ledger_specs(root)?;
    let mut outputs = Vec::new();
    let mut errors = Vec::new();
    for path in sources(root)? {
        let source =
            fs::read_to_string(root.join(&path)).map_err(|e| format!("{path} を読めない: {e}"))?;
        let profile: Profile = toml::from_str(&source).map_err(|e| format!("{path}: {e}"))?;
        let raw: toml::Table = toml::from_str(&source).map_err(|e| format!("{path}: {e}"))?;
        let expected = path
            .strip_prefix("profiles/")
            .and_then(|rest| rest.strip_suffix("/profile.toml"))
            .unwrap_or_default();
        let place = format!("{}/{}", profile.id, profile.edition);
        if expected != place {
            errors.push(format!(
                "{path}: 置き場所は profiles/{place}/profile.toml にする"
            ));
        }
        errors.extend(
            check(&profile, &specs)
                .into_iter()
                .map(|e| format!("{path}: {e}")),
        );
        let json = render_json(&toml::Value::Table(raw)).map_err(|e| format!("{path}: {e}"))?;
        let target = path.replace("profile.toml", "profile.json");
        outputs.push((target, json));
    }
    if errors.is_empty() {
        Ok(outputs)
    } else {
        Err(format!(
            "プロファイルに問題がある:\n- {}",
            errors.join("\n- ")
        ))
    }
}

/// `profiles/*/*/profile.toml` を、名前の順に集める。
fn sources(root: &Path) -> Result<Vec<String>, String> {
    let mut found = Vec::new();
    let base = root.join("profiles");
    let entries = |dir: &Path| -> Result<Vec<String>, String> {
        let mut names = Vec::new();
        for entry in fs::read_dir(dir).map_err(|e| format!("{} を読めない: {e}", dir.display()))?
        {
            let entry = entry.map_err(|e| format!("{} を読めない: {e}", dir.display()))?;
            if entry.path().is_dir() {
                names.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
        names.sort();
        Ok(names)
    };
    if !base.is_dir() {
        return Ok(found);
    }
    for id in entries(&base)? {
        for edition in entries(&base.join(&id))? {
            let path = format!("profiles/{id}/{edition}/profile.toml");
            if root.join(&path).is_file() {
                found.push(path);
            }
        }
    }
    Ok(found)
}

/// 権利の台帳（`formats/ledger.toml`）の仕様の名前。
fn ledger_specs(root: &Path) -> Result<BTreeSet<String>, String> {
    let path = "formats/ledger.toml";
    let source =
        fs::read_to_string(root.join(path)).map_err(|e| format!("{path} を読めない: {e}"))?;
    let ledger: toml::Table = toml::from_str(&source).map_err(|e| format!("{path}: {e}"))?;
    Ok(ledger
        .get("specs")
        .and_then(toml::Value::as_table)
        .map(|specs| specs.keys().cloned().collect())
        .unwrap_or_default())
}

/// プロファイルの中の参照と値を確かめ、問題の一覧を返す。
pub fn check(profile: &Profile, specs: &BTreeSet<String>) -> Vec<String> {
    let mut errors = Vec::new();
    if profile.format.split('.').next() != Some(FORMAT_MAJOR) {
        errors.push(format!(
            "format の大版は {FORMAT_MAJOR} にする（{}）",
            profile.format
        ));
    }
    for (name, value) in [
        ("id", &profile.id),
        ("edition", &profile.edition),
        ("title", &profile.title),
    ] {
        if value.is_empty() {
            errors.push(format!("{name} が空"));
        }
    }
    if profile.revision == 0 {
        errors.push("revision は 1 から数える".to_string());
    }
    if profile.license != "CC-BY-4.0" {
        errors.push(format!(
            "license は CC-BY-4.0 にする（決定 0002）: {}",
            profile.license
        ));
    }
    if let Some(extends) = &profile.extends
        && (extends.id.is_empty() || extends.edition.is_empty())
    {
        errors.push("extends には id と edition を書く".to_string());
    }
    if profile.standard_code.as_deref() == Some("") {
        errors.push("standard-code が空".to_string());
    }
    for (name, source) in &profile.sources {
        match &source.spec {
            Some(spec) => {
                if !specs.contains(spec) {
                    errors.push(format!(
                        "sources.{name}: 台帳にない仕様 {spec}（formats/ledger.toml）"
                    ));
                }
                if source.title.is_some() || source.url.is_some() {
                    errors.push(format!(
                        "sources.{name}: spec と title・url を一緒に書かない"
                    ));
                }
            }
            None => {
                let missing: Vec<&str> = [
                    ("title", &source.title),
                    ("publisher", &source.publisher),
                    ("version", &source.version),
                    ("url", &source.url),
                    ("checked", &source.checked),
                ]
                .into_iter()
                .filter(|(_, value)| value.as_deref().is_none_or(str::is_empty))
                .map(|(key, _)| key)
                .collect();
                if !missing.is_empty() {
                    errors.push(format!("sources.{name}: {} がない", missing.join("・")));
                }
            }
        }
    }
    let reference = |place: &str, reference: &Reference, errors: &mut Vec<String>| {
        if !profile.sources.contains_key(&reference.document) {
            errors.push(format!(
                "{place}: sources にない文書 {}",
                reference.document
            ));
        }
        if reference.section.as_deref() == Some("") || reference.item.as_deref() == Some("") {
            errors.push(format!("{place}: section と item は空にしない"));
        }
        if let Some(basis) = &reference.basis
            && basis != "chosen"
        {
            errors.push(format!(
                "{place}: basis は chosen だけを書く（出典が列挙しない値）: {basis}"
            ));
        }
    };
    for (name, vocabulary) in &profile.vocabularies {
        reference(
            &format!("vocabularies.{name}"),
            &vocabulary.source,
            &mut errors,
        );
        if vocabulary.values.is_empty() {
            errors.push(format!("vocabularies.{name}: values が空"));
        }
        if vocabulary.unit.as_deref().is_some_and(|unit| unit != "mm") {
            errors.push(format!("vocabularies.{name}: unit は mm だけを書く"));
        }
    }
    let mut ids = BTreeSet::new();
    for rule in &profile.rules {
        let place = format!("rules.{}", rule.id);
        if !ids.insert(rule.id.as_str()) {
            errors.push(format!("{place}: id が重なる"));
        }
        reference(&place, &rule.source, &mut errors);
        if !["error", "warning", "info"].contains(&rule.level.as_str()) {
            errors.push(format!(
                "{place}: level は error・warning・info のどれか: {}",
                rule.level
            ));
        }
        let Some((_, params)) = CHECKS.iter().find(|(name, _)| *name == rule.check) else {
            errors.push(format!("{place}: 知らない照合 {}", rule.check));
            continue;
        };
        for key in rule.params.keys() {
            if !params.contains(&key.as_str()) {
                errors.push(format!("{place}: {} に書けない欄 {key}", rule.check));
            }
        }
        match &rule.vocabulary {
            Some(name) if !profile.vocabularies.contains_key(name) => {
                errors.push(format!("{place}: vocabularies にない語彙 {name}"));
            }
            None if !WITHOUT_VOCABULARY.contains(&rule.check.as_str()) => {
                errors.push(format!("{place}: vocabulary がない"));
            }
            _ => {}
        }
        if let Some(parts) = rule.params.get("parts").and_then(toml::Value::as_array) {
            for part in parts {
                match part.as_str() {
                    Some(name) if profile.vocabularies.contains_key(name) => {}
                    _ => errors.push(format!("{place}: parts に vocabularies にない語彙 {part}")),
                }
            }
        }
        if let Some(orientation) = rule.params.get("orientation")
            && !matches!(orientation.as_str(), Some("landscape" | "portrait"))
        {
            errors.push(format!("{place}: orientation は landscape か portrait"));
        }
    }
    errors
}

/// TOML の値を、キーの順に並べた JSON にする。差分を読みやすくするため、2 字で字下げする。
pub fn render_json(value: &toml::Value) -> Result<String, String> {
    let mut out = String::new();
    write_value(&mut out, value, 0)?;
    out.push('\n');
    Ok(out)
}

fn write_value(out: &mut String, value: &toml::Value, depth: usize) -> Result<(), String> {
    let indent = "  ".repeat(depth + 1);
    let close = "  ".repeat(depth);
    match value {
        toml::Value::String(text) => out.push_str(&quoted(text)),
        toml::Value::Integer(number) => {
            let _ = write!(out, "{number}");
        }
        toml::Value::Float(number) => out.push_str(
            &zumen_json::float_text(*number)
                .ok_or_else(|| format!("JSON にできない数 {number}"))?,
        ),
        toml::Value::Boolean(flag) => {
            let _ = write!(out, "{flag}");
        }
        toml::Value::Datetime(date) => {
            return Err(format!("日時は文字列で書く: {date}"));
        }
        toml::Value::Array(items) => {
            // 数と文字列だけの配列は 1 行にする
            if items.iter().all(|item| {
                matches!(
                    item,
                    toml::Value::String(_) | toml::Value::Integer(_) | toml::Value::Float(_)
                )
            }) {
                out.push('[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    write_value(out, item, depth + 1)?;
                }
                out.push(']');
            } else {
                out.push_str("[\n");
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push_str(",\n");
                    }
                    out.push_str(&indent);
                    write_value(out, item, depth + 1)?;
                }
                let _ = write!(out, "\n{close}]");
            }
        }
        toml::Value::Table(table) => {
            if table.is_empty() {
                out.push_str("{}");
                return Ok(());
            }
            out.push_str("{\n");
            for (index, (key, item)) in table.iter().enumerate() {
                if index > 0 {
                    out.push_str(",\n");
                }
                let _ = write!(out, "{indent}{}: ", quoted(key));
                write_value(out, item, depth + 1)?;
            }
            let _ = write!(out, "\n{close}}}");
        }
    }
    Ok(())
}

fn quoted(text: &str) -> String {
    let mut writer = zumen_json::Writer::new();
    writer.string(text);
    String::from_utf8_lossy(&writer.into_bytes()).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE: &str = r##"
        format = "0.1"
        id = "office"
        edition = "1"
        revision = 1
        title = "社内の製図規則"
        license = "CC-BY-4.0"

        [sources.main]
        title = "製図規則"
        publisher = "社内"
        version = "1"
        url = "https://example.com/rules"
        checked = "2026-10-28"

        [vocabularies.color]
        source = { document = "main", section = "3" }
        values = [{ name = "黒", rgb = "#000000" }]

        [[rules]]
        id = "color"
        check = "color"
        level = "warning"
        source = { document = "main", section = "3" }
        vocabulary = "color"
    "##;

    fn errors(source: &str) -> Vec<String> {
        let profile: Profile = toml::from_str(source).unwrap();
        check(&profile, &BTreeSet::from(["sxf_3_1".to_string()]))
    }

    #[test]
    fn a_complete_profile_has_no_errors() {
        assert_eq!(errors(PROFILE), Vec::<String>::new());
    }

    #[test]
    fn references_to_missing_sources_and_vocabularies_are_errors() {
        let source = PROFILE
            .replace(
                "vocabulary = \"color\"",
                "vocabulary = \"pen\"\ntolerance = 1",
            )
            .replace(
                "document = \"main\", section = \"3\" }\n        values",
                "document = \"spec\" }\n        values",
            );
        assert_eq!(
            errors(&source),
            [
                "vocabularies.color: sources にない文書 spec",
                "rules.color: color に書けない欄 tolerance",
                "rules.color: vocabularies にない語彙 pen",
            ]
        );
    }

    #[test]
    fn unknown_checks_levels_and_specs_are_errors() {
        let source = PROFILE
            .replace("check = \"color\"", "check = \"pen\"")
            .replace("level = \"warning\"", "level = \"fatal\"")
            .replace(
                "[sources.main]",
                "[sources.sxf]\nspec = \"sxf_9\"\n\n[sources.main]",
            );
        assert_eq!(
            errors(&source),
            [
                "sources.sxf: 台帳にない仕様 sxf_9（formats/ledger.toml）",
                "rules.color: level は error・warning・info のどれか: fatal",
                "rules.color: 知らない照合 pen",
            ]
        );
    }

    #[test]
    fn json_sorts_keys_and_keeps_numbers() {
        let value: toml::Value = toml::from_str("b = [1, 1.5]\na = { c = \"黒\" }").unwrap();
        assert_eq!(
            render_json(&value).unwrap(),
            "{\n  \"a\": {\n    \"c\": \"黒\"\n  },\n  \"b\": [1, 1.5]\n}\n"
        );
    }
}
