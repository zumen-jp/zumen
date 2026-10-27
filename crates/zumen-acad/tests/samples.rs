//! `ZUMEN_SAMPLES` のフォルダーにある、同じ図面の DWG（R2000・R2004・R2010 以降）と DXF を読み、表を比べる。
//! フォルダーを指定しないときは何も確かめない。図面は Git に入れない。
//!
//! `名前.dwg` と `名前_ascii.dxf` の組を探す。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use zumen_acad::{AcadDb, Data, Dwg, Dxf, Object, Origin, Section, Value};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, codes};
use zumen_sdk::{Codec, FileModel, Transact, Transaction};

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

/// フォルダーの中の、読める版の DWG と同じ名前の DXF の組
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
        .filter(|path| {
            let src = std::fs::read(path).unwrap();
            [b"AC1015", b"AC1018", b"AC1024", b"AC1027", b"AC1032"]
                .iter()
                .any(|version| src.starts_with(*version))
        })
        .filter_map(|dwg| {
            let stem = dwg.file_stem()?.to_str()?;
            let dxf = dwg.with_file_name(format!("{stem}_ascii.dxf"));
            dxf.exists().then_some((dwg, dxf))
        })
        .collect();
    found.sort();
    found
}

/// フォルダーの中の、版が `version` の DWG
#[cfg(test)]
#[allow(
    clippy::disallowed_methods,
    reason = "環境変数を読むのは試験だけで、核は読まない（AGENTS.md の図面と秘密）"
)]
fn dwgs(version: &[u8]) -> Vec<PathBuf> {
    let Some(dir) = std::env::var_os("ZUMEN_SAMPLES").filter(|d| !d.is_empty()) else {
        return Vec::new();
    };
    let mut found: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "dwg"))
        .filter(|path| std::fs::read(path).unwrap().starts_with(version))
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

/// 辞書の項目のうち、名前が `name` のものが指すハンドル
#[cfg(test)]
fn named(db: &AcadDb, name: &[u8]) -> Vec<u64> {
    db.objects()
        .iter()
        .filter(|object| object.kind == b"DICTIONARY")
        .flat_map(|object| object.values.windows(2))
        .filter_map(|pair| match (&pair[0].data, &pair[1].data) {
            (Data::Text(text), Data::Handle(h)) if pair[0].code == 3 && text == name => Some(*h),
            _ => None,
        })
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
        // 最後に保存したときの情報（GUID とファイルのパス）は、DWG と DXF で違うので比べない
        let saved_info = named(&dwg_db, b"ACAD_LAST_SAVED_VERSION_INFO");
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
            // 図形の所有者は、空間の図形（見出しの変数で決まる）も含めて、DXF と同じ
            if object.section == Section::Entities {
                assert_eq!(object.owner(), other.owner(), "{at}");
            }
            if object
                .owner()
                .is_some_and(|owner| dxf_objects.contains_key(&owner))
            {
                assert_eq!(object.owner(), other.owner(), "{at}");
            }
            // DWG が置いた値は、DXF の同じグループ・コードの同じ順番の値と同じ
            if saved_info.contains(&handle) {
                continue;
            }
            let mut seen: BTreeMap<i16, usize> = BTreeMap::new();
            // 辞書の項目（名前 3 と指すオブジェクト 350・360 の組）は、組で比べる。DXF にない
            // オブジェクトを指す項目は、DXF を書くときに落とされたか作り直されたものなので比べない
            // （R2000 の DXF は、新しい版の見出しの変数の辞書を別のハンドルで書き、版の往復のための
            // XRECORD を書かない）
            let mut values: Vec<&Value> = object.values.iter().collect();
            if matches!(
                object.kind.as_slice(),
                b"DICTIONARY" | b"ACDBDICTIONARYWDFLT"
            ) {
                let entries = |values: &[Value]| -> Vec<(Data, Data)> {
                    values
                        .windows(2)
                        .filter(|pair| pair[0].code == 3 && matches!(pair[1].code, 350 | 360))
                        .map(|pair| (pair[0].data.clone(), pair[1].data.clone()))
                        .collect()
                };
                let theirs = entries(&other.values);
                for entry in entries(&object.values) {
                    if matches!(entry.1, Data::Handle(h) if dxf_objects.contains_key(&h)) {
                        assert!(theirs.contains(&entry), "{at} {entry:?}");
                    }
                }
                values.retain(|v| !matches!(v.code, 3 | 350 | 360));
            }
            for value in values {
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
                b"SHAPE",
                b"ELLIPSE",
                b"RAY",
                b"XLINE",
                b"LWPOLYLINE",
                b"SPLINE",
                b"HATCH",
                b"POLYLINE",
                b"VERTEX",
                b"DIMENSION",
                b"ATTRIB",
                b"ATTDEF",
                b"TOLERANCE",
                b"MLINE",
                b"VIEWPORT",
            ]
            .contains(&object.kind.as_slice())
            {
                for code in SHAPE_CODES {
                    // SPLINE の 70 は、DXF を書くときに平らかどうかから決まるので比べない
                    if object.kind == b"SPLINE" && code == 70 {
                        continue;
                    }
                    // SHAPE の 2 は形の名前で、形のファイルの中にあるので、まだ置かない
                    if object.kind == b"SHAPE" && code == 2 {
                        continue;
                    }
                    // 真の色の 62 は、DXF が近い番号を置くが、近さの決まりが分からないので置かない
                    if code == 62 && object.first(420).is_some() {
                        continue;
                    }
                    // R2018 の DXF は、MTEXT の後に埋め込みのオブジェクト（101 から）を書く。その中の
                    // 値は、まだ読まないので比べない
                    let theirs = other
                        .values
                        .iter()
                        .take_while(|v| v.code != 101)
                        .find(|v| v.code == code)
                        .map(|v| &v.data);
                    if let (None, Some(theirs)) = (object.first(code), theirs) {
                        assert!(is_default(code, theirs), "{at} {code}: {theirs:?}");
                    }
                }
            }
        }
    }
}

/// 図面モデルの値を、実数の誤差を許して比べるための文字列。図形とビューの元のレコードは、
/// DWG（バイトの範囲）と DXF（レコードの番号）で違うので消す
#[cfg(test)]
#[cfg(feature = "drawing")]
fn lifted(path: &Path) -> (String, Vec<zumen_base::Code>) {
    use zumen_base::Ref;
    use zumen_sdk::Lift;
    let src = std::fs::read(path).unwrap();
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let mut drawing = if path.extension().is_some_and(|e| e == "dwg") {
        Dwg::read(&src, &mut cx).unwrap().lift(&mut cx).unwrap()
    } else {
        Dxf::read(&src, &mut cx).unwrap().lift(&mut cx).unwrap()
    };
    let nowhere = Ref::at_record(0);
    for space in &mut drawing.spaces {
        space
            .elements
            .iter_mut()
            .for_each(|e| e.source = nowhere.clone());
    }
    for sheet in &mut drawing.sheets {
        sheet
            .elements
            .iter_mut()
            .for_each(|e| e.source = nowhere.clone());
        sheet
            .views
            .iter_mut()
            .for_each(|v| v.source = nowhere.clone());
    }
    let codes = diag
        .iter()
        .map(|d| d.code())
        .filter(|c| c.name().starts_with("dxf."))
        .collect();
    (format!("{drawing:?}"), codes)
}

/// 数を除いた部分が同じで、数が相対の誤差の中で同じか
#[cfg(test)]
fn nearly(a: &str, b: &str) -> bool {
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
                (Ok(x), Ok(y)) => (x - y).abs() <= 1e-9 * x.abs().max(y.abs()).max(1.0),
                _ => a == b,
            })
}

#[cfg(feature = "drawing")]
#[test]
fn dwg_and_dxf_of_the_same_drawing_lift_to_the_same_drawing() {
    for (dwg, dxf) in pairs() {
        let (ours, our_codes) = lifted(&dwg);
        let (theirs, their_codes) = lifted(&dxf);
        assert!(
            nearly(&ours, &theirs),
            "{}\n{ours}\n{theirs}",
            dwg.display()
        );
        assert_eq!(our_codes, their_codes, "{}", dwg.display());
    }
}

