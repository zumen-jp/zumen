//! fuzz の target を検査し、種の入力を書く（#501、決定 0010・0017）。
//!
//! 検査は `codegen` から呼ぶ。登録した形式と L0 の crate のどれにも、同じ名前の fuzz の target があることを確かめる。
//! 登録した形式は、crate が `[package.metadata.zumen]` の `formats` で名乗る形式と、`schema/boundary.toml` の
//! `[[detection]]` と `[[modules]]` の形式。target は `fuzz/Cargo.toml` の `[[bin]]` で、`fuzz/fuzz_targets/<名前>.rs`
//! に置き、テストキットの fuzz の関数（`survives`・`survives_lift`・`survives_with`）を呼ぶ。
//!
//! `cargo xtask fuzz-seeds` は、公開サンプル、適合試験の組の合成の入力、L0 の攻撃の形（ZIP 爆弾・書庫の入れ子・
//! パスの走査・XML の外部実体と深い入れ子・ISO 10303-21 の深い入れ子）を、どの target にも種として書く。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::Command;

use serde::Deserialize;
use zumen_json::Value;

use crate::boundary::Boundary;
use crate::sha256;

/// fuzz の crate のフォルダー
const DIR: &str = "fuzz";
/// 形式ではないが、受け取ったバイト列を読む L0 の crate（決定 0008）。target の名前にする
const L0: &[&str] = &["zip", "xml", "step"];
/// target が呼ぶ、テストキットの fuzz の関数の始まり
const TESTKIT: &str = "zumen_sdk::testkit::survives";
/// 深い入れ子の種の段の数。fuzz の予算の深さ（既定の 256）を超える
const DEEP: usize = 1000;
/// 空の deflate のデータ（固定のハフマン符号の、終わりの印だけの区画）
const EMPTY_DEFLATE: &[u8] = &[0x03, 0x00];

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(default)]
    bin: Vec<Bin>,
}

/// fuzz の target（`[[bin]]`）
#[derive(Debug, Deserialize)]
struct Bin {
    name: String,
    path: String,
}

pub fn check(root: &Path, boundary: &Boundary) -> Result<(), String> {
    let mut required = declared(root)?;
    for detection in &boundary.detection {
        required.insert(detection.format.clone());
    }
    for module in &boundary.modules {
        required.extend(module.formats.iter().cloned());
    }
    required.extend(L0.iter().map(|name| (*name).to_string()));
    let errors = check_targets(&required, &bins(root)?, &files(root)?);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "fuzz の target の検査で問題がある（fuzz/README.md）:\n- {}",
            errors.join("\n- ")
        ))
    }
}

