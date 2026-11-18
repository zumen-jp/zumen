//! 電子納品の管理ファイルの読みと書き戻しを、要領が配る DTD と lxml 6.1.3 で照らす（#505）。
//!
//! 国土交通省の作成例と DTD は転載と改変が禁じられているので、同梱せず、テストも取得しない。利用者が
//! 電子納品の頁から入手して展開したフォルダーを `ZUMEN_CALS_DTD` で渡したときだけ動く。管理ファイルの作成例を
//! 1 つずつ、別のプロセスで動く `xtask/verify/dtd/check.py` に渡し、隣の DTD で検証させて要素の木と照らす。
//! 書き戻しは、DTD が文字を許す項目の値をすべて置き換えた XML を、同じ DTD で検証させる。
//! 手順は `xtask/verify/dtd/README.md`。

#![cfg(test)]

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics};
use zumen_delivery::{Edit, PathStep, Record, read, write_xml};

/// 照らす管理ファイルと、その DTD（工事・業務・地質の要領の令和 8 年 2 月の版）
const MANAGED: &[(&str, &str)] = &[
    ("INDEX_C.XML", "INDE_C09.DTD"),
    ("REGISTER.XML", "REGIST06.DTD"),
    ("MEET.XML", "MEET05.DTD"),
    ("PLAN.XML", "PLAN05.DTD"),
    ("OTHRS.XML", "OTHRS05.DTD"),
    ("INDEX_D.XML", "INDE_D08.DTD"),
    ("REGISTER.XML", "REGIST07.DTD"),
    ("REPORT.XML", "REP05.DTD"),
    ("OPENREP.XML", "OPREP01.DTD"),
    ("BORING.XML", "BRG0200.DTD"),
    ("COREPIC.XML", "CPIC0200.DTD"),
    ("GRNDTST.XML", "GTST0300.DTD"),
    ("OTHRFLS.XML", "OTHR0110.DTD"),
];

/// libxml2 の Shift_JIS は JIS X 0201・JIS X 0208 の対応で、zumen は WHATWG Encoding（Windows の符号）の対応で
/// 復号する（[XML の規則](../../../docs/formats/xml.md#shift_jis-の名前)）。対応の違う 8 字を WHATWG の側に
/// そろえて比べる
const JIS_TO_WHATWG: &[(char, char)] = &[
    ('\u{00A5}', '\\'),
    ('\u{203E}', '~'),
    ('\u{301C}', '\u{FF5E}'),
    ('\u{2016}', '\u{2225}'),
    ('\u{2212}', '\u{FF0D}'),
    ('\u{00A2}', '\u{FFE0}'),
    ('\u{00A3}', '\u{FFE1}'),
    ('\u{00AC}', '\u{FFE2}'),
];

type Oracle = (PathBuf, PathBuf);

/// 要素の深さ・名前・値
type Row = (usize, String, Option<String>);

/// lxml の読み。文書型宣言（根の名前・システム識別子）と、要素の行と DTD がその要素に文字を許すか
type Dump = ((String, String), Vec<(Row, bool)>);

/// 作成例のフォルダーと lxml。フォルダーがなければ飛ばす。フォルダーを渡したときは lxml がないと失敗にする
// コアの決定性とは別に、テストのツールの起動条件を読む。
#[allow(clippy::disallowed_methods, clippy::print_stderr)]
fn setup() -> Option<(PathBuf, Oracle)> {
    let Some(folder) = std::env::var_os("ZUMEN_CALS_DTD") else {
        assert!(
            std::env::var_os("ZUMEN_DTD_REQUIRED").is_none(),
            "ZUMEN_CALS_DTD に作成例のフォルダーが要る"
        );
        eprintln!("ZUMEN_CALS_DTD がないため DTD の照合を飛ばす（xtask/verify/dtd/README.md）");
        return None;
    };
    let python = std::env::var_os("ZUMEN_DTD_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| "python".into());
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../xtask/verify/dtd/check.py");
    let output = Command::new(&python)
        .arg("-I")
        .arg(&script)
        .arg("--check")
        .output()
        .unwrap();
    assert!(output.status.success(), "lxml の起動に失敗: {output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "lxml\t6.1.3"
    );
    Some((PathBuf::from(folder), (python, script)))
}

/// フォルダーの下の、名前が `MANAGED` にあるファイル。道筋の順
fn find(folder: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            find(&path, out);
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| MANAGED.iter().any(|(file, _)| *file == name))
        {
            out.push(path);
        }
    }
}

/// lxml に検証させて読ませる。検証に失敗すれば `Err` に lxml の最初の誤り
fn external(oracle: &Oracle, folder: &Path, bytes: &[u8]) -> Result<Dump, String> {
    let mut child = Command::new(&oracle.0)
        .arg("-I")
        .arg("-X")
        .arg("utf8")
        .arg(&oracle.1)
        .arg(folder)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "lxml が動かなかった: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    let mut lines = text.lines();
    let first = lines.next().unwrap();
    if let Some(error) = first.strip_prefix("invalid\t") {
        return Err(unescape(error));
    }
    assert_eq!(first, "valid");
    let mut doctype = None;
    let mut rows = Vec::new();
    for line in lines {
        let fields: Vec<&str> = line.split('\t').collect();
        match fields.as_slice() {
            ["doctype", root, system] => doctype = Some((unescape(root), unescape(system))),
            ["element", depth, name, kind, has, value] => rows.push((
                (
                    depth.parse().unwrap(),
                    unescape(name),
                    (*has == "1").then(|| whatwg(&unescape(value))),
                ),
                matches!(*kind, "mixed" | "pcdata" | "any"),
            )),
            _ => panic!("読めない行: {line}"),
        }
    }
    Ok((doctype.unwrap(), rows))
}