#[test]
fn r2004_and_later_drawings_are_read_without_diagnostics() {
    let found = [b"AC1018", b"AC1024", b"AC1027", b"AC1032"];
    for path in found.iter().flat_map(|version| dwgs(*version)) {
        let src = std::fs::read(&path).unwrap();
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        assert!(Dwg::read(&src, &mut cx).is_ok(), "{}", path.display());
        let found: Vec<_> = diag.iter().map(|d| d.code()).collect();
        assert_eq!(found, [], "{}", path.display());
    }
}

#[test]
fn r2000_drawings_keep_their_table_when_every_object_is_rewritten() {
    for path in dwgs(b"AC1015") {
        let src = std::fs::read(&path).unwrap();
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut file = Dwg::read(&src, &mut cx).unwrap();
        let before = file.objects(&mut cx).unwrap();
        // すべてのオブジェクトを同じデータで置き換え、後ろに並べ直す
        let changes: Vec<_> = (0..file.record_count())
            .map(|i| {
                let span = file.record(i).unwrap().bytes.unwrap();
                let object = &src[span.start as usize..span.end as usize - 2];
                let mut at = 0;
                while object[at + 1] & 0x80 != 0 {
                    at += 2;
                }
                object[at + 2..].to_vec()
            })
            .collect();
        let mut txn = file.transaction();
        for (i, data) in changes.iter().enumerate() {
            txn.set_data(i, data).unwrap();
        }
        txn.commit(&mut cx).unwrap();
        let mut out = Vec::new();
        Dwg::write(&file, &mut out, &mut cx).unwrap();
        let reported = diag.len();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let after = Dwg::read(&out, &mut cx).unwrap().objects(&mut cx).unwrap();
        assert_eq!(after.objects().len(), before.objects().len());
        for (a, b) in before.objects().iter().zip(after.objects()) {
            assert_eq!(
                (&a.kind, &a.values),
                (&b.kind, &b.values),
                "{}",
                path.display()
            );
        }
        let found: Vec<_> = diag.iter().skip(reported).map(|d| d.code()).collect();
        let first: Vec<_> = diag.iter().take(reported).map(|d| d.code()).collect();
        assert_eq!(found, first, "{}", path.display());
    }
}

#[test]
fn r2004_drawings_keep_their_table_when_objects_are_rewritten_and_removed() {
    objects_are_rewritten_and_removed(b"AC1018");
}

#[test]
fn r2010_and_later_drawings_keep_their_table_when_objects_are_rewritten_and_removed() {
    for version in [b"AC1024", b"AC1027", b"AC1032"] {
        objects_are_rewritten_and_removed(version);
    }
}

/// 版 `version` の図面のすべてのオブジェクトを同じデータで置き換え、最後のオブジェクトを除いて書くと、読み直した
/// 表が、除いたものを除いて元と同じになる
#[cfg(test)]
fn objects_are_rewritten_and_removed(version: &[u8]) {
    for path in dwgs(version) {
        let src = std::fs::read(&path).unwrap();
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut file = Dwg::read(&src, &mut cx).unwrap();
        let before = file.objects(&mut cx).unwrap();
        let count = file.record_count();
        // すべてのオブジェクトを同じデータで置き換えて区切りの後ろに並べ直し、最後のオブジェクトを除く
        let changes: Vec<_> = (0..count)
            .map(|i| file.object_data(i).unwrap().to_vec())
            .collect();
        let mut txn = file.transaction();
        for (i, data) in changes.iter().enumerate() {
            txn.set_data(i, data).unwrap();
        }
        txn.remove(count - 1).unwrap();
        txn.commit(&mut cx).unwrap();
        let mut out = Vec::new();
        Dwg::write(&file, &mut out, &mut cx).unwrap();
        // 同じ種類の診断はまとまるので、読み直しの診断は別に集める
        let mut reread = Diagnostics::new();
        let mut cx = Cx::new(&mut budget, &mut reread, &cancel);
        let again = Dwg::read(&out, &mut cx).unwrap();
        assert_eq!(again.record_count(), count - 1, "{}", path.display());
        let after = again.objects(&mut cx).unwrap();
        let removed = before.objects()[count - 1].handle();
        let kept: Vec<_> = before
            .objects()
            .iter()
            .filter(|o| o.handle() != removed)
            .collect();
        assert_eq!(after.objects().len(), kept.len(), "{}", path.display());
        for (a, b) in kept.iter().zip(after.objects()) {
            assert_eq!(
                (&a.kind, &a.values),
                (&b.kind, &b.values),
                "{}",
                path.display()
            );
        }
        let found: Vec<_> = reread.iter().map(|d| d.code()).collect();
        let first: Vec<_> = diag.iter().map(|d| d.code()).collect();
        assert_eq!(found, first, "{}", path.display());
    }
}

#[test]
fn r2004_drawings_take_back_changed_entities() {
    const SIMPLE: [&[u8]; 25] = [
        b"LINE",
        b"CIRCLE",
        b"ARC",
        b"POINT",
        b"ELLIPSE",
        b"RAY",
        b"XLINE",
        b"SOLID",
        b"TRACE",
        b"3DFACE",
        b"TEXT",
        b"ATTRIB",
        b"ATTDEF",
        b"LWPOLYLINE",
        b"SPLINE",
        b"DIMENSION",
        b"TOLERANCE",
        b"LEADER",
        b"MLINE",
        b"SHAPE",
        b"VERTEX",
        b"INSERT",
        b"MTEXT",
        b"HATCH",
        b"VIEWPORT",
    ];
    let (mut changed, mut colored) = (0, 0);
    for path in dwgs(b"AC1018") {
        let src = std::fs::read(&path).unwrap();
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let file = Dwg::read(&src, &mut cx).unwrap();
        let base = file.objects(&mut cx).unwrap();
        // 型ごとの欄を R2000 と同じに符号化する種類の図形の、線種の尺度を変え、点を動かす
        let mut db = base.clone();
        for index in 0..db.objects().len() {
            let object = db.object_mut(index).unwrap();
            if !SIMPLE.contains(&object.kind.as_slice()) {
                continue;
            }
            // 多面の面の VERTEX（70 が 128）の点と、HATCH の初めの 10・20 は 0 と決まっているので動かさない
            let fixed = object.kind == b"VERTEX" && object.first(70) == Some(&Data::Int(128));
            let mut skip = if object.kind == b"HATCH" { 2 } else { 0 };
            for value in object.values.iter_mut() {
                match (value.code, &mut value.data) {
                    (48, Data::Real(v)) => *v *= 2.0,
                    (10 | 20 | 11 | 21, Data::Real(_)) if fixed => continue,
                    (10 | 20 | 11 | 21, Data::Real(_)) if skip > 0 => {
                        skip -= 1;
                        continue;
                    }
                    (10 | 20 | 11 | 21, Data::Real(v)) => *v += 1.5,
                    _ => continue,
                }
                changed += 1;
            }
        }
        let first: Vec<_> = diag.iter().map(|d| d.code()).collect();
        assert_eq!(
            taken_back(&file, &db),
            Ok(first.clone()),
            "{}",
            path.display()
        );
        // 色は 1 つずつ、図面ごとに 40 個まで変える。RGB の値か DBCOLOR を持つ色は、まだ書き戻さない
        let mut tried = 0;
        for index in 0..base.objects().len() {
            if tried == 40 {
                break;
            }
            let mut db = base.clone();
            let object = db.object_mut(index).unwrap();
            if !SIMPLE.contains(&object.kind.as_slice()) {
                continue;
            }
            let Some(value) = object.values.iter_mut().find(|v| v.code == 62) else {
                continue;
            };
            if let Data::Int(v) = &mut value.data {
                *v = *v % 255 + 1;
            }
            tried += 1;
            match taken_back(&file, &db) {
                Ok(found) => {
                    assert_eq!(found, first, "{}", path.display());
                    colored += 1;
                }
                Err(found) => {
                    assert_eq!(found, [codes::DWG_CHANGE_UNSUPPORTED], "{}", path.display())
                }
            }
        }
        // 型ごとの欄をまだ符号化しない種類の図形の変更は書き戻さない
        for kind in [&b"MULTILEADER"[..]] {
            let mut db = base.clone();
            let Some(index) = db.objects().iter().position(|o| o.kind == kind) else {
                continue;
            };
            db.object_mut(index)
                .unwrap()
                .values
                .push(zumen_acad::Value {
                    code: 48,
                    data: Data::Real(3.0),
                });
            let found = taken_back(&file, &db);
            assert_eq!(
                found,
                Err(vec![codes::DWG_CHANGE_UNSUPPORTED]),
                "{}",
                path.display()
            );
        }
    }
    if !dwgs(b"AC1018").is_empty() {
        assert!(changed > 0 && colored > 0);
    }
}

