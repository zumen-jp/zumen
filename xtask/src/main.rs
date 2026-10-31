//! リポジトリの作業。`cargo xtask <コマンド>` で使う（決定 0011）。
//!
//! - `codegen`: 形式の一覧、Rust の定数と型（シーンの配置を含む）、C のヘッダー（シーンの配置を含む）、TypeScript・JavaScript・Python・.NET
//!   の型と定数、CLI の MCP の道具の一覧、カタログとプロファイルの JSON を生成する
//! - `codegen --check`: 権利の台帳と依存・カタログ・スキーマ・プロファイル・公開サンプルの台帳と、包みの版を確かめ、
//!   生成したファイルが最新かを確かめる

mod boundary;
mod catalog;
mod cli;
mod dotnet;
mod formats;
mod java;
mod javascript;
mod profiles;
mod python;
mod samples;
mod scene;
mod schema;
// 適合試験と同じ SHA-256 を使う
#[path = "../../bindings/conformance/src/sha256.rs"]
mod sha256;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

const USAGE: &str = "使い方: cargo xtask codegen [--check]";

/// 生成するファイルの置き場所（リポジトリの根からの相対パス）
const CODES_RS: &str = "crates/zumen-base/src/codes.rs";
const ABI_RS: &str = "crates/zumen-abi/src/abi.rs";
const MESSAGES_RS: &str = "crates/zumen-abi/src/messages.rs";
const SCENE_RS: &str = "crates/zumen-scene/src/layout.rs";
const HEADER: &str = "bindings/c/include/zumen.h";
const SCENE_HEADER: &str = "bindings/c/include/zumen_scene.h";
const JS_MESSAGES: &str = "bindings/js/src/messages.d.ts";
const JS_CONSTANTS: &str = "bindings/js/src/constants.js";
const JS_CONSTANTS_TYPES: &str = "bindings/js/src/constants.d.ts";
const JS_PACKAGE: &str = "bindings/js/package.json";
const PY_MESSAGES: &str = "bindings/python/zumen/messages.py";
const PY_CONSTANTS: &str = "bindings/python/zumen/constants.py";
const PY_PROJECT: &str = "bindings/python/pyproject.toml";
const CS_MESSAGES: &str = "bindings/dotnet/Zumen/Messages.g.cs";
const CS_CONSTANTS: &str = "bindings/dotnet/Zumen/Constants.g.cs";
const CS_PROJECT: &str = "bindings/dotnet/Zumen/Zumen.csproj";
const JAVA_MESSAGES: &str = "bindings/java/src/main/java/dev/zumen/Messages.java";
const JAVA_CONSTANTS: &str = "bindings/java/src/main/java/dev/zumen/Constants.java";
const JAVA_PROJECT: &str = "bindings/java/pom.xml";
const CLI_TOOLS: &str = "bindings/cli/src/tools.json";
const CATALOG_JA: &str = "catalog/ja.json";
/// CLI の MCP のリソース。crates.io の crate はフォルダーの外を読めないので、crate の中に写す（決定 0205）
const CLI_SCHEMA: &str = "bindings/cli/resources/messages.json";
const CLI_CATALOG_JA: &str = "bindings/cli/resources/ja.json";
const CLI_CATALOG_EN: &str = "bindings/cli/resources/en.json";
const CATALOG_EN: &str = "catalog/en.json";
const FORMATS_MD: &str = "docs/formats/README.md";

