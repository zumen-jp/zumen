//! 診断の code の台帳とカタログを確かめ、Rust の定数を作る（決定 0009）。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use serde::Deserialize;

/// 文に差し込める、台帳の値ではない名前
const RESERVED_PLACEHOLDERS: &[&str] = &["count"];

const SEVERITIES: &[&str] = &["info", "warning", "error"];
const FIDELITIES: &[&str] = &["equivalent", "approximated", "drawn", "dropped"];
const PARAM_TYPES: &[&str] = &["uint", "int", "float", "text", "key"];
const STATUSES: &[&str] = &["active", "retired"];

/// 台帳（`catalog/codes.toml`）
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    /// 出来事の語と、その説明
    pub events: BTreeMap<String, String>,
    /// code の名前と、その性質
    pub codes: BTreeMap<String, Entry>,
}

/// 台帳の 1 つの code
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Entry {
    pub description: String,
    pub severity: String,
    pub fidelity: Option<String>,
    #[serde(default)]
    pub params: BTreeMap<String, String>,
    #[serde(default)]
    pub group_by: Vec<String>,
    pub since: String,
    pub status: String,
}

/// 文のカタログ（`catalog/ja.toml`・`catalog/en.toml`）
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Messages {
    pub number: NumberFormat,
    pub codes: BTreeMap<String, String>,
}

/// 文に差し込む数の書き方
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct NumberFormat {
    pub group_separator: String,
    pub decimal_separator: String,
}

/// 台帳とカタログを確かめ、見つけた問題をすべて返す。
pub fn check(registry: &Registry, catalogs: &[(&str, &Messages)]) -> Vec<String> {
    let mut errors = Vec::new();

    for event in registry.events.keys() {
        if !is_segment(event) {
            errors.push(format!(
                "出来事の語 `{event}` に、英小文字・数字・ハイフン以外がある"
            ));
        }
    }

    let mut constants: BTreeMap<String, &str> = BTreeMap::new();
    for (name, entry) in &registry.codes {
        check_entry(registry, name, entry, &mut errors);
        let constant = constant_name(name);
        if let Some(other) = constants.insert(constant.clone(), name) {
            errors.push(format!(
                "`{other}` と `{name}` が、同じ定数名 `{constant}` になる"
            ));
        }
    }

    for (file, messages) in catalogs {
        if messages.number.decimal_separator.is_empty() {
            errors.push(format!("{file}: [number] の decimal-separator が空"));
        }
        for (name, entry) in &registry.codes {
            let Some(text) = messages.codes.get(name) else {
                errors.push(format!("{file}: `{name}` の文がない"));
                continue;
            };
            match placeholders(text) {
                Ok(found) => {
                    for placeholder in found {
                        let known = entry.params.contains_key(&placeholder)
                            || RESERVED_PLACEHOLDERS.contains(&placeholder.as_str());
                        if !known {
                            errors.push(format!(
                                "{file}: `{name}` の文の {{{placeholder}}} が、台帳の値にない"
                            ));
                        }
                    }
                }
                Err(problem) => errors.push(format!("{file}: `{name}` の文: {problem}")),
            }
        }
        for name in messages.codes.keys() {
            if !registry.codes.contains_key(name) {
                errors.push(format!("{file}: 台帳にない `{name}` の文がある"));
            }
        }
    }

    errors
}

fn check_entry(registry: &Registry, name: &str, entry: &Entry, errors: &mut Vec<String>) {
    let parts: Vec<&str> = name.split('.').collect();
    if parts.len() != 3 || !parts.iter().all(|part| is_segment(part)) {
        errors.push(format!(
            "`{name}`: 名前は `<範囲>.<対象>.<出来事>` にし、英小文字・数字・ハイフンだけを使う"
        ));
    } else if let Some(event) = parts.last()
        && !registry.events.contains_key(*event)
    {
        errors.push(format!("`{name}`: 出来事 `{event}` が [events] にない"));
    }
    if !SEVERITIES.contains(&entry.severity.as_str()) {
        errors.push(format!(
            "`{name}`: severity `{}` は使えない",
            entry.severity
        ));
    }
    if let Some(fidelity) = &entry.fidelity
        && !FIDELITIES.contains(&fidelity.as_str())
    {
        errors.push(format!("`{name}`: fidelity `{fidelity}` は使えない"));
    }
    for (param, ty) in &entry.params {
        if !is_param_name(param) || RESERVED_PLACEHOLDERS.contains(&param.as_str()) {
            errors.push(format!("`{name}`: 値の名前 `{param}` は使えない"));
        }
        if !PARAM_TYPES.contains(&ty.as_str()) {
            errors.push(format!("`{name}`: 値 `{param}` の型 `{ty}` は使えない"));
        }
    }
    let mut seen = BTreeSet::new();
    for param in &entry.group_by {
        if !entry.params.contains_key(param) {
            errors.push(format!("`{name}`: group-by の `{param}` が params にない"));
        }
        if !seen.insert(param) {
            errors.push(format!("`{name}`: group-by に `{param}` が重なっている"));
        }
    }
    if entry.description.trim().is_empty() {
        errors.push(format!("`{name}`: description が空"));
    }
    if entry.since.trim().is_empty() {
        errors.push(format!("`{name}`: since が空"));
    }
    if !STATUSES.contains(&entry.status.as_str()) {
        errors.push(format!("`{name}`: status `{}` は使えない", entry.status));
    }
}

