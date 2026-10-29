//! 線種の置き換え・追加・除去を、線種の表のレコード（LTYPE）に書く（決定 0164）。
//!
//! 置き換えは、元のレコードの値のうち、名前（2）・説明（3）・要素の数（73）・全体の長さ（40）・要素の長さ
//! （49）だけを書き直す。形か文字を持つレコードと、DWG の今あるレコードは、要素の数を変えず、要素の長さだけを
//! 順に書き直す。それ以外は、要素の値（49 と 74 から 9 まで）を除いてから、形も文字もない要素として書き直す。
//! 追加は、Continuous のレコードを写し、ハンドル・拡張辞書・拡張データと要素を除いてから同じ欄を書く。名前を
//! 変えたら、DXF では、図形と画層のレコードの線種の名前（6）と、見出しの変数の線種の名前（6。`$CELTYPE`）も
//! 書き直す。DWG の図形と画層は線種をハンドルで指すので、足した図形と画層のほかは書き直さない

use super::add::{Born, Target, int};
use super::layer::{external, find, set_in, usable_name};
use super::{count_transform_values, real, work};
use crate::db::{AcadDb, Data, Object, Origin, Section, Value};
use zumen_base::{Cx, Ref};
use zumen_drawing::{LineType, PatternBasis};
use zumen_sdk::Error;

/// 欄を書く順。ない欄は、この順で前にある欄の後に足す
const ORDER: [i16; 6] = [2, 70, 3, 72, 73, 40];

/// 要素ごとの値のグループ・コード
const DASH_CODES: [i16; 9] = [49, 74, 75, 340, 46, 50, 44, 45, 9];

/// DWG に足すレコードが持てる欄（決定 0094）
const DWG_CODES: [i16; 7] = [330, 2, 70, 3, 72, 73, 40];

/// 線種の表のレコードか
pub(super) fn is_line_type(object: &Object) -> bool {
    object.section == Section::Tables && object.kind == b"LTYPE"
}

/// 名前を変えられず、除けない線種（ByLayer・ByBlock・Continuous）か
pub(super) fn fixed_name(name: &[u8]) -> bool {
    [b"ByLayer".as_slice(), b"ByBlock", b"Continuous"]
        .iter()
        .any(|fixed| name.eq_ignore_ascii_case(fixed))
}

/// 足すレコードの元にする値。Continuous のレコードを写し、ハンドル（5）・拡張辞書と反応子の組（102）・
/// 拡張データ（1000 から）・要素の値を除く。DWG は、足せる欄だけを残す
pub(super) fn template(object: &Object, target: Target) -> Vec<Value> {
    let mut values = Vec::new();
    let mut depth = 0usize;
    for value in &object.values {
        match (value.code, &value.data) {
            (102, Data::Text(text)) if text.starts_with(b"{") => depth += 1,
            (102, Data::Text(text)) if text.starts_with(b"}") => depth = depth.saturating_sub(1),
            (code, _) if depth > 0 || code == 5 || code >= 1000 || DASH_CODES.contains(&code) => {}
            (code, _) if matches!(target, Target::Dwg(_)) && !DWG_CODES.contains(&code) => {}
            _ => values.push(value.clone()),
        }
    }
    values
}