/// ワークスペースの crate が `[package.metadata.zumen]` の `formats` で名乗る形式
fn declared(root: &Path) -> Result<BTreeSet<String>, String> {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("cargo metadata を起動できない: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata が失敗した:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let metadata = zumen_json::parse(&output.stdout)
        .map_err(|e| format!("cargo metadata の JSON を読めない（位置 {}）", e.offset))?;
    let Some(Value::Array(packages)) = metadata.pointer("/packages") else {
        return Err("cargo metadata: packages の配列がない".to_string());
    };
    Ok(packages
        .iter()
        .filter_map(|package| match package.pointer("/metadata/zumen/formats") {
            Some(Value::Array(formats)) => Some(formats),
            _ => None,
        })
        .flatten()
        .filter_map(|format| match format {
            Value::String(format) => Some(format.clone()),
            _ => None,
        })
        .collect())
}

fn bins(root: &Path) -> Result<Vec<Bin>, String> {
    let path = format!("{DIR}/Cargo.toml");
    let text =
        fs::read_to_string(root.join(&path)).map_err(|e| format!("{path} を読めない: {e}"))?;
    let manifest: Manifest = toml::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    Ok(manifest.bin)
}

/// `fuzz/fuzz_targets` のファイルの名前と中身
fn files(root: &Path) -> Result<BTreeMap<String, String>, String> {
    let dir = format!("{DIR}/fuzz_targets");
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(root.join(&dir)).map_err(|e| format!("{dir} を読めない: {e}"))? {
        let entry = entry.map_err(|e| format!("{dir} を読めない: {e}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let text = fs::read_to_string(entry.path())
            .map_err(|e| format!("{dir}/{name} を読めない: {e}"))?;
        files.insert(name, text);
    }
    Ok(files)
}

/// 登録した形式と L0 の crate に target があり、target とファイルが一対一で、どの target もテストキットの fuzz の
/// 関数を呼ぶことを確かめる
fn check_targets(
    required: &BTreeSet<String>,
    bins: &[Bin],
    files: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut errors = Vec::new();
    let mut names = BTreeSet::new();
    for bin in bins {
        let name = bin.name.as_str();
        if !names.insert(name) {
            errors.push(format!("target `{name}` が重なっている"));
        }
        let file = format!("{name}.rs");
        if bin.path != format!("fuzz_targets/{file}") {
            errors.push(format!(
                "target `{name}` の path は fuzz_targets/{file} にする"
            ));
        }
        match files.get(&file) {
            None => errors.push(format!(
                "target `{name}` のファイル fuzz/fuzz_targets/{file} がない"
            )),
            Some(text) if !text.contains(TESTKIT) => errors.push(format!(
                "target `{name}` が、テストキットの fuzz の関数（{TESTKIT}…）を呼ばない"
            )),
            Some(_) => {}
        }
    }
    for file in files.keys() {
        if !file
            .strip_suffix(".rs")
            .is_some_and(|name| names.contains(name))
        {
            errors.push(format!(
                "fuzz/fuzz_targets/{file} が、fuzz/Cargo.toml の [[bin]] にない"
            ));
        }
    }
    for name in required {
        if !names.contains(name.as_str()) {
            errors.push(format!(
                "`{name}` に fuzz の target がない。fuzz/fuzz_targets/{name}.rs を作り、fuzz/Cargo.toml の [[bin]] に足す"
            ));
        }
    }
    errors
}

/// 公開サンプル、適合試験の組の合成の入力、L0 の攻撃の形を、どの target にも `fuzz/corpus/<target>/` の種として
/// 書く。ファイルの名前は、中身の SHA-256 にする
pub fn seeds(root: &Path) -> Result<(), String> {
    let mut seeds = BTreeMap::new();
    for data in samples(root)?
        .into_iter()
        .chain(cases(root)?)
        .chain(attacks()?)
    {
        seeds.insert(sha256::hex(&data), data);
    }
    let bins = bins(root)?;
    for bin in &bins {
        let dir = format!("{DIR}/corpus/{}", bin.name);
        fs::create_dir_all(root.join(&dir)).map_err(|e| format!("{dir} を作れない: {e}"))?;
        for (name, data) in &seeds {
            fs::write(root.join(&dir).join(name), data)
                .map_err(|e| format!("{dir}/{name} を書けない: {e}"))?;
        }
    }
    println!(
        "{DIR}/corpus の {} の target に、{} の種を書いた",
        bins.len(),
        seeds.len()
    );
    Ok(())
}

/// `samples/ledger.toml` に載せた公開サンプル
fn samples(root: &Path) -> Result<Vec<Vec<u8>>, String> {
    let path = "samples/ledger.toml";
    let text =
        fs::read_to_string(root.join(path)).map_err(|e| format!("{path} を読めない: {e}"))?;
    let ledger: toml::Value = toml::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    let entries = ledger
        .get("samples")
        .and_then(toml::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    entries
        .iter()
        .filter_map(|entry| entry.get("path").and_then(toml::Value::as_str))
        .map(|sample| {
            fs::read(root.join("samples").join(sample))
                .map_err(|e| format!("samples/{sample} を読めない: {e}"))
        })
        .collect()
}

/// 適合試験の組（`bindings/conformance/cases`）の、`push` で入れる合成の入力。ファイルを指すものは公開サンプルなので
/// 除く
fn cases(root: &Path) -> Result<Vec<Vec<u8>>, String> {
    let dir = "bindings/conformance/cases";
    let mut paths = Vec::new();
    for entry in fs::read_dir(root.join(dir)).map_err(|e| format!("{dir} を読めない: {e}"))? {
        let path = entry.map_err(|e| format!("{dir} を読めない: {e}"))?.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            paths.push(path);
        }
    }
    paths.sort();
    let mut inputs = Vec::new();
    for path in paths {
        let text = fs::read(&path).map_err(|e| format!("{} を読めない: {e}", path.display()))?;
        let case = zumen_json::parse(&text)
            .map_err(|e| format!("{} を読めない（位置 {}）", path.display(), e.offset))?;
        let Some(Value::Array(steps)) = case.pointer("/steps") else {
            continue;
        };
        for step in steps {
            match (step.pointer("/push/text"), step.pointer("/push/hex")) {
                (Some(Value::String(text)), _) => inputs.push(text.as_bytes().to_vec()),
                (_, Some(Value::String(hex))) => inputs.push(
                    unhex(hex)
                        .ok_or_else(|| format!("{} の push の hex を読めない", path.display()))?,
                ),
                _ => {}
            }
        }
    }
    Ok(inputs)
}

/// 16 進の文字列を読む。適合試験の組と同じく、空白は飛ばす
fn unhex(hex: &str) -> Option<Vec<u8>> {
    let digits: Vec<u8> = hex
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| c.to_digit(16).and_then(|digit| u8::try_from(digit).ok()))
        .collect::<Option<_>>()?;
    let (pairs, rest) = digits.as_chunks::<2>();
    rest.is_empty()
        .then(|| pairs.iter().map(|[high, low]| high << 4 | low).collect())
}

/// L0 の攻撃の形。どれも、予算で止まるか、読まずに診断を返す
fn attacks() -> Result<Vec<Vec<u8>>, String> {
    let zip = |entries: &[(&str, &[u8])]| {
        zumen_zip::write(entries).ok_or_else(|| "種の ZIP を書けない".to_string())
    };
    // 書庫の外を指す名前（パスの走査）
    let traversal = zip(&[
        ("../up.txt", b"x".as_slice()),
        ("/root.txt", b"x".as_slice()),
        ("C:/drive.txt", b"x".as_slice()),
        ("a\\..\\..\\b.txt", b"x".as_slice()),
        ("ok.txt", b"x".as_slice()),
    ])?;
    // 書庫の中の書庫の中の書庫
    let inner = zip(&[("leaf.txt", b"leaf".as_slice())])?;
    let middle = zip(&[("inner.zip", inner.as_slice())])?;
    let nested = zip(&[("middle.zip", middle.as_slice())])?;
    // ZIP 爆弾。展開すると 4 GiB 近くになると記録した deflate の項目
    let mut bomb = zip(&[("bomb.txt", EMPTY_DEFLATE)])?;
    claim(&mut bomb, 0xFFFF_FFF0).ok_or_else(|| "種の ZIP 爆弾を作れない".to_string())?;

    let mut laughs = String::from(r#"<?xml version="1.0"?><!DOCTYPE a [<!ENTITY l0 "lol">"#);
    for level in 1..10 {
        let reference = format!("&l{};", level - 1);
        let _ = write!(laughs, r#"<!ENTITY l{level} "{}">"#, reference.repeat(10));
    }
    laughs.push_str("]><a>&l9;</a>");
    Ok(vec![
        traversal,
        nested,
        bomb,
        // 外部の実体と外部の DTD
        br#"<?xml version="1.0"?><!DOCTYPE a [<!ENTITY x SYSTEM "file:///etc/passwd">]><a>&x;</a>"#
            .to_vec(),
        br#"<?xml version="1.0"?><!DOCTYPE a SYSTEM "http://example.invalid/a.dtd"><a/>"#.to_vec(),
        // 実体を重ねて膨らむ文書
        laughs.into_bytes(),
        // 深い入れ子
        format!("{}{}", "<a>".repeat(DEEP), "</a>".repeat(DEEP)).into_bytes(),
        format!(
            "ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n#1=A({}{});\nENDSEC;\nEND-ISO-10303-21;\n",
            "(".repeat(DEEP),
            ")".repeat(DEEP)
        )
        .into_bytes(),
    ])
}

/// 書庫の最初の項目を、deflate で展開すると `size` バイトになると記録したものにする。見出しの欄の位置は、APPNOTE
/// 6.3.10 の 4.3.7（ローカルの見出し）と 4.3.12（中央ディレクトリの見出し）
fn claim(archive: &mut [u8], size: u32) -> Option<()> {
    let local = find(archive, b"PK\x03\x04")?;
    let central = find(archive, b"PK\x01\x02")?;
    for (at, bytes) in [
        (local + 8, 8u16.to_le_bytes().as_slice()),
        (local + 22, size.to_le_bytes().as_slice()),
        (central + 10, 8u16.to_le_bytes().as_slice()),
        (central + 24, size.to_le_bytes().as_slice()),
    ] {
        archive
            .get_mut(at..at + bytes.len())?
            .copy_from_slice(bytes);
    }
    Some(())
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bin(name: &str) -> Bin {
        Bin {
            name: name.to_string(),
            path: format!("fuzz_targets/{name}.rs"),
        }
    }

    fn target(body: &str) -> String {
        format!("fuzz_target!(|data: &[u8]| {{ {body} }});")
    }

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    #[test]
    fn every_registered_format_has_a_target_that_uses_the_testkit() {
        let files = BTreeMap::from([
            (
                "dxf.rs".to_string(),
                target("zumen_sdk::testkit::survives_lift::<Dxf, Drawing2D>(data)"),
            ),
            (
                "zip.rs".to_string(),
                target("zumen_sdk::testkit::survives_with(data, read)"),
            ),
        ]);
        let bins = [bin("dxf"), bin("zip")];
        assert_eq!(
            check_targets(&set(&["dxf", "zip"]), &bins, &files),
            Vec::<String>::new()
        );
        // 登録した形式に target がない
        let errors = check_targets(&set(&["dxf", "las", "zip"]), &bins, &files);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].starts_with("`las` に fuzz の target がない"));
    }

    #[test]
    fn targets_and_files_match_one_to_one() {
        let files = BTreeMap::from([
            ("dxf.rs".to_string(), target("read(data);")),
            (
                "orphan.rs".to_string(),
                target("zumen_sdk::testkit::survives"),
            ),
        ]);
        let mut moved = bin("las");
        moved.path = "targets/las.rs".to_string();
        let errors = check_targets(&set(&[]), &[bin("dxf"), bin("dxf"), moved], &files);
        assert_eq!(
            errors,
            [
                "target `dxf` が、テストキットの fuzz の関数（zumen_sdk::testkit::survives…）を呼ばない",
                "target `dxf` が重なっている",
                "target `dxf` が、テストキットの fuzz の関数（zumen_sdk::testkit::survives…）を呼ばない",
                "target `las` の path は fuzz_targets/las.rs にする",
                "target `las` のファイル fuzz/fuzz_targets/las.rs がない",
                "fuzz/fuzz_targets/orphan.rs が、fuzz/Cargo.toml の [[bin]] にない",
            ]
        );
    }

    #[test]
    fn the_repository_has_a_target_for_every_registered_format() {
        let root = crate::root();
        let boundary: Boundary =
            toml::from_str(&fs::read_to_string(root.join("schema/boundary.toml")).unwrap())
                .unwrap();
        check(&root, &boundary).unwrap();
    }

    #[test]
    fn the_bomb_claims_its_size_in_both_headers() {
        let mut bomb = zumen_zip::write(&[("bomb.txt", EMPTY_DEFLATE)]).unwrap();
        claim(&mut bomb, 0x0102_0304).unwrap();
        let local = find(&bomb, b"PK\x03\x04").unwrap();
        let central = find(&bomb, b"PK\x01\x02").unwrap();
        assert_eq!(bomb[local + 8..local + 10], [8, 0]);
        assert_eq!(bomb[local + 22..local + 26], [4, 3, 2, 1]);
        assert_eq!(bomb[central + 10..central + 12], [8, 0]);
        assert_eq!(bomb[central + 24..central + 28], [4, 3, 2, 1]);
        assert_eq!(unhex("0aFf"), Some(vec![0x0a, 0xff]));
        assert_eq!(unhex("00 ff\n1"), None);
        assert_eq!(unhex("00 ff\n10"), Some(vec![0, 0xff, 0x10]));
        assert_eq!(unhex("0g"), None);
    }
}
