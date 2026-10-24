//! リポジトリの作業。`cargo xtask <コマンド>` で使う（決定 0011）。
//!
//! - `codegen`: 診断の code の台帳から、Rust の定数を生成する
//! - `codegen --check`: 台帳とカタログを確かめ、生成したファイルが最新かを確かめる

mod catalog;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

const USAGE: &str = "使い方: cargo xtask codegen [--check]";

/// 生成した code の定数の置き場所（リポジトリの根からの相対パス）
const CODES_RS: &str = "crates/zumen-base/src/codes.rs";

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
    let read = |name: &str| {
        let path = root.join("catalog").join(name);
        fs::read_to_string(&path).map_err(|e| format!("{} を読めない: {e}", path.display()))
    };
    let registry: catalog::Registry =
        toml::from_str(&read("codes.toml")?).map_err(|e| format!("catalog/codes.toml: {e}"))?;
    let ja: catalog::Messages =
        toml::from_str(&read("ja.toml")?).map_err(|e| format!("catalog/ja.toml: {e}"))?;
    let en: catalog::Messages =
        toml::from_str(&read("en.toml")?).map_err(|e| format!("catalog/en.toml: {e}"))?;

    let errors = catalog::check(&registry, &[("ja.toml", &ja), ("en.toml", &en)]);
    if !errors.is_empty() {
        return Err(format!(
            "台帳とカタログに問題がある:\n- {}",
            errors.join("\n- ")
        ));
    }

    let generated = rustfmt(&root, &catalog::render(&registry))?;
    let path = root.join(CODES_RS);
    let current = fs::read_to_string(&path).unwrap_or_default();
    match mode {
        Mode::Check if current != generated => Err(format!(
            "{CODES_RS} が台帳と合わない。`cargo xtask codegen` を実行して、生成したファイルをコミットする"
        )),
        Mode::Check => Ok(()),
        Mode::Write if current == generated => Ok(()),
        Mode::Write => {
            fs::write(&path, generated).map_err(|e| format!("{CODES_RS} に書けない: {e}"))
        }
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
