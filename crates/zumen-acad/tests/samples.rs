//! `ZUMEN_SAMPLES` のフォルダーにある、同じ図面の DWG（R2000・R2004・R2010 以降）と DXF を読み、表を比べる。
//! フォルダーを指定しないときは何も確かめない。図面は Git に入れない。
//!
//! `名前.dwg` と `名前_ascii.dxf` の組を探す。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use zumen_acad::{AcadDb, Data, Dwg, Dxf, Object, Section, Value};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
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
            // 図面の変数の値の後ろに文字を足し、辞書の複製の扱いと、多重線の体裁の線の位置と、レイアウトの高さを変える
            for value in object.values.iter_mut() {
                match (object.kind.as_slice(), value.code, &mut value.data) {
                    (b"DICTIONARYVAR", 1, Data::Text(text)) => text.extend(b"0"),
                    (b"DICTIONARY" | b"ACDBDICTIONARYWDFLT", 281, Data::Int(v)) => *v = *v % 5 + 1,
                    (b"MLINESTYLE", 49, Data::Real(v)) => *v *= 2.0,
                    (b"LAYOUT", 146, Data::Real(v)) => *v += 1.0,
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