/// R2010 から後の図面の、図形の線種の尺度と点と色と文字列を変えて書き戻すと、読み直した表が変えた表と
/// 同じになる。図形でないオブジェクトの変更は、まだ書き戻さない（決定 0085〜0088）
#[test]
fn r2010_and_later_drawings_take_back_changed_entities() {
    const SIMPLE: [&[u8]; 10] = [
        b"LINE", b"CIRCLE", b"ARC", b"POINT", b"ELLIPSE", b"RAY", b"XLINE", b"SOLID", b"TRACE",
        b"3DFACE",
    ];
    // 文字列を持つ図形と、R2004 と同じに書き戻せる種類（決定 0086・0097）。R2018 の属性は除く
    let more = |version: &[u8; 6], kind: &[u8]| match kind {
        b"TEXT" | b"LWPOLYLINE" | b"INSERT" | b"MLINE" | b"MTEXT" | b"VERTEX" | b"HATCH"
        | b"DIMENSION" | b"TOLERANCE" | b"VIEWPORT" | b"SHAPE" | b"SPLINE" => true,
        b"ATTRIB" | b"ATTDEF" => version != b"AC1032",
        _ => false,
    };
    let mut texts = 0;
    let (mut changed, mut colored, mut found) = (0, 0, false);
    for version in [b"AC1024", b"AC1027", b"AC1032"] {
        for path in dwgs(version) {
            found = true;
            let src = std::fs::read(&path).unwrap();
            let mut diag = Diagnostics::new();
            let mut budget = Budget::default();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
            let file = Dwg::read(&src, &mut cx).unwrap();
            let base = file.objects(&mut cx).unwrap();
            let mut db = base.clone();
            for index in 0..db.objects().len() {
                let object = db.object_mut(index).unwrap();
                let kind = object.kind.clone();
                // MINSERT（列の数の 70 を持つ INSERT）と、回転（50）を持つ INSERT は除く。回転は度から直すので、
                // 元のビットと同じに符号化できるとは限らない
                // 多面の面の VERTEX（70 が 128）の点は 0 と決まっているので動かさない。R2013 からの、当てはめる
                // 点から制御点を求めた SPLINE（節点の許容差が 1e-9）は書き戻さない（決定 0097）
                let skipped = kind == b"INSERT"
                    && object.values.iter().any(|v| [70, 50].contains(&v.code))
                    || kind == b"VERTEX" && object.first(70) == Some(&Data::Int(128))
                    || kind == b"SPLINE" && object.first(42) == Some(&Data::Real(1e-9));
                if !SIMPLE.contains(&kind.as_slice()) && !more(version, &kind) || skipped {
                    continue;
                }
                // MTEXT の文字列は 250 バイトずつに分けるので、長い文字列は、足すと分け方が変わる。変えない
                let split = object.values.iter().any(|v| match (v.code, &v.data) {
                    (3, _) => true,
                    (1, Data::Text(text)) => text.len() > 200,
                    _ => false,
                });
                // HATCH の初めの 10・20 は 0 と決まっているので動かさない
                let mut fixed = if kind == b"HATCH" { 2 } else { 0 };
                for value in object.values.iter_mut() {
                    match (value.code, &mut value.data) {
                        // 2 倍で既定の 1 になる尺度は、読み直すと値に置かれないので変えない
                        (48, Data::Real(v)) if *v != 0.5 => *v *= 2.0,
                        (10 | 20, Data::Real(_)) if fixed > 0 => {
                            fixed -= 1;
                            continue;
                        }
                        (10 | 20, Data::Real(v)) => *v += 1.5,
                        (11 | 21, Data::Real(v)) if SIMPLE.contains(&kind.as_slice()) => *v += 1.5,
                        (1, Data::Text(text)) if kind != b"MLINE" && !split => {
                            text.extend_from_slice("（改）".as_bytes());
                            texts += 1;
                        }
                        _ => continue,
                    }
                    changed += 1;
                }
            }
            let first: Vec<_> = diag.iter().map(|d| d.code()).collect();
            assert_eq!(
                taken_back(&file, &db),
                Ok(first.clone()),
                "{}",
                path.display()
            );
            // 色は 1 つずつ、図面ごとに 10 個まで変える
            let mut tried = 0;
            for index in 0..base.objects().len() {
                if tried == 10 {
                    break;
                }
                let mut db = base.clone();
                let object = db.object_mut(index).unwrap();
                if !SIMPLE.contains(&object.kind.as_slice()) {
                    continue;
                }
                let Some(value) = object.values.iter_mut().find(|v| v.code == 62) else {
                    continue;
                };
                if let Data::Int(v) = &mut value.data {
                    *v = *v % 255 + 1;
                }
                tried += 1;
                match taken_back(&file, &db) {
                    Ok(found) => {
                        assert_eq!(found, first, "{}", path.display());
                        colored += 1;
                    }
                    Err(found) => {
                        assert_eq!(found, [codes::DWG_CHANGE_UNSUPPORTED], "{}", path.display())
                    }
                }
            }
            // 図形でないオブジェクトの、型ごとの欄でない値の変更は書き戻さない
            for kind in [&b"DICTIONARY"[..]] {
                let mut db = base.clone();
                let Some(index) = db.objects().iter().position(|o| o.kind == kind) else {
                    continue;
                };
                db.object_mut(index)
                    .unwrap()
                    .values
                    .push(zumen_acad::Value {
                        code: 48,
                        data: Data::Real(3.0),
                    });
                assert_eq!(
                    taken_back(&file, &db),
                    Err(vec![codes::DWG_CHANGE_UNSUPPORTED]),
                    "{}",
                    path.display()
                );
            }
        }
    }
    if found {
        assert!(changed > 0 && colored > 0 && texts > 0);
    }
}

#[test]
fn r2004_drawings_take_back_changed_table_records() {
    table_records_are_taken_back(b"AC1018");
}

/// R2010 から後の図面の表のレコードの欄と文字列を変えて書き戻すと、読み直した表が変えた表と同じになる
/// （決定 0089）
#[test]
fn r2010_and_later_drawings_take_back_changed_table_records() {
    for version in [b"AC1024", b"AC1027", b"AC1032"] {
        table_records_are_taken_back(version);
    }
}