/// パッケージの版。カタログはコアと同じ版で出す（決定 0009）
const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Write,
    Check,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["codegen"] => codegen(Mode::Write),
        ["codegen", "--check"] => codegen(Mode::Check),
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn codegen(mode: Mode) -> Result<(), String> {
    let root = root();
    let formats = formats::generate(&root)?;
    let read = |path: &str| {
        fs::read_to_string(root.join(path)).map_err(|e| format!("{path} を読めない: {e}"))
    };

    let registry: catalog::Registry = toml::from_str(&read("catalog/codes.toml")?)
        .map_err(|e| format!("catalog/codes.toml: {e}"))?;
    let ja: catalog::Messages =
        toml::from_str(&read("catalog/ja.toml")?).map_err(|e| format!("catalog/ja.toml: {e}"))?;
    let en: catalog::Messages =
        toml::from_str(&read("catalog/en.toml")?).map_err(|e| format!("catalog/en.toml: {e}"))?;
    let errors = catalog::check(&registry, &[("ja.toml", &ja), ("en.toml", &en)]);
    if !errors.is_empty() {
        return Err(format!(
            "台帳とカタログに問題がある:\n- {}",
            errors.join("\n- ")
        ));
    }

    let boundary: boundary::Boundary = toml::from_str(&read("schema/boundary.toml")?)
        .map_err(|e| format!("schema/boundary.toml: {e}"))?;
    let errors = boundary::check(&boundary);
    if !errors.is_empty() {
        return Err(format!(
            "schema/boundary.toml に問題がある:\n- {}",
            errors.join("\n- ")
        ));
    }

    let schema = schema::parse(read("schema/messages.json")?.as_bytes()).map_err(|errors| {
        format!(
            "schema/messages.json に問題がある:\n- {}",
            errors.join("\n- ")
        )
    })?;

    let ops = schema.request_tags();
    let errors = boundary::check_ops(&boundary, &ops);
    if !errors.is_empty() {
        return Err(format!(
            "schema/boundary.toml のモジュールに問題がある:\n- {}",
            errors.join("\n- ")
        ));
    }

    let scene: scene::Scene = toml::from_str(&read("schema/scene.toml")?)
        .map_err(|e| format!("schema/scene.toml: {e}"))?;
    let errors = scene::check(&scene);
    if !errors.is_empty() {
        return Err(format!(
            "schema/scene.toml に問題がある:\n- {}",
            errors.join("\n- ")
        ));
    }

    let profiles = profiles::generate(&root)?;
    samples::check(&root)?;
    check_package_version(JS_PACKAGE, &read(JS_PACKAGE)?)?;
    check_pyproject_version(PY_PROJECT, &read(PY_PROJECT)?)?;
    check_csproj_version(CS_PROJECT, &read(CS_PROJECT)?, boundary.versions.abi)?;
    check_pom_version(JAVA_PROJECT, &read(JAVA_PROJECT)?)?;

    let outputs = [
        (CODES_RS, rustfmt(&root, &catalog::render(&registry))?),
        (ABI_RS, rustfmt(&root, &boundary::render_rust(&boundary))?),
        (MESSAGES_RS, rustfmt(&root, &schema::render(&schema))?),
        (SCENE_RS, rustfmt(&root, &scene::render_rust(&scene))?),
        (HEADER, boundary::render_header(&boundary)),
        (SCENE_HEADER, scene::render_c_header(&scene)),
        (JS_MESSAGES, javascript::render_types(&schema)),
        (
            JS_CONSTANTS,
            javascript::render_constants(PACKAGE_VERSION, &boundary, &scene),
        ),
        (
            JS_CONSTANTS_TYPES,
            javascript::render_constants_types(&boundary, &scene),
        ),
        (PY_MESSAGES, python::render_types(&schema)?),
        (
            PY_CONSTANTS,
            python::render_constants(PACKAGE_VERSION, &boundary, &scene)?,
        ),
        (CS_MESSAGES, dotnet::render_types(&schema)?),
        (
            CS_CONSTANTS,
            dotnet::render_constants(PACKAGE_VERSION, &boundary, &scene)?,
        ),
        (JAVA_MESSAGES, java::render_types(&schema)?),
        (
            JAVA_CONSTANTS,
            java::render_constants(PACKAGE_VERSION, &boundary, &scene)?,
        ),
        (
            CLI_TOOLS,
            cli::render(read("schema/messages.json")?.as_bytes())?,
        ),
        (CLI_SCHEMA, read("schema/messages.json")?),
        (CATALOG_JA, catalog::render_json("ja", PACKAGE_VERSION, &ja)),
        (CATALOG_EN, catalog::render_json("en", PACKAGE_VERSION, &en)),
        (
            CLI_CATALOG_JA,
            catalog::render_json("ja", PACKAGE_VERSION, &ja),
        ),
        (
            CLI_CATALOG_EN,
            catalog::render_json("en", PACKAGE_VERSION, &en),
        ),
        (FORMATS_MD, formats),
    ];
    let outputs: Vec<(&str, String)> = outputs
        .into_iter()
        .chain(
            profiles
                .iter()
                .map(|(path, json)| (path.as_str(), json.clone())),
        )
        .collect();
    update_outputs(&root, &outputs, mode)
}

/// 包みの package.json の版が、コアと同じかを確かめる（決定 0015）
fn check_package_version(path: &str, source: &str) -> Result<(), String> {
    let value = zumen_json::parse(source.as_bytes()).map_err(|e| format!("{path}: {e:?}"))?;
    let version = match &value {
        zumen_json::Value::Object(object) => object.get("version"),
        _ => None,
    };
    match version {
        Some(zumen_json::Value::String(version)) if version == PACKAGE_VERSION => Ok(()),
        _ => Err(format!(
            "{path} の version を、コアと同じ {PACKAGE_VERSION} にする（決定 0015）"
        )),
    }
}

/// Python の包みの pyproject.toml の版が、コアと同じかを確かめる（決定 0015）
fn check_pyproject_version(path: &str, source: &str) -> Result<(), String> {
    let value: toml::Table = toml::from_str(source).map_err(|e| format!("{path}: {e}"))?;
    let version = value
        .get("project")
        .and_then(|project| project.get("version"))
        .and_then(toml::Value::as_str);
    match version {
        Some(version) if version == PACKAGE_VERSION => Ok(()),
        _ => Err(format!(
            "{path} の project.version を、コアと同じ {PACKAGE_VERSION} にする（決定 0015）"
        )),
    }
}