/// レコードの値 `values` に、線種 `line_type` の名前と定義を書く。`was` は置き換える元のレコードと線種
/// （足すなら `None`）。書けない値なら `None`
pub(super) fn write(
    born: &Born,
    values: &mut Vec<Value>,
    line_type: &LineType,
    was: Option<(&Object, &LineType)>,
    target: Target,
) -> Option<()> {
    let definition = line_type.definition.as_ref()?;
    usable_name(&line_type.name).then_some(())?;
    // 列の長さは要素の空間の長さだけを書ける。形と文字は変えられない
    (definition.basis == PatternBasis::Element
        && definition.pattern.iter().all(|length| length.is_finite()))
    .then_some(())?;
    let old = was.and_then(|(_, old)| old.definition.as_ref());
    (old.map_or(&[][..], |old| &old.marks[..]) == definition.marks.as_slice()).then_some(())?;
    let name = born.text_value(2, &line_type.name)?;
    if let Some((record, _)) = was
        && record.first(2) != Some(&name.data)
    {
        // ByLayer・ByBlock・Continuous と、外部参照に依る線種は、名前を変えない
        let Some(Data::Text(old)) = record.first(2) else {
            return None;
        };
        (!fixed_name(old) && !external(record)).then_some(())?;
    }
    set(values, 2, Some(name));
    set(
        values,
        3,
        Some(born.text_value(3, &definition.description)?),
    );
    let total = definition.pattern.iter().map(|length| length.abs()).sum();
    let count = i64::try_from(definition.pattern.len()).ok()?;
    set(values, 73, Some(int(73, count)));
    set(values, 40, Some(real(40, total)?));
    // モデルに現れない値も捨てない。要素の付加値か 0 でない印があれば、長さだけを書き直す。
    let marked = values.iter().any(|value| {
        matches!(value.code, 75 | 340 | 46 | 50 | 44 | 45 | 9)
            || (value.code == 74 && !matches!(value.data, Data::Int(0)))
    });
    let dashes = values.iter().filter(|value| value.code == 49).count();
    if marked || (was.is_some() && matches!(target, Target::Dwg(_))) {
        (dashes == definition.pattern.len()).then_some(())?;
        let mut lengths = definition.pattern.iter();
        for value in values.iter_mut().filter(|value| value.code == 49) {
            *value = real(49, *lengths.next()?)?;
        }
        return Some(());
    }
    // R13 からの DXF（区切りの 100 を持つ）と DWG は、要素ごとに形と文字の印（74）を書く
    let flagged = matches!(target, Target::Dwg(_))
        || values.iter().any(|value| matches!(value.code, 74 | 100));
    values.retain(|value| !DASH_CODES.contains(&value.code));
    let mut written = Vec::with_capacity(definition.pattern.len() * 2);
    for length in &definition.pattern {
        written.push(real(49, *length)?);
        if flagged {
            written.push(int(74, 0));
        }
    }
    // 要素は、拡張データ（1000 から）の前に置く
    let at = values
        .iter()
        .position(|value| value.code >= 1000)
        .unwrap_or(values.len());
    values.splice(at..at, written);
    Some(())
}

fn set(values: &mut Vec<Value>, code: i16, value: Option<Value>) {
    set_in(values, code, value, &ORDER);
}

/// 線種を名前（6）で指すか
fn names(value: &Value, name: &[u8]) -> bool {
    value.code == 6 && matches!(&value.data, Data::Text(text) if text.eq_ignore_ascii_case(name))
}

/// 線種の名前（6）を書き直すオブジェクトか。図形と画層のレコード
fn naming(object: &Object) -> bool {
    matches!(object.section, Section::Entities | Section::Blocks)
        || (object.section == Section::Tables && object.kind == b"LAYER")
}