#[cfg(test)]
fn table_records_are_taken_back(version: &[u8; 6]) {
    let r2010 = version != b"AC1018";
    let mut changed = 0;
    for path in dwgs(version) {
        let src = std::fs::read(&path).unwrap();
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let file = Dwg::read(&src, &mut cx).unwrap();
        let mut db = file.objects(&mut cx).unwrap();
        // R2000 の試験と同じ欄と、寸法の色（176〜178）を変える。画層と寸法の色は CMC の形で書き直す
        for index in 0..db.objects().len() {
            let object = db.object_mut(index).unwrap();
            if object.section != Section::Tables {
                continue;
            }
            for value in object.values.iter_mut() {
                match (object.kind.as_slice(), value.code, &mut value.data) {
                    (b"LAYER", 62, Data::Int(v)) => *v = v.signum() * (v.abs() % 255 + 1),
                    (b"STYLE", 41, Data::Real(v)) => *v *= 2.0,
                    (b"LTYPE", 49, Data::Real(v)) => *v *= 2.0,
                    (b"VIEW" | b"VPORT", 40, Data::Real(v)) => *v *= 2.0,
                    (b"UCS", 10, Data::Real(v)) => *v += 1.5,
                    (b"DIMSTYLE", 40, Data::Real(v)) => *v *= 2.0,
                    (b"DIMSTYLE", 176..=178, Data::Int(v)) => *v = *v % 255 + 1,
                    // R2010 からは、文字列の流れの文字列も変える
                    (b"STYLE" | b"LTYPE", 3, Data::Text(text)) if r2010 => {
                        text.extend("あ".as_bytes())
                    }
                    _ => continue,
                }
                changed += 1;
            }
        }
        let first: Vec<_> = diag.iter().map(|d| d.code()).collect();
        assert_eq!(taken_back(&file, &db), Ok(first), "{}", path.display());
    }
    if !dwgs(version).is_empty() {
        assert!(changed > 0);
    }
}

#[test]
fn r2004_drawings_take_back_changed_objects() {
    objects_are_taken_back(b"AC1018");
}

/// R2010 から後の図面の、図形でないオブジェクトの欄と文字列を変えて書き戻すと、読み直した表が変えた表と
/// 同じになる（決定 0090）
#[test]
fn r2010_and_later_drawings_take_back_changed_objects() {
    for version in [b"AC1024", b"AC1027", b"AC1032"] {
        objects_are_taken_back(version);
    }
}

#[cfg(test)]
fn objects_are_taken_back(version: &[u8; 6]) {
    let mut changed = 0;
    for path in dwgs(version) {
        let src = std::fs::read(&path).unwrap();
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let file = Dwg::read(&src, &mut cx).unwrap();
        let mut db = file.objects(&mut cx).unwrap();
        // R2000 の試験と同じ欄と、多重線の体裁の色を変える。色は CMC の形で書き直す
        for index in 0..db.objects().len() {
            let object = db.object_mut(index).unwrap();
            for value in object.values.iter_mut() {
                match (object.kind.as_slice(), value.code, &mut value.data) {
                    (b"GROUP", 300, Data::Text(text)) => text.extend(b"0"),
                    (b"DICTIONARYVAR", 1, Data::Text(text)) => text.extend(b"0"),
                    (b"DICTIONARY" | b"ACDBDICTIONARYWDFLT", 281, Data::Int(v)) => *v = *v % 5 + 1,
                    // R2018 からの MLINESTYLE は、まだ書き戻さない
                    (b"MLINESTYLE", 49, Data::Real(v)) if version != b"AC1032" => *v *= 2.0,
                    (b"MLINESTYLE", 62, Data::Int(v)) if version != b"AC1032" => *v = *v % 255 + 1,
                    (b"LAYOUT", 146, Data::Real(v)) => *v += 1.0,
                    (b"LAYOUT", 78, Data::Int(v)) => *v += 100,
                    (b"XRECORD", _, Data::Real(v)) => *v += 1.0,
                    (b"XRECORD", _, Data::Text(text)) => text.extend(b"0"),
                    _ => continue,
                }
                changed += 1;
            }
        }
        let first: Vec<_> = diag.iter().map(|d| d.code()).collect();
        assert_eq!(taken_back(&file, &db), Ok(first), "{}", path.display());
    }
    if !dwgs(version).is_empty() {
        assert!(changed > 0);
    }
}

/// `file` の写しに表 `db` を書き戻して書き、読み直した表が `db` と同じことを確かめ、読み直しの診断を返す。
/// 書き戻せなければ、その診断
#[cfg(test)]
fn taken_back(
    file: &zumen_acad::DwgFile,
    db: &AcadDb,
) -> Result<Vec<zumen_base::Code>, Vec<zumen_base::Code>> {
    let mut file = file.clone();
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    if file.write_back(db, &mut cx).is_err() {
        return Err(diag.iter().map(|d| d.code()).collect());
    }
    let mut out = Vec::new();
    Dwg::write(&file, &mut out, &mut cx).unwrap();
    assert!(diag.is_empty());
    let mut diag = Diagnostics::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let after = Dwg::read(&out, &mut cx).unwrap().objects(&mut cx).unwrap();
    assert_eq!(after.objects().len(), db.objects().len());
    for (a, b) in db.objects().iter().zip(after.objects()) {
        assert_eq!(a.kind, b.kind);
        assert_eq!(a.values.len(), b.values.len());
        for (x, y) in a.values.iter().zip(&b.values) {
            assert!(x.code == y.code && same(&x.data, &y.data));
        }
    }
    Ok(diag.iter().map(|d| d.code()).collect())
}

#[test]
fn r2000_drawings_take_back_changed_entities() {
    let mut changed = 0;
    for path in dwgs(b"AC1015") {
        let src = std::fs::read(&path).unwrap();
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut file = Dwg::read(&src, &mut cx).unwrap();
        let mut db = file.objects(&mut cx).unwrap();
        let reported = diag.len();
        // 図形の色と線種の尺度を変え、書き戻せる種類の図形の点を動かす。画層と文字スタイルの欄も変える
        for index in 0..db.objects().len() {
            let object = db.object_mut(index).unwrap();
            if object.section == Section::Entities {
                for value in object.values.iter_mut() {
                    match (value.code, &mut value.data) {
                        (62, Data::Int(v)) => *v = *v % 255 + 1,
                        (48, Data::Real(v)) => *v *= 2.0,
                        _ => continue,
                    }
                    changed += 1;
                }
            }
            // 画層の色（表示しない印の負の符号は保つ）と、文字スタイルの幅の係数と、線種の要素の長さと、ビューとビューポートの高さと、UCS の原点と、寸法の尺度を変える
            if object.section == Section::Tables {
                for value in object.values.iter_mut() {
                    match (object.kind.as_slice(), value.code, &mut value.data) {
                        (b"LAYER", 62, Data::Int(v)) => *v = v.signum() * (v.abs() % 255 + 1),
                        (b"STYLE", 41, Data::Real(v)) => *v *= 2.0,
                        (b"LTYPE", 49, Data::Real(v)) => *v *= 2.0,
                        (b"VIEW" | b"VPORT", 40, Data::Real(v)) => *v *= 2.0,
                        (b"UCS", 10, Data::Real(v)) => *v += 1.5,
                        (b"DIMSTYLE", 40, Data::Real(v)) => *v *= 2.0,
                        _ => continue,
                    }
                    changed += 1;
                }
            }
            // 図面の変数の値の後ろに文字を足し、辞書の複製の扱いと、多重線の体裁の線の位置と、レイアウトの高さと、XRECORD の実数と文字列を変える
            for value in object.values.iter_mut() {
                match (object.kind.as_slice(), value.code, &mut value.data) {
                    (b"DICTIONARYVAR", 1, Data::Text(text)) => text.extend(b"0"),
                    (b"DICTIONARY" | b"ACDBDICTIONARYWDFLT", 281, Data::Int(v)) => *v = *v % 5 + 1,
                    (b"MLINESTYLE", 49, Data::Real(v)) => *v *= 2.0,
                    (b"LAYOUT", 146, Data::Real(v)) => *v += 1.0,
                    (b"XRECORD", _, Data::Real(v)) => *v += 1.0,
                    (b"XRECORD", _, Data::Text(text)) => text.extend(b"0"),
                    _ => continue,
                }
                changed += 1;
            }
            if !matches!(
                object.kind.as_slice(),
                b"LINE"
                    | b"CIRCLE"
                    | b"ARC"
                    | b"POINT"
                    | b"ELLIPSE"
                    | b"RAY"
                    | b"XLINE"
                    | b"SOLID"
                    | b"TRACE"
                    | b"3DFACE"
                    | b"TEXT"
                    | b"INSERT"
                    | b"ATTRIB"
                    | b"ATTDEF"
                    | b"MTEXT"
                    | b"LWPOLYLINE"
                    | b"SPLINE"
                    | b"HATCH"
                    | b"VERTEX"
                    | b"DIMENSION"
                    | b"TOLERANCE"
                    | b"MLINE"
                    | b"VIEWPORT"
                    | b"SHAPE"
            ) {
                continue;
            }
            // 多面の面の VERTEX（70 が 128）の点は 0 と決まっているので動かさない
            if object.kind == b"VERTEX" && object.first(70) == Some(&Data::Int(128)) {
                continue;
            }
            // HATCH の初めの 10・20 は 0 と決まっているので動かさない
            let skip = if object.kind == b"HATCH" { 2 } else { 0 };
            let points = object
                .values
                .iter_mut()
                .filter(|value| matches!(value.code, 10 | 20 | 11 | 21));
            for value in points.skip(skip) {
                if let Data::Real(v) = &mut value.data {
                    *v += 1.5;
                    changed += 1;
                }
            }
        }
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        file.write_back(&db, &mut cx).unwrap();
        let mut out = Vec::new();
        Dwg::write(&file, &mut out, &mut cx).unwrap();
        let after = Dwg::read(&out, &mut cx).unwrap().objects(&mut cx).unwrap();
        assert_eq!(after.objects().len(), db.objects().len());
        for (a, b) in db.objects().iter().zip(after.objects()) {
            assert_eq!(a.kind, b.kind, "{}", path.display());
            assert_eq!(a.values.len(), b.values.len(), "{}", path.display());
            for (x, y) in a.values.iter().zip(&b.values) {
                assert!(
                    x.code == y.code && same(&x.data, &y.data),
                    "{}",
                    path.display()
                );
            }
        }
        // 書き戻しは診断を足さず、読み直しは同じ診断を出す
        let found: Vec<_> = diag.iter().skip(reported).map(|d| d.code()).collect();
        let first: Vec<_> = diag.iter().take(reported).map(|d| d.code()).collect();
        assert_eq!(found, first, "{}", path.display());
    }
    if !dwgs(b"AC1015").is_empty() {
        assert!(changed > 0);
    }
}

