//! 共有のオブジェクトの表の変更を、DXF のレコードに書き戻す（決定 0024）。
//!
//! 表と、今のファイルから作り直した表を、元のレコード（[`Origin::Dxf`]）ごとに比べる。値の並びが違う
//! レコードだけを、1 つの [`DxfTxn`] で直す。並びの前と後ろで同じ値は、元のタグのまま残す。間の値は、
//! グループ・コードの並びが同じなら値だけを差し替え、違えばタグを除いて足し直す。見出しの変数も、
//! 変数の名前ごとに同じように比べる。
//!
//! 表から除いたオブジェクトのレコードは除く。表に足したオブジェクト（[`Origin::New`]）は、同じ所の後ろで
//! 最初の残したオブジェクトの前に、なければその所の ENDSEC の前に、新しいレコードとして足す（決定 0025）。
//! 表のハンドルのどれかが `$HANDSEED` 以上なら、`$HANDSEED` を最大のハンドルの次に進める。値を変えたか
//! 足した図形が `$EXTMIN`・`$EXTMAX` の外にあれば、範囲を広げる。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Cx, Diagnostic, codes};
use zumen_sdk::{Error, Transact, Transaction};

use super::{DxfFile, DxfTxn, format_real};
use crate::db::{AcadDb, Data, Object, Origin, Section, Value};

impl DxfFile {
    /// 表 `db` のオブジェクトの値と見出しの変数の変更を、このファイルに書き戻す。`db` は、このファイルから
    /// 作った表を変えたもの。変えていないレコードは、元のバイトのまま残る。1 つでも書き戻せない変更が
    /// あれば、何も反映しない。今は、変数の削除は扱わない
    pub fn write_back(&mut self, db: &AcadDb, cx: &mut Cx) -> Result<(), Error> {
        let base = self.table(cx, false)?;
        let header = self.header_variables();
        let ends = self.section_ends();
        let mut txn = self.transaction();
        let staged = stage(&mut txn, db, &base, header, &ends);
        // 失敗した変更があれば、確定が最初の失敗を診断として返し、何も反映しない
        let committed = txn.commit(cx);
        staged.and(committed)
    }

    /// 所ごとの、最初の ENDSEC のレコードの番号
    fn section_ends(&self) -> Vec<(Section, usize)> {
        let mut ends: Vec<(Section, usize)> = Vec::new();
        let mut section = None;
        for (index, record) in self.records.iter().enumerate() {
            match self.kind(record) {
                b"SECTION" => section = Some(self.section(record)),
                b"ENDSEC" => {
                    if let Some(found) = section.take()
                        && !ends.iter().any(|(s, _)| *s == found)
                    {
                        ends.push((found, index));
                    }
                }
                b"EOF" => break,
                _ => {}
            }
        }
        ends
    }

    /// HEADER のレコードの番号と、変数ごとの値のタグの範囲（名前のタグの次から）。同じ名前が 2 度あれば、
    /// 最初のもの（表を作るときと同じ）
    fn header_variables(&self) -> Option<Header> {
        let record = self.records.iter().position(|r| {
            self.kind(r) == b"SECTION" && self.section(r) == Section::Other(b"HEADER".to_vec())
        })?;
        let tags = &self.records.get(record)?.tags;
        let mut variables = BTreeMap::new();
        let mut start = 0;
        for chunk in tags.chunk_by(|_, b| b.code != 9) {
            if let Some(head) = chunk.first().filter(|head| head.code == 9) {
                let name = self.value(head).trim_ascii().to_vec();
                variables.entry(name).or_insert(start + 1);
            }
            start += chunk.len();
        }
        Some(Header {
            record,
            end: tags.len(),
            variables,
        })
    }
}

/// HEADER のレコードの番号と、変数の値の始まりのタグの番号
struct Header {
    record: usize,
    /// タグの数
    end: usize,
    variables: BTreeMap<Vec<u8>, usize>,
}

