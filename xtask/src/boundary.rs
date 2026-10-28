//! 境界の版・状態コード・関数（`schema/boundary.toml`）から、C のヘッダーと Rust の定数を作る（決定 0011・0012）。

use std::collections::BTreeSet;
use std::fmt::Write;

use serde::Deserialize;

/// 境界の定義（`schema/boundary.toml`）
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Boundary {
    pub versions: Versions,
    pub status: Vec<Status>,
    pub functions: Vec<Function>,
}

/// 版
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Versions {
    pub abi: u32,
    pub schema_major: u64,
    pub schema_minor: u64,
}

/// 状態コード
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub name: String,
    pub value: i32,
    pub description: String,
}

/// 関数
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Function {
    pub name: String,
    pub returns: String,
    pub description: String,
    #[serde(default)]
    pub params: Vec<Param>,
    #[serde(default = "every_target")]
    pub targets: Vec<String>,
}

/// 関数の引数
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Param {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub description: String,
}

const TARGETS: &[&str] = &["native", "wasm"];

fn every_target() -> Vec<String> {
    TARGETS.iter().map(|target| target.to_string()).collect()
}

/// 引数と戻り値の型の名前と、C と Rust での形
const TYPES: &[(&str, &str, &str)] = &[
    ("void", "void", "()"),
    ("u32", "uint32_t", "u32"),
    ("size", "size_t", "usize"),
    ("status", "zm_status", "i32"),
    ("bytes", "const uint8_t *", "*const u8"),
    ("bytes_mut", "uint8_t *", "*mut u8"),
    ("session", "zm_session *", "*mut crate::ffi::SessionHandle"),
    (
        "session_out",
        "zm_session **",
        "*mut *mut crate::ffi::SessionHandle",
    ),
    ("buf", "zm_buf *", "*mut crate::ffi::Buf"),
    (
        "arrow_stream",
        "struct ArrowArrayStream *",
        "*mut crate::ffi::ArrowArrayStream",
    ),
];

fn type_of(name: &str) -> Option<(&'static str, &'static str)> {
    TYPES
        .iter()
        .find(|(ty, _, _)| *ty == name)
        .map(|(_, c, rust)| (*c, *rust))
}

fn is_snake(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// 定義を確かめ、見つけた問題をすべて返す。
pub fn check(boundary: &Boundary) -> Vec<String> {
    let mut errors = Vec::new();
    if boundary.versions.abi == 0 || boundary.versions.schema_major == 0 {
        errors.push("版は 1 から数える".to_string());
    }

    let mut names = BTreeSet::new();
    let mut values = BTreeSet::new();
    for status in &boundary.status {
        if !is_snake(&status.name) {
            errors.push(format!("状態 `{}`: 名前は英小文字と _ にする", status.name));
        }
        if !names.insert(status.name.as_str()) {
            errors.push(format!("状態 `{}` が重なっている", status.name));
        }
        if !values.insert(status.value) {
            errors.push(format!("状態の値 {} が重なっている", status.value));
        }
        if status.description.trim().is_empty() {
            errors.push(format!("状態 `{}`: description が空", status.name));
        }
    }
    if !boundary
        .status
        .iter()
        .any(|status| status.name == "ok" && status.value == 0)
    {
        errors.push("値が 0 の状態 `ok` が要る".to_string());
    }

    let mut functions = BTreeSet::new();
    for function in &boundary.functions {
        let name = &function.name;
        if !(name.starts_with("zm_") && is_snake(name)) {
            errors.push(format!(
                "関数 `{name}`: 名前は zm_ で始め、英小文字と _ にする"
            ));
        }
        if !functions.insert(name.as_str()) {
            errors.push(format!("関数 `{name}` が重なっている"));
        }
        if function.description.trim().is_empty() {
            errors.push(format!("関数 `{name}`: description が空"));
        }
        if type_of(&function.returns).is_none() {
            errors.push(format!(
                "関数 `{name}`: 戻り値の型 `{}` を知らない",
                function.returns
            ));
        }
        if function.targets.is_empty()
            || !function
                .targets
                .iter()
                .all(|target| TARGETS.contains(&target.as_str()))
        {
            errors.push(format!(
                "関数 `{name}`: targets は {} から選ぶ",
                TARGETS.join("・")
            ));
        }
        let mut params = BTreeSet::new();
        for param in &function.params {
            if !is_snake(&param.name) || !params.insert(param.name.as_str()) {
                errors.push(format!(
                    "関数 `{name}`: 引数 `{}` の名前が使えない",
                    param.name
                ));
            }
            if param.ty == "void" || type_of(&param.ty).is_none() {
                errors.push(format!(
                    "関数 `{name}`: 引数 `{}` の型 `{}` は使えない",
                    param.name, param.ty
                ));
            }
            if param.description.trim().is_empty() {
                errors.push(format!(
                    "関数 `{name}`: 引数 `{}` の description が空",
                    param.name
                ));
            }
        }
    }
    errors
}

fn c_type(name: &str) -> &'static str {
    type_of(name).map_or("void", |(c, _)| c)
}