#[test]
fn r2000_drawings_take_back_removed_entities() {
    let removed = removed_entities_are_taken_back(b"AC1015");
    if !dwgs(b"AC1015").is_empty() {
        assert!(removed > 0);
    }
}

#[test]
fn r2004_drawings_take_back_removed_entities() {
    let removed = removed_entities_are_taken_back(b"AC1018");
    if !dwgs(b"AC1018").is_empty() {
        assert!(removed > 0);
    }
}

/// R2010 から後の図面から図形を除いて書き戻すと、読み直した表が除いた表と同じになる（決定 0091）
#[test]
fn r2010_and_later_drawings_take_back_removed_entities() {
    for version in [b"AC1024", b"AC1027", b"AC1032"] {
        let removed = removed_entities_are_taken_back(version);
        if !dwgs(version).is_empty() {
            assert!(removed > 0);
        }
    }
}

/// 版 `version` の試料の図面から図形を除いて書き戻し、読み直した表が除いた表と同じことを確かめ、除いた
/// 図形の数を返す
#[cfg(test)]
fn removed_entities_are_taken_back(version: &[u8]) -> usize {
    let mut removed = 0;
    for path in dwgs(version) {
        let src = std::fs::read(&path).unwrap();
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let base = Dwg::read(&src, &mut cx).unwrap().objects(&mut cx).unwrap();
        let without = |gone: &[u64]| {
            let mut db = base.clone();
            while let Some(index) = db
                .objects()
                .iter()
                .position(|o| o.handle().is_some_and(|h| gone.contains(&h)))
            {
                db.remove(index);
            }
            db
        };
        let written = |db: &AcadDb| {
            let mut diag = Diagnostics::new();
            let mut budget = Budget::default();
            let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
            let mut file = Dwg::read(&src, &mut cx).unwrap();
            file.write_back(db, &mut cx).ok()?;
            let mut out = Vec::new();
            Dwg::write(&file, &mut out, &mut cx).unwrap();
            Some(Dwg::read(&out, &mut cx).unwrap().objects(&mut cx).unwrap())
        };
        // 図形を持たない LINE・CIRCLE・ARC・TEXT・INSERT を 2 つおきに選び、1 つずつ足して除く。反応子か
        // 拡張辞書を持つものは除けないので、足さない
        let owners: Vec<u64> = base.objects().iter().filter_map(Object::owner).collect();
        let candidates: Vec<u64> = base
            .objects()
            .iter()
            .filter(|o| {
                matches!(o.section, Section::Entities | Section::Blocks)
                    && matches!(
                        o.kind.as_slice(),
                        b"LINE" | b"CIRCLE" | b"ARC" | b"TEXT" | b"INSERT"
                    )
            })
            .filter_map(Object::handle)
            .filter(|h| !owners.contains(h))
            .step_by(3)
            .take(8)
            .collect();
        let mut gone = Vec::new();
        for handle in candidates {
            gone.push(handle);
            if written(&without(&gone)).is_none() {
                gone.pop();
            }
        }
        // 除いた図形の前後の図形も、また読める
        let db = without(&gone);
        let after = written(&db).unwrap();
        assert_eq!(
            after.objects().len(),
            db.objects().len(),
            "{}",
            path.display()
        );
        for (a, b) in db.objects().iter().zip(after.objects()) {
            assert_eq!(a.kind, b.kind, "{}", path.display());
            assert_eq!(a.values, b.values, "{}", path.display());
        }
        removed += gone.len();
    }
    removed
}

#[test]
fn r2000_drawings_take_back_added_entities() {
    added_entities_are_taken_back(b"AC1015", true);
}

#[test]
fn r2004_drawings_take_back_added_entities() {
    added_entities_are_taken_back(b"AC1018", false);
}

/// R2010 から後の図面に LINE・CIRCLE・TEXT・LWPOLYLINE・INSERT を足して書き戻すと、読み直したものが足したものと
/// 合う（決定 0092）
#[test]
fn r2010_and_later_drawings_take_back_added_entities() {
    for version in [b"AC1024", b"AC1027", b"AC1032"] {
        added_entities_are_taken_back(version, false);
    }
}

