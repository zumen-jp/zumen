//! リリースのワークフローが crates.io に出す crate の一覧を検査する（決定 0205・0259）。
//!
//! `.github/workflows/release.yml` の `CRATES` に、crates.io に出すワークスペースの crate がすべてあり、どの crate も
//! ワークスペースの中の依存より後ろに並ぶことを確かめる。crate を足したときに一覧へ足し忘れると、`cargo publish` が
//! 一覧にない crate を crates.io で探して失敗する。

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Deserialize;

const WORKFLOW: &str = ".github/workflows/release.yml";

#[derive(Debug, Deserialize)]
struct Root {
    workspace: Workspace,
}

#[derive(Debug, Deserialize)]
struct Workspace {
    members: Vec<String>,
    #[serde(default)]
    package: Option<WorkspacePackage>,
}

#[derive(Debug, Deserialize)]
struct WorkspacePackage {
    #[serde(default)]
    publish: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    package: Package,
    #[serde(default)]
    dependencies: BTreeMap<String, toml::Value>,
    #[serde(default, rename = "build-dependencies")]
    build_dependencies: BTreeMap<String, toml::Value>,
    #[serde(default)]
    target: BTreeMap<String, Target>,
}

#[derive(Debug, Deserialize)]
struct Package {
    name: String,
    #[serde(default)]
    publish: Option<toml::Value>,
}

#[derive(Debug, Deserialize)]
struct Target {
    #[serde(default)]
    dependencies: BTreeMap<String, toml::Value>,
    #[serde(default, rename = "build-dependencies")]
    build_dependencies: BTreeMap<String, toml::Value>,
}

/// ワークスペースの crate。依存は、dev を除いた、ワークスペースの中のものに限る
#[derive(Debug)]
struct Crate {
    publish: bool,
    dependencies: BTreeSet<String>,
}

pub fn check(root: &Path) -> Result<(), String> {
    let crates = workspace(root)?;
    let workflow = fs::read_to_string(root.join(WORKFLOW))
        .map_err(|e| format!("{WORKFLOW} を読めない: {e}"))?;
    let listed = listed_crates(&workflow)?;
    let errors = check_list(&listed, &crates);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{WORKFLOW} の CRATES に問題がある（決定 0205）:\n- {}",
            errors.join("\n- ")
        ))
    }
}

/// ワークスペースの crate を、名前から引けるように集める
fn workspace(root: &Path) -> Result<BTreeMap<String, Crate>, String> {
    let text = fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|e| format!("Cargo.toml を読めない: {e}"))?;
    let root_manifest: Root = toml::from_str(&text).map_err(|e| format!("Cargo.toml: {e}"))?;
    let inherited = root_manifest
        .workspace
        .package
        .and_then(|package| package.publish)
        .unwrap_or(true);
    let mut manifests = Vec::new();
    for member in &root_manifest.workspace.members {
        if let Some(dir) = member.strip_suffix("/*") {
            let entries =
                fs::read_dir(root.join(dir)).map_err(|e| format!("{dir} を読めない: {e}"))?;
            let mut paths = Vec::new();
            for entry in entries {
                let entry = entry.map_err(|e| format!("{dir} を読めない: {e}"))?;
                let manifest = entry.path().join("Cargo.toml");
                if manifest.is_file() {
                    paths.push(manifest);
                }
            }
            paths.sort();
            manifests.extend(paths);
        } else {
            manifests.push(root.join(member).join("Cargo.toml"));
        }
    }
    let mut parsed = Vec::new();
    for path in manifests {
        let text =
            fs::read_to_string(&path).map_err(|e| format!("{} を読めない: {e}", path.display()))?;
        let manifest: Manifest =
            toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let publish = match &manifest.package.publish {
            None => true,
            Some(toml::Value::Boolean(publish)) => *publish,
            Some(toml::Value::Table(table))
                if table.get("workspace") == Some(&toml::Value::Boolean(true)) =>
            {
                inherited
            }
            Some(other) => {
                return Err(format!(
                    "{}: publish の値（{other}）を読めない。true か false にする",
                    path.display()
                ));
            }
        };
        parsed.push((manifest, publish));
    }
    let names: BTreeSet<String> = parsed
        .iter()
        .map(|(manifest, _)| manifest.package.name.clone())
        .collect();
    let mut crates = BTreeMap::new();
    for (manifest, publish) in parsed {
        let tables = [&manifest.dependencies, &manifest.build_dependencies]
            .into_iter()
            .chain(
                manifest
                    .target
                    .values()
                    .flat_map(|target| [&target.dependencies, &target.build_dependencies]),
            );
        let dependencies = tables
            .flat_map(|table| table.iter().map(|(key, value)| package_name(key, value)))
            .filter(|name| names.contains(name))
            .collect();
        crates.insert(
            manifest.package.name,
            Crate {
                publish,
                dependencies,
            },
        );
    }
    Ok(crates)
}