fn rust_type(name: &str) -> &'static str {
    type_of(name).map_or("()", |(_, rust)| rust)
}

fn has_target(function: &Function, target: &str) -> bool {
    function.targets.iter().any(|t| t == target)
}

/// C の宣言。`const uint8_t *` のような型と名前の間の空白を整える
fn c_declaration(ty: &str, name: &str) -> String {
    if ty.ends_with('*') {
        format!("{ty}{name}")
    } else {
        format!("{ty} {name}")
    }
}

/// UTF-8 の BOM。日本語のコメントを、Windows の MSVC が、OS のコードページによらず UTF-8 として読むように
const BOM: &str = "\u{feff}";

/// `bindings/c/include/zumen.h` の中身を作る。
pub fn render_header(boundary: &Boundary) -> String {
    let versions = &boundary.versions;
    let mut out = String::new();
    out.push_str(BOM);
    out.push_str(
        "/* 生成したファイル。手で直さない。\n * 元は schema/boundary.toml で、`cargo xtask codegen` で作り直す。 */\n\n",
    );
    out.push_str(concat!(
        "/*\n",
        " * zumen の境界（決定 0012）。\n",
        " *\n",
        " * - 文字列は UTF-8。要求と応答は JSON で、形は schema/messages.json にある\n",
        " * - 入力のバッファーは呼び出しの間だけ借りる。コアが返したバッファーは zm_free で返す\n",
        " * - 1 つのセッションは、一度に 1 つのスレッドだけが使う。zm_cancel だけは、どのスレッドからも呼べる\n",
        " * - 関数・状態コードは足すだけにし、消さず、名前・値・引数・意味を変えない（決定 0015）\n",
        " */\n\n",
        "#ifndef ZUMEN_H\n",
        "#define ZUMEN_H\n\n",
        "#include <stddef.h>\n",
        "#include <stdint.h>\n\n",
        "#ifdef __cplusplus\n",
        "extern \"C\" {\n",
        "#endif\n\n",
        "/* 呼び出し規約。32 bit の Windows では stdcall にする（VBA の Declare のため、決定 0011） */\n",
        "#if defined(_WIN32) && !defined(_WIN64)\n",
        "#define ZM_CALL __stdcall\n",
        "#else\n",
        "#define ZM_CALL\n",
        "#endif\n\n",
    ));
    let _ = writeln!(out, "/* ABI の大版。zm_abi_version が返す */");
    let _ = writeln!(out, "#define ZM_ABI_VERSION {}", versions.abi);
    let _ = writeln!(
        out,
        "/* メッセージのスキーマの版。大版は要求と応答の v、小版は capabilities が返す */"
    );
    let _ = writeln!(
        out,
        "#define ZM_SCHEMA_VERSION_MAJOR {}",
        versions.schema_major
    );
    let _ = writeln!(
        out,
        "#define ZM_SCHEMA_VERSION_MINOR {}\n",
        versions.schema_minor
    );

    out.push_str("/* 状態コード。理由は応答の診断で返す */\ntypedef int32_t zm_status;\n");
    for status in &boundary.status {
        let _ = writeln!(out, "/* {} */", status.description);
        let _ = writeln!(
            out,
            "#define ZM_{} {}",
            status.name.to_ascii_uppercase(),
            status.value
        );
    }
    out.push_str(concat!(
        "\n/* セッション。中身はコアだけが知る */\n",
        "typedef struct zm_session zm_session;\n\n",
        "/* コアが返すバッファー。zm_free で返す */\n",
        "typedef struct zm_buf {\n",
        "    uint8_t *ptr;\n",
        "    size_t len;\n",
        "} zm_buf;\n\n",
        "/* Arrow の C Data Interface と C Stream Interface。Arrow の文書が勧めるとおりの定義と目印\n",
        " * https://arrow.apache.org/docs/format/CDataInterface.html\n",
        " * https://arrow.apache.org/docs/format/CStreamInterface.html */\n",
        "#ifndef ARROW_C_DATA_INTERFACE\n",
        "#define ARROW_C_DATA_INTERFACE\n\n",
        "#define ARROW_FLAG_DICTIONARY_ORDERED 1\n",
        "#define ARROW_FLAG_NULLABLE 2\n",
        "#define ARROW_FLAG_MAP_KEYS_SORTED 4\n\n",
        "struct ArrowSchema {\n",
        "    const char *format;\n",
        "    const char *name;\n",
        "    const char *metadata;\n",
        "    int64_t flags;\n",
        "    int64_t n_children;\n",
        "    struct ArrowSchema **children;\n",
        "    struct ArrowSchema *dictionary;\n",
        "    void (*release)(struct ArrowSchema *);\n",
        "    void *private_data;\n",
        "};\n\n",
        "struct ArrowArray {\n",
        "    int64_t length;\n",
        "    int64_t null_count;\n",
        "    int64_t offset;\n",
        "    int64_t n_buffers;\n",
        "    int64_t n_children;\n",
        "    const void **buffers;\n",
        "    struct ArrowArray **children;\n",
        "    struct ArrowArray *dictionary;\n",
        "    void (*release)(struct ArrowArray *);\n",
        "    void *private_data;\n",
        "};\n\n",
        "#endif /* ARROW_C_DATA_INTERFACE */\n\n",
        "#ifndef ARROW_C_STREAM_INTERFACE\n",
        "#define ARROW_C_STREAM_INTERFACE\n\n",
        "struct ArrowArrayStream {\n",
        "    int (*get_schema)(struct ArrowArrayStream *, struct ArrowSchema *out);\n",
        "    int (*get_next)(struct ArrowArrayStream *, struct ArrowArray *out);\n",
        "    const char *(*get_last_error)(struct ArrowArrayStream *);\n",
        "    void (*release)(struct ArrowArrayStream *);\n",
        "    void *private_data;\n",
        "};\n\n",
        "#endif /* ARROW_C_STREAM_INTERFACE */\n",
    ));

    for function in boundary
        .functions
        .iter()
        .filter(|function| has_target(function, "native"))
    {
        out.push('\n');
        if function.params.is_empty() {
            let _ = writeln!(out, "/* {} */", function.description);
        } else {
            let _ = writeln!(out, "/*\n * {}\n *", function.description);
            for param in &function.params {
                let _ = writeln!(out, " * {}: {}", param.name, param.description);
            }
            out.push_str(" */\n");
        }
        let params = if function.params.is_empty() {
            "void".to_string()
        } else {
            function
                .params
                .iter()
                .map(|param| c_declaration(c_type(&param.ty), &param.name))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let _ = writeln!(
            out,
            "{} ZM_CALL {}({params});",
            c_type(&function.returns),
            function.name
        );
    }
    out.push_str("\n#ifdef __cplusplus\n}\n#endif\n\n#endif /* ZUMEN_H */\n");
    out
}

fn pascal(text: &str) -> String {
    text.split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect()
}

/// `crates/zumen-abi/src/abi.rs` の中身を作る。rustfmt で整える前のもの。
pub fn render_rust(boundary: &Boundary) -> String {
    let versions = &boundary.versions;
    let mut out = String::new();
    out.push_str("// 生成したファイル。手で直さない。\n");
    out.push_str("// 元は schema/boundary.toml で、`cargo xtask codegen` で作り直す。\n\n");
    out.push_str("//! 境界の版・状態コード・関数の形（決定 0012・0015）。\n\n");
    let _ = writeln!(
        out,
        "/// ABI の大版。`zm_abi_version` が返す\npub const ABI_VERSION: u32 = {};",
        versions.abi
    );
    let _ = writeln!(
        out,
        "/// メッセージのスキーマの大版。要求と応答の `v`\npub const SCHEMA_MAJOR: u64 = {};",
        versions.schema_major
    );
    let _ = writeln!(
        out,
        "/// メッセージのスキーマの小版。`capabilities` が返す\npub const SCHEMA_MINOR: u64 = {};\n",
        versions.schema_minor
    );

    out.push_str("/// 状態コード。理由は応答の診断で返す\n");
    out.push_str("#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub enum Status {\n");
    for status in &boundary.status {
        let _ = writeln!(
            out,
            "    /// {}\n    {},",
            status.description,
            pascal(&status.name)
        );
    }
    out.push_str("}\n\nimpl Status {\n    /// すべての状態\n    pub const ALL: &[Self] = &[");
    for status in &boundary.status {
        let _ = write!(out, "Self::{}, ", pascal(&status.name));
    }
    out.push_str("];\n\n    /// C の値（`ZM_OK` など）\n    pub const fn code(self) -> i32 {\n        match self {\n");
    for status in &boundary.status {
        let _ = writeln!(
            out,
            "            Self::{} => {},",
            pascal(&status.name),
            status.value
        );
    }
    out.push_str(
        "        }\n    }\n\n    /// 名前（`ok` など）\n    pub const fn name(self) -> &'static str {\n        match self {\n",
    );
    for status in &boundary.status {
        let _ = writeln!(
            out,
            "            Self::{} => \"{}\",",
            pascal(&status.name),
            status.name
        );
    }
    out.push_str("        }\n    }\n}\n\n");

    for (target, label) in [
        ("native", "ネイティブのライブラリ"),
        ("wasm", "wasm のモジュール"),
    ] {
        let _ = writeln!(
            out,
            "/// {label}が出す関数の名前\npub const {}_FUNCTIONS: &[&str] = &[",
            target.to_ascii_uppercase()
        );
        for function in boundary
            .functions
            .iter()
            .filter(|function| has_target(function, target))
        {
            let _ = writeln!(out, "    \"{}\",", function.name);
        }
        out.push_str("];\n\n");
    }

    out.push_str("// 関数の形が schema/boundary.toml と合うことを、コンパイルで確かめる\n");
    for function in &boundary.functions {
        let native = has_target(function, "native");
        let wasm = has_target(function, "wasm");
        if native && !wasm {
            out.push_str("#[cfg(not(target_family = \"wasm\"))]\n");
        } else if wasm && !native {
            out.push_str("#[cfg(target_family = \"wasm\")]\n");
        }
        let params: Vec<&str> = function
            .params
            .iter()
            .map(|param| rust_type(&param.ty))
            .collect();
        let returns = if function.returns == "void" {
            String::new()
        } else {
            format!(" -> {}", rust_type(&function.returns))
        };
        let _ = writeln!(
            out,
            "const _: unsafe extern \"system\" fn({}){returns} = crate::ffi::{};",
            params.join(", "),
            function.name
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUNDARY: &str = r#"
        [versions]
        abi = 1
        schema-major = 1
        schema-minor = 0

        [[status]]
        name = "ok"
        value = 0
        description = "成功した"

        [[status]]
        name = "failed"
        value = 1
        description = "失敗した"

        [[functions]]
        name = "zm_abi_version"
        returns = "u32"
        description = "ABI の大版を返す"

        [[functions]]
        name = "zm_call"
        returns = "status"
        description = "呼ぶ"
        params = [
            { name = "session", type = "session", description = "セッション" },
            { name = "request", type = "bytes", description = "要求" },
            { name = "request_len", type = "size", description = "長さ" },
        ]

        [[functions]]
        name = "zm_alloc"
        returns = "bytes_mut"
        targets = ["wasm"]
        description = "確保する"
        params = [{ name = "len", type = "size", description = "長さ" }]
    "#;

    fn boundary(source: &str) -> Boundary {
        toml::from_str(source).unwrap()
    }

    #[test]
    fn accepts_a_consistent_definition() {
        assert_eq!(check(&boundary(BOUNDARY)), Vec::<String>::new());
    }

    #[test]
    fn reports_bad_names_types_and_values() {
        let source = BOUNDARY
            .replace(
                "name = \"failed\"\n        value = 1",
                "name = \"Failed\"\n        value = 0",
            )
            .replace(
                "type = \"size\", description = \"長さ\" },\n        ]",
                "type = \"void\", description = \"長さ\" },\n        ]",
            )
            .replace("targets = [\"wasm\"]", "targets = [\"jvm\"]");
        let errors = check(&boundary(&source));
        assert_eq!(errors.len(), 4, "{errors:?}");
    }

    #[test]
    fn renders_the_header_for_native_functions_only() {
        let header = render_header(&boundary(BOUNDARY));
        assert!(header.starts_with(BOM));
        assert!(header.contains("#define ZM_FAILED 1\n"));
        assert!(header.contains("uint32_t ZM_CALL zm_abi_version(void);"));
        assert!(header.contains(
            "zm_status ZM_CALL zm_call(zm_session *session, const uint8_t *request, size_t request_len);"
        ));
        assert!(!header.contains("zm_alloc"));
    }

    #[test]
    fn renders_signature_checks_with_target_conditions() {
        let rust = render_rust(&boundary(BOUNDARY));
        assert!(rust.contains(
            "const _: unsafe extern \"system\" fn() -> u32 = crate::ffi::zm_abi_version;"
        ));
        assert!(rust.contains(
            "#[cfg(target_family = \"wasm\")]\nconst _: unsafe extern \"system\" fn(usize) -> *mut u8 = crate::ffi::zm_alloc;"
        ));
        assert!(rust.contains("Self::Failed => 1,"));
    }
}
