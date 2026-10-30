//! 公開サンプルの台帳を検査する（決定 0171）。
//!
//! `samples/` のファイルと `samples/ledger.toml` の項目が一対一であること、ハッシュとバイト数が合うこと、
//! ライセンスが再配布を認める一覧にあること、大きさが上限の内であることを確かめる。

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::formats::{is_date, is_url};
use crate::sha256;

const DIR: &str = "samples";
const LEDGER: &str = "samples/ledger.toml";
/// `samples/` の直下に置く、サンプルでないファイル
const OWN_FILES: &[&str] = &["README.md", "ledger.toml"];
/// 1 つのファイルの上限
const MAX_FILE_BYTES: u64 = 5_000_000;
/// `samples/` の全体の上限
const MAX_TOTAL_BYTES: u64 = 100_000_000;
/// 目的を問わず、再配布・改変・商用の利用を、条件なしか表示だけを条件に認めるライセンス。
/// SPDX の識別子か、規約の名前と版で書く
const LICENSES: &[&str] = &[
    "パブリックドメイン",
    "CC0-1.0",
    "CC-BY-2.0",
    "CC-BY-2.1-JP",
    "CC-BY-2.5",
    "CC-BY-3.0",
    "CC-BY-4.0",
    "公共データ利用規約（第 1.0 版）",
    "政府標準利用規約（第 2.0 版）",
    "MIT",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "Apache-2.0",
    "ISC",
    "Zlib",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ledger {
    #[serde(default)]
    samples: Vec<Sample>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Sample {
    /// `samples/` からの道筋
    path: String,
    sha256: String,
    bytes: u64,
    title: String,
    publisher: String,
    /// 出所の URL
    url: String,
    /// 取得した日
    retrieved: String,
    license: String,
    license_url: String,
    /// 表示の文
    attribution: String,
    /// 取得したものから変えた内容
    changes: Option<String>,
}

/// サンプルのファイルの大きさとハッシュ
struct Found {
    bytes: u64,
    sha256: String,
}

pub fn check(root: &Path) -> Result<(), String> {
    let text =
        fs::read_to_string(root.join(LEDGER)).map_err(|e| format!("{LEDGER} を読めない: {e}"))?;
    let ledger: Ledger = toml::from_str(&text).map_err(|e| format!("{LEDGER}: {e}"))?;
    let mut found = BTreeMap::new();
    let mut errors = Vec::new();
    collect(&root.join(DIR), "", &mut found, &mut errors)?;
    errors.extend(check_ledger(&ledger, &found));
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "公開サンプルの台帳の検査で問題がある:\n- {}",
            errors.join("\n- ")
        ))
    }
}

/// `dir` の中のファイルを、`samples/` からの道筋で集める
fn collect(
    dir: &Path,
    prefix: &str,
    found: &mut BTreeMap<String, Found>,
    errors: &mut Vec<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("{} を読めない: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{} を読めない: {e}", dir.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{prefix}{name}");
        let kind = entry
            .file_type()
            .map_err(|e| format!("{DIR}/{path} を読めない: {e}"))?;
        if kind.is_dir() {
            collect(&entry.path(), &format!("{path}/"), found, errors)?;
        } else if !kind.is_file() {
            errors.push(format!("{DIR}/{path}: 通常のファイルでない"));
        } else if !(prefix.is_empty() && OWN_FILES.contains(&name.as_str())) {
            let data =
                fs::read(entry.path()).map_err(|e| format!("{DIR}/{path} を読めない: {e}"))?;
            let bytes = data.len() as u64;
            found.insert(
                path,
                Found {
                    bytes,
                    sha256: sha256::hex(&data),
                },
            );
        }
    }
    Ok(())
}