fn stage(
    txn: &mut DxfTxn<'_>,
    db: &AcadDb,
    base: &AcadDb,
    header: Option<Header>,
    ends: &[(Section, usize)],
) -> Result<(), Error> {
    let before: BTreeMap<usize, &Object> = base
        .objects()
        .iter()
        .filter_map(|o| match o.origin {
            Origin::Dxf { record } => Some((record, o)),
            Origin::Dwg { .. } | Origin::New => None,
        })
        .collect();
    // 表のオブジェクトごとに、直すレコード。足したものは `None`
    let mut records = Vec::with_capacity(db.objects().len());
    // 値を変えたか足したオブジェクト。図面の範囲を広げるのに使う
    let mut touched = Vec::new();
    let mut kept = BTreeSet::new();
    for object in db.objects() {
        let record = match object.origin {
            Origin::New => None,
            Origin::Dxf { record } => Some(record).filter(|record| {
                before
                    .get(record)
                    .is_some_and(|o| o.kind == object.kind && o.section == object.section)
                    && kept.insert(*record)
            }),
            Origin::Dwg { .. } => None,
        };
        if record.is_none() && object.origin != Origin::New {
            return Err(unsupported(txn, object));
        }
        records.push(record);
    }
    // 表から除いたオブジェクトのレコードを除く
    for &record in before.keys().filter(|r| !kept.contains(r)) {
        txn.remove_record(record)?;
    }
    for (index, (object, record)) in db.objects().iter().zip(&records).enumerate() {
        match record {
            Some(record) => {
                let Some(old) = before.get(record) else {
                    return Err(unsupported(txn, object));
                };
                if old.values != object.values {
                    touched.push(object);
                }
                // 値の並びはグループ・コード 0 のタグの次から
                replace(txn, *record, 1, &old.values, &object.values)?;
            }
            None => {
                // 同じ所の、後ろで最初の残したオブジェクトの前に足す。なければ、その所の ENDSEC の前
                let next = db
                    .objects()
                    .iter()
                    .zip(&records)
                    .skip(index + 1)
                    .find_map(|(o, r)| r.filter(|_| o.section == object.section));
                let end = ends
                    .iter()
                    .find(|(section, _)| *section == object.section)
                    .map(|&(_, end)| end);
                let Some(at) = next.or(end) else {
                    return Err(unsupported(txn, object));
                };
                let mut values = Vec::with_capacity(object.values.len());
                for value in &object.values {
                    values.push((value.code, encode(txn, at, value)?));
                }
                let tags: Vec<(i16, &[u8])> = core::iter::once((0, object.kind.as_slice()))
                    .chain(values.iter().map(|(code, bytes)| (*code, bytes.as_slice())))
                    .collect();
                txn.insert_record(at, &tags)?;
                touched.push(object);
            }
        }
    }
    let mut fixed = extents(db, &touched);
    fixed.extend(handle_seed(db).map(|seed| (HANDSEED, seed)));
    let changed: Vec<_> = db
        .variables()
        .map(
            |(name, values)| match fixed.iter().find(|(fixed, _)| *fixed == name) {
                Some((_, fixed)) => (name, fixed.as_slice()),
                None => (name, values),
            },
        )
        .filter(|&(name, values)| base.variable(name) != Some(values))
        .collect();
    if changed.is_empty() {
        return Ok(());
    }
    let Some(header) = header else {
        return Err(
            txn.reject(Diagnostic::new(codes::DXF_OBJECT_UNSUPPORTED).with("kind", "HEADER"))
        );
    };
    // 新しい変数は最後に足す。後ろから直すと、前の変数のタグの番号は変わらない
    let mut end = header.end;
    for &(name, values) in &changed {
        if !header.variables.contains_key(name) {
            txn.insert_tag(header.record, end, 9, name)?;
            end += 1;
            replace(txn, header.record, end, &[], values)?;
            end += values.len();
        }
    }
    let mut known: Vec<_> = changed
        .iter()
        .filter_map(|&(name, values)| {
            let start = *header.variables.get(name)?;
            Some((start, base.variable(name).unwrap_or_default(), values))
        })
        .collect();
    known.sort_by_key(|&(start, ..)| core::cmp::Reverse(start));
    for (start, old, new) in known {
        replace(txn, header.record, start, old, new)?;
    }
    Ok(())
}

const HANDSEED: &[u8] = b"$HANDSEED";
const EXTMIN: &[u8] = b"$EXTMIN";
const EXTMAX: &[u8] = b"$EXTMAX";

/// 値を変えたか足したモデル空間の図形が `$EXTMIN`・`$EXTMAX` の外にあれば、図形を含むように広げた
/// 変数の値（DXF Reference「HEADER Section Group Codes」の $EXTMIN・$EXTMAX）。範囲は狭めない。
/// 囲む箱を値から決められる図形（LINE・POINT と、押し出しの向きが既定の CIRCLE）だけを見る。
/// 表に変数がないか、10・20・30 の実数を持たなければ、直さない
fn extents(db: &AcadDb, touched: &[&Object]) -> Vec<(&'static [u8], Vec<Value>)> {
    let Some(found) = touched
        .iter()
        .filter_map(|o| bounds(o))
        .reduce(|[a0, a1], [b0, b1]| [min3(a0, b0), max3(a1, b1)])
    else {
        return Vec::new();
    };
    let mut fixed = Vec::new();
    for (name, corner, pick) in [
        (EXTMIN, found[0], min3 as fn([f64; 3], [f64; 3]) -> [f64; 3]),
        (EXTMAX, found[1], max3),
    ] {
        let Some(values) = db.variable(name) else {
            continue;
        };
        let Some(old) = point(values, 10) else {
            continue;
        };
        let new = pick(old, corner);
        if new != old {
            let mut values = values.to_vec();
            for value in &mut values {
                if let Some(axis) = [10, 20, 30].iter().position(|&code| code == value.code)
                    && let Some(&v) = new.get(axis)
                {
                    value.data = Data::Real(v);
                }
            }
            fixed.push((name, values));
        }
    }
    fixed
}

