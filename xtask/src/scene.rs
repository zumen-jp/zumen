//! 2D のシーンの配置（`schema/scene.toml`）から、Rust の定数を作る（決定 0137・0172・0178）。

use std::collections::BTreeSet;
use std::fmt::Write;

use serde::Deserialize;

/// シーンの配置の定義（`schema/scene.toml`）
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    pub versions: Versions,
    pub sections: Vec<Section>,
    #[serde(default)]
    pub values: Vec<Values>,
}

/// 版
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Versions {
    pub major: u16,
    pub minor: u16,
}

/// 区画（1 つの列）
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Section {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub width: u32,
    pub description: String,
}

/// 列に入れる値の組
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Values {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub description: String,
    pub items: Vec<Item>,
}

/// 値
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub name: String,
    pub value: u32,
    pub description: String,
}

/// 区画の値の型と、Rust の型・バイト数
const TYPES: &[(&str, &str, &str, u32)] = &[
    ("u8", "u8", "U8", 1),
    ("u32", "u32", "U32", 4),
    ("f32", "f32", "F32", 4),
    ("f64", "f64", "F64", 8),
];

fn is_snake(text: &str) -> bool {
    !text.is_empty()
        && text.starts_with(|c: char| c.is_ascii_lowercase())
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// 定義の誤りを返す
pub fn check(scene: &Scene) -> Vec<String> {
    let mut errors = Vec::new();
    if scene.versions.major == 0 {
        errors.push("versions.major は 1 から".to_string());
    }
    let mut names = BTreeSet::new();
    for section in &scene.sections {
        let name = &section.name;
        if !is_snake(name) {
            errors.push(format!("区画 `{name}`: 名前は英小文字・数字・_ にする"));
        }
        if !names.insert(name.as_str()) {
            errors.push(format!("区画 `{name}` が重なる"));
        }
        if !TYPES.iter().any(|(ty, ..)| *ty == section.ty) {
            errors.push(format!("区画 `{name}`: 型 `{}` は使えない", section.ty));
        }
        if !(1..=4).contains(&section.width) {
            errors.push(format!("区画 `{name}`: width は 1〜4"));
        }
        if section.description.trim().is_empty() {
            errors.push(format!("区画 `{name}`: 説明がない"));
        }
    }
    let mut groups = BTreeSet::new();
    for values in &scene.values {
        let name = &values.name;
        if !is_snake(name) {
            errors.push(format!("値の組 `{name}`: 名前は英小文字・数字・_ にする"));
        }
        if !groups.insert(name.as_str()) {
            errors.push(format!("値の組 `{name}` が重なる"));
        }
        if values.ty != "u8" && values.ty != "u32" {
            errors.push(format!("値の組 `{name}`: 型は u8 か u32"));
        }
        if values.description.trim().is_empty() {
            errors.push(format!("値の組 `{name}`: 説明がない"));
        }
        let mut items = BTreeSet::new();
        let mut numbers = BTreeSet::new();
        for item in &values.items {
            if !is_snake(&item.name) || !items.insert(item.name.as_str()) {
                errors.push(format!(
                    "値の組 `{name}`: 値 `{}` の名前が使えない",
                    item.name
                ));
            }
            if !numbers.insert(item.value) {
                errors.push(format!("値の組 `{name}`: 値 {} が重なる", item.value));
            }
            if values.ty == "u8" && item.value > u32::from(u8::MAX) {
                errors.push(format!(
                    "値の組 `{name}`: 値 {} が u8 に入らない",
                    item.value
                ));
            }
            if item.description.trim().is_empty() {
                errors.push(format!("値の組 `{name}`: 値 `{}` に説明がない", item.name));
            }
        }
    }
    errors
}

/// `crates/zumen-scene/src/layout.rs` の中身を作る。rustfmt で整える前のもの。
pub fn render_rust(scene: &Scene) -> String {
    let mut out = String::new();
    out.push_str("// 生成したファイル。手で直さない。\n");
    out.push_str("// 元は schema/scene.toml で、`cargo xtask codegen` で作り直す。\n\n");
    out.push_str("//! 2D のシーンの配置（決定 0137・0172・0178）。\n\n");
    out.push_str("/// 見出しの始まりの 4 バイト\npub const MAGIC: [u8; 4] = *b\"ZMSC\";\n");
    let _ = writeln!(
        out,
        "/// 大版。区画の意味を変えるときに上げる\npub const MAJOR: u16 = {};",
        scene.versions.major
    );
    let _ = writeln!(
        out,
        "/// 小版。区画か値を足すたびに上げる\npub const MINOR: u16 = {};",
        scene.versions.minor
    );
    out.push_str("/// 番号の「なし」\npub const NONE: u32 = u32::MAX;\n\n");
    out.push_str(
        "/// 区画の値の型\n#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub enum Type {\n",
    );
    for (_, rust, variant, _) in TYPES {
        let _ = writeln!(out, "    /// `{rust}`\n    {variant},");
    }
    out.push_str("}\n\nimpl Type {\n    /// 値のバイト数\n    pub const fn size(self) -> usize {\n        match self {\n");
    for (_, _, variant, size) in TYPES {
        let _ = writeln!(out, "            Self::{variant} => {size},");
    }
    out.push_str("        }\n    }\n}\n\n");
    out.push_str(
        "/// 区画の定義\n#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub struct Section {\n    /// 名前\n    pub name: &'static str,\n    /// 値の型\n    pub ty: Type,\n    /// 行ごとの値の数\n    pub width: usize,\n}\n\n",
    );
    for (index, section) in scene.sections.iter().enumerate() {
        let _ = writeln!(
            out,
            "/// {}\npub const {}: usize = {index};",
            section.description,
            section.name.to_ascii_uppercase()
        );
    }
    let _ = writeln!(
        out,
        "\n/// 区画の定義。番号の順\npub const SECTIONS: [Section; {}] = [",
        scene.sections.len()
    );
    for section in &scene.sections {
        let variant = TYPES
            .iter()
            .find(|(ty, ..)| *ty == section.ty)
            .map_or("U8", |(_, _, variant, _)| variant);
        let _ = writeln!(
            out,
            "    Section {{ name: \"{}\", ty: Type::{variant}, width: {} }},",
            section.name, section.width
        );
    }
    out.push_str("];\n");
    for values in &scene.values {
        let _ = writeln!(
            out,
            "\n/// {}\npub mod {} {{",
            values.description, values.name
        );
        for item in &values.items {
            let _ = writeln!(
                out,
                "    /// {}\n    pub const {}: {} = {};",
                item.description,
                item.name.to_ascii_uppercase(),
                values.ty,
                item.value
            );
        }
        out.push_str("}\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_sections_and_values() {
        let scene: Scene = toml::from_str(
            r#"
            [versions]
            major = 0
            minor = 0
            [[sections]]
            name = "A"
            type = "i16"
            width = 5
            description = ""
            [[values]]
            name = "v"
            type = "u8"
            description = "値"
            items = [{ name = "a", value = 300, description = "a" }, { name = "a", value = 300, description = "" }]
            "#,
        )
        .unwrap();
        let errors = check(&scene);
        assert_eq!(errors.len(), 10, "{errors:?}");
    }

    #[test]
    fn renders_section_numbers_in_order() {
        let scene: Scene = toml::from_str(
            r#"
            [versions]
            major = 1
            minor = 2
            [[sections]]
            name = "a"
            type = "f64"
            width = 2
            description = "A"
            [[sections]]
            name = "b"
            type = "u8"
            width = 1
            description = "B"
            "#,
        )
        .unwrap();
        assert!(check(&scene).is_empty());
        let rust = render_rust(&scene);
        assert!(rust.contains("pub const A: usize = 0;"));
        assert!(rust.contains("pub const B: usize = 1;"));
        assert!(rust.contains("pub const MINOR: u16 = 2;"));
    }
}
