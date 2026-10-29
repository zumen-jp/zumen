//! `ZUMEN_SAMPLES` のフォルダーにある、同じ図面の DWG（R2000・R2004・R2010 以降）と DXF を読み、表を比べる。
//! フォルダーを指定しないときは何も確かめない。図面は Git に入れない。
//!
//! `名前.dwg` と `名前_ascii.dxf` の組を探す。

#[cfg(feature = "drawing")]
mod by_source;
#[cfg(feature = "drawing")]
mod support;

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
    reason = "環境変数を読むのは試験だけで、コアは読まない（AGENTS.md の図面と秘密）"
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

/// 数を許容差で比べ、ほかの文字は同じであること。用紙の mm を図面の単位に戻すか、DWG のラジアンを度に戻すと、
/// 最後の桁が違うことがある
#[cfg(test)]
fn close(found: &str, expected: &str) -> bool {
    fn tokens(text: &str) -> Vec<Result<f64, String>> {
        let mut out = Vec::new();
        let mut rest = text;
        while let Some(first) = rest.chars().next() {
            let number = first.is_ascii_digit()
                || (first == '-' && rest.chars().nth(1).is_some_and(|c| c.is_ascii_digit()));
            let end = if number {
                rest.char_indices()
                    .skip(1)
                    .find(|(_, c)| !(c.is_ascii_digit() || matches!(c, '.' | 'e' | '-')))
                    .map_or(rest.len(), |(at, _)| at)
            } else {
                first.len_utf8()
            };
            let (token, tail) = rest.split_at(end);
            out.push(match token.parse::<f64>() {
                Ok(value) if number => Ok(value),
                _ => Err(token.to_owned()),
            });
            rest = tail;
        }
        out
    }
    let (found, expected) = (tokens(found), tokens(expected));
    found.len() == expected.len()
        && found.iter().zip(&expected).all(|pair| match pair {
            (Ok(a), Ok(b)) => (a - b).abs() <= 1e-9 * a.abs().max(1.0),
            (a, b) => a == b,
        })
}

/// フォルダーの中の、版が `version` の DWG
#[cfg(test)]
#[allow(
    clippy::disallowed_methods,
    reason = "環境変数を読むのは試験だけで、コアは読まない（AGENTS.md の図面と秘密）"
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