fn check_ledger(ledger: &Ledger, found: &BTreeMap<String, Found>) -> Vec<String> {
    let mut errors = Vec::new();
    let mut listed = BTreeSet::new();
    for sample in &ledger.samples {
        let at = format!("{LEDGER} の {}", sample.path);
        if !is_sample_path(&sample.path) {
            errors.push(format!(
                "{at}: path は `<形式>/<名前>` にする（形式は英小文字・数字）"
            ));
        }
        if !listed.insert(sample.path.as_str()) {
            errors.push(format!("{at}: 同じファイルを 2 度書いている"));
        }
        match found.get(&sample.path) {
            None => errors.push(format!("{at}: ファイルがない")),
            Some(file) => {
                if file.bytes != sample.bytes {
                    errors.push(format!(
                        "{at}: bytes が {}（ファイルは {}）",
                        sample.bytes, file.bytes
                    ));
                }
                if file.sha256 != sample.sha256 {
                    errors.push(format!(
                        "{at}: sha256 がファイルと合わない（ファイルは {}）",
                        file.sha256
                    ));
                }
            }
        }
        if sample.bytes > MAX_FILE_BYTES {
            errors.push(format!(
                "{at}: {} バイトで、1 つのファイルの上限 {MAX_FILE_BYTES} を超える。`ZUMEN_SAMPLES` で扱う",
                sample.bytes
            ));
        }
        for (key, value) in [
            ("title", &sample.title),
            ("publisher", &sample.publisher),
            ("attribution", &sample.attribution),
        ] {
            if value.trim().is_empty() {
                errors.push(format!("{at}: {key} が空"));
            }
        }
        if sample
            .changes
            .as_deref()
            .is_some_and(|c| c.trim().is_empty())
        {
            errors.push(format!("{at}: changes が空。変えていなければ書かない"));
        }
        for (key, value) in [("url", &sample.url), ("license-url", &sample.license_url)] {
            if !is_url(value) {
                errors.push(format!(
                    "{at}: {key} はホストのある HTTP または HTTPS の URL にする"
                ));
            }
        }
        if !is_date(&sample.retrieved) {
            errors.push(format!("{at}: retrieved は実在する日付 YYYY-MM-DD にする"));
        }
        if !LICENSES.contains(&sample.license.as_str()) {
            errors.push(format!(
                "{at}: license の `{}` は、再配布を認めるライセンスの一覧（決定 0171）にない",
                sample.license
            ));
        }
    }
    for path in found.keys() {
        if !listed.contains(path.as_str()) {
            errors.push(format!("{DIR}/{path}: 台帳に書いていない"));
        }
    }
    let total: u64 = found.values().map(|file| file.bytes).sum();
    if total > MAX_TOTAL_BYTES {
        errors.push(format!(
            "{DIR}/ の全体が {total} バイトで、上限 {MAX_TOTAL_BYTES} を超える。上限を変えるには新しい決定が要る"
        ));
    }
    errors
}