/// Java の包みの pom.xml の、project の直下の version が、コアと同じかを確かめる（決定 0015・0224）
fn check_pom_version(path: &str, source: &str) -> Result<(), String> {
    // project の直下の version は、parent と dependencies より前の、最初の version に書く
    let head = source
        .split("<dependencies>")
        .next()
        .unwrap_or(source)
        .split("<parent>")
        .next()
        .unwrap_or(source);
    let versions: Vec<&str> = head
        .split("<version>")
        .skip(1)
        .filter_map(|rest| rest.split_once("</version>").map(|(value, _)| value.trim()))
        .collect();
    if versions == [PACKAGE_VERSION] {
        Ok(())
    } else {
        Err(format!(
            "{path} の project の version を 1 つにし、コアと同じ {PACKAGE_VERSION} にする（決定 0015）。dependencies より前に書く"
        ))
    }
}

/// .NET の包みの csproj の版が、コアと同じかを確かめる。AssemblyVersion は ABI の大版に固定する（決定 0015・0196）
fn check_csproj_version(path: &str, source: &str, abi: u32) -> Result<(), String> {
    let values = |tag: &str| -> Vec<String> {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");
        source
            .split(open.as_str())
            .skip(1)
            .filter_map(|rest| {
                rest.split_once(close.as_str())
                    .map(|(value, _)| value.trim().to_string())
            })
            .collect()
    };
    if values("Version") != [PACKAGE_VERSION] {
        return Err(format!(
            "{path} の Version を 1 つにし、コアと同じ {PACKAGE_VERSION} にする（決定 0015）"
        ));
    }
    let assembly = format!("{abi}.0.0.0");
    if values("AssemblyVersion") != [assembly.as_str()] {
        return Err(format!(
            "{path} の AssemblyVersion を 1 つにし、ABI の大版の {assembly} にする（決定 0015）"
        ));
    }
    Ok(())
}

fn update_outputs(root: &Path, outputs: &[(&str, String)], mode: Mode) -> Result<(), String> {
    let mut stale = Vec::new();
    for (path, generated) in outputs {
        let full = root.join(path);
        let current = fs::read_to_string(&full).unwrap_or_default();
        if current == *generated {
            continue;
        }
        match mode {
            Mode::Check => stale.push(*path),
            Mode::Write => {
                if let Some(parent) = full.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("{path} の置き場所を作れない: {e}"))?;
                }
                fs::write(&full, generated).map_err(|e| format!("{path} に書けない: {e}"))?;
            }
        }
    }
    if stale.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "生成したファイルが、元と合わない: {}。`cargo xtask codegen` を実行して、生成したファイルをコミットする",
            stale.join("、")
        ))
    }
}

/// 生成したコードを rustfmt で整える。`cargo fmt --check` と同じ結果にするため。
fn rustfmt(root: &Path, source: &str) -> Result<String, String> {
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("rustfmt を起動できない: {e}"))?;
    child
        .stdin
        .take()
        .ok_or("rustfmt の標準入力を開けない")?
        .write_all(source.as_bytes())
        .map_err(|e| format!("rustfmt に書けない: {e}"))?;
    let output = child
        .wait_with_output()
        .map_err(|e| format!("rustfmt が終わらない: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "rustfmt が失敗した:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout).map_err(|e| format!("rustfmt の出力が UTF-8 でない: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_missing_and_stale_outputs_without_writing() {
        let dir = formats::tests::TempDir::new();
        let outputs = [(FORMATS_MD, "形式の一覧\n".to_string())];
        assert!(update_outputs(&dir.0, &outputs, Mode::Check).is_err());
        assert!(!dir.0.join(FORMATS_MD).exists());
        update_outputs(&dir.0, &outputs, Mode::Write).unwrap();
        assert!(update_outputs(&dir.0, &outputs, Mode::Check).is_ok());
        fs::write(dir.0.join(FORMATS_MD), "古い一覧\n").unwrap();
        let error = update_outputs(&dir.0, &outputs, Mode::Check).unwrap_err();
        assert!(error.contains(FORMATS_MD));
        assert_eq!(
            fs::read_to_string(dir.0.join(FORMATS_MD)).unwrap(),
            "古い一覧\n"
        );
        update_outputs(&dir.0, &outputs, Mode::Write).unwrap();
        assert!(update_outputs(&dir.0, &outputs, Mode::Check).is_ok());
    }

    #[test]
    fn reads_the_project_version_of_a_pom() {
        let pom = |version: &str| {
            format!(
                "<project><version>{version}</version><dependencies><dependency><version>5.19.1</version></dependency></dependencies></project>"
            )
        };
        assert!(check_pom_version("pom.xml", &pom(PACKAGE_VERSION)).is_ok());
        assert!(check_pom_version("pom.xml", &pom("0.0.0")).is_err());
        assert!(
            check_pom_version(
                "pom.xml",
                "<project><dependencies><version>1</version></dependencies></project>"
            )
            .is_err()
        );
    }
}