/// 依存の表の項目が指す crate の名前。`package` で別の名前にしていれば、その名前
fn package_name(key: &str, value: &toml::Value) -> String {
    match value.get("package") {
        Some(toml::Value::String(package)) => package.clone(),
        _ => key.to_string(),
    }
}

/// ワークフローの `CRATES:` の値から、`-p` で渡す crate の名前を並びのまま取り出す
fn listed_crates(workflow: &str) -> Result<Vec<String>, String> {
    let mut lines = workflow.lines();
    let indent = loop {
        let Some(line) = lines.next() else {
            return Err(format!("{WORKFLOW} に CRATES がない"));
        };
        if line.trim_start().starts_with("CRATES:") {
            break line.len() - line.trim_start().len();
        }
    };
    let mut words = Vec::new();
    for line in lines {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || line.len() - trimmed.len() <= indent {
            break;
        }
        words.extend(trimmed.split_whitespace());
    }
    let mut listed = Vec::new();
    let mut words = words.into_iter();
    while let Some(word) = words.next() {
        match (word, words.next()) {
            ("-p", Some(name)) => listed.push(name.to_string()),
            _ => return Err(format!("{WORKFLOW} の CRATES を `-p <crate>` の並びで書く")),
        }
    }
    Ok(listed)
}

fn check_list(listed: &[String], crates: &BTreeMap<String, Crate>) -> Vec<String> {
    let mut errors = Vec::new();
    let mut position = BTreeMap::new();
    for (index, name) in listed.iter().enumerate() {
        if position.insert(name.as_str(), index).is_some() {
            errors.push(format!("{name} が 2 回ある"));
        }
        match crates.get(name) {
            None => errors.push(format!("{name} は、ワークスペースの crate でない")),
            Some(found) if !found.publish => {
                errors.push(format!(
                    "{name} は publish = false で、crates.io に出さない"
                ));
            }
            Some(_) => {}
        }
    }
    for (name, found) in crates {
        if found.publish && !position.contains_key(name.as_str()) {
            errors.push(format!("{name} を足す（publish = true の crate）"));
        }
    }
    for (index, name) in listed.iter().enumerate() {
        let Some(found) = crates.get(name) else {
            continue;
        };
        for dependency in &found.dependencies {
            match (crates.get(dependency), position.get(dependency.as_str())) {
                (Some(target), _) if !target.publish => errors.push(format!(
                    "{name} が、crates.io に出さない {dependency} に依存している"
                )),
                (_, Some(&at)) if at > index => {
                    errors.push(format!("{name} を、依存する {dependency} より後ろに置く"));
                }
                _ => {}
            }
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crates(entries: &[(&str, bool, &[&str])]) -> BTreeMap<String, Crate> {
        entries
            .iter()
            .map(|(name, publish, dependencies)| {
                (
                    (*name).to_string(),
                    Crate {
                        publish: *publish,
                        dependencies: dependencies
                            .iter()
                            .map(|name| (*name).to_string())
                            .collect(),
                    },
                )
            })
            .collect()
    }

    fn list(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    #[test]
    fn reads_the_crates_in_order() {
        let workflow = "env:\n  CARGO_TERM_COLOR: always\n  # 出す順\n  CRATES: >-\n    -p a -p b\n    -p c\n\njobs:\n";
        assert_eq!(listed_crates(workflow).unwrap(), list(&["a", "b", "c"]));
    }

    #[test]
    fn rejects_a_list_without_p() {
        assert!(listed_crates("env:\n  CRATES: >-\n    -p a b\n").is_err());
        assert!(listed_crates("env:\n  OTHER: 1\n").is_err());
    }

    #[test]
    fn accepts_a_complete_list_in_dependency_order() {
        let crates = crates(&[("a", true, &[]), ("b", true, &["a"]), ("t", false, &["a"])]);
        assert!(check_list(&list(&["a", "b"]), &crates).is_empty());
    }

    #[test]
    fn finds_missing_unknown_unpublished_and_misordered_crates() {
        let crates = crates(&[
            ("a", true, &[]),
            ("b", true, &["a", "t"]),
            ("c", true, &["b"]),
            ("t", false, &[]),
        ]);
        let errors = check_list(&list(&["b", "a", "x", "t", "a"]), &crates);
        assert_eq!(
            errors,
            [
                "x は、ワークスペースの crate でない",
                "t は publish = false で、crates.io に出さない",
                "a が 2 回ある",
                "c を足す（publish = true の crate）",
                "b を、依存する a より後ろに置く",
                "b が、crates.io に出さない t に依存している",
            ]
        );
    }

    #[test]
    fn the_release_workflow_lists_every_published_crate() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        check(&root).unwrap();
    }
}