/// 版 `version` の試料の図面のモデル空間に図形を足して書き戻し、読み直した表が足した表と同じことを確かめる。
/// `minsert` なら、属性を持つ MINSERT も足す
#[cfg(test)]
fn added_entities_are_taken_back(version: &[u8], minsert: bool) {
    for path in dwgs(version) {
        let src = std::fs::read(&path).unwrap();
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut file = Dwg::read(&src, &mut cx).unwrap();
        let mut db = file.objects(&mut cx).unwrap();
        let seed = file.handle_seed().unwrap();
        let model = db
            .objects()
            .iter()
            .find(|o| {
                o.kind == b"BLOCK_RECORD"
                    && o.values
                        .iter()
                        .any(|v| v.code == 2 && v.data == Data::Text(b"*Model_Space".to_vec()))
            })
            .and_then(Object::handle)
            .unwrap();
        let real = |code, value| Value {
            code,
            data: Data::Real(value),
        };
        let text = |code, value: &[u8]| Value {
            code,
            data: Data::Text(value.to_vec()),
        };
        let int = |code, value| Value {
            code,
            data: Data::Int(value),
        };
        // モデル空間に、画層 0 の LINE と CIRCLE と、文字の体裁が Standard の TEXT と MTEXT と、LWPOLYLINE と、重みのない・重みのある・当てはめる点の SPLINE を足す
        let mut added: Vec<(&[u8], Vec<Value>)> = vec![
            (
                b"LINE",
                vec![
                    real(10, 1.0),
                    real(20, 2.0),
                    real(30, 0.0),
                    real(11, 3.0),
                    real(21, 4.0),
                    real(31, 0.0),
                ],
            ),
            (
                b"CIRCLE",
                vec![real(10, 5.0), real(20, 6.0), real(30, 0.0), real(40, 7.0)],
            ),
            (
                b"TEXT",
                vec![
                    real(10, 1.0),
                    real(20, 2.0),
                    real(30, 0.0),
                    real(40, 2.5),
                    text(1, b"zumen"),
                ],
            ),
            (
                b"MTEXT",
                vec![
                    real(10, 1.0),
                    real(20, 2.0),
                    real(30, 0.0),
                    real(41, 10.0),
                    real(40, 2.5),
                    int(71, 1),
                    int(72, 1),
                    text(1, b"zumen"),
                    int(73, 1),
                    real(44, 1.0),
                ],
            ),
            (
                b"LWPOLYLINE",
                vec![
                    int(90, 2),
                    int(70, 1),
                    real(43, 0.0),
                    real(10, 0.0),
                    real(20, 0.0),
                    real(42, 1.0),
                    real(10, 3.0),
                    real(20, 4.0),
                ],
            ),
            (
                b"SPLINE",
                [
                    vec![
                        int(71, 3),
                        int(72, 8),
                        int(73, 4),
                        int(74, 0),
                        real(42, 1e-10),
                        real(43, 1e-10),
                    ],
                    [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]
                        .map(|k| real(40, k))
                        .to_vec(),
                    [[0.0, 0.0], [1.0, 2.0], [3.0, 2.0], [4.0, 0.0]]
                        .iter()
                        .flat_map(|[x, y]| [real(10, *x), real(20, *y), real(30, 0.0)])
                        .collect(),
                ]
                .concat(),
            ),
            (
                b"SPLINE",
                [
                    vec![
                        int(71, 3),
                        int(72, 8),
                        int(73, 4),
                        int(74, 0),
                        real(42, 1e-10),
                        real(43, 1e-10),
                    ],
                    [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]
                        .map(|k| real(40, k))
                        .to_vec(),
                    [[0.0, 0.0], [1.0, 2.0], [3.0, 2.0], [4.0, 0.0]]
                        .iter()
                        .flat_map(|[x, y]| {
                            [real(10, *x), real(20, *y), real(30, 0.0), real(41, 0.5)]
                        })
                        .collect(),
                ]
                .concat(),
            ),
            (
                b"SPLINE",
                vec![
                    int(71, 3),
                    int(72, 0),
                    int(73, 0),
                    int(74, 2),
                    real(44, 1e-10),
                    real(11, 0.0),
                    real(21, 0.0),
                    real(31, 0.0),
                    real(11, 1.0),
                    real(21, 1.0),
                    real(31, 0.0),
                ],
            ),
        ];
        // 名前のあるブロックがあれば、その INSERT も足す
        let block = db
            .objects()
            .iter()
            .filter(|o| o.kind == b"BLOCK_RECORD")
            .find_map(|o| {
                o.values.iter().find_map(|v| match &v.data {
                    Data::Text(name) if v.code == 2 && !name.starts_with(b"*") => {
                        Some(name.clone())
                    }
                    _ => None,
                })
            });
        if let Some(name) = &block {
            added.push((
                b"INSERT",
                vec![real(10, 8.0), real(20, 9.0), real(30, 0.0), text(2, name)],
            ));
            // 属性を 2 つ持つ INSERT も足す。ATTRIB と SEQEND は、すぐ前の INSERT が持つ
            added.push((
                b"INSERT",
                vec![
                    real(10, 8.0),
                    real(20, 9.0),
                    real(30, 0.0),
                    int(66, 1),
                    text(2, name),
                ],
            ));
            for value in [b"1", b"2"] {
                added.push((
                    b"ATTRIB",
                    vec![
                        real(10, 8.0),
                        real(20, 9.0),
                        real(30, 0.0),
                        real(40, 2.5),
                        text(1, value),
                        text(2, b"TAG"),
                        int(70, 0),
                    ],
                ));
            }
            added.push((b"SEQEND", Vec::new()));
        }
        if let Some(name) = block.filter(|_| minsert) {
            // 列と行の数と間隔を持ち、属性を 1 つ持つ MINSERT も足す
            added.push((
                b"INSERT",
                vec![
                    real(10, 8.0),
                    real(20, 9.0),
                    real(30, 0.0),
                    int(66, 1),
                    int(70, 2),
                    int(71, 3),
                    real(44, 5.0),
                    real(45, 6.0),
                    text(2, &name),
                ],
            ));
            added.push((
                b"ATTRIB",
                vec![
                    real(10, 8.0),
                    real(20, 9.0),
                    real(30, 0.0),
                    real(40, 2.5),
                    text(1, b"3"),
                    text(2, b"TAG"),
                    int(70, 0),
                ],
            ));
            added.push((b"SEQEND", Vec::new()));
        }
        // R2010 から後は、R2018 からの MTEXT と属性と、R2013 からの当てはめる点の SPLINE をまだ足せない
        // （決定 0092・0093・0097）。
        // MTEXT は枠の高さ（46）を、ATTRIB は属性の版と位置を固定するか（280）を持つ
        if !matches!(version, b"AC1015" | b"AC1018") {
            let r2018 = version == b"AC1032";
            added.retain(|(kind, own)| match *kind {
                b"MTEXT" | b"ATTRIB" | b"SEQEND" => !r2018,
                b"INSERT" => !r2018 || own.iter().all(|value| value.code != 66),
                b"SPLINE" => version == b"AC1024" || own.iter().all(|value| value.code != 11),
                _ => true,
            });
            for (kind, own) in &mut added {
                if *kind == b"MTEXT" {
                    let at = own.iter().position(|value| value.code == 41).unwrap();
                    own.insert(at + 1, real(46, 0.0));
                } else if *kind == b"ATTRIB" {
                    let at = own.iter().position(|value| value.code == 2).unwrap();
                    own.insert(at, int(280, 0));
                    own.push(int(280, 0));
                }
            }
        }
        let count = added.len() as u64;
        let mut insert = model;
        for (handle, (kind, own)) in (seed..).zip(added) {
            let owner = match kind {
                b"ATTRIB" | b"SEQEND" => insert,
                _ => model,
            };
            if kind == b"INSERT" {
                insert = handle;
            }
            let mut values = vec![
                Value {
                    code: 5,
                    data: Data::Handle(handle),
                },
                Value {
                    code: 330,
                    data: Data::Handle(owner),
                },
                text(8, b"0"),
            ];
            values.extend(own);
            db.push(Object {
                kind: kind.to_vec(),
                section: Section::Entities,
                values,
                origin: Origin::New,
            });
        }
        file.write_back(&db, &mut cx).unwrap();
        let mut out = Vec::new();
        Dwg::write(&file, &mut out, &mut cx).unwrap();
        // 書き換えた見出しの変数と、足したオブジェクトの CRC は合う
        let mut fresh = Diagnostics::new();
        let mut cx = Cx::new(&mut budget, &mut fresh, &cancel);
        let again = Dwg::read(&out, &mut cx).unwrap();
        assert_eq!(
            again.handle_seed(),
            Some(seed + count),
            "{}",
            path.display()
        );
        let after = again.objects(&mut cx).unwrap();
        assert!(
            fresh.iter().all(|d| d.code() != codes::DWG_CRC_INVALID),
            "{}",
            path.display()
        );
        assert_eq!(
            after.objects().len(),
            db.objects().len(),
            "{}",
            path.display()
        );
        for (a, b) in db.objects().iter().zip(after.objects()) {
            assert_eq!(a.kind, b.kind, "{}", path.display());
            assert_eq!(a.values, b.values, "{}", path.display());
        }
    }
}