/// DXF の図形と画層のレコードでないオブジェクト（多重線のスタイルなど）が、線種を名前（6）で指すか
pub(super) fn named_elsewhere(
    db: &AcadDb,
    name: &[u8],
    cx: &mut Cx,
    at: &Ref,
) -> Result<bool, Error> {
    for object in db.objects() {
        work(cx, 1, at)?;
        if !naming(object) {
            count_transform_values(object, at, cx)?;
            if object.values.iter().any(|value| names(value, name)) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// 名前を変えた線種 `old` を、名前 `new` で指すように書き直す。DXF は、図形と画層のレコードの線種（6）と、
/// 見出しの変数の線種（6）を書き直す。DWG は、足した図形と画層の線種だけを書き直す。ほかのオブジェクトが
/// 名前で指せば、呼ぶ側が先に拒む（[`named_elsewhere`]）
pub(super) fn rename(
    db: &mut AcadDb,
    old: &[u8],
    new: &Value,
    target: Target,
    cx: &mut Cx,
) -> Result<(), Error> {
    let at = Ref::default();
    for position in 0..db.objects().len() {
        let Some(object) = db.object_mut(position) else {
            continue;
        };
        work(cx, 1, &at)?;
        count_transform_values(object, &at, cx)?;
        let rewrite = naming(object)
            && match target {
                Target::Dxf => true,
                Target::Dwg(_) => object.origin == Origin::New,
            };
        if !rewrite {
            continue;
        }
        for value in &mut object.values {
            if names(value, old) {
                if let Data::Text(text) = &new.data {
                    work(cx, 1 + text.len() as u64 / 1024, &at)?;
                }
                value.data = new.data.clone();
            }
        }
    }
    if target != Target::Dxf {
        return Ok(());
    }
    for (_, values) in db.variables() {
        for value in values {
            let bytes = match &value.data {
                Data::Text(bytes) | Data::Binary(bytes) => bytes.len(),
                _ => 0,
            };
            work(cx, 1 + bytes as u64 / 1024, &at)?;
        }
    }
    let renamed: Vec<_> = db
        .variables()
        .filter(|(_, values)| values.iter().any(|value| names(value, old)))
        .map(|(name, values)| {
            let values = values
                .iter()
                .map(|value| match names(value, old) {
                    true => Value {
                        code: value.code,
                        data: new.data.clone(),
                    },
                    false => value.clone(),
                })
                .collect::<Vec<_>>();
            (name.to_vec(), values)
        })
        .collect();
    for (name, values) in renamed {
        db.set_variable(&name, values);
    }
    Ok(())
}

/// 除く線種のレコード `record`（表の `position`）を、ほかが指しているか。図形と画層の線種の名前（6）、
/// ほかのオブジェクトのハンドル（DWG の図形と画層・寸法スタイルの線種など）、見出しの変数を見る。DWG の
/// 表のオブジェクト（TABLE）の項目の並びは、共有の表に置かれず、書き戻しが直す
pub(super) fn referenced(
    db: &AcadDb,
    position: usize,
    record: &Object,
    cx: &mut Cx,
    place: &Ref,
) -> Result<bool, Error> {
    let Some(Data::Text(name)) = record.first(2) else {
        return Ok(true);
    };
    let handle = record.handle();
    let points = |value: &Value| match &value.data {
        Data::Handle(found) => handle == Some(*found),
        _ => names(value, name),
    };
    for (at, object) in db.objects().iter().enumerate() {
        work(cx, 1, place)?;
        count_transform_values(object, place, cx)?;
        if at != position && object.values.iter().any(points) {
            return Ok(true);
        }
    }
    for (_, values) in db.variables() {
        for value in values {
            let bytes = match &value.data {
                Data::Text(bytes) | Data::Binary(bytes) => bytes.len(),
                _ => 0,
            };
            work(cx, 1 + bytes as u64 / 1024, place)?;
        }
        if values.iter().any(points) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// 除ける線種のレコードか。ByLayer・ByBlock・Continuous・外部参照に依るものと、ハンドルを持たない
/// レコードと、拡張辞書か反応子を持つレコードは除かない
pub(super) fn removable(record: &Object) -> bool {
    matches!(record.first(2), Some(Data::Text(name)) if !fixed_name(name))
        && !external(record)
        && record.handle().is_some_and(|handle| handle != 0)
        && !record
            .values
            .iter()
            .any(|value| matches!(value.code, 102 | 360))
}

/// 足した図形と画層のうち、名前 `name` の線種を指すものがあるか
pub(super) fn named_by_new(db: &AcadDb, name: &[u8], cx: &mut Cx) -> Result<bool, Error> {
    let at = Ref::default();
    for object in db.objects() {
        work(cx, 1, &at)?;
        if object.origin == Origin::New && naming(object) {
            count_transform_values(object, &at, cx)?;
            if object.values.iter().any(|value| names(value, name)) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// 組んだ値の名前（2）
pub(super) fn value_name(values: &[Value]) -> Option<&[u8]> {
    match find(values, 2) {
        Some(Data::Text(name)) => Some(name),
        _ => None,
    }
}