/// `<形式>/<名前>` の形か。名前の中にはフォルダーを置ける
fn is_sample_path(path: &str) -> bool {
    let mut parts = path.split('/');
    let format = parts.next().unwrap_or_default();
    let rest: Vec<&str> = parts.collect();
    !format.is_empty()
        && format
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && !rest.is_empty()
        && rest
            .iter()
            .all(|part| !part.is_empty() && *part != "." && *part != ".." && !part.contains('\\'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::tests::TempDir;

    const ENTRY: &str = r#"
[[samples]]
path = "dxf/a.dxf"
sha256 = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
bytes = 3
title = "試験用の図面"
publisher = "試験用の公開元"
url = "https://example.com/a.dxf"
retrieved = "2026-10-30"
license = "CC-BY-4.0"
license-url = "https://creativecommons.org/licenses/by/4.0/"
attribution = "試験用の公開元「試験用の図面」（CC BY 4.0）"
"#;

    fn ledger(text: &str) -> Ledger {
        toml::from_str(text).unwrap()
    }

    fn files(entries: &[(&str, &[u8])]) -> BTreeMap<String, Found> {
        entries
            .iter()
            .map(|(path, data)| {
                (
                    (*path).to_string(),
                    Found {
                        bytes: data.len() as u64,
                        sha256: sha256::hex(data),
                    },
                )
            })
            .collect()
    }

    #[test]
    fn accepts_matching_files_and_an_empty_ledger() {
        assert!(check_ledger(&ledger(ENTRY), &files(&[("dxf/a.dxf", b"abc")])).is_empty());
        assert!(check_ledger(&ledger(""), &files(&[])).is_empty());
    }

    #[test]
    fn requires_each_file_and_entry_once() {
        let errors = check_ledger(&ledger(ENTRY), &files(&[("dxf/b.dxf", b"abc")]));
        assert!(
            errors.iter().any(|e| e.contains("ファイルがない")),
            "{errors:?}"
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("dxf/b.dxf: 台帳に書いていない")),
            "{errors:?}"
        );

        let twice = format!("{ENTRY}{ENTRY}");
        let errors = check_ledger(&ledger(&twice), &files(&[("dxf/a.dxf", b"abc")]));
        assert!(errors.iter().any(|e| e.contains("2 度")), "{errors:?}");
    }

    #[test]
    fn rejects_changed_bytes() {
        let errors = check_ledger(&ledger(ENTRY), &files(&[("dxf/a.dxf", b"abd")]));
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("sha256"));
        let errors = check_ledger(&ledger(ENTRY), &files(&[("dxf/a.dxf", b"abcd")]));
        assert!(
            errors.iter().any(|e| e.contains("bytes が 3")),
            "{errors:?}"
        );
    }

    #[test]
    fn rejects_licenses_outside_the_list() {
        for license in ["CC-BY-SA-4.0", "CC-BY-NC-4.0", "GPL-3.0-only", ""] {
            let text = ENTRY.replace("\"CC-BY-4.0\"", &format!("\"{license}\""));
            let errors = check_ledger(&ledger(&text), &files(&[("dxf/a.dxf", b"abc")]));
            assert!(
                errors.iter().any(|e| e.contains("license の")),
                "{license}: {errors:?}"
            );
        }
    }

    #[test]
    fn rejects_missing_fields_and_bad_values() {
        let text = ENTRY
            .replace("\"試験用の図面\"", "\" \"")
            .replace("https://example.com/a.dxf", "example.com/a.dxf")
            .replace("2026-10-30", "2026-02-30")
            .replace("attribution", "changes = \"\"\nattribution");
        let errors = check_ledger(&ledger(&text), &files(&[("dxf/a.dxf", b"abc")]));
        for expected in ["title が空", "url は", "retrieved", "changes が空"] {
            assert!(
                errors.iter().any(|e| e.contains(expected)),
                "{expected}: {errors:?}"
            );
        }
        assert!(toml::from_str::<Ledger>(&ENTRY.replace("title", "name")).is_err());
    }

    #[test]
    fn rejects_paths_outside_a_format_folder() {
        for path in [
            "a.dxf",
            "DXF/a.dxf",
            "dxf/../a.dxf",
            "dxf//a.dxf",
            "/a.dxf",
            "dxf/",
        ] {
            assert!(!is_sample_path(path), "{path}");
        }
        assert!(is_sample_path("sxf/set/a.p21"));
    }

    #[test]
    fn limits_the_size_of_each_file_and_the_whole() {
        let big = ENTRY.replace("bytes = 3", "bytes = 5000001");
        let errors = check_ledger(&ledger(&big), &files(&[("dxf/a.dxf", b"abc")]));
        assert!(
            errors.iter().any(|e| e.contains("上限 5000000")),
            "{errors:?}"
        );

        let mut found = files(&[]);
        for i in 0..21 {
            found.insert(
                format!("dxf/{i}.dxf"),
                Found {
                    bytes: MAX_FILE_BYTES,
                    sha256: String::new(),
                },
            );
        }
        let errors = check_ledger(&ledger(""), &found);
        assert!(
            errors.iter().any(|e| e.contains("全体が 105000000")),
            "{errors:?}"
        );
    }

    #[test]
    fn collects_files_below_the_folder_but_not_its_own_files() {
        let dir = TempDir::new();
        let samples = dir.0.join(DIR);
        fs::create_dir_all(samples.join("dxf/set")).unwrap();
        fs::write(samples.join("README.md"), "説明").unwrap();
        fs::write(samples.join("ledger.toml"), "").unwrap();
        fs::write(
            samples.join("dxf/README.md"),
            "形式ごとの説明もサンプルと同じに扱う",
        )
        .unwrap();
        fs::write(samples.join("dxf/set/a.dxf"), "abc").unwrap();
        let error = check(&dir.0).unwrap_err();
        assert!(
            error.contains("samples/dxf/README.md: 台帳に書いていない"),
            "{error}"
        );
        assert!(
            error.contains("samples/dxf/set/a.dxf: 台帳に書いていない"),
            "{error}"
        );
        assert!(!error.contains("samples/README.md"), "{error}");
    }
}