/// R2000 の図面に画層と文字の体裁と線種と、それらを使う LINE と TEXT と、アプリケーションの名前と、足した
/// 文字の体裁を使う寸法の体裁と、UCS を伴うビューと、ビューポートの設定を足して書き戻すと、読み直したものが足したものと合う。書き戻した図面に
/// もう一度足しても合う（決定 0061〜0067）
#[test]
fn r2000_drawings_take_back_added_table_records() {
    added_table_records_are_taken_back(b"AC1015");
}

#[test]
fn r2004_drawings_take_back_added_table_records() {
    added_table_records_are_taken_back(b"AC1018");
}

/// R2010 以降の図面に、表のレコードと、それを使う図形とブロックを足して書き戻すと、読み直したものが足したものと
/// 合う（決定 0094〜0096）
#[test]
fn r2010_and_later_drawings_take_back_added_table_records() {
    for version in [b"AC1024", b"AC1027", b"AC1032"] {
        added_table_records_are_taken_back(version);
    }
}

/// 版 `version` の図面に表のレコードと、それを使う図形とブロックを足して書き戻すと、読み直したものが足したものと合う
#[cfg(test)]
fn added_table_records_are_taken_back(version: &[u8]) {
    for path in dwgs(version) {
        let mut src = std::fs::read(&path).unwrap();
        for name in [b"zumen-1".as_slice(), b"zumen-2"] {
            let mut diag = Diagnostics::new();
            let mut budget = Budget::default();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
            let mut file = Dwg::read(&src, &mut cx).unwrap();
            let mut db = file.objects(&mut cx).unwrap();
            let seed = file.handle_seed().unwrap();
            let named = |kind: &[u8], name: &[u8]| {
                db.objects()
                    .iter()
                    .find(|o| {
                        o.kind == kind
                            && o.values
                                .iter()
                                .any(|v| matches!(&v.data, Data::Text(t) if v.code == 2 && t.eq_ignore_ascii_case(name)))
                    })
                    .cloned()
                    .unwrap()
            };
            let model = named(b"BLOCK_RECORD", b"*Model_Space").handle().unwrap();
            let blocks = named(b"BLOCK_RECORD", b"*Model_Space").owner().unwrap();
            let mut layer = named(b"LAYER", b"0");
            layer.origin = Origin::New;
            for value in &mut layer.values {
                match value.code {
                    5 => value.data = Data::Handle(seed),
                    2 | 6 => value.data = Data::Text(name.to_vec()),
                    // 真の色は足せないので、番号の色にする
                    420 => {
                        *value = Value {
                            code: 62,
                            data: Data::Int(7),
                        }
                    }
                    _ => {}
                }
            }
            let mut style = named(b"STYLE", b"Standard");
            style.origin = Origin::New;
            for value in &mut style.values {
                match value.code {
                    5 => value.data = Data::Handle(seed + 1),
                    2 => value.data = Data::Text(name.to_vec()),
                    _ => {}
                }
            }
            // 長さ 0.5 の線と 0.25 の空きの、形も文字もない要素の線種
            let ltypes = named(b"LTYPE", b"Continuous").owner().unwrap();
            let mut ltype = vec![
                (5, Data::Handle(seed + 2)),
                (330, Data::Handle(ltypes)),
                (2, Data::Text(name.to_vec())),
                (70, Data::Int(0)),
                (3, Data::Text(b"__ __".to_vec())),
                (72, Data::Int(65)),
                (73, Data::Int(2)),
                (40, Data::Real(0.75)),
            ];
            for dash in [0.5, -0.25] {
                ltype.extend([(49, Data::Real(dash)), (74, Data::Int(0))]);
            }
            // 今のビューポートの設定を写し、名前を変えたもの
            let mut vport_record = db
                .objects()
                .iter()
                .find(|o| o.kind == b"VPORT")
                .unwrap()
                .clone();
            vport_record.origin = Origin::New;
            for value in &mut vport_record.values {
                match value.code {
                    5 => value.data = Data::Handle(seed + 8),
                    2 => value.data = Data::Text(name.to_vec()),
                    _ => {}
                }
            }
            let mut appid = named(b"APPID", b"ACAD");
            appid.origin = Origin::New;
            for value in &mut appid.values {
                match value.code {
                    5 => value.data = Data::Handle(seed + 5),
                    2 => value.data = Data::Text(name.to_vec()),
                    _ => {}
                }
            }
            // 接尾辞・矢印の大きさ・文字の高さを持ち、足した文字の体裁を使う寸法の体裁
            let dimstyles = db
                .objects()
                .iter()
                .find(|o| o.kind == b"DIMSTYLE")
                .and_then(Object::owner)
                .unwrap();
            let dimstyle = [
                (105, Data::Handle(seed + 6)),
                (330, Data::Handle(dimstyles)),
                (2, Data::Text(name.to_vec())),
                (70, Data::Int(0)),
                (3, Data::Text(b"<>mm".to_vec())),
                (41, Data::Real(2.5)),
                (140, Data::Real(3.0)),
                (340, Data::Handle(seed + 1)),
            ];
            db.push(layer);
            db.push(style);
            db.push(Object {
                kind: b"LTYPE".to_vec(),
                section: Section::Tables,
                values: ltype
                    .into_iter()
                    .map(|(code, data)| Value { code, data })
                    .collect(),
                origin: Origin::New,
            });
            let mut values = vec![
                Value {
                    code: 5,
                    data: Data::Handle(seed + 3),
                },
                Value {
                    code: 330,
                    data: Data::Handle(model),
                },
                Value {
                    code: 8,
                    data: Data::Text(name.to_vec()),
                },
            ];
            for (code, value) in [
                (10, 1.0),
                (20, 2.0),
                (30, 0.0),
                (11, 3.0),
                (21, 4.0),
                (31, 0.0),
            ] {
                values.push(Value {
                    code,
                    data: Data::Real(value),
                });
            }
            let mut line = values.clone();
            line.push(Value {
                code: 6,
                data: Data::Text(name.to_vec()),
            });
            db.push(Object {
                kind: b"LINE".to_vec(),
                section: Section::Entities,
                values: line,
                origin: Origin::New,
            });
            let mut text = values;
            text[0].data = Data::Handle(seed + 4);
            text.truncate(6);
            text.extend([
                Value {
                    code: 40,
                    data: Data::Real(2.5),
                },
                Value {
                    code: 1,
                    data: Data::Text(b"zumen".to_vec()),
                },
                Value {
                    code: 7,
                    data: Data::Text(name.to_vec()),
                },
            ]);
            db.push(Object {
                kind: b"TEXT".to_vec(),
                section: Section::Entities,
                values: text,
                origin: Origin::New,
            });
            db.push(appid);
            db.push(Object {
                kind: b"DIMSTYLE".to_vec(),
                section: Section::Tables,
                values: dimstyle
                    .into_iter()
                    .map(|(code, data)| Value { code, data })
                    .collect(),
                origin: Origin::New,
            });
            // UCS を伴うビュー。ビューの表は、見出しの並びと同じく線種の表の次にある
            let mut tables: Vec<u64> = db
                .objects()
                .iter()
                .filter(|o| o.kind == b"TABLE")
                .filter_map(Object::handle)
                .collect();
            tables.sort_unstable();
            // ビューがあれば、その表を使う。なければ、レコードを持たない表のうち、線種の表の次のもの
            let owners: Vec<u64> = db
                .objects()
                .iter()
                .filter(|o| o.section == Section::Tables && o.kind != b"TABLE")
                .filter_map(Object::owner)
                .collect();
            let views = db
                .objects()
                .iter()
                .find(|o| o.kind == b"VIEW")
                .and_then(Object::owner)
                .unwrap_or_else(|| {
                    *tables
                        .iter()
                        .find(|&&h| h > ltypes && !owners.contains(&h))
                        .unwrap()
                });
            let mut view = vec![
                (5, Data::Handle(seed + 7)),
                (330, Data::Handle(views)),
                (2, Data::Text(name.to_vec())),
                (70, Data::Int(0)),
            ];
            for (code, v) in [
                (40, 100.0),
                (10, 50.0),
                (20, 40.0),
                (41, 150.0),
                (11, 0.0),
                (21, 0.0),
                (31, 1.0),
                (12, 0.0),
                (22, 0.0),
                (32, 0.0),
                (42, 50.0),
                (43, 0.0),
                (44, 0.0),
                (50, 0.0),
            ] {
                view.push((code, Data::Real(v)));
            }
            view.extend([(71, Data::Int(0)), (281, Data::Int(0)), (72, Data::Int(1))]);
            for (code, v) in [
                (110, 0.0),
                (120, 0.0),
                (130, 0.0),
                (111, 1.0),
                (121, 0.0),
                (131, 0.0),
                (112, 0.0),
                (122, 1.0),
                (132, 0.0),
            ] {
                view.push((code, Data::Real(v)));
            }
            view.extend([(79, Data::Int(0)), (146, Data::Real(0.0))]);
            db.push(Object {
                kind: b"VIEW".to_vec(),
                section: Section::Tables,
                values: view
                    .into_iter()
                    .map(|(code, data)| Value { code, data })
                    .collect(),
                origin: Origin::New,
            });
            db.push(vport_record);
            // 線を 1 本持つブロックと、それをモデル空間に挿す INSERT
            let record = seed + 9;
            let block = |kind: &[u8], handle: u64, values: Vec<(i16, Data)>| {
                let owner = if kind == b"INSERT" { model } else { record };
                let mut all = vec![
                    (5, Data::Handle(handle)),
                    (330, Data::Handle(owner)),
                    (8, Data::Text(name.to_vec())),
                ];
                all.extend(values);
                Object {
                    kind: kind.to_vec(),
                    section: if kind == b"INSERT" {
                        Section::Entities
                    } else {
                        Section::Blocks
                    },
                    values: all
                        .into_iter()
                        .map(|(code, data)| Value { code, data })
                        .collect(),
                    origin: Origin::New,
                }
            };
            db.push(Object {
                kind: b"BLOCK_RECORD".to_vec(),
                section: Section::Tables,
                values: [
                    (5, Data::Handle(record)),
                    (330, Data::Handle(blocks)),
                    (2, Data::Text(name.to_vec())),
                ]
                .into_iter()
                .map(|(code, data)| Value { code, data })
                .collect(),
                origin: Origin::New,
            });
            let point = [
                (10, Data::Real(1.0)),
                (20, Data::Real(2.0)),
                (30, Data::Real(0.0)),
            ];
            let mut begin = vec![(2, Data::Text(name.to_vec()))];
            begin.extend(point.clone());
            db.push(block(b"BLOCK", seed + 10, begin));
            let mut line = point.to_vec();
            line.extend([
                (11, Data::Real(3.0)),
                (21, Data::Real(4.0)),
                (31, Data::Real(0.0)),
            ]);
            db.push(block(b"LINE", seed + 11, line));
            db.push(block(b"ENDBLK", seed + 12, Vec::new()));
            let mut insert = point.to_vec();
            insert.push((2, Data::Text(name.to_vec())));
            db.push(block(b"INSERT", seed + 13, insert));
            file.write_back(&db, &mut cx).unwrap();
            // DWG の BLOCK は、名前と基点を値に持たない
            let at = db.objects().len() - 4;
            db.object_mut(at)
                .unwrap()
                .values
                .retain(|v| !matches!(v.code, 2 | 10 | 20 | 30));
            let mut out = Vec::new();
            Dwg::write(&file, &mut out, &mut cx).unwrap();
            let mut fresh = Diagnostics::new();
            let mut cx = Cx::new(&mut budget, &mut fresh, &cancel);
            let again = Dwg::read(&out, &mut cx).unwrap();
            assert_eq!(again.handle_seed(), Some(seed + 14), "{}", path.display());
            let after = again.objects(&mut cx).unwrap();
            assert!(
                fresh.iter().all(|d| d.code() != codes::DWG_CRC_INVALID),
                "{}",
                path.display()
            );
            assert_eq!(
                after.objects().len(),
                db.objects().len(),
                "{}",
                path.display()
            );
            for (a, b) in db.objects().iter().zip(after.objects()) {
                assert_eq!(a.kind, b.kind, "{}", path.display());
                assert_eq!(a.values, b.values, "{}", path.display());
            }
            src = out;
        }
    }
}