/// モデル空間の図形を囲む箱の、最小と最大の角（WCS）
fn bounds(object: &Object) -> Option<[[f64; 3]; 2]> {
    let paper = matches!(object.first(67), Some(Data::Int(1)));
    if object.section != Section::Entities || paper {
        return None;
    }
    let corners = |a: [f64; 3], b: [f64; 3]| Some([min3(a, b), max3(a, b)]);
    match object.kind.as_slice() {
        b"LINE" => corners(point(&object.values, 10)?, point(&object.values, 11)?),
        b"POINT" => {
            let at = point(&object.values, 10)?;
            corners(at, at)
        }
        // 中心は OCS なので、押し出しの向きが既定（0,0,1）のときだけ WCS と同じになる
        b"CIRCLE" if extrusion(object)? == [0.0, 0.0, 1.0] => {
            let [x, y, z] = point(&object.values, 10)?;
            let r = object.real(40)??.abs();
            corners([x - r, y - r, z], [x + r, y + r, z])
        }
        _ => None,
    }
}

/// 押し出しの向き（210・220・230）。なければ既定の（0,0,1）
fn extrusion(object: &Object) -> Option<[f64; 3]> {
    if object.first(210).is_none() {
        return Some([0.0, 0.0, 1.0]);
    }
    point(&object.values, 210)
}

/// `code`・`code + 10`・`code + 20` の最初の実数を点として読む。Z がなければ 0 とする。
/// 有限の実数でなければ `None`
fn point(values: &[Value], code: i16) -> Option<[f64; 3]> {
    let real = |code: i16| {
        values
            .iter()
            .find(|v| v.code == code)
            .map(|v| match v.data {
                Data::Real(r) if r.is_finite() => Some(r),
                _ => None,
            })
    };
    Some([
        real(code)??,
        real(code + 10)??,
        real(code + 20).unwrap_or(Some(0.0))?,
    ])
}

fn min3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])]
}

fn max3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]
}

/// 表のハンドルのどれかが `$HANDSEED` 以上なら、最大のハンドルの次にした `$HANDSEED` の値。
/// `$HANDSEED` は、次に払い出すハンドルで、どのハンドルよりも大きい（DXF Reference「HEADER Section
/// Group Codes」の $HANDSEED）。表に `$HANDSEED` がなければ、足さない
fn handle_seed(db: &AcadDb) -> Option<Vec<Value>> {
    let values = db.variable(HANDSEED)?;
    let (at, seed) = values.iter().enumerate().find_map(|(i, v)| match v.data {
        Data::Handle(seed) if v.code == 5 => Some((i, seed)),
        _ => None,
    })?;
    let max = db.objects().iter().filter_map(Object::handle).max()?;
    if max < seed {
        return None;
    }
    let mut fixed = values.to_vec();
    let value = fixed.get_mut(at)?;
    value.data = Data::Handle(max.checked_add(1)?);
    Some(fixed)
}

/// 書き戻せないオブジェクトを診断にする
fn unsupported(txn: &mut DxfTxn<'_>, object: &Object) -> Error {
    let kind = String::from_utf8_lossy(&object.kind).into_owned();
    txn.reject(Diagnostic::new(codes::DXF_OBJECT_UNSUPPORTED).with("kind", kind))
}

/// `record` の `offset` 番目のタグから並ぶ値 `old` を、`new` に直す
fn replace(
    txn: &mut DxfTxn<'_>,
    record: usize,
    offset: usize,
    old: &[Value],
    new: &[Value],
) -> Result<(), Error> {
    let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let (old, new) = (
        old.get(prefix..).unwrap_or_default(),
        new.get(prefix..).unwrap_or_default(),
    );
    let suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let old = old.get(..old.len() - suffix).unwrap_or_default();
    let new = new.get(..new.len() - suffix).unwrap_or_default();
    let at = offset + prefix;
    let same_codes = old.len() == new.len() && old.iter().zip(new).all(|(a, b)| a.code == b.code);
    if same_codes {
        for (i, (a, b)) in old.iter().zip(new).enumerate() {
            if a != b {
                let value = encode(txn, record, b)?;
                txn.set_value(record, at + i, &value)?;
            }
        }
        return Ok(());
    }
    for i in (0..old.len()).rev() {
        txn.remove_tag(record, at + i)?;
    }
    for (i, value) in new.iter().enumerate() {
        let bytes = encode(txn, record, value)?;
        txn.insert_tag(record, at + i, value.code, &bytes)?;
    }
    Ok(())
}

/// 値を DXF の値の行にする（DXF Reference「Group Code Value Types」）。表を作るときの読み方の逆
fn encode(txn: &mut DxfTxn<'_>, record: usize, value: &Value) -> Result<Vec<u8>, Error> {
    Ok(match &value.data {
        Data::Text(text) => text.clone(),
        Data::Int(int) => int.to_string().into_bytes(),
        Data::Real(real) if real.is_finite() => format_real(*real),
        Data::Real(_) => {
            return Err(txn.reject(super::on_tag(
                codes::DXF_REAL_UNSUPPORTED,
                record,
                value.code,
            )));
        }
        Data::Bool(flag) => if *flag { b"1" } else { b"0" }.to_vec(),
        Data::Handle(handle) => format!("{handle:X}").into_bytes(),
        Data::Binary(bytes) => bytes
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<String>()
            .into_bytes(),
    })
}