#[test]
fn dwg_and_dxf_of_the_same_drawing_give_the_same_table() {
    for (dwg, dxf) in pairs() {
        let (dwg_db, dxf_db) = (table(&dwg), table(&dxf));
        let dxf_objects = by_handle(&dxf_db);
        let dwg_objects = by_handle(&dwg_db);
        // 最後に保存したときの情報（GUID とファイルのパス）は、DWG と DXF で違うので比べない
        let saved_info = named(&dwg_db, b"ACAD_LAST_SAVED_VERSION_INFO");
        for (&handle, object) in &dwg_objects {
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
            let hookless;
            let other = if object.kind == b"LEADER" {
                hookless = without_hook(other);
                &hookless
            } else {
                other
            };
            // R2000 の DXF は、代わりの図形のバイト数を 92 に置き、続けて図形（310）を置く。DWG は置かない
            let proxy = other
                .values
                .windows(2)
                .position(|pair| pair[0].code == 92 && pair[1].code == 310);
            let mut copy = other.clone();
            if let Some(at) = proxy {
                copy.values.remove(at);
            }
            // 拡張辞書（`{ACAD_XDICTIONARY` の組）は、DXF を書くときに作り直されて別のハンドルになることが
            // あるので比べない
            copy.values = without_xdictionary(&copy.values);
            let other = &copy;
            let ours = without_xdictionary(&object.values);
            let mut seen: BTreeMap<i16, usize> = BTreeMap::new();
            // 辞書の項目（名前 3 と指すオブジェクト 350・360 の組）は、組で比べる。DXF にない
            // オブジェクトを指す項目は、DXF を書くときに落とされたか作り直されたものなので比べない
            // （R2000 の DXF は、新しい版の見出しの変数の辞書を別のハンドルで書き、版の往復のための
            // XRECORD を書かない）
            let mut values: Vec<&Value> = ours.iter().collect();
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
            // 拡張データ（1001 から）は、登録アプリケーションごとの組で比べる。AutoCAD の往復のための拡張データ
            // （ACAD の RTTcAl などと ACAD_SPLINE_DATA）は、DXF に書かれない
            let theirs = xdata_groups(&other.values);
            for group in xdata_groups(&object.values) {
                let found = theirs.iter().any(|their| {
                    their.len() == group.len()
                        && their
                            .iter()
                            .zip(group)
                            .all(|(a, b)| a.code == b.code && same(&a.data, &b.data))
                });
                let round_trip = matches!(
                    group.first().map(|v| &v.data),
                    Some(Data::Text(app)) if app == b"ACAD" || app == b"ACAD_SPLINE_DATA"
                );
                assert!(found || round_trip, "{at} {group:?}");
            }
            if let Some(start) = values.iter().position(|v| v.code == 1001) {
                values.truncate(start);
            }
            let mut embedded = false;
            let mut cell_value = false;
            for value in values {
                embedded |= value.code == 101;
                if matches!(value.code, 5 | 105 | 330) {
                    continue;
                }
                // 表のセルの値（301〜304）の型のある値は、表の上書きと同じコードを使うので、並びの何番目かで比べられない。
                // 表示する文字（302）だけを比べ、値は図面モデルの値として比べる（tests/cell_value_samples.rs）
                if object.kind == b"ACAD_TABLE" {
                    match value.code {
                        301 => cell_value = true,
                        304 => cell_value = false,
                        302 => {}
                        _ if cell_value => continue,
                        _ => {}
                    }
                }
                // SPLINE の 70 は、DXF を書くときに平らかどうかから決まるので比べない
                if object.kind == b"SPLINE" && value.code == 70 {
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
                // R2000 の MULTILEADER の色は番号だけを持つ（0xC3）。DXF が RGB の色（0xC2）を持つときは比べない
                let nearest = |data: &Data, theirs: &Data| {
                    matches!((data, theirs), (Data::Int(ours), Data::Int(theirs))
                        if ours >> 24 & 0xFF == 0xC3 && theirs >> 24 & 0xFF == 0xC2)
                };
                // DXF は、表のセルが指す名前のないブロック（`*U`）を、別のハンドルで書き直すことがある
                let anonymous = |data: &Data, theirs: &Data| {
                    let name = |db: &BTreeMap<u64, &Object>, data: &Data| match data {
                        Data::Handle(h) => db
                            .get(h)
                            .filter(|o| o.kind == b"BLOCK_RECORD")
                            .and_then(|o| {
                                o.values
                                    .iter()
                                    .find(|v| v.code == 2)
                                    .map(|v| v.data.clone())
                            }),
                        _ => None,
                    };
                    matches!((name(&dwg_objects, data), name(&dxf_objects, theirs)),
                        (Some(Data::Text(a)), Some(Data::Text(b))) if a.starts_with(b"*U") && b.starts_with(b"*U"))
                };
                // 書き直したブロックは、名前の番号も変わる。表の ACAD_TABLE と、表のブロックの中の INSERT が指す
                let rewritten = |data: &Data, theirs: &Data| {
                    matches!((data, theirs), (Data::Text(a), Data::Text(b))
                        if a.starts_with(b"*U") && b.starts_with(b"*U"))
                };
                // DXF の TABLESTYLE は文字スタイル（7）を名前で書き、DWG はハンドルで持つ
                let style_name = |data: &Data, theirs: &Data| match data {
                    Data::Handle(h) => dwg_objects
                        .get(h)
                        .filter(|o| o.kind == b"STYLE")
                        .and_then(|o| o.values.iter().find(|v| v.code == 2))
                        .is_some_and(|v| &v.data == theirs),
                    _ => false,
                };
                assert!(
                    theirs.is_some_and(|theirs| same(&value.data, theirs)
                        || (object.kind == b"TABLESTYLE"
                            && value.code == 7
                            && style_name(&value.data, theirs))
                        || (object.kind == b"ACAD_TABLE"
                            && value.code == 340
                            && anonymous(&value.data, theirs))
                        || (value.code == 2 && rewritten(&value.data, theirs))
                        // DXF は段組みの MTEXT の段ごとの高さを 0.0 と書く
                        || (object.kind == b"MTEXT" && value.code == 46 && embedded)
                        || (object.kind == b"MULTILEADER" && nearest(&value.data, theirs))),
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

/// 図面モデルと、変換で返した DXF の診断のコード。図形とビューの元のレコードは、DWG（バイトの範囲）と
/// DXF（レコードの番号）で違うので消す。拡張辞書の XRECORD の中のハンドルも消す
#[cfg(test)]
#[cfg(feature = "drawing")]
fn lifted(path: &Path) -> (zumen_drawing::Drawing2D, Vec<zumen_base::Code>) {
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
    support::normalize_extensions(&mut drawing);
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
    // R2010 からの DWG の読み手は、TABLESTYLE の欄とセルのスタイルをまだ読まない（決定 0151）。
    // 書式の一致は table_format_samples.rs が確かめる。記号・用紙と、拡張に控えた値も同じ条件で比べる。
    for space in drawing
        .spaces
        .iter_mut()
        .chain(drawing.symbols.iter_mut().map(|symbol| &mut symbol.space))
    {
        space.elements.iter_mut().for_each(without_table_formats);
    }
    for sheet in &mut drawing.sheets {
        sheet.elements.iter_mut().for_each(without_table_formats);
    }
    for extension in &mut drawing.extensions {
        match &mut extension.basis {
            zumen_drawing::Basis::Model(element) => without_table_formats(element),
            zumen_drawing::Basis::Paper(element) => without_table_formats(element),
            _ => {}
        }
    }
    let codes = diag
        .iter()
        .map(|d| d.code())
        .filter(|c| c.name().starts_with("dxf."))
        .collect();
    (drawing, codes)
}

/// DWG でまだ読まない表とセルの書式だけを除く。値・内容・回転などは残す。
#[cfg(feature = "drawing")]
fn without_table_formats<P>(element: &mut zumen_drawing::Element<P>) {
    if let zumen_drawing::Shape::Table(table) = &mut element.shape {
        table.formats = None;
        for cell in &mut table.cells {
            cell.format = zumen_drawing::CellFormat::default();
        }
    }
}

/// 図面モデルの、DWG と DXF で比べられる文字列。元の場所（`Ref { .. }`）と、名前のないブロック `*U` の番号と、
/// 真の色の近い色の番号を除く。元の場所は、
/// DWG（バイトの範囲）と DXF（レコードの番号）で違う。図形の元のほか、ハッチングの輪の元・引出線の注釈・
/// 寸法の体裁なども元の場所で指す
#[cfg(test)]
#[cfg(feature = "drawing")]
fn comparable(text: &str) -> String {
    const ACI: &str = "code: Some(ColorCode { system: Aci, value: ";
    let digit = |c: char| c.is_ascii_digit();
    let text = support::without_source_refs(text);
    // DXF は、表のセルが指す名前のないブロック（`*U`）を別のハンドルで書き直し、名前の番号も変わる
    let text = rewrite(&text, "name: \"*U", |rest| {
        ("name: \"*U".into(), rest.trim_start_matches(digit))
    });
    // DWG は真の色の RGB だけを持ち、DXF の近い色の番号を持たない（docs/formats/dwg.md）
    rewrite(&text, ACI, |rest| {
        match rest
            .trim_start_matches(digit)
            .strip_prefix(" }), rgb: Some(")
        {
            Some(after) => ("code: None, rgb: Some(".into(), after),
            None => (ACI.into(), rest),
        }
    })
}

/// `text` の `marker` ごとに、その前を写し、後ろを `f` で置き換えるものと続きに分ける
#[cfg(test)]
#[cfg(feature = "drawing")]
fn rewrite<'a>(
    text: &'a str,
    marker: &str,
    mut f: impl FnMut(&'a str) -> (String, &'a str),
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some((before, after)) = rest.split_once(marker) {
        out.push_str(before);
        let (with, next) = f(after);
        out.push_str(&with);
        rest = next;
    }
    out.push_str(rest);
    out
}

/// DXF は、鉤の線を持つ LEADER の終わりの前に鉤の線の点を足して書くが、DWG はこの点を持たない
/// （docs/formats/dwg.md）。DWG から作った引出線より頂点が 1 つ多い、DXF から作った引出線のその点を除く
#[cfg(test)]
#[cfg(feature = "drawing")]
fn without_hooks<P>(ours: &[zumen_drawing::Element<P>], theirs: &mut [zumen_drawing::Element<P>]) {
    use zumen_drawing::Shape;
    for (ours, theirs) in ours.iter().zip(theirs) {
        if let (Shape::Leader(ours), Shape::Leader(theirs)) = (&ours.shape, &mut theirs.shape)
            && theirs.path.len() == ours.path.len() + 1
            && ours.path.len() >= 2
        {
            theirs.path.remove(ours.path.len() - 1);
        }
    }
}

/// 拡張辞書の組（`102 {ACAD_XDICTIONARY`・360・`102 }`）を除いた値
fn without_xdictionary(values: &[Value]) -> Vec<Value> {
    let mut out = Vec::with_capacity(values.len());
    let mut inside = false;
    for value in values {
        match (value.code, &value.data) {
            (102, Data::Text(text)) if text == b"{ACAD_XDICTIONARY" => inside = true,
            (102, Data::Text(text)) if inside && text.starts_with(b"}") => inside = false,
            _ if inside => {}
            _ => out.push(value.clone()),
        }
    }
    out
}

/// 拡張データの、登録アプリケーションの名前（1001）ごとの組
#[cfg(test)]
fn xdata_groups(values: &[Value]) -> Vec<&[Value]> {
    let start = values
        .iter()
        .position(|v| v.code == 1001)
        .unwrap_or(values.len());
    values[start..]
        .chunk_by(|_, next| next.code != 1001)
        .collect()
}

/// DXF の LEADER から、鉤の線を持つ（75 が 1）ときに足される、終わりの前の頂点を除く。DWG は
/// この点を持たない
#[cfg(test)]
fn without_hook(leader: &Object) -> Object {
    let mut leader = leader.clone();
    if leader.first(75) != Some(&Data::Int(1)) {
        return leader;
    }
    let count = leader.values.iter().filter(|v| v.code == 10).count();
    let mut nth = 0;
    leader.values.retain(|value| {
        if !matches!(value.code, 10 | 20 | 30) {
            return true;
        }
        if value.code == 10 {
            nth += 1;
        }
        nth + 1 != count
    });
    for value in &mut leader.values {
        if let (76, Data::Int(n)) = (value.code, &mut value.data) {
            *n -= 1;
        }
    }
    leader
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

/// R2010 から後の DXF の表のセルの値（301 `CELL_VALUE` から 304 `ACVALUE_END` まで）は、印（93）・
/// 型（90）・型ごとの値・単位（94）・書式（300）・表示する文字（302）の順に並ぶ。文字の型（4）の値は
/// 1 に元の文字を持つ。印の 1 のビットが立てば、型ごとの値はない。DWG の値（ODS 5.4.1 の 20.4.99）と
/// 同じ並び
#[test]
fn r2010_and_later_table_cell_values_end_with_the_displayed_text() {
    let mut checked = 0;
    for (dwg, dxf) in pairs() {
        let version = std::fs::read(&dwg).unwrap();
        if version.starts_with(b"AC1015") || version.starts_with(b"AC1018") {
            continue;
        }
        let db = table(&dxf);
        for object in db.objects().iter().filter(|o| o.kind == b"ACAD_TABLE") {
            let values = &object.values;
            let starts = values
                .iter()
                .enumerate()
                .filter(|(_, v)| v.code == 301 && v.data == Data::Text(b"CELL_VALUE".to_vec()))
                .map(|(i, _)| i);
            for start in starts {
                let end = start + values[start..].iter().position(|v| v.code == 304).unwrap();
                let codes: Vec<i16> = values[start + 1..end].iter().map(|v| v.code).collect();
                let at = format!("{} {:?}", dxf.display(), object.handle());
                assert_eq!(codes[..2], [93, 90], "{at}");
                assert_eq!(codes[codes.len() - 3..], [94, 300, 302], "{at}");
                let data = &codes[2..codes.len() - 3];
                match (&values[start + 1].data, &values[start + 2].data) {
                    (Data::Int(flags), _) if flags & 1 != 0 => assert!(data.is_empty(), "{at}"),
                    (_, Data::Int(4)) => assert_eq!(data, [1], "{at}"),
                    _ => {}
                }
                checked += 1;
            }
        }
    }
    assert!(pairs().is_empty() || checked > 0);
}

/// 試料の線を ID と読んだときのハッシュで指して動かし、図形を 1 つ足すと、apply が返す ID とハッシュは、
/// 書き戻したファイルを読み直したときと同じになる。文書のハッシュは、読んだバイト列のハッシュで通る
/// （決定 0139・0156）。DWG と DXF の組と、バイナリの DXF で確かめる
#[cfg(feature = "drawing")]
#[test]
#[allow(
    clippy::disallowed_methods,
    reason = "環境変数を読むのは試験だけで、コアは読まない（AGENTS.md の図面と秘密）"
)]
fn drawings_changed_by_id_return_the_ids_and_hashes_of_reading_again() {
    use std::collections::BTreeMap;
    use zumen_drawing::{Applied, Changes, Drawing2D, Edit, Hash, Placed, Seen, Shape, id};
    use zumen_sdk::{Apply, Codec, Lift, testkit};
    fn seen<F: Lift<Drawing2D>>(
        file: &F,
    ) -> Vec<(Seen, zumen_drawing::Element<zumen_drawing::ModelPoint>)> {
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let drawing = file
            .lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
            .unwrap();
        let space = drawing.spaces.into_iter().next().unwrap();
        let place = id::space(space.id);
        let mut namer = id::Namer::new(&place);
        let ids: Vec<_> = space
            .elements
            .iter()
            .enumerate()
            .map(|(index, element)| Seen {
                id: namer.next(index, element),
                hash: Hash::element(element),
            })
            .collect();
        ids.into_iter().zip(space.elements).collect()
    }
    fn check<C: Codec>(path: &Path) -> bool
    where
        C::File: Lift<Drawing2D> + Apply<Changes, Output = Vec<Applied>>,
    {
        let src = std::fs::read(path).unwrap();
        let mut file = testkit::read::<C>(&src).0.unwrap();
        let before = seen(&file);
        let Some((target, line)) = before
            .iter()
            .find(|(seen, element)| {
                matches!(element.shape, Shape::Line { .. })
                    && seen.id.starts_with("space/0/h")
                    && seen.id.matches('/').count() == 2
            })
            .cloned()
        else {
            return false;
        };
        let mut moved = line.clone();
        if let Shape::Line { end, .. } = &mut moved.shape {
            end.x += 1.5;
            end.y -= 0.25;
        }
        let changes = Changes {
            edits: vec![
                Edit::Replace {
                    target: target.clone(),
                    element: Box::new(Placed::Model(moved.clone())),
                },
                Edit::Add {
                    place: "space/0".into(),
                    element: Box::new(Placed::Model(line)),
                },
            ],
            document: Some(Hash::document(&src)),
        };
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let applied = file
            .apply(&changes, &mut Cx::new(&mut budget, &mut diag, &cancel))
            .unwrap_or_else(|error| {
                panic!(
                    "{}: {error:?} {:?}",
                    path.display(),
                    diag.iter().map(|d| d.code()).collect::<Vec<_>>()
                )
            });
        let written = testkit::write::<C>(&file).unwrap();
        let again: BTreeMap<_, _> = seen(&testkit::read::<C>(&written).0.unwrap())
            .into_iter()
            .map(|(seen, _)| (seen.id, seen.hash))
            .collect();
        assert_eq!(again.len(), before.len() + 1, "{}", path.display());
        for (applied, expected_id) in applied.iter().zip([Some(&target.id), None]) {
            let id = applied.id.as_ref().unwrap();
            if let Some(expected) = expected_id {
                assert_eq!(id, expected, "{}", path.display());
            }
            assert_eq!(
                applied.hash.as_ref(),
                again.get(id),
                "{}: {id}",
                path.display()
            );
        }
        assert_ne!(applied[0].hash, Some(target.hash), "{}", path.display());
        true
    }
    let mut checked = 0;
    for (dwg, dxf) in pairs() {
        checked += usize::from(check::<Dwg>(&dwg));
        checked += usize::from(check::<Dxf>(&dxf));
    }
    if let Some(dir) = std::env::var_os("ZUMEN_SAMPLES").filter(|d| !d.is_empty()) {
        let binary = PathBuf::from(dir).join("features-r2018-binary.dxf");
        if binary.exists() {
            assert!(check::<Dxf>(&binary));
        }
    }
    assert!(pairs().is_empty() || checked > 0);
}

/// 試料の線のスタイルを ID で変えると、形は変わらず、変えた欄だけが読み直したときに変わっている。DWG は
/// 色と線種の倍率を、DXF はレイヤも変える（決定 0139）
#[cfg(feature = "drawing")]
#[test]
fn drawings_take_back_restyled_lines() {
    use zumen_drawing::{
        Applied, Changes, Color, ColorCode, ColorSystem, Drawing2D, Edit, Hash, Inherit, LayerId,
        Restyle, Seen, Shape, id,
    };
    use zumen_sdk::{Apply, Codec, Lift, testkit};
    fn lift<F: Lift<Drawing2D>>(file: &F) -> Drawing2D {
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
            .unwrap()
    }
    fn check<C: Codec>(path: &Path, relayer: bool) -> bool
    where
        C::File: Lift<Drawing2D> + Apply<Changes, Output = Vec<Applied>>,
    {
        let src = std::fs::read(path).unwrap();
        let mut file = testkit::read::<C>(&src).0.unwrap();
        let drawing = lift(&file);
        let space = &drawing.spaces[0];
        let place = id::space(space.id);
        let mut namer = id::Namer::new(&place);
        let found = space
            .elements
            .iter()
            .enumerate()
            .map(|(index, element)| (namer.next(index, element), element))
            .find(|(id, element)| {
                matches!(element.shape, Shape::Line { .. })
                    && id.starts_with("space/0/h")
                    && id.matches('/').count() == 2
            });
        let Some((target, line)) = found else {
            return false;
        };
        let layer = relayer
            .then(|| {
                (0..drawing.layers.len() as u32)
                    .map(LayerId)
                    .find(|layer| *layer != line.layer)
            })
            .flatten();
        let green = Inherit::Own(Color {
            code: Some(ColorCode {
                system: ColorSystem::Aci,
                value: 3,
            }),
            rgb: None,
        });
        // DWG は、今ある図形のレイヤを書き戻せないので、何も変えずに断る
        if !relayer && drawing.layers.len() > 1 {
            let other = (0..drawing.layers.len() as u32)
                .map(LayerId)
                .find(|layer| *layer != line.layer);
            let refused = Changes {
                edits: vec![Edit::Restyle {
                    target: Seen {
                        id: target.clone(),
                        hash: Hash::element(line),
                    },
                    style: Restyle {
                        layer: other,
                        ..Restyle::default()
                    },
                }],
                document: None,
            };
            let mut diag = Diagnostics::new();
            let mut budget = Budget::default();
            let cancel = CancelToken::new();
            let result = file.apply(&refused, &mut Cx::new(&mut budget, &mut diag, &cancel));
            assert!(result.is_err(), "{}", path.display());
            assert_eq!(
                diag.iter().map(|d| d.code()).collect::<Vec<_>>(),
                [codes::CHANGE_EDIT_UNSUPPORTED],
                "{}",
                path.display()
            );
            assert_eq!(
                testkit::write::<C>(&file).unwrap(),
                src,
                "{}",
                path.display()
            );
        }
        let changes = Changes {
            edits: vec![Edit::Restyle {
                target: Seen {
                    id: target.clone(),
                    hash: Hash::element(line),
                },
                style: Restyle {
                    layer,
                    color: Some(green),
                    line_type_scale: Some(line.style.line_type_scale * 2.0),
                    ..Restyle::default()
                },
            }],
            document: Some(Hash::document(&src)),
        };
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let applied = file
            .apply(&changes, &mut Cx::new(&mut budget, &mut diag, &cancel))
            .unwrap_or_else(|error| {
                panic!(
                    "{}: {error:?} {:?}",
                    path.display(),
                    diag.iter().map(|d| d.code()).collect::<Vec<_>>()
                )
            });
        let written = testkit::write::<C>(&file).unwrap();
        let after = lift(&testkit::read::<C>(&written).0.unwrap());
        let space = &after.spaces[0];
        let place = id::space(space.id);
        let mut namer = id::Namer::new(&place);
        let (_, restyled) = space
            .elements
            .iter()
            .enumerate()
            .map(|(index, element)| (namer.next(index, element), element))
            .find(|(id, _)| *id == target)
            .unwrap();
        assert_eq!(restyled.shape, line.shape, "{}", path.display());
        assert_eq!(restyled.style.color, green, "{}", path.display());
        assert_eq!(
            restyled.style.line_type_scale,
            line.style.line_type_scale * 2.0,
            "{}",
            path.display()
        );
        assert_eq!(
            restyled.layer,
            layer.unwrap_or(line.layer),
            "{}",
            path.display()
        );
        assert_eq!(applied[0].id.as_deref(), Some(target.as_str()));
        assert_eq!(
            applied[0].hash,
            Some(Hash::element(restyled)),
            "{}",
            path.display()
        );
        true
    }
    let mut checked = 0;
    for (dwg, dxf) in pairs() {
        checked += usize::from(check::<Dwg>(&dwg, false));
        checked += usize::from(check::<Dxf>(&dxf, true));
    }
    assert!(pairs().is_empty() || checked > 0);
}

/// 試料の線・円・弧・折れ線・文字・記号（INSERT）を、種類ごとに 1 つずつ ID で回して動かすと、読み直した形は変換で
/// 写した形と同じになり、返すハッシュも読み直したときと同じになる（決定 0139）
#[cfg(feature = "drawing")]
#[test]
#[allow(
    clippy::disallowed_methods,
    reason = "環境変数を読むのは試験だけで、コアは読まない（AGENTS.md の図面と秘密）"
)]
fn drawings_take_back_transformed_elements() {
    use zumen_drawing::{
        Affine, Applied, Changes, Drawing2D, Edit, Grid, Hash, Length, ModelLength, Placed, Seen,
        Shape, id, transform::transform,
    };
    use zumen_sdk::{Apply, Codec, Lift, testkit};
    fn lift<F: Lift<Drawing2D>>(file: &F) -> Drawing2D {
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
            .unwrap()
    }
    fn kind<P>(shape: &Shape<P>) -> Option<&'static str> {
        Some(match shape {
            Shape::Line { .. } => "line",
            Shape::Circle { .. } => "circle",
            Shape::Curve(_) => "curve",
            Shape::Polyline(_) => "polyline",
            Shape::Text(_) => "text",
            // 点（`dxf.point`）は、置き換えをまだ扱わない。格子に並べる記号（MINSERT）は分けて数える
            Shape::Symbol(symbol)
                if symbol.key.namespace == "dxf.block" && symbol.grid.is_some() =>
            {
                "grid"
            }
            Shape::Symbol(symbol) if symbol.key.namespace == "dxf.block" => "symbol",
            _ => return None,
        })
    }
    /// 動かした要素の種類と、鏡映して書き戻せた要素の種類
    fn check<C: Codec>(path: &Path) -> (Vec<&'static str>, Vec<&'static str>)
    where
        C::File: Lift<Drawing2D> + Apply<Changes, Output = Vec<Applied>>,
    {
        let src = std::fs::read(path).unwrap();
        let drawing = lift(&testkit::read::<C>(&src).0.unwrap());
        let space = &drawing.spaces[0];
        let place = id::space(space.id);
        let mut namer = id::Namer::new(&place);
        let mut kinds = Vec::new();
        let mut targets = Vec::new();
        let mut mirrors = Vec::new();
        for (index, element) in space.elements.iter().enumerate() {
            let id = namer.next(index, element);
            if let Some(kind) = kind(&element.shape)
                && id.matches('/').count() == 2
                && !kinds.contains(&kind)
            {
                kinds.push(kind);
                targets.push((kind, id, element));
            }
        }
        // 30° 回し、尺度 1.5 にして、(100, -50) 動かす。鏡映するときは、先に X 軸で鏡映する
        let affine = |mirrored: bool| {
            let turned = Affine::rotation(0.0, 0.0, 30.0);
            let [a, b, _, d, e, _] = turned.m;
            let flip = if mirrored { -1.0 } else { 1.0 };
            Affine {
                m: [
                    a * 1.5,
                    b * 1.5 * flip,
                    100.0,
                    d * 1.5,
                    e * 1.5 * flip,
                    -50.0,
                ],
            }
        };
        for ((kind, target, element), mirrored) in targets
            .iter()
            .flat_map(|target| [(target, false), (target, true)])
        {
            let affine = affine(mirrored);
            // 書式つきの属性を持つ記号は、鏡映を表せないので当てない
            let refused = mirrored
                && match &element.shape {
                    Shape::Symbol(symbol) => {
                        symbol.attributes.iter().any(|a| a.formatted.is_some())
                    }
                    _ => false,
                };
            let target = target.clone();
            let element = *element;
            let mut file = testkit::read::<C>(&src).0.unwrap();
            let changes = Changes {
                edits: vec![Edit::Transform {
                    target: Seen {
                        id: target.clone(),
                        hash: Hash::element(element),
                    },
                    transform: affine,
                }],
                document: Some(Hash::document(&src)),
            };
            let mut diag = Diagnostics::new();
            let mut budget = Budget::default();
            let cancel = CancelToken::new();
            let result = file.apply(&changes, &mut Cx::new(&mut budget, &mut diag, &cancel));
            if refused {
                assert_eq!(result.unwrap_err(), zumen_sdk::Error::Rejected, "{target}");
                assert_eq!(
                    diag.iter().map(|d| d.code()).collect::<Vec<_>>(),
                    [codes::CHANGE_EDIT_UNSUPPORTED],
                    "{} {target}",
                    path.display()
                );
                continue;
            }
            if mirrored {
                mirrors.push(*kind);
            }
            let applied = result.unwrap_or_else(|error| {
                panic!(
                    "{} {target}: {error:?} {:?}",
                    path.display(),
                    diag.iter().map(|d| d.code()).collect::<Vec<_>>()
                )
            });
            let written = testkit::write::<C>(&file).unwrap();
            let after = lift(&testkit::read::<C>(&written).0.unwrap());
            let space = &after.spaces[0];
            let place = id::space(space.id);
            let mut namer = id::Namer::new(&place);
            let (_, moved) = space
                .elements
                .iter()
                .enumerate()
                .map(|(index, element)| (namer.next(index, element), element))
                .find(|(id, _)| *id == target)
                .unwrap();
            // 書き戻すとレコードの位置が変わるので、属性の出所は鍵だけで比べる。鏡映した弧は、
            // ファイルが反時計回りでしか持てないので、始まりと終わりを入れ替えた反時計回りの弧と比べる
            let keyed = |shape: &Shape<_>| {
                let mut shape = shape.clone();
                if let Shape::Curve(zumen_drawing::Edge::Arc {
                    start,
                    end,
                    counterclockwise,
                    ..
                }) = &mut shape
                    && !*counterclockwise
                {
                    std::mem::swap(start, end);
                    *counterclockwise = true;
                }
                if let Shape::Symbol(symbol) = &mut shape {
                    for attribute in &mut symbol.attributes {
                        let key = attribute.source.key();
                        attribute.source = key
                            .map(|key| zumen_base::Ref::default().with_key(key))
                            .unwrap_or_default();
                    }
                }
                shape
            };
            let expected = transform(&element.shape, &affine).unwrap();
            assert_eq!(
                keyed(&moved.shape),
                keyed(&expected),
                "{} {target}",
                path.display()
            );
            assert_eq!(moved.layer, element.layer, "{}", path.display());
            assert_eq!(moved.style, element.style, "{}", path.display());
            assert_eq!(applied[0].id.as_deref(), Some(target.as_str()));
            assert_eq!(
                applied[0].hash,
                Some(Hash::element(moved)),
                "{}",
                path.display()
            );
        }
        // 格子に並べる記号は試料にないので、属性を除いた記号を 3 列 2 行で足してから、回して動かし、鏡映する。
        // DWG は MINSERT として足す。R2004 から後の DWG は、MINSERT をまだ足せない
        let minsert = !src.starts_with(b"AC10") || src.starts_with(b"AC1015");
        if let Some((_, _, element)) = targets.iter().find(|(kind, _, _)| *kind == "symbol")
            && let Shape::Symbol(symbol) = &element.shape
        {
            let spacing = |value: f64| {
                Length::Model(ModelLength {
                    space: space.id,
                    value,
                })
            };
            let mut gridded = (*element).clone();
            gridded.shape = Shape::Symbol(zumen_drawing::Symbol {
                grid: Some(Grid {
                    columns: 3,
                    rows: 2,
                    column_spacing: spacing(600.0),
                    row_spacing: spacing(450.0),
                }),
                attributes: Vec::new(),
                ..symbol.clone()
            });
            let mut diag = Diagnostics::new();
            let mut budget = Budget::default();
            let cancel = CancelToken::new();
            let mut file = testkit::read::<C>(&src).0.unwrap();
            let added = file.apply(
                &Changes {
                    edits: vec![Edit::Add {
                        place: place.clone(),
                        element: Box::new(Placed::Model(gridded)),
                    }],
                    document: None,
                },
                &mut Cx::new(&mut budget, &mut diag, &cancel),
            );
            let added = match added {
                Ok(added) => added,
                Err(error) => {
                    assert!(!minsert, "{} {error:?}", path.display());
                    assert_eq!(
                        diag.iter().map(|d| d.code()).collect::<Vec<_>>(),
                        [codes::DWG_CHANGE_UNSUPPORTED]
                    );
                    return (kinds, mirrors);
                }
            };
            let base = testkit::write::<C>(&file).unwrap();
            let seen = |bytes: &[u8], target: &str| {
                let drawing = lift(&testkit::read::<C>(bytes).0.unwrap());
                let space = &drawing.spaces[0];
                let place = id::space(space.id);
                let mut namer = id::Namer::new(&place);
                space
                    .elements
                    .iter()
                    .enumerate()
                    .map(|(index, element)| (namer.next(index, element), element.clone()))
                    .find(|(id, _)| id == target)
                    .unwrap()
                    .1
            };
            let target = added[0].id.clone().unwrap();
            let was = seen(&base, &target);
            for mirrored in [false, true] {
                let affine = affine(mirrored);
                let mut file = testkit::read::<C>(&base).0.unwrap();
                let applied = file
                    .apply(
                        &Changes {
                            edits: vec![Edit::Transform {
                                target: Seen {
                                    id: target.clone(),
                                    hash: Hash::element(&was),
                                },
                                transform: affine,
                            }],
                            document: None,
                        },
                        &mut Cx::new(&mut budget, &mut diag, &cancel),
                    )
                    .unwrap_or_else(|error| panic!("{} {error:?}", path.display()));
                let moved = seen(&testkit::write::<C>(&file).unwrap(), &target);
                assert_eq!(
                    moved.shape,
                    transform(&was.shape, &affine).unwrap(),
                    "{}",
                    path.display()
                );
                assert_eq!(applied[0].hash, Some(Hash::element(&moved)));
                if mirrored {
                    mirrors.push("grid");
                }
            }
        }
        (kinds, mirrors)
    }
    let mut lines = 0;
    for (dwg, dxf) in pairs() {
        for (kinds, _) in [check::<Dwg>(&dwg), check::<Dxf>(&dxf)] {
            lines += usize::from(kinds.contains(&"line"));
        }
    }
    if let Some(dir) = std::env::var_os("ZUMEN_SAMPLES").filter(|d| !d.is_empty()) {
        let binary = PathBuf::from(dir).join("features-r2018-binary.dxf");
        if binary.exists() {
            let (kinds, mirrors) = check::<Dxf>(&binary);
            assert!(kinds.contains(&"symbol"));
            assert!(mirrors.contains(&"symbol"));
        }
    }
    assert!(pairs().is_empty() || lines > 0);
}

/// 試料の記号の定義の中の線を ID で動かすと、読み直した線は動いていて、ID は変わらない（決定 0139・0156）
#[cfg(feature = "drawing")]
#[test]
fn drawings_take_back_lines_moved_inside_symbol_definitions() {
    use zumen_drawing::{
        Affine, Applied, Changes, Drawing2D, Edit, Element, Hash, ModelPoint, Seen, Shape, id,
        transform::transform,
    };
    use zumen_sdk::{Apply, Codec, Lift, testkit};
    fn lift<F: Lift<Drawing2D>>(file: &F) -> Drawing2D {
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
            .unwrap()
    }
    /// 名前を持つ定義の中の、ハンドルで指す要素
    fn named(drawing: &Drawing2D) -> Vec<(String, Element<ModelPoint>)> {
        let mut found = Vec::new();
        let mut definitions = id::DefinitionNamer::default();
        for definition in &drawing.symbols {
            let place = definitions.next(definition);
            if definition.anonymous {
                continue;
            }
            let mut namer = id::Namer::new(&place);
            for (index, element) in definition.space.elements.iter().enumerate() {
                let id = namer.next(index, element);
                if id.matches('/').count() == 2 && id.contains("/h") {
                    found.push((id, element.clone()));
                }
            }
        }
        found
    }
    fn check<C: Codec>(path: &Path) -> bool
    where
        C::File: Lift<Drawing2D> + Apply<Changes, Output = Vec<Applied>>,
    {
        let src = std::fs::read(path).unwrap();
        let mut file = testkit::read::<C>(&src).0.unwrap();
        let Some((target, line)) = named(&lift(&file))
            .into_iter()
            .find(|(_, element)| matches!(element.shape, Shape::Line { .. }))
        else {
            return false;
        };
        let moved = Affine::translation(7.0, -3.0);
        let changes = Changes {
            edits: vec![Edit::Transform {
                target: Seen {
                    id: target.clone(),
                    hash: Hash::element(&line),
                },
                transform: moved,
            }],
            document: Some(Hash::document(&src)),
        };
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let applied = file
            .apply(&changes, &mut Cx::new(&mut budget, &mut diag, &cancel))
            .unwrap_or_else(|error| panic!("{} {target}: {error:?}", path.display()));
        let written = testkit::write::<C>(&file).unwrap();
        let after = named(&lift(&testkit::read::<C>(&written).0.unwrap()));
        let (_, now) = after.iter().find(|(id, _)| *id == target).unwrap();
        assert_eq!(
            now.shape,
            transform(&line.shape, &moved).unwrap(),
            "{}",
            path.display()
        );
        assert_eq!(applied[0].id.as_deref(), Some(target.as_str()));
        assert_eq!(
            applied[0].hash,
            Some(Hash::element(now)),
            "{}",
            path.display()
        );
        true
    }
    let mut checked = 0;
    for (dwg, dxf) in pairs() {
        checked += usize::from(check::<Dwg>(&dwg));
        checked += usize::from(check::<Dxf>(&dxf));
    }
    assert!(pairs().is_empty() || checked > 0);
}

/// 図面モデルで動かした線を、DWG に書き戻して読み直すと、同じ所に動いている（決定 0139）
#[cfg(feature = "drawing")]
#[test]
fn dwg_drawings_take_back_lines_moved_in_the_drawing_model() {
    use crate::by_source::{Apply, Changes, Edit};
    use zumen_drawing::{Drawing2D, Shape};
    use zumen_sdk::{Lift, testkit};
    let lift = |file: &zumen_acad::DwgFile| -> Drawing2D {
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
            .unwrap()
    };
    let mut checked = 0;
    for (dwg, _) in pairs() {
        let src = std::fs::read(&dwg).unwrap();
        let mut file = testkit::read::<Dwg>(&src).0.unwrap();
        let elements = lift(&file).spaces.swap_remove(0).elements;
        let Some(index) = elements
            .iter()
            .position(|e| matches!(e.shape, Shape::Line { .. }))
        else {
            continue;
        };
        let mut moved = elements[index].clone();
        if let Shape::Line { end, .. } = &mut moved.shape {
            end.x += 1.5;
            end.y -= 0.25;
        }
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let changes = Changes {
            edits: vec![Edit::Replace(Box::new(moved.clone()))],
        };
        file.apply(&changes, &mut Cx::new(&mut budget, &mut diag, &cancel))
            .unwrap();
        let written = testkit::write::<Dwg>(&file).unwrap();
        let again = testkit::read::<Dwg>(&written).0.unwrap();
        let after = lift(&again).spaces.swap_remove(0).elements;
        assert_eq!(after.len(), elements.len(), "{}", dwg.display());
        assert_eq!(after[index].shape, moved.shape, "{}", dwg.display());
        checked += 1;
    }
    assert!(pairs().is_empty() || checked > 0);
}

/// DWG の見出しから読んだ変数（INSUNITS・PLIMMIN・PLIMMAX・PDMODE）は、同じ図面の DXF の HEADER と同じ値を持つ
#[test]
fn dwg_header_variables_match_the_dxf_of_the_same_drawing() {
    let names: [&[u8]; 4] = [b"$INSUNITS", b"$PLIMMIN", b"$PLIMMAX", b"$PDMODE"];
    for (dwg, dxf) in pairs() {
        let (ours, theirs) = (table(&dwg), table(&dxf));
        for name in names {
            let at = format!("{} {}", dwg.display(), String::from_utf8_lossy(name));
            assert!(ours.variable(name).is_some(), "{at}");
            let (ours, theirs) = (
                format!("{:?}", ours.variable(name)),
                format!("{:?}", theirs.variable(name)),
            );
            assert!(nearly(&ours, &theirs), "{at}\n{ours}\n{theirs}");
        }
    }
}

#[cfg(feature = "drawing")]
#[test]
fn dwg_and_dxf_of_the_same_drawing_lift_to_the_same_drawing() {
    for (dwg, dxf) in pairs() {
        let (mut ours, our_codes) = lifted(&dwg);
        let (mut theirs, their_codes) = lifted(&dxf);
        // AutoCAD の往復のための拡張データ（ACAD の RTTcAl などと ACAD_SPLINE_DATA）は、DXF に書かれない
        // ACAD の拡張データの初めの RTTcAl・2 つの数・名前の 4 つの値も、DXF に書かれない
        for ext in &mut ours.extensions {
            if ext.name == "ACAD"
                && ext.fields.first().map(|f| &f.value)
                    == Some(&zumen_drawing::FieldValue::Text("RTTcAl".into()))
            {
                ext.fields.drain(..ext.fields.len().min(4));
            }
        }
        ours.extensions.retain(|ext| {
            !matches!(ext.name.as_str(), "ACAD" | "ACAD_SPLINE_DATA")
                || theirs
                    .extensions
                    .iter()
                    .any(|their| their.name == ext.name && their.target == ext.target)
        });
        support::without_unpaired_record_roundtrips(&mut ours, &mut theirs);
        for (ours, theirs) in ours.spaces.iter().zip(&mut theirs.spaces) {
            without_hooks(&ours.elements, &mut theirs.elements);
        }
        for (ours, theirs) in ours.sheets.iter().zip(&mut theirs.sheets) {
            without_hooks(&ours.elements, &mut theirs.elements);
        }
        for (ours, theirs) in ours.symbols.iter().zip(&mut theirs.symbols) {
            without_hooks(&ours.space.elements, &mut theirs.space.elements);
        }
        let (ours, theirs) = (
            comparable(&format!("{ours:?}")),
            comparable(&format!("{theirs:?}")),
        );
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
        let removed = &before.objects()[count - 1];
        // 除いたものが APPID なら、その名前の拡張データは名前が分からなくなるので置かれない
        let app = (removed.kind == b"APPID")
            .then(|| removed.first(2).cloned())
            .flatten();
        let kept: Vec<_> = before
            .objects()
            .iter()
            .filter(|o| o.handle() != removed.handle())
            .map(|o| {
                let mut o = o.clone();
                let mut dropped = false;
                o.values.retain(|v| {
                    if v.code == 1001 {
                        dropped = Some(&v.data) == app.as_ref();
                    }
                    !dropped
                });
                o
            })
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
        let found: Vec<_> = reread
            .iter()
            .map(|d| d.code())
            .filter(|code| app.is_none() || *code != codes::DWG_RECORD_MISSING)
            .collect();
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
    // 文字列を持つ図形と、R2004 と同じに書き戻せる種類（決定 0086・0097・0098）
    let more = |kind: &[u8]| {
        matches!(
            kind,
            b"TEXT"
                | b"LWPOLYLINE"
                | b"INSERT"
                | b"MLINE"
                | b"MTEXT"
                | b"VERTEX"
                | b"HATCH"
                | b"DIMENSION"
                | b"TOLERANCE"
                | b"VIEWPORT"
                | b"SHAPE"
                | b"SPLINE"
                | b"ATTRIB"
                | b"ATTDEF"
        )
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
                // MINSERT（列の数の 70 を持つ INSERT）は除く
                // 多面の面の VERTEX（70 が 128）の点は 0 と決まっているので動かさない
                let skipped = kind == b"INSERT" && object.values.iter().any(|v| v.code == 70)
                    || kind == b"VERTEX" && object.first(70) == Some(&Data::Int(128));
                if !SIMPLE.contains(&kind.as_slice()) && !more(&kind) || skipped {
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
                    // 段組みの MTEXT の段組みの欄（101 から後）は、元のビットを写すので変えない
                    if kind == b"MTEXT" && value.code == 101 {
                        break;
                    }
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
                    // R2010 からは、文字列の流れの文字列と、線種の文字の要素の文字列も変える（決定 0100）
                    (b"STYLE" | b"LTYPE", 3, Data::Text(text))
                    | (b"LTYPE", 9, Data::Text(text))
                        if r2010 =>
                    {
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
                    (b"MLINESTYLE", 49, Data::Real(v)) => *v *= 2.0,
                    (b"MLINESTYLE", 62, Data::Int(v)) => *v = *v % 255 + 1,
                    (b"LAYOUT", 146, Data::Real(v)) => *v += 1.0,
                    (b"LAYOUT", 78, Data::Int(v)) => *v += 100,
                    (b"XRECORD", _, Data::Real(v)) => *v += 1.0,
                    (b"XRECORD", code, Data::Text(text)) if code != 102 => text.extend(b"0"),
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
                    (b"XRECORD", code, Data::Text(text)) if code != 102 => text.extend(b"0"),
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
                        int(70, 0),
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
                        int(70, 4),
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
            // 回した INSERT も足す（決定 0101）
            added.push((
                b"INSERT",
                vec![
                    real(10, 8.0),
                    real(20, 9.0),
                    real(30, 0.0),
                    real(50, 90.0),
                    text(2, name),
                ],
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
        // R2010 から後は、R2013 からの当てはめる点の SPLINE をまだ足せない（決定 0092・0093・0097）。
        // MTEXT は枠の高さ（46）を、ATTRIB は属性の版と位置を固定するか（280）を持つ。R2018 からは、初めの
        // ATTRIB を、埋め込みの MTEXT を持つ複数行の属性とする（決定 0102）
        if !matches!(version, b"AC1015" | b"AC1018") {
            let mut multiline = version == b"AC1032";
            added.retain(|(kind, own)| match *kind {
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
                    if multiline {
                        multiline = false;
                        own.extend([
                            int(71, 2),
                            int(72, 0),
                            text(101, b"Embedded Object"),
                            real(10, 8.0),
                            real(20, 9.0),
                            real(30, 0.0),
                            real(41, 0.0),
                            real(46, 0.0),
                            real(40, 2.5),
                            int(71, 1),
                            int(72, 5),
                            text(1, b"one\\Ptwo"),
                            int(73, 1),
                            real(44, 1.0),
                        ]);
                    }
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
            // 足すオブジェクトは拡張データと拡張辞書を持てないので、写したレコードの拡張データと拡張辞書の組を除く
            let mut layer = named(b"LAYER", b"0");
            layer.origin = Origin::New;
            layer
                .values
                .retain(|v| v.code < 1000 && !matches!(v.code, 102 | 360));
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
            style
                .values
                .retain(|v| v.code < 1000 && !matches!(v.code, 102 | 360));
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
            vport_record
                .values
                .retain(|v| v.code < 1000 && !matches!(v.code, 102 | 360));
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
                // 挿す単位（mm）・分解できるか・尺度を揃えるか。DWG は R2010 から持つ
                .chain(
                    [(70, Data::Int(4)), (280, Data::Int(0)), (281, Data::Int(1))]
                        .into_iter()
                        .filter(|_| version >= b"AC1024".as_slice()),
                )
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
            // DWG の BLOCK は名前を値に持たず、ブロックのレコードの印（70）と基点を持つ
            let at = db.objects().len() - 4;
            let values = &mut db.object_mut(at).unwrap().values;
            values.retain(|v| v.code != 2);
            values.insert(
                values.len() - 3,
                Value {
                    code: 70,
                    data: Data::Int(0),
                },
            );
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

/// 図面モデルで動かした円弧・楕円・軽量の折れ線・文字・MTEXT・記号を、DWG と DXF に書き戻して読み直すと、同じ所に動いている
/// （決定 0139）
#[cfg(feature = "drawing")]
#[test]
fn drawings_take_back_curves_and_polylines_moved_in_the_drawing_model() {
    use crate::by_source::{Apply, Changes, Edit};
    use zumen_drawing::{Drawing2D, Edge, Element, ModelPoint, Placement, Shape};
    use zumen_sdk::{Lift, testkit};
    fn lift<F: Lift<Drawing2D>>(file: &F) -> Vec<Element<ModelPoint>> {
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
            .unwrap()
            .spaces
            .swap_remove(0)
            .elements
    }
    /// 形を (dx, dy) だけ動かす。動かせない形なら `None`
    fn moved(shape: &Shape<ModelPoint>) -> Option<Shape<ModelPoint>> {
        let by = |p: &mut ModelPoint| {
            p.x += 1.5;
            p.y -= 0.25;
        };
        let mut shape = shape.clone();
        match &mut shape {
            Shape::Curve(Edge::Arc { center, .. }) => by(center),
            Shape::Curve(Edge::Elliptic { center, major, .. }) => {
                by(center);
                by(major);
            }
            Shape::Polyline(line) => line.vertices.iter_mut().for_each(|v| by(&mut v.point)),
            Shape::Text(text) => match &mut text.placement {
                Placement::Anchor { point, .. } => by(point),
                Placement::Between { first, second, .. } => {
                    by(first);
                    by(second);
                }
            },
            Shape::Formatted(text) => by(&mut text.point),
            Shape::Symbol(symbol) => {
                by(&mut symbol.position);
                symbol.rotation += 15.0;
                symbol.scale[0] *= 2.0;
            }
            _ => return None,
        }
        Some(shape)
    }
    /// 書式のない内容 `content` の MTEXT を読んで得る区間
    fn plain(content: &str) -> Vec<zumen_drawing::Span> {
        let src = format!(
            "  0\nSECTION\n  2\nENTITIES\n  0\nMTEXT\n  5\n1A\n  8\n0\n 10\n0.0\n 20\n0.0\n 40\n2.5\n  1\n{content}\n  0\nENDSEC\n  0\nEOF\n"
        );
        match &lift(&testkit::read::<Dxf>(src.as_bytes()).0.unwrap())[0].shape {
            Shape::Formatted(text) => text.spans.clone(),
            shape => panic!("{shape:?}"),
        }
    }
    fn check<C: Codec>(path: &Path, kinds: &mut BTreeMap<&'static str, usize>)
    where
        C::File: Lift<Drawing2D> + Apply,
    {
        let src = std::fs::read(path).unwrap();
        let mut file = testkit::read::<C>(&src).0.unwrap();
        let elements = lift(&file);
        let mut edits = Vec::new();
        let mut expected = Vec::new();
        for kind in ["arc", "ellipse", "polyline", "text", "mtext", "symbol"] {
            let found = elements.iter().enumerate().find_map(|(index, element)| {
                let name = match element.shape {
                    Shape::Curve(Edge::Arc { .. }) => "arc",
                    Shape::Curve(Edge::Elliptic { .. }) => "ellipse",
                    Shape::Polyline(_) => "polyline",
                    Shape::Text(_) => "text",
                    Shape::Formatted(_) => "mtext",
                    // POINT も記号になるので、ブロックを置く INSERT だけを選ぶ。属性を持つ記号は、置き換えでは
                    // 属性を動かせないので動かさない（`Transform` で動かす）
                    Shape::Symbol(ref symbol)
                        if symbol.key.namespace == "dxf.block" && symbol.attributes.is_empty() =>
                    {
                        "symbol"
                    }
                    _ => return None,
                };
                (name == kind).then(|| (index, moved(&element.shape)))
            });
            let Some((index, Some(shape))) = found else {
                continue;
            };
            let mut element = elements[index].clone();
            element.shape = shape;
            if let Shape::Formatted(text) = &mut element.shape {
                // 内容も書き換える。区間は内容から決まるので、同じ内容を読んだ区間に合わせる
                text.content = "ZUMEN".to_owned();
                text.spans = plain(&text.content);
            }
            edits.push(Edit::Replace(Box::new(element.clone())));
            expected.push((index, element.shape));
            *kinds.entry(kind).or_default() += 1;
        }
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        file.apply(
            &Changes { edits },
            &mut Cx::new(&mut budget, &mut diag, &cancel),
        )
        .unwrap();
        let written = testkit::write::<C>(&file).unwrap();
        let after = lift(&testkit::read::<C>(&written).0.unwrap());
        assert_eq!(after.len(), elements.len(), "{}", path.display());
        for (index, shape) in expected {
            let mut found = after[index].shape.clone();
            // DWG は記号の回転をラジアンで持つので、度に戻すと最後の桁が違うことがある
            if let (Shape::Symbol(found), Shape::Symbol(shape)) = (&mut found, &shape) {
                assert!(
                    (found.rotation - shape.rotation).abs() < 1e-9,
                    "{}",
                    path.display()
                );
                found.rotation = shape.rotation;
            }
            assert_eq!(found, shape, "{}", path.display());
        }
    }
    let mut kinds = BTreeMap::new();
    for (dwg, dxf) in pairs() {
        check::<Dwg>(&dwg, &mut kinds);
        check::<Dxf>(&dxf, &mut kinds);
    }
    assert!(pairs().is_empty() || kinds.len() == 6, "{kinds:?}");
}

/// 図面モデルで足した線分・円・円弧・楕円・軽量の折れ線・文字・記号を、DWG と DXF に書き戻して読み直すと、
/// モデル空間の終わりに同じ形で並ぶ（決定 0155）
#[cfg(feature = "drawing")]
#[test]
fn drawings_take_back_elements_added_in_the_drawing_model() {
    use crate::by_source::{Apply, Changes, Edit};
    use zumen_drawing::{
        Attribute, Drawing2D, Edge, Element, Flow, Format, FormattedText, Height, HeightMeaning,
        Horizontal, Inherit, Length, LineSpacing, LineWeight, ModelLength, ModelPoint, PaperLength,
        Piece, Placement, Polyline, Rotation, Shape, SpaceId, Span, Style, Text, Vertex, Vertical,
    };
    use zumen_sdk::{Error, Lift, testkit};
    fn lift<F: Lift<Drawing2D>>(file: &F) -> Drawing2D {
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
            .unwrap()
    }
    let at = |x: f64, y: f64| ModelPoint {
        space: SpaceId(0),
        x,
        y,
    };
    let length = |value: f64| {
        Length::Model(ModelLength {
            space: SpaceId(0),
            value,
        })
    };
    let shapes = vec![
        Shape::Line {
            start: at(1.0, 2.0),
            end: at(3.0, 4.0),
        },
        Shape::Circle {
            center: at(5.0, 6.0),
            radius: length(7.0),
        },
        Shape::Curve(Edge::Arc {
            center: at(1.0, 1.0),
            radius: length(2.0),
            start: 30.0,
            end: 120.0,
            counterclockwise: true,
        }),
        Shape::Curve(Edge::Elliptic {
            center: at(5.0, 5.0),
            major: at(8.0, 9.0),
            ratio: 0.5,
            start: 10.0,
            end: 200.0,
            counterclockwise: true,
        }),
        Shape::Polyline(Polyline {
            vertices: [(0.0, 0.0, 0.5), (4.0, 0.0, 0.0), (4.0, 3.0, 0.0)]
                .map(|(x, y, bulge)| Vertex {
                    point: at(x, y),
                    bulge,
                })
                .to_vec(),
            closed: true,
            width: Some(length(0.25)),
            vertex_widths: Vec::new(),
        }),
        Shape::Text(Text {
            content: "zumen".to_owned(),
            height: Height {
                value: length(2.5),
                meaning: HeightMeaning::Declared("dxf.TEXT.40"),
            },
            placement: Placement::Anchor {
                point: at(3.0, 3.0),
                horizontal: Horizontal::Center,
                vertical: Vertical::Middle,
            },
            rotation: 15.0,
            width_factor: 0.8,
            oblique: 0.0,
            backward: false,
            upside_down: false,
            style: "Standard".to_owned(),
        }),
        Shape::Formatted(FormattedText {
            content: "zumen\\P{\\Ldrawing}".to_owned(),
            spans: vec![
                Span {
                    piece: Piece::Text("zumen".to_owned()),
                    format: Format::default(),
                },
                Span {
                    piece: Piece::Paragraph,
                    format: Format::default(),
                },
                Span {
                    piece: Piece::Text("drawing".to_owned()),
                    format: Format {
                        underline: true,
                        ..Format::default()
                    },
                },
            ],
            height: Height {
                value: length(2.5),
                meaning: HeightMeaning::Declared("dxf.MTEXT.40"),
            },
            point: at(6.0, 1.0),
            horizontal: Horizontal::Right,
            vertical: Vertical::Bottom,
            rotation: Rotation::Direction([0.0, 1.0]),
            width: Some(length(12.0)),
            frame_height: None,
            flow: Flow::LeftToRight,
            line_spacing: LineSpacing {
                factor: 1.5,
                exact: true,
            },
            background: None,
            style: "Standard".to_owned(),
        }),
    ];
    let check = |path: &Path, dwg: bool, symbols: &mut usize| {
        fn read<C: Codec>(src: &[u8]) -> C::File {
            testkit::read::<C>(src).0.unwrap()
        }
        fn run<C: Codec>(path: &Path, dwg: bool, shapes: &[Shape<ModelPoint>], symbols: &mut usize)
        where
            C::File: Lift<Drawing2D> + Apply,
        {
            let src = std::fs::read(path).unwrap();
            let mut file = read::<C>(&src);
            let drawing = lift(&file);
            let before = drawing.spaces[0].elements.len();
            let layer = drawing
                .layers
                .iter()
                .find(|layer| layer.name == "0")
                .unwrap()
                .id;
            let mut shapes = shapes.to_vec();
            // 名前のあるブロックを置く記号があれば、その定義を回して置く記号も足す
            let symbol =
                drawing.spaces[0]
                    .elements
                    .iter()
                    .find_map(|element| match &element.shape {
                        Shape::Symbol(symbol)
                            if symbol.key.namespace == "dxf.block"
                                && !symbol.key.name.starts_with('*')
                                && symbol.grid.is_none() =>
                        {
                            let mut symbol = symbol.clone();
                            symbol.position = ModelPoint {
                                space: SpaceId(0),
                                x: 8.0,
                                y: 9.0,
                            };
                            symbol.rotation = 90.0;
                            symbol.scale = [2.0, 2.0];
                            symbol.attributes.clear();
                            symbol.clip = None;
                            Some(Shape::Symbol(symbol))
                        }
                        _ => None,
                    });
            if let Some(symbol) = symbol {
                // 属性を 1 つ持つ、同じ定義の記号も足す
                let mut attributed = symbol.clone();
                let (Shape::Symbol(placed), Shape::Text(text)) = (&mut attributed, &shapes[5])
                else {
                    panic!("the template is a symbol and a text")
                };
                {
                    let mut text = text.clone();
                    text.height.meaning = HeightMeaning::Declared("dxf.ATTRIB.40");
                    text.content = "zumen-attribute".to_owned();
                    placed.attributes.push(Attribute {
                        tag: "ZUMEN".to_owned(),
                        text,
                        formatted: None,
                        hidden: true,
                        layer,
                        style: Style {
                            color: Inherit::Layer,
                            line_type: Inherit::Layer,
                            line_type_scale: 1.0,
                            line_weight: Some(Inherit::Layer),
                        },
                        source: zumen_base::Ref::default(),
                    });
                }
                shapes.push(symbol);
                shapes.push(attributed);
                *symbols += 1;
            }
            let edits = shapes
                .iter()
                .map(|shape| {
                    Edit::Add(Box::new(Element {
                        source: zumen_base::Ref::default(),
                        layer,
                        style: Style {
                            color: Inherit::Layer,
                            line_type: Inherit::Layer,
                            line_type_scale: 1.0,
                            line_weight: Some(Inherit::Layer),
                        },
                        shape: shape.clone(),
                    }))
                })
                .collect();
            let mut diag = Diagnostics::new();
            let mut budget = Budget::default();
            let cancel = CancelToken::new();
            let result = file.apply(
                &Changes { edits },
                &mut Cx::new(&mut budget, &mut diag, &cancel),
            );
            assert_eq!(
                result,
                Ok(()),
                "{}: {:?}",
                path.display(),
                diag.iter().map(|d| d.code()).collect::<Vec<_>>()
            );
            let written = testkit::write::<C>(&file).unwrap();
            let after = lift(&read::<C>(&written));
            let elements = &after.spaces[0].elements;
            assert_eq!(elements.len(), before + shapes.len(), "{}", path.display());
            for (found, shape) in elements[before..].iter().zip(&shapes) {
                // 文字の体裁は、大文字と小文字を区別せずに引いた定義の名前で書く
                let mut shape = shape.clone();
                match (&found.shape, &mut shape) {
                    (Shape::Text(found), Shape::Text(text)) => {
                        assert!(found.style.eq_ignore_ascii_case(&text.style));
                        text.style.clone_from(&found.style);
                    }
                    (Shape::Formatted(found), Shape::Formatted(text)) => {
                        assert!(found.style.eq_ignore_ascii_case(&text.style));
                        text.style.clone_from(&found.style);
                    }
                    // 属性の出所は、書き戻したファイルの ATTRIB を指す
                    (Shape::Symbol(found), Shape::Symbol(symbol)) => {
                        assert_eq!(found.attributes.len(), symbol.attributes.len());
                        for (found, attribute) in
                            found.attributes.iter().zip(&mut symbol.attributes)
                        {
                            assert!(found.text.style.eq_ignore_ascii_case(&attribute.text.style));
                            attribute.text.style.clone_from(&found.text.style);
                            attribute.source = found.source.clone();
                        }
                    }
                    _ => {}
                }
                assert!(
                    close(&format!("{:?}", found.shape), &format!("{shape:?}")),
                    "{}: {:?} != {shape:?}",
                    path.display(),
                    found.shape
                );
                assert_eq!(found.layer, layer, "{}", path.display());
            }
            // 線の太さを持つ線分は、DXF には足せ、DWG にはまだ足せない（決定 0079）
            let mut file = read::<C>(&src);
            let weighted = Element {
                source: zumen_base::Ref::default(),
                layer,
                style: Style {
                    color: Inherit::Layer,
                    line_type: Inherit::Layer,
                    line_type_scale: 1.0,
                    line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(0.25)))),
                },
                shape: shapes[0].clone(),
            };
            let mut diag = Diagnostics::new();
            let result = file.apply(
                &Changes {
                    edits: vec![Edit::Add(Box::new(weighted.clone()))],
                },
                &mut Cx::new(&mut budget, &mut diag, &cancel),
            );
            if dwg {
                assert_eq!(result, Err(Error::Rejected), "{}", path.display());
                assert_eq!(testkit::write::<C>(&file).unwrap(), src);
                // DWG に角度（50）を書けない MTEXT を、向き 0 として足してはいけない。
                let mut angled = weighted.clone();
                angled.style.line_weight = Some(Inherit::Layer);
                angled.shape = shapes
                    .iter()
                    .find(|shape| matches!(shape, Shape::Formatted(_)))
                    .unwrap()
                    .clone();
                if let Shape::Formatted(text) = &mut angled.shape {
                    text.rotation = zumen_drawing::Rotation::Angle(45.0);
                }
                let mut file = read::<C>(&src);
                let result = file.apply(
                    &Changes {
                        edits: vec![Edit::Add(Box::new(angled))],
                    },
                    &mut Cx::new(&mut budget, &mut diag, &cancel),
                );
                assert_eq!(result, Err(Error::Rejected), "{}", path.display());
                assert_eq!(testkit::write::<C>(&file).unwrap(), src);
            } else {
                assert_eq!(result, Ok(()), "{}", path.display());
                let written = testkit::write::<C>(&file).unwrap();
                let after = lift(&read::<C>(&written));
                let found = after.spaces[0].elements.last().unwrap();
                assert_eq!(found.style, weighted.style, "{}", path.display());
            }
        }
        if dwg {
            run::<Dwg>(path, dwg, &shapes, symbols);
        } else {
            run::<Dxf>(path, dwg, &shapes, symbols);
        }
    };
    let mut symbols = 0;
    for (dwg, dxf) in pairs() {
        check(&dwg, true, &mut symbols);
        check(&dxf, false, &mut symbols);
    }
    assert!(pairs().is_empty() || symbols > 0);
}

/// 図形とともに動く XDATA（位置・変位・向き・距離・倍率）を持つ図形の形を変えると、DWG も DXF も何も反映しない
/// （決定 0154）
#[cfg(feature = "drawing")]
#[test]
fn drawings_refuse_to_move_entities_with_moving_xdata() {
    use crate::by_source::{Apply, Changes, Edit};
    use zumen_drawing::{Drawing2D, Placement, Shape, Target};
    use zumen_sdk::{Error, Lift, testkit};
    fn check<C: Codec>(path: &Path) -> usize
    where
        C::File: Lift<Drawing2D> + Apply,
    {
        let src = std::fs::read(path).unwrap();
        let mut file = testkit::read::<C>(&src).0.unwrap();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut diag = Diagnostics::new();
        let drawing = file
            .lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
            .unwrap();
        let moving = ["position", "displacement", "direction", "distance", "scale"];
        let found = drawing.extensions.iter().find_map(|extension| {
            let Target::Model { space, index } = extension.target else {
                return None;
            };
            let space = drawing.spaces.iter().find(|s| s.id == space)?;
            let element = space.elements.get(index)?;
            (extension.namespace == "dxf.xdata"
                && extension.fields.iter().any(|f| moving.contains(&f.key))
                && matches!(element.shape, Shape::Text(_)))
            .then(|| element.clone())
        });
        let Some(mut element) = found else {
            return 0;
        };
        if let Shape::Text(text) = &mut element.shape
            && let Placement::Anchor { point, .. } = &mut text.placement
        {
            point.x += 1.0;
        }
        let at = element.source.clone();
        let mut diag = Diagnostics::new();
        let result = file.apply(
            &Changes {
                edits: vec![Edit::Replace(Box::new(element))],
            },
            &mut Cx::new(&mut budget, &mut diag, &cancel),
        );
        assert_eq!(result, Err(Error::Rejected), "{}", path.display());
        let found: Vec<_> = diag
            .iter()
            .map(|d| (d.code(), d.places().to_vec()))
            .collect();
        assert_eq!(
            found,
            [(codes::CHANGE_XDATA_UNSUPPORTED, vec![at])],
            "{}",
            path.display()
        );
        assert_eq!(
            testkit::write::<C>(&file).unwrap(),
            src,
            "{}",
            path.display()
        );
        1
    }
    let mut checked = 0;
    for (dwg, dxf) in pairs() {
        checked += check::<Dwg>(&dwg) + check::<Dxf>(&dxf);
    }
    assert!(pairs().is_empty() || checked >= 2, "{checked}");
}

/// 図面モデルで用紙の要素を動かして、DWG と DXF に書き戻して読み直すと、同じ所に動いている。今のレイアウトの
/// 用紙も、ほかのレイアウトの用紙も確かめる（決定 0139）
#[cfg(feature = "drawing")]
#[test]
#[allow(
    clippy::disallowed_methods,
    reason = "環境変数を読むのは試験だけで、コアは読まない（AGENTS.md の図面と秘密）"
)]
fn drawings_take_back_sheet_elements_moved_in_the_drawing_model() {
    use crate::by_source::{Apply, Changes, Edit};
    use zumen_drawing::{
        Drawing2D, Inherit, Length, PaperLength, PaperPoint, Placement, Shape, Style,
    };
    use zumen_sdk::{Lift, testkit};
    fn lift<F: Lift<Drawing2D>>(file: &F) -> Drawing2D {
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
            .unwrap()
    }
    fn moved(shape: &Shape<PaperPoint>) -> Option<Shape<PaperPoint>> {
        let by = |p: &mut PaperPoint| {
            p.x += 1.5;
            p.y -= 0.25;
        };
        let mut shape = shape.clone();
        match &mut shape {
            Shape::Line { start, end } => {
                by(start);
                by(end);
            }
            Shape::Circle { center, .. } => by(center),
            Shape::Polyline(line) => line.vertices.iter_mut().for_each(|v| by(&mut v.point)),
            Shape::Text(text) => match &mut text.placement {
                Placement::Anchor { point, .. } => by(point),
                Placement::Between { first, second, .. } => {
                    by(first);
                    by(second);
                }
            },
            Shape::Formatted(text) => by(&mut text.point),
            _ => return None,
        }
        Some(shape)
    }
    fn check<C: Codec>(path: &Path) -> usize
    where
        C::File: Lift<Drawing2D> + Apply,
    {
        let src = std::fs::read(path).unwrap();
        let mut file = testkit::read::<C>(&src).0.unwrap();
        let before = lift(&file);
        let mut edits = Vec::new();
        let mut expected = Vec::new();
        for sheet in &before.sheets {
            let found = sheet
                .elements
                .iter()
                .enumerate()
                .find_map(|(index, element)| Some((index, moved(&element.shape)?)));
            let Some((index, shape)) = found else {
                continue;
            };
            let mut element = sheet.elements[index].clone();
            element.shape = shape;
            edits.push(Edit::ReplaceOnSheet(Box::new(element.clone())));
            expected.push((sheet.id, index, element.shape));
        }
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        file.apply(
            &Changes { edits },
            &mut Cx::new(&mut budget, &mut diag, &cancel),
        )
        .unwrap();
        let after = lift(
            &testkit::read::<C>(&testkit::write::<C>(&file).unwrap())
                .0
                .unwrap(),
        );
        for (id, index, shape) in &expected {
            let sheet = after.sheets.iter().find(|s| s.id == *id).unwrap();
            let found = format!("{:?}", sheet.elements[*index].shape);
            assert!(
                close(&found, &format!("{shape:?}")),
                "{}: {found}",
                path.display()
            );
        }
        // ほかのレイアウトの用紙の要素（BLOCKS のペーパー空間のブロックの中の図形）も除ける
        let written = testkit::write::<C>(&file).unwrap();
        let mut file = testkit::read::<C>(&written).0.unwrap();
        let sheet = after
            .sheets
            .iter()
            .rfind(|sheet| sheet.id.0 != 0 && !sheet.elements.is_empty())
            .unwrap();
        let first = sheet.elements.first().unwrap().source.clone();
        let mut diag = Diagnostics::new();
        file.apply(
            &Changes {
                edits: vec![Edit::Remove(first)],
            },
            &mut Cx::new(&mut budget, &mut diag, &cancel),
        )
        .unwrap();
        let removed = lift(
            &testkit::read::<C>(&testkit::write::<C>(&file).unwrap())
                .0
                .unwrap(),
        );
        let left = removed.sheets.iter().find(|s| s.id == sheet.id).unwrap();
        assert_eq!(
            left.elements.len() + 1,
            sheet.elements.len(),
            "{}",
            path.display()
        );
        // 要素を持つどの用紙にも、線分と円を足せる。足した要素は、その用紙の終わりに並ぶ（決定 0155）
        let mut file = testkit::read::<C>(&src).0.unwrap();
        let mut added = Vec::new();
        for sheet in before
            .sheets
            .iter()
            .filter(|sheet| !sheet.elements.is_empty())
        {
            let at = |x: f64, y: f64| PaperPoint {
                sheet: sheet.id,
                x,
                y,
            };
            let shapes = [
                Shape::Line {
                    start: at(5.0, 5.0),
                    end: at(50.0, 30.0),
                },
                Shape::Circle {
                    center: at(20.0, 20.0),
                    radius: Length::Paper(PaperLength(7.5)),
                },
            ];
            for shape in shapes {
                let mut element = sheet.elements[0].clone();
                element.shape = shape;
                element.style = Style {
                    color: Inherit::Layer,
                    line_type: Inherit::Layer,
                    line_type_scale: 1.0,
                    line_weight: Some(Inherit::Layer),
                };
                added.push(element);
            }
        }
        let edits = added
            .iter()
            .map(|element| Edit::AddOnSheet(Box::new(element.clone())))
            .collect();
        let mut diag = Diagnostics::new();
        let result = file.apply(
            &Changes { edits },
            &mut Cx::new(&mut budget, &mut diag, &cancel),
        );
        assert_eq!(
            result,
            Ok(()),
            "{}: {:?}",
            path.display(),
            diag.iter().map(|d| d.code()).collect::<Vec<_>>()
        );
        let grown = lift(
            &testkit::read::<C>(&testkit::write::<C>(&file).unwrap())
                .0
                .unwrap(),
        );
        for sheet in before
            .sheets
            .iter()
            .filter(|sheet| !sheet.elements.is_empty())
        {
            let found = &grown
                .sheets
                .iter()
                .find(|s| s.id == sheet.id)
                .unwrap()
                .elements;
            assert_eq!(found.len(), sheet.elements.len() + 2, "{}", path.display());
            let mine = added
                .iter()
                .filter(|e| anchor_sheet(&e.shape) == Some(sheet.id));
            for (found, element) in found[sheet.elements.len()..].iter().zip(mine) {
                assert!(
                    close(
                        &format!("{:?}", found.shape),
                        &format!("{:?}", element.shape)
                    ),
                    "{}: {:?}",
                    path.display(),
                    found.shape
                );
            }
        }
        expected.len()
    }
    /// 要素を持たない用紙にも、LAYOUT の範囲の原点で線分を足せる。足すと要素を持つ用紙になり、ID が変わるので、
    /// 名前で引く（決定 0155）。要素を持たない用紙の数を返す
    fn empty_sheets<C: Codec>(path: &Path) -> usize
    where
        C::File: Lift<Drawing2D> + Apply,
    {
        let src = std::fs::read(path).unwrap();
        let mut file = testkit::read::<C>(&src).0.unwrap();
        let before = lift(&file);
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let layer = before
            .layers
            .iter()
            .find(|layer| layer.name == "0")
            .unwrap()
            .id;
        let empty: Vec<_> = before
            .sheets
            .iter()
            .filter(|sheet| sheet.elements.is_empty() && sheet.views.is_empty())
            .filter(|sheet| sheet.name.is_some())
            .collect();
        let line = |sheet| Shape::Line {
            start: PaperPoint {
                sheet,
                x: 5.0,
                y: 5.0,
            },
            end: PaperPoint {
                sheet,
                x: 50.0,
                y: 30.0,
            },
        };
        let edits = empty
            .iter()
            .map(|sheet| {
                Edit::AddOnSheet(Box::new(zumen_drawing::Element {
                    source: zumen_base::Ref::default(),
                    layer,
                    style: Style {
                        color: Inherit::Layer,
                        line_type: Inherit::Layer,
                        line_type_scale: 1.0,
                        line_weight: Some(Inherit::Layer),
                    },
                    shape: line(sheet.id),
                }))
            })
            .collect();
        let mut diag = Diagnostics::new();
        let result = file.apply(
            &Changes { edits },
            &mut Cx::new(&mut budget, &mut diag, &cancel),
        );
        assert_eq!(
            result,
            Ok(()),
            "{}: {:?}",
            path.display(),
            diag.iter().map(|d| d.code()).collect::<Vec<_>>()
        );
        let grown = lift(
            &testkit::read::<C>(&testkit::write::<C>(&file).unwrap())
                .0
                .unwrap(),
        );
        for sheet in &empty {
            let found = grown.sheets.iter().find(|s| s.name == sheet.name).unwrap();
            assert_eq!(
                found.elements.len(),
                1,
                "{}: {:?}",
                path.display(),
                sheet.name
            );
            assert!(
                close(
                    &format!("{:?}", found.elements[0].shape),
                    &format!("{:?}", line(found.id))
                ),
                "{}: {:?}",
                path.display(),
                found.elements[0].shape
            );
        }
        empty.len()
    }
    /// 線分と円の点が置かれた用紙
    fn anchor_sheet(shape: &Shape<PaperPoint>) -> Option<zumen_drawing::SheetId> {
        match shape {
            Shape::Line { start, .. } => Some(start.sheet),
            Shape::Circle { center, .. } => Some(center.sheet),
            _ => None,
        }
    }
    // 用紙に要素を持つ試料。farmhouse.dwg は今のレイアウトとほかの 25 のレイアウトに、features-r2018.dxf は
    // ほかのレイアウト（A3-PLAN）に要素を持つ
    let Some(dir) = std::env::var_os("ZUMEN_SAMPLES").filter(|d| !d.is_empty()) else {
        return;
    };
    let dir = PathBuf::from(dir);
    let mut sheets = 0;
    let farmhouse = dir.join("farmhouse.dwg");
    if farmhouse.exists() {
        sheets += check::<Dwg>(&farmhouse);
        assert!(sheets >= 26, "{sheets}");
    }
    for name in ["features-r2018.dxf", "features-r2018-binary.dxf"] {
        let path = dir.join(name);
        if path.exists() {
            assert_eq!(check::<Dxf>(&path), 1, "{}", path.display());
            assert_eq!(empty_sheets::<Dxf>(&path), 1, "{}", path.display());
        }
    }
    // 要素を持たない 2 つのレイアウトを持つ DWG
    for name in [
        "line_2000.dwg",
        "arc_2000.dwg",
        "text_2000.dwg",
        "mtext_2000.dwg",
        "polyline2d_line_2000.dwg",
        "BLOCKPOINTPARAMETER.dwg",
        "jp-yokosuka-minami-survey-plan.dwg",
    ] {
        let path = dir.join(name);
        if path.exists() {
            assert_eq!(empty_sheets::<Dwg>(&path), 2, "{}", path.display());
        }
    }
}

/// 空間・記号の定義の空間・用紙ごとの、要素の ID（決定 0156）
#[cfg(feature = "drawing")]
fn element_ids(drawing: &zumen_drawing::Drawing2D) -> Vec<Vec<String>> {
    use zumen_drawing::id;
    fn named<P>(place: &str, elements: &[zumen_drawing::Element<P>]) -> Vec<String> {
        let mut namer = id::Namer::new(place);
        let ids = elements.iter().enumerate();
        ids.map(|(index, element)| namer.next(index, element))
            .collect()
    }
    let spaces = drawing
        .spaces
        .iter()
        .map(|space| named(&id::space(space.id), &space.elements));
    // 名前のない定義（*U・*D など）は、CAD が保存するたびに作り直し、ハンドルも変わりうるので比べない
    let mut definition_namer = id::DefinitionNamer::default();
    let definitions = drawing
        .symbols
        .iter()
        .map(|definition| (definition, definition_namer.next(definition)))
        .filter(|(definition, _)| !definition.anonymous)
        .map(|(definition, place)| named(&place, &definition.space.elements));
    let sheets = drawing
        .sheets
        .iter()
        .map(|sheet| named(&id::sheet(sheet), &sheet.elements));
    spaces.chain(definitions).chain(sheets).collect()
}

/// 記号の定義の空間・レイヤ・線種の ID（決定 0156）
#[cfg(feature = "drawing")]
fn table_ids(drawing: &zumen_drawing::Drawing2D) -> [Vec<String>; 3] {
    use zumen_drawing::id;
    let mut definitions = id::DefinitionNamer::default();
    [
        drawing
            .symbols
            .iter()
            .map(|definition| (definition, definitions.next(definition)))
            .filter(|(definition, _)| !definition.anonymous)
            .map(|(_, id)| id)
            .collect(),
        // 注釈尺度を分けて書いたレイヤ（`0 @ 1` など）も、保存するたびに作り直す
        drawing
            .layers
            .iter()
            .filter(|layer| !layer.name.contains(" @ "))
            .map(id::layer)
            .collect(),
        drawing.line_types.iter().map(id::line_type).collect(),
    ]
}

#[test]
#[cfg(feature = "drawing")]
#[allow(
    clippy::disallowed_methods,
    reason = "環境変数を読むのは試験だけで、コアは読まない（AGENTS.md の図面と秘密）"
)]
fn dwg_and_dxf_of_the_same_drawing_give_the_same_element_ids() {
    use zumen_drawing::id;
    use zumen_sdk::Lift;
    let lift = |path: &Path| {
        let src = std::fs::read(path).unwrap();
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        if path.extension().is_some_and(|e| e == "dwg") {
            Dwg::read(&src, &mut cx).unwrap().lift(&mut cx).unwrap()
        } else {
            Dxf::read(&src, &mut cx).unwrap().lift(&mut cx).unwrap()
        }
    };
    let mut found = pairs();
    if let Some(dir) = std::env::var_os("ZUMEN_SAMPLES").filter(|d| !d.is_empty()) {
        let dwg = Path::new(&dir).join("jp-yokosuka-minami-survey-plan.dwg");
        if dwg.exists() {
            found.push((dwg.clone(), dwg.with_extension("dxf")));
        }
    }
    for (dwg, dxf) in found {
        let (ours, theirs) = (lift(&dwg), lift(&dxf));
        let ids = element_ids(&ours);
        assert_eq!(ids, element_ids(&theirs), "{}", dwg.display());
        // どの要素もハンドルで指し、レコードの番号やバイトの位置で指さない
        let keyed = |id: &String| id.rsplit('/').any(|part| part.starts_with('h'));
        assert!(ids.iter().flatten().all(keyed), "{}", dwg.display());
        // 記号の定義の空間・レイヤ・線種も、同じ ID になり、定義を持つものはハンドルで指す
        let tables = table_ids(&ours);
        assert_eq!(tables, table_ids(&theirs), "{}", dwg.display());
        let mut definitions = id::DefinitionNamer::default();
        let defined = ours
            .symbols
            .iter()
            .map(|d| (&d.source, definitions.next(d)));
        let layers = ours.layers.iter().map(|l| (&l.source, id::layer(l)));
        let line_types = ours
            .line_types
            .iter()
            .map(|l| (&l.source, id::line_type(l)));
        for (source, id) in defined.chain(layers).chain(line_types) {
            if *source != zumen_base::Ref::default() {
                assert!(id.contains("/h"), "{id} {}", dwg.display());
            }
        }
        // 用紙は、LAYOUT のハンドルで指す
        for sheet in &ours.sheets {
            let id = zumen_drawing::id::sheet(sheet);
            assert!(id.starts_with("sheet/h"), "{id} {}", dwg.display());
        }
    }
}