fn unescape(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn whatwg(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            JIS_TO_WHATWG
                .iter()
                .find(|(jis, _)| *jis == c)
                .map_or(c, |(_, whatwg)| *whatwg)
        })
        .collect()
}

/// 要素の行と、子の要素を持たない項目なら根からの経路。文書の順
fn flatten(
    record: &Record,
    depth: usize,
    path: &mut Vec<PathStep>,
    out: &mut Vec<(Row, Option<Vec<PathStep>>)>,
) {
    let leaf = record.children.is_empty().then(|| path.clone());
    out.push(((depth, record.name.clone(), record.value.clone()), leaf));
    let mut seen: Vec<&str> = Vec::new();
    for child in &record.children {
        let index = seen.iter().filter(|name| **name == child.name).count();
        seen.push(&child.name);
        path.push(PathStep {
            name: child.name.clone(),
            index: index as u64,
        });
        flatten(child, depth + 1, path, out);
        path.pop();
    }
}

fn rows(record: &Record) -> Vec<(Row, Option<Vec<PathStep>>)> {
    let mut out = Vec::new();
    flatten(record, 0, &mut Vec::new(), &mut out);
    out
}

fn with_cx<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Vec<Code>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let out = {
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        f(&mut cx)
    };
    (out, diag.iter().map(|d| d.code()).collect())
}

/// コアで読み、文書型宣言のシステム識別子と根の記録を返す
fn core(name: &str, bytes: &[u8]) -> (Option<String>, Record) {
    let (package, codes) = with_cx(|cx| read(&[(name, bytes)], cx));
    assert!(codes.is_empty(), "{name}: {codes:?}");
    let mut managed = package.unwrap().managed;
    assert_eq!(managed.len(), 1, "{name}");
    let managed = managed.remove(0);
    (managed.doctype, managed.root)
}

/// 作成例を読み、ファイルの名前・バイト列・DTD のフォルダーを返す
fn examples(folder: &Path) -> Vec<(String, Vec<u8>, PathBuf)> {
    let mut files = Vec::new();
    find(folder, &mut files);
    files
        .iter()
        .map(|path| {
            (
                path.file_name().unwrap().to_str().unwrap().to_string(),
                std::fs::read(path).unwrap(),
                path.parent().unwrap().to_path_buf(),
            )
        })
        .collect()
}

#[test]
fn management_files_read_as_lxml_reads_them_after_dtd_validation() {
    let Some((folder, oracle)) = setup() else {
        return;
    };
    let mut seen = BTreeSet::new();
    for (name, bytes, dtd) in examples(&folder) {
        let ((root, system), lxml) =
            external(&oracle, &dtd, &bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        let (doctype, record) = core(&name, &bytes);
        assert_eq!(doctype.as_deref(), Some(system.as_str()), "{name}");
        assert_eq!(record.name, root, "{name}");
        let core: Vec<Row> = rows(&record).into_iter().map(|(row, _)| row).collect();
        let lxml: Vec<Row> = lxml.into_iter().map(|(row, _)| row).collect();
        assert_eq!(core, lxml, "{name}");
        assert!(
            MANAGED.contains(&(name.as_str(), system.as_str())),
            "{name}: 知らない DTD {system}"
        );
        assert!(seen.insert(system), "{name}: 同じ DTD の作成例が 2 つある");
    }
    let expected: BTreeSet<String> = MANAGED.iter().map(|(_, dtd)| dtd.to_string()).collect();
    assert_eq!(seen, expected, "作成例が足りない");
}

#[test]
fn rewritten_management_files_stay_valid_under_the_dtd() {
    let Some((folder, oracle)) = setup() else {
        return;
    };
    let examples = examples(&folder);
    assert_eq!(examples.len(), MANAGED.len(), "作成例が足りない");
    for (name, bytes, dtd) in examples {
        let (_, lxml) = external(&oracle, &dtd, &bytes).unwrap();
        let (_, record) = core(&name, &bytes);
        let mut expected = rows(&record);
        assert_eq!(expected.len(), lxml.len(), "{name}");
        // DTD が文字を許す、子の要素を持たない項目だけを変える。値は文字参照にする記号と、Shift_JIS で書ける日本語
        let mut paths = Vec::new();
        let mut values = Vec::new();
        for ((row, leaf), (_, allowed)) in expected.iter_mut().zip(&lxml) {
            if let (Some(path), true) = (leaf.take(), *allowed) {
                let value = format!("更新{} <&> \"'値", values.len());
                row.2 = Some(value.clone());
                paths.push(path);
                values.push(value);
            }
        }
        assert!(!paths.is_empty(), "{name}");
        let edits: Vec<Edit<'_>> = paths
            .iter()
            .zip(&values)
            .map(|(path, value)| Edit::SetText { path, value })
            .collect();
        let (written, codes) = with_cx(|cx| write_xml(&bytes, &edits, cx));
        assert!(codes.is_empty(), "{name}: {codes:?}");
        let written = written.unwrap();
        let (_, after) = external(&oracle, &dtd, &written)
            .unwrap_or_else(|error| panic!("{name} の書き戻し: {error}"));
        let expected: Vec<Row> = expected.into_iter().map(|(row, _)| row).collect();
        let after: Vec<Row> = after.into_iter().map(|(row, _)| row).collect();
        assert_eq!(after, expected, "{name} の書き戻し");
        let (_, reread) = core(&name, &written);
        let reread: Vec<Row> = rows(&reread).into_iter().map(|(row, _)| row).collect();
        assert_eq!(reread, expected, "{name} の再読");
    }
}