/// R2000 の図面の INSUNITS と、用紙空間の PLIMMIN・PLIMMAX を変えて書き戻すと、読み直した変数が変えたものと合い、
/// ほかは変わらない（決定 0069）
#[test]
fn r2000_drawings_take_back_changed_header_variables() {
    header_variables_are_taken_back(b"AC1015");
}

#[test]
fn r2004_drawings_take_back_changed_header_variables() {
    header_variables_are_taken_back(b"AC1018");
}

#[test]
fn r2010_and_later_drawings_take_back_changed_header_variables() {
    for version in [b"AC1024", b"AC1027", b"AC1032"] {
        header_variables_are_taken_back(version);
    }
}

/// 版 `version` の試料の図面の見出しの変数を変えて書き戻し、読み直した表が変えた表と同じことと、
/// HANDSEED を書き換えられることを確かめる
#[cfg(test)]
fn header_variables_are_taken_back(version: &[u8]) {
    for path in dwgs(version) {
        let src = std::fs::read(&path).unwrap();
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut file = Dwg::read(&src, &mut cx).unwrap();
        let mut db = file.objects(&mut cx).unwrap();
        let value = |code, data| Value { code, data };
        // BS の幅を変えない値にする
        if let Some(
            [
                Value {
                    data: Data::Int(units @ 1..=255),
                    ..
                },
            ],
        ) = db.variable(b"$INSUNITS")
        {
            let units = if *units == 4 { 6 } else { 4 };
            db.set_variable(b"$INSUNITS", vec![value(70, Data::Int(units))]);
        }
        let point = |x, y| vec![value(10, Data::Real(x)), value(20, Data::Real(y))];
        db.set_variable(b"$PLIMMIN", point(-1.0, -2.0));
        db.set_variable(b"$PLIMMAX", point(841.0, 594.0));
        if matches!(version, b"AC1015" | b"AC1018") {
            file.write_back(&db, &mut cx).unwrap();
        } else {
            // R2010 から後は共有の表から書き戻せないので、変更で同じ値にする
            let mut txn = file.transaction();
            if let Some(
                [
                    Value {
                        data: Data::Int(units),
                        ..
                    },
                ],
            ) = db.variable(b"$INSUNITS")
            {
                txn.set_units(i16::try_from(*units).unwrap()).unwrap();
            }
            txn.set_paper_limits([-1.0, -2.0, 841.0, 594.0]).unwrap();
            txn.commit(&mut cx).unwrap();
        }
        let mut out = Vec::new();
        Dwg::write(&file, &mut out, &mut cx).unwrap();
        let mut fresh = Diagnostics::new();
        let mut cx = Cx::new(&mut budget, &mut fresh, &cancel);
        let after = Dwg::read(&out, &mut cx).unwrap().objects(&mut cx).unwrap();
        assert!(
            fresh.iter().all(|d| d.code() != codes::DWG_CRC_INVALID),
            "{}",
            path.display()
        );
        assert!(after == db, "{}", path.display());
        // HANDSEED は、大きさ（バイト）を変えない値に進める
        let mut diag = Diagnostics::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut file = Dwg::read(&out, &mut cx).unwrap();
        let seed = file.handle_seed().unwrap();
        let mut txn = file.transaction();
        txn.set_handle_seed(seed + 1).unwrap();
        txn.commit(&mut cx).unwrap();
        let mut out = Vec::new();
        Dwg::write(&file, &mut out, &mut cx).unwrap();
        let again = Dwg::read(&out, &mut cx).unwrap();
        assert_eq!(again.handle_seed(), Some(seed + 1), "{}", path.display());
    }
}