/// 英小文字・数字の並びを、ハイフン 1 つでつないだもの
fn is_segment(text: &str) -> bool {
    !text.is_empty()
        && text.split('-').all(|word| {
            !word.is_empty()
                && word
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

/// 英小文字で始まり、英小文字・数字・下線が続くもの
fn is_param_name(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// 文の中の `{名前}` を、出てくる順に返す。
fn placeholders(text: &str) -> Result<Vec<String>, String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(['{', '}']) {
        let after = &rest[start..];
        if after.starts_with('}') {
            return Err("対になっていない `}` がある".to_string());
        }
        let inner = &after[1..];
        let Some(end) = inner.find('}') else {
            return Err("閉じていない `{` がある".to_string());
        };
        let name = &inner[..end];
        if !is_param_name(name) {
            return Err(format!("{{{name}}} は差し込みの名前にならない"));
        }
        found.push(name.to_string());
        rest = &inner[end + 1..];
    }
    Ok(found)
}

/// `budget.decoded-bytes.exceeded` を `BUDGET_DECODED_BYTES_EXCEEDED` にする。
fn constant_name(code: &str) -> String {
    code.chars()
        .map(|c| match c {
            '.' | '-' => '_',
            c => c.to_ascii_uppercase(),
        })
        .collect()
}

fn variant(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
        .unwrap_or_default()
}

fn param_type(ty: &str) -> &'static str {
    match ty {
        "uint" => "UInt",
        "int" => "Int",
        "float" => "Float",
        "text" => "Text",
        _ => "Key",
    }
}

/// 台帳から、`crates/zumen-base/src/codes.rs` の中身を作る。rustfmt で整える前のもの。
pub fn render(registry: &Registry) -> String {
    let mut out = String::new();
    out.push_str("// 生成したファイル。手で直さない。\n");
    out.push_str("// 元は catalog/codes.toml で、`cargo xtask codegen` で作り直す。\n\n");
    out.push_str("//! 診断の code の定数（決定 0009）。台帳の名前の順に並べる。\n\n");
    let mut imports = vec!["Code", "Severity"];
    if registry
        .codes
        .values()
        .any(|entry| entry.fidelity.is_some())
    {
        imports.push("Fidelity");
    }
    if registry
        .codes
        .values()
        .any(|entry| !entry.params.is_empty())
    {
        imports.extend(["Param", "ParamType"]);
    }
    imports.sort_unstable();
    let _ = writeln!(out, "use crate::code::{{{}}};\n", imports.join(", "));

    for (name, entry) in &registry.codes {
        let _ = writeln!(out, "/// {}", entry.description);
        let _ = writeln!(out, "///");
        let _ = writeln!(out, "/// `{name}`（{} から）", entry.since);
        if entry.status == "retired" {
            out.push_str("#[deprecated = \"廃止した code（catalog/codes.toml）\"]\n");
        }
        let fidelity = entry
            .fidelity
            .as_deref()
            .map(|f| format!("Some(Fidelity::{})", variant(f)))
            .unwrap_or_else(|| "None".to_string());
        let params: Vec<String> = entry
            .params
            .iter()
            .map(|(param, ty)| format!("Param::new(\"{param}\", ParamType::{})", param_type(ty)))
            .collect();
        let group_by: Vec<String> = entry.group_by.iter().map(|p| format!("\"{p}\"")).collect();
        let _ = writeln!(
            out,
            "pub const {}: Code = Code::new(\"{name}\", Severity::{}, {fidelity}, &[{}], &[{}]);\n",
            constant_name(name),
            variant(&entry.severity),
            params.join(", "),
            group_by.join(", "),
        );
    }

    out.push_str("/// 台帳のすべての code（名前の順）\n");
    out.push_str("#[allow(deprecated)]\n");
    out.push_str("pub const ALL: &[Code] = &[\n");
    for name in registry.codes.keys() {
        let _ = writeln!(out, "    {},", constant_name(name));
    }
    out.push_str("];\n");
    out
}

/// JSON の文字列として書く
fn quoted(text: &str) -> String {
    let mut writer = zumen_json::Writer::new();
    writer.string(text);
    String::from_utf8_lossy(&writer.into_bytes()).into_owned()
}

/// コアに `zm_push` で渡すカタログの JSON（`catalog/<言語>.json`）を作る。
///
/// 形は `schema/messages.json` の `catalog`。差分を読みやすくするため、code ごとに 1 行にする。
pub fn render_json(lang: &str, package: &str, messages: &Messages) -> String {
    let mut out = String::new();
    out.push_str("{\n  \"v\": 1,\n");
    let _ = writeln!(out, "  \"lang\": {},", quoted(lang));
    let _ = writeln!(out, "  \"package\": {},", quoted(package));
    let _ = writeln!(
        out,
        "  \"number\": {{ \"group_separator\": {}, \"decimal_separator\": {} }},",
        quoted(&messages.number.group_separator),
        quoted(&messages.number.decimal_separator)
    );
    out.push_str("  \"codes\": {");
    for (index, (name, text)) in messages.codes.iter().enumerate() {
        let separator = if index == 0 { "\n" } else { ",\n" };
        let _ = write!(out, "{separator}    {}: {}", quoted(name), quoted(text));
    }
    out.push_str(if messages.codes.is_empty() {
        "}\n}\n"
    } else {
        "\n  }\n}\n"
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry(source: &str) -> Registry {
        toml::from_str(source).unwrap()
    }

    fn messages(source: &str) -> Messages {
        let source =
            format!("number = {{ group-separator = \",\", decimal-separator = \".\" }}\n{source}");
        toml::from_str(&source).unwrap()
    }

    const REGISTRY: &str = r#"
        [events]
        exceeded = "予算の上限を超えた"

        [codes."budget.records.exceeded"]
        description = "レコードの数が上限を超えた"
        severity = "error"
        params = { limit = "uint" }
        since = "0.0.0"
        status = "active"
    "#;

    #[test]
    fn finds_placeholders_in_order() {
        assert_eq!(
            placeholders("a {limit} b {count}").unwrap(),
            ["limit", "count"]
        );
        assert_eq!(placeholders("なし").unwrap(), Vec::<String>::new());
        assert!(placeholders("{limit").is_err());
        assert!(placeholders("limit}").is_err());
        assert!(placeholders("{Limit}").is_err());
    }

    #[test]
    fn checks_segments_and_constant_names() {
        assert!(is_segment("decoded-bytes"));
        assert!(!is_segment("decoded--bytes"));
        assert!(!is_segment("Decoded"));
        assert_eq!(
            constant_name("budget.decoded-bytes.exceeded"),
            "BUDGET_DECODED_BYTES_EXCEEDED"
        );
    }

    #[test]
    fn accepts_a_consistent_catalog() {
        let registry = registry(REGISTRY);
        let ja = messages(r#"codes = { "budget.records.exceeded" = "上限（{limit}）を超えた" }"#);
        assert_eq!(check(&registry, &[("ja.toml", &ja)]), Vec::<String>::new());
    }

    #[test]
    fn reports_missing_unknown_and_mismatched_messages() {
        let registry = registry(REGISTRY);
        let ja = messages(
            r#"codes = { "budget.records.exceeded" = "{used} を超えた", "x.y.z" = "余り" }"#,
        );
        let en = messages("codes = {}");
        let errors = check(&registry, &[("ja.toml", &ja), ("en.toml", &en)]);
        assert_eq!(errors.len(), 3, "{errors:?}");
    }

    #[test]
    fn reports_bad_names_and_unknown_events() {
        let registry = registry(
            r#"
            [events]
            exceeded = "予算の上限を超えた"

            [codes."budget.exceeded"]
            description = "部分が足りない"
            severity = "error"
            since = "0.0.0"
            status = "active"

            [codes."budget.records.vanished"]
            description = "出来事が台帳にない"
            severity = "fatal"
            since = "0.0.0"
            status = "active"
            "#,
        );
        let errors = check(&registry, &[]);
        assert_eq!(errors.len(), 3, "{errors:?}");
    }

    #[test]
    fn renders_the_catalog_as_json_that_reads_back() {
        let ja = messages(
            r#"codes = { "budget.records.exceeded" = "上限（{limit}）を超えた", "zip.path.unsafe" = "「{name}」\"" }"#,
        );
        let text = render_json("ja", "0.0.0", &ja);
        let value = zumen_json::parse(text.as_bytes()).unwrap();
        assert_eq!(
            value.pointer("/codes/zip.path.unsafe"),
            Some(&zumen_json::Value::String("「{name}」\"".to_string()))
        );
        assert_eq!(
            value.pointer("/number/group_separator"),
            Some(&zumen_json::Value::String(",".to_string()))
        );
        assert!(text.ends_with("\n  }\n}\n"));
    }

    #[test]
    fn renders_constants_in_name_order() {
        let source = render(&registry(REGISTRY));
        assert!(source.contains(
            "pub const BUDGET_RECORDS_EXCEEDED: Code = Code::new(\"budget.records.exceeded\""
        ));
        assert!(source.contains("Param::new(\"limit\", ParamType::UInt)"));
    }
}
