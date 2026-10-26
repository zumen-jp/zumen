//! `ZUMEN_SAMPLES` のフォルダーにある、同じ図面の DWG（R2000）と DXF を読み、表を比べる。
//! フォルダーを指定しないときは何も確かめない。図面は Git に入れない。
//!
//! `名前.dwg` と `名前_ascii.dxf` の組を探す。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use zumen_acad::{AcadDb, Data, Dwg, Dxf, Object};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_sdk::Codec;

/// 図形の形の欄。DWG の表が既定の値を省くので、DXF だけにあるときは既定の値かを確かめる
#[cfg(test)]
const SHAPE_CODES: [i16; 33] = [
    10, 20, 30, 11, 21, 31, 12, 22, 32, 13, 23, 33, 39, 40, 42, 48, 50, 51, 60, 62, 210, 220, 230,
    8, 2, 6, 43, 70, 90, 71, 72, 73, 74,
];

/// DXF が書き、DWG の表が省く既定の値か
#[cfg(test)]
fn is_default(code: i16, data: &Data) -> bool {
    matches!(
        (code, data),
        (62, Data::Int(256)) | (60 | 70, Data::Int(0)) | (48, Data::Real(1.0))
    ) || matches!((code, data), (39 | 210 | 220, Data::Real(r)) if *r == 0.0)
        || matches!((code, data), (230, Data::Real(r)) if *r == 1.0)
}

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

/// DXF は、名前のないブロック（`*` で始まる名前）に番号を付ける（DWG の `*Paper_Space` が
/// `*Paper_Space0`、`*D` が `*D3` など）
#[cfg(test)]
fn numbered(dwg: &Data, dxf: &Data) -> bool {
    match (dwg, dxf) {
        (Data::Text(a), Data::Text(b)) => {
            a.starts_with(b"*")
                && b.strip_prefix(a.as_slice())
                    .is_some_and(|n| !n.is_empty() && n.iter().all(u8::is_ascii_digit))
        }
        _ => false,
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
            // 所有者が DXF に書かれないとき（R2000 の DXF は、新しい版の表の中身を XRECORD の下に
            // 移す）は比べない
            if object
                .owner()
                .is_some_and(|owner| dxf_objects.contains_key(&owner))
            {
                assert_eq!(object.owner(), other.owner(), "{at}");
            }
            // DWG が置いた値は、DXF の同じグループ・コードの同じ順番の値と同じ
            let mut seen: BTreeMap<i16, usize> = BTreeMap::new();
            for value in &object.values {
                if matches!(value.code, 5 | 105 | 330) {
                    continue;
                }
                let nth = seen.entry(value.code).or_default();
                let theirs = other
                    .values
                    .iter()
                    .filter(|v| v.code == value.code)
                    .nth(*nth)
                    .map(|v| &v.data);
                *nth += 1;
                assert!(
                    theirs.is_some_and(
                        |theirs| same(&value.data, theirs) || numbered(&value.data, theirs)
                    ),
                    "{at} {}: {:?} {theirs:?}",
                    value.code,
                    value.data
                );
            }
            if [
                &b"LINE"[..],
                b"ARC",
                b"CIRCLE",
                b"POINT",
                b"TEXT",
                b"INSERT",
                b"MTEXT",
                b"SOLID",
                b"TRACE",
                b"3DFACE",
                b"ELLIPSE",
                b"RAY",
                b"XLINE",
                b"LWPOLYLINE",
                b"SPLINE",
                b"HATCH",
                b"POLYLINE",
                b"VERTEX",
                b"DIMENSION",
            ]
            .contains(&object.kind.as_slice())
            {
                for code in SHAPE_CODES {
                    // SPLINE の 70 は、DXF を書くときに平らかどうかから決まるので比べない
                    if object.kind == b"SPLINE" && code == 70 {
                        continue;
                    }
                    if let (None, Some(theirs)) = (object.first(code), other.first(code)) {
                        assert!(is_default(code, theirs), "{at} {code}: {theirs:?}");
                    }
                }
            }
        }
    }
}
