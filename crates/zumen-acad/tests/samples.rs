//! `ZUMEN_SAMPLES` のフォルダーにある、同じ図面の DWG（R2000）と DXF を読み、表を比べる。
//! フォルダーを指定しないときは何も確かめない。図面は Git に入れない。
//!
//! `名前.dwg` と `名前_ascii.dxf` の組を探す。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use zumen_acad::{AcadDb, Data, Dwg, Dxf, Object};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_sdk::Codec;

/// 比べる LINE の欄
#[cfg(test)]
const LINE_CODES: [i16; 13] = [10, 20, 30, 11, 21, 31, 39, 48, 60, 62, 210, 220, 230];

/// フォルダーの中の、R2000 の DWG と同じ名前の DXF の組
#[cfg(test)]
#[allow(
    clippy::disallowed_methods,
    reason = "環境変数を読むのは試験だけで、核は読まない（AGENTS.md の図面と秘密）"
)]
fn pairs() -> Vec<(PathBuf, PathBuf)> {
    let Some(dir) = std::env::var_os("ZUMEN_SAMPLES").filter(|d| !d.is_empty()) else {
        return Vec::new();
    };
    let mut found: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "dwg"))
        .filter(|path| std::fs::read(path).unwrap().starts_with(b"AC1015"))
        .filter_map(|dwg| {
            let stem = dwg.file_stem()?.to_str()?;
            let dxf = dwg.with_file_name(format!("{stem}_ascii.dxf"));
            dxf.exists().then_some((dwg, dxf))
        })
        .collect();
    found.sort();
    found
}

/// DWG か DXF を読み、表を作る
#[cfg(test)]
fn table(path: &Path) -> AcadDb {
    let src = std::fs::read(path).unwrap();
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    if path.extension().is_some_and(|e| e == "dwg") {
        Dwg::read(&src, &mut cx).unwrap().objects(&mut cx).unwrap()
    } else {
        Dxf::read(&src, &mut cx).unwrap().objects(&mut cx).unwrap()
    }
}

#[cfg(test)]
fn by_handle(db: &AcadDb) -> BTreeMap<u64, &Object> {
    db.objects()
        .iter()
        .filter_map(|object| Some((object.handle()?, object)))
        .collect()
}

/// 同じ値か。DXF は実数を 16 桁で書くので、実数は相対の誤差を許す
#[cfg(test)]
fn same(a: &Data, b: &Data) -> bool {
    match (a, b) {
        (Data::Real(a), Data::Real(b)) => (a - b).abs() <= 1e-14 * a.abs().max(b.abs()).max(1.0),
        _ => a == b,
    }
}

#[test]
fn dwg_and_dxf_of_the_same_drawing_give_the_same_table() {
    for (dwg, dxf) in pairs() {
        let (dwg_db, dxf_db) = (table(&dwg), table(&dxf));
        let dxf_objects = by_handle(&dxf_db);
        for (handle, object) in by_handle(&dwg_db) {
            // 型を知らないオブジェクトと、DXF に書かれないオブジェクトは比べない
            let Some(other) = dxf_objects.get(&handle) else {
                continue;
            };
            if object.kind.is_empty() {
                continue;
            }
            let at = format!("{} {handle:#X}", dwg.display());
            assert_eq!(object.kind, other.kind, "{at}");
            if object.owner().is_some() {
                assert_eq!(object.owner(), other.owner(), "{at}");
            }
            if object.kind == b"LINE" {
                for code in LINE_CODES {
                    let (a, b) = (object.first(code), other.first(code));
                    match (a, b) {
                        (Some(a), Some(b)) => assert!(same(a, b), "{at} {code}: {a:?} {b:?}"),
                        // DXF だけにある既定の値（色 256 など）は、DWG の表に置かない
                        (None, Some(Data::Int(256))) if code == 62 => {}
                        (None, None) => {}
                        _ => panic!("{at} {code}: {a:?} {b:?}"),
                    }
                }
            }
        }
    }
}
