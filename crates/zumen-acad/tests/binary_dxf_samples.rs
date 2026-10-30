//! 試料の、同じ図面のバイナリの DXF（`名前_binary.dxf`）と ASCII の DXF（`名前_ascii.dxf`）を読み、比べる。
//! 試料は公開サンプルの `samples/` と、受け取った図面を置く `ZUMEN_SAMPLES` のフォルダーから集める。

mod support;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use zumen_acad::{Dxf, DxfFile};
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics};
use zumen_sdk::{Codec, Lift, Transact, Transaction};

/// バイナリと ASCII の DXF の組
#[cfg(test)]
fn pairs() -> Vec<(PathBuf, PathBuf)> {
    let mut found: Vec<_> = support::sample_files()
        .into_iter()
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?.strip_suffix("_binary.dxf")?;
            let ascii = path.with_file_name(format!("{name}_ascii.dxf"));
            ascii.exists().then(|| (path.clone(), ascii))
        })
        .collect();
    found.sort();
    found
}

/// 読んだファイルと、出た診断の code
#[cfg(test)]
fn read(src: &[u8]) -> (DxfFile, Vec<Code>) {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let file = Dxf::read(src, &mut Cx::new(&mut budget, &mut diag, &cancel)).unwrap();
    (file, diag.iter().map(|d| d.code()).collect())
}

#[cfg(test)]
fn write(file: &DxfFile) -> Vec<u8> {
    let mut out = Vec::new();
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    Dxf::write(
        file,
        &mut out,
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    )
    .unwrap();
    out
}

/// 図面モデル（出所と XRECORD のハンドルを除き、拡張はキーごとに比べる）と、出た DXF の診断の code
#[cfg(test)]
fn lifted(path: &Path) -> (String, BTreeSet<Code>) {
    let src = std::fs::read(path).unwrap();
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let mut drawing = Dxf::read(&src, &mut cx).unwrap().lift(&mut cx).unwrap();
    support::normalize_extensions(&mut drawing);
    (
        support::without_source_refs(&format!("{drawing:?}")),
        diag.iter().map(|d| d.code()).collect(),
    )
}

/// 数を除いた部分が同じで、数が相対の誤差 `tolerance` の中で同じか
#[cfg(test)]
fn nearly(a: &str, b: &str, tolerance: f64) -> bool {
    let split = |s: &str| -> Vec<String> {
        let mut parts: Vec<String> = Vec::new();
        let mut number = false;
        for c in s.chars() {
            let digit = c.is_ascii_digit() || matches!(c, '.' | '-' | 'e');
            if parts.is_empty() || digit != number {
                parts.push(String::new());
                number = digit;
            }
            parts.last_mut().unwrap().push(c);
        }
        parts
    };
    let (a, b) = (split(a), split(b));
    a.len() == b.len()
        && a.iter()
            .zip(&b)
            .all(|(a, b)| match (a.parse::<f64>(), b.parse::<f64>()) {
                (Ok(x), Ok(y)) => (x - y).abs() <= tolerance * x.abs().max(y.abs()).max(1.0),
                _ => a == b,
            })
}

#[test]
fn binary_dxf_is_read_and_written_back_byte_for_byte() {
    for (path, _) in pairs() {
        let src = std::fs::read(&path).unwrap();
        let (mut file, found) = read(&src);
        assert!(file.is_binary());
        assert_eq!(found, [], "{}", path.display());
        assert_eq!(write(&file), src, "{}", path.display());
        // すべてのレコードを、同じ値で書き換えたものとして符号化し直しても、元のバイトに戻る
        let edited: Vec<(usize, Vec<u8>)> = file
            .records()
            .iter()
            .enumerate()
            .filter_map(|(i, r)| Some((i, file.value(r.tags().get(1)?).to_vec())))
            .collect();
        let mut txn = file.transaction();
        for (record, value) in &edited {
            txn.set_value(*record, 1, value).unwrap();
        }
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        txn.commit(&mut Cx::new(&mut budget, &mut diag, &cancel))
            .unwrap();
        assert_eq!(write(&file), src, "{}", path.display());
    }
}

#[test]
fn binary_and_ascii_dxf_of_the_same_drawing_lift_to_the_same_drawing() {
    for (binary, ascii) in pairs() {
        let (ours, our_codes) = lifted(&binary);
        let (theirs, their_codes) = lifted(&ascii);
        // ASCII の DXF は実数を 16 桁ほどに丸めて書く。R12 の試料は、ポリラインの頂点を 6 桁に丸める
        let r12 = binary.to_string_lossy().contains("AC1009");
        let tolerance = if r12 { 1e-5 } else { 1e-9 };
        assert!(
            nearly(&ours, &theirs, tolerance),
            "{}\n{ours}\n{theirs}",
            binary.display()
        );
        assert_eq!(our_codes, their_codes, "{}", binary.display());
    }
}
